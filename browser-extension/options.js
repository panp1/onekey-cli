import { initI18n, t, setText, showError, UIError } from "./i18n.js";
import { safeOrigin, originsOf, approvalOf, approvalKey } from "./policy.js";
import {
  hasAllHttpsAccess,
  setAllHttpsAccess,
  removeUnusedHostPermissions,
} from "./host-permissions.js";
const $ = (id) => document.getElementById(id);
const status = $("status");
let bindings = [],
  projects = [];
let editing = null;
let editingAiApproved = false;
await initI18n();
const allHttps = $("allHttps");
const accessStatus = $("accessStatus");
let changingAccess = false;
async function refreshAccess() {
  allHttps.checked = await hasAllHttpsAccess();
}
allHttps.addEventListener("change", async () => {
  const enabled = allHttps.checked;
  changingAccess = true;
  allHttps.disabled = true;
  let readable = true;
  try {
    const changed = await setAllHttpsAccess(enabled);
    await refreshAccess();
    setText(
      accessStatus,
      changed
        ? allHttps.checked
          ? "allHttpsGranted"
          : "allHttpsRevoked"
        : "accessDenied",
    );
  } catch {
    setText(accessStatus, "accessFailed");
    try {
      await refreshAccess();
    } catch {
      readable = false;
    }
  } finally {
    changingAccess = false;
    allHttps.disabled = !readable;
  }
});
const accessChanged = () => {
  if (!changingAccess)
    refreshAccess()
      .then(() =>
        setText(
          accessStatus,
          allHttps.checked ? "allHttpsGranted" : "allHttpsRevoked",
        ),
      )
      .catch(() => setText(accessStatus, "accessFailed"));
};
chrome.permissions.onAdded.addListener(accessChanged);
chrome.permissions.onRemoved.addListener(accessChanged);
try {
  await refreshAccess();
  allHttps.disabled = false;
} catch {
  setText(accessStatus, "accessFailed");
}
async function send(message) {
  const response = await chrome.runtime.sendMessage(message);
  if (!response?.ok)
    throw response?.error
      ? new Error(response.error)
      : new UIError("bridgeUnavailable");
  return response;
}
function options(element, items, selected = "") {
  const placeholder = new Option("", "");
  setText(placeholder, "choose");
  element.replaceChildren(placeholder);
  for (const [value, text] of items) element.append(new Option(text, value));
  element.value = selected;
}
async function keys(username = "", password = "") {
  if (!$("project").value) {
    options($("usernameKey"), []);
    options($("passwordKey"), []);
    return;
  }
  let response;
  try {
    response = await send({ action: "catalog", project: $("project").value });
  } catch (error) {
    if (!username || !password) throw error;
    response = { keys: [...new Set([username, password])] };
    setText(status, "metadataOffline");
  }
  const items = response.keys.map((key) => [key, key]);
  options($("usernameKey"), items, username);
  options($("passwordKey"), items, password);
}
async function load() {
  bindings = (await send({ action: "list" })).bindings;
  options(
    $("bindings"),
    bindings.map((binding) => [
      binding.name,
      t(binding.enabled ? "accountEnabled" : "accountDisabled", {
        name: binding.name,
      }),
    ]),
    editing?.name,
  );
  for (const option of $("bindings").options) {
    const binding = bindings.find((b) => b.name === option.value);
    if (binding)
      setText(option, binding.enabled ? "accountEnabled" : "accountDisabled", {
        name: binding.name,
      });
  }
}
async function edit(binding) {
  editing = binding || null;
  editingAiApproved = false;
  $("editor").reset();
  $("name").disabled = !!binding;
  setText($("title"), binding ? "editAccount" : "newWebsiteAccount");
  $("remove").disabled = !binding;
  for (const key of ["name", "usernameSelector", "passwordSelector"])
    $(key).value = binding?.[key] || "";
  $("website").value = binding?.origin || "";
  $("enabled").checked = binding?.enabled !== false;
  $("allowAi").checked = binding?.allowAi || false;
  $("allowJs").checked = binding?.allowJs || false;
  if (binding?.allowAi) {
    const key = approvalKey(binding.name);
    editingAiApproved =
      (await chrome.storage.local.get(key))[key] === approvalOf(binding);
  }
  for (const key of ["loginOrigins", "submitOrigins"])
    $(key).value = (binding?.[key] || []).join("\n");
  $("project").value = binding?.project || "";
  // Old terminal bindings may contain a project name rather than ID.
  if (binding && !$("project").value)
    $("project").value =
      projects.find((p) => p.name === binding.project)?.id || "";
  if (binding && !$("project").value) {
    $("project").append(new Option(binding.project, binding.project));
    $("project").value = binding.project;
  }
  await keys(binding?.usernameKey, binding?.passwordKey);
}
function origin(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new UIError("invalidOrigin");
  }
  if (url.pathname !== "/" || url.search || url.hash)
    throw new UIError("invalidOrigin");
  return safeOrigin(value);
}
const lines = (id) => [
  ...new Set(
    $(id)
      .value.split(/\n/)
      .map((s) => s.trim())
      .filter(Boolean)
      .map(origin),
  ),
];
$("editor").addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    const binding = {
      // Account configuration is shared. MCP selects a browser for each request.
      browser: null,
      name: $("name").value.trim(),
      serverUrl: "",
      origin: origin($("website").value.trim()),
      project: $("project").value,
      usernameKey: $("usernameKey").value,
      passwordKey: $("passwordKey").value,
      usernameSelector: $("usernameSelector").value.trim() || null,
      passwordSelector: $("passwordSelector").value.trim() || null,
      loginOrigins: lines("loginOrigins"),
      submitOrigins: lines("submitOrigins"),
      enabled: $("enabled").checked,
      allowAi: $("allowAi").checked,
      allowJs: $("allowJs").checked,
    };
    if (
      !binding.name ||
      !binding.project ||
      !binding.usernameKey ||
      !binding.passwordKey
    )
      throw new UIError("missingSelection");
    if (!editing && bindings.some((b) => b.name === binding.name))
      throw new UIError("duplicateName");
    const oldOrigins = editing
      ? [...originsOf(editing), ...(editing.submitOrigins || [])]
      : [];
    const added = [
      ...new Set([...originsOf(binding), ...binding.submitOrigins]),
    ].filter((o) => !oldOrigins.includes(o));
    const changedAccount =
      editing &&
      ["project", "usernameKey", "passwordKey"].some(
        (k) => editing[k] !== binding[k],
      );
    if (
      (added.length ||
        changedAccount ||
        (binding.allowAi && !editingAiApproved) ||
        (binding.allowJs && !editing?.allowJs)) &&
      !window.confirm(
        t("confirmAuthorization", {
          name: binding.name,
          changes: added.length
            ? t("newDomains", { origins: added.join("\n") })
            : t("permissionsChanged"),
          permissions:
            (binding.allowAi ? t("confirmAi") : "") +
            (binding.allowJs ? t("confirmJs") : ""),
        }),
      )
    )
      return;
    // Permission prompt must run directly from this user gesture, before async native calls.
    const requested = originsOf(binding).map((o) => `${o}/*`);
    if (
      binding.enabled &&
      !(await chrome.permissions.request({ origins: requested }))
    )
      throw new UIError("permissionDenied");
    await send({ action: "save", binding });
    editing = binding;
    await load();
    await edit(binding);
    await removeUnusedPermissions();
    setText(status, "saved");
  } catch (error) {
    showError(status, error);
  }
});
async function removeUnusedPermissions() {
  await removeUnusedHostPermissions(bindings);
}
$("remove").addEventListener("click", async () => {
  if (!editing || !window.confirm(t("confirmRemove", { name: editing.name })))
    return;
  try {
    await send({ action: "remove", name: editing.name });
    editing = null;
    await load();
    await removeUnusedPermissions();
    await edit(null);
    setText(status, "revoked");
  } catch (error) {
    showError(status, error);
  }
});
$("new").addEventListener("click", () =>
  edit(null).catch((error) => showError(status, error)),
);
$("bindings").addEventListener("change", () =>
  edit(bindings.find((b) => b.name === $("bindings").value)).catch((error) =>
    showError(status, error),
  ),
);
$("project").addEventListener("change", () =>
  keys().catch((error) => showError(status, error)),
);
try {
  await load();
  projects = (await send({ action: "catalog" })).projects;
  options(
    $("project"),
    projects.map((p) => [p.id, p.name]),
  );
  await edit(null);
  setText(status, "connected");
} catch (error) {
  showError(status, error);
}

$("update").addEventListener("click", async () => {
  try {
    const result = await chrome.runtime.requestUpdateCheck();
    setText(
      status,
      result.status === "update_available"
        ? "updateAvailable"
        : result.status === "throttled"
          ? "updateThrottled"
          : "noUpdate",
    );
  } catch {
    setText(status, "updateUnavailable");
  }
});

import { initI18n, t, setText, showError, UIError } from "./i18n.js";
import { safeOrigin, originsOf } from "./policy.js";
const $ = (id) => document.getElementById(id);
const status = $("status");
let bindings = [],
  projects = [];
let editing = null;
await initI18n();
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
  $("editor").reset();
  $("name").disabled = !!binding;
  setText($("title"), binding ? "editAccount" : "newWebsiteAccount");
  $("remove").disabled = !binding;
  for (const key of ["name", "usernameSelector", "passwordSelector"])
    $(key).value = binding?.[key] || "";
  $("browser").value =
    binding?.browser ||
    (navigator.userAgent.includes("Edg/") ? "edge" : "chrome");
  $("website").value = binding?.origin || "";
  $("enabled").checked = binding?.enabled !== false;
  $("allowAi").checked = binding?.allowAi || false;
  $("allowJs").checked = binding?.allowJs || false;
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
      browser: $("browser").value,
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
        (binding.allowAi && !editing?.allowAi) ||
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
  const permissions = await chrome.permissions.getAll();
  const used = new Set(
    bindings
      .filter((b) => b.enabled)
      .flatMap((b) => originsOf(b).map((o) => `${o}/*`)),
  );
  const unused = (permissions.origins || []).filter((o) => !used.has(o));
  if (unused.length) await chrome.permissions.remove({ origins: unused });
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

import { fillLogin } from "./fill.js";
const HOST = "com.onekey.browser";
import {
  safeOrigin,
  originsOf,
  approvalOf,
  approvalKey,
  currentBrowser,
} from "./policy.js";
export { safeOrigin };
async function activeTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id || !tab.url)
    throw new Error("Open the authorized login website first.");
  return { id: tab.id, origin: safeOrigin(tab.url) };
}
export async function native(request) {
  const response = await chrome.runtime.sendNativeMessage(HOST, request);
  if (!response?.ok)
    throw new Error(
      "OneKey bridge refused the request. Check login, host installation and authorization.",
    );
  return response;
}
const permitted = (binding, origin) =>
  binding.enabled !== false && originsOf(binding).includes(origin);
const policyOf = (binding) => ({
  username: binding.usernameSelector,
  password: binding.passwordSelector,
  submitOrigins: binding.submitOrigins || [],
  allowJs: binding.allowJs || false,
});
// A fixed refusal code travels back to MCP; the message itself never leaves the extension.
const refuse = (reason, message) =>
  Object.assign(new Error(message), { reason });
async function fillTab(binding, tab, requestId) {
  if (!permitted(binding, tab.origin))
    throw refuse("notAuthorized", "Website is not authorized.");
  const policy = policyOf(binding);
  const preflight = await chrome.scripting
    .executeScript({
      target: { tabId: tab.id, frameIds: [0] },
      func: fillLogin,
      args: [tab.origin, null, policy],
    })
    .catch((error) => {
      throw refuse("noFields", error.message);
    });
  if (
    preflight.length !== 1 ||
    preflight[0].frameId !== 0 ||
    !preflight[0].result?.ready
  )
    throw refuse(
      "noFields",
      "No supported login fields. Credentials were not requested.",
    );
  const fields = preflight[0].result.fields || "both";
  const response = await native({
    action: "fill",
    name: binding.name,
    origin: tab.origin,
    fields,
    ...(requestId ? { requestId } : {}),
  }).catch((error) => {
    throw refuse("bridge", error.message);
  });
  try {
    if (
      response.origin !== tab.origin ||
      typeof response.credentials?.username !== "string" ||
      typeof response.credentials?.password !== "string"
    )
      throw new Error("Invalid credential response.");
    const current = requestId
      ? await chrome.tabs.get(tab.id)
      : await activeTab();
    if (
      current.id !== tab.id ||
      (current.origin || safeOrigin(current.url)) !== tab.origin
    )
      throw refuse("tabChanged", "Tab changed.");
    const fresh = (await native({ action: "list" })).bindings.find(
      (item) => item.name === binding.name,
    );
    if (
      !fresh ||
      !permitted(fresh, tab.origin) ||
      (requestId && !fresh.allowAi) ||
      JSON.stringify(fresh) !== JSON.stringify(binding)
    )
      throw refuse("authorizationChanged", "Authorization changed.");
    if (requestId) {
      const checked = await native({
        action: "check",
        name: binding.name,
        origin: tab.origin,
        requestId,
      });
      if (Date.now() >= checked.expiresAt * 1000)
        throw refuse("expired", "AI request expired.");
    }
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id, frameIds: [0] },
      func: fillLogin,
      args: [tab.origin, response.credentials, { ...policy, fields }],
    });
    if (
      results.length !== 1 ||
      results[0].frameId !== 0 ||
      !results[0].result?.filled
    )
      throw refuse("fillFailed", "Fill refused.");
    return { ok: true, filled: true, source: response.source };
  } finally {
    if (response.credentials) {
      response.credentials.username = "";
      response.credentials.password = "";
    }
  }
}
export async function handlePopup(message) {
  const tab = await activeTab();
  const { bindings } = await native({ action: "list" });
  const matches = bindings.filter((binding) => permitted(binding, tab.origin));
  if (message.action === "list")
    return {
      ok: true,
      origin: tab.origin,
      bindings: matches.map(({ name, project }) => ({ name, project })),
    };
  if (message.action !== "fill" || typeof message.name !== "string")
    throw new Error("Unknown request.");
  const binding = matches.find((item) => item.name === message.name);
  if (!binding)
    throw new Error("This connection is not authorized for the website.");
  return fillTab(binding, tab);
}
// Record the approved AI binding as the host stored it (origins are normalized there).
async function recordApproval(message) {
  const name = message.action === "save" ? message.binding?.name : message.name;
  if (typeof name !== "string") return;
  const key = approvalKey(name);
  const stored = (await native({ action: "list" })).bindings.find(
    (item) => item.name === name,
  );
  if (message.action === "save" && stored?.allowAi && stored.enabled !== false)
    await chrome.storage.local.set({ [key]: approvalOf(stored) });
  else await chrome.storage.local.remove(key);
}
let polling = false;
export async function pollAi() {
  if (polling) return;
  polling = true;
  try {
    const { requests } = await native({
      action: "poll",
      browser: currentBrowser(),
    });
    for (const request of requests) {
      let outcome = "refused";
      let reason;
      try {
        const binding = (await native({ action: "list" })).bindings.find(
          (item) =>
            item.name === request.name &&
            item.allowAi &&
            item.enabled !== false,
        );
        if (binding) {
          const key = approvalKey(binding.name);
          const approved = (await chrome.storage.local.get(key))[key];
          if (approved !== approvalOf(binding))
            throw refuse(
              "notApproved",
              "AI filling was not approved in the options page.",
            );
          const patterns = originsOf(binding).map((origin) => `${origin}/*`);
          if (!(await chrome.permissions.contains({ origins: patterns })))
            throw refuse("noPermission", "Browser permission revoked.");
          const tabs = await chrome.tabs.query({ url: patterns });
          const matches = tabs.filter(
            (tab) => tab.id && permitted(binding, safeOrigin(tab.url)),
          );
          if (matches.length !== 1)
            throw refuse(
              matches.length ? "multipleTabs" : "noTab",
              "Leave exactly one authorized login tab open.",
            );
          await fillTab(
            binding,
            { id: matches[0].id, origin: safeOrigin(matches[0].url) },
            request.id,
          );
          outcome = "filled";
        } else reason = "notAuthorized";
      } catch (error) {
        outcome = "refused";
        reason = error.reason || "other";
      }
      await native({
        action: "complete",
        id: request.id,
        outcome,
        ...(outcome === "refused" ? { reason } : {}),
      }).catch(() => {});
    }
  } finally {
    polling = false;
  }
}
chrome.runtime.onMessage.addListener((message, sender, reply) => {
  if (sender.id !== chrome.runtime.id) return false;
  const popup = sender.url === chrome.runtime.getURL("popup.html");
  const options = sender.url === chrome.runtime.getURL("options.html");
  let operation;
  if (popup && ["list", "fill"].includes(message.action))
    operation = handlePopup(message);
  else if (
    options &&
    ["list", "catalog", "save", "remove"].includes(message.action)
  )
    operation = native(message).then(async (response) => {
      if (["save", "remove"].includes(message.action))
        await recordApproval(message);
      return response;
    });
  else return false;
  operation.then(reply, (error) => reply({ ok: false, error: error.message }));
  return true;
});
// Alarms survive MV3 service worker suspension; credentials never enter storage.
if (chrome.alarms) {
  const start = async () => {
    await chrome.alarms.create("onekey-ai", { periodInMinutes: 0.5 });
    await pollAi().catch(() => {});
  };
  chrome.runtime.onInstalled.addListener(start);
  chrome.runtime.onStartup.addListener(start);
  chrome.alarms.onAlarm.addListener((alarm) => {
    if (alarm.name === "onekey-ai") pollAi().catch(() => {});
  });
}

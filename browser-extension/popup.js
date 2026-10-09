import { initI18n, setText, showError, UIError } from "./i18n.js";
const origin = document.querySelector("#origin");
const connection = document.querySelector("#connection");
const fill = document.querySelector("#fill");
const status = document.querySelector("#status");
await initI18n();
async function send(request) {
  const response = await chrome.runtime.sendMessage(request);
  if (!response?.ok)
    throw response?.error
      ? new Error(response.error)
      : new UIError("bridgeUnavailable");
  return response;
}
try {
  const response = await send({ action: "list" });
  delete origin.dataset.i18n;
  origin.textContent = response.origin;
  connection.replaceChildren();
  for (const binding of response.bindings) {
    const option = document.createElement("option");
    option.value = binding.name;
    option.textContent = `${binding.name} · ${binding.project}`;
    connection.append(option);
  }
  connection.disabled = response.bindings.length === 0;
  fill.disabled = connection.disabled;
  if (connection.disabled) setText(status, "noAccounts");
} catch {
  setText(origin, "notConnected");
  const unavailable = new Option("", "");
  setText(unavailable, "bridgeUnavailableShort");
  connection.replaceChildren(unavailable);
  setText(status, "bridgeUnavailable");
}
fill.addEventListener("click", async () => {
  fill.disabled = true;
  setText(status, "filling");
  try {
    const response = await send({ action: "fill", name: connection.value });
    setText(status, response.source === "cache" ? "filledCache" : "filledLive");
  } catch (error) {
    showError(status, error);
  } finally {
    fill.disabled = false;
  }
});

document
  .querySelector("#configure")
  .addEventListener("click", () => chrome.runtime.openOptionsPage());

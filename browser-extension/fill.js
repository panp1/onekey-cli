// Serialized into the isolated extension world. Never returns field values.
export function fillLogin(expectedOrigin, credentials, policy = {}) {
  if (window.top !== window || window.location.origin !== expectedOrigin)
    throw new Error("Page origin changed or frame is unsupported.");
  const visible = (input) => {
    if (
      !(input instanceof HTMLInputElement) ||
      input.disabled ||
      input.readOnly ||
      input.getClientRects().length === 0
    )
      return false;
    // Injected markup can park "visible" inputs out of sight; require a real, opaque box.
    const box = input.getBoundingClientRect();
    const style = getComputedStyle(input);
    return (
      box.width >= 4 &&
      box.height >= 4 &&
      style.visibility !== "hidden" &&
      Number(style.opacity) > 0
    );
  };
  const select = (selector, predicate, optional = false) => {
    const inputs = [...document.querySelectorAll(selector)].filter(
      (input) => visible(input) && predicate(input),
    );
    if (inputs.length === 0 && optional) return null;
    if (inputs.length !== 1)
      throw new Error("Login fields are ambiguous; configure selectors.");
    return inputs[0];
  };
  const password = select(
    policy.password || 'input[type="password"]',
    (input) => input.type === "password",
    true,
  );
  const username = select(
    policy.username ||
      'input[autocomplete="username"], input[type="email"], input[name="username"], input[name="email"]',
    (input) =>
      ["text", "email", "tel"].includes(input.type) &&
      (password
        ? input.form === password.form
        : // A username-only step must be marked as one, or any email box would match.
          policy.username ||
          // "webauthn" marks a sign-in identifier field (passkey-capable login).
          input.autocomplete
            .split(/\s+/)
            .some((token) => token === "username" || token === "webauthn")),
    true,
  );
  if (!password && !username)
    throw new Error("No supported visible login fields.");
  const form = (password || username).form;
  const approved = new Set([expectedOrigin, ...(policy.submitOrigins || [])]);
  const destination = (value) => {
    const url = new URL(value || window.location.href, window.location.href);
    if (url.username || url.password || !approved.has(url.origin))
      throw new Error("Submit destination is not authorized.");
  };
  // Named controls clobber form properties (<input name="action"> replaces
  // form.action), so read the real values through the prototype getters.
  const formProperty = (name) =>
    Object.getOwnPropertyDescriptor(HTMLFormElement.prototype, name).get.call(
      form,
    );
  if (form) {
    // Default GET is accepted only for explicitly authorized JS handlers with no
    // explicit action/method. An explicit GET can expose passwords in URLs.
    const jsOnly =
      policy.allowJs &&
      !form.hasAttribute?.("action") &&
      !form.hasAttribute?.("method");
    if (formProperty("method").toLowerCase() !== "post" && !jsOnly)
      throw new Error("GET login forms are refused.");
    destination(formProperty("action"));
    for (const button of document.querySelectorAll(
      'button, input[type="submit"], input[type="image"]',
    )) {
      if (button.form !== form) continue;
      if (button.hasAttribute("formaction")) destination(button.formAction);
      if (
        button.hasAttribute("formmethod") &&
        button.formMethod.toLowerCase() !== "post"
      )
        throw new Error("Unsafe GET submit override.");
    }
  } else if (!policy.allowJs)
    throw new Error("Enable JS form authorization for this website first.");
  const fields = password ? (username ? "both" : "password") : "username";
  if (!credentials) return { ready: true, fields };
  if (policy.fields && policy.fields !== fields)
    throw new Error("Login step changed after preflight.");
  const set = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  ).set;
  for (const [input, value] of [
    [username, credentials.username],
    [password, credentials.password],
  ]) {
    if (!input) continue;
    if (window.location.origin !== expectedOrigin || !input.isConnected)
      throw new Error("Page changed; fill refused.");
    set.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }
  return { filled: true };
}

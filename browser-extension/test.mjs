import assert from "node:assert/strict";
import { test, beforeEach } from "node:test";
import { fillLogin } from "./fill.js";
import { approvalOf, approvalKey } from "./policy.js";

let listener;
let sent;
let injected;
function resetChrome() {
  sent = [];
  injected = [];
  const storage = {};
  globalThis.chrome = {
    storage: {
      local: {
        get: async (key) => (key in storage ? { [key]: storage[key] } : {}),
        set: async (items) => Object.assign(storage, items),
        remove: async (key) => delete storage[key],
      },
    },
    tabs: {
      query: async () => [{ id: 7, url: "https://accounts.example.com/login" }],
    },
    runtime: {
      id: "extension-id",
      getURL: (file) => `chrome-extension://extension-id/${file}`,
      onMessage: {
        addListener: (fn) => {
          listener = fn;
        },
      },
      sendNativeMessage: async (host, request) => {
        assert.equal(host, "com.onekey.browser");
        sent.push(request);
        if (request.action === "check")
          return { ok: true, expiresAt: Math.floor(Date.now() / 1000) + 75 };
        if (request.action === "list")
          return {
            ok: true,
            bindings: [
              {
                name: "work",
                project: "website",
                origin: "https://accounts.example.com",
              },
            ],
          };
        return {
          ok: true,
          origin: "https://accounts.example.com",
          source: "live",
          credentials: { username: "test-user", password: "test-password" },
        };
      },
    },
    scripting: {
      executeScript: async (args) => {
        injected.push(args);
        return [
          {
            frameId: 0,
            result: args.args[1] ? { filled: true } : { ready: true },
          },
        ];
      },
    },
  };
}
resetChrome();
const { safeOrigin, handlePopup } = await import("./worker.js");

class Input {
  constructor(type, form) {
    this.type = type;
    this.form = form;
    this.isConnected = true;
    this.disabled = false;
    this.readOnly = false;
    this._value = "";
  }
  get value() {
    return this._value;
  }
  set value(value) {
    this._value = value;
  }
  getClientRects() {
    return [{}];
  }
  getBoundingClientRect() {
    return this.box ?? { width: 200, height: 32 };
  }
  dispatchEvent() {
    return true;
  }
}
// Accessors on the prototype, like HTMLFormElement; own properties model clobbering.
class Form {
  constructor(method, action) {
    this._method = method;
    this._action = action;
  }
  get method() {
    return this._method;
  }
  set method(value) {
    this._method = value;
  }
  get action() {
    return this._action;
  }
  set action(value) {
    this._action = value;
  }
  querySelectorAll() {
    return [];
  }
}
let form, username, password, fields;
beforeEach(() => {
  resetChrome();
  form = new Form("post", "https://accounts.example.com/login");
  username = new Input("email", form);
  password = new Input("password", form);
  fields = { password: [password], username: [username] };
  globalThis.HTMLInputElement = Input;
  globalThis.HTMLFormElement = Form;
  globalThis.getComputedStyle = (input) =>
    input.style ?? { visibility: "visible", opacity: "1" };
  globalThis.window = {
    location: {
      origin: "https://accounts.example.com",
      href: "https://accounts.example.com/login",
    },
  };
  window.top = window;
  globalThis.document = {
    querySelectorAll: (selector) =>
      selector.startsWith("button")
        ? form.querySelectorAll(selector)
        : selector.includes("password")
          ? fields.password
          : fields.username,
  };
});
const credentials = { username: "test-user", password: "test-password" };
const fill = () => fillLogin("https://accounts.example.com", credentials, {});

test("fills one same-origin POST form without returning values or submitting", () => {
  assert.deepEqual(fill(), { filled: true });
  assert.equal(username.value, "test-user");
  assert.equal(password.value, "test-password");
});
test("refuses another origin before writing fields", () => {
  window.location.origin = "https://evil.example.com";
  assert.throws(fill);
  assert.equal(password.value, "");
});
test("refuses iframes", () => {
  window.top = {};
  assert.throws(fill);
});
test("refuses cross-origin form destinations", () => {
  form.action = "https://evil.example.com";
  assert.throws(fill);
  assert.equal(password.value, "");
});
test("refuses GET forms and missing forms", () => {
  form.method = "get";
  assert.throws(fill);
  password.form = null;
  assert.throws(fill);
});
test("refuses cross-origin or GET submit overrides", () => {
  form.querySelectorAll = () => [
    {
      form,
      hasAttribute: (attr) => attr === "formaction",
      formAction: "https://evil.example.com",
    },
  ];
  assert.throws(fill);
  form.querySelectorAll = () => [
    { form, hasAttribute: (attr) => attr === "formmethod", formMethod: "get" },
  ];
  assert.throws(fill);
});
test("refuses ambiguous or hidden password fields", () => {
  fields.password.push(new Input("password", form));
  assert.throws(fill);
  fields.password = [password];
  password.getClientRects = () => [];
  username.autocomplete = "username";
  assert.deepEqual(fill(), { filled: true });
  assert.equal(password.value, "");
});
test("ignores a username field from another form on a password step", () => {
  username.form = {};
  assert.deepEqual(fill(), { filled: true });
  assert.equal(username.value, "");
  assert.equal(password.value, "test-password");
});
test("website origins require HTTPS except loopback", () => {
  assert.equal(
    safeOrigin("https://example.com:443/login"),
    "https://example.com",
  );
  assert.equal(
    safeOrigin("http://127.0.0.1:3000/login"),
    "http://127.0.0.1:3000",
  );
  for (const value of [
    "http://example.com",
    "https://user:pass@example.com",
    "file:///tmp/login",
  ])
    assert.throws(() => safeOrigin(value));
});
test("popup lists only matching origins and never returns credentials", async () => {
  const result = await handlePopup({ action: "list" });
  assert.deepEqual(result.bindings, [{ name: "work", project: "website" }]);
  assert.equal(sent.length, 1);
  assert.equal(JSON.stringify(result).includes("test-password"), false);
});
test("filling keeps secrets out of the popup result", async () => {
  const result = await handlePopup({ action: "fill", name: "work" });
  assert.deepEqual(result, { ok: true, filled: true, source: "live" });
  assert.equal(JSON.stringify(result).includes("test-password"), false);
  assert.deepEqual(injected[0].target, { tabId: 7, frameIds: [0] });
});
test("a different connection cannot obtain credentials", async () => {
  await assert.rejects(handlePopup({ action: "fill", name: "other" }));
  assert.equal(sent.length, 1);
  assert.equal(injected.length, 0);
});
test("a tab switch after fetching credentials prevents injection", async () => {
  let calls = 0;
  chrome.tabs.query = async () => [
    { id: ++calls === 1 ? 7 : 8, url: "https://accounts.example.com/login" },
  ];
  await assert.rejects(handlePopup({ action: "fill", name: "work" }));
  assert.equal(injected.length, 1);
});
test("web pages and content scripts cannot invoke the native host", () => {
  assert.equal(
    listener(
      {},
      { id: "extension-id", url: "https://accounts.example.com" },
      () => assert.fail(),
    ),
    false,
  );
  assert.equal(sent.length, 0);
});

test("preflight refuses unsupported forms before requesting credentials", async () => {
  chrome.scripting.executeScript = async () => [{ frameId: 0, result: {} }];
  await assert.rejects(handlePopup({ action: "fill", name: "work" }));
  assert.equal(sent.length, 1);
});

test("a named action control cannot hide the real form destination", () => {
  form.action = "https://evil.example/collect";
  Object.defineProperty(form, "action", { value: new Input("hidden", form) });
  assert.throws(fill, /Submit destination is not authorized/);
  assert.equal(password.value, "");
});
test("transparent, invisible or tiny password fields are not filled", () => {
  password.style = { visibility: "visible", opacity: "0" };
  assert.throws(fill, /No supported visible login fields|ambiguous/);
  password.style = { visibility: "hidden", opacity: "1" };
  assert.throws(fill, /No supported visible login fields|ambiguous/);
  password.style = undefined;
  password.box = { width: 1, height: 1 };
  assert.throws(fill, /No supported visible login fields|ambiguous/);
  assert.equal(password.value, "");
});
test("a username-only step needs autocomplete=username or a selector", () => {
  fields.password = [];
  assert.throws(
    () => fillLogin(window.location.origin, null, {}),
    /No supported visible login fields/,
  );
  assert.deepEqual(
    fillLogin(window.location.origin, null, { username: "#login" }),
    { ready: true, fields: "username" },
  );
});
test("fills username and password steps separately", () => {
  username.autocomplete = "username";
  fields.password = [];
  assert.deepEqual(fillLogin(window.location.origin, null, {}), {
    ready: true,
    fields: "username",
  });
  assert.deepEqual(fill(), { filled: true });
  assert.equal(password.value, "");
  fields.username = [];
  fields.password = [password];
  assert.deepEqual(fillLogin(window.location.origin, null, {}), {
    ready: true,
    fields: "password",
  });
  assert.deepEqual(fill(), { filled: true });
  assert.equal(password.value, "test-password");
});
test("JS pages require explicit authorization", () => {
  username.form = null;
  password.form = null;
  assert.throws(fill);
  assert.deepEqual(
    fillLogin(window.location.origin, credentials, { allowJs: true }),
    { filled: true },
  );
});
test("explicit GET forms remain forbidden even with JS authorization", () => {
  form.method = "get";
  form.hasAttribute = (key) => key === "method";
  assert.throws(() =>
    fillLogin(window.location.origin, credentials, { allowJs: true }),
  );
  form.hasAttribute = () => false;
  assert.deepEqual(
    fillLogin(window.location.origin, credentials, { allowJs: true }),
    { filled: true },
  );
});
test("cross-origin POST accepts only explicitly approved submit origins", () => {
  form.action = "https://auth.example.com/login";
  assert.throws(fill);
  assert.deepEqual(
    fillLogin(window.location.origin, credentials, {
      submitOrigins: ["https://auth.example.com"],
    }),
    { filled: true },
  );
});
test("step changes after preflight are rejected before input writes", () => {
  assert.throws(() =>
    fillLogin(window.location.origin, credentials, { fields: "username" }),
  );
  assert.equal(username.value, "");
  assert.equal(password.value, "");
});
test("popup cannot modify authorization", () => {
  assert.equal(
    listener(
      { action: "save", binding: {} },
      { id: chrome.runtime.id, url: chrome.runtime.getURL("popup.html") },
      () => assert.fail(),
    ),
    false,
  );
});
test("AI polling automatically identifies the browser running the extension", async () => {
  const { pollAi } = await import("./worker.js");
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  const browsers = [];
  chrome.runtime.sendNativeMessage = async (_host, request) => {
    assert.equal(request.action, "poll");
    browsers.push(request.browser);
    return { ok: true, requests: [] };
  };
  try {
    for (const userAgent of [
      "Mozilla/5.0 Chrome/140.0.0.0 Safari/537.36",
      "Mozilla/5.0 Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0",
    ]) {
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: { userAgent },
      });
      await pollAi();
    }
    assert.deepEqual(browsers, ["chrome", "edge"]);
    assert.equal(injected.length, 0);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "navigator", descriptor);
    else delete globalThis.navigator;
  }
});
test("AI fill returns only outcome through the native bridge", async () => {
  const { pollAi } = await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.permissions = { contains: async () => true };
  chrome.tabs.get = async () => ({
    id: 7,
    url: "https://accounts.example.com/login",
  });
  chrome.runtime.sendNativeMessage = async (host, request) => {
    if (request.action === "poll")
      return { ok: true, requests: [{ id: "request", name: "work" }] };
    if (request.action === "complete") {
      sent.push(request);
      return { ok: true };
    }
    const result = await original(host, request);
    if (request.action === "list") result.bindings[0].allowAi = true;
    return result;
  };
  const approved = (
    await chrome.runtime.sendNativeMessage("com.onekey.browser", {
      action: "list",
    })
  ).bindings[0];
  await chrome.storage.local.set({
    [approvalKey("work")]: approvalOf(approved),
  });
  sent.length = 0;
  await pollAi();
  const completion = sent.find((request) => request.action === "complete");
  assert.deepEqual(completion, {
    action: "complete",
    id: "request",
    outcome: "filled",
  });
  assert(!JSON.stringify(completion).includes("test-password"));
  assert.equal(
    sent.find((request) => request.action === "fill").requestId,
    "request",
  );
});
test("AI refuses multiple matching tabs before retrieving credentials", async () => {
  const { pollAi } = await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.permissions = { contains: async () => true };
  chrome.tabs.query = async () => [
    { id: 7, url: "https://accounts.example.com" },
    { id: 8, url: "https://accounts.example.com" },
  ];
  chrome.runtime.sendNativeMessage = async (host, request) => {
    if (request.action === "poll")
      return { ok: true, requests: [{ id: "request", name: "work" }] };
    if (request.action === "complete") {
      sent.push(request);
      return { ok: true };
    }
    const result = await original(host, request);
    if (request.action === "list") result.bindings[0].allowAi = true;
    return result;
  };
  const listed = await chrome.runtime.sendNativeMessage("com.onekey.browser", {
    action: "list",
  });
  await chrome.storage.local.set({
    [approvalKey("work")]: approvalOf(listed.bindings[0]),
  });
  await pollAi();
  assert(!sent.some((request) => request.action === "fill"));
  const completion = sent.find((request) => request.action === "complete");
  assert.equal(completion.outcome, "refused");
  assert.equal(completion.reason, "multipleTabs");
});
test("all-site browser access does not allow AI filling an unbound origin", async () => {
  const { pollAi } = await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.permissions = { contains: async () => true };
  chrome.tabs.query = async () => [
    { id: 7, url: "https://unbound.example.com/login" },
  ];
  chrome.runtime.sendNativeMessage = async (host, request) => {
    if (request.action === "poll")
      return { ok: true, requests: [{ id: "request", name: "work" }] };
    if (request.action === "complete") {
      sent.push(request);
      return { ok: true };
    }
    const result = await original(host, request);
    if (request.action === "list") result.bindings[0].allowAi = true;
    return result;
  };
  const binding = (
    await chrome.runtime.sendNativeMessage("com.onekey.browser", {
      action: "list",
    })
  ).bindings[0];
  await chrome.storage.local.set({
    [approvalKey("work")]: approvalOf(binding),
  });
  sent.length = 0;
  await pollAi();
  assert.equal(sent.at(-1).outcome, "refused");
  assert(!sent.some((request) => request.action === "fill"));
  assert.equal(injected.length, 0);
});
test("AI refuses revoked browser host permissions", async () => {
  const { pollAi } = await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.permissions = { contains: async () => false };
  chrome.runtime.sendNativeMessage = async (host, request) => {
    if (request.action === "poll")
      return { ok: true, requests: [{ id: "request", name: "work" }] };
    if (request.action === "complete") {
      sent.push(request);
      return { ok: true };
    }
    const result = await original(host, request);
    if (request.action === "list") result.bindings[0].allowAi = true;
    return result;
  };
  await pollAi();
  assert(!sent.some((request) => request.action === "fill"));
  assert.equal(
    sent.find((request) => request.action === "complete").outcome,
    "refused",
  );
});

test("AI refuses bindings changed outside the options page", async () => {
  const { pollAi } = await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.permissions = { contains: async () => true };
  let swapped = false;
  chrome.runtime.sendNativeMessage = async (host, request) => {
    if (request.action === "poll")
      return { ok: true, requests: [{ id: "request", name: "work" }] };
    if (request.action === "complete") {
      sent.push(request);
      return { ok: true };
    }
    const result = await original(host, request);
    if (request.action === "list") {
      result.bindings[0].allowAi = true;
      if (swapped) result.bindings[0].passwordKey = "prod_password";
    }
    return result;
  };
  // No approval recorded: a hand-edited allowAi is refused.
  await pollAi();
  assert.equal(sent.at(-1).outcome, "refused");
  assert.equal(sent.at(-1).reason, "notApproved");
  // Approved, then the bound secret is swapped behind the options page's back.
  const listed = await chrome.runtime.sendNativeMessage("com.onekey.browser", {
    action: "list",
  });
  await chrome.storage.local.set({
    [approvalKey("work")]: approvalOf(listed.bindings[0]),
  });
  swapped = true;
  sent.length = 0;
  await pollAi();
  assert.equal(sent.at(-1).outcome, "refused");
  assert(!sent.some((request) => request.action === "fill"));
});
test("options save records and remove clears the AI approval", async () => {
  await import("./worker.js");
  const original = chrome.runtime.sendNativeMessage;
  chrome.runtime.sendNativeMessage = async (host, request) => {
    const result = await original(host, request);
    if (request.action === "list") result.bindings[0].allowAi = true;
    return result;
  };
  const options = {
    id: "extension-id",
    url: "chrome-extension://extension-id/options.html",
  };
  const call = (message) =>
    new Promise((resolve) => listener(message, options, resolve));
  await call({ action: "save", binding: { name: "work" } });
  const key = approvalKey("work");
  assert.ok((await chrome.storage.local.get(key))[key]);
  await call({ action: "remove", name: "work" });
  assert.deepEqual(await chrome.storage.local.get(key), {});
});

// Language preference and live page updates must never reset account drafts.
const {
  messages,
  initI18n,
  setLanguage,
  getLanguage,
  setText,
  showError,
  UIError,
} = await import("./i18n.js");
function languagePage() {
  const nodes = [];
  const element = (dataset = {}, value = "") => {
    const node = {
      dataset,
      value,
      textContent: "",
      attrs: {},
      setAttribute(name, value) {
        this.attrs[name] = value;
      },
    };
    nodes.push(node);
    return node;
  };
  const language = element({}, "en");
  language.addEventListener = () => {};
  const title = element({ i18n: "newWebsiteAccount" });
  const status = element();
  const name = element(
    { i18nPlaceholder: "connectionPlaceholder" },
    "User's unsaved account",
  );
  const binding = element();
  setText(binding, "accountDisabled", { name: "账号 <script> & account" });
  globalThis.document = {
    documentElement: { lang: "" },
    querySelector: (selector) =>
      selector === "#language"
        ? language
        : selector === "#status"
          ? status
          : null,
    querySelectorAll: (selector) =>
      nodes.filter((node) =>
        selector === "[data-i18n]"
          ? node.dataset.i18n
          : selector === "[data-i18n-placeholder]"
            ? node.dataset.i18nPlaceholder
            : node.dataset.i18nLabel,
      ),
  };
  return { language, title, status, name, binding };
}
test("all English/Chinese UI keys and interpolation parameters are complete", async () => {
  const { readFile } = await import("node:fs/promises");
  assert.deepEqual(
    Object.keys(messages.en).sort(),
    Object.keys(messages.zh).sort(),
  );
  for (const key of Object.keys(messages.en)) {
    assert(messages.en[key].trim());
    assert(messages.zh[key].trim());
    const params = (text) =>
      [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
    assert.deepEqual(params(messages.en[key]), params(messages.zh[key]), key);
  }
  for (const name of ["options.html", "popup.html", "options.js", "popup.js"]) {
    const source = await readFile(new URL(name, import.meta.url), "utf8");
    const keys = [
      ...source.matchAll(/data-i18n(?:-placeholder|-label)?="([\w]+)"/g),
      ...source.matchAll(/\b(?:t|UIError)\("([\w]+)"/g),
      ...source.matchAll(/setText\([^,]+,\s*"([\w]+)"/g),
    ].map((match) => match[1]);
    for (const key of keys) assert(key in messages.en, `${name}: ${key}`);
  }
});
test("fresh installation defaults to English even on a Chinese browser", async () => {
  const page = languagePage();
  await initI18n();
  assert.equal(getLanguage(), "en");
  assert.equal(document.documentElement.lang, "en");
  assert.equal(page.title.textContent, "Add website account");
  assert.equal(page.language.value, "en");
});
test("language changes persist, translate status and preserve form values", async () => {
  const page = languagePage();
  await initI18n();
  setText(page.status, "saved");
  await setLanguage("zh");
  assert.equal((await chrome.storage.local.get("language")).language, "zh");
  assert.equal(document.documentElement.lang, "zh-CN");
  assert.equal(page.title.textContent, "新增网站账号");
  assert(page.status.textContent.startsWith("已保存授权"));
  assert.equal(page.name.value, "User's unsaved account");
  assert.equal(page.name.attrs.placeholder, "公司 Jira");
  assert.equal(page.binding.textContent, "已停用 · 账号 <script> & account");
  await initI18n();
  assert.equal(getLanguage(), "zh");
  await setLanguage("en");
  assert(page.status.textContent.startsWith("Authorization saved"));
});
test("other extension pages react to stored language changes", async () => {
  let changed;
  chrome.storage.onChanged = {
    addListener: (fn) => {
      changed = fn;
    },
  };
  const page = languagePage();
  await initI18n();
  changed({ language: { newValue: "zh" } }, "local");
  assert.equal(page.title.textContent, "新增网站账号");
  changed({ language: { newValue: "en" } }, "sync");
  assert.equal(getLanguage(), "zh");
  changed({ language: { newValue: "unsupported" } }, "local");
  assert.equal(getLanguage(), "en");
  assert.equal(page.name.value, "User's unsaved account");
});
test("UI errors are translated without exposing unknown native error bodies", async () => {
  const page = languagePage();
  await initI18n();
  await setLanguage("zh");
  showError(page.status, new UIError("permissionDenied"));
  assert(page.status.textContent.includes("未授予网站权限"));
  showError(
    page.status,
    new Error(
      "OneKey bridge refused the request. Check login, host installation and authorization.",
    ),
  );
  assert(page.status.textContent.includes("OneKey 拒绝"));
  showError(page.status, new Error("MUST_NOT_LEAK"));
  assert(!page.status.textContent.includes("MUST_NOT_LEAK"));
});

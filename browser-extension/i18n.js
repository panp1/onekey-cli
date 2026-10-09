// User-selected UI language; intentionally defaults to English on every OS.
export const messages = {
  en: {
    pageTitle: "OneKey · Website authorization",
    appTitle: "OneKey Website Login",
    language: "Language",
    intro:
      "Choose existing secrets and authorize each website and account. This page reads secret names only.",
    authorizedAccounts: "Authorized accounts",
    accountsLabel: "Website accounts",
    newAccount: "Add account",
    editAccount: "Edit website account",
    newWebsiteAccount: "Add website account",
    connectionName: "Connection name",
    connectionPlaceholder: "Work Jira",
    website: "Website origin",
    aiBrowser: "Browser for AI filling",
    project: "Project",
    chooseProject: "Choose a project",
    choose: "Select…",
    usernameKey: "Username secret",
    passwordKey: "Password secret",
    advanced: "More login pages and cross-origin settings",
    stepsHint:
      "Multi-step login detects the current username or password step. Additional login origins allow credentials on those pages; submit origins allow forms to send credentials there.",
    loginOrigins: "Additional login origins (one complete origin per line)",
    submitOrigins: "Additional submit origins (one complete origin per line)",
    usernameSelector: "Username selector (optional)",
    passwordSelector: "Password selector (optional)",
    allowJs: "Allow JS login pages (no form or no declared submit method)",
    jsHint:
      "JS request destinations cannot be verified by inspecting forms. Enable this only for trusted websites. Explicit GET forms are always refused.",
    enabled: "Enable this account authorization",
    allowAi: "Allow AI to fill an open login page by connection name",
    aiHint:
      "AI filling never submits. Keep exactly one matching login tab open; complete CAPTCHA and MFA yourself.",
    save: "Save and authorize",
    remove: "Revoke and delete",
    footer:
      "Passwords go directly to the destination website; the website and privileged browser debugging tools can still read inputs. After upgrading the CLI, run browser install again to update the Windows native host copy.",
    update: "Check extension updates",
    bridgeUnavailable:
      "Local bridge unavailable. Register the native host and configure CLI login, then reopen this page.",
    metadataOffline:
      "Metadata is temporarily unavailable. You can still disable or revoke an existing account.",
    accountEnabled: "{name}",
    accountDisabled: "Disabled · {name}",
    invalidOrigin:
      "Enter a complete website origin without a path, query or fragment.",
    httpsRequired: "Use HTTPS. HTTP is allowed only for loopback testing.",
    missingSelection:
      "Choose a project and both secrets, and enter a connection name.",
    duplicateName:
      "This connection name exists. Select that account to edit it.",
    confirmAuthorization:
      "Authorize account “{name}”?\n{changes}\n{permissions}Credentials will enter the authorized website.",
    newDomains: "New origins:\n{origins}",
    permissionsChanged: "The account or filling permissions have changed.",
    confirmAi: "Allow automatic AI filling.\n",
    confirmJs: "Allow JS login forms.\n",
    permissionDenied:
      "Website permission was not granted. Configuration was not saved.",
    saved:
      "Authorization saved. Return to the login website to fill. AI authorization takes effect in about 30 seconds.",
    confirmRemove:
      "Revoke and delete website and AI filling authorization for “{name}”?",
    revoked: "Authorization revoked. Queued AI requests will be refused.",
    connected:
      "Local bridge connected. Choose a project and secrets to add a website account.",
    updateAvailable:
      "A store update is ready. The browser will upgrade when appropriate.",
    noUpdate:
      "No store update is available. Unpacked extensions must be updated and reloaded manually.",
    updateThrottled:
      "Update checks are temporarily throttled. Try again later.",
    updateUnavailable:
      "Update check unavailable. For an unpacked extension, update its files and reload manually.",
    languageSaveFailed: "Could not save the language preference. Try again.",
    popupSubtitle: "Authorized website credential filling",
    readingWebsite: "Reading current website…",
    connectionLabel: "Website connection",
    loading: "Loading…",
    fill: "Fill current login step",
    configure: "Manage websites and accounts",
    popupHint:
      "Fill this website only, without submitting. Complete CAPTCHA and MFA yourself.",
    firstSetup: "First-time setup",
    setupBefore: "Register the native host first:",
    setupAfter:
      "Choose websites, projects and secrets in “Manage websites and accounts”. Passwords are never displayed here.",
    noAccounts:
      "No account is authorized for this website. Add one in “Manage websites and accounts”.",
    notConnected: "Not connected to OneKey",
    bridgeUnavailableShort: "Local bridge unavailable",
    filling: "Fetching and filling credentials…",
    filledCache:
      "Filled using an unexpired offline cache. Review the page before submitting.",
    filledLive: "Filled. Review the page before submitting.",
    bridgeRefused:
      "OneKey refused the request. Check CLI login, host installation and website authorization.",
    openWebsite: "Open the authorized login website first.",
    websiteUnauthorized:
      "This connection is not authorized for the current website.",
    unsupportedForm:
      "No supported login fields. Check the form and configured selectors.",
    pageChanged: "The tab, login page or authorization changed. Fill refused.",
    operationFailed:
      "Operation failed. Check the website authorization and local bridge, then retry.",
  },
  zh: {
    pageTitle: "OneKey · 网站授权",
    appTitle: "OneKey 网站登录",
    language: "语言",
    intro: "选择现有密钥，按网站和账号授权。此页面只读取密钥名称。",
    authorizedAccounts: "已授权的账号",
    accountsLabel: "网站账号",
    newAccount: "新增账号",
    editAccount: "编辑网站账号",
    newWebsiteAccount: "新增网站账号",
    connectionName: "连接名称",
    connectionPlaceholder: "公司 Jira",
    website: "网站地址",
    aiBrowser: "AI 填充使用的浏览器",
    project: "项目",
    chooseProject: "选择项目",
    choose: "请选择",
    usernameKey: "用户名密钥",
    passwordKey: "密码密钥",
    advanced: "更多页面与跨域设置",
    stepsHint:
      "分步登录自动识别当前用户名或密码步骤。额外登录域名允许凭据进入该页面；提交域名允许表单向其发送凭据。",
    loginOrigins: "额外登录域名（每行一个完整 origin）",
    submitOrigins: "额外提交域名（每行一个完整 origin）",
    usernameSelector: "用户名选择器（可选）",
    passwordSelector: "密码选择器（可选）",
    allowJs: "允许 JS 登录表单（无表单／无显式提交方法）",
    jsHint:
      "JS 的实际请求目标无法通过表单检查确认，仅为可信网站开启。显式 GET 表单始终拒绝。",
    enabled: "启用此账号授权",
    allowAi: "允许 AI 按名称自动填入已打开的登录页",
    aiHint:
      "AI 填充不会自动提交。浏览器里必须只有一个匹配的登录标签页；验证码和 MFA 由你完成。",
    save: "保存并授权",
    remove: "撤销并删除",
    footer:
      "密码直接交给目标网页；网页自身和有权限的浏览器调试工具仍可读取输入框。升级 CLI 后请重新运行 browser install，Windows 会更新本地桥接副本。",
    update: "检查扩展更新",
    bridgeUnavailable:
      "本地桥接不可用，请先注册本地桥接并配置 CLI 登录，再重新打开页面。",
    metadataOffline: "元数据暂不可用，仍可停用或撤销已有账号授权。",
    accountEnabled: "{name}",
    accountDisabled: "已停用 · {name}",
    invalidOrigin: "域名请只填写完整 origin，不含路径、查询或片段。",
    httpsRequired: "请使用 HTTPS；HTTP 仅允许本地回环测试。",
    missingSelection: "请填写连接名称，并选择项目和两个密钥。",
    duplicateName: "连接名称已存在，请选择该账号编辑。",
    confirmAuthorization:
      "确认授权账号「{name}」？\n{changes}\n{permissions}密码会进入已授权的网页。",
    newDomains: "新增域名：\n{origins}",
    permissionsChanged: "账号或填充权限已变更。",
    confirmAi: "允许 AI 自动填充。\n",
    confirmJs: "允许 JS 登录表单。\n",
    permissionDenied: "未授予网站权限，配置未保存。",
    saved: "已保存授权。返回登录网站，使用扩展填入；AI 授权约 30 秒内生效。",
    confirmRemove: "撤销并删除「{name}」的网站和 AI 填充授权？",
    revoked: "已撤销，排队中的 AI 请求会被拒绝。",
    connected: "本地桥接已连接。选择项目和密钥即可添加网站账号。",
    updateAvailable: "商店更新已就绪，浏览器会在合适时机升级。",
    noUpdate: "暂无商店更新。开发者模式需更新文件后手动重新加载。",
    updateThrottled: "更新检查暂时受到频率限制，请稍后重试。",
    updateUnavailable: "更新检查不可用。开发者模式请更新文件后手动重新加载。",
    languageSaveFailed: "无法保存语言偏好，请重试。",
    popupSubtitle: "已授权的网站凭据填充",
    readingWebsite: "正在读取当前网站…",
    connectionLabel: "网站登录绑定",
    loading: "正在加载…",
    fill: "填入当前登录步骤",
    configure: "管理网站与账号授权",
    popupHint: "只填当前网站，不自动提交。验证码和 MFA 请自行完成。",
    firstSetup: "首次设置",
    setupBefore: "首次使用先注册本地桥接：",
    setupAfter:
      "网站、项目和密钥在「管理网站与账号授权」中选择，密码不会显示在此窗口。",
    noAccounts: "当前网站没有已授权的账号，请点击「管理网站与账号授权」添加。",
    notConnected: "尚未连接 OneKey",
    bridgeUnavailableShort: "本地桥接不可用",
    filling: "正在获取并填入凭据…",
    filledCache: "已使用有效期内的离线缓存填入。请检查页面后提交登录。",
    filledLive: "已填入。请检查页面后提交登录。",
    bridgeRefused:
      "OneKey 拒绝了请求，请检查 CLI 登录、本地桥接安装和网站授权。",
    openWebsite: "请先打开已授权的登录网站。",
    websiteUnauthorized: "此连接未获当前网站授权。",
    unsupportedForm: "未找到支持的登录字段，请检查表单和配置的选择器。",
    pageChanged: "标签页、登录页面或授权已变更，已拒绝填充。",
    operationFailed: "操作失败，请检查网站授权和本地桥接后重试。",
  },
};
let language = "en";
export const normalizeLanguage = (value) => (value === "zh" ? "zh" : "en");
export const getLanguage = () => language;
export function t(key, params = {}) {
  const template = messages[language][key] ?? messages.en[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (match, name) =>
    Object.hasOwn(params, name) ? String(params[name]) : match,
  );
}
export function setText(element, key, params = {}) {
  element.dataset.i18n = key;
  element.dataset.i18nParams = JSON.stringify(params);
  element.textContent = t(key, params);
}
export class UIError extends Error {
  constructor(key) {
    super(key);
    this.key = key;
  }
}
const errors = {
  "Use HTTPS; HTTP only for loopback testing.": "httpsRequired",
  "Open the authorized login website first.": "openWebsite",
  "Website is not authorized.": "websiteUnauthorized",
  "This connection is not authorized for the website.": "websiteUnauthorized",
  "No supported login fields. Credentials were not requested.":
    "unsupportedForm",
  "OneKey bridge refused the request. Check login, host installation and authorization.":
    "bridgeRefused",
  "Tab changed.": "pageChanged",
  "Authorization changed.": "pageChanged",
  "AI request expired.": "pageChanged",
  "Fill refused.": "operationFailed",
};
export function showError(element, error) {
  setText(element, error?.key || errors[error?.message] || "operationFailed");
}
export function applyLanguage(value) {
  language = normalizeLanguage(value);
  document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
  for (const element of document.querySelectorAll("[data-i18n]")) {
    element.textContent = t(
      element.dataset.i18n,
      JSON.parse(element.dataset.i18nParams || "{}"),
    );
  }
  for (const element of document.querySelectorAll("[data-i18n-placeholder]"))
    element.setAttribute("placeholder", t(element.dataset.i18nPlaceholder));
  for (const element of document.querySelectorAll("[data-i18n-label]"))
    element.setAttribute("aria-label", t(element.dataset.i18nLabel));
  const selector = document.querySelector("#language");
  if (selector) selector.value = language;
}
export async function setLanguage(value) {
  const next = normalizeLanguage(value);
  await chrome.storage.local.set({ language: next });
  applyLanguage(next);
}
export async function initI18n() {
  let saved;
  try {
    saved = (await chrome.storage.local.get("language")).language;
  } catch {
    /* English still works when storage is unavailable. */
  }
  applyLanguage(saved);
  document
    .querySelector("#language")
    ?.addEventListener("change", async (event) => {
      try {
        await setLanguage(event.target.value);
      } catch {
        applyLanguage(language);
        const status = document.querySelector("#status");
        if (status) setText(status, "languageSaveFailed");
      }
    });
  chrome.storage.onChanged?.addListener((changes, area) => {
    if (area === "local" && changes.language)
      applyLanguage(changes.language.newValue);
  });
}

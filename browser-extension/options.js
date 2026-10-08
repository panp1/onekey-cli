import { safeOrigin, originsOf } from "./policy.js";
const $ = (id) => document.getElementById(id);
const status = $("status");
let bindings = [],
  projects = [];
let editing = null;
async function send(message) {
  const response = await chrome.runtime.sendMessage(message);
  if (!response?.ok)
    throw new Error(response?.error || "本地桥接不可用，请先安装并登录 CLI。");
  return response;
}
function options(element, items, selected = "") {
  element.replaceChildren(new Option("请选择", ""));
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
    status.textContent = "元数据暂不可用，仍可停用或撤销已有账号授权。";
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
      `${binding.enabled ? "" : "已停用 · "}${binding.name}`,
    ]),
    editing?.name,
  );
}
async function edit(binding) {
  editing = binding || null;
  $("editor").reset();
  $("name").disabled = !!binding;
  $("title").textContent = binding ? "编辑网站账号" : "新增网站账号";
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
  const url = new URL(value);
  if (url.pathname !== "/" || url.search || url.hash)
    throw new Error("域名请只填写 origin，不含路径、查询或片段。");
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
      throw new Error("请选择项目和两个密钥。");
    if (!editing && bindings.some((b) => b.name === binding.name))
      throw new Error("连接名称已存在，请选择该账号编辑。");
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
        `确认授权账号「${binding.name}」？\n${added.length ? "新增域名：\n" + added.join("\n") : "账号或填充权限已变更"}\n${binding.allowAi ? "允许 AI 自动填充。\n" : ""}${binding.allowJs ? "允许 JS 登录表单。\n" : ""}密码会进入已授权的网页。`,
      )
    )
      return;
    // Permission prompt must run directly from this user gesture, before async native calls.
    const requested = originsOf(binding).map((o) => `${o}/*`);
    if (
      binding.enabled &&
      !(await chrome.permissions.request({ origins: requested }))
    )
      throw new Error("未授予网站权限，配置未保存。");
    await send({ action: "save", binding });
    editing = binding;
    await load();
    await edit(binding);
    await removeUnusedPermissions();
    status.textContent =
      "已保存授权。返回登录网站，使用扩展填入；AI 授权约 30 秒内生效。";
  } catch (error) {
    status.textContent = error.message;
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
  if (
    !editing ||
    !window.confirm(`撤销并删除「${editing.name}」的网站和 AI 填充授权？`)
  )
    return;
  try {
    await send({ action: "remove", name: editing.name });
    editing = null;
    await load();
    await removeUnusedPermissions();
    await edit(null);
    status.textContent = "已撤销，排队中的 AI 请求会被拒绝。";
  } catch (error) {
    status.textContent = error.message;
  }
});
$("new").addEventListener("click", () =>
  edit(null).catch((error) => (status.textContent = error.message)),
);
$("bindings").addEventListener("change", () =>
  edit(bindings.find((b) => b.name === $("bindings").value)).catch(
    (error) => (status.textContent = error.message),
  ),
);
$("project").addEventListener("change", () =>
  keys().catch((error) => (status.textContent = error.message)),
);
try {
  await load();
  projects = (await send({ action: "catalog" })).projects;
  options(
    $("project"),
    projects.map((p) => [p.id, p.name]),
  );
  await edit(null);
  status.textContent = "本地桥接已连接。选择项目和密钥即可添加网站账号。";
} catch (error) {
  status.textContent = error.message;
}

$("update").addEventListener("click", async () => {
  try {
    const result = await chrome.runtime.requestUpdateCheck();
    status.textContent =
      result.status === "update_available"
        ? "商店更新已就绪，浏览器会在合适时机升级。"
        : "更新检查：" +
          result.status +
          "。开发者模式需在扩展页面手动重新加载。";
  } catch {
    status.textContent = "更新检查不可用。开发者模式请更新文件后手动重新加载。";
  }
});

const origin = document.querySelector("#origin");
const connection = document.querySelector("#connection");
const fill = document.querySelector("#fill");
const status = document.querySelector("#status");
async function send(request) {
  const response = await chrome.runtime.sendMessage(request);
  if (!response?.ok)
    throw new Error(
      response?.error || "本地桥接不可用，请检查安装和 CLI 登录。",
    );
  return response;
}
try {
  const response = await send({ action: "list" });
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
  if (connection.disabled)
    status.textContent =
      "当前网站没有已授权的账号，请点击「管理网站与账号授权」添加。";
} catch {
  origin.textContent = "尚未连接 OneKey";
  connection.replaceChildren(new Option("本地桥接不可用", ""));
  status.textContent = "请先注册本地桥接并配置 CLI 登录，然后重新打开此窗口。";
}
fill.addEventListener("click", async () => {
  fill.disabled = true;
  status.textContent = "正在获取并填入凭据…";
  try {
    const response = await send({ action: "fill", name: connection.value });
    status.textContent =
      response.source === "cache"
        ? "已使用有效期内的离线缓存填入。请检查页面后提交登录。"
        : "已填入。请检查页面后提交登录。";
  } catch (error) {
    status.textContent = error.message;
  } finally {
    fill.disabled = false;
  }
});

document
  .querySelector("#configure")
  .addEventListener("click", () => chrome.runtime.openOptionsPage());

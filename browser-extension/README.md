# OneKey website login (Chrome and Edge)

Supports Chrome/Edge 120+ on macOS, Linux and Windows, using the same MV3 extension.
The server does not need changes. Website configuration is visual; only the
initial native host registration and CLI authentication use the terminal.

## Build and install locally

```sh
cargo build --locked --release
node --test browser-extension/test.mjs browser-extension/host-permissions.test.mjs
node browser-extension/package.mjs
```

Load `browser-extension/dist/` in Developer mode at `chrome://extensions` or
`edge://extensions`, copy its ID, then register the corresponding browser:

```sh
onekey browser install --browser chrome --extension-id YOUR_CHROME_ID
onekey browser install --browser edge --extension-id YOUR_EDGE_ID
```

Use the built CLI (`target/release/onekey`) until it has replaced your installed
CLI. Prefer a saved PAT scoped to the website project and necessary secrets;
configure it with `onekey config` outside the AI conversation.

`onekey-browser_0.2.2.zip` includes the extension and `install.sh` / `install.ps1`.
Install helpers accept binary path, browser and extension ID. For Windows:

```powershell
.\install.ps1 -Binary C:\Tools\onekey.exe -Browser edge -ExtensionId YOUR_EDGE_ID
```

Windows copies an executable and pinned sidecar into the OneKey profile, and
registers `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.onekey.browser`
or the corresponding `Microsoft\Edge` key. No administrator access is needed.
On macOS/Linux the native host is a pinned shell launcher. Chrome and Edge retain
separate registrations. Re-run installation after changing binary, profile,
server or after upgrading the Windows CLI. `--manifest-dir` stages registration
without changing browser registration; use an isolated `--data-dir` for tests.

## UI language

The popup and configuration page default to English. Use the **Language** selector
on either page to switch between English and 简体中文. The preference is saved in
extension-local storage, shared by both pages, and applied immediately without
clearing account configuration or unsaved form inputs. Website names, project
names and secret names remain exactly as provided.

## Configure websites visually

Click the extension → **Manage websites and accounts** (中文：**管理网站与账号授权**). Choose a project and existing username /
password secret names. Enter an exact HTTPS origin (no path; HTTP only loopback
for development). Name the connection and save; newly added domains and changes
to sensitive permissions require confirmation and browser site permission.

Existing `browser add` bindings remain compatible and can be edited in the UI.
Each named binding represents one account. Disable or delete it to revoke further
fill requests. Deleting does not log out a website or clear already-filled inputs.
Unused browser permissions are removed when saving/deleting in this page.

The extension detects one visible username/password pair, or one visible field
for username-first/password-first steps. A username-only step needs an input
marked `autocomplete="username"` or a configured username selector. Click fill again after navigating to the
next step. It never clicks Next or Submit. Custom CSS selectors resolve ambiguity.
Same-origin POST forms work by default. Additional login origins authorize pages
where credentials may be inserted; additional submit origins authorize HTML form
destinations. Neither uses wildcard matching. Iframes are refused.

JS/no-form pages require **允许 JS 登录表单**. Implicit default-GET forms without
explicit `action`/`method` are accepted only with this authorization; explicit
GET forms and GET submit overrides remain refused. JS network destinations cannot
be inferred from DOM and are entrusted to the explicitly authorized website.
This covers ordinary input-based reactive pages; shadow DOM, closed components,
passkeys, CAPTCHA and MFA are not automated.

## Browser site access

In **Manage websites and accounts**, enable **Allow access to all HTTPS websites**
under **Browser site access** to request a single browser grant for all HTTPS
websites. It defaults to off. Chrome/Edge displays its own permission prompt;
denying it keeps per-site access. The switch reflects the browser's actual grant,
including revocation through browser settings.

This grant survives saving, disabling or deleting individual accounts. Credentials
still require a configured, enabled website/account binding, and AI filling still
requires separate approval. HTTP remains limited to loopback testing. Turn the
switch off to revoke the global grant; if the browser also removes a site's access,
save that account's authorization again to request its specific site permission.

在「管理网站与账号授权 → 浏览器网站访问权限」勾选「允许访问所有 HTTPS 网站」，
由 Chrome/Edge 弹窗授权，默认关闭。全站访问权限不会因删除账号而自动撤销；
填充仍按网站和账号授权，AI 填充仍需单独批准。取消勾选可撤销全站权限，
若某账号的网站权限也被浏览器移除，请重新保存该账号授权。HTTP 仍仅限回环测试。

## AI/MCP integration

Enable **允许 AI…** per connection and select Chrome or Edge. Save the same scoped
PAT/runner token in the native host profile that MCP uses; a different environment
token will not cause the bridge to use a higher-privilege saved identity.
Configure your AI client's MCP process as:

```json
{ "command": "onekey", "args": ["mcp", "serve"] }
```

For Codex, register OneKey in your personal global configuration:

```bash
codex mcp add onekey -- onekey mcp serve
codex mcp get onekey --json
```

The configuration is saved in `~/.codex/config.toml` and shared across projects.
If the desktop app cannot find `onekey` on its PATH, use the executable's absolute
path after `--`, for example `~/.local/bin/onekey` on macOS/Linux. Restart Codex
to load the new tools. See the [official MCP setup](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

Codex 个人全局配置使用上面的两条命令，保存到 `~/.codex/config.toml`，各项目共用。
桌面应用找不到 `onekey` 时，把 `--` 后的程序名换成可执行文件的绝对路径；
配置完成后重启 Codex。无需在 MCP 配置中写入令牌，沿用 CLI 已保存的 PAT 或 Runner token。

For a separate profile, include `--data-dir` and `--server` before `mcp serve`.
The actual Chrome/Edge extension executes filling in the existing browser tab.
Your browser automation tool can navigate/open that login tab first, call MCP,
then continue after a successful fill; the bridge never returns password values.
This does not connect to Codex's separate in-app browser or isolated browser profiles.

**What this protects, and what it does not.** AI filling is meant for automated
testing with test accounts. Passwords never appear in MCP results, the AI
transcript or logs. An agent that also controls the browser can still read a
filled input with page JavaScript, so treat every AI-enabled connection as
readable by that agent: enable **允许 AI…** only for test accounts, never for
production or personal ones.

AI approval is recorded by the extension when you save the connection on the
options page. A binding changed anywhere else (for example by editing
`browser-logins.json`) is refused for AI filling until you save it there again;
connections enabled for AI before this check must be saved once more.

Available tools:

| Tool                              | Purpose                                             |
| --------------------------------- | --------------------------------------------------- |
| `onekey_list_projects`            | Scoped project IDs/names                            |
| `onekey_list_groups`              | Scoped group metadata                               |
| `onekey_list_secret_names`        | Names, descriptions, versions; never values         |
| `onekey_list_browser_connections` | Enabled, explicitly AI-authorized connections       |
| `onekey_get_browser_status`       | Recent extension heartbeat and requirements         |
| `onekey_request_browser_fill`     | Queue by `name`, return requestId immediately       |
| `onekey_get_browser_fill_result`  | Query by `request_id`; status/outcome only          |
| `onekey_cancel_browser_fill`      | Cancel by `request_id`; cannot undo prior insertion |
| `onekey_fill_browser_connection`  | Convenience blocking fill, up to 60 seconds         |

Use the asynchronous request/result pair by default. Poll no faster than once
per second. MV3 alarms poll every 30 seconds but browsers may delay them; requests
expire after 75 seconds. Exactly one matching login tab must be open in the
selected browser. Ambiguity, revoked permission, changed binding or token mismatch
refuses filling. Request records contain no credential values. Existing encrypted
cache and server authentication TTL are reused; offline reads never renew TTL.

The popup and MCP do not expose password values. The destination page and
privileged browser debugging tools can read filled fields; avoid logging form
values, DOM dumps or screenshots that reveal usernames/passwords after filling.

## Packaging, publishing and upgrading

`package.mjs` builds reproducible ZIPs with SHA-256 checksums:

- `onekey-browser_0.2.2.zip`: local install/upgrade bundle.
- `onekey-browser-store_0.2.2.zip`: runtime-only package for Chrome Web Store and Edge Add-ons.

Both repositories' release workflows test/build these packages and add them to
the GitHub release assets when a release tag is pushed. No release is published
by local builds. Use one store listing per browser with the privacy disclosure
in [PRIVACY.md](PRIVACY.md); increment manifest version for every store update.
Store-installed extensions use the store's automatic update mechanism. The
configuration page's **检查扩展更新** button requests a store update check.
Unpacked extensions require replacing files at the same path and clicking Reload;
they do not receive store updates. CLI updates and native host registration are
separate; Windows copies must be refreshed by running install again.

Publishing to stores requires publisher accounts, listing IDs, hosted privacy
URL, screenshots and store review. These local ZIPs are submission packages,
not signed CRX files or already-published listings.

## Remove

Delete local account bindings in the UI, remove the extension, and delete the
native messaging registration. On Windows delete the matching HKCU registry key;
on macOS use `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`
or `Microsoft Edge/NativeMessagingHosts/`; on Linux use
`~/.config/google-chrome/NativeMessagingHosts/` or `microsoft-edge/NativeMessagingHosts/`.

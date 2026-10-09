# Changelog

All notable changes to the OneKey CLI are documented in this file.

## Unreleased

- A refused AI fill now reports a fixed reason code (for example `noTab`, `multipleTabs`, `notApproved`, `noPermission`) through MCP; no page text or credentials.
- Browser extension 0.2.2 adds an opt-in, revocable all-HTTPS site access switch in English and Chinese. Browser access remains separate from website/account and AI filling authorization.

- Browser extension 0.2.1 supports English and Simplified Chinese, defaults to English, and saves the language selected in the popup or configuration page.

## 0.7.4 - 2026-10-08

Website login filling for automated testing.

- Add visual website/account configuration, per-origin authorization and revocation, JavaScript and multi-step login filling.
- Extend MCP with browser connection discovery, bridge status, synchronous/asynchronous filling, result lookup and cancellation. Credentials never enter MCP responses.
- Add Windows native host registration and Chrome/Edge install/store packages, release assets and update checks.
- AI filling is approved per connection on the extension options page; bindings changed elsewhere are refused.
- Fill checks read the real form action and method, skip invisible fields and need a marked username field on username-only steps.

## 0.7.3 - 2026-10-08

Offline cache lifetime, aligned with OneKey 0.7.3.

### Added

- `run` and `curl` enforce the offline cache lifetime the server issues with each
  fetch. Administrators set it in Settings → Security, separately for password
  sign-in, runner tokens and personal / AI tokens (defaults 1 hour, 1 hour, 15
  minutes; 0 disables fallback; maximum 24 hours). Offline reads never renew it.
- Expired, future-dated, disabled or legacy caches without a lifetime never start
  a child. With fallback disabled no cached copy is kept on disk. Older servers
  remain usable online.

### Fixed

- The installers find the latest release without the rate-limited GitHub API.

## 0.7.2 - 2026-10-02

Version aligned with OneKey 0.7.2; no change in behaviour.

### Changed

- Help and README state that SSO sign-in is not supported yet, and which commands a token
  covers. The help no longer points to the server repository.

### Note

- The CLI follows the OneKey server's version numbers; `onekey --version` prints
  `OneKey CLI v0.7.2`.

## 0.7.1 - 2026-10-02

First standalone release of the lightweight OneKey client, extracted from OneKey 0.7.1.

### Added

- The client commands of OneKey 0.7.1 in a 7 MB binary: `config`, `profile`, `login`, `status`,
  `ls`, `use`, `run`, `curl`, `secret` (including `describe`), `project`, `group`, `token`,
  `init`, `import`, `export`, `cache`, `mcp`, `backup`, `restore` and `update`.
- `onekey --version` prints `OneKey CLI v0.7.1`, so it is clear which edition is installed (the
  full release prints `OneKey Server v… (includes CLI)`).
- One-line installers with checksum verification: `install.sh` for macOS and Linux,
  `install.ps1` for Windows.

### Note

- `onekey server` and `onekey admin` are not part of this CLI. Run the server from the full
  OneKey release (https://github.com/panp1/onekey). Both read the same `~/.onekey` profiles.

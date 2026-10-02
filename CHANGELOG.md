# Changelog

All notable changes to the OneKey CLI are documented in this file.

## 0.7.2 - 2026-10-02

Version aligned with OneKey 0.7.2; no change in behaviour.

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

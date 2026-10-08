#!/bin/sh
# Install the OneKey CLI: curl -fsSL https://raw.githubusercontent.com/panp1/onekey-cli/main/install.sh | sh
#   ONEKEY_VERSION=0.7.1      install a specific release (default: latest)
#   ONEKEY_INSTALL_DIR=DIR    install location (default: ~/.local/bin)
set -eu

repo="panp1/onekey-cli"
install_dir="${ONEKEY_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  printf 'onekey install: %s\n' "$1" >&2
  exit 1
}

case "$(uname -s)" in
  Darwin) os=darwin ;;
  Linux) os=linux ;;
  *) fail "unsupported OS $(uname -s); download a release from https://github.com/$repo/releases" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=amd64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) fail "unsupported CPU $(uname -m)" ;;
esac

if [ -n "${ONEKEY_VERSION:-}" ]; then
  version="$ONEKEY_VERSION"
else
  # Follow the releases/latest redirect instead of the GitHub API: the API allows 60 anonymous
  # requests an hour per IP and answers 403 to a busy office or VPN address.
  latest="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")" ||
    fail "could not reach GitHub"
  version="${latest##*/tag/}"
  [ "$version" != "$latest" ] && [ -n "$version" ] || fail "could not find the latest release"
fi

archive="onekey-cli_${version}_${os}_${arch}.tar.gz"
base="https://github.com/$repo/releases/download/$version"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

printf 'Downloading OneKey CLI %s (%s/%s)...\n' "$version" "$os" "$arch"
curl -fsSL -o "$tmp/$archive" "$base/$archive" || fail "download failed: $base/$archive"
curl -fsSL -o "$tmp/checksums.txt" "$base/checksums.txt" || fail "could not download checksums.txt"

expected="$(grep " $archive\$" "$tmp/checksums.txt" | cut -d ' ' -f 1)"
[ -n "$expected" ] || fail "$archive is not listed in checksums.txt"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$archive" | cut -d ' ' -f 1)"
else
  actual="$(shasum -a 256 "$tmp/$archive" | cut -d ' ' -f 1)"
fi
[ "$expected" = "$actual" ] || fail "checksum mismatch for $archive"

tar -xzf "$tmp/$archive" -C "$tmp" onekey
mkdir -p "$install_dir"
install -m 755 "$tmp/onekey" "$install_dir/onekey"
printf 'Installed %s to %s\n' "$("$install_dir/onekey" --version)" "$install_dir/onekey"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to your PATH, for example:\n  export PATH="%s:$PATH"\n' "$install_dir" "$install_dir" ;;
esac
printf 'Next: onekey config\n'

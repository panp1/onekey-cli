#!/bin/sh
set -eu
# Usage: sh install.sh /absolute/path/to/onekey chrome EXTENSION_ID
binary=${1:-onekey}
browser=${2:-chrome}
id=${3:-}
case "$browser" in chrome|edge) ;; *) echo 'Browser must be chrome or edge' >&2; exit 1;; esac
if [ -z "$id" ]; then
  echo "Load the extension/ directory at ${browser}://extensions, enable Developer mode, then copy its ID."
  printf 'Extension ID: '
  read -r id
fi
"$binary" browser install --browser "$browser" --extension-id "$id"
echo 'Open the OneKey extension → Manage websites and accounts. Bindings are preserved during upgrades.'
echo 'For unpacked extension upgrades, replace extension/ at the SAME location and click Reload on the extensions page.'

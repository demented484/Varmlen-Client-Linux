#!/usr/bin/env bash
# Fetch the xray-core binary that gets bundled INTO the installer, so the app
# works on first launch without reaching GitHub — vital in censored networks
# where the on-demand core download would be blocked.
#
# Pinned version; the running app reads the actual version from the binary
# itself (`xray version`), so this number only decides what ships. Skips the
# download if the binary is already present. Run from the project root (the
# tauri beforeBuildCommand invokes it there).
set -euo pipefail

VERSION="26.6.27"
DEST="src-tauri/cores/xray"

if [ -f "$DEST" ]; then
  echo "xray already present: $DEST"
  exit 0
fi

mkdir -p "$(dirname "$DEST")"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
URL="https://github.com/XTLS/Xray-core/releases/download/v${VERSION}/Xray-linux-64.zip"

echo "fetching xray v${VERSION}…"
curl -fsSL "$URL" -o "$TMP/xray.zip"
unzip -o -q "$TMP/xray.zip" xray -d "$TMP"
install -m 0755 "$TMP/xray" "$DEST"
echo "xray v${VERSION} -> $DEST"

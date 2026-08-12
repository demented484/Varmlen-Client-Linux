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

VERSION="26.7.28"
DEST="src-tauri/cores/xray"
MARKER="${DEST}.asset"

target_triple="${VARMLEN_TARGET_TRIPLE:-${TAURI_ENV_TARGET_TRIPLE:-${CARGO_BUILD_TARGET:-}}}"
if [ -z "$target_triple" ]; then
  target_triple="$(rustc -vV | sed -n 's/^host: //p')"
fi
target_arch="${target_triple%%-*}"

case "$target_arch" in
  x86_64|amd64) asset="Xray-linux-64.zip" ;;
  x86|i386|i486|i586|i686) asset="Xray-linux-32.zip" ;;
  aarch64|arm64) asset="Xray-linux-arm64-v8a.zip" ;;
  armv7*) asset="Xray-linux-arm32-v7a.zip" ;;
  armv6*|arm) asset="Xray-linux-arm32-v6.zip" ;;
  armv5*) asset="Xray-linux-arm32-v5.zip" ;;
  riscv64*) asset="Xray-linux-riscv64.zip" ;;
  loongarch64|loong64) asset="Xray-linux-loong64.zip" ;;
  powerpc64le|ppc64le) asset="Xray-linux-ppc64le.zip" ;;
  powerpc64|ppc64) asset="Xray-linux-ppc64.zip" ;;
  s390x) asset="Xray-linux-s390x.zip" ;;
  mipsel|mips32el) asset="Xray-linux-mips32le.zip" ;;
  mips|mips32) asset="Xray-linux-mips32.zip" ;;
  mips64el) asset="Xray-linux-mips64le.zip" ;;
  mips64) asset="Xray-linux-mips64.zip" ;;
  *)
    echo "unsupported Linux target architecture: $target_arch ($target_triple)" >&2
    exit 1
    ;;
esac

if [ "${1:-}" = "--print-asset" ]; then
  echo "$asset"
  exit 0
fi

if [ -f "$DEST" ] && [ -f "$MARKER" ] && [ "$(cat "$MARKER")" = "$VERSION:$asset" ]; then
  echo "xray already present for $target_triple: $DEST"
  exit 0
fi

mkdir -p "$(dirname "$DEST")"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
URL="https://github.com/XTLS/Xray-core/releases/download/v${VERSION}/${asset}"

echo "fetching xray v${VERSION} for ${target_triple} (${asset})…"
curl -fsSL "$URL" -o "$TMP/xray.zip"
unzip -o -q "$TMP/xray.zip" xray -d "$TMP"
install -m 0755 "$TMP/xray" "$DEST"
printf '%s:%s\n' "$VERSION" "$asset" >"$MARKER"
echo "xray v${VERSION} -> $DEST"

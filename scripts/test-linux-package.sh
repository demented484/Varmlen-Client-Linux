#!/bin/sh
set -eu

DEB="${1:-target/release/bundle/deb/Varmlen_0.2.5_amd64.deb}"
EXPECTED_ARCH="${2:-}"

if [ ! -f "$DEB" ]; then
  echo "missing Debian package: $DEB" >&2
  exit 1
fi

if [ -n "$EXPECTED_ARCH" ]; then
  ACTUAL_ARCH="$(dpkg-deb -f "$DEB" Architecture)"
  if [ "$ACTUAL_ARCH" != "$EXPECTED_ARCH" ]; then
    echo "package architecture: expected $EXPECTED_ARCH, got $ACTUAL_ARCH" >&2
    exit 1
  fi
fi

LISTING="$(mktemp)"
ROOT="$(mktemp -d)"
trap 'rm -f "$LISTING"; rm -rf "$ROOT"' EXIT

dpkg-deb -c "$DEB" >"$LISTING"

require_entry() {
  mode="$1"
  path="$2"
  if ! awk -v mode="$mode" -v path="$path" \
    '$1 == mode && ($2 == "root/root" || $2 == "0/0") && $NF == path { found = 1 } END { exit !found }' \
    "$LISTING"; then
    echo "package entry has wrong owner/mode or is missing: $path" >&2
    exit 1
  fi
}

require_entry "-rwxr-xr-x" "usr/libexec/varmlen/varmlend"
require_entry "-rwxr-xr-x" "usr/libexec/varmlen/varmlen-net"
require_entry "-rwxr-xr-x" "usr/libexec/varmlen/xray"
require_entry "-rw-r--r--" "usr/share/polkit-1/actions/app.varmlen.client.policy"

if awk '$NF ~ /^usr\/lib\/Varmlen\/(varmlend|varmlen-net|xray)$/ { found = 1 } END { exit !found }' \
  "$LISTING"; then
  echo "package contains duplicate privileged binaries under /usr/lib/Varmlen" >&2
  exit 1
fi

dpkg-deb -x "$DEB" "$ROOT"
if find "$ROOT" -type f -exec getcap {} + 2>/dev/null | grep -q .; then
  echo "package unexpectedly grants file capabilities" >&2
  exit 1
fi

echo "Linux package layout: PASS"

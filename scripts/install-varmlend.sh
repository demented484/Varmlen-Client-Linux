#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ] && [ -z "${FAKEROOTKEY:-}" ]; then
  echo "install-varmlend.sh must run as root" >&2
  exit 1
fi

DESTDIR="${DESTDIR:-}"
SOURCE="${VARMLEND_SOURCE:-target/release/varmlend}"
NET_SOURCE="${VARMLEN_NET_SOURCE:-target/release/varmlen-net}"
XRAY_SOURCE="${VARMLEN_XRAY_SOURCE:-src-tauri/cores/xray}"

for component in "$SOURCE" "$NET_SOURCE" "$XRAY_SOURCE"; do
  if [ ! -f "$component" ] || [ ! -x "$component" ]; then
    echo "Varmlen component is missing or not executable: $component" >&2
    exit 1
  fi
done

install -d -o root -g root -m 0755 "$DESTDIR/usr/libexec/varmlen"
install -o root -g root -m 0755 "$SOURCE" "$DESTDIR/usr/libexec/varmlen/varmlend"
install -o root -g root -m 0755 "$NET_SOURCE" "$DESTDIR/usr/libexec/varmlen/varmlen-net"
install -o root -g root -m 0755 "$XRAY_SOURCE" "$DESTDIR/usr/libexec/varmlen/xray"

install -d -o root -g root -m 0755 "$DESTDIR/usr/share/polkit-1/actions"
install -o root -g root -m 0644 \
  packaging/varmlend/app.varmlen.client.policy \
  "$DESTDIR/usr/share/polkit-1/actions/app.varmlen.client.policy"

if command -v setcap >/dev/null 2>&1; then
  for component in varmlend varmlen-net xray; do
    setcap -r "$DESTDIR/usr/libexec/varmlen/$component" 2>/dev/null || true
  done
fi

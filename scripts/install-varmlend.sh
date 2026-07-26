#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ] && [ -z "${FAKEROOTKEY:-}" ]; then
  echo "install-varmlend.sh must run as root" >&2
  exit 1
fi

DESTDIR="${DESTDIR:-}"
SOURCE="${VARMLEND_SOURCE:-target/release/varmlend}"

if [ ! -f "$SOURCE" ] || [ ! -x "$SOURCE" ]; then
  echo "varmlend source is missing or not executable: $SOURCE" >&2
  exit 1
fi

install -d -o root -g root -m 0755 "$DESTDIR/usr/libexec/varmlen"
install -o root -g root -m 0755 "$SOURCE" "$DESTDIR/usr/libexec/varmlen/varmlend"

install -d -o root -g root -m 0755 "$DESTDIR/usr/share/polkit-1/actions"
install -o root -g root -m 0644 \
  packaging/varmlend/app.varmlen.client.policy \
  "$DESTDIR/usr/share/polkit-1/actions/app.varmlen.client.policy"

if command -v setcap >/dev/null 2>&1; then
  setcap -r "$DESTDIR/usr/libexec/varmlen/varmlend" 2>/dev/null || true
fi


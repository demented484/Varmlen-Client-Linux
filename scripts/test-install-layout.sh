#!/bin/sh
set -eu

ROOT="$(mktemp -d)"
SOURCE="$ROOT/varmlend-source"
trap 'rm -rf "$ROOT"' EXIT

printf '#!/bin/sh\nexit 0\n' >"$SOURCE"
chmod 0755 "$SOURCE"

DESTDIR="$ROOT/root" VARMLEND_SOURCE="$SOURCE" sh scripts/install-varmlend.sh

DAEMON="$ROOT/root/usr/libexec/varmlen/varmlend"
test -x "$DAEMON"
test "$(stat -c %u "$DAEMON")" = 0
test "$(stat -c %g "$DAEMON")" = 0
test "$(stat -c %a "$DAEMON")" = 755
test -z "$(getcap "$DAEMON")"

test -f "$ROOT/root/usr/share/polkit-1/actions/app.varmlen.client.policy"

if find "$ROOT/root" -type f -exec getcap {} + | grep -q .; then
  echo "installed file unexpectedly has capabilities" >&2
  exit 1
fi

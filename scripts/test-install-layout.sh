#!/bin/sh
set -eu

if [ -z "${FAKEROOTKEY:-}" ]; then
  exec fakeroot sh "$0" "$@"
fi

ROOT="$(mktemp -d)"
SOURCE="$ROOT/varmlend-source"
NET_SOURCE="$ROOT/varmlen-net-source"
XRAY_SOURCE="$ROOT/xray-source"
trap 'rm -rf "$ROOT"' EXIT

printf '#!/bin/sh\nexit 0\n' >"$SOURCE"
printf '#!/bin/sh\nexit 0\n' >"$NET_SOURCE"
printf '#!/bin/sh\nexit 0\n' >"$XRAY_SOURCE"
chmod 0755 "$SOURCE" "$NET_SOURCE" "$XRAY_SOURCE"

DESTDIR="$ROOT/root" \
  VARMLEND_SOURCE="$SOURCE" \
  VARMLEN_NET_SOURCE="$NET_SOURCE" \
  VARMLEN_XRAY_SOURCE="$XRAY_SOURCE" \
  sh scripts/install-varmlend.sh

DAEMON="$ROOT/root/usr/libexec/varmlen/varmlend"
for component in "$DAEMON" \
  "$ROOT/root/usr/libexec/varmlen/varmlen-net" \
  "$ROOT/root/usr/libexec/varmlen/xray"; do
  test -x "$component"
  test "$(stat -c %u "$component")" = 0
  test "$(stat -c %g "$component")" = 0
  test "$(stat -c %a "$component")" = 755
  test -z "$(getcap "$component")"
done

test -f "$ROOT/root/usr/share/polkit-1/actions/app.varmlen.client.policy"

if find "$ROOT/root" -type f -exec getcap {} + | grep -q .; then
  echo "installed file unexpectedly has capabilities" >&2
  exit 1
fi

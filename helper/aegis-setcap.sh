#!/usr/bin/env bash
# Grant the file capabilities the AegisVPN cores need, then verify them.
# Runs as root via pkexec (one password prompt for the whole batch).
#
#   aegis-setcap.sh <sing-box-path> <aegis-probe-path> [old-helper-uninstall.sh]
#
# - sing-box gets cap_net_admin (TUN device, auto_route, auto_redirect nft, SO_MARK)
# - aegis-probe gets cap_net_admin,cap_net_raw (SO_MARK + SO_BINDTODEVICE, nft killswitch)
# - if a third arg is given, it's the old root-helper uninstaller: run it so the
#   migration (remove daemon) + grant-caps happen under a single pkexec prompt.
#
# File caps are cleared whenever a binary is replaced (core update), so this is
# re-run after every download/activate of the active sing-box.
set -u

SINGBOX="${1:-}"
PROBE="${2:-}"
OLD_UNINSTALL="${3:-}"

fail=0

cap_set() {
  local bin="$1" caps="$2"
  if [ -z "$bin" ] || [ ! -f "$bin" ]; then
    echo "skip: $bin (missing)" >&2
    return 0
  fi
  setcap "$caps" "$bin" || { echo "setcap failed on $bin" >&2; fail=1; return 1; }
  # Read back — file caps silently no-op on nosuid/NFS/overlay homes.
  local got
  got="$(getcap "$bin" 2>/dev/null)"
  if [ -z "$got" ]; then
    echo "VERIFY FAILED: no caps on $bin after setcap (nosuid/NFS home?)" >&2
    fail=1
    return 1
  fi
  echo "ok: $got"
}

# Run the old-helper uninstall first if requested (migration in the same prompt).
if [ -n "$OLD_UNINSTALL" ] && [ -f "$OLD_UNINSTALL" ]; then
  echo "removing legacy root helper…"
  sh "$OLD_UNINSTALL" || echo "legacy uninstall returned non-zero (continuing)" >&2
fi

cap_set "$SINGBOX" "cap_net_admin+ep"
cap_set "$PROBE"   "cap_net_admin,cap_net_raw+ep"

exit "$fail"

#!/usr/bin/env bash
# Grant the file capabilities the Varmlen core + helper need, then verify them.
# Runs as root via pkexec (one password prompt for the whole batch).
#
#   varmlen-setcap.sh <xray-path> <varmlen-probe-path> [old-helper-uninstall.sh]
#
# - xray gets cap_net_admin (its native TUN device + routing).
# - varmlen-probe gets cap_net_admin,cap_net_raw,cap_dac_override,cap_bpf:
#     cap_net_admin   - SO_MARK, nft killswitch, ip routes/rules, net sysctls
#     cap_net_raw     - SO_BINDTODEVICE for the tunnel-bypass probes
#     cap_dac_override- write the root-owned rp_filter sysctl + /run state
#     cap_bpf         - the cgroup socket-mark program for per-app exclusions
#                       (sockets get the real source IP; flows survive VPN off).
#                       Unknown on kernels < 5.8 -> granted without it, the
#                       helper then falls back to the masquerade-only bypass.
# - if a third arg is given, it's the old root-helper uninstaller: run it so the
#   migration (remove daemon) + grant-caps happen under a single pkexec prompt.
#
# File caps are cleared whenever a binary is replaced (core update), so this is
# re-run after every download/activate of the active xray.
set -u

XRAY="${1:-}"
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

cap_set "$XRAY"  "cap_net_admin+ep"
# cap_bpf first; kernels < 5.8 don't know it — retry without (helper falls back
# to the masquerade-only bypass there).
if setcap "cap_net_admin,cap_net_raw,cap_dac_override,cap_bpf+ep" "$PROBE" 2>/dev/null; then
  cap_set "$PROBE" "cap_net_admin,cap_net_raw,cap_dac_override,cap_bpf+ep"
else
  cap_set "$PROBE" "cap_net_admin,cap_net_raw,cap_dac_override+ep"
fi

exit "$fail"

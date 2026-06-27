#!/usr/bin/env bash
# Remove the Varmlen privileged helper. Run as root.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "error: must run as root (sudo $0)" >&2
  exit 1
fi

systemctl disable --now varmlen-helper.service 2>/dev/null || true
rm -f /etc/systemd/system/varmlen-helper.service
rm -rf /usr/local/lib/varmlen
systemctl daemon-reload

echo "Varmlen helper removed."

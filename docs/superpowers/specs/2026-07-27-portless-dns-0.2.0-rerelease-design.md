# Portless DNS and 0.2.0 Re-release Design

## Goal

Replace the broken Linux 0.2.0 release and refresh the Android 0.2.0 release
without changing either public version number. Neither client will create a
local Xray DNS listener.

## Confirmed Linux failure

Linux 0.2.0 adds a `dns-in` dokodemo-door on `127.0.0.1:5353`. Port 5353 is the
standard mDNS port and was already owned by ADB on the affected host. Xray
exited with:

```text
Failed to start: listen udp 127.0.0.1:5353: bind: address already in use
```

The lifecycle had already installed its hold block. With the user killswitch
enabled, the failed initial connection correctly remained fail-closed, which
made all non-excluded traffic appear to lose connectivity.

## Selected architecture

### Xray configuration

Both clients keep exactly one data inbound:

- Linux native TUN uses `tun-in`.
- Android tun2socks uses `socks-in`.

The separate `dns-in` inbound and its fixed port are removed. The existing
first routing rule remains responsible for DNS:

```text
data inbound + destination port 53 -> dns-out
```

`dns-out` continues using Xray's built-in DNS module, whose DoH endpoint is
forced through the proxy outbound.

### Linux DNS capture

Linux system DNS needs special handling because a configured router resolver,
for example `192.168.1.1`, has a more-specific LAN route than the TUN default.
The `varmlen_dns` nftables table will therefore:

1. Run after the per-app split marking chain.
2. Match locally generated TCP and UDP destination port 53.
3. Preserve Xray's own `0x2024` dial mark.
4. Replace every other matching packet's meta and connection marks with the
   dedicated DNS mark `0x2023`.
5. Permit marked DNS only when its selected output path is `varmlen0`.
6. Reject direct TCP/UDP port 53 and direct TCP port 853 as a fail-closed
   backstop.

The network helper will install IPv4 policy routing for mark `0x2023` into a
dedicated table whose default is `dev varmlen0`. IPv6 remains fail-closed under
the existing IPv6 blackhole policy.

This preserves per-app bypass for application traffic while intentionally
forcing classic system DNS through the VPN. It also works when the system first
queries a local stub such as `127.0.0.53`: the stub's upstream packet receives
the DNS mark.

### Linux DNS verification

The daemon no longer checks a loopback TCP listener. After TUN routing and DNS
marking are active, it sends a bounded DNS query to a public port-53
destination. A valid matching response proves that the packet entered the TUN,
was handled by `dns-out`, and returned successfully. Failure removes the DNS
policy and follows the existing transactional rollback path.

No development-host VPN, reconnect, DNS, public-IP, nftables, or route test is
permitted. Tests render and inspect rules or use fake backends only.

## Release behavior

### Linux

- Keep package and tag version `0.2.0`.
- Bump the authenticated daemon protocol version from 1 to 2. If the GUI
  reaches a surviving protocol-1 daemon, it must reject it before sending a
  connect request and report that one reboot is required. It must never
  silently run the withdrawn daemon code.
- Build and verify DEB, RPM, and AppImage artifacts.
- Replace the existing GitHub `v0.2.0` release and tag only after all offline
  tests and artifact gates pass.
- Because the affected host still has an old root daemon from the withdrawn
  build, do not stop it during development. A reboot is required before locally
  trying the replacement package; this is not part of automated verification.

### Android

- Apply the same removal of the redundant `dns-in` inbound.
- Keep `versionName = 0.2.0` and `versionCode = 2000`.
- Build and verify one signed arm64 APK with the existing 0.2 signing key.
- Do not build or publish an AAB.
- Replace the existing GitHub `v0.2.0` release and tag only after APK tests,
  JNI descriptor checks, signature verification, and hash verification pass.

## Required regression tests

1. Linux and Android generated configurations contain no `dns-in`, no
   dokodemo-door, and no port 5353.
2. Both configurations retain the data-inbound port-53 to `dns-out` rule.
3. Linux nftables rules overwrite the split mark for system DNS, preserve the
   Xray dial mark, and contain no redirect.
4. Linux routing setup installs and removes the `0x2023` policy rule and TUN
   table transactionally.
5. DNS verification succeeds only for a valid matching DNS response and rolls
   policy back on failure.
6. Existing split-tunnel, reconnect, killswitch, package-layout, JNI, and APK
   gates remain green.

## Out of scope

- No systemd dependency is introduced.
- No live network testing is performed on the development host.
- No frontend redesign or Windows implementation is bundled into this
  re-release.
- General seamless daemon binary handover is a separate lifecycle feature. This
  hotfix uses the explicit protocol gate and one-time reboot requirement
  instead of attempting an unsafe in-place takeover.

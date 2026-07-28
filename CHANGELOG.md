# Changelog

## 0.2.5

- Add editable location details: exact source JSON for JSON profiles and
  structured parameters for URI-based locations.
- Refresh subscriptions only when their configured interval is due, allow
  automatic refresh to be disabled, and let provider updates replace local
  location edits.
- Add location dividers and a neutral globe for entries without a country flag,
  and remove the selected-location stripe.
- Simplify protocol labels: show only Hysteria or Hysteria2 for those protocols
  and omit redundant REALITY suffixes.

## 0.2.4

- Make one-shot HTTP latency checks use a composite location's deterministic
  fallback outbound instead of racing its cold load balancer and observatory.
- Match Xray's health-check request with an HTTP HEAD probe to the provider's
  gstatic 204 endpoint, reducing inflated latency and fixing Proxen USA probes.

## 0.2.3

- Send client-family subscription User-Agents as
  `<client>/<platform>/<architecture>` without an application version.
- Fix Happ and INCY compatibility with providers such as Proxen that select
  full Xray JSON profiles only when the client family is slash-delimited.
- Select bundled and downloadable Xray cores from the Linux target
  architecture instead of always assuming amd64.

## 0.2.2

- Preserve complete multi-outbound Xray profiles as one logical location,
  including provider balancers and observatories.
- Route every profile endpoint safely outside the Linux TUN and support
  balanced profiles in real HTTP latency checks.
- Add selectable Varmlen, Happ, INCY, and v2rayTun subscription User-Agents
  with a platform header and no app-version device churn.
- Keep provider JSON lossless and editable while retaining Varmlen's own DNS,
  split-tunnel, and kill-switch policy.
- Support Xray JSON outbounds for VLESS, VMess, Trojan, Shadowsocks, Hysteria,
  WireGuard, HTTP, and SOCKS; omit forbidden WireGuard stream settings.
- Stop grouping similarly named locations. Migrate local Configuration N cards
  into one flat Configuration/Configurations card without a network request.

## 0.2.1

- Use a stable platform-specific Varmlen subscription user agent without
  treating the app version as a separate device.
- Preserve provider location names, editable source JSON, and the exact Xray
  proxy outbound instead of flattening JSON subscriptions into a lossy model.
- Preserve Proxen XHTTP `extra`, mode, and XMUX settings.
- Group primary and backup variants under one expandable location.
- Accept Xray JSON outbounds for VMess, Trojan, Shadowsocks, Hysteria,
  WireGuard, HTTP, and SOCKS in addition to VLESS.
- Reject unsupported normalized protocols and transports instead of silently
  falling back to TCP.
- Reparse JSON already stored by 0.2.0 locally, without downloading the
  subscription again.
- Restore the AppImage for systems that already have Varmlen's privileged
  backend installed by the DEB or RPM package.

## 0.2.0

### Corrected Linux reissue

- Removed the fixed loopback DNS listener and its collision-prone port.
- Marked classic DNS traffic into a dedicated `varmlen0` policy route while
  keeping local stub resolvers reachable and blocking direct DNS/DoT fallback.
- Added independent recovery for the DNS policy route.
- Bumped the daemon protocol so the withdrawn port-based build is rejected
  before a connect command. Systems where that daemon is still running need one
  reboot after installing this corrected package.

### Security

- Replaced GUI-owned privileged networking with an authenticated, root-owned
  daemon and a bounded command protocol.
- Removed file capabilities and arbitrary privileged command/config paths.
- Made reconnect transactional and fail-closed, including crash recovery.
- Redirected system DNS into Xray and blocked direct DNS/DoT leakage even when
  LAN access is enabled, without requiring systemd-resolved.
- Added strict ownership, mode, listener, and configuration validation for
  privileged components.

### Split tunneling

- Applied bypass marks to both TCP and UDP sockets.
- Added cgroup-v2 process-tree tracking for native games and launchers.
- Added executable-open tracking for Proton/Windows game binaries.
- Added Flatpak command resolution and safe handling of existing sockets.

### Reliability

- Kept the tunnel alive independently of GUI restarts.
- Ignored stale frontend connection results after a newer disconnect/reconnect.
- Added daemon state recovery after GUI, Xray, or daemon crashes.
- Replaced per-ping Xray process launches with lightweight TCP probes.

### Packaging

- Added root-owned daemon, network helper, Xray, and polkit policy to DEB/RPM
  layouts.
- Removed duplicate privileged binaries from application resources.
- AppImage publishing is paused until a safe one-time daemon installer exists.

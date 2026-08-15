# Changelog

## 0.3.1

- Use hostname-based Cloudflare and Google DoH with static bootstrap addresses
  and parallel fallback, avoiding DNS stalls on routes that reject HTTPS to a
  bare IP while keeping every resolver connection inside the VPN.
- Accept provider hostname DNS only with an explicit public-IP bootstrap,
  preserve compatible provider DNS fields, and apply the Google APIs hostname
  compatibility mapping used by established Xray clients.
- Measure Hysteria2, WireGuard, mKCP and QUIC locations through their real
  proxy path instead of failing an inapplicable TCP-connect probe.
- Keep the package fallback on stable Xray 26.3.27. The first 0.3.1 assets
  briefly bundled prerelease 26.7.28 and were replaced; package upgrades move
  installations still on that default back to the bundled stable core while
  manually installed versions stay available for switching. Known trade-off:
  26.3.27 predates the Hysteria client-reuse and native-TUN UDP FullCone
  dataplane fixes, so QUIC-heavy apps on HY2 locations may remain partially
  offline.
- Preserve public literal-IP DNS servers from full JSON profiles, including
  ordinary UDP DNS such as `8.8.8.8`, and force them through the selected
  proxy instead of silently replacing them with Cloudflare DoH.
- Preserve a full JSON location's safe public DNS-over-HTTPS resolver instead
  of replacing it with Cloudflare, fixing Hysteria2 profiles whose tunnel can
  reach the provider resolver but not `1.1.1.1`.
- Require both HTTP and the location's effective resolver to work through the
  same concrete outbound before reporting latency, without making that
  synthetic probe a condition for connecting.
- Keep non-site provider routing such as protocol, public-IP, and port policy;
  Varmlen still owns website/app split, the final route, native TUN capture,
  LAN permission, and DNS leak prevention.

## 0.3.0

- Pin the package fallback to stable Xray 26.3.27 and always display it as a
  non-removable option, even when a newer downloaded core is active.
- Start the installed, root-owned networking daemon without asking the active
  desktop user for an administrator password again after every reboot.
- Treat every successful subscription response as authoritative: quota, usage,
  and expiry values omitted by the provider are now cleared instead of showing
  stale data from an earlier refresh.
- Harden the privileged daemon, subscription fetching, and IPC limits.
- Validate the final native Xray configuration and the selected profile's
  effective route before switching traffic; optional, fallback, balancer, and
  chained outbounds are no longer independently mandatory.
- Move validation-port ownership into the daemon, bound rotated logs, and keep
  protocol builders synchronized.
- Keep AppImage publishing paused until a safe standalone root-daemon bootstrap
  is available.

## 0.2.6

- Disable per-app split controls in Proxy mode and explain on hover, focus, or
  press that application routing requires TUN.
- Apply General and Selective website split rules to traffic sent through the
  local SOCKS proxy while keeping process rules out of Proxy mode.
- Remove the misleading manual Network permissions setup; privileged daemon
  startup remains lazy and is requested only by real operations.
- Correct the Proxy label and description to the actual SOCKS endpoint at
  `127.0.0.1:2081`.

## 0.2.5

- Add editable location details: exact source JSON for JSON profiles and
  structured parameters for URI-based locations.
- Refresh subscriptions only when their configured interval is due, allow
  automatic refresh to be disabled, and let provider updates replace local
  location edits.
- Add location dividers and a neutral globe for entries without a country flag,
  and remove the selected-location stripe.
- Make location dividers span the full card width using the page background
  color.
- Pretty-print valid location JSON in the editor and make reopening the same
  location toggle its details closed.
- Simplify protocol labels: show only Hysteria or Hysteria2 for those protocols
  and omit redundant REALITY suffixes.
- Rebuild the location editor around one modal lifecycle so JSON and structured
  editors can always be closed and reopened without stale touch layers.
- Populate finite editor fields from the Xray-supported protocol catalogue,
  including VMess, Trojan, Shadowsocks, Hysteria, WireGuard, HTTP, and SOCKS.
- Probe all locations concurrently and use the first healthy outbound from a
  composite JSON location instead of waiting on slower fallback paths.
- Keep long log lines inside the diagnostics dialog and draw a full-width
  divider above every location, including the first one.
- Build native DEB, RPM, and AppImage artifacts for both amd64 and arm64.

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

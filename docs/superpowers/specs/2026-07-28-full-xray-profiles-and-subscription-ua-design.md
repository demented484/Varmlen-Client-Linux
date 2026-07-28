# Full Xray Profiles and Subscription UA Design

## Scope

This change affects only the Linux and Android Varmlen client repositories.
The AegisVPN service repository is explicitly out of scope.

## Subscription identity

Settings gains one global `Subscription User-Agent` dropdown with four values:
`Varmlen`, `Happ`, `INCY`, and `v2rayTun`. `Varmlen` remains the default.
The Rust backend, not the WebView, constructs the final bounded header:

- `<brand> (Android; arm64)`
- `<brand> (Linux; x86_64)`

Every remote subscription request also sends `X-Device-OS: android` or
`X-Device-OS: linux`. No application version is sent. The selected identity
applies to the next import or explicit/background refresh; changing it does not
perform a network request by itself and does not reconnect the VPN.

## Full Xray JSON profiles

An object with `outbounds` is one logical location, even when it contains
multiple proxy outbounds. An array of such objects is a list of locations.
Varmlen stores the complete source object and an internal profile containing:

- every proxy outbound;
- `routing.balancers`;
- `observatory` and/or `burstObservatory`;
- the provider's catch-all proxy target.

Provider `inbounds`, DNS, logging, direct/block rules, and unrelated routing
rules are not adopted. Varmlen keeps ownership of TUN, DNS anti-leak, LAN,
killswitch, and split-tunnel routing. Rules that mean “use the VPN” target the
profile balancer when present, otherwise the profile's proxy outbound.

The supported Xray proxy protocols are `http`, `socks`, `shadowsocks`, `vmess`,
`vless`, `trojan`, `hysteria`, and `wireguard`. Their raw JSON is preserved
instead of normalized and regenerated. `freedom`, `blackhole`, `dns`, and
`loopback` remain Varmlen-owned utility protocols and are not displayed as
locations.

Every stream-capable proxy outbound receives Varmlen's Linux dial mark after
unsafe source binding is removed. WireGuard is preserved without
`streamSettings`, which Xray forbids; Linux resolves and pins every profile
endpoint to the physical route. Android already excludes Varmlen's own package
from its VPN capture.

Malformed profiles fail with an explicit error. Varmlen never silently falls
back to the first outbound or to TCP.

## Display and manual configurations

Subscription locations remain flat. The primary/backup grouping component is
removed, so `Нидерланды` and `Нидерланды [Запасной]` are independent rows.

All manually pasted URI and JSON configurations share one local card:

- `Configuration` when it contains one logical location;
- `Configurations` when it contains more than one.

Each full Xray profile is one row regardless of its internal outbound count.
Existing non-remote `Configuration N` cards are merged locally on load without
network access. Remote subscriptions remain separate cards.

The per-location JSON editor shows and updates the complete provider profile.
Mixed-protocol profiles display `XRAY / BALANCER / JSON`; single-protocol
profiles retain the protocol name and add `BALANCER / JSON` when applicable.

## Verification

Tests use a credential-free version of the supplied Estonia profile with seven
VLESS outbounds, a least-ping balancer, and burst observatory. Required
regressions cover:

- one profile parses as one location;
- all seven outbounds, balancer, and observatory survive config generation;
- Varmlen DNS/TUN/split rules remain authoritative;
- all eight proxy protocols survive raw JSON generation;
- WireGuard receives no illegal `streamSettings`;
- every profile endpoint is resolved for Linux bypass routing;
- UA choice is validated and produces platform headers;
- backup rows are not grouped;
- legacy manual cards merge into `Configuration`/`Configurations`.

No live connect, disconnect, package installation, or active-VPN lifecycle test
is permitted. Verification is limited to unit/contract tests, static checks,
package inspection, and builds.

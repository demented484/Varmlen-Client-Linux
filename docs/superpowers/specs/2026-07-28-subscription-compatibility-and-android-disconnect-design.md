# Subscription Compatibility and Android Disconnect Design

## Goal

Make Linux and Android Varmlen consume real Xray JSON subscriptions without
discarding provider fields, make Proxen's XHTTP and backup locations behave as
intended, use a stable versionless subscription User-Agent, and make Android
disconnect complete only after the VPN data plane is actually down.

## Global constraints

- Never connect, disconnect, restart, or otherwise alter the developer's active
  Linux VPN while implementing or verifying these changes.
- Keep Linux and Android subscription parsing/config generation behavior in
  sync.
- Do not identify a device by an application version.
- Do not silently reinterpret an unsupported protocol as VLESS or an
  unsupported transport as TCP.
- Treat subscription JSON as untrusted input: provider inbounds, DNS, routing,
  logging, and policy must not replace Varmlen's own network policy.

## Stable User-Agent

Subscription requests use:

```text
Varmlen (Linux; x86_64)
Varmlen (Android; arm64)
```

The AegisVPN response classifier accepts both this stable form and the legacy
`Varmlen/<version> (...)` form so installed 0.2.0 clients keep receiving the
expected response. Version and update checks remain application metadata and
are not sent as part of subscription identity.

## Real JSON locations

Each parsed server gains two optional fields:

- `source_json`: the exact JSON object that represented the location in the
  subscription.
- `raw_outbound`: the exact proxy outbound selected from that object.

For an AegisVPN array, every standalone config object is retained independently.
The UI formats `source_json` in the location editor instead of serializing
Varmlen's normalized `VlessServer`. Saving reparses the edited JSON and requires
exactly one connectable location.

Connection generation never adopts the provider's top-level `inbounds`, `dns`,
`routing`, `log`, or `policy`. It only reuses the selected `raw_outbound`,
forces its tag to `proxy`, and injects Varmlen's anti-loop socket mark. This
preserves provider-owned outbound details such as XHTTP `xmux` without allowing
the subscription to replace Varmlen's kill switch, DNS, or split-tunnel policy.

Rows originating from JSON append ` / JSON` to the protocol/transport/security
summary. Share-link and Base64 subscriptions do not receive that suffix.

## Proxen compatibility

Proxen's VLESS URI carries URL-encoded JSON in the `extra` query parameter. For
XHTTP, Varmlen parses that object and merges it into `xhttpSettings`. Explicit
URI fields (`path`, `mode`, and `host`) override the same fields from `extra`;
all other fields, including `xmux`, survive unchanged. Invalid or non-object
`extra` is rejected during configuration generation instead of being ignored.

Repeated locations are grouped conservatively by label. Recognized variant
suffixes include bracketed or parenthesized `Backup`, `Reserve`, `Alt`, their
Russian equivalents, and numeric variants such as `#2` or `[2]`. A group is
created only when at least two imported rows reduce to the same base label.
`Auto-select` remains standalone. A grouped parent selects the primary (first)
server and can expand to expose every concrete server, each with its own
selection, ping, and details.

## Protocol behavior

Typed URI support remains VLESS, VMess, Trojan, and Shadowsocks in this patch.
JSON locations may reuse any raw outbound accepted by the bundled Xray core,
provided Varmlen can derive a host and port for display and probing. Initially
the JSON metadata parser recognizes VLESS, VMess, Trojan, Shadowsocks,
Hysteria, WireGuard, HTTP, and SOCKS.

Unknown typed protocols and transports return an explicit error. Internal
Freedom, Blackhole, DNS, and Loopback outbounds are never exposed as selectable
locations.

## Android disconnect lifecycle

The plugin sends an explicit `ACTION_DISCONNECT` intent containing a request ID.
It keeps the Tauri invocation pending until the service broadcasts
`running=false` with that same ID. A timeout rejects the invocation if the
service never confirms.

The service clears the desired-running flag, stops tun2socks, stops Xray,
closes the TUN descriptor, removes the foreground notification, and only then
broadcasts the disconnected state. Teardown is idempotent so `onDestroy()` does
not recursively repeat the stop sequence.

## Verification

- Rust parser tests cover exact JSON retention and Proxen-style `extra`.
- Xray config tests prove `xmux` survives and unknown transports fail.
- TypeScript tests cover `/ JSON` and conservative backup grouping.
- Android JVM tests cover matching request-ID completion semantics.
- AegisVPN tests cover stable and legacy Varmlen User-Agents.
- Full Rust, frontend, Android unit, Linux bundle, and Android APK builds run
  without touching the active VPN.


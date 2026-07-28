# Linux–Android parity for 0.2.5

## Goal

Bring the Linux client’s shared location UI and Xray probing behavior up to the
already tested Android 0.2.5 implementation, publish native amd64 and arm64
Linux packages, and preserve every Linux-only daemon, DNS, split-tunnelling and
killswitch path.

## Scope

- Use one page-level modal state and one close dispatcher for location editors,
  subscription JSON, import, rename and information dialogs.
- Populate finite location-editor choices from the Rust/Xray catalogue and use
  the Varmlen `Dropdown` component instead of native HTML selects.
- Cover all outbound protocols accepted by the existing Xray builder, including
  VLESS, VMess, Trojan, Shadowsocks, Hysteria2, WireGuard, HTTP and SOCKS.
- Probe all locations concurrently. For a composite JSON location, expose one
  temporary SOCKS inbound per concrete proxy and return the first successful
  HTTP health-check latency.
- Draw a full-width divider above every location, including the first.
- Keep long logs clipped and scrollable inside the log modal.
- Build amd64 and arm64 DEB, RPM and AppImage artifacts from native GitHub
  runners and replace the existing prerelease assets under tag `v0.2.5`.

## Architecture

Shared frontend and Xray catalogue/probe code is ported selectively from
`Varmlen-Client-Android`. Linux-specific process ownership remains in
`varmlend`; no Android VPN service code is copied. The Linux `proxy_get_ping`
command may start temporary Xray processes, but connection, disconnect, DNS,
killswitch and routing commands remain unchanged.

The editor receives a mutable `LocationEditDraft` owned by the page-level modal
controller. Rust exposes serializable editor choices generated from the same
catalogue used by outbound builders. This removes duplicated protocol lists
without making the frontend parse Xray source code.

The release workflow uses native `ubuntu-24.04` and `ubuntu-24.04-arm` runners.
Each runner fetches the matching Xray binary through the existing
architecture-aware script, builds all three package formats, validates package
contents, and uploads architecture-suffixed artifacts.

## Safety and compatibility

- Do not start, stop, reconnect or inspect the user’s active VPN.
- Do not invoke live DNS, routing, leak or public-IP tests.
- Keep package/app version `0.2.5`; keep the GitHub release a prerelease.
- Do not modify the subscription service or AegisVPN repositories.
- Preserve the existing release signing and repository history conventions;
  commit messages contain no AI markers.

## Error handling

- Invalid location drafts remain editable and surface the current validation
  error without replacing the saved location.
- Failure of one proxy variant does not delay a successful sibling variant.
- A composite probe returns an error only after all variants fail.
- Workflow jobs fail before release upload if tests, package validation, or an
  expected artifact is missing.

## Verification

- Frontend contract/component tests cover modal close sequences, Xray-derived
  dropdowns, first-row separators and log containment.
- Rust tests cover catalogue/builder parity, one inbound per composite proxy,
  and first-success ping behavior.
- Existing daemon, DNS and split-tunnel tests remain green.
- Both architectures run package-content checks.
- Downloaded GitHub assets are compared against workflow-produced checksums and
  inspected for package architecture.

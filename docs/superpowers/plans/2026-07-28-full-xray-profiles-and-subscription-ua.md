# Full Xray Profiles and Subscription UA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Proxen-style multi-outbound Xray profiles work as one location, add four selectable subscription identities, remove location grouping, and merge manual configurations.

**Architecture:** Rust parses and validates a lossless `raw_profile`, then merges only its proxy data plane into Varmlen-owned TUN/DNS/split routing. Svelte persists a bounded global UA choice and maintains one local manual-configuration card. Linux resolves every profile endpoint; Android keeps its existing package-level VPN bypass.

**Tech Stack:** Rust, serde_json, reqwest, Tauri 2, Svelte 5, TypeScript, Vitest, Kotlin/Android VpnService.

## Global Constraints

- Modify only `varmlen` and `Varmlen-Client-Android`; never modify AegisVPN.
- Do not connect, disconnect, install, restart, or otherwise disturb the active Linux VPN.
- User-Agent choices are exactly `Varmlen`, `Happ`, `INCY`, and `v2rayTun`; default is `Varmlen`; no application version is sent.
- Full JSON supports Xray proxy protocols `http`, `socks`, `shadowsocks`, `vmess`, `vless`, `trojan`, `hysteria`, and `wireguard`.
- Provider inbounds, DNS, direct/block rules, and unrelated routing rules never replace Varmlen policy.
- Do not create AI/co-author markers in commits.

---

### Task 1: Subscription identity

**Files:**
- Modify: `src/lib/settings.svelte.ts`
- Modify: `src/routes/settings/+page.svelte`
- Modify: `src/lib/i18n.svelte.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/subs.svelte.ts`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`
- Test: `src/lib/settings.test.ts`

**Interfaces:**
- Produces: `SubscriptionUserAgent = "varmlen" | "happ" | "incy" | "v2raytun"`.
- Produces: Rust `subscription_headers(choice) -> Result<(String, &'static str), String>`.
- Consumes: `fetch_subscription(url, subscription_user_agent)`.

- [ ] Write Rust tests asserting all four bounded UA strings, platform OS header, default Varmlen, and rejection of arbitrary input.
- [ ] Run `cargo test subscription_ua --manifest-path src-tauri/Cargo.toml` and verify the new tests fail.
- [ ] Write frontend persistence tests for the default and four allowed values.
- [ ] Run `npm test -- settings.test.ts` and verify failure.
- [ ] Implement the persisted setting, dropdown, translations, API argument, validated Rust headers, and `X-Device-OS`.
- [ ] Run the focused Rust and frontend tests and verify they pass.

### Task 2: Lossless full-profile parsing

**Files:**
- Modify: `src-tauri/src/subscription.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/subscription-json.ts`
- Test: `src-tauri/src/subscription.rs`

**Interfaces:**
- Produces: `VlessServer.raw_profile: Option<Value>`.
- Produces: `profile_proxy_outbounds(profile) -> Result<Vec<Value>, String>`.
- Produces: best-effort metadata for all eight official Xray proxy protocols.

- [ ] Add a credential-free seven-outbound Estonia fixture with balancer and burst observatory.
- [ ] Add tests asserting the fixture parses as one location and an array of two profiles parses as two locations.
- [ ] Add table tests for all eight proxy protocols and modern/legacy settings shapes.
- [ ] Run the focused subscription tests and verify they fail because one profile is currently split into seven rows.
- [ ] Implement full-profile detection, one-location parsing, lossless storage, and metadata extraction.
- [ ] Run the focused tests and verify they pass.

### Task 3: Merge the profile into Varmlen routing

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Modify: `src-tauri/src/vpn.rs`
- Modify: `daemon/src/system.rs`
- Test: `src-tauri/src/xray.rs`
- Test: `src-tauri/src/vpn.rs`
- Test: `daemon/src/system.rs`

**Interfaces:**
- Produces: `ProxyTarget` selecting either `outboundTag` or `balancerTag`.
- Produces: `profile_endpoint_hosts(server) -> Vec<(String, u16)>`.
- Consumes: `VlessServer.raw_profile`.

- [ ] Add tests asserting seven proxy outbounds, balancer, burst observatory, Varmlen DNS/inbounds, and balancer-targeted default/DoH rules.
- [ ] Add one raw JSON generation test per official proxy protocol.
- [ ] Add a WireGuard test asserting no injected `streamSettings` and all endpoint hosts are returned.
- [ ] Run focused Xray/VPN tests and verify failure.
- [ ] Implement validated profile extraction, safe tag preservation, dial marking for stream-capable outbounds, balancer routing, and endpoint collection.
- [ ] Update daemon ping-document validation to accept a valid balancer profile while retaining loopback-only and no-direct-route invariants.
- [ ] Run focused tests and verify they pass.

### Task 4: Flat locations and one manual card

**Files:**
- Delete: `src/lib/components/GroupedServerList.svelte`
- Delete: `src/lib/location-groups.ts`
- Delete: `src/lib/location-groups.test.ts`
- Create: `src/lib/manual-configurations.ts`
- Create: `src/lib/manual-configurations.test.ts`
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/subs.svelte.ts`
- Modify: `src/lib/i18n.svelte.ts`

**Interfaces:**
- Produces: `mergeManualConfigurations(subscriptions)`.
- Produces: `manualConfigurationName(serverCount)`.

- [ ] Write tests asserting backup rows stay independent and legacy non-remote `Configuration N` cards merge without remote subscriptions.
- [ ] Run the focused Vitest file and verify failure.
- [ ] Implement the migration and append new manual imports to the shared card.
- [ ] Replace grouped rendering with the original flat row rendering.
- [ ] Run the focused tests and verify they pass.

### Task 5: Mirror and verify both clients

**Files:**
- Modify: matching files under `Varmlen-Client-Android`
- Modify: both `CHANGELOG.md`, package manifests, Cargo manifests, and Tauri config for version `0.2.2`.

**Interfaces:**
- Consumes all prior task interfaces unchanged on both platforms.

- [ ] Mirror shared Rust/Svelte changes into Android and keep platform-specific VPN code unchanged.
- [ ] Run Linux `cargo test --workspace --all-targets`, `npm test`, and `npm run check`.
- [ ] Run Android Rust tests, `npm test`, `npm run check`, Android manifest/JNI/VPN contract tests, and Gradle unit tests.
- [ ] Build Linux DEB/RPM/AppImage and signed Android ARM64 APK without AAB.
- [ ] Inspect versions, signatures, layouts, SHA-256 checksums, and `git diff --check`.
- [ ] Commit without AI markers, push both client repositories, and create GitHub releases only after all non-live checks pass.

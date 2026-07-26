# Portless DNS 0.2.0 Re-release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the unnecessary local Xray DNS listener, force classic Linux
DNS through `varmlen0`, and replace both public 0.2.0 releases.

**Architecture:** Xray keeps only its data inbound and routes data-inbound
port-53 traffic to `dns-out`. Linux nftables overwrites non-Xray DNS traffic
with mark `0x2023`, while policy routing sends that mark to `varmlen0`; the
daemon verifies DNS with a bounded external query rather than a loopback
listener. Android relies on its existing VpnService DNS route and the same
data-inbound rule.

**Tech Stack:** Rust, Tokio, nftables/iproute2 rule rendering, Tauri 2,
Kotlin/Android Gradle, shell artifact gates, GitHub CLI.

## Global Constraints

- Keep Linux and Android public versions at `0.2.0`.
- Keep Android `versionCode = 2000`.
- Publish no Android AAB.
- Do not stop, reconnect, inspect, or mutate the host's active VPN, DNS,
  nftables, routes, or Varmlen processes.
- Run only unit, static, packaging, and artifact-level verification.
- Do not add AI attribution or co-author markers to commits.
- Do not use subagents.

---

### Task 1: Remove the Xray DNS listener

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Test: inline Rust tests in `src-tauri/src/xray.rs`

**Interfaces:**
- Consumes: `build_xray_config(...) -> serde_json::Value`.
- Produces: one data inbound and a data-inbound port-53 rule to `dns-out`.

- [ ] **Step 1: Write the failing configuration test**

Change `dns_routes_through_proxy_no_leak` to assert:

```rust
assert_eq!(cfg["inbounds"].as_array().unwrap().len(), 1);
assert!(cfg["inbounds"].as_array().unwrap().iter().all(|inbound| {
    inbound["tag"] != "dns-in" && inbound["protocol"] != "dokodemo-door"
}));
assert!(!serde_json::to_string(&cfg).unwrap().contains("5353"));
```

Keep an assertion that the rule whose `inboundTag` contains `tun-in` and whose
port is 53 uses `outboundTag = "dns-out"`.

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml dns_routes_through_proxy_no_leak -- --exact
```

Expected: FAIL because the configuration still contains `dns-in` on port 5353.

- [ ] **Step 3: Remove the listener and its routing rule**

Make `build_inbounds` return only the TUN or SOCKS data inbound. Remove the
`dns-in -> dns-out` rule; retain the first data-inbound port-53 rule.

- [ ] **Step 4: Run GREEN**

Run the focused command again and expect PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/xray.rs
git commit -m "Remove local Xray DNS listener"
```

### Task 2: Route Linux DNS into the TUN without a port

**Files:**
- Modify: `daemon/src/nft.rs`
- Modify: `daemon/src/dns.rs`
- Modify: `daemon/src/system.rs`
- Modify: `helper/src/main.rs`

**Interfaces:**
- Produces: `DNS_MARK = 0x2023`.
- Produces: `render_dns_rules() -> String` with no port argument.
- Produces: `DnsPlan` with no local-listener state.
- Consumes: helper `route-up`/`route-down` to add and remove the DNS mark route.

- [ ] **Step 1: Write failing nftables tests**

Assert the rendered rules:

```rust
assert!(!rules.contains("redirect"));
assert!(!rules.contains("5353"));
assert!(rules.contains("priority mangle + 10"));
assert!(rules.contains("meta mark & 0x0000ffff != 0x2024"));
assert!(rules.contains("meta mark set 0x2023"));
assert!(rules.contains("ct mark set meta mark"));
assert!(rules.contains("meta mark 0x2023 oifname \"varmlen0\""));
```

- [ ] **Step 2: Run nft RED**

Run:

```bash
cargo test -p varmlend nft::tests -- --nocapture
```

Expected: FAIL because the rules still redirect to port 5353.

- [ ] **Step 3: Write failing helper route tests**

Extract pure route command rendering where needed and assert that setup contains
an IPv4 default `dev varmlen0` in DNS table `101` and an `ip rule` for mark
`0x2023`, while teardown removes the rule and flushes table `101`.

- [ ] **Step 4: Run helper RED**

Run:

```bash
cargo test -p varmlen-net dns_mark
```

Expected: FAIL because no DNS policy route exists.

- [ ] **Step 5: Implement DNS marking and policy routing**

Render an nft output route chain after split marking. Preserve mark `0x2024`;
overwrite other TCP/UDP port-53 marks with `0x2023`; accept DNS only on
`varmlen0`; reject direct 53 and 853. Add helper setup/teardown for table 101
and mark `0x2023`.

- [ ] **Step 6: Replace listener verification with tunnel verification**

Remove `listener_is_ready`. `DnsGuard::install` applies policy, sends the
bounded query through the backend, validates its matching response, and removes
policy on failure. Update `SystemLifecycleBackend` and privileged Xray document
validation to require exactly one data inbound and no local DNS port.

- [ ] **Step 7: Run GREEN**

Run:

```bash
cargo test -p varmlend
cargo test -p varmlen-net
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Expected: all pass without touching host network state.

- [ ] **Step 8: Commit**

```bash
git add daemon helper src-tauri/src/xray.rs src-tauri/src/vpn.rs
git commit -m "Route Linux DNS through the TUN"
```

### Task 3: Reject the withdrawn daemon protocol

**Files:**
- Modify: `daemon/src/protocol.rs`
- Modify: `src-tauri/src/daemon_client.rs`
- Test: inline tests in both files

**Interfaces:**
- Produces: `PROTOCOL_VERSION = 2`.
- Produces: an explicit stale-daemon/reboot error before `Connect`.

- [ ] **Step 1: Write the failing client test**

Serve a protocol-1 status response and assert that the compatibility probe
returns an `Unavailable` error containing `reboot` instead of returning a
usable client.

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml stale_daemon -- --nocapture
```

Expected: FAIL because the client currently accepts any reachable socket until
its first normal command.

- [ ] **Step 3: Implement the compatibility probe**

Bump the protocol to 2. After opening an installed socket, issue `Status` before
returning the client. Map protocol/version mismatch to an explicit one-time
reboot message and never send `Connect` to that daemon.

- [ ] **Step 4: Run GREEN and commit**

Run the focused test and the daemon/client suites, then:

```bash
git add daemon/src/protocol.rs src-tauri/src/daemon_client.rs
git commit -m "Reject outdated Linux VPN daemons"
```

### Task 4: Apply the portless configuration to Android

**Files:**
- Modify: `/home/daniil/projects/VPN-Client/Varmlen-Client-Android/src-tauri/src/xray.rs`
- Test: inline Rust tests in that file

**Interfaces:**
- Produces: one Android `socks-in` data inbound with port-53 routing to
  `dns-out`.

- [ ] **Step 1: Write and run the Android RED test**

Add the same no-`dns-in`, no-dokodemo-door, no-5353 assertions and run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml dns_routes_through_proxy_no_leak -- --exact
```

Expected: FAIL against the current listener.

- [ ] **Step 2: Remove the listener and run GREEN**

Remove `dns-in` and its routing rule, then run the focused and complete Rust
tests.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/xray.rs
git commit -m "Remove redundant Android DNS listener"
```

### Task 5: Verify, build, and replace both releases

**Files:**
- Modify: Linux and Android `CHANGELOG.md` if release notes need correction.
- Produce: Linux DEB, RPM, and AppImage.
- Produce: Android signed arm64 APK only.

**Interfaces:**
- Produces: public Linux and Android `v0.2.0` releases.

- [ ] **Step 1: Run complete Linux offline gates**

Run npm tests/check/audit, workspace Rust tests/clippy, manifest/package scripts,
and `git diff --check`. Do not install the package or invoke VPN commands.

- [ ] **Step 2: Build and verify Linux artifacts**

Build DEB, RPM, and AppImage. Inspect package ownership/layout, metadata,
version, embedded binaries, and SHA-256 hashes.

- [ ] **Step 3: Run complete Android offline gates**

Run npm tests/check/audit, Rust tests/clippy, Gradle unit tests, manifest,
VPN-contract, native dependency, and APK-content/JNI gates.

- [ ] **Step 4: Build and verify the Android APK**

Use the external 0.2 signing configuration. Verify version `0.2.0`, code
`2000`, arm64 ABI, JNI descriptors, APK Signature Scheme v2/v3, expected
certificate SHA-256, and artifact SHA-256. Do not produce an AAB.

- [ ] **Step 5: Push both repositories**

Push verified `main` commits and compare local and remote commit IDs.

- [ ] **Step 6: Replace Linux `v0.2.0`**

Delete only the Linux GitHub release and tag, recreate the tag at verified
Linux `main`, publish the three verified artifacts, and compare remote
names/sizes/hashes.

- [ ] **Step 7: Replace Android `v0.2.0`**

Delete only the Android GitHub release and tag, recreate the tag at verified
Android `main`, publish exactly the verified APK, and compare remote
name/size/hash.

- [ ] **Step 8: Final verification**

Confirm both releases are public, non-draft, non-prerelease, point at the
expected commits, and contain no AAB. Keep the host VPN untouched.

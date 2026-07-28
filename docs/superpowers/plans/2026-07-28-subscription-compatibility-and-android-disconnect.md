# Subscription Compatibility and Android Disconnect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve real provider JSON, make Proxen XHTTP/grouped locations work, stabilize subscription UA, and make Android disconnect wait for completed teardown.

**Architecture:** Rust retains a sanitized raw outbound alongside normalized display metadata, while the frontend derives grouped presentation without changing persisted subscription order. Android uses a request-ID lifecycle for both connect and disconnect. AegisVPN recognizes stable and legacy Varmlen UA forms.

**Tech Stack:** Rust, serde_json, Svelte 5, TypeScript, Vitest, Kotlin, Android VpnService, JUnit 4, Python, pytest.

## Global Constraints

- Do not connect, disconnect, restart, or alter the active Linux VPN.
- Apply equivalent parser/config/frontend changes to both client repositories.
- Use tests before production changes.
- Preserve unrelated untracked files in `/home/daniil/projects/VPN`.
- Do not add AI attribution to commits.

---

### Task 1: Stable subscription User-Agent

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `../Varmlen-Client-Android/src-tauri/src/lib.rs`
- Modify: `../../VPN/bot/src/main.py`
- Test: `src-tauri/src/lib.rs`
- Test: `../Varmlen-Client-Android/src-tauri/src/lib.rs`
- Test: `../../VPN/bot/tests/test_subscription_response.py`

**Interfaces:**
- Produces: `subscription_user_agent() -> String` returning `Varmlen (<platform>; <arch>)`.
- Produces: `client_wants_xray_json(str) -> bool` accepting stable and legacy Varmlen forms.

- [ ] **Step 1: Change Rust and Python assertions to the stable UA**

```rust
assert_eq!(subscription_user_agent(), format!("Varmlen ({}; {})", target_platform(), target_arch()));
```

```python
assert client_wants_xray_json("Varmlen (Linux; x86_64)")
assert client_wants_xray_json("Varmlen/0.2.0 (Android; arm64)")
```

- [ ] **Step 2: Run the focused tests and verify they fail on versioned output**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml subscription_user_agent
cargo test --manifest-path ../Varmlen-Client-Android/src-tauri/Cargo.toml subscription_user_agent
cd ../../VPN/bot && uv run pytest tests/test_subscription_response.py -q
```

- [ ] **Step 3: Remove the version from both Rust UA builders and make the backend matcher explicit**

```rust
format!("Varmlen ({}; {})", target_platform(), target_arch())
```

The Python helper accepts prefixes `varmlen (` and `varmlen/` before consulting
the other client substring rules.

- [ ] **Step 4: Re-run all three focused test commands**

- [ ] **Step 5: Commit the backend change without staging `AGENTS.md` or `landing/`**

```bash
git add bot/src/main.py bot/tests/test_subscription_response.py
git commit -m "Handle stable Varmlen subscription user agent"
```

### Task 2: Real JSON retention and safe outbound reuse

**Files:**
- Modify: `src-tauri/src/subscription.rs`
- Modify: `src-tauri/src/xray.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/subscription-json.ts`
- Modify: `src/lib/subscription-json.test.ts`
- Modify: `src/routes/+page.svelte`
- Apply the same changes under `../Varmlen-Client-Android/`.

**Interfaces:**
- Produces: `VlessServer.source_json: Option<String>`.
- Produces: `VlessServer.raw_outbound: Option<serde_json::Value>`.
- Produces: `build_proxy_outbound(&VlessServer) -> Result<Value, String>`.
- Consumes: existing `parse_subscription_body` command for edited source JSON.

- [ ] **Step 1: Add Rust tests proving an Aegis-style config retains its exact object and XHTTP XMUX**

The fixture contains one full config with `remarks`, a VLESS proxy outbound,
direct/block outbounds, and `xhttpSettings.xmux.hKeepAlivePeriod = 15`.
Assertions require one server, non-empty `source_json`, a proxy-only
`raw_outbound`, and generated XMUX plus the anti-loop mark.

- [ ] **Step 2: Run focused Rust tests in both repositories and verify missing fields fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml json_location
cargo test --manifest-path ../Varmlen-Client-Android/src-tauri/Cargo.toml json_location
```

- [ ] **Step 3: Extend `VlessServer` and JSON traversal**

When traversing an array, serialize each object item as its location source.
When parsing a recognized proxy outbound, retain that exact outbound. Ignore
top-level direct/block/DNS utility outbounds.

- [ ] **Step 4: Reuse only `raw_outbound` in Xray generation**

Clone the object, force `tag = "proxy"`, and merge
`streamSettings.sockopt.mark = XRAY_DIAL_MARK`. Return an error for unsupported
normalized protocols instead of defaulting to VLESS.

- [ ] **Step 5: Add frontend tests for real JSON formatting and `/ JSON`**

```ts
expect(formatLocationJson(serverWithSourceJson)).toContain('"outbounds"');
expect(transportSummary(serverWithSourceJson)).toBe("VLESS / XHTTP / REALITY / JSON");
```

- [ ] **Step 6: Run Vitest and verify failures**

```bash
npm test -- src/lib/subscription-json.test.ts
cd ../Varmlen-Client-Android && npm test -- src/lib/subscription-json.test.ts
```

- [ ] **Step 7: Make the location editor reparse source JSON asynchronously**

Source-JSON rows call `parseSubscriptionBody`, require exactly one server, and
store the reparsed server. Non-JSON rows keep the existing normalized editor.

- [ ] **Step 8: Run Rust, Vitest, and Svelte checks in both repositories**

### Task 3: Proxen XHTTP `extra`

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Test: `src-tauri/src/xray.rs`
- Apply the same changes under `../Varmlen-Client-Android/`.

**Interfaces:**
- Consumes: `VlessServer.raw_params["extra"]`.
- Produces: complete `xhttpSettings` retaining arbitrary valid provider keys.

- [ ] **Step 1: Add a failing Proxen-style test**

Parse a VLESS URI with URL-encoded:

```json
{"mode":"packet-up","xmux":{"maxConcurrency":1,"hKeepAlivePeriod":30}}
```

Assert generated `mode`, `xmux.maxConcurrency`, and `xmux.hKeepAlivePeriod`.

- [ ] **Step 2: Run the focused test in both repositories and verify XMUX is absent**

- [ ] **Step 3: Parse `extra` as an object and merge it before explicit URI fields**

Malformed JSON or a non-object value returns a configuration error.

- [ ] **Step 4: Re-run focused and complete Rust tests**

### Task 4: Group backup locations

**Files:**
- Create: `src/lib/location-groups.ts`
- Create: `src/lib/location-groups.test.ts`
- Modify: `src/lib/subs.svelte.ts`
- Modify: `src/routes/+page.svelte`
- Apply the same changes under `../Varmlen-Client-Android/`.

**Interfaces:**
- Produces: `groupLocations(servers: ServerEntry[]) -> LocationGroup[]`.
- Produces: `LocationGroup { id, name, flag, servers }`.

- [ ] **Step 1: Add tests for Proxen names**

Tests require `Netherlands` and `Netherlands [Backup]` to form one group,
`Auto-select` to stay independent, original order to remain stable, and an
unmatched single numbered name to remain unmodified.

- [ ] **Step 2: Run the focused Vitest files and verify grouping is absent**

- [ ] **Step 3: Implement conservative suffix normalization**

Only collapse a candidate base when at least two entries share it. Preserve
every concrete `ServerEntry`.

- [ ] **Step 4: Render grouped parents with expandable concrete children**

Clicking a parent selects its first server. Its active state reflects any
selected child. Expanded children retain individual ping and detail controls.

- [ ] **Step 5: Run Vitest and Svelte checks in both repositories**

### Task 5: Confirmed Android disconnect

**Files:**
- Create: `../Varmlen-Client-Android/src-tauri/gen/android/app/src/main/java/app/varmlen/client/VpnRequestCompletion.kt`
- Create: `../Varmlen-Client-Android/src-tauri/gen/android/app/src/test/java/app/varmlen/client/VpnRequestCompletionTest.kt`
- Modify: `../Varmlen-Client-Android/src-tauri/gen/android/app/src/main/java/app/varmlen/client/VpnPlugin.kt`
- Modify: `../Varmlen-Client-Android/src-tauri/gen/android/app/src/main/java/app/varmlen/client/VarmlenVpnService.kt`

**Interfaces:**
- Produces: request matching that distinguishes connect success from disconnect success.
- Produces: `VarmlenVpnService.stop(Context, requestId)` sending `ACTION_DISCONNECT`.

- [ ] **Step 1: Add JVM tests for request matching**

Tests require a disconnect to resolve only for matching request ID plus
`running=false`; a stale broadcast and `running=true` are ignored.

- [ ] **Step 2: Run `:app:testUniversalDebugUnitTest` and verify the missing helper fails**

- [ ] **Step 3: Add pending disconnect state and explicit action intent**

Reject superseded requests, use a 15-second timeout, and resolve only from the
matching state broadcast.

- [ ] **Step 4: Reorder and guard service teardown**

Broadcast `running=false` after tun2socks, Xray, TUN, and foreground teardown.
Guard repeated `stopAll()` calls from `onDestroy()`.

- [ ] **Step 5: Run Android JVM tests**

### Task 6: Full verification and artifacts

**Files:**
- Verify all changed files in all three repositories.

**Interfaces:**
- Produces: Linux `.deb` and AppImage plus Android universal APK.

- [ ] **Step 1: Run complete backend tests relevant to subscriptions**

```bash
cd /home/daniil/projects/VPN/bot
uv run pytest tests/test_subscription_response.py tests/test_subscription_service.py tests/test_hy2_subscription.py -q
```

- [ ] **Step 2: Run complete frontend and Rust suites in both clients**

```bash
npm test && npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 3: Run Android JVM tests and build the universal release APK**

```bash
cd src-tauri/gen/android
./gradlew :app:testUniversalDebugUnitTest :app:assembleUniversalRelease
```

- [ ] **Step 4: Build Linux release bundles without starting or stopping Varmlen**

```bash
npm run tauri build
```

- [ ] **Step 5: Inspect diffs, generated artifact paths, and repository status**

- [ ] **Step 6: Commit client changes with ordinary human-readable messages**


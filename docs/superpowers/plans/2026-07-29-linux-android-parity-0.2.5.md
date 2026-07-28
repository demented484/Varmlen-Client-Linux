# Linux–Android parity 0.2.5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the tested shared Android editor, modal and proxy-ping behavior to Linux and publish native amd64/arm64 prerelease packages.

**Architecture:** Keep Linux connection ownership in `varmlend` and selectively port only shared frontend, Xray catalogue and temporary ping-process behavior. Build each Linux architecture on a matching native GitHub runner.

**Tech Stack:** Svelte 5, TypeScript, Vitest, Rust, Tauri 2, Xray, GitHub Actions, DEB/RPM/AppImage.

## Global Constraints

- Do not start, stop, reconnect or inspect the active VPN.
- Do not run live DNS, route, leak or public-IP tests.
- Keep Linux daemon, DNS, killswitch and split-tunnel control paths intact.
- Keep version `0.2.5` and publish only as a GitHub prerelease.
- Build native amd64 and arm64 artifacts; do not hardcode Xray to x86_64.
- Use no AI marker in commit messages.

---

### Task 1: Xray-derived location editor

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/location-draft.ts`
- Modify: `src/lib/components/LocationEditor.svelte`
- Create: `src/lib/components/Dropdown.svelte`
- Create: `src/lib/location-editor-options.ts`
- Test: `src-tauri/src/xray.rs`
- Test: `src/lib/location-draft.test.ts`
- Test: `src/lib/location-editor-options.test.ts`
- Test: `src/lib/components/Dropdown.svelte.test.ts`
- Test: `src/lib/components/LocationEditor.svelte.test.ts`

**Interfaces:**
- Produces Rust command `location_editor_options() -> LocationEditorOptions`.
- Produces frontend `getLocationEditorOptions(): Promise<LocationEditorOptions>`.
- `LocationEditor` consumes `draft: LocationEditDraft`; the page owns saving.

- [ ] **Step 1: Write failing catalogue and component tests**

```rust
#[test]
fn editor_catalog_covers_every_remote_proxy_builder() {
    let options = location_editor_options();
    for protocol in ["vless", "vmess", "trojan", "shadowsocks",
                     "hysteria", "wireguard", "http", "socks"] {
        assert!(options.protocols.iter().any(|item| item.value == protocol));
    }
}
```

```ts
expect(editorSource).toContain('import Dropdown from "./Dropdown.svelte"');
expect(editorSource).not.toContain("<select");
```

- [ ] **Step 2: Run focused tests and verify failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml editor_catalog_covers_every_remote_proxy_builder`

Run: `npm test -- --run src/lib/components/LocationEditor.svelte.test.ts src/lib/location-editor-options.test.ts`

Expected: failures because the Rust command, catalogue and Dropdown integration do not exist.

- [ ] **Step 3: Implement the catalogue and controlled editor**

```rust
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorChoice {
    pub value: String,
    pub label: String,
}

#[tauri::command]
pub fn location_editor_options() -> LocationEditorOptions {
    xray::location_editor_options()
}
```

Use `Dropdown` for every finite field and keep free-form inputs for values Xray
does not expose as a finite catalogue.

- [ ] **Step 4: Run focused tests**

Run: `npm test -- --run src/lib/components/Dropdown.svelte.test.ts src/lib/components/LocationEditor.svelte.test.ts src/lib/location-editor-options.test.ts src/lib/location-draft.test.ts`

Run: `cargo test --manifest-path src-tauri/Cargo.toml editor_catalog`

Expected: all focused tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/xray.rs src-tauri/src/lib.rs src/lib
git commit -m "Use Xray catalogue in location editor"
```

### Task 2: Unified modal lifecycle

**Files:**
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/components/LocationEditor.svelte`
- Create: `src/lib/modal-events.ts`
- Create: `src/lib/modal-lifecycle.ts`
- Test: `src/lib/modal-events.test.ts`
- Test: `src/lib/modal-lifecycle.test.ts`
- Test: `src/routes/page-modal-lifecycle.test.ts`

**Interfaces:**
- `type ModalKind = "none" | "info" | "rename" | "subscription-json" | "location" | "import"`.
- `closeModal()` clears the active kind and every modal payload.
- `modalActionFromTarget(target, root)` resolves delegated close/save actions.

- [ ] **Step 1: Write the failing JSON-to-fields close regression**

```ts
it("closes a structured editor after a JSON editor", async () => {
  const page = renderPage();
  await page.openLocation(jsonServer);
  await page.closeModal();
  await page.openLocation(fieldServer);
  await page.closeModal();
  expect(page.activeModal()).toBe("none");
});
```

- [ ] **Step 2: Run the regression**

Run: `npm test -- --run src/routes/page-modal-lifecycle.test.ts`

Expected: failure because Linux still owns modal state in independent booleans and child effects.

- [ ] **Step 3: Implement one page-level controller**

```ts
function closeModal(): void {
  activeModal = "none";
  infoFor = null;
  renameFor = null;
  jsonFor = null;
  detailFor = null;
  locationSaveError = null;
}
```

Move `LocationEditDraft` creation and save state to the page. Keep Escape,
backdrop, close and cancel routed through `closeModal`.

- [ ] **Step 4: Run modal and component tests**

Run: `npm test -- --run src/lib/modal-events.test.ts src/lib/modal-lifecycle.test.ts src/routes/page-modal-lifecycle.test.ts src/lib/components/LocationEditor.svelte.test.ts`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/routes/+page.svelte src/lib/components/LocationEditor.svelte src/lib/modal-*.ts src/routes/page-modal-lifecycle.test.ts
git commit -m "Unify Linux modal lifecycle"
```

### Task 3: Parallel composite proxy latency

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Modify: `src-tauri/src/vpn.rs`
- Modify: `src/lib/subs.svelte.ts`
- Create: `src/lib/ping-scheduler.ts`
- Test: `src-tauri/src/xray.rs`
- Test: `src-tauri/src/vpn.rs`
- Test: `src/lib/ping-scheduler.test.ts`

**Interfaces:**
- `build_ping_config(server, ports)` creates one SOCKS inbound per concrete proxy.
- `ping_proxy_count(server)` returns the number of probe paths.
- `runPingsInParallel(servers, probe)` starts every location immediately.
- `proxy_get_ping` returns the first successful variant latency.

- [ ] **Step 1: Write failing concurrency tests**

```rust
#[tokio::test]
async fn composite_ping_returns_without_waiting_for_slower_variants() {
    let result = first_success([
        delayed_ok(15, 42),
        delayed_ok(500, 900),
    ]).await;
    assert_eq!(result, Some(42));
}
```

```ts
expect(started).toEqual(["a", "b", "c"]);
```

- [ ] **Step 2: Run focused tests and verify failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml composite_ping`

Run: `npm test -- --run src/lib/ping-scheduler.test.ts`

Expected: old balancer-fallback config and bounded workers fail the new assertions.

- [ ] **Step 3: Implement one inbound per proxy and first-success probing**

Use `FuturesUnordered` for variant probes, terminate the temporary Xray process
after the first success, and remove the frontend eight-worker queue.

- [ ] **Step 4: Run focused tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml composite_ping`

Run: `npm test -- --run src/lib/ping-scheduler.test.ts`

Expected: focused tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/xray.rs src-tauri/src/vpn.rs src/lib/subs.svelte.ts src/lib/ping-scheduler*
git commit -m "Probe Linux locations concurrently"
```

### Task 4: Location and log surfaces

**Files:**
- Modify: `src/lib/components/ServerList.svelte`
- Modify: `src/routes/settings/+page.svelte`
- Modify: `src/lib/card-surface-contract.test.ts`

**Interfaces:**
- Every `.srv-row` owns its top divider.
- `.log-modal`, `.log-wrap` and `.log-text` form a zero-min-size clipped flex chain.

- [ ] **Step 1: Change contract tests and verify failure**

```ts
expect(list).toMatch(/\.srv-row::before/);
expect(list).not.toContain(".srv-row + .srv-row::before");
expect(settings).toMatch(/\.log-modal\s*\{[^}]*overflow:\s*hidden/s);
```

Run: `npm test -- --run src/lib/card-surface-contract.test.ts`

Expected: failure on the old adjacent-row divider and unclipped log modal.

- [ ] **Step 2: Implement the minimal CSS**

Give every row a full-width `::before`; add `overflow: hidden`, `min-width: 0`,
`min-height: 0`, `max-width: 100%`, and `margin: 0` to the log flex chain.

- [ ] **Step 3: Run the focused contract**

Run: `npm test -- --run src/lib/card-surface-contract.test.ts`

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/ServerList.svelte src/routes/settings/+page.svelte src/lib/card-surface-contract.test.ts
git commit -m "Contain Linux logs and complete dividers"
```

### Task 5: Native multi-architecture release workflow

**Files:**
- Create: `.github/workflows/release-linux.yml`
- Modify: `scripts/test-linux-package.sh`
- Test: `scripts/test-fetch-xray-arch.sh`
- Test: `.github/workflows/release-linux.yml`

**Interfaces:**
- Matrix entries provide `runner`, `rust_target`, `deb_arch`, `rpm_arch` and `asset_arch`.
- amd64 runs on `ubuntu-24.04`; arm64 runs on `ubuntu-24.04-arm`.

- [ ] **Step 1: Add a failing workflow contract test**

Extend `scripts/test-fetch-xray-arch.sh` to assert the workflow contains both
native runner labels and both release asset suffixes.

Run: `bash scripts/test-fetch-xray-arch.sh`

Expected: failure because no release workflow exists.

- [ ] **Step 2: Implement the matrix workflow**

```yaml
strategy:
  matrix:
    include:
      - runner: ubuntu-24.04
        rust_target: x86_64-unknown-linux-gnu
        asset_arch: amd64
      - runner: ubuntu-24.04-arm
        rust_target: aarch64-unknown-linux-gnu
        asset_arch: arm64
runs-on: ${{ matrix.runner }}
```

Install native WebKitGTK/Tauri dependencies, run all tests, fetch Xray for the
runner architecture, build all bundles, validate them, and upload artifacts.

- [ ] **Step 3: Run script and workflow syntax checks**

Run: `bash scripts/test-fetch-xray-arch.sh`

Run: `npx prettier --check .github/workflows/release-linux.yml`

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-linux.yml scripts
git commit -m "Build native amd64 and arm64 Linux packages"
```

### Task 6: Verify and replace prerelease 0.2.5

**Files:**
- Modify: `CHANGELOG.md`
- Build outputs: `target/*/release/bundle/**`

- [ ] **Step 1: Run fresh local verification**

Run: `npm test && npm run check && npm run build && npm audit --audit-level=low`

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`

Run: `bash scripts/test-install-layout.sh && bash scripts/test-fetch-xray-arch.sh`

Expected: every command exits zero.

- [ ] **Step 2: Build and inspect local amd64 packages**

Run: `npm run tauri build`

Run: `bash scripts/test-linux-package.sh`

Expected: DEB, RPM and AppImage contain matching amd64 binaries and the daemon.

- [ ] **Step 3: Push and execute the release workflow**

Push `main`, force-update tag `v0.2.5`, dispatch `release-linux.yml`, and wait for
both native architecture jobs to finish.

- [ ] **Step 4: Replace GitHub prerelease assets**

Download workflow artifacts, upload architecture-suffixed DEB/RPM/AppImage
files with `gh release upload v0.2.5 --clobber`, and retain prerelease status.

- [ ] **Step 5: Verify published assets**

Download all six release assets, compare SHA-256 values with workflow outputs,
inspect DEB/RPM architecture metadata, verify AppImage ELF architecture, and
confirm `v0.2.5` resolves to `main`.

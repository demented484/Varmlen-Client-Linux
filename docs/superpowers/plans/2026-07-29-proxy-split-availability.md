# Proxy Split Availability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Disable per-app split controls in Proxy mode with an accessible explanation, make per-site Proxy rules effective, and remove the misleading Linux network-permissions setup.

**Architecture:** Keep the split store unchanged so app choices survive mode switches. The Split page derives app availability from `settings.vpnMode`, while Xray's existing route builder supplies site-only rules to the SOCKS inbound in Proxy mode. Remove the unused permission façade from frontend and Tauri while retaining lazy daemon startup through the existing connection paths.

**Tech Stack:** Svelte 5 runes, TypeScript, Vitest, Rust, serde_json, Tauri 2, Xray routing JSON.

## Global Constraints

- Do not start, stop, reconnect, inspect, or otherwise touch the user's active VPN.
- Do not run live DNS, routing, leak, or public-IP tests.
- Per-application split tunnelling is available only in TUN mode.
- Proxy mode exposes a local SOCKS proxy at `127.0.0.1:2081`.
- App split settings remain stored while Proxy mode is active.
- Do not modify AegisVPN or subscription-service repositories.
- Commit messages contain no AI markers.

---

### Task 1: Site split routing in Proxy mode

**Files:**
- Modify: `src-tauri/src/xray.rs`
- Test: `src-tauri/src/xray.rs`

**Interfaces:**
- Consumes: `build_route_rules(split, allow_lan, inbound_tag, tun, proxy_target) -> Vec<Value>`
- Produces: Proxy-mode routing that excludes process rules and honors `SplitInput.sites_mode` and `SplitInput.sites`

- [ ] **Step 1: Write failing Rust tests**

Add tests beside `proxy_mode_is_socks_only_no_tun`:

```rust
#[test]
fn proxy_mode_ignores_apps_and_honors_general_sites() {
    let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
    let sp = SplitInput {
        apps_mode: "general".into(),
        sites_mode: "general".into(),
        apps: vec!["firefox".into()],
        sites: vec!["example.com".into()],
    };
    let cfg = build_xray_config(&s, &sp, "proxy", TunMode::XrayNative, true, "warning");
    assert!(rule_for(&cfg, "process").is_none());
    assert_eq!(rule_for(&cfg, "domain").unwrap()["outboundTag"], "direct");
    assert_eq!(cfg["routing"]["rules"].as_array().unwrap().last().unwrap()["outboundTag"], "proxy");
}

#[test]
fn proxy_mode_honors_selective_sites() {
    let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
    let sp = SplitInput {
        apps_mode: "general".into(),
        sites_mode: "selective".into(),
        apps: vec!["firefox".into()],
        sites: vec!["example.com".into()],
    };
    let cfg = build_xray_config(&s, &sp, "proxy", TunMode::XrayNative, true, "warning");
    assert!(rule_for(&cfg, "process").is_none());
    assert_eq!(rule_for(&cfg, "domain").unwrap()["outboundTag"], "proxy");
    assert_eq!(cfg["routing"]["rules"].as_array().unwrap().last().unwrap()["outboundTag"], "direct");
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml proxy_mode_ --lib
```

Expected: the new tests fail because Proxy routing has no domain rule.

- [ ] **Step 3: Reuse the route builder for the SOCKS inbound**

Replace Proxy mode's hand-built DoH/default rules with:

```rust
let rules = build_route_rules(
    split,
    allow_lan,
    TunMode::Tun2socks.inbound_tag(),
    TunMode::Tun2socks,
    &target,
);
let mut routing = json!({ "rules": rules });
```

Update nearby comments so `Tun2socks` means an inbound without Xray process
matching: Android handles apps in `VpnService`, while desktop Proxy mode cannot
provide per-app routing.

- [ ] **Step 4: Run focused and existing Xray tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml proxy_mode_ --lib
cargo test --manifest-path src-tauri/Cargo.toml xray::tests --lib
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/xray.rs
git commit -m "fix: honor site split rules in proxy mode"
```

### Task 2: Disable per-app controls in Proxy mode

**Files:**
- Create: `src/lib/split-availability.ts`
- Create: `src/lib/split-availability.test.ts`
- Modify: `src/routes/split/+page.svelte`
- Modify: `src/lib/i18n.svelte.ts`

**Interfaces:**
- Produces: `appSplitAvailable(mode: VpnMode): boolean`
- Consumes: `settings.vpnMode`, `t("split.appsProxyUnavailable")`

- [ ] **Step 1: Write the failing availability and UI contract tests**

Create `src/lib/split-availability.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { appSplitAvailable } from "./split-availability";

describe("per-app split availability", () => {
  it("requires TUN mode", () => {
    expect(appSplitAvailable("tun")).toBe(true);
    expect(appSplitAvailable("proxy")).toBe(false);
  });
});
```

Create `src/lib/proxy-split-contract.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

describe("Proxy per-app split UI", () => {
  it("keeps Apps discoverable but unavailable with an accessible notice", () => {
    const page = read("../routes/split/+page.svelte");
    const i18n = read("./i18n.svelte.ts");

    expect(page).toContain("appSplitAvailable(settings.vpnMode)");
    expect(page).toContain("aria-disabled={!appsAvailable}");
    expect(page).toContain("onmouseenter={requestAppsTab}");
    expect(page).toContain("onfocus={requestAppsTab}");
    expect(page).toContain("onclick={requestAppsTab}");
    expect(page).toContain('role="status"');
    expect(page).toContain('aria-live="polite"');
    expect(page).toContain('t("split.appsProxyUnavailable")');
    expect(i18n.match(/"split\.appsProxyUnavailable":/g)).toHaveLength(2);
  });
});
```

- [ ] **Step 2: Run Vitest and verify failure**

Run:

```bash
npm test -- src/lib/split-availability.test.ts src/lib/proxy-split-contract.test.ts
```

Expected: failure because the helper and UI contract do not exist.

- [ ] **Step 3: Implement the availability helper**

Create:

```ts
import type { VpnMode } from "./settings.svelte";

export function appSplitAvailable(mode: VpnMode): boolean {
  return mode === "tun";
}
```

- [ ] **Step 4: Guard the Apps tab and show the in-app notice**

In `src/routes/split/+page.svelte`:

- import `onDestroy`, `settings`, and `appSplitAvailable`;
- derive `appsAvailable` from `settings.vpnMode`;
- initialize and force `tab` to `websites` whenever Proxy is active;
- keep the Apps button focusable with `aria-disabled={!appsAvailable}`;
- on click, mouse enter, or focus, call one notice function that resets a
  three-second timer and never selects Apps while unavailable;
- render the translated message in `role="status" aria-live="polite"`;
- clear the timer in `onDestroy`;
- style the unavailable tab with muted text and a not-allowed cursor while
  preserving keyboard focus visibility.

Add translations:

```ts
"split.appsProxyUnavailable": "Per-app split tunnelling is unavailable in Proxy mode. Switch to TUN mode.",
"split.appsProxyUnavailable": "Per-app split-туннелинг недоступен в режиме Proxy. Переключитесь на режим TUN.",
```

- [ ] **Step 5: Run focused frontend tests and checks**

Run:

```bash
npm test -- src/lib/split-availability.test.ts src/lib/proxy-split-contract.test.ts
npm run check
```

Expected: tests and Svelte type checks pass.

- [ ] **Step 6: Commit**

```bash
git add src/lib/split-availability.ts src/lib/split-availability.test.ts src/lib/proxy-split-contract.test.ts src/routes/split/+page.svelte src/lib/i18n.svelte.ts
git commit -m "fix: disable app split controls in proxy mode"
```

### Task 3: Remove the network-permissions façade and correct Proxy copy

**Files:**
- Create: `src/lib/linux-permissions-contract.test.ts`
- Modify: `src/routes/settings/+page.svelte`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/i18n.svelte.ts`
- Modify: `src-tauri/src/vpn.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Removes: frontend `capsGranted()` and `grantCaps()`
- Removes: Tauri commands `caps_granted` and `grant_caps`
- Preserves: `DaemonClient::connect_or_start_installed()` calls in actual VPN and ping operations

- [ ] **Step 1: Write a failing source-contract test**

Create `src/lib/linux-permissions-contract.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

describe("Linux permission setup", () => {
  it("uses lazy daemon startup without a manual permission façade", () => {
    const settingsPage = read("../routes/settings/+page.svelte");
    const api = read("./api.ts");
    const i18n = read("./i18n.svelte.ts");
    const vpn = read("../../src-tauri/src/vpn.rs");
    const tauriLib = read("../../src-tauri/src/lib.rs");

    expect(settingsPage).not.toContain("capsGranted");
    expect(settingsPage).not.toContain("grantCaps");
    expect(settingsPage).not.toContain('t("settings.helper")');
    expect(api).not.toContain('invoke<boolean>("caps_granted")');
    expect(api).not.toContain('invoke<void>("grant_caps")');
    expect(vpn).not.toContain("pub async fn caps_granted");
    expect(vpn).not.toContain("pub async fn grant_caps");
    expect(tauriLib).not.toContain("vpn::caps_granted");
    expect(tauriLib).not.toContain("vpn::grant_caps");
    expect(i18n).toContain('"mode.proxy": "Proxy (SOCKS)"');
    expect(i18n).toContain('"mode.proxy": "Прокси (SOCKS)"');
  });
});
```

- [ ] **Step 2: Run the contract test and verify failure**

Run:

```bash
npm test -- src/lib/linux-permissions-contract.test.ts
```

Expected: failure because the façade and old copy are still present.

- [ ] **Step 3: Remove unused UI, commands, and translations**

- Delete helper imports, state, effects, setup functions, settings section, and
  now-unused status-dot CSS from `settings/+page.svelte`.
- Delete `capsGranted` and `grantCaps` from `api.ts`.
- Delete `caps_granted` and `grant_caps` from `vpn.rs` and the Tauri handler.
- Delete `settings.helper` and `helper.*` translations.
- Change mode copy to:

```ts
"mode.tunSub": "Routes all system traffic through a virtual network interface.",
"mode.proxy": "Proxy (SOCKS)",
"mode.proxySub": "Local SOCKS proxy at 127.0.0.1:2081. Configure apps or the system to use it.",
```

and:

```ts
"mode.tunSub": "Направляет весь системный трафик через виртуальный сетевой интерфейс.",
"mode.proxy": "Прокси (SOCKS)",
"mode.proxySub": "Локальный SOCKS-прокси 127.0.0.1:2081. Укажите его в приложениях или системе.",
```

Correct the `vpnConnect` API comment to describe both modes without claiming
that Proxy runs as the user.

- [ ] **Step 4: Run focused tests and compile checks**

Run:

```bash
npm test -- src/lib/linux-permissions-contract.test.ts
npm run check
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all commands pass and no dead helper references remain.

- [ ] **Step 5: Commit**

```bash
git add src/routes/settings/+page.svelte src/lib/api.ts src/lib/i18n.svelte.ts src-tauri/src/vpn.rs src-tauri/src/lib.rs src/lib/linux-permissions-contract.test.ts
git commit -m "refactor: remove misleading permission setup"
```

### Task 4: Full non-invasive verification

**Files:**
- Verify only; no network-state mutation

**Interfaces:**
- Consumes: all deliverables from Tasks 1–3
- Produces: build and test evidence

- [ ] **Step 1: Run the complete frontend suite**

```bash
npm test
npm run check
npm run build
```

- [ ] **Step 2: Run Rust and daemon suites**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path daemon/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- [ ] **Step 3: Inspect the final diff and repository state**

```bash
git diff --check
git status --short
git log --oneline -5
```

Expected: no whitespace errors, only the plan file remains uncommitted if it was
not committed before execution, and no VPN/network command was run.

# Import, JSON, Flags, and Platform UA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native-looking country flags, separate link and JSON import flows, editable JSON subscription sources, and platform-aware subscription User-Agents to Linux and Android.

**Architecture:** Rust classifies subscription payloads and returns the original JSON when applicable. Svelte persists that source with each subscription, exposes one JSON editor for imports and later edits, and renders country emoji through a reusable SVG flag component backed by bundled assets.

**Tech Stack:** Rust, Tauri 2, Svelte 5, TypeScript, Vitest, Cargo test, `flag-icons` 7.5.0

## Global Constraints

- Linux and Android must expose the same feature behaviour and copy.
- Country flags are bundled locally; no CDN or runtime flag downloads.
- Link input is one line; JSON input is multiline.
- Invalid or empty JSON never mutates a saved subscription.
- Automatic refresh skips locally edited remote JSON; explicit Refresh replaces it.
- Subscription UA is `Varmlen/<version> (<platform>; <architecture>)`.
- Do not send HWID, device model, OS version, or an installation identifier.
- Do not connect, disconnect, probe, or otherwise change the active VPN.

---

### Task 1: Linux subscription payload and User-Agent backend

**Files:**
- Modify: `src-tauri/src/subscription.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`

**Interfaces:**
- Produces: `ImportResult.source_json: Option<String>` in Rust and `source_json: string | null` in TypeScript.
- Produces: `subscription_user_agent() -> String`.
- Consumes: existing `parse_json_subscription()` and `parse_subscription()`.

- [ ] **Step 1: Add failing Rust tests**

In `subscription.rs` add a test that passes an Xray outbound JSON object to
`parse_subscription()` and expects one VLESS server.

In `lib.rs` add:

```rust
#[test]
fn subscription_ua_identifies_target() {
    let ua = subscription_user_agent();
    assert!(ua.starts_with(concat!("Varmlen/", env!("CARGO_PKG_VERSION"), " (")));
    assert!(ua.ends_with(')'));
    assert!(ua.contains(target_platform()));
    assert!(ua.contains(target_arch()));
}
```

- [ ] **Step 2: Verify the Rust tests fail**

Run:

```bash
cargo test subscription_ua_identifies_target -- --nocapture
cargo test parse_subscription_accepts_json -- --nocapture
```

Expected: the JSON assertion fails and the UA test cannot resolve the new helper functions.

- [ ] **Step 3: Implement payload classification and UA**

Change `parse_subscription()` so a trimmed body beginning with `{` or `[` is
delegated to `parse_json_subscription()`.

Add compile-time label helpers:

```rust
fn target_platform() -> &'static str {
    if cfg!(target_os = "android") { "Android" }
    else if cfg!(target_os = "linux") { "Linux" }
    else if cfg!(target_os = "windows") { "Windows" }
    else if cfg!(target_os = "macos") { "macOS" }
    else { std::env::consts::OS }
}

fn target_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    }
}

fn subscription_user_agent() -> String {
    format!(
        "Varmlen/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        target_platform(),
        target_arch()
    )
}
```

Use that value in `reqwest::Client::builder().user_agent(...)`.

Add `source_json` to every `ImportResult` constructor. Pasted JSON returns its
trimmed source. A fetched response returns its body when the trimmed response is
valid JSON and `None` otherwise.

- [ ] **Step 4: Verify backend GREEN**

Run: `cargo test`

Expected: all Rust tests pass.

- [ ] **Step 5: Expose the field to TypeScript**

Add `source_json: string | null` to `ImportResult` in `src/lib/api.ts`.

### Task 2: Linux flag and JSON helpers

**Files:**
- Create: `src/lib/flags.ts`
- Create: `src/lib/flags.test.ts`
- Create: `src/lib/subscription-json.ts`
- Create: `src/lib/subscription-json.test.ts`
- Create: `src/lib/components/FlagIcon.svelte`
- Modify: `src/app.css`
- Modify: `package.json`
- Modify: `package-lock.json`

**Interfaces:**
- Produces: `countryCodeFromFlag(flag: string): string | null`.
- Produces: `isJsonInput(value: string): boolean`.
- Produces: `formatJson(value: string): string`.
- Produces: `isRemoteSource(value: string): boolean`.
- Consumes: CSS classes from `flag-icons/css/flag-icons.min.css`.

- [ ] **Step 1: Add failing TypeScript tests**

```ts
import { describe, expect, it } from "vitest";
import { countryCodeFromFlag } from "./flags";

describe("countryCodeFromFlag", () => {
  it("converts regional indicator flags to lowercase ISO codes", () => {
    expect(countryCodeFromFlag("🇩🇪")).toBe("de");
    expect(countryCodeFromFlag("🇺🇸")).toBe("us");
  });

  it("rejects non-country symbols", () => {
    expect(countryCodeFromFlag("📶")).toBeNull();
    expect(countryCodeFromFlag("")).toBeNull();
  });
});
```

```ts
import { describe, expect, it } from "vitest";
import { formatJson, isJsonInput, isRemoteSource } from "./subscription-json";

describe("subscription JSON helpers", () => {
  it("detects JSON objects and arrays after whitespace", () => {
    expect(isJsonInput("  {\"outbounds\":[]}")).toBe(true);
    expect(isJsonInput("\\n[\"vless://…\"]")).toBe(true);
    expect(isJsonInput("https://example.com/sub")).toBe(false);
  });

  it("formats valid JSON and rejects invalid JSON", () => {
    expect(formatJson("{\"a\":1}")).toBe("{\\n  \"a\": 1\\n}");
    expect(() => formatJson("{")).toThrow();
  });

  it("recognises only HTTP subscription sources as remote", () => {
    expect(isRemoteSource("https://example.com/sub")).toBe(true);
    expect(isRemoteSource("vless://example")).toBe(false);
  });
});
```

- [ ] **Step 2: Verify helper tests fail**

Run: `npm test -- src/lib/flags.test.ts src/lib/subscription-json.test.ts`

Expected: Vitest fails because both modules are absent.

- [ ] **Step 3: Implement the helpers**

`countryCodeFromFlag()` must require exactly two regional-indicator code points
and convert each from U+1F1E6..U+1F1FF to `a`..`z`.

`isJsonInput()` checks the first non-whitespace character. `formatJson()` uses
`JSON.parse` followed by `JSON.stringify(value, null, 2)`.
`isRemoteSource()` accepts only case-insensitive `http://` and `https://`.

- [ ] **Step 4: Verify helper tests pass**

Run: `npm test -- src/lib/flags.test.ts src/lib/subscription-json.test.ts`

Expected: 7 assertions pass.

- [ ] **Step 5: Add bundled flag assets and component**

Run: `npm install flag-icons@7.5.0 --save`

Import `flag-icons/css/flag-icons.min.css` at the top of `src/app.css`.

Create `FlagIcon.svelte` with a `flag: string` prop. Render a `fi fi-<code>`
span when `countryCodeFromFlag()` returns a code; otherwise render the original
non-country symbol in a fallback span. Give the shared slot a 28×20 px 4:3
shape, 4 px radius, and fixed width.

### Task 3: Linux persisted JSON and editor UI

**Files:**
- Modify: `src/lib/subs.svelte.ts`
- Modify: `src/lib/i18n.svelte.ts`
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/card-surface-contract.test.ts`

**Interfaces:**
- Adds to `Subscription`: `sourceJson: string | null`, `jsonEdited: boolean`.
- Produces: `SubsStore.updateJson(subId: string, source: string): Promise<void>`.
- Consumes: `ImportResult.source_json`, `formatJson()`, `isJsonInput()`, and `isRemoteSource()`.

- [ ] **Step 1: Add failing UI source-contract assertions**

Extend `card-surface-contract.test.ts` to assert that Home imports
`FlagIcon.svelte`, has `importMode` values `"choose" | "link" | "json"`, renders
an `<input class="import-link">`, renders a `<textarea class="import-json">`,
contains the `menu.json` action, and contains a JSON editor modal.

- [ ] **Step 2: Verify the UI contract fails**

Run: `npm test -- src/lib/card-surface-contract.test.ts`

Expected: the new assertions fail on the existing two-mode textarea UI.

- [ ] **Step 3: Persist and update JSON sources**

Add defaults for migrated subscriptions:

```ts
if (sub.sourceJson === undefined) sub.sourceJson = null;
if (sub.jsonEdited === undefined) sub.jsonEdited = false;
```

On import, set `sourceJson: result.source_json` and `jsonEdited: false`.
On explicit refresh, replace `sourceJson` from the response and clear
`jsonEdited`. In `refreshDue()`, exclude subscriptions with
`jsonEdited === true`.

Implement `updateJson()` by parsing through `fetchSubscription(source)`,
rejecting empty results, then atomically replacing `servers`, `sourceJson`,
selection state, and persistence. Preserve an HTTP(S) `url`; replace a local
JSON `url` with the edited source. Set `jsonEdited` only for HTTP(S) sources.

- [ ] **Step 4: Implement import modes and JSON editor**

Use:

```ts
let importMode = $state<"choose" | "link" | "json">("choose");
let jsonFor = $state<Subscription | null>(null);
let jsonDraft = $state("");
let jsonError = $state<string | null>(null);
```

The choose view has Clipboard, Link, and JSON buttons. Link mode uses
`<input class="import-link" type="text">` and submits on Enter. JSON mode uses
`<textarea class="import-json">`. Clipboard content that fails to import routes
to Link or JSON based on `isJsonInput()`.

Add **View JSON** to a subscription menu only when `sourceJson` exists. Open a
modal with formatted JSON and Save/Cancel actions. Save calls
`subs.updateJson()` and leaves the modal open with an error on failure.

Replace direct emoji markup in server rows and location-details headers with
`FlagIcon`.

- [ ] **Step 5: Add English and Russian copy**

Add keys for Link, JSON, Back, View JSON, Edit JSON, local-edit warning, invalid
JSON, and Save changes. Use plain user-facing terms and keep existing keys
compatible.

- [ ] **Step 6: Verify Linux frontend**

Run: `npm test && npm run check && npm run build`

Expected: all Vitest tests pass, Svelte diagnostics report zero errors and
warnings, and Vite builds successfully.

- [ ] **Step 7: Commit Linux implementation**

```bash
git add package.json package-lock.json src src-tauri
git commit -m "Add JSON subscription editing and native flags"
```

### Task 4: Android parity

**Files:**
- Modify/create the corresponding files under `../Varmlen-Client-Android/`.

**Interfaces:**
- Produces the same Rust and TypeScript interfaces as Tasks 1–3.
- Preserves Android-only clipboard, notification, and touch behaviour.

- [ ] **Step 1: Add the same failing Rust and TypeScript tests first**

Create Android `flags.test.ts` and `subscription-json.test.ts`, extend its
existing UI contract, and add the same Rust assertions before production edits.

- [ ] **Step 2: Verify Android RED**

Run:

```bash
cargo test subscription_ua_identifies_target -- --nocapture
cargo test parse_subscription_accepts_json -- --nocapture
npm test -- src/lib/flags.test.ts src/lib/subscription-json.test.ts src/lib/card-surface-contract.test.ts
```

Expected: failures match the missing backend helpers, helper modules, and UI.

- [ ] **Step 3: Apply the Android production changes**

Add `flag-icons@7.5.0`, the helper modules, `FlagIcon.svelte`, backend JSON
source and UA handling, persisted JSON fields, three-mode import UI, JSON
editor, and translations. Keep `readClipboard()` for Android and do not remove
the log-copy or touch-only CSS already present in this repository.

- [ ] **Step 4: Verify Android GREEN**

Run: `cargo test && npm test && npm run check && npm run build`

Expected: all Rust and Vitest tests pass, Svelte reports zero diagnostics, and
the production frontend builds.

- [ ] **Step 5: Commit Android implementation**

```bash
git add package.json package-lock.json src src-tauri
git commit -m "Add JSON subscription editing and native flags"
```

### Task 5: Cross-client and visual verification

**Files:**
- Review all files committed by Tasks 1–4.

**Interfaces:**
- Consumes both completed implementations.
- Produces verification evidence without touching VPN state.

- [ ] **Step 1: Compare shared helpers and tests**

Run:

```bash
diff -u src/lib/flags.ts ../Varmlen-Client-Android/src/lib/flags.ts
diff -u src/lib/subscription-json.ts ../Varmlen-Client-Android/src/lib/subscription-json.ts
diff -u src/lib/flags.test.ts ../Varmlen-Client-Android/src/lib/flags.test.ts
diff -u src/lib/subscription-json.test.ts ../Varmlen-Client-Android/src/lib/subscription-json.test.ts
```

Expected: no differences.

- [ ] **Step 2: Inspect both commits**

Run in both repositories:

```bash
git show --stat --oneline HEAD
git status --short
```

Expected: only scoped source, tests, lockfiles, and bundled dependency metadata
changed; both worktrees are clean.

- [ ] **Step 3: Visually inspect local UI**

Start the Linux Vite development server on `127.0.0.1`, open Home in the local
browser, and inspect the three import modes and editor layout. Use fixture data
in localStorage only if needed; do not invoke Tauri VPN commands or fetch a real
subscription URL.

- [ ] **Step 4: Stop local preview**

Close the preview tab and stop the local Vite server.

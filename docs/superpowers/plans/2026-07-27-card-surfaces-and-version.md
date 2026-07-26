# Card Surfaces and Version Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove neutral outlines from card surfaces and display the installed Varmlen version in Settings on Linux and Android.

**Architecture:** Keep existing elevated backgrounds and functional control borders. Apply the visual contract through the shared card/list CSS plus the few route-local card implementations, and read the installed version from Tauri runtime metadata.

**Tech Stack:** Svelte 5, SvelteKit, TypeScript, Tauri 2, Vitest

## Global Constraints

- Linux and Android must present the same card and version behaviour.
- Grouped-row separators use exactly `1px solid var(--bg)`.
- Form controls, menus, errors, badges, tab navigation, and the VPN power control keep their functional outlines.
- The version value comes from `getVersion()` in `@tauri-apps/api/app`.
- Do not touch VPN connection, route, DNS, helper, or split-tunnelling logic.
- Do not access or change the active VPN or network state during verification.

---

### Task 1: Linux visual contract

**Files:**
- Create: `src/lib/card-surface-contract.test.ts`
- Modify: `src/app.css`
- Modify: `src/routes/+page.svelte`
- Modify: `src/routes/settings/+page.svelte`
- Modify: `src/routes/split/+page.svelte`

**Interfaces:**
- Consumes: Tauri `getVersion(): Promise<string>`.
- Produces: a muted `Varmlen {appVersion}` Settings footer and borderless card surfaces.

- [ ] **Step 1: Write the failing source-contract test**

```ts
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

describe("card surface contract", () => {
  it("uses borderless cards and page-coloured row separators", () => {
    const css = read("../app.css");
    const home = read("../routes/+page.svelte");
    const settings = read("../routes/settings/+page.svelte");
    const split = read("../routes/split/+page.svelte");

    expect(css).toMatch(/\.card\s*\{[^}]*border:\s*none;/s);
    expect(css).toMatch(/\.list\s*\{[^}]*border:\s*none;/s);
    expect(css).toMatch(/\.list > \* \+ \*\s*\{[^}]*border-top:\s*1px solid var\(--bg\);/s);
    expect(home).toMatch(/\.sub-card\s*\{[^}]*border:\s*none;/s);
    expect(settings).toMatch(/\.theme-tile\s*\{[^}]*border:\s*none;/s);
    expect(settings).toMatch(/\.row \+ \.row\s*\{[^}]*border-top:\s*1px solid var\(--bg\);/s);
    expect(settings).toMatch(/\.ver-list\s*\{[^}]*border:\s*none;/s);
    expect(settings).toMatch(/\.ver-list li \+ li\s*\{\s*border-top:\s*1px solid var\(--bg\);/s);
    expect(split).toMatch(/\.picker\s*\{[^}]*border:\s*none;/s);
    expect(split).toMatch(/\.picker-row \+ \.picker-row\s*\{[^}]*border-top:\s*1px solid var\(--bg\);/s);
  });

  it("shows the Tauri application version in Settings", () => {
    const settings = read("../routes/settings/+page.svelte");

    expect(settings).toContain('import { getVersion } from "@tauri-apps/api/app";');
    expect(settings).toContain("appVersion = await getVersion()");
    expect(settings).toContain("Varmlen {appVersion}");
  });
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `npm test -- src/lib/card-surface-contract.test.ts`

Expected: both tests fail because the existing surfaces use `var(--border)` and Settings does not call `getVersion()`.

- [ ] **Step 3: Implement the Linux surface and version changes**

In `src/app.css`, set `.card` and `.list` to `border: none`, and set `.list > * + *` to `border-top: 1px solid var(--bg)`.

In `src/routes/+page.svelte`, set `.sub-card` to `border: none` and remove the obsolete pinned-card border-colour rule.

In `src/routes/settings/+page.svelte`:

```ts
import { getVersion } from "@tauri-apps/api/app";

let appVersion = $state("…");
onMount(async () => {
  try {
    appVersion = await getVersion();
  } catch {
    appVersion = "—";
  }
});
```

Add `<footer class="app-version muted">Varmlen {appVersion}</footer>` as the last child of the Settings scroll region. Give it centred, quiet text. Remove the theme-tile and local list outlines, use `var(--bg)` for row and version-list separators, and use an elevated active fill plus the existing soft accent shadow for active theme selection.

In `src/routes/split/+page.svelte`, remove the picker outline and use `var(--bg)` for picker-row separators.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `npm test -- src/lib/card-surface-contract.test.ts`

Expected: 2 tests pass.

- [ ] **Step 5: Verify Linux frontend**

Run: `npm test && npm run check && npm run build`

Expected: all tests pass, `svelte-check` reports zero errors, and Vite exits successfully.

- [ ] **Step 6: Commit Linux changes**

```bash
git add src/app.css src/lib/card-surface-contract.test.ts src/routes/+page.svelte src/routes/settings/+page.svelte src/routes/split/+page.svelte
git commit -m "Refine card surfaces and show app version"
```

### Task 2: Android visual contract

**Files:**
- Create: `../Varmlen-Client-Android/src/lib/card-surface-contract.test.ts`
- Modify: `../Varmlen-Client-Android/src/app.css`
- Modify: `../Varmlen-Client-Android/src/routes/+page.svelte`
- Modify: `../Varmlen-Client-Android/src/routes/settings/+page.svelte`
- Modify: `../Varmlen-Client-Android/src/routes/split/+page.svelte`

**Interfaces:**
- Consumes: the same Tauri `getVersion(): Promise<string>` API as Linux.
- Produces: the same Settings footer and card-surface contract as Linux.

- [ ] **Step 1: Copy the Linux source-contract test before changing Android production files**

Run:

```bash
cp src/lib/card-surface-contract.test.ts ../Varmlen-Client-Android/src/lib/card-surface-contract.test.ts
```

- [ ] **Step 2: Run the focused Android test and verify RED**

Run from `../Varmlen-Client-Android`: `npm test -- src/lib/card-surface-contract.test.ts`

Expected: both tests fail for the same missing visual and version behaviour.

- [ ] **Step 3: Apply the same minimal production changes to Android**

Mirror the five Linux production-file edits in the corresponding Android files. Preserve Android-only Settings rows, notification handling, log copying, and platform-specific CSS.

- [ ] **Step 4: Run the focused Android test and verify GREEN**

Run: `npm test -- src/lib/card-surface-contract.test.ts`

Expected: 2 tests pass.

- [ ] **Step 5: Verify Android frontend**

Run: `npm test && npm run check && npm run build`

Expected: all tests pass, `svelte-check` reports zero errors, and Vite exits successfully.

- [ ] **Step 6: Commit Android changes**

```bash
git add src/app.css src/lib/card-surface-contract.test.ts src/routes/+page.svelte src/routes/settings/+page.svelte src/routes/split/+page.svelte
git commit -m "Refine card surfaces and show app version"
```

### Task 3: Cross-client parity review

**Files:**
- Review: the five changed production files and one test file in each repository.

**Interfaces:**
- Consumes: completed Linux and Android changes.
- Produces: evidence that the duplicated frontends differ only where platform behaviour requires it.

- [ ] **Step 1: Compare the visual rules**

Run:

```bash
diff -u src/app.css ../Varmlen-Client-Android/src/app.css
diff -u src/lib/card-surface-contract.test.ts ../Varmlen-Client-Android/src/lib/card-surface-contract.test.ts
```

Expected: the contract tests are identical; existing platform-specific `app.css` differences are retained.

- [ ] **Step 2: Confirm no network or VPN code changed**

Run in both repositories: `git show --stat --oneline HEAD && git show --format= --name-only HEAD`

Expected: only the six frontend/test files listed above appear.

- [ ] **Step 3: Check both worktrees**

Run in both repositories: `git status --short`

Expected: both worktrees are clean.

# Location editor and Android background refresh implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add clear location rows, source-appropriate permissive editors, an auto-update switch, exact foreground scheduling, and Android subscription refresh that survives a closed UI.

**Architecture:** Shared TypeScript modules own edit drafts and due-time calculations in both clients. Android adds a WorkManager bridge that stages private HTTP responses; the existing Rust subscription parser consumes those responses when the UI next opens. Provider refreshes remain authoritative and clear local drafts.

**Tech Stack:** Svelte 5, TypeScript, Vitest, Rust/Tauri 2, Kotlin, AndroidX WorkManager, Gradle.

## Global constraints

- Do not connect, disconnect, restart, install, or otherwise touch the user's active VPN.
- Background refresh while fully closed is Android-only and independent of `VarmlenVpnService`.
- Opening the application must not initiate a subscription network request.
- The auto-update setting defaults to enabled and cancels all automatic work when disabled.
- Provider refreshes overwrite local edits and do not reconnect a running VPN.
- Location drafts are saved even when invalid; connection must surface the draft error instead of silently using stale parsed data.
- JSON locations expose only exact JSON; non-JSON locations expose only structured fields.
- Missing flags use one outer globe circle and exactly four inner arcs.

---

### Task 1: Pure refresh schedule and global setting

**Files:**
- Create in both repos: `src/lib/subscription-refresh.ts`
- Create in both repos: `src/lib/subscription-refresh.test.ts`
- Modify in both repos: `src/lib/settings.svelte.ts`
- Modify in both repos: `src/routes/settings/+page.svelte`
- Modify in both repos: `src/lib/i18n.svelte.ts`

**Interfaces:**
- Produces `nextFutureRefresh(lastSuccessIso: string, intervalHours: number, nowMs: number): number`.
- Produces persisted `settings.subscriptionAutoUpdate: boolean` and `setSubscriptionAutoUpdate(value: boolean)`.

- [ ] **Step 1: Write failing schedule and settings contract tests**

```ts
expect(nextFutureRefresh("2026-07-28T10:00:00Z", 1, Date.parse("2026-07-28T10:20:00Z")))
  .toBe(Date.parse("2026-07-28T11:00:00Z"));
expect(nextFutureRefresh("2026-07-28T10:00:00Z", 1, Date.parse("2026-07-28T12:20:00Z")))
  .toBe(Date.parse("2026-07-28T13:00:00Z"));
expect(settingsSource).toContain("subscriptionAutoUpdate");
```

- [ ] **Step 2: Run `npm test -- src/lib/subscription-refresh.test.ts src/lib/card-surface-contract.test.ts` in both repos and verify the new assertions fail for missing behavior**

- [ ] **Step 3: Implement the pure boundary calculation and persisted default-enabled setting**

```ts
export function nextFutureRefresh(lastSuccessIso: string, intervalHours: number, nowMs: number): number {
  const intervalMs = intervalHours * 3_600_000;
  const last = Date.parse(lastSuccessIso);
  if (!Number.isFinite(last) || !Number.isFinite(intervalMs) || intervalMs <= 0) {
    throw new Error("invalid subscription refresh schedule");
  }
  const elapsed = Math.max(0, nowMs - last);
  return last + (Math.floor(elapsed / intervalMs) + 1) * intervalMs;
}
```

- [ ] **Step 4: Add a Settings switch with localized copy and run the focused tests until green**

- [ ] **Step 5: Commit the task separately in each repository with message `Add subscription auto-update setting`**

### Task 2: Permissive location drafts and authoritative refresh

**Files:**
- Create in both repos: `src/lib/location-draft.ts`
- Create in both repos: `src/lib/location-draft.test.ts`
- Modify in both repos: `src/lib/subs.svelte.ts`
- Modify in both repos: `src/lib/conn.svelte.ts`
- Modify in both repos: `src/lib/api.ts`

**Interfaces:**
- Produces `LocationEditDraft`, `createLocationDraft(server)`, and `compileLocationDraft(draft, previous)`.
- Adds `editDraft: LocationEditDraft | null` to `ServerEntry`.
- `subs.saveServerDraft(id, draft)` always persists.
- `subs.selectedServerRaw()` throws a bounded user-facing parse error when the selected draft is invalid.

- [ ] **Step 1: Write failing tests for field/JSON draft persistence, invalid compilation, valid compilation, and provider overwrite**

```ts
const invalid = { kind: "fields", values: { ...values, port: "not-a-port" }, rawParams: [] };
expect(compileLocationDraft(invalid, server)).toEqual({
  ok: false,
  error: expect.stringContaining("port"),
});
expect(JSON.stringify(invalid)).toContain("not-a-port");
```

- [ ] **Step 2: Run the focused Vitest files in both repos and verify RED**

- [ ] **Step 3: Implement discriminated drafts**

```ts
export type LocationEditDraft =
  | { kind: "json"; source: string }
  | {
      kind: "fields";
      values: Record<LocationField, string>;
      rawParams: Array<{ id: string; key: string; value: string }>;
    };

export type DraftCompileResult =
  | { ok: true; server: VlessServer }
  | { ok: false; error: string };
```

- [ ] **Step 4: Persist drafts without validation; compile only for display updates and connection**

- [ ] **Step 5: Make manual/automatic provider refresh replace servers with `editDraft: null`, including formerly `jsonEdited` subscriptions**

- [ ] **Step 6: Move selected-draft compilation inside `ConnStore.connect()` error handling so invalid drafts produce a visible connection error**

- [ ] **Step 7: Run focused tests and then `npm test` in both repos**

- [ ] **Step 8: Commit separately with message `Add editable location drafts`**

### Task 3: Location row and source-specific editor UI

**Files:**
- Create in both repos: `src/lib/components/LocationEditor.svelte`
- Modify in both repos: `src/lib/components/ServerList.svelte`
- Modify in both repos: `src/lib/components/FlagIcon.svelte`
- Modify in both repos: `src/routes/+page.svelte`
- Modify in both repos: `src/lib/card-surface-contract.test.ts`
- Modify in both repos: `src/lib/i18n.svelte.ts`

**Interfaces:**
- `LocationEditor` consumes `{ server: ServerEntry; onSave(draft: LocationEditDraft): void; onCancel(): void }`.
- `FlagIcon` retains the existing `flag: string` prop and renders the fallback globe when no country code or fallback symbol exists.

- [ ] **Step 1: Add failing source-contract tests for visible row dividers, no `.srv-stripe`, four globe paths, and mutually exclusive JSON/field branches**

```ts
expect(serverList).toMatch(/\.srv-row \+ \.srv-row::before/);
expect(serverList).not.toContain("srv-stripe");
expect(flagIcon.match(/class="globe-arc"/g)).toHaveLength(4);
expect(editor).toContain('{#if draft.kind === "json"}');
expect(editor).toContain("{:else}");
```

- [ ] **Step 2: Run the contract test in both repos and verify RED**

- [ ] **Step 3: Remove the stripe, retain active background, and add an inset `var(--border-strong)` divider**

- [ ] **Step 4: Implement the gray globe SVG with one circle and four explicit arc paths**

- [ ] **Step 5: Implement conditional protocol fields and editable extra key/value rows; bind every keystroke into a local draft and save without validation**

- [ ] **Step 6: Replace the current rows-plus-JSON modal in `+page.svelte` with `LocationEditor`**

- [ ] **Step 7: Run contract tests, all frontend tests, and `npm run check` in both repos**

- [ ] **Step 8: Capture desktop and Android-sized local screenshots and inspect dividers, selection, fallback globe, and both editor modes**

- [ ] **Step 9: Commit separately with message `Refine location list and editor`**

### Task 4: Exact foreground scheduling

**Files:**
- Modify in both repos: `src/lib/subs.svelte.ts`
- Modify in both repos: `src/routes/+layout.svelte`
- Modify in both repos: `src/lib/subscription-refresh.test.ts`

**Interfaces:**
- `subs.startAutoRefresh()` schedules only future boundaries and returns cleanup.
- `subs.stopAutoRefresh()` cancels a pending timer.
- Android native synchronization is called separately by Task 6.

- [ ] **Step 1: Add failing tests proving mount does not call `refreshDue()` immediately, missed boundaries advance to the next future boundary, and disabling cancels the timer**

- [ ] **Step 2: Run focused tests and verify RED**

- [ ] **Step 3: Replace the five-minute polling interval with one `setTimeout` for the earliest future due subscription**

- [ ] **Step 4: Reschedule after successful/manual refresh, subscription changes, and setting changes; never replay a missed Linux cycle on mount**

- [ ] **Step 5: Run focused and full frontend tests in both repos**

- [ ] **Step 6: Commit separately with message `Schedule subscription refresh at due time`**

### Task 5: Parse staged Android responses through Rust

**Files:**
- Modify Android: `src-tauri/src/subscription.rs`
- Modify Android: `src-tauri/src/lib.rs`
- Modify Android: `src/lib/api.ts`

**Interfaces:**
- Adds command `parse_subscription_response(body: String, headers: HashMap<String, String>) -> Result<ImportResult, String>`.
- Reuses the same body metadata merge, `parse_headers`, and server parser as `fetch_subscription`.

- [ ] **Step 1: Add a failing Rust test with staged body and headers that asserts title, interval, userinfo, description, and servers**

- [ ] **Step 2: Run the exact Rust test and verify RED**

- [ ] **Step 3: Extract response parsing from `fetch_subscription` into a pure function and expose the Tauri command**

- [ ] **Step 4: Add the typed frontend API wrapper**

```ts
export function parseSubscriptionResponse(
  body: string,
  headers: Record<string, string>,
): Promise<ImportResult> {
  return invoke("parse_subscription_response", { body, headers });
}
```

- [ ] **Step 5: Run the Rust test and complete Android Rust test suite**

- [ ] **Step 6: Commit with message `Parse cached subscription responses`**

### Task 6: Android WorkManager scheduler and staged cache

**Files:**
- Create Android: `src-tauri/gen/android/app/src/main/java/app/varmlen/client/SubscriptionRefreshStore.kt`
- Create Android: `src-tauri/gen/android/app/src/main/java/app/varmlen/client/SubscriptionRefreshWorker.kt`
- Create Android tests under `src-tauri/gen/android/app/src/test/java/app/varmlen/client/`
- Modify Android: `src-tauri/gen/android/app/build.gradle.kts`
- Modify Android: `src-tauri/gen/android/app/src/main/java/app/varmlen/client/VpnPlugin.kt`
- Modify Android: `src-tauri/src/mobile_vpn.rs`
- Modify Android: `src/lib/api.ts`
- Modify Android: `src/lib/subs.svelte.ts`
- Modify Android: `src/routes/+layout.svelte`

**Interfaces:**
- Native commands: `syncSubscriptionRefresh`, `cancelSubscriptionRefresh`, `drainSubscriptionRefreshes`.
- Schedule item: `{ id, url, userAgent, intervalHours, nextUpdateAt }`.
- Staged result: `{ id, body, headers, refreshedAt }`.

- [ ] **Step 1: Add failing Kotlin tests for private schedule serialization, unique work naming, cancel-all behavior, staged-response replacement, and bounded errors**

- [ ] **Step 2: Run Gradle unit tests and verify RED**

- [ ] **Step 3: Add `androidx.work:work-runtime-ktx` and implement app-private schedule/result storage with atomic file replacement**

- [ ] **Step 4: Implement a network-constrained one-time worker that sends the selected UA, stores only successful bounded responses, uses backoff on failure, and schedules the next run**

- [ ] **Step 5: Add bridge commands and typed frontend wrappers; no command may start `VarmlenVpnService` or an Activity**

- [ ] **Step 6: Sync/cancel schedules reactively from the frontend and drain staged results on normal mount without performing a network request**

- [ ] **Step 7: Apply each parsed staged result through the same authoritative replacement path as manual Refresh, clearing edit drafts and preserving the selected stable key**

- [ ] **Step 8: Run Kotlin, Rust, and frontend tests; inspect the merged manifest for the WorkManager initializer and absence of exact-alarm permissions**

- [ ] **Step 9: Commit with message `Refresh Android subscriptions in background`**

### Task 7: Final verification

**Files:**
- No production changes unless verification exposes a defect.

- [ ] **Step 1: Run `npm test`, `npm run check`, `cargo test --workspace`, and Clippy in Linux**

- [ ] **Step 2: Run `npm test`, `npm run check`, Android Rust/helper tests, Clippy, and Gradle unit tests in Android**

- [ ] **Step 3: Build Linux packages and a signed arm64 Android APK without installing either**

- [ ] **Step 4: Verify package versions, APK signature continuity, native libraries, and artifact layouts**

- [ ] **Step 5: Review `git diff`, commit messages, and both clean worktrees; do not push or release unless the user asks after reviewing the result**

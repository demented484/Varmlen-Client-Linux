<script lang="ts">
  import { theme } from "$lib/theme.svelte";
  import { settings, type VpnMode } from "$lib/settings.svelte";
  import { i18n, t, LANGUAGES, type Lang } from "$lib/i18n.svelte";
  import { core, xrayCore } from "$lib/core.svelte";
  import { capsGranted, grantCaps } from "$lib/api";
  import Dropdown from "$lib/components/Dropdown.svelte";

  const modeOptions = $derived([
    { value: "tun", label: t("mode.tun") },
    { value: "proxy", label: t("mode.proxy") },
  ]);
  const modeSub = $derived(settings.vpnMode === "proxy" ? t("mode.proxySub") : t("mode.tunSub"));

  // Refresh both cores' status when Settings opens (cheap GitHub check).
  $effect(() => {
    void core.check();
    void xrayCore.check();
  });

  /** Tri-state so we don't flash "Not installed" while the very first check
   *  is still in flight (the helper check is async but the section paints
   *  instantly). "checking" renders the neutral yellow dot + a soft label
   *  until we actually know. */
  type HelperState = "checking" | "ok" | "missing";
  let helperState = $state<HelperState>("checking");
  let helperBusy = $state(false);
  let helperErr = $state<string | null>(null);

  async function refreshHelper(allowChecking = false) {
    if (allowChecking) helperState = "checking";
    try {
      helperState = (await capsGranted()) ? "ok" : "missing";
    } catch {
      helperState = "missing";
    }
  }
  $effect(() => {
    void refreshHelper(true);
  });

  async function setupHelper() {
    helperBusy = true;
    helperErr = null;
    try {
      await grantCaps();
      for (let i = 0; i < 10; i++) {
        await refreshHelper();
        if (helperState === "ok") break;
        await new Promise((r) => setTimeout(r, 250));
      }
    } catch (e) {
      helperErr = e instanceof Error ? e.message : String(e);
    } finally {
      helperBusy = false;
    }
  }

  const helperStatus = $derived.by(() => {
    if (helperState === "checking") return t("helper.checking");
    if (helperState === "ok") return t("helper.ready");
    return helperErr ?? t("helper.notInstalled");
  });

  // The versions modal is shared by both cores; `activeCore` is whichever the
  // user opened it for.
  let showVersions = $state(false);
  let activeCore = $state(core);

  async function openVersions(which: typeof core) {
    activeCore = which;
    showVersions = true;
    await which.loadReleases();
  }

  function formatReleaseDate(d: string | null): string {
    if (!d) return "";
    const dt = new Date(d);
    if (!Number.isFinite(dt.getTime())) return "";
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${pad(dt.getDate())}.${pad(dt.getMonth() + 1)}.${dt.getFullYear()}`;
  }

  /** Compact byte formatter for the download speed indicator. */
  function formatBps(n: number): string {
    if (!Number.isFinite(n) || n <= 0) return "—";
    const units = ["B", "KB", "MB", "GB"];
    let v = n;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}/s`;
  }
  function formatBytes(n: number): string {
    if (!Number.isFinite(n) || n <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    let v = n;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
  }

  function coreStatus(store: typeof core): string {
    if (store.checking && !store.info) return t("core.checking");
    const info = store.info;
    if (!info) return store.error ? t("core.checkFailed") : t("core.checking");
    if (info.active) {
      return info.has_update && info.latest
        ? `${info.active} → ${info.latest}`
        : info.active;
    }
    return info.latest
      ? `${t("core.notInstalled")} · ${t("core.latest", { v: info.latest })}`
      : t("core.notInstalled");
  }

</script>

<header class="topbar">
  <h1>{t("settings.title")}</h1>
</header>

<main class="scroll">
  <section>
    <h2>{t("settings.appearance")}</h2>
    <div class="card theme-card">
      <div class="theme-row">
        <button
          class="theme-tile"
          class:active={theme.current === "dark"}
          onclick={() => theme.set("dark")}
          aria-pressed={theme.current === "dark"}
        >
          <div class="swatch swatch-dark"></div>
          <span>{t("settings.dark")}</span>
        </button>
        <button
          class="theme-tile"
          class:active={theme.current === "light"}
          onclick={() => theme.set("light")}
          aria-pressed={theme.current === "light"}
        >
          <div class="swatch swatch-light"></div>
          <span>{t("settings.light")}</span>
        </button>
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.vpnMode")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.vpnMode")}</div>
          <div class="row-sub muted">{modeSub}</div>
        </div>
        <Dropdown
          value={settings.vpnMode}
          options={modeOptions}
          onChange={(v) => settings.setVpnMode(v as VpnMode)}
          ariaLabel={t("settings.vpnMode")}
        />
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.general")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.language")}</div>
        </div>
        <Dropdown
          value={i18n.lang}
          options={LANGUAGES}
          onChange={(v) => i18n.set(v as Lang)}
          ariaLabel={t("settings.language")}
        />
      </div>
      <label class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.killswitch")}</div>
          <div class="row-sub muted">{t("settings.killswitchSub")}</div>
        </div>
        <span class="switch">
          <input
            type="checkbox"
            checked={settings.killswitch}
            onchange={(e) => settings.setKillswitch((e.currentTarget as HTMLInputElement).checked)}
          />
          <span class="slider"></span>
        </span>
      </label>
      <label class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.allowLan")}</div>
          <div class="row-sub muted">{t("settings.allowLanSub")}</div>
        </div>
        <span class="switch">
          <input
            type="checkbox"
            checked={settings.allowLan}
            onchange={(e) => settings.setAllowLan((e.currentTarget as HTMLInputElement).checked)}
          />
          <span class="slider"></span>
        </span>
      </label>
    </div>
  </section>

  <section>
    <h2>{t("settings.core")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">sing-box <span class="muted" style="font-weight:400">· TUN</span></div>
          <div class="row-sub muted">{coreStatus(core)}</div>
        </div>
        <button class="btn" onclick={() => openVersions(core)} title={t("core.versionsTitle")}>
          <svg class="btn-ico" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 6h16M4 12h16M4 18h16" stroke="currentColor" stroke-width="1.9"
              stroke-linecap="round" />
          </svg>
          <span>{t("core.versions")}</span>
        </button>
      </div>
      <div class="row">
        <div class="row-text">
          <div class="row-title">xray <span class="muted" style="font-weight:400">· XHTTP</span></div>
          <div class="row-sub muted">{coreStatus(xrayCore)}</div>
        </div>
        <button class="btn" onclick={() => openVersions(xrayCore)} title={t("core.versionsTitle")}>
          <svg class="btn-ico" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 6h16M4 12h16M4 18h16" stroke="currentColor" stroke-width="1.9"
              stroke-linecap="round" />
          </svg>
          <span>{t("core.versions")}</span>
        </button>
      </div>
    </div>
    {#if core.error}
      <div class="row-sub" style="color: var(--danger); padding: 0 4px;">{core.error}</div>
    {/if}
    {#if xrayCore.error}
      <div class="row-sub" style="color: var(--danger); padding: 0 4px;">{xrayCore.error}</div>
    {/if}
  </section>

  {#if showVersions}
    <div class="modal-backdrop" onclick={() => (showVersions = false)} role="presentation">
      <div
        class="modal card"
        onclick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={t("core.versionsTitle")}
      >
        <header class="modal-head">
          <h2>{t("core.versionsTitle")}</h2>
          <button
            class="icon-btn"
            onclick={() => (showVersions = false)}
            aria-label={t("common.close")}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2"
                stroke-linecap="round" />
            </svg>
          </button>
        </header>

        {#if activeCore.releasesLoading && activeCore.releases.length === 0}
          <p class="muted">{t("core.checking")}</p>
        {:else if activeCore.error && activeCore.releases.length === 0}
          <p style="color: var(--danger)">{activeCore.error}</p>
        {:else}
          <ul class="ver-list">
            {#each activeCore.releases as r (r.tag)}
              {@const ver = r.tag.replace(/^v/, "")}
              {@const isActive = activeCore.isActive(r.tag)}
              {@const isInstalled = activeCore.isInstalled(r.tag)}
              {@const prog = activeCore.progress[ver]}
              {@const isDownloading = activeCore.busyTags.has(ver)}
              {@const isSwitching = activeCore.switchingTag === ver}
              {@const pct = prog && prog.total > 0
                ? Math.min(100, Math.round((prog.downloaded / prog.total) * 100))
                : prog && prog.downloaded > 0 ? 0 : 0}
              <li class="ver-row" class:current={isActive}>
                <div class="ver-info">
                  <span class="ver-tag">{r.tag}</span>
                  <span class="ver-meta muted">
                    {formatReleaseDate(r.date)}
                    {#if r.prerelease}<span class="badge">{t("core.preview")}</span>{/if}
                  </span>
                </div>

                <div class="ver-actions">
                  {#if isDownloading && prog}
                    <div class="progress" aria-label="downloading">
                      <div class="progress-track">
                        <div class="progress-fill" style="width: {pct}%"></div>
                      </div>
                      <div class="progress-meta muted">
                        {formatBytes(prog.downloaded)}
                        {#if prog.total > 0}
                          / {formatBytes(prog.total)} · {pct}%
                        {/if}
                        · {formatBps(prog.speed_bps)}
                      </div>
                    </div>
                  {:else if isActive}
                    <!-- No badge: the active row is already tinted accent so
                         "this one is current" is obvious without a label.
                         Delete still spells out "Delete" because removing the
                         version the user is running is destructive enough to
                         deserve text confirmation. -->
                    <button
                      class="btn btn-sm btn-danger"
                      onclick={() => activeCore.uninstall(r.tag)}
                      disabled={isSwitching}
                      title={t("core.delete")}
                    >
                      <svg class="btn-ico" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                        <path d="M9 3a1 1 0 0 0-1 1v1H4a1 1 0 1 0 0 2h1l1 13a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-13h1a1 1 0 1 0 0-2h-4V4a1 1 0 0 0-1-1H9zm1 2h4v1h-4V5zm-3 3h10l-.9 12.1a.1.1 0 0 1-.1.1H8.1a.1.1 0 0 1-.1-.1L7 8zm3 2a1 1 0 0 0-1 1v6a1 1 0 1 0 2 0v-6a1 1 0 0 0-1-1zm4 0a1 1 0 0 0-1 1v6a1 1 0 1 0 2 0v-6a1 1 0 0 0-1-1z" />
                      </svg>
                      <span>{t("core.delete")}</span>
                    </button>
                  {:else if isInstalled}
                    <button
                      class="btn btn-primary btn-sm"
                      onclick={() => activeCore.activate(r.tag)}
                      disabled={isSwitching}
                    >
                      {isSwitching ? "…" : t("core.use")}
                    </button>
                    <!-- Non-active row: a proper text+icon Delete button
                         (matching the active row's). The earlier icon-only
                         square rendered as an unrecognisable red dot in
                         WebKitGTK — the multi-subpath SVG was being clipped
                         or simplified out by the renderer. Plain text is
                         unambiguous. -->
                    <button
                      class="btn btn-sm btn-danger"
                      onclick={() => activeCore.uninstall(r.tag)}
                      disabled={isSwitching}
                      title={t("core.delete")}
                    >
                      <svg class="btn-ico" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                        <path d="M9 3a1 1 0 0 0-1 1v1H4a1 1 0 1 0 0 2h1l1 13a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-13h1a1 1 0 1 0 0-2h-4V4a1 1 0 0 0-1-1H9zm1 2h4v1h-4V5zm-3 3h10l-.9 12.1a.1.1 0 0 1-.1.1H8.1a.1.1 0 0 1-.1-.1L7 8zm3 2a1 1 0 0 0-1 1v6a1 1 0 1 0 2 0v-6a1 1 0 0 0-1-1zm4 0a1 1 0 0 0-1 1v6a1 1 0 1 0 2 0v-6a1 1 0 0 0-1-1z" />
                      </svg>
                      <span>{t("core.delete")}</span>
                    </button>
                  {:else}
                    <button
                      class="btn btn-sm"
                      onclick={() => activeCore.install(r.tag)}
                      disabled={isDownloading}
                    >
                      <svg class="btn-ico" width="14" height="14" viewBox="0 0 24 24" fill="none"
                        aria-hidden="true">
                        <path d="M12 4v12m0 0l-4-4m4 4l4-4M5 20h14"
                          stroke="currentColor" stroke-width="1.9"
                          stroke-linecap="round" stroke-linejoin="round" />
                      </svg>
                      <span>{t("core.download")}</span>
                    </button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}

  <section>
    <h2>{t("settings.helper")}</h2>
    <div class="list">
      <div class="row">
        <span
          class="status-dot"
          class:on={helperState === "ok" && !helperBusy}
          class:off={helperState === "missing" && !helperBusy}
          class:busy={helperState === "checking" || helperBusy}
        ></span>
        <div class="row-text">
          <div class="row-title">{t("helper.title")}</div>
          <div class="row-sub muted">{helperStatus}</div>
        </div>
        {#if helperState !== "checking"}
          <button
            class="btn {helperState === "ok" ? '' : 'btn-primary'}"
            onclick={setupHelper}
            disabled={helperBusy}
          >
            {helperBusy
              ? t("helper.installing")
              : helperState === "ok"
                ? t("helper.reinstall")
                : t("helper.install")}
          </button>
        {/if}
      </div>
      {#if helperErr}
        <div class="row-sub" style="color: var(--danger); padding: 6px 14px;">{helperErr}</div>
      {/if}
    </div>
  </section>

</main>

<style>
  .topbar {
    display: flex;
    align-items: center;
    padding: 14px 16px 6px;
    flex-shrink: 0;
  }
  .topbar h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
  }

  .scroll {
    position: absolute;
    inset: 56px 0 0 0;
    /* Always reserve the scrollbar (no `auto`/`stable both-edges` — older
       WebKitGTK doesn't honour `both-edges` and falls back to right-only,
       which is exactly the asymmetric look we're trying to avoid). The
       scrollbar is 6px (see app.css). We add 6px to padding-left so the
       visible gap from the app edge to the panel edge is identical on
       both sides. */
    overflow-y: scroll;
    padding: 0 14px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h2 {
    margin: 0;
    padding: 0 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .theme-card { padding: 12px; }
  .theme-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .theme-tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 12px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text);
  }
  .theme-tile.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-faint);
  }
  .swatch {
    width: 100%;
    height: 56px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .swatch-dark { background: linear-gradient(135deg, #1a1a1a 50%, #2e2e2e 50%); }
  .swatch-light { background: linear-gradient(135deg, #ffffff 50%, #ebebeb 50%); }

  /* Same .list-row layout as the design system, but using <label> so the
     entire row activates the toggle without the cell extending past the
     visual edge. */
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    cursor: pointer;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row:hover {
    background: var(--bg-elev-2);
  }
  .list {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .row-text {
    flex: 1;
    min-width: 0;
  }
  .row-title { font-size: 14px; }
  .row-sub { font-size: 12px; margin-top: 2px; }

  .status-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-right: 2px;
  }
  .status-dot.on {
    background: #2eb872;
    box-shadow: 0 0 0 3px rgba(46, 184, 114, 0.18);
  }
  .status-dot.off {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-faint);
  }
  .status-dot.busy {
    background: var(--warn);
    box-shadow: 0 0 0 3px rgba(245, 165, 36, 0.22);
    animation: status-pulse 1.2s ease-in-out infinite;
  }
  @keyframes status-pulse {
    50% { opacity: 0.45; }
  }

  /* ---------- version-picker modal ---------- */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 16px;
  }
  .modal {
    width: min(500px, 100%);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
  }
  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .modal h2 {
    margin: 0;
    font-size: 16px;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
    padding: 0;
  }
  .modal p { margin: 0; font-size: 13px; }
  .icon-btn {
    background: transparent;
    border: 0;
    color: var(--text-muted);
    padding: 6px;
    border-radius: 8px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background var(--transition), color var(--transition);
  }
  .icon-btn:hover { background: var(--bg-elev-2); color: var(--text); }

  /* The Versions button on the core row carries its own icon. */
  .btn-ico {
    margin-right: 6px;
    vertical-align: -2px;
  }

  .ver-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-elev-2);
  }
  .ver-list li + li { border-top: 1px solid var(--border); }
  .ver-row {
    color: var(--text);
    padding: 10px 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 52px;
  }
  .ver-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .ver-row.current {
    background: var(--accent-faint);
  }
  .ver-row.current .ver-tag { color: var(--accent); }
  .ver-tag { font-weight: 600; font-size: 13px; }
  .ver-meta { font-size: 11px; display: flex; align-items: center; gap: 6px; }
  .badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 999px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .ver-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  /* Compact variant of the standard btn for the row actions. */
  :global(.btn.btn-sm) {
    font-size: 12px;
    padding: 6px 10px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  /* Destructive (delete) — outline red so it reads as "removes data" without
     screaming for attention next to a primary action. */
  :global(.btn.btn-danger) {
    border-color: var(--danger-faint);
    color: var(--danger);
    background: transparent;
  }
  :global(.btn.btn-danger:hover:not(:disabled)) {
    background: var(--danger-faint);
    border-color: var(--danger);
  }


  /* Download progress: filled bar + bytes/speed line. */
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 160px;
    flex-shrink: 0;
  }
  .progress-track {
    height: 6px;
    background: var(--bg-elev);
    border-radius: 999px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 120ms linear;
  }
  .progress-meta {
    font-size: 11px;
    text-align: right;
  }
</style>

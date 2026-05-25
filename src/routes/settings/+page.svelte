<script lang="ts">
  import { theme } from "$lib/theme.svelte";
  import { settings, type LogLevel } from "$lib/settings.svelte";
  import { i18n, t, LANGUAGES, type Lang } from "$lib/i18n.svelte";
  import Dropdown from "$lib/components/Dropdown.svelte";

  const logOptions = [
    { value: "warn",  label: "Warn"  },
    { value: "info",  label: "Info"  },
    { value: "debug", label: "Debug" },
  ];
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
          <div class="row-title">{t("settings.autostart")}</div>
          <div class="row-sub muted">{t("settings.autostartSub")}</div>
        </div>
        <span class="switch">
          <input
            type="checkbox"
            checked={settings.autostart}
            onchange={(e) => settings.setAutostart((e.currentTarget as HTMLInputElement).checked)}
          />
          <span class="slider"></span>
        </span>
      </label>
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
    <h2>{t("settings.diagnostics")}</h2>
    <div class="list">
      <div class="row">
        <div class="row-text">
          <div class="row-title">{t("settings.logLevel")}</div>
          <div class="row-sub muted">{t("settings.logLevelSub")}</div>
        </div>
        <Dropdown
          value={settings.logLevel}
          options={logOptions}
          onChange={(v) => settings.setLogLevel(v as LogLevel)}
          ariaLabel={t("settings.logLevel")}
        />
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.about")}</h2>
    <div class="card about">
      <div class="about-row">
        <div class="about-name">AegisVPN</div>
        <div class="about-version muted">v0.1.0</div>
      </div>
      <div class="about-desc muted">{t("settings.aboutDesc")}</div>
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
    overflow-y: auto;
    padding: 0 14px 24px;
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

  .about {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .about-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .about-name { font-weight: 600; }
  .about-version {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .about-desc { font-size: 12px; }

</style>

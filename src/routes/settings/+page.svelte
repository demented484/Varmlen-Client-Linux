<script lang="ts">
  import { theme, type Theme } from "$lib/theme.svelte";

  function setTheme(t: Theme) {
    theme.set(t);
  }
</script>

<header class="topbar">
  <h1>Settings</h1>
</header>

<main class="scroll">
  <section>
    <h2>Appearance</h2>
    <div class="card theme-card">
      <div class="theme-row">
        <button
          class="theme-tile"
          class:active={theme.current === "dark"}
          onclick={() => setTheme("dark")}
          aria-pressed={theme.current === "dark"}
        >
          <div class="swatch swatch-dark"></div>
          <span>Dark</span>
        </button>
        <button
          class="theme-tile"
          class:active={theme.current === "light"}
          onclick={() => setTheme("light")}
          aria-pressed={theme.current === "light"}
        >
          <div class="swatch swatch-light"></div>
          <span>Light</span>
        </button>
      </div>
    </div>
  </section>

  <section>
    <h2>About</h2>
    <div class="card about">
      <div class="about-row">
        <div class="about-name">AegisVPN</div>
        <div class="about-version muted">v0.1.0</div>
      </div>
      <div class="about-desc muted">
        Open-source sing-box client. Licensed under AGPL-3.0.
      </div>
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

  .theme-card {
    padding: 12px;
  }
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
  .about-name {
    font-weight: 600;
  }
  .about-version {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .about-desc {
    font-size: 12px;
  }
</style>

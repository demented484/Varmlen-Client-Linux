<script lang="ts">
  let autostart = $state(false);
  let killswitch = $state(true);
  let allowLan = $state(true);
  let logLevel = $state<"warn" | "info" | "debug">("warn");
</script>

<div class="settings">
  <header>
    <h1>Settings</h1>
    <p class="muted">Client behavior and diagnostics.</p>
  </header>

  <section class="card group">
    <h2>General</h2>

    <div class="row">
      <div>
        <div class="row-title">Launch on system startup</div>
        <div class="row-sub muted">Open AegisVPN automatically after login.</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={autostart} />
        <span class="slider"></span>
      </label>
    </div>

    <div class="row">
      <div>
        <div class="row-title">Killswitch</div>
        <div class="row-sub muted">Block all traffic if the VPN connection drops.</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={killswitch} />
        <span class="slider"></span>
      </label>
    </div>

    <div class="row">
      <div>
        <div class="row-title">Allow LAN traffic</div>
        <div class="row-sub muted">Keep access to printers, NAS, and other local devices while connected.</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={allowLan} />
        <span class="slider"></span>
      </label>
    </div>
  </section>

  <section class="card group">
    <h2>Diagnostics</h2>

    <div class="row">
      <div>
        <div class="row-title">Log level</div>
        <div class="row-sub muted">Set sing-box verbosity. Use <code>debug</code> only when reporting bugs.</div>
      </div>
      <select bind:value={logLevel}>
        <option value="warn">Warn</option>
        <option value="info">Info</option>
        <option value="debug">Debug</option>
      </select>
    </div>

    <div class="row">
      <div>
        <div class="row-title">Open log directory</div>
        <div class="row-sub muted">Show the folder containing connection logs.</div>
      </div>
      <button class="btn">Open</button>
    </div>
  </section>

  <section class="card group">
    <h2>About</h2>
    <div class="about dim">
      AegisVPN v0.1.0 — open-source sing-box client. AGPL-3.0.
    </div>
  </section>
</div>

<style>
  .settings {
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  header h1 {
    margin: 0 0 4px;
    font-size: 22px;
    font-weight: 600;
  }
  header p {
    margin: 0;
    font-size: 13px;
  }

  .group {
    padding: 16px 20px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .row:first-of-type {
    border-top: none;
    padding-top: 0;
  }

  .row-title {
    font-size: 14px;
  }
  .row-sub {
    font-size: 12px;
    margin-top: 2px;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 38px;
    height: 22px;
    flex-shrink: 0;
  }
  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }
  .slider {
    position: absolute;
    inset: 0;
    cursor: pointer;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 22px;
    transition: background var(--transition), border-color var(--transition);
  }
  .slider::before {
    content: "";
    position: absolute;
    width: 16px;
    height: 16px;
    left: 2px;
    top: 2px;
    background: var(--text-muted);
    border-radius: 50%;
    transition: transform var(--transition), background var(--transition);
  }
  .switch input:checked + .slider {
    background: var(--accent);
    border-color: var(--accent);
  }
  .switch input:checked + .slider::before {
    background: #0a1a10;
    transform: translateX(16px);
  }

  .about {
    font-size: 13px;
  }

  code {
    font-family: ui-monospace, "JetBrains Mono", "Cascadia Code", monospace;
    background: var(--bg-elev);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.95em;
  }
</style>

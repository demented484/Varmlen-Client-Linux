<script lang="ts">
  type Status = "disconnected" | "connecting" | "connected";

  let status = $state<Status>("disconnected");
  let currentServerName = $state("No server selected");
  let elapsedSec = $state(0);
  let timer: ReturnType<typeof setInterval> | null = null;

  const statusLabel = $derived(
    {
      disconnected: "Disconnected",
      connecting: "Connecting…",
      connected: "Connected",
    }[status],
  );

  const statusColor = $derived(
    {
      disconnected: "var(--text-muted)",
      connecting: "var(--warn)",
      connected: "var(--accent)",
    }[status],
  );

  function formatElapsed(s: number): string {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    const pad = (n: number) => n.toString().padStart(2, "0");
    return h ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
  }

  async function toggle() {
    if (status === "disconnected") {
      status = "connecting";
      // TODO: invoke('start_singbox', { ... })
      setTimeout(() => {
        status = "connected";
        elapsedSec = 0;
        timer = setInterval(() => (elapsedSec += 1), 1000);
      }, 800);
    } else {
      // TODO: invoke('stop_singbox')
      status = "disconnected";
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
      elapsedSec = 0;
    }
  }
</script>

<div class="dashboard">
  <header>
    <h1>Dashboard</h1>
    <p class="muted">Connection status and quick controls.</p>
  </header>

  <section class="hero card">
    <div class="status-row">
      <span class="dot" style="background: {statusColor}"></span>
      <span class="status-text" style="color: {statusColor}">{statusLabel}</span>
    </div>

    <button
      class="power"
      class:on={status === "connected"}
      class:loading={status === "connecting"}
      onclick={toggle}
      aria-label={status === "connected" ? "Disconnect" : "Connect"}
    >
      <svg viewBox="0 0 64 64" width="80" height="80" aria-hidden="true">
        <circle cx="32" cy="32" r="28" stroke="currentColor" stroke-width="2.5" fill="none" opacity="0.25" />
        <path
          d="M22 18a16 16 0 1 0 20 0"
          stroke="currentColor"
          stroke-width="3"
          stroke-linecap="round"
          fill="none"
        />
        <line x1="32" y1="12" x2="32" y2="30" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
      </svg>
    </button>

    <div class="server-name">{currentServerName}</div>
    {#if status === "connected"}
      <div class="elapsed">{formatElapsed(elapsedSec)}</div>
    {/if}
  </section>

  <section class="stats">
    <div class="stat card">
      <div class="stat-label muted">Download</div>
      <div class="stat-value">— <span class="unit muted">MB/s</span></div>
    </div>
    <div class="stat card">
      <div class="stat-label muted">Upload</div>
      <div class="stat-value">— <span class="unit muted">MB/s</span></div>
    </div>
    <div class="stat card">
      <div class="stat-label muted">Latency</div>
      <div class="stat-value">— <span class="unit muted">ms</span></div>
    </div>
  </section>
</div>

<style>
  .dashboard {
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 24px;
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

  .hero {
    padding: 36px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    transition: background var(--transition);
  }
  .status-text {
    font-weight: 600;
  }

  .power {
    width: 140px;
    height: 140px;
    border-radius: 50%;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all var(--transition);
    padding: 0;
  }
  .power:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .power.on {
    background: var(--accent-faint);
    border-color: var(--accent);
    color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-faint);
  }
  .power.loading {
    color: var(--warn);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.7;
    }
    50% {
      opacity: 1;
    }
  }

  .server-name {
    font-size: 15px;
    font-weight: 500;
  }
  .elapsed {
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    color: var(--text-muted);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .stat {
    padding: 14px 16px;
  }
  .stat-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .stat-value {
    font-size: 22px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 12px;
    font-weight: 400;
    margin-left: 2px;
  }
</style>

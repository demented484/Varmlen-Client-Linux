<script lang="ts">
  interface Server {
    id: string;
    name: string;
    host: string;
    transport: string;
    latencyMs: number | null;
  }

  let subUrl = $state("");
  // Static mock until M1 wires the parser.
  let servers = $state<Server[]>([]);
  let importing = $state(false);

  async function importSubscription() {
    if (!subUrl.trim()) return;
    importing = true;
    try {
      // TODO: invoke('import_subscription', { url: subUrl })
      await new Promise((r) => setTimeout(r, 600));
      servers = [
        { id: "fi", name: "Finland Exp", host: "89.125.181.236", transport: "xhttp / reality", latencyMs: null },
        { id: "se", name: "Sweden | Stockholm", host: "se.89-125-138-157.sslip.io", transport: "xhttp / reality", latencyMs: null },
        { id: "us", name: "USA | New York", host: "us.166-1-62-246.sslip.io", transport: "xhttp / reality", latencyMs: null },
      ];
      subUrl = "";
    } finally {
      importing = false;
    }
  }
</script>

<div class="servers">
  <header>
    <h1>Servers</h1>
    <p class="muted">Add a subscription URL or import a single VLESS link.</p>
  </header>

  <section class="card import">
    <label for="sub-url" class="label">Subscription URL</label>
    <div class="import-row">
      <input
        id="sub-url"
        type="url"
        placeholder="https://example.com/sub/abc123"
        bind:value={subUrl}
        disabled={importing}
      />
      <button class="btn btn-primary" onclick={importSubscription} disabled={importing || !subUrl.trim()}>
        {importing ? "Importing…" : "Import"}
      </button>
    </div>
  </section>

  <section>
    <h2>Configured ({servers.length})</h2>
    {#if servers.length === 0}
      <div class="empty card">
        <p class="muted">No servers yet. Paste a subscription URL above.</p>
      </div>
    {:else}
      <ul class="server-list">
        {#each servers as s}
          <li class="card server">
            <div class="server-main">
              <div class="server-name">{s.name}</div>
              <div class="server-host muted">{s.host}</div>
            </div>
            <div class="server-meta dim">{s.transport}</div>
            <div class="server-latency">
              {s.latencyMs !== null ? `${s.latencyMs} ms` : "n/a"}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .servers {
    max-width: 820px;
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

  h2 {
    margin: 0 0 12px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .import {
    padding: 16px;
  }
  .label {
    display: block;
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: 8px;
  }
  .import-row {
    display: flex;
    gap: 8px;
  }
  .import-row input {
    flex: 1;
  }

  .empty {
    padding: 24px;
    text-align: center;
  }
  .empty p {
    margin: 0;
  }

  .server-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .server {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 16px;
    align-items: center;
    padding: 12px 16px;
  }

  .server-name {
    font-weight: 500;
  }
  .server-host {
    font-size: 12px;
    margin-top: 2px;
  }
  .server-meta {
    font-size: 12px;
  }
  .server-latency {
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    color: var(--text-muted);
  }
</style>

<script lang="ts">
  type Action = "proxy" | "direct" | "block";
  type Kind = "domain" | "process";

  interface Rule {
    id: string;
    kind: Kind;
    pattern: string;
    action: Action;
    enabled: boolean;
  }

  const ACTION_COLOR: Record<Action, string> = {
    proxy: "var(--accent)",
    direct: "var(--warn)",
    block: "var(--danger)",
  };

  let rules = $state<Rule[]>([
    { id: "1", kind: "domain", pattern: "*.ru", action: "direct", enabled: true },
    { id: "2", kind: "domain", pattern: "instagram.com", action: "proxy", enabled: true },
    { id: "3", kind: "process", pattern: "telegram-desktop", action: "proxy", enabled: true },
    { id: "4", kind: "domain", pattern: "*.doubleclick.net", action: "block", enabled: false },
  ]);

  let draftPattern = $state("");
  let draftKind = $state<Kind>("domain");
  let draftAction = $state<Action>("proxy");

  function addRule() {
    if (!draftPattern.trim()) return;
    rules = [
      ...rules,
      {
        id: crypto.randomUUID(),
        kind: draftKind,
        pattern: draftPattern.trim(),
        action: draftAction,
        enabled: true,
      },
    ];
    draftPattern = "";
  }

  function toggleRule(id: string) {
    rules = rules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r));
  }

  function removeRule(id: string) {
    rules = rules.filter((r) => r.id !== id);
  }

  function moveRule(id: string, dir: -1 | 1) {
    const i = rules.findIndex((r) => r.id === id);
    if (i < 0) return;
    const j = i + dir;
    if (j < 0 || j >= rules.length) return;
    const copy = [...rules];
    [copy[i], copy[j]] = [copy[j], copy[i]];
    rules = copy;
  }
</script>

<div class="rules">
  <header>
    <h1>Split-tunnel rules</h1>
    <p class="muted">Rules are evaluated top-down. The first matching rule wins.</p>
  </header>

  <section class="card add-rule">
    <select bind:value={draftKind}>
      <option value="domain">Domain</option>
      <option value="process">Process</option>
    </select>
    <input
      type="text"
      placeholder={draftKind === "domain" ? "example.com or *.example.com" : "telegram-desktop"}
      bind:value={draftPattern}
      onkeydown={(e) => e.key === "Enter" && addRule()}
    />
    <select bind:value={draftAction}>
      <option value="proxy">Through VPN</option>
      <option value="direct">Direct (bypass)</option>
      <option value="block">Block</option>
    </select>
    <button class="btn btn-primary" onclick={addRule} disabled={!draftPattern.trim()}>Add</button>
  </section>

  <section>
    <h2>Active rules ({rules.length})</h2>
    {#if rules.length === 0}
      <div class="empty card"><p class="muted">No rules. All traffic flows through the VPN.</p></div>
    {:else}
      <ol class="rule-list">
        {#each rules as r, i (r.id)}
          <li class="card rule" class:dim={!r.enabled}>
            <div class="rule-index dim">{i + 1}</div>
            <div class="rule-kind">{r.kind}</div>
            <div class="rule-pattern">{r.pattern}</div>
            <div class="rule-action" style="color: {ACTION_COLOR[r.action]}">
              {r.action}
            </div>
            <div class="rule-controls">
              <button class="icon" onclick={() => moveRule(r.id, -1)} disabled={i === 0} title="Move up">↑</button>
              <button class="icon" onclick={() => moveRule(r.id, 1)} disabled={i === rules.length - 1} title="Move down">↓</button>
              <button class="icon" onclick={() => toggleRule(r.id)} title={r.enabled ? "Disable" : "Enable"}>
                {r.enabled ? "●" : "○"}
              </button>
              <button class="icon danger" onclick={() => removeRule(r.id)} title="Delete">✕</button>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </section>

  <section class="hint dim">
    <p>
      Per-process rules require sing-box to read <code>/proc</code>; on Linux that's automatic. On
      Android, process patterns become package names (e.g. <code>org.telegram.messenger</code>).
    </p>
  </section>
</div>

<style>
  .rules {
    max-width: 900px;
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

  .add-rule {
    padding: 12px;
    display: grid;
    grid-template-columns: 120px 1fr 160px auto;
    gap: 8px;
  }

  .empty {
    padding: 20px;
    text-align: center;
  }

  .rule-list {
    list-style: none;
    counter-reset: ruleidx;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rule {
    display: grid;
    grid-template-columns: 32px 80px 1fr 110px auto;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    transition: opacity var(--transition);
  }
  .rule.dim {
    opacity: 0.45;
  }

  .rule-index {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .rule-kind {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .rule-pattern {
    font-family: ui-monospace, "JetBrains Mono", "Cascadia Code", monospace;
    font-size: 13px;
  }
  .rule-action {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }
  .rule-controls {
    display: flex;
    gap: 2px;
  }
  .icon {
    width: 26px;
    height: 26px;
    padding: 0;
    font-size: 13px;
    color: var(--text-muted);
  }
  .icon:hover:not(:disabled) {
    background: var(--bg-elev-2);
    color: var(--text);
  }
  .icon.danger:hover:not(:disabled) {
    color: var(--danger);
  }

  .hint {
    font-size: 12px;
  }
  .hint p {
    margin: 0;
  }
  code {
    font-family: ui-monospace, "JetBrains Mono", "Cascadia Code", monospace;
    background: var(--bg-elev);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.95em;
  }
</style>

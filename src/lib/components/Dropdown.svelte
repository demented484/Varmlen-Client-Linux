<script lang="ts">
  interface Option<V extends string> {
    value: V;
    label: string;
  }

  interface Props<V extends string> {
    value: V;
    options: Option<V>[];
    onChange: (v: V) => void;
    ariaLabel?: string;
  }

  let { value, options, onChange, ariaLabel = "Select" }: Props<string> = $props();

  let open = $state(false);
  let trigger: HTMLButtonElement | undefined = $state();
  let panel: HTMLDivElement | undefined = $state();

  const current = $derived(
    options.find((o) => o.value === value)?.label ?? value,
  );

  function handleDocClick(e: MouseEvent) {
    if (!open) return;
    const t = e.target as Node | null;
    if (t && (trigger?.contains(t) || panel?.contains(t))) return;
    open = false;
  }

  $effect(() => {
    if (open) {
      document.addEventListener("click", handleDocClick, true);
      return () => document.removeEventListener("click", handleDocClick, true);
    }
  });

  function pick(v: string) {
    onChange(v);
    open = false;
  }
</script>

<div class="dd">
  <button
    bind:this={trigger}
    type="button"
    class="trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={ariaLabel}
    onclick={() => (open = !open)}
  >
    <span class="trigger-text">{current}</span>
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      class="caret"
      class:flipped={open}
      aria-hidden="true"
    >
      <path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
  {#if open}
    <div bind:this={panel} class="panel" role="listbox">
      {#each options as opt (opt.value)}
        <button
          type="button"
          class="opt"
          class:selected={opt.value === value}
          role="option"
          aria-selected={opt.value === value}
          onclick={() => pick(opt.value)}
        >
          <span>{opt.label}</span>
          {#if opt.value === value}
            <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M5 12.5L10 17.5L19.5 8" stroke="var(--accent)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dd {
    position: relative;
    flex-shrink: 0;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 12px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text);
  }
  .trigger:hover {
    border-color: var(--border-strong);
  }
  .trigger-text {
    font-weight: 500;
  }
  .caret {
    color: var(--text-muted);
    transition: transform var(--transition);
  }
  .caret.flipped {
    transform: rotate(180deg);
  }

  .panel {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    min-width: 120px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow);
    padding: 4px;
    z-index: 30;
  }
  .opt {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: var(--text);
    font-size: 13px;
    text-align: left;
  }
  .opt:hover {
    background: var(--bg-elev-3);
  }
  .opt.selected {
    color: var(--accent);
  }
</style>

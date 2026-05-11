<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";
  import { NAV } from "$lib/nav";

  let { children } = $props();

  function isActive(path: string): boolean {
    if (path === "/") return page.url.pathname === "/";
    return page.url.pathname.startsWith(path);
  }
</script>

<div class="app">
  <aside class="sidebar">
    <div class="brand">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path
          d="M12 2 4 5v6c0 5 3.5 9.5 8 11 4.5-1.5 8-6 8-11V5l-8-3z"
          stroke="var(--accent)"
          stroke-width="1.7"
          stroke-linejoin="round"
        />
        <path
          d="M9 12l2 2 4-4"
          stroke="var(--accent)"
          stroke-width="1.7"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <span>AegisVPN</span>
    </div>

    <nav>
      {#each NAV as item}
        <a
          href={item.path}
          class="nav-item"
          class:active={isActive(item.path)}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
            <path d={item.icon} fill="currentColor" />
          </svg>
          <span>{item.label}</span>
        </a>
      {/each}
    </nav>

    <div class="version dim">v0.1.0 · early dev</div>
  </aside>

  <main class="content">
    {@render children?.()}
  </main>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100vh;
    background: var(--bg);
  }

  .sidebar {
    background: var(--bg-elev);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 16px 12px;
    gap: 4px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.2px;
    padding: 6px 8px 18px;
    color: var(--text);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    transition: background var(--transition), color var(--transition);
  }
  .nav-item:hover {
    background: var(--bg-elev-2);
    color: var(--text);
  }
  .nav-item.active {
    background: var(--accent-faint);
    color: var(--accent);
  }
  .nav-item svg {
    flex-shrink: 0;
  }

  .version {
    font-size: 11px;
    text-align: center;
    padding: 8px;
  }

  .content {
    overflow-y: auto;
    padding: 28px 32px;
  }
</style>

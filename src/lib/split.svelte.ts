import { browser } from "$app/environment";

export type Mode = "selective" | "general";

export interface AppEntry {
  /** Process name on Linux/Windows or package on Android. */
  id: string;
  /** Display name. */
  name: string;
  /** Emoji or short text placeholder until we resolve real icons. */
  icon: string;
  enabled: boolean;
}

export interface SiteEntry {
  id: string;
  pattern: string;
  enabled: boolean;
}

interface Persisted {
  appsMode: Mode;
  sitesMode: Mode;
  apps: AppEntry[];
  sites: SiteEntry[];
}

const KEY = "aegisvpn.split";
const DEFAULTS: Persisted = {
  appsMode: "selective",
  sitesMode: "selective",
  apps: [],
  sites: [],
};

function load(): Persisted {
  if (!browser) return DEFAULTS;
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<Persisted> & { mode?: Mode };
    // Migrate legacy single `mode` field if present.
    const legacy = parsed.mode === "general" ? "general" : "selective";
    return {
      appsMode: parsed.appsMode === "general" ? "general"
              : parsed.appsMode === "selective" ? "selective" : legacy,
      sitesMode: parsed.sitesMode === "general" ? "general"
               : parsed.sitesMode === "selective" ? "selective" : legacy,
      apps: Array.isArray(parsed.apps) ? parsed.apps : [],
      sites: Array.isArray(parsed.sites) ? parsed.sites : [],
    };
  } catch {
    return DEFAULTS;
  }
}

class SplitStore {
  private readonly _initial = load();
  appsMode = $state<Mode>(this._initial.appsMode);
  sitesMode = $state<Mode>(this._initial.sitesMode);
  apps = $state<AppEntry[]>(this._initial.apps);
  sites = $state<SiteEntry[]>(this._initial.sites);

  private persist(): void {
    if (!browser) return;
    const payload: Persisted = {
      appsMode: this.appsMode,
      sitesMode: this.sitesMode,
      apps: this.apps,
      sites: this.sites,
    };
    localStorage.setItem(KEY, JSON.stringify(payload));
  }

  setAppsMode(m: Mode): void {
    this.appsMode = m;
    this.persist();
  }

  setSitesMode(m: Mode): void {
    this.sitesMode = m;
    this.persist();
  }

  toggleApp(id: string): void {
    this.apps = this.apps.map((a) =>
      a.id === id ? { ...a, enabled: !a.enabled } : a,
    );
    this.persist();
  }

  addApp(app: Omit<AppEntry, "enabled">): void {
    if (this.apps.some((a) => a.id === app.id)) return;
    this.apps = [...this.apps, { ...app, enabled: true }];
    this.persist();
  }

  removeApp(id: string): void {
    this.apps = this.apps.filter((a) => a.id !== id);
    this.persist();
  }

  addSite(pattern: string): void {
    const v = pattern.trim();
    if (!v) return;
    if (this.sites.some((s) => s.pattern === v)) return;
    this.sites = [
      ...this.sites,
      { id: crypto.randomUUID(), pattern: v, enabled: true },
    ];
    this.persist();
  }

  toggleSite(id: string): void {
    this.sites = this.sites.map((s) =>
      s.id === id ? { ...s, enabled: !s.enabled } : s,
    );
    this.persist();
  }

  removeSite(id: string): void {
    this.sites = this.sites.filter((s) => s.id !== id);
    this.persist();
  }
}

export const split = new SplitStore();

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

interface Bucket {
  apps: AppEntry[];
  sites: SiteEntry[];
}

/** Each mode keeps its OWN apps + sites, so switching general↔selective swaps
 *  the whole list. The mode still governs whether the listed entries are the
 *  blacklist (general: listed stay direct) or the whitelist (selective: only
 *  listed are tunneled). */
interface Persisted {
  mode: Mode;
  general: Bucket;
  selective: Bucket;
}

const KEY = "varmlen.split";

function emptyBucket(): Bucket {
  return { apps: [], sites: [] };
}

function defaults(): Persisted {
  // Default to "general": VPN carries everything, exceptions are direct.
  return { mode: "general", general: emptyBucket(), selective: emptyBucket() };
}

function asBucket(x: unknown): Bucket {
  const o = (x ?? {}) as Record<string, unknown>;
  return {
    apps: Array.isArray(o.apps) ? (o.apps as AppEntry[]) : [],
    sites: Array.isArray(o.sites) ? (o.sites as SiteEntry[]) : [],
  };
}

function load(): Persisted {
  if (!browser) return defaults();
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return defaults();
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const mode: Mode = parsed.mode === "selective" ? "selective" : "general";

    // Current shape: a bucket per mode.
    if (parsed.general || parsed.selective) {
      return {
        mode,
        general: asBucket(parsed.general),
        selective: asBucket(parsed.selective),
      };
    }

    // Prior unified shape: one apps/sites list shared by both modes. Migrate it
    // into the bucket for the persisted mode; the other mode starts empty.
    if (Array.isArray(parsed.apps) || Array.isArray(parsed.sites)) {
      const out = defaults();
      out.mode = mode;
      out[mode] = asBucket(parsed);
      return out;
    }

    return { ...defaults(), mode };
  } catch {
    return defaults();
  }
}

const _initialSplit = load();

class SplitStore {
  mode = $state<Mode>(_initialSplit.mode);
  general = $state<Bucket>(_initialSplit.general);
  selective = $state<Bucket>(_initialSplit.selective);

  /** The active mode's bucket. */
  private get bucket(): Bucket {
    return this.mode === "selective" ? this.selective : this.general;
  }
  private setBucket(next: Bucket): void {
    if (this.mode === "selective") this.selective = next;
    else this.general = next;
    this.persist();
  }

  /** Active-mode apps/sites — what the UI binds to and what's sent to the
   *  backend. Switching mode swaps these. */
  get apps(): AppEntry[] {
    return this.bucket.apps;
  }
  get sites(): SiteEntry[] {
    return this.bucket.sites;
  }

  private persist(): void {
    if (!browser) return;
    const payload: Persisted = {
      mode: this.mode,
      general: this.general,
      selective: this.selective,
    };
    localStorage.setItem(KEY, JSON.stringify(payload));
  }

  setMode(m: Mode): void {
    this.mode = m;
    this.persist();
  }

  toggleApp(id: string): void {
    this.setBucket({
      ...this.bucket,
      apps: this.bucket.apps.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a)),
    });
  }

  addApp(app: Omit<AppEntry, "enabled">): void {
    if (this.bucket.apps.some((a) => a.id === app.id)) return;
    this.setBucket({ ...this.bucket, apps: [...this.bucket.apps, { ...app, enabled: true }] });
  }

  removeApp(id: string): void {
    this.setBucket({ ...this.bucket, apps: this.bucket.apps.filter((a) => a.id !== id) });
  }

  addSite(pattern: string): void {
    const v = pattern.trim();
    if (!v) return;
    if (this.bucket.sites.some((s) => s.pattern === v)) return;
    this.setBucket({
      ...this.bucket,
      sites: [...this.bucket.sites, { id: crypto.randomUUID(), pattern: v, enabled: true }],
    });
  }

  toggleSite(id: string): void {
    this.setBucket({
      ...this.bucket,
      sites: this.bucket.sites.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s)),
    });
  }

  removeSite(id: string): void {
    this.setBucket({ ...this.bucket, sites: this.bucket.sites.filter((s) => s.id !== id) });
  }
}

export const split = new SplitStore();

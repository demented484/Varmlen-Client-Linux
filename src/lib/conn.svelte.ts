import { vpnConnect, vpnDisconnect, vpnStatus, type SplitInput } from "$lib/api";
import { subs } from "$lib/subs.svelte";
import { split } from "$lib/split.svelte";
import { settings } from "$lib/settings.svelte";
import { t } from "$lib/i18n.svelte";
import { listen } from "@tauri-apps/api/event";

// "dropped": the tunnel died unexpectedly and the kill switch is holding traffic
// blocked (fail-closed). Distinct from "disconnected" so the UI can say so.
export type Status = "disconnected" | "connecting" | "connected" | "dropped";

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Current split-tunnel selection (enabled entries for the active mode). */
function splitInput(): SplitInput {
  return {
    mode: split.mode,
    apps: split.apps.filter((a) => a.enabled).map((a) => a.id),
    sites: split.sites.filter((s) => s.enabled).map((s) => s.pattern),
  };
}

class ConnStore {
  status = $state<Status>("disconnected");
  /** Last connect error, surfaced under the power button. */
  error = $state<string | null>(null);
  /** True while in the "dropped" phase WITH the kill switch holding traffic. */
  blockedByKillswitch = $state(false);

  private reapplyTimer: ReturnType<typeof setTimeout> | null = null;
  private dropListenerStarted = false;

  /** Subscribe to backend "vpn-dropped" events (tunnel died unexpectedly). The
   *  payload is `true` when the kill switch is holding traffic blocked. */
  startDropListener(): void {
    if (this.dropListenerStarted) return;
    this.dropListenerStarted = true;
    void listen<boolean>("vpn-dropped", (e) => {
      if (e.payload) {
        this.status = "dropped";
        this.blockedByKillswitch = true;
        this.error = t("conn.dropped");
      } else {
        this.status = "disconnected";
        this.blockedByKillswitch = false;
        this.error = t("conn.droppedNoKill");
      }
    });
  }
  /** Signature of the config last applied, to avoid redundant reconnects. */
  private lastSig: string | null = null;

  /** Signature of everything that affects the generated config. */
  private configSig(): string {
    return JSON.stringify({
      server: subs.selectedServerId,
      mode: settings.vpnMode,
      killswitch: settings.killswitch,
      allowLan: settings.allowLan,
      split: splitInput(),
    });
  }

  /** Called reactively when config (location / split / mode / settings)
   *  changes: while connected, debounce-reconnect with the new config so the
   *  change takes effect live. The killswitch (if on) holds across the gap. */
  onConfigChanged(): void {
    const sig = this.configSig();
    if (this.lastSig === null) {
      this.lastSig = sig; // baseline on first run, no reconnect
      return;
    }
    if (sig === this.lastSig) return;
    this.lastSig = sig;
    if (this.status !== "connected" && this.status !== "connecting") return;
    if (this.reapplyTimer) clearTimeout(this.reapplyTimer);
    this.reapplyTimer = setTimeout(() => void this.connect(), 500);
  }

  async toggle(): Promise<void> {
    if (this.status === "connected" || this.status === "connecting") {
      await this.disconnect();
    } else if (this.status === "dropped") {
      // From a kill-switch-blocked drop, the power button reconnects.
      await this.connect();
    } else {
      await this.connect();
    }
  }

  /** Clear a kill-switch-blocked drop without reconnecting (restores traffic). */
  async clearDrop(): Promise<void> {
    await this.disconnect();
  }

  async connect(): Promise<void> {
    this.error = null;
    this.blockedByKillswitch = false;
    const server = subs.selectedServerRaw();
    if (!server) {
      this.error = t("conn.selectLocation");
      return;
    }
    this.status = "connecting";
    this.lastSig = this.configSig();
    // Hold the "connecting" indicator visible for at least this long even
    // when the helper rejects in <50ms — otherwise the spinner / animated
    // ring is gone before the user perceives it, and a fast failure looks
    // like "the button does nothing".
    const startedAt = Date.now();
    const MIN_CONNECTING_MS = 700;
    try {
      const resp = await vpnConnect(
        server,
        splitInput(),
        settings.vpnMode,
        settings.killswitch,
        settings.allowLan,
      );
      const remain = MIN_CONNECTING_MS - (Date.now() - startedAt);
      if (remain > 0) await new Promise((r) => setTimeout(r, remain));
      if (!resp.ok) throw new Error(resp.error || "connection failed");
      this.status = "connected";
    } catch (e) {
      const remain = MIN_CONNECTING_MS - (Date.now() - startedAt);
      if (remain > 0) await new Promise((r) => setTimeout(r, remain));
      this.error = msg(e);
      this.status = "disconnected";
    }
  }

  async disconnect(): Promise<void> {
    try {
      await vpnDisconnect();
    } catch {
      // best effort — drop to disconnected regardless
    }
    this.status = "disconnected";
    this.blockedByKillswitch = false;
    this.error = null;
  }

  /** Reconcile UI with the helper's actual state (e.g. window recreated while
   *  still connected, or core crashed/dropped while the window was away). */
  async refresh(): Promise<void> {
    this.startDropListener();
    try {
      const resp = await vpnStatus();
      if (resp.state === "connected") {
        this.status = "connected";
        this.blockedByKillswitch = false;
      } else if (resp.state === "dropped") {
        this.status = "dropped";
        this.blockedByKillswitch = true;
        this.error = t("conn.dropped");
      } else if (
        resp.state === "disconnected" &&
        (this.status === "connected" || this.status === "dropped")
      ) {
        this.status = "disconnected";
        this.blockedByKillswitch = false;
      }
    } catch {
      // helper unreachable — leave UI as is
    }
  }
}

export const conn = new ConnStore();

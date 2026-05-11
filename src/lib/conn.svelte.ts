export type Status = "disconnected" | "connecting" | "connected";

class ConnStore {
  status = $state<Status>("disconnected");
  elapsedSec = $state(0);
  /**
   * True while outgoing traffic is currently flowing. Wired to a real
   * sing-box statistics feed later; for now a simple simulator pulses it
   * on/off when connected, so the UI can be developed against the same
   * surface area.
   */
  outgoing = $state(false);

  private timer: ReturnType<typeof setInterval> | null = null;
  private trafficTimer: ReturnType<typeof setInterval> | null = null;

  async toggle(): Promise<void> {
    if (this.status === "disconnected") {
      this.status = "connecting";
      // TODO: invoke('start_singbox', { server_id })
      await new Promise((r) => setTimeout(r, 700));
      this.status = "connected";
      this.elapsedSec = 0;
      this.timer = setInterval(() => (this.elapsedSec += 1), 1000);
      // Simulator: traffic is "on" most of the time when connected, blinks off
      // for ~1s every ~3s, mirroring a browsing pattern. Replace with the real
      // bps reading once the agent ships.
      this.trafficTimer = setInterval(() => {
        this.outgoing = Math.random() > 0.25;
      }, 1500);
      this.outgoing = true;
    } else {
      // TODO: invoke('stop_singbox')
      this.status = "disconnected";
      this.outgoing = false;
      if (this.timer) {
        clearInterval(this.timer);
        this.timer = null;
      }
      if (this.trafficTimer) {
        clearInterval(this.trafficTimer);
        this.trafficTimer = null;
      }
      this.elapsedSec = 0;
    }
  }
}

export const conn = new ConnStore();

export function fmtElapsed(s: number): string {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(sec)}`;
}

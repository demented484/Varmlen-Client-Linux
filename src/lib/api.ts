import { invoke } from "@tauri-apps/api/core";

export interface VlessServer {
  id: string;
  /** "vless" | "trojan" | "shadowsocks" | "vmess" */
  protocol: string;
  uuid: string;
  password: string | null;
  method: string | null;
  host: string;
  port: number;
  label: string;
  transport: string;
  security: string;
  sni: string | null;
  fingerprint: string | null;
  public_key: string | null;
  short_id: string | null;
  flow: string | null;
  path: string | null;
  mode: string | null;
  packet_encoding: string | null;
  raw_params: Record<string, string>;
}

export interface SubscriptionMeta {
  title: string | null;
  update_interval_hours: number | null;
  upload_bytes: number | null;
  download_bytes: number | null;
  total_bytes: number | null;
  expires_at_unix: number | null;
  support_url: string | null;
  web_page_url: string | null;
}

export interface ImportResult {
  meta: SubscriptionMeta;
  servers: VlessServer[];
  description: string | null;
}

export function parseVlessUri(uri: string): Promise<VlessServer> {
  return invoke<VlessServer>("parse_vless_uri", { uri });
}

export function parseSubscriptionBody(body: string): Promise<VlessServer[]> {
  return invoke<VlessServer[]>("parse_subscription_body", { body });
}

export function fetchSubscription(url: string): Promise<ImportResult> {
  return invoke<ImportResult>("fetch_subscription", { url });
}

/** TCP RTT in ms. Throws on timeout / unreachable. */
export function pingTcp(host: string, port: number): Promise<number> {
  return invoke<number>("ping_tcp", { host, port });
}

export interface InstalledApp {
  /** Binary / process name used to match the running app. */
  id: string;
  /** Display name from the desktop entry. */
  name: string;
  /** Icon as a `data:image/...` URI, or null when none was resolved. */
  icon: string | null;
}

/** Installed desktop apps, parsed from the system's `.desktop` entries. */
export function listInstalledApps(): Promise<InstalledApp[]> {
  return invoke<InstalledApp[]>("list_installed_apps");
}

/** Build an app entry from a user-picked file: a `.desktop` file is parsed
 *  (name / exec / icon), any other file is treated as the binary. */
export function appFromFile(path: string): Promise<InstalledApp | null> {
  return invoke<InstalledApp | null>("app_from_file", { path });
}

/** The single leading emoji cluster at the start of a label: a country flag
 *  (two regional indicators) or one pictographic emoji (📶 …) with its
 *  modifiers / ZWJ sequence / variation selector. Only the FIRST one. */
const LEADING_EMOJI =
  /^(?:\p{Regional_Indicator}\p{Regional_Indicator}|\p{Extended_Pictographic})(?:️|\p{Emoji_Modifier}|‍\p{Extended_Pictographic}️?)*/u;

/** Split a server label into its leading emoji icon (just the first one) and
 *  the remaining text, so the icon renders in its own slot and isn't duplicated
 *  in the name. Panels prefix a country flag (or a 📶-style marker); we use
 *  whatever they send rather than guessing from the text. */
export function splitLabelEmoji(label: string): { icon: string; name: string } {
  const m = label.match(LEADING_EMOJI);
  if (!m) return { icon: "", name: label.trim() };
  return { icon: m[0], name: label.slice(m[0].length).trim() };
}

/** Server name with the leading emoji icon removed. */
export function stripLeadingFlag(label: string): string {
  return splitLabelEmoji(label).name;
}

/** The icon (flag or other leading emoji) for a server, or "" when none. */
export function flagFor(label: string): string {
  return splitLabelEmoji(label).icon;
}

/** Pretty bytes like 742.3GB / 1.5TB / 0B. */
export function formatBytes(n: number | null): string {
  if (n == null || n <= 0) return "0B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)}${units[i]}`;
}

/** Format unix seconds as DD.MM.YYYY for the expires badge. */
export function formatExpires(unix: number | null): string | null {
  if (!unix || unix <= 0) return null;
  const d = new Date(unix * 1000);
  if (!Number.isFinite(d.getTime())) return null;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
}

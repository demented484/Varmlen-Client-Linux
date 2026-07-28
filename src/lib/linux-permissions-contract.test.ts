import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

describe("Linux permission setup", () => {
  it("uses lazy daemon startup without a manual permission façade", () => {
    const settingsPage = read("../routes/settings/+page.svelte");
    const api = read("./api.ts");
    const i18n = read("./i18n.svelte.ts");
    const vpn = read("../../src-tauri/src/vpn.rs");
    const tauriLib = read("../../src-tauri/src/lib.rs");

    expect(settingsPage).not.toContain("capsGranted");
    expect(settingsPage).not.toContain("grantCaps");
    expect(settingsPage).not.toContain('t("settings.helper")');
    expect(api).not.toContain('invoke<boolean>("caps_granted")');
    expect(api).not.toContain('invoke<void>("grant_caps")');
    expect(vpn).not.toContain("pub async fn caps_granted");
    expect(vpn).not.toContain("pub async fn grant_caps");
    expect(tauriLib).not.toContain("vpn::caps_granted");
    expect(tauriLib).not.toContain("vpn::grant_caps");
    expect(i18n).toContain('"mode.proxy": "Proxy (SOCKS)"');
    expect(i18n).toContain('"mode.proxy": "Прокси (SOCKS)"');
  });
});

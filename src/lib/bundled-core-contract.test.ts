import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

describe("Linux bundled Xray fallback", () => {
  it("is seeded into the menu even when another core is already active", () => {
    const core = read("../../src-tauri/src/core.rs");
    const fetchScript = read("../../scripts/fetch-xray.sh");
    const settings = read("../routes/settings/+page.svelte");

    expect(fetchScript).toContain('VERSION="26.3.27"');
    expect(fetchScript).toContain('"$VERSION:$asset"');
    expect(core).toContain('PathBuf::from("/usr/libexec/varmlen/xray")');
    expect(core).toContain("let had_usable_active = binary_path(app, kind).is_ok()");
    expect(core).toContain("if !dest.is_file()");
    expect(core).toContain("seed_bundled_core(&app)");
    expect(core).toContain("bundled: bundled.as_deref() == Some(tag.as_str())");
    expect(core).toContain("the Xray version bundled with Varmlen cannot be removed");
    expect(settings).toContain('t("core.bundled")');
    expect(settings).toContain("{#if !v.bundled}");
  });
});

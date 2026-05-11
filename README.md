# AegisVPN

Open-source sing-box client with per-app and per-domain split tunneling. Built on Tauri 2 + SvelteKit.

> **Status:** early development. Nothing works yet.

## Goals

- Linux first, Android second, Windows last.
- Bundles [sing-box](https://github.com/SagerNet/sing-box) as the protocol engine. Compatible with any sing-box / v2ray subscription.
- Split tunneling that is actually usable:
  - per-domain rules with wildcards (`*.ru`, `instagram.com`, …) → `direct` / `proxy` / `block`
  - per-process rules (`telegram-desktop`, `discord`, …) on Linux/Windows; per-app (`org.telegram.messenger`) on Android
  - rule order is visible in the UI so you can see exactly how sing-box will resolve the next packet

## Roadmap

- [ ] M0 — Scaffold (Tauri 2 + Svelte 5)
- [ ] M1 — Parse `vless://` + base64 subscription URLs
- [ ] M2 — Bundle and spawn sing-box, connect/disconnect from the UI
- [ ] M3 — Split-tunnel rule editor with live preview of generated route rules
- [ ] M4 — System tray, autostart, killswitch
- [ ] M5 — Subscription auto-refresh
- [ ] M6 — Linux packaging (.deb / .AppImage)
- [ ] M7 — Android port (Tauri Mobile)
- [ ] M8 — Windows port

## Development

```bash
npm install
npm run tauri dev
```

Requires Rust 1.77+, Node 20+, and the system libraries documented at <https://tauri.app/start/prerequisites/>.

## License

[AGPL-3.0-only](./LICENSE). If you fork this and run it as a service, your modifications must be open-sourced under the same license.

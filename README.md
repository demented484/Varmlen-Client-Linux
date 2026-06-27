# Varmlen

Open-source xray-core client with per-app and per-domain split tunneling. Built on Tauri 2 + SvelteKit.

> **Status:** early development.

## Goals

- Linux first, Android second, Windows last.
- Bundles [xray-core](https://github.com/XTLS/Xray-core) as the protocol engine (native TUN + routing). Compatible with any xray / v2ray (vless / vmess / trojan / shadowsocks) subscription.
- Split tunneling that is actually usable:
  - per-domain rules with wildcards (`*.ru`, `instagram.com`, …) → `direct` / `proxy`
  - per-process rules (`telegram-desktop`, `discord`, …) via xray's native `process` matcher on Linux/Windows
  - rule order is visible in the UI so you can see exactly how xray will resolve the next packet

## Roadmap

- [ ] M0 — Scaffold (Tauri 2 + Svelte 5)
- [ ] M1 — Parse `vless://` + base64 subscription URLs
- [ ] M2 — Bundle and spawn xray-core, connect/disconnect from the UI
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

### Wayland / WebKitGTK

The app already disables the WebKitGTK DMABUF renderer and falls back to XWayland
under Wayland (set at startup), so it should launch out of the box. If you still
hit a blank window, override the backend explicitly:

```bash
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 varmlen
```

## License

[MIT](./LICENSE).

Varmlen bundles [xray-core](https://github.com/XTLS/Xray-core) (Mozilla Public License 2.0) as its protocol engine; see [NOTICE](./NOTICE) for third-party licenses.

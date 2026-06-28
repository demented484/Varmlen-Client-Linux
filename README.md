# Varmlen Client Linux

Open-source xray-core VPN client for Linux, with per-app and per-domain split tunneling. Built on Tauri 2 and SvelteKit.

The Android client lives in a separate repo: [Varmlen-Client-Android](https://github.com/demented484/Varmlen-Client-Android). It shares the UI, the subscription parser and the xray config generator.

## Features

- Bundles xray-core as the protocol engine (native TUN and routing). Compatible with any xray or v2ray (vless, vmess, trojan, shadowsocks) subscription, a single share-link, several links, or a raw xray/v2ray JSON config.
- Split tunneling that is actually usable:
  - per-domain rules with wildcards (`*.ru`, `instagram.com`) routed to direct or proxy
  - per-process rules (`telegram-desktop`, `discord`) via xray's native `process` matcher
  - independent whitelist and blacklist modes for apps and for sites
- System tray, autostart, close-to-tray, and a kill switch that holds traffic if the tunnel drops.

## Install

Grab a release `.AppImage` (portable), `.deb` or `.rpm` from [Releases](https://github.com/demented484/Varmlen-Client-Linux/releases), or build from source.

## Build

```bash
npm install
npm run tauri build
```

This produces bundles in `src-tauri/target/release/bundle/` (appimage, deb, rpm). Use `npm run tauri dev` for a live-reload dev build.

Requires Rust 1.77+, Node 20+, and the system libraries documented at <https://tauri.app/start/prerequisites/>.

### Wayland and WebKitGTK

The app disables the WebKitGTK DMABUF renderer and falls back to XWayland under Wayland at startup, so it should launch out of the box. If you still hit a blank window, override the backend explicitly:

```bash
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 varmlen
```

## License

[MIT](./LICENSE). Varmlen bundles [xray-core](https://github.com/XTLS/Xray-core) (Mozilla Public License 2.0) as its protocol engine; see [NOTICE](./NOTICE) for third-party licenses.

# Windows port (work in progress - branch `windows-port`)

Brings the Varmlen xray VPN client to Windows. Status: **M1 code-complete**, the
Linux build still compiles clean, but the Windows build is **not yet verified**
(see "Building" - no Windows machine was available and CI is currently blocked).

## Architecture

Mirrors the Android data plane (not the Linux native-tun one), because xray's
native Windows tun is experimental and the Linux path depends on Linux-only
machinery (fwmark, `/proc` process routing, nft):

```
system apps -> Wintun adapter (10.7.0.1/24, default route) -> tun2socks.exe
            -> SOCKS5 127.0.0.1:2081 -> xray.exe (vless/reality) -> server
```

- `xray.exe` runs as a local SOCKS proxy (the existing `TunMode::Tun2socks`
  config path already emits exactly this and drops per-app routing).
- `tun2socks.exe` owns the Wintun adapter and bridges it to the SOCKS proxy.
- Routing, DNS and a kill switch are applied from the (admin-elevated) app with
  `netsh` / `route` - there is no separate helper. The app requests
  `requireAdministrator` via an embedded manifest (UAC prompt at launch).
- Anti-loop: a host route for each server IP via the physical gateway keeps
  xray's own dial to the server off the tun.

Code: `src-tauri/src/win_vpn.rs` (the whole data plane), `#[cfg(windows)]` arms in
`src-tauri/src/vpn.rs`, `build.rs` (manifest), `tauri.windows.conf.json`
(bundles `xray.exe` + `wintun.dll` + `tun2socks.exe`, NSIS perMachine).

## Building

Three bundled binaries are NOT in git - they are fetched at build time:
`xray.exe` + `wintun.dll` (from XTLS/Xray-core `Xray-windows-64.zip`) and
`tun2socks.exe` (from xjasonlyu/tun2socks). They go in `src-tauri/cores/`.

**Option A - GitHub Actions (recommended).** `.github/workflows/windows.yml`
builds + bundles the NSIS installer on a `windows-latest` runner and uploads it
as an artifact on every push to `windows-port`. It currently fails because the
GitHub account's Actions are **locked for a billing issue** - resolve that in
GitHub Settings -> Billing, then re-run the workflow and download
`Varmlen_*-setup.exe` from the run's artifacts.

**Option B - build inside the Windows VM** (`~/win-vm`, see its README):
1. Install Rust (rustup), Node 20, and the MSVC C++ build tools (Visual Studio
   Build Tools, "Desktop development with C++"). WebView2 ships with Windows 11.
2. Clone this repo, `git checkout windows-port`.
3. Fetch the three binaries into `src-tauri/cores/` (same URLs as the CI step).
4. `npm install && npm run tauri build`. Installer lands in
   `src-tauri/target/release/bundle/nsis/`.

## Verified vs not

- Verified: the Linux build still compiles (`cargo check`) after the cfg split,
  and the `&[&str]` command-arg coercions used in `win_vpn.rs` compile.
- NOT verified (no Windows / no CI yet): the Windows target compile of
  `win_vpn.rs`, and all runtime behaviour.

## VM-validate / v0.2 TODO (cannot be done without a real Windows + a live server)

- Per-site DIRECT exclusions loop on Windows for now (their dials default into
  the tun). v0.1 is effectively full-tunnel until `sockopt.interface` (bind
  xray's outbound to the physical adapter) is wired and validated. Until then,
  prefer general/full-tunnel use.
- Confirm `route print -4` gateway parsing, the `wintun` adapter name, DNS
  hijack, and the `netsh advfirewall` kill switch on a real adapter.
- Harden: swap `netsh` kill switch -> WFP, `route`/`netsh` -> IP Helper, and add
  Windows-Firewall per-process split for true per-app.

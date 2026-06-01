//! Client side of connect/disconnect — hybrid, no root daemon.
//!
//! - "tun" mode: full-system TUN. sing-box (setcap cap_net_admin) creates the
//!   TUN + routing + split + DNS and forwards everything to a local xray SOCKS;
//!   xray (plain user, no caps) does the vless/reality/XHTTP transport. The GUI
//!   owns both child processes directly. The killswitch is applied/removed by
//!   the setcap'd `aegis-probe` at connect/disconnect.
//! - "proxy" mode: just xray's SOCKS inbound on 127.0.0.1:XRAY_SOCKS_PORT — no
//!   TUN, no caps. Apps point at it.
//!
//! sing-box can't speak XHTTP, which is why xray is the upstream. There is no
//! unix socket / systemd service anymore.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::core::CoreKind;
use crate::singbox::{build_config, SplitInput};
use crate::subscription::VlessServer;
use crate::xray::build_xray_config;

/// Returned to the frontend; shape unchanged from the old socket protocol so
/// `api.ts` keeps working.
#[derive(Serialize, Deserialize)]
pub struct HelperResponse {
    pub ok: bool,
    pub state: String,
    pub pid: Option<u32>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
}

impl HelperResponse {
    fn connected(pid: u32) -> Self {
        HelperResponse { ok: true, state: "connected".into(), pid: Some(pid), error: None, rtt_ms: None }
    }
    fn disconnected() -> Self {
        HelperResponse { ok: true, state: "disconnected".into(), pid: None, error: None, rtt_ms: None }
    }
}

// --- child processes (owned by the GUI) ------------------------------------

fn xray_child() -> &'static Mutex<Option<Child>> {
    static C: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}
fn singbox_child() -> &'static Mutex<Option<Child>> {
    static C: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn pid_of(slot: &Mutex<Option<Child>>) -> Option<u32> {
    let mut g = slot.lock().unwrap();
    match g.as_mut() {
        Some(c) => match c.try_wait() {
            Ok(None) => Some(c.id()),
            _ => {
                *g = None;
                None
            }
        },
        None => None,
    }
}

/// SIGTERM a child so it can tear down cleanly (sing-box must remove its TUN
/// routes + auto_redirect nftables; a SIGKILL would leave the box unrouteable),
/// wait up to ~5s, then SIGKILL.
fn terminate_gracefully(child: &mut Child) {
    let pid = child.id() as i32;
    unsafe { libc::kill(pid, libc::SIGTERM); }
    for _ in 0..50 {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Stop both cores + killswitch + stray TUN. Order: killswitch down, sing-box
/// (graceful, tears down routes), xray, then a best-effort probe cleanup.
fn stop_all(app: &tauri::AppHandle) {
    if let Some(probe) = probe_bin(app) {
        let _ = Command::new(&probe).arg("killswitch-down").status();
    }
    if let Some(mut c) = singbox_child().lock().unwrap().take() {
        terminate_gracefully(&mut c);
    }
    if let Some(mut c) = xray_child().lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    if let Some(probe) = probe_bin(app) {
        let _ = Command::new(&probe).arg("cleanup").status();
    }
}

fn last_error_line(stderr: &str) -> String {
    let strip = |s: &str| s.replace(|c: char| c == '\u{1b}', "");
    stderr
        .lines()
        .map(|l| strip(l).trim().to_string())
        .filter(|l| !l.is_empty())
        .find(|l| l.contains("FATAL") || l.contains("ERROR") || l.contains("Failed"))
        .or_else(|| stderr.lines().map(|l| strip(l).trim().to_string()).filter(|l| !l.is_empty()).last())
        .unwrap_or_else(|| "no output".to_string())
}

// --- resource locations ----------------------------------------------------

/// Locate the bundled `aegis-probe`. Dev: helper build output. Packaged: the
/// copy placed in app-data/bin (resource → bin on first run; see grant_caps).
fn probe_bin(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    // Installed copy (what we setcap).
    if let Ok(data) = app.path().app_data_dir() {
        let p = data.join("bin").join("aegis-probe");
        if p.exists() {
            return Some(p);
        }
    }
    // Dev fallback: the freshly-built binary in the helper crate.
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../helper/target/release/aegis-probe"));
    if dev.exists() {
        return Some(dev);
    }
    None
}

/// The aegis-probe source binary to install/setcap (resource in prod, dev build
/// output otherwise).
fn probe_source(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("aegis-probe");
        if p.exists() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../helper/target/release/aegis-probe"));
    dev.exists().then_some(dev)
}

fn setcap_script(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("aegis-setcap.sh");
        if p.exists() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../helper/aegis-setcap.sh"));
    dev.exists().then_some(dev)
}

fn old_helper_uninstall(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("uninstall.sh");
        if p.exists() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../helper/uninstall.sh"));
    dev.exists().then_some(dev)
}

// --- DNS / killswitch helpers ----------------------------------------------

/// Resolve a server host to IPs for the killswitch allow-list. Servers are IP
/// literals now, so this is usually a no-op parse.
fn resolve_ips(host: &str) -> Vec<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    use std::str::FromStr;
    if host.is_empty() {
        return Vec::new();
    }
    if let Ok(ip) = std::net::IpAddr::from_str(host) {
        return vec![ip];
    }
    (host, 443u16).to_socket_addrs().map(|it| it.map(|s| s.ip()).collect()).unwrap_or_default()
}

// --- caps -------------------------------------------------------------------

/// Does `bin` carry the given capability (substring match on getcap output)?
fn has_cap(bin: &PathBuf, cap: &str) -> bool {
    Command::new("getcap")
        .arg(bin)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(cap))
        .unwrap_or(false)
}

/// Run the setcap batch via pkexec (one prompt): grant caps to the active
/// sing-box + aegis-probe, optionally removing the legacy root helper too.
/// Blocking (pkexec shows a password dialog) — call from spawn_blocking.
pub fn request_setcap_blocking(app: &tauri::AppHandle) -> Result<(), String> {
    let script = setcap_script(app).ok_or("setcap script not found")?;
    let singbox = crate::core::binary_path(app, CoreKind::SingBox)
        .map_err(|e| format!("sing-box core: {e}"))?;
    // Ensure aegis-probe is installed in app-data/bin so we can setcap a stable path.
    let probe = ensure_probe_installed(app)?;

    let mut cmd = Command::new("pkexec");
    cmd.arg(&script).arg(&singbox).arg(&probe);
    // If the legacy root helper is present, fold its removal into this prompt.
    if std::path::Path::new("/etc/systemd/system/aegisvpn-helper.service").exists() {
        if let Some(unins) = old_helper_uninstall(app) {
            cmd.arg(unins);
        }
    }
    let status = cmd.status().map_err(|e| format!("pkexec: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("granting permissions failed or was cancelled (exit {:?})", status.code()))
    }
}

/// Copy the bundled aegis-probe into app-data/bin (idempotent) so it has a
/// stable path to setcap (resources get replaced on app update, clearing caps).
fn ensure_probe_installed(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let data = app.path().app_data_dir().map_err(|e| format!("app data dir: {e}"))?;
    let bin_dir = data.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("create bin dir: {e}"))?;
    let dest = bin_dir.join("aegis-probe");
    let src = probe_source(app).ok_or("bundled aegis-probe not found")?;
    // Only copy if missing or different size (cheap freshness check).
    let need = !dest.exists()
        || std::fs::metadata(&dest).map(|m| m.len()).ok()
            != std::fs::metadata(&src).map(|m| m.len()).ok();
    if need {
        std::fs::copy(&src, &dest).map_err(|e| format!("copy probe: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
    }
    Ok(dest)
}

// --- connect / disconnect ---------------------------------------------------

fn spawn_core(bin: &PathBuf, cfg_path: &PathBuf) -> Result<Child, String> {
    let mut child = Command::new(bin)
        .arg("run")
        .arg("-c")
        .arg(cfg_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    // Catch an immediate crash (bad config / missing caps).
    std::thread::sleep(Duration::from_millis(700));
    if let Ok(Some(_)) = child.try_wait() {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            use std::io::Read;
            let _ = s.read_to_string(&mut err);
        }
        return Err(last_error_line(&err));
    }
    // Drain stderr to the journal so the pipe never fills.
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("core: {line}");
            }
        });
    }
    Ok(child)
}

fn runtime_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let d = base.join("aegisvpn");
    let _ = std::fs::create_dir_all(&d);
    d
}

#[tauri::command]
pub async fn vpn_connect(
    app: tauri::AppHandle,
    server: VlessServer,
    split: SplitInput,
    mode: String,
    killswitch: bool,
    allow_lan: bool,
) -> Result<HelperResponse, String> {
    let xray_cfg = serde_json::to_string(&build_xray_config(&server)).map_err(|e| e.to_string())?;
    let server_host = server.host.clone();

    tokio::task::spawn_blocking(move || -> Result<HelperResponse, String> {
        stop_all(&app);

        let xray_bin = crate::core::binary_path(&app, CoreKind::Xray)
            .map_err(|e| format!("xray core: {e} — install it in Settings → VPN core"))?;
        let rt = runtime_dir();

        // 1) xray first (the SOCKS upstream sing-box dials).
        let xray_path = rt.join("xray.json");
        std::fs::write(&xray_path, &xray_cfg).map_err(|e| format!("write xray cfg: {e}"))?;
        let xray = spawn_core(&xray_bin, &xray_path).map_err(|e| format!("xray: {e}"))?;
        *xray_child().lock().unwrap() = Some(xray);

        if mode == "proxy" {
            // Proxy mode is just xray's SOCKS inbound — no TUN, no sing-box.
            let pid = pid_of(xray_child()).unwrap_or(0);
            return Ok(HelperResponse::connected(pid));
        }

        // 2) sing-box TUN (needs caps). Preflight.
        let singbox_bin = crate::core::binary_path(&app, CoreKind::SingBox)
            .map_err(|e| format!("sing-box core: {e}"))?;
        if !has_cap(&singbox_bin, "cap_net_admin") {
            stop_all(&app);
            return Err(
                "sing-box lacks network permissions — click \"Grant network permissions\" in Settings"
                    .into(),
            );
        }
        let sb_cfg = serde_json::to_string(&build_config(&server, &split, &mode, allow_lan))
            .map_err(|e| e.to_string())?;
        let sb_path = rt.join("singbox.json");
        std::fs::write(&sb_path, &sb_cfg).map_err(|e| format!("write singbox cfg: {e}"))?;
        // Validate before launch.
        let check = Command::new(&singbox_bin).arg("check").arg("-c").arg(&sb_path).output();
        if let Ok(out) = check {
            if !out.status.success() {
                stop_all(&app);
                return Err(format!("sing-box config invalid: {}", last_error_line(&String::from_utf8_lossy(&out.stderr))));
            }
        }
        let sb = match spawn_core(&singbox_bin, &sb_path) {
            Ok(c) => c,
            Err(e) => {
                stop_all(&app);
                return Err(format!("sing-box: {e}"));
            }
        };
        let pid = sb.id();
        *singbox_child().lock().unwrap() = Some(sb);

        // 3) killswitch via the setcap'd probe.
        if killswitch {
            if let Some(probe) = probe_bin(&app) {
                let mut cmd = Command::new(&probe);
                cmd.arg("killswitch-up");
                if allow_lan {
                    cmd.arg("--allow-lan");
                }
                for ip in resolve_ips(&server_host) {
                    cmd.arg(ip.to_string());
                }
                let _ = cmd.status();
            }
        }

        Ok(HelperResponse::connected(pid))
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

#[tauri::command]
pub async fn vpn_disconnect(app: tauri::AppHandle) -> Result<HelperResponse, String> {
    let _ = tokio::task::spawn_blocking(move || stop_all(&app)).await;
    Ok(HelperResponse::disconnected())
}

#[tauri::command]
pub async fn vpn_status() -> Result<HelperResponse, String> {
    // sing-box alive → tun connected; else xray alone alive → proxy connected.
    if let Some(pid) = pid_of(singbox_child()) {
        return Ok(HelperResponse::connected(pid));
    }
    if let Some(pid) = pid_of(xray_child()) {
        return Ok(HelperResponse::connected(pid));
    }
    Ok(HelperResponse::disconnected())
}

/// Whether the cores have the capabilities they need (replaces the old
/// "helper installed" check).
#[tauri::command]
pub async fn caps_granted(app: tauri::AppHandle) -> bool {
    tokio::task::spawn_blocking(move || {
        let sb_ok = crate::core::binary_path(&app, CoreKind::SingBox)
            .map(|b| has_cap(&b, "cap_net_admin"))
            .unwrap_or(false);
        sb_ok
    })
    .await
    .unwrap_or(false)
}

/// Grant network permissions (setcap via pkexec). Replaces install_helper.
#[tauri::command]
pub async fn grant_caps(app: tauri::AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || request_setcap_blocking(&app))
        .await
        .map_err(|e| format!("join: {e}"))?
}

// --- location ping ----------------------------------------------------------

/// Local source-bound TCP connect — fallback when aegis-probe is unavailable.
/// Can't escape sing-box's auto_redirect while connected (no caps), so the RTT
/// is only accurate when disconnected.
fn tcp_ping_local(host: &str, port: u16, timeout: Duration) -> Result<u32, String> {
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use std::net::{SocketAddr, ToSocketAddrs};

    let dst: SocketAddr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| "no ipv4 addr".to_string())?;
    let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("socket: {e}"))?;
    let started = Instant::now();
    sock.connect_timeout(&SockAddr::from(dst), timeout).map_err(|e| format!("connect: {e}"))?;
    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// TCP-connect RTT — Happ-style latency probe. Uses the setcap'd aegis-probe
/// (SO_MARK + SO_BINDTODEVICE) so it bypasses the active tunnel; falls back to
/// a plain local connect if the probe is missing/uncapped.
#[tauri::command]
pub async fn tcp_ping_host(app: tauri::AppHandle, host: String, port: u16, timeout_ms: Option<u32>) -> Result<u32, String> {
    let ms = timeout_ms.unwrap_or(2500);
    tokio::task::spawn_blocking(move || {
        if let Some(probe) = probe_bin(&app) {
            let out = Command::new(&probe)
                .arg("tcp").arg(&host).arg(port.to_string()).arg(ms.to_string())
                .output();
            if let Ok(o) = out {
                if o.status.success() {
                    if let Ok(rtt) = String::from_utf8_lossy(&o.stdout).trim().parse::<u32>() {
                        return Ok(rtt);
                    }
                }
            }
        }
        tcp_ping_local(&host, port, Duration::from_millis(ms as u64))
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

//! Windows data plane. Mirrors the Android tun2socks model: xray.exe runs a
//! local SOCKS proxy (127.0.0.1:2081) and tun2socks.exe bridges a Wintun adapter
//! to it. Routing + DNS + a firewall kill switch are applied with `netsh`/`route`
//! from this (administrator-elevated) process - there is no separate helper.
//!
//! Anti-loop: a host route for each server IP via the physical gateway (metric 1)
//! keeps xray's own dial to the server off the tun. Per-site *direct* exclusions
//! would still loop (their dials default into the tun); v0.1 is therefore
//! effectively full-tunnel until a `sockopt.interface` physical bind is validated
//! on a real Windows VM. See the windows-port notes.
#![cfg(windows)]

use std::net::IpAddr;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::core::CoreKind;
use crate::vpn::{last_error_line, runtime_dir, validate_xray, write_private, HelperResponse};

const TUN_ADDR: &str = "10.7.0.1";
const TUN_MASK: &str = "255.255.255.0";
const ADAPTER: &str = "wintun"; // tun2socks' default Wintun adapter name
const SOCKS: &str = "127.0.0.1:2081";
const FW_GROUP: &str = "VarmlenKillswitch";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// --- shared state -----------------------------------------------------------

fn xray_child() -> &'static Mutex<Option<Child>> {
    static C: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}
fn t2s_child() -> &'static Mutex<Option<Child>> {
    static C: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}
/// "disconnected" | "connected" | "dropped".
fn conn_phase() -> &'static Mutex<String> {
    static P: OnceLock<Mutex<String>> = OnceLock::new();
    P.get_or_init(|| Mutex::new("disconnected".into()))
}
fn set_phase(p: &str) {
    *conn_phase().lock().unwrap() = p.to_string();
}
/// Destinations we added to the routing table, for teardown.
fn added_routes() -> &'static Mutex<Vec<String>> {
    static R: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}
static CONN_GEN: AtomicU64 = AtomicU64::new(0);
static INTENTIONAL_STOP: AtomicBool = AtomicBool::new(false);
/// Proxy mode: only xray runs (a local SOCKS), no tun2socks adapter.
static PROXY_MODE: AtomicBool = AtomicBool::new(false);

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

// --- small command helpers --------------------------------------------------

/// Run a console tool without flashing a window.
fn run(prog: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(prog).args(args).creation_flags(CREATE_NO_WINDOW).output()
}

fn netsh(args: &[&str]) {
    let _ = run("netsh", args);
}

fn log_path() -> PathBuf {
    runtime_dir().join("xray.log")
}

fn applog(line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{line}");
    }
}

// --- resource discovery -----------------------------------------------------

/// (tun2socks.exe path, directory containing wintun.dll). Both are bundled as
/// resources next to each other, so tun2socks loads wintun.dll from its cwd.
fn resources(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let res = app.path().resource_dir().map_err(|e| format!("resource dir: {e}"))?;
    let t2s = res.join("tun2socks.exe");
    if !t2s.exists() {
        return Err("tun2socks.exe not bundled".into());
    }
    if !res.join("wintun.dll").exists() {
        return Err("wintun.dll not bundled".into());
    }
    Ok((t2s, res))
}

// --- routing discovery ------------------------------------------------------

/// Physical default gateway IP, parsed from `route print -4`. The active-routes
/// row `0.0.0.0  0.0.0.0  <gateway>  <ifaceIP>  <metric>` carries it.
fn default_gateway() -> Option<String> {
    let out = run("route", &["print", "-4"]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 3 && cols[0] == "0.0.0.0" && cols[1] == "0.0.0.0" {
            let gw = cols[2];
            if gw != "On-link" && gw.parse::<IpAddr>().is_ok() {
                return Some(gw.to_string());
            }
        }
    }
    None
}

fn route_add(dest: &str, mask: &str, gw: &str, metric: u32) {
    let _ = run("route", &["add", dest, "mask", mask, gw, "metric", &metric.to_string()]);
}

fn route_delete(dest: &str) {
    let _ = run("route", &["delete", dest]);
}

/// Poll for the Wintun adapter to come up (tun2socks creates it on start).
fn wait_adapter(timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Ok(out) = run("netsh", &["interface", "show", "interface"]) {
            if String::from_utf8_lossy(&out.stdout).contains(ADAPTER) {
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

// --- child spawning ---------------------------------------------------------

/// Spawn a core, catch an immediate crash (700 ms), then drain its output to the
/// app log so the pipe never fills.
fn spawn(bin: &PathBuf, args: &[&str], cwd: &PathBuf, tag: &'static str) -> Result<Child, String> {
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    std::thread::sleep(Duration::from_millis(700));
    if let Ok(Some(_)) = child.try_wait() {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            use std::io::Read;
            let _ = s.read_to_string(&mut err);
        }
        return Err(last_error_line(&err));
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                applog(&format!("{tag}: {line}"));
            }
        });
    }
    Ok(child)
}

// --- kill switch (netsh advfirewall) ---------------------------------------

fn apply_killswitch(allow_lan: bool, server_ips: &[IpAddr]) -> Result<(), String> {
    let block = run(
        "netsh",
        &[
            "advfirewall", "firewall", "add", "rule",
            &format!("name={FW_GROUP}-block"),
            "dir=out", "action=block", &format!("group={FW_GROUP}"),
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false);
    if !block {
        return Err("kill switch: could not add firewall block rule".into());
    }
    let allow = |suffix: &str, remoteip: &str| {
        netsh(&[
            "advfirewall", "firewall", "add", "rule",
            &format!("name={FW_GROUP}-{suffix}"),
            "dir=out", "action=allow", &format!("group={FW_GROUP}"),
            &format!("remoteip={remoteip}"),
        ]);
    };
    allow("loop", "127.0.0.1");
    allow("tun", "10.7.0.0/24");
    for ip in server_ips {
        allow("srv", &ip.to_string());
    }
    if allow_lan {
        for cidr in ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"] {
            allow("lan", cidr);
        }
    }
    Ok(())
}

fn clear_killswitch() {
    netsh(&["advfirewall", "firewall", "delete", "rule", "name=all", &format!("group={FW_GROUP}")]);
}

// --- crash watcher ----------------------------------------------------------

fn spawn_watcher(app: AppHandle, killswitch: bool, generation: u64) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1000));
        if CONN_GEN.load(Ordering::SeqCst) != generation || INTENTIONAL_STOP.load(Ordering::SeqCst) {
            return;
        }
        let tun = !PROXY_MODE.load(Ordering::SeqCst);
        let alive = pid_of(xray_child()).is_some() && (!tun || pid_of(t2s_child()).is_some());
        if alive {
            continue;
        }
        if CONN_GEN.load(Ordering::SeqCst) != generation || INTENTIONAL_STOP.load(Ordering::SeqCst) {
            return;
        }
        applog("core exited unexpectedly");
        if killswitch && tun {
            set_phase("dropped");
            let _ = app.emit("vpn-dropped", true);
        } else {
            teardown(&app);
            set_phase("disconnected");
            let _ = app.emit("vpn-dropped", false);
        }
        return;
    });
}

// --- public API (called from vpn.rs windows arms) ---------------------------

#[allow(clippy::too_many_arguments)]
pub fn connect(
    app: &AppHandle,
    xray_cfg: String,
    validate_cfg: String,
    proxy_only: bool,
    killswitch: bool,
    allow_lan: bool,
    server_ips: Vec<IpAddr>,
) -> Result<u32, String> {
    INTENTIONAL_STOP.store(true, Ordering::SeqCst);
    teardown(app);
    set_phase("disconnected");
    PROXY_MODE.store(proxy_only, Ordering::SeqCst);

    let xray_bin = crate::core::binary_path(app, CoreKind::Xray)
        .map_err(|e| format!("xray core: {e} - install it in Settings"))?;

    let rt = runtime_dir();
    let vpath = rt.join("xray-validate.json");
    write_private(&vpath, &validate_cfg)?;
    validate_xray(&xray_bin, &vpath)?;

    // Anti-loop FIRST (tun mode only): pin each server IP to the physical gateway
    // before any default route points into the tun.
    let gw = if proxy_only {
        None
    } else {
        Some(default_gateway().ok_or("could not find the default gateway")?)
    };
    if !proxy_only {
        if server_ips.is_empty() {
            return Err("could not resolve the server address".into());
        }
        let mut routes = added_routes().lock().unwrap();
        for ip in &server_ips {
            route_add(&ip.to_string(), "255.255.255.255", gw.as_deref().unwrap(), 1);
            routes.push(ip.to_string());
        }
    }

    // xray (SOCKS inbound). In proxy mode this is the whole data plane.
    let xpath = rt.join("xray.json");
    write_private(&xpath, &xray_cfg)?;
    let xray = spawn(&xray_bin, &["run", "-c", &xpath.to_string_lossy()], &rt, "xray").map_err(|e| {
        teardown(app);
        format!("xray: {e}")
    })?;
    *xray_child().lock().unwrap() = Some(xray);
    let pid = pid_of(xray_child()).unwrap_or(0);

    if !proxy_only {
        // tun2socks: owns the Wintun adapter; cwd holds wintun.dll so it loads.
        let (t2s_bin, wintun_dir) = resources(app)?;
        let proxy = format!("socks5://{SOCKS}");
        let t2s = spawn(
            &t2s_bin,
            &["-device", ADAPTER, "-proxy", &proxy, "-loglevel", "warning"],
            &wintun_dir,
            "tun2socks",
        )
        .map_err(|e| {
            teardown(app);
            format!("tun2socks: {e}")
        })?;
        *t2s_child().lock().unwrap() = Some(t2s);

        if !wait_adapter(Duration::from_secs(6)) {
            teardown(app);
            return Err("the Wintun adapter did not come up".into());
        }

        netsh(&["interface", "ip", "set", "address", &format!("name={ADAPTER}"), "static", TUN_ADDR, TUN_MASK]);
        netsh(&["interface", "ip", "set", "dnsservers", &format!("name={ADAPTER}"), "static", TUN_ADDR]);

        let mut routes = added_routes().lock().unwrap();
        route_add("0.0.0.0", "128.0.0.0", TUN_ADDR, 1);
        routes.push("0.0.0.0".into());
        route_add("128.0.0.0", "128.0.0.0", TUN_ADDR, 1);
        routes.push("128.0.0.0".into());
        drop(routes);

        if killswitch {
            apply_killswitch(allow_lan, &server_ips).map_err(|e| {
                teardown(app);
                e
            })?;
        }
    }

    let generation = CONN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    INTENTIONAL_STOP.store(false, Ordering::SeqCst);
    set_phase("connected");
    spawn_watcher(app.clone(), killswitch, generation);
    applog("connected");
    Ok(pid)
}

/// Fail-open teardown: drop firewall rules, kill tun2socks (releases the
/// adapter), kill xray, delete the routes. Idempotent.
pub fn teardown(_app: &AppHandle) {
    clear_killswitch();
    if let Some(mut c) = t2s_child().lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    if let Some(mut c) = xray_child().lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    let mut routes = added_routes().lock().unwrap();
    for dest in routes.drain(..) {
        route_delete(&dest);
    }
}

pub fn disconnect(app: &AppHandle) {
    INTENTIONAL_STOP.store(true, Ordering::SeqCst);
    teardown(app);
    set_phase("disconnected");
}

pub fn teardown_on_exit(app: &AppHandle) {
    INTENTIONAL_STOP.store(true, Ordering::SeqCst);
    teardown(app);
}

pub fn status() -> HelperResponse {
    if conn_phase().lock().unwrap().as_str() == "dropped" {
        return HelperResponse::dropped();
    }
    let tun = !PROXY_MODE.load(Ordering::SeqCst);
    match pid_of(xray_child()) {
        Some(pid) if !tun || pid_of(t2s_child()).is_some() => HelperResponse::connected(pid),
        _ => HelperResponse::disconnected(),
    }
}

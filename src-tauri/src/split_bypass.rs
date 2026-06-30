//! Per-app split exclusion via cgroup v2 (Linux desktop).
//!
//! xray's native-tun `process` matcher reliably attributes normal TCP apps, but
//! misses UDP-heavy games and Proton/wine processes, so excluded games leak onto
//! the VPN. Instead of relying on that resolution, we put each excluded app's
//! processes into a user-owned cgroup; the helper tags that cgroup's traffic with
//! BYPASS_MARK (nft `socket cgroupv2`) → routed via the physical table +
//! masqueraded → bypasses the tun, for ANY protocol, native or Proton.
//!
//! Membership is maintained by a helper `bypass-watch` child: it subscribes to
//! the kernel proc-connector and moves a matching process into the cgroup the
//! instant it execs / renames itself (no polling). The cgroup lives under the
//! GUI's own systemd-delegated subtree, so creating it + moving the user's own
//! processes needs no privilege; only the nft tagging + the netlink connector
//! (helper, CAP_NET_ADMIN) do.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

const CG_ROOT: &str = "/sys/fs/cgroup";
const CG_NAME: &str = "varmlen-bypass";

/// The running `bypass-watch` helper child (killed on disconnect / superseded on
/// reconnect).
fn watcher() -> &'static Mutex<Option<Child>> {
    static W: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(None))
}

/// Absolute path of the bypass cgroup dir: a child of the GUI's own (delegated,
/// user-writable) cgroup parent. None if cgroup v2 isn't in use.
fn cgroup_dir() -> Option<PathBuf> {
    let content = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = content.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
    if rel.is_empty() || rel == "/" {
        return None;
    }
    let parent = Path::new(rel).parent()?;
    let mut p = PathBuf::from(CG_ROOT);
    p.push(parent.strip_prefix("/").unwrap_or(parent));
    p.push(CG_NAME);
    Some(p)
}

/// The cgroup path the helper feeds nft / the watcher: relative to the cgroup-v2
/// root, no leading slash.
fn cgroup_rel(dir: &Path) -> Option<String> {
    Some(dir.strip_prefix(CG_ROOT).ok()?.to_string_lossy().trim_start_matches('/').to_string())
}

fn kill_watcher() {
    if let Some(mut c) = watcher().lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Create the bypass cgroup (idempotent) and return its cgroup-v2-relative path,
/// to hand to the helper's `route-up --bypass-cgroup`. Doing the initial move
/// inside route-up — the same step that lays the routing — is what lets the
/// helper put already-running excluded apps onto the physical path BEFORE the
/// default route enters the tun, so their live connections are never captured by
/// the VPN. Returns None when nothing is excluded or the system isn't cgroup-v2.
pub fn prepare(excluded: &[String]) -> Option<String> {
    if excluded.is_empty() {
        return None;
    }
    let dir = cgroup_dir()?;
    let rel = cgroup_rel(&dir)?;
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    Some(rel)
}

/// Begin bypassing the given excluded apps. No-op when the list is empty or the
/// system isn't cgroup-v2. Supersedes any prior watcher.
pub fn setup(app: &AppHandle, excluded: Vec<String>) {
    if excluded.is_empty() {
        teardown(app);
        return;
    }
    let Some(dir) = cgroup_dir() else { return };
    let Some(rel) = cgroup_rel(&dir) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Some(probe) = crate::vpn::probe_bin(app) else { return };
    // The nft tag + the initial move of already-running excluded apps are done by
    // route-up (BEFORE it flips the default into the tun, with the correct egress
    // iface), so we don't re-apply bypass-up here. We only need the ongoing
    // watcher (below) to move apps the user launches AFTER connect.
    // Replace any prior watcher with one for the current exclusion list.
    kill_watcher();
    let child = std::process::Command::new(&probe)
        .arg("bypass-watch")
        .arg(&rel)
        .args(&excluded)
        .spawn();
    if let Ok(c) = child {
        *watcher().lock().unwrap() = Some(c);
    }
}

/// Stop the watcher and drop the helper's tagging. The cgroup itself is left in
/// place (its members harmlessly stay until they exit); a fresh `setup` reuses it.
pub fn teardown(app: &AppHandle) {
    kill_watcher();
    if let Some(probe) = crate::vpn::probe_bin(app) {
        let _ = std::process::Command::new(&probe).arg("bypass-down").status();
    }
}

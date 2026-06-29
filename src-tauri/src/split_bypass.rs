//! Per-app split exclusion via cgroup v2 (Linux desktop).
//!
//! xray's native-tun `process` matcher reliably attributes normal TCP apps, but
//! misses UDP-heavy games and Proton/wine processes, so excluded games leak onto
//! the VPN. Instead of relying on that resolution, we put each excluded app's
//! processes into a user-owned cgroup; the helper tags that cgroup's traffic with
//! BYPASS_MARK (nft `socket cgroupv2`) → routed via the physical table +
//! masqueraded → bypasses the tun, for ANY protocol, native or Proton.
//!
//! The cgroup lives under the GUI's own systemd-delegated subtree, so creating it
//! and moving the user's own processes into it needs no privilege; only the nft
//! tagging (helper, CAP_NET_ADMIN) does.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::AppHandle;

/// Bumped to supersede a running scanner (reconnect / disconnect).
static SCAN_GEN: AtomicU64 = AtomicU64::new(0);

const CG_ROOT: &str = "/sys/fs/cgroup";
const CG_NAME: &str = "varmlen-bypass";

/// Absolute path of the bypass cgroup dir: a child of the GUI's own
/// (delegated, user-writable) cgroup parent. None if cgroup v2 isn't in use.
fn cgroup_dir() -> Option<PathBuf> {
    // /proc/self/cgroup on v2: "0::/user.slice/…/app-….scope"
    let content = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = content.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
    if rel.is_empty() || rel == "/" {
        return None;
    }
    // Our own scope's parent is the delegated dir we can write under.
    let parent = Path::new(rel).parent()?;
    let mut p = PathBuf::from(CG_ROOT);
    // parent is like "/user.slice/…/app.slice" → strip the leading "/"
    p.push(parent.strip_prefix("/").unwrap_or(parent));
    p.push(CG_NAME);
    Some(p)
}

/// The cgroup path the helper feeds nft: relative to the cgroup-v2 root, no
/// leading slash (e.g. "user.slice/user-1000.slice/user@1000.service/app.slice/varmlen-bypass").
fn cgroup_rel(dir: &Path) -> Option<String> {
    Some(dir.strip_prefix(CG_ROOT).ok()?.to_string_lossy().trim_start_matches('/').to_string())
}

fn matches(id: &str, comm: &str, exe: &str, exe_base: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    // Trailing-slash value = a folder: match any process whose executable lives
    // under it (catches a whole Steam library, native + Proton, in one entry).
    if id.ends_with('/') {
        return exe.starts_with(id);
    }
    comm == id || exe_base == id || exe == id
}

/// Move every running process matching an excluded id into the bypass cgroup.
fn sweep(procs_file: &Path, excluded: &[String]) {
    let Ok(entries) = std::fs::read_dir("/proc") else { return };
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
        let comm = comm.trim();
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
        let exe_str = exe.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let exe_base = exe
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if excluded.iter().any(|id| matches(id, comm, &exe_str, &exe_base)) {
            // Best effort: a few kernel threads / transient pids reject the move.
            let _ = std::fs::write(procs_file, pid.to_string());
        }
    }
}

/// Begin bypassing the given excluded apps. No-op when the list is empty or the
/// system isn't cgroup-v2. Supersedes any prior scanner.
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
    // Helper tags this cgroup's traffic (CAP_NET_ADMIN); idempotent re-apply.
    if let Some(probe) = crate::vpn::probe_bin(app) {
        let _ = std::process::Command::new(&probe).arg("bypass-up").arg(&rel).status();
    }
    let generation = SCAN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let procs = dir.join("cgroup.procs");
    std::thread::spawn(move || {
        // Sweep on a short interval so games launched AFTER connect are caught.
        while SCAN_GEN.load(Ordering::SeqCst) == generation {
            sweep(&procs, &excluded);
            std::thread::sleep(Duration::from_millis(1500));
        }
    });
}

/// Stop the scanner and drop the helper's tagging. The cgroup itself is left in
/// place (its members harmlessly stay until they exit); a fresh `setup` reuses
/// it. Re-applying with no excluded apps therefore truly disables the bypass.
pub fn teardown(app: &AppHandle) {
    SCAN_GEN.fetch_add(1, Ordering::SeqCst);
    if let Some(probe) = crate::vpn::probe_bin(app) {
        let _ = std::process::Command::new(&probe).arg("bypass-down").status();
    }
}

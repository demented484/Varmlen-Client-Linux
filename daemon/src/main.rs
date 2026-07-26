use std::fs::{self, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::PathBuf;

use tokio::net::UnixListener;
use varmlend::server::{parse_owner_uid, serve_connection, PeerPolicy};

fn runtime_paths(uid: u32) -> (PathBuf, PathBuf) {
    let root = PathBuf::from("/run/varmlen");
    (
        root.join(format!("daemon-{uid}.sock")),
        root.join("daemon.lock"),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("varmlend must run as root".into());
    }
    let owner_uid = parse_owner_uid(std::env::var("PKEXEC_UID").ok().as_deref())?;
    let (socket_path, lock_path) = runtime_paths(owner_uid);

    fs::create_dir_all("/run/varmlen")?;
    fs::set_permissions("/run/varmlen", fs::Permissions::from_mode(0o711))?;

    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("another Varmlen daemon already owns the network".into());
    }

    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    chown(&socket_path, Some(owner_uid), Some(owner_uid))?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;

    let policy = PeerPolicy::new(owner_uid);
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let _ = serve_connection(stream, policy).await;
        });
    }
}

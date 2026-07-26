use std::fs::{self, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout, Duration};

use crate::dns::{DnsBackend, DnsGuard, SystemDnsBackend};
use crate::lifecycle::LifecycleBackend;
use crate::protocol::{ConnectRequest, ConnectionMode, DaemonError, DaemonErrorCode};
use crate::recovery::ProcessIdentity;
use crate::split::system::SystemSplitBackend;
use crate::split::{SplitBackend, SplitManager, SplitPlan};

pub const INSTALLED_XRAY: &str = "/usr/libexec/varmlen/xray";
pub const INSTALLED_NET_HELPER: &str = "/usr/libexec/varmlen/varmlen-net";
const TUN_INTERFACE: &str = "varmlen0";
const PROXY_PORT: u16 = 2081;

pub struct SystemLifecycleBackend {
    owner_uid: u32,
    xray_path: PathBuf,
    net_helper_path: PathBuf,
    runtime_dir: PathBuf,
    xray: Option<Child>,
    xray_identity: Option<ProcessIdentity>,
    mode: Option<ConnectionMode>,
    dns_active: bool,
    split: Option<SplitManager<SystemSplitBackend>>,
}

impl SystemLifecycleBackend {
    pub fn installed(owner_uid: u32) -> Self {
        Self::new(
            owner_uid,
            PathBuf::from(INSTALLED_XRAY),
            PathBuf::from(INSTALLED_NET_HELPER),
            PathBuf::from(format!("/run/varmlen/user-{owner_uid}")),
        )
    }

    pub fn new(
        owner_uid: u32,
        xray_path: PathBuf,
        net_helper_path: PathBuf,
        runtime_dir: PathBuf,
    ) -> Self {
        Self {
            owner_uid,
            xray_path,
            net_helper_path,
            runtime_dir,
            xray: None,
            xray_identity: None,
            mode: None,
            dns_active: false,
            split: None,
        }
    }

    pub fn xray_identity(&self) -> Option<ProcessIdentity> {
        self.xray_identity.clone()
    }

    pub fn child_is_running(&mut self) -> bool {
        self.xray
            .as_mut()
            .is_some_and(|child| child.try_wait().is_ok_and(|status| status.is_none()))
    }

    fn config_path(&self) -> PathBuf {
        self.runtime_dir.join("xray.json")
    }

    fn validation_path(&self) -> PathBuf {
        self.runtime_dir.join("xray-validation.json")
    }

    fn log_path(&self) -> PathBuf {
        PathBuf::from(format!("/run/varmlen/xray-{}.log", self.owner_uid))
    }

    fn prepare_runtime_dir(&self) -> Result<(), DaemonError> {
        fs::create_dir_all(&self.runtime_dir).map_err(internal_io("create runtime directory"))?;
        fs::set_permissions(&self.runtime_dir, fs::Permissions::from_mode(0o700))
            .map_err(internal_io("secure runtime directory"))?;
        Ok(())
    }

    fn write_private(&self, path: &Path, content: &str) -> Result<(), DaemonError> {
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&temporary)?;
            std::io::Write::write_all(&mut file, content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            Ok::<(), std::io::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(internal_io("write private Xray configuration"))
    }

    async fn run_xray_validation(&self) -> Result<(), DaemonError> {
        let output = Command::new(&self.xray_path)
            .args(["run", "-test", "-c"])
            .arg(self.validation_path())
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .output()
            .await
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::XrayValidationFailed,
                    format!("could not start Xray validation: {error}"),
                )
            })?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(DaemonError::new(
            DaemonErrorCode::XrayValidationFailed,
            format!(
                "Xray rejected the generated configuration: {}",
                stderr.trim()
            ),
        ))
    }

    async fn run_net_helper(&self, arguments: &[String]) -> Result<(), DaemonError> {
        let output = Command::new(&self.net_helper_path)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .output()
            .await
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::TunnelCommitFailed,
                    format!("could not start installed network helper: {error}"),
                )
            })?;
        if output.status.success() {
            return Ok(());
        }
        Err(DaemonError::new(
            DaemonErrorCode::TunnelCommitFailed,
            format!(
                "network setup failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }

    async fn route_up(&self, request: &ConnectRequest) -> Result<(), DaemonError> {
        let mut arguments = vec!["route-up".to_string()];
        for address in &request.server_ips {
            arguments.push("--server".into());
            arguments.push(address.to_string());
        }
        self.run_net_helper(&arguments).await
    }

    async fn route_down(&self) -> Result<(), DaemonError> {
        self.run_net_helper(&["route-down".into()]).await
    }

    async fn terminate_xray(&mut self) -> Result<(), DaemonError> {
        let Some(mut child) = self.xray.take() else {
            self.xray_identity = None;
            return Ok(());
        };
        if let Some(pid) = child.id() {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
        if timeout(Duration::from_secs(3), child.wait()).await.is_err() {
            child.kill().await.map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::TunnelCleanupFailed,
                    format!("could not terminate Xray: {error}"),
                )
            })?;
            let _ = child.wait().await;
        }
        self.xray_identity = None;
        Ok(())
    }

    async fn wait_until_ready(&mut self, mode: ConnectionMode) -> Result<(), DaemonError> {
        for _ in 0..50 {
            if !self.child_is_running() {
                return Err(DaemonError::new(
                    DaemonErrorCode::XrayStartFailed,
                    "Xray exited before the data plane became ready",
                ));
            }
            let ready = match mode {
                ConnectionMode::Tun => Path::new("/sys/class/net/varmlen0").exists(),
                ConnectionMode::Proxy => tokio::net::TcpStream::connect(("127.0.0.1", PROXY_PORT))
                    .await
                    .is_ok(),
            };
            if ready {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        Err(DaemonError::new(
            DaemonErrorCode::XrayStartFailed,
            "timed out waiting for Xray",
        ))
    }

    async fn remove_dns(&mut self) -> Result<(), DaemonError> {
        if !self.dns_active {
            return Ok(());
        }
        let mut backend = SystemDnsBackend::new();
        backend.remove().await?;
        self.dns_active = false;
        Ok(())
    }

    async fn remove_split(&mut self) -> Result<(), DaemonError> {
        let Some(mut split) = self.split.take() else {
            return Ok(());
        };
        split.backend_mut().rollback().await.map_err(|error| {
            DaemonError::new(
                DaemonErrorCode::TunnelCleanupFailed,
                format!("split-tunnel cleanup failed: {error}"),
            )
        })
    }
}

#[async_trait]
impl LifecycleBackend for SystemLifecycleBackend {
    fn data_plane_alive(&mut self) -> bool {
        self.child_is_running()
    }

    async fn validate(&mut self, request: &ConnectRequest) -> Result<(), DaemonError> {
        ensure_trusted_binary(&self.xray_path)?;
        ensure_trusted_binary(&self.net_helper_path)?;
        validate_xray_document(&request.xray_config, request.mode == ConnectionMode::Tun)?;
        validate_xray_document(&request.validation_config, false)?;
        self.prepare_runtime_dir()?;
        self.write_private(&self.validation_path(), &request.validation_config)?;
        self.run_xray_validation().await
    }

    async fn install_hold_block(&mut self, request: &ConnectRequest) -> Result<(), DaemonError> {
        let mut arguments = vec!["killswitch-up".to_string()];
        if request.allow_lan {
            arguments.push("--allow-lan".into());
        }
        arguments.extend(request.server_ips.iter().map(ToString::to_string));
        self.run_net_helper(&arguments)
            .await
            .map_err(|error| DaemonError::new(DaemonErrorCode::HoldBlockFailed, error.message))
    }

    async fn verify_hold_block(&mut self) -> Result<(), DaemonError> {
        let output = Command::new("nft")
            .args(["list", "table", "inet", "varmlen_ks"])
            .output()
            .await
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::HoldBlockVerificationFailed,
                    format!("could not inspect hold block: {error}"),
                )
            })?;
        let rules = String::from_utf8_lossy(&output.stdout);
        let has_dial_mark = rules.contains("0x2024") || rules.contains("0x00002024");
        let has_split_mark = rules.contains("0x2025") || rules.contains("0x00002025");
        if output.status.success()
            && rules.contains("policy drop")
            && has_dial_mark
            && has_split_mark
        {
            Ok(())
        } else {
            Err(DaemonError::new(
                DaemonErrorCode::HoldBlockVerificationFailed,
                "hold block was not installed with the required drop policy",
            ))
        }
    }

    async fn stop_data_plane(&mut self, preserve_split: bool) -> Result<(), DaemonError> {
        let mut first_error = None;
        if !preserve_split {
            if let Err(error) = self.remove_split().await {
                first_error = Some(error);
            }
        }
        if let Err(error) = self.remove_dns().await {
            first_error.get_or_insert(error);
        }
        if self.mode == Some(ConnectionMode::Tun) && self.net_helper_path.exists() {
            if let Err(error) = self.route_down().await {
                first_error.get_or_insert(error);
            }
        }
        if let Err(error) = self.terminate_xray().await {
            first_error.get_or_insert(error);
        }
        self.mode = None;
        let _ = fs::remove_file(self.config_path());
        let _ = fs::remove_file(self.validation_path());
        first_error.map_or(Ok(()), Err)
    }

    async fn start_data_plane(&mut self, request: &ConnectRequest) -> Result<(), DaemonError> {
        self.prepare_runtime_dir()?;
        self.write_private(&self.config_path(), &request.xray_config)?;
        let log = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.log_path())
            .map_err(internal_io("open Xray log"))?;
        let stderr = log.try_clone().map_err(internal_io("clone Xray log"))?;
        std::os::unix::fs::chown(self.log_path(), Some(self.owner_uid), Some(self.owner_uid))
            .map_err(internal_io("assign Xray log ownership"))?;
        let child = Command::new(&self.xray_path)
            .args(["run", "-c"])
            .arg(self.config_path())
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::XrayStartFailed,
                    format!("could not start installed Xray: {error}"),
                )
            })?;
        let pid = child.id().ok_or_else(|| {
            DaemonError::new(
                DaemonErrorCode::XrayStartFailed,
                "Xray did not expose a process ID",
            )
        })?;
        self.xray = Some(child);
        self.xray_identity = ProcessIdentity::from_pid(pid).ok();
        self.mode = Some(request.mode);
        self.wait_until_ready(request.mode).await
    }

    async fn activate_network(&mut self, request: &ConnectRequest) -> Result<(), DaemonError> {
        if request.mode == ConnectionMode::Proxy {
            return Ok(());
        }
        self.route_up(request).await?;

        if self.split.is_some() {
            self.remove_split().await?;
        }
        if !request.excluded_apps.is_empty() {
            let mut split = SplitManager::new(SystemSplitBackend::new(self.owner_uid));
            split
                .apply(SplitPlan::new(
                    self.owner_uid,
                    request.excluded_apps.clone(),
                ))
                .await
                .map_err(|error| {
                    DaemonError::new(
                        DaemonErrorCode::SplitUnavailable,
                        format!("per-app split tunnelling is unavailable: {error}"),
                    )
                })?;
            self.split = Some(split);
        }

        let mut dns = DnsGuard::new(SystemDnsBackend::new());
        // Arm lifecycle cleanup before installation so a failed verification
        // still gets a second, fail-closed attempt to remove the nft table.
        self.dns_active = true;
        dns.install().await?;
        Ok(())
    }

    async fn verify_connection(&mut self, request: &ConnectRequest) -> Result<(), DaemonError> {
        if !self.child_is_running() {
            return Err(DaemonError::new(
                DaemonErrorCode::XrayStartFailed,
                "Xray is not running",
            ));
        }
        if request.mode == ConnectionMode::Proxy {
            return Ok(());
        }
        if !Path::new("/sys/class/net/varmlen0").exists() || !self.dns_active {
            return Err(DaemonError::new(
                DaemonErrorCode::TunnelCommitFailed,
                "TUN or DNS protection disappeared during activation",
            ));
        }
        if let Some(split) = self.split.as_ref() {
            if !split.backend().is_healthy() {
                return Err(DaemonError::new(
                    DaemonErrorCode::SplitUnavailable,
                    "per-app split watcher stopped",
                ));
            }
        }
        let route = Command::new("ip")
            .args(["route", "get", "1.0.0.1"])
            .output()
            .await
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::TunnelCommitFailed,
                    format!("could not verify tunnel route: {error}"),
                )
            })?;
        if route.status.success() && String::from_utf8_lossy(&route.stdout).contains(TUN_INTERFACE)
        {
            Ok(())
        } else {
            Err(DaemonError::new(
                DaemonErrorCode::TunnelCommitFailed,
                "the default IPv4 path does not use varmlen0",
            ))
        }
    }

    async fn remove_hold_block(&mut self) -> Result<(), DaemonError> {
        self.run_net_helper(&["killswitch-down".into()])
            .await
            .map_err(|error| {
                DaemonError::new(DaemonErrorCode::HoldBlockRemovalFailed, error.message)
            })
    }
}

fn internal_io(context: &'static str) -> impl FnOnce(std::io::Error) -> DaemonError {
    move |error| DaemonError::new(DaemonErrorCode::Internal, format!("{context}: {error}"))
}

pub fn ensure_trusted_binary(path: &Path) -> Result<(), DaemonError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        DaemonError::new(
            DaemonErrorCode::XrayUnavailable,
            format!(
                "installed component {} is unavailable: {error}",
                path.display()
            ),
        )
    })?;
    if !metadata.file_type().is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o111 == 0
    {
        return Err(DaemonError::new(
            DaemonErrorCode::XrayUnavailable,
            format!(
                "installed component {} is not a trusted root-owned executable",
                path.display()
            ),
        ));
    }
    Ok(())
}

pub fn validate_xray_document(raw: &str, expects_tun: bool) -> Result<(), DaemonError> {
    let document: Value = serde_json::from_str(raw).map_err(|error| {
        DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            format!("invalid generated Xray JSON: {error}"),
        )
    })?;
    let object = document.as_object().ok_or_else(|| {
        DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray configuration must be an object",
        )
    })?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "log" | "dns" | "inbounds" | "outbounds" | "routing"
        )
    }) {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray configuration contains an unsupported privileged section",
        ));
    }
    let log = object
        .get("log")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DaemonError::new(
                DaemonErrorCode::InvalidRequest,
                "Xray log section is missing",
            )
        })?;
    if log.keys().any(|key| key != "loglevel") {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray file logging is not permitted",
        ));
    }

    let inbounds = object
        .get("inbounds")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DaemonError::new(DaemonErrorCode::InvalidRequest, "Xray inbounds are missing")
        })?;
    if inbounds.len() != 1 {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray must contain exactly one data inbound",
        ));
    }
    let data_tag = if expects_tun { "tun-in" } else { "socks-in" };
    let data_protocol = if expects_tun { "tun" } else { "socks" };
    let data = inbound_by_tag(inbounds, data_tag, data_protocol)?;
    if expects_tun {
        if data.pointer("/settings/name").and_then(Value::as_str) != Some(TUN_INTERFACE) {
            return Err(DaemonError::new(
                DaemonErrorCode::InvalidRequest,
                "Xray TUN interface name is not permitted",
            ));
        }
    } else if data.get("listen").and_then(Value::as_str) != Some("127.0.0.1")
        || data.get("port").and_then(Value::as_u64) != Some(PROXY_PORT.into())
    {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray SOCKS inbound must remain loopback-only",
        ));
    }
    let outbounds = object
        .get("outbounds")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DaemonError::new(
                DaemonErrorCode::InvalidRequest,
                "Xray outbounds are missing",
            )
        })?;
    if outbounds.len() != 4
        || outbounds.iter().any(|outbound| {
            !matches!(
                outbound.get("protocol").and_then(Value::as_str),
                Some(
                    "vless" | "vmess" | "trojan" | "shadowsocks" | "freedom" | "dns" | "blackhole"
                )
            )
        })
    {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray outbound set is not permitted",
        ));
    }
    if object.get("dns").and_then(Value::as_object).is_none()
        || object.get("routing").and_then(Value::as_object).is_none()
    {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray DNS or routing section is missing",
        ));
    }
    if contains_forbidden_file_key(&document) {
        return Err(DaemonError::new(
            DaemonErrorCode::InvalidRequest,
            "Xray configuration may not reference privileged files",
        ));
    }
    Ok(())
}

fn inbound_by_tag<'a>(
    inbounds: &'a [Value],
    tag: &str,
    protocol: &str,
) -> Result<&'a Value, DaemonError> {
    inbounds
        .iter()
        .find(|inbound| {
            inbound.get("tag").and_then(Value::as_str) == Some(tag)
                && inbound.get("protocol").and_then(Value::as_str) == Some(protocol)
        })
        .ok_or_else(|| {
            DaemonError::new(
                DaemonErrorCode::InvalidRequest,
                format!("required Xray inbound {tag} is missing"),
            )
        })
}

fn contains_forbidden_file_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "certificateFile" | "keyFile" | "certificatePath" | "keyPath"
            ) || contains_forbidden_file_key(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_file_key),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_xray_document;
    use crate::protocol::DaemonErrorCode;

    fn config(data: serde_json::Value) -> String {
        json!({
            "log": {"loglevel": "warning"},
            "dns": {"servers": ["https://1.1.1.1/dns-query"]},
            "inbounds": [data],
            "outbounds": [
                {"tag": "proxy", "protocol": "vless"},
                {"tag": "direct", "protocol": "freedom"},
                {"tag": "dns-out", "protocol": "dns"},
                {"tag": "block", "protocol": "blackhole"}
            ],
            "routing": {"rules": []}
        })
        .to_string()
    }

    #[test]
    fn accepts_fixed_tun_with_no_local_dns_inbound() {
        let raw = config(json!({
            "tag": "tun-in",
            "protocol": "tun",
            "settings": {"name": "varmlen0", "mtu": 1500}
        }));
        assert!(validate_xray_document(&raw, true).is_ok());
    }

    #[test]
    fn rejects_privileged_file_logging_and_non_loopback_listener() {
        let mut value: serde_json::Value = serde_json::from_str(&config(json!({
            "tag": "socks-in",
            "listen": "0.0.0.0",
            "port": 2081,
            "protocol": "socks",
            "settings": {"udp": true}
        })))
        .unwrap();
        value["log"]["access"] = json!("/etc/shadow");
        let error = validate_xray_document(&value.to_string(), false).unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::InvalidRequest);
    }

    #[test]
    fn rejects_certificate_paths_from_authenticated_but_untrusted_gui() {
        let mut value: serde_json::Value = serde_json::from_str(&config(json!({
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "port": 2081,
            "protocol": "socks",
            "settings": {"udp": true}
        })))
        .unwrap();
        value["outbounds"][0]["streamSettings"] =
            json!({"tlsSettings": {"certificateFile": "/root/private.pem"}});
        let error = validate_xray_document(&value.to_string(), false).unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::InvalidRequest);
    }
}

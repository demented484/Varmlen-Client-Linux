//! Platform VPN lifecycle adapter.
//!
//! Android delegates to its `VpnService`. Linux only generates a bounded
//! connection request and sends it to the authenticated root-owned daemon.
//! The GUI never launches Xray, edits routes, or owns privileged processes.

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::split::SplitInput;
use crate::subscription::VlessServer;
use crate::xray::{build_xray_config, TunMode};

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
    #[cfg(target_os = "android")]
    fn connected(pid: u32) -> Self {
        Self {
            ok: true,
            state: "connected".into(),
            pid: Some(pid),
            error: None,
            rtt_ms: None,
        }
    }

    fn disconnected() -> Self {
        Self {
            ok: true,
            state: "disconnected".into(),
            pid: None,
            error: None,
            rtt_ms: None,
        }
    }

    fn dropped() -> Self {
        Self {
            ok: true,
            state: "dropped".into(),
            pid: None,
            error: None,
            rtt_ms: None,
        }
    }
}

fn vpn_op_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(target_os = "linux")]
fn response_from_daemon(state: varmlend::protocol::DaemonState) -> HelperResponse {
    use varmlend::protocol::ConnectionPhase;

    match state.phase {
        ConnectionPhase::Connected => HelperResponse {
            ok: true,
            state: "connected".into(),
            pid: None,
            error: None,
            rtt_ms: None,
        },
        ConnectionPhase::Blocking | ConnectionPhase::RecoveryRequired => HelperResponse::dropped(),
        _ => HelperResponse::disconnected(),
    }
}

#[cfg(target_os = "linux")]
async fn resolve_server_ips(server: &VlessServer) -> Result<Vec<std::net::IpAddr>, String> {
    let addresses = tokio::net::lookup_host((server.host.as_str(), server.port))
        .await
        .map_err(|error| format!("could not resolve VPN server: {error}"))?;
    let unique: BTreeSet<_> = addresses.map(|address| address.ip()).collect();
    if unique.is_empty() {
        return Err(format!(
            "VPN server {} did not resolve to an address",
            server.host
        ));
    }
    Ok(unique.into_iter().take(16).collect())
}

#[tauri::command]
pub async fn vpn_connect(
    app: tauri::AppHandle,
    server: VlessServer,
    split: SplitInput,
    mode: String,
    killswitch: bool,
    allow_lan: bool,
    log_level: Option<String>,
) -> Result<HelperResponse, String> {
    let level = log_level.unwrap_or_else(|| "warn".to_string());

    #[cfg(target_os = "android")]
    {
        let _ = killswitch;
        let xray_config = serde_json::to_string(&build_xray_config(
            &server,
            &split,
            &mode,
            TunMode::Tun2socks,
            allow_lan,
            &level,
        ))
        .map_err(|error| error.to_string())?;
        let applications_are_allowlist = split.apps_selective();
        crate::mobile_vpn::connect(
            &app,
            xray_config,
            crate::xray::XRAY_SOCKS_PORT,
            split.apps.clone(),
            applications_are_allowlist,
            level,
        )?;
        return Ok(HelperResponse::connected(0));
    }

    #[cfg(target_os = "linux")]
    {
        use varmlend::protocol::{ConnectRequest, ConnectionMode, DaemonCommand};

        let _operation = vpn_op_lock().lock().await;
        let connection_mode = match mode.as_str() {
            "tun" => ConnectionMode::Tun,
            "proxy" => ConnectionMode::Proxy,
            _ => return Err(format!("unsupported VPN mode: {mode}")),
        };
        let xray_config = serde_json::to_string(&build_xray_config(
            &server,
            &split,
            &mode,
            TunMode::XrayNative,
            allow_lan,
            &level,
        ))
        .map_err(|error| error.to_string())?;
        let validation_config = serde_json::to_string(&build_xray_config(
            &server,
            &split,
            &mode,
            TunMode::Tun2socks,
            allow_lan,
            &level,
        ))
        .map_err(|error| error.to_string())?;
        let excluded_apps = if connection_mode == ConnectionMode::Tun && !split.apps_selective() {
            split.enabled_apps()
        } else {
            Vec::new()
        };
        let request = ConnectRequest {
            mode: connection_mode,
            xray_config,
            validation_config,
            server_ips: resolve_server_ips(&server).await?,
            excluded_apps,
            killswitch,
            allow_lan,
        };
        let mut daemon = crate::daemon_client::DaemonClient::connect_or_start_installed()
            .await
            .map_err(|error| error.to_string())?;
        let state = daemon
            .request(DaemonCommand::Connect(request))
            .await
            .map_err(|error| error.to_string())?;
        let _ = app;
        Ok(response_from_daemon(state))
    }

    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = (app, server, split, mode, killswitch, allow_lan, level);
        Err("VPN lifecycle is not implemented on this platform".into())
    }
}

#[tauri::command]
pub async fn vpn_disconnect(app: tauri::AppHandle) -> Result<HelperResponse, String> {
    #[cfg(target_os = "android")]
    {
        crate::mobile_vpn::disconnect(&app)?;
        return Ok(HelperResponse::disconnected());
    }
    #[cfg(target_os = "linux")]
    {
        use varmlend::protocol::DaemonCommand;

        let _operation = vpn_op_lock().lock().await;
        let Ok(mut daemon) = crate::daemon_client::DaemonClient::connect_installed().await else {
            return Ok(HelperResponse::disconnected());
        };
        let state = daemon
            .request(DaemonCommand::Disconnect)
            .await
            .map_err(|error| error.to_string())?;
        let _ = app;
        Ok(response_from_daemon(state))
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = app;
        Ok(HelperResponse::disconnected())
    }
}

#[tauri::command]
pub async fn vpn_status(app: tauri::AppHandle) -> Result<HelperResponse, String> {
    #[cfg(target_os = "android")]
    {
        return Ok(if crate::mobile_vpn::is_running(&app) {
            HelperResponse::connected(0)
        } else {
            HelperResponse::disconnected()
        });
    }
    #[cfg(target_os = "linux")]
    {
        use varmlend::protocol::DaemonCommand;

        let Ok(mut daemon) = crate::daemon_client::DaemonClient::connect_installed().await else {
            return Ok(HelperResponse::disconnected());
        };
        let state = daemon
            .request(DaemonCommand::Status)
            .await
            .map_err(|error| error.to_string())?;
        let _ = app;
        Ok(response_from_daemon(state))
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = app;
        Ok(HelperResponse::disconnected())
    }
}

#[tauri::command]
pub async fn vpn_log(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::read_log(&app);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let path = format!("/run/varmlen/xray-{}.log", unsafe { libc::getuid() });
        Ok(std::fs::read_to_string(path).unwrap_or_default())
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = app;
        Ok(String::new())
    }
}

#[tauri::command]
pub async fn clear_vpn_log(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::clear_log(&app);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let path = format!("/run/varmlen/xray-{}.log", unsafe { libc::getuid() });
        std::fs::write(path, "").map_err(|error| error.to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = app;
        Ok(())
    }
}

#[tauri::command]
pub async fn read_clipboard(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::read_clipboard(&app);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("use navigator.clipboard on desktop".into())
    }
}

#[tauri::command]
pub async fn set_status_bar(app: tauri::AppHandle, light: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::set_bar_style(&app, light);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, light);
        Ok(())
    }
}

#[tauri::command]
pub async fn notifications_enabled(app: tauri::AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::notifications_enabled(&app).unwrap_or(false);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        true
    }
}

#[tauri::command]
pub async fn open_notification_settings(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return crate::mobile_vpn::open_notification_settings(&app);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(())
    }
}

#[tauri::command]
pub async fn caps_granted(app: tauri::AppHandle) -> bool {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        varmlend::system::ensure_trusted_binary(std::path::Path::new(
            varmlend::system::INSTALLED_XRAY,
        ))
        .is_ok()
            && varmlend::system::ensure_trusted_binary(std::path::Path::new(
                varmlend::system::INSTALLED_NET_HELPER,
            ))
            .is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        true
    }
}

#[tauri::command]
pub async fn grant_caps(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        crate::daemon_client::DaemonClient::connect_or_start_installed()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Ok(())
    }
}

fn tcp_ping_local(host: &str, port: u16, timeout: Duration) -> Result<u32, String> {
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use std::net::{SocketAddr, ToSocketAddrs};

    let destination: SocketAddr = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve: {error}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| "no IPv4 address".to_string())?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|error| format!("socket: {error}"))?;
    let started = Instant::now();
    socket
        .connect_timeout(&SockAddr::from(destination), timeout)
        .map_err(|error| format!("connect: {error}"))?;
    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

#[tauri::command]
pub async fn tcp_ping_host(
    app: tauri::AppHandle,
    host: String,
    port: u16,
    timeout_ms: Option<u32>,
) -> Result<u32, String> {
    let _ = app;
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(2500).into());
    tokio::task::spawn_blocking(move || tcp_ping_local(&host, port, timeout))
        .await
        .map_err(|error| format!("ping task: {error}"))?
}

#[tauri::command]
pub async fn proxy_get_ping(
    app: tauri::AppHandle,
    server: VlessServer,
    timeout_ms: Option<u32>,
) -> Result<u32, String> {
    tcp_ping_host(app, server.host, server.port, timeout_ms).await
}

/// The daemon intentionally outlives the GUI, so closing or restarting the
/// interface never tears down an otherwise healthy 24/7 tunnel.
pub(crate) fn teardown_on_exit(_app: &tauri::AppHandle) {}

#[cfg(test)]
mod tests {
    #[test]
    fn linux_lifecycle_source_has_no_local_privileged_launcher() {
        let source = include_str!("vpn.rs");
        assert!(!source.contains(concat!("set", "cap")));
        assert!(!source.contains(concat!("Command", "::new")));
        assert!(!source.contains(concat!("varmlen", "-probe")));
    }
}

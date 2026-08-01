//! Platform VPN lifecycle adapter.
//!
//! Android delegates to its `VpnService`. Linux only generates a bounded
//! connection request and sends it to the authenticated root-owned daemon.
//! The GUI never launches Xray, edits routes, or owns privileged processes.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::split::SplitInput;
use crate::subscription::{server_endpoints, VlessServer};
use crate::xray::{build_xray_config, validate_server, TunMode};

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
    let mut unique = BTreeSet::new();
    for (host, port) in server_endpoints(server) {
        let addresses = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|error| format!("could not resolve VPN endpoint {host}:{port}: {error}"))?;
        unique.extend(addresses.map(|address| address.ip()));
    }
    if unique.is_empty() {
        return Err(format!(
            "VPN location {} did not resolve to an address",
            server.label
        ));
    }
    if unique.len() > varmlend::protocol::MAX_SERVER_IPS {
        return Err(format!(
            "VPN location resolves to {} addresses; the safe limit is {}",
            unique.len(),
            varmlend::protocol::MAX_SERVER_IPS
        ));
    }
    Ok(unique.into_iter().collect())
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
    validate_server(&server)?;

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
        use varmlend::protocol::DaemonCommand;

        let _ = app;
        let Ok(mut daemon) = crate::daemon_client::DaemonClient::connect_installed().await else {
            return Ok(String::new());
        };
        let state = daemon
            .request(DaemonCommand::LogTail)
            .await
            .map_err(|error| error.to_string())?;
        Ok(state.log_tail.unwrap_or_default())
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
        use varmlend::protocol::DaemonCommand;

        let _ = app;
        let mut daemon = crate::daemon_client::DaemonClient::connect_installed()
            .await
            .map_err(|error| error.to_string())?;
        daemon
            .request(DaemonCommand::ClearLog)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
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
pub async fn tcp_ping_host(
    app: tauri::AppHandle,
    host: String,
    port: u16,
    timeout_ms: Option<u32>,
) -> Result<u32, String> {
    #[cfg(target_os = "linux")]
    {
        use varmlend::protocol::{DaemonCommand, TcpPingRequest};

        let mut daemon = crate::daemon_client::DaemonClient::connect_or_start_installed()
            .await
            .map_err(|error| error.to_string())?;
        let state = daemon
            .request(DaemonCommand::TcpPing(TcpPingRequest {
                host,
                port,
                timeout_ms: timeout_ms.unwrap_or(2500),
            }))
            .await
            .map_err(|error| error.to_string())?;
        let _ = app;
        state
            .rtt_ms
            .ok_or_else(|| "daemon did not return a TCP RTT".to_string())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, host, port, timeout_ms);
        Err("TCP location ping is unavailable on this platform".into())
    }
}

fn free_local_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("could not allocate ping port: {error}"))
}

#[tauri::command]
pub async fn proxy_get_ping(
    app: tauri::AppHandle,
    server: VlessServer,
    timeout_ms: Option<u32>,
) -> Result<u32, String> {
    #[cfg(target_os = "linux")]
    {
        use varmlend::protocol::{DaemonCommand, DaemonErrorCode, ProxyPingRequest};

        validate_server(&server)?;
        let proxy_count = crate::xray::ping_proxy_count(&server)?;
        let mut socks_ports = Vec::with_capacity(proxy_count);
        while socks_ports.len() < proxy_count {
            let port = free_local_port()?;
            if !socks_ports.contains(&port) {
                socks_ports.push(port);
            }
        }
        let socks_port = socks_ports[0];
        let xray_config =
            serde_json::to_string(&crate::xray::build_ping_config(&server, &socks_ports)?)
                .map_err(|error| error.to_string())?;
        let timeout_ms = timeout_ms.unwrap_or(5000);
        let mut daemon = crate::daemon_client::DaemonClient::connect_or_start_installed()
            .await
            .map_err(|error| error.to_string())?;
        let request = ProxyPingRequest {
            xray_config,
            socks_port,
            socks_ports: socks_ports.clone(),
            timeout_ms,
        };
        let state = match daemon
            .request(DaemonCommand::ProxyPing(ProxyPingRequest { ..request }))
            .await
        {
            Ok(state) => state,
            Err(crate::daemon_client::ClientError::Daemon(DaemonErrorCode::InvalidRequest, _)) => {
                let xray_config = serde_json::to_string(&crate::xray::build_legacy_ping_config(
                    &server, socks_port,
                )?)
                .map_err(|error| error.to_string())?;
                daemon
                    .request(DaemonCommand::ProxyPing(ProxyPingRequest {
                        xray_config,
                        socks_port,
                        socks_ports: Vec::new(),
                        timeout_ms,
                    }))
                    .await
                    .map_err(|error| error.to_string())?
            }
            Err(error) => return Err(error.to_string()),
        };
        let _ = app;
        state
            .rtt_ms
            .ok_or_else(|| "daemon did not return an HTTP RTT".to_string())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, server, timeout_ms);
        Err("HTTP location ping is unavailable on this platform".into())
    }
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

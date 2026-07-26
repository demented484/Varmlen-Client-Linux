use serde::{Deserialize, Serialize};
use std::net::IpAddr;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_CONFIG_BYTES: usize = 384 * 1024;
const MAX_SERVER_IPS: usize = 16;
const MAX_EXCLUDED_APPS: usize = 256;
const MAX_APP_SELECTOR_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    Tun,
    Proxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub mode: ConnectionMode,
    pub xray_config: String,
    pub validation_config: String,
    pub server_ips: Vec<IpAddr>,
    pub excluded_apps: Vec<String>,
    pub killswitch: bool,
    pub allow_lan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCommand {
    Status,
    Connect(ConnectRequest),
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub version: u16,
    pub operation_id: u64,
    pub command: DaemonCommand,
}

impl RequestEnvelope {
    pub fn new(version: u16, operation_id: u64, command: DaemonCommand) -> Self {
        Self {
            version,
            operation_id,
            command,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionPhase {
    Disconnected,
    Preparing,
    Connected,
    Blocking,
    Reconnecting,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonState {
    pub phase: ConnectionPhase,
    pub split_active: bool,
    pub dns_protected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonErrorCode {
    UnsupportedVersion,
    FrameTooLarge,
    InvalidFrame,
    Unauthorized,
    HoldBlockFailed,
    HoldBlockVerificationFailed,
    HoldBlockRemovalFailed,
    TunnelPreparationFailed,
    TunnelCommitFailed,
    TunnelCleanupFailed,
    DnsInstallFailed,
    DnsVerificationFailed,
    InvalidRequest,
    XrayUnavailable,
    XrayValidationFailed,
    XrayStartFailed,
    SplitUnavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonError {
    pub code: DaemonErrorCode,
    pub message: String,
}

impl DaemonError {
    pub fn new(code: DaemonErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub version: u16,
    pub operation_id: u64,
    pub result: Result<DaemonState, DaemonError>,
}

pub fn validate_request(request: &RequestEnvelope) -> Result<(), DaemonErrorCode> {
    if request.version != PROTOCOL_VERSION {
        return Err(DaemonErrorCode::UnsupportedVersion);
    }
    if let DaemonCommand::Connect(connect) = &request.command {
        validate_connect_request(connect)?;
    }
    Ok(())
}

pub fn validate_connect_request(request: &ConnectRequest) -> Result<(), DaemonErrorCode> {
    if request.xray_config.is_empty()
        || request.validation_config.is_empty()
        || request.xray_config.len() > MAX_CONFIG_BYTES
        || request.validation_config.len() > MAX_CONFIG_BYTES
        || request.server_ips.is_empty()
        || request.server_ips.len() > MAX_SERVER_IPS
        || request.excluded_apps.len() > MAX_EXCLUDED_APPS
        || request.excluded_apps.iter().any(|application| {
            application.trim().is_empty()
                || application.len() > MAX_APP_SELECTOR_BYTES
                || application.contains('\0')
        })
        || (request.mode == ConnectionMode::Proxy && !request.excluded_apps.is_empty())
    {
        return Err(DaemonErrorCode::InvalidRequest);
    }
    Ok(())
}

pub fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, DaemonErrorCode> {
    let encoded = serde_json::to_vec(value).map_err(|_| DaemonErrorCode::InvalidFrame)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(DaemonErrorCode::FrameTooLarge);
    }
    Ok(encoded)
}

pub fn decode_request_frame(bytes: &[u8]) -> Result<RequestEnvelope, DaemonErrorCode> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(DaemonErrorCode::FrameTooLarge);
    }
    let request = serde_json::from_slice(bytes).map_err(|_| DaemonErrorCode::InvalidFrame)?;
    validate_request(&request)?;
    Ok(request)
}

pub fn decode_response_frame(bytes: &[u8]) -> Result<ResponseEnvelope, DaemonErrorCode> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(DaemonErrorCode::FrameTooLarge);
    }
    let response: ResponseEnvelope =
        serde_json::from_slice(bytes).map_err(|_| DaemonErrorCode::InvalidFrame)?;
    if response.version != PROTOCOL_VERSION {
        return Err(DaemonErrorCode::UnsupportedVersion);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{validate_connect_request, ConnectRequest, ConnectionMode, DaemonErrorCode};

    fn valid_connect() -> ConnectRequest {
        ConnectRequest {
            mode: ConnectionMode::Tun,
            xray_config: "{}".into(),
            validation_config: "{}".into(),
            server_ips: vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))],
            excluded_apps: vec!["cs2".into()],
            killswitch: true,
            allow_lan: false,
        }
    }

    #[test]
    fn connect_request_rejects_unbounded_or_inconsistent_input() {
        let mut request = valid_connect();
        request.server_ips.clear();
        assert_eq!(
            validate_connect_request(&request),
            Err(DaemonErrorCode::InvalidRequest)
        );

        let mut request = valid_connect();
        request.mode = ConnectionMode::Proxy;
        assert_eq!(
            validate_connect_request(&request),
            Err(DaemonErrorCode::InvalidRequest)
        );

        let mut request = valid_connect();
        request.excluded_apps = vec!["../\0escape".into()];
        assert_eq!(
            validate_connect_request(&request),
            Err(DaemonErrorCode::InvalidRequest)
        );
    }

    #[test]
    fn valid_tun_connect_request_is_accepted() {
        assert_eq!(validate_connect_request(&valid_connect()), Ok(()));
    }
}

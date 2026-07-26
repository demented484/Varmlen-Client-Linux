use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCommand {
    Status,
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
    let request =
        serde_json::from_slice(bytes).map_err(|_| DaemonErrorCode::InvalidFrame)?;
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

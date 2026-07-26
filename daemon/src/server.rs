use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::RwLock;

use crate::protocol::{
    decode_request_frame, encode_frame, DaemonCommand, DaemonError, DaemonErrorCode, DaemonState,
    ResponseEnvelope, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};

#[derive(Debug, Clone, Copy)]
pub struct PeerPolicy {
    allowed_uid: u32,
}

impl PeerPolicy {
    pub fn new(allowed_uid: u32) -> Self {
        Self { allowed_uid }
    }

    pub fn authorize(&self, uid: u32) -> bool {
        uid == self.allowed_uid
    }
}

pub fn parse_owner_uid(value: Option<&str>) -> Result<u32, &'static str> {
    let uid = value
        .ok_or("PKEXEC_UID is missing")?
        .parse::<u32>()
        .map_err(|_| "PKEXEC_UID is invalid")?;
    if uid == 0 {
        return Err("refusing to authorize root as desktop owner");
    }
    Ok(uid)
}

pub fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(credentials).cast(),
            &mut length,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(credentials.uid)
}

pub async fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, DaemonErrorCode> {
    let length = stream
        .read_u32()
        .await
        .map_err(|_| DaemonErrorCode::InvalidFrame)? as usize;
    if length > MAX_FRAME_BYTES {
        return Err(DaemonErrorCode::FrameTooLarge);
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| DaemonErrorCode::InvalidFrame)?;
    Ok(bytes)
}

pub async fn write_response(
    stream: &mut UnixStream,
    response: &ResponseEnvelope,
) -> Result<(), DaemonErrorCode> {
    let bytes = encode_frame(response)?;
    stream
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| DaemonErrorCode::Internal)?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|_| DaemonErrorCode::Internal)
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn handle(&self, command: DaemonCommand) -> Result<DaemonState, DaemonError>;
}

pub struct SnapshotHandler {
    state: Arc<RwLock<DaemonState>>,
}

impl SnapshotHandler {
    pub fn new(state: Arc<RwLock<DaemonState>>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl CommandHandler for SnapshotHandler {
    async fn handle(&self, command: DaemonCommand) -> Result<DaemonState, DaemonError> {
        match command {
            DaemonCommand::Status => Ok(self.state.read().await.clone()),
            DaemonCommand::Connect(_) | DaemonCommand::Disconnect => Err(DaemonError::new(
                DaemonErrorCode::Internal,
                "VPN lifecycle controller is unavailable",
            )),
        }
    }
}

pub async fn serve_connection(
    mut stream: UnixStream,
    policy: PeerPolicy,
    handler: Arc<dyn CommandHandler>,
) -> Result<(), DaemonErrorCode> {
    let uid = peer_uid(&stream).map_err(|_| DaemonErrorCode::Unauthorized)?;
    if !policy.authorize(uid) {
        return Err(DaemonErrorCode::Unauthorized);
    }

    loop {
        let bytes = match read_frame(&mut stream).await {
            Ok(bytes) => bytes,
            Err(DaemonErrorCode::InvalidFrame) => return Ok(()),
            Err(error) => return Err(error),
        };
        let request = match decode_request_frame(&bytes) {
            Ok(request) => request,
            Err(code) => {
                let response = ResponseEnvelope {
                    version: PROTOCOL_VERSION,
                    operation_id: 0,
                    result: Err(DaemonError {
                        code,
                        message: "invalid daemon request".to_string(),
                    }),
                };
                write_response(&mut stream, &response).await?;
                continue;
            }
        };
        let response = ResponseEnvelope {
            version: PROTOCOL_VERSION,
            operation_id: request.operation_id,
            result: handler.handle(request.command).await,
        };
        write_response(&mut stream, &response).await?;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tokio::sync::RwLock;

    use super::{serve_connection, PeerPolicy, SnapshotHandler};
    use crate::protocol::{
        decode_response_frame, encode_frame, ConnectionPhase, DaemonCommand, DaemonState,
        RequestEnvelope, PROTOCOL_VERSION,
    };

    #[tokio::test]
    async fn status_returns_recovery_required_snapshot() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let state = Arc::new(RwLock::new(DaemonState {
            phase: ConnectionPhase::RecoveryRequired,
            split_active: false,
            dns_protected: false,
        }));
        let handler = Arc::new(SnapshotHandler::new(state));
        let server_task = tokio::spawn(serve_connection(
            server,
            PeerPolicy::new(unsafe { libc::getuid() }),
            handler,
        ));
        let request = RequestEnvelope::new(PROTOCOL_VERSION, 9, DaemonCommand::Status);
        let bytes = encode_frame(&request).unwrap();
        client.write_u32(bytes.len() as u32).await.unwrap();
        client.write_all(&bytes).await.unwrap();
        let length = client.read_u32().await.unwrap();
        let mut response = vec![0; length as usize];
        client.read_exact(&mut response).await.unwrap();
        let response = decode_response_frame(&response).unwrap();
        assert_eq!(
            response.result.unwrap().phase,
            ConnectionPhase::RecoveryRequired
        );
        drop(client);
        server_task.await.unwrap().unwrap();
    }
}

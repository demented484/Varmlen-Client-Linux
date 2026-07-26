use std::io;
use std::os::fd::AsRawFd;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::protocol::{
    decode_request_frame, encode_frame, ConnectionPhase, DaemonError, DaemonErrorCode,
    DaemonState, ResponseEnvelope, PROTOCOL_VERSION, MAX_FRAME_BYTES,
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

pub async fn serve_connection(
    mut stream: UnixStream,
    policy: PeerPolicy,
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
            result: Ok(DaemonState {
                phase: ConnectionPhase::Disconnected,
                split_active: false,
                dns_protected: false,
            }),
        };
        write_response(&mut stream, &response).await?;
    }
}

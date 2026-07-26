use std::path::Path;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use varmlend::protocol::{
    decode_response_frame, encode_frame, DaemonCommand, DaemonError, DaemonState,
    RequestEnvelope, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("daemon I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("daemon protocol: {0:?}")]
    Protocol(varmlend::protocol::DaemonErrorCode),
    #[error("daemon operation: {0:?}: {1}")]
    Daemon(varmlend::protocol::DaemonErrorCode, String),
    #[error("daemon response operation ID mismatch")]
    OperationMismatch,
}

impl From<DaemonError> for ClientError {
    fn from(value: DaemonError) -> Self {
        Self::Daemon(value.code, value.message)
    }
}

pub struct DaemonClient {
    stream: UnixStream,
    next_operation_id: u64,
}

impl DaemonClient {
    pub async fn connect(path: &Path) -> Result<Self, ClientError> {
        Ok(Self {
            stream: UnixStream::connect(path).await?,
            next_operation_id: 1,
        })
    }

    pub async fn request(&mut self, command: DaemonCommand) -> Result<DaemonState, ClientError> {
        let operation_id = self.next_operation_id;
        self.next_operation_id = self.next_operation_id.wrapping_add(1).max(1);
        let request = RequestEnvelope::new(PROTOCOL_VERSION, operation_id, command);
        let bytes = encode_frame(&request).map_err(ClientError::Protocol)?;

        self.stream.write_u32(bytes.len() as u32).await?;
        self.stream.write_all(&bytes).await?;

        let length = self.stream.read_u32().await? as usize;
        if length > MAX_FRAME_BYTES {
            return Err(ClientError::Protocol(
                varmlend::protocol::DaemonErrorCode::FrameTooLarge,
            ));
        }
        let mut response_bytes = vec![0; length];
        self.stream.read_exact(&mut response_bytes).await?;
        let response = decode_response_frame(&response_bytes).map_err(ClientError::Protocol)?;
        if response.operation_id != operation_id {
            return Err(ClientError::OperationMismatch);
        }
        response.result.map_err(ClientError::from)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tokio::net::UnixListener;
    use varmlend::protocol::{
        ConnectionPhase, DaemonCommand, DaemonState, ResponseEnvelope, PROTOCOL_VERSION,
    };
    use varmlend::server::{read_frame, write_response};

    use super::DaemonClient;

    #[tokio::test]
    async fn client_round_trip_preserves_operation_id() {
        let socket = unique_socket_path();
        let listener = UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let bytes = read_frame(&mut stream).await.unwrap();
            let request = varmlend::protocol::decode_request_frame(&bytes).unwrap();
            write_response(
                &mut stream,
                &ResponseEnvelope {
                    version: PROTOCOL_VERSION,
                    operation_id: request.operation_id,
                    result: Ok(DaemonState {
                        phase: ConnectionPhase::Disconnected,
                        split_active: false,
                        dns_protected: false,
                    }),
                },
            )
            .await
            .unwrap();
        });

        let mut client = DaemonClient::connect(&socket).await.unwrap();
        let state = client.request(DaemonCommand::Status).await.unwrap();
        assert_eq!(state.phase, ConnectionPhase::Disconnected);
        server.await.unwrap();
        let _ = std::fs::remove_file(socket);
    }

    fn unique_socket_path() -> PathBuf {
        std::env::temp_dir().join(format!("vd-{}.sock", std::process::id()))
    }
}

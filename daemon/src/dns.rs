use async_trait::async_trait;
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};

use crate::nft::{apply_ruleset, remove_dns_table, render_dns_rules, DnsNftPlan};
use crate::protocol::{DaemonError, DaemonErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsPlan {
    local_port: u16,
}

impl DnsPlan {
    pub fn new(local_port: u16) -> Self {
        Self { local_port }
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsLease {
    port: u16,
}

impl DnsLease {
    pub fn port(&self) -> u16 {
        self.port
    }
}

#[async_trait]
pub trait DnsBackend {
    async fn apply(&mut self, plan: &DnsPlan) -> Result<(), DaemonError>;
    async fn listener_is_ready(&mut self, port: u16) -> Result<bool, DaemonError>;
    async fn probe_through_tunnel(&mut self) -> Result<bool, DaemonError>;
    async fn remove(&mut self) -> Result<(), DaemonError>;
}

pub struct DnsGuard<B> {
    backend: B,
}

pub struct SystemDnsBackend {
    local_port: u16,
}

impl SystemDnsBackend {
    pub fn new(local_port: u16) -> Self {
        Self { local_port }
    }
}

#[async_trait]
impl DnsBackend for SystemDnsBackend {
    async fn apply(&mut self, plan: &DnsPlan) -> Result<(), DaemonError> {
        if plan.local_port() != self.local_port {
            return Err(DaemonError::new(
                DaemonErrorCode::DnsInstallFailed,
                "DNS backend port does not match the requested plan",
            ));
        }
        let rules = render_dns_rules(&DnsNftPlan::new(self.local_port));
        apply_ruleset(&rules).await
    }

    async fn listener_is_ready(&mut self, port: u16) -> Result<bool, DaemonError> {
        Ok(timeout(
            Duration::from_secs(2),
            TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        .is_ok_and(|result| result.is_ok()))
    }

    async fn probe_through_tunnel(&mut self) -> Result<bool, DaemonError> {
        let socket = UdpSocket::bind(("127.0.0.1", 0)).await.map_err(|error| {
            DaemonError::new(
                DaemonErrorCode::DnsVerificationFailed,
                format!("could not bind DNS probe socket: {error}"),
            )
        })?;
        let transaction_id = (std::process::id() as u16) ^ 0x564d;
        let query = build_dns_query(transaction_id, "mullvad.net")?;
        socket
            .send_to(&query, ("127.0.0.1", self.local_port))
            .await
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::DnsVerificationFailed,
                    format!("could not send DNS probe: {error}"),
                )
            })?;
        let mut response = [0_u8; 2048];
        let received = timeout(Duration::from_secs(5), socket.recv(&mut response))
            .await
            .map_err(|_| {
                DaemonError::new(
                    DaemonErrorCode::DnsVerificationFailed,
                    "DNS probe timed out",
                )
            })?
            .map_err(|error| {
                DaemonError::new(
                    DaemonErrorCode::DnsVerificationFailed,
                    format!("DNS probe failed: {error}"),
                )
            })?;
        Ok(validate_dns_response(
            transaction_id,
            &response[..received],
        ))
    }

    async fn remove(&mut self) -> Result<(), DaemonError> {
        remove_dns_table().await
    }
}

pub fn build_dns_query(
    transaction_id: u16,
    domain: &str,
) -> Result<Vec<u8>, DaemonError> {
    let mut query = Vec::with_capacity(64);
    query.extend_from_slice(&transaction_id.to_be_bytes());
    query.extend_from_slice(&[0x01, 0x00]);
    query.extend_from_slice(&1_u16.to_be_bytes());
    query.extend_from_slice(&0_u16.to_be_bytes());
    query.extend_from_slice(&0_u16.to_be_bytes());
    query.extend_from_slice(&0_u16.to_be_bytes());
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 || !label.is_ascii() {
            return Err(DaemonError::new(
                DaemonErrorCode::DnsVerificationFailed,
                "invalid DNS probe domain",
            ));
        }
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.push(0);
    query.extend_from_slice(&1_u16.to_be_bytes());
    query.extend_from_slice(&1_u16.to_be_bytes());
    if query.len() > 512 {
        return Err(DaemonError::new(
            DaemonErrorCode::DnsVerificationFailed,
            "DNS probe query is too large",
        ));
    }
    Ok(query)
}

pub fn validate_dns_response(transaction_id: u16, response: &[u8]) -> bool {
    response.len() >= 12
        && response[0..2] == transaction_id.to_be_bytes()
        && response[2] & 0x80 != 0
}

impl<B: DnsBackend> DnsGuard<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub async fn install(&mut self, plan: DnsPlan) -> Result<DnsLease, DaemonError> {
        self.backend.apply(&plan).await?;
        let listener_ready = self.backend.listener_is_ready(plan.local_port()).await?;
        let probe_ok = if listener_ready {
            self.backend.probe_through_tunnel().await?
        } else {
            false
        };
        if !listener_ready || !probe_ok {
            let _ = self.backend.remove().await;
            return Err(DaemonError::new(
                DaemonErrorCode::DnsVerificationFailed,
                "DNS interception could not be verified",
            ));
        }
        Ok(DnsLease {
            port: plan.local_port(),
        })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::{DnsBackend, DnsGuard, DnsPlan};
    use super::{build_dns_query, validate_dns_response};
    use crate::protocol::{DaemonError, DaemonErrorCode};

    #[derive(Default)]
    struct FakeDnsBackend {
        listener_up: bool,
        probe_ok: bool,
        installed: bool,
        removed: bool,
    }

    #[async_trait]
    impl DnsBackend for FakeDnsBackend {
        async fn apply(&mut self, _plan: &DnsPlan) -> Result<(), DaemonError> {
            self.installed = true;
            Ok(())
        }

        async fn listener_is_ready(&mut self, _port: u16) -> Result<bool, DaemonError> {
            Ok(self.listener_up)
        }

        async fn probe_through_tunnel(&mut self) -> Result<bool, DaemonError> {
            Ok(self.probe_ok)
        }

        async fn remove(&mut self) -> Result<(), DaemonError> {
            self.removed = true;
            self.installed = false;
            Ok(())
        }
    }

    #[tokio::test]
    async fn listener_failure_rolls_back_and_fails_closed() {
        let backend = FakeDnsBackend {
            listener_up: false,
            probe_ok: true,
            ..Default::default()
        };
        let mut guard = DnsGuard::new(backend);
        let error = guard.install(DnsPlan::new(5353)).await.unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::DnsVerificationFailed);
        assert!(guard.backend().removed);
        assert!(!guard.backend().installed);
    }

    #[tokio::test]
    async fn upstream_probe_failure_rolls_back_and_fails_closed() {
        let backend = FakeDnsBackend {
            listener_up: true,
            probe_ok: false,
            ..Default::default()
        };
        let mut guard = DnsGuard::new(backend);
        let error = guard.install(DnsPlan::new(5353)).await.unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::DnsVerificationFailed);
        assert!(guard.backend().removed);
    }

    #[tokio::test]
    async fn verified_dns_returns_active_lease() {
        let backend = FakeDnsBackend {
            listener_up: true,
            probe_ok: true,
            ..Default::default()
        };
        let mut guard = DnsGuard::new(backend);
        let lease = guard.install(DnsPlan::new(5353)).await.unwrap();
        assert_eq!(lease.port(), 5353);
        assert!(guard.backend().installed);
        assert!(!guard.backend().removed);
    }

    #[test]
    fn probe_query_is_a_bounded_a_record_request() {
        let query = build_dns_query(0x1234, "example.com").unwrap();
        assert_eq!(&query[0..2], &[0x12, 0x34]);
        assert_eq!(&query[2..4], &[0x01, 0x00]);
        assert_eq!(&query[4..6], &[0x00, 0x01]);
        assert!(query.len() < 512);
    }

    #[test]
    fn response_validation_requires_matching_id_and_response_bit() {
        let mut response = vec![0x12, 0x34, 0x81, 0x80, 0, 1, 0, 1, 0, 0, 0, 0];
        assert!(validate_dns_response(0x1234, &response));
        assert!(!validate_dns_response(0x9999, &response));
        response[2] &= 0x7f;
        assert!(!validate_dns_response(0x1234, &response));
    }
}

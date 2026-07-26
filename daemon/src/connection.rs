use async_trait::async_trait;

use crate::protocol::{ConnectionPhase, DaemonError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelConfig {
    pub id: String,
}

impl TunnelConfig {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTunnel {
    pub id: String,
}

impl PreparedTunnel {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
pub trait NetworkBackend {
    async fn install_hold_block(&mut self) -> Result<(), DaemonError>;
    async fn verify_hold_block(&mut self) -> Result<(), DaemonError>;
    async fn prepare_tunnel(
        &mut self,
        config: &TunnelConfig,
    ) -> Result<PreparedTunnel, DaemonError>;
    async fn commit_tunnel(&mut self, tunnel: &PreparedTunnel) -> Result<(), DaemonError>;
    async fn remove_old_tunnel(&mut self) -> Result<(), DaemonError>;
    async fn remove_hold_block(&mut self) -> Result<(), DaemonError>;
}

pub struct ConnectionManager<B> {
    backend: B,
    phase: ConnectionPhase,
}

impl<B: NetworkBackend> ConnectionManager<B> {
    pub fn connected(backend: B) -> Self {
        Self {
            backend,
            phase: ConnectionPhase::Connected,
        }
    }

    pub fn phase(&self) -> ConnectionPhase {
        self.phase
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub async fn reconnect(&mut self, config: TunnelConfig) -> Result<(), DaemonError> {
        self.phase = ConnectionPhase::Reconnecting;
        if let Err(error) = self.backend.install_hold_block().await {
            self.phase = ConnectionPhase::Connected;
            return Err(error);
        }
        if let Err(error) = self.backend.verify_hold_block().await {
            if self.backend.remove_hold_block().await.is_ok() {
                self.phase = ConnectionPhase::Connected;
            } else {
                self.phase = ConnectionPhase::RecoveryRequired;
            }
            return Err(error);
        }

        self.phase = ConnectionPhase::Blocking;
        let prepared = self.backend.prepare_tunnel(&config).await?;
        self.backend.commit_tunnel(&prepared).await?;
        self.backend.remove_old_tunnel().await?;
        self.backend.remove_hold_block().await?;
        self.phase = ConnectionPhase::Connected;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use async_trait::async_trait;

    use super::{ConnectionManager, NetworkBackend, PreparedTunnel, TunnelConfig};
    use crate::protocol::{ConnectionPhase, DaemonError, DaemonErrorCode};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Action {
        InstallHoldBlock,
        VerifyHoldBlock,
        PrepareTunnel,
        CommitTunnel,
        RemoveOldTunnel,
        RemoveHoldBlock,
    }

    struct FakeBackend {
        fail_on: Option<Action>,
        actions: VecDeque<Action>,
        old_tunnel_running: bool,
        hold_block_active: bool,
    }

    impl FakeBackend {
        fn connected() -> Self {
            Self {
                fail_on: None,
                actions: VecDeque::new(),
                old_tunnel_running: true,
                hold_block_active: false,
            }
        }

        fn fail_on(mut self, action: Action) -> Self {
            self.fail_on = Some(action);
            self
        }

        fn run(&mut self, action: Action, code: DaemonErrorCode) -> Result<(), DaemonError> {
            self.actions.push_back(action);
            if self.fail_on == Some(action) {
                return Err(DaemonError::new(code, "injected failure"));
            }
            Ok(())
        }
    }

    #[async_trait]
    impl NetworkBackend for FakeBackend {
        async fn install_hold_block(&mut self) -> Result<(), DaemonError> {
            self.run(Action::InstallHoldBlock, DaemonErrorCode::HoldBlockFailed)?;
            self.hold_block_active = true;
            Ok(())
        }

        async fn verify_hold_block(&mut self) -> Result<(), DaemonError> {
            self.run(Action::VerifyHoldBlock, DaemonErrorCode::HoldBlockVerificationFailed)
        }

        async fn prepare_tunnel(
            &mut self,
            config: &TunnelConfig,
        ) -> Result<PreparedTunnel, DaemonError> {
            self.run(Action::PrepareTunnel, DaemonErrorCode::TunnelPreparationFailed)?;
            Ok(PreparedTunnel::new(config.id.clone()))
        }

        async fn commit_tunnel(
            &mut self,
            _tunnel: &PreparedTunnel,
        ) -> Result<(), DaemonError> {
            self.run(Action::CommitTunnel, DaemonErrorCode::TunnelCommitFailed)
        }

        async fn remove_old_tunnel(&mut self) -> Result<(), DaemonError> {
            self.run(Action::RemoveOldTunnel, DaemonErrorCode::TunnelCleanupFailed)?;
            self.old_tunnel_running = false;
            Ok(())
        }

        async fn remove_hold_block(&mut self) -> Result<(), DaemonError> {
            self.run(Action::RemoveHoldBlock, DaemonErrorCode::HoldBlockRemovalFailed)?;
            self.hold_block_active = false;
            Ok(())
        }
    }

    #[tokio::test]
    async fn failed_hold_block_preserves_old_tunnel() {
        let backend = FakeBackend::connected().fail_on(Action::InstallHoldBlock);
        let mut manager = ConnectionManager::connected(backend);
        let error = manager.reconnect(TunnelConfig::new("new")).await.unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::HoldBlockFailed);
        assert!(manager.backend().old_tunnel_running);
        assert!(!manager.backend().hold_block_active);
        assert_eq!(manager.phase(), ConnectionPhase::Connected);
    }

    #[tokio::test]
    async fn failure_after_verified_block_remains_fail_closed() {
        let backend = FakeBackend::connected().fail_on(Action::PrepareTunnel);
        let mut manager = ConnectionManager::connected(backend);
        let error = manager.reconnect(TunnelConfig::new("new")).await.unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::TunnelPreparationFailed);
        assert!(manager.backend().old_tunnel_running);
        assert!(manager.backend().hold_block_active);
        assert_eq!(manager.phase(), ConnectionPhase::Blocking);
    }

    #[tokio::test]
    async fn unverified_hold_block_is_removed_without_touching_old_tunnel() {
        let backend = FakeBackend::connected().fail_on(Action::VerifyHoldBlock);
        let mut manager = ConnectionManager::connected(backend);
        let error = manager.reconnect(TunnelConfig::new("new")).await.unwrap_err();
        assert_eq!(error.code, DaemonErrorCode::HoldBlockVerificationFailed);
        assert!(manager.backend().old_tunnel_running);
        assert!(!manager.backend().hold_block_active);
        assert_eq!(
            manager.backend().actions,
            VecDeque::from([
                Action::InstallHoldBlock,
                Action::VerifyHoldBlock,
                Action::RemoveHoldBlock,
            ])
        );
        assert_eq!(manager.phase(), ConnectionPhase::Connected);
    }

    #[tokio::test]
    async fn successful_reconnect_orders_block_before_old_tunnel_removal() {
        let mut manager = ConnectionManager::connected(FakeBackend::connected());
        manager.reconnect(TunnelConfig::new("new")).await.unwrap();
        assert_eq!(
            manager.backend().actions,
            VecDeque::from([
                Action::InstallHoldBlock,
                Action::VerifyHoldBlock,
                Action::PrepareTunnel,
                Action::CommitTunnel,
                Action::RemoveOldTunnel,
                Action::RemoveHoldBlock,
            ])
        );
        assert_eq!(manager.phase(), ConnectionPhase::Connected);
        assert!(!manager.backend().old_tunnel_running);
        assert!(!manager.backend().hold_block_active);
    }
}

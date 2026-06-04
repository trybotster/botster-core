//! Core daemon supervisor and typed API implementation.

use std::path::PathBuf;

use botster_core::{
    BotsterEngineObservation, ClientId, CoreSession, DefaultBotsterEngine,
    DefaultBotsterEngineError, QueueSource, ResizePayload, SessionId, SubscriptionId,
};
use thiserror::Error;

use crate::api::{
    AttachedSession, DaemonHealth, DaemonSession, DaemonStatus, DrainResult, GuardedWriteRequest,
    GuardedWriteResult, SessionAdoptionReport, SessionAdoptionState, SpawnSessionRequest,
};
use crate::guarded_write::{decide_guarded_write, GuardedWriteDecision, GuardedWriteDeliveryState};
use crate::registry::{
    command_label, RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError,
};

/// Daemon configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreDaemonConfig {
    /// Caller-chosen data directory for registry metadata.
    pub data_dir: PathBuf,
    /// Logical daemon client queue capacity.
    pub client_queue_capacity: usize,
}

impl CoreDaemonConfig {
    /// Build a config with the default bounded client queue capacity.
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            client_queue_capacity: QueueSource::ClientWorker.default_capacity(),
        }
    }
}

/// Daemon API error.
#[derive(Debug, Error)]
pub enum CoreDaemonError {
    /// Core engine error.
    #[error(transparent)]
    Engine(#[from] DefaultBotsterEngineError),
    /// Registry error.
    #[error(transparent)]
    Registry(#[from] SessionRegistryError),
    /// Session id was not found.
    #[error("unknown session: {0:?}")]
    UnknownSession(SessionId),
    /// Daemon has shut down.
    #[error("daemon is shut down")]
    Shutdown,
}

/// Production core daemon supervisor.
pub struct CoreDaemon {
    config: CoreDaemonConfig,
    registry: SessionRegistry,
    engine: DefaultBotsterEngine,
    running: bool,
}

impl CoreDaemon {
    /// Build a daemon with a caller-provided data directory.
    #[must_use]
    pub fn new(config: CoreDaemonConfig) -> Self {
        let registry = SessionRegistry::new(&config.data_dir);
        Self {
            config,
            registry,
            engine: DefaultBotsterEngine::new(),
            running: true,
        }
    }

    /// Return the registry handle.
    #[must_use]
    pub const fn registry(&self) -> &SessionRegistry {
        &self.registry
    }

    /// Spawn a session through the existing core local engine path and persist registry metadata.
    pub fn spawn(
        &mut self,
        request: SpawnSessionRequest,
        now_seconds: u64,
    ) -> Result<CoreSession, CoreDaemonError> {
        self.ensure_running()?;
        let session_id = request.request.session_id.clone();
        let size = request
            .request
            .initial_pty_size
            .clone()
            .unwrap_or(ResizePayload { rows: 24, cols: 80 });
        let label = command_label(&request.request.executable);
        let spawn = self
            .engine
            .spawn_session(request.request, request.metadata)?;
        let record = RegistryRecord::running(
            session_id,
            Some(spawn.handle.process),
            size,
            label,
            now_seconds,
        );
        self.registry.save(&record)?;
        Ok(spawn.session)
    }

    /// List durable daemon sessions.
    pub fn list(&self) -> Result<Vec<DaemonSession>, CoreDaemonError> {
        Ok(self
            .registry
            .load_all()?
            .iter()
            .map(DaemonSession::from)
            .collect())
    }

    /// Attach a client through the existing subscription path.
    pub fn attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<AttachedSession, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine.attach_client(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            now_seconds,
        )?;
        Ok(AttachedSession {
            client_id,
            session_id,
            subscription_id,
        })
    }

    /// Detach a client through the existing subscription path.
    pub fn detach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine
            .detach_client(client_id, session_id, subscription_id, now_seconds)?;
        Ok(())
    }

    /// Send PTY input through the existing engine path.
    pub fn input(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine
            .write_bytes(client_id, session_id, data.into(), now_seconds)?;
        Ok(())
    }

    /// Resize a session through the existing engine path and update registry metadata.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine
            .resize(client_id, session_id.clone(), rows, cols, now_seconds)?;
        if let Some(mut record) = self.registry.load(&session_id)? {
            record.rows = rows;
            record.cols = cols;
            record.updated_at = now_seconds;
            self.registry.save(&record)?;
        }
        Ok(())
    }

    /// Drain one session's runtime output through subscription fanout.
    pub fn drain(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<DrainResult, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(session_id)?;
        let outcome = self.engine.drain_runtime_once(session_id, last_output_at)?;
        Ok(DrainResult {
            client_egress: outcome.client_egress,
            observations: outcome.observations.clone(),
            backpressure: outcome
                .observations
                .into_iter()
                .filter_map(|observation| match observation {
                    BotsterEngineObservation::Backpressure(summary) => Some(summary),
                    _ => None,
                })
                .collect(),
        })
    }

    /// Subscribe output by attaching a client; embedders drain through [`Self::drain`].
    pub fn subscribe_output(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<AttachedSession, CoreDaemonError> {
        self.attach(client_id, session_id, subscription_id, now_seconds)
    }

    /// Evaluate readiness and inject only through the existing PTY input path.
    pub fn guarded_write(
        &mut self,
        request: GuardedWriteRequest,
    ) -> Result<GuardedWriteResult, CoreDaemonError> {
        self.ensure_running()?;
        if self.engine.session(&request.session_id).is_none() {
            return Ok(GuardedWriteResult {
                decision: GuardedWriteDecision::Reject {
                    reason: "unknown session".to_string(),
                },
                states: vec![
                    GuardedWriteDeliveryState::Accepted,
                    GuardedWriteDeliveryState::Rejected,
                ],
            });
        }

        let decision = decide_guarded_write(&request.readiness);
        let mut states = vec![GuardedWriteDeliveryState::Accepted];
        match &decision {
            GuardedWriteDecision::Write => {
                self.engine.write_bytes(
                    request.client_id,
                    request.session_id,
                    request.data,
                    request.now_seconds,
                )?;
                states.push(GuardedWriteDeliveryState::Written);
            }
            GuardedWriteDecision::Defer { .. } => {
                states.push(GuardedWriteDeliveryState::Deferred);
            }
            GuardedWriteDecision::Reject { .. } => {
                states.push(GuardedWriteDeliveryState::Rejected);
            }
        }

        Ok(GuardedWriteResult { decision, states })
    }

    /// Return daemon health.
    pub fn health(&self) -> Result<DaemonHealth, CoreDaemonError> {
        Ok(DaemonHealth {
            running: self.running,
            live_sessions: self.engine.list_sessions().len(),
            registry_records: self.registry.load_all()?.len(),
            data_dir: self.config.data_dir.display().to_string(),
        })
    }

    /// Return daemon status.
    pub fn status(&self) -> Result<DaemonStatus, CoreDaemonError> {
        Ok(DaemonStatus {
            health: self.health()?,
            sessions: self.list()?,
        })
    }

    /// Scan persisted records for follow-up restart/adoption work.
    pub fn adoption_scan(&self) -> Result<Vec<SessionAdoptionReport>, CoreDaemonError> {
        Ok(self
            .registry
            .load_all()?
            .into_iter()
            .map(|record| {
                let state = if matches!(
                    record.state,
                    RegistrySessionState::Exited | RegistrySessionState::Stale
                ) {
                    SessionAdoptionState::Terminal
                } else if record.handshake_verified
                    && record.ping_pong_supported
                    && record.protocol_version == botster_core::PROTOCOL_VERSION
                {
                    SessionAdoptionState::Adoptable
                } else {
                    SessionAdoptionState::MissingProtocolEvidence
                };
                SessionAdoptionReport { record, state }
            })
            .collect())
    }

    /// Shut down one session or all sessions when `session_id` is absent.
    pub fn shutdown(
        &mut self,
        session_id: Option<SessionId>,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        if let Some(session_id) = session_id {
            self.shutdown_session(session_id, now_seconds)?;
            return Ok(());
        }

        let sessions: Vec<_> = self.engine.list_sessions();
        for session in sessions {
            self.shutdown_session(session.session_id, now_seconds)?;
        }
        self.running = false;
        Ok(())
    }

    fn shutdown_session(
        &mut self,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_session(&session_id)?;
        self.engine
            .shutdown_session(session_id.clone(), "daemon shutdown", now_seconds)?;
        if let Some(mut record) = self.registry.load(&session_id)? {
            record.mark(RegistrySessionState::Stopping, now_seconds);
            self.registry.save(&record)?;
        }
        Ok(())
    }

    fn ensure_running(&self) -> Result<(), CoreDaemonError> {
        if self.running {
            Ok(())
        } else {
            Err(CoreDaemonError::Shutdown)
        }
    }

    fn ensure_session(&self, session_id: &SessionId) -> Result<(), CoreDaemonError> {
        self.engine
            .session(session_id)
            .map(|_| ())
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))
    }
}

//! Core daemon supervisor and typed API implementation.

use std::path::PathBuf;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use botster_core::{
    BotsterEngineObservation, ClientId, CoreSession, DefaultBotsterEngine,
    DefaultBotsterEngineError, EnvelopeId, EnvelopeTarget, NotificationId, NotificationInbox,
    QueueSource, ResizePayload, RoutedEnvelopeQueueConfig, RoutedEnvelopeRouter, SessionId,
    SessionWorkerHealthReason, SessionWorkerStaleReason, SubscriptionId, WorkerBackedBotsterEngine,
    WorkerProcessRuntimeOptions,
};
use thiserror::Error;

use crate::api::{
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, AttachedSession,
    DaemonHealth, DaemonSession, DaemonStatus, DrainNotificationsRequest, DrainNotificationsResult,
    DrainResult, DrainRoutedEnvelopesRequest, DrainRoutedEnvelopesResult, GuardedWriteRequest,
    GuardedWriteResult, NotificationStatusResult, PostNotificationRequest, PostNotificationResult,
    PublishRoutedEnvelopeRequest, PublishRoutedEnvelopeResult, RoutedEnvelopeDeliveryStateResult,
    SessionAdoptionReport, SessionAdoptionState, SpawnSessionRequest,
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
    /// Optional worker process executable for worker-backed durable sessions.
    pub worker_path: Option<PathBuf>,
    /// Bounded per-target routed-envelope queue settings.
    pub routed_envelope_queue: RoutedEnvelopeQueueConfig,
}

impl CoreDaemonConfig {
    /// Build a config with the default bounded client queue capacity.
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            client_queue_capacity: QueueSource::ClientWorker.default_capacity(),
            worker_path: None,
            routed_envelope_queue: RoutedEnvelopeQueueConfig::default(),
        }
    }

    /// Use worker process backed sessions through the supplied worker executable.
    #[must_use]
    pub fn with_worker_path(mut self, worker_path: impl Into<PathBuf>) -> Self {
        self.worker_path = Some(worker_path.into());
        self
    }

    /// Use explicit per-target routed-envelope queue settings.
    #[must_use]
    pub const fn with_routed_envelope_queue(mut self, config: RoutedEnvelopeQueueConfig) -> Self {
        self.routed_envelope_queue = config;
        self
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
    engine: DaemonEngine,
    notification_inbox: NotificationInbox,
    envelope_router: RoutedEnvelopeRouter,
    running: bool,
}

enum DaemonEngine {
    Local(DefaultBotsterEngine),
    Worker(WorkerBackedBotsterEngine),
}

impl CoreDaemon {
    /// Build a daemon with a caller-provided data directory.
    #[must_use]
    pub fn new(config: CoreDaemonConfig) -> Self {
        let registry = SessionRegistry::new(&config.data_dir);
        let engine = config
            .worker_path
            .as_ref()
            .map(|worker_path| {
                let mut options = WorkerProcessRuntimeOptions::new(worker_path);
                options.control_socket_dir = Some(worker_socket_dir(&config.data_dir));
                DaemonEngine::Worker(WorkerBackedBotsterEngine::with_options(options))
            })
            .unwrap_or_else(|| DaemonEngine::Local(DefaultBotsterEngine::new()));
        let envelope_queue = config.routed_envelope_queue.clone();
        Self {
            config,
            registry,
            engine,
            notification_inbox: NotificationInbox::new(),
            envelope_router: RoutedEnvelopeRouter::with_config(envelope_queue),
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
        let mut record = RegistryRecord::running(
            session_id,
            Some(spawn.handle.process),
            size,
            label,
            now_seconds,
        );
        if let Some(metadata) = self.engine.worker_metadata(&record.session_id) {
            if let Some(identity) = metadata.recovery_identity.clone() {
                record.observe_restart_contract(identity, now_seconds);
            }
        }
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
        self.reconcile_lifecycle_observations(&outcome.observations, last_output_at)?;
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

    /// Queue one policy-free notification inbox item.
    pub fn post_notification(
        &mut self,
        request: PostNotificationRequest,
    ) -> Result<PostNotificationResult, CoreDaemonError> {
        self.ensure_running()?;
        let id = self.notification_inbox.post(request.item);
        Ok(PostNotificationResult { id })
    }

    /// Drain deliverable notifications for one target exactly once.
    pub fn drain_notifications(
        &mut self,
        request: DrainNotificationsRequest,
    ) -> Result<DrainNotificationsResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(DrainNotificationsResult {
            items: self.notification_inbox.drain(&request.target, request.now),
        })
    }

    /// Acknowledge one notification inbox item.
    pub fn acknowledge_notification(
        &mut self,
        request: AcknowledgeNotificationRequest,
    ) -> Result<NotificationStatusResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(NotificationStatusResult {
            status: self.notification_inbox.acknowledge(&request.id),
        })
    }

    /// Return notification delivery status without changing daemon state.
    pub fn notification_status(&self, id: &NotificationId) -> NotificationStatusResult {
        NotificationStatusResult {
            status: self.notification_inbox.status(id),
        }
    }

    /// Publish one generic routed envelope through the daemon-owned router.
    pub fn publish_routed_envelope(
        &mut self,
        request: PublishRoutedEnvelopeRequest,
    ) -> Result<PublishRoutedEnvelopeResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(self.envelope_router.publish(request.envelope))
    }

    /// Drain routed envelopes for one target with cursor and limit semantics.
    pub fn drain_routed_envelopes(
        &mut self,
        request: DrainRoutedEnvelopesRequest,
    ) -> Result<DrainRoutedEnvelopesResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(self
            .envelope_router
            .drain(&request.target, request.after, request.limit))
    }

    /// Acknowledge one routed envelope target copy.
    pub fn acknowledge_routed_envelope(
        &mut self,
        request: AcknowledgeRoutedEnvelopeRequest,
    ) -> Result<RoutedEnvelopeDeliveryStateResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(RoutedEnvelopeDeliveryStateResult {
            state: self
                .envelope_router
                .acknowledge(&request.target, &request.envelope_id),
        })
    }

    /// Return one routed envelope delivery state without changing daemon state.
    pub fn routed_envelope_delivery_state(
        &self,
        target: &EnvelopeTarget,
        envelope_id: &EnvelopeId,
    ) -> RoutedEnvelopeDeliveryStateResult {
        RoutedEnvelopeDeliveryStateResult {
            state: self
                .envelope_router
                .delivery_state(target, envelope_id)
                .cloned(),
        }
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
                let live_candidates = adoption_candidate_count(&self.engine, &record);
                let state = if matches!(
                    record.state,
                    RegistrySessionState::Stopping
                        | RegistrySessionState::Exited
                        | RegistrySessionState::Stale
                ) {
                    SessionAdoptionState::Terminal
                } else if live_candidates > 1 {
                    SessionAdoptionState::DuplicateWorker {
                        candidates: live_candidates,
                    }
                } else if record.protocol_version != botster_core::PROTOCOL_VERSION {
                    SessionAdoptionState::StaleWorker {
                        reason: SessionWorkerStaleReason::IncompatibleProtocol,
                    }
                } else if record.handshake_verified
                    && record.ping_pong_supported
                    && record.recovery_identity.is_some()
                    && self.engine.session(&record.session_id).is_none()
                    && !has_worker_control_socket(&record)
                {
                    SessionAdoptionState::StaleWorker {
                        reason: stale_worker_reason(&record),
                    }
                } else if record.handshake_verified
                    && record.recovery_identity.is_some()
                    && !record.ping_pong_supported
                {
                    SessionAdoptionState::UnhealthyWorker {
                        reason: SessionWorkerHealthReason::MissedHeartbeat,
                    }
                } else if record.handshake_verified
                    && record.ping_pong_supported
                    && record.recovery_identity.is_some()
                {
                    SessionAdoptionState::Adoptable
                } else {
                    SessionAdoptionState::MissingProtocolEvidence
                };
                SessionAdoptionReport { record, state }
            })
            .collect())
    }

    /// Explicitly mark a registry record stale after a read-only adoption scan.
    pub fn mark_stale(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        if let Some(mut record) = self.registry.load(session_id)? {
            record.mark(RegistrySessionState::Stale, now_seconds);
            self.registry.save(&record)?;
        }
        Ok(())
    }

    /// Adopt a live worker-backed session from durable registry metadata.
    pub fn adopt_session(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<CoreSession, CoreDaemonError> {
        self.ensure_running()?;
        let record = self
            .registry
            .load(session_id)?
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let process = record
            .process
            .clone()
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let socket_path = worker_control_socket(&record)
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let session = self.engine.adopt_worker_process(
            session_id.clone(),
            process,
            socket_path,
            botster_core::CoreSessionMetadata::new(),
        )?;
        if let Some(mut record) = self.registry.load(session_id)? {
            record.mark(RegistrySessionState::Running, now_seconds);
            self.registry.save(&record)?;
        }
        Ok(session)
    }

    /// Release worker processes for an intentional daemon restart without shutting them down.
    pub fn release_for_restart(&mut self) {
        self.engine.release_workers_for_restart();
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
            record.mark(RegistrySessionState::Exited, now_seconds);
            self.registry.save(&record)?;
        }
        Ok(())
    }

    fn reconcile_lifecycle_observations(
        &self,
        observations: &[BotsterEngineObservation],
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        for observation in observations {
            let BotsterEngineObservation::SessionLifecycle { session_id, state } = observation
            else {
                continue;
            };
            let registry_state = match state {
                botster_core::SessionLifecycleState::Stopping => RegistrySessionState::Stopping,
                botster_core::SessionLifecycleState::Exited { .. } => RegistrySessionState::Exited,
                botster_core::SessionLifecycleState::Failed { .. } => RegistrySessionState::Stale,
                botster_core::SessionLifecycleState::Starting
                | botster_core::SessionLifecycleState::Running => continue,
            };
            if let Some(mut record) = self.registry.load(session_id)? {
                record.mark(registry_state, now_seconds);
                self.registry.save(&record)?;
            }
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

impl DaemonEngine {
    fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        match self {
            Self::Local(engine) => engine.session(session_id),
            Self::Worker(engine) => engine.session(session_id),
        }
    }

    fn list_sessions(&self) -> Vec<CoreSession> {
        match self {
            Self::Local(engine) => engine.list_sessions(),
            Self::Worker(engine) => engine.list_sessions(),
        }
    }

    fn worker_metadata(&self, session_id: &SessionId) -> Option<&botster_core::SessionMetadata> {
        match self {
            Self::Local(_) => None,
            Self::Worker(engine) => engine.worker_metadata(session_id),
        }
    }

    fn spawn_session(
        &mut self,
        request: botster_core::SessionSpawnRequest,
        metadata: botster_core::CoreSessionMetadata,
    ) -> Result<botster_core::BotsterSpawnOutcome, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.spawn_session(request, metadata),
            Self::Worker(engine) => engine.spawn_session(request, metadata),
        }
    }

    fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => {
                engine.attach_client(client_id, session_id, subscription_id, now_seconds)
            }
            Self::Worker(engine) => {
                engine.attach_client(client_id, session_id, subscription_id, now_seconds)
            }
        }
    }

    fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => {
                engine.detach_client(client_id, session_id, subscription_id, now_seconds)
            }
            Self::Worker(engine) => {
                engine.detach_client(client_id, session_id, subscription_id, now_seconds)
            }
        }
    }

    fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: Vec<u8>,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.write_bytes(client_id, session_id, data, now_seconds),
            Self::Worker(engine) => engine.write_bytes(client_id, session_id, data, now_seconds),
        }
    }

    fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.resize(client_id, session_id, rows, cols, now_seconds),
            Self::Worker(engine) => engine.resize(client_id, session_id, rows, cols, now_seconds),
        }
    }

    fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.drain_runtime_once(session_id, last_output_at),
            Self::Worker(engine) => engine.drain_runtime_once(session_id, last_output_at),
        }
    }

    fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        let reason = reason.into();
        match self {
            Self::Local(engine) => engine.shutdown_session(session_id, reason, now_seconds),
            Self::Worker(engine) => engine.shutdown_session(session_id, reason, now_seconds),
        }
    }

    fn adopt_worker_process(
        &mut self,
        session_id: SessionId,
        process: botster_core::ProcessIdentity,
        socket_path: PathBuf,
        metadata: botster_core::CoreSessionMetadata,
    ) -> Result<CoreSession, DefaultBotsterEngineError> {
        match self {
            Self::Local(_) => Err(DefaultBotsterEngineError::Runtime(
                botster_core::SessionRuntimeError::new(
                    botster_core::SessionRuntimeErrorKind::SessionNotFound,
                    "local daemon engine cannot adopt worker process",
                ),
            )),
            Self::Worker(engine) => engine
                .adopt_worker_process(session_id, process, socket_path, metadata)
                .map(|outcome| outcome.session),
        }
    }

    fn release_workers_for_restart(&mut self) {
        if let Self::Worker(engine) = self {
            engine.release_workers_for_restart();
        }
    }
}

fn live_session_count(engine: &DaemonEngine, session_id: &SessionId) -> usize {
    engine
        .list_sessions()
        .into_iter()
        .filter(|session| &session.session_id == session_id)
        .count()
}

fn adoption_candidate_count(engine: &DaemonEngine, record: &RegistryRecord) -> usize {
    let live_candidates = live_session_count(engine, &record.session_id);
    let registry_candidate = usize::from(
        record.handshake_verified
            && record.recovery_identity.is_some()
            && record.protocol_version == botster_core::PROTOCOL_VERSION,
    );
    live_candidates.max(registry_candidate) + record.duplicate_worker_candidates
}

fn has_worker_control_socket(record: &RegistryRecord) -> bool {
    worker_control_socket(record).is_some()
}

fn worker_control_socket(record: &RegistryRecord) -> Option<PathBuf> {
    record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
}

fn worker_socket_dir(data_dir: &PathBuf) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    data_dir.hash(&mut hasher);
    std::env::temp_dir().join(format!("bcd-{:x}", hasher.finish()))
}

fn stale_worker_reason(record: &RegistryRecord) -> SessionWorkerStaleReason {
    if record.process.is_some() {
        SessionWorkerStaleReason::WorkerDied
    } else {
        SessionWorkerStaleReason::ProcessMissing
    }
}

//! Host-facing facade that assembles the reusable core multiplexer engines.

use std::collections::HashMap;

use thiserror::Error;

use crate::actor::{
    ClientControlFrame, MailboxSendFailureReason, PluginCleanupResult, PluginCleanupScope,
    PluginInvocationRequest, PluginKey, PluginReloadSpec, PluginTimerCancellationResult,
    PluginTimerId, PluginTimerSchedule, PluginUnloadSpec, QueueSource,
};
use crate::contract::actor::{
    BackpressureSummary, SessionIoEvent, SessionIoRequest, SessionLifecycleState,
};
use crate::contract::notification::{
    NotificationId, NotificationInbox, NotificationItem, NotificationTarget, NotificationTimestamp,
};
use crate::contract::transport::{TransportEgress, TransportIngress};
use crate::engine::plugin_timer::{
    PluginTimerDrainOutcome, PluginTimerScheduleOutcome, PluginTimerScheduler,
};
use crate::engine::plugin_worker::{
    PluginInvocationOutcome, PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerRegistration,
};
use crate::engine::session_activity::{apply_session_activity_event, classify_session_activity};
use crate::engine::session_worker::{
    SessionWorkerEngine, SessionWorkerOutcome, SessionWorkerRuntime, SessionWorkerRuntimeEvent,
};
use crate::engine::subscription_multiplexer::{
    SubscriptionMultiplexer, SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome,
};
use crate::runtime::{
    SessionRuntime, SessionRuntimeError, SessionRuntimeHandle, SessionSpawnRequest,
};
use crate::session::{
    CoreSession, CoreSessionMetadata, SessionActivityEvent, SessionActivityStatus, SessionId,
    SubscriptionId,
};
use crate::ClientId;

/// Facade-level error for assembled engine operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MultiplexerEngineError {
    /// A session id was already registered in this engine.
    #[error("session already exists: {session_id:?}")]
    SessionAlreadyExists {
        /// Duplicate session id.
        session_id: SessionId,
    },
    /// A request targeted a session unknown to this engine.
    #[error("unknown session: {session_id:?}")]
    UnknownSession {
        /// Missing session id.
        session_id: SessionId,
    },
    /// Host metadata exceeded the public core metadata cap.
    #[error("core session metadata exceeds encoded length limit")]
    MetadataTooLarge,
    /// Host session runtime returned an error.
    #[error(transparent)]
    Runtime(#[from] SessionRuntimeError),
}

/// Observable state change emitted by the assembled engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiplexerEngineObservation {
    /// A session lifecycle state changed.
    SessionLifecycle {
        /// Session whose lifecycle changed.
        session_id: SessionId,
        /// New lifecycle state.
        state: SessionLifecycleState,
    },
    /// A session activity classification was observed.
    SessionActivity {
        /// Session whose activity was classified.
        session_id: SessionId,
        /// Activity status at the caller-provided clock value.
        status: SessionActivityStatus,
    },
    /// A lower-level subscription multiplexer observation was emitted.
    Subscription(SubscriptionMultiplexerObservation),
    /// Runtime-originated bounded-queue pressure was observed.
    Backpressure(BackpressureSummary),
}

/// Result of a successful session spawn through the assembled engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiplexerSpawnOutcome {
    /// Runtime handle returned by the host session runtime.
    pub handle: SessionRuntimeHandle,
    /// Core session state recorded by the engine.
    pub session: CoreSession,
    /// Observations emitted during spawn.
    pub observations: Vec<MultiplexerEngineObservation>,
}

/// Accumulated output from one assembled engine operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiplexerEngineOutcome {
    /// Frames emitted to client transports, paired with the receiving client.
    pub client_egress: Vec<(ClientId, TransportEgress)>,
    /// Requests routed to session workers, paired with the target session.
    pub session_requests: Vec<(SessionId, SessionIoRequest)>,
    /// Client-side control frames, paired with the receiving client.
    pub client_control_frames: Vec<(ClientId, ClientControlFrame)>,
    /// Session I/O events emitted by workers.
    pub session_events: Vec<SessionIoEvent>,
    /// Engine and lower-level observations emitted by this operation.
    pub observations: Vec<MultiplexerEngineObservation>,
}

impl MultiplexerEngineOutcome {
    /// Build an empty outcome.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            client_egress: Vec::new(),
            session_requests: Vec::new(),
            client_control_frames: Vec::new(),
            session_events: Vec::new(),
            observations: Vec::new(),
        }
    }

    fn append_multiplexer(&mut self, outcome: SubscriptionMultiplexerOutcome) {
        self.client_egress.extend(outcome.client_egress);
        self.session_requests.extend(outcome.session_requests);
        self.client_control_frames
            .extend(outcome.client_control_frames);
        self.observations.extend(
            outcome
                .observations
                .into_iter()
                .map(MultiplexerEngineObservation::Subscription),
        );
    }

    fn append_worker(&mut self, outcome: SessionWorkerOutcome) {
        self.session_events.extend(outcome.events);
    }
}

/// Synchronous embeddable core multiplexer facade.
///
/// Hosts provide concrete runtime adapters and policy-resolved requests. The
/// facade coordinates core state machines and returns typed outcomes without
/// performing transport writes, persistence, auth, or product policy.
#[derive(Clone)]
pub struct MultiplexerEngine<R, W> {
    session_runtime: R,
    sessions: HashMap<SessionId, CoreSession>,
    session_handles: HashMap<SessionId, SessionRuntimeHandle>,
    session_workers: HashMap<SessionId, SessionWorkerEngine<W>>,
    subscriptions: SubscriptionMultiplexer,
    notifications: NotificationInbox,
    plugins: PluginWorkerEngine,
    timers: PluginTimerScheduler,
}

impl<R, W> MultiplexerEngine<R, W>
where
    R: SessionRuntime,
    W: SessionWorkerRuntime,
{
    /// Build an engine with a host session runtime and default plugin settings.
    pub fn new(session_runtime: R) -> Self {
        Self::with_plugin_config(session_runtime, PluginWorkerEngineConfig::default())
    }

    /// Build an engine with explicit plugin worker settings.
    pub fn with_plugin_config(session_runtime: R, plugin_config: PluginWorkerEngineConfig) -> Self {
        Self {
            session_runtime,
            sessions: HashMap::new(),
            session_handles: HashMap::new(),
            session_workers: HashMap::new(),
            subscriptions: SubscriptionMultiplexer::new(),
            notifications: NotificationInbox::new(),
            plugins: PluginWorkerEngine::with_config(plugin_config),
            timers: PluginTimerScheduler::new(),
        }
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.sessions.get(session_id)
    }

    /// Return known session ids.
    #[must_use]
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().cloned().collect()
    }

    /// Return sessions currently recorded by the assembled engine.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.sessions.values().cloned().collect()
    }

    /// Return the host runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &R {
        &self.session_runtime
    }

    /// Return a mutable host runtime adapter.
    pub const fn session_runtime_mut(&mut self) -> &mut R {
        &mut self.session_runtime
    }

    /// Return a mutable session worker runtime adapter.
    pub fn session_worker_runtime_mut(&mut self, session_id: &SessionId) -> Option<&mut W> {
        self.session_workers
            .get_mut(session_id)
            .map(SessionWorkerEngine::runtime_mut)
    }

    /// Return the plugin worker engine.
    ///
    /// Hosts that need reload or unload cleanup should call
    /// [`Self::reload_plugin`] or [`Self::unload_plugin`] so scheduler-owned
    /// timer resources are cleaned with worker-owned resources.
    #[must_use]
    pub const fn plugin_workers(&self) -> &PluginWorkerEngine {
        &self.plugins
    }

    /// Return the plugin timer scheduler.
    #[must_use]
    pub const fn plugin_timers(&self) -> &PluginTimerScheduler {
        &self.timers
    }

    /// Spawn a session, record core state, and install its worker adapter.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
        worker_runtime: W,
    ) -> Result<MultiplexerSpawnOutcome, MultiplexerEngineError> {
        if self.sessions.contains_key(&request.session_id) {
            return Err(MultiplexerEngineError::SessionAlreadyExists {
                session_id: request.session_id,
            });
        }
        if !metadata.is_within_encoded_len_limit() {
            return Err(MultiplexerEngineError::MetadataTooLarge);
        }

        let handle = self.session_runtime.spawn_session(request)?;
        let mut session = CoreSession::with_metadata(
            handle.session_id.clone(),
            SessionLifecycleState::Starting,
            metadata,
        );
        apply_session_activity_event(
            &mut session,
            SessionActivityEvent::Lifecycle {
                state: SessionLifecycleState::Running,
            },
        );

        self.session_handles
            .insert(handle.session_id.clone(), handle.clone());
        self.session_workers.insert(
            handle.session_id.clone(),
            SessionWorkerEngine::new(worker_runtime),
        );
        self.sessions
            .insert(handle.session_id.clone(), session.clone());

        Ok(MultiplexerSpawnOutcome {
            handle,
            session: session.clone(),
            observations: vec![MultiplexerEngineObservation::SessionLifecycle {
                session_id: session.session_id,
                state: SessionLifecycleState::Running,
            }],
        })
    }

    /// Route one client ingress frame through subscriptions and session workers.
    pub fn handle_client_ingress(
        &mut self,
        client_id: ClientId,
        ingress: TransportIngress,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        if let Some(session_id) = ingress_session_id(&ingress) {
            self.ensure_session(&session_id)?;
        }

        let multiplexer_outcome = self.subscriptions.handle_client_ingress(client_id, ingress);
        let session_requests = multiplexer_outcome.session_requests.clone();
        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_multiplexer(multiplexer_outcome);

        for (session_id, request) in session_requests {
            let worker_was_closed = self.session_worker_is_closed(&session_id)?;
            let worker_outcome = self.handle_session_request_inner(
                session_id.clone(),
                request.clone(),
                now_seconds,
            )?;
            outcome.append_worker(worker_outcome.clone());
            self.route_worker_events(worker_outcome, &mut outcome)?;
            if !worker_was_closed {
                self.apply_request_activity(&request, now_seconds)?;
            }
        }

        Ok(outcome)
    }

    /// Route one session worker request directly through the assembled engine.
    pub fn handle_session_request(
        &mut self,
        request: SessionIoRequest,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        let session_id = request_session_id(&request);
        let worker_was_closed = self.session_worker_is_closed(&session_id)?;
        let worker_outcome =
            self.handle_session_request_inner(session_id, request.clone(), now_seconds)?;
        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_worker(worker_outcome.clone());
        self.route_worker_events(worker_outcome, &mut outcome)?;
        if !worker_was_closed {
            self.apply_request_activity(&request, now_seconds)?;
        }
        Ok(outcome)
    }

    /// Route one runtime-originated session worker event.
    pub fn handle_runtime_event(
        &mut self,
        event: SessionWorkerRuntimeEvent,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        let session_id = runtime_event_session_id(&event);
        self.ensure_session(&session_id)?;
        let worker_was_closed = self.session_worker_is_closed(&session_id)?;

        let worker = self.session_workers.get_mut(&session_id).ok_or_else(|| {
            MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        let worker_outcome = worker.handle_runtime_event(event);

        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_worker(worker_outcome.clone());
        if !worker_was_closed {
            self.route_worker_events(worker_outcome, &mut outcome)?;
        }
        Ok(outcome)
    }

    /// Report client-side backpressure through the public facade.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        self.ensure_session(&session_id)?;
        let multiplexer_outcome = self
            .subscriptions
            .report_backpressure(client_id, session_id, source, capacity, depth);
        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_multiplexer(multiplexer_outcome);
        Ok(outcome)
    }

    /// Report accepted-but-slow delivery through the public facade.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        self.ensure_session(&session_id)?;
        let multiplexer_outcome = self.subscriptions.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        );
        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_multiplexer(multiplexer_outcome);
        Ok(outcome)
    }

    /// Report a failed delivery attempt through the public facade.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        self.ensure_session(&session_id)?;
        let multiplexer_outcome = self.subscriptions.report_delivery_failure(
            client_id,
            session_id,
            subscription_id,
            source,
            reason,
        );
        let mut outcome = MultiplexerEngineOutcome::empty();
        outcome.append_multiplexer(multiplexer_outcome);
        Ok(outcome)
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        self.plugins.load_plugin(registration);
    }

    /// Reload one plugin and cleanup scheduler-owned timer resources for it.
    pub fn reload_plugin(
        &self,
        spec: PluginReloadSpec,
        registration: PluginWorkerRegistration,
    ) -> PluginCleanupResult {
        let timer_cleanup = if cleanup_removes_resources(&spec.cleanup) {
            self.timers
                .cleanup_plugin(spec.request_id.clone(), &spec.plugin_key)
        } else {
            empty_cleanup(spec.request_id.clone(), spec.plugin_key.clone())
        };
        let worker_cleanup = self.plugins.reload_plugin(spec, registration);
        merge_cleanup(worker_cleanup, timer_cleanup)
    }

    /// Unload one plugin and cleanup scheduler-owned timer resources for it.
    pub fn unload_plugin(&self, spec: PluginUnloadSpec) -> PluginCleanupResult {
        let timer_cleanup = if cleanup_removes_resources(&spec.cleanup) {
            self.timers
                .cleanup_plugin(spec.request_id.clone(), &spec.plugin_key)
        } else {
            empty_cleanup(spec.request_id.clone(), spec.plugin_key.clone())
        };
        let worker_cleanup = self.plugins.unload_plugin(spec);
        merge_cleanup(worker_cleanup, timer_cleanup)
    }

    /// Invoke a registered plugin handler.
    pub fn invoke_plugin(&self, request: PluginInvocationRequest) -> PluginInvocationOutcome {
        self.plugins.invoke(request)
    }

    /// Schedule plugin timer work without invoking plugin code inline.
    pub fn schedule_plugin_timer(
        &self,
        schedule: PluginTimerSchedule,
    ) -> PluginTimerScheduleOutcome {
        self.timers.schedule(schedule)
    }

    /// Cancel one plugin timer by handle.
    pub fn cancel_plugin_timer(
        &self,
        request_id: crate::RequestId,
        plugin_key: &PluginKey,
        timer_id: &PluginTimerId,
    ) -> PluginTimerCancellationResult {
        self.timers.cancel(request_id, plugin_key, timer_id)
    }

    /// Drain due plugin timers through the existing plugin worker engine.
    pub fn drain_plugin_timers_due(&self, now_ms: u64) -> PluginTimerDrainOutcome {
        self.timers.drain_due(now_ms, &self.plugins)
    }

    /// Queue a notification item in the core inbox.
    pub fn post_notification(&mut self, item: NotificationItem) -> NotificationId {
        self.notifications.post(item)
    }

    /// Drain deliverable notifications for one target.
    pub fn drain_notifications(
        &mut self,
        target: NotificationTarget,
        now: NotificationTimestamp,
    ) -> Vec<NotificationItem> {
        self.notifications.drain(&target, now)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_session_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, MultiplexerEngineError> {
        let session = self.ensure_session(session_id)?;
        Ok(classify_session_activity(
            &session.activity,
            now_seconds,
            active_threshold_seconds,
        ))
    }

    /// Shut down one session worker and update core lifecycle state.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, MultiplexerEngineError> {
        let reason = reason.into();
        let request = SessionIoRequest::Shutdown {
            session_id: session_id.clone(),
            reason,
        };
        let mut outcome = self.handle_session_request(request, now_seconds)?;
        if !outcome
            .session_events
            .iter()
            .any(|event| matches!(event, SessionIoEvent::ProcessExited { .. }))
        {
            self.apply_lifecycle(session_id.clone(), SessionLifecycleState::Stopping)?;
            outcome
                .observations
                .push(MultiplexerEngineObservation::SessionLifecycle {
                    session_id,
                    state: SessionLifecycleState::Stopping,
                });
        }
        Ok(outcome)
    }

    fn handle_session_request_inner(
        &mut self,
        session_id: SessionId,
        request: SessionIoRequest,
        _now_seconds: u64,
    ) -> Result<SessionWorkerOutcome, MultiplexerEngineError> {
        self.ensure_session(&session_id)?;
        let worker = self.session_workers.get_mut(&session_id).ok_or_else(|| {
            MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        Ok(worker.handle_request(request)?)
    }

    fn session_worker_is_closed(
        &self,
        session_id: &SessionId,
    ) -> Result<bool, MultiplexerEngineError> {
        let worker = self.session_workers.get(session_id).ok_or_else(|| {
            MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        Ok(worker.is_closed())
    }

    fn route_worker_events(
        &mut self,
        worker_outcome: SessionWorkerOutcome,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), MultiplexerEngineError> {
        let last_output_at = worker_outcome.last_output_at;
        for event in worker_outcome.events {
            if let (Some(at), SessionIoEvent::TerminalBytes { session_id, data }) =
                (last_output_at, &event)
            {
                self.apply_activity(
                    session_id.clone(),
                    SessionActivityEvent::OutputBytes {
                        at,
                        bytes: data.len() as u64,
                    },
                )?;
            }
            self.apply_session_event_activity(&event)?;
            outcome.append_multiplexer(self.subscriptions.handle_session_event(event));
        }
        Ok(())
    }

    fn apply_request_activity(
        &mut self,
        request: &SessionIoRequest,
        now_seconds: u64,
    ) -> Result<(), MultiplexerEngineError> {
        if let SessionIoRequest::PtyInput { session_id, data } = request {
            self.apply_activity(
                session_id.clone(),
                SessionActivityEvent::InputBytes {
                    at: now_seconds,
                    bytes: data.len() as u64,
                },
            )?;
        }
        Ok(())
    }

    fn apply_session_event_activity(
        &mut self,
        event: &SessionIoEvent,
    ) -> Result<(), MultiplexerEngineError> {
        match event {
            SessionIoEvent::ProcessExited {
                session_id,
                payload,
            } => self.apply_lifecycle(
                session_id.clone(),
                SessionLifecycleState::Exited {
                    code: payload.exit_code,
                },
            ),
            SessionIoEvent::Shutdown { session_id, .. } => {
                match self.session(session_id).map(|session| &session.lifecycle) {
                    Some(SessionLifecycleState::Exited { .. }) => Ok(()),
                    _ => self.apply_lifecycle(session_id.clone(), SessionLifecycleState::Stopping),
                }
            }
            SessionIoEvent::TerminalBytes { .. }
            | SessionIoEvent::InitialSnapshotReady(_)
            | SessionIoEvent::SnapshotReady(_)
            | SessionIoEvent::SendFileWritten(_)
            | SessionIoEvent::SendFileFailed(_)
            | SessionIoEvent::PreparedSnapshotReady(_)
            | SessionIoEvent::ModeFlagsReady(_)
            | SessionIoEvent::ScreenReady(_)
            | SessionIoEvent::PromptMark { .. }
            | SessionIoEvent::Bell { .. }
            | SessionIoEvent::Notification { .. } => Ok(()),
        }
    }

    fn apply_activity(
        &mut self,
        session_id: SessionId,
        event: SessionActivityEvent,
    ) -> Result<(), MultiplexerEngineError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or(MultiplexerEngineError::UnknownSession { session_id })?;
        apply_session_activity_event(session, event);
        Ok(())
    }

    fn apply_lifecycle(
        &mut self,
        session_id: SessionId,
        state: SessionLifecycleState,
    ) -> Result<(), MultiplexerEngineError> {
        self.apply_activity(session_id, SessionActivityEvent::Lifecycle { state })
    }

    fn ensure_session(
        &self,
        session_id: &SessionId,
    ) -> Result<&CoreSession, MultiplexerEngineError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })
    }
}

impl<R, W> Default for MultiplexerEngine<R, W>
where
    R: SessionRuntime + Default,
    W: SessionWorkerRuntime,
{
    fn default() -> Self {
        Self::new(R::default())
    }
}

fn cleanup_removes_resources(scope: &PluginCleanupScope) -> bool {
    matches!(
        scope,
        PluginCleanupScope::Resources | PluginCleanupScope::DescriptorsAndResources
    )
}

fn empty_cleanup(request_id: crate::RequestId, plugin_key: PluginKey) -> PluginCleanupResult {
    PluginCleanupResult {
        request_id,
        plugin_key,
        removed_descriptors: Vec::new(),
        removed_resources: Vec::new(),
    }
}

fn merge_cleanup(
    mut worker_cleanup: PluginCleanupResult,
    timer_cleanup: PluginCleanupResult,
) -> PluginCleanupResult {
    for resource in timer_cleanup.removed_resources {
        if !worker_cleanup.removed_resources.contains(&resource) {
            worker_cleanup.removed_resources.push(resource);
        }
    }
    worker_cleanup
}

fn ingress_session_id(ingress: &TransportIngress) -> Option<SessionId> {
    match ingress {
        TransportIngress::SubscribeSession { session_id, .. }
        | TransportIngress::UnsubscribeSession { session_id, .. }
        | TransportIngress::TerminalInput { session_id, .. }
        | TransportIngress::Resize { session_id, .. }
        | TransportIngress::RequestSnapshot { session_id, .. }
        | TransportIngress::SendFile { session_id, .. }
        | TransportIngress::Focus { session_id, .. } => Some(session_id.clone()),
        TransportIngress::Heartbeat { .. }
        | TransportIngress::BoundaryPayload { .. }
        | TransportIngress::ClientState { .. }
        | TransportIngress::Ping { .. } => None,
    }
}

fn runtime_event_session_id(event: &SessionWorkerRuntimeEvent) -> SessionId {
    match event {
        SessionWorkerRuntimeEvent::TerminalBytes { session_id, .. }
        | SessionWorkerRuntimeEvent::ProcessExited { session_id, .. } => session_id.clone(),
        SessionWorkerRuntimeEvent::InitialSnapshotReady(snapshot) => snapshot.session_id.clone(),
    }
}

fn request_session_id(request: &SessionIoRequest) -> SessionId {
    match request {
        SessionIoRequest::SubscribeTerminal { session_id, .. }
        | SessionIoRequest::UnsubscribeTerminal { session_id, .. }
        | SessionIoRequest::PtyInput { session_id, .. }
        | SessionIoRequest::Resize { session_id, .. }
        | SessionIoRequest::GetSnapshot { session_id, .. }
        | SessionIoRequest::GetModeFlags { session_id, .. }
        | SessionIoRequest::GetScreen { session_id, .. }
        | SessionIoRequest::SetColorProfile { session_id, .. }
        | SessionIoRequest::Shutdown { session_id, .. } => session_id.clone(),
        SessionIoRequest::GetInitialSnapshot(request) => request.session_id.clone(),
        SessionIoRequest::SendFile(request) => request.session_id.clone(),
        SessionIoRequest::PrepareSnapshot(request) => request.session_id.clone(),
    }
}

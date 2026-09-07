//! Ergonomic embeddable Botster engine facade.

#[cfg(feature = "local-runtime")]
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

#[cfg(feature = "local-runtime")]
use botster_terminal_protocol::{
    encode_history_unavailable, encode_modes, encode_snapshot_finish, encode_snapshot_history,
    encode_snapshot_ready, AttachStateCode, HistoryUnavailableReason, InputOutcome, ModesBody,
};

use crate::actor::{
    MailboxSendFailureReason, PluginAdmissionResult, PluginCleanupResult, PluginCompletionDrain,
    PluginInvocationClass, PluginInvocationRequest, PluginKey, PluginReloadSpec,
    PluginTimerCancellationResult, PluginTimerId, PluginTimerSchedule, PluginUnloadSpec,
    PreparedSnapshotRequest, QueueSource,
};
use crate::contract::notification::{
    NotificationId, NotificationItem, NotificationTarget, NotificationTimestamp,
};
#[cfg(feature = "local-runtime")]
use crate::contract::terminal_screen::TerminalSnapshotFramePhase;
use crate::contract::terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalCapabilitySet,
    TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::contract::transport::{TransportEgress, TransportIngress};
#[cfg(feature = "local-runtime")]
use crate::engine::client_worker::{CaptureIdentity, EnqueueRouteFrameError};
#[cfg(feature = "local-runtime")]
use crate::engine::command::DefaultEngineCommand;
use crate::engine::command::{
    EngineCommand, EngineCommandError, EngineCommandOutcome, EngineSessionInspection,
};
#[cfg(feature = "local-runtime")]
use crate::engine::managed_session_runtime::{ManagedSessionRuntime, ManagedSessionRuntimeError};
use crate::engine::multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
use crate::engine::plugin_timer::{
    PluginTimerDrainOutcome, PluginTimerScheduleOutcome, PluginTimerScheduler,
};
use crate::engine::plugin_worker::{
    PluginInvocationOutcome, PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerRegistration,
};
use crate::engine::session_worker::{SessionWorkerRuntime, SessionWorkerRuntimeEvent};
#[cfg(feature = "local-runtime")]
use crate::engine::terminal_screen::{NullTerminalScreenRuntime, TerminalScreenRuntime};
#[cfg(feature = "local-runtime")]
use crate::runtime::ProcessIdentity;
#[cfg(feature = "local-runtime")]
use crate::runtime::{
    LocalProcessRuntime, RetainedWorkerFinalState, SnapshotCancelAdmission, WorkerProcessRuntime,
    WorkerProcessRuntimeOptions, WorkerSpawnPoll,
};
use crate::runtime::{SessionRuntime, SessionSpawnRequest};
use crate::session::{CoreSession, CoreSessionMetadata, SessionActivityStatus, SessionId};
#[cfg(feature = "local-runtime")]
use crate::terminal_screen::TerminalScreenSize;
#[cfg(feature = "local-runtime")]
use crate::terminal_screen::TerminalSnapshotPayload;
use crate::{ClientId, SubscriptionId};
#[cfg(feature = "local-runtime")]
use crate::{ModeFlagsPayload, ScreenPayload, SessionMetadata};

/// Facade-level error for ergonomic Botster engine operations.
pub type BotsterEngineError = MultiplexerEngineError;

/// Observable state change emitted by the ergonomic Botster engine.
pub type BotsterEngineObservation = MultiplexerEngineObservation;

/// Result of a successful session spawn through the ergonomic Botster engine.
pub type BotsterSpawnOutcome = MultiplexerSpawnOutcome;

/// Accumulated output from one ergonomic Botster engine operation.
pub type BotsterEngineOutput = MultiplexerEngineOutcome;

/// Default local PTY-backed engine error.
#[cfg(feature = "local-runtime")]
pub type DefaultBotsterEngineError = ManagedSessionRuntimeError;

/// Worker-backed local PTY engine error.
#[cfg(feature = "local-runtime")]
pub type WorkerBackedBotsterEngineError = ManagedSessionRuntimeError;

/// Public default local PTY-backed Botster engine facade.
///
/// This is the policy-free default path for embedders that want to run a real
/// local process without supplying custom runtime adapters. Hosts still provide
/// explicit spawn requests; the facade only wires the local process runtime
/// through the managed session worker and subscription fanout path.
#[cfg(feature = "local-runtime")]
pub struct DefaultBotsterEngine {
    runtime: ManagedSessionRuntime<LocalProcessRuntime, Box<dyn TerminalScreenRuntime>>,
}

/// Public local PTY-backed engine facade whose live PTY is owned by a worker process.
///
/// The worker Ghostty is the only terminal parser for these sessions. This
/// engine keeps no parent terminal shadow: readbacks are worker probes and
/// attach captures are worker snapshot boundaries streamed to bound routes.
#[cfg(feature = "local-runtime")]
pub struct WorkerBackedBotsterEngine {
    runtime: ManagedSessionRuntime<WorkerProcessRuntime, NullTerminalScreenRuntime>,
    /// At most one worker capture per session at a time.
    captures: HashMap<SessionId, RouteCapture>,
    /// Routes waiting for the active capture to finish.
    capture_queue: HashMap<SessionId, VecDeque<CaptureRequest>>,
    next_host_capture: u64,
    /// Finished host captures awaiting `take_host_capture`.
    host_captures: HashMap<u64, Result<HostCaptureResult, String>>,
    /// Worker barrier cancels the control queue has not accepted yet. At
    /// most one per session: the runtime holds at most one outstanding
    /// barrier request, and a cancel exists only while that request does.
    pending_barrier_cancels: HashMap<SessionId, PendingBarrierCancel>,
}

/// How long one refused barrier cancel may stay pending.
///
/// The control writer gives every queued frame `WORKER_CONTROL_WRITE_TIMEOUT`
/// and fails the control plane itself when a write exceeds it. A queue that
/// stays full for longer than every queued frame could legitimately take is
/// a worker that does not consume control frames without the writer noticing;
/// the engine then fails the control plane explicitly. Retries themselves are
/// driven by session wakes, never by this clock, so fast unrelated turns
/// cannot shorten the bound.
#[cfg(feature = "local-runtime")]
const BARRIER_CANCEL_RETRY_BOUND: Duration = Duration::from_secs(
    crate::runtime::WORKER_CONTROL_WRITE_TIMEOUT.as_secs()
        * crate::runtime::WORKER_CONTROL_QUEUE_FRAMES as u64,
);

/// A snapshot barrier cancel that the worker control queue refused.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBarrierCancel {
    /// Process-unique barrier request id; it names one worker incarnation.
    request_id: String,
    /// Monotonic time of the first refusal.
    first_refused: Instant,
}

/// Outcome of one barrier cancel attempt.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarrierCancelStep {
    /// Nothing is owed for the request any more.
    Done,
    /// The request is still outstanding and the cancel was not queued.
    Retry,
}

/// Why one route needs a worker capture.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureKind {
    /// A new attach: `ATTACH_STATE attached` precedes `MODES` and `SNAPSHOT_READY`.
    Attach,
    /// Egress overflow recovery: `ROUTE_RESYNC` already went out under the new epoch.
    Resync,
    /// Host readback: bytes are collected for the daemon, no route is written.
    Host(u64),
}

/// Result of one host-owned worker capture.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCaptureResult {
    /// Complete GHOSTSNP bytes in stream order.
    pub bytes: Vec<u8>,
    /// Terminal size represented by the snapshot.
    pub size: TerminalScreenSize,
    /// Ghostty colors frozen with the snapshot.
    pub color_profile: crate::TerminalColorProfile,
}

#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureRequest {
    client_id: ClientId,
    subscription_id: SubscriptionId,
    kind: CaptureKind,
    /// Identity captured when the request was created. Host captures carry
    /// `None`; route captures carry the attachment generation and fence.
    identity: Option<CaptureIdentity>,
}

#[cfg(feature = "local-runtime")]
struct RouteCapture {
    client_id: ClientId,
    subscription_id: SubscriptionId,
    kind: CaptureKind,
    request_id: String,
    /// Identity from the request that created this capture. Pages are
    /// enqueued only while the route still carries exactly this identity.
    identity: Option<CaptureIdentity>,
    ready: bool,
    /// `FINISH` or an error ended the pages; the barrier release is pending.
    awaiting_release: bool,
    history_incomplete: bool,
    /// Host capture accumulation.
    collected: Vec<u8>,
    collected_size: Option<TerminalScreenSize>,
    collected_colors: Option<crate::TerminalColorProfile>,
}

#[cfg(feature = "local-runtime")]
fn runtime_with_plain_terminal_backend<R>(
    runtime: R,
) -> ManagedSessionRuntime<R, Box<dyn TerminalScreenRuntime>>
where
    R: SessionRuntime,
{
    ManagedSessionRuntime::with_terminal_backend_factory(runtime, |size| {
        Ok::<_, std::convert::Infallible>(Box::new(
            crate::engine::terminal_screen::PlainTerminalScreenRuntime::new(size),
        ) as Box<dyn TerminalScreenRuntime>)
    })
}

#[cfg(feature = "local-runtime")]
fn runtime_with_boxed_terminal_backend<E, T, F, R>(
    runtime: R,
    factory: F,
) -> ManagedSessionRuntime<R, Box<dyn TerminalScreenRuntime>>
where
    E: std::error::Error + Send + Sync + 'static,
    T: TerminalScreenRuntime + 'static,
    F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    R: SessionRuntime,
{
    ManagedSessionRuntime::with_terminal_backend_factory(runtime, move |size| {
        factory(size).map(|terminal| Box::new(terminal) as Box<dyn TerminalScreenRuntime>)
    })
}

#[cfg(feature = "local-runtime")]
fn runtime_without_terminal_shadow(
    runtime: WorkerProcessRuntime,
) -> ManagedSessionRuntime<WorkerProcessRuntime, NullTerminalScreenRuntime> {
    ManagedSessionRuntime::with_terminal_backend_factory(runtime, |size| {
        Ok::<_, std::convert::Infallible>(NullTerminalScreenRuntime::new(size))
    })
}

#[cfg(feature = "local-runtime")]
impl DefaultBotsterEngine {
    /// Build an empty local PTY-backed engine.
    #[must_use]
    pub fn new() -> Self {
        let wakes = TerminalWakeSource::new();
        Self {
            runtime: runtime_with_plain_terminal_backend(
                LocalProcessRuntime::new().with_wake_source(wakes.clone()),
            )
            .with_shared_wake_source(wakes),
        }
    }

    /// Build an empty local PTY-backed engine with explicit runtime options.
    #[must_use]
    pub fn with_local_options(options: crate::runtime::LocalProcessRuntimeOptions) -> Self {
        let wakes = TerminalWakeSource::new();
        Self {
            runtime: runtime_with_plain_terminal_backend(
                LocalProcessRuntime::with_options(options).with_wake_source(wakes.clone()),
            )
            .with_shared_wake_source(wakes),
        }
    }

    /// Build an empty local PTY-backed engine with a host-supplied terminal backend.
    ///
    /// This keeps the facade monomorphic while letting first-party host
    /// profiles install a concrete terminal parser/snapshot backend.
    pub fn with_terminal_backend_factory<E, T, F>(factory: F) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
        T: TerminalScreenRuntime + 'static,
        F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    {
        let wakes = TerminalWakeSource::new();
        Self {
            runtime: runtime_with_boxed_terminal_backend(
                LocalProcessRuntime::new().with_wake_source(wakes.clone()),
                factory,
            )
            .with_shared_wake_source(wakes),
        }
    }

    /// Build an empty local engine whose sessions are owned by worker processes.
    #[must_use]
    pub fn worker_backed(worker_path: impl Into<std::path::PathBuf>) -> WorkerBackedBotsterEngine {
        WorkerBackedBotsterEngine::new(worker_path)
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.runtime.session(session_id)
    }

    /// Return sessions currently recorded by the local command facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.runtime.list_sessions()
    }

    /// Forget all local engine state for one terminal session.
    pub fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        self.runtime.forget_terminal_session(session_id)
    }

    /// Return the local process runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &LocalProcessRuntime {
        self.runtime.session_runtime()
    }

    /// Spawn a local PTY-backed session with an explicit host-owned request.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, DefaultBotsterEngineError> {
        self.runtime.spawn_session(request, metadata)
    }

    /// Execute one typed command through the default local engine facade.
    pub fn execute_command(
        &mut self,
        command: DefaultEngineCommand,
    ) -> Result<EngineCommandOutcome, EngineCommandError<DefaultBotsterEngineError>> {
        let kind = command.kind();
        match command {
            DefaultEngineCommand::SpawnSession { request, metadata } => self
                .spawn_session(request, metadata)
                .map(EngineCommandOutcome::SpawnSession),
            DefaultEngineCommand::AttachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .attach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::DetachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .detach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::SendInput {
                client_id,
                session_id,
                data,
                now_seconds,
            } => self
                .write_bytes(client_id, session_id, data, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::Resize {
                client_id,
                session_id,
                rows,
                cols,
                now_seconds,
            } => self
                .resize(client_id, session_id, rows, cols, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::ListSessions => {
                Ok(EngineCommandOutcome::Sessions(self.list_sessions()))
            }
            DefaultEngineCommand::InspectSession {
                session_id,
                now_seconds,
                active_threshold_seconds,
            } => self
                .inspect_session(&session_id, now_seconds, active_threshold_seconds)
                .map(EngineCommandOutcome::Inspection),
            DefaultEngineCommand::ReadScreen {
                request_id,
                session_id,
                now_seconds,
            } => self
                .read_screen(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::CaptureSnapshot {
                request_id,
                session_id,
                now_seconds,
            } => self
                .capture_snapshot(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::ReplaySnapshot {
                request,
                now_seconds,
            } => self
                .replay_snapshot(request, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::Shutdown {
                session_id,
                reason,
                now_seconds,
            } => self
                .shutdown_session(session_id, reason, now_seconds)
                .map(EngineCommandOutcome::Output),
        }
        .map_err(|source| EngineCommandError::new(kind, source))
    }

    /// Attach a client to a session stream.
    ///
    /// Unbound consumers receive the harness attach frames through the drain
    /// path. A bound or pre-bind held route receives the scheme 2 sequence
    /// directly: `ATTACH_STATE attached`, `MODES`, `SNAPSHOT_READY`, history
    /// pages, `SNAPSHOT_FINISH`. A backend without a streaming exporter fails
    /// the bound route with `ATTACH_STATE failed`.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        let mut output = self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id: client_id.clone(),
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            },
            now_seconds,
        )?;
        let initial_snapshot = self.runtime.drain_runtime_once(&session_id, now_seconds)?;
        append_engine_output(&mut output, initial_snapshot);
        if self
            .runtime
            .client_worker()
            .route_is_bound(&client_id, &session_id, &subscription_id)
        {
            self.push_local_capture(&session_id, &subscription_id)?;
        }
        Ok(output)
    }

    fn push_local_capture(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Result<(), DefaultBotsterEngineError> {
        let frames = match self.runtime.capture_local_snapshot_frames(session_id) {
            Ok(frames) => frames,
            Err(error) => {
                let _ = self
                    .runtime
                    .client_worker_mut()
                    .fail_route(session_id, subscription_id);
                return Err(error);
            }
        };
        let modes = self
            .runtime
            .client_worker()
            .session_modes(session_id)
            .unwrap_or_default();
        let worker = self.runtime.client_worker_mut();
        let mut sequence = Vec::new();
        sequence.push(botster_terminal_protocol::encode_attach_state(
            AttachStateCode::Attached,
        ));
        sequence.push(encode_modes(modes));
        for (phase, bytes) in frames {
            sequence.push(match phase {
                TerminalSnapshotFramePhase::Ready => encode_snapshot_ready(&bytes),
                TerminalSnapshotFramePhase::History | TerminalSnapshotFramePhase::Finish => {
                    encode_snapshot_history(&bytes)
                }
            });
        }
        sequence.push(encode_snapshot_finish());
        for frame in sequence {
            let Ok(frame) = frame else {
                let _ = worker.fail_route(session_id, subscription_id);
                return Ok(());
            };
            match worker.push_route_frame(session_id, subscription_id, frame) {
                Ok(None) => {}
                Ok(Some(_)) | Err(_) => return Ok(()),
            }
        }
        Ok(())
    }

    /// Record that the next attach for this identity will bind an adapter.
    pub fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.runtime
            .expect_terminal_adapter(client_id, session_id, subscription_id);
    }

    /// Retire an unconsumed pre-attach adapter declaration.
    pub fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.runtime
            .cancel_expected_terminal_adapter(client_id, session_id, subscription_id);
    }

    /// Bind a waking adapter through the production engine path.
    pub fn bind_waking_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        self.runtime.bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )
    }

    /// Block until adapter or ingress wakes arrive.
    #[must_use]
    pub fn wait_wakes(&self, timeout: std::time::Duration) -> TerminalWakeBatch {
        self.runtime.wait_wakes(timeout)
    }

    /// Clamp a host wait to the earliest paste or pending-resize deadline.
    #[must_use]
    pub fn clamp_paste_wait(&self, timeout: std::time::Duration) -> std::time::Duration {
        self.runtime.clamp_paste_wait(timeout)
    }

    /// Return exact routes with expired paste assemblies and pending resizes.
    #[must_use]
    pub fn expired_paste_wake_batch(&self, now: std::time::Instant) -> TerminalWakeBatch {
        self.runtime.expired_paste_wake_batch(now)
    }

    /// Merge expired paste and pending-resize sessions into a returned wake batch.
    #[must_use]
    pub fn merge_deadline_wakes(&self, batch: TerminalWakeBatch) -> TerminalWakeBatch {
        self.runtime.merge_deadline_wakes(batch)
    }

    /// Targeted pump of woken routes.
    pub fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        let (mut outcome, sessions) =
            self.runtime
                .pump_woken_phase_one(batch, now_seconds, &HashSet::new())?;
        self.runtime
            .apply_woken_terminal_input(batch, now_seconds, &mut outcome)?;
        self.runtime
            .pump_woken_phase_three(batch, outcome, &sessions)
    }

    /// Take the latest resize applied from targeted terminal input for one session.
    pub fn take_applied_terminal_resize(
        &mut self,
        session_id: &SessionId,
    ) -> Option<(u16, u16, u64)> {
        self.runtime.take_applied_terminal_resize(session_id)
    }

    /// Whether this session has accepted ingress resizes awaiting acknowledgement.
    #[must_use]
    pub fn has_pending_terminal_resizes(&self, session_id: &SessionId) -> bool {
        self.runtime.has_pending_terminal_resizes(session_id)
    }

    /// Number of accepted-but-unacknowledged ingress resizes for one session.
    #[must_use]
    pub fn pending_terminal_resize_len(&self, session_id: &SessionId) -> usize {
        self.runtime.pending_terminal_resize_len(session_id)
    }

    /// Shared wake source for tests and host wait loops.
    #[must_use]
    pub fn wake_source(&self) -> &TerminalWakeSource {
        self.runtime.wake_source()
    }

    /// Control-plane subscription inventory.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        self.runtime.list_terminal_subscriptions()
    }

    /// Detach one live generation if present.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<(DetachTerminalSubscriptionResult, BotsterEngineOutput), DefaultBotsterEngineError>
    {
        self.runtime.detach_terminal_subscription(
            client_id,
            session_id,
            subscription_id,
            generation,
            now_seconds,
        )
    }

    /// Live generation for a subscription, if any.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.runtime
            .terminal_subscription_generation(session_id, subscription_id)
    }

    /// Whether a bound adapter is still held.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime.adapter_is_bound(session_id, subscription_id)
    }

    /// Take session ids whose bound Ready queues grew since the last take.
    #[must_use]
    pub fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        self.runtime.take_bound_queue_wake_sessions()
    }

    /// Whether any live owner still holds undelivered frames for this session.
    #[must_use]
    pub fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        self.runtime.session_has_undelivered_frames(session_id)
    }

    /// Whether the bound owner still holds frames that the next pump must flush.
    #[must_use]
    pub fn bound_owner_has_held_frames(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime
            .bound_owner_has_held_frames(session_id, subscription_id)
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Write terminal bytes from a client into the local process runtime.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a client-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Drain currently available local runtime output through subscription fanout.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.drain_runtime_once(session_id, last_output_at)
    }

    /// Drain currently available local runtime output once for every live session.
    pub fn drain_runtime_all_once(
        &mut self,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.drain_runtime_all_once(last_output_at)
    }

    /// Report client-side backpressure through the default local engine path.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .report_backpressure(client_id, session_id, source, capacity, depth)
    }

    /// Report accepted-but-slow delivery through the default local engine path.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        )
    }

    /// Report a failed delivery attempt through the default local engine path.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .report_delivery_failure(client_id, session_id, subscription_id, source, reason)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, DefaultBotsterEngineError> {
        self.runtime
            .classify_activity(session_id, now_seconds, active_threshold_seconds)
    }

    /// Inspect one session's lifecycle and activity through the command facade.
    pub fn inspect_session(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, DefaultBotsterEngineError> {
        self.runtime
            .inspect_session(session_id, now_seconds, active_threshold_seconds)
    }

    /// Read a session's plain screen state where the managed runtime supports it.
    pub fn read_screen(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .read_screen(request_id, session_id, now_seconds)
    }

    /// Read authoritative terminal mode flags through the managed session path.
    pub fn read_mode_flags(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_session_request(
            crate::SessionIoRequest::GetModeFlags {
                request_id,
                session_id,
            },
            now_seconds,
        )
    }

    /// Capture a session snapshot where the managed runtime supports it.
    pub fn capture_snapshot(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .capture_snapshot(request_id, session_id, now_seconds)
    }

    /// Capture a reusable opaque snapshot payload for one session.
    pub fn capture_snapshot_payload(
        &mut self,
        session_id: &SessionId,
    ) -> Result<TerminalSnapshotPayload, DefaultBotsterEngineError> {
        self.runtime.capture_snapshot_payload(session_id)
    }

    /// Capture screen, snapshot, and authoritative mode read from one terminal shadow.
    pub fn capture_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (
            crate::TerminalScreenState,
            TerminalSnapshotPayload,
            Result<crate::ModeFlags, crate::TerminalBackendError>,
        ),
        DefaultBotsterEngineError,
    > {
        self.runtime.capture_terminal_state(session_id)
    }

    /// Capture colors and GHOSTSNP under one terminal ownership section.
    pub fn capture_color_and_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(crate::TerminalColorProfile, TerminalSnapshotPayload), DefaultBotsterEngineError>
    {
        self.runtime.capture_color_and_snapshot(session_id)
    }

    /// Replay or prepare a snapshot where the managed runtime supports it.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.replay_snapshot(request, now_seconds)
    }

    /// Shut down one local PTY-backed session through the managed runtime path.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .shutdown_session(session_id, reason, now_seconds)
    }
}

#[cfg(feature = "local-runtime")]
impl WorkerBackedBotsterEngine {
    /// Build an empty worker-backed local PTY engine.
    #[must_use]
    pub fn new(worker_path: impl Into<std::path::PathBuf>) -> Self {
        Self::with_options(WorkerProcessRuntimeOptions::new(worker_path))
    }

    /// Build an empty worker-backed local PTY engine with explicit options.
    #[must_use]
    pub fn with_options(options: WorkerProcessRuntimeOptions) -> Self {
        let wakes = TerminalWakeSource::new();
        Self {
            runtime: runtime_without_terminal_shadow(
                WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone()),
            )
            .with_shared_wake_source(wakes),
            captures: HashMap::new(),
            capture_queue: HashMap::new(),
            next_host_capture: 1,
            host_captures: HashMap::new(),
            pending_barrier_cancels: HashMap::new(),
        }
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.runtime.session(session_id)
    }

    /// Return sessions currently recorded by the worker-backed facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.runtime.list_sessions()
    }

    /// Forget all worker-backed engine state for one terminal session.
    pub fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        self.captures.remove(session_id);
        self.capture_queue.remove(session_id);
        self.runtime.forget_terminal_session(session_id)
    }

    /// Return the worker process runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &WorkerProcessRuntime {
        self.runtime.session_runtime()
    }

    /// Return the worker process runtime adapter mutably.
    pub const fn session_runtime_mut(&mut self) -> &mut WorkerProcessRuntime {
        self.runtime.session_runtime_mut()
    }

    /// Return worker welcome metadata captured after spawning a session.
    #[must_use]
    pub fn worker_metadata(&self, session_id: &SessionId) -> Option<&SessionMetadata> {
        self.runtime.session_runtime().metadata(session_id)
    }

    /// Adopt a live worker process through its reconnectable control endpoint.
    pub fn adopt_worker_process(
        &mut self,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: impl Into<std::path::PathBuf>,
        supports_snapshot_boundary: bool,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, WorkerBackedBotsterEngineError> {
        self.runtime.adopt_worker_process(
            session_id,
            process,
            socket_path,
            supports_snapshot_boundary,
            metadata,
        )
    }

    /// Release workers without sending shutdown frames for an intentional daemon restart.
    pub fn release_workers_for_restart(&mut self) {
        for (session_id, capture) in std::mem::take(&mut self.captures) {
            self.cancel_capture_boundary(&session_id, capture.request_id.clone());
        }
        self.capture_queue.clear();
        self.pending_barrier_cancels.clear();
        self.runtime.release_workers_for_restart();
    }

    /// Spawn a local session whose PTY is owned by a worker process.
    ///
    /// This waits for the worker handshake on the calling thread. Hosts that
    /// must not block use [`Self::begin_spawn`] and [`Self::poll_spawn`].
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, WorkerBackedBotsterEngineError> {
        self.runtime.spawn_session(request, metadata)
    }

    /// Start one worker spawn on the launch thread and return at once.
    pub fn begin_spawn(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        Ok(self.runtime.session_runtime_mut().begin_spawn(request)?)
    }

    /// Poll one spawn started by [`Self::begin_spawn`]. Never blocks.
    ///
    /// `Ready` installs the session with `metadata` and the initial size.
    pub fn poll_spawn(
        &mut self,
        session_id: &SessionId,
        metadata: CoreSessionMetadata,
        size: TerminalScreenSize,
    ) -> Result<Option<BotsterSpawnOutcome>, WorkerBackedBotsterEngineError> {
        match self.runtime.session_runtime_mut().poll_spawn(session_id) {
            WorkerSpawnPoll::Pending => Ok(None),
            WorkerSpawnPoll::Failed(error) => Err(error.into()),
            WorkerSpawnPoll::Ready(handle) => Ok(Some(
                self.runtime
                    .install_spawned_worker(handle, metadata, size)?,
            )),
        }
    }

    /// Whether a spawn for this session is still on the launch thread.
    #[must_use]
    pub fn spawn_pending(&self, session_id: &SessionId) -> bool {
        self.runtime.session_runtime().has_pending_spawn(session_id)
    }

    /// Abandon a spawn started by [`Self::begin_spawn`]. The launch keeps
    /// running until [`Self::poll_spawn`] collects it; the worker is then
    /// stopped and reaped instead of installed.
    pub fn abandon_spawn(&mut self, session_id: &SessionId) -> bool {
        self.runtime.session_runtime_mut().abandon_spawn(session_id)
    }

    /// A route left the epoch its capture started in: cancel the worker
    /// barrier and let the queued resync request start a fresh capture.
    fn supersede_capture(
        &mut self,
        session_id: &SessionId,
        capture: RouteCapture,
        output: BotsterEngineOutput,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.cancel_capture_boundary(session_id, capture.request_id.clone());
        self.start_resync_captures()?;
        self.start_next_capture(session_id)?;
        Ok(output)
    }

    /// Attach a client to a session stream.
    ///
    /// Records the subscription, sends `ATTACH_STATE attaching` to a bound
    /// route, and starts (or queues) one worker snapshot capture for it.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let _ = now_seconds;
        self.runtime
            .worker_supports_snapshot_boundary(&session_id)?;
        self.cancel_capture_for_route(&session_id, &subscription_id);
        let output = self.runtime.begin_snapshot_attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )?;
        let worker = self.runtime.client_worker_mut();
        let _ = worker.push_attach_state(&session_id, &subscription_id, AttachStateCode::Attaching);
        let identity = worker.capture_identity(&session_id, &subscription_id);
        self.enqueue_capture(
            &session_id,
            CaptureRequest {
                client_id,
                subscription_id,
                kind: CaptureKind::Attach,
                identity,
            },
        );
        self.sync_worker_consumers(&session_id)?;
        Ok(output)
    }

    /// Record that the next attach for this identity will bind an adapter.
    pub fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.runtime
            .expect_terminal_adapter(client_id, session_id, subscription_id);
    }

    /// Retire an unconsumed pre-attach adapter declaration.
    pub fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.runtime
            .cancel_expected_terminal_adapter(client_id, session_id, subscription_id);
    }

    /// Bind a waking adapter through the worker-backed production path.
    pub fn bind_waking_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        mut adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        if matches!(
            self.runtime.control_plane_state(&session_id),
            crate::runtime::ControlPlaneState::Failed(_)
        ) {
            adapter.close();
            drop(adapter);
            return Err(BindTerminalAdapterError::ControlPlaneFailed { session_id });
        }
        self.runtime.bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )
    }

    /// Block until adapter or ingress wakes arrive.
    #[must_use]
    pub fn wait_wakes(&self, timeout: std::time::Duration) -> TerminalWakeBatch {
        self.runtime.wait_wakes(timeout)
    }

    /// Clamp a host wait to the earliest paste or pending-resize deadline.
    #[must_use]
    pub fn clamp_paste_wait(&self, timeout: std::time::Duration) -> std::time::Duration {
        self.runtime.clamp_paste_wait(timeout)
    }

    /// Return exact routes with expired paste assemblies and pending resizes.
    #[must_use]
    pub fn expired_paste_wake_batch(&self, now: std::time::Instant) -> TerminalWakeBatch {
        self.runtime.expired_paste_wake_batch(now)
    }

    /// Merge expired paste and pending-resize sessions into a returned wake batch.
    #[must_use]
    pub fn merge_deadline_wakes(&self, batch: TerminalWakeBatch) -> TerminalWakeBatch {
        self.runtime.merge_deadline_wakes(batch)
    }

    /// Targeted pump of woken routes.
    pub fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let deferred_sessions: HashSet<_> = self.captures.keys().cloned().collect();
        let (mut outcome, sessions) =
            self.runtime
                .pump_woken_phase_one(batch, now_seconds, &deferred_sessions)?;
        for session_id in sessions.intersection(&deferred_sessions) {
            let step = self.drain_runtime_once(session_id, now_seconds)?;
            append_engine_output(&mut outcome, step);
        }
        self.runtime
            .reconcile_terminal_resize_acknowledgments(&sessions)?;
        self.runtime.apply_woken_terminal_input(
            batch,
            now_seconds,
            &deferred_sessions,
            &mut outcome,
        )?;
        self.start_resync_captures()?;
        self.runtime
            .pump_woken_phase_three(batch, outcome, &sessions)
    }

    /// Take the latest resize applied from targeted terminal input for one session.
    pub fn take_applied_terminal_resize(
        &mut self,
        session_id: &SessionId,
    ) -> Option<(u16, u16, u64)> {
        self.runtime.take_applied_terminal_resize(session_id)
    }

    /// Whether this session has accepted ingress resizes awaiting acknowledgement.
    #[must_use]
    pub fn has_pending_terminal_resizes(&self, session_id: &SessionId) -> bool {
        self.runtime.has_pending_terminal_resizes(session_id)
    }

    /// Number of accepted-but-unacknowledged ingress resizes for one session.
    #[must_use]
    pub fn pending_terminal_resize_len(&self, session_id: &SessionId) -> usize {
        self.runtime.pending_terminal_resize_len(session_id)
    }

    /// Shared wake source for tests and host wait loops.
    #[must_use]
    pub fn wake_source(&self) -> &TerminalWakeSource {
        self.runtime.wake_source()
    }

    /// Control-plane subscription inventory.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        self.runtime.list_terminal_subscriptions()
    }

    /// Detach one live generation if present.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<
        (DetachTerminalSubscriptionResult, BotsterEngineOutput),
        WorkerBackedBotsterEngineError,
    > {
        if self
            .runtime
            .terminal_subscription_generation(&session_id, &subscription_id)
            == Some(generation)
        {
            self.cancel_capture_for_route(&session_id, &subscription_id);
        }
        let result = self.runtime.detach_terminal_subscription(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            now_seconds,
        );
        let _ = self.start_next_capture(&session_id);
        let _ = self.sync_worker_consumers(&session_id);
        result
    }

    /// Live generation for a subscription, if any.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.runtime
            .terminal_subscription_generation(session_id, subscription_id)
    }

    /// Whether a bound adapter is still held.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime.adapter_is_bound(session_id, subscription_id)
    }

    /// Take session ids whose bound Ready queues grew since the last take.
    #[must_use]
    pub fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        self.runtime.take_bound_queue_wake_sessions()
    }

    /// Whether any live owner still holds undelivered frames for this session.
    #[must_use]
    pub fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        self.runtime.session_has_undelivered_frames(session_id)
    }

    /// Whether the bound owner still holds frames that the next pump must flush.
    #[must_use]
    pub fn bound_owner_has_held_frames(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime
            .bound_owner_has_held_frames(session_id, subscription_id)
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.cancel_capture_for_route(&session_id, &subscription_id);
        let output = self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id: session_id.clone(),
                subscription_id,
            },
            now_seconds,
        );
        let _ = self.start_next_capture(&session_id);
        let _ = self.sync_worker_consumers(&session_id);
        output
    }

    /// Write raw bytes from a host client into the worker-owned PTY.
    ///
    /// The worker applies the bytes after any barrier in progress; ordering
    /// against the capture is preserved by the worker control lane.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a host-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Whether a worker capture is in progress for this session.
    #[must_use]
    pub fn capture_active(&self, session_id: &SessionId) -> bool {
        self.captures.contains_key(session_id)
    }

    /// Durable control-plane state for one worker session.
    #[must_use]
    pub fn control_plane_state(&self, session_id: &SessionId) -> crate::runtime::ControlPlaneState {
        self.runtime.control_plane_state(session_id)
    }

    /// Latest worker modes for one session in this daemon incarnation.
    #[must_use]
    pub fn latest_modes(&self, session_id: &SessionId) -> Option<ModesBody> {
        self.runtime.session_runtime().latest_modes(session_id)
    }

    /// Start one correlated mode-flags probe at the worker.
    pub fn begin_mode_flags_probe(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, WorkerBackedBotsterEngineError> {
        Ok(self
            .runtime
            .session_runtime_mut()
            .begin_mode_flags_probe(session_id)?)
    }

    /// Take correlated mode-flags replies.
    pub fn take_mode_flags_replies(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<ModeFlagsPayload>, WorkerBackedBotsterEngineError> {
        Ok(self
            .runtime
            .session_runtime_mut()
            .take_mode_flags_replies(session_id)?)
    }

    /// Start one correlated plain-text screen probe at the worker.
    pub fn begin_screen_probe(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, WorkerBackedBotsterEngineError> {
        Ok(self
            .runtime
            .session_runtime_mut()
            .begin_screen_probe(session_id)?)
    }

    /// Take correlated screen replies.
    pub fn take_screen_replies(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<ScreenPayload>, WorkerBackedBotsterEngineError> {
        Ok(self
            .runtime
            .session_runtime_mut()
            .take_screen_replies(session_id)?)
    }

    /// Take the final worker terminal state retained at exit.
    pub fn take_final_state(&mut self, session_id: &SessionId) -> Option<RetainedWorkerFinalState> {
        self.runtime
            .session_runtime_mut()
            .take_final_state(session_id)
    }

    /// Whether the worker egress link for this session has ended.
    pub fn worker_link_ended(&mut self, session_id: &SessionId) -> bool {
        self.runtime
            .session_runtime_mut()
            .session_reader_finished(session_id)
            .unwrap_or(true)
    }

    /// Drain currently available worker process output through subscription fanout.
    ///
    /// When a worker capture is active for the session, this advances the
    /// capture: pre-READY output is routed to other routes, snapshot frames
    /// are streamed to the capturing route, and the barrier is released after
    /// `FINISH`.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let Some(mut capture) = self.captures.remove(session_id) else {
            let output = self
                .runtime
                .drain_runtime_once(session_id, last_output_at)?;
            self.start_resync_captures()?;
            self.start_next_capture(session_id)?;
            return Ok(output);
        };
        let poll = match self
            .runtime
            .session_runtime_mut()
            .poll_snapshot_boundary(session_id, &capture.request_id)
        {
            Ok(poll) => poll,
            Err(error) => {
                let not_found = error.kind == crate::SessionRuntimeErrorKind::SessionNotFound;
                self.cancel_capture_boundary(session_id, capture.request_id.clone());
                self.fail_capture_route(session_id, &capture);
                if not_found {
                    let _ = self.start_next_capture(session_id);
                    return self.runtime.drain_runtime_once(session_id, last_output_at);
                }
                return Err(error.into());
            }
        };
        let mut output = match self.runtime.route_worker_boundary_outputs(
            session_id,
            poll.before_ready,
            last_output_at,
        ) {
            Ok(output) => output,
            Err(error) => {
                self.cancel_capture_boundary(session_id, capture.request_id.clone());
                self.fail_capture_route(session_id, &capture);
                return Err(error);
            }
        };
        suppress_capture_route_output(&mut output, session_id, &capture, &self.capture_queue);

        let mut finished = false;
        for frame in poll.frames {
            let error = frame
                .error_kind
                .or_else(|| {
                    frame
                        .phase
                        .is_none()
                        .then(|| "worker snapshot frame omitted its phase".to_string())
                })
                .or_else(|| {
                    frame
                        .snapshot
                        .is_none()
                        .then(|| "worker snapshot frame omitted its bytes".to_string())
                });
            if let Some(error) = error {
                if !capture.ready {
                    // Capture failed before READY: the route ends explicitly.
                    self.cancel_capture_boundary(session_id, capture.request_id.clone());
                    self.fail_capture_route(session_id, &capture);
                    let _ = self.runtime.handle_client_ingress(
                        capture.client_id.clone(),
                        TransportIngress::UnsubscribeSession {
                            client_id: capture.client_id.clone(),
                            session_id: session_id.clone(),
                            subscription_id: capture.subscription_id.clone(),
                        },
                        last_output_at,
                    );
                    let _ = self.start_next_capture(session_id);
                    return Err(ManagedSessionRuntimeError::Runtime(
                        crate::SessionRuntimeError::new(
                            crate::SessionRuntimeErrorKind::OutputFailed,
                            error,
                        ),
                    ));
                }
                if let CaptureKind::Host(id) = capture.kind {
                    // A host capture needs the whole object; a partial one is a failure.
                    self.cancel_capture_boundary(session_id, capture.request_id.clone());
                    self.record_host_capture(id, Err(error));
                    let _ = self.start_next_capture(session_id);
                    return Ok(output);
                }
                // History failed after READY: the screen is valid, history is not.
                capture.history_incomplete = true;
                if self.push_capture_frames(
                    session_id,
                    &capture,
                    vec![
                        encode_history_unavailable(HistoryUnavailableReason::CaptureFailed),
                        encode_snapshot_finish(),
                    ],
                ) {
                    return self.supersede_capture(session_id, capture, output);
                }
                finished = true;
                break;
            }
            let phase = frame.phase.expect("snapshot phase was validated above");
            let snapshot = frame.snapshot.expect("snapshot bytes were validated above");
            if matches!(capture.kind, CaptureKind::Host(_)) {
                capture.collected.extend_from_slice(&snapshot.bytes);
                capture.collected_size = Some(snapshot.size);
                if let Some(colors) = frame.color_profile {
                    capture.collected_colors = Some(colors);
                }
            }
            if capture.kind == CaptureKind::Attach {
                // Unbound drain consumers still receive the harness attach frames.
                let frame_output = self.runtime.snapshot_attach_frame(
                    capture.client_id.clone(),
                    session_id.clone(),
                    capture.subscription_id.clone(),
                    snapshot.bytes.clone(),
                )?;
                append_engine_output(&mut output, frame_output);
            }
            match phase {
                crate::WorkerSnapshotPhase::Ready => {
                    capture.ready = true;
                    let modes = self.current_modes(session_id);
                    let mut frames = Vec::new();
                    if capture.kind == CaptureKind::Attach {
                        frames.push(botster_terminal_protocol::encode_attach_state(
                            AttachStateCode::Attached,
                        ));
                    }
                    frames.push(encode_modes(modes));
                    frames.push(encode_snapshot_ready(&snapshot.bytes));
                    if self.push_capture_frames(session_id, &capture, frames) {
                        return self.supersede_capture(session_id, capture, output);
                    }
                }
                crate::WorkerSnapshotPhase::History => {
                    if self.push_capture_frames(
                        session_id,
                        &capture,
                        vec![encode_snapshot_history(&snapshot.bytes)],
                    ) {
                        return self.supersede_capture(session_id, capture, output);
                    }
                }
                crate::WorkerSnapshotPhase::Finish => {
                    // The GHOSTSNP finish record is the last history page.
                    if self.push_capture_frames(
                        session_id,
                        &capture,
                        vec![
                            encode_snapshot_history(&snapshot.bytes),
                            encode_snapshot_finish(),
                        ],
                    ) {
                        return self.supersede_capture(session_id, capture, output);
                    }
                    finished = true;
                }
            }
        }

        if finished && !capture.awaiting_release {
            capture.awaiting_release = true;
            if let Err(error) = self
                .runtime
                .session_runtime_mut()
                .complete_snapshot_boundary(session_id, &capture.request_id)
            {
                self.cancel_capture_boundary(session_id, capture.request_id.clone());
                self.fail_capture_route(session_id, &capture);
                let _ = self.start_next_capture(session_id);
                return Err(error.into());
            }
        }

        if !poll.complete {
            // Bound routes have no other consumer for live bytes queued behind
            // the boundary. Pull them now; the ClientWorker suppresses output
            // for routes that still await their READY.
            if self
                .runtime
                .adapter_is_bound(session_id, &capture.subscription_id)
            {
                let live = self
                    .runtime
                    .drain_runtime_once(session_id, last_output_at)?;
                append_engine_output(&mut output, live);
                if matches!(
                    self.runtime
                        .session(session_id)
                        .map(|session| &session.lifecycle),
                    None | Some(crate::SessionLifecycleState::Exited { .. })
                        | Some(crate::SessionLifecycleState::Failed { .. })
                ) {
                    self.cancel_capture_boundary(session_id, capture.request_id.clone());
                    self.capture_queue.remove(session_id);
                    return Ok(output);
                }
            }
            self.captures.insert(session_id.clone(), capture);
            self.reconcile_capture_after_teardown(session_id)?;
            return Ok(output);
        }

        // Barrier released. Finish the harness attach for unbound consumers.
        if let CaptureKind::Host(id) = capture.kind {
            let result = match (capture.collected_size, capture.collected_colors.take()) {
                (Some(size), Some(color_profile)) => Ok(HostCaptureResult {
                    bytes: std::mem::take(&mut capture.collected),
                    size,
                    color_profile,
                }),
                _ => Err("worker capture omitted size or color profile".to_owned()),
            };
            self.record_host_capture(id, result);
        }
        if capture.kind == CaptureKind::Attach {
            let attached = self.runtime.complete_snapshot_attach(
                capture.client_id.clone(),
                session_id.clone(),
                capture.subscription_id.clone(),
                capture.history_incomplete,
            )?;
            append_engine_output(&mut output, attached);
        }
        self.sync_worker_consumers(session_id)?;
        let leftover = self
            .runtime
            .drain_runtime_once(session_id, last_output_at)?;
        append_engine_output(&mut output, leftover);
        self.start_next_capture(session_id)?;
        self.start_resync_captures()?;
        Ok(output)
    }

    fn current_modes(&self, session_id: &SessionId) -> ModesBody {
        self.runtime
            .session_runtime()
            .latest_modes(session_id)
            .or_else(|| self.runtime.client_worker().session_modes(session_id))
            .unwrap_or_default()
    }

    /// Enqueue capture pages under the capture's epoch. Returns `true` when
    /// the route left that epoch and the capture is superseded.
    fn push_capture_frames(
        &mut self,
        session_id: &SessionId,
        capture: &RouteCapture,
        frames: Vec<
            Result<
                botster_terminal_protocol::TerminalFrame,
                botster_terminal_protocol::TerminalFrameError,
            >,
        >,
    ) -> bool {
        if matches!(capture.kind, CaptureKind::Host(_)) {
            return false;
        }
        let worker = self.runtime.client_worker_mut();
        for frame in frames {
            let Ok(frame) = frame else {
                let _ = worker.fail_route(session_id, &capture.subscription_id);
                return false;
            };
            let Some(identity) = capture.identity else {
                return true;
            };
            match worker.push_capture_frame(session_id, &capture.subscription_id, identity, frame) {
                Ok(None) => {}
                Err(EnqueueRouteFrameError::EpochSuperseded) => return true,
                Ok(Some(_)) | Err(_) => return false,
            }
        }
        false
    }

    fn record_host_capture(&mut self, id: u64, result: Result<HostCaptureResult, String>) {
        self.host_captures.insert(id, result);
    }

    fn fail_capture_route(&mut self, session_id: &SessionId, capture: &RouteCapture) {
        if let CaptureKind::Host(id) = capture.kind {
            self.record_host_capture(id, Err("worker capture failed".to_owned()));
            return;
        }
        let _ = self
            .runtime
            .client_worker_mut()
            .fail_route(session_id, &capture.subscription_id);
    }

    /// Start one host-owned worker capture. Returns its handle.
    ///
    /// The capture shares the per-session capture queue with route captures
    /// and completes through [`Self::take_host_capture`].
    pub fn begin_host_capture(&mut self, session_id: &SessionId) -> u64 {
        let id = self.next_host_capture;
        self.next_host_capture += 1;
        self.enqueue_capture(
            session_id,
            CaptureRequest {
                client_id: ClientId(format!("host-capture-{id}")),
                subscription_id: SubscriptionId(format!("host-capture-{id}")),
                kind: CaptureKind::Host(id),
                identity: None,
            },
        );
        id
    }

    /// Take one finished host capture, when it has completed.
    pub fn take_host_capture(&mut self, id: u64) -> Option<Result<HostCaptureResult, String>> {
        self.host_captures.remove(&id)
    }

    /// Cancel one host capture, whether queued, active, or finished but not
    /// yet taken. Queued and active work is removed, so no later result can
    /// exist and nothing is retained for the cancelled id.
    pub fn cancel_host_capture(&mut self, session_id: &SessionId, id: u64) {
        self.host_captures.remove(&id);
        let subscription_id = SubscriptionId(format!("host-capture-{id}"));
        self.cancel_capture_for_route(session_id, &subscription_id);
        let _ = self.start_next_capture(session_id);
    }

    fn enqueue_capture(&mut self, session_id: &SessionId, request: CaptureRequest) {
        let queue = self.capture_queue.entry(session_id.clone()).or_default();
        queue.retain(|queued| queued.subscription_id != request.subscription_id);
        queue.push_back(request);
        let _ = self.start_next_capture(session_id);
    }

    /// Start the next queued capture when none is active.
    fn start_next_capture(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        if self.captures.contains_key(session_id) {
            return Ok(());
        }
        // A refused cancel still owns the worker barrier; no new capture can
        // begin until the worker has been asked to release it.
        if !self.retry_barrier_cancels(session_id) {
            return Ok(());
        }
        loop {
            let Some(next) = self
                .capture_queue
                .get_mut(session_id)
                .and_then(VecDeque::pop_front)
            else {
                self.capture_queue.remove(session_id);
                return Ok(());
            };
            let is_host = matches!(next.kind, CaptureKind::Host(_));
            if !is_host {
                // A queued request must still name the live attachment and
                // fence; a stale request never starts work.
                let live = self.runtime.terminal_subscription_matches(
                    session_id,
                    &next.client_id,
                    &next.subscription_id,
                ) && next.identity.is_some_and(|identity| {
                    self.runtime.client_worker().capture_identity_is_live(
                        session_id,
                        &next.subscription_id,
                        identity,
                    )
                });
                if !live {
                    continue;
                }
            }
            match self
                .runtime
                .session_runtime_mut()
                .begin_snapshot_boundary(session_id)
            {
                Ok(request_id) => {
                    if !is_host {
                        self.runtime
                            .client_worker_mut()
                            .begin_route_capture(session_id, &next.subscription_id);
                    }
                    self.captures.insert(
                        session_id.clone(),
                        RouteCapture {
                            client_id: next.client_id,
                            subscription_id: next.subscription_id,
                            kind: next.kind,
                            request_id,
                            identity: next.identity,
                            ready: false,
                            awaiting_release: false,
                            history_incomplete: false,
                            collected: Vec::new(),
                            collected_size: None,
                            collected_colors: None,
                        },
                    );
                    let _ = self.sync_worker_consumers(session_id);
                    return Ok(());
                }
                Err(error) if error.message.contains("already in flight") => {
                    // A host capture holds the worker barrier. Keep the route
                    // queued; the next drain retries.
                    self.capture_queue
                        .entry(session_id.clone())
                        .or_default()
                        .push_front(next);
                    return Ok(());
                }
                Err(error) => {
                    if let CaptureKind::Host(id) = next.kind {
                        self.record_host_capture(id, Err(error.message));
                        continue;
                    }
                    let _ = self
                        .runtime
                        .client_worker_mut()
                        .fail_route(session_id, &next.subscription_id);
                    let _ = self.runtime.detach_live_subscription(
                        next.client_id,
                        session_id.clone(),
                        next.subscription_id,
                        0,
                    );
                }
            }
        }
    }

    /// Turn overflow resync requests into captures.
    fn start_resync_captures(&mut self) -> Result<(), WorkerBackedBotsterEngineError> {
        let requests = self.runtime.client_worker_mut().take_resync_requests();
        for request in requests {
            let identity = CaptureIdentity {
                generation: request.generation,
                capture_fence: request.capture_fence,
            };
            // A request that no longer names the live attachment and fence is
            // stale. It must not touch capture work: the route it came from
            // is gone or has already moved on, and any active or queued
            // capture belongs to the current owner.
            let live = self.runtime.terminal_subscription_matches(
                &request.session_id,
                &request.client_id,
                &request.subscription_id,
            ) && self.runtime.client_worker().capture_identity_is_live(
                &request.session_id,
                &request.subscription_id,
                identity,
            );
            if !live {
                continue;
            }
            // A capture still running for this route belongs to the epoch the
            // overflow left; its remaining pages must not enter the new one.
            let superseded = self
                .captures
                .get(&request.session_id)
                .is_some_and(|capture| {
                    capture.subscription_id == request.subscription_id
                        && capture.identity != Some(identity)
                        && !matches!(capture.kind, CaptureKind::Host(_))
                });
            if superseded {
                self.cancel_capture_for_route(&request.session_id, &request.subscription_id);
            }
            self.enqueue_capture(
                &request.session_id,
                CaptureRequest {
                    client_id: request.client_id,
                    subscription_id: request.subscription_id,
                    kind: CaptureKind::Resync,
                    identity: Some(identity),
                },
            );
        }
        Ok(())
    }

    fn cancel_capture_for_route(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) {
        if let Some(queue) = self.capture_queue.get_mut(session_id) {
            queue.retain(|queued| &queued.subscription_id != subscription_id);
        }
        let active_matches = self
            .captures
            .get(session_id)
            .is_some_and(|capture| &capture.subscription_id == subscription_id);
        if active_matches {
            if let Some(capture) = self.captures.remove(session_id) {
                self.cancel_capture_boundary(session_id, capture.request_id);
            }
        }
    }

    /// Ask the worker to release the barrier of one capture.
    ///
    /// Ownership of the release moves from the capture to this engine. The
    /// obligation exists exactly while the runtime still holds `request_id`
    /// as its outstanding barrier request: a cancel the control queue refuses
    /// is retained and retried on later session wakes; once the runtime no
    /// longer holds the request (accepted cancel, worker gone, worker
    /// respawned or adopted under a new request) nothing remains to clean up.
    fn cancel_capture_boundary(&mut self, session_id: &SessionId, request_id: String) {
        self.cancel_capture_boundary_at(session_id, request_id, Instant::now());
    }

    fn cancel_capture_boundary_at(
        &mut self,
        session_id: &SessionId,
        request_id: String,
        now: Instant,
    ) {
        if let Some(pending) = self.pending_barrier_cancels.get(session_id) {
            if pending.request_id == request_id {
                // Already owed; the retry path carries it.
                return;
            }
        }
        match self.try_cancel_barrier(session_id, &request_id) {
            BarrierCancelStep::Done => {
                self.pending_barrier_cancels.remove(session_id);
            }
            BarrierCancelStep::Retry => {
                // The runtime holds one outstanding request per session, so
                // an older pending cancel for another id cannot still be owed.
                self.pending_barrier_cancels.insert(
                    session_id.clone(),
                    PendingBarrierCancel {
                        request_id,
                        first_refused: now,
                    },
                );
            }
        }
    }

    /// One cancel attempt. `Done` means nothing is owed for this request any
    /// more; `Retry` means the exact request is still outstanding at the
    /// worker and the cancel did not enter the control queue.
    fn try_cancel_barrier(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> BarrierCancelStep {
        let runtime = self.runtime.session_runtime_mut();
        if !runtime.snapshot_request_is_outstanding(session_id, request_id) {
            return BarrierCancelStep::Done;
        }
        if matches!(
            runtime.control_plane_state(session_id),
            crate::runtime::ControlPlaneState::Failed(_)
        ) {
            // Recovery is respawn; the barrier ends with this worker.
            return BarrierCancelStep::Done;
        }
        match runtime.cancel_snapshot_boundary(session_id, request_id) {
            Ok(SnapshotCancelAdmission::Accepted) => BarrierCancelStep::Done,
            Ok(SnapshotCancelAdmission::QueueFull) => BarrierCancelStep::Retry,
            // A queued shutdown ends the worker, barrier included.
            Ok(SnapshotCancelAdmission::Sealed) => BarrierCancelStep::Done,
            // The session exists and holds the request, yet the attempt
            // failed: keep the obligation and try again on the next wake.
            Err(_) => BarrierCancelStep::Retry,
        }
    }

    /// Retry the refused barrier cancel of one session, if any.
    ///
    /// Called from session wakes: each drain and each attempt to start a
    /// capture. Returns `true` when nothing is owed, so a new capture may
    /// begin. A cancel still refused after `BARRIER_CANCEL_RETRY_BOUND` fails
    /// the session control plane explicitly instead of leaving the worker
    /// barrier unowned.
    fn retry_barrier_cancels(&mut self, session_id: &SessionId) -> bool {
        self.retry_barrier_cancels_at(session_id, Instant::now())
    }

    fn retry_barrier_cancels_at(&mut self, session_id: &SessionId, now: Instant) -> bool {
        let Some(pending) = self.pending_barrier_cancels.get(session_id).cloned() else {
            return true;
        };
        match self.try_cancel_barrier(session_id, &pending.request_id) {
            BarrierCancelStep::Done => {
                self.pending_barrier_cancels.remove(session_id);
                true
            }
            BarrierCancelStep::Retry => {
                if now.duration_since(pending.first_refused) >= BARRIER_CANCEL_RETRY_BOUND {
                    self.runtime
                        .session_runtime_mut()
                        .mark_control_plane_failed(
                            session_id,
                            crate::runtime::ControlWriterError::DeadlineExpired,
                        );
                    self.pending_barrier_cancels.remove(session_id);
                    self.capture_queue.remove(session_id);
                    // The control plane is failed; nothing else can be sent.
                    return true;
                }
                false
            }
        }
    }

    fn sync_worker_consumers(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        // Stall only after a route has its capture. A route whose capture is
        // active or queued has an inventory row, but the parent may stop
        // pumping at READY.
        let mut excluded = HashSet::new();
        if let Some(capture) = self.captures.get(session_id) {
            excluded.insert((capture.client_id.clone(), capture.subscription_id.clone()));
        }
        if let Some(queue) = self.capture_queue.get(session_id) {
            excluded.extend(
                queue
                    .iter()
                    .map(|request| (request.client_id.clone(), request.subscription_id.clone())),
            );
        }
        let owners = self
            .runtime
            .list_terminal_subscriptions()
            .into_iter()
            .filter(|row| {
                row.session_id == *session_id
                    && !excluded.contains(&(row.client_id.clone(), row.subscription_id.clone()))
            })
            .map(|row| (row.client_id, row.subscription_id))
            .collect::<Vec<_>>();
        self.runtime
            .session_runtime_mut()
            .replace_named_consumers(session_id, owners)
            .map_err(Into::into)
    }

    fn reconcile_capture_after_teardown(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        let Some(capture) = self.captures.get(session_id) else {
            return Ok(());
        };
        // Host captures belong to pending host operations, not to a route.
        if matches!(capture.kind, CaptureKind::Host(_))
            || self.runtime.terminal_subscription_matches(
                session_id,
                &capture.client_id,
                &capture.subscription_id,
            )
        {
            return Ok(());
        }
        let capture = self
            .captures
            .remove(session_id)
            .expect("capture existed above");
        self.cancel_capture_boundary(session_id, capture.request_id.clone());
        self.start_next_capture(session_id)
    }

    /// Shut down a worker-owned session.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        if let Some(capture) = self.captures.remove(&session_id) {
            self.cancel_capture_boundary(&session_id, capture.request_id.clone());
        }
        self.capture_queue.remove(&session_id);
        self.runtime
            .shutdown_session(session_id, reason, now_seconds)
    }

    /// Cancel one in-flight client input operation at the worker.
    ///
    /// Returns `true` when the operation was still in flight and a cancel was
    /// sent. The route receives the worker's `INPUT_RESULT`.
    pub fn cancel_input_operation(
        &mut self,
        route: &botster_terminal_protocol::RouteId,
        generation: u64,
        operation_id: u64,
    ) -> Result<bool, WorkerBackedBotsterEngineError> {
        let Some((session_id, key)) = self.runtime.client_worker_mut().in_flight_key_for_route(
            route,
            generation,
            operation_id,
        ) else {
            return Ok(false);
        };
        self.runtime
            .session_runtime_mut()
            .cancel_input_operation(&session_id, key)?;
        Ok(true)
    }

    /// Fail every in-flight input operation of a session as unknown.
    ///
    /// Hosts call this when they observe a worker link failure outside the
    /// pump, for example during shutdown.
    pub fn fail_in_flight_input(&mut self, session_id: &SessionId) {
        let _ = self.runtime.client_worker_mut().fail_in_flight_for_session(
            session_id,
            InputOutcome::OutcomeUnknown,
            "worker link failed",
        );
    }
}

#[cfg(feature = "local-runtime")]
fn append_engine_output(target: &mut BotsterEngineOutput, source: BotsterEngineOutput) {
    target.client_egress.extend(source.client_egress);
    target.session_requests.extend(source.session_requests);
    target
        .client_control_frames
        .extend(source.client_control_frames);
    target.session_events.extend(source.session_events);
    target.observations.extend(source.observations);
}

/// Drop drain-path output for routes whose capture is active or queued: the
/// capture already contains those bytes for the attaching consumer.
#[cfg(feature = "local-runtime")]
fn suppress_capture_route_output(
    output: &mut BotsterEngineOutput,
    session_id: &SessionId,
    capture: &RouteCapture,
    queue: &HashMap<SessionId, VecDeque<CaptureRequest>>,
) {
    output.client_egress.retain(|(routed_client, frame)| {
        let TransportEgress::TerminalOutput {
            session_id: routed_session,
            subscription_id: routed_subscription,
            ..
        } = frame
        else {
            return true;
        };
        if routed_session != session_id {
            return true;
        }
        let active =
            routed_client == &capture.client_id && routed_subscription == &capture.subscription_id;
        let pending = queue.get(session_id).is_some_and(|queue| {
            queue.iter().any(|request| {
                routed_client == &request.client_id
                    && routed_subscription == &request.subscription_id
            })
        });
        !active && !pending
    });
}

#[cfg(feature = "local-runtime")]
impl Default for DefaultBotsterEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Ergonomic embeddable core API for tmux-like Botster consumers.
///
/// Hosts still provide concrete runtime adapters and policy-resolved spawn
/// requests. This facade only turns common session/client/plugin operations
/// into method calls over the lower-level `MultiplexerEngine`.
///
/// # Example
///
/// This example uses test-support fakes so the host-owned PTY process and
/// plugin callback policy stay outside `botster-core`.
///
/// ```
/// # use std::sync::Arc;
/// # use botster_core::{
/// #     BotsterEngine, BoundaryJson, ClientId, CoreSessionMetadata, ExtensionEntrypoint,
/// #     ExtensionKind, ExtensionRuntime, InitialSnapshotReady, NotificationContent,
/// #     NotificationId, NotificationItem, NotificationSeverity, NotificationSource,
/// #     NotificationTarget, NotificationTimestamp, PackageManifest, PluginHandlerKind,
/// #     PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext,
/// #     PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec,
/// #     PluginWorkerRegistration, RequestId, SessionActivityStatus, SessionId,
/// #     SessionSpawnRequest, SessionWorkerRuntimeEvent, SpawnEnvironment, SpawnWorkingDirectory,
/// #     SubscriptionId, TransportEgress,
/// # };
/// # use botster_core_test_support::fake::{
/// #     FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
/// # };
/// let session_id = SessionId("docs-session".to_string());
/// let client_id = ClientId("docs-client".to_string());
/// let subscription_id = SubscriptionId("docs-subscription".to_string());
///
/// let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
///     BotsterEngine::new(FakeSessionRuntime::new());
///
/// engine.spawn_session(
///     SessionSpawnRequest {
///         request_id: RequestId("docs-spawn".to_string()),
///         session_id: session_id.clone(),
///         executable: "fake-shell".to_string(),
///         arguments: vec!["--login".to_string()],
///         working_directory: SpawnWorkingDirectory {
///             path: "/workspace".to_string(),
///         },
///         environment: SpawnEnvironment::default(),
///         initial_pty_size: None,
///     },
///     CoreSessionMetadata::new(),
///     FakeSessionWorkerRuntime::new(),
/// )?;
///
/// engine.attach_client(client_id.clone(), session_id.clone(), subscription_id.clone(), 1)?;
/// engine.handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
///     InitialSnapshotReady {
///         request_id: RequestId("docs-initial".to_string()),
///         session_id: session_id.clone(),
///         client_id: client_id.clone(),
///         subscription_id: subscription_id.clone(),
///         snapshot: Vec::new(),
///         rows: 24,
///         cols: 80,
///     },
/// ))?;
/// engine.write_bytes(client_id.clone(), session_id.clone(), b"echo docs\n".to_vec(), 2)?;
/// engine.resize(client_id.clone(), session_id.clone(), 40, 120, 3)?;
///
/// let output = engine.receive_output(session_id.clone(), b"docs output\n".to_vec(), 4)?;
/// assert!(output.client_egress.iter().any(|(_, frame)| {
///     matches!(frame, TransportEgress::TerminalOutput { data, .. } if data == b"docs output\n")
/// }));
///
/// let notification_id = engine.post_notification(NotificationItem::message(
///     NotificationId("docs-notification".to_string()),
///     NotificationTarget::Session(session_id.clone()),
///     NotificationSeverity::Info,
///     NotificationSource {
///         label: "docs-host".to_string(),
///         plugin_key: None,
///     },
///     NotificationContent {
///         title: "Docs notice".to_string(),
///         body: None,
///         extension: None,
///     },
///     NotificationTimestamp(5),
/// ));
/// let notifications = engine.drain_notifications(
///     NotificationTarget::Session(session_id.clone()),
///     NotificationTimestamp(6),
/// );
/// assert_eq!(notifications[0].id, notification_id);
///
/// let plugin_key = PluginKey("docs-plugin".to_string());
/// let handler = PluginHandlerRef {
///     plugin_key: plugin_key.clone(),
///     kind: PluginHandlerKind::Command,
///     handler_id: "run".to_string(),
/// };
/// engine.load_plugin(PluginWorkerRegistration {
///     load: PluginLoadSpec {
///         plugin_key: plugin_key.clone(),
///         package: plugin_key.0.clone(),
///         entrypoint: "plugin.lua".to_string(),
///         descriptors: Vec::new(),
///         metadata: None,
///     },
///     manifest: PackageManifest {
///         name: plugin_key.0.clone(),
///         version: "0.1.0".to_string(),
///         kind: ExtensionKind::Plugin,
///         botster: ">=0.1.0".to_string(),
///         source: None,
///         capabilities: Vec::new(),
///         entrypoints: vec![ExtensionEntrypoint {
///             runtime: ExtensionRuntime::Lua,
///             path: "plugin.lua".to_string(),
///             bootstrap: false,
///         }],
///         dependencies: Vec::new(),
///         features: Vec::new(),
///         host_profile: None,
///         configuration: None,
///         runnable_entrypoints: Vec::new(),
///     },
///     runtime: Arc::new(FakePluginRuntime::success("ok")),
///     handlers: vec![PluginHandlerRegistration {
///         handler: handler.clone(),
///         required_capability: None,
///     }],
///     resources: Vec::new(),
/// });
/// let plugin_result = engine.invoke_plugin(PluginInvocationRequest {
///     request_id: RequestId("docs-plugin-request".to_string()),
///     handler,
///     timeout_ms: 1_000,
///     context: PluginInvocationContext {
///         client_id: Some(client_id.clone()),
///         session_id: Some(session_id.clone()),
///         subscription_id: Some(subscription_id.clone()),
///         surface_id: None,
///         origin: Some("docs-host".to_string()),
///         metadata: None,
///     },
///     payload: BoundaryJson(serde_json::json!({ "command": "run" })),
/// });
/// assert!(matches!(plugin_result.result, PluginInvocationResult::Completed(_)));
///
/// assert_eq!(
///     engine.classify_activity(&session_id, 5, 10)?,
///     SessionActivityStatus::Active
/// );
/// engine.shutdown_session(session_id, "docs complete", 7)?;
/// # Ok::<(), botster_core::BotsterEngineError>(())
/// ```
#[derive(Clone)]
pub struct BotsterEngine<R, W> {
    multiplexer: MultiplexerEngine<R, W>,
}

impl<R, W> BotsterEngine<R, W>
where
    R: SessionRuntime,
    W: SessionWorkerRuntime,
{
    /// Build an engine with a host session runtime and default plugin settings.
    pub fn new(session_runtime: R) -> Self {
        Self {
            multiplexer: MultiplexerEngine::new(session_runtime),
        }
    }

    /// Build an engine with explicit plugin worker settings.
    pub fn with_plugin_config(session_runtime: R, plugin_config: PluginWorkerEngineConfig) -> Self {
        Self {
            multiplexer: MultiplexerEngine::with_plugin_config(session_runtime, plugin_config),
        }
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.multiplexer.session(session_id)
    }

    /// Return sessions currently recorded by the command facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.multiplexer.list_sessions()
    }

    /// Return the host runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &R {
        self.multiplexer.session_runtime()
    }

    /// Return the plugin worker engine.
    ///
    /// Hosts that need reload or unload cleanup should call
    /// [`Self::reload_plugin`] or [`Self::unload_plugin`] so scheduler-owned
    /// timer resources are cleaned with worker-owned resources.
    #[must_use]
    pub const fn plugin_workers(&self) -> &PluginWorkerEngine {
        self.multiplexer.plugin_workers()
    }

    /// Return the plugin timer scheduler.
    #[must_use]
    pub const fn plugin_timers(&self) -> &PluginTimerScheduler {
        self.multiplexer.plugin_timers()
    }

    /// Return the lower-level assembled multiplexer engine.
    #[must_use]
    pub const fn multiplexer(&self) -> &MultiplexerEngine<R, W> {
        &self.multiplexer
    }

    /// Spawn a session, record core state, and install its worker adapter.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
        worker_runtime: W,
    ) -> Result<BotsterSpawnOutcome, BotsterEngineError> {
        self.multiplexer
            .spawn_session(request, metadata, worker_runtime)
    }

    /// Execute one typed command through the public engine facade.
    pub fn execute_command(
        &mut self,
        command: EngineCommand<W>,
    ) -> Result<EngineCommandOutcome, EngineCommandError<BotsterEngineError>> {
        let kind = command.kind();
        match command {
            EngineCommand::SpawnSession {
                request,
                metadata,
                worker_runtime,
            } => self
                .spawn_session(request, metadata, worker_runtime)
                .map(EngineCommandOutcome::SpawnSession),
            EngineCommand::AttachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .attach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::DetachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .detach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::SendInput {
                client_id,
                session_id,
                data,
                now_seconds,
            } => self
                .write_bytes(client_id, session_id, data, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::Resize {
                client_id,
                session_id,
                rows,
                cols,
                now_seconds,
            } => self
                .resize(client_id, session_id, rows, cols, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::ListSessions => Ok(EngineCommandOutcome::Sessions(self.list_sessions())),
            EngineCommand::InspectSession {
                session_id,
                now_seconds,
                active_threshold_seconds,
            } => self
                .inspect_session(&session_id, now_seconds, active_threshold_seconds)
                .map(EngineCommandOutcome::Inspection),
            EngineCommand::ReadScreen {
                request_id,
                session_id,
                now_seconds,
            } => self
                .read_screen(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::CaptureSnapshot {
                request_id,
                session_id,
                now_seconds,
            } => self
                .capture_snapshot(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::ReplaySnapshot {
                request,
                now_seconds,
            } => self
                .replay_snapshot(request, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::Shutdown {
                session_id,
                reason,
                now_seconds,
            } => self
                .shutdown_session(session_id, reason, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::PostNotification { item } => Ok(
                EngineCommandOutcome::NotificationPosted(self.post_notification(item)),
            ),
            EngineCommand::DrainNotifications { target, now } => Ok(
                EngineCommandOutcome::NotificationsDrained(self.drain_notifications(target, now)),
            ),
            EngineCommand::LoadPlugin { registration } => {
                let plugin_key = registration.load.plugin_key.clone();
                self.load_plugin(registration);
                Ok(EngineCommandOutcome::PluginLoaded(plugin_key))
            }
            EngineCommand::ReloadPlugin { spec, registration } => Ok(
                EngineCommandOutcome::PluginReloaded(self.reload_plugin(spec, registration)),
            ),
            EngineCommand::UnloadPlugin { spec } => Ok(EngineCommandOutcome::PluginUnloaded(
                self.unload_plugin(spec),
            )),
            EngineCommand::InvokePlugin { request } => Ok(EngineCommandOutcome::PluginInvoked(
                self.invoke_plugin(request),
            )),
        }
        .map_err(|source| EngineCommandError::new(kind, source))
    }

    /// Attach a client to a session stream.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Write terminal bytes from a client into a session.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a client-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Receive terminal output bytes from the host runtime.
    pub fn receive_output(
        &mut self,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
            session_id,
            data: data.into(),
            last_output_at,
        })
    }

    /// Read a session's plain screen state through the session worker path.
    pub fn read_screen(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::GetScreen {
                request_id,
                session_id,
            },
            now_seconds,
        )
    }

    /// Capture a session snapshot through the session worker path.
    pub fn capture_snapshot(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::GetSnapshot {
                request_id,
                session_id,
            },
            now_seconds,
        )
    }

    /// Replay or prepare a snapshot through the session worker path.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::PrepareSnapshot(request),
            now_seconds,
        )
    }

    /// Route one runtime-originated session worker event.
    pub fn handle_runtime_event(
        &mut self,
        event: SessionWorkerRuntimeEvent,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_runtime_event(event)
    }

    /// Report client-side backpressure through the public engine facade.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer
            .report_backpressure(client_id, session_id, source, capacity, depth)
    }

    /// Report accepted-but-slow delivery through the public engine facade.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        )
    }

    /// Report a failed delivery attempt through the public engine facade.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.report_delivery_failure(
            client_id,
            session_id,
            subscription_id,
            source,
            reason,
        )
    }

    /// Queue a notification item in the core inbox.
    pub fn post_notification(&mut self, item: NotificationItem) -> NotificationId {
        self.multiplexer.post_notification(item)
    }

    /// Drain deliverable notifications for one target.
    pub fn drain_notifications(
        &mut self,
        target: NotificationTarget,
        now: NotificationTimestamp,
    ) -> Vec<NotificationItem> {
        self.multiplexer.drain_notifications(target, now)
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        self.multiplexer.load_plugin(registration);
    }

    /// Reload one plugin and cleanup scheduler-owned timer resources.
    pub fn reload_plugin(
        &self,
        spec: PluginReloadSpec,
        registration: PluginWorkerRegistration,
    ) -> PluginCleanupResult {
        self.multiplexer.reload_plugin(spec, registration)
    }

    /// Unload one plugin and cleanup scheduler-owned timer resources.
    pub fn unload_plugin(&self, spec: PluginUnloadSpec) -> PluginCleanupResult {
        self.multiplexer.unload_plugin(spec)
    }

    /// Invoke a registered plugin handler.
    pub fn invoke_plugin(&self, request: PluginInvocationRequest) -> PluginInvocationOutcome {
        self.multiplexer.invoke_plugin(request)
    }

    /// Admit one plugin invocation without waiting for execution or completion.
    pub fn try_admit_plugin(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        self.multiplexer.try_admit_plugin(class, request)
    }

    /// Drain previously published async plugin completions without waiting.
    pub fn drain_plugin_completions(
        &self,
        max_items: usize,
        max_bytes: usize,
    ) -> PluginCompletionDrain {
        self.multiplexer
            .drain_plugin_completions(max_items, max_bytes)
    }

    /// Schedule plugin timer work without invoking plugin code inline.
    pub fn schedule_plugin_timer(
        &self,
        schedule: PluginTimerSchedule,
    ) -> PluginTimerScheduleOutcome {
        self.multiplexer.schedule_plugin_timer(schedule)
    }

    /// Cancel one plugin timer by handle.
    pub fn cancel_plugin_timer(
        &self,
        request_id: crate::RequestId,
        plugin_key: &PluginKey,
        timer_id: &PluginTimerId,
    ) -> PluginTimerCancellationResult {
        self.multiplexer
            .cancel_plugin_timer(request_id, plugin_key, timer_id)
    }

    /// Drain due plugin timers through the existing plugin worker engine.
    pub fn drain_plugin_timers_due(&self, now_ms: u64) -> PluginTimerDrainOutcome {
        self.multiplexer.drain_plugin_timers_due(now_ms)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, BotsterEngineError> {
        self.multiplexer.classify_session_activity(
            session_id,
            now_seconds,
            active_threshold_seconds,
        )
    }

    /// Inspect one session's lifecycle and activity through the command facade.
    pub fn inspect_session(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, BotsterEngineError> {
        Ok(EngineSessionInspection {
            session: self
                .session(session_id)
                .ok_or_else(|| BotsterEngineError::UnknownSession {
                    session_id: session_id.clone(),
                })?
                .clone(),
            activity_status: self.classify_activity(
                session_id,
                now_seconds,
                active_threshold_seconds,
            )?,
        })
    }

    /// Shut down one session worker and update core lifecycle state.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer
            .shutdown_session(session_id, reason, now_seconds)
    }
}

impl<R, W> Default for BotsterEngine<R, W>
where
    R: SessionRuntime + Default,
    W: SessionWorkerRuntime,
{
    fn default() -> Self {
        Self::new(R::default())
    }
}

#[cfg(all(test, feature = "local-runtime"))]
mod capture_identity_tests {
    use super::*;
    use crate::engine::client_worker::RouteResyncRequest;
    use crate::runtime::{ControlFrameClass, ControlPlaneState, ControlQueue, ControlWriterError};

    fn engine() -> WorkerBackedBotsterEngine {
        WorkerBackedBotsterEngine::new("/missing/botster-session-worker")
    }

    fn ids() -> (ClientId, SessionId, SubscriptionId) {
        (
            ClientId("client".into()),
            SessionId("session".into()),
            SubscriptionId("route".into()),
        )
    }

    #[test]
    fn a_queued_request_with_a_stale_attachment_identity_never_starts_work() {
        let mut engine = engine();
        let (client, session, subscription) = ids();
        let (stale_generation, _) = engine
            .runtime
            .client_worker_mut()
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("first attach");
        // A replacement attach by another client supersedes the first owner.
        let other = ClientId("other".into());
        engine
            .runtime
            .client_worker_mut()
            .record_attach(other.clone(), session.clone(), subscription.clone())
            .expect("replacement attach");
        engine.enqueue_capture(
            &session,
            CaptureRequest {
                client_id: other.clone(),
                subscription_id: subscription.clone(),
                kind: CaptureKind::Resync,
                identity: Some(CaptureIdentity {
                    generation: stale_generation,
                    capture_fence: 0,
                }),
            },
        );

        // Without the identity check the request would reach the runtime,
        // fail (no such session), and tear the live route down.
        assert!(engine.captures.is_empty());
        assert!(engine
            .runtime
            .client_worker()
            .has_subscription(&session, &subscription));
        assert!(!engine.capture_queue.contains_key(&session));
    }

    fn route_capture(
        client_id: &ClientId,
        subscription_id: &SubscriptionId,
        request_id: &str,
        identity: Option<CaptureIdentity>,
    ) -> RouteCapture {
        RouteCapture {
            client_id: client_id.clone(),
            subscription_id: subscription_id.clone(),
            kind: CaptureKind::Attach,
            request_id: request_id.to_string(),
            identity,
            ready: false,
            awaiting_release: false,
            history_incomplete: false,
            collected: Vec::new(),
            collected_size: None,
            collected_colors: None,
        }
    }

    /// Attach `client`, then replace it with `other`. Returns the stale
    /// resync request of the first owner and the live identity of the second.
    fn stale_resync_after_replacement(
        engine: &mut WorkerBackedBotsterEngine,
    ) -> (RouteResyncRequest, ClientId, CaptureIdentity) {
        let (client, session, subscription) = ids();
        let worker = engine.runtime.client_worker_mut();
        let (stale_generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("first attach");
        let other = ClientId("other".into());
        worker
            .record_attach(other.clone(), session.clone(), subscription.clone())
            .expect("replacement attach");
        let live = worker
            .capture_identity(&session, &subscription)
            .expect("live identity");
        assert_ne!(live.generation, stale_generation);
        let stale = RouteResyncRequest {
            client_id: client,
            session_id: session,
            subscription_id: subscription,
            generation: stale_generation,
            stream_epoch: 1,
            capture_fence: 0,
        };
        (stale, other, live)
    }

    #[test]
    fn a_stale_resync_request_leaves_the_replacement_active_capture_intact() {
        let mut engine = engine();
        let (_, session, subscription) = ids();
        let (stale, other, live) = stale_resync_after_replacement(&mut engine);
        engine.captures.insert(
            session.clone(),
            route_capture(&other, &subscription, "req-replacement", Some(live)),
        );
        engine
            .runtime
            .client_worker_mut()
            .queue_resync_request(stale);

        engine.start_resync_captures().expect("resync pass");

        let active = engine.captures.get(&session).expect("capture survives");
        assert_eq!(active.request_id, "req-replacement");
        assert_eq!(active.identity, Some(live));
        assert!(
            !engine.capture_queue.contains_key(&session),
            "a stale request must not queue a resync capture"
        );
    }

    #[test]
    fn a_stale_resync_request_leaves_the_replacement_queued_request_intact() {
        let mut engine = engine();
        let (_, session, subscription) = ids();
        let (stale, other, live) = stale_resync_after_replacement(&mut engine);
        // Another capture holds the session; the replacement waits its turn.
        engine.captures.insert(
            session.clone(),
            route_capture(
                &ClientId("host-capture-1".into()),
                &SubscriptionId("host-capture-1".into()),
                "req-host",
                None,
            ),
        );
        engine
            .capture_queue
            .entry(session.clone())
            .or_default()
            .push_back(CaptureRequest {
                client_id: other,
                subscription_id: subscription.clone(),
                kind: CaptureKind::Attach,
                identity: Some(live),
            });
        engine
            .runtime
            .client_worker_mut()
            .queue_resync_request(stale);

        engine.start_resync_captures().expect("resync pass");

        let queued = &engine.capture_queue[&session];
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].kind, CaptureKind::Attach);
        assert_eq!(queued[0].identity, Some(live));
        assert_eq!(engine.captures[&session].request_id, "req-host");
    }

    /// Fill every control slot of the session so no cancel can enter.
    fn saturate_control_queue(
        engine: &WorkerBackedBotsterEngine,
        session: &SessionId,
    ) -> ControlQueue {
        let queue = engine
            .runtime
            .session_runtime()
            .test_control_queue(session)
            .expect("session queue");
        while queue.admit(ControlFrameClass::Ordinary, vec![0]).is_ok() {}
        while queue.admit(ControlFrameClass::Cancel, vec![0]).is_ok() {}
        queue
    }

    #[test]
    fn cancelling_queued_and_active_host_captures_retains_nothing() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        let active = engine.begin_host_capture(&session);
        let queued = engine.begin_host_capture(&session);
        assert_eq!(
            engine.captures[&session].kind,
            CaptureKind::Host(active),
            "the first host capture holds the worker barrier"
        );
        assert_eq!(engine.capture_queue[&session].len(), 1);

        engine.cancel_host_capture(&session, queued);
        assert_eq!(engine.captures[&session].kind, CaptureKind::Host(active));
        assert!(!engine.capture_queue.contains_key(&session));

        engine.cancel_host_capture(&session, active);
        assert!(engine.take_host_capture(active).is_none());
        assert!(engine.take_host_capture(queued).is_none());
        assert!(engine.host_captures.is_empty());
        assert!(!engine.captures.contains_key(&session));
        assert!(!engine.capture_queue.contains_key(&session));
        assert!(
            !engine.pending_barrier_cancels.contains_key(&session),
            "an accepted cancel leaves no obligation behind"
        );
        assert!(
            engine
                .runtime
                .session_runtime_mut()
                .begin_snapshot_boundary(&session)
                .is_ok(),
            "the worker barrier was released"
        );
    }

    #[test]
    fn a_barrier_cancel_refused_by_a_full_control_queue_is_retained_and_retried() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        let active = engine.begin_host_capture(&session);
        let request_id = engine.captures[&session].request_id.clone();
        let queue = saturate_control_queue(&engine, &session);

        engine.cancel_host_capture(&session, active);

        assert!(!engine.captures.contains_key(&session));
        assert_eq!(
            engine.pending_barrier_cancels[&session].request_id, request_id,
            "the refused cancel stays owned by the engine"
        );
        assert!(
            engine
                .runtime
                .session_runtime()
                .snapshot_request_is_outstanding(&session, &request_id),
            "the runtime still holds the barrier request"
        );
        // While the cancel is pending no new capture may begin.
        let next = engine.begin_host_capture(&session);
        assert!(!engine.captures.contains_key(&session));
        assert_eq!(
            engine.capture_queue[&session][0].kind,
            CaptureKind::Host(next)
        );

        // One drained slot: the next wake retries and the queued capture starts.
        assert!(queue.pop().is_some());
        engine.start_next_capture(&session).expect("retry pass");

        assert!(!engine.pending_barrier_cancels.contains_key(&session));
        assert!(!engine
            .runtime
            .session_runtime()
            .snapshot_request_is_outstanding(&session, &request_id));
        assert_eq!(engine.captures[&session].kind, CaptureKind::Host(next));
    }

    #[test]
    fn a_second_cancel_of_the_same_request_is_deduplicated() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        let active = engine.begin_host_capture(&session);
        let request_id = engine.captures[&session].request_id.clone();
        let _queue = saturate_control_queue(&engine, &session);
        let first = Instant::now();
        engine.cancel_capture_boundary_at(&session, request_id.clone(), first);

        engine.cancel_capture_boundary_at(
            &session,
            request_id.clone(),
            first + Duration::from_secs(1),
        );
        engine.cancel_host_capture(&session, active);

        assert_eq!(engine.pending_barrier_cancels.len(), 1);
        let pending = &engine.pending_barrier_cancels[&session];
        assert_eq!(pending.request_id, request_id);
        assert_eq!(
            pending.first_refused, first,
            "the first refusal keeps the bound"
        );
    }

    #[test]
    fn fast_wakes_never_fail_a_healthy_worker_before_the_retry_bound() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        let active = engine.begin_host_capture(&session);
        let _queue = saturate_control_queue(&engine, &session);
        engine.cancel_host_capture(&session, active);
        let first = engine.pending_barrier_cancels[&session].first_refused;

        // Many wakes inside the bound: still pending, control plane live.
        let inside = first + BARRIER_CANCEL_RETRY_BOUND - Duration::from_millis(1);
        for _ in 0..1_000 {
            assert!(!engine.retry_barrier_cancels_at(&session, inside));
        }
        assert!(engine.pending_barrier_cancels.contains_key(&session));
        assert_eq!(
            engine
                .runtime
                .session_runtime()
                .control_plane_state(&session),
            ControlPlaneState::Live
        );

        // The bound itself, still refused: explicit control-plane failure.
        assert!(engine.retry_barrier_cancels_at(&session, first + BARRIER_CANCEL_RETRY_BOUND));
        assert!(!engine.pending_barrier_cancels.contains_key(&session));
        assert_eq!(
            engine
                .runtime
                .session_runtime()
                .control_plane_state(&session),
            ControlPlaneState::Failed(ControlWriterError::DeadlineExpired)
        );
    }

    #[test]
    fn a_pending_cancel_is_discharged_only_when_its_request_is_no_longer_outstanding() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        let active = engine.begin_host_capture(&session);
        let request_id = engine.captures[&session].request_id.clone();
        let _queue = saturate_control_queue(&engine, &session);
        engine.cancel_host_capture(&session, active);
        assert!(engine.pending_barrier_cancels.contains_key(&session));

        // A worker replaced under the same session id holds no such request.
        engine
            .runtime
            .session_runtime_mut()
            .remove_test_session(&session);
        engine
            .runtime
            .session_runtime_mut()
            .insert_test_session(session.clone());
        assert!(!engine
            .runtime
            .session_runtime()
            .snapshot_request_is_outstanding(&session, &request_id));

        assert!(engine.retry_barrier_cancels_at(&session, Instant::now()));
        assert!(!engine.pending_barrier_cancels.contains_key(&session));
        assert_eq!(
            engine
                .runtime
                .session_runtime()
                .control_plane_state(&session),
            ControlPlaneState::Live
        );
        // The new worker can take a barrier at once.
        assert!(engine
            .runtime
            .session_runtime_mut()
            .begin_snapshot_boundary(&session)
            .is_ok());
    }

    #[test]
    fn host_captures_survive_the_route_ownership_reconcile() {
        let mut engine = engine();
        let (_, session, _) = ids();
        engine.captures.insert(
            session.clone(),
            RouteCapture {
                client_id: ClientId("host-capture-9".into()),
                subscription_id: SubscriptionId("host-capture-9".into()),
                kind: CaptureKind::Host(9),
                request_id: "req".into(),
                identity: None,
                ready: false,
                awaiting_release: false,
                history_incomplete: false,
                collected: Vec::new(),
                collected_size: None,
                collected_colors: None,
            },
        );

        engine
            .reconcile_capture_after_teardown(&session)
            .expect("reconcile");

        assert!(engine.captures.contains_key(&session));
    }
}

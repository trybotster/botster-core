//! Runtime interfaces and default adapters for the multiplexer engine.
//!
//! Runtime traits let embedders supply clocks, process/session execution,
//! plugin runtimes, and I/O without coupling core to a specific hub process.
//! The local process adapter is a policy-free default for explicit spawn
//! requests; hosts still decide command, directory, environment, and lifecycle
//! policy before entering core.

pub mod capability;
#[cfg(feature = "local-runtime")]
mod control_queue;
mod file_watch;
#[cfg(feature = "local-runtime")]
mod local_process;
#[cfg(all(feature = "local-runtime", unix))]
pub mod plugin_process;
#[cfg(all(feature = "local-runtime", unix))]
#[allow(
    dead_code,
    reason = "the plugin process host and local shutdown are its first callers"
)]
mod process_exit;
mod session_admission;
#[cfg(feature = "local-runtime")]
mod worker_process;

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::actor::{PluginInvocationRequest, PluginInvocationResult, PluginKey};
use crate::{
    BackpressureSummary, NotificationPayload, ProcessExitedPayload, PromptMarkPayload, RequestId,
    ResizePayload, SessionId, TerminalMetadataShapingObservation,
};

pub use capability::{
    apply_plugin_store_merge_patch, plugin_store_payload_bytes, CapabilityOperation,
    CapabilityOperationCompleted, CapabilityOperationFailure, CapabilityOperationId,
    CapabilityOperationResult, CapabilityResourceEvent, CapabilityResourceId,
    CapabilityRuntimeError, CapabilityRuntimeErrorKind, CapabilityRuntimeEvent,
    CapabilityRuntimeHandle, CapabilityRuntimeRequest, CapabilityTimerEvent, CapabilityWatchEvent,
    CapabilityWebSocketEvent, FilesystemCapabilityGrant, FilesystemCapabilityLimits,
    FilesystemCapabilityPermissions, FilesystemCapabilityRequest, FilesystemCapabilityResult,
    FilesystemEntry, FilesystemEntryKind, FilesystemMetadata, FilesystemOperation,
    HttpCapabilityEndpointPolicy, HttpCapabilityRequest, HttpCapabilityResponse,
    HttpCapabilityRuntime, HttpCapabilityRuntimeConfig, HttpCapabilityTransport, HttpHeader,
    HttpTransportRequest, InMemoryWebSocketCapabilityRuntime, PluginCapabilityRuntime,
    PluginStoreBackend, PluginStoreCapabilityRequest, PluginStoreEntry, PluginStoreKey,
    PluginStoreLimits, PluginStoreOperation, PluginStoreRecord, PluginStoreResult,
    ScopedRelativePath, TimerCapabilityRequest, WatchCapabilityRequest, WatchChangeKind,
    WebSocketCapabilityRequest, WebSocketCapabilityRuntimeConfig, WebSocketMessage,
    DEFAULT_WEBSOCKET_EVENT_CAPACITY, DEFAULT_WEBSOCKET_INBOUND_CAPACITY,
    DEFAULT_WEBSOCKET_OUTBOUND_CAPACITY,
};
#[cfg(feature = "local-runtime")]
pub(crate) use control_queue::ControlAdmission;
#[cfg(feature = "local-runtime")]
pub use control_queue::{
    ControlFrameClass, ControlPlaneState, ControlQueue, ControlQueueAdmitError, ControlWriterError,
    ControlWriterOutcome, ControlWriterSlot, WORKER_CONTROL_QUEUE_FRAMES,
    WORKER_CONTROL_RESERVED_SLOTS, WORKER_CONTROL_WRITER_JOIN_BOUND, WORKER_CONTROL_WRITE_SLICE,
    WORKER_CONTROL_WRITE_TIMEOUT,
};
pub use file_watch::{
    FileWatchEventSource, FileWatchRegistration, FileWatchRuntime, FileWatchRuntimeConfig,
    FileWatchSourceError, FileWatchSourceEvent, DEFAULT_FILE_WATCH_DEBOUNCE_MS,
};
#[cfg(feature = "local-runtime")]
pub use local_process::{
    LocalProcessRuntime, LocalProcessRuntimeOptions, LocalProcessWorkerRuntime, PtyIoBarrier,
    DEFAULT_PTY_READER_CHUNK_CAPACITY,
};
pub(crate) use session_admission::{EngineSessionAdmission, SessionAdmissionOwner};
pub use session_admission::{
    SessionAdmission, SessionReservation, SessionReservationIdentity, SessionReservationRefusal,
    SessionReservationRelease, SessionReservationState,
};
#[cfg(feature = "local-runtime")]
pub use worker_process::{
    ResizeAckHold, RetainedWorkerFinalState, SnapshotCancelAdmission, WorkerHealth,
    WorkerProcessRuntime, WorkerProcessRuntimeOptions, WorkerSpawnPoll,
    DEFAULT_WORKER_EGRESS_CAPACITY, DEFAULT_WORKER_REPLY_TIMEOUT,
};

/// Host-implemented session runtime boundary.
///
/// Core defines this synchronous contract so embedders can adapt it to their
/// own process, thread, Tokio, PTY, or test runtime without `botster-core`
/// selecting one.
pub trait SessionRuntime {
    /// Return the runtime's authoritative admission capability, when supported.
    ///
    /// Existing adapters can keep ordinary spawning without this capability.
    /// Reserved producers must refuse an adapter that returns `None`.
    fn session_admission(&self) -> Option<&SessionAdmission> {
        None
    }

    /// Launch only the process that owns the supplied reservation.
    ///
    /// The default preserves existing adapters without claiming exclusion.
    fn spawn_reserved(
        &mut self,
        _reservation: &SessionReservation,
        _request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, ReservedSessionSpawnError> {
        Err(ReservedSessionSpawnError::Refused(
            SessionReservationRefusal::Unsupported.into(),
        ))
    }

    /// Spawn a new session from an explicit, policy-free request.
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError>;

    /// Deliver input or control data to a spawned session.
    fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError>;

    /// Drain currently available runtime output for one session.
    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError>;
}

/// Failure before or after the runtime consumes a reservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservedSessionSpawnError {
    /// The runtime did not consume the reservation.
    Refused(SessionRuntimeError),
    /// The runtime consumed the reservation and retained execution ownership.
    Admitted(SessionRuntimeError),
}

impl ReservedSessionSpawnError {
    /// Return the error expected by the ordinary synchronous spawn API.
    pub fn into_runtime_error(self) -> SessionRuntimeError {
        match self {
            Self::Refused(error) | Self::Admitted(error) => error,
        }
    }
}

impl fmt::Display for ReservedSessionSpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) | Self::Admitted(error) => error.fmt(formatter),
        }
    }
}

impl Error for ReservedSessionSpawnError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Refused(error) | Self::Admitted(error) => Some(error),
        }
    }
}

impl From<SessionReservationRefusal> for SessionRuntimeError {
    fn from(refusal: SessionReservationRefusal) -> Self {
        Self::new(
            SessionRuntimeErrorKind::SpawnFailed,
            match refusal {
                SessionReservationRefusal::Occupied => "session identity is occupied",
                SessionReservationRefusal::Busy => "session admission is busy",
                SessionReservationRefusal::Unsupported => {
                    "runtime does not support session reservations"
                }
                SessionReservationRefusal::IdentityExhausted => {
                    "session reservation identity is exhausted"
                }
                SessionReservationRefusal::Unavailable => "session admission is unavailable",
                SessionReservationRefusal::InvalidToken => "session reservation token is invalid",
                SessionReservationRefusal::Capacity => "session reservation capacity is full",
            },
        )
    }
}

/// Explicit request for a host runtime to spawn and connect one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSpawnRequest {
    /// Correlation identifier for the spawn request.
    pub request_id: RequestId,
    /// Stable session identifier assigned before host spawning.
    pub session_id: SessionId,
    /// Executable path or command name chosen by the host.
    pub executable: String,
    /// Argument vector supplied without shell expansion.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Working directory selected by the host before entering core.
    pub working_directory: SpawnWorkingDirectory,
    /// Explicit environment variables to set for the child process.
    #[serde(default)]
    pub environment: SpawnEnvironment,
    /// Initial PTY rows and columns, when a PTY-backed runtime needs them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_pty_size: Option<ResizePayload>,
}

/// Working directory contract for a session spawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnWorkingDirectory {
    /// Directory path selected by the host before it builds the spawn request.
    pub path: String,
}

/// Deterministic set-vars environment contract for a session spawn.
///
/// This collection does not model ambient inheritance or variable removal. A
/// host that needs those policies resolves them before building the request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnEnvironment {
    /// Environment variables to set, in deterministic order.
    #[serde(default)]
    pub variables: Vec<SpawnEnvironmentVariable>,
}

/// One explicit environment variable assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnEnvironmentVariable {
    /// Environment variable name.
    pub name: String,
    /// Environment variable value.
    pub value: String,
}

/// Runtime-owned child process identity returned after a successful spawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    /// Operating-system process identifier, when the host exposes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Stable host-side process identifier for runtimes without OS PIDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
}

/// Connected session handle returned by a runtime after spawning succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRuntimeHandle {
    /// Correlation identifier from the spawn request.
    pub request_id: RequestId,
    /// Spawned session identifier.
    pub session_id: SessionId,
    /// Runtime-owned child process identity.
    pub process: ProcessIdentity,
}

/// Input or control data delivered from a host data plane into a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRuntimeInput {
    /// Raw PTY input bytes.
    PtyInput {
        /// Target session identifier.
        session_id: SessionId,
        /// Raw input bytes.
        data: Vec<u8>,
    },
    /// Resize the session PTY to rows and columns.
    Resize {
        /// Target session identifier.
        session_id: SessionId,
        /// Rows and columns for the PTY.
        size: ResizePayload,
    },
    /// Request an orderly session shutdown.
    Shutdown {
        /// Target session identifier.
        session_id: SessionId,
    },
}

/// Output or lifecycle data emitted by a session runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRuntimeOutput {
    /// Raw PTY output bytes.
    PtyOutput {
        /// Source session identifier.
        session_id: SessionId,
        /// Raw output bytes.
        data: Vec<u8>,
    },
    /// Child process exit status.
    ProcessExited {
        /// Source session identifier.
        session_id: SessionId,
        /// Process exit payload reused from the session protocol.
        payload: ProcessExitedPayload,
    },
    /// Terminal title changed.
    TitleChanged {
        /// Source session identifier.
        session_id: SessionId,
        /// Current terminal title.
        title: String,
    },
    /// Terminal working directory changed.
    CwdChanged {
        /// Source session identifier.
        session_id: SessionId,
        /// Current terminal working directory.
        cwd: String,
    },
    /// Semantic prompt mark detected.
    PromptMark {
        /// Source session identifier.
        session_id: SessionId,
        /// Prompt mark payload.
        payload: PromptMarkPayload,
    },
    /// Bell character received.
    Bell {
        /// Source session identifier.
        session_id: SessionId,
    },
    /// OSC notification detected.
    Notification {
        /// Source session identifier.
        session_id: SessionId,
        /// Notification payload.
        payload: NotificationPayload,
    },
    /// Runtime-originated bounded-queue pressure.
    Backpressure(BackpressureSummary),
    /// Runtime-originated terminal metadata lane shaping observation.
    MetadataShaping(TerminalMetadataShapingObservation),
}

/// Stable category for a session runtime error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRuntimeErrorKind {
    /// Spawn request could not be started by the host runtime.
    SpawnFailed,
    /// A requested session is not known to the runtime.
    SessionNotFound,
    /// Runtime input could not be delivered.
    InputFailed,
    /// Runtime output could not be read.
    OutputFailed,
    /// Runtime shutdown could not complete cleanly.
    ShutdownFailed,
    /// Runtime process cleanup failed after shutdown started.
    CleanupFailed,
}

/// Typed error returned by a host session runtime implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRuntimeError {
    /// Stable machine-readable error kind.
    pub kind: SessionRuntimeErrorKind,
    /// Human-readable error detail.
    pub message: String,
}

impl SessionRuntimeError {
    /// Build a typed runtime error.
    pub fn new(kind: SessionRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for SessionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for SessionRuntimeError {}

/// Cooperative cancellation signal for one plugin invocation.
///
/// `PluginWorkerEngine` signals this token when an invocation times out or
/// when its owning plugin is unloaded/reloaded. Runtimes should check it while
/// executing long-running handlers and return promptly once cancellation is
/// requested.
///
/// Core runtimes that wait on something other than their own code (the plugin
/// process host) subscribe a Core-owned [`CancelTarget`] instead of checking
/// the flag in a loop. Subscription is crate-private, so `cancel` never runs
/// host code, whatever locks its caller holds.
#[derive(Clone, Default)]
pub struct PluginCancellationToken {
    inner: Arc<CancellationInner>,
}

/// Core-owned receiver of a cancellation.
///
/// `cancel` may run on any thread that cancels, including the engine's shared
/// deadline waiter and callers that hold engine or capability-runtime locks.
/// An implementation must therefore only record the cancellation and notify
/// its own waiter: it may take only a leaf lock of its own, must not block on
/// anything else, and must not call into the engine or a capability runtime.
/// A panic is contained and does not stop other targets.
pub(crate) trait CancelTarget: Send + Sync + 'static {
    fn cancelled(&self);
}

type CancelWake = Arc<dyn CancelTarget>;

fn run_cancel_target(target: &dyn CancelTarget) {
    // A buggy target must not skip the other targets or unwind the thread
    // that cancelled, which can be the engine's only deadline waiter.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| target.cancelled()));
}

#[derive(Default)]
struct CancellationInner {
    /// Lock-free read path for [`PluginCancellationToken::is_cancelled`].
    /// Written only while `wakers` is locked.
    cancelled: AtomicBool,
    wakers: Mutex<CancellationWakers>,
}

#[derive(Default)]
struct CancellationWakers {
    next_id: u64,
    pending: Vec<(u64, CancelWake)>,
}

impl fmt::Debug for PluginCancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginCancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl PluginCancellationToken {
    /// Build a fresh token in the non-cancelled state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark this invocation as cancelled and notify every subscribed target
    /// once.
    ///
    /// Targets are Core-owned and run on the calling thread after the token's
    /// lock is released; each one is isolated from the others' panics. A
    /// second call does nothing.
    pub fn cancel(&self) {
        let wakes = {
            let mut wakers = self.inner.lock_wakers();
            if self.inner.cancelled.load(Ordering::SeqCst) {
                return;
            }
            self.inner.cancelled.store(true, Ordering::SeqCst);
            std::mem::take(&mut wakers.pending)
        };
        for (_, wake) in &wakes {
            run_cancel_target(wake.as_ref());
        }
    }

    /// Returns true after core has requested cooperative cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    /// Notify `target` once when this token is cancelled.
    ///
    /// Registration and [`cancel`](Self::cancel) are serialized, so no wake is
    /// lost: if the token is already cancelled, `target` is notified at once
    /// on the calling thread. `target` never runs while the token's lock is
    /// held. Dropping the returned subscription before cancellation removes
    /// `target` without notifying it.
    pub(crate) fn subscribe(&self, target: Arc<dyn CancelTarget>) -> CancelSubscription {
        let mut wakers = self.inner.lock_wakers();
        if !self.inner.cancelled.load(Ordering::SeqCst) {
            let id = wakers.next_id;
            wakers.next_id += 1;
            wakers.pending.push((id, target));
            return CancelSubscription {
                inner: Arc::downgrade(&self.inner),
                id: Some(id),
            };
        }
        drop(wakers);
        run_cancel_target(target.as_ref());
        CancelSubscription {
            inner: std::sync::Weak::new(),
            id: None,
        }
    }
}

impl CancellationInner {
    fn lock_wakers(&self) -> std::sync::MutexGuard<'_, CancellationWakers> {
        // Wakes run outside this lock, so a poisoned guard only means a
        // panic in this module's own bookkeeping; the state stays consistent.
        self.wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Registration returned by [`PluginCancellationToken::subscribe`].
///
/// Dropping it before cancellation removes the wake without running it.
#[must_use = "dropping the subscription removes the wake"]
pub(crate) struct CancelSubscription {
    inner: std::sync::Weak<CancellationInner>,
    id: Option<u64>,
}

impl fmt::Debug for CancelSubscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelSubscription")
            .field("armed", &self.id.is_some())
            .finish()
    }
}

impl Drop for CancelSubscription {
    fn drop(&mut self) {
        let (Some(id), Some(inner)) = (self.id, self.inner.upgrade()) else {
            return;
        };
        let removed = {
            let mut wakers = inner.lock_wakers();
            wakers
                .pending
                .iter()
                .position(|(pending, _)| *pending == id)
                .map(|index| wakers.pending.swap_remove(index))
        };
        // Drop the removed target outside the lock: dropping it can run code.
        drop(removed);
    }
}

#[cfg(test)]
#[path = "plugin_cancellation_token_test.rs"]
mod plugin_cancellation_token_test;

/// Host-provided executable runtime for one or more plugin workers.
///
/// `PluginWorkerEngine` invokes this trait across a `std::thread` boundary so
/// core can enforce invocation deadlines without taking a dependency on Tokio.
/// Implementors must therefore be `Send + Sync + 'static`. Runtimes that wrap a
/// `!Send` interpreter need to hide it behind their own worker thread or
/// mailbox before implementing this trait.
pub trait PluginRuntime: Send + Sync + 'static {
    /// Invoke a stable plugin handler request.
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult;

    /// Stop runtime-owned resources for one plugin.
    ///
    /// This must promptly unblock invocations after their cancellation tokens
    /// are signalled. The worker engine calls `stop` before joining the
    /// plugin's executor workers during replacement, reload, unload, and final
    /// engine drop.
    fn stop(&self, _plugin_key: &PluginKey) {}
}

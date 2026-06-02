//! Runtime interfaces and default adapters for the multiplexer engine.
//!
//! Runtime traits let embedders supply clocks, process/session execution,
//! plugin runtimes, and I/O without coupling core to a specific hub process.
//! The local process adapter is a policy-free default for explicit spawn
//! requests; hosts still decide command, directory, environment, and lifecycle
//! policy before entering core.

pub mod capability;
#[cfg(feature = "local-runtime")]
mod local_process;

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::actor::{PluginInvocationRequest, PluginInvocationResult, PluginKey};
use crate::{BackpressureSummary, ProcessExitedPayload, RequestId, ResizePayload, SessionId};

pub use capability::{
    CapabilityOperation, CapabilityOperationCompleted, CapabilityOperationFailure,
    CapabilityOperationId, CapabilityResourceEvent, CapabilityResourceId, CapabilityRuntimeError,
    CapabilityRuntimeErrorKind, CapabilityRuntimeEvent, CapabilityRuntimeHandle,
    CapabilityRuntimeRequest, CapabilityTimerEvent, CapabilityWatchEvent, CapabilityWebSocketEvent,
    FilesystemCapabilityRequest, FilesystemOperation, HttpCapabilityRequest,
    HttpCapabilityResponse, HttpHeader, PluginCapabilityRuntime, PluginStoreCapabilityRequest,
    PluginStoreKey, PluginStoreOperation, ScopedRelativePath, TimerCapabilityRequest,
    WatchCapabilityRequest, WatchChangeKind, WebSocketCapabilityRequest, WebSocketMessage,
};
#[cfg(feature = "local-runtime")]
pub use local_process::{
    LocalProcessRuntime, LocalProcessRuntimeOptions, LocalProcessWorkerRuntime,
    DEFAULT_PTY_READER_CHUNK_CAPACITY,
};

/// Host-implemented session runtime boundary.
///
/// Core defines this synchronous contract so embedders can adapt it to their
/// own process, thread, Tokio, PTY, or test runtime without `botster-core`
/// selecting one.
pub trait SessionRuntime {
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
    /// Runtime-originated bounded-queue pressure.
    Backpressure(BackpressureSummary),
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
#[derive(Debug, Clone, Default)]
pub struct PluginCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl PluginCancellationToken {
    /// Build a fresh token in the non-cancelled state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark this invocation as cancelled.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns true after core has requested cooperative cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

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
    fn stop(&self, _plugin_key: &PluginKey) {}
}

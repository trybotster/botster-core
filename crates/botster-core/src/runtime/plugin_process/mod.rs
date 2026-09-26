//! One plugin per OS process: the policy-free process host mechanism.
//!
//! The parent side spawns a worker binary, supervises it from spawn through
//! `Loaded`, and is the only reaper of its process. The child side
//! ([`worker`]) is a library that the Hub links into its worker binary
//! together with its Lua runtime. Core chooses no sandbox profile and no
//! limit: every value in [`PluginProcessConfig`] is Hub-supplied.
//!
//! See `docs/plans/plugin-process-host.md` for the accepted design.

mod host;
mod launch;
mod protocol;
mod supervisor;
pub mod worker;

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use crate::boundary::BoundaryJson;
use crate::contract::session_protocol::ProtocolError;

pub use host::PluginProcess;
pub use protocol::LoadFrame;

/// Hub-supplied configuration for one plugin process. Core has no defaults.
#[derive(Debug, Clone)]
pub struct PluginProcessConfig {
    /// Worker executable (the Hub's worker binary, or a test worker).
    pub worker_path: PathBuf,
    /// Working directory of the child.
    pub cwd: PathBuf,
    /// The child's whole environment; the parent's environment is cleared.
    pub env: Vec<(OsString, OsString)>,
    /// Resource limits applied in the child before `exec`.
    pub rlimits: PluginProcessRlimits,
    /// Opaque sandbox profile, passed to the worker's `apply_sandbox` hook.
    pub sandbox: BoundaryJson,
    /// Allocator cap installed in the child before plugin code loads.
    pub memory_cap_bytes: Option<u64>,
    /// Largest frame (type byte plus payload) accepted in either direction.
    pub max_frame_bytes: usize,
    /// Bound from spawn through `Loaded`; expiry kills the process group.
    pub startup_deadline: Duration,
    /// Bound from `stop` to exit; expiry kills the process group.
    pub shutdown_deadline: Duration,
    /// Bytes of the child's stderr kept for diagnostics.
    pub stderr_tail_bytes: usize,
}

/// Kernel resource limits for the child. Each set value becomes both the soft
/// and the hard limit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginProcessRlimits {
    /// `RLIMIT_CPU`, in seconds of CPU time.
    pub cpu_seconds: Option<u64>,
    /// `RLIMIT_NOFILE`.
    pub open_files: Option<u64>,
    /// `RLIMIT_CORE`, in bytes.
    pub core_bytes: Option<u64>,
    /// `RLIMIT_AS`, in bytes (enforced by Linux; not by macOS).
    pub address_space_bytes: Option<u64>,
}

/// Why a parent thread killed the process group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginKillReason {
    /// The process did not reach `Loaded` within the startup deadline.
    StartupDeadline,
    /// The worker reported a bootstrap or load failure.
    StartupFailed,
    /// The process did not exit within the shutdown deadline.
    ShutdownDeadline,
    /// The IPC channel closed or failed while the process was alive.
    TransportClosed,
    /// The child sent a frame that breaks the protocol.
    ProtocolViolation(String),
    /// The Hub asked for the kill.
    Requested,
}

/// How a plugin process ended, classified from the evidence of its exit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginExitCause {
    /// A clean exit after `stop`.
    Stopped,
    /// A parent kill that the process died of.
    Killed(PluginKillReason),
    /// The child's allocator cap was exceeded.
    MemoryCap,
    /// The child panicked.
    Panic,
    /// Any other exit: a signal the parent did not send, or an exit code.
    Crashed {
        /// Terminating signal, when the process died of one.
        signal: Option<i32>,
        /// Exit code, when the process exited.
        code: Option<i32>,
    },
}

/// The reaped exit of one plugin process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginProcessExited {
    /// Process id (also the process group id).
    pub pid: u32,
    /// Classified cause.
    pub cause: PluginExitCause,
    /// Last bytes of the child's stderr at the time of the exit.
    pub stderr_tail: Vec<u8>,
    /// Stderr bytes that did not fit the tail.
    pub stderr_dropped_bytes: u64,
}

/// A failed spawn. Every failure after the process started has already
/// killed the group and been reaped; `exit` records how it ended.
#[derive(Debug)]
#[non_exhaustive]
pub enum PluginProcessError {
    /// The configuration cannot be used.
    InvalidConfig(String),
    /// The OS refused to start the worker.
    Launch(std::io::Error),
    /// A frame for the child could not be encoded within the frame bound.
    Encode(ProtocolError),
    /// The worker reported that its bootstrap (cap or sandbox) failed.
    BootstrapFailed {
        /// Worker-reported reason.
        reason: String,
        /// The reaped exit.
        exit: PluginProcessExited,
    },
    /// The worker reported that loading the plugin failed.
    LoadFailed {
        /// Worker-reported reason.
        reason: String,
        /// The reaped exit.
        exit: PluginProcessExited,
    },
    /// The process exited before it reached `Loaded`.
    Exited(PluginProcessExited),
}

impl std::fmt::Display for PluginProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(reason) => write!(f, "invalid plugin process config: {reason}"),
            Self::Launch(error) => write!(f, "cannot start plugin worker: {error}"),
            Self::Encode(error) => write!(f, "cannot encode plugin frame: {error}"),
            Self::BootstrapFailed { reason, .. } => write!(f, "plugin bootstrap failed: {reason}"),
            Self::LoadFailed { reason, .. } => write!(f, "plugin load failed: {reason}"),
            Self::Exited(exit) => {
                write!(f, "plugin process exited during startup: {:?}", exit.cause)
            }
        }
    }
}

impl std::error::Error for PluginProcessError {}

//! The edge traits (plan 2.3): signatures only.
//!
//! A driver calls an edge for one kind of I/O. Each edge has a real implementation (a driver or `botster-core-sys`) and a
//! testkit implementation. A machine never calls an edge: it emits an action, and its driver calls the edge. Later packages
//! extend these traits in their own pull requests; this file holds what P0 can state without inventing a contract.
//!
//! The `Scheduler` edge is in [`crate::scheduler`].

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

/// The clock. The real edge reads `Instant::now()` in the driver only; the testkit edge is the virtual clock.
///
/// Clause: Core TM-1, Core TM-3.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// Random values (plan 2.3a). The real edge is the OS CSPRNG and nothing else. The seeded stream exists only in the testkit.
///
/// Clause: Core AD-6 (the per-worker token), Core SV-1 (the service secret), Core ID-1.
pub trait Entropy {
    /// Fills `buf` with random bytes.
    fn fill(&mut self, buf: &mut [u8]);
}

/// A failed registry operation (plan 2.3, `Storage`).
///
/// Clause: Core AD-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    /// The operation failed and had no effect.
    Failed { errno: i32 },
    /// The effect of the operation is unknown (`RegistryFailed{uncertain: true}`).
    Uncertain { errno: i32 },
}

/// The durable registry: one row per key. A write is atomic and durable when it returns `Ok`.
///
/// Clause: Core AD-1, Core AD-7, Core LC-2.
pub trait Storage {
    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    fn read_row(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete_row(&mut self, key: &str) -> Result<(), StorageError>;
    /// The keys of every row, in ascending order.
    fn list_rows(&self) -> Result<Vec<String>, StorageError>;
}

/// The identity of a process: its pid and its start time. A pid alone can be reused.
///
/// Clause: Core AD-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// The start time in the unit of the edge (the real edge keeps the OS value). Only equality has a meaning.
    pub start_time: u64,
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Code(i32),
    Signal(i32),
}

/// What to start. The environment is exactly `env`; the edge inherits nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub cwd: Option<PathBuf>,
}

/// A refused spawn (`StartFailed{ExecFailed{errno}}`).
///
/// Clause: Core LC-1, Core AD-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnError {
    pub errno: i32,
}

/// A signal for a whole process group.
///
/// Clause: Core SV-9 (SIGTERM, then SIGKILL after `stop_grace`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSignal {
    Term,
    Kill,
    /// The worker-control signal `SIGUSR1`, sent to the verified worker process only, never to its group (LC-5, AD-6).
    /// Meaning: "end your payload: ask it to stop, then kill its group after `stop_grace` from the leader you hold
    /// unreaped, keep running, and keep serving the final model". Sent when the control link is broken. Idempotent: a
    /// worker that already ends its payload ignores a second signal. P3 implements the handler.
    EndPayload,
}

/// What a process identity matches now (AD-6: a process that does not match is never signalled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityState {
    /// A live process has this pid and this start time.
    Matches,
    /// No process with this pid exists.
    Absent,
    /// A process has this pid and a different start time.
    Reused,
}

/// Workers, guardians and services as processes.
///
/// A testkit edge may keep a worker alive with its control link withheld (A6-1).
///
/// Clause: Core AD-2, Core AD-6, Core AD-7, Core LC-1, Core SV-9, Core A6-1.
pub trait Process {
    fn spawn(&mut self, spec: &SpawnSpec) -> Result<ProcessIdentity, SpawnError>;
    /// Signals the process group of `id`, only when `identity_state(id)` is `Matches`.
    fn signal_group(&mut self, id: ProcessIdentity, signal: GroupSignal);
    /// The next process that has ended, or `None`.
    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)>;
    fn identity_state(&self, id: ProcessIdentity) -> IdentityState;
}

/// The size of the terminal window of a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub cols: u16,
    pub rows: u16,
    pub width_px: u16,
    pub height_px: u16,
}

/// The payload on the PTY. A read or a write that cannot proceed returns `io::ErrorKind::WouldBlock`.
///
/// Clause: Core A5-2 (write and read sizes vary), Core A5-3 (`pty_blocked`).
pub trait Program {
    /// Writes input to the program; returns the bytes taken.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize>;
    /// Reads output of the program; `Ok(0)` means the output ended.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn resize(&mut self, size: WindowSize) -> io::Result<()>;
    /// The exit of the payload, once, or `None`.
    fn poll_exit(&mut self) -> Option<ExitStatus>;
}

/// The control link to a worker or a guardian: a byte stream. Descriptor handoff (`SCM_RIGHTS`) is added by the package that
/// owns DP-2.
///
/// A call that cannot proceed returns `io::ErrorKind::WouldBlock`. `recv` returns `Ok(0)` when the peer closed.
///
/// Clause: Core AD-6, Core DP-2, Core DP-8, Core TP-1.
pub trait Link {
    fn send(&mut self, bytes: &[u8]) -> io::Result<usize>;
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn close(&mut self);
}

/// A connected stream of a route, on the worker side. The datagram form of a WebRTC route is added by P4c.
///
/// A call that cannot proceed returns `io::ErrorKind::WouldBlock`. `read` returns `Ok(0)` when the peer closed.
///
/// Clause: Core DP-2, Core DP-3, Core OU-3.
pub trait RouteTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize>;
    fn close(&mut self);
}

/// One accepted connection of a service lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneConnection(pub u64);

/// The lane listeners and connections of a service. Core listens; the service connects.
///
/// A call that cannot proceed returns `io::ErrorKind::WouldBlock`. `read` returns `Ok(0)` when the peer closed.
///
/// Clause: Core SV-2, Core SV-3, Core SV-6, Core A6-1.
pub trait ServiceLane {
    /// Accepts one pending connection on the listener of `lane`, or `None`.
    fn accept(&mut self, lane: u8) -> io::Result<Option<LaneConnection>>;
    fn read(&mut self, connection: LaneConnection, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, connection: LaneConnection, bytes: &[u8]) -> io::Result<usize>;
    fn close(&mut self, connection: LaneConnection);
}

/// The wake of the host loop. It holds the "runnable work exists" flag.
///
/// Clause: Core TM-6, Core TH-2, Core A5-1 (a spurious wake).
pub trait Wake {
    /// Marks work as runnable and wakes a waiter. It never blocks.
    fn signal(&self);
    /// Clears the flag.
    fn drain(&self);
}

/// The id of an open file of a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u64);

/// A file request of the worker machine (plan 2.3b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRequest {
    /// Exclusive create of `name` in `dir`.
    Create {
        file: FileId,
        dir: PathBuf,
        name: String,
    },
    Write {
        file: FileId,
        bytes: Vec<u8>,
    },
    Close {
        file: FileId,
    },
    Delete {
        file: FileId,
    },
}

/// The completion of a file request. It is an input to the same machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCompletion {
    Created { file: FileId, path: PathBuf },
    Written { file: FileId, n: usize },
    Closed { file: FileId },
    Deleted { file: FileId },
    Failed { file: FileId, errno: i32 },
}

/// File input of the worker, outside the readiness loop. The requests of one file run in order.
///
/// Clause: Core DP-5b, Core A2-9.
pub trait FileSink {
    fn submit(&mut self, request: FileRequest);
    /// The next completion, or `None`.
    fn poll_completion(&mut self) -> Option<FileCompletion>;
}

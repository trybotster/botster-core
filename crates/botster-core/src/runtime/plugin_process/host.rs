//! Parent side of one plugin process: startup through `Loaded`, stop, kill,
//! and the exit watch that is the only reaper.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStderr, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Instant;

use crate::boundary::BoundaryJson;
use crate::contract::session_protocol::{Frame, FrameDecoder};
use crate::runtime::process_exit::ExitWatch;

use super::launch::{launch, Launched};
use super::protocol::{
    decode_json, encode_bounded, encode_json_bounded, send_all, BootstrapFrame, FailedFrame,
    LoadFrame, LoadedFrame, ReadyFrame, CAUSE_MEMORY_CAP, CAUSE_PANIC, FRAME_BOOTSTRAP,
    FRAME_BOOTSTRAP_FAILED, FRAME_LOAD, FRAME_LOADED, FRAME_LOAD_FAILED, FRAME_READY,
    FRAME_SHUTDOWN, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
use super::supervisor::{KillState, ProcessKiller, Supervisor};
use super::{
    PluginExitCause, PluginKillReason, PluginProcessConfig, PluginProcessError, PluginProcessExited,
};

/// Callback run once, outside every lock, after the process is reaped.
pub type PluginExitNotifier = Arc<dyn Fn() + Send + Sync + 'static>;

/// A loaded plugin process. Dropping it stops the process; the exit watch
/// reaps it even after the handle is gone.
pub struct PluginProcess {
    shared: Arc<Shared>,
}

struct Shared {
    pid: u32,
    killer: Arc<ProcessKiller>,
    supervisor: Supervisor,
    ipc: UnixStream,
    max_frame_bytes: usize,
    shutdown_deadline: std::time::Duration,
    shutdown_sent: AtomicBool,
    state: Mutex<State>,
    changed: Condvar,
    inbound: Mutex<Inbound>,
    stderr: Arc<Mutex<StderrTail>>,
}

struct State {
    startup: Startup,
    exit: Option<PluginProcessExited>,
    notifier: Option<PluginExitNotifier>,
}

enum Startup {
    AwaitReady,
    AwaitLoaded,
    Loaded(Option<BoundaryJson>),
    BootstrapFailed(String),
    LoadFailed(String),
}

impl std::fmt::Debug for PluginProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginProcess")
            .field("pid", &self.shared.pid)
            .finish_non_exhaustive()
    }
}

impl PluginProcess {
    /// Start a worker, bootstrap it, and load the plugin.
    ///
    /// Returns the handle and the worker's registration once the worker
    /// reports `Loaded`. The startup deadline bounds this call: on expiry,
    /// and on every other failure after the process started, the group is
    /// killed and reaped before the error returns.
    pub fn spawn(
        config: &PluginProcessConfig,
        load: &LoadFrame,
    ) -> Result<(Self, BoundaryJson), PluginProcessError> {
        if config.max_frame_bytes == 0 {
            return Err(PluginProcessError::InvalidConfig(
                "max_frame_bytes must be positive".to_string(),
            ));
        }
        // Encode before starting anything, so an oversize frame starts no process.
        let bootstrap = encode_json_bounded(
            FRAME_BOOTSTRAP,
            &BootstrapFrame {
                magic: PROTOCOL_MAGIC.to_string(),
                version: PROTOCOL_VERSION,
                sandbox: config.sandbox.clone(),
                memory_cap_bytes: config.memory_cap_bytes,
                max_frame_bytes: config.max_frame_bytes,
            },
            config.max_frame_bytes,
        )
        .map_err(PluginProcessError::Encode)?;
        let load = encode_json_bounded(FRAME_LOAD, load, config.max_frame_bytes)
            .map_err(PluginProcessError::Encode)?;

        let shared = start(config)?;
        let startup = shared.supervisor.arm(
            Instant::now() + config.startup_deadline,
            PluginKillReason::StartupDeadline,
        );
        shared.send(&bootstrap);
        let phase = shared.wait_startup(|startup| !matches!(startup, Startup::AwaitReady));
        let phase = match phase {
            Phase::Advanced => {
                shared.send(&load);
                shared.wait_startup(|startup| !matches!(startup, Startup::AwaitLoaded))
            }
            other => other,
        };
        match phase {
            Phase::Advanced => {}
            Phase::Exited(exit) => return Err(PluginProcessError::Exited(exit)),
        }
        let mut state = shared.lock_state();
        let result = match std::mem::replace(&mut state.startup, Startup::Loaded(None)) {
            Startup::Loaded(Some(registration)) => Ok(registration),
            Startup::BootstrapFailed(reason) => Err(Failure::Bootstrap(reason)),
            Startup::LoadFailed(reason) => Err(Failure::Load(reason)),
            Startup::AwaitReady | Startup::AwaitLoaded | Startup::Loaded(None) => {
                unreachable!("wait_startup returns only after the startup phase advanced")
            }
        };
        drop(state);
        match result {
            Ok(registration) => {
                shared.supervisor.disarm(startup);
                Ok((Self { shared }, registration))
            }
            Err(failure) => {
                shared.killer.kill(PluginKillReason::StartupFailed);
                let exit = shared.wait_exit();
                Err(match failure {
                    Failure::Bootstrap(reason) => {
                        PluginProcessError::BootstrapFailed { reason, exit }
                    }
                    Failure::Load(reason) => PluginProcessError::LoadFailed { reason, exit },
                })
            }
        }
    }

    /// Process id, which is also the process group id.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.shared.pid
    }

    /// Ask the worker to exit, bounded by the shutdown deadline. Later calls
    /// do nothing.
    pub fn stop(&self) {
        self.shared.stop();
    }

    /// Kill the process group now.
    pub fn kill(&self) {
        self.shared.killer.kill(PluginKillReason::Requested);
    }

    /// The reaped exit, once the exit watch has recorded it.
    #[must_use]
    pub fn exit(&self) -> Option<PluginProcessExited> {
        self.shared.lock_state().exit.clone()
    }

    /// Run `notifier` once after the process is reaped (at once when it
    /// already is). It runs outside every lock and must return promptly.
    pub fn install_exit_notifier(&self, notifier: PluginExitNotifier) {
        let mut state = self.shared.lock_state();
        if state.exit.is_some() {
            drop(state);
            notifier();
        } else {
            state.notifier = Some(notifier);
        }
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

enum Failure {
    Bootstrap(String),
    Load(String),
}

enum Phase {
    Advanced,
    Exited(PluginProcessExited),
}

impl Shared {
    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Lock order: `inbound` before `state`.
    fn lock_inbound(&self) -> MutexGuard<'_, Inbound> {
        self.inbound.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wait for the startup phase to advance or for the exit. There is no
    /// timer here: the supervisor's startup deadline kills the group, and the
    /// exit watch then records the exit.
    fn wait_startup(&self, advanced: impl Fn(&Startup) -> bool) -> Phase {
        let mut state = self.lock_state();
        loop {
            if advanced(&state.startup) {
                return Phase::Advanced;
            }
            if let Some(exit) = &state.exit {
                return Phase::Exited(exit.clone());
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Wait for the exit watch to record the reaped exit. Callers kill the
    /// group first, so the exit follows as an event.
    fn wait_exit(&self) -> PluginProcessExited {
        let mut state = self.lock_state();
        loop {
            if let Some(exit) = &state.exit {
                return exit.clone();
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Send one encoded frame. A closed or failed channel kills the group:
    /// the process cannot be driven any further.
    fn send(&self, frame: &[u8]) {
        if send_all(self.ipc.as_fd(), frame).is_err() {
            self.killer.kill(PluginKillReason::TransportClosed);
        }
    }

    fn stop(&self) {
        if self.shutdown_sent.swap(true, Ordering::SeqCst) {
            return;
        }
        if self.lock_state().exit.is_some() {
            return;
        }
        // Arm first: the send below can block on a child that does not read,
        // and the deadline kill is what ends that send.
        self.supervisor.arm(
            Instant::now() + self.shutdown_deadline,
            PluginKillReason::ShutdownDeadline,
        );
        match encode_bounded(FRAME_SHUTDOWN, &[], self.max_frame_bytes) {
            Ok(frame) => self.send(&frame),
            Err(_) => self.killer.kill(PluginKillReason::ShutdownDeadline),
        }
    }
}

/// Start the process and its threads. On any failure after the process
/// started, it is killed and reaped before this returns.
fn start(config: &PluginProcessConfig) -> Result<Arc<Shared>, PluginProcessError> {
    let Launched {
        child,
        ipc,
        fatal,
        stderr,
    } = launch(config).map_err(PluginProcessError::Launch)?;
    let pid = child.id();
    let abort = |mut child: Child, error: io::Error| {
        let _ = child.kill();
        let _ = child.wait();
        PluginProcessError::Launch(error)
    };
    let Ok(pgid) = libc::pid_t::try_from(pid) else {
        return Err(abort(child, io::Error::other("pid out of range")));
    };
    // Register while the child is unreaped, so its pid cannot be reused first.
    let watch = match ExitWatch::register(pid) {
        Ok(watch) => watch,
        Err(error) => return Err(abort(child, error)),
    };
    let killer = Arc::new(ProcessKiller::new(pgid));
    let supervisor = match Supervisor::start(killer.clone(), format!("plugin-supervisor-{pid}")) {
        Ok(supervisor) => supervisor,
        Err(error) => return Err(abort(child, error)),
    };
    let reader_ipc = match ipc.try_clone() {
        Ok(stream) => stream,
        Err(error) => {
            supervisor.stop();
            return Err(abort(child, error));
        }
    };
    let stderr_fd = stderr.as_raw_fd();
    let tail = match StderrTail::new(stderr, config.stderr_tail_bytes) {
        Ok(tail) => Arc::new(Mutex::new(tail)),
        Err(error) => {
            supervisor.stop();
            return Err(abort(child, error));
        }
    };
    let shared = Arc::new(Shared {
        pid,
        killer,
        supervisor,
        ipc,
        max_frame_bytes: config.max_frame_bytes,
        shutdown_deadline: config.shutdown_deadline,
        shutdown_sent: AtomicBool::new(false),
        state: Mutex::new(State {
            startup: Startup::AwaitReady,
            exit: None,
            notifier: None,
        }),
        changed: Condvar::new(),
        inbound: Mutex::new(Inbound {
            ipc: reader_ipc,
            decoder: FrameDecoder::with_max_len(config.max_frame_bytes),
            done: false,
        }),
        stderr: tail.clone(),
    });

    let watch_shared = shared.clone();
    if let Err(error) = thread::Builder::new()
        .name(format!("plugin-exit-watch-{pid}"))
        .spawn(move || run_exit_watch(&watch_shared, watch, child, &fatal))
    {
        // The closure (and the child) was dropped unrun; the process group
        // is still unreaped, and nothing else can reap it now.
        shared.killer.kill(PluginKillReason::Requested);
        shared.supervisor.stop();
        return Err(PluginProcessError::Launch(error));
    }
    // From here the exit watch owns the child; failures kill and wait for it.
    let reader_shared = shared.clone();
    let started = thread::Builder::new()
        .name(format!("plugin-reader-{pid}"))
        .spawn(move || run_reader(&reader_shared))
        .and_then(|_| {
            thread::Builder::new()
                .name(format!("plugin-stderr-{pid}"))
                .spawn(move || run_stderr(stderr_fd, &tail))
        });
    if let Err(error) = started {
        shared.killer.kill(PluginKillReason::Requested);
        shared.wait_exit();
        return Err(PluginProcessError::Launch(error));
    }
    Ok(shared)
}

/// The parent's receiving end. Frames are consumed only under this lock: the
/// reader thread takes it when the socket becomes readable, and the exit
/// watch takes it after the reap to handle every frame the child sent before
/// it died. So a typed failure frame is never lost to the exit race.
struct Inbound {
    ipc: UnixStream,
    decoder: FrameDecoder,
    done: bool,
}

enum InboundEnd {
    Closed,
    Violation(String),
}

impl Inbound {
    /// Handle every frame that is readable now, without blocking. Returns
    /// `Some` once the channel is finished.
    fn drain(&mut self, shared: &Shared) -> Option<InboundEnd> {
        let mut buf = vec![0u8; 64 * 1024];
        while !self.done {
            // SAFETY: recv into a live buffer of its stated length; the
            // socket is owned by `self`. MSG_DONTWAIT keeps the shared
            // descriptor blocking for the writer.
            let read = unsafe {
                libc::recv(
                    self.ipc.as_raw_fd(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            let read = match read {
                0 => {
                    self.done = true;
                    return Some(InboundEnd::Closed);
                }
                read if read > 0 => usize::try_from(read).unwrap_or(0),
                _ => {
                    let error = io::Error::last_os_error();
                    match error.kind() {
                        io::ErrorKind::Interrupted => continue,
                        io::ErrorKind::WouldBlock => return None,
                        _ => {
                            self.done = true;
                            return Some(InboundEnd::Closed);
                        }
                    }
                }
            };
            let frames = match self.decoder.feed(&buf[..read]) {
                Ok(frames) => frames,
                Err(error) => {
                    self.done = true;
                    return Some(InboundEnd::Violation(error.to_string()));
                }
            };
            for frame in frames {
                if let Err(violation) = handle_frame(shared, &frame) {
                    self.done = true;
                    return Some(InboundEnd::Violation(violation));
                }
            }
        }
        Some(InboundEnd::Closed)
    }
}

fn run_reader(shared: &Shared) {
    let fd = shared.lock_inbound().ipc.as_raw_fd();
    loop {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll on one valid pollfd; the socket lives in `shared`,
        // which this thread keeps alive. No timeout: readability is the event.
        if unsafe { libc::poll(&mut poll_fd, 1, -1) } < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        let mut inbound = shared.lock_inbound();
        if inbound.done {
            // The exit watch already finished the channel.
            return;
        }
        match inbound.drain(shared) {
            None => {}
            Some(InboundEnd::Closed) => break,
            Some(InboundEnd::Violation(violation)) => {
                drop(inbound);
                shared
                    .killer
                    .kill(PluginKillReason::ProtocolViolation(violation));
                return;
            }
        }
    }
    // The channel is gone. That is not an exit: kill, and let the exit watch
    // record how the process ended.
    shared.killer.kill(PluginKillReason::TransportClosed);
}

fn handle_frame(shared: &Shared, frame: &Frame) -> Result<(), String> {
    let mut state = shared.lock_state();
    let next = match (&state.startup, frame.frame_type) {
        (Startup::AwaitReady, FRAME_READY) => {
            let ready: ReadyFrame = decode_json(frame).map_err(|error| error.to_string())?;
            if ready.magic != PROTOCOL_MAGIC || ready.version != PROTOCOL_VERSION {
                return Err(format!(
                    "Ready carries protocol {} v{}",
                    ready.magic, ready.version
                ));
            }
            if ready.pid != shared.pid {
                return Err(format!("Ready carries pid {}", ready.pid));
            }
            Startup::AwaitLoaded
        }
        (Startup::AwaitReady, FRAME_BOOTSTRAP_FAILED) => {
            let failed: FailedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::BootstrapFailed(failed.reason)
        }
        (Startup::AwaitLoaded, FRAME_LOADED) => {
            let loaded: LoadedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::Loaded(Some(loaded.registration))
        }
        (Startup::AwaitLoaded, FRAME_LOAD_FAILED) => {
            let failed: FailedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::LoadFailed(failed.reason)
        }
        (_, frame_type) => {
            return Err(format!(
                "frame type {frame_type:#04x} is not valid in this phase"
            ))
        }
    };
    state.startup = next;
    drop(state);
    shared.changed.notify_all();
    Ok(())
}

fn run_exit_watch(shared: &Shared, watch: ExitWatch, mut child: Child, fatal: &OwnedFd) {
    // The exit is an OS event; a failed wait falls back to the blocking reap.
    let _ = watch.wait(None);
    let shutdown_sent = shared.shutdown_sent.load(Ordering::SeqCst);
    let cause = shared.killer.reap_with(|kills| {
        let fatal_byte = read_fatal_byte(fatal);
        let status = match child.try_wait() {
            Ok(Some(status)) => Some(status),
            _ => child.wait().ok(),
        };
        classify(kills, fatal_byte, status, shutdown_sent)
    });
    let (stderr_tail, stderr_dropped_bytes) = {
        let mut tail = shared.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        tail.drain();
        tail.snapshot()
    };
    let exit = PluginProcessExited {
        pid: shared.pid,
        cause,
        stderr_tail,
        stderr_dropped_bytes,
    };
    shared.supervisor.stop();
    // Handle every frame the child sent before it died (for example a
    // LoadFailed report), before the exit becomes visible. Violations no
    // longer matter: the process is gone.
    let _ = shared.lock_inbound().drain(shared);
    let _ = shared.ipc.shutdown(Shutdown::Both);
    let notifier = {
        let mut state = shared.lock_state();
        state.exit = Some(exit);
        state.notifier.take()
    };
    shared.changed.notify_all();
    if let Some(notifier) = notifier {
        notifier();
    }
}

fn read_fatal_byte(fatal: &OwnedFd) -> Option<u8> {
    let mut byte = 0u8;
    // SAFETY: a one-byte read into a live local from a descriptor we own.
    let read = unsafe { libc::read(fatal.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
    (read == 1).then_some(byte)
}

/// Classify from exit evidence (plan section 7.5). A kill reason counts only
/// when the process died of SIGKILL and a parent kill was delivered.
fn classify(
    kills: &KillState,
    fatal_byte: Option<u8>,
    status: Option<ExitStatus>,
    shutdown_sent: bool,
) -> PluginExitCause {
    match fatal_byte {
        Some(CAUSE_MEMORY_CAP) => return PluginExitCause::MemoryCap,
        Some(CAUSE_PANIC) => return PluginExitCause::Panic,
        _ => {}
    }
    let signal = status.and_then(|status| status.signal());
    let code = status.and_then(|status| status.code());
    if signal == Some(libc::SIGKILL) {
        if let Some(reason) = &kills.first_delivered {
            return PluginExitCause::Killed(reason.clone());
        }
    }
    if code == Some(0) && shutdown_sent {
        return PluginExitCause::Stopped;
    }
    PluginExitCause::Crashed { signal, code }
}

/// Drain stderr as it becomes readable, so a chatty child never blocks on a
/// full pipe. Reads happen only under the tail lock; the exit watch drains
/// the rest under the same lock after the reap, so the tail holds every byte
/// written before the exit.
fn run_stderr(fd: libc::c_int, tail: &Mutex<StderrTail>) {
    loop {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll on one valid pollfd; the descriptor lives in `tail`,
        // which this thread keeps alive. No timeout: readability is the event.
        let ready = unsafe { libc::poll(&mut poll_fd, 1, -1) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
        if tail.drain() {
            return;
        }
    }
}

/// The child's stderr and its last `capacity` bytes, plus a count of what
/// fell off.
struct StderrTail {
    stderr: ChildStderr,
    eof: bool,
    capacity: usize,
    bytes: VecDeque<u8>,
    dropped: u64,
}

impl StderrTail {
    fn new(stderr: ChildStderr, capacity: usize) -> io::Result<Self> {
        let fd = stderr.as_raw_fd();
        // SAFETY: fcntl get/set on the stderr pipe that this process owns.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            stderr,
            eof: false,
            capacity,
            bytes: VecDeque::with_capacity(capacity.min(64 * 1024)),
            dropped: 0,
        })
    }

    /// Read everything the pipe holds now. Returns true at EOF or on error.
    fn drain(&mut self) -> bool {
        let mut buf = [0u8; 4096];
        while !self.eof {
            match self.stderr.read(&mut buf) {
                Ok(0) => self.eof = true,
                Ok(read) => self.push(&buf[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return false,
                Err(_) => self.eof = true,
            }
        }
        true
    }

    fn push(&mut self, data: &[u8]) {
        self.bytes.extend(data);
        let excess = self.bytes.len().saturating_sub(self.capacity);
        self.bytes.drain(..excess);
        self.dropped += excess as u64;
    }

    fn snapshot(&self) -> (Vec<u8>, u64) {
        (self.bytes.iter().copied().collect(), self.dropped)
    }
}

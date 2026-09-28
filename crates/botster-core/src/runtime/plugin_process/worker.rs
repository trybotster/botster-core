//! Child side of a plugin process: the library that a worker binary runs.
//!
//! The Hub links this into its `botster-plugin-worker` binary together with
//! its Lua runtime and calls [`run_worker`] first thing in `main`. Before any
//! plugin byte is read, the library closes every inherited descriptor except
//! 0-4, installs the fatal panic hook, installs the memory cap, and applies
//! the Hub's sandbox profile.
//!
//! The worker takes one argument, `--botster-plugin-max-frame-bytes <n>`,
//! which the parent launcher passes; the binary must not use other arguments.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{self, Read};
use std::os::fd::{AsFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

pub use super::capped_allocator::CappedAllocator;
pub use super::host_port::{HostPort, HostPortRefusal};
use super::host_port::{Sender, EXIT_PROTOCOL};

use crate::actor::{
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult,
};
use crate::contract::session_protocol::{Frame, FrameDecoder, ProtocolError, MAX_FRAME_LEN};
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;

use super::protocol::{
    decode_json, encode_json_bounded, send_all, BootstrapFrame, CancelFrame, CreditFrame,
    FailedFrame, LoadEnvelope, LoadFrame, LoadedFrame, PluginRegistration, ReadyFrame,
    SandboxProfile, CAUSE_PANIC, CHILD_FATAL_FD, CHILD_IPC_FD, FATAL_MESSAGE_BYTES,
    FRAME_BOOTSTRAP, FRAME_BOOTSTRAP_FAILED, FRAME_CANCEL, FRAME_CREDIT, FRAME_INVOCATION_RESULT,
    FRAME_INVOKE, FRAME_LOAD, FRAME_LOADED, FRAME_LOAD_FAILED, FRAME_READY, FRAME_SHUTDOWN,
    MAX_FRAME_ARG, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};

/// Worker-binary hooks: the policy and the runtime that the Hub supplies.
#[derive(Clone, Copy)]
pub struct WorkerHooks {
    /// Apply the Hub's opaque sandbox profile. Runs before plugin code loads.
    pub apply_sandbox: fn(&SandboxProfile) -> Result<(), String>,
    /// Load the plugin from its in-memory sources. The runtime keeps the
    /// [`HostPort`] for its host calls, replies, and log lines; the port
    /// refuses them until an invocation runs.
    pub load: fn(LoadFrame, HostPort) -> Result<LoadedPlugin, String>,
}

/// A loaded plugin: its runtime and the registration reported to the parent.
pub struct LoadedPlugin {
    /// Runtime that executes the plugin's handlers.
    pub runtime: Arc<dyn PluginRuntime>,
    /// Hub-defined registration, returned to the parent in `Loaded`.
    pub registration: PluginRegistration,
}

/// Exit code for a failure that the worker reported to the parent.
const EXIT_REPORTED_FAILURE: i32 = 1;
/// Exit code when the parent process exited first.
const EXIT_PARENT_GONE: i32 = 3;

/// Run the worker. Never returns.
pub fn run_worker(hooks: WorkerHooks) -> ! {
    close_inherited_descriptors();
    // Before any hook runs: a worker whose parent dies must never outlive it.
    start_parent_watch();
    install_fatal_panic_hook();
    let max_frame_bytes = max_frame_bytes_from_args().unwrap_or_else(|reason| {
        eprintln!("botster plugin worker: {reason}");
        std::process::exit(EXIT_PROTOCOL);
    });
    if !descriptor_is_open(CHILD_IPC_FD) {
        eprintln!("botster plugin worker: no IPC channel at fd {CHILD_IPC_FD}");
        std::process::exit(EXIT_PROTOCOL);
    }
    // SAFETY: fd 3 is open, was placed by the parent launcher, and nothing
    // else in this process owns it.
    let ipc = unsafe { UnixStream::from_raw_fd(CHILD_IPC_FD) };
    let mut channel = Channel::new(ipc, max_frame_bytes);

    let bootstrap: BootstrapFrame = channel.expect(FRAME_BOOTSTRAP, "Bootstrap");
    if bootstrap.magic != PROTOCOL_MAGIC || bootstrap.version != PROTOCOL_VERSION {
        eprintln!(
            "botster plugin worker: protocol {} v{} is not {PROTOCOL_MAGIC} v{PROTOCOL_VERSION}",
            bootstrap.magic, bootstrap.version
        );
        std::process::exit(EXIT_PROTOCOL);
    }
    if let Err(reason) = install_memory_cap(bootstrap.memory_cap_bytes)
        .and_then(|()| (hooks.apply_sandbox)(&bootstrap.sandbox))
    {
        channel.fail(FRAME_BOOTSTRAP_FAILED, reason);
    }
    channel.send(
        FRAME_READY,
        &ReadyFrame {
            magic: PROTOCOL_MAGIC.to_string(),
            version: PROTOCOL_VERSION,
            pid: std::process::id(),
        },
    );

    let envelope: LoadEnvelope<LoadFrame> = channel.expect(FRAME_LOAD, "Load");
    // The sender shares the channel's socket rather than a duplicate, so
    // the plugin sees exactly descriptors 0-4.
    let sender = match Sender::start(
        channel.ipc.clone(),
        channel.max_frame_bytes,
        envelope.grants.max_in_flight_invokes,
    ) {
        Ok(sender) => sender,
        Err(_) => std::process::exit(EXIT_PROTOCOL),
    };
    let port = HostPort::new(sender.clone(), envelope.grants);
    let loaded = match (hooks.load)(envelope.load, port.clone()) {
        Ok(loaded) => loaded,
        Err(reason) => channel.fail(FRAME_LOAD_FAILED, reason),
    };
    let registration = LoadedFrame {
        registration: loaded.registration,
    };
    match encode_json_bounded(FRAME_LOADED, &registration, channel.max_frame_bytes) {
        Ok(frame) => channel.write(&frame),
        Err(error) => channel.fail(
            FRAME_LOAD_FAILED,
            format!("registration does not fit the frame bound: {error}"),
        ),
    }
    sender.start_serving();
    serve(channel, &loaded.runtime, &sender, port)
}

/// Run invocations one at a time, in arrival order, on this thread, while a
/// reader thread takes frames from the parent.
fn serve(
    channel: Channel,
    runtime: &Arc<dyn PluginRuntime>,
    sender: &Arc<Sender>,
    port: HostPort,
) -> ! {
    let queue = Arc::new(InvokeQueue::default());
    let reader_queue = queue.clone();
    let reader_sender = sender.clone();
    if std::thread::Builder::new()
        .name("plugin-worker-reader".to_string())
        .spawn(move || read_parent(channel, &reader_queue, &reader_sender, &port))
        .is_err()
    {
        std::process::exit(EXIT_PROTOCOL);
    }
    loop {
        let (request, cancellation) = queue.next();
        sender.begin(request.request_id.clone());
        let result = runtime.invoke(request, cancellation);
        queue.finished();
        // Clears the running invocation and sends its result under one
        // lock, so none of its host calls can follow the result.
        sender.finish(FRAME_INVOCATION_RESULT, &result);
    }
}

/// Take frames from the parent until Shutdown or EOF, which end the worker.
fn read_parent(mut channel: Channel, queue: &InvokeQueue, sender: &Sender, port: &HostPort) {
    loop {
        match channel.next() {
            None => std::process::exit(0),
            Some(frame) => match frame.frame_type {
                FRAME_SHUTDOWN => {
                    // Exit once the frames already queued are written.
                    sender.shutdown();
                    return;
                }
                FRAME_INVOKE => match decode_json::<PluginInvocationRequest>(&frame) {
                    Ok(request) => queue.push(request),
                    Err(error) => {
                        eprintln!("botster plugin worker: bad Invoke frame: {error}");
                        std::process::exit(EXIT_PROTOCOL);
                    }
                },
                FRAME_CREDIT => {
                    let applied = decode_json::<CreditFrame>(&frame)
                        .map_err(|error| error.to_string())
                        .and_then(|credit| port.credit(credit));
                    if let Err(error) = applied {
                        eprintln!("botster plugin worker: bad Credit frame: {error}");
                        std::process::exit(EXIT_PROTOCOL);
                    }
                }
                FRAME_CANCEL => match decode_json::<CancelFrame>(&frame) {
                    Ok(cancel) => {
                        if let Some(request) = queue.cancel(&cancel.request_id) {
                            sender.send_result(FRAME_INVOCATION_RESULT, &cancelled(request));
                        }
                    }
                    Err(error) => {
                        eprintln!("botster plugin worker: bad Cancel frame: {error}");
                        std::process::exit(EXIT_PROTOCOL);
                    }
                },
                other => {
                    eprintln!("botster plugin worker: unexpected frame type {other:#04x}");
                    std::process::exit(EXIT_PROTOCOL);
                }
            },
        }
    }
}

fn cancelled(request: PluginInvocationRequest) -> PluginInvocationResult {
    PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id,
        handler: request.handler,
        kind: PluginInvocationFailureKind::Cancelled,
        timeout_ms: None,
        reason: "cancelled before it started".to_string(),
    })
}

/// Invocations waiting to run, and the one running now.
#[derive(Default)]
struct InvokeQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

#[derive(Default)]
struct QueueState {
    waiting: VecDeque<PluginInvocationRequest>,
    running: Option<(RequestId, PluginCancellationToken)>,
}

impl InvokeQueue {
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, request: PluginInvocationRequest) {
        self.lock().waiting.push_back(request);
        self.ready.notify_one();
    }

    /// The next invocation to run, marked running with a fresh token.
    fn next(&self) -> (PluginInvocationRequest, PluginCancellationToken) {
        let mut state = self.lock();
        loop {
            if let Some(request) = state.waiting.pop_front() {
                let cancellation = PluginCancellationToken::new();
                state.running = Some((request.request_id.clone(), cancellation.clone()));
                return (request, cancellation);
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn finished(&self) {
        self.lock().running = None;
    }

    /// Cancel `request_id`: a waiting invocation is removed and returned, to
    /// be answered at once; a running one has its token cancelled. An id
    /// that is neither has already finished.
    fn cancel(&self, request_id: &RequestId) -> Option<PluginInvocationRequest> {
        let mut state = self.lock();
        if let Some(index) = state
            .waiting
            .iter()
            .position(|request| &request.request_id == request_id)
        {
            return state.waiting.remove(index);
        }
        let running = state
            .running
            .as_ref()
            .filter(|(running, _)| running == request_id)
            .map(|(_, cancellation)| cancellation.clone());
        drop(state);
        if let Some(cancellation) = running {
            cancellation.cancel();
        }
        None
    }
}

/// Read `--botster-plugin-max-frame-bytes <n>` from argv.
fn max_frame_bytes_from_args() -> Result<usize, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, value] if flag == MAX_FRAME_ARG => match value.parse::<usize>() {
            Ok(bytes) if (1..=MAX_FRAME_LEN).contains(&bytes) => Ok(bytes),
            _ => Err(format!(
                "{MAX_FRAME_ARG} must be between 1 and {MAX_FRAME_LEN}, got {value:?}"
            )),
        },
        _ => Err(format!("expected exactly `{MAX_FRAME_ARG} <bytes>`")),
    }
}

/// The child's end of the IPC channel, bounded by the negotiated frame size
/// in both directions from the first frame on.
pub(super) struct Channel {
    ipc: Arc<UnixStream>,
    decoder: FrameDecoder,
    pending: VecDeque<Frame>,
    max_frame_bytes: usize,
}

impl Channel {
    pub(super) fn new(ipc: UnixStream, max_frame_bytes: usize) -> Self {
        Self {
            ipc: Arc::new(ipc),
            decoder: FrameDecoder::with_max_len(max_frame_bytes),
            pending: VecDeque::new(),
            max_frame_bytes,
        }
    }

    /// The next frame, `Ok(None)` at EOF, or the decode error of a frame
    /// that breaks the negotiated bound.
    pub(super) fn recv(&mut self) -> Result<Option<Frame>, ProtocolError> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(Some(frame));
            }
            let read = match (&*self.ipc).read(&mut buf) {
                Ok(0) => return Ok(None),
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(ProtocolError::Io(error)),
            };
            self.pending.extend(self.decoder.feed(&buf[..read])?);
        }
    }

    /// The next frame, or `None` at EOF. A broken stream ends the worker.
    fn next(&mut self) -> Option<Frame> {
        match self.recv() {
            Ok(frame) => frame,
            Err(ProtocolError::Io(_)) => None,
            Err(error) => {
                eprintln!("botster plugin worker: bad frame from parent: {error}");
                std::process::exit(EXIT_PROTOCOL);
            }
        }
    }

    fn expect<T: serde::de::DeserializeOwned>(&mut self, frame_type: u8, name: &str) -> T {
        let Some(frame) = self.next() else {
            std::process::exit(0);
        };
        if frame.frame_type != frame_type {
            eprintln!(
                "botster plugin worker: expected {name}, got frame type {:#04x}",
                frame.frame_type
            );
            std::process::exit(EXIT_PROTOCOL);
        }
        decode_json(&frame).unwrap_or_else(|error| {
            eprintln!("botster plugin worker: bad {name} frame: {error}");
            std::process::exit(EXIT_PROTOCOL);
        })
    }

    fn send<T: serde::Serialize>(&mut self, frame_type: u8, value: &T) {
        match encode_json_bounded(frame_type, value, self.max_frame_bytes) {
            Ok(frame) => self.write(&frame),
            Err(error) => {
                eprintln!("botster plugin worker: cannot encode frame: {error}");
                std::process::exit(EXIT_PROTOCOL);
            }
        }
    }

    fn write(&mut self, frame: &[u8]) {
        if send_all(self.ipc.as_fd(), frame).is_err() {
            std::process::exit(EXIT_PROTOCOL);
        }
    }

    /// Report a failure to the parent and exit.
    fn fail(&mut self, frame_type: u8, reason: String) -> ! {
        let mut reason = reason;
        // Keep the report inside the frame bound; the reason is diagnostic.
        let budget = self.max_frame_bytes.saturating_sub(64);
        if reason.len() > budget {
            let mut end = budget;
            while !reason.is_char_boundary(end) {
                end -= 1;
            }
            reason.truncate(end);
        }
        self.send(frame_type, &FailedFrame { reason });
        std::process::exit(EXIT_REPORTED_FAILURE);
    }
}

fn install_memory_cap(cap: Option<u64>) -> Result<(), String> {
    match cap {
        None => Ok(()),
        Some(cap) => super::capped_allocator::install(cap),
    }
}

/// On panic: publish the cause and a short message through the fatal pipe,
/// then abort. The previous hook never runs: it is worker-host code, and even
/// the standard hook can block on the stderr lock or a full stderr pipe
/// before the cause is published. Formatting uses a stack buffer, and the one
/// write is non-blocking, so nothing here can stall termination.
fn install_fatal_panic_hook() {
    drop(std::panic::take_hook());
    std::panic::set_hook(Box::new(|info| {
        let mut message = FatalMessage::new();
        message.bytes[0] = CAUSE_PANIC;
        let _ = write!(message, "{info}");
        write_fatal(&message.bytes[..message.len]);
        std::process::abort();
    }));
}

/// Publish a fatal cause byte (and optional message) with one non-blocking,
/// allocation-free write to the fatal pipe.
pub(crate) fn write_fatal(bytes: &[u8]) {
    // SAFETY: one write from a live buffer to the fatal-cause pipe; write is
    // async-signal-safe and allocation-free, and the pipe is non-blocking.
    let _ = unsafe { libc::write(CHILD_FATAL_FD, bytes.as_ptr().cast(), bytes.len()) };
}

/// A fixed stack buffer: the cause byte, then a truncated message.
struct FatalMessage {
    bytes: [u8; 1 + FATAL_MESSAGE_BYTES],
    len: usize,
}

impl FatalMessage {
    fn new() -> Self {
        Self {
            bytes: [0; 1 + FATAL_MESSAGE_BYTES],
            len: 1,
        }
    }
}

impl std::fmt::Write for FatalMessage {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let room = self.bytes.len() - self.len;
        let take = text.len().min(room);
        self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

/// Exit as soon as the parent process exits, whatever the worker is doing.
///
/// Once serving, the channel's EOF already ends the worker. Before that, the
/// main thread may run the Hub's sandbox or load hook for as long as it
/// likes, and nothing reads the channel. A parent that dies without its
/// group kill (a crash, SIGKILL, a killed test binary) would then leave the
/// worker running forever. This watch runs from before the first hook, on
/// its own thread, and ends the worker when the parent exits. It holds one
/// descriptor, the first free one after the sweep (fd 5), close-on-exec.
fn start_parent_watch() {
    // SAFETY: getppid has no preconditions.
    let parent = unsafe { libc::getppid() };
    if parent <= 1 {
        parent_gone();
    }
    let watch = match ParentExit::register(parent) {
        Ok(Some(watch)) => watch,
        Ok(None) => parent_gone(),
        Err(error) => {
            eprintln!("botster plugin worker: cannot watch the parent process: {error}");
            std::process::exit(EXIT_PROTOCOL);
        }
    };
    // The parent may have exited before the registration; the worker was
    // then already reparented.
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != parent {
        parent_gone();
    }
    if std::thread::Builder::new()
        .name("plugin-worker-parent-watch".to_string())
        .spawn(move || {
            watch.wait();
            parent_gone();
        })
        .is_err()
    {
        std::process::exit(EXIT_PROTOCOL);
    }
}

/// End the worker at once. `_exit` runs no exit handlers, so it cannot wait
/// on a lock that plugin code holds.
fn parent_gone() -> ! {
    // SAFETY: _exit only ends the process.
    unsafe { libc::_exit(EXIT_PARENT_GONE) }
}

/// An event that fires once when one process exits: kqueue `NOTE_EXIT` on
/// macOS, a pidfd on Linux.
pub(crate) struct ParentExit {
    fd: std::os::fd::OwnedFd,
}

impl ParentExit {
    /// Register for the exit of `pid`. `Ok(None)` means it is already gone.
    pub(crate) fn register(pid: libc::pid_t) -> io::Result<Option<Self>> {
        use std::os::fd::FromRawFd as _;
        #[cfg(target_os = "macos")]
        {
            // SAFETY: kqueue has no preconditions.
            let kq = unsafe { libc::kqueue() };
            if kq < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: kq is a new descriptor that nothing else owns.
            let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(kq) };
            // SAFETY: fcntl on a descriptor this process owns.
            unsafe { libc::fcntl(kq, libc::F_SETFD, libc::FD_CLOEXEC) };
            let change = libc::kevent {
                ident: pid as libc::uintptr_t,
                filter: libc::EVFILT_PROC,
                flags: libc::EV_ADD | libc::EV_ONESHOT,
                fflags: libc::NOTE_EXIT,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            // SAFETY: one valid change record and no output buffer.
            let added =
                unsafe { libc::kevent(kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
            if added < 0 {
                let error = io::Error::last_os_error();
                return if error.raw_os_error() == Some(libc::ESRCH) {
                    Ok(None)
                } else {
                    Err(error)
                };
            }
            Ok(Some(Self { fd }))
        }
        #[cfg(target_os = "linux")]
        {
            // SAFETY: pidfd_open takes a pid and flags; the result is a new
            // close-on-exec descriptor or -1.
            let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            if raw < 0 {
                let error = io::Error::last_os_error();
                return if error.raw_os_error() == Some(libc::ESRCH) {
                    Ok(None)
                } else {
                    Err(error)
                };
            }
            // SAFETY: raw is a new descriptor that nothing else owns.
            let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw as libc::c_int) };
            Ok(Some(Self { fd }))
        }
    }

    /// Block until the process exits. There is no timer: the exit is the
    /// event. An unexpected wait error also returns, so a broken watch ends
    /// the worker rather than spinning or leaving it unwatched.
    pub(crate) fn wait(&self) {
        use std::os::fd::AsRawFd as _;
        loop {
            #[cfg(target_os = "macos")]
            let fired = {
                // SAFETY: a zeroed kevent is a valid output record.
                let mut event: libc::kevent = unsafe { std::mem::zeroed() };
                // SAFETY: no changes and one output record; no timeout.
                unsafe {
                    libc::kevent(
                        self.fd.as_raw_fd(),
                        std::ptr::null(),
                        0,
                        &mut event,
                        1,
                        std::ptr::null(),
                    )
                }
            };
            #[cfg(target_os = "linux")]
            let fired = {
                let mut poll_fd = libc::pollfd {
                    fd: self.fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd; no timeout.
                unsafe { libc::poll(&mut poll_fd, 1, -1) }
            };
            if fired > 0 {
                return;
            }
            if fired < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return;
            }
        }
    }
}

fn descriptor_is_open(fd: libc::c_int) -> bool {
    // SAFETY: F_GETFD only queries the descriptor table.
    unsafe { libc::fcntl(fd, libc::F_GETFD) >= 0 }
}

/// Close every descriptor above the fatal-cause pipe. This covers
/// descriptors that the parent's libraries opened without close-on-exec,
/// and the launcher's intermediate copies.
fn close_inherited_descriptors() {
    #[cfg(target_os = "linux")]
    const FD_DIR: &str = "/proc/self/fd";
    #[cfg(not(target_os = "linux"))]
    const FD_DIR: &str = "/dev/fd";
    let Ok(entries) = std::fs::read_dir(FD_DIR) else {
        eprintln!("botster plugin worker: cannot list {FD_DIR}");
        std::process::exit(EXIT_PROTOCOL);
    };
    let open: Vec<libc::c_int> = entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > CHILD_FATAL_FD)
        .collect();
    // The directory's own descriptor is closed by now; closing it again only
    // returns EBADF.
    for fd in open {
        // SAFETY: closing a descriptor number this process may own; no Rust
        // object owns any of them this early in the worker.
        unsafe { libc::close(fd) };
    }
}

#[cfg(test)]
#[path = "worker_test.rs"]
mod worker_test;

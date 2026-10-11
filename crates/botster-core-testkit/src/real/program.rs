//! The program controls of the real tier: `pty_output`, `pty_input`, `pty_blocked`, `pty_accept` and `pty_chunk`, through the
//! control socket of the production probe (`botster_probe_script::control`; steward approval of 2026-10-10). The real forms are
//! P3's statement (`p3-program-controls-real-forms.md`): the payload is the production probe on a real PTY that the production
//! worker started, and the harness never touches the PTY.
//!
//! [`Programs`] listens on `<root>/p`. Both real harnesses name that path in `probe_env`, so the driver puts it in the `env` of
//! every `Create` and no transcript names it (Core A5-4). Each probe connects after its terminal setup (R-48) and before its first
//! step, and its hello names its pid and its parent's. The parent is the session's worker, which spawns the payload as its
//! direct child, so a control finds its probe by the worker of Core's registry row (a fresh start stores no payload in the row).
//!
//! A control returns when the probe acknowledges the request, so the request is in effect (P3 rule 4: it never waits for a
//! write to complete). A probe that has not connected yet is waited for: the connection, or the worker's exit, ends the wait. Each
//! input byte that a probe reports goes to its log, and the wake handle of every open handle is signalled, because a read of the
//! probe gives Core no wake and `await_pty` waits on that handle (TM-6). The waits are on these real events, bounded by the
//! guards' cleanup bound: a probe connects right after its terminal setup and acknowledges at once, so a wait that reaches the
//! bound is a setup failure.

use super::RealCoreHarness;
use crate::controls::parse;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_host::driver::HostWake;
use botster_probe_script::control::{Report, Request, CONNECT_FAILED_EXIT, CONTROL_ENV};
use botster_route_codec::prelude::{hex_decode, HexBytes};
use botster_test_process::platform;
use botster_test_process::{Bounded, Deadline, ReadError};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::JoinHandle;

/// The controls that this module serves.
pub(super) const CONTROLS: [&str; 5] = [
    "pty_output",
    "pty_input",
    "pty_blocked",
    "pty_accept",
    "pty_chunk",
];

/// One probe's connection.
struct Probe {
    /// The write end of the socket: the requests. A control writes it outside the state lock (`send_line`), so the reader
    /// of the reports never waits for a write.
    requests: Arc<UnixStream>,
    /// Every byte that the probe reported reading, in order.
    input: Vec<u8>,
    /// The requests sent and the acknowledgements received.
    sent: u64,
    acked: u64,
    /// The socket ended: the probe exited.
    closed: bool,
}

#[derive(Default)]
struct State {
    /// The probes, by the identity of their parent (pid and start time): the session's worker. A reused pid is another
    /// identity, so it never finds the probe of an earlier worker.
    probes: BTreeMap<ProcessIdentity, Probe>,
    /// The wake handles of the handles that `open` opened.
    wakes: Vec<Weak<dyn HostWake>>,
    /// Every accepted connection, so that the drop can end its reader.
    connections: Vec<UnixStream>,
    closing: bool,
}

#[derive(Default)]
struct Hub {
    state: Mutex<State>,
    changed: Condvar,
}

impl Hub {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for the next change of the state, by `deadline`; `None` once the deadline has passed. A probe connects right
    /// after its terminal setup and acknowledges a request as soon as it is in effect, so a wait that reaches the deadline is
    /// a setup failure, never a slow probe.
    fn wait<'a>(
        &self,
        state: MutexGuard<'a, State>,
        deadline: Deadline,
    ) -> Option<MutexGuard<'a, State>> {
        if deadline.expired() {
            return None;
        }
        let (state, _) = self
            .changed
            // timer: deadline — the cleanup bound of a control's wait for its probe
            .wait_timeout(state, deadline.remaining())
            .unwrap_or_else(PoisonError::into_inner);
        Some(state)
    }

    /// Signals every live wake handle, outside the state lock.
    fn wake_hosts(&self) {
        let wakes: Vec<Arc<dyn HostWake>> =
            self.lock().wakes.iter().filter_map(Weak::upgrade).collect();
        for wake in wakes {
            wake.signal();
        }
    }
}

/// The harness's end of the probes' control sockets.
pub(super) struct Programs {
    path: PathBuf,
    hub: Arc<Hub>,
    threads: Vec<JoinHandle<()>>,
}

impl Programs {
    /// Listens on `<root>/p`.
    ///
    /// # Errors
    /// The socket could not be bound, or its thread could not start.
    pub(super) fn listen(root: &Path) -> io::Result<Programs> {
        let path = root.join("p");
        let listener = UnixListener::bind(&path)?;
        let hub = Arc::new(Hub::default());
        let accepting = Arc::clone(&hub);
        let accept = std::thread::Builder::new()
            .name("probe-control-accept".into())
            .spawn(move || accept(&listener, &accepting))?;
        Ok(Programs {
            path,
            hub,
            threads: vec![accept],
        })
    }

    /// The variables that the driver adds to each `Create` (`CoreHarness::probe_env`).
    pub(super) fn env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([(CONTROL_ENV.to_string(), self.path.display().to_string())])
    }

    /// Signals `wake` whenever a probe reports input, while the handle that owns it lives.
    pub(super) fn watch(&self, wake: &Arc<dyn HostWake>) {
        self.hub.lock().wakes.push(Arc::downgrade(wake));
    }

    /// Waits until the probe of the session whose worker is `worker` connected, by the cleanup bound. A worker that is no
    /// longer the live process of its identity (it exited, or its pid was reused) fails at once, and at the bound: a worker ends
    /// with its payload, so a probe that exited (69 included: it could not connect) never connects.
    fn connected(&self, worker: ProcessIdentity) -> Result<MutexGuard<'_, State>, ControlError> {
        let deadline = Deadline::cleanup();
        let mut state = self.hub.lock();
        loop {
            if state.probes.contains_key(&worker) {
                return Ok(state);
            }
            if !live(worker) {
                return Err(ControlError::Bad(format!(
                    "the worker {} exited before its probe connected to the control socket (a probe that cannot connect \
                     exits {CONNECT_FAILED_EXIT}): a setup failure",
                    worker.pid
                )));
            }
            state = self.hub.wait(state, deadline).ok_or_else(|| {
                ControlError::Bad(format!(
                    "the probe of the worker {} did not connect to the control socket within {:?} (is {CONTROL_ENV} in \
                     its environment?): a setup failure",
                    worker.pid,
                    deadline.limit()
                ))
            })?;
        }
    }

    /// Sends `request` to the probe of the session whose worker is `worker`, and returns when the probe acknowledged it. The
    /// write and the wait for the acknowledgement share one cleanup bound; the write holds no lock.
    fn request(&self, worker: ProcessIdentity, request: &Request) -> Result<(), ControlError> {
        let pid = worker.pid;
        let deadline = Deadline::cleanup();
        let (requests, target) = {
            let mut state = self.connected(worker)?;
            let probe = state.probes.get_mut(&worker).expect("connected");
            if probe.closed {
                return Err(exited(pid));
            }
            probe.sent += 1;
            (Arc::clone(&probe.requests), probe.sent)
        };
        let mut line = serde_json::to_vec(request).expect("a request serializes");
        line.push(b'\n');
        send_line(&requests, &line, deadline).map_err(|e| {
            ControlError::Bad(format!(
                "the request {request:?} to the probe of the worker {pid} was not written within {:?}: {e}",
                deadline.limit()
            ))
        })?;
        let mut state = self.hub.lock();
        loop {
            let probe = &state.probes[&worker];
            if probe.acked == target {
                return Ok(());
            }
            if probe.closed {
                return Err(exited(pid));
            }
            state = self.hub.wait(state, deadline).ok_or_else(|| {
                ControlError::Bad(format!(
                    "the probe of the worker {pid} did not acknowledge {request:?} within {:?}: a setup failure",
                    deadline.limit()
                ))
            })?;
        }
    }

    /// Every byte that the probe of the session whose worker is `worker` reported reading, in order.
    fn input(&self, worker: ProcessIdentity) -> Result<Vec<u8>, ControlError> {
        let state = self.connected(worker)?;
        Ok(state.probes[&worker].input.clone())
    }
}

impl Drop for Programs {
    /// Stops the accept thread (one connection of its own wakes its poll) and ends every reader (the shutdown of its socket),
    /// then joins them. No other thread exists.
    fn drop(&mut self) {
        let connections = {
            let mut state = self.hub.lock();
            state.closing = true;
            std::mem::take(&mut state.connections)
        };
        let _ = UnixStream::connect(&self.path);
        for connection in connections {
            let _ = connection.shutdown(Shutdown::Both);
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Whether `worker` is still the live process of its identity: a reused pid is another process (R-46).
fn live(worker: ProcessIdentity) -> bool {
    identity(worker.pid) == Some(worker)
}

/// The identity of the live process `pid`, as the real edge records it.
fn identity(pid: u32) -> Option<ProcessIdentity> {
    let start_time = platform::pid(pid).and_then(platform::start_time).ok()??;
    Some(ProcessIdentity { pid, start_time })
}

/// Writes `line` by `deadline` to a non-blocking socket: a write that finds no room waits in `poll` until the socket takes
/// bytes, and never blocks. No write starts after the deadline.
fn send_line(stream: &UnixStream, line: &[u8], deadline: Deadline) -> io::Result<()> {
    let mut at = 0;
    while at < line.len() {
        if deadline.expired() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match (&*stream).write(&line[at..]) {
            Ok(n) => at += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                let mut ready = [PollFd::new(stream, PollFlags::OUT)];
                let left = Timespec::try_from(deadline.remaining()).map_err(io::Error::other)?;
                // timer: deadline — bounds the wait for room in the request socket
                match poll(&mut ready, Some(&left)) {
                    Ok(_) | Err(Errno::INTR) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn exited(worker: u32) -> ControlError {
    ControlError::Bad(format!(
        "the payload of the worker {worker} has exited: it writes and reads nothing more"
    ))
}

/// What the accept loop does after an accept error.
#[derive(Debug, PartialEq, Eq)]
enum AcceptError {
    /// No connection is queued: wait for one.
    Wait,
    /// A peer that reset before the accept: the next connection is accepted as usual.
    Next,
    /// Any other error: the listener is unusable, so no probe connects any more.
    Stop,
}

fn after_accept(kind: io::ErrorKind) -> AcceptError {
    match kind {
        io::ErrorKind::WouldBlock => AcceptError::Wait,
        io::ErrorKind::ConnectionAborted => AcceptError::Next,
        _ => AcceptError::Stop,
    }
}

/// The accept thread: one reader thread for each connection, until the drop.
fn accept(listener: &UnixListener, hub: &Arc<Hub>) {
    let mut readers = vec![];
    // botster-test-process has no bounded accept yet (P6 adds one in its next crate PR, which replaces this loop). The
    // listener is non-blocking, so the accept never blocks; a poll by the cleanup bound waits for a connection, and the bound
    // only re-arms it (a probe connects whenever a session starts). The drop's `closing` is checked at each round.
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    loop {
        if hub.lock().closing {
            break;
        }
        let stream = match listener
            .accept()
            .map_err(|error| after_accept(error.kind()))
        {
            Ok((stream, _)) => stream,
            Err(AcceptError::Wait) => {
                let deadline = Deadline::cleanup();
                let mut ready = [PollFd::new(listener, PollFlags::IN)];
                let Ok(left) = Timespec::try_from(deadline.remaining()) else {
                    break;
                };
                // timer: deadline — bounds each wait for a probe's connection; the loop re-arms it.
                match poll(&mut ready, Some(&left)) {
                    Ok(_) | Err(Errno::INTR) => continue,
                    Err(_) => break,
                }
            }
            Err(AcceptError::Next) => continue,
            Err(AcceptError::Stop) => break,
        };
        // The socket is non-blocking (macOS: inherited from the listener; Linux: set here), so a request write never blocks
        // (`send_line`). macOS ignores `MSG_DONTWAIT` on a full Unix stream socket (bug catalog B10), so the flag on the
        // descriptor is the only reliable form. The report reader polls before each read (`Bounded`).
        if stream.set_nonblocking(true).is_err() {
            continue;
        }
        let Ok(copy) = stream.try_clone() else {
            continue;
        };
        let mut state = hub.lock();
        if state.closing {
            break;
        }
        state.connections.push(copy);
        drop(state);
        let hub = Arc::clone(hub);
        if let Ok(reader) = std::thread::Builder::new()
            .name("probe-control-read".into())
            .spawn(move || serve(&hub, stream))
        {
            readers.push(reader);
        }
    }
    for reader in readers {
        let _ = reader.join();
    }
}

/// The reader of one connection: the hello, then the reports, until the socket ends.
fn serve(hub: &Hub, stream: UnixStream) {
    let Ok(requests) = stream.try_clone() else {
        return;
    };
    let mut lines = Lines {
        reader: Bounded::new(stream),
        partial: vec![],
    };
    let parent = match lines.next().map(|line| serde_json::from_str(&line)) {
        Some(Ok(Report::Hello { parent, .. })) => parent,
        _ => return,
    };
    // The worker is the probe's parent, alive while its payload runs: its identity now is the one in Core's row.
    let Some(worker) = identity(parent) else {
        return;
    };
    hub.lock().probes.insert(
        worker,
        Probe {
            requests: Arc::new(requests),
            input: vec![],
            sent: 0,
            acked: 0,
            closed: false,
        },
    );
    hub.changed.notify_all();
    loop {
        let report = lines
            .next()
            .and_then(|line| serde_json::from_str::<Report>(&line).ok());
        let mut state = hub.lock();
        let probe = state.probes.get_mut(&worker).expect("registered");
        let woke = match report {
            Some(Report::Input(hex)) => {
                probe
                    .input
                    .extend_from_slice(&hex_decode(&hex).unwrap_or_default());
                true
            }
            Some(Report::Ack {}) => {
                probe.acked += 1;
                false
            }
            Some(Report::InputEnd {} | Report::Hello { .. }) => false,
            None => {
                probe.closed = true;
                drop(state);
                hub.changed.notify_all();
                return;
            }
        };
        drop(state);
        hub.changed.notify_all();
        if woke {
            hub.wake_hosts();
        }
    }
}

/// The lines of one connection.
struct Lines {
    reader: Bounded<UnixStream>,
    /// The start of a line that a bound cut: the next line begins with it.
    partial: Vec<u8>,
}

impl Lines {
    /// The next line, or `None` at the end of the socket or at a read error. A probe may stay silent for the whole session, so
    /// the cleanup bound of each read only re-arms it. The drop shuts the socket down, which ends the read.
    fn next(&mut self) -> Option<String> {
        loop {
            match self.reader.line(Deadline::cleanup()) {
                Ok(line) => {
                    let line = line?;
                    let mut whole = std::mem::take(&mut self.partial);
                    whole.extend_from_slice(line.as_bytes());
                    return Some(String::from_utf8_lossy(&whole).into_owned());
                }
                Err(ReadError::Deadline { partial, .. }) => {
                    self.partial.extend_from_slice(&partial);
                }
                Err(_) => return None,
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PtyOutput {
    session: SessionId,
    bytes_hex: String,
}

fn on() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PtyBlocked {
    session: SessionId,
    #[serde(default = "on")]
    on: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    session: SessionId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bytes {
    session: SessionId,
    bytes: usize,
}

impl RealCoreHarness {
    /// The worker of `session` on `handle`, from Core's registry row.
    fn worker_of(
        &self,
        handle: &str,
        session: &SessionId,
    ) -> Result<ProcessIdentity, ControlError> {
        let dir = self
            .data_dir_of(handle)
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        self.session_processes(&dir, session)
            .and_then(|p| p.worker)
            .ok_or_else(|| {
                ControlError::Bad(format!(
                    "the session {} has no worker process yet",
                    session.0
                ))
            })
    }

    /// Serves one of [`CONTROLS`].
    pub(super) fn program_control(
        &mut self,
        handle: &str,
        op: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        match op {
            "pty_output" => {
                let args: PtyOutput = parse(args)?;
                let bytes = hex_decode(&args.bytes_hex)
                    .map_err(|e| ControlError::Bad(format!("bytes_hex: {e}")))?;
                if bytes.is_empty() {
                    return Err(ControlError::Bad("bytes_hex: no bytes".into()));
                }
                let worker = self.worker_of(handle, &args.session)?;
                self.programs
                    .request(worker, &Request::Output(HexBytes(bytes).to_hex()))?;
                Ok(Value::Null)
            }
            "pty_input" => {
                let args: Session = parse(args)?;
                let worker = self.worker_of(handle, &args.session)?;
                let input = self.programs.input(worker)?;
                Ok(json!({ "bytes": { "$bytes_hex": HexBytes(input).to_hex() } }))
            }
            "pty_blocked" => {
                let args: PtyBlocked = parse(args)?;
                let worker = self.worker_of(handle, &args.session)?;
                self.programs.request(worker, &Request::Blocked(args.on))?;
                Ok(Value::Null)
            }
            "pty_accept" => {
                let args: Bytes = parse(args)?;
                let worker = self.worker_of(handle, &args.session)?;
                self.programs
                    .request(worker, &Request::Accept(args.bytes))?;
                Ok(Value::Null)
            }
            "pty_chunk" => {
                let args: Bytes = parse(args)?;
                if args.bytes == 0 {
                    return Err(ControlError::Bad(
                        "bytes: a cap of 0 lets no input through".into(),
                    ));
                }
                let worker = self.worker_of(handle, &args.session)?;
                self.programs.request(worker, &Request::Chunk(args.bytes))?;
                Ok(Value::Null)
            }
            _ => Err(ControlError::Unsupported),
        }
    }
}

#[cfg(test)]
mod slow_tests {
    use super::*;
    use botster_core_conformance::CoreHarness;
    use botster_core_contract::prelude::{Wake, WakeHandle};
    use botster_core_edges::edges::Wake as WakeEdge;
    use botster_test_process::{OwnedChild, CLEANUP};
    use std::os::fd::RawFd;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// A stand-in for the real probe at the socket boundary: it speaks the probe's protocol (`botster_probe_script::control`).
    /// The real probe's own tests are in botster-contracts; the real-tier conformance ids run the real probe.
    struct FakeProbe {
        stream: UnixStream,
        lines: Bounded<UnixStream>,
    }

    impl FakeProbe {
        fn connect(programs: &Programs, parent: u32) -> FakeProbe {
            let stream = UnixStream::connect(&programs.path).unwrap();
            let mut probe = FakeProbe {
                lines: Bounded::new(stream.try_clone().unwrap()),
                stream,
            };
            probe.report(&Report::Hello { pid: 1, parent });
            probe
        }

        fn report(&mut self, report: &Report) {
            let mut line = serde_json::to_vec(report).unwrap();
            line.push(b'\n');
            self.stream.write_all(&line).unwrap();
        }

        /// Waits until bytes of a request are readable, and reads none: the harness has started to write it.
        fn request_started(&self) {
            let mut ready = [PollFd::new(&self.stream, PollFlags::IN)];
            let left = Timespec::try_from(CLEANUP).unwrap();
            // timer: deadline — a request that never starts fails the test instead of stalling it
            assert_eq!(
                poll(&mut ready, Some(&left)).unwrap(),
                1,
                "no request started"
            );
        }

        fn next_request(&mut self) -> Request {
            let line = self
                .lines
                .line(Deadline::cleanup())
                .unwrap()
                .expect("the harness sent a request");
            serde_json::from_str(&line).unwrap()
        }
    }

    /// A wake handle that counts its signals.
    #[derive(Default)]
    struct CountingWake {
        signals: AtomicUsize,
    }

    impl WakeHandle for CountingWake {
        fn wait(&self, _timeout: Duration) -> Wake {
            Wake::TimedOut
        }
        fn fd(&self) -> RawFd {
            -1
        }
    }

    impl WakeEdge for CountingWake {
        fn signal(&self) {
            self.signals.fetch_add(1, Ordering::SeqCst);
        }
        fn drain(&self) {}
    }

    /// The identity of a live process, as Core records a worker.
    fn identity(pid: u32) -> ProcessIdentity {
        let start_time = platform::start_time(platform::pid(pid).unwrap())
            .unwrap()
            .unwrap();
        ProcessIdentity { pid, start_time }
    }

    fn programs() -> (tempfile::TempDir, Programs) {
        let root = tempfile::tempdir().unwrap();
        let programs = Programs::listen(root.path()).unwrap();
        (root, programs)
    }

    /// Both real harnesses name the socket under the one variable that the probe reads.
    #[test]
    fn the_probe_env_names_the_socket() {
        let (root, programs) = programs();
        assert_eq!(
            programs.env(),
            BTreeMap::from([(
                CONTROL_ENV.to_string(),
                root.path().join("p").display().to_string()
            )])
        );
    }

    /// A control returns when the probe acknowledges its request: the request is in effect (P3 rule 4).
    #[test]
    fn a_request_returns_on_the_acknowledgement() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        let mut probe = FakeProbe::connect(&programs, me.pid);
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Output("61".into())));
            assert_eq!(probe.next_request(), Request::Output("61".into()));
            probe.report(&Report::Ack {});
            // The acknowledgement comes before the end of the socket, so the request is acknowledged, not failed.
            drop(probe);
            assert!(control.join().unwrap().is_ok());
        });
    }

    /// A probe that ends without acknowledging gives `Bad`, never a pass: the payload has exited.
    #[test]
    fn a_probe_that_ends_without_acknowledging_gives_bad() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        let mut probe = FakeProbe::connect(&programs, me.pid);
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Blocked(true)));
            assert_eq!(probe.next_request(), Request::Blocked(true));
            drop(probe);
            assert!(matches!(control.join().unwrap(), Err(ControlError::Bad(_))));
        });
        // Later requests to the ended probe fail at once.
        assert!(matches!(
            programs.request(me, &Request::Chunk(1)),
            Err(ControlError::Bad(_))
        ));
    }

    /// A control for a probe that has not connected yet waits for its connection.
    #[test]
    fn a_request_waits_for_the_probes_connection() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Accept(2)));
            let mut probe = FakeProbe::connect(&programs, me.pid);
            assert_eq!(probe.next_request(), Request::Accept(2));
            probe.report(&Report::Ack {});
            assert!(control.join().unwrap().is_ok());
        });
    }

    /// The probe's reads build the session's `pty_input` log in order, and each one wakes every open handle's host, because a
    /// read of the probe gives Core no wake of its own (TM-6, `await_pty`).
    #[test]
    fn input_reports_build_the_log_in_order_and_wake_the_hosts() {
        let (_root, programs) = programs();
        let wake = Arc::new(CountingWake::default());
        programs.watch(&(Arc::clone(&wake) as Arc<dyn HostWake>));
        let me = identity(std::process::id());
        let mut probe = FakeProbe::connect(&programs, me.pid);
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Chunk(1)));
            assert_eq!(probe.next_request(), Request::Chunk(1));
            // The reports are read in order, so the acknowledgement fences the two reads before it.
            probe.report(&Report::Input("6162".into()));
            probe.report(&Report::Input("63".into()));
            probe.report(&Report::Ack {});
            control.join().unwrap().unwrap();
        });
        assert_eq!(programs.input(me).unwrap(), b"abc");
        assert_eq!(wake.signals.load(Ordering::SeqCst), 2);
    }

    /// A probe may stay silent for longer than the cleanup bound, even inside a line: the reader re-arms its bound, keeps the
    /// start of the line, and still serves the probe.
    #[test]
    fn a_probe_silent_past_the_cleanup_bound_is_still_served() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        let mut probe = FakeProbe::connect(&programs, me.pid);
        let mut line = serde_json::to_vec(&Report::Input("61".into())).unwrap();
        line.push(b'\n');
        let (start, end) = line.split_at(line.len() / 2);
        probe.stream.write_all(start).unwrap();
        // The silence outlasts the reader's cleanup bound; the harness sends no request meanwhile.
        assert!(matches!(
            probe
                .lines
                .line(Deadline::after(CLEANUP + Duration::from_secs(1))),
            Err(ReadError::Deadline { .. })
        ));
        probe.stream.write_all(end).unwrap();
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Chunk(1)));
            assert_eq!(probe.next_request(), Request::Chunk(1));
            probe.report(&Report::Ack {});
            control.join().unwrap().unwrap();
        });
        assert_eq!(programs.input(me).unwrap(), b"a");
    }

    /// The accept loop waits when no connection is queued, skips a peer that reset before the accept, and stops at any
    /// other error.
    #[test]
    fn an_accept_error_waits_skips_or_stops() {
        assert_eq!(after_accept(io::ErrorKind::WouldBlock), AcceptError::Wait);
        assert_eq!(
            after_accept(io::ErrorKind::ConnectionAborted),
            AcceptError::Next
        );
        assert_eq!(after_accept(io::ErrorKind::InvalidInput), AcceptError::Stop);
    }

    /// `pty_blocked` with no `on` blocks the probe's reads.
    #[test]
    fn pty_blocked_is_on_by_default() {
        let args: PtyBlocked = parse(&json!({"session": "s1"})).unwrap();
        assert!(args.on);
        let args: PtyBlocked = parse(&json!({"session": "s1", "on": false})).unwrap();
        assert!(!args.on);
    }

    /// A recorded worker that no longer runs gives `Bad` at once: its probe can never connect, and a reused pid is never
    /// awaited (R-46).
    #[test]
    fn a_worker_that_is_not_the_recorded_process_gives_bad_at_once() {
        let (_root, programs) = programs();
        let mut me = identity(std::process::id());
        me.start_time += 1;
        assert!(matches!(programs.input(me), Err(ControlError::Bad(_))));
    }

    /// A worker that exited before its probe connected gives `Bad` at once, a setup failure (a probe that cannot connect
    /// exits, and its worker ends with it).
    #[test]
    fn a_worker_that_exited_before_its_probe_connected_gives_bad_at_once() {
        let (_root, programs) = programs();
        let mut worker = OwnedChild::spawn(Command::new("/bin/cat").stdin(Stdio::piped())).unwrap();
        let recorded = identity(worker.id());
        // The end of its stdin ends `cat`.
        drop(worker.take_stdin());
        assert!(worker.status().success());
        let result = programs.request(recorded, &Request::Blocked(true));
        assert!(
            matches!(&result, Err(ControlError::Bad(why)) if why.contains("exited before its probe connected")),
            "{result:?}"
        );
    }

    /// A probe is found by its worker's whole identity: a pid whose start time differs is another process (a reused pid), so
    /// it never finds the probe that connected under the first one (R-46).
    #[test]
    fn a_connected_probe_is_never_found_by_another_start_time() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        let _probe = FakeProbe::connect(&programs, me.pid);
        programs.connected(me).map(drop).unwrap();
        let reused = ProcessIdentity {
            start_time: me.start_time + 1,
            ..me
        };
        let result = programs.input(reused);
        assert!(
            matches!(&result, Err(ControlError::Bad(why)) if why.contains("exited before its probe connected")),
            "{result:?}"
        );
    }

    /// A wake handle that reports each signal on a channel.
    struct SignalWake(Mutex<std::sync::mpsc::Sender<()>>);

    impl WakeHandle for SignalWake {
        fn wait(&self, _timeout: Duration) -> Wake {
            Wake::TimedOut
        }
        fn fd(&self) -> RawFd {
            -1
        }
    }

    impl WakeEdge for SignalWake {
        fn signal(&self) {
            let _ = self.0.lock().unwrap().send(());
        }
        fn drain(&self) {}
    }

    /// A control writes its request without the state lock: while the probe reads no request (its socket is full), the reader
    /// still takes the probe's reports, and `pty_input` still answers.
    #[test]
    fn reports_are_taken_while_a_request_waits_for_room() {
        let (_root, programs) = programs();
        let (signalled, signals) = std::sync::mpsc::channel();
        let wake = Arc::new(SignalWake(Mutex::new(signalled)));
        // `watch` keeps a weak reference: the test holds the handle, as a Core handle does.
        programs.watch(&(Arc::clone(&wake) as Arc<dyn HostWake>));
        let me = identity(std::process::id());
        let mut probe = FakeProbe::connect(&programs, me.pid);
        programs.connected(me).map(drop).unwrap();
        // More bytes than a socket holds on either OS, so the write waits until the probe reads.
        let big = "61".repeat(1 << 19);
        std::thread::scope(|s| {
            let control = s.spawn(|| programs.request(me, &Request::Output(big.clone())));
            // The write has started, and the probe reads none of it: the rest waits for room until the report is taken.
            probe.request_started();
            probe.report(&Report::Input("78".into()));
            // The reader took the report, and woke the hosts, although the request is not written yet.
            signals
                // timer: deadline — a reader that waits for the request's write fails the test instead of stalling it
                .recv_timeout(CLEANUP)
                .expect("the reader took the report");
            assert_eq!(programs.input(me).unwrap(), b"x");
            assert_eq!(probe.next_request(), Request::Output(big.clone()));
            probe.report(&Report::Ack {});
            control.join().unwrap().unwrap();
        });
    }

    /// A probe that never reads its requests gives `Bad` at the cleanup bound: the write never blocks the control.
    #[test]
    fn a_request_that_the_probe_never_reads_gives_bad_at_the_bound() {
        let (_root, programs) = programs();
        let me = identity(std::process::id());
        let _probe = FakeProbe::connect(&programs, me.pid);
        programs.connected(me).map(drop).unwrap();
        let big = "61".repeat(1 << 19);
        let result = programs.request(me, &Request::Output(big));
        assert!(
            matches!(&result, Err(ControlError::Bad(why)) if why.contains("was not written within")),
            "{result:?}"
        );
    }

    /// The drop ends the accept thread and every reader, and removes the socket.
    #[test]
    fn the_drop_ends_the_threads_and_removes_the_socket() {
        let (root, programs) = programs();
        let _probe = FakeProbe::connect(&programs, 1);
        drop(programs);
        assert!(!root.path().join("p").exists());
    }

    /// The controls' own argument checks, before any probe: an empty `pty_output` and a zero `pty_chunk` are `Bad`.
    #[test]
    fn an_empty_output_and_a_zero_chunk_are_bad() {
        let mut harness = RealCoreHarness::new(crate::candidate::Candidate {
            worker: PathBuf::from("/nonexistent/worker"),
            probe: PathBuf::from("/nonexistent/probe"),
            anchor: PathBuf::from("/nonexistent/anchor"),
        })
        .unwrap();
        let result = harness.control(
            "h",
            "pty_output",
            &json!({"session": "s1", "bytes_hex": ""}),
        );
        assert!(
            matches!(&result, Err(ControlError::Bad(why)) if why.contains("no bytes")),
            "{result:?}"
        );
        let result = harness.control("h", "pty_chunk", &json!({"session": "s1", "bytes": 0}));
        assert!(
            matches!(&result, Err(ControlError::Bad(why)) if why.contains("cap of 0")),
            "{result:?}"
        );
        // With valid arguments, every control is served, and the handle is the next check.
        for (op, args) in [
            ("pty_output", json!({"session": "s1", "bytes_hex": "61"})),
            ("pty_input", json!({"session": "s1"})),
            ("pty_blocked", json!({"session": "s1"})),
            ("pty_accept", json!({"session": "s1", "bytes": 1})),
            ("pty_chunk", json!({"session": "s1", "bytes": 1})),
        ] {
            let result = harness.control("h", op, &args);
            assert!(
                matches!(&result, Err(ControlError::Bad(why)) if why.contains("not open")),
                "{op}: {result:?}"
            );
        }
    }
}

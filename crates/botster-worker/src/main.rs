//! `botster-worker`: the thin binary of the session worker (plan 2.2, role `session`).
//!
//! It wires the real edges to the `Worker` machine of `botster-worker-core` and holds no worker logic (plan 2.1: one machine,
//! two drivers). One `mio` loop serves the control socket, the PTY master, the worker-control signal (`SIGUSR1`, LC-5) and the
//! wake of the payload's exit watch. `SIGTERM` ends the worker after its payload (`Input::Terminate`).
//!
//! **Bounded turns (plan 2.4, 2.5).** `mio` reports readiness by edge, so the driver keeps a readiness flag per descriptor
//! and clears it only when a read or write finds `WouldBlock`. Each turn reads at most one chunk from each descriptor, control
//! first, and the machine handles every input before the next turn: a payload that writes without end cannot hold back a
//! host request, the worker-control signal or a due grace, and the driver holds at most one chunk per descriptor.
//! **Nothing blocks:** writes to the control socket are nonblocking, and a staged close waits in the machine
//! (`Input::LinkWritten`), so a host that stops reading never stops the worker from serving its payload.
//!
//! The host starts it with the arguments and the environment of `botster_core_link::launch::WorkerLaunch` (AD-6: the token is
//! in the environment only).
//!
//! **The worker endpoint (DESIGN.md parts 1 and 7).** The driver binds `--endpoint` before the first hello and removes it at
//! its end. Each connection on it is a candidate: the driver names it, reads at most one chunk of it per turn, and never
//! writes to it. `AdoptLink` makes a candidate the control link (the fence drops the old link and its queued inputs).

mod command_line;
mod io_decisions;

use botster_core_edges::Machine;
use botster_core_link::launch::{WorkerLaunch, TOKEN_VAR};
use botster_core_link::msg::PayloadId;
use botster_core_sys::payload::{self, Payload, PayloadCommand};
use botster_core_sys::process::start_time;
use botster_worker_core::{Action, CandidateId, Input, PayloadSpec, SpawnFailure, Worker};
use io_decisions::{IoFailure, ReadyState};
use mio::net::{UnixListener, UnixStream};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token, Waker};
use signal_hook::consts::{SIGTERM, SIGUSR1};
use signal_hook_mio::v1_0::Signals;
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::num::NonZeroUsize;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

const CONTROL: Token = Token(0);
const PTY: Token = Token(1);
const SIGNALS: Token = Token(2);
const EXIT: Token = Token(3);
const ENDPOINT: Token = Token(4);

/// The bytes of one read of the control socket or the PTY.
const READ_CHUNK: NonZeroUsize = NonZeroUsize::new(64 * 1024).expect("positive driver read bound");

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let token = std::env::var(TOKEN_VAR).ok();
    let (code, error) = command_line::execute(&args, token.as_deref(), |launch| {
        Driver::start(launch).and_then(Driver::run)
    });
    if let Some(error) = error {
        eprintln!("botster-worker: {error}");
    }
    code
}

/// The bound worker endpoint. Its drop removes the endpoint, so every end of the driver removes it.
struct Endpoint {
    listener: UnixListener,
    path: PathBuf,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        if let Err(error) = io_decisions::unlinked(std::fs::remove_file(&self.path)) {
            eprintln!(
                "botster-worker: the endpoint {} is not removed: {error}",
                self.path.display()
            );
        }
    }
}

/// A connection on the worker endpoint that is not the control link.
struct CandidateIo {
    stream: UnixStream,
    readable: bool,
}

/// The real edges of one worker and the machine they drive.
struct Driver {
    read_chunk: NonZeroUsize,
    worker: Worker,
    poll: Poll,
    control: UnixStream,
    link_open: bool,
    /// Bytes of `LinkSend` that the socket has not taken yet.
    outbound: VecDeque<u8>,
    /// The bytes of `LinkSend` written so far (`Input::LinkWritten`).
    written: u64,
    writable_interest: bool,
    /// Readiness flags, cleared on `WouldBlock` (edge-triggered readiness).
    control_readable: bool,
    control_writable: bool,
    pty_readable: bool,
    endpoint: Endpoint,
    endpoint_readable: bool,
    candidates: BTreeMap<CandidateId, CandidateIo>,
    next_candidate: u64,
    signals: Signals,
    payload: Option<Payload>,
    pty_registered: bool,
    /// The PTY refused bytes. Write interest stays enabled until the writable event.
    pty_wants_write: bool,
    /// The driver writes this action once per turn, after it serves the control link.
    pty_write: Option<Vec<u8>>,
    /// The bytes still to read for a `DrainPty`; `None` when no drain is asked.
    drain_left: Option<usize>,
    waker: Arc<Waker>,
    exits: (
        mpsc::Sender<botster_core_edges::edges::ExitStatus>,
        mpsc::Receiver<botster_core_edges::edges::ExitStatus>,
    ),
    /// Inputs of the current turn; the machine handles them before the next one.
    inputs: VecDeque<Input>,
    exit: bool,
}

impl Driver {
    fn start(launch: &WorkerLaunch) -> io::Result<Driver> {
        Self::start_with_read_bound(launch, READ_CHUNK)
    }

    fn start_with_read_bound(
        launch: &WorkerLaunch,
        read_chunk: NonZeroUsize,
    ) -> io::Result<Driver> {
        let poll = Poll::new()?;
        // The endpoint is bound before the first hello: a host that has the hello can adopt the worker. `Endpoint` owns it
        // from the bind on, so every later failure of the start removes it (DESIGN.md part 7).
        let mut endpoint = Endpoint {
            listener: UnixListener::bind(&launch.endpoint)?,
            path: launch.endpoint.clone(),
        };
        poll.registry()
            .register(&mut endpoint.listener, ENDPOINT, Interest::READABLE)?;
        let std_control = std::os::unix::net::UnixStream::connect(&launch.control)?;
        std_control.set_nonblocking(true)?;
        let mut control = UnixStream::from_std(std_control);
        poll.registry()
            .register(&mut control, CONTROL, Interest::READABLE)?;
        let mut signals = Signals::new([SIGUSR1, SIGTERM])?;
        poll.registry()
            .register(&mut signals, SIGNALS, Interest::READABLE)?;
        let waker = Arc::new(Waker::new(poll.registry(), EXIT)?);
        let worker = Worker::new(io_decisions::worker_config(launch));
        Ok(Driver {
            read_chunk,
            worker,
            poll,
            control,
            link_open: true,
            outbound: VecDeque::new(),
            written: 0,
            writable_interest: false,
            control_readable: true,
            control_writable: true,
            pty_readable: false,
            endpoint,
            endpoint_readable: true,
            candidates: BTreeMap::new(),
            next_candidate: 0,
            signals,
            payload: None,
            pty_registered: false,
            pty_wants_write: false,
            pty_write: None,
            drain_left: None,
            waker,
            exits: mpsc::channel(),
            inputs: VecDeque::new(),
            exit: false,
        })
    }

    fn run(mut self) -> io::Result<()> {
        let mut events = Events::with_capacity(16);
        loop {
            self.settle()?;
            if self.exit {
                return Ok(());
            }
            // One bounded turn. Plan 2.4: the control link first.
            self.flush()?;
            self.read_control();
            self.accept_candidate();
            self.read_candidates();
            self.settle()?;
            if self.exit {
                return Ok(());
            }
            if io_decisions::due(self.worker.next_deadline(), Instant::now()) {
                self.inputs.push_back(Input::Timer);
            }
            self.write_pty_once()?;
            self.read_pty_chunk();
            self.settle()?;
            if self.exit {
                return Ok(());
            }
            let timeout = ReadyState {
                link_open: self.link_open,
                control_readable: self.control_readable,
                pty_registered: self.pty_registered,
                pty_readable: self.pty_readable,
                draining: self.drain_left,
                queued_inputs: self.inputs.len(),
                pending_write: self.pty_write.is_some(),
                write_blocked: self.pty_wants_write,
                endpoint_readable: self.endpoint_readable,
                candidate_readable: self.candidates.values().any(|c| c.readable),
            }
            .timeout(self.worker.next_deadline(), Instant::now());
            if io_decisions::poll_interrupted(self.poll.poll(&mut events, timeout))? {
                continue;
            }
            for event in &events {
                match event.token() {
                    CONTROL => {
                        self.control_readable = io_decisions::control_ready(
                            self.control_readable,
                            event.is_readable(),
                            event.is_read_closed(),
                            event.is_error(),
                        );
                        self.control_writable = io_decisions::writable_ready(
                            self.control_writable,
                            event.is_writable(),
                            event.is_error(),
                        );
                    }
                    PTY => {
                        self.pty_readable = io_decisions::control_ready(
                            self.pty_readable,
                            event.is_readable(),
                            event.is_read_closed(),
                            event.is_error(),
                        );
                        if io_decisions::pty_writable(self.pty_wants_write, event.is_writable()) {
                            self.set_pty_write_interest(false)?;
                            self.inputs.push_back(Input::PtyWritable);
                        }
                    }
                    SIGNALS => {
                        for signal in self.signals.pending() {
                            self.inputs.push_back(if signal == SIGTERM {
                                Input::Terminate
                            } else {
                                Input::EndPayload
                            });
                        }
                    }
                    EXIT => {
                        while let Ok(status) = self.exits.1.try_recv() {
                            self.inputs.push_back(Input::PayloadExited(status));
                        }
                    }
                    ENDPOINT => {
                        self.endpoint_readable = io_decisions::control_ready(
                            self.endpoint_readable,
                            event.is_readable(),
                            event.is_read_closed(),
                            event.is_error(),
                        );
                    }
                    Token(token) => {
                        let candidate = io_decisions::candidate_of(token)
                            .and_then(|id| self.candidates.get_mut(&id));
                        if let Some(candidate) = candidate {
                            candidate.readable = io_decisions::control_ready(
                                candidate.readable,
                                event.is_readable(),
                                event.is_read_closed(),
                                event.is_error(),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Hands every queued input to the machine and performs every action, until neither is left.
    fn settle(&mut self) -> io::Result<()> {
        loop {
            while let Some(action) = self.worker.poll_action() {
                self.perform(action)?;
            }
            let Some(input) = self.inputs.pop_front() else {
                return Ok(());
            };
            self.worker.handle(Instant::now(), input);
        }
    }

    fn perform(&mut self, action: Action) -> io::Result<()> {
        match action {
            Action::LinkSend(bytes) => {
                if self.link_open {
                    self.outbound.extend(bytes);
                    self.flush()?;
                }
            }
            // The machine closes only when everything it sent is written (`LinkWritten`).
            Action::LinkClose => self.drop_link(),
            Action::SpawnPayload(spec) => {
                let result = self.spawn(&spec);
                self.inputs.push_back(Input::Spawned(result));
            }
            Action::PtyWrite(bytes) => self.pty_write = Some(bytes),
            Action::DrainPty => {
                let left = match self.payload.as_ref() {
                    Some(payload) => payload.pending_output()?,
                    None => 0,
                };
                self.drain_left = Some(left);
            }
            Action::SignalPayload(signal) => {
                if let Some(payload) = self.payload.as_ref() {
                    payload.signal_group(signal);
                }
            }
            Action::ReapPayload => {
                if let Some(payload) = self.payload.take() {
                    self.deregister_pty(&payload);
                    payload.reap();
                }
                self.drain_left = None;
            }
            Action::Exit => self.exit = true,
            Action::CandidateClose(id) => {
                if let Some(mut candidate) = self.candidates.remove(&id) {
                    let _ = self.poll.registry().deregister(&mut candidate.stream);
                    let _ = candidate.stream.shutdown(std::net::Shutdown::Both);
                }
            }
            Action::AdoptLink(id) => self.adopt_link(id)?,
            // The route descriptors come with `SCM_RIGHTS` on the control socket, which is real-only work after the testkit
            // PRs of P4a (worker-core DESIGN.md "Real-only"). Until then this driver gives no `Input::Descriptor`, so the
            // machine binds no route and names none; with no route, the PTY read budget only lifts a limit it never set.
            Action::BindRoute { .. }
            | Action::CloseDescriptor(_)
            | Action::RouteWrite { .. }
            | Action::RouteClose { .. }
            | Action::PtyReadBudget(_) => {}
        }
        Ok(())
    }

    fn spawn(&mut self, spec: &PayloadSpec) -> Result<PayloadId, SpawnFailure> {
        let clamp = |n: u32| u16::try_from(n).unwrap_or(u16::MAX);
        let payload = Payload::spawn(&PayloadCommand {
            argv: &spec.argv,
            env: &spec.env,
            cwd: &spec.cwd,
            rows: clamp(spec.size.rows),
            cols: clamp(spec.size.cols),
        })
        .map_err(|failure| match failure {
            payload::SpawnFailure::CwdMissing => SpawnFailure::CwdMissing,
            payload::SpawnFailure::Exec { errno } => SpawnFailure::Exec { errno },
        })?;
        let pid = payload.pid();
        let fd = payload.master().as_raw_fd();
        let registered = self
            .poll
            .registry()
            .register(&mut SourceFd(&fd), PTY, Interest::READABLE);
        let tx = self.exits.0.clone();
        let waker = Arc::clone(&self.waker);
        let watched = payload.watch_exit(move |status| {
            let _ = tx.send(status);
            let _ = waker.wake();
        });
        if let Err(error) = registered.and(watched) {
            // The payload cannot be served: it is ended (its drop kills the group and reaps it) and the launch fails.
            let _ = self.poll.registry().deregister(&mut SourceFd(&fd));
            drop(payload);
            return Err(SpawnFailure::Exec {
                errno: error.raw_os_error().unwrap_or(io_decisions::EIO),
            });
        }
        self.pty_registered = true;
        self.pty_readable = true;
        self.payload = Some(payload);
        Ok(PayloadId {
            pid,
            // A payload that ended at once may have no readable start time; the identity is still unique while unreaped.
            start_time: start_time(pid).unwrap_or(0),
        })
    }

    /// One PTY write of the pending `PtyWrite` (plan 2.4: one bounded piece per turn). A write that the PTY did not take
    /// waits for its write readiness; an interrupted write is tried again in the next turn.
    fn write_pty_once(&mut self) -> io::Result<()> {
        if self.pty_wants_write {
            return Ok(());
        }
        let Some(bytes) = self.pty_write.take() else {
            return Ok(());
        };
        let written = self.payload.as_ref().map(|payload| payload.write(&bytes));
        match io_decisions::pty_write(written, bytes.len()) {
            io_decisions::PtyWrite::Retry => self.pty_write = Some(bytes),
            io_decisions::PtyWrite::Report {
                result,
                wait_writable,
            } => {
                if wait_writable {
                    self.set_pty_write_interest(true)?;
                }
                self.inputs.push_back(Input::PtyWritten(result));
            }
        }
        Ok(())
    }

    /// Write interest on the PTY follows a write that it did not take (plan 2.5). A PTY that left the loop takes none.
    fn set_pty_write_interest(&mut self, on: bool) -> io::Result<()> {
        let change =
            io_decisions::pty_write_interest(self.pty_registered, self.pty_wants_write, on);
        if change.reregister {
            let Some(payload) = self.payload.as_ref() else {
                return Ok(());
            };
            let fd = payload.master().as_raw_fd();
            let interest = if on {
                Interest::READABLE | Interest::WRITABLE
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(&mut SourceFd(&fd), PTY, interest)?;
        }
        self.pty_wants_write = change.wants_write;
        Ok(())
    }

    fn deregister_pty(&mut self, payload: &Payload) {
        if self.pty_registered {
            let fd = payload.master().as_raw_fd();
            let _ = self.poll.registry().deregister(&mut SourceFd(&fd));
            self.pty_registered = false;
            self.pty_readable = false;
            self.pty_wants_write = false;
        }
    }

    /// At most one chunk of the PTY. During a drain the read is bounded by what the PTY held when the drain was asked, and
    /// `PtyDrained` follows once that is read (or the output ended).
    fn read_pty_chunk(&mut self) {
        let Some(payload) = self.payload.as_ref() else {
            if self.drain_left.take().is_some() {
                self.inputs.push_back(Input::PtyDrained);
            }
            return;
        };
        let draining = self.drain_left;
        if draining == Some(0) {
            self.drain_left = None;
            self.inputs.push_back(Input::PtyDrained);
            return;
        }
        if !io_decisions::read_pty(self.pty_registered, self.pty_readable, draining) {
            return;
        }
        let want = draining.map_or(self.read_chunk.get(), |left| {
            left.min(self.read_chunk.get())
        });
        let mut buf = vec![0u8; want];
        let mut ended = false;
        match payload.read(&mut buf) {
            Ok(0) => ended = true,
            Ok(n) => {
                self.inputs.push_back(Input::PtyOutput(buf[..n].to_vec()));
                if let Some(left) = self.drain_left.as_mut() {
                    *left = left.saturating_sub(n);
                }
            }
            Err(error) => match io_decisions::failure(&error) {
                IoFailure::Retry => {}
                IoFailure::Blocked => {
                    self.pty_readable = false;
                    if self.drain_left.is_some() {
                        self.drain_left = Some(0);
                    }
                }
                IoFailure::Closed => ended = true,
            },
        }
        if ended {
            // The output ended: the descriptor would stay readable, so it leaves the loop.
            if let Some(payload) = self.payload.take() {
                self.deregister_pty(&payload);
                self.payload = Some(payload);
            }
            if self.drain_left.is_some() {
                self.drain_left = Some(0);
            }
        }
    }

    /// At most one chunk of the control socket.
    fn read_control(&mut self) {
        if !io_decisions::read_control(self.link_open, self.control_readable) {
            return;
        }
        let mut buf = vec![0u8; self.read_chunk.get()];
        match self.control.read(&mut buf) {
            Ok(0) => self.link_lost(),
            Ok(n) => self.inputs.push_back(Input::LinkBytes(buf[..n].to_vec())),
            Err(error) => match io_decisions::failure(&error) {
                IoFailure::Retry => {}
                IoFailure::Blocked => self.control_readable = false,
                IoFailure::Closed => self.link_lost(),
            },
        }
    }

    /// Writes what the socket takes now, without blocking, and tells the machine how many bytes are written in all
    /// (`LinkWritten`). Write interest follows the outbound queue (plan 2.5).
    fn flush(&mut self) -> io::Result<()> {
        if !io_decisions::flush(self.link_open, self.outbound.len()) {
            return Ok(());
        }
        let before = self.written;
        while io_decisions::keep_writing(self.control_writable, self.outbound.len()) {
            let (head, _) = self.outbound.as_slices();
            match self.control.write(head) {
                Ok(0) => self.control_writable = false,
                Ok(n) => {
                    self.outbound.drain(..n);
                    self.written += n as u64;
                }
                Err(error) => match io_decisions::failure(&error) {
                    IoFailure::Retry => {}
                    IoFailure::Blocked => self.control_writable = false,
                    IoFailure::Closed => {
                        self.link_lost();
                        return Ok(());
                    }
                },
            }
        }
        if self.written != before {
            self.inputs.push_back(Input::LinkWritten {
                total: self.written,
            });
        }
        let want = !self.outbound.is_empty();
        if want != self.writable_interest {
            let interest = if want {
                Interest::READABLE | Interest::WRITABLE
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(&mut self.control, CONTROL, interest)?;
            self.writable_interest = want;
        }
        Ok(())
    }

    /// At most one connection of the endpoint per turn. Each one is a candidate for the machine. A failed accept leaves
    /// the endpoint; the worker keeps its control link and its payload.
    fn accept_candidate(&mut self) {
        if io_decisions::endpoint_waits(self.endpoint_readable) {
            return;
        }
        match self.endpoint.listener.accept() {
            Ok((mut stream, _)) => {
                let id = io_decisions::take_candidate(&mut self.next_candidate);
                let token = Token(io_decisions::candidate_token(id));
                if self
                    .poll
                    .registry()
                    .register(&mut stream, token, Interest::READABLE)
                    .is_ok()
                {
                    self.candidates.insert(
                        id,
                        CandidateIo {
                            stream,
                            readable: true,
                        },
                    );
                    self.inputs.push_back(Input::Candidate(id));
                }
            }
            Err(error) => match io_decisions::failure(&error) {
                IoFailure::Retry => {}
                IoFailure::Blocked => self.endpoint_readable = false,
                IoFailure::Closed => {
                    let _ = self.poll.registry().deregister(&mut self.endpoint.listener);
                    self.endpoint_readable = false;
                }
            },
        }
    }

    /// At most one chunk of each candidate. A candidate whose connection ended leaves the driver: the machine hears
    /// `CandidateClosed` and closes nothing of it.
    fn read_candidates(&mut self) {
        let mut buf = vec![0u8; self.read_chunk.get()];
        let mut ended = Vec::new();
        for (&id, candidate) in &mut self.candidates {
            if io_decisions::candidate_waits(candidate.readable) {
                continue;
            }
            let closed = match candidate.stream.read(&mut buf) {
                Ok(0) => true,
                Ok(n) => {
                    self.inputs
                        .push_back(Input::CandidateBytes(id, buf[..n].to_vec()));
                    false
                }
                Err(error) => match io_decisions::failure(&error) {
                    IoFailure::Retry => false,
                    IoFailure::Blocked => {
                        candidate.readable = false;
                        false
                    }
                    IoFailure::Closed => true,
                },
            };
            if closed {
                ended.push(id);
            }
        }
        for id in ended {
            if let Some(mut candidate) = self.candidates.remove(&id) {
                let _ = self.poll.registry().deregister(&mut candidate.stream);
            }
            self.inputs.push_back(Input::CandidateClosed(id));
        }
    }

    /// The fence (DP-8): the old link and every byte still unwritten on it are dropped, and the candidate becomes the
    /// control link. `LinkWritten` counts from zero on it. A candidate that the driver no longer holds fails closed
    /// (`io_decisions::fence`): the old link is dropped all the same.
    fn adopt_link(&mut self, id: CandidateId) -> io::Result<()> {
        self.drop_link();
        let Some(mut candidate) = self.candidates.remove(&id) else {
            io_decisions::fence(&mut self.inputs, id, false);
            return Ok(());
        };
        self.poll.registry().deregister(&mut candidate.stream)?;
        self.poll
            .registry()
            .register(&mut candidate.stream, CONTROL, Interest::READABLE)?;
        self.control = candidate.stream;
        self.link_open = true;
        self.written = 0;
        self.writable_interest = false;
        self.control_readable = true;
        self.control_writable = true;
        io_decisions::fence(&mut self.inputs, id, true);
        Ok(())
    }

    /// The link failed or the host closed it: the machine hears `LinkClosed`.
    fn link_lost(&mut self) {
        if self.link_open {
            self.drop_link();
            self.inputs.push_back(Input::LinkClosed);
        }
    }

    fn drop_link(&mut self) {
        if self.link_open {
            let _ = self.poll.registry().deregister(&mut self.control);
            let _ = self.control.shutdown(std::net::Shutdown::Both);
            self.link_open = false;
            self.outbound.clear();
            self.control_readable = false;
        }
    }
}

// Both fixtures use the same session tests and the same production Driver.
#[cfg(all(test, feature = "slow"))]
const DRIVER_OBSERVER: Option<&str> = Some("driver_observer");

#[cfg(all(test, feature = "slow"))]
#[path = "../tests/common/session.rs"]
mod slow_driver;

#[cfg(all(test, feature = "slow"))]
#[test]
fn driver_observer() {
    let Some(control) = std::env::var_os("BOTSTER_DRIVER_CONTROL") else {
        return;
    };
    let launch = WorkerLaunch {
        control: control.clone().into(),
        instance: botster_core_contract::prelude::InstanceId("1-1".into()),
        host_epoch: 1,
        token: [5; 32],
        endpoint: PathBuf::from(&control).with_file_name("e"),
        startup_ms: WorkerLaunch::millis(
            botster_core_contract::prelude::CoreLimits::default().startup,
        ),
    };
    Driver::start(&launch).and_then(Driver::run).unwrap();
}

#[cfg(all(test, feature = "slow"))]
#[path = "../tests/common/driver_edges.rs"]
mod slow_edges;

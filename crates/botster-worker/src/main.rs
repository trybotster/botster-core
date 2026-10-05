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

mod io_decisions;

use botster_core_edges::Machine;
use botster_core_link::launch::{WorkerLaunch, TOKEN_VAR};
use botster_core_link::msg::PayloadId;
use botster_core_sys::payload::{self, Payload, PayloadCommand};
use botster_core_sys::process::start_time;
use botster_worker_core::{Action, Input, PayloadSpec, SpawnFailure, Worker, WorkerConfig};
use io_decisions::{IoFailure, ReadyState};
use mio::net::UnixStream;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token, Waker};
use signal_hook::consts::{SIGTERM, SIGUSR1};
use signal_hook_mio::v1_0::Signals;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::process::ExitCode;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

const CONTROL: Token = Token(0);
const PTY: Token = Token(1);
const SIGNALS: Token = Token(2);
const EXIT: Token = Token(3);

/// The bytes of one read of the control socket or the PTY.
const READ_CHUNK: usize = 64 * 1024;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let token = std::env::var(TOKEN_VAR).ok();
    let launch = match WorkerLaunch::parse(&args, token.as_deref()) {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("botster-worker: {error}");
            return ExitCode::from(2);
        }
    };
    match Driver::start(&launch).and_then(Driver::run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("botster-worker: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The real edges of one worker and the machine they drive.
struct Driver {
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
    signals: Signals,
    payload: Option<Payload>,
    pty_registered: bool,
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
        let poll = Poll::new()?;
        let std_control = std::os::unix::net::UnixStream::connect(&launch.control)?;
        std_control.set_nonblocking(true)?;
        let mut control = UnixStream::from_std(std_control);
        poll.registry()
            .register(&mut control, CONTROL, Interest::READABLE)?;
        let mut signals = Signals::new([SIGUSR1, SIGTERM])?;
        poll.registry()
            .register(&mut signals, SIGNALS, Interest::READABLE)?;
        let waker = Arc::new(Waker::new(poll.registry(), EXIT)?);
        let worker = Worker::new(WorkerConfig::new(
            launch.instance.clone(),
            launch.token,
            launch.host_epoch,
        ));
        Ok(Driver {
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
            signals,
            payload: None,
            pty_registered: false,
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
            self.settle()?;
            if self.exit {
                return Ok(());
            }
            if io_decisions::due(self.worker.next_deadline(), Instant::now()) {
                self.inputs.push_back(Input::Timer);
            }
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
                    PTY => self.pty_readable = true,
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
                    _ => {}
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
                errno: error.raw_os_error().unwrap_or(5),
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

    fn deregister_pty(&mut self, payload: &Payload) {
        if self.pty_registered {
            let fd = payload.master().as_raw_fd();
            let _ = self.poll.registry().deregister(&mut SourceFd(&fd));
            self.pty_registered = false;
            self.pty_readable = false;
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
        let want = draining.map_or(READ_CHUNK, |left| left.min(READ_CHUNK));
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
        let mut buf = vec![0u8; READ_CHUNK];
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
        control: control.into(),
        instance: botster_core_contract::prelude::InstanceId("1-1".into()),
        host_epoch: 1,
        token: [5; 32],
    };
    Driver::start(&launch).and_then(Driver::run).unwrap();
}

#[cfg(all(test, feature = "slow"))]
#[path = "../tests/common/driver_edges.rs"]
mod slow_edges;

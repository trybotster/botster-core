//! `botster-worker`: the thin binary of the session worker (plan 2.2, role `session`).
//!
//! It wires the real edges to the `Worker` machine of `botster-worker-core` and holds no worker logic (plan 2.1: one machine,
//! two drivers). One `mio` loop serves the control socket, the PTY master, the worker-control signal (`SIGUSR1`, LC-5) and the
//! wake of the payload's exit watch. In each turn the control link is served first (plan 2.4).
//!
//! The host starts it with the arguments and the environment of `botster_core_link::launch::WorkerLaunch` (AD-6: the token is
//! in the environment only).

use botster_core_edges::Machine;
use botster_core_link::launch::{WorkerLaunch, TOKEN_VAR};
use botster_core_link::msg::PayloadId;
use botster_core_sys::payload::{self, Payload, PayloadCommand};
use botster_core_sys::process::start_time;
use botster_worker_core::{Action, Input, PayloadSpec, SpawnFailure, Worker, WorkerConfig};
use mio::net::UnixStream;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token, Waker};
use signal_hook::consts::SIGUSR1;
use signal_hook_mio::v1_0::Signals;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
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
    writable_interest: bool,
    signals: Signals,
    payload: Option<Payload>,
    pty_registered: bool,
    waker: Arc<Waker>,
    exits: (
        mpsc::Sender<botster_core_edges::edges::ExitStatus>,
        mpsc::Receiver<botster_core_edges::edges::ExitStatus>,
    ),
    /// Inputs that performing an action produced; they are handled before the next poll.
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
        let mut signals = Signals::new([SIGUSR1])?;
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
            writable_interest: false,
            signals,
            payload: None,
            pty_registered: false,
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
                return self.finish();
            }
            let timeout = self
                .worker
                .next_deadline()
                .map(|at| at.saturating_duration_since(Instant::now()));
            match self.poll.poll(&mut events, timeout) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            let mut control = (false, false);
            let (mut pty, mut signals, mut exit) = (false, false, false);
            for event in &events {
                match event.token() {
                    CONTROL => {
                        control.0 |=
                            event.is_readable() || event.is_read_closed() || event.is_error();
                        control.1 |= event.is_writable();
                    }
                    PTY => pty = true,
                    SIGNALS => signals = true,
                    EXIT => exit = true,
                    _ => {}
                }
            }
            // Plan 2.4: the control link first.
            if control.1 {
                self.flush()?;
            }
            if control.0 {
                self.read_control();
            }
            if signals {
                let count = self.signals.pending().filter(|s| *s == SIGUSR1).count();
                for _ in 0..count {
                    self.inputs.push_back(Input::EndPayload);
                }
            }
            if exit {
                while let Ok(status) = self.exits.1.try_recv() {
                    self.inputs.push_back(Input::PayloadExited(status));
                }
            }
            if pty {
                self.read_pty(false);
            }
            if self
                .worker
                .next_deadline()
                .is_some_and(|at| at <= Instant::now())
            {
                self.inputs.push_back(Input::Timer);
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
            Action::LinkClose => self.close_link(),
            Action::SpawnPayload(spec) => {
                let result = self.spawn(&spec);
                self.inputs.push_back(Input::Spawned(result));
            }
            Action::DrainPty => self.read_pty(true),
            Action::SignalPayload(signal) => {
                if let Some(payload) = self.payload.as_ref() {
                    payload.signal_group(signal);
                }
            }
            Action::ReapPayload => {
                if let Some(payload) = self.payload.take() {
                    if self.pty_registered {
                        let fd = payload.master().as_raw_fd();
                        let _ = self.poll.registry().deregister(&mut SourceFd(&fd));
                        self.pty_registered = false;
                    }
                    payload.reap();
                }
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
            // The payload cannot be served: it is ended (its drop kills the group) and the launch fails.
            drop(payload);
            return Err(SpawnFailure::Exec {
                errno: error.raw_os_error().unwrap_or(5),
            });
        }
        self.pty_registered = true;
        self.payload = Some(payload);
        Ok(PayloadId {
            pid,
            // A payload that ended at once may have no readable start time; the identity is still unique while unreaped.
            start_time: start_time(pid).unwrap_or(0),
        })
    }

    /// Reads the PTY until it has no byte now. With `drained`, the machine asked for the drain (EV-4): it gets `PtyDrained`
    /// at the end.
    fn read_pty(&mut self, drained: bool) {
        let mut ended = false;
        if let Some(payload) = self.payload.as_ref() {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match payload.read(&mut buf) {
                    Ok(0) => {
                        ended = true;
                        break;
                    }
                    Ok(n) => self.inputs.push_back(Input::PtyOutput(buf[..n].to_vec())),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        ended = true;
                        break;
                    }
                }
            }
            if ended && self.pty_registered {
                // The output ended: the descriptor would stay readable, so it leaves the loop.
                let fd = payload.master().as_raw_fd();
                let _ = self.poll.registry().deregister(&mut SourceFd(&fd));
                self.pty_registered = false;
            }
        }
        if drained {
            self.inputs.push_back(Input::PtyDrained);
        }
    }

    fn read_control(&mut self) {
        let mut buf = vec![0u8; READ_CHUNK];
        while self.link_open {
            match self.control.read(&mut buf) {
                Ok(0) => {
                    self.inputs.push_back(Input::LinkClosed);
                    self.drop_link();
                }
                Ok(n) => self.inputs.push_back(Input::LinkBytes(buf[..n].to_vec())),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.inputs.push_back(Input::LinkClosed);
                    self.drop_link();
                }
            }
        }
    }

    /// Writes what the socket takes now; write interest follows the outbound queue (plan 2.5).
    fn flush(&mut self) -> io::Result<()> {
        while self.link_open && !self.outbound.is_empty() {
            let (head, _) = self.outbound.as_slices();
            match self.control.write(head) {
                Ok(0) => break,
                Ok(n) => {
                    self.outbound.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    // The host is gone: the bytes are lost with the link, and the read side reports the end.
                    self.outbound.clear();
                }
            }
        }
        let want = self.link_open && !self.outbound.is_empty();
        if want != self.writable_interest && self.link_open {
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

    /// The machine closes the link: what it sent before is written first.
    fn close_link(&mut self) {
        if self.link_open {
            self.write_all_blocking();
            self.drop_link();
        }
    }

    fn drop_link(&mut self) {
        if self.link_open {
            let _ = self.poll.registry().deregister(&mut self.control);
            let _ = self.control.shutdown(std::net::Shutdown::Both);
            self.link_open = false;
            self.outbound.clear();
        }
    }

    /// Writes every queued byte, waiting for the socket: the last reports (`RemoveResult`) reach the host before the end.
    /// The socket leaves the readiness loop for this write, so blocking on it starves nothing.
    fn write_all_blocking(&mut self) {
        if self.outbound.is_empty() {
            return;
        }
        let bytes: Vec<u8> = self.outbound.drain(..).collect();
        let fd = self.control.as_fd();
        let blocking = rustix::fs::fcntl_getfl(fd)
            .and_then(|flags| rustix::fs::fcntl_setfl(fd, flags - rustix::fs::OFlags::NONBLOCK));
        if blocking.is_ok() {
            // A host that is gone takes nothing more: the bytes are lost with the link.
            let _ = self.control.write_all(&bytes);
        }
    }

    fn finish(mut self) -> io::Result<()> {
        self.close_link();
        Ok(())
    }
}

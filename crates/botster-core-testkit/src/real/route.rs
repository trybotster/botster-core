//! The client end of a real route (R-47 A): one end of a Unix stream pair. Core receives the other end as the route's
//! transport and hands it to the worker process in a pump (Core OU-9). The harness keeps no copy of the worker's end, so
//! the client sees the worker's own writes, its close and the kernel's backpressure, and nothing else.
//!
//! A read waits on the stream only after the handoff. Core hands the transport off in a pump, and the conformance driver
//! pumps after a read of a route that finds nothing. So the first empty read returns at once, and a later read waits for
//! bytes until its deadline. A read that waited before the handoff would hold the only thread that pumps Core. The
//! driver's route wait (steward ruling R-68) reads with a deadline that has passed, which is one attempt that does not
//! wait, and it waits itself on [`RouteClient::readiness`] (this stream) and the host's wake together, so that a pump
//! runs when the host has work: the end of a route reaches the client only after a host pump (R-65).

use super::holders::{self, Socket};
use super::program::{Inputs, Requests};
use crate::edge_tap::Tap;
use botster_conformance::Deadline;
use botster_core::RealEdges;
use botster_core_conformance::RouteClient;
use botster_core_contract::prelude::{InstanceId, SessionId};
use botster_core_edges::edges::ProcessIdentity;
use botster_core_host::session::{row_key, Row};
use botster_hub_conformance::route::RouteRead;
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, Weak};
use std::time::Duration;

/// The controls that [`RealRoute::control`] runs.
pub(super) const CONTROLS: &[&str] = &[
    "client_close",
    "route_gate",
    "input_blocked",
    "route_accept",
    "drop_transport",
    "route_fill",
    "route_stream_written",
    "route_stream_holders",
];

/// The most bytes that one read of a `route_accept` drain takes: 64 KiB.
const DRAIN_CHUNK: usize = 1 << 16;

/// What the `route_fill` of a route needs (R-47 item 3, the control row): the requests to the probes, the worker of the
/// route's session as Core's stored row names it now, the `SO_SNDBUF` of the worker's end read before the handoff, and the
/// route's applied frame bound F.
pub(super) struct Fill {
    pub(super) requests: Requests,
    pub(super) worker: Box<dyn Fn() -> Result<ProcessIdentity, String>>,
    pub(super) worker_send_buffer: usize,
    pub(super) max_frame_bytes: usize,
}

/// What the `route_stream_holders` of a route needs (plan 23bf): the identity of the worker end's socket, read before Core
/// took the end, and the bytes that Core moved through that socket at its edges ([`crate::edge_tap::Tap::stream_bytes`]).
pub(super) struct Holders {
    pub(super) socket: Socket,
    pub(super) host_bytes: Box<dyn Fn() -> Result<u64, String>>,
}

/// N = C + F (R-47 item 3): C is what the transport holds outside the worker, the worker end's `SO_SNDBUF` plus the client
/// end's `SO_RCVBUF`; F is the route's frame bound. The kernel's reported sizes are a safe upper bound, so at least one
/// frame stays pending in the worker.
fn fill_size(
    worker_send_buffer: usize,
    client_receive_buffer: usize,
    max_frame_bytes: usize,
) -> usize {
    worker_send_buffer
        .saturating_add(client_receive_buffer)
        .saturating_add(max_frame_bytes)
}

/// The client end of one route.
pub(super) struct RealRoute {
    /// The client's end of the stream, always nonblocking; `None` after `client_close` or `drop_transport`.
    end: Option<UnixStream>,
    /// `None` for a route of the harness with no tap: it runs no `route_fill`.
    fill: Option<Fill>,
    /// `None` for a route of the harness with no tap: it runs no `route_stream_holders`.
    holders: Option<Holders>,
    /// The client bytes that wait for room in the stream.
    unsent: Vec<u8>,
    /// While on (`route_gate`), the client reads nothing from the stream beyond a `route_accept` allowance, so the worker's
    /// writes meet the kernel's backpressure.
    gated: bool,
    /// The bytes that the client may still take from the stream under the gate (`route_accept`); `None`: no allowance.
    accept: Option<usize>,
    /// The bytes that a `route_accept` drain took from the stream and no read has given yet, in order.
    taken: Vec<u8>,
    /// Set by the first empty read: the driver pumped after it, so the worker holds the stream.
    handed: bool,
    /// The bytes that the client's end read from the stream, and the unread bytes that a close of that end lost
    /// (`route_stream_written`).
    received: u64,
    /// The input log of the route's session (`pty_input`); `None` on the plain harness, which has no controls.
    input: Option<SessionInput>,
}

/// The bytes that the stream holds for the client's end and that the client has not read (`FIONREAD`). On Linux and macOS,
/// a write to an `AF_UNIX` stream socket puts its bytes on the receiving socket's queue at once, so these are all the bytes
/// that the kernel accepted from the worker and the client has not read.
fn unread(end: &UnixStream) -> io::Result<u64> {
    rustix::io::ioctl_fionread(end).map_err(io::Error::from)
}

/// What `pty_input` on a route reads: the tap of the route's handle (its stored rows), the route's session and the session's
/// instance at the attach, and the probes' input logs. The answer is [`crate::route_client::route_pty_log`], as on the testkit.
pub(super) struct SessionInput {
    /// Weak, as the harness's handle holds it: a route that outlives its host must not keep the host's edges, and with them its
    /// claim of the data directory (Core LC-2, LC-12).
    pub(super) tap: Weak<Mutex<Tap<RealEdges>>>,
    pub(super) session: SessionId,
    /// The session's instance when the route attached (`None`: the session had no stored row then).
    pub(super) instance: Option<InstanceId>,
    pub(super) inputs: Inputs,
}

impl SessionInput {
    /// The worker of the route's session instance, as its stored row names it now, and the input that its probe read. A
    /// session that was removed, or created again with a new instance, is no longer the route's: the route has no input log
    /// then. The record of the session's processes outlives `Remove`, so the stored row decides.
    fn log(&self) -> Result<Value, String> {
        let tap = self
            .tap
            .upgrade()
            .ok_or_else(|| format!("the host of the route of {} is gone", self.session.0))?;
        let row = tap
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stored_row(&row_key(&self.session))
            .and_then(|bytes| Row::decode(&self.session, &bytes))
            .filter(|row| Some(&row.instance) == self.instance.as_ref())
            .ok_or_else(|| format!("the route's session instance of {} is gone", self.session.0))?;
        let worker = row
            .worker
            .map(|worker| worker.identity())
            .ok_or_else(|| format!("the session {} has no worker process yet", self.session.0))?;
        let input = self.inputs.input(worker).map_err(|e| format!("{e:?}"))?;
        Ok(crate::route_client::route_pty_log(input))
    }
}

/// How long a read waits for bytes: only after the handoff, and only under a deadline that has time left. With no
/// deadline, a read does not wait, so it can never hold the run.
fn wait_of(handed: bool, remaining: Option<Duration>) -> Option<Duration> {
    remaining.filter(|left| handed && !left.is_zero())
}

/// What a read of the stream gives: bytes, `Empty` when nothing came, and the end of the route otherwise. A real route
/// reports no end cause (`ended`), because the stream carries none.
fn read_of(result: io::Result<usize>, buf: &[u8]) -> RouteRead {
    match result {
        Ok(0) => RouteRead::Eof { ended: None },
        Ok(n) => RouteRead::Bytes(buf[..n].to_vec()),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
            ) =>
        {
            RouteRead::Empty
        }
        Err(_) => RouteRead::Eof { ended: None },
    }
}

/// What a write result means for the unsent bytes: it drains the written bytes, and the flush goes on (true). A write
/// that makes no progress (`Ok(0)`, a full stream `WouldBlock`, a signal `Interrupted`) keeps the bytes and stops the
/// flush: the next write or read of the route flushes again, so a flush never spins (#238 RR-R1-1). Any other error
/// loses the bytes, as with a lost transport (Core OU-5).
fn after_write(result: io::Result<usize>, unsent: &mut Vec<u8>) -> bool {
    match result {
        Ok(0) => false,
        Ok(n) => {
            unsent.drain(..n);
            true
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            false
        }
        Err(_) => {
            unsent.clear();
            false
        }
    }
}

/// Writes `unsent` to `end` while each write makes progress: until nothing is left, or a write takes nothing
/// (`after_write`). Each pass drains at least one byte or ends the flush, so the loop ends.
fn flush_into(end: &mut impl Write, unsent: &mut Vec<u8>) {
    while !unsent.is_empty() {
        let result = end.write(unsent);
        if !after_write(result, unsent) {
            return;
        }
    }
}

/// The poll timeout of a wait.
fn timespec_of(wait: Duration) -> rustix::event::Timespec {
    rustix::event::Timespec {
        tv_sec: i64::try_from(wait.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: wait.subsec_nanos().into(),
    }
}

/// What the wait for the first byte gives: `None` when the stream is readable (bytes, its end or an error), so the read
/// follows; `Empty` when nothing came by the deadline or a signal ended the wait; the end of the route when the poll
/// failed.
fn after_poll(result: rustix::io::Result<usize>) -> Option<RouteRead> {
    match result {
        Ok(0) | Err(rustix::io::Errno::INTR) => Some(RouteRead::Empty),
        Ok(_) => None,
        Err(_) => Some(RouteRead::Eof { ended: None }),
    }
}

impl RealRoute {
    /// The route over `end`, the client's end of the stream.
    ///
    /// # Errors
    /// The end could not be made nonblocking.
    pub(super) fn new(end: UnixStream) -> io::Result<RealRoute> {
        end.set_nonblocking(true)?;
        Ok(RealRoute {
            end: Some(end),
            fill: None,
            holders: None,
            unsent: Vec::new(),
            gated: false,
            accept: None,
            taken: Vec::new(),
            handed: false,
            received: 0,
            input: None,
        })
    }

    /// The route runs `route_fill` with `fill`.
    pub(super) fn with_fill(mut self, fill: Fill) -> RealRoute {
        self.fill = Some(fill);
        self
    }

    /// The route runs `route_stream_holders` with `holders`.
    pub(super) fn with_holders(mut self, holders: Holders) -> RealRoute {
        self.holders = Some(holders);
        self
    }

    /// `route_stream_holders` (Core DP-1, DP-2, OU-1, OU-11; plan 23bf): `holders` is the number of open descriptors of the
    /// worker end's socket in every process ([`holders::holders`]): Core's copy in this process, the worker's,
    /// and any other. `host_bytes` is the bytes that Core moved through that socket: Core is sans-IO, and `HostEdges` is its
    /// only I/O path, so the tap's count of them is the count (lead, plan 23bf). A read through a raw copy outside the edges
    /// would break the sans-IO rule; such a copy shows in `holders`, and `slow:fd-handoff` is the real byte proof.
    fn route_stream_holders(&self) -> Result<Value, String> {
        let of = self
            .holders
            .as_ref()
            .ok_or("route_stream_holders needs a route of the tapped real harness")?;
        let holders =
            holders::holders(&of.socket).map_err(|e| format!("route_stream_holders: {e}"))?;
        let host_bytes = (of.host_bytes)()?;
        Ok(json!({ "holders": holders, "host_bytes": host_bytes }))
    }

    /// `route_fill` (R-47 item 3): the payload of the route's session writes N = C + F bytes of the fill pattern through the
    /// real `pty_output` path. The control returns when the probe acknowledges the request, never when the write completes
    /// (Core OU-3d). The pattern has no `0a` byte and no other byte that the terminal's output processing changes (R-48,
    /// R-60). A precondition that is not reached is an error, never a pass.
    fn route_fill(&self) -> Result<Value, String> {
        let fill = self
            .fill
            .as_ref()
            .ok_or("route_fill needs a route of the tapped real harness")?;
        let end = self
            .end
            .as_ref()
            .ok_or("route_fill: the client's end of the route is closed")?;
        let client_receive_buffer = rustix::net::sockopt::socket_recv_buffer_size(end)
            .map_err(|e| format!("route_fill: SO_RCVBUF of the client's end: {e}"))?;
        let bytes = fill_size(
            fill.worker_send_buffer,
            client_receive_buffer,
            fill.max_frame_bytes,
        );
        let worker = (fill.worker)().map_err(|e| format!("route_fill: {e}"))?;
        fill.requests
            .write_output(worker, crate::route_client::fill_pattern(bytes))
            .map_err(|e| format!("route_fill: {e:?}"))?;
        Ok(json!({ "bytes": bytes }))
    }

    /// `route_accept` (the control row; R-47 (A)): takes up to the allowance from the stream into `taken`, with one read and
    /// no wait. The room that these reads free in the kernel's buffers is the worker's progress (Core OU-3a: the bytes that
    /// the kernel accepted from the worker's write). Each later call of the route drains again, so bytes that come later, or
    /// that one read did not give, are taken then. A read that gives no bytes (nothing yet, the end, an error) takes nothing;
    /// the end or the error then reaches a read of the stream.
    fn drain(&mut self) {
        let RealRoute {
            end,
            accept,
            taken,
            received,
            ..
        } = self;
        let (Some(end), Some(left)) = (end.as_mut(), accept.as_mut()) else {
            return;
        };
        let mut buf = vec![0u8; (*left).min(DRAIN_CHUNK)];
        if let Ok(n) = end.read(&mut buf) {
            taken.extend_from_slice(&buf[..n]);
            *left -= n;
            *received += n as u64;
        }
    }

    /// The client's end is gone: its unread and taken bytes are lost, and no allowance is left. The kernel accepted the
    /// unread bytes from the worker, so `route_stream_written` keeps them. The caller drops the end after this call.
    fn closed(&mut self) {
        if let Some(end) = &self.end {
            self.received += unread(end).unwrap_or(0);
        }
        self.unsent.clear();
        self.taken.clear();
        self.accept = None;
    }

    /// The route answers `pty_input` from `input` (Core OU-1, DP-4: the conformance driver reads `route_expect_pty`'s log
    /// through the route).
    pub(super) fn with_input(mut self, input: SessionInput) -> RealRoute {
        self.input = Some(input);
        self
    }

    /// Writes the unsent bytes while the stream takes them. A failed write loses the stream's bytes, as with a lost
    /// transport (Core OU-5).
    fn flush(&mut self) {
        let Some(end) = self.end.as_mut() else {
            self.unsent.clear();
            return;
        };
        flush_into(end, &mut self.unsent);
    }

    /// One read of at most `max` bytes: a wait of `wait` for the first byte, or no wait. The stream stays nonblocking: the
    /// wait is a `poll` for readability, so the read never blocks.
    fn read_stream(end: &mut UnixStream, max: usize, wait: Option<Duration>) -> RouteRead {
        if let Some(wait) = wait {
            let mut fds = [rustix::event::PollFd::new(
                &*end,
                rustix::event::PollFlags::IN,
            )];
            // timer: deadline — the wait ends at the step deadline that the driver gave
            let polled = rustix::event::poll(&mut fds, Some(&timespec_of(wait)));
            if let Some(read) = after_poll(polled) {
                return read;
            }
        }
        let mut buf = vec![0u8; max];
        read_of(end.read(&mut buf), &buf)
    }
}

impl RouteClient for RealRoute {
    fn write(&mut self, bytes: &[u8]) {
        if self.end.is_some() {
            self.unsent.extend_from_slice(bytes);
        }
        self.flush();
        self.drain();
    }

    /// Bytes of the worker, `Empty` when none came (at once before the handoff, at the deadline after it), or the end of
    /// the route. The bytes that a `route_accept` took come first, gated or not: the stream took them from the worker.
    fn read(&mut self, max: usize, deadline: &Deadline) -> RouteRead {
        self.flush();
        self.drain();
        let read = match self.end.as_mut() {
            _ if !self.taken.is_empty() => {
                let n = max.min(self.taken.len());
                RouteRead::Bytes(self.taken.drain(..n).collect())
            }
            None => RouteRead::Eof { ended: None },
            Some(_) if self.gated => RouteRead::Empty,
            Some(end) => {
                let read =
                    RealRoute::read_stream(end, max, wait_of(self.handed, deadline.remaining()));
                if let RouteRead::Bytes(bytes) = &read {
                    self.received += bytes.len() as u64;
                }
                read
            }
        };
        if read == RouteRead::Empty {
            self.handed = true;
        }
        read
    }

    /// The controls of the client's end (`docs/core-testkit-controls.md`, step key `route`).
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        match op {
            // Core OU-5, A2-3: the client's end closes; a frame that the client had not completed is lost with it.
            "client_close" => {
                self.closed();
                if let Some(end) = self.end.take() {
                    let _ = end.shutdown(Shutdown::Both);
                }
                Ok(json!({}))
            }
            // Core OU-2, OU-3b (R-47 A): while on, the client reads nothing beyond a `route_accept` allowance, and the
            // worker's writes stop at the kernel's backpressure. `on: false` also ends the allowance.
            "route_gate" => {
                self.gated = crate::route_client::flag(args, "on")?;
                if !self.gated {
                    self.accept = None;
                }
                Ok(json!({}))
            }
            // Core OU-3a, OU-4 (the control row, R-47 A): the client takes at most `bytes` more bytes from the stream, and
            // then none until `route_gate` with `on: false`. The client drains them at once and at each later call of the
            // route, so the kernel takes that much more of the worker's output. A frame that straddles the allowance is
            // taken in part. No transcript asserts how many bytes the kernel took (R-47 A).
            "route_accept" => {
                let bytes = args
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .and_then(|b| usize::try_from(b).ok())
                    .ok_or("route_accept needs `bytes`")?;
                self.gated = true;
                self.accept = Some(bytes);
                self.drain();
                Ok(json!({}))
            }
            // Core OU-5 (the steward's real form): the client's end closes with no `shutdown`, and its unsent and unread
            // bytes are lost. A Unix stream has no reset: the worker sees `ECONNRESET` (Linux, with unread bytes at the close)
            // or the end of the stream and `EPIPE`, and Core closes the route `PeerClosed` for each. No `SO_LINGER`.
            "drop_transport" => {
                self.closed();
                self.end = None;
                Ok(json!({}))
            }
            // R-47 item 3, Core OU-3b: the session's output fills the transport and one frame more.
            "route_fill" => self.route_fill(),
            // Core OU-9, OU-3, DP-5 (plan 23bf): the bytes that the stream accepted from the worker, cumulative (R-14.3): what
            // the client read, and what the kernel holds for the client unread (`FIONREAD`). After a close of the client's
            // end, the count at the close stays.
            // Core DP-1, DP-2, OU-1, OU-11 (plan 23bf).
            "route_stream_holders" => self.route_stream_holders(),
            "route_stream_written" => {
                let held = match &self.end {
                    Some(end) => {
                        unread(end).map_err(|e| format!("route_stream_written: FIONREAD: {e}"))?
                    }
                    None => 0,
                };
                Ok(json!({ "bytes": self.received + held }))
            }
            // Core DP-5: whether the stream takes no more bytes from the client now (the worker has stopped reading the
            // route): client bytes still wait for room, or the kernel reports the client's end not writable. The worker
            // process reads on its own, so this is the stream boundary at the moment of the call, and nothing about the
            // clause.
            "input_blocked" => {
                self.flush();
                let end = self
                    .end
                    .as_ref()
                    .ok_or("input_blocked: the client's end is closed")?;
                let mut fds = [rustix::event::PollFd::new(
                    end,
                    rustix::event::PollFlags::OUT,
                )];
                // One descriptor: its `revents` alone says whether the end is writable (the count adds nothing).
                rustix::event::poll(&mut fds, Some(&timespec_of(Duration::ZERO)))
                    .map_err(|e| format!("input_blocked: poll: {e}"))?;
                let writable = fds[0].revents().contains(rustix::event::PollFlags::OUT);
                Ok(json!({"blocked": !self.unsent.is_empty() || !writable}))
            }
            // Core OU-1, DP-4: the input that reached the PTY of the route's session.
            "pty_input" => match &self.input {
                Some(input) => input.log(),
                None => Err(format!("unsupported_control: {op}")),
            },
            _ => Err(format!("unsupported_control: {op}")),
        }
    }

    fn has_control(&self, op: &str) -> bool {
        CONTROLS.contains(&op) || (op == "pty_input" && self.input.is_some())
    }

    /// The client's end of the stream itself, so the driver waits on it and on the host's wake together (steward ruling
    /// R-68). `None` while gated: a gated end reads nothing on purpose. `None` after `client_close`: a read gives the end
    /// at once.
    fn readiness(&self) -> Option<BorrowedFd<'_>> {
        if self.gated {
            return None;
        }
        self.end.as_ref().map(AsFd::as_fd)
    }
}

#[cfg(test)]
mod slow_tests {
    use super::*;
    use botster_test_process::{Bounded, Deadline as TestDeadline};
    use std::time::Instant;

    /// A route over one end of a pair, and the other end as the worker holds it.
    fn pair() -> (RealRoute, UnixStream) {
        let (client, worker) = UnixStream::pair().unwrap();
        (RealRoute::new(client).unwrap(), worker)
    }

    fn deadline(after: Duration) -> Deadline {
        Deadline::after(Some(after))
    }

    /// The step deadline of the tests that must not wait: long, so that a wait shows as a failed bound.
    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn a_read_waits_only_after_the_handoff_and_only_with_time_left() {
        let left = Duration::from_millis(5);
        assert_eq!(wait_of(true, Some(left)), Some(left));
        assert_eq!(wait_of(false, Some(left)), None);
        assert_eq!(wait_of(true, Some(Duration::ZERO)), None);
        assert_eq!(wait_of(true, None), None);
    }

    #[test]
    fn a_read_gives_bytes_empty_or_the_end_of_the_route() {
        let buf = [1u8, 2, 3];
        assert_eq!(read_of(Ok(2), &buf), RouteRead::Bytes(vec![1, 2]));
        assert_eq!(read_of(Ok(0), &buf), RouteRead::Eof { ended: None });
        for kind in [
            io::ErrorKind::WouldBlock,
            io::ErrorKind::TimedOut,
            io::ErrorKind::Interrupted,
        ] {
            assert_eq!(
                read_of(Err(kind.into()), &buf),
                RouteRead::Empty,
                "{kind:?}"
            );
        }
        assert_eq!(
            read_of(Err(io::ErrorKind::ConnectionReset.into()), &buf),
            RouteRead::Eof { ended: None }
        );
    }

    /// Before the handoff, an empty read returns at once, so the driver can pump Core; after it, a read waits until the
    /// deadline.
    /// A write drains what it wrote and the flush goes on; a write with no progress (`Ok(0)`, a full stream, a signal)
    /// keeps the bytes and stops the flush; any other error loses them (Core OU-5) and stops it.
    #[test]
    fn a_write_result_drains_keeps_or_loses_the_unsent_bytes() {
        let failed = |kind: io::ErrorKind| Err(io::Error::from(kind));
        let mut unsent = b"abcd".to_vec();
        assert!(after_write(Ok(3), &mut unsent));
        assert_eq!(unsent, b"d");
        assert!(!after_write(Ok(0), &mut unsent));
        assert_eq!(unsent, b"d");
        assert!(!after_write(failed(io::ErrorKind::WouldBlock), &mut unsent));
        assert_eq!(unsent, b"d");
        assert!(!after_write(
            failed(io::ErrorKind::Interrupted),
            &mut unsent
        ));
        assert_eq!(unsent, b"d");
        assert!(!after_write(failed(io::ErrorKind::BrokenPipe), &mut unsent));
        assert!(unsent.is_empty());
    }

    /// A stream that fails its next `interrupts` writes with `Interrupted`, then takes `chunk` bytes per write until
    /// `room` is used, then is full (`WouldBlock`). A `chunk` of 0 takes nothing (`Ok(0)`). A write past `allowed`
    /// panics, so a flush that writes again after a write with no progress fails at once and never spins (#238 round 1).
    struct Chunky {
        taken: Vec<u8>,
        room: usize,
        chunk: usize,
        interrupts: usize,
        writes: usize,
        allowed: usize,
    }

    impl Write for Chunky {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            assert!(
                self.writes <= self.allowed,
                "write {} after a write with no progress",
                self.writes
            );
            if self.interrupts > 0 {
                self.interrupts -= 1;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            if self.room == 0 {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            let n = buf.len().min(self.chunk).min(self.room);
            self.taken.extend_from_slice(&buf[..n]);
            self.room -= n;
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A flush writes until the stream is full, over several partial writes, and keeps the rest for the next flush. A
    /// write with no progress (a signal, or `Ok(0)`) ends the flush after that one write and keeps every byte; the next
    /// flush goes on (#238 RR-R1-1: a flush never spins).
    #[test]
    fn a_flush_writes_while_it_makes_progress_and_keeps_the_rest() {
        let mut end = Chunky {
            taken: Vec::new(),
            room: 4,
            chunk: 2,
            interrupts: 1,
            writes: 0,
            allowed: 1,
        };
        let mut unsent = b"abcdef".to_vec();
        flush_into(&mut end, &mut unsent);
        assert_eq!((end.writes, end.taken.as_slice()), (1, &b""[..]));
        assert_eq!(unsent, b"abcdef");
        // Two partial writes, then the full stream: three writes.
        end.allowed = 4;
        flush_into(&mut end, &mut unsent);
        assert_eq!(end.taken, b"abcd");
        assert_eq!(unsent, b"ef");
        end.room = 2;
        end.allowed = 5;
        flush_into(&mut end, &mut unsent);
        assert_eq!(end.taken, b"abcdef");
        assert!(unsent.is_empty());
        let mut stuck = Chunky {
            taken: Vec::new(),
            room: 4,
            chunk: 0,
            interrupts: 0,
            writes: 0,
            allowed: 1,
        };
        let mut unsent = b"ab".to_vec();
        flush_into(&mut stuck, &mut unsent);
        assert_eq!(stuck.writes, 1);
        assert_eq!(unsent, b"ab");
    }

    /// The wait for the first byte: nothing by the deadline or a signal gives `Empty`, a readable stream gives the read,
    /// and a failed poll ends the route. The poll timeout is the wait itself.
    #[test]
    fn a_poll_result_reads_gives_empty_or_ends_the_route() {
        assert_eq!(after_poll(Ok(0)), Some(RouteRead::Empty));
        assert_eq!(
            after_poll(Err(rustix::io::Errno::INTR)),
            Some(RouteRead::Empty)
        );
        assert_eq!(after_poll(Ok(1)), None);
        assert_eq!(
            after_poll(Err(rustix::io::Errno::BADF)),
            Some(RouteRead::Eof { ended: None })
        );
        let timeout = timespec_of(Duration::new(3, 5));
        assert_eq!((timeout.tv_sec, timeout.tv_nsec), (3, 5));
    }

    #[test]
    fn the_first_empty_read_returns_at_once_and_a_later_one_waits_for_the_deadline() {
        let (mut route, _worker) = pair();
        let start = Instant::now();
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        assert!(start.elapsed() < LONG, "{:?}", start.elapsed());
        let wait = Duration::from_millis(100);
        let start = Instant::now();
        assert_eq!(route.read(64, &deadline(wait)), RouteRead::Empty);
        assert!(start.elapsed() >= wait, "{:?}", start.elapsed());
    }

    /// The worker's bytes reach the client: before the handoff and after it, at most `max` of them for each read.
    #[test]
    fn the_workers_bytes_reach_the_client_in_order() {
        let (mut route, mut worker) = pair();
        worker.write_all(b"hello").unwrap();
        assert_eq!(
            route.read(3, &deadline(LONG)),
            RouteRead::Bytes(b"hel".to_vec())
        );
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"lo".to_vec())
        );
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        worker.write_all(b"!").unwrap();
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"!".to_vec())
        );
    }

    /// The worker's close ends the route with no cause, and the read does not wait for it.
    #[test]
    fn the_workers_close_ends_the_route() {
        let (mut route, worker) = pair();
        drop(worker);
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Eof { ended: None }
        );
    }

    /// The client's writes reach the worker in order, before the client's close ends the stream.
    #[test]
    fn the_clients_writes_reach_the_worker() {
        let (mut route, worker) = pair();
        route.write(b"ab");
        route.write(b"c");
        assert_eq!(route.control("client_close", &json!({})), Ok(json!({})));
        let got = Bounded::new(worker)
            .to_eof(TestDeadline::cleanup())
            .unwrap();
        assert_eq!(got, b"abc");
    }

    /// `client_close`: the worker reads the end of the stream, a later write goes nowhere and a later read is the end.
    #[test]
    fn a_client_close_ends_the_stream_for_both_ends() {
        let (mut route, worker) = pair();
        assert_eq!(route.control("client_close", &json!({})), Ok(json!({})));
        let got = Bounded::new(worker)
            .to_eof(TestDeadline::cleanup())
            .unwrap();
        assert!(got.is_empty(), "{got:?}");
        route.write(b"late");
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Eof { ended: None }
        );
    }

    /// `route_gate`: while on, the client reads nothing although bytes wait; off, it reads them. Absent `on` is on.
    #[test]
    fn a_gated_route_reads_nothing_until_the_gate_opens() {
        let (mut route, mut worker) = pair();
        worker.write_all(b"held").unwrap();
        assert_eq!(route.control("route_gate", &json!({})), Ok(json!({})));
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        assert_eq!(
            route.control("route_gate", &json!({"on": false})),
            Ok(json!({}))
        );
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"held".to_vec())
        );
        assert_eq!(
            route.control("route_gate", &json!({"on": true})),
            Ok(json!({}))
        );
        worker.write_all(b"more").unwrap();
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        assert!(route.control("route_gate", &json!({"on": 1})).is_err());
    }

    /// `route_accept` (the control row): under the gate the client takes at most the allowance, a read gives those bytes
    /// first, a second allowance takes more, and `on: false` releases the rest. Without a gate, an allowance gates the
    /// route after its bytes.
    #[test]
    fn route_accept_takes_its_bytes_under_the_gate_and_then_none() {
        let (mut route, mut worker) = pair();
        worker.write_all(b"abcdefghij").unwrap();
        route.control("route_gate", &json!({})).unwrap();
        assert_eq!(
            route.control("route_accept", &json!({"bytes": 3})),
            Ok(json!({}))
        );
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"abc".to_vec())
        );
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        route.control("route_accept", &json!({"bytes": 4})).unwrap();
        assert_eq!(
            route.read(2, &deadline(LONG)),
            RouteRead::Bytes(b"de".to_vec()),
            "a read gives at most `max` of the taken bytes"
        );
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"fg".to_vec())
        );
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        route.control("route_gate", &json!({"on": false})).unwrap();
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"hij".to_vec())
        );
        // No gate: the allowance gates the route after its bytes.
        worker.write_all(b"klmn").unwrap();
        route.control("route_accept", &json!({"bytes": 1})).unwrap();
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"k".to_vec())
        );
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
        assert!(route.control("route_accept", &json!({})).is_err());
        assert!(route
            .control("route_accept", &json!({"bytes": -1}))
            .is_err());
        // `on: true` keeps an allowance in effect: bytes that come after it are taken under the new gate.
        let (mut kept, mut writer) = pair();
        kept.control("route_accept", &json!({"bytes": 3})).unwrap();
        kept.control("route_gate", &json!({"on": true})).unwrap();
        writer.write_all(b"abcde").unwrap();
        assert_eq!(
            kept.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"abc".to_vec())
        );
        // `on: false` ends an allowance that is not used up: under a new gate, the client takes nothing more.
        route.control("route_accept", &json!({"bytes": 8})).unwrap();
        route.control("route_gate", &json!({"on": false})).unwrap();
        route.control("route_gate", &json!({"on": true})).unwrap();
        worker.write_all(b"op").unwrap();
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(b"lmn".to_vec())
        );
        assert_eq!(route.read(64, &deadline(LONG)), RouteRead::Empty);
    }

    /// OU-3a on the real tier: the bytes that a `route_accept` takes free room in the kernel, so a worker whose writes were
    /// blocked can write again, while the gate holds.
    #[test]
    fn route_accept_frees_room_for_the_blocked_worker() {
        let (mut route, mut worker) = pair();
        worker.set_nonblocking(true).unwrap();
        route.control("route_gate", &json!({})).unwrap();
        let chunk = [b'x'; 4096];
        let mut blocked = false;
        // A bound, not a wait: the kernel's buffers hold far fewer than 4096 chunks.
        for _ in 0..4096 {
            match worker.write(&chunk) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    blocked = true;
                    break;
                }
                Err(e) => panic!("{e}"),
            }
        }
        assert!(blocked, "the worker's writes fill the kernel's buffers");
        assert_eq!(
            worker.write(&chunk).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        route
            .control("route_accept", &json!({"bytes": 4 * 4096}))
            .unwrap();
        assert!(
            worker.write(&chunk).is_ok(),
            "the taken bytes made room for the worker"
        );
        assert_eq!(
            route.read(64, &deadline(LONG)),
            RouteRead::Bytes(vec![b'x'; 64])
        );
    }

    /// `client_close` and `drop_transport` lose the bytes that a `route_accept` took and end the allowance.
    #[test]
    fn a_closed_end_loses_the_taken_bytes() {
        for op in ["client_close", "drop_transport"] {
            let (mut route, mut worker) = pair();
            worker.write_all(b"abc").unwrap();
            route.control("route_accept", &json!({"bytes": 2})).unwrap();
            route.control(op, &json!({})).unwrap();
            assert_eq!(
                route.read(64, &deadline(LONG)),
                RouteRead::Eof { ended: None },
                "{op}"
            );
        }
    }

    /// Steward ruling R-68: the readiness descriptor is the client's end itself, so a write of the worker makes it
    /// readable. A gated end and a closed end give none. After the handoff, a read with a deadline that has passed is one
    /// attempt that does not wait.
    #[test]
    fn the_readiness_is_the_clients_end_and_a_read_with_a_passed_deadline_does_not_wait() {
        let readable = |route: &RealRoute| {
            let fd = route.readiness().expect("an open end that is not gated");
            let mut fds = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            rustix::event::poll(&mut fds, Some(&timespec_of(Duration::ZERO))).unwrap() > 0
        };
        let (mut route, mut worker) = pair();
        // The first empty read is the handoff: later reads with time left wait.
        assert_eq!(route.read(64, &deadline(Duration::ZERO)), RouteRead::Empty);
        assert!(!readable(&route));
        let started = Instant::now();
        assert_eq!(route.read(64, &deadline(Duration::ZERO)), RouteRead::Empty);
        assert!(started.elapsed() < LONG / 2, "{:?}", started.elapsed());
        worker.write_all(b"x").unwrap();
        assert!(readable(&route));
        assert_eq!(route.control("route_gate", &json!({})), Ok(json!({})));
        assert!(
            route.readiness().is_none(),
            "a gated end reads nothing on purpose"
        );
        assert_eq!(
            route.control("route_gate", &json!({"on": false})),
            Ok(json!({}))
        );
        assert_eq!(
            route.read(64, &deadline(Duration::ZERO)),
            RouteRead::Bytes(b"x".to_vec())
        );
        assert_eq!(route.control("client_close", &json!({})), Ok(json!({})));
        assert!(
            route.readiness().is_none(),
            "a closed end gives its end at once"
        );
    }

    /// The route serves only its own controls; any other is `unsupported_control`.
    /// `input_blocked` (Core DP-5): while the worker's end reads nothing, the client's bytes fill the stream, and the route
    /// reports blocked. When the worker's end reads everything, the route sends the waiting bytes first and reports not
    /// blocked. No byte is lost. A closed client end is an error.
    #[test]
    fn input_blocked_reports_a_stream_that_takes_no_more_client_bytes() {
        /// Far more than a Unix stream's socket buffers on macOS and Linux.
        const VOLUME: usize = 4 * 1024 * 1024;
        let (mut route, mut worker) = pair();
        let blocked = |route: &mut RealRoute| route.control("input_blocked", &json!({}));
        assert_eq!(blocked(&mut route), Ok(json!({"blocked": false})));
        let sent: Vec<u8> = (0..VOLUME).map(|i| (i % 251) as u8).collect();
        route.write(&sent);
        assert_eq!(blocked(&mut route), Ok(json!({"blocked": true})));
        worker.set_nonblocking(true).unwrap();
        let mut got = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        // A `WouldBlock` read means the worker's end is empty; `input_blocked` then sends the waiting bytes, and a Unix stream
        // makes written bytes readable at once. So a second `WouldBlock` in a row is a route that makes no progress.
        let mut empty_before = false;
        while got.len() < VOLUME {
            match worker.read(&mut buf) {
                Ok(0) => panic!("the stream ended after {} of {VOLUME} bytes", got.len()),
                Ok(n) => {
                    got.extend_from_slice(&buf[..n]);
                    empty_before = false;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        !empty_before,
                        "no progress after {} of {VOLUME} bytes",
                        got.len()
                    );
                    empty_before = true;
                    route.control("input_blocked", &json!({})).unwrap();
                }
                Err(e) => panic!("{e}"),
            }
        }
        assert_eq!(got, sent, "every byte, in order");
        assert_eq!(blocked(&mut route), Ok(json!({"blocked": false})));
        route.control("client_close", &json!({})).unwrap();
        assert!(blocked(&mut route).is_err());
    }

    #[test]
    fn the_route_serves_only_its_own_controls() {
        let (mut route, _worker) = pair();
        for op in CONTROLS {
            assert!(route.has_control(op), "{op}");
        }
        assert!(!route.has_control("fail_writes"));
        assert_eq!(
            route.control("fail_writes", &json!({})),
            Err("unsupported_control: fail_writes".to_string())
        );
        // A route with no input log of its session (the plain harness) has no `pty_input`.
        assert!(!route.has_control("pty_input"));
        assert_eq!(
            route.control("pty_input", &json!({})),
            Err("unsupported_control: pty_input".to_string())
        );
    }

    /// The `route_fill` row: C is an upper bound only while Core does not change the buffer sizes of the route's socket; the
    /// Core lead confirms that it does not. No source of a workspace crate outside this testkit sets a socket option, except
    /// tests (a `tests` directory or a `tests.rs` file). The test walks the tree, because a mutation copy has no `.git`.
    #[test]
    fn no_crate_sets_a_socket_buffer_size_outside_tests() {
        const SETTERS: [&str; 3] = [
            "set_socket_send_buffer_size",
            "set_socket_recv_buffer_size",
            "setsockopt",
        ];
        let mut dirs = vec![crate::statements::workspace_root().join("crates")];
        let (mut read, mut found) = (vec![], vec![]);
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    if !["botster-core-testkit", "tests", "vendor", "target"]
                        .contains(&name.as_str())
                    {
                        dirs.push(path);
                    }
                } else if name.ends_with(".rs") && name != "tests.rs" {
                    read.push(path.clone());
                    let text = std::fs::read_to_string(&path).unwrap();
                    found.extend(
                        SETTERS
                            .iter()
                            .filter(|setter| text.contains(*setter))
                            .map(|setter| format!("{}: {setter}", path.display())),
                    );
                }
            }
        }
        // The walk reaches the sources that hold the route's socket: the descriptor pass (it names `SO_SNDBUF`), the
        // worker's route and the facade.
        for source in [
            "crates/botster-core-sys/src/fd_pass.rs",
            "crates/botster-worker-core/src/worker/route.rs",
            "crates/botster-core/src/lib.rs",
        ] {
            let source = crate::statements::workspace_root().join(source);
            assert!(read.contains(&source), "{} not read", source.display());
        }
        assert_eq!(found, Vec::<String>::new());
    }

    /// R-47 item 3: N = C + F, with C the worker end's send buffer plus the client end's receive buffer; it saturates.
    #[test]
    fn the_fill_is_both_buffers_and_one_frame() {
        assert_eq!(fill_size(8192, 4096, 1000), 13_288);
        assert_eq!(fill_size(usize::MAX, 1, 1), usize::MAX);
        assert_eq!(fill_size(0, 0, 7), 7);
    }

    /// A route with no fill context (the harness with no tap) refuses `route_fill`; a closed client end refuses it too.
    #[test]
    fn route_fill_needs_the_tapped_harness_and_an_open_end() {
        let (mut route, _worker) = pair();
        assert!(route
            .control("route_fill", &json!({}))
            .unwrap_err()
            .contains("tapped"));
        route.control("client_close", &json!({})).unwrap();
        let root = tempfile::tempdir().unwrap();
        let programs = super::super::program::Programs::listen(root.path()).unwrap();
        route.fill = Some(Fill {
            requests: programs.requests(),
            worker: Box::new(|| Err("unused".into())),
            worker_send_buffer: 1,
            max_frame_bytes: 1,
        });
        assert!(route
            .control("route_fill", &json!({}))
            .unwrap_err()
            .contains("closed"));
    }

    /// `route_stream_holders` (plan 23bf) needs the tapped harness. It counts the descriptors of the worker end's socket,
    /// also after the client's end closed, and gives the tap's count of Core's bytes.
    #[test]
    fn route_stream_holders_counts_the_worker_ends_descriptors_and_gives_the_host_bytes() {
        let (mut route, worker) = pair();
        assert!(route
            .control("route_stream_holders", &json!({}))
            .unwrap_err()
            .contains("tapped"));
        let socket = holders::socket(std::os::fd::AsFd::as_fd(&worker)).unwrap();
        route = route.with_holders(Holders {
            socket,
            host_bytes: Box::new(|| Ok(7)),
        });
        let answer =
            |route: &mut RealRoute| route.control("route_stream_holders", &json!({})).unwrap();
        assert_eq!(answer(&mut route), json!({"holders": 1, "host_bytes": 7}));
        let copy = worker.try_clone().unwrap();
        assert_eq!(
            answer(&mut route),
            json!({"holders": 2, "host_bytes": 7}),
            "a kept copy shows"
        );
        drop((worker, copy));
        route.control("client_close", &json!({})).unwrap();
        assert_eq!(answer(&mut route), json!({"holders": 0, "host_bytes": 7}));
    }

    /// `drop_transport` (the steward's real form of OU-5): the client's end closes with no shutdown, and its unsent and
    /// unread bytes are lost. The worker reads the end of the stream, or `ECONNRESET` when bytes were unread (Linux), and its
    /// writes fail; the client reads the end.
    #[test]
    fn drop_transport_closes_the_client_end_and_loses_its_bytes() {
        let (mut route, mut worker) = pair();
        worker.write_all(b"unread").unwrap();
        route.control("drop_transport", &json!({})).unwrap();
        route.write(b"lost");
        // Nonblocking: the close is done, so the read gives the end or the reset at once; a wait would be a regression.
        worker.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 8];
        match worker.read(&mut buf) {
            Ok(0) => {}
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("the worker reads the end or a reset, not {other:?}"),
        }
        assert!(worker.write_all(b"x").is_err(), "no peer");
        assert_eq!(
            route.read(8, &deadline(Duration::ZERO)),
            RouteRead::Eof { ended: None }
        );
    }

    fn written(route: &mut RealRoute) -> u64 {
        route.control("route_stream_written", &json!({})).unwrap()["bytes"]
            .as_u64()
            .unwrap()
    }

    /// Writes `worker`'s bytes until a write makes no progress (the stream takes no more), and gives how many it took. Each
    /// write that continues takes at least one byte, and the kernel's buffers are finite, so the loop ends.
    fn fill(worker: &mut UnixStream) -> u64 {
        worker.set_nonblocking(true).unwrap();
        let chunk = [b'x'; 4096];
        let mut took = 0u64;
        loop {
            match worker.write(&chunk) {
                Ok(n) if n > 0 => took += n as u64,
                Ok(_) => return took,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return took
                }
                Err(e) => panic!("the worker's write: {e}"),
            }
        }
    }

    /// `route_stream_written` (Core OU-9, OU-3, DP-5; plan 23bf) on the OS that runs the test (the Linux gate and the Mac slow
    /// tier): the bytes that the client read, a `route_accept` drain included, plus `FIONREAD` at the client's end equal the
    /// bytes that the worker's writes gave the stream, also when the stream is full. So the kernel keeps no accepted byte
    /// that `FIONREAD` does not count. After `drop_transport` or `client_close`, the count at the close stays.
    #[test]
    fn the_written_count_is_what_the_client_read_and_what_the_stream_holds_unread() {
        for close in ["drop_transport", "client_close"] {
            let (mut route, mut worker) = pair();
            assert_eq!(written(&mut route), 0, "{close}");
            worker.write_all(b"abcdef").unwrap();
            assert_eq!(written(&mut route), 6, "6 unread");
            assert_eq!(
                route.read(2, &deadline(LONG)),
                RouteRead::Bytes(b"ab".to_vec())
            );
            assert_eq!(written(&mut route), 6, "2 read, 4 unread");
            route.control("route_accept", &json!({"bytes": 3})).unwrap();
            assert_eq!(written(&mut route), 6, "2 read, 3 taken, 1 unread");
            let total = 6 + fill(&mut worker);
            assert!(total > 6, "the stream took bytes");
            assert_eq!(written(&mut route), total, "a full stream");
            route.control(close, &json!({})).unwrap();
            assert_eq!(written(&mut route), total, "{close}: the count stays");
        }
    }
}

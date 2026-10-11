//! The client end of a real route (R-47 A): one end of a Unix stream pair. Core receives the other end as the route's
//! transport and hands it to the worker process in a pump (Core OU-9). The harness keeps no copy of the worker's end, so
//! the client sees the worker's own writes, its close and the kernel's backpressure, and nothing else.
//!
//! A read waits on the stream only after the handoff. Core hands the transport off in a pump, and the conformance driver
//! pumps once at the first read of a route that finds nothing ("one pump per route"). So the first empty read returns at
//! once, and every later read waits for bytes until the step deadline. A read that waited before the handoff would hold
//! the only thread that pumps Core.

use super::program::Inputs;
use super::SharedTap;
use botster_conformance::Deadline;
use botster_core_conformance::RouteClient;
use botster_core_contract::prelude::{InstanceId, SessionId};
use botster_core_host::session::{row_key, Row};
use botster_hub_conformance::route::RouteRead;
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// The controls that [`RealRoute::control`] runs.
pub(super) const CONTROLS: &[&str] = &["client_close", "route_gate"];

/// The client end of one route.
pub(super) struct RealRoute {
    /// The client's end of the stream, always nonblocking; `None` after `client_close`.
    end: Option<UnixStream>,
    /// The client bytes that wait for room in the stream.
    unsent: Vec<u8>,
    /// While on (`route_gate`), the client reads nothing, so the worker's writes meet the kernel's backpressure.
    gated: bool,
    /// Set by the first empty read: the driver pumped after it, so the worker holds the stream.
    handed: bool,
    /// The input log of the route's session (`pty_input`); `None` on the plain harness, which has no controls.
    input: Option<SessionInput>,
}

/// What `pty_input` on a route reads: the tap of the route's handle (its stored rows), the route's session and the session's
/// instance at the attach, and the probes' input logs. The answer is [`crate::route_client::route_pty_log`], as on the testkit.
pub(super) struct SessionInput {
    pub(super) tap: SharedTap,
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
        let row = self
            .tap
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
            unsent: Vec::new(),
            gated: false,
            handed: false,
            input: None,
        })
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
    }

    /// Bytes of the worker, `Empty` when none came (at once before the handoff, at the deadline after it), or the end of
    /// the route.
    fn read(&mut self, max: usize, deadline: &Deadline) -> RouteRead {
        self.flush();
        let read = match self.end.as_mut() {
            None => RouteRead::Eof { ended: None },
            Some(_) if self.gated => RouteRead::Empty,
            Some(end) => {
                RealRoute::read_stream(end, max, wait_of(self.handed, deadline.remaining()))
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
                self.unsent.clear();
                if let Some(end) = self.end.take() {
                    let _ = end.shutdown(Shutdown::Both);
                }
                Ok(json!({}))
            }
            // Core OU-2, OU-3b (R-47 A): while on, the client reads nothing, and the worker's writes stop at the
            // kernel's backpressure.
            "route_gate" => {
                self.gated = crate::route_client::flag(args, "on")?;
                Ok(json!({}))
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

    /// The route serves only its own controls; any other is `unsupported_control`.
    #[test]
    fn the_route_serves_only_its_own_controls() {
        let (mut route, _worker) = pair();
        for op in CONTROLS {
            assert!(route.has_control(op), "{op}");
        }
        assert!(!route.has_control("route_fill"));
        assert_eq!(
            route.control("route_fill", &json!({})),
            Err("unsupported_control: route_fill".to_string())
        );
        // A route with no input log of its session (the plain harness) has no `pty_input`.
        assert!(!route.has_control("pty_input"));
        assert_eq!(
            route.control("pty_input", &json!({})),
            Err("unsupported_control: pty_input".to_string())
        );
    }
}

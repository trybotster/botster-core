//! The client end of a real route (R-47 A): one end of a Unix stream pair. Core receives the other end as the route's
//! transport and hands it to the worker process in a pump (Core OU-9). The harness keeps no copy of the worker's end, so
//! the client sees the worker's own writes, its close and the kernel's backpressure, and nothing else.
//!
//! A read waits on the stream only after the handoff. Core hands the transport off in a pump, and the conformance driver
//! pumps once at the first read of a route that finds nothing ("one pump per route"). So the first empty read returns at
//! once, and every later read waits for bytes until the step deadline. A read that waited before the handoff would hold
//! the only thread that pumps Core.

use botster_conformance::Deadline;
use botster_core_conformance::RouteClient;
use botster_hub_conformance::route::RouteRead;
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// The controls that [`RealRoute::control`] runs.
pub(super) const CONTROLS: &[&str] = &["client_close", "route_gate"];

/// The client end of one route.
#[derive(Debug)]
pub(super) struct RealRoute {
    /// The client's end of the stream, nonblocking outside a wait; `None` after `client_close`.
    end: Option<UnixStream>,
    /// The client bytes that wait for room in the stream.
    unsent: Vec<u8>,
    /// While on (`route_gate`), the client reads nothing, so the worker's writes meet the kernel's backpressure.
    gated: bool,
    /// Set by the first empty read: the driver pumped after it, so the worker holds the stream.
    handed: bool,
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
        })
    }

    /// Writes the unsent bytes while the stream takes them. A failed write loses the stream's bytes, as with a lost
    /// transport (Core OU-5).
    fn flush(&mut self) {
        let Some(end) = self.end.as_mut() else {
            self.unsent.clear();
            return;
        };
        while !self.unsent.is_empty() {
            match end.write(&self.unsent) {
                Ok(n) => {
                    self.unsent.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => self.unsent.clear(),
            }
        }
    }

    /// One read of at most `max` bytes: a wait of `wait` for the first byte, or no wait.
    fn read_stream(end: &mut UnixStream, max: usize, wait: Option<Duration>) -> RouteRead {
        let mut buf = vec![0u8; max];
        let Some(wait) = wait else {
            return read_of(end.read(&mut buf), &buf);
        };
        if end.set_nonblocking(false).is_err() || end.set_read_timeout(Some(wait)).is_err() {
            return RouteRead::Eof { ended: None };
        }
        // timer: deadline — the wait ends at the step deadline that the driver gave
        let result = end.read(&mut buf);
        if end.set_nonblocking(true).is_err() {
            return RouteRead::Eof { ended: None };
        }
        read_of(result, &buf)
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
            _ => Err(format!("unsupported_control: {op}")),
        }
    }

    fn has_control(&self, op: &str) -> bool {
        CONTROLS.contains(&op)
    }
}

#[cfg(test)]
mod tests {
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
        assert!(route.unsent.is_empty());
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
    }
}

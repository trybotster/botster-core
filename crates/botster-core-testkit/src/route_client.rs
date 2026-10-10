//! The client end of a stream route (`attach_stream`, P4a): the in-memory stream whose other end Core hands to the worker
//! (DP-2). A read runs the workers' ready work and no host pump, because the data plane is the worker's (Core TH-3).

use crate::core::RowReader;
use crate::net::StreamEnd;
use crate::worker::Workers;
use botster_conformance::Deadline;
use botster_core_conformance::RouteClient;
use botster_core_contract::prelude::{InstanceId, RouteCloseReason, RouteId, SessionId};
use botster_core_edges::RouteTransport;
use botster_core_host::session::Row;
use botster_hub_conformance::route::RouteRead;
use botster_route_codec::prelude::RouteEndReason;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, Mutex};

/// The queue of each direction of a route stream: a socket buffer's size, so a large baseline is written in parts.
pub const ROUTE_STREAM_BYTES: usize = 64 * 1024;

/// The route-ended cause of each route that Core closed for a failed reason (Core A2-3, OU-2b). A failed route ends with no
/// `route_closed` frame, and a Hub tells its client the cause (Codec TS-9), so the client end reports it at its end of
/// stream. The testkit records Core's `RouteClosed` events here; a real harness records them the same way.
#[derive(Debug, Clone, Default)]
pub struct RouteEnds(Arc<Mutex<BTreeMap<RouteId, RouteEndReason>>>);

impl RouteEnds {
    /// Records a close. A healthy reason has its `route_closed` frame and no route-ended cause.
    pub fn closed(&self, route: RouteId, reason: RouteCloseReason) {
        let ended = match reason {
            RouteCloseReason::HandoffFailed => RouteEndReason::HandoffFailed,
            RouteCloseReason::WriteFailed => RouteEndReason::WriteFailed,
            RouteCloseReason::StallTimeout => RouteEndReason::Stalled,
            RouteCloseReason::SessionLost => RouteEndReason::SessionLost,
            RouteCloseReason::PeerClosed => RouteEndReason::TransportLost,
            _ => return,
        };
        self.0.lock().expect("not poisoned").insert(route, ended);
    }

    /// The wire name of the route's cause (Codec A.2), if Core closed it for a failed reason.
    fn ended(&self, route: RouteId) -> Option<String> {
        let reason = *self.0.lock().expect("not poisoned").get(&route)?;
        serde_json::to_value(reason)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
    }
}

/// The controls that `TestkitRoute::control` runs.
pub(crate) const ROUTE_CONTROLS: &[&str] = &[
    "client_close",
    "route_stream_holders",
    "route_gate",
    "route_accept",
    "fail_writes",
    "drop_transport",
    "route_stream_written",
    "route_fill",
];

/// The optional bool `key` of a control's arguments; absent is true (`route_gate`, `fail_writes`).
fn flag(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None => Ok(true),
        Some(Value::Bool(on)) => Ok(*on),
        Some(other) => Err(format!("`{key}` is a bool, not {other}")),
    }
}

/// The byte `k` of a `route_fill` is 0x61 + (k mod 26), with `k` from 0 for each fill (R-47 item 3).
fn fill_pattern(bytes: usize) -> Vec<u8> {
    (0..bytes).map(|k| b'a' + (k % 26) as u8).collect()
}

/// The stored row of the route's session, and the session instance that the route attached to.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub rows: RowReader,
    pub session: SessionId,
    pub instance: InstanceId,
}

/// What a `route_fill` needs of the route: the row of its session, whose worker's program edge the fill writes to, and the
/// route's applied frame bound F (`AttachResult.limits.max_frame_bytes`).
#[derive(Debug, Clone)]
pub struct RouteFill {
    pub session: Option<SessionRow>,
    pub max_frame_bytes: usize,
}

pub struct TestkitRoute {
    end: StreamEnd,
    route: RouteId,
    ends: RouteEnds,
    workers: Workers,
    /// Client bytes that the full stream did not take yet, in order. They go before any later bytes.
    unsent: Vec<u8>,
    /// The stream failed: the client's bytes have nowhere to go.
    failed: bool,
    /// `None` for a route that `attach_stream` did not build: it runs no `route_fill`.
    fill: Option<RouteFill>,
}

impl TestkitRoute {
    pub fn new(end: StreamEnd, workers: Workers, route: RouteId, ends: RouteEnds) -> TestkitRoute {
        TestkitRoute {
            end,
            route,
            ends,
            workers,
            unsent: Vec::new(),
            failed: false,
            fill: None,
        }
    }

    /// The route runs `route_fill` with its session's worker and its frame bound.
    pub fn with_fill(mut self, fill: RouteFill) -> TestkitRoute {
        self.fill = Some(fill);
        self
    }

    /// `route_fill` (R-47 item 3): the session's program writes N = C + F bytes of the fill pattern, where C is the bytes that
    /// the stream takes from the worker now (0 while gated) and F is the route's frame bound. The worker's queue to the route
    /// then holds a frame that the stream does not take. The control returns when the program edge has the bytes, and the
    /// host is woken. A payload that has exited is refused.
    fn route_fill(&mut self) -> Result<Value, String> {
        let fill = self
            .fill
            .as_ref()
            .ok_or("route_fill needs a route that attach_stream built")?;
        let max_frame_bytes = fill.max_frame_bytes;
        let session = fill
            .session
            .as_ref()
            .ok_or("route_fill: the route has no session row")?;
        // The worker of the route's own session instance, as the row names it now (the worker can come after the attach,
        // while `Starting`). A removed or recreated session is not the route's.
        let row = session
            .rows
            .read()
            .and_then(|bytes| Row::decode(&session.session, &bytes))
            .filter(|row| row.instance == session.instance)
            .ok_or("route_fill: the route's session instance is gone")?;
        let worker = row
            .worker
            .ok_or("route_fill: the route's session has no worker process yet")?
            .identity();
        if !self.workers.payload_alive(worker) {
            return Err(
                "route_fill: the payload of the route's session has exited: it writes nothing more"
                    .into(),
            );
        }
        let (program, wake) = self.workers.program_edge(worker)?;
        let bytes = self.worker_end().room().saturating_add(max_frame_bytes);
        program.write(&fill_pattern(bytes));
        if let Some(wake) = wake {
            wake.signal();
        }
        Ok(json!({"bytes": bytes}))
    }

    /// Writes the waiting bytes while the stream takes them. `Interrupted` writes again; any other error ends the writes.
    fn flush(&mut self) {
        while !self.unsent.is_empty() {
            match self.end.write(&self.unsent) {
                Ok(n) => {
                    self.unsent.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                // The stream is lost: its bytes go with it, as with a lost transport (OU-5).
                Err(_) => {
                    self.failed = true;
                    self.unsent.clear();
                }
            }
        }
    }

    /// The controls of the worker's end of the stream: they hold after the end moved into the worker (DP-2).
    fn worker_end(&mut self) -> crate::net::EndControl {
        self.end.end().control().peer()
    }

    /// The client bytes that wait for room in the stream.
    pub fn unsent(&self) -> usize {
        self.unsent.len()
    }

    fn try_read(&mut self, max: usize) -> Option<RouteRead> {
        let mut buf = vec![0u8; max.max(1)];
        match self.end.read(&mut buf) {
            Ok(0) => Some(RouteRead::Eof {
                ended: self.ends.ended(self.route),
            }),
            Ok(n) => {
                buf.truncate(n);
                Some(RouteRead::Bytes(buf))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(_) => Some(RouteRead::Eof {
                ended: self.ends.ended(self.route),
            }),
        }
    }
}

impl RouteClient for TestkitRoute {
    /// Queues the bytes after any that wait, and writes what the stream takes. A full stream keeps the rest in order, and
    /// each later write or read sends it first (no byte is lost). After `client_close`, or when the stream failed, the bytes
    /// go nowhere.
    fn write(&mut self, bytes: &[u8]) {
        if !self.failed {
            self.unsent.extend_from_slice(bytes);
        }
        self.flush();
    }

    /// The bytes that wait, or the bytes that the workers' ready work writes; `Empty` when that writes none.
    fn read(&mut self, max: usize, _deadline: &Deadline) -> RouteRead {
        self.flush();
        if let Some(read) = self.try_read(max) {
            return read;
        }
        self.workers.run_ready();
        self.try_read(max).unwrap_or(RouteRead::Empty)
    }

    /// The controls of the route's stream (`docs/core-testkit-controls.md`, step key `route`).
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        match op {
            // Core OU-5, A2-3: the client's end closes; a frame that the client had not completed is lost with it.
            "client_close" => {
                self.unsent.clear();
                self.failed = true;
                self.end.close();
                Ok(json!({}))
            }
            // Core DP-1, DP-2, OU-1: the live handles of the stream's worker end, and the bytes that moved through it before
            // the worker held it.
            "route_stream_holders" => {
                let (holders, host_bytes) = self.end.end().control().peer_holders();
                Ok(json!({"holders": holders, "host_bytes": host_bytes}))
            }
            // Core OU-2, OU-3b: while on, the stream takes no byte from the worker. `on: false` also ends a `route_accept`
            // limit.
            "route_gate" => {
                let on = flag(args, "on")?;
                self.worker_end().gate(on);
                Ok(json!({}))
            }
            // Core OU-3a, OU-4: the stream takes at most `bytes` more bytes from the worker, then none.
            "route_accept" => {
                let bytes = args
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .and_then(|b| usize::try_from(b).ok())
                    .ok_or("route_accept needs `bytes`")?;
                self.worker_end().accept_at_most(bytes);
                Ok(json!({}))
            }
            // Core OU-2b, A2-3: the next write of the worker to the stream fails.
            "fail_writes" => {
                if flag(args, "on")? {
                    self.worker_end().fail_next_write(io::ErrorKind::BrokenPipe);
                } else {
                    self.worker_end().clear_write_failure();
                }
                Ok(json!({}))
            }
            // Core OU-5: the stream is reset with no close handshake.
            "drop_transport" => {
                self.worker_end().reset();
                Ok(json!({}))
            }
            // Core OU-9, OU-3, DP-5: the bytes that the stream took from the worker, cumulative (R-14.3).
            "route_stream_written" => Ok(json!({"bytes": self.worker_end().written()})),
            // R-47 item 3, Core OU-3b: the session's output fills the stream and one frame more.
            "route_fill" => self.route_fill(),
            _ => Err(format!("unsupported_control: {op}")),
        }
    }

    fn has_control(&self, op: &str) -> bool {
        ROUTE_CONTROLS.contains(&op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::SchedulerHandle;
    use std::time::Instant;

    fn route(capacity: usize) -> (TestkitRoute, StreamEnd) {
        let scheduler = SchedulerHandle::with_seed(0);
        let (worker, client) = crate::net::stream_pair(&scheduler, capacity);
        let workers = Workers::new(scheduler, Instant::now());
        (
            TestkitRoute::new(client, workers, RouteId(1), RouteEnds::default()),
            worker,
        )
    }

    fn deadline() -> Deadline {
        Deadline::after(None)
    }

    /// R-47 item 3: byte k of a fill is 0x61 + (k mod 26). `route_fill` needs the session's worker, which only a route that
    /// `attach_stream` built has; the harness tests prove the fill itself.
    #[test]
    fn route_fill_is_the_pattern_and_needs_the_session_worker() {
        assert_eq!(fill_pattern(28), b"abcdefghijklmnopqrstuvwxyzab");
        // C: the free room of the worker's queue, limited by route_accept, and 0 while gated.
        let (mut client, mut worker) = route(16);
        assert_eq!(worker.write(b"abcde").unwrap(), 5);
        assert_eq!(client.worker_end().room(), 11, "16 less the 5 queued");
        client.worker_end().accept_at_most(3);
        assert_eq!(client.worker_end().room(), 3);
        client.worker_end().accept_at_most(20);
        assert_eq!(client.worker_end().room(), 11);
        client.worker_end().gate(true);
        assert_eq!(client.worker_end().room(), 0);
        let (client, _worker) = route(16);
        let mut bare = client;
        assert!(bare
            .control("route_fill", &json!({}))
            .unwrap_err()
            .contains("attach_stream"));
        let (client, _worker) = route(16);
        let mut no_row = client.with_fill(RouteFill {
            session: None,
            max_frame_bytes: 8,
        });
        assert!(no_row
            .control("route_fill", &json!({}))
            .unwrap_err()
            .contains("no session row"));
    }

    /// The route stream holds a socket buffer each way: 64 KiB that no one reads yet are taken whole.
    #[test]
    fn a_route_stream_holds_64_kib_each_way() {
        let scheduler = SchedulerHandle::with_seed(0);
        let (mut worker, _client) = crate::net::stream_pair(&scheduler, ROUTE_STREAM_BYTES);
        assert_eq!(worker.write(&vec![7u8; 64 * 1024 + 1]).unwrap(), 64 * 1024);
    }

    /// The controls of the route's stream act on the worker's end, which the client no longer holds (DP-2), and
    /// `client_close` closes the client's end. The testkit lists exactly the controls that `control` runs.
    #[test]
    fn the_route_controls_act_on_the_worker_end_and_client_close_ends_the_client() {
        let (mut client, mut worker) = route(16);
        let control = |client: &mut TestkitRoute, op: &str, args: Value| {
            let mut args = args;
            args["op"] = json!(op);
            client.control(op, &args)
        };
        for &op in ROUTE_CONTROLS {
            assert!(client.has_control(op), "{op}");
        }
        assert!(!client.has_control("pty_input"));
        assert_eq!(
            control(&mut client, "pty_input", json!({})).unwrap_err(),
            "unsupported_control: pty_input"
        );
        assert_eq!(
            control(&mut client, "route_stream_holders", json!({})).unwrap(),
            json!({"holders": 1, "host_bytes": 0})
        );
        // route_gate: on by default; off ends a route_accept limit too.
        control(&mut client, "route_gate", json!({})).unwrap();
        assert!(worker.write(b"a").is_err(), "a gated stream takes no byte");
        control(&mut client, "route_gate", json!({"on": false})).unwrap();
        assert_eq!(worker.write(b"a").unwrap(), 1);
        assert!(control(&mut client, "route_gate", json!({"on": 1})).is_err());
        control(&mut client, "route_accept", json!({"bytes": 2})).unwrap();
        assert_eq!(
            worker.write(b"bcd").unwrap(),
            2,
            "at most `bytes` more bytes"
        );
        assert!(worker.write(b"d").is_err());
        control(&mut client, "route_gate", json!({"on": false})).unwrap();
        assert!(control(&mut client, "route_accept", json!({})).is_err());
        assert_eq!(
            control(&mut client, "route_stream_written", json!({})).unwrap(),
            json!({"bytes": 3})
        );
        // fail_writes: the next write fails, once; `on: false` lifts a failure that did not happen yet.
        control(&mut client, "fail_writes", json!({})).unwrap();
        assert_eq!(
            worker.write(b"e").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(worker.write(b"e").unwrap(), 1);
        control(&mut client, "fail_writes", json!({})).unwrap();
        control(&mut client, "fail_writes", json!({"on": false})).unwrap();
        assert_eq!(worker.write(b"f").unwrap(), 1);
        // client_close: the worker reads the queued client bytes, then the end; the client writes no more.
        client.write(b"x");
        control(&mut client, "client_close", json!({})).unwrap();
        client.write(b"y");
        let mut buf = [0u8; 8];
        assert_eq!(worker.read(&mut buf).unwrap(), 1);
        assert_eq!(&buf[..1], b"x");
        assert_eq!(worker.read(&mut buf).unwrap(), 0, "the client's close");
        assert_eq!(client.unsent(), 0);
    }

    /// `drop_transport`: the stream is reset, so both ends fail their next calls (OU-5).
    #[test]
    fn drop_transport_resets_the_stream() {
        let (mut client, mut worker) = route(16);
        client.control("drop_transport", &json!({})).unwrap();
        assert_eq!(
            worker.write(b"a").unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        assert_eq!(client.read(8, &deadline()), RouteRead::Eof { ended: None });
    }

    /// A2-3: a failed reason records its route-ended cause; a healthy reason has its `route_closed` frame and no cause.
    #[test]
    fn a_failed_close_records_its_cause_and_a_healthy_close_records_none() {
        let ends = RouteEnds::default();
        let failed = [
            (RouteCloseReason::HandoffFailed, "handoff_failed"),
            (RouteCloseReason::WriteFailed, "write_failed"),
            (RouteCloseReason::StallTimeout, "stalled"),
            (RouteCloseReason::SessionLost, "session_lost"),
            (RouteCloseReason::PeerClosed, "transport_lost"),
        ];
        for (n, (reason, cause)) in failed.into_iter().enumerate() {
            let route = RouteId(n as u64 + 1);
            ends.closed(route, reason);
            assert_eq!(ends.ended(route).as_deref(), Some(cause), "{reason:?}");
        }
        let healthy = [
            RouteCloseReason::Detached,
            RouteCloseReason::Replaced,
            RouteCloseReason::Revoked,
            RouteCloseReason::SessionRemoved,
            RouteCloseReason::SnapshotTooLarge,
            RouteCloseReason::BadFrame {
                code: botster_route_codec::prelude::ProtocolErrorCode::FrameTooLarge,
            },
        ];
        for (n, reason) in healthy.into_iter().enumerate() {
            let route = RouteId(n as u64 + 100);
            ends.closed(route, reason);
            assert_eq!(ends.ended(route), None, "{reason:?}");
        }
        assert_eq!(
            ends.ended(RouteId(999)),
            None,
            "a route that Core did not close"
        );
    }

    /// A read returns the bytes that wait, `Empty` when none wait, and the end of file when the worker closes or the stream
    /// fails.
    #[test]
    fn a_read_returns_bytes_then_empty_then_the_end() {
        let (mut client, mut worker) = route(16);
        assert_eq!(worker.write(b"xy").unwrap(), 2);
        let mut got = Vec::new();
        loop {
            match client.read(8, &deadline()) {
                RouteRead::Bytes(bytes) => got.extend(bytes),
                RouteRead::Empty => break,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(got, b"xy", "the bytes, then Empty");
        worker.close();
        assert_eq!(client.read(8, &deadline()), RouteRead::Eof { ended: None });
        let (mut client, mut worker) = route(16);
        worker.end().control().reset();
        assert_eq!(client.read(8, &deadline()), RouteRead::Eof { ended: None });
    }

    /// A write sends the bytes in order, and a full stream keeps the rest.
    #[test]
    fn a_write_sends_what_the_stream_takes() {
        let (mut client, mut worker) = route(4);
        let drain = |worker: &mut StreamEnd| {
            let mut got = Vec::new();
            let mut buf = [0u8; 8];
            while let Ok(n) = worker.read(&mut buf) {
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            got
        };
        client.write(b"abcdef");
        assert_eq!(client.unsent(), 2, "the full stream keeps two bytes");
        assert_eq!(drain(&mut worker), b"abcd", "the stream took four bytes");
        client.write(b"g");
        assert_eq!(
            drain(&mut worker),
            b"efg",
            "the kept bytes go first, then the new one"
        );
        assert_eq!(client.unsent(), 0);
        // A read sends the kept bytes too.
        client.write(b"hijkl");
        assert_eq!(drain(&mut worker), b"hijk");
        assert_eq!(client.read(8, &deadline()), RouteRead::Empty);
        assert_eq!(drain(&mut worker), b"l");
        // `Interrupted` writes again; it loses and repeats nothing.
        client
            .end
            .end()
            .control()
            .fail_next_write(io::ErrorKind::Interrupted);
        client.write(b"mn");
        assert_eq!(drain(&mut worker), b"mn");
        // Another error is the end of the stream for the client: nothing more is sent.
        client
            .end
            .end()
            .control()
            .fail_next_write(io::ErrorKind::BrokenPipe);
        client.write(b"o");
        client.write(b"p");
        assert_eq!(drain(&mut worker), b"", "a failed stream takes no byte");
        assert_eq!(client.unsent(), 0, "and keeps none");
    }
}

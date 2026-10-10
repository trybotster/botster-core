//! The client end of a stream route (`attach_stream`, P4a): the in-memory stream whose other end Core hands to the worker
//! (DP-2). A read runs the workers' ready work and no host pump, because the data plane is the worker's (Core TH-3).

use crate::net::StreamEnd;
use crate::worker::Workers;
use botster_conformance::Deadline;
use botster_core_conformance::RouteClient;
use botster_core_edges::RouteTransport;
use botster_hub_conformance::route::RouteRead;
use serde_json::Value;
use std::io;

/// The queue of each direction of a route stream: a socket buffer's size, so a large baseline is written in parts.
pub const ROUTE_STREAM_BYTES: usize = 64 * 1024;

pub struct TestkitRoute {
    end: StreamEnd,
    workers: Workers,
    /// Client bytes that the full stream did not take yet, in order. They go before any later bytes.
    unsent: Vec<u8>,
    /// The stream failed: the client's bytes have nowhere to go.
    failed: bool,
}

impl TestkitRoute {
    pub fn new(end: StreamEnd, workers: Workers) -> TestkitRoute {
        TestkitRoute {
            end,
            workers,
            unsent: Vec::new(),
            failed: false,
        }
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

    /// The client bytes that wait for room in the stream.
    pub fn unsent(&self) -> usize {
        self.unsent.len()
    }

    fn try_read(&mut self, max: usize) -> Option<RouteRead> {
        let mut buf = vec![0u8; max.max(1)];
        match self.end.read(&mut buf) {
            Ok(0) => Some(RouteRead::Eof { ended: None }),
            Ok(n) => {
                buf.truncate(n);
                Some(RouteRead::Bytes(buf))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(_) => Some(RouteRead::Eof { ended: None }),
        }
    }
}

impl RouteClient for TestkitRoute {
    /// Queues the bytes after any that wait, and writes what the stream takes. A full stream keeps the rest in order, and
    /// each later write or read sends it first (no byte is lost). No worker reads a route's input yet (P4a PR1).
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

    fn control(&mut self, op: &str, _args: &Value) -> Result<Value, String> {
        Err(format!("unsupported_control: {op}"))
    }

    fn has_control(&self, _op: &str) -> bool {
        false
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
        (TestkitRoute::new(client, workers), worker)
    }

    fn deadline() -> Deadline {
        Deadline::after(None)
    }

    /// The route stream holds a socket buffer each way: 64 KiB that no one reads yet are taken whole.
    #[test]
    fn a_route_stream_holds_64_kib_each_way() {
        let scheduler = SchedulerHandle::with_seed(0);
        let (mut worker, _client) = crate::net::stream_pair(&scheduler, ROUTE_STREAM_BYTES);
        assert_eq!(worker.write(&vec![7u8; 64 * 1024 + 1]).unwrap(), 64 * 1024);
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

    #[test]
    fn a_testkit_route_has_no_controls() {
        let (mut client, _worker) = route(4);
        assert!(!client.has_control("route_gate"));
        assert_eq!(
            client.control("route_gate", &Value::Null),
            Err("unsupported_control: route_gate".into())
        );
    }
}

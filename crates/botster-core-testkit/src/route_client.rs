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
}

impl TestkitRoute {
    pub fn new(end: StreamEnd, workers: Workers) -> TestkitRoute {
        TestkitRoute { end, workers }
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
    /// Writes every byte: while the stream is full, the workers run and read from it.
    fn write(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        while !rest.is_empty() {
            match self.end.write(rest) {
                Ok(n) => rest = &rest[n..],
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.workers.run_ready();
                    match self.end.write(rest) {
                        Ok(n) => rest = &rest[n..],
                        // No worker reads the stream now (or it failed): the rest stays unwritten, as a full socket keeps it.
                        Err(_) => return,
                    }
                }
                Err(_) => return,
            }
        }
    }

    /// The bytes that wait, or the bytes that the workers' ready work writes; `Empty` when that writes none.
    fn read(&mut self, max: usize, _deadline: &Deadline) -> RouteRead {
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

//! The subject of the route driver: one session worker with routes.

use botster_route_codec::prelude::{FrameBounds, ToClient};
use serde_json::Value;

/// How the binding delimits route frames (Codec TS-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// `[u32 big-endian length][frame]` (Codec TB-L2).
    Stream,
    /// DataChannel messages `[u8 flags][chunk]` (Codec TR-3). One write is one message.
    Chunked,
}

/// What a read of a route gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRead {
    Bytes(Vec<u8>),
    /// The reader makes no progress.
    Blocked,
    /// The subject's scheduler kept reporting progress and the read gave up at its execution bound: the run proved nothing (design 6.1), so the driver
    /// reports `inconclusive`, never an idle route.
    Exhausted,
    /// Nothing to read until a new stimulus arrives.
    Empty,
    /// The route ended. `ended` is the cause that the worker reports for a failed route (Codec TS-9).
    Eof {
        ended: Option<String>,
    },
}

/// A terminal model of a client: it loads a baseline and applies later frames (Codec TS-4).
pub trait ClientModel {
    fn frame(&mut self, frame: &ToClient, max_frame: u64);
    fn canonical(&self) -> Value;
}

/// A session worker that the codec transcripts drive (design 7.1, subject `worker`).
///
/// A route is a `u32` handle that the subject gives at attach.
pub trait RouteSubject {
    /// True for a fake. A `structural` transcript is only checked structurally on it.
    fn is_fake(&self) -> bool {
        false
    }
    fn has_feature(&self, name: &str) -> bool;
    fn has_control(&self, op: &str) -> bool;
    /// Applies the `worker` part of a transcript's `setup`: limits, features and Hub policy.
    fn configure(&mut self, worker: &Value) -> Result<(), String>;
    /// The Hub hands a connected route to the worker (Codec TS-1).
    fn attach(&mut self, spec: &Value) -> Result<u32, String>;
    fn framing(&self) -> Framing;
    fn bounds(&self) -> FrameBounds;
    /// The chunk bound of a route on a chunked binding (Codec TR-3).
    fn max_chunk_bytes(&self) -> usize;
    /// The client writes bytes to the route. On a chunked route, one write is one message.
    fn write(&mut self, route: u32, bytes: &[u8]);
    /// Reads at most `max` bytes. A subject that can block must give up at the deadline and return `RouteRead::Empty`.
    fn read(
        &mut self,
        route: u32,
        max: usize,
        deadline: &botster_conformance::Deadline,
    ) -> RouteRead;
    /// Runs a test control (design 6.3). The control `pty_input` gives `{ bytes, segments?: [..] }`: `bytes` is the literal log of
    /// the bytes that reached the PTY, in order. The route driver reads only `bytes` and ignores `segments`, which a subject may
    /// omit: a subject never needs to know which op caused a write (steward ruling R-73). `Err` means that the subject has no such
    /// control, or its arguments are wrong.
    fn control(&mut self, op: &str, args: &Value, route: Option<u32>) -> Result<Value, String>;
    /// A client model for the terminal format, or `None` when the runner has none for it.
    fn client_model(&self, terminal_format: &str) -> Option<Box<dyn ClientModel>>;
}

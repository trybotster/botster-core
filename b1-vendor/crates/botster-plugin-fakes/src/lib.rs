//! The plugin-side FakeHub and FakePlugin (HP J4e with A1 and A2; ledger prefix `hp`; design 7.5, steward ruling R-1.3).
//!
//! Both are scripted, sans-IO and deterministic: bytes in, bytes out, an injected clock and a ChaCha8 seed. They use the frame codec of `botster-plugin-contract`.
//! They hold the protocol state of HP section 19 (handshake, registration, epochs, lanes, invocation, deadlines and `Cancel`, limits, `Fault`, channel close) and no
//! semantic Hub behavior that no clause states. `FakeEndpoint` is the one interface the runner adapts, so a fake never depends on the runner.

mod events;
mod hub;
mod limits;
mod link;
mod plugin;

pub use hub::{FakeHub, CONTROLS as HUB_CONTROLS};
pub use limits::test_limits;
pub use link::{lane_by_name, lane_name, Backpressured, Choice};
pub use plugin::{FakePlugin, CONTROLS as PLUGIN_CONTROLS};

use serde_json::Value;

/// A control that the endpoint does not implement (design 6.3). The runner reports it as `unsupported_control`, never as a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

/// One end of a plugin channel, as the runner drives it.
pub trait FakeEndpoint {
    /// Applies the transcript `setup`: limits, the script or fixture, and for a Hub the plugin to load.
    fn configure(&mut self, setup: &Value) -> Result<(), String>;
    /// The peer writes bytes on connection `conn`. A write that a full held lane refuses is `Backpressured` (HP MD-3).
    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured>;
    /// The next write for the peer on `conn`, if any.
    fn read(&mut self, conn: &str) -> Option<Vec<u8>>;
    /// The cause that the connection ended, if it did.
    fn closed(&self, conn: &str) -> Option<&'static str>;
    /// The peer ends the connection.
    fn close_channel(&mut self, conn: &str);
    /// Moves the injected clock (HP WF-5: no wall clock).
    fn advance_clock(&mut self, ms: u64);
    /// A test control (design 6.3).
    fn control(&mut self, op: &Value) -> Result<Value, Unsupported>;
    /// The names of the controls that this endpoint implements.
    fn controls(&self) -> &'static [&'static str];
    /// The names of the connections that this endpoint has now.
    fn conns(&self) -> Vec<String>;
}

//! The subject of the HP runner (design 6.5): one channel endpoint that the runner writes bytes to and reads bytes from.
//!
//! For a `plugin_hub` transcript the subject is a Hub endpoint and the runner plays the plugin. For a `plugin_host` transcript the subject is a plugin host
//! and the runner plays the Hub. The subject also implements the test controls (design 6.3) that its transcripts name.

use serde_json::Value;

/// A write that the subject refuses without blocking: the lane is full (HP MD-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backpressured;

/// A control that the subject does not implement. The runner reports it as `unsupported_control`, never as a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

/// One subject under test. A Hub subject has a connection per generation (`main`, and `cand` during a reload); a host subject has `main` only.
pub trait HpSubject {
    /// True for a fake. A `structural` transcript is only checked structurally on it.
    fn is_fake(&self) -> bool {
        false
    }
    /// True when the subject offers the feature. A transcript that needs one the subject lacks is `not_applicable`.
    fn has_feature(&self, name: &str) -> bool;
    /// True when the subject implements the control.
    fn has_control(&self, op: &str) -> bool;
    /// Applies the transcript `setup`: limits, the script (a Hub) or the fixture body (a host).
    fn configure(&mut self, setup: &Value) -> Result<(), String>;
    /// The runner writes bytes. One call is one write (design 5.4).
    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured>;
    /// The next write of the subject on `conn`, if it has one ready.
    fn read(&mut self, conn: &str) -> Option<Vec<u8>>;
    /// The names of the connections that the subject has now.
    fn conns(&self) -> Vec<String>;
    /// The cause that the connection ended, if it did.
    fn closed(&self, conn: &str) -> Option<String>;
    /// The runner ends the connection.
    fn close_channel(&mut self, conn: &str);
    /// Moves the subject's clock (HP WF-5, design 6.3).
    fn advance_clock(&mut self, ms: u64);
    /// A test control. `Ok` carries what the control observed.
    fn control(&mut self, op: &Value) -> Result<Value, Unsupported>;
}

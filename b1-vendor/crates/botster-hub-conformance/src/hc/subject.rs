//! The Hub subject of the HC runner (HC A5-1; design 6.4): the harness trait that a Phase 1 Hub implements, so the suite runs against the
//! real Hub in-process.
//!
//! The harness builds the Hub from injected parts (a clock, storage, a process and program backend, a plugin loader, a transport, and a seeded
//! scheduler, BUILD.md "HARD Phase 1 requirement"). It never adds a test branch to production code. The edges that [`HubSubject::connect`],
//! [`HubSubject::write`], [`HubSubject::read`], [`HubSubject::advance_clock`] and [`HubSubject::pump`] move are those injected edges.
//! Every control is a harness obligation, with the clause that needs it, in `docs/hub-subject-controls.md`.
//!
//! The in-memory transport that a harness offers is test plumbing: it is never a binding. It never appears in `welcome.features` as
//! `binding:*`, and a run over it never satisfies a binding clause (HC A5-1, TB-0).

use botster_conformance::Deadline;
use serde_json::Value;

/// How a step asks for a connection (HC TB-0, HS-7, TB-L3).
#[derive(Debug, Clone, Default)]
pub struct ConnectSpec {
    /// The binding to use. `None` is the harness's in-memory transport (HC A5-1), which is not a binding.
    pub binding: Option<String>,
    /// The peer uid that the binding authenticates (HC TB-L3). `None` is the Hub's own uid.
    pub uid: Option<u32>,
    pub credential: Option<String>,
}

/// What a read of a connection gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubRead {
    Bytes(Vec<u8>),
    /// Nothing is ready now. The driver calls [`HubSubject::pump`] before it gives up.
    Empty,
    /// The connection ended (the close signal, TB-0 item 4).
    Eof,
}

/// What one [`HubSubject::pump`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pump {
    /// The Hub's scheduler ran one step. Reading again may give a frame.
    Progress,
    /// The scheduler has nothing to run until a new stimulus arrives.
    Quiet,
}

/// A Hub under test. A connection is a `u32` handle that the subject gives.
pub trait HubSubject {
    fn has_feature(&self, name: &str) -> bool;
    fn has_control(&self, op: &str) -> bool;
    /// Applies the `hub` part of the transcript `setup` (limits, features, fixture rows, plugins), by building the Hub from its edges.
    fn configure(&mut self, hub: &Value) -> Result<(), String>;
    /// Starts the Hub (HC HS-0). The ready descriptor is read with the control `ready_line`.
    fn start(&mut self) -> Result<(), String>;
    /// A new connection. `Ok(None)` means that nothing listens (HC HS-0).
    fn connect(&mut self, spec: &ConnectSpec) -> Result<Option<u32>, String>;
    /// The client writes bytes. One call is one write (design 5.4).
    fn write(&mut self, conn: u32, bytes: &[u8]);
    /// Reads at most `max` bytes. It does not wait for the Hub: the driver pumps. A subject that blocks gives up at the deadline and returns `Empty`.
    fn read(&mut self, conn: u32, max: usize, deadline: &Deadline) -> HubRead;
    /// The client closes its end.
    fn close(&mut self, conn: u32);
    /// The injected clock moves (design 6.2). A subject that cannot inject its clock returns `Err`, and the step is `unsupported_control`.
    fn advance_clock(&mut self, ms: u64) -> Result<(), String>;
    /// Runs the Hub's injected scheduler one step (HC A5-4: the seed picks the producer variations inside the Hub).
    fn pump(&mut self) -> Pump;
    /// Runs a test control (design 6.3). `conn` arguments are handles. `Err` is a control failure, never a pass.
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String>;
}

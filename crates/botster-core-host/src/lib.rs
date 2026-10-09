//! The `HostEngine`: the sans-IO state machine of the Botster Core host (plan 2.2).
//!
//! No compatibility promise; use `botster-core`.
//!
//! The engine reads no clock, draws no random number, starts no thread and touches no file. A driver connects it to the
//! edges: it passes the time and every edge result as an input, and it performs the engine's actions (plan 2.1).

pub mod admit;
pub mod adopt;
pub mod driver;
pub mod engine;
pub mod flow;
pub mod flows;
pub mod inbound;
pub mod io;
pub mod queue;
pub mod run;
pub mod session;

pub use engine::{EngineConfig, HostEngine};
pub use io::{Action, Input, LinkId, Ticket, Work};

#[cfg(test)]
mod tests;

//! The `Machine` trait, the edge traits and the scheduler policy of Core (plan 2.1 to 2.4).
//!
//! No compatibility promise; use `botster-core`.
//!
//! A machine is sans-IO: it takes inputs and produces actions. A driver connects it to the world through the edges of
//! this crate. This crate holds signatures only, except for the production [`scheduler::Production`] policy. A real
//! edge lives in a driver or in `botster-core-sys`, and a test edge lives in the testkit.

pub mod edges;
pub mod machine;
pub mod scheduler;

pub use edges::{
    Clock, Entropy, FileSink, Link, Process, Program, RouteTransport, ServiceLane, Storage, Wake,
};
pub use machine::Machine;
pub use scheduler::Scheduler;

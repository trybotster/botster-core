//! The Core testkit (Core A5-1, plan section 4): a small discrete-event simulation over sans-IO machines.
//!
//! Rust-only: not in the facade `botster-core` and not in any FFI crate. It may use the threads, clocks and randomness that
//! the machine crates may not (plan 2.3c), and it adds no test branch to a production crate (BUILD.md testing rule 8).

pub mod core;
pub mod entropy;
pub mod harness;
pub mod net;
pub mod program;
pub mod scheduler;
pub mod sim;
pub mod status;
pub mod wake;
pub mod worker;

pub use entropy::SeededEntropy;
pub use harness::TestkitHarness;
pub use net::{link_pair, stream_pair, Descriptor, End, Interest, LinkEnd, Readiness, StreamEnd};
pub use program::{ProgramError, ScriptedProgram};
pub use scheduler::{SchedulerHandle, SeededScheduler};
pub use sim::{Binding, Handled, Livelock, MachineNode, Node, NodeId, Sim, TraceEntry};
pub use wake::SimWake;
pub use worker::{TestkitCore, WorkerSpawner, Workers};

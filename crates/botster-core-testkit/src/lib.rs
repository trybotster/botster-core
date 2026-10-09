//! The Core testkit (Core A5-1, plan section 4): a small discrete-event simulation over sans-IO machines.
//!
//! Rust-only: not in the facade `botster-core` and not in any FFI crate. It may use the threads, clocks and randomness that
//! the machine crates may not (plan 2.3c), and it adds no test branch to a production crate (BUILD.md testing rule 8).

pub mod candidate;
pub mod controls;
pub mod core;
pub mod entropy;
pub mod every_cut;
pub mod harness;
pub mod net;
pub mod oracle;
pub mod process_controls;
pub mod process_group;
pub mod program;
pub mod pty_controls;
pub mod refusal;
pub mod resume_controls;
pub mod scheduler;
pub mod sim;
pub mod snapshot_controls;
pub mod start_controls;
pub mod statement_runs;
pub mod statements;
pub mod status;
pub mod wake;
pub mod wake_controls;
pub mod worker;

pub use entropy::SeededEntropy;
pub use harness::TestkitHarness;
pub use net::{link_pair, stream_pair, Descriptor, End, Interest, LinkEnd, Readiness, StreamEnd};
pub use program::{ProgramError, ScriptedProgram};
pub use refusal::{RefusalHandle, RefusalLayer, RefusalScript, ScriptError};
pub use scheduler::{SchedulerHandle, SeededScheduler};
pub use sim::{Binding, Handled, Livelock, MachineNode, Node, NodeId, Sim, TraceEntry};
pub use wake::SimWake;
pub use worker::{TestkitCore, WorkerSpawner, Workers};

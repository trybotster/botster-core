//! The Core testkit (Core A5-1, plan section 4): a small discrete-event simulation over sans-IO machines.
//!
//! Rust-only: not in the facade `botster-core` and not in any FFI crate. It may use the threads, clocks and randomness that
//! the machine crates may not (plan 2.3c), and it adds no test branch to a production crate (BUILD.md testing rule 8).

pub mod adopt_controls;
pub mod alloc_controls;
pub use botster_test_process::built_in;
pub mod candidate;
// Only the real harness (the `slow` feature) composes the capture layer; its tests run in the default tier too.
#[cfg_attr(not(feature = "slow"), allow(dead_code))]
mod capture_layer;
pub mod controls;
pub mod core;
pub mod crash;
pub mod edge_tap;
pub mod entropy;
pub mod every_cut;
pub mod facts_controls;
pub mod harness;
pub mod net;
pub mod op_holds;
pub mod oracle;
pub mod oracle_compare;
pub mod oracle_controls;
pub mod pending;
pub mod process_controls;
pub mod process_group;
pub mod program;
pub mod pty_controls;
#[cfg(feature = "slow")]
pub mod real;
pub mod refusal;
pub mod resume_controls;
pub mod route_client;
pub mod route_controls;
pub mod route_reports;
pub mod scheduler;
pub mod sim;
pub mod snapshot_controls;
pub mod stand_in;
pub mod start_controls;
pub mod statement_runs;
pub mod statements;
pub mod status;
pub mod stream_holders;
pub mod wake;
pub mod wake_controls;
pub mod worker;

pub use alloc_controls::PeakAllocator;
pub use entropy::SeededEntropy;
pub use harness::TestkitHarness;
pub use net::{link_pair, stream_pair, Descriptor, End, Interest, LinkEnd, Readiness, StreamEnd};
pub use program::{ProgramError, ScriptedProgram};
pub use refusal::{RefusalHandle, RefusalLayer, RefusalScript, ScriptError};
pub use scheduler::{SchedulerHandle, SeededScheduler};
pub use sim::{Binding, Handled, Livelock, MachineNode, Node, NodeId, Sim, TraceEntry};
pub use wake::SimWake;
pub use worker::{TestkitCore, WorkerSpawner, Workers};

/// The unit tests of the allocation controls measure through the allocator, as the conformance binary does.
#[cfg(test)]
#[global_allocator]
static ALLOCATOR: PeakAllocator = PeakAllocator;

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `botster_test_process::built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    botster_test_process::built_in::assert_built_here(env!("CARGO_PKG_NAME"), BUILT_IN);
}

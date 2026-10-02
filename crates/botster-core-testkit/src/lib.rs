//! The Core testkit (Core A5-1, plan section 4): a small discrete-event simulation over sans-IO machines.
//!
//! Rust-only: not in the facade `botster-core` and not in any FFI crate. It may use the threads, clocks and randomness that
//! the machine crates may not (plan 2.3c), and it adds no test branch to a production crate (BUILD.md testing rule 8).

pub mod entropy;
pub mod scheduler;

pub use entropy::SeededEntropy;
pub use scheduler::{SchedulerHandle, SeededScheduler};

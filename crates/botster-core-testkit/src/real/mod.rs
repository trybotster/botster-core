//! The real-process tier of the testkit (plan 4.2, `slow` feature): the guarded launch wrappers and `RealCoreHarness`.

pub mod guard;
pub mod harness;

pub use guard::{AnchorGuard, Finished};
pub use harness::RealCoreHarness;

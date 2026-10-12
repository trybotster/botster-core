//! The service guardian (Core SV-1, SV-5, SV-8, SV-9).
//!
//! This crate has no compatibility promise. Use `botster-core`.
//! The real driver and the testkit driver use the same sans-IO machine.

pub mod guardian;
mod link;
mod log;
pub mod wire;

pub use guardian::{Action, Guardian, GuardianConfig, Input};

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `botster_test_process::built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    botster_test_process::built_in::assert_built_here(env!("CARGO_PKG_NAME"), BUILT_IN);
}

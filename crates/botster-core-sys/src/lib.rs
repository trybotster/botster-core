//! The real operating-system edges of Core (plan 2.3, 2.6): the code that machines never contain.
//!
//! No compatibility promise; use `botster-core`. This crate is exempt from the machine-crate lints (plan 2.3c).
//! Modules are added by the packages that need them. The first is [`lock`].

pub mod entropy;
pub mod fd_pass;
pub mod lock;
pub mod payload;
pub mod process;
pub mod signal;
pub mod storage;

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `botster_test_process::built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    botster_test_process::built_in::assert_built_here(env!("CARGO_PKG_NAME"), BUILT_IN);
}

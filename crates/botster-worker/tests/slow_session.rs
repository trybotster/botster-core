//! Real session tests of the prebuilt worker binary.
#![cfg(feature = "slow")]

const DRIVER_OBSERVER: Option<&str> = None;

#[path = "common/session.rs"]
mod session;

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();

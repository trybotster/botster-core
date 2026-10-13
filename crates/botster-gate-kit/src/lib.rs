//! The gate code that Core's xtask and the gates of the Stage 2 repositories share (botster-hub, botster-tui; repo plan
//! section 5, lead 2026-10-12). Each module moved here from Core's xtask without a change of its rule; a repository passes
//! its own data (paths, lists). Its one Core dependency is botster-core-sys, which owns the signal rule (`bounded`).

pub mod bounded;
pub mod files;
pub mod high_tier;
pub mod junit;
pub mod mutant_logs;
pub mod mutant_outcomes;
pub mod taint;
pub mod unsafe_exception;

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `botster_test_process::built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
botster_test_process::built_in_test!();

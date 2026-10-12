//! Process and temp-root helpers for the test tooling (BUILD.md Testing rules 6 and 10).
//!
//! - [`census`]: which processes belong to a test run, and which of them it left behind (zombies count).
//! - [`process`]: stop a slow-tier driver in the order TERM, grace, KILL, reap, drain the pipes.
//! - [`tempdir`]: a short, canonicalized, unique temp root, short enough for a Unix socket path.
//!
//! The crate has no `unsafe` and no timer in production code: a wait is a channel `recv_timeout` on the real event.

pub mod census;
pub mod process;
pub mod tempdir;

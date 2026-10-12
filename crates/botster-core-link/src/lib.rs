//! # Worker-control signal for P3 (LC-5, AD-6)
//!
//! When the control link of a session is broken, the host signals the worker process only (never a group), and only after it
//! verified the worker by pid and start time. The signals are in `botster-core-edges`:
//! - **`GroupSignal::EndPayload` (`SIGUSR1`), at the stop:** ask your payload to stop (`SIGTERM` to its group). Keep no timer.
//! - **`GroupSignal::KillPayload` (`SIGUSR2`), at the end of `stop_grace`:** kill your payload's process group now, from the
//!   leader that you hold unreaped (`waitid` with `WNOWAIT`), so the group id cannot be reused. `stop_grace` is the Core
//!   library's deadline: Core sends this at its due `pump` (steward ruling R-97, follow-up).
//! - Keep running and keep serving the final model. The host never kills the worker for a stop.
//! - **Idempotent:** a worker that already ends its payload ignores a repeat of either signal.
//! - **After:** when the link returns, or at adoption, the worker reports the exit of the payload, as for any exit.
//!
//! The private control-link wire between the host and a worker or a guardian (plan section 3).
//!
//! No compatibility promise; use `botster-core`.
//!
//! This crate has the framing and the hello as types and codecs. It performs no I/O: a driver reads bytes and gives them to
//! a [`frame::FrameDecoder`]. The messages that follow the hello belong to the packages that own them.

pub mod frame;
pub mod hello;
pub mod launch;
pub mod msg;
pub mod proof;
pub mod route;

/// The compile-time manifest directory of this crate, for the build-root check of the test binaries (bug catalog B13,
/// `botster_test_process::built_in`).
#[doc(hidden)]
pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");

#[cfg(test)]
botster_test_process::built_in_test!();

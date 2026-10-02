//! # Worker-control signal for P3 (LC-5, AD-6)
//!
//! When the control link of a session is broken, the host sends `SIGUSR1` to the worker process only (never to a group), and
//! only after it verified the worker by pid and start time. The signal is `GroupSignal::EndPayload` in `botster-core-edges`.
//! - **Meaning:** end your payload. Ask it to stop, then kill its process group after `stop_grace`, from the leader that you
//!   hold unreaped (`waitid` with `WNOWAIT`), so the group id cannot be reused. Keep running and keep serving the final
//!   model. The host never kills the worker for a stop.
//! - **Idempotent:** the host repeats the signal (at the stop and at the grace). A worker that already ends its payload
//!   ignores it.
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

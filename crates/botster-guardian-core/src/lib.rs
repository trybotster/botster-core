//! The service guardian (Core SV-1, SV-5, SV-8, SV-9).
//!
//! This crate has no compatibility promise. Use `botster-core`.
//! The real driver and the testkit driver use the same sans-IO machine.

pub mod guardian;
mod link;
mod log;
pub mod wire;

pub use guardian::{Action, Guardian, GuardianConfig, Input};

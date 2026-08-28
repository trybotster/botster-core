//! ClientWorker-facing wake-queue integration.
//!
//! Queue mechanics live in [`crate::contract::terminal_wake`]. This module is
//! the engine integration surface named by the delivery plan.

pub use crate::contract::terminal_wake::{
    SessionWakeHandle, TerminalWakeBatch, TerminalWakeKind, TerminalWakeRoute, TerminalWakeSink,
    TerminalWakeSource, WakingTerminalAdapter, WAKE_QUEUE_CAPACITY,
};

//! Thread-safe control for a single-owner Core wake pump.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use botster_core::{TerminalWakeBatch, TerminalWakeInterrupt};
use thiserror::Error;

/// Thread-safe control handle for a host-owned wake pump loop.
///
/// This handle has no access to [`crate::CoreDaemon`]. The thread that creates
/// the daemon must keep exclusive ownership of it from construction through
/// shutdown.
#[derive(Clone, Debug)]
pub struct WakePumpControl {
    interrupt: TerminalWakeInterrupt,
    stop_requested: Arc<AtomicBool>,
}

impl WakePumpControl {
    pub(crate) fn new(interrupt: TerminalWakeInterrupt, stop_requested: Arc<AtomicBool>) -> Self {
        Self {
            interrupt,
            stop_requested,
        }
    }

    /// Interrupt a blocked pump wait without naming terminal work.
    pub fn interrupt(&self) {
        self.interrupt.interrupt();
    }

    /// Request an ordered pump stop and interrupt a blocked wait.
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
        self.interrupt.interrupt();
    }

    /// Report whether a stop was requested.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Acquire)
    }
}

/// Result from one [`crate::CoreDaemon::wait_pump`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WakePumpWait {
    /// Real adapter or ingress work was drained.
    Wakes(TerminalWakeBatch),
    /// A control interrupt ended the wait without real work.
    Interrupted,
    /// An ordered stop ended the pump loop.
    Stopped,
}

/// Wake pump lifecycle error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WakePumpError {
    /// A pump control was issued, but the pump loop did not observe its stop.
    #[error("wake pump stop was not observed before daemon shutdown")]
    StopNotObserved,
}

pub(crate) struct WakePumpState {
    pub(crate) interrupt: TerminalWakeInterrupt,
    pub(crate) stop_requested: Arc<AtomicBool>,
    pub(crate) stop_collision_consumed: AtomicBool,
    pub(crate) stop_observed: AtomicBool,
}

impl WakePumpState {
    pub(crate) fn new(interrupt: TerminalWakeInterrupt) -> Self {
        Self {
            interrupt,
            stop_requested: Arc::new(AtomicBool::new(false)),
            stop_collision_consumed: AtomicBool::new(false),
            stop_observed: AtomicBool::new(false),
        }
    }

    pub(crate) fn control(&self) -> WakePumpControl {
        WakePumpControl::new(self.interrupt.clone(), Arc::clone(&self.stop_requested))
    }
}

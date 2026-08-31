//! Bounded parent-to-worker control egress queue.
//!
//! One synchronized owner admits frames in a single critical section. The
//! writer thread pops under the lock, releases it, then writes. The lock is
//! never held across I/O.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Bounded parent-to-worker control egress queue, per session.
pub const WORKER_CONTROL_QUEUE_FRAMES: usize = 32;
/// Slots ordinary frames may never occupy: one cancel, one shutdown.
pub const WORKER_CONTROL_RESERVED_SLOTS: usize = 2;
/// Total deadline for one frame, stamped once.
pub const WORKER_CONTROL_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Slice ceiling. Each syscall uses `min(SLICE, remaining)`.
pub const WORKER_CONTROL_WRITE_SLICE: Duration = Duration::from_millis(250);
/// Teardown joins the writer thread under this bound, then detaches.
pub const WORKER_CONTROL_WRITER_JOIN_BOUND: Duration = Duration::from_secs(1);

/// Admission class for one control frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFrameClass {
    /// Ordinary post-spawn control traffic.
    Ordinary,
    /// Mode-gated cancel. Uses a reserved slot.
    Cancel,
    /// Shutdown. Uses a reserved slot and seals the queue.
    Terminal,
}

/// Why the writer stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlWriterError {
    /// The stamped total deadline expired.
    DeadlineExpired,
    /// A write syscall failed.
    WriteError(String),
    /// The peer closed the write half.
    PeerClosed,
}

/// Writer thread exit observed by the production tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlWriterOutcome {
    /// Writer is still running.
    Running,
    /// Writer failed. `consumed` is set after the first sweep.
    Failed {
        /// Failure cause.
        error: ControlWriterError,
        /// Whether the production tick has already swept owners.
        consumed: bool,
    },
    /// Writer stopped after a clean shutdown.
    Stopped,
}

/// Durable session control-plane state. Recovery is respawn, not time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlPlaneState {
    /// Control plane can admit attach and bind.
    Live,
    /// Control plane failed. Attach and bind are rejected until respawn.
    Failed(ControlWriterError),
}

/// Queue admission failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlQueueAdmitError {
    /// Ordinary capacity is full.
    ControlQueueFull,
    /// The queue is sealed.
    Sealed,
}

/// Current admission state for one ordinary control frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlAdmission {
    /// One ordinary frame can be admitted now.
    Ready,
    /// The bounded ordinary lane is full.
    Full,
    /// The queue has stopped accepting frames.
    Sealed,
}

struct ControlQueueState {
    frames: VecDeque<(ControlFrameClass, Vec<u8>)>,
    ordinary_len: usize,
    sealed: bool,
    #[cfg(test)]
    hold_pops: bool,
}

/// Synchronized control-queue owner.
#[derive(Clone)]
pub struct ControlQueue {
    state: Arc<Mutex<ControlQueueState>>,
    ready: Arc<Condvar>,
}

impl ControlQueue {
    /// Build an empty open queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ControlQueueState {
                frames: VecDeque::new(),
                ordinary_len: 0,
                sealed: false,
                #[cfg(test)]
                hold_pops: false,
            })),
            ready: Arc::new(Condvar::new()),
        }
    }

    /// Admit one frame in a single critical section.
    pub fn admit(
        &self,
        class: ControlFrameClass,
        frame: Vec<u8>,
    ) -> Result<(), ControlQueueAdmitError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.sealed {
            return Err(ControlQueueAdmitError::Sealed);
        }
        let total = state.frames.len();
        match class {
            ControlFrameClass::Ordinary => {
                if state.ordinary_len >= WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS
                {
                    return Err(ControlQueueAdmitError::ControlQueueFull);
                }
                state.ordinary_len += 1;
            }
            ControlFrameClass::Cancel | ControlFrameClass::Terminal => {
                if total >= WORKER_CONTROL_QUEUE_FRAMES {
                    return Err(ControlQueueAdmitError::ControlQueueFull);
                }
            }
        }
        if class == ControlFrameClass::Terminal {
            state.sealed = true;
        }
        state.frames.push_back((class, frame));
        self.ready.notify_one();
        Ok(())
    }

    /// Probe ordinary capacity under the same lock used by [`Self::admit`].
    #[must_use]
    pub(crate) fn probe_ordinary(&self) -> ControlAdmission {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.sealed {
            ControlAdmission::Sealed
        } else if state.ordinary_len >= WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS
        {
            ControlAdmission::Full
        } else {
            ControlAdmission::Ready
        }
    }

    /// Seal without enqueueing. Used after a truncated write.
    pub fn seal(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.sealed = true;
        self.ready.notify_all();
    }

    /// Pop one frame. Waits while empty and not sealed.
    pub fn pop(&self) -> Option<(ControlFrameClass, Vec<u8>)> {
        self.pop_with_capacity_transition()
            .map(|(class, frame, _)| (class, frame))
    }

    /// Pop one frame and report an ordinary-capacity transition.
    pub(crate) fn pop_with_capacity_transition(
        &self,
    ) -> Option<(ControlFrameClass, Vec<u8>, bool)> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            #[cfg(test)]
            if state.hold_pops {
                if state.sealed && state.frames.is_empty() {
                    return None;
                }
                state = self
                    .ready
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner());
                continue;
            }
            if let Some((class, frame)) = state.frames.pop_front() {
                let was_ordinary_full = state.ordinary_len
                    >= WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS;
                if class == ControlFrameClass::Ordinary {
                    state.ordinary_len = state.ordinary_len.saturating_sub(1);
                }
                return Some((
                    class,
                    frame,
                    class == ControlFrameClass::Ordinary && was_ordinary_full,
                ));
            }
            if state.sealed {
                return None;
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    /// Current length, for tests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .frames
            .len()
    }

    /// Whether the queue currently holds no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Stop the writer from popping so crate unit tests can fill the bound.
    #[cfg(test)]
    pub(crate) fn hold_pops(&self, hold: bool) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.hold_pops = hold;
        self.ready.notify_all();
    }

    /// Count queued frames by class for crate unit tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn class_counts(&self) -> (usize, usize, usize) {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let mut ordinary = 0;
        let mut cancel = 0;
        let mut terminal = 0;
        for (class, _) in &state.frames {
            match class {
                ControlFrameClass::Ordinary => ordinary += 1,
                ControlFrameClass::Cancel => cancel += 1,
                ControlFrameClass::Terminal => terminal += 1,
            }
        }
        (ordinary, cancel, terminal)
    }

    /// Whether the queue is sealed.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sealed
    }
}

impl Default for ControlQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared writer outcome slot.
#[derive(Clone)]
pub struct ControlWriterSlot {
    inner: Arc<Mutex<ControlWriterOutcome>>,
}

impl ControlWriterSlot {
    /// Start in [`ControlWriterOutcome::Running`].
    #[must_use]
    pub fn running() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ControlWriterOutcome::Running)),
        }
    }

    /// Record the writer exit.
    pub fn set(&self, outcome: ControlWriterOutcome) {
        *self.inner.lock().unwrap_or_else(|error| error.into_inner()) = outcome;
    }

    /// Snapshot the current outcome.
    #[must_use]
    pub fn get(&self) -> ControlWriterOutcome {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Mark a failed outcome consumed. Returns the error on first consume.
    pub fn consume_failure(&self) -> Option<ControlWriterError> {
        let mut outcome = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        match &mut *outcome {
            ControlWriterOutcome::Failed {
                error,
                consumed: consumed @ false,
            } => {
                *consumed = true;
                Some(error.clone())
            }
            _ => None,
        }
    }
}

/// Compute the next write-slice timeout clamped to the remaining total.
#[must_use]
pub fn write_slice_timeout(deadline: Instant, now: Instant) -> Option<Duration> {
    let remaining = deadline.checked_duration_since(now)?;
    if remaining.is_zero() {
        return None;
    }
    Some(remaining.min(WORKER_CONTROL_WRITE_SLICE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_capacity_is_thirty_and_reserved_slots_remain() {
        let queue = ControlQueue::new();
        assert_eq!(queue.probe_ordinary(), ControlAdmission::Ready);
        for _ in 0..30 {
            queue
                .admit(ControlFrameClass::Ordinary, vec![1])
                .expect("ordinary");
        }
        assert_eq!(
            queue.admit(ControlFrameClass::Ordinary, vec![2]),
            Err(ControlQueueAdmitError::ControlQueueFull)
        );
        assert_eq!(queue.probe_ordinary(), ControlAdmission::Full);
        queue
            .admit(ControlFrameClass::Cancel, vec![3])
            .expect("cancel reserved");
        queue
            .admit(ControlFrameClass::Terminal, vec![4])
            .expect("shutdown reserved");
        assert!(queue.is_sealed());
        assert_eq!(queue.probe_ordinary(), ControlAdmission::Sealed);
        assert_eq!(
            queue.admit(ControlFrameClass::Cancel, vec![5]),
            Err(ControlQueueAdmitError::Sealed)
        );
        assert_eq!(queue.len(), 32);
    }

    #[test]
    fn two_cancels_at_ordinary_capacity_consume_the_shutdown_slot() {
        let queue = ControlQueue::new();
        for _ in 0..30 {
            queue
                .admit(ControlFrameClass::Ordinary, vec![1])
                .expect("ordinary");
        }
        queue
            .admit(ControlFrameClass::Cancel, vec![2])
            .expect("first cancel");
        queue
            .admit(ControlFrameClass::Cancel, vec![3])
            .expect("second cancel occupies the shutdown slot");
        assert_eq!(
            queue.admit(ControlFrameClass::Terminal, vec![4]),
            Err(ControlQueueAdmitError::ControlQueueFull)
        );
    }
}

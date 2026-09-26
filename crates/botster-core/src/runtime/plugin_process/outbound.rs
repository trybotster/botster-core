//! Parent-to-child frame lanes (plan section 5.2).
//!
//! One writer thread sends frames in FIFO order. Each lane admits against
//! its own bound, so one lane can never consume another's room: a full
//! `Invoke` lane cannot refuse a `Cancel` or the `Shutdown`. A frame stays
//! counted until the writer has sent all of it. Kill never uses this queue.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, PoisonError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    /// `Bootstrap` and `Load`: one each, before anything else.
    Startup,
    /// One per in-flight invocation.
    Invoke,
    /// At most one per in-flight invocation.
    Cancel,
    /// Sent at most once.
    Shutdown,
}

const LANES: usize = 4;

impl Lane {
    fn index(self) -> usize {
        match self {
            Self::Startup => 0,
            Self::Invoke => 1,
            Self::Cancel => 2,
            Self::Shutdown => 3,
        }
    }
}

/// Why a frame was not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refused {
    /// The lane is at its bound. For the derived lanes this is a caller bug.
    Full,
    /// The writer has ended; the process is gone or going.
    Closed,
}

/// Per-lane count bounds, all derived from Hub-supplied numbers.
#[derive(Debug, Clone, Copy)]
pub(super) struct LaneBounds {
    counts: [usize; LANES],
}

impl LaneBounds {
    /// `max_in_flight_invokes` is the Hub's invocation concurrency for this
    /// plugin (the engine's executor width).
    pub(super) fn derived(max_in_flight_invokes: usize) -> Self {
        let mut counts = [0; LANES];
        counts[Lane::Startup.index()] = 2;
        counts[Lane::Invoke.index()] = max_in_flight_invokes;
        counts[Lane::Cancel.index()] = max_in_flight_invokes;
        counts[Lane::Shutdown.index()] = 1;
        Self { counts }
    }
}

struct Queue {
    frames: VecDeque<(Lane, Vec<u8>)>,
    /// Frames per lane that are queued or being written.
    held: [usize; LANES],
    /// Bytes per lane that are queued or being written.
    held_bytes: [usize; LANES],
    bounds: LaneBounds,
    closed: bool,
}

pub(super) struct Outbound {
    queue: Mutex<Queue>,
    ready: Condvar,
}

impl Outbound {
    pub(super) fn new(bounds: LaneBounds) -> Self {
        Self {
            queue: Mutex::new(Queue {
                frames: VecDeque::new(),
                held: [0; LANES],
                held_bytes: [0; LANES],
                bounds,
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        // Only this module's bookkeeping runs under the lock.
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Admit one encoded frame on `lane`. Never blocks.
    pub(super) fn push(&self, lane: Lane, frame: Vec<u8>) -> Result<(), Refused> {
        let mut queue = self.lock();
        if queue.closed {
            return Err(Refused::Closed);
        }
        let index = lane.index();
        if queue.held[index] >= queue.bounds.counts[index] {
            return Err(Refused::Full);
        }
        queue.held[index] += 1;
        queue.held_bytes[index] += frame.len();
        queue.frames.push_back((lane, frame));
        drop(queue);
        self.ready.notify_one();
        Ok(())
    }

    /// The next frame for the writer, or `None` once closed and empty. The
    /// frame stays counted until [`Self::written`].
    pub(super) fn next(&self) -> Option<(Lane, Vec<u8>)> {
        let mut queue = self.lock();
        loop {
            if let Some(frame) = queue.frames.pop_front() {
                return Some(frame);
            }
            if queue.closed {
                return None;
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The writer finished (or abandoned) a frame of `lane` with `len` bytes.
    pub(super) fn written(&self, lane: Lane, len: usize) {
        let mut queue = self.lock();
        let index = lane.index();
        queue.held[index] = queue.held[index].saturating_sub(1);
        queue.held_bytes[index] = queue.held_bytes[index].saturating_sub(len);
    }

    /// Refuse further frames, drop the queued ones, and end the writer.
    pub(super) fn close(&self) {
        let mut queue = self.lock();
        queue.closed = true;
        queue.frames.clear();
        queue.held = [0; LANES];
        queue.held_bytes = [0; LANES];
        drop(queue);
        self.ready.notify_all();
    }

    #[cfg(test)]
    pub(super) fn held(&self, lane: Lane) -> (usize, usize) {
        let queue = self.lock();
        (queue.held[lane.index()], queue.held_bytes[lane.index()])
    }
}

#[cfg(test)]
#[path = "outbound_test.rs"]
mod outbound_test;

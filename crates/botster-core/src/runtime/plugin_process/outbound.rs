//! Parent-to-child frame lanes (plan section 5.2).
//!
//! One writer thread sends frames in FIFO order. Each lane admits against
//! its own bound, so one lane can never consume another's room: a full
//! `Invoke` lane cannot refuse a `Cancel` or the `Shutdown`. A frame stays
//! counted until the writer has sent all of it. Kill never uses this queue.
//!
//! Credits are not queued as frames. Each returned delivery unit and each
//! released reply is one pending id; the parent's credit account keeps each
//! one spent until the writer takes it, so their count is bounded by the
//! granted units plus the reply credits. Ingress and log credit coalesce into
//! one pending total each, which keeps the position of its first return.
//!
//! Frames and credits share one push sequence, and the writer sends them in
//! that order. So nothing waits behind items pushed after it: a stream of
//! returned credit cannot hold back a queued `Cancel` or `Shutdown`, and a
//! credit returned before an `Invoke` reaches the child first.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, PoisonError};

use crate::session::RequestId;

use super::protocol::CreditFrame;

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

/// A queued frame and the invocation that owns it, if any.
pub(super) struct Queued {
    pub lane: Lane,
    pub frame: Vec<u8>,
    pub owner: Option<RequestId>,
}

/// What the writer sends next.
pub(super) enum Next {
    Frame(Queued),
    Credit(CreditFrame),
}

/// Credit owed to the child and not yet taken by the writer. Each entry
/// carries its push sequence number.
#[derive(Default)]
struct PendingCredits {
    /// `DeliveryPool`, `Delivery`, and `Reply` credits, in the order owed.
    ids: VecDeque<(u64, CreditFrame)>,
    /// Coalesced ingress bytes, at the position of the first return.
    ingress: Option<(u64, usize)>,
    /// Coalesced log count and bytes, at the position of the first return.
    log: Option<(u64, usize, usize)>,
}

impl PendingCredits {
    /// The earliest pending credit's sequence number.
    fn first(&self) -> Option<u64> {
        [
            self.ids.front().map(|(seq, _)| *seq),
            self.ingress.map(|(seq, _)| seq),
            self.log.map(|(seq, _, _)| seq),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Take the earliest pending credit.
    fn take(&mut self) -> Option<CreditFrame> {
        let first = self.first()?;
        if self.ids.front().is_some_and(|(seq, _)| *seq == first) {
            return self.ids.pop_front().map(|(_, credit)| credit);
        }
        if let Some((seq, bytes)) = self.ingress {
            if seq == first {
                self.ingress = None;
                return Some(CreditFrame::IngressBytes { bytes });
            }
        }
        let (_, count, bytes) = self.log.take()?;
        Some(CreditFrame::Log { count, bytes })
    }
}

struct Queue {
    /// The next push sequence number.
    seq: u64,
    frames: VecDeque<(u64, Queued)>,
    credits: PendingCredits,
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

impl Queue {
    fn next_seq(&mut self) -> u64 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }
}

impl Outbound {
    pub(super) fn new(bounds: LaneBounds) -> Self {
        Self {
            queue: Mutex::new(Queue {
                seq: 0,
                frames: VecDeque::new(),
                credits: PendingCredits::default(),
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

    /// Admit one encoded frame on `lane`, owned by `owner`. Never blocks.
    pub(super) fn push(
        &self,
        lane: Lane,
        frame: Vec<u8>,
        owner: Option<RequestId>,
    ) -> Result<(), Refused> {
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
        let seq = queue.next_seq();
        queue.frames.push_back((seq, Queued { lane, frame, owner }));
        drop(queue);
        self.ready.notify_one();
        Ok(())
    }

    /// Owe `credit` to the child. Never blocks and never refuses: every
    /// credit returns what the child already debited, so the pending set is
    /// bounded by the grants. After close, credit no longer matters.
    pub(super) fn push_credit(&self, credit: CreditFrame) {
        let mut queue = self.lock();
        if queue.closed {
            return;
        }
        let seq = queue.next_seq();
        let pending = &mut queue.credits;
        match credit {
            CreditFrame::IngressBytes { bytes } => {
                let (first, total) = pending.ingress.unwrap_or((seq, 0));
                pending.ingress = Some((first, total.saturating_add(bytes)));
            }
            CreditFrame::Log { count, bytes } => {
                let (first, total_count, total_bytes) = pending.log.unwrap_or((seq, 0, 0));
                pending.log = Some((
                    first,
                    total_count.saturating_add(count),
                    total_bytes.saturating_add(bytes),
                ));
            }
            CreditFrame::DeliveryPool { .. }
            | CreditFrame::Delivery { .. }
            | CreditFrame::Reply { .. } => pending.ids.push_back((seq, credit)),
        }
        drop(queue);
        self.ready.notify_one();
    }

    /// The next credit or frame for the writer, or `None` once closed. A
    /// frame stays counted until [`Self::written`].
    pub(super) fn next(&self) -> Option<Next> {
        let mut queue = self.lock();
        loop {
            if queue.closed && queue.frames.is_empty() {
                return None;
            }
            // Strict push order across frames and credits.
            let frame_first = queue.frames.front().map(|(seq, _)| *seq);
            match (frame_first, queue.credits.first()) {
                (Some(frame), Some(credit)) if credit < frame => {
                    if let Some(credit) = queue.credits.take() {
                        return Some(Next::Credit(credit));
                    }
                }
                (Some(_), _) => {
                    if let Some((_, frame)) = queue.frames.pop_front() {
                        return Some(Next::Frame(frame));
                    }
                }
                (None, Some(_)) => {
                    if let Some(credit) = queue.credits.take() {
                        return Some(Next::Credit(credit));
                    }
                }
                (None, None) => {}
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
        queue.credits = PendingCredits::default();
        queue.held = [0; LANES];
        queue.held_bytes = [0; LANES];
        drop(queue);
        self.ready.notify_all();
    }

    #[cfg(test)]
    pub(super) fn pending_credit_ids(&self) -> usize {
        self.lock().credits.ids.len()
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

//! The parent's table of invocations in flight to one child.
//!
//! A record lives until its outcome is settled **and** every frame it queued
//! (its `Invoke`, and its `Cancel` if any) has been written. Only then does
//! its slot return, so the invoke and cancel lanes can never overflow on a
//! legal interleaving, and a delayed `Cancel` can never meet a later
//! invocation that reuses the id: a new record with the same id is admitted
//! only after the old one has retired, and the writer keeps frame order.
//!
//! Lock order: the table, then an invocation's own state, then the outbound
//! queue or the supervisor's book. The supervisor never takes a table or
//! invocation lock while it holds its book.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use crate::actor::{
    PluginHandlerRef, PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult,
};
use crate::runtime::CancelTarget;
use crate::session::RequestId;

use super::supervisor::{DeadlineId, ExpiryGuard, ProcessKiller};
use super::PluginKillReason;

/// How an invocation ended, before its caller attaches its own ids.
pub(super) enum Outcome {
    /// The child's result, already checked against the invocation's identity.
    Result(PluginInvocationResult),
    /// A host-side failure: admission refused, the exit, or `stop`.
    Failed(PluginInvocationFailureKind, String),
}

/// One invocation: its identity, its encoded `Invoke` frame until admitted,
/// and the state its caller waits on. It is also the Core cancel target of
/// the invocation's token, and the guard of its cancel-grace deadline.
pub(super) struct Invocation {
    request_id: RequestId,
    handler: PluginHandlerRef,
    state: Mutex<CallState>,
    changed: Condvar,
}

#[derive(Default)]
pub(super) struct CallState {
    pub outcome: Option<Outcome>,
    /// The token was cancelled.
    pub cancelled: bool,
    /// `stop` failed the caller early; the child's result is dropped.
    pub abandoned: bool,
    /// The record is in the table (its `Invoke` was queued).
    pub admitted: bool,
    /// The encoded `Invoke`, held until admission.
    invoke_frame: Option<Vec<u8>>,
    /// The cancel grace was armed for this invocation.
    pub grace: Option<DeadlineId>,
}

impl Invocation {
    pub(super) fn new(request: &PluginInvocationRequest, invoke_frame: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            request_id: request.request_id.clone(),
            handler: request.handler.clone(),
            state: Mutex::new(CallState {
                invoke_frame: Some(invoke_frame),
                ..CallState::default()
            }),
            changed: Condvar::new(),
        })
    }

    pub(super) fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, CallState> {
        // Only this module's and the host's bookkeeping runs under the lock.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, CallState>) -> MutexGuard<'a, CallState> {
        self.changed
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Settle the outcome once. An abandoned invocation keeps its outcome.
    /// Returns the grace deadline to disarm, if one was armed.
    fn settle(&self, outcome: Outcome) -> Option<DeadlineId> {
        let mut state = self.lock();
        if state.outcome.is_none() && !state.abandoned {
            state.outcome = Some(outcome);
        }
        let grace = state.grace.take();
        drop(state);
        self.changed.notify_all();
        grace
    }

    fn wake(&self) {
        self.changed.notify_all();
    }
}

impl CancelTarget for Invocation {
    fn cancelled(&self) {
        self.lock().cancelled = true;
        self.changed.notify_all();
    }
}

/// The cancel-grace deadline of one invocation. On expiry it kills the
/// group only if, under the invocation's own lock, the invocation is still
/// unsettled; the kill happens under that lock, so a settlement cannot slip
/// between the check and the kill.
pub(super) struct GraceExpiry {
    pub invocation: Arc<Invocation>,
}

impl ExpiryGuard for GraceExpiry {
    fn expire(&self, killer: &ProcessKiller) {
        let state = self.invocation.lock();
        if state.outcome.is_none() && !state.abandoned && state.grace.is_some() {
            killer.kill(PluginKillReason::Deadline);
        }
    }
}

struct Record {
    invocation: Arc<Invocation>,
    /// Frames this record queued that the writer has not finished.
    frames_pending: usize,
    /// The outcome is settled (a result, `stop`, or the exit).
    retired: bool,
}

#[derive(Default)]
struct Table {
    records: HashMap<RequestId, Record>,
    /// Invocations waiting for room, in arrival order.
    waiting: VecDeque<Arc<Invocation>>,
    closed: bool,
}

/// Queue a frame for the writer on behalf of an invocation. Returns false if
/// the queue refused it (the writer has ended).
pub(super) type FrameSink<'a> = &'a dyn Fn(super::outbound::Lane, Vec<u8>, RequestId) -> bool;

pub(super) enum Admission {
    /// The `Invoke` is queued.
    Admitted,
    /// Waiting for room; the caller waits on its invocation.
    Waiting,
    /// No new invocation is accepted (stop or exit).
    Closed,
    /// The waiting queue is at its bound: more callers than the engine's
    /// executor width are invoking at once.
    Full,
}

pub(super) struct Invocations {
    table: Mutex<Table>,
    max_in_flight: usize,
}

impl Invocations {
    pub(super) fn new(max_in_flight: usize) -> Self {
        Self {
            table: Mutex::new(Table::default()),
            max_in_flight,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queue `invocation` behind the earlier waiters and admit every waiter
    /// that fits. The waiting queue holds at most `max_in_flight` callers:
    /// each engine executor waits for at most one invocation, so more means
    /// the caller exceeded the executor width, and it is refused at once.
    /// Each waiter holds one encoded frame of at most the frame bound.
    pub(super) fn admit(&self, invocation: &Arc<Invocation>, sink: FrameSink<'_>) -> Admission {
        let mut table = self.lock();
        if table.closed {
            return Admission::Closed;
        }
        if table.waiting.len() >= self.max_in_flight {
            return Admission::Full;
        }
        table.waiting.push_back(invocation.clone());
        admit_waiting(&mut table, self.max_in_flight, sink);
        if invocation.lock().admitted {
            Admission::Admitted
        } else {
            Admission::Waiting
        }
    }

    /// Withdraw a waiting invocation (its token was cancelled), then admit
    /// any waiter it was holding back. Returns false if it was admitted
    /// meanwhile.
    pub(super) fn withdraw(&self, invocation: &Arc<Invocation>, sink: FrameSink<'_>) -> bool {
        let mut table = self.lock();
        let Some(index) = table
            .waiting
            .iter()
            .position(|waiting| Arc::ptr_eq(waiting, invocation))
        else {
            return false;
        };
        table.waiting.remove(index);
        admit_waiting(&mut table, self.max_in_flight, sink);
        true
    }

    /// Queue `Cancel` for an admitted, unsettled invocation. The record then
    /// retires only after this frame is written too. Returns false if the
    /// invocation already settled, so no `Cancel` is needed.
    pub(super) fn cancel(
        &self,
        invocation: &Arc<Invocation>,
        frame: Vec<u8>,
        sink: FrameSink<'_>,
    ) -> bool {
        let mut table = self.lock();
        let Some(record) = table.records.get_mut(&invocation.request_id) else {
            return false;
        };
        if record.retired || !Arc::ptr_eq(&record.invocation, invocation) {
            return false;
        }
        if !sink(
            super::outbound::Lane::Cancel,
            frame,
            invocation.request_id.clone(),
        ) {
            return false;
        }
        record.frames_pending += 1;
        true
    }

    /// Settle the caller of one child result. The result must carry the
    /// request id and the exact handler of a live, unsettled record; the
    /// returned grace deadline must be disarmed by the caller.
    pub(super) fn settle_result(
        &self,
        result: PluginInvocationResult,
        sink: FrameSink<'_>,
    ) -> Result<Option<DeadlineId>, String> {
        let (request_id, handler) = match &result {
            PluginInvocationResult::Completed(success) => (&success.request_id, &success.handler),
            PluginInvocationResult::Failed(failure) => (&failure.request_id, &failure.handler),
        };
        let mut table = self.lock();
        let Some(record) = table.records.get_mut(request_id) else {
            return Err(format!(
                "result for request {request_id:?}, which is not in flight"
            ));
        };
        if record.retired {
            return Err(format!("a second result for request {request_id:?}"));
        }
        if handler != &record.invocation.handler {
            return Err(format!(
                "result for request {request_id:?} names handler {handler:?}, not {:?}",
                record.invocation.handler
            ));
        }
        record.retired = true;
        let invocation = record.invocation.clone();
        let request_id = request_id.clone();
        retire_if_done(&mut table, &request_id, self.max_in_flight, sink);
        drop(table);
        Ok(invocation.settle(Outcome::Result(result)))
    }

    /// Whether `request_id` is admitted and its child result has not
    /// arrived. `stop` leaves such a record live: the child still runs it.
    pub(super) fn is_live(&self, request_id: &RequestId) -> bool {
        self.lock()
            .records
            .get(request_id)
            .is_some_and(|record| !record.retired)
    }

    /// The writer finished a frame owned by `request_id`.
    pub(super) fn frame_written(&self, request_id: &RequestId, sink: FrameSink<'_>) {
        let mut table = self.lock();
        if let Some(record) = table.records.get_mut(request_id) {
            record.frames_pending = record.frames_pending.saturating_sub(1);
        }
        retire_if_done(&mut table, request_id, self.max_in_flight, sink);
    }

    /// `stop`: accept nothing new; fail every caller at once. Admitted
    /// records stay until their result or the exit, marked abandoned.
    pub(super) fn stop(&self) {
        let mut table = self.lock();
        table.closed = true;
        let waiting: Vec<_> = table.waiting.drain(..).collect();
        let admitted: Vec<_> = table
            .records
            .values()
            .map(|record| record.invocation.clone())
            .collect();
        drop(table);
        for invocation in waiting.iter().chain(admitted.iter()) {
            let mut state = invocation.lock();
            if state.outcome.is_none() {
                state.outcome = Some(Outcome::Failed(
                    PluginInvocationFailureKind::WorkerStopped,
                    "the plugin process was stopped".to_string(),
                ));
            }
            state.abandoned = true;
            drop(state);
            invocation.wake();
        }
    }

    /// The exit: settle every caller with the exit's failure and clear the
    /// table. Returns the grace deadlines to disarm.
    pub(super) fn exit(&self, kind: &PluginInvocationFailureKind, reason: &str) -> Vec<DeadlineId> {
        let invocations: Vec<_> = {
            let mut guard = self.lock();
            let table = &mut *guard;
            table.closed = true;
            table
                .waiting
                .drain(..)
                .chain(table.records.drain().map(|(_, record)| record.invocation))
                .collect()
        };
        invocations
            .into_iter()
            .filter_map(|invocation| {
                invocation.settle(Outcome::Failed(kind.clone(), reason.to_string()))
            })
            .collect()
    }
}

/// Insert `invocation`'s record and queue its `Invoke`. The caller holds the
/// table lock.
fn insert(table: &mut Table, invocation: &Arc<Invocation>, sink: FrameSink<'_>) {
    let frame = {
        let mut state = invocation.lock();
        state.admitted = true;
        state.invoke_frame.take()
    };
    let queued = frame.is_some_and(|frame| {
        sink(
            super::outbound::Lane::Invoke,
            frame,
            invocation.request_id.clone(),
        )
    });
    table.records.insert(
        invocation.request_id.clone(),
        Record {
            invocation: invocation.clone(),
            frames_pending: usize::from(queued),
            retired: false,
        },
    );
    invocation.wake();
}

/// Admit waiting invocations in arrival order while there is room. A waiter
/// whose id is still live is skipped, not allowed to hold back the waiters
/// behind it.
fn admit_waiting(table: &mut Table, max_in_flight: usize, sink: FrameSink<'_>) {
    let mut index = 0;
    while index < table.waiting.len() && table.records.len() < max_in_flight {
        let next = table.waiting[index].clone();
        if table.records.contains_key(&next.request_id) {
            index += 1;
            continue;
        }
        table.waiting.remove(index);
        insert(table, &next, sink);
    }
}

/// Remove a record once it is settled and all its frames are written, then
/// admit waiting invocations into the freed room.
fn retire_if_done(
    table: &mut Table,
    request_id: &RequestId,
    max_in_flight: usize,
    sink: FrameSink<'_>,
) {
    let done = table
        .records
        .get(request_id)
        .is_some_and(|record| record.retired && record.frames_pending == 0);
    if !done {
        return;
    }
    table.records.remove(request_id);
    admit_waiting(table, max_in_flight, sink);
}

#[cfg(test)]
#[path = "invocations_test.rs"]
mod invocations_test;

//! The one admission point of the worker for PTY input (Core AM-2), with the host's writes (IN-1 to IN-7) and the guards of
//! IN-10.
//!
//! - **One admission point.** Every PTY input goes through [`Worker::try_start`]: one transaction owns the PTY input at a
//!   time, from its first byte to its last, across short writes (AM-2: contiguous). The next starts only after the previous
//!   completed, failed or was cancelled. Host writes are admitted in `begin` order (the host's request order).
//! - **Start-time decisions.** The guards (IN-10) and the encoding are decided when a transaction starts, never when it is
//!   queued: a client frame or a model change before the start is seen; one after it is ordered after the write.
//! - **Exact counts (IN-2).** A short write, a cancel, a failure and the payload's end report the bytes really written. A
//!   write that is cancelled while a PTY write is out waits for that write's count before it completes (IN-6: "the
//!   cancelled op's completion reports exactly what happened").
//!
//! `Paste`, `Key`, `Mouse` and `Focus` are encoded by the terminal model at the start of their transaction (IN-8, IN-9:
//! libghostty). A reply of the model (a query's shadow reply, the model's own PTY writes) is a transaction of the same
//! admission point, in arrival order, that advances no input revision and completes no operation (EV-8).

use super::{PayloadState, Worker};
use botster_core_contract::prelude::*;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_route_codec::prelude::{Op, RefusalReason};
use std::collections::VecDeque;

/// A host write that waits for its turn at the admission point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostWrite {
    req: u64,
    payload: InputPayload,
    guard: Option<Guard>,
}

/// What waits at the admission point, in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pending {
    Host(HostWrite),
    /// A reply of the model to the program (EV-8 shadow reply, a model PTY write).
    Reply(Vec<u8>),
    /// A client's `bytes` or `text` frame (DP-5): fire-and-forget, in its receive order on the route.
    Route(RouteInput),
}

/// A client's input and the route that owns it: an exception goes back on that route, with the frame's op (DP-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteInput {
    route: RouteId,
    op: Option<Op>,
    bytes: Vec<u8>,
}

/// Who owns a transaction, and so where its outcome goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// A host write: its `Done` goes to the host.
    Host(u64),
    /// A reply of the model: it reports nothing.
    Reply,
    /// A client's input: only an exception goes back, on its route (DP-5).
    Route(RouteId, Option<Op>),
}

/// The transaction that owns the PTY input (AM-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Active {
    owner: Owner,
    /// Every byte that this transaction writes to the PTY.
    bytes: Vec<u8>,
    /// Where the caller's payload is in `bytes`: markers that the worker adds are outside it (IN-2 units).
    payload_start: usize,
    payload_len: usize,
    /// The bytes that the PTY took.
    written: usize,
    /// A PTY write is out, and its count has not come.
    in_flight: bool,
    /// A cancel came; the transaction ends at the next count.
    cancelled: bool,
    /// An adoption fenced its host: the transaction runs to its end (AM-2: contiguous) and reports nothing.
    retired: bool,
}

impl Active {
    fn payload_written(&self) -> u64 {
        let past_start = self.written.saturating_sub(self.payload_start);
        past_start.min(self.payload_len) as u64
    }

    fn result(&self, outcome: WriteOutcome, detail: &str) -> InputResult {
        InputResult {
            outcome,
            payload_bytes_written: self.payload_written(),
            pty_bytes_written: self.written as u64,
            detail: detail.to_string(),
        }
    }
}

/// The input state of a session instance.
#[derive(Debug, Default)]
pub(super) struct InputState {
    queue: VecDeque<Pending>,
    active: Option<Active>,
    /// The PTY took no byte at the last write: the next write waits for `PtyWritable`.
    blocked: bool,
    /// The input revisions of IN-10, per source class.
    host_rev: u64,
    client_rev: u64,
}

impl InputState {
    /// The input bytes of a route that the admission point holds: queued, and the rest of the route's write in progress
    /// (DP-5, the route's read allowance).
    pub(super) fn route_bytes(&self, route: RouteId) -> usize {
        let queued: usize = self
            .queue
            .iter()
            .filter_map(|p| match p {
                Pending::Route(input) if input.route == route => Some(input.bytes.len()),
                _ => None,
            })
            .sum();
        let active = self
            .active
            .as_ref()
            .filter(|a| matches!(a.owner, Owner::Route(r, _) if r == route))
            .map_or(0, |a| a.bytes.len() - a.written);
        queued + active
    }

    /// IN-4: a complete client input frame advances `input_rev{client}` on receipt.
    pub(super) fn client_received(&mut self) -> InputRev {
        self.client_rev += 1;
        InputRev(self.client_rev)
    }

    pub(super) fn input_revs(&self) -> InputRevs {
        InputRevs {
            client: InputRev(self.client_rev),
            host: InputRev(self.host_rev),
        }
    }
}

/// A write that writes zero bytes: a certain zero (IN-2).
fn not_written(reason: NotWrittenReason, detail: &str) -> InputResult {
    InputResult {
        outcome: WriteOutcome::NotWritten(reason),
        payload_bytes_written: 0,
        pty_bytes_written: 0,
        detail: detail.to_string(),
    }
}

impl Worker {
    /// IN-1: a host write joins the host's FIFO; it starts at its turn.
    pub(super) fn on_write_input(&mut self, req: u64, payload: InputPayload, guard: Option<Guard>) {
        self.input.queue.push_back(Pending::Host(HostWrite {
            req,
            payload,
            guard,
        }));
        self.try_start();
    }

    /// A reply of the model joins the admission point (EV-8: it is written as one contiguous transaction, in order). An
    /// empty reply writes nothing, so it does not join.
    pub(super) fn enqueue_reply(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        self.input.queue.push_back(Pending::Reply(bytes));
        self.try_start();
    }

    /// DP-5: a client's input joins the admission point in its receive order, with its route and op. An empty frame writes
    /// nothing, so it does not join.
    pub(super) fn enqueue_route_input(&mut self, route: RouteId, op: Option<Op>, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        self.input
            .queue
            .push_back(Pending::Route(RouteInput { route, op, bytes }));
        self.try_start();
    }

    /// IN-6: a cancel of a queued write ends it with exact zero; of the active one, at its next count; of a write that already
    /// ended, nothing (the host reports `TooLate`).
    pub(super) fn on_cancel(&mut self, req: u64) {
        if let Some(at) = self
            .input
            .queue
            .iter()
            .position(|p| matches!(p, Pending::Host(w) if w.req == req))
        {
            self.input.queue.remove(at);
            self.complete_write(
                req,
                InputResult {
                    outcome: WriteOutcome::Cancelled,
                    payload_bytes_written: 0,
                    pty_bytes_written: 0,
                    detail: "cancelled before it started".into(),
                },
            );
            return;
        }
        let Some(active) = self
            .input
            .active
            .as_mut()
            .filter(|a| a.owner == Owner::Host(req) && !a.retired)
        else {
            return;
        };
        active.cancelled = true;
        if !active.in_flight {
            self.finish_active(WriteOutcome::Cancelled, "cancelled during the write");
        }
    }

    /// The admission point (AM-2): when no transaction owns the PTY input, the next one starts, with its guards and its
    /// encoding decided now.
    pub(super) fn try_start(&mut self) {
        // A write waits for the spawn's answer: it starts on a live payload or ends on a failed one.
        if self.payload == PayloadState::Spawning {
            return;
        }
        while self.input.active.is_none() {
            let Some(pending) = self.input.queue.pop_front() else {
                return;
            };
            let write = match pending {
                Pending::Reply(bytes) => {
                    // A reply goes only to a live payload. It advances no revision and completes no operation (EV-8).
                    if self.payload_takes_input() {
                        self.start_unowned(Owner::Reply, bytes);
                    }
                    continue;
                }
                Pending::Route(RouteInput { route, op, bytes }) => {
                    // A client's input advanced its revision on receipt (IN-4). On a payload that ended it is refused
                    // `session_ended`, with nothing written (DP-5).
                    if self.payload_takes_input() {
                        self.start_unowned(Owner::Route(route, op), bytes);
                    } else {
                        self.refuse(route, op, RefusalReason::SessionEnded, None);
                    }
                    continue;
                }
                Pending::Host(write) => write,
            };
            if let Err(result) = self.start_checks(&write) {
                self.complete_write(write.req, result);
                continue;
            }
            let encoded = match &write.payload {
                InputPayload::Bytes { bytes } => {
                    let len = bytes.0.len();
                    Ok(Some((bytes.0.clone(), 0, len)))
                }
                InputPayload::Text { text } => {
                    let len = text.len();
                    Ok(Some((text.as_bytes().to_vec(), 0, len)))
                }
                // IN-8, IN-9: the model encodes with its modes at this start.
                other => match self.model.as_ref() {
                    Some(model) => super::model::encode(model, other),
                    None => Ok(None),
                },
            };
            let (bytes, payload_start, payload_len) = match encoded {
                Ok(Some(encoded)) => encoded,
                Err(reason) => {
                    self.complete_write(
                        write.req,
                        not_written(reason, "the event has no write with the modes at its start"),
                    );
                    continue;
                }
                Ok(None) => {
                    self.report(&WorkerMsg::Done {
                        req: write.req,
                        result: OpResult::Err(CoreError::new(
                            ErrorCode::Internal,
                            "this worker cannot write this payload kind",
                        )),
                    });
                    continue;
                }
            };
            // Admitted: the host's revision advances, and the host learns it (IN-10, IN-4).
            self.input.host_rev += 1;
            let input_rev = InputRev(self.input.host_rev);
            self.report(&WorkerMsg::Observed {
                observation: Observation::HostInput { input_rev },
            });
            let empty = bytes.is_empty();
            self.input.active = Some(Active {
                owner: Owner::Host(write.req),
                bytes,
                payload_start,
                payload_len,
                written: 0,
                in_flight: false,
                cancelled: false,
                retired: false,
            });
            if empty {
                self.finish_active(WriteOutcome::Written, "");
            } else {
                self.write_more();
            }
        }
    }

    fn payload_takes_input(&self) -> bool {
        matches!(self.payload, PayloadState::Live(_)) && self.exit.is_none()
    }

    /// Starts a transaction that no host request owns: a reply, or a client's input.
    fn start_unowned(&mut self, owner: Owner, bytes: Vec<u8>) {
        let len = bytes.len();
        self.input.active = Some(Active {
            owner,
            bytes,
            payload_start: 0,
            payload_len: len,
            written: 0,
            in_flight: false,
            cancelled: false,
            retired: false,
        });
        self.write_more();
    }

    /// The decisions at the start of a transaction: the session can take input, and both guards pass (IN-10; a failed guard
    /// is `Stale` with exact zero).
    fn start_checks(&self, write: &HostWrite) -> Result<(), InputResult> {
        if !matches!(self.payload, PayloadState::Live(_)) || self.exit.is_some() {
            return Err(not_written(
                NotWrittenReason::SessionEnded,
                "the payload has ended",
            ));
        }
        if self.termed || self.killed || self.grace.is_some() {
            return Err(not_written(
                NotWrittenReason::Stopping,
                "the payload is being stopped",
            ));
        }
        if let Some(guard) = &write.guard {
            if let Some(input) = &guard.input {
                let current = match input.source_class {
                    SourceClass::Host => self.input.host_rev,
                    SourceClass::Client => self.input.client_rev,
                    // A later class of the non-exhaustive enum: this worker cannot tell, so it is conservative.
                    _ => u64::MAX,
                };
                if InputRev(current) != input.rev {
                    return Err(not_written(
                        NotWrittenReason::Stale,
                        "input of the guarded class came after the guard's revision",
                    ));
                }
            }
            if let Some(model_rev) = guard.model_rev {
                if self.model_rev != model_rev {
                    return Err(not_written(
                        NotWrittenReason::Stale,
                        "the model changed after the guard's revision",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Hands the rest of the active transaction to the PTY, unless a write is out or the PTY takes no byte now.
    fn write_more(&mut self) {
        if self.input.blocked {
            return;
        }
        let Some(active) = self.input.active.as_mut() else {
            return;
        };
        if active.in_flight || active.written >= active.bytes.len() {
            return;
        }
        active.in_flight = true;
        let rest = active.bytes[active.written..].to_vec();
        self.actions.push_back(super::Action::PtyWrite(rest));
    }

    /// The count of a PTY write (`Ok(0)`: the PTY took nothing now; `PtyWritable` follows), or its OS error.
    pub(super) fn on_pty_written(&mut self, result: Result<usize, i32>) {
        let Some(active) = self.input.active.as_mut().filter(|a| a.in_flight) else {
            return;
        };
        active.in_flight = false;
        match result {
            Ok(n) => {
                active.written = (active.written + n).min(active.bytes.len());
                let done = active.written == active.bytes.len();
                let cancelled = active.cancelled;
                if n == 0 {
                    self.input.blocked = true;
                }
                if done {
                    // IN-6: a write that finished first reports its real outcome.
                    self.finish_active(WriteOutcome::Written, "");
                } else if cancelled {
                    self.finish_active(WriteOutcome::Cancelled, "cancelled during the write");
                } else if self.exit.is_some() {
                    self.finish_active(WriteOutcome::Partial, "the payload ended during the write");
                } else {
                    self.write_more();
                }
            }
            Err(errno) => {
                if self.exit.is_some() {
                    self.finish_active(WriteOutcome::Partial, "the payload ended during the write");
                } else {
                    self.finish_active(
                        WriteOutcome::Failed,
                        &format!("the PTY write failed with errno {errno}"),
                    );
                }
            }
        }
    }

    /// The PTY takes bytes again.
    pub(super) fn on_pty_writable(&mut self) {
        self.input.blocked = false;
        self.write_more();
    }

    /// The payload ended: a transaction that waits for room ends with what it wrote; queued ones end at their turn.
    pub(super) fn input_payload_ended(&mut self) {
        if self.input.active.as_ref().is_some_and(|a| !a.in_flight) {
            self.finish_active(WriteOutcome::Partial, "the payload ended during the write");
        }
        self.try_start();
    }

    /// Ends the active transaction. A transaction that wrote nothing is a certain zero, whatever ended it (IN-2).
    fn finish_active(&mut self, outcome: WriteOutcome, detail: &str) {
        let Some(active) = self.input.active.take() else {
            return;
        };
        let outcome = match outcome {
            WriteOutcome::Partial if active.written == 0 => {
                WriteOutcome::NotWritten(NotWrittenReason::SessionEnded)
            }
            other => other,
        };
        match active.owner {
            // A retired write belongs to the host before the fence: it reports nothing (DESIGN.md 3.4).
            Owner::Host(_) if active.retired => {}
            Owner::Host(req) => {
                let result = active.result(outcome, detail);
                self.complete_write(req, result);
            }
            Owner::Reply => {}
            // DP-5: a client's input that is not written as sent is refused on its route, with the bytes written when
            // part of it was. A written input sends no frame.
            Owner::Route(route, op) => {
                let written = active.payload_written();
                let refusal = match outcome {
                    WriteOutcome::Written => None,
                    WriteOutcome::Failed => Some(RefusalReason::Failed),
                    _ => Some(RefusalReason::SessionEnded),
                };
                if let Some(reason) = refusal {
                    self.refuse(route, op, reason, (written > 0).then_some(written));
                }
                // The route's held input is less: its read allowance grows (DP-5).
                self.read_allowances();
            }
        }
        self.try_start();
    }

    /// The fence of an adoption (DESIGN.md 3.4): request numbers are per link, so no request of the old host may complete
    /// on the new link. Its queued writes are dropped with no report, and its write in progress runs to its end and
    /// reports nothing. The routes and their input do not depend on the host (DP-8): a client's input and a model reply keep
    /// their places at the admission point.
    pub(super) fn input_fence(&mut self) {
        self.input.queue.retain(|p| !matches!(p, Pending::Host(_)));
        if let Some(active) = self
            .input
            .active
            .as_mut()
            .filter(|a| matches!(a.owner, Owner::Host(_)))
        {
            active.retired = true;
        }
    }

    fn complete_write(&mut self, req: u64, result: InputResult) {
        self.report(&WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Input(result)),
        });
    }
}

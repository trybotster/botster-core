//! The one admission point of the worker for PTY input (Core AM-2), with the host's writes (IN-1 to IN-7) and the guards of
//! IN-10.
//!
//! - **One admission point.** Every PTY input goes through [`Worker::try_start`]: one transaction owns the PTY input at a
//!   time, from its first byte to its last, across short writes (AM-2: contiguous). The next starts only after the previous
//!   completed, failed or was cancelled. Pending inputs are taken in arrival order (host writes in `begin` order).
//! - **The start is the first byte.** A transaction starts when the PTY takes its first byte. Its decisions (the guards of
//!   IN-10, the bracket and the encoding of IN-8 and IN-9) are made when it is offered to the PTY, and made again while the
//!   PTY takes none of it: a client frame or a model change before the start is seen; one after it is ordered after the
//!   write. The host's input revision advances, and the host hears it, at the start.
//! - **Exact counts (IN-2).** A short write, a cancel, a failure and the payload's end report the bytes really written. A
//!   write that is cancelled while a PTY write is out waits for that write's count before it completes (IN-6: "the
//!   cancelled op's completion reports exactly what happened").
//!
//! A reply of the model (a query's shadow reply, the model's own PTY writes) is a transaction of the same admission point, in
//! arrival order, that advances no input revision and completes no operation (EV-8).

use super::{PayloadState, Worker};
use botster_core_contract::prelude::*;
use botster_core_link::msg::{Observation, WorkerMsg};
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
}

/// The transaction that owns the PTY input (AM-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Active {
    /// The host's write (decided again until it starts), or none for a reply.
    host: Option<HostWrite>,
    /// Every byte that this transaction writes to the PTY.
    bytes: Vec<u8>,
    /// Where the caller's payload is in `bytes`: markers that the worker adds are outside it (IN-2 units).
    payload_start: usize,
    payload_len: usize,
    /// The bytes that the PTY took.
    written: usize,
    /// The PTY took a byte: the transaction has started, and its decisions are final.
    started: bool,
    /// A PTY write is out, and its count has not come.
    in_flight: bool,
    /// A cancel came; the transaction ends at the next count.
    cancelled: bool,
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
    /// The read-visible revision of the model (ST-1, IN-10's terminal guard). It advances at every read-visible change the
    /// worker sees: output and resize.
    pub(super) model_rev: u64,
}

impl InputState {
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

/// The decisions of a host write at its start: the bytes and where its payload is in them, or how it ends with no byte.
enum Decision {
    Write {
        bytes: Vec<u8>,
        payload_start: usize,
        payload_len: usize,
    },
    /// A certain zero (IN-2, IN-8, IN-9, IN-10).
    Zero(InputResult),
    /// A payload kind that this worker cannot write: `Internal`, the only error a write may complete with (A2-2).
    Internal,
}

impl Worker {
    /// IN-1: a host write joins the admission point; it starts at its turn.
    pub(super) fn on_write_input(&mut self, req: u64, payload: InputPayload, guard: Option<Guard>) {
        self.input.queue.push_back(Pending::Host(HostWrite {
            req,
            payload,
            guard,
        }));
        self.try_start();
    }

    /// A reply of the model joins the admission point (EV-8: it is written as one contiguous transaction, in order).
    pub(super) fn enqueue_reply(&mut self, bytes: Vec<u8>) {
        self.input.queue.push_back(Pending::Reply(bytes));
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
            .filter(|a| a.host.as_ref().is_some_and(|w| w.req == req))
        else {
            return;
        };
        active.cancelled = true;
        if !active.in_flight {
            self.finish_active(WriteOutcome::Cancelled, "cancelled during the write");
        }
    }

    /// The admission point (AM-2): when no transaction owns the PTY input, the next one is offered to the PTY.
    pub(super) fn try_start(&mut self) {
        // A write waits for the spawn's answer: it starts on a live payload or ends on a failed one.
        if self.payload == PayloadState::Spawning {
            return;
        }
        while self.input.active.is_none() {
            let Some(pending) = self.input.queue.pop_front() else {
                return;
            };
            match pending {
                Pending::Reply(bytes) => {
                    // A reply goes only to a live payload; it advances no revision and completes no operation.
                    if self.payload_live() && !bytes.is_empty() {
                        let len = bytes.len();
                        self.input.active = Some(Active {
                            host: None,
                            bytes,
                            payload_start: 0,
                            payload_len: len,
                            written: 0,
                            started: false,
                            in_flight: false,
                            cancelled: false,
                        });
                        self.write_more();
                    }
                }
                Pending::Host(write) => match self.decide(&write) {
                    Decision::Zero(result) => self.complete_write(write.req, result),
                    Decision::Internal => self.complete_internal(write.req),
                    Decision::Write {
                        bytes,
                        payload_start,
                        payload_len,
                    } => {
                        let empty = bytes.is_empty();
                        self.input.active = Some(Active {
                            host: Some(write),
                            bytes,
                            payload_start,
                            payload_len,
                            written: 0,
                            started: false,
                            in_flight: false,
                            cancelled: false,
                        });
                        if empty {
                            // Nothing to write: it starts and completes at once.
                            self.mark_started();
                            self.finish_active(WriteOutcome::Written, "");
                        } else {
                            self.write_more();
                        }
                    }
                },
            }
        }
    }

    fn payload_live(&self) -> bool {
        matches!(self.payload, PayloadState::Live(_)) && self.exit.is_none()
    }

    /// The decisions of a host write now: the session takes input, both guards pass (IN-10), and the model encodes the
    /// payload with its modes now (IN-8, IN-9).
    fn decide(&self, write: &HostWrite) -> Decision {
        if !self.payload_live() {
            return Decision::Zero(not_written(
                NotWrittenReason::SessionEnded,
                "the payload has ended",
            ));
        }
        if self.termed || self.killed || self.grace.is_some() {
            return Decision::Zero(not_written(
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
                    return Decision::Zero(not_written(
                        NotWrittenReason::Stale,
                        "input of the guarded class came after the guard's revision",
                    ));
                }
            }
            if let Some(model_rev) = guard.model_rev {
                if ModelRev(self.input.model_rev) != model_rev {
                    return Decision::Zero(not_written(
                        NotWrittenReason::Stale,
                        "the model changed after the guard's revision",
                    ));
                }
            }
        }
        let encoded = match &write.payload {
            InputPayload::Bytes { bytes } => Ok(Some((bytes.0.clone(), 0, bytes.0.len()))),
            InputPayload::Text { text } => Ok(Some((text.as_bytes().to_vec(), 0, text.len()))),
            other => match self.model.as_ref() {
                Some(model) => super::model::encode(model, other),
                None => Ok(None),
            },
        };
        match encoded {
            Ok(Some((bytes, payload_start, payload_len))) => Decision::Write {
                bytes,
                payload_start,
                payload_len,
            },
            Ok(None) => Decision::Internal,
            Err(reason) => Decision::Zero(not_written(
                reason,
                "the event has no write with the modes at its start",
            )),
        }
    }

    /// The transaction's first byte reached the PTY: a host write's revision advances, and the host hears it (IN-10, IN-4).
    fn mark_started(&mut self) {
        let Some(active) = self.input.active.as_mut() else {
            return;
        };
        if active.started {
            return;
        }
        active.started = true;
        if active.host.is_some() {
            self.input.host_rev += 1;
            let input_rev = InputRev(self.input.host_rev);
            self.report(&WorkerMsg::Observed {
                observation: Observation::HostInput { input_rev },
            });
        }
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
                if n > 0 {
                    self.mark_started();
                }
                let Some(active) = self.input.active.as_mut() else {
                    return;
                };
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

    /// The PTY takes bytes again. A host write that has not started is decided again with the state now (its start is its
    /// first byte).
    pub(super) fn on_pty_writable(&mut self) {
        self.input.blocked = false;
        let redecide = self
            .input
            .active
            .as_ref()
            .filter(|a| !a.started && !a.in_flight)
            .and_then(|a| a.host.clone());
        if let Some(write) = redecide {
            match self.decide(&write) {
                Decision::Write {
                    bytes,
                    payload_start,
                    payload_len,
                } => {
                    if let Some(active) = self.input.active.as_mut() {
                        active.bytes = bytes;
                        active.payload_start = payload_start;
                        active.payload_len = payload_len;
                    }
                }
                Decision::Zero(result) => {
                    self.input.active = None;
                    self.complete_write(write.req, result);
                    self.try_start();
                    return;
                }
                Decision::Internal => {
                    self.input.active = None;
                    self.complete_internal(write.req);
                    self.try_start();
                    return;
                }
            }
        }
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
        let result = active.result(outcome, detail);
        if let Some(write) = &active.host {
            self.complete_write(write.req, result);
        }
        self.try_start();
    }

    fn complete_write(&mut self, req: u64, result: InputResult) {
        self.report(&WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Input(result)),
        });
    }

    fn complete_internal(&mut self, req: u64) {
        self.report(&WorkerMsg::Done {
            req,
            result: OpResult::Err(CoreError::new(
                ErrorCode::Internal,
                "this worker cannot write this payload kind",
            )),
        });
    }
}

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
//! libghostty), for the host's writes and for a route's input alike (`model::encode`, the one encoder). A reply of the model (a query's shadow reply, the model's own PTY writes) is a transaction of the same
//! admission point, in arrival order, that advances no input revision and completes no operation (EV-8).

use super::facts::{Content, Exhausted, RecordId};
use super::{PayloadState, Worker};
use botster_core_contract::prelude::*;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_route_codec::prelude::{Op, RefusalReason};
use std::collections::VecDeque;
use std::time::Instant;

/// The PTY write errors that mean the program's side of the PTY is gone (steward ruling R-82, by R-50). Such a write ends as
/// the payload's end ends it (`session_ended` on a route), whatever its order against `PayloadExited`. Any other errno is an
/// OS write error while that side still exists (`Failed`). Each entry names its platforms and its OS meaning.
const PROGRAM_SIDE_GONE: [i32; 2] = [
    // EIO. Linux: a write to a PTY master after every descriptor of its slave closed (the hangup). macOS, measured (R-82
    // evidence, gates/botster-core-rc-r82-pty-errno-mac-e062de53-20261011-113602.log): after the program exited, and while the program
    // lives with its tty closed.
    5,
    // EPIPE. The testkit's program edge: a write after the program ended fails as a write to a closed pipe does.
    32,
];

/// A host write that waits for its turn at the admission point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostWrite {
    req: u64,
    payload: InputPayload,
    guard: Option<Guard>,
    /// Its input record (ST-7), made at receipt.
    record: Option<RecordId>,
}

/// What waits at the admission point, in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pending {
    Host(HostWrite),
    /// A reply of the model to the program (EV-8 shadow reply, a model PTY write).
    Reply(Vec<u8>),
    /// A client's input (DP-5): a `bytes` or `text` frame, or a committed paste, in its receive order on the route.
    Route(RouteInput),
}

/// The source of a transaction, for the session checks at its start (`Worker::admission`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Host,
    Reply,
    Route,
}

/// A client's input and the route that owns it: an exception goes back on that route, with the frame's op (DP-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteInput {
    route: RouteId,
    op: Option<Op>,
    payload: InputPayload,
    /// `paste_commit{want_result: true}`: a written paste is answered with `input_done` (Codec TS-5b).
    want_result: bool,
    /// Its input record (ST-7), made at receipt.
    record: Option<RecordId>,
    /// What the input holds of the route's input bound while it waits (DP-5, the read allowance): [`held_len`].
    held: usize,
}

/// What a waiting route input holds of the route's input bound (DP-5, `route_input_queue_bytes`, the read allowance): the
/// caller's bytes of a `bytes`, `text` or paste payload, and for a semantic key, mouse or focus input (whose bytes exist
/// only when it is encoded, IN-9) the bytes of its frame on the stream, `wire`. The undecoded frame held the same bytes in
/// the reader, so its decode gives no room back.
fn held_len(payload: &InputPayload, wire: usize) -> usize {
    match payload {
        InputPayload::Bytes { bytes } | InputPayload::Paste { bytes, .. } => bytes.0.len(),
        InputPayload::Text { text } => text.len(),
        _ => wire,
    }
}

/// Who owns a transaction, and so where its outcome goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// A host write: its `Done` goes to the host.
    Host(u64),
    /// A reply of the model: it reports nothing.
    Reply,
    /// A client's input: only an exception goes back, on its route (DP-5), and `input_done` when the paste asked for it.
    Route(RouteId, Option<Op>, bool),
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
    /// The input record of a host write or a client's input (ST-7); a reply has none.
    record: Option<RecordId>,
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
                Pending::Route(input) if input.route == route => Some(input.held),
                _ => None,
            })
            .sum();
        let active = self
            .active
            .as_ref()
            .filter(|a| matches!(a.owner, Owner::Route(r, ..) if r == route))
            .map_or(0, |a| a.bytes.len() - a.written);
        queued + active
    }

    /// The inputs of a route that the admission point holds: queued, and the route's write in progress. Each may still be
    /// refused on the route (DP-5).
    pub(super) fn route_inputs(&self, route: RouteId) -> usize {
        let queued = self
            .queue
            .iter()
            .filter(|p| matches!(p, Pending::Route(input) if input.route == route))
            .count();
        let active = self
            .active
            .as_ref()
            .is_some_and(|a| matches!(a.owner, Owner::Route(r, ..) if r == route));
        queued + usize::from(active)
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

/// The bytes of a transaction, and where the caller's payload is in them (`Active`).
type Encoded = (Vec<u8>, usize, usize);

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
    /// IN-1: a host write joins the host's FIFO; it starts at its turn. Its record is made now, at receipt (ST-7). When the
    /// instance has no sequence number left for a record, the write is refused with exact zero, and no record is made
    /// (steward ruling R-101, follow-up).
    pub(super) fn on_write_input(
        &mut self,
        now: Instant,
        req: u64,
        payload: InputPayload,
        guard: Option<Guard>,
    ) {
        let record = match self
            .facts
            .as_mut()
            .map(|f| f.received(now, InputSource::Host, Content::Payload(&payload)))
        {
            Some(Err(Exhausted)) => {
                self.complete_write(
                    req,
                    None,
                    not_written(
                        NotWrittenReason::Refused,
                        "the input records of this instance are used up",
                    ),
                );
                return;
            }
            Some(Ok(record)) => Some(record),
            None => None,
        };
        self.input.queue.push_back(Pending::Host(HostWrite {
            req,
            payload,
            guard,
            record,
        }));
        self.try_start();
    }

    /// A reply of the model joins the admission point (EV-8: it is written as one contiguous transaction, in order). An
    /// empty reply joins too: admission is the first decision of every source (PR3 round 6), and an admitted empty reply
    /// writes nothing.
    pub(super) fn enqueue_reply(&mut self, bytes: Vec<u8>) {
        self.input.queue.push_back(Pending::Reply(bytes));
        self.try_start();
    }

    /// DP-5: a client's input joins the admission point in its receive order, with its route and op. An empty frame joins
    /// too: at its turn it takes the same admission as every other client input (PR3 R5, Sol), and then writes nothing.
    pub(super) fn enqueue_route_input(
        &mut self,
        route: RouteId,
        op: Option<Op>,
        payload: InputPayload,
        want_result: bool,
        record: Option<RecordId>,
        wire: usize,
    ) {
        let held = held_len(&payload, wire);
        self.input.queue.push_back(Pending::Route(RouteInput {
            route,
            op,
            payload,
            want_result,
            record,
            held,
        }));
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
            let record = match self.input.queue.remove(at) {
                Some(Pending::Host(write)) => write.record,
                _ => None,
            };
            self.complete_write(
                req,
                record,
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
                    if self.admission(Source::Reply).is_ok() && !bytes.is_empty() {
                        let len = bytes.len();
                        self.start_unowned(Owner::Reply, None, (bytes, 0, len));
                    }
                    continue;
                }
                Pending::Route(input) => {
                    self.start_route(input);
                    continue;
                }
                Pending::Host(write) => write,
            };
            if let Err(result) = self.start_checks(&write) {
                self.complete_write(write.req, write.record, result);
                continue;
            }
            let (bytes, payload_start, payload_len) = match self.encoded(&write.payload) {
                Ok(Some(encoded)) => encoded,
                Err(reason) => {
                    self.complete_write(
                        write.req,
                        write.record,
                        not_written(reason, "the event has no write with the modes at its start"),
                    );
                    continue;
                }
                // IN-2 (steward ruling R-101 Q3): a kind that this worker cannot write is a certain zero, not an error.
                Ok(None) => {
                    self.complete_write(
                        write.req,
                        write.record,
                        not_written(
                            NotWrittenReason::Unsupported {
                                what: UnsupportedWhat::Other,
                            },
                            "this worker cannot write this payload kind",
                        ),
                    );
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
                record: write.record,
            });
            if empty {
                self.finish_active(WriteOutcome::Written, "");
            } else {
                self.write_more();
            }
        }
    }

    /// The session checks at the start of every transaction (AM-2, IN-2). The three sources call this one function, so
    /// that their checks cannot differ (PR3 F113):
    /// - the payload is live and has not exited, or the transaction is `SessionEnded`;
    /// - a host write and a client's input do not start while the payload is being stopped (`Stopping`).
    ///
    /// A reply is not input: it is the terminal's answer to a query of the payload, it has no outcome, and it advances no
    /// revision (EV-8). `Stopping` is an outcome of a write (IN-2), so it does not apply to a reply: a live payload that
    /// queries the terminal while it shuts down gets its answer.
    fn admission(&self, source: Source) -> Result<(), NotWrittenReason> {
        if !matches!(self.payload, PayloadState::Live(_)) || self.exit.is_some() {
            return Err(NotWrittenReason::SessionEnded);
        }
        if self.stopping() && source != Source::Reply {
            return Err(NotWrittenReason::Stopping);
        }
        Ok(())
    }

    /// The IN-2 reason of a client input that the session's end keeps from the PTY (R-103 (2), R-105 Q1): the reason of
    /// `admission`, and `SessionEnded` when `admission` would admit it.
    pub(super) fn session_reason(&self) -> NotWrittenReason {
        self.admission(Source::Route)
            .err()
            .unwrap_or(NotWrittenReason::SessionEnded)
    }

    /// The payload is being stopped: a `SIGTERM` or a `SIGKILL` went to its group (IN-2: `Stopping`).
    fn stopping(&self) -> bool {
        self.termed || self.killed
    }

    /// The bytes of a payload at the start of its transaction, with the caller's payload in them: `bytes` and `text` as they
    /// are; the other kinds encoded by the model with its modes at this start (IN-8, IN-9). `Err` is the certain zero of
    /// IN-8 or IN-9; `None` is a payload that this worker cannot write.
    fn encoded(&self, payload: &InputPayload) -> Result<Option<Encoded>, NotWrittenReason> {
        match payload {
            InputPayload::Bytes { bytes } => {
                let len = bytes.0.len();
                Ok(Some((bytes.0.clone(), 0, len)))
            }
            InputPayload::Text { text } => {
                let len = text.len();
                Ok(Some((text.as_bytes().to_vec(), 0, len)))
            }
            other => match self.model.as_ref() {
                Some(model) => super::model::encode(model, other),
                None => Ok(None),
            },
        }
    }

    /// A client's input at its turn (DP-5). It advanced its revision on receipt (IN-4).
    /// - On a payload that ended or is being stopped, it is refused `session_ended`, with nothing written (DP-5:
    ///   `NotWritten(SessionEnded)` and `NotWritten(Stopping)`).
    /// - It is encoded now, with the model's modes at this start (IN-8). A certain zero of the encoding is refused with its
    ///   reason (Codec TS-5b: `mode_precondition_failed`), with nothing written.
    /// - An input with no bytes writes nothing; a paste that asked for its result gets `input_done` with 0 bytes.
    fn start_route(&mut self, input: RouteInput) {
        let RouteInput {
            route,
            op,
            payload,
            want_result,
            record,
            ..
        } = input;
        if let Err(reason) = self.admission(Source::Route) {
            // The wire merges the reasons; the record keeps the IN-2 reason (steward ruling R-101 Q2).
            self.refuse(route, op, RefusalReason::SessionEnded, None);
            self.end_client_record(route, record, WriteOutcome::NotWritten(reason), 0);
            return;
        }
        let (refusal, reason) = match self.encoded(&payload) {
            Ok(Some(encoded)) if encoded.0.is_empty() => {
                if want_result {
                    self.input_done(route, op, 0);
                }
                self.end_client_record(route, record, WriteOutcome::Written, 0);
                return;
            }
            Ok(Some(encoded)) => {
                self.start_unowned(Owner::Route(route, op, want_result), record, encoded);
                return;
            }
            // DP-5: an event that the modes do not report (IN-9 `NotReported`, such as a mouse event with tracking off) is
            // omitted with no frame, as on a real terminal, with or without an op. Its record still ends, with the IN-2
            // reason (steward ruling R-101 Q2).
            Err(NotWrittenReason::NotReported) => {
                let outcome = WriteOutcome::NotWritten(NotWrittenReason::NotReported);
                self.end_client_record(route, record, outcome, 0);
                return;
            }
            Err(NotWrittenReason::ModePreconditionFailed) => (
                RefusalReason::ModePreconditionFailed,
                NotWrittenReason::ModePreconditionFailed,
            ),
            // DP-5: an event with no encoding (IN-9 `Unsupported{what}`) is refused with the encoder's `what` and its op.
            Err(NotWrittenReason::Unsupported { what }) => (
                RefusalReason::Unsupported {
                    what: what_name(what),
                },
                NotWrittenReason::Unsupported { what },
            ),
            // A kind with no encoding path in the worker: no other certain zero.
            Err(reason) => (
                RefusalReason::Unsupported {
                    what: "input".into(),
                },
                reason,
            ),
            Ok(None) => (
                RefusalReason::Unsupported {
                    what: "input".into(),
                },
                NotWrittenReason::Unsupported {
                    what: UnsupportedWhat::Other,
                },
            ),
        };
        self.refuse(route, op, refusal, None);
        self.end_client_record(route, record, WriteOutcome::NotWritten(reason), 0);
    }

    /// Starts a transaction that no host request owns: a reply, or a client's input.
    fn start_unowned(
        &mut self,
        owner: Owner,
        record: Option<RecordId>,
        (bytes, payload_start, payload_len): Encoded,
    ) {
        self.input.active = Some(Active {
            owner,
            bytes,
            payload_start,
            payload_len,
            written: 0,
            in_flight: false,
            cancelled: false,
            retired: false,
            record,
        });
        self.write_more();
    }

    /// The decisions at the start of a transaction: the session can take input, and both guards pass (IN-10; a failed guard
    /// is `Stale` with exact zero).
    fn start_checks(&self, write: &HostWrite) -> Result<(), InputResult> {
        match self.admission(Source::Host) {
            Ok(()) => {}
            Err(NotWrittenReason::Stopping) => {
                return Err(not_written(
                    NotWrittenReason::Stopping,
                    "the payload is being stopped",
                ))
            }
            Err(reason) => return Err(not_written(reason, "the payload has ended")),
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
                let from = active.written;
                active.written = (active.written + n).min(active.bytes.len());
                // ST-7: the client tail keeps the PTY bytes that client input wrote.
                if let (Owner::Route(..), Some(facts)) = (active.owner, self.facts.as_mut()) {
                    facts.client_wrote(&active.bytes[from..active.written]);
                }
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
                    // A route's held input is the unwritten rest of its write: its read allowance grows (DP-5).
                    self.settle_routes();
                    self.write_more();
                }
            }
            // R-82: the error says what failed. Its order against `PayloadExited` decides nothing.
            Err(errno) if PROGRAM_SIDE_GONE.contains(&errno) => self.finish_active(
                WriteOutcome::Partial,
                "the program's side of the PTY is gone",
            ),
            Err(errno) => self.finish_active(
                WriteOutcome::Failed,
                &format!("the PTY write failed with errno {errno}"),
            ),
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
            // A retired write belongs to the host before the fence: it reports nothing (DESIGN.md 3.4). Its record ends
            // with what it wrote (ST-7).
            Owner::Host(_) if active.retired => {
                self.end_record(active.record, outcome, active.written as u64);
            }
            Owner::Host(req) => {
                let result = active.result(outcome, detail);
                self.complete_write(req, active.record, result);
            }
            Owner::Reply => {}
            // DP-5: a client's input that is not written as sent is refused on its route, with the bytes written when
            // part of it was. A written input sends no frame, except a paste that asked for its result (Codec TS-5b).
            // `written_bytes` counts the PTY bytes after encoding, markers included (Codec TS-5b, Core IN-2).
            Owner::Route(route, op, want_result) => {
                let written = active.written as u64;
                let refusal = match outcome {
                    WriteOutcome::Written => None,
                    WriteOutcome::Failed => Some(RefusalReason::Failed),
                    _ => Some(RefusalReason::SessionEnded),
                };
                match refusal {
                    Some(reason) => {
                        self.refuse(route, op, reason, (written > 0).then_some(written))
                    }
                    None if want_result => self.input_done(route, op, written),
                    None => {}
                }
                self.end_client_record(route, active.record, outcome, written);
                // The route holds one input less: its reserved refusal room is free (DP-5).
                self.settle_routes();
            }
        }
        self.try_start();
    }

    /// The fence of an adoption (DESIGN.md 3.4): request numbers are per link, so no request of the old host may complete
    /// on the new link. Its queued writes are dropped with no report, and its write in progress runs to its end and
    /// reports nothing. The routes and their input do not depend on the host (DP-8): a client's input and a model reply keep
    /// their places at the admission point.
    pub(super) fn input_fence(&mut self) {
        let (dropped, kept) = std::mem::take(&mut self.input.queue)
            .into_iter()
            .partition(|p| matches!(p, Pending::Host(_)));
        self.input.queue = kept;
        for pending in dropped {
            if let Pending::Host(write) = pending {
                self.end_record(write.record, fenced_outcome(), 0);
            }
        }
        if let Some(active) = self
            .input
            .active
            .as_mut()
            .filter(|a| matches!(a.owner, Owner::Host(_)))
        {
            active.retired = true;
        }
    }

    /// The host write `req` ends: its `Done`, and the status update of its record.
    fn complete_write(&mut self, req: u64, record: Option<RecordId>, result: InputResult) {
        self.end_record(record, result.outcome, result.pty_bytes_written);
        self.report(&WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Input(result)),
        });
    }

    /// The end of an input's transaction (ST-7): its record's status update, from the IN-2 outcome and its PTY bytes.
    pub(super) fn end_record(
        &mut self,
        record: Option<RecordId>,
        outcome: WriteOutcome,
        pty_bytes: u64,
    ) {
        if let (Some(record), Some(facts)) = (record, self.facts.as_mut()) {
            let (state, pty_bytes) = record_end(outcome, pty_bytes);
            facts.ended(record, state, pty_bytes);
        }
    }

    /// The end of a client input's transaction: its record's status update, and the keyed `Activity` edge again (ST-7,
    /// EV-6).
    pub(super) fn end_client_record(
        &mut self,
        route: RouteId,
        record: Option<RecordId>,
        outcome: WriteOutcome,
        pty_bytes: u64,
    ) {
        if record.is_none() {
            return;
        }
        self.end_record(record, outcome, pty_bytes);
        self.report(&WorkerMsg::Observed {
            observation: Observation::ClientInputEnded { route },
        });
    }
}

/// A record's end state from its IN-2 outcome (ST-7, steward ruling R-101 Q1 and Q3), with `pty_bytes` exact for every
/// outcome except `Unknown`:
/// - `Written` -> `written`; `Cancelled` -> `cancelled`; `NotWritten(reason)` -> `not_written{reason}` (exact zero).
/// - `Partial` and `Failed` -> `partial` (Q1: a failed write is certain, and the rest is never written), even at 0 bytes.
/// - `Unknown` -> `unknown`, `pty_bytes` null (Q3: Core never claims zero).
fn record_end(outcome: WriteOutcome, pty_bytes: u64) -> (InputRecordState, Option<u64>) {
    match outcome {
        WriteOutcome::Written => (InputRecordState::Written, Some(pty_bytes)),
        WriteOutcome::Partial | WriteOutcome::Failed => {
            (InputRecordState::Partial, Some(pty_bytes))
        }
        WriteOutcome::Cancelled => (InputRecordState::Cancelled, Some(pty_bytes)),
        WriteOutcome::NotWritten(reason) => (InputRecordState::NotWritten { reason }, Some(0)),
        // `Unknown`, and a later outcome of the non-exhaustive enum: this worker cannot tell the count, so it claims none.
        _ => (InputRecordState::Unknown, None),
    }
}

/// The outcome of a host write that the adoption fence dropped before it started (DESIGN.md 3.4): the fence refuses the old
/// epoch's commands (DP-8), so it is a certain zero, `not_written{refused}`. No cancel was issued, so it is not
/// `cancelled`, and no host learns its end (ID-2): the record is how a reader learns it (steward ruling R-103 (3)).
fn fenced_outcome() -> WriteOutcome {
    WriteOutcome::NotWritten(NotWrittenReason::Refused)
}

/// The wire name of an encoder's `Unsupported{what}` (DP-5's `input_refused{unsupported{what}}`): its JSON value.
fn what_name(what: UnsupportedWhat) -> String {
    serde_json::to_value(what)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "input".to_string())
}

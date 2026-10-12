//! The stream routes of the session (P4a; DESIGN.md "P4a: the stream route").
//!
//! **Handoff (DP-2).** A route's stream rides on its `AttachRoute` frame: it is the one descriptor of the link read that
//! holds the frame's first byte (`handoff.rs`, plan revision 23ae).
//!
//! **Baseline (OU-9).** At the bind, in one step, the worker takes the baseline at the model's consumed cut `R`: `attached`,
//! `baseline_begin`, `screen`, `baseline_end`, `live`. Output that the model holds and has not applied (`unfed`) is after
//! `R`, so it is the first `output` frame; later PTY output follows. Each byte after `R` is on the route once.
//!
//! **Backpressure (OU-3d).** Each route has a queue with the threshold `route_queue_bytes`. The worker gives the driver a
//! PTY read budget: the smallest free payload space of the routes, so that no read can overflow a queue and no byte is
//! dropped. The baseline goes whole into the route's empty queue at the bind. Steward ruling R-45: the one baseline
//! sequence in delivery (the attach frames through `live`, with their stream prefixes) is exempt from the threshold, and
//! only the frames behind it count (the output after `R`, the held suffix included). A route over the threshold only
//! because of its baseline is not "not progressing" (9B): it stalls only by OU-3(b), when frames are pending and the
//! transport accepts no byte (the stall clock, `stall.rs`). While the frames behind the sequence are at the threshold, the
//! budget is 0 and no PTY byte is read. A resync starts a new sequence and sets its exempt bytes the same way.
//!
//! **Input backpressure (DP-5).** The driver reads a route's client bytes only within the route's read allowance
//! ([`Action::RouteReadAllowance`]). The allowance is the input bound less the bytes that the route holds: the client bytes
//! that do not make a whole frame yet, and the route's input that waits at the admission point or is being written. (A
//! complete frame that waits to be applied waits only while the route has no refusal room, and the allowance is then 0.)
//! The input bound is
//! `route_input_queue_bytes`, and at least one whole frame of the route with its stream prefix (steward ruling R-53: DP-3
//! admits a frame of `max_frame_bytes`, so the reader must be able to read one). The driver spends an allowance read by
//! read, so the worker's copy of it is lowered by each read, and a new allowance goes out whenever the driver's rest differs
//! from the value now. The allowance is 0 while the route's outgoing queue cannot hold one more `input_refused` frame behind
//! the baseline sequence (`route_queue_bytes`), and the worker then applies no more frames: a client that floods refused
//! input and does not read is held at `route_queue_bytes`. Each complete frame is still received on its read (IN-4): it
//! advances `input_rev{client}` then, and waits to be applied (PR3 F106). The frames that wait are applied when the room
//! comes back: a write of the route's queue, or the end of one of its inputs (PR3 F105). Each input of the route at the admission point can still be
//! refused (a failed write, the payload's end), so the room of one refusal is kept for each of them, in that check and in the
//! PTY read budget. A closing route discards the client bytes that it reads: no frame of it is applied, and no frame goes
//! after its `route_closed` (OU-2b).
//!
//! **Frame bounds (DP-3).** Every frame is checked against its codec bound (`bound_of`) before it is queued, and the
//! snapshot's size is checked before it is allocated. A route whose limits cannot carry the attach frames is closed
//! `HandoffFailed` with no frame (steward ruling R-44; the host's floor check, A19-1, keeps this from happening). So is a
//! route with no common terminal format (the host refuses it first, OU-1).

use super::facts::{Content, Exhausted, RecordId};
use super::receipt::{self, Kept, Receipt};
use super::{model, Action, Worker};
use botster_core_contract::prelude::*;
use botster_core_link::msg::Observation;
use botster_core_link::msg::WorkerMsg;
use botster_core_link::route::{frame_bounds, frame_fits, route_closed_frame};
use botster_route_codec::prelude::{
    stream_wrap, Attached, BaselineBegin, BaselineEnd, Decoded, HexBytes, HistoryState,
    HistoryUnavailable, Input, InputDone, InputRefused, Modes, Op, RefusalReason, RouteLimits,
    StreamReader, ToClient, ToWorker, TYPE_MODES,
};
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

mod stall;

/// A descriptor that the control link delivered. The driver keeps the object; the machine has the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DescriptorId(pub u64);

/// The optional route features that this worker can send (OU-1: `attached.features` is the intersection with the client's).
/// None yet: `output_deflate` is not negotiated (DP-3).
const ROUTE_FEATURES: &[&str] = &[];

/// The bytes that the stream binding adds to a frame (`stream_wrap`, TB-L2), and the overhead of one `output` frame (that
/// prefix and the type byte).
const STREAM_PREFIX: usize = 4;
const OUTPUT_OVERHEAD: usize = STREAM_PREFIX + 1;

/// The routes of the session.
#[derive(Debug, Default)]
pub(super) struct Routes {
    routes: BTreeMap<RouteId, Route>,
    /// The last budget that the driver got was a limit: when the last route ends, the limit is lifted once.
    limited: bool,
    /// `settle_routes` runs: a call from inside it (a refusal that an applied frame queues) only asks for one more pass.
    settling: bool,
    /// A change came during a pass of `settle_routes`: it makes one more pass.
    unsettled: bool,
    /// The receive order of the last client input frame of any route (DP-6 activity).
    activity: u64,
    /// The last size that the size policy gave (DP-6).
    computed: Option<(u32, u32)>,
}

/// One stream-delimited frame in a route's queue, with its class (DESIGN.md "The output queue and the resync sequence").
#[derive(Debug)]
struct Queued {
    bytes: Vec<u8>,
    kind: Kind,
    /// The frame belongs to a baseline sequence in delivery (`attached` through `live`, or `resync` through `live`): steward
    /// ruling R-45 exempts its bytes from the `route_queue_bytes` threshold.
    exempt: bool,
}

/// The class of a queued frame (DESIGN.md "The output queue and the resync sequence").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `output` and the baseline frames: a stall drops the frame when no byte of it is written (OU-3b).
    Droppable,
    /// `attached`, `input_refused`, `input_done` and `route_closed`: a stall never drops it.
    Kept,
    /// The `resync` of a stall (OU-2): kept while the route is `Stalled`. The new baseline goes right behind it when the route
    /// resumes, and the `resync` becomes `Droppable` with it.
    Resync,
}

#[derive(Debug)]
struct Route {
    limits: AppliedRouteLimits,
    route_tag: Option<String>,
    /// The client's owner (`AttachOptions.owner`): its input records name it (ST-7).
    owner: Option<String>,
    /// Stream-delimited frames in order; `written` bytes of the first one are on the transport.
    queue: VecDeque<Queued>,
    written: usize,
    /// The bytes of `queue` that are not written yet.
    queued: usize,
    /// The unwritten bytes of the baseline sequence in delivery (the attach frames through `live`, or a resync through
    /// `live`, with their stream prefixes): the bytes of the `exempt` frames. Steward ruling R-45 exempts them from the
    /// `route_queue_bytes` threshold: only the frames behind the sequence count.
    baseline: usize,
    /// The stall clock (R-90): the route's state, its progress mark, and its progress reports (`stall.rs`).
    stall: stall::Clock,
    /// The `baseline_end.history` of this route's baselines, from its options (OU-9).
    history: HistoryState,
    /// A `RouteWrite` is out (one at a time).
    writing: bool,
    /// A healthy close: `route_closed` is the last queued frame, and the route closes when it is written (OU-2b).
    closing: Option<RouteCloseReason>,
    /// The client's bytes that do not make a whole frame yet (TS-3).
    reader: StreamReader,
    /// The complete frames that the route received and has not applied yet, in receive order: each was counted as received
    /// (IN-4) when its read came, and each waits for the room of its refusal (DP-5).
    received: VecDeque<Received>,
    /// `options.input`: false makes the route view-only (OU-1).
    input: bool,
    /// The last read allowance that the driver got (`Action::RouteReadAllowance`); none yet at the bind.
    allowance: Option<usize>,
    /// The open paste (Codec TS-5d): at most one per route. A file has a slot of its own (steward ruling R-75).
    paste: Option<OpenPaste>,
    /// The last size that the route reported with a `resize` input, as rows and columns (DP-6).
    size: Option<(u32, u32)>,
    /// The route reported a size that the size policy has not read yet (DP-6).
    reported: bool,
    /// The receive order of the route's last input frame of any kind (DP-6 activity); 0 before its first.
    activity: u64,
}

impl Routes {
    /// DP-6 `Latest`: the last size of the open route with the most recent activity, among the routes that reported a size
    /// (the receive order has no ties). It is `Some` when it differs from the last size that the policy gave, or when a
    /// route reported a size since the last call: that report is newer than any host `Resize`. With no such route the
    /// size stays (`None`). A closing route applies no more input, so it is not a candidate. The policy runs after each
    /// route read, so a route that reported since the last call is the one with the most recent activity: the report that
    /// counts is the winner's.
    pub(super) fn policy(&mut self) -> Option<(u32, u32)> {
        let winner = self
            .routes
            .values()
            .filter(|route| route.closing.is_none())
            .filter_map(|route| {
                route
                    .size
                    .map(|size| (route.activity, size, route.reported))
            })
            .max_by_key(|&(activity, ..)| activity);
        for route in self.routes.values_mut() {
            route.reported = false;
        }
        let (_, latest, reported) = winner?;
        if !reported && self.computed == Some(latest) {
            return None;
        }
        self.computed = Some(latest);
        Some(latest)
    }
}

/// A paste between its `paste_begin` and its `paste_commit` or `paste_abort` (Codec TS-5d). Its chunks are assembled here,
/// within `max_paste_bytes`, and its commit goes to the admission point as one transaction. These bytes are outside the
/// route's input bound: `max_paste_bytes` bounds them, and they are not held at the admission point until the commit. A
/// closing route applies no more input, so a paste that is not committed when its route ends writes nothing.
#[derive(Debug)]
struct OpenPaste {
    op: Op,
    bytes: Vec<u8>,
    require_bracketed: bool,
}

/// A complete client frame that the route received (IN-4), with what its receipt decided (DP-5): the decision is made when
/// the frame is received, never when it is applied, so a frame received before `live` stays `NotReady` (PR3 F107).
#[derive(Debug)]
enum Received {
    /// Client input that goes to the admission point, with its op, true when a written paste is answered `input_done`, and
    /// the bytes of the frame that carried it on the stream (its length prefix included), and its input record (ST-7).
    Input(Option<Op>, InputPayload, bool, usize, Option<RecordId>),
    /// Client input that is refused with this op and reason.
    Refused(Option<Op>, RefusalReason),
    /// A frame that cannot be decoded: the route closes with this reason (OU-5), after the frames before it.
    Bad(RouteCloseReason),
}

impl Received {
    fn op(&self) -> Option<Op> {
        match self {
            Received::Input(op, ..) | Received::Refused(op, _) => *op,
            Received::Bad(_) => None,
        }
    }
}

impl Route {
    /// Queues a frame after the frames that are queued. `output` is droppable; `input_refused`, `input_done` and
    /// `route_closed` are kept.
    fn push(&mut self, frame: &ToClient) {
        let kind = if matches!(frame, ToClient::Output { .. }) {
            Kind::Droppable
        } else {
            Kind::Kept
        };
        self.push_encoded(&frame.encode(), kind, false);
    }

    fn push_encoded(&mut self, frame: &[u8], kind: Kind, exempt: bool) {
        let entry = self.entry(frame, kind, exempt);
        self.queue.push_back(entry);
    }

    /// One encoded frame as a queue entry, counted in `queued` (and in `baseline` when it is exempt).
    fn entry(&mut self, frame: &[u8], kind: Kind, exempt: bool) -> Queued {
        let bytes = stream_wrap(frame);
        self.queued += bytes.len();
        if exempt {
            self.baseline += bytes.len();
        }
        Queued {
            bytes,
            kind,
            exempt,
        }
    }

    /// DP-5: `modes` coalesces to one latest unstarted frame per route. An unstarted `modes` frame leaves the queue, and the
    /// new one goes at the end. A frame that has started (the front one, while a write of it is out or part of it is
    /// written) completes. So `modes` adds at most one frame to the route's queue, and a frame never in the baseline
    /// sequence, which has no `modes` frame. The frame is droppable: a stall drops it when it is not started (OU-3b), and the
    /// new baseline's `baseline_begin` carries the modes and the size again (OU-9).
    fn push_modes(&mut self, frame: &[u8]) {
        let started = usize::from(self.writing || self.written > 0);
        let unstarted = (started..self.queue.len())
            .find(|&i| self.queue[i].bytes.get(STREAM_PREFIX) == Some(&TYPE_MODES));
        if let Some(i) = unstarted {
            let old = self.queue.remove(i).expect("found above");
            self.queued -= old.bytes.len();
        }
        self.push_encoded(frame, Kind::Droppable, false);
    }

    /// What the receipt of one complete client input frame decides (DP-5), from the route's state at the receipt. `None` is a
    /// frame that has its whole effect now and sends no frame: a paste's chunk, its `paste_begin`, its abort before the
    /// commit (Codec TS-5d), and a `resize` (DP-6: the route's size, read by the size policy after the read). `wire` is the frame's size on the stream, and `key_text_bound` is
    /// `CoreLimits.max_key_text_bytes` (A15-1).
    fn decide(&mut self, frame: ToWorker, wire: usize, key_text_bound: usize) -> Option<Received> {
        let op = match &frame {
            ToWorker::Bytes { op, .. } | ToWorker::Text { op, .. } => (op.0 != 0).then_some(*op),
            ToWorker::PasteChunk { op, .. } | ToWorker::FileChunk { op, .. } => Some(*op),
            ToWorker::Input(input) => input.op(),
        };
        if !self.input {
            return Some(Received::Refused(op, RefusalReason::NotWritable));
        }
        if self.baseline > 0 {
            // DP-5: no input before `live` is written.
            return Some(Received::Refused(op, RefusalReason::NotReady));
        }
        let input = |payload| Some(Received::Input(op, payload, false, wire, None));
        match frame {
            ToWorker::Bytes { bytes, .. } => input(InputPayload::Bytes {
                bytes: HexBytes(bytes.0.to_vec()),
            }),
            ToWorker::Text { text, .. } => input(InputPayload::Text { text }),
            ToWorker::PasteChunk { op, bytes } => self.paste_chunk(op, &bytes.0),
            ToWorker::Input(Input::PasteBegin {
                op,
                total_bytes,
                require_bracketed,
            }) => self.paste_begin(op, total_bytes, require_bracketed.unwrap_or(false)),
            ToWorker::Input(Input::PasteCommit { op, want_result }) => {
                Some(match self.paste.take_if(|paste| paste.op == op) {
                    Some(paste) => Received::Input(
                        Some(op),
                        InputPayload::Paste {
                            bytes: HexBytes(paste.bytes),
                            require_bracketed: paste.require_bracketed,
                        },
                        want_result.unwrap_or(false),
                        wire,
                        None,
                    ),
                    // Codec TS-5d (R-5): a commit with no open paste, or with another op. The open paste stays open.
                    None => Received::Refused(Some(op), RefusalReason::Refused),
                })
            }
            ToWorker::Input(Input::PasteAbort { op }) => {
                match self.paste.take_if(|paste| paste.op == op) {
                    // Codec TS-5d: an abort before the commit writes nothing and sends no frame.
                    Some(_) => None,
                    // Codec TS-5d (R-5): an abort with no open paste, or with another op. The open paste stays open.
                    None => Some(Received::Refused(Some(op), RefusalReason::Refused)),
                }
            }
            // IN-9: a semantic key, mouse or focus event is a route input like `bytes`; the worker encodes it with the modes at
            // its transaction's start, and DP-5's outcome table answers it (`start_route`). A route key has no `repeat`
            // (IN-9: host `WriteInput` only). A15-1: a `text` over `max_key_text_bytes` UTF-8 bytes is refused `too_large`,
            // with no effect and the route open, before any other size check of the key.
            ToWorker::Input(Input::Key {
                text: Some(text), ..
            }) if text.len() > key_text_bound => {
                Some(Received::Refused(op, RefusalReason::TooLarge))
            }
            ToWorker::Input(Input::Key {
                key,
                mods,
                event,
                text,
                shifted_key,
                base_layout_key,
                ..
            }) => input(InputPayload::Key(KeyInput {
                key,
                shifted_key,
                base_layout_key,
                mods,
                event,
                text,
                repeat: None,
            })),
            ToWorker::Input(Input::Mouse {
                action,
                button,
                row,
                col,
                x,
                y,
                mods,
                notches,
                ..
            }) => input(InputPayload::Mouse(MouseInput {
                action,
                button,
                row,
                col,
                x,
                y,
                mods,
                notches,
            })),
            ToWorker::Input(Input::Focus { focused, .. }) => input(InputPayload::Focus { focused }),
            // DP-6: a resize reports the route's size and has no answer; the effective size arrives in `modes`. Its
            // activity is its receipt, as for every input frame, so its size counts from the same point. The codec
            // refuses a zero size (`Input::check`).
            ToWorker::Input(Input::Resize { rows, cols, .. }) => {
                self.size = Some((rows, cols));
                self.reported = true;
                None
            }
            // Files and query replies are later P4a work (DP-5): they are refused with their op (TS-5b), and the route stays
            // open.
            other => {
                let what = match other {
                    ToWorker::FileChunk { .. } => "file_chunk",
                    _ => "input",
                };
                Some(Received::Refused(
                    op,
                    RefusalReason::Unsupported {
                        what: what.to_string(),
                    },
                ))
            }
        }
    }

    /// `paste_begin` (Codec TS-5d): it opens the route's paste. A second one while a paste is open is refused `refused` (R-5),
    /// and the open paste stays open. A `total_bytes` over `max_paste_bytes` is refused `too_large` now (R-9: the worker
    /// may refuse at `paste_begin`), with no paste opened.
    fn paste_begin(
        &mut self,
        op: Op,
        total_bytes: Option<u64>,
        require_bracketed: bool,
    ) -> Option<Received> {
        if self.paste.is_some() {
            return Some(Received::Refused(Some(op), RefusalReason::Refused));
        }
        if total_bytes.is_some_and(|total| total > self.limits.max_paste_bytes) {
            return Some(Received::Refused(Some(op), RefusalReason::TooLarge));
        }
        self.paste = Some(OpenPaste {
            op,
            bytes: Vec::new(),
            require_bracketed,
        });
        None
    }

    /// `paste_chunk` (Codec TS-5d): its bytes join the open paste of its op. A chunk with no open paste, or with another op,
    /// is refused `refused` (R-5), and the open paste stays as it is. The first chunk that takes the paste over
    /// `max_paste_bytes` is refused `too_large`, and the paste is discarded: nothing of it is written.
    fn paste_chunk(&mut self, op: Op, bytes: &[u8]) -> Option<Received> {
        let max = usize::try_from(self.limits.max_paste_bytes).unwrap_or(usize::MAX);
        let Some(paste) = self.paste.as_mut().filter(|paste| paste.op == op) else {
            return Some(Received::Refused(Some(op), RefusalReason::Refused));
        };
        if paste.bytes.len().saturating_add(bytes.len()) > max {
            self.paste = None;
            return Some(Received::Refused(Some(op), RefusalReason::TooLarge));
        }
        paste.bytes.extend_from_slice(bytes);
        None
    }

    /// True when the encoded frame is within its bound on this route (DP-3, `bound_of`).
    fn fits(&self, frame: &[u8]) -> bool {
        frame_fits(frame, &self.limits)
    }

    /// The PTY bytes that the queue can still take as `output` frames: `free` bytes of queue hold `n` payload bytes in
    /// `ceil(n / per)` frames of `OUTPUT_OVERHEAD` each. With `f = free / (per + OUTPUT_OVERHEAD) + 1`, the payload
    /// `free - f * OUTPUT_OVERHEAD` needs at most `f` frames, so it always fits.
    fn free_payload(&self, queue_bytes: usize, reserved: usize) -> usize {
        let free = queue_bytes.saturating_sub(self.queued - self.baseline + reserved);
        let per = self.output_payload();
        let frames = free / (per + OUTPUT_OVERHEAD) + 1;
        free.saturating_sub(frames * OUTPUT_OVERHEAD)
    }

    /// True when one more `input_refused` frame fits behind the baseline sequence under `route_queue_bytes`, with the room
    /// `reserved` for the refusals that the route's held input may still need (DP-5).
    fn refusal_room(&self, queue_bytes: usize, reserved: usize) -> bool {
        self.queued - self.baseline + reserved + refusal_bound() <= queue_bytes
    }

    /// The input bound of the route (DP-5): `route_input_queue_bytes`, and at least one whole frame with its stream prefix
    /// (steward ruling R-53).
    fn input_bound(&self) -> usize {
        let frame = usize::try_from(self.limits.max_frame_bytes).unwrap_or(usize::MAX);
        usize::try_from(self.limits.route_input_queue_bytes)
            .unwrap_or(usize::MAX)
            .max(frame.saturating_add(STREAM_PREFIX))
    }

    /// The payload of one `output` frame: the frame bound less the type byte (TS-3). A route that takes output carried its
    /// attach frames, so its bound holds the type byte and at least one byte.
    fn output_payload(&self) -> usize {
        usize::try_from(self.limits.max_frame_bytes)
            .unwrap_or(usize::MAX)
            .saturating_sub(1)
    }
}

/// The largest `input_refused` frame that this worker queues, with its stream prefix: the largest op and written count, and
/// an `unsupported` reason whose `what` is at most `REFUSAL_WHAT_BYTES` (DP-5, "Bound on refusals").
fn refusal_bound() -> usize {
    stream_wrap(
        &ToClient::InputRefused(InputRefused {
            op: Some(Op(u64::MAX)),
            reason: RefusalReason::Unsupported {
                what: "w".repeat(REFUSAL_WHAT_BYTES),
            },
            written_bytes: Some(u64::MAX),
            path: None,
        })
        .encode(),
    )
    .len()
}

/// The longest `what` of a refusal: the codec's names of an input field or kind, and this worker's own names.
const REFUSAL_WHAT_BYTES: usize = 64;

/// The wire form of the applied limits (OU-1, "wire projection").
fn wire_limits(limits: &AppliedRouteLimits) -> RouteLimits {
    let ms = |d: std::time::Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    RouteLimits {
        max_frame_bytes: limits.max_frame_bytes,
        max_screen_frame_bytes: limits.max_screen_frame_bytes,
        max_history_page_bytes: limits.max_history_page_bytes,
        max_paste_bytes: limits.max_paste_bytes,
        max_file_bytes: limits.max_file_bytes,
        max_query_reply_bytes: limits.max_query_reply_bytes,
        query_client_deadline_ms: ms(limits.query_deadline),
        stall_deadline_ms: ms(limits.stall_deadline),
        stall_close_after_ms: ms(limits.stall_close_after),
    }
}

/// The client's first format that this worker can emit (OU-1); with no list, this worker's first.
fn negotiate(client: &[String]) -> Option<String> {
    let mine: Vec<String> = model::snapshot_formats()
        .iter()
        .map(botster_core_link::route::terminal_format)
        .collect();
    if client.is_empty() {
        return mine.into_iter().next();
    }
    client.iter().find(|name| mine.contains(name)).cloned()
}

impl Worker {
    /// `AttachRoute` (OU-1, OU-9, DP-2): the descriptor of the read that holds the frame's first byte is this route's stream,
    /// and the baseline is queued in this step. A frame with no descriptor of its own is a host fault (`handoff.rs`): the
    /// link closes.
    pub(super) fn on_attach_route(
        &mut self,
        route: RouteId,
        options: AttachOptions,
        limits: AppliedRouteLimits,
    ) {
        let Some(descriptor) = self.attach_descriptor(route) else {
            return;
        };
        self.actions
            .push_back(Action::BindRoute { descriptor, route });
        // The worker holds its own copy of the stream now: the host may close its copy (DP-2, steward ruling R-65).
        self.report(&WorkerMsg::RouteReceived { route });
        let mut entry = Route {
            limits,
            route_tag: options.route_tag.clone(),
            owner: options.owner.clone(),
            queue: VecDeque::new(),
            written: 0,
            queued: 0,
            baseline: 0,
            writing: false,
            closing: None,
            reader: StreamReader::default(),
            received: VecDeque::new(),
            input: options.input,
            allowance: None,
            paste: None,
            stall: stall::Clock::default(),
            history: match options.history {
                None | Some(HistoryMode::None) => HistoryState::NotRequested,
                // History pages are a later P4a PR; until then a route that asks for them is told that they are unavailable.
                Some(_) => HistoryState::Unavailable(HistoryUnavailable::Other),
            },
            size: None,
            reported: false,
            activity: 0,
        };
        match self.baseline(&entry, &options) {
            Ok((frames, sequence)) => {
                for (i, frame) in frames.iter().enumerate() {
                    // `attached` is kept; the baseline frames and the output are droppable (OU-3b). R-45: the baseline
                    // sequence ends at `live`; the output after `R` is behind it and counts.
                    let kind = if i == 0 { Kind::Kept } else { Kind::Droppable };
                    entry.push_encoded(frame, kind, i < sequence);
                }
            }
            Err(RouteCloseReason::HandoffFailed) => {
                // R-44: a failed handoff sends no frame (OU-2b); the transport closes and the host is told once.
                self.routes.routes.insert(route, entry);
                self.end_route(route, RouteCloseReason::HandoffFailed);
                return;
            }
            Err(reason) => {
                // No partial baseline (OU-9): `route_closed` is the only frame, when the route's bound carries it.
                let closed = ToClient::RouteClosed(
                    route_closed_frame(RouteCloseReason::SnapshotTooLarge)
                        .expect("a healthy reason has its frame"),
                )
                .encode();
                entry.closing = Some(reason);
                if !entry.fits(&closed) {
                    // Not even `route_closed` fits: the transport closes with no frame.
                    self.routes.routes.insert(route, entry);
                    self.end_route(route, reason);
                    return;
                }
                entry.push_encoded(&closed, Kind::Kept, false);
            }
        }
        self.routes.routes.insert(route, entry);
        // OU-7: a route that attaches after the exit was reported gets its baseline, then the session's close.
        if let (true, Some(status)) = (self.exit_drained, self.exit) {
            let (code, signal) = match status {
                super::ExitStatus::Code(code) => (Some(code), None),
                super::ExitStatus::Signal(signal) => (None, Some(signal)),
            };
            self.end_with_session(route, code, signal);
        }
        self.pump_route(route);
        self.settle_routes();
    }

    /// The encoded baseline frames at the consumed cut `R`, then the output after `R` that the model holds (OU-9), and the
    /// number of frames in the baseline sequence (through `live`, R-45). Each frame is within its bound on the route (DP-3).
    fn baseline(
        &self,
        route: &Route,
        options: &AttachOptions,
    ) -> Result<(Vec<Vec<u8>>, usize), RouteCloseReason> {
        // The host refuses an attach with no common format (OU-1); a worker that gets one cannot make a working route
        // from the options, so the handoff failed (R-44).
        let Some(terminal_format) = negotiate(&options.terminal_formats) else {
            return Err(RouteCloseReason::HandoffFailed);
        };
        let features = options
            .route_features
            .iter()
            .filter(|name| ROUTE_FEATURES.contains(&name.as_str()))
            .cloned()
            .collect();
        let mut encoded = vec![ToClient::Attached(Attached {
            features,
            terminal_format,
            limits: wire_limits(&route.limits),
        })
        .encode()];
        encoded.extend(self.baseline_sequence(route)?);
        // A host keeps `max_frame_bytes` at least the attach frames (A19-1); a route below it cannot be served, so the
        // handoff failed (R-44).
        if !encoded.iter().all(|frame| route.fits(frame)) {
            return Err(RouteCloseReason::HandoffFailed);
        }
        let sequence = encoded.len();
        encoded.extend(self.unfed_output(route));
        Ok((encoded, sequence))
    }

    /// The encoded baseline sequence at the model's consumed cut `R` (OU-9): `baseline_begin`, `screen`, `baseline_end`,
    /// `live`. An attach puts `attached` before it, and a resync puts `resync` (OU-2). The caller checks each frame against
    /// its bound on the route (DP-3).
    fn baseline_sequence(&self, route: &Route) -> Result<Vec<Vec<u8>>, RouteCloseReason> {
        // OU-2: a baseline that cannot be formed closes the route `SnapshotTooLarge`.
        let model = self
            .model
            .as_ref()
            .ok_or(RouteCloseReason::SnapshotTooLarge)?;
        // Two limits (DESIGN.md "Frames"): the native snapshot, and the route's screen frame (the snapshot and the type byte).
        // Both are checked on the library's size, before the snapshot is allocated (DP-3).
        let limit = self
            .limits
            .max_snapshot_bytes
            .min(route.limits.max_screen_frame_bytes.saturating_sub(1));
        let snapshot = model
            .term
            .snapshot_at_most(usize::try_from(limit).unwrap_or(usize::MAX))
            .ok()
            .flatten()
            .ok_or(RouteCloseReason::SnapshotTooLarge)?;
        let frames = [
            ToClient::BaselineBegin(BaselineBegin {
                rows: u32::from(model.term.rows()),
                cols: u32::from(model.term.cols()),
                modes: model.modes(),
            }),
            ToClient::Screen {
                payload: snapshot.into(),
            },
            ToClient::BaselineEnd(BaselineEnd {
                history: route.history,
            }),
            ToClient::Live,
        ];
        Ok(frames.iter().map(ToClient::encode).collect())
    }

    /// The output after `R` that the model holds and has not applied (`unfed`), as `output` frames within the route's frame
    /// bound: the first output after a baseline (OU-9).
    fn unfed_output(&self, route: &Route) -> Vec<Vec<u8>> {
        let Some(model) = self.model.as_ref() else {
            return Vec::new();
        };
        let per = route.output_payload();
        model
            .unfed()
            .chunks(per)
            .map(|chunk| {
                ToClient::Output {
                    payload: chunk.into(),
                }
                .encode()
            })
            .collect()
    }

    /// PTY output goes to every route that is not closing, unchanged and in order, in `output` frames within the route's
    /// frame bound (OU-12, DP-3). A `Stalled` route gets none: Core does not hold the PTY for it, and its resync carries a
    /// new baseline (OU-3b, OU-7).
    pub(super) fn route_output(&mut self, bytes: &[u8]) {
        let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
        for id in ids {
            let route = self.routes.routes.get_mut(&id).expect("listed");
            if route.closing.is_some() || route.stall.stalled() {
                continue;
            }
            let per = route.output_payload();
            for chunk in bytes.chunks(per) {
                route.push(&ToClient::Output {
                    payload: chunk.into(),
                });
            }
            self.pump_route(id);
        }
        self.settle_routes();
    }

    /// Starts the next write of a route, when none is out.
    fn pump_route(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        if route.writing {
            return;
        }
        let Some(front) = route.queue.front() else {
            return;
        };
        route.writing = true;
        let bytes = front.bytes[route.written..].to_vec();
        self.actions
            .push_back(Action::RouteWrite { route: id, bytes });
    }

    /// The answer to a `RouteWrite` (OU-3a: progress is the bytes that the kernel accepted).
    pub(super) fn on_route_written(&mut self, id: RouteId, result: Result<usize, i32>) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        route.writing = false;
        let n = match result {
            // A terminal write error (the driver reports `WouldBlock` as `Ok(0)`): a failed close (OU-2b `write_failed`). A
            // healthy close that was delivering keeps its first reason (OU-2).
            Err(_) => {
                let reason = route.closing.unwrap_or(RouteCloseReason::WriteFailed);
                self.end_route(id, reason);
                return;
            }
            // Nothing taken: the next `RouteWritable` retries.
            Ok(0) => return,
            Ok(n) => n,
        };
        // A write is the rest of the front frame (`pump_route`), and the driver reports at most the bytes of the write.
        let Some(front) = route.queue.front() else {
            return;
        };
        route.written += n;
        route.queued -= n;
        if front.exempt {
            route.baseline = route.baseline.saturating_sub(n);
        }
        if route.written == front.bytes.len() {
            route.queue.pop_front();
            route.written = 0;
        }
        // The transport accepted bytes: the route progressed (OU-3a), and a `Stalled` route resumes (OU-2).
        self.route_progressed(id);
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        if route.queue.is_empty() {
            if let Some(reason) = route.closing {
                self.end_route(id, reason);
                return;
            }
        }
        // The written bytes may make room for a refusal (DP-5).
        self.pump_route(id);
        self.settle_routes();
    }

    pub(super) fn on_route_writable(&mut self, id: RouteId) {
        self.pump_route(id);
    }

    /// Closes the transport once and reports the close once (OU-2). An ended session's close is reported too, when its queue
    /// is delivered or it fails or stalls (OU-7): the host waits for it, and posts it with the exit and the cause that the
    /// host decides (LC-5).
    pub(super) fn end_route(&mut self, id: RouteId, reason: RouteCloseReason) {
        let Some(mut route) = self.routes.routes.remove(&id) else {
            return;
        };
        self.discard_received(id, std::mem::take(&mut route.received), &reason);
        self.actions.push_back(Action::RouteClose { route: id });
        self.report(&WorkerMsg::RouteClosed {
            route: id,
            reason,
            route_tag: route.route_tag,
        });
        // DP-6: the next most recently active route's size applies.
        self.size_policy();
        self.settle_routes();
    }

    /// A healthy close (OU-2b): `route_closed` goes after every frame that is queued, and the transport closes when it is
    /// written. A route whose bound cannot carry the frame closes with no frame. A route that is closing already keeps its
    /// first reason.
    fn close_healthy(&mut self, id: RouteId, reason: RouteCloseReason) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        if route.closing.is_some() {
            return;
        }
        route.closing = Some(reason);
        // A closing route applies no more input: the frames that wait are discarded (plan 23ad).
        let discarded = std::mem::take(&mut route.received);
        self.discard_received(id, discarded, &reason);
        // DP-6: the closing route is no longer a candidate; the next most recently active route's size applies.
        self.size_policy();
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        let frame = ToClient::RouteClosed(
            route_closed_frame(reason).expect("a healthy reason has its frame"),
        )
        .encode();
        if !route.fits(&frame) {
            self.end_route(id, reason);
            return;
        }
        route.push_encoded(&frame, Kind::Kept, false);
        self.pump_route(id);
        self.settle_routes();
    }

    /// `Detach` (DP-7, OU-2b): the route closes with its reason, after the frames that are queued. A route that this worker
    /// does not hold is reported closed at once, so the host's `Detach` completes.
    pub(super) fn on_detach(&mut self, id: RouteId, reason: DetachReason) {
        let close = match reason {
            DetachReason::Replaced => RouteCloseReason::Replaced,
            DetachReason::Revoked => RouteCloseReason::Revoked,
            // `Detached`; the enum is non-exhaustive, so a later reason closes the route as a plain detach (as the host does).
            _ => RouteCloseReason::Detached,
        };
        if !self.routes.routes.contains_key(&id) {
            self.report(&WorkerMsg::RouteClosed {
                route: id,
                reason: close,
                route_tag: None,
            });
            return;
        }
        self.close_healthy(id, close);
    }

    /// OU-7: when the exit is reported, after the output tail is queued, every route closes `session_ended` with the exit.
    pub(super) fn routes_session_ended(&mut self, code: Option<i32>, signal: Option<i32>) {
        let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
        for id in ids {
            self.end_with_session(id, code, signal);
        }
    }

    /// One route's close at the session's end. A route that is closing already keeps its first reason (`close_healthy`). The
    /// host replaces the exit of the reported reason with its own (LC-5).
    fn end_with_session(&mut self, id: RouteId, code: Option<i32>, signal: Option<i32>) {
        let reason = RouteCloseReason::SessionEnded {
            exit: Exit {
                code,
                signal,
                cause: ExitCause::Other,
            },
        };
        self.close_healthy(id, reason);
    }

    /// The client's bytes on a route (DP-5, TS-3). Each complete frame is client input: it is tagged with its route and
    /// advances `input_rev{client}` on receipt (IN-4). `bytes` and `text` go to the admission point in their receive order
    /// (AM-2). A frame that cannot be decoded closes the route `BadFrame` with its code (OU-5: only that route).
    pub(super) fn on_route_read(&mut self, now: Instant, id: RouteId, bytes: &[u8]) {
        let Some(route) = self.routes.routes.get_mut(&id) else {
            return;
        };
        // The driver spent these bytes of its allowance (an unlimited one stays unlimited).
        if let Some(allowance) = route.allowance.filter(|&a| a != usize::MAX) {
            route.allowance = Some(allowance.saturating_sub(bytes.len()));
        }
        // A closing route applies no more input: its bytes are discarded, so they are not held (DP-5, OU-2b).
        if route.closing.is_none() {
            route.reader.push(bytes);
            self.receive_route(now, id);
            // DP-6: the read's inputs changed the route's activity, and a resize its size.
            self.size_policy();
        }
        // The decoded input may reserve the room of its refusal: the PTY budget and the allowances follow.
        self.settle_routes();
    }

    /// Receives every complete frame of the route's reader, in order (TS-3): each client input, applied or refused, advances
    /// `input_rev{client}` now, on receipt, whatever room the route has, and its record is made (IN-4, IN-10, ST-7). A paste's
    /// or a file's begin, chunks and abort are not input (steward ruling R-102). The frames wait in `received` for
    /// `apply_route`. A frame that cannot be decoded ends the receipt: nothing after it is a frame.
    fn receive_route(&mut self, now: Instant, id: RouteId) {
        let key_text_bound = usize::try_from(self.limits.max_key_text_bytes).unwrap_or(usize::MAX);
        loop {
            let Some(route) = self.routes.routes.get_mut(&id) else {
                return;
            };
            if matches!(route.received.back(), Some(Received::Bad(_))) {
                return;
            }
            let bounds = frame_bounds(&route.limits);
            let (decoded, raw) = match route.reader.next_frame(&bounds) {
                Ok(None) => return,
                Ok(Some(frame)) => (ToWorker::decode(&frame, &bounds), frame),
                Err(error) => (Err(error), Default::default()),
            };
            // The frame's bytes on the stream: its 4-byte length prefix and its body (Codec TS-3).
            let wire = 4 + raw.len();
            let decoded = match decoded {
                Ok(decoded) => decoded,
                // A frame that cannot be decoded is no input: it ends the receipt, and the route closes `bad_frame`.
                Err(error) => {
                    route
                        .received
                        .push_back(Received::Bad(RouteCloseReason::BadFrame {
                            code: error.code,
                        }));
                    return;
                }
            };
            let receipt = receipt::of_decoded(&decoded, &raw);
            let received = match decoded {
                Decoded::Frame(frame) => route.decide(frame, wire, key_text_bound),
                Decoded::Ignored { .. } => continue,
                // A complete input frame that cannot be applied is still client input: it is received, then refused (IN-4).
                Decoded::Unsupported { op, what } => {
                    Some(Received::Refused(op, RefusalReason::Unsupported { what }))
                }
                // Steward ruling R-75: a parseable frame that breaks a field rule of Core 5.1A, and is not over a size bound
                // or of an unknown kind or value, is refused `refused`, with zero effect. The codec's field rules
                // (`Input::check`) are of that class.
                Decoded::Invalid { op, .. } => Some(Received::Refused(op, RefusalReason::Refused)),
            };
            // DP-6: every decoded input frame of the route, input or not (a paste's begin, a resize), is its activity.
            self.route_active(id);
            let received = match (receipt, received) {
                (Receipt::Input(kept), Some(received)) => {
                    self.client_input(now, id, &kept, received)
                }
                (_, received) => received,
            };
            if let Some(route) = self.routes.routes.get_mut(&id) {
                route.received.extend(received);
            }
        }
    }

    /// The receipt of one client input (IN-4, ST-7, steward ruling R-102): `input_rev{client}` advances and its record is
    /// made, both now. A refused input's record ends at once, `not_written` with its IN-2 reason (R-101 Q2). When the
    /// instance has no sequence number left for a record, the input is refused `refused`, and the revision does not advance
    /// (R-101, follow-up).
    fn client_input(
        &mut self,
        now: Instant,
        id: RouteId,
        kept: &Kept,
        received: Received,
    ) -> Option<Received> {
        let source = self
            .routes
            .routes
            .get(&id)
            .map(|route| InputSource::Client {
                route_tag: route.route_tag.clone(),
                owner: route.owner.clone(),
            })?;
        let record = match &mut self.facts {
            Some(facts) => {
                let content = match &received {
                    Received::Input(_, payload, ..) => Content::Payload(payload),
                    _ => kept.content(),
                };
                match facts.received(now, source, content) {
                    Ok(record) => Some(record),
                    Err(Exhausted) => {
                        return Some(Received::Refused(received.op(), RefusalReason::Refused))
                    }
                }
            }
            None => None,
        };
        self.client_received(id);
        Some(match received {
            Received::Input(op, payload, want_result, wire, _) => {
                Received::Input(op, payload, want_result, wire, record)
            }
            Received::Refused(op, reason) => {
                let outcome = WriteOutcome::NotWritten(receipt::refused(&reason));
                self.end_client_record(id, record, outcome, 0);
                Received::Refused(op, reason)
            }
            bad @ Received::Bad(_) => bad,
        })
    }

    /// The records of the received inputs that a route discards when it closes with `close` (ST-7, R-105 Q1): nothing of
    /// them is written.
    fn discard_received(
        &mut self,
        id: RouteId,
        received: VecDeque<Received>,
        close: &RouteCloseReason,
    ) {
        let session = self.session_reason();
        for input in received {
            if let Received::Input(.., record) = input {
                self.end_client_record(id, record, receipt::discarded(close, session), 0);
            }
        }
    }

    /// Applies the route's received frames in order, while the route is open and one more refusal fits (DP-5). A frame that
    /// is not applied stays in `received`; `settle_routes` applies it when the route has room again.
    fn apply_route(&mut self, id: RouteId) {
        let queue_bytes = usize::try_from(self.limits.route_queue_bytes).unwrap_or(usize::MAX);
        loop {
            let reserved = self.reserved(id);
            let Some(route) = self.routes.routes.get_mut(&id) else {
                return;
            };
            if route.closing.is_some() || !route.refusal_room(queue_bytes, reserved) {
                return;
            }
            let Some(received) = route.received.pop_front() else {
                return;
            };
            match received {
                Received::Input(op, payload, want_result, wire, record) => {
                    self.enqueue_route_input(id, op, payload, want_result, record, wire)
                }
                Received::Refused(op, reason) => self.refuse(id, op, reason, None),
                Received::Bad(reason) => {
                    self.close_healthy(id, reason);
                    return;
                }
            }
        }
    }

    /// The read allowance of each route (DP-5), sent to the driver when it changed. A closing route reads with no limit: it
    /// discards what it reads.
    pub(super) fn read_allowances(&mut self) {
        let queue_bytes = usize::try_from(self.limits.route_queue_bytes).unwrap_or(usize::MAX);
        let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
        for id in ids {
            let held = self.input.route_bytes(id);
            let reserved = self.reserved(id);
            let route = self.routes.routes.get_mut(&id).expect("listed");
            let allowance = if route.closing.is_some() {
                usize::MAX
            } else if !route.refusal_room(queue_bytes, reserved) {
                0
            } else {
                // `settle_routes` applies the waiting frames first, and a frame still waits only for a route with no
                // refusal room: a route with room has no waiting frame, so its held bytes are the reader's and its input's.
                debug_assert!(route.received.is_empty());
                route
                    .input_bound()
                    .saturating_sub(held.saturating_add(route.reader.pending()))
            };
            if route.allowance != Some(allowance) {
                route.allowance = Some(allowance);
                self.actions.push_back(Action::RouteReadAllowance {
                    route: id,
                    bytes: allowance,
                });
            }
        }
    }

    /// The client closed its end, or its read failed (OU-5: `PeerClosed`, only that route). The transport ends now. A healthy
    /// close that was delivering keeps its first reason (OU-2), as a failed write does in `on_route_written`.
    pub(super) fn on_route_ended(&mut self, id: RouteId) {
        let Some(route) = self.routes.routes.get(&id) else {
            return;
        };
        let reason = route.closing.unwrap_or(RouteCloseReason::PeerClosed);
        self.end_route(id, reason);
    }

    /// DP-6: the route's activity is the receive order of its last input frame of any kind.
    fn route_active(&mut self, id: RouteId) {
        self.routes.activity += 1;
        if let Some(route) = self.routes.routes.get_mut(&id) {
            route.activity = self.routes.activity;
        }
    }

    /// The receipt of one client input, applied or refused: `input_rev{client}` advances, and the activity carries its route
    /// (IN-4). An ignored extension frame is not input, nor is a paste's or a file's begin, chunk or abort (R-102).
    fn client_received(&mut self, id: RouteId) {
        let input_rev = self.input.client_received();
        self.report(&WorkerMsg::Observed {
            observation: Observation::ClientInput {
                route: id,
                input_rev,
            },
        });
    }

    /// An `input_refused` for a frame that is not applied as sent (DP-5), queued in order with the route's frames, with the
    /// bytes written when part of it was (exact, or `None` when nothing was). A route that closed gets nothing, and so does a
    /// closing one: its `route_closed` is its last frame (OU-2b).
    pub(super) fn refuse(
        &mut self,
        id: RouteId,
        op: Option<Op>,
        reason: RefusalReason,
        written_bytes: Option<u64>,
    ) {
        let Some(route) = self
            .routes
            .routes
            .get_mut(&id)
            .filter(|r| r.closing.is_none())
        else {
            return;
        };
        let frame = ToClient::InputRefused(InputRefused {
            op,
            reason,
            written_bytes,
            path: None,
        });
        debug_assert!(stream_wrap(&frame.encode()).len() <= refusal_bound());
        route.push(&frame);
        self.pump_route(id);
        self.settle_routes();
    }

    /// `input_done` (Codec TS-5b): a paste that asked for its result was written whole, `written` bytes to the PTY. It is
    /// queued in order with the route's frames, in the room that the route kept for the input's refusal. A route that closed
    /// gets nothing, and so does a closing one (OU-2b).
    pub(super) fn input_done(&mut self, id: RouteId, op: Option<Op>, written: u64) {
        let Some(op) = op else {
            return;
        };
        let Some(route) = self
            .routes
            .routes
            .get_mut(&id)
            .filter(|r| r.closing.is_none())
        else {
            return;
        };
        let frame = ToClient::InputDone(InputDone {
            op,
            written_bytes: written,
            path: None,
        });
        debug_assert!(stream_wrap(&frame.encode()).len() <= refusal_bound());
        route.push(&frame);
        self.pump_route(id);
        self.settle_routes();
    }

    /// DP-6: the size that the session took goes to every route that is not closing, in `modes` with the model's modes
    /// (coalesced, [`Route::push_modes`]). A route whose bound cannot carry the frame gets none.
    pub(super) fn routes_size(&mut self, size: Size) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let frame = ToClient::Modes(Modes {
            modes: model.modes(),
            rows: size.rows,
            cols: size.cols,
        })
        .encode();
        let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
        for id in ids {
            let route = self.routes.routes.get_mut(&id).expect("listed");
            if route.closing.is_some() || !route.fits(&frame) {
                continue;
            }
            route.push_modes(&frame);
            self.pump_route(id);
        }
        self.settle_routes();
    }

    /// The one place where the routes' waiting work is re-evaluated (DP-5, PR3 F105). Each event that changes what a route
    /// holds or the room of its queue calls it: a read, a write of the queue, a queued frame, the end or the progress of a
    /// route's input, a bind and a close. It applies the frames that wait for room on every route, then sends the PTY read
    /// budget and the read allowances. A call from inside a pass (an applied frame queues a refusal) makes one more pass.
    pub(super) fn settle_routes(&mut self) {
        if self.routes.settling {
            self.routes.unsettled = true;
            return;
        }
        self.routes.settling = true;
        self.routes.unsettled = true;
        while std::mem::take(&mut self.routes.unsettled) {
            let ids: Vec<RouteId> = self.routes.routes.keys().copied().collect();
            for id in ids {
                self.apply_route(id);
            }
        }
        self.routes.settling = false;
        self.send_budget();
        self.report_route_progress();
    }

    /// The PTY read budget (OU-3d): the smallest free payload space of the routes that are not closing and not `Stalled`
    /// (OU-3b: Core stops holding the PTY for a stalled reader). With no such route the limit is lifted, once.
    fn send_budget(&mut self) {
        let queue_bytes = usize::try_from(self.limits.route_queue_bytes).unwrap_or(usize::MAX);
        let budget = self
            .routes
            .routes
            .iter()
            .filter(|(_, r)| r.closing.is_none() && !r.stall.stalled())
            .map(|(&id, r)| r.free_payload(queue_bytes, self.reserved(id)))
            .min();
        match budget {
            Some(n) => {
                self.routes.limited = true;
                self.actions.push_back(Action::PtyReadBudget(Some(n)));
            }
            None if self.routes.limited => {
                self.routes.limited = false;
                self.actions.push_back(Action::PtyReadBudget(None));
            }
            None => {}
        }
        self.read_allowances();
    }

    /// The queue room that the route keeps for the refusals of its held input: one `input_refused` for each of its inputs at
    /// the admission point (DP-5).
    fn reserved(&self, id: RouteId) -> usize {
        self.input.route_inputs(id) * refusal_bound()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(max_frame_bytes: u64, queued: usize) -> Route {
        Route {
            limits: AppliedRouteLimits {
                max_frame_bytes,
                max_screen_frame_bytes: 0,
                max_history_page_bytes: 0,
                max_paste_bytes: 0,
                max_query_bytes: 0,
                max_query_reply_bytes: 0,
                max_file_bytes: 0,
                query_deadline: std::time::Duration::ZERO,
                stall_deadline: std::time::Duration::ZERO,
                stall_close_after: std::time::Duration::ZERO,
                route_input_queue_bytes: 0,
            },
            route_tag: None,
            owner: None,
            queue: VecDeque::new(),
            written: 0,
            queued,
            baseline: 0,
            reader: StreamReader::default(),
            received: VecDeque::new(),
            input: true,
            writing: false,
            closing: None,
            allowance: None,
            paste: None,
            stall: stall::Clock::default(),
            history: HistoryState::NotRequested,
            size: None,
            reported: false,
            activity: 0,
        }
    }

    /// OU-3d: the budget is the largest payload whose `output` frames always fit the free queue space, less the room kept
    /// for refusals (DP-5), for every frame size.
    #[test]
    fn the_free_payload_always_fits_the_queue_as_output_frames() {
        for max_frame in [2u64, 3, 7, 64, 1025] {
            for queued in [0usize, 1, 50, 99, 100] {
                for reserved in [0usize, 1, 30] {
                    let r = route(max_frame, queued);
                    let payload = r.free_payload(100, reserved);
                    let per = r.output_payload();
                    let cost = payload + payload.div_ceil(per) * OUTPUT_OVERHEAD;
                    assert!(
                        cost <= 100usize.saturating_sub(queued + reserved),
                        "frame {max_frame}, queued {queued}, reserved {reserved}: {payload} costs {cost}"
                    );
                }
            }
        }
        assert_eq!(
            route(1025, 0).free_payload(100, 0),
            95,
            "one frame of 95 bytes and its 5"
        );
        assert_eq!(
            route(1025, 0).free_payload(100, 50),
            45,
            "the reserved room is not free"
        );
        assert_eq!(
            route(1025, 100).free_payload(100, 0),
            0,
            "a full queue takes nothing"
        );
        assert_eq!(
            route(1025, 101).free_payload(100, 0),
            0,
            "an overfull queue takes nothing"
        );
    }

    #[test]
    fn an_output_frame_carries_the_frame_bound_less_the_type_byte() {
        assert_eq!(route(1024, 0).output_payload(), 1023);
        assert_eq!(route(2, 0).output_payload(), 1);
    }

    /// OU-1: the client's first format that the worker emits; with no list, the worker's own first.
    #[test]
    fn the_format_is_the_clients_first_that_the_worker_emits() {
        let mine = botster_core_link::route::terminal_format(&model::snapshot_formats()[0]);
        assert_eq!(negotiate(&[]), Some(mine.clone()));
        assert_eq!(
            negotiate(&["no_such_format".into(), mine.clone()]),
            Some(mine.clone())
        );
        assert_eq!(negotiate(&["no_such_format".into()]), None);
    }
}

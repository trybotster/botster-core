//! The fake session-worker route endpoint (design 7.3). It is sans-IO and deterministic.
//!
//! One [`FakeWorker`] is one session: a scripted PTY program and any number of routes. The caller writes bytes to a route and reads bytes from it.
//! The fake has no terminal: screen, history, modes, notifications and the bytes of semantic input are script data (`script`, R-5, R-7).
//! The route carries TS-3 frames in a binding's delimiting: `stream` (`[u32 length][frame]`, TB-L2) or `chunked` (DataChannel messages, TR-3).
//! There is no wall clock, no thread and no OS randomness. Time moves with [`FakeWorker::advance_clock`]. The only randomness is a ChaCha8 generator from the seed.
//!
//! Every branch cites the clause that requires it. Open orders are the named choice points of [`Choice`].

mod controls;
mod host;
mod input;
pub mod script;

use botster_route_codec::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use script::{Notification, Outcome, SessionScript};
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

pub use controls::CONTROLS;
pub use host::{Decide, HostDone, HostOutcome, Planned, RouteEvent, StartView};

/// The `terminal_format` that the fake negotiates: the screen and history payloads are opaque script bytes (Codec TS-1, A.4).
pub const TERMINAL_FORMAT: &str = "fake-opaque/1";

/// The route identity inside the fake. The caller maps its names to these.
pub type RouteId = u32;

/// How the binding delimits route frames (Codec TS-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    /// `[u32 big-endian length][frame]` (Codec TB-L2).
    #[default]
    Stream,
    /// DataChannel messages `[u8 flags][chunk]` (Codec TR-3).
    Chunked,
}

/// The session's notification policy that the Hub supplies (Codec TS-4c).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPolicy {
    #[default]
    All,
    None,
    Matching(Vec<String>),
}

/// What the fake history capture produces for a baseline that asks for `full` (Codec TS-4, A.2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HistoryCapture {
    #[default]
    Complete,
    Unavailable(HistoryUnavailable),
    Incomplete(HistoryIncomplete),
}

/// The worker options that the Hub supplies, and the limits of the fake. Every field has a default.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WorkerConfig {
    pub framing: Framing,
    /// The features that the worker offers. `output_dropped` is left out when the frame bound is too small (Codec A.4).
    pub features: Vec<String>,
    pub rows: u32,
    pub cols: u32,
    pub max_frame_bytes: u64,
    pub max_screen_frame_bytes: u64,
    pub max_history_page_bytes: u64,
    /// `welcome.limits.max_chunk_bytes`: the chunk payload bound of every channel. It is not a route limit (Codec TR-3, HC A8-3).
    pub max_chunk_bytes: u64,
    pub max_paste_bytes: u64,
    pub max_file_bytes: u64,
    pub max_query_reply_bytes: u64,
    pub query_client_deadline_ms: u64,
    pub stall_deadline_ms: u64,
    pub stall_close_after_ms: u64,
    /// Core's `route_queue_bytes`: the byte bound of a route's outgoing queue.
    pub route_queue_bytes: u64,
    /// The bytes that the worker holds for input that waits for the PTY.
    pub input_queue_bytes: u64,
    pub notification_policy: NotificationPolicy,
    /// Core `admission_fairness_frames` (AM-2): the most frames of one source admitted while another source waits.
    pub admission_fairness_frames: u32,
    pub file_directory: String,
    pub file_name_limit: usize,
    pub history_capture: HistoryCapture,
    /// The script data that the terminal library and the PTY program supply (R-5.1, R-5.2).
    pub script: SessionScript,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            framing: Framing::Stream,
            features: vec!["output_dropped".into(), "output_deflate".into()],
            rows: 24,
            cols: 80,
            max_frame_bytes: 4096,
            max_screen_frame_bytes: 65536,
            max_history_page_bytes: 4096,
            max_chunk_bytes: 64,
            max_paste_bytes: 1024,
            max_file_bytes: 1024,
            max_query_reply_bytes: 64,
            query_client_deadline_ms: 100,
            stall_deadline_ms: 1000,
            stall_close_after_ms: 1000,
            route_queue_bytes: 16384,
            input_queue_bytes: 4096,
            notification_policy: NotificationPolicy::All,
            admission_fairness_frames: 1,
            file_directory: "/files".into(),
            file_name_limit: 255,
            history_capture: HistoryCapture::Complete,
            script: SessionScript::default(),
        }
    }
}

impl WorkerConfig {
    fn limits(&self) -> RouteLimits {
        RouteLimits {
            max_frame_bytes: self.max_frame_bytes,
            max_screen_frame_bytes: self.max_screen_frame_bytes,
            max_history_page_bytes: self.max_history_page_bytes,
            max_paste_bytes: self.max_paste_bytes,
            max_file_bytes: self.max_file_bytes,
            max_query_reply_bytes: self.max_query_reply_bytes,
            query_client_deadline_ms: self.query_client_deadline_ms,
            stall_deadline_ms: self.stall_deadline_ms,
            stall_close_after_ms: self.stall_close_after_ms,
        }
    }
}

/// What a client asks for at attach (Codec TS-1).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AttachSpec {
    pub history: HistoryRequest,
    pub route_features: Vec<String>,
    pub writable: bool,
    pub answers_queries: bool,
}

impl Default for AttachSpec {
    fn default() -> Self {
        AttachSpec {
            history: HistoryRequest::None,
            route_features: vec![],
            writable: true,
            answers_queries: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryRequest {
    None,
    Screen,
    Full,
}

/// The open orders of the contract, as named choice points. Each cites its clause (design 7.2, rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Choice {
    /// Where the worker cuts PTY output into `output` frames. Clause: Codec BP-2, TS-3.
    OutputSplit,
    /// Where a read cuts the bytes of a frame on a stream binding. Clause: Codec TS-3, TB-L4.
    ReadSplit,
    /// The size of each DataChannel chunk. Clause: Codec TR-3.
    ChunkSize,
    /// Whether the worker sends an `output` frame as `output_deflate` when the route negotiated it. Clause: Codec A.4 (0x0E).
    DeflatePerFrame,
    /// Whether `modes` comes before or after the `output` that caused the change. Clause: Codec TS-4d.
    ModesOrder,
    /// Whether a stalled reader gets its queued output discarded at the deadline, or the worker keeps waiting. Clause: Codec TS-7, TS-4c.
    StallDiscard,
}

struct OutItem {
    frame: Vec<u8>,
    class: Class,
    /// The notifications that complete inside an `output` frame, found as the output passed (Codec TS-4c).
    notes: Vec<Notification>,
    query: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// A baseline, `modes` or `resync` frame.
    Control,
    Output,
    /// An exception frame: retained and never discarded (Codec TS-7).
    Exception,
    Query,
    Closed,
}

enum Piece {
    Stream(Vec<u8>),
    Message(Vec<u8>),
}

struct Started {
    pieces: VecDeque<Piece>,
    live: bool,
    closed: bool,
}

pub(crate) struct Route {
    pub(crate) features: Vec<String>,
    pub(crate) writable: bool,
    pub(crate) answers_queries: bool,
    pub(crate) limits: RouteLimits,
    pub(crate) ever_live: bool,
    out: VecDeque<OutItem>,
    pub(crate) out_bytes: u64,
    started: Option<Started>,
    pub(crate) eof: Option<Option<RouteEndReason>>,
    pub(crate) reader: StreamReader,
    pub(crate) chunks: ChunkReader,
    pub(crate) inbound: VecDeque<Vec<u8>>,
    pub(crate) stalled: bool,
    last_progress: u64,
    stall_resync_at: Option<u64>,
    pub(crate) fail_writes: bool,
    pub(crate) closing: bool,
    pub(crate) last_activity: u64,
    pub(crate) last_size: Option<(u32, u32)>,
    pub(crate) last_focus: bool,
    pub(crate) paste: Option<input::Paste>,
    pub(crate) file: Option<input::FileUpload>,
    attached_at: u64,
}

/// A query that waits for its answer (Codec TS-7).
pub(crate) struct Query {
    pub(crate) route: RouteId,
    pub(crate) asked_at: u64,
    pub(crate) state: QueryState,
    pub(crate) shadow: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryState {
    /// The client may answer.
    Asked,
    /// The fallback is admitted or written: the client path is closed (Codec TS-7).
    FallbackDone,
    /// The fallback was refused at admission and waits (Codec TS-7).
    FallbackWaiting,
    /// A client reply was admitted.
    Replied,
}

/// What a write to the PTY does. The guards are checked, and the scripted bytes are chosen, when the write starts (Core IN-9, Codec TS-5e).
pub(crate) enum Action {
    Raw(Vec<u8>),
    /// A host write (Core IN-1): its guards and bytes are chosen when it starts.
    Host {
        token: u64,
        decide: Option<Decide>,
    },
    /// A key, mouse or focus input. The script supplies the bytes it writes.
    Semantic(Input),
    Paste {
        require_bracketed: bool,
        want_result: bool,
    },
    File {
        name: String,
        content: Vec<u8>,
    },
}

/// A PTY write that waits for the PTY (Codec TS-5, TS-5b).
pub(crate) struct PendingWrite {
    pub(crate) route: RouteId,
    pub(crate) op: Option<Op>,
    pub(crate) action: Action,
    /// No frame answers it: the focus-out, a query fallback.
    pub(crate) silent: bool,
}

/// PTY output that the worker has not read yet. The session's output source waits while a progressing reader's queue is full (Codec TS-7).
pub(crate) struct SourceChunk {
    bytes: Vec<u8>,
    notes: Vec<(usize, Notification)>,
    /// The modes after the chunk, from the script (Codec TS-4d).
    modes: Option<ModeFlags>,
    pos: usize,
    take: Option<usize>,
}

/// What a read of a route gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    /// Bytes of the route, in the binding's delimiting. On a chunked route, one DataChannel message.
    Bytes(Vec<u8>),
    /// The reader makes no progress (the stall control).
    Blocked,
    /// Nothing to read until a new stimulus arrives.
    Empty,
    /// The route ended. `ended` is the cause that the worker reports to the Hub for a failed route (Codec TS-9).
    Eof { ended: Option<RouteEndReason> },
}

/// The fake session worker.
pub struct FakeWorker {
    pub(crate) cfg: WorkerConfig,
    rng: ChaCha8Rng,
    pub(crate) now: u64,
    /// The size applied to the PTY (Codec TS-4d, TS-6).
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    /// The scripted modes (Codec TS-4d). Only a `pty_output` control with `scripted.modes` changes them.
    pub(crate) modes: ModeFlags,
    pub(crate) routes: BTreeMap<RouteId, Route>,
    next_route: RouteId,
    activity: u64,
    /// The bytes that reached the PTY, one segment per write, with the `op` that caused it (Codec TS-5b).
    pub(crate) pty_log: Vec<(Option<Op>, Vec<u8>)>,
    pub(crate) pty_blocked: bool,
    pub(crate) pty_fail_after: Option<usize>,
    pub(crate) pty_pending: VecDeque<PendingWrite>,
    pub(crate) queries: BTreeMap<String, Query>,
    next_query: u64,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) file_counter: u64,
    pub(crate) fail_file_write: bool,
    pub(crate) fail_next_attach: Option<AttachFailedReason>,
    pub(crate) exited: Option<Exit>,
    pub(crate) exit_pending: bool,
    pub(crate) source: VecDeque<SourceChunk>,
    choices: BTreeMap<Choice, Vec<usize>>,
    /// Input revisions and the model revision (Core IN-10, ST-1).
    pub(crate) client_rev: u64,
    pub(crate) host_rev: u64,
    pub(crate) model_rev: u64,
    pub(crate) host_txn: Option<host::HostTxn>,
    pub(crate) host_done: VecDeque<HostDone>,
    pub(crate) pty_chunk: Option<usize>,
    /// The source of the last admitted write and how many in a row (Core AM-2 fairness).
    pub(crate) streak: (RouteId, u32),
    pub(crate) route_events: Vec<RouteEvent>,
    pub(crate) activity_routes: Vec<RouteId>,
    pub(crate) link_lost: bool,
    pub(crate) output_activity: bool,
}

impl FakeWorker {
    pub fn new(seed: u64, cfg: WorkerConfig) -> Self {
        FakeWorker {
            rows: cfg.rows,
            cols: cfg.cols,
            modes: cfg
                .script
                .modes
                .apply(&ModeFlags::default())
                .unwrap_or_default(),
            cfg,
            rng: ChaCha8Rng::seed_from_u64(seed),
            now: 0,
            routes: BTreeMap::new(),
            next_route: 1,
            activity: 0,
            pty_log: vec![],
            pty_blocked: false,
            pty_fail_after: None,
            pty_pending: VecDeque::new(),
            queries: BTreeMap::new(),
            next_query: 1,
            files: BTreeMap::new(),
            file_counter: 0,
            fail_file_write: false,
            fail_next_attach: None,
            exited: None,
            exit_pending: false,
            source: VecDeque::new(),
            choices: BTreeMap::new(),
            client_rev: 0,
            host_rev: 0,
            model_rev: 0,
            host_txn: None,
            host_done: VecDeque::new(),
            pty_chunk: None,
            streak: (u32::MAX, 0),
            route_events: vec![],
            activity_routes: vec![],
            link_lost: false,
            output_activity: false,
        }
    }

    /// The frame bounds of the routes of this worker (Codec TS-1).
    pub fn bounds(&self) -> FrameBounds {
        self.cfg.limits().bounds()
    }

    /// The bytes in a route's outgoing queue, not counting a frame that has started (Codec TS-7).
    pub fn queued_bytes(&self, id: RouteId) -> u64 {
        self.routes.get(&id).map_or(0, |r| r.out_bytes)
    }

    /// The values that each choice point took so far, for the reachability test (design 7.2, rule 3).
    pub fn choices_taken(&self) -> &BTreeMap<Choice, Vec<usize>> {
        &self.choices
    }

    /// One seeded decision with `n` outcomes.
    pub(crate) fn choose(&mut self, point: Choice, n: usize) -> usize {
        let pick = if n <= 1 {
            0
        } else {
            (self.rng.next_u64() % n as u64) as usize
        };
        let seen = self.choices.entry(point).or_default();
        if !seen.contains(&pick) {
            seen.push(pick);
        }
        pick
    }

    /// The largest exception frame that this worker can queue for a route (Codec TS-7): a refusal or a completion that carries a path.
    pub(crate) fn worst_exception(&self) -> u64 {
        160 + self.cfg.file_directory.len() as u64 + self.cfg.file_name_limit as u64 + 24
    }

    // ---- attach and baseline ----

    /// The Hub hands the connected route to the worker (Codec TS-1, TS-4). The baseline is queued at once.
    pub fn attach(&mut self, spec: &AttachSpec) -> RouteId {
        let id = self.next_route;
        self.next_route += 1;
        let limits = self.cfg.limits();
        // Codec TS-1: attached.features is the intersection. Codec TS-4c (rev 35): output_dropped needs a frame bound that can carry it.
        let features: Vec<String> = self
            .cfg
            .features
            .iter()
            .filter(|f| spec.route_features.contains(f))
            .filter(|f| {
                f.as_str() != "output_dropped"
                    || limits.max_frame_bytes
                        >= OutputDropped::min_frame_bytes(self.cfg.route_queue_bytes)
            })
            .cloned()
            .collect();
        self.activity += 1;
        self.routes.insert(
            id,
            Route {
                features: features.clone(),
                writable: spec.writable,
                answers_queries: spec.answers_queries,
                limits: limits.clone(),
                ever_live: false,
                out: VecDeque::new(),
                out_bytes: 0,
                started: None,
                eof: None,
                reader: StreamReader::new(),
                chunks: ChunkReader::with_max_chunk(self.cfg.max_chunk_bytes),
                inbound: VecDeque::new(),
                stalled: false,
                last_progress: self.now,
                stall_resync_at: None,
                fail_writes: false,
                closing: false,
                last_activity: 0,
                last_size: None,
                last_focus: false,
                paste: None,
                file: None,
                attached_at: self.activity,
            },
        );
        let screen = &self.cfg.script.screen;
        let failure = self.fail_next_attach.take().or_else(|| {
            // Codec TS-4: a screen over its frame bound cannot be produced, so the attach fails.
            (screen.len() as u64 + 1 > limits.max_screen_frame_bytes)
                .then_some(AttachFailedReason::SnapshotTooLarge)
        });
        if let Some(reason) = failure {
            // Codec TS-4: a screen that cannot be produced ends the route with attach_failed.
            self.push(
                id,
                ToClient::RouteClosed(RouteClosed {
                    reason: CloseReason::AttachFailed { reason },
                    exit: None,
                }),
                Class::Closed,
            );
            self.routes.get_mut(&id).expect("route").closing = true;
            return id;
        }
        self.push(
            id,
            ToClient::Attached(Attached {
                features,
                terminal_format: TERMINAL_FORMAT.into(),
                limits,
            }),
            Class::Control,
        );
        self.baseline(id, spec.history, false);
        id
    }

    /// Queues `baseline_begin`, `screen`, history and `baseline_end`, then `live` (Codec TS-4, TS-4b).
    /// With `resync` the baseline sends `screen` only and `baseline_end.history` is `not_requested` (Codec TS-4c).
    fn baseline(&mut self, id: RouteId, history: HistoryRequest, resync: bool) {
        let (rows, cols, modes) = (self.rows, self.cols, self.modes.clone());
        let screen = self.cfg.script.screen.clone();
        // Codec TS-4: a history page over its frame bound leaves the history unavailable (oversize).
        let page_max = self.cfg.max_history_page_bytes.saturating_sub(1) as usize;
        let pages = (history == HistoryRequest::Full && !resync).then(|| {
            let pages = self.cfg.script.history_pages.clone();
            pages.iter().all(|p| p.len() <= page_max).then_some(pages)
        });
        self.push(
            id,
            ToClient::BaselineBegin(BaselineBegin { rows, cols, modes }),
            Class::Control,
        );
        self.push(
            id,
            ToClient::Screen {
                payload: Payload::from(screen),
            },
            Class::Control,
        );
        let mut state = HistoryState::NotRequested;
        if let Some(pages) = pages {
            state = match (pages, self.cfg.history_capture) {
                // Codec TS-4: a row that does not fit a page leaves the history unavailable (oversize).
                (None, _) => HistoryState::Unavailable(HistoryUnavailable::Oversize),
                (Some(_), HistoryCapture::Unavailable(r)) => HistoryState::Unavailable(r),
                (Some(pages), capture) => {
                    for page in pages {
                        self.push(
                            id,
                            ToClient::History {
                                payload: Payload::from(page),
                            },
                            Class::Control,
                        );
                    }
                    match capture {
                        HistoryCapture::Incomplete(r) => HistoryState::Incomplete(r),
                        _ => HistoryState::Complete,
                    }
                }
            };
        }
        self.push(
            id,
            ToClient::BaselineEnd(BaselineEnd { history: state }),
            Class::Control,
        );
        self.push(id, ToClient::Live, Class::Control);
    }

    pub(crate) fn push(&mut self, id: RouteId, frame: ToClient, class: Class) {
        let Some(route) = self.routes.get_mut(&id) else {
            return;
        };
        if route.eof.is_some() {
            return;
        }
        let bytes = frame.encode();
        route.out_bytes += bytes.len() as u64;
        route.out.push_back(OutItem {
            frame: bytes,
            class,
            notes: vec![],
            query: None,
        });
    }

    /// Queues an exception frame: it is retained and never discarded (Codec TS-7).
    pub(crate) fn exception(&mut self, id: RouteId, frame: ToClient) {
        self.push(id, frame, Class::Exception);
    }

    // ---- output ----

    /// PTY output reaches the worker's source. The worker reads it when every route can take it (Codec BP-2, TS-7).
    pub(crate) fn pty_output(
        &mut self,
        bytes: &[u8],
        notifications: Vec<Notification>,
        modes: Option<ModeFlags>,
    ) {
        if bytes.is_empty() {
            return;
        }
        // Core ST-1: output is a read-visible change. Core 6.2: it is activity of the session.
        self.model_rev += 1;
        self.output_activity = true;
        // Codec TS-4c: the script names the notifications, so the fake never parses output.
        let notes = notifications
            .into_iter()
            .map(|n| (n.at.unwrap_or(bytes.len() - 1), n))
            .collect();
        self.source.push_back(SourceChunk {
            bytes: bytes.to_vec(),
            notes,
            modes,
            pos: 0,
            take: None,
        });
        self.pump_source();
    }

    /// Moves PTY output to the routes while every route has room. A progressing reader is lossless: the source waits (Codec TS-7).
    pub(crate) fn pump_source(&mut self) {
        loop {
            let Some(front) = self.source.front() else {
                break;
            };
            let remaining = front.bytes.len() - front.pos;
            let take = match front.take {
                Some(t) => t,
                None => {
                    // Choice OutputSplit: Codec BP-2, TS-3.
                    let cap = remaining.min(self.cfg.max_frame_bytes as usize - 1);
                    let t = 1 + self.choose(Choice::OutputSplit, cap);
                    self.source.front_mut().expect("chunk").take = Some(t);
                    t
                }
            };
            let need = take as u64 + 1;
            let ids: Vec<RouteId> = self
                .routes
                .iter()
                .filter(|(_, r)| r.eof.is_none() && !r.closing)
                .map(|(id, _)| *id)
                .collect();
            let mut wait = false;
            for id in &ids {
                let route = &self.routes[id];
                if route.out_bytes > 0 && route.out_bytes + need > self.cfg.route_queue_bytes {
                    // Codec TS-7 (overflow): the queue is full and the reader makes no progress, so the queued output is discarded.
                    if route.stalled && route.out.iter().any(|i| i.class == Class::Output) {
                        self.resync(*id, ResyncReason::Overflow);
                    }
                    let route = &self.routes[id];
                    if route.eof.is_none()
                        && route.out_bytes > 0
                        && route.out_bytes + need > self.cfg.route_queue_bytes
                    {
                        wait = true;
                    }
                }
            }
            if wait {
                break;
            }
            let front = self.source.front_mut().expect("chunk");
            let (start, end) = (front.pos, front.pos + take);
            let slice = front.bytes[start..end].to_vec();
            let notes: Vec<Notification> = front
                .notes
                .iter()
                .filter(|(o, _)| (start..end).contains(o))
                .map(|(_, n)| n.clone())
                .collect();
            front.pos = end;
            front.take = None;
            // Codec TS-4d: the scripted modes take effect once the whole chunk has passed.
            let changed = if front.pos == front.bytes.len() {
                let modes = front.modes.take();
                self.source.pop_front();
                modes
            } else {
                None
            };
            let changed = changed.map(|m| self.modes = m).is_some();
            if changed {
                // Core ST-1: a mode change is a read-visible change.
                self.model_rev += 1;
            }
            for id in ids {
                if !self.routes.contains_key(&id) {
                    continue;
                }
                let modes_first = changed && self.choose(Choice::ModesOrder, 2) == 0;
                if modes_first {
                    self.send_modes(id);
                }
                self.send_output(id, &slice, &notes);
                if changed && !modes_first {
                    self.send_modes(id);
                }
            }
        }
        if self.source.is_empty() && self.exit_pending {
            // Codec TS-8: the final tail is delivered before route_closed with session_ended.
            self.exit_pending = false;
            let exit = self.exited;
            let ids: Vec<RouteId> = self.routes.keys().copied().collect();
            for id in ids {
                self.close_route(id, CloseReason::SessionEnded, exit);
            }
        }
    }

    pub(crate) fn send_modes(&mut self, id: RouteId) {
        let frame = ToClient::Modes(Modes {
            modes: self.modes.clone(),
            rows: self.rows,
            cols: self.cols,
        });
        self.push(id, frame, Class::Control);
    }

    fn send_output(&mut self, id: RouteId, bytes: &[u8], notes: &[Notification]) {
        let Some(route) = self.routes.get(&id) else {
            return;
        };
        if route.eof.is_some() || route.closing {
            return;
        }
        let deflate = deflate_negotiated(&route.features);
        let max = route.limits.max_frame_bytes;
        let raw = ToClient::Output {
            payload: Payload::from(bytes),
        }
        .encode();
        let frame = if deflate && self.choose(Choice::DeflatePerFrame, 2) == 1 {
            let d = ToClient::output_deflate(Payload::from(deflate_output(bytes))).encode();
            // Codec TS-3: a frame never goes over its bound.
            if d.len() as u64 <= max {
                d
            } else {
                raw
            }
        } else {
            raw
        };
        let route = self.routes.get_mut(&id).expect("route");
        route.out_bytes += frame.len() as u64;
        route.out.push_back(OutItem {
            frame,
            class: Class::Output,
            notes: notes.to_vec(),
            query: None,
        });
    }

    // ---- resync (Codec TS-4c, TS-7) ----

    /// Discards queued output, reports the notifications in it, and starts a new baseline on the same route.
    pub(crate) fn resync(&mut self, id: RouteId, reason: ResyncReason) {
        let Some(route) = self.routes.get_mut(&id) else {
            return;
        };
        // Codec TS-4c: a frame that has started is finished first. If it cannot be, the route ends (Codec TS-9 write_failed).
        if route.started.is_some() && route.fail_writes {
            self.end_route(id, RouteEndReason::WriteFailed);
            return;
        }
        // The started frame stays in `started`: it finishes first. Only frames that have not started go.
        let mut kept = VecDeque::new();
        let mut discarded: Vec<Notification> = vec![];
        let mut retired: Vec<String> = vec![];
        let mut kept_bytes = 0;
        for item in route.out.drain(..) {
            match item.class {
                Class::Exception | Class::Closed => {
                    kept_bytes += item.frame.len() as u64;
                    kept.push_back(item);
                }
                Class::Output => discarded.extend(item.notes),
                Class::Query => retired.extend(item.query),
                Class::Control => {}
            }
        }
        route.out = kept;
        route.out_bytes = kept_bytes;
        route.stall_resync_at = None;
        route.last_progress = self.now;
        let offered = route.features.iter().any(|f| f == "output_dropped");
        // Codec TS-7: a query whose frame had not started takes the saved fallback, and its client path never reopens.
        for q in retired {
            self.fallback(&q);
        }
        if offered {
            let policy = self.cfg.notification_policy.clone();
            let passing: Vec<Notification> = discarded
                .into_iter()
                .filter(|n| passes(&policy, n))
                .collect();
            if let Some(last) = passing.last() {
                let max = self.routes[&id].limits.max_frame_bytes;
                let frame = OutputDropped::fit(
                    passing.len() as u64,
                    last.source,
                    &last.title,
                    &last.body,
                    max,
                );
                self.exception(id, ToClient::OutputDropped(frame));
            }
        }
        self.push(id, ToClient::Resync(Resync { reason }), Class::Control);
        // Codec TS-4: a screen over its bound cannot be produced, so the route ends.
        if self.cfg.script.screen.len() as u64 + 1 > self.cfg.max_screen_frame_bytes {
            self.end_route(id, RouteEndReason::WriteFailed);
            return;
        }
        self.baseline(id, HistoryRequest::Screen, true);
    }

    // ---- reading ----

    /// Reads route bytes: at most `max` bytes of a frame on a stream route, or one message on a chunked route.
    pub fn read(&mut self, id: RouteId, max: usize) -> Read {
        let framing = self.cfg.framing;
        let now = self.now;
        let Some(route) = self.routes.get_mut(&id) else {
            return Read::Eof { ended: None };
        };
        if route.stalled {
            // Codec TS-7: a stalled reader takes nothing, so its progress stops.
            return Read::Blocked;
        }
        if route.started.is_none() {
            let Some(item) = route.out.pop_front() else {
                return match route.eof {
                    Some(ended) => Read::Eof { ended },
                    None => Read::Empty,
                };
            };
            route.out_bytes -= item.frame.len() as u64;
            let live = item.frame.first() == Some(&TYPE_LIVE);
            let closed = item.class == Class::Closed;
            let cap = self.cfg.max_chunk_bytes.max(1) as usize;
            let pieces: VecDeque<Piece> = match framing {
                Framing::Stream => VecDeque::from([Piece::Stream(stream_wrap(&item.frame))]),
                Framing::Chunked => {
                    let mut sizes = vec![];
                    let mut left = item.frame.len();
                    while left > 0 {
                        // Choice ChunkSize: Codec TR-3. A chunk is at most max_chunk_bytes.
                        let s = 1 + self.choose(Choice::ChunkSize, cap.min(left));
                        sizes.push(s);
                        left -= s.min(left);
                    }
                    chunk_wrap(&item.frame, &sizes)
                        .into_iter()
                        .map(Piece::Message)
                        .collect()
                }
            };
            self.routes.get_mut(&id).expect("route").started = Some(Started {
                pieces,
                live,
                closed,
            });
        }
        let cut = match framing {
            Framing::Stream => self.choose(Choice::ReadSplit, max.max(1)) + 1,
            Framing::Chunked => 0,
        };
        let route = self.routes.get_mut(&id).expect("route");
        let started = route.started.as_mut().expect("started");
        let bytes = match started.pieces.pop_front() {
            Some(Piece::Stream(mut b)) => {
                let take = cut.min(b.len()).min(max.max(1));
                let rest = b.split_off(take);
                if !rest.is_empty() {
                    started.pieces.push_front(Piece::Stream(rest));
                }
                b
            }
            Some(Piece::Message(m)) => m,
            None => vec![],
        };
        if started.pieces.is_empty() {
            let done = route.started.take().expect("started");
            if done.live {
                // Codec TS-4b: live is the input-ready point.
                route.ever_live = true;
            }
            if done.closed {
                // Codec TS-8: route_closed is the last frame, then EOF.
                route.eof = Some(None);
            }
        }
        if !bytes.is_empty() {
            route.last_progress = now;
            route.stall_resync_at = None;
        }
        // Reading frees room: the source may go on, and the worker may read more input (Codec TS-7).
        self.pump_source();
        self.process_inbound(id);
        Read::Bytes(bytes)
    }

    // ---- ending a route ----

    /// A failed route (Codec TS-9): undelivered output is lost, and the worker reports the cause to the Hub.
    pub(crate) fn end_route(&mut self, id: RouteId, reason: RouteEndReason) {
        let Some(route) = self.routes.get_mut(&id) else {
            return;
        };
        if route.eof.is_some() {
            return;
        }
        route.out.clear();
        route.out_bytes = 0;
        route.started = None;
        route.eof = Some(Some(reason));
        // Core OU-2: the host learns of a failed route once.
        self.route_events
            .push(RouteEvent::Ended { route: id, reason });
        route.paste = None;
        route.file = None;
        self.route_gone(id);
        self.pump_source();
    }

    /// A healthy close (Codec TS-8): the queue drains, then `route_closed` is the last frame, then EOF.
    pub(crate) fn close_route(&mut self, id: RouteId, reason: CloseReason, exit: Option<Exit>) {
        let Some(route) = self.routes.get_mut(&id) else {
            return;
        };
        if route.eof.is_some() || route.closing {
            return;
        }
        route.closing = true;
        route.paste = None;
        route.file = None;
        // Core OU-2: the host learns of a close once, with the first reason.
        self.route_events.push(RouteEvent::Closed {
            route: id,
            reason,
            exit,
        });
        self.push(
            id,
            ToClient::RouteClosed(RouteClosed { reason, exit }),
            Class::Closed,
        );
        self.route_gone(id);
    }

    /// Effects of a route that ends (Codec TS-10, TS-6, TS-7).
    fn route_gone(&mut self, id: RouteId) {
        let focus_was = self.routes.get(&id).is_some_and(|r| r.last_focus);
        if let Some(r) = self.routes.get_mut(&id) {
            r.last_focus = false;
            r.last_size = None;
        }
        // Codec TS-10 (Core DP-12): the last route whose latest focus input was true ends, and the program asked for focus reports.
        let others_focused = self
            .routes
            .iter()
            .any(|(k, r)| *k != id && r.last_focus && r.eof.is_none() && !r.closing);
        // Codec TS-10 names both conditions: the last focused route ends, and the program enabled focus reporting (scripted modes).
        if focus_was && !others_focused && self.modes.focus_reporting {
            // The script supplies the focus-out outcome. A not-written outcome writes nothing.
            if let script::Outcome::Bytes(out) = self.cfg.script.writes.focus_out.clone() {
                self.write_pty(id, None, Action::Raw(out), true);
            }
        }
        // Codec TS-7: a query whose chosen route ends before a reply is admitted goes to the fallback.
        let open: Vec<String> = self
            .queries
            .iter()
            .filter(|(_, q)| q.route == id && q.state == QueryState::Asked)
            .map(|(k, _)| k.clone())
            .collect();
        for q in open {
            self.fallback(&q);
        }
        // Codec TS-6: when the active route closes, the next most recently active route's size applies.
        self.apply_size_policy();
    }

    // ---- time (Codec TS-7) ----

    pub fn advance_clock(&mut self, ms: u64) {
        self.now += ms;
        let ids: Vec<RouteId> = self.routes.keys().copied().collect();
        for id in ids {
            let (waiting, since, resynced, limits) = {
                let r = &self.routes[&id];
                (
                    r.eof.is_none() && (!r.out.is_empty() || r.started.is_some()),
                    self.now - r.last_progress,
                    r.stall_resync_at,
                    r.limits.clone(),
                )
            };
            if !waiting || since < limits.stall_deadline_ms {
                continue;
            }
            match resynced {
                None => {
                    // Codec TS-7: no progress for the stall deadline. Choice StallDiscard: the clause lets the worker discard.
                    if self.choose(Choice::StallDiscard, 2) == 1 {
                        self.resync(id, ResyncReason::Stalled);
                    }
                    if let Some(r) = self.routes.get_mut(&id) {
                        if r.eof.is_none() {
                            r.stall_resync_at = Some(self.now);
                            r.last_progress = self.now - limits.stall_deadline_ms;
                        }
                    }
                }
                Some(_) if since >= limits.stall_deadline_ms + limits.stall_close_after_ms => {
                    // Codec TS-7: still no progress after stall_close_after_ms.
                    self.end_route(id, RouteEndReason::Stalled);
                }
                Some(_) => {}
            }
        }
        // Codec TS-7: the host-supplied deadline passes without an answer, so the fallback runs.
        let late: Vec<String> = self
            .queries
            .iter()
            .filter(|(_, q)| {
                q.state == QueryState::Asked
                    && self.now - q.asked_at >= self.cfg.query_client_deadline_ms
            })
            .map(|(k, _)| k.clone())
            .collect();
        for q in late {
            self.fallback(&q);
        }
        self.pump_source();
    }

    // ---- terminal queries (Codec TS-7) ----

    /// The route that is asked: the most recently active route that can answer, or else the most recently attached one (Codec TS-6, TS-7).
    fn asked_route(&self) -> Option<RouteId> {
        let live = |r: &&Route| r.eof.is_none() && !r.closing && r.writable && r.answers_queries;
        self.routes
            .iter()
            .filter(|(_, r)| live(r) && r.last_activity > 0)
            .max_by_key(|(_, r)| r.last_activity)
            .or_else(|| {
                self.routes
                    .iter()
                    .filter(|(_, r)| live(r))
                    .max_by_key(|(_, r)| r.attached_at)
            })
            .map(|(id, _)| *id)
    }

    /// The program sent a query that needs a reply (Codec TS-7). The worker saves the shadow answer and never answers on its own.
    pub(crate) fn terminal_query(
        &mut self,
        request: &[u8],
        kind: Option<QueryKind>,
        shadow: Option<Vec<u8>>,
    ) -> String {
        let qid = format!("q{}", self.next_query);
        self.next_query += 1;
        let route = self.asked_route();
        self.queries.insert(
            qid.clone(),
            Query {
                route: route.unwrap_or(0),
                asked_at: self.now,
                state: QueryState::Asked,
                shadow: shadow.clone(),
            },
        );
        match route {
            Some(id) => {
                let frame = ToClient::TerminalQuery(TerminalQuery {
                    query_id: qid.clone(),
                    request_bytes_base64: base64_encode(request),
                    kind,
                    shadow_answerable: shadow.is_some(),
                    answer_within_ms: self.cfg.query_client_deadline_ms,
                });
                self.push(id, frame, Class::Query);
                if let Some(item) = self.routes.get_mut(&id).and_then(|r| r.out.back_mut()) {
                    item.query = Some(qid.clone());
                }
            }
            // Codec TS-7: no route to ask, so the fallback runs.
            None => self.fallback(&qid),
        }
        qid
    }

    /// Admits the saved shadow answer (Codec TS-7). A refusal at admission leaves the query pending.
    pub(crate) fn fallback(&mut self, qid: &str) {
        let Some(q) = self.queries.get(qid) else {
            return;
        };
        if matches!(q.state, QueryState::FallbackDone | QueryState::Replied) {
            return;
        }
        let route = q.route;
        match q.shadow.clone() {
            // Codec TS-7: with no shadow answer nothing is written.
            None => self.queries.get_mut(qid).expect("query").state = QueryState::FallbackDone,
            Some(bytes) => {
                if self.pty_blocked
                    && self.pty_queued_bytes() + bytes.len() > self.cfg.input_queue_bytes as usize
                {
                    // Codec TS-7: refused at admission, the query stays pending on the fallback path.
                    self.queries.get_mut(qid).expect("query").state = QueryState::FallbackWaiting;
                } else {
                    self.queries.get_mut(qid).expect("query").state = QueryState::FallbackDone;
                    self.write_pty(route, None, Action::Raw(bytes), true);
                }
            }
        }
    }

    // ---- size policy (Codec TS-6) ----

    /// The effective size is the last size of the most recently active route that reported one (Codec TS-6).
    pub(crate) fn apply_size_policy(&mut self) {
        let best = self
            .routes
            .values()
            .filter(|r| r.eof.is_none() && !r.closing && r.last_size.is_some())
            .max_by_key(|r| r.last_activity)
            .and_then(|r| r.last_size);
        if let Some((rows, cols)) = best {
            if (rows, cols) != (self.rows, self.cols) {
                (self.rows, self.cols) = (rows, cols);
                // Core ST-1: a resize is a read-visible change.
                self.model_rev += 1;
                let ids: Vec<RouteId> = self.routes.keys().copied().collect();
                for id in ids {
                    self.send_modes(id);
                }
            }
        }
    }

    pub(crate) fn next_activity(&mut self) -> u64 {
        self.activity += 1;
        self.activity
    }

    /// The bytes that a waiting write holds in the input queue (Codec TS-5, TS-7). A semantic input counts its scripted worst case.
    fn action_size(&self, action: &Action) -> usize {
        match action {
            Action::Raw(b) => b.len(),
            Action::Host { .. } => 0,
            Action::Semantic(i) => self.admission_size(i) as usize,
            Action::Paste { .. } => self.cfg.script.admission.paste as usize,
            Action::File { .. } => self.cfg.script.admission.file as usize,
        }
    }

    /// The worst-case admission size of a semantic input, from the script (Core IN-9, Codec TS-5b).
    pub(crate) fn admission_size(&self, input: &Input) -> u64 {
        let a = &self.cfg.script.admission;
        match input {
            Input::Key { .. } => a.key,
            Input::Mouse { .. } => a.mouse,
            Input::Focus { .. } => a.focus,
            _ => 0,
        }
    }

    pub(crate) fn pty_queued_bytes(&self) -> usize {
        self.pty_pending
            .iter()
            .map(|p| self.action_size(&p.action))
            .sum()
    }
}

/// Whether the route negotiated `output_deflate`. Clause: Codec A.4 (0x0E). A free function, so a mutant of this comparison has its own name
/// and is never covered by the equivalent-mutant entry of `.cargo/mutants.toml` for the seeded coin in `send_output`.
fn deflate_negotiated(features: &[String]) -> bool {
    features.iter().any(|f| f == "output_deflate")
}

fn passes(policy: &NotificationPolicy, n: &Notification) -> bool {
    match policy {
        NotificationPolicy::All => true,
        NotificationPolicy::None => false,
        // Codec TS-4c, HC SF-5c: a bell has no text and never matches. The text is the OSC 9 message, or the OSC 777 title and body joined
        // by one newline, and the patterns are literal substrings of it.
        NotificationPolicy::Matching(subs) => {
            let text = match n.source {
                NotificationSource::Bell => return false,
                NotificationSource::Osc9 => n.body.clone(),
                NotificationSource::Osc777 => format!("{}\n{}", n.title, n.body),
            };
            subs.iter().any(|s| text.contains(s.as_str()))
        }
    }
}

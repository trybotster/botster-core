//! The messages that follow the hello (plan section 3): the host's requests to a worker and the worker's reports.
//!
//! Both are JSON objects with a `"t"` tag, so a later worker of the same protocol number can add fields and variants that an
//! older host ignores or refuses by name (plan section 3: "additive fields keep N-1 compatible"). A contract type travels in its
//! own JSON form (`Op`, `OpResult`, `Exit`), so this wire has no second spelling of it.
//!
//! The host sends [`HostMsg`] in frames of type [`crate::frame::FrameType::HOST_MSG`]. The worker sends [`WorkerMsg`] in frames
//! of type [`crate::frame::FrameType::WORKER_MSG`].
//!
//! Clause: Core LC-3, Core LC-4, Core LC-5, Core LC-6, Core LC-7, Core AD-7, Core A2-1, Core EV-7, Core ST-4, Core ST-6.

use botster_core_contract::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The identity of a payload process: pid and start time (AD-6). The payload is the leader of its own process group, so its
/// pid is its group id (LC-5, LC-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadId {
    pub pid: u32,
    pub start_time: u64,
}

/// What the host tells a worker to launch (AD-7 step 4, after the worker's identity is durable).
///
/// Clause: Core LC-3, Core A2-1, Core A3-1, Core DP-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSpec {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    pub size: Size,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_profile: Option<ColorProfile>,
    #[serde(default)]
    pub notification_policy: NotificationPolicy,
    #[serde(default)]
    pub size_policy: SizePolicy,
    /// The largest frame payload that the host accepts and sends on this link, in bytes. The worker sizes its decoder with it
    /// (plan section 3: `len` is checked against a bound before any allocation).
    pub link_frame_bound: u32,
    /// The Core limits that the worker applies itself: the clipboard and notification bound (EV-3, A2-4), the input records
    /// (ST-7), the queries (EV-8) and the tap (TP-1). Absent: the defaults of the Core limits table.
    #[serde(default)]
    pub limits: CoreLimits,
    /// The ordinal of this instance in the data directory, from the host registry's durable counter (steward ruling R-101
    /// Q6): the high bits of every input cursor of `ReadFacts` (ST-7). It is required: protocol 1 is not released (R-98).
    pub cursor_ordinal: u64,
}

/// The cap of a worker link's frame bound, `u32::MAX / 2`. It is one literal, and a unit test ties it to `u32::MAX`.
pub const LINK_FRAME_CAP: u32 = 0x7FFF_FFFF;

/// The JSON length that each of the at most four protected entries of `ReadFacts` (ST-7, steward rulings R-116 and
/// R-117) can take past `input_events_bytes`, apart from its two route tags: a minimal record or a status update with
/// every number at its largest. A worker-core test measures the largest ones under it.
pub const PROTECTED_ENTRY_FIXED: u64 = 1024;

/// The largest JSON length of the reply of a `ReadFacts` (ST-7) under `limits`, apart from its fixed part: the record
/// list (at most `input_events_bytes`, and past it only the protected entries, at most four, each with two route tags
/// at `max_route_tag_bytes` that JSON can write as six bytes a byte), and the client tail as hex, two bytes a byte.
pub fn facts_reply_bound(limits: &CoreLimits) -> u64 {
    let tags = 12 * u64::from(limits.max_route_tag_bytes);
    let protected = 4 * (PROTECTED_ENTRY_FIXED + tags);
    let tail = 2 * u64::from(limits.input_tail_bytes);
    limits
        .input_events_bytes
        .saturating_add(protected)
        .saturating_add(tail)
}

/// The largest frame payload of a worker link under `limits` (plan section 3), in both directions. A host write travels
/// in its JSON form and a snapshot as pages, so the bound is six times the largest payload that `CoreLimits` allows: a
/// `Text` control byte is the six-byte escape `\u00XX` (lead ruling, 2026-10-12). A `ReadFacts` reply travels in one
/// frame, so the bound also holds the largest one (`facts_reply_bound`; #320 R320-SI-5). 1 MiB more holds every fixed
/// part. The bound is at most `LINK_FRAME_CAP`.
pub fn link_frame_bound(limits: &CoreLimits) -> u32 {
    let largest = limits.max_paste_bytes.max(limits.max_snapshot_bytes);
    let payload = largest.saturating_mul(6).max(facts_reply_bound(limits));
    let bound = payload.saturating_add(1 << 20);
    u32::try_from(bound).map_or(LINK_FRAME_CAP, |b| b.min(LINK_FRAME_CAP))
}

/// The bits of the cursor ordinal in an input cursor of `ReadFacts` (ST-7; steward ruling R-101, follow-up). The sequence
/// number of the instance's log is below them; together they stay below 2^53.
pub const CURSOR_ORDINAL_BITS: u32 = 24;
/// The largest cursor ordinal of a data directory, 2^24 - 1. The host mints ordinals from 1. It is one literal, and a unit
/// test ties it to `CURSOR_ORDINAL_BITS`.
pub const MAX_CURSOR_ORDINAL: u64 = 0xFF_FFFF;

/// A request of the host to a session worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
#[non_exhaustive]
pub enum HostMsg {
    /// Launches the payload (AD-7 step 4). The worker answers [`WorkerMsg::Launched`] or [`WorkerMsg::LaunchFailed`].
    Launch(Box<LaunchSpec>),
    /// The graceful request to end the payload (LC-5). The worker answers [`WorkerMsg::Exited`] when it has ended.
    Stop,
    /// The kill of the payload's process group, which Core sends at the end of `stop_grace` (LC-5).
    Kill,
    /// An operation that needs the worker's model or input path (the reads, `WriteInput`, the setters). `req` is the host's
    /// request number, unique on this link. The worker answers with [`WorkerMsg::Done`].
    Op { req: u64, op: Op },
    /// A cancel of the `WriteInput` that was sent as request `req` (IN-6).
    Cancel { req: u64 },
    /// LC-7 step 3: delete the uploaded files, answer [`WorkerMsg::RemoveResult`], and end.
    Remove,
    /// A route is registered (OU-1). Its connected stream rides with the first byte of this frame (DP-2, `SCM_RIGHTS`).
    /// `limits` are the values that the host applied and returned in `AttachResult`: the worker enforces them as given
    /// (OU-1: one source, passed through).
    AttachRoute {
        route: RouteId,
        options: AttachOptions,
        limits: AppliedRouteLimits,
        /// The client of `attach`: a stable tag that the worker keeps, so that an adopting host gets it back (DP-8).
        client: ClientId,
    },
    /// `Detach` (DP-7). The worker answers with `RouteClosed`.
    Detach {
        route: RouteId,
        reason: DetachReason,
    },
    /// The route's `stall_deadline` is due, measured from the worker's [`WorkerMsg::RouteProgress`] with this `mark` (R-90,
    /// OU-3b). The worker makes the route `Stalled` only while it is `Open`, its frames wait, and its mark is still `mark`;
    /// otherwise its newer report is already on the link, and it does nothing.
    RouteStallDue { route: RouteId, mark: u64 },
    /// The route's `stall_close_after` is due for the stall that began at this `mark` (R-90, OU-2). The worker closes the
    /// route only while it is still `Stalled` at that mark.
    RouteCloseDue { route: RouteId, mark: u64 },
    /// The new handle's id of the live route that the worker reported as [`WorkerMsg::RouteLive`] with `key` (DP-8, ID-2).
    /// The host sends one per route, after the row's `SessionState` posts. From it on, `route` names that route on this link.
    RouteId { key: u64, route: RouteId },
}

/// What a worker observed in its model or on its input path. The host adds the instance and the time (TM-1, EV-9).
///
/// Clause: Core EV-1, Core EV-6, Core EV-7, Core IN-4, Core DP-12, Core ST-4, Core TP-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Observation {
    /// The payload produced output (class K `Activity`, and `last_output_at`).
    Output {
        model_rev: ModelRev,
    },
    Modes {
        flags: ModeFlags,
        model_rev: ModelRev,
    },
    Title {
        title: String,
        model_rev: ModelRev,
    },
    Cwd {
        cwd: String,
        model_rev: ModelRev,
    },
    Size {
        size: Size,
        model_rev: ModelRev,
    },
    /// A client input frame was received on `route` (IN-4).
    ClientInput {
        route: RouteId,
        input_rev: InputRev,
    },
    /// The transaction of a client input of `route` ended (ST-7): the record's status update is appended. It changes no
    /// revision; the host raises the keyed `Activity` edge again (EV-6).
    ClientInputEnded {
        route: RouteId,
    },
    /// The host's write was admitted (IN-10).
    HostInput {
        input_rev: InputRev,
    },
    Focus {
        focused: FocusState,
    },
    Bell,
    PromptMark {
        mark: PromptMarkKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },
    Notification {
        source: NotificationSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        body: String,
        truncated: bool,
    },
    /// `contents` is the event's own field (Core A13-1): `None` is `TooLarge`; an empty list clears the destination.
    ClipboardWrite {
        selection: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contents: Option<Vec<ClipboardContent>>,
        total_bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<ClipboardReason>,
    },
    /// The worker dropped something that it could not carry (a long query, tap bytes): the loss marker of EV-2.
    Lost {
        kind: LostKind,
        #[serde(default)]
        tap_dropped_bytes: u64,
    },
}

/// A report of a session worker to the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkerMsg {
    /// The payload runs. `features` are the worker's own (A2-6), and `terminal` is the state that `terminal_state` caches
    /// from now on (ST-4).
    Launched {
        features: BTreeSet<Feature>,
        terminal: TerminalState,
        /// The snapshot formats that the worker can emit (`snapshot_formats`, ST-6).
        #[serde(default)]
        formats: Vec<SnapshotFormat>,
        /// The payload's identity, so that the host can end its group when the link is gone (LC-5). It is required: a worker
        /// that cannot name its payload cannot be controlled without its link, and the host refuses such a launch.
        payload: PayloadId,
    },
    /// The payload did not start (LC-4).
    LaunchFailed { reason: StartFailReason },
    /// The payload ended (EV-4). The host decides the `cause` (LC-5), because it knows what it asked for.
    Exited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signal: Option<i32>,
    },
    /// The answer to `HostMsg::Op` with this request number.
    Done { req: u64, result: OpResult },
    /// The pages of the snapshot that answers request `req` (ST-6). A `Done` with `Capture` follows. The host mints the
    /// `CaptureId` and keeps the pages.
    Pages { req: u64, pages: Vec<Page> },
    /// A change that the worker saw. The worker sends none before `Launched` (every observation comes from the payload's
    /// output, its input or its resize). The host reads none while the session's start is not through: the frame stays unread
    /// on the link, so the observations keep their order and follow `Running` (OR-2, ST-4).
    Observed { observation: Observation },
    /// The worker holds its own copy of the route's stream: its `AttachRoute` frame bound the descriptor that came with it
    /// (DP-2, steward ruling R-65). The host closes its copy only now: a copy in flight is not the worker's own.
    RouteReceived { route: RouteId },
    /// A route reached `Closed` (OU-2): the host posts `RouteClosed` and frees the route.
    RouteClosed {
        route: RouteId,
        reason: RouteCloseReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_tag: Option<String>,
    },
    /// The route became `Stalled` at `mark`, in answer to [`HostMsg::RouteStallDue`] (OU-2, R-90).
    RouteStalled {
        route: RouteId,
        #[serde(default)]
        mark: u64,
    },
    /// The route is `Open` again at `mark` (OU-2): the transport accepted bytes, and the resync is queued.
    RouteResumed {
        route: RouteId,
        #[serde(default)]
        mark: u64,
    },
    /// The progress report of a route (R-90, OU-3a). `mark` counts the writes of the route that the transport accepted
    /// bytes of. `waiting` is true while the route is `Open` and holds bytes that the transport has not accepted. The host
    /// measures the route's `stall_deadline` from the report that it consumes with `waiting: true`.
    RouteProgress {
        route: RouteId,
        mark: u64,
        waiting: bool,
    },
    /// The complete result of the upload cleanup (A6-3): after it the worker ends.
    RemoveResult { uploads: UploadsOutcome },
    /// The worker's live state, sent once right after its hello in an adoption (AD-1, AD-3; steward ruling R-35). Every value
    /// is read from the worker's live state, never remembered from the spawn.
    Adopted { report: Box<AdoptReport> },
    /// One live route of an adopted worker (DP-8), one frame each, right after `Adopted` (`AdoptReport.routes` of them). The
    /// worker names no route of this link before the host's [`HostMsg::RouteId`] for its `key`.
    RouteLive(Box<RouteLive>),
}

/// What an adopted worker reports about itself (AD-1, AD-3, DP-12, R-35).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptReport {
    pub payload: AdoptedPayload,
    /// The number of live routes (OU-2 `Open` or `Stalled`; steward ruling R-99) that follow as [`WorkerMsg::RouteLive`]
    /// (DP-8). Required in worker protocol 1 (steward ruling R-98).
    pub routes: u32,
    /// The worker's own features (A2-6; AD-4: the `worker_features` of a worker of protocol N - 1).
    pub features: BTreeSet<Feature>,
    /// The terminal state, with the focus and the model and input revisions (ST-1, IN-10, DP-12). Present once the payload
    /// was launched; absent while no payload ever ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalState>,
    /// The snapshot formats that the worker can emit (ST-6).
    #[serde(default)]
    pub formats: Vec<SnapshotFormat>,
}

/// A live route of an adopted worker (DP-8): the stable tags of its attach and what the new host's stall clock needs. One
/// route per frame, so the route count never affects a frame's size; it holds a subset of the route's `AttachRoute` frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteLive {
    /// The worker's key of the route: an opaque token of this link, never a `RouteId`.
    pub key: u64,
    /// Absent when the attach gave none, as in `AttachOptions`, so the shared fields encode alike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub client: ClientId,
    /// The limits that the old host applied at attach (OU-1).
    pub limits: AppliedRouteLimits,
    /// `Some(mark)` while the route is `Stalled` at `mark` (OU-2); None while it is `Open`.
    pub stalled: Option<u64>,
}

/// The payload of an adopted worker (R-35: the host's recovery depends on it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AdoptedPayload {
    /// No `Launch` came (a crash between AD-7 steps 3 and 4).
    NotLaunched,
    /// A `Launch` came and its spawn has not answered yet.
    Spawning,
    /// The payload runs.
    Running { payload: PayloadId },
    /// The payload ended.
    Exited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signal: Option<i32>,
    },
    /// The spawn failed (LC-4).
    LaunchFailed { reason: StartFailReason },
}

/// Why a message is not a message of this wire: the reason that the decoder gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgError(String);

impl MsgError {
    fn from_decoder(error: &serde_json::Error) -> MsgError {
        MsgError(error.to_string())
    }
}

impl std::fmt::Display for MsgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the payload is not a message of the control link: {}",
            self.0
        )
    }
}

impl std::error::Error for MsgError {}

impl HostMsg {
    pub fn encode(&self, out: &mut Vec<u8>) {
        serde_json::to_writer(out, self).expect("a host message is JSON");
    }

    pub fn decode(payload: &[u8]) -> Result<HostMsg, MsgError> {
        serde_json::from_slice(payload).map_err(|e| MsgError::from_decoder(&e))
    }

    /// The length of [`HostMsg::encode`]'s output: the frame payload of this message. It encodes into a counter, so no buffer
    /// of the encoded output is kept (steward ruling R-100: a call whose message can never fit the link frame is refused at
    /// the call). The serializers can still allocate on their own (`HexBytes` makes its hex string).
    pub fn encoded_len(&self) -> u64 {
        json_len(self)
    }
}

/// The length of `value`'s JSON form, as a host message carries it. It encodes into a counter and keeps no byte.
pub fn json_len<T: Serialize + ?Sized>(value: &T) -> u64 {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).expect("a host message's value is JSON");
    counter.0
}

/// A writer that counts the bytes that it is given.
struct Counter(u64);

impl std::io::Write for Counter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl WorkerMsg {
    pub fn encode(&self, out: &mut Vec<u8>) {
        serde_json::to_writer(out, self).expect("a worker message is JSON");
    }

    pub fn decode(payload: &[u8]) -> Result<WorkerMsg, MsgError> {
        serde_json::from_slice(payload).map_err(|e| MsgError::from_decoder(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_largest_cursor_ordinal_fills_its_bits() {
        assert_eq!(MAX_CURSOR_ORDINAL, (1 << CURSOR_ORDINAL_BITS) - 1);
        assert_eq!(MAX_CURSOR_ORDINAL, 16_777_215);
    }

    fn limits(paste: u64, snapshot: u64, events: u64, tail: u32, tag: u32) -> CoreLimits {
        CoreLimits {
            max_paste_bytes: paste,
            max_snapshot_bytes: snapshot,
            input_events_bytes: events,
            input_tail_bytes: tail,
            max_route_tag_bytes: tag,
            ..CoreLimits::default()
        }
    }

    #[test]
    fn the_link_frame_cap_is_half_of_u32() {
        assert_eq!(LINK_FRAME_CAP, u32::MAX / 2);
    }

    /// #320 R320-SI-5: the record bound, four protected entries with their tags at six bytes a byte, and the tail as
    /// hex.
    #[test]
    fn the_facts_reply_bound_counts_records_protected_entries_and_the_tail() {
        assert_eq!(facts_reply_bound(&limits(0, 0, 0, 0, 0)), 4 * 1024);
        assert_eq!(
            facts_reply_bound(&limits(0, 0, 1000, 10, 1)),
            1000 + 4 * (1024 + 12) + 20
        );
        let largest = limits(0, 0, u64::MAX, u32::MAX, u32::MAX);
        assert_eq!(facts_reply_bound(&largest), u64::MAX);
    }

    /// Plan section 3, #320 R320-SI-5: the bound is the larger of six times the largest payload and the largest facts
    /// reply, with 1 MiB more, and at most `LINK_FRAME_CAP`.
    #[test]
    fn the_link_frame_bound_holds_the_largest_payload_and_facts_reply() {
        let mib = 1 << 20;
        assert_eq!(link_frame_bound(&limits(1000, 10, 0, 0, 0)), 6000 + mib);
        assert_eq!(link_frame_bound(&limits(10, 1000, 0, 0, 0)), 6000 + mib);
        assert_eq!(
            link_frame_bound(&limits(10, 10, 100_000, 0, 0)),
            100_000 + 4096 + mib
        );
        assert_eq!(
            link_frame_bound(&limits(1 << 20, 1024, 16 << 10, 4 << 20, 256)),
            (16 << 10) + 4 * (1024 + 12 * 256) + (8 << 20) + mib,
            "Sol's case: the facts reply binds, not the paste"
        );
        let at_cap = u64::from(LINK_FRAME_CAP) - u64::from(mib) - 4096;
        assert_eq!(
            link_frame_bound(&limits(0, 0, at_cap, 0, 0)),
            LINK_FRAME_CAP
        );
        assert_eq!(
            link_frame_bound(&limits(0, 0, at_cap + 1, 0, 0)),
            LINK_FRAME_CAP
        );
        assert_eq!(
            link_frame_bound(&limits(500_000_000, 0, 0, 0, 0)),
            LINK_FRAME_CAP,
            "below u32::MAX, above the cap"
        );
        assert_eq!(
            link_frame_bound(&limits(u64::MAX, 0, 0, 0, 0)),
            LINK_FRAME_CAP
        );
    }

    fn size() -> Size {
        Size {
            rows: 24,
            cols: 80,
            cell_px: None,
        }
    }

    #[test]
    fn a_launch_round_trips_with_its_defaults() {
        let msg = HostMsg::Launch(Box::new(LaunchSpec {
            argv: vec!["/bin/sh".into()],
            env: BTreeMap::from([("A".to_string(), "1".to_string())]),
            cwd: "/".into(),
            size: size(),
            color_profile: None,
            notification_policy: NotificationPolicy::All,
            size_policy: SizePolicy::Latest,
            link_frame_bound: 1 << 20,
            limits: CoreLimits {
                clipboard_bytes: 77,
                ..CoreLimits::default()
            },
            cursor_ordinal: 9,
        }));
        let mut bytes = Vec::new();
        msg.encode(&mut bytes);
        assert_eq!(HostMsg::decode(&bytes), Ok(msg));
    }

    /// An absent `limits` field is the defaults of the Core limits table.
    #[test]
    fn an_absent_limits_field_is_the_default_of_the_limits_table() {
        let text = r#"{"t":"launch","argv":["/bin/sh"],"env":{},"cwd":"/","size":{"rows":24,"cols":80},"link_frame_bound":1024,"cursor_ordinal":1}"#;
        let Ok(HostMsg::Launch(spec)) = HostMsg::decode(text.as_bytes()) else {
            panic!("a launch");
        };
        assert_eq!(
            spec.limits,
            CoreLimits::default(),
            "an absent limits field is the table's defaults"
        );
    }

    /// The removed `stop_grace_ms` (steward ruling R-97, follow-up) changes no adoption across versions (A6-2). An older host's
    /// launch, which carries it, still decodes: `LaunchSpec` takes unknown fields. A launch of this host has no such key, and
    /// an older worker read it with `#[serde(default)]` as the table's default.
    #[test]
    fn the_removed_stop_grace_field_decodes_in_both_directions() {
        let older = r#"{"t":"launch","argv":["/bin/sh"],"env":{},"cwd":"/","size":{"rows":24,"cols":80},"link_frame_bound":1024,"stop_grace_ms":250,"cursor_ordinal":1}"#;
        let Ok(HostMsg::Launch(spec)) = HostMsg::decode(older.as_bytes()) else {
            panic!("an older host's launch decodes");
        };
        assert_eq!(spec.link_frame_bound, 1024);
        let mut bytes = Vec::new();
        HostMsg::Launch(spec).encode(&mut bytes);
        let text = String::from_utf8(bytes).expect("a launch is text");
        assert!(text.contains(r#""t":"launch""#), "{text}");
        assert!(!text.contains("stop_grace_ms"), "{text}");
    }

    /// The cursor ordinal is required (steward rulings R-101 Q6 and R-98): a launch without it does not decode.
    #[test]
    fn a_launch_without_its_cursor_ordinal_does_not_decode() {
        let text = r#"{"t":"launch","argv":["/bin/sh"],"env":{},"cwd":"/","size":{"rows":24,"cols":80},"link_frame_bound":1024}"#;
        assert!(HostMsg::decode(text.as_bytes()).is_err());
    }

    #[test]
    fn an_op_travels_in_its_contract_form() {
        let msg = HostMsg::Op {
            req: 7,
            op: Op::ReadCursor {
                session: SessionId("s".into()),
            },
        };
        let mut bytes = Vec::new();
        msg.encode(&mut bytes);
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            text.contains(r#""t":"op""#) && text.contains("ReadCursor"),
            "{text}"
        );
        assert_eq!(HostMsg::decode(&bytes), Ok(msg));
    }

    /// R-100: the counted length is the encoded length, also for text that JSON escapes (a control byte takes six).
    #[test]
    fn the_encoded_len_is_the_length_of_the_encoding() {
        let msgs = [
            HostMsg::Stop,
            HostMsg::Op {
                req: u64::MAX,
                op: Op::CaptureSnapshot {
                    session: SessionId("s".into()),
                    owner: ClientId("o\u{1}\"\\é".into()),
                },
            },
            HostMsg::Launch(Box::new(LaunchSpec {
                argv: vec!["/bin/sh".into(), "\u{7}".repeat(100)],
                env: BTreeMap::new(),
                cwd: "/".into(),
                size: size(),
                color_profile: None,
                notification_policy: NotificationPolicy::All,
                size_policy: SizePolicy::Latest,
                link_frame_bound: 1 << 20,
                limits: CoreLimits::default(),
                cursor_ordinal: MAX_CURSOR_ORDINAL,
            })),
        ];
        for msg in msgs {
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(msg.encoded_len(), bytes.len() as u64, "{msg:?}");
        }
    }

    #[test]
    fn a_report_round_trips() {
        let msgs = [
            WorkerMsg::Exited {
                code: None,
                signal: Some(9),
            },
            WorkerMsg::LaunchFailed {
                reason: StartFailReason::CwdMissing,
            },
            WorkerMsg::Done {
                req: 3,
                result: OpResult::Ok(OpOutput::Unit),
            },
            WorkerMsg::Observed {
                observation: Observation::Bell,
            },
            WorkerMsg::RouteClosed {
                route: RouteId(2),
                reason: RouteCloseReason::Detached,
                route_tag: Some("t".into()),
            },
            WorkerMsg::Observed {
                observation: Observation::Lost {
                    kind: LostKind::Tap,
                    tap_dropped_bytes: 4,
                },
            },
            WorkerMsg::RemoveResult {
                uploads: UploadsOutcome::Deleted,
            },
            WorkerMsg::RouteProgress {
                route: RouteId(4),
                mark: 9,
                waiting: true,
            },
            WorkerMsg::RouteStalled {
                route: RouteId(4),
                mark: 9,
            },
            WorkerMsg::RouteResumed {
                route: RouteId(4),
                mark: 10,
            },
        ];
        let adopted = [
            AdoptedPayload::NotLaunched,
            AdoptedPayload::Spawning,
            AdoptedPayload::Running {
                payload: PayloadId {
                    pid: 41,
                    start_time: 7,
                },
            },
            AdoptedPayload::Exited {
                code: Some(3),
                signal: None,
            },
            AdoptedPayload::LaunchFailed {
                reason: StartFailReason::CwdMissing,
            },
        ]
        .map(|payload| WorkerMsg::Adopted {
            report: Box::new(AdoptReport {
                payload,
                routes: 2,
                features: BTreeSet::from([Feature::FocusReport]),
                terminal: None,
                formats: vec![],
            }),
        });
        let msgs = msgs.into_iter().chain(adopted);
        for msg in msgs {
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(WorkerMsg::decode(&bytes), Ok(msg.clone()), "{msg:?}");
        }
    }

    /// R-90: the two due messages round-trip, and a stall report of an older worker (no `mark`) reads as mark 0.
    #[test]
    fn the_stall_messages_round_trip_and_an_absent_mark_is_zero() {
        for msg in [
            HostMsg::RouteStallDue {
                route: RouteId(1),
                mark: 3,
            },
            HostMsg::RouteCloseDue {
                route: RouteId(1),
                mark: 3,
            },
        ] {
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(HostMsg::decode(&bytes), Ok(msg.clone()), "{msg:?}");
        }
        assert_eq!(
            WorkerMsg::decode(br#"{"t":"route_stalled","route":1}"#),
            Ok(WorkerMsg::RouteStalled {
                route: RouteId(1),
                mark: 0
            })
        );
    }

    fn route_limits(n: u64) -> AppliedRouteLimits {
        let d = Duration::from_millis(n);
        AppliedRouteLimits {
            max_frame_bytes: n,
            max_screen_frame_bytes: n,
            max_history_page_bytes: n,
            max_paste_bytes: n,
            max_query_bytes: n,
            max_query_reply_bytes: n,
            max_file_bytes: n,
            query_deadline: d,
            stall_deadline: d,
            stall_close_after: d,
            route_input_queue_bytes: n,
        }
    }

    fn route_live(tags: Option<&str>, stalled: Option<u64>) -> RouteLive {
        RouteLive {
            key: u64::MAX,
            route_tag: tags.map(String::from),
            owner: tags.map(String::from),
            client: ClientId(tags.unwrap_or("c").into()),
            limits: route_limits(u64::MAX),
            stalled,
        }
    }

    /// DP-8: the route adoption messages and `AttachRoute`'s client round-trip, with the tags absent and present.
    #[test]
    fn the_route_adoption_messages_round_trip() {
        for live in [route_live(None, None), route_live(Some("t"), Some(7))] {
            let msg = WorkerMsg::RouteLive(Box::new(live));
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(WorkerMsg::decode(&bytes), Ok(msg.clone()), "{msg:?}");
        }
        let id = HostMsg::RouteId {
            key: 3,
            route: RouteId(9),
        };
        let attach = attach_route(Some("t"));
        for msg in [id, attach] {
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(HostMsg::decode(&bytes), Ok(msg.clone()), "{msg:?}");
        }
    }

    /// Steward ruling R-98: DP-8's fields are part of worker protocol 1, with no default for an older peer. A message that
    /// lacks a required one does not decode.
    #[test]
    fn a_message_without_a_required_dp_8_field_does_not_decode() {
        let mut report = Vec::new();
        WorkerMsg::Adopted {
            report: Box::new(AdoptReport {
                payload: AdoptedPayload::NotLaunched,
                routes: 0,
                features: BTreeSet::new(),
                terminal: None,
                formats: vec![],
            }),
        }
        .encode(&mut report);
        let mut live = Vec::new();
        WorkerMsg::RouteLive(Box::new(route_live(Some("t"), None))).encode(&mut live);
        let mut attach = Vec::new();
        attach_route(None).encode(&mut attach);
        let mut id = Vec::new();
        HostMsg::RouteId {
            key: 1,
            route: RouteId(1),
        }
        .encode(&mut id);
        let without = |bytes: &[u8], field: &str| {
            let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let object = match value.get_mut("report") {
                Some(report) => report.as_object_mut().unwrap(),
                None => value.as_object_mut().unwrap(),
            };
            assert!(object.remove(field).is_some(), "{field}");
            serde_json::to_vec(&value).unwrap()
        };
        assert!(WorkerMsg::decode(&without(&report, "routes")).is_err());
        for field in ["key", "client", "limits"] {
            assert!(
                WorkerMsg::decode(&without(&live, field)).is_err(),
                "{field}"
            );
        }
        assert!(HostMsg::decode(&without(&attach, "client")).is_err());
        for field in ["key", "route"] {
            assert!(HostMsg::decode(&without(&id, field)).is_err(), "{field}");
        }
    }

    /// The smallest `AttachRoute` of a route: a one-digit id, and only the options that are always encoded, each at its
    /// shortest (`file_directory` is an absolute path, so at least `/`).
    fn attach_route(tags: Option<&str>) -> HostMsg {
        HostMsg::AttachRoute {
            route: RouteId(1),
            options: AttachOptions {
                file_directory: "/".into(),
                file_permissions: None,
                route_features: vec![],
                terminal_formats: vec![],
                owner: tags.map(String::from),
                query_deadline: None,
                route_tag: tags.map(String::from),
                route_limits: None,
                history: None,
                stall_deadline: None,
                answers_queries: true,
                input: true,
            },
            limits: route_limits(u64::MAX),
            client: ClientId(tags.unwrap_or("c").into()),
        }
    }

    /// DP-8, 0181's STDP8-D3: a route's `RouteLive`, with its own integers at their widest, is smaller than the smallest
    /// `AttachRoute` of the same route, with the tags absent and present. The worker decoded that `AttachRoute` within its
    /// `frame_bound`, and it sends with the same bound, so the route's `RouteLive` always fits.
    #[test]
    fn a_route_live_frame_is_smaller_than_its_attach_route_frame() {
        for tags in [None, Some("t"), Some(&*"x".repeat(4096))] {
            let mut live = Vec::new();
            WorkerMsg::RouteLive(Box::new(route_live(tags, Some(u64::MAX)))).encode(&mut live);
            let mut attach = Vec::new();
            attach_route(tags).encode(&mut attach);
            assert!(
                live.len() < attach.len(),
                "{} >= {}",
                live.len(),
                attach.len()
            );
        }
    }

    /// Plan section 3: an unknown field of a known message is ignored, so a newer worker may add fields.
    #[test]
    fn an_unknown_field_is_ignored() {
        let msg = WorkerMsg::decode(br#"{"t":"exited","code":0,"later":[1]}"#);
        assert_eq!(
            msg,
            Ok(WorkerMsg::Exited {
                code: Some(0),
                signal: None
            })
        );
    }

    /// Plan section 3: garbage and an unknown tag are refused, and an unknown field of a known message is ignored (additive
    /// fields keep N - 1 compatible). A refusal keeps the reason that the decoder gave (audit A28).
    #[test]
    fn garbage_and_an_unknown_tag_are_refused_with_the_decoder_reason() {
        for payload in [&b"nope"[..], br#"{"t":"later"}"#] {
            let reason = serde_json::from_slice::<WorkerMsg>(payload)
                .unwrap_err()
                .to_string();
            let error = WorkerMsg::decode(payload).unwrap_err();
            assert!(error.to_string().ends_with(&reason), "{error}");
        }
        assert_eq!(HostMsg::decode(br#"{"t":"stop","x":1}"#), Ok(HostMsg::Stop));
    }
}

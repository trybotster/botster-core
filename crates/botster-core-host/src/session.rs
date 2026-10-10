//! The session table: the registry row (AD-6, AD-7) and what the engine keeps for a live session.
//!
//! A session has two states. The **admission** state moves at `begin`, in `begin` order (AM-1), so that two `Start` before a
//! `pump` are told apart. The **shown** state is what `get` and the events report, and it moves only when its event is posted
//! (OR-1, OR-2, EV-5b).

use botster_core_contract::prelude::*;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_link::hello::PROOF_LEN;
use botster_core_link::proof::{token_hex, TOKEN_LEN};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::io::{LinkId, Ticket};

/// The key of the registry row of a session. The `Storage` edge maps a key to its own file name.
pub fn row_key(id: &SessionId) -> String {
    format!("session/{}", id.0)
}

/// The prefix of every session row key.
pub const ROW_PREFIX: &str = "session/";

/// How a session ended: one of the two ends that Core reaches. A type of this crate, so that every match on it is total and
/// no fallback is needed: Core never posts `Lost(Other)` (AD-2; steward ruling R-35, correction `c3ed727`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Exited(Exit),
    Lost(LostReason),
}

impl End {
    /// The state that this end shows.
    pub fn state(self) -> SessionState {
        match self {
            End::Exited(exit) => SessionState::Exited(exit),
            End::Lost(reason) => SessionState::Lost(reason),
        }
    }

    /// The end as an operation result gives it (A2-1).
    pub fn public(self) -> SessionEnd {
        match self {
            End::Exited(exit) => SessionEnd::Exited(exit),
            End::Lost(reason) => SessionEnd::Lost(reason),
        }
    }

    /// The end that a shown state names, or `None` for a state that is not an end.
    pub fn of(state: SessionState) -> Option<End> {
        match state {
            SessionState::Exited(exit) => Some(End::Exited(exit)),
            SessionState::Lost(reason) => Some(End::Lost(reason)),
            _ => None,
        }
    }
}

/// The state of a session as the admission table sees it (AM-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admit {
    Created,
    Starting,
    Running,
    Stopping,
    Exited,
    Lost,
    /// `Remove` was admitted. The id is still in use until step 5 of LC-7.
    Removing,
    /// An adoption runs (AD-1, AD-2 retry). Until it posts the row's state, no operation is admitted on the session.
    Adopting,
}

/// The durable row of a session: one JSON object per row, version 1.
///
/// Clause: Core AD-6, Core AD-7, Core LC-9, Core A3-1 (the notification policy of a `Created` row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub version: u32,
    pub id: SessionId,
    pub instance: InstanceId,
    pub state: SessionState,
    pub request: SpawnRequest,
    pub labels: BTreeMap<String, String>,
    /// The per-worker token, 64 lowercase hex digits. The row is readable only by the host's uid (AD-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<RowWorker>,
    /// The payload's identity, once the worker launched it (LC-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<RowWorker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_protocol: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_features: Option<BTreeSet<Feature>>,
}

/// The identity of the worker process: its pid and its start time (AD-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowWorker {
    pub pid: u32,
    pub start_time: u64,
}

impl RowWorker {
    pub fn identity(self) -> ProcessIdentity {
        ProcessIdentity {
            pid: self.pid,
            start_time: self.start_time,
        }
    }
}

impl From<ProcessIdentity> for RowWorker {
    fn from(identity: ProcessIdentity) -> RowWorker {
        RowWorker {
            pid: identity.pid,
            start_time: identity.start_time,
        }
    }
}

pub const ROW_VERSION: u32 = 1;

impl Row {
    /// The row of `id` that `bytes` hold, or `None` when Core's decoder rejects them: bytes that are not a row, a row of
    /// another version, or a row of another id (A10-2, AD-2 `RegistryCorrupt`).
    pub fn decode(id: &SessionId, bytes: &[u8]) -> Option<Row> {
        serde_json::from_slice::<Row>(bytes)
            .ok()
            .filter(|row| row.version == ROW_VERSION && &row.id == id)
    }
}

/// The request of a session whose row is corrupt: Core cannot read what was asked. Its size of 0 rows by 0 columns is
/// outside every valid size (A2-1), so a reader of the record cannot take it for a real one, and no worker ever gets it: a
/// `Lost` session is never started (AD-2).
pub fn unknown_request() -> SpawnRequest {
    SpawnRequest {
        argv: Vec::new(),
        env: BTreeMap::new(),
        cwd: String::new(),
        size: Size {
            rows: 0,
            cols: 0,
            cell_px: None,
        },
        labels: BTreeMap::new(),
        color_profile: None,
        notification_policy: None,
        size_policy: None,
    }
}

/// A set of `OpId`s as disjoint ranges. `cancel` needs exact identity for the whole life of the handle (ID-1, IN-6), and ranges
/// keep that exact set small.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdRanges {
    /// Start to inclusive end.
    ranges: std::collections::BTreeMap<u64, u64>,
}

impl IdRanges {
    pub fn insert(&mut self, id: u64) {
        let (mut start, mut end) = (id, id);
        if let Some((&s, &e)) = self.ranges.range(..=id).next_back() {
            if e >= id {
                return;
            }
            if e + 1 == id {
                start = s;
            }
        }
        if let Some(&e) = self.ranges.get(&(id + 1)) {
            end = e;
            self.ranges.remove(&(id + 1));
        }
        self.ranges.insert(start, end);
    }

    pub fn contains(&self, id: u64) -> bool {
        self.ranges
            .range(..=id)
            .next_back()
            .is_some_and(|(_, &e)| e >= id)
    }

    pub fn extend(&mut self, other: &IdRanges) {
        for (&s, &e) in &other.ranges {
            for id in s..=e {
                self.insert(id);
            }
        }
    }
}

// A proof is the same size as a token; the two never mix because they have different types.
const _: () = assert!(PROOF_LEN == TOKEN_LEN);

/// What a session's silence threshold needs (TM-4).
#[derive(Debug, Clone, Default)]
pub struct Silence {
    pub threshold: Option<Duration>,
    /// The idle start (Core A18-2): the monotonic time and the unix time of the pump in which this host observed the latest
    /// output, or of the adoption point for an adopted session with no output under this host.
    pub idle_start: Option<(Instant, UnixSeconds)>,
    /// This host observed output of the session, so the idle start is that output's, never an adoption point (A18-2).
    pub output_seen: bool,
    /// `Silent` was posted for this idle period (TM-4: once per period).
    pub fired: bool,
}

impl Silence {
    /// The instant at which `Silent` is due, or `None` (TM-3).
    pub fn deadline(&self) -> Option<Instant> {
        match (self.threshold, self.idle_start) {
            (Some(threshold), Some((at, _))) if !self.fired => Some(at + threshold),
            _ => None,
        }
    }
}

/// How a worker that the host spawned or adopted is reached.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkerHandle {
    pub identity: Option<ProcessIdentity>,
    pub link: Option<LinkId>,
    /// The link existed and ended: ops on it fail with `WorkerLinkFailed` (A2-1).
    pub link_failed: bool,
    /// The worker process ended (the exit watch reported it), or the AD-6 check refused the process at its endpoint (AD-2
    /// `WorkerGone`). The host never signals a gone worker.
    pub gone: bool,
}

/// A live session: the row, the two states, and the machines that work on it.
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub instance: InstanceId,
    pub admit: Admit,
    /// What `get` and the events report. `None` until `SessionState{Created}` is posted.
    pub shown: Option<SessionState>,
    pub request: SpawnRequest,
    pub size: Size,
    pub labels: BTreeMap<String, String>,
    pub exit: Option<Exit>,
    /// The worker protocol that this host learned from the worker's hello, or from the row when no adoption has run (LC-9).
    pub worker_protocol: Option<u8>,
    /// The protocol of the row while an adoption has not read a hello: the row keeps it (R-36: a `Lost` row records only
    /// its end), and `get` never shows it (LC-9: Core never guesses a protocol that it did not learn).
    pub row_protocol: Option<u8>,
    pub worker_features: Option<BTreeSet<Feature>>,
    pub token: Option<[u8; TOKEN_LEN]>,
    pub worker: WorkerHandle,
    /// The payload's identity, from the launch report (LC-5, LC-6).
    pub payload: Option<ProcessIdentity>,
    pub terminal: Option<TerminalState>,
    pub silence: Silence,
    /// The host asked for the end of the payload, or signalled it (decides the `ExitCause`, LC-5, LC-6).
    pub host_ended: bool,
    /// The kill of `stop_grace` was sent (LC-5).
    pub killed: bool,
    /// A `Stop` or `StopAll` reached a `Starting` session: the stop begins when the payload runs (LC-12).
    pub stop_after_start: bool,
    /// Ops that wait for the end of the running flow (a joined `Stop`).
    pub waiters: Vec<OpId>,
    /// Requests that were sent to the worker and wait for `Done`: request number to op.
    pub inflight: BTreeMap<u64, OpId>,
    /// Input ops in flight, and the payload bytes that they hold (IN-5).
    pub input_ops: u32,
    pub input_bytes: u64,
    pub routes: BTreeSet<RouteId>,
    /// The routes whose queue the worker delivered after the session ended, with their `route_closed{session_ended}`: the
    /// worker reported the close (OU-7). `StopPhase::Finish` closes them `SessionEnded`. While the worker's link lives, a
    /// bound route that is not here keeps `Finish` waiting; once the link is gone, it closes `SessionLost`.
    pub delivered: BTreeSet<RouteId>,
    /// The routes that were bound when the session ended. Only these close in `StopPhase::Finish`; a route that attaches
    /// after the exit stays bound until `Remove` closes it `SessionRemoved` (LC-7 step 1, A2-3).
    pub bound_at_end: BTreeSet<RouteId>,
    pub flow: crate::flow::Flow,
    /// Flows that were admitted while another one runs: `Start` or `Remove` after a `Create` that a `pump` has not finished
    /// (AM-1). The next one begins when the running one ends.
    pub queue: std::collections::VecDeque<crate::flow::Flow>,
    /// The tickets that this session's flow waits for.
    pub ticket: Option<Ticket>,
    /// The snapshot formats that the worker reported at launch (ST-6).
    pub formats: Vec<SnapshotFormat>,
    /// The ops that acted on this instance, so that `cancel` can tell an op of a removed instance (ID-1).
    pub ops: IdRanges,
    /// Admitted setters of a `Created` session that have not run: a start waits for them (AM-1 order).
    pub pending_setters: u32,
    /// `MetadataChanged` is due: it follows the completion of an `UpdateMetadata` in a step of its own (LC-9).
    pub metadata_pending: bool,
    /// How the payload ended while a start flow was still running: applied when the flow ends.
    pub pending_end: Option<End>,
    /// The `AdoptAll` or `Adopt` op that waits for this session's state (LC-11).
    pub adopting: Option<OpId>,
}

impl Session {
    /// The `SessionRecord` that `get` returns, from the shown state.
    pub fn record(&self) -> Option<SessionRecord> {
        let state = self.shown?;
        Some(SessionRecord {
            id: self.id.clone(),
            state,
            size: self.size,
            labels: self.labels.clone(),
            exit: self.exit,
            worker_protocol: self.worker_protocol,
            worker_features: self.worker_features.clone(),
        })
    }

    /// The state that a row write of this session records: the shown state. A `Lost` row keeps the worker's identity, not
    /// an earlier state (steward ruling R-36, contracts `main` `c62085f`).
    pub fn recorded_state(&self) -> SessionState {
        self.shown.unwrap_or(SessionState::Created)
    }

    pub fn to_row(&self) -> Row {
        Row {
            version: ROW_VERSION,
            id: self.id.clone(),
            instance: self.instance.clone(),
            state: self.recorded_state(),
            request: self.request.clone(),
            labels: self.labels.clone(),
            token: self.token.as_ref().map(token_hex),
            worker: self.worker.identity.map(RowWorker::from),
            payload: self.payload.map(RowWorker::from),
            worker_protocol: self.worker_protocol.or(self.row_protocol),
            worker_features: self.worker_features.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silence_deadline_needs_a_threshold_an_output_and_an_unfired_period() {
        let now = crate::tests::real_now();
        let mut silence = Silence::default();
        assert_eq!(silence.deadline(), None);
        silence.threshold = Some(Duration::from_secs(2));
        assert_eq!(silence.deadline(), None);
        silence.idle_start = Some((now, 5));
        assert_eq!(silence.deadline(), Some(now + Duration::from_secs(2)));
        silence.fired = true;
        assert_eq!(silence.deadline(), None);
    }

    #[test]
    fn a_row_round_trips_through_json() {
        let row = Row {
            version: ROW_VERSION,
            id: SessionId("s".into()),
            instance: InstanceId("1-1".into()),
            state: SessionState::Created,
            request: SpawnRequest {
                argv: vec!["/bin/true".into()],
                env: BTreeMap::new(),
                cwd: "/".into(),
                size: Size {
                    rows: 24,
                    cols: 80,
                    cell_px: None,
                },
                labels: BTreeMap::new(),
                color_profile: None,
                notification_policy: None,
                size_policy: None,
            },
            labels: BTreeMap::from([("k".to_string(), "v".to_string())]),
            token: Some("00".repeat(32)),
            worker: Some(RowWorker {
                pid: 4,
                start_time: 9,
            }),
            payload: None,
            worker_protocol: Some(1),
            worker_features: None,
        };
        let bytes = serde_json::to_vec(&row).unwrap();
        assert_eq!(serde_json::from_slice::<Row>(&bytes).unwrap(), row);
    }

    /// Core ID-1, IN-6: the op identity of a session is the exact set of its ids, whatever the order of the inserts: a gap
    /// stays a gap, and ranges that touch merge.
    #[test]
    fn id_ranges_keep_the_exact_set_in_any_order() {
        let mut ranges = IdRanges::default();
        for id in [5, 3, 4, 9, 1] {
            ranges.insert(id);
        }
        for id in 0..12u64 {
            assert_eq!(ranges.contains(id), [1, 3, 4, 5, 9].contains(&id), "{id}");
        }
        assert_eq!(
            ranges.ranges.len(),
            3,
            "3..=5 is one range: {:?}",
            ranges.ranges
        );
        ranges.insert(4);
        ranges.insert(2);
        assert_eq!(ranges.ranges.len(), 2, "1..=5 merged: {:?}", ranges.ranges);
        assert_eq!(ranges.ranges.get(&1), Some(&5));
        ranges.insert(6);
        ranges.insert(8);
        assert_eq!(ranges.ranges.get(&1), Some(&6));
        ranges.insert(7);
        assert_eq!(ranges.ranges.len(), 1);
        assert_eq!(ranges.ranges.get(&1), Some(&9));
        assert!(!ranges.contains(0) && !ranges.contains(10));
    }
}

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
    /// `CoreLimits.stop_grace` in milliseconds. The worker times the kill of its payload's group with it when the host asks
    /// without its link (`GroupSignal::EndPayload`, LC-5); on the linked path the host sends `Kill` itself. Absent: the
    /// default of the Core limits table.
    #[serde(default = "default_stop_grace_ms")]
    pub stop_grace_ms: u64,
}

fn default_stop_grace_ms() -> u64 {
    u64::try_from(CoreLimits::default().stop_grace.as_millis()).unwrap_or(u64::MAX)
}

/// A request of the host to a session worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
#[non_exhaustive]
pub enum HostMsg {
    /// Launches the payload (AD-7 step 4). The worker answers [`WorkerMsg::Launched`] or [`WorkerMsg::LaunchFailed`].
    Launch(Box<LaunchSpec>),
    /// The graceful request to end the payload (LC-5). The worker answers [`WorkerMsg::Exited`] when it has ended.
    Stop,
    /// The kill of the payload's process group after `stop_grace` (LC-5).
    Kill,
    /// An operation that needs the worker's model or input path (the reads, `WriteInput`, the setters). `req` is the host's
    /// request number, unique on this link. The worker answers with [`WorkerMsg::Done`].
    Op { req: u64, op: Op },
    /// A cancel of the `WriteInput` that was sent as request `req` (IN-6).
    Cancel { req: u64 },
    /// LC-7 step 3: delete the uploaded files, answer [`WorkerMsg::RemoveResult`], and end.
    Remove,
    /// A route is registered (OU-1). The route's connected stream follows by descriptor handoff (DP-2); P4a owns the rest.
    AttachRoute {
        route: RouteId,
        options: AttachOptions,
    },
    /// `Detach` (DP-7). The worker answers with `RouteClosed`.
    Detach {
        route: RouteId,
        reason: DetachReason,
    },
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
    ClipboardWrite {
        selection: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<Vec<u8>>,
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
    /// The control queue has room again after a refusal (IN-6).
    Writable,
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
    LaunchFailed {
        reason: StartFailReason,
    },
    /// The payload ended (EV-4). The host decides the `cause` (LC-5), because it knows what it asked for.
    Exited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signal: Option<i32>,
    },
    /// The answer to `HostMsg::Op` with this request number.
    Done {
        req: u64,
        result: OpResult,
    },
    /// The pages of the snapshot that answers request `req` (ST-6). A `Done` with `Capture` follows. The host mints the
    /// `CaptureId` and keeps the pages.
    Pages {
        req: u64,
        pages: Vec<Page>,
    },
    /// A change that the worker saw. The worker sends none before `Launched` (every observation comes from the payload's
    /// output, its input or its resize). The host reads none while the session's start is not through: the frame stays unread
    /// on the link, so the observations keep their order and follow `Running` (OR-2, ST-4).
    Observed {
        observation: Observation,
    },
    /// A route reached `Closed` (OU-2): the host posts `RouteClosed` and frees the route.
    RouteClosed {
        route: RouteId,
        reason: RouteCloseReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_tag: Option<String>,
    },
    RouteStalled {
        route: RouteId,
    },
    RouteResumed {
        route: RouteId,
    },
    /// The complete result of the upload cleanup (A6-3): after it the worker ends.
    RemoveResult {
        uploads: UploadsOutcome,
    },
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
            stop_grace_ms: 250,
        }));
        let mut bytes = Vec::new();
        msg.encode(&mut bytes);
        assert_eq!(HostMsg::decode(&bytes), Ok(msg));
    }

    /// An older host sends no `stop_grace_ms`: the worker uses the default of the Core limits table (5 s).
    #[test]
    fn an_absent_stop_grace_is_the_default_of_the_limits_table() {
        let text = r#"{"t":"launch","argv":["/bin/sh"],"env":{},"cwd":"/","size":{"rows":24,"cols":80},"link_frame_bound":1024}"#;
        let Ok(HostMsg::Launch(spec)) = HostMsg::decode(text.as_bytes()) else {
            panic!("a launch");
        };
        assert_eq!(spec.stop_grace_ms, 5000);
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
        ];
        for msg in msgs {
            let mut bytes = Vec::new();
            msg.encode(&mut bytes);
            assert_eq!(WorkerMsg::decode(&bytes), Ok(msg.clone()), "{msg:?}");
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

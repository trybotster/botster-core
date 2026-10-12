//! Serde round trips for every enum variant, the schema against the wire forms, and the code tables.
//! The `all!` macro matches each enum without a wildcard, so a new variant fails to compile until it is listed here.

use crate::prelude::*;
use botster_route_codec::prelude::{
    HexBytes, Key, KeyEvent, Modifier, MouseAction, MouseButton, NamedKey,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn validator_ok(schema: &Value, v: &Value) -> bool {
    jsonschema::validator_for(schema).unwrap().is_valid(v)
}

fn schema_of(name: &str) -> Value {
    crate::schema::all()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}

/// Every variant of `$ty` is listed twice: as a pattern (the compile-time guard) and as a value (the sample).
macro_rules! all {
    ($ty:ty; $($pat:pat => $val:expr),* $(,)?) => {{
        let values: Vec<$ty> = vec![$($val),*];
        for v in &values {
            #[allow(unreachable_patterns)]
            match v { $($pat => {}),* }
        }
        values
    }};
}

/// Serializes, parses back and compares. Returns the JSON for further checks.
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) -> Value {
    let json = serde_json::to_value(v).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap_or_else(|e| panic!("{json}: {e}"));
    assert_eq!(&back, v, "{json}");
    json
}

fn rt_all<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    values: Vec<T>,
) -> Vec<Value> {
    values.iter().map(round_trip).collect()
}

fn sid(s: &str) -> SessionId {
    SessionId(s.into())
}
fn iid() -> InstanceId {
    InstanceId("i1".into())
}
fn size() -> Size {
    Size {
        rows: 24,
        cols: 80,
        cell_px: Some(CellPx {
            width: 8,
            height: 16,
        }),
    }
}
fn exit() -> Exit {
    Exit {
        code: Some(0),
        signal: None,
        cause: ExitCause::Normal,
    }
}
fn svc() -> ServiceId {
    ServiceId([7; 32])
}
fn request() -> SpawnRequest {
    SpawnRequest {
        argv: vec!["sh".into()],
        env: BTreeMap::from([("A".into(), "b".into())]),
        cwd: "/tmp".into(),
        size: size(),
        labels: BTreeMap::new(),
        color_profile: Some(ColorProfile {
            palette: Some(vec![Rgb { r: 0, g: 0, b: 0 }; 256]),
            foreground: Rgb { r: 1, g: 2, b: 3 },
            background: Rgb { r: 4, g: 5, b: 6 },
            cursor: None,
        }),
        notification_policy: Some(NotificationPolicy::Matching {
            patterns: vec!["x".into()],
        }),
        size_policy: Some(SizePolicy::Smallest),
    }
}
fn record() -> SessionRecord {
    SessionRecord {
        id: sid("s1"),
        state: SessionState::Running,
        size: size(),
        labels: BTreeMap::new(),
        exit: None,
        worker_protocol: Some(1),
        worker_features: Some(BTreeSet::from([Feature::FocusReport])),
    }
}
fn options() -> AttachOptions {
    AttachOptions {
        file_directory: "/tmp/f".into(),
        file_permissions: Some(0o600),
        route_features: vec!["output_dropped".into()],
        terminal_formats: vec!["ghostsnp".into()],
        owner: Some("o".into()),
        query_deadline: Some(std::time::Duration::from_millis(250)),
        route_tag: Some("t".into()),
        route_limits: Some(RouteLimitChoices {
            max_frame_bytes: Some(4096),
            ..Default::default()
        }),
        history: Some(HistoryMode::Full),
        stall_deadline: None,
        answers_queries: true,
        input: false,
    }
}
fn limits_service() -> ServiceLimits {
    ServiceLimits {
        lanes: vec![LaneSpec {
            max_frame_bytes: 1024,
            outbound_queue_bytes: 4096,
            inbound_queue_bytes: 4096,
        }],
        rlimits: Rlimits {
            cpu_seconds: Some(RlimitSpec {
                value: 5,
                required: true,
            }),
            ..Default::default()
        },
        startup: std::time::Duration::from_secs(1),
        stop_grace: std::time::Duration::from_secs(1),
        orphan_grace: std::time::Duration::from_secs(1),
        peer_identity: Some(501),
    }
}
fn key_input() -> KeyInput {
    KeyInput {
        key: Key::Named(NamedKey("enter".into())),
        shifted_key: None,
        base_layout_key: None,
        mods: vec![Modifier::Ctrl],
        event: KeyEvent::Press,
        text: None,
        repeat: Some(2),
    }
}
fn mouse_input() -> MouseInput {
    MouseInput {
        action: MouseAction::Press,
        button: MouseButton::Left,
        row: 1,
        col: 2,
        x: None,
        y: None,
        mods: vec![],
        notches: None,
    }
}
fn input_result() -> InputResult {
    InputResult {
        outcome: WriteOutcome::Written,
        payload_bytes_written: 1,
        pty_bytes_written: 1,
        detail: String::new(),
    }
}
fn record_entry() -> InputRecord {
    InputRecord {
        cursor: InputCursor(1),
        source: InputSource::Host,
        at: 5,
        kind: InputKind::Key,
        event: Some(KeyEvent::Press),
        key: None,
        mods: None,
        shifted_key: None,
        base_layout_key: None,
        text: None,
        repeat: None,
        require_bracketed: None,
        mouse: None,
        focus: None,
        payload: None,
        payload_truncated: false,
        state: InputRecordState::Pending,
        pty_bytes: None,
    }
}

// Core 3, Core 9.3, Core A2-1: every enum variant survives serde.
#[test]
fn state_and_exit_enums_round_trip() {
    rt_all(all!(Feature;
        Feature::RouteTransportStream => Feature::RouteTransportStream,
        Feature::ServicePreambleVersions => Feature::ServicePreambleVersions, Feature::Silence => Feature::Silence,
        Feature::SizePolicyOther => Feature::SizePolicyOther, Feature::SnapshotGraphics => Feature::SnapshotGraphics,
        Feature::FocusReport => Feature::FocusReport, Feature::PayloadDiesWithGuardian => Feature::PayloadDiesWithGuardian,
        Feature::BoundCpuSeconds => Feature::BoundCpuSeconds, Feature::BoundOpenFiles => Feature::BoundOpenFiles,
        Feature::BoundProcesses => Feature::BoundProcesses, Feature::BoundAddressSpaceBytes => Feature::BoundAddressSpaceBytes,
        Feature::OutputDropped => Feature::OutputDropped, Feature::NotificationPolicy => Feature::NotificationPolicy));
    let names: Vec<Value> = [
        Feature::RouteTransportStream,
        Feature::BoundCpuSeconds,
        Feature::NotificationPolicy,
    ]
    .iter()
    .map(|f| serde_json::to_value(f).unwrap())
    .collect();
    assert_eq!(
        names,
        [
            json!("route_transport:stream"),
            json!("bound:cpu_seconds"),
            json!("notification_policy")
        ]
    );
    rt_all(all!(NotificationPolicy;
        NotificationPolicy::All => NotificationPolicy::All, NotificationPolicy::None => NotificationPolicy::None,
        NotificationPolicy::Matching { .. } => NotificationPolicy::Matching { patterns: vec!["".into()] }));
    rt_all(
        all!(SizePolicy; SizePolicy::Latest => SizePolicy::Latest, SizePolicy::Smallest => SizePolicy::Smallest, SizePolicy::Largest => SizePolicy::Largest),
    );
    rt_all(all!(ExitCause;
        ExitCause::Normal => ExitCause::Normal, ExitCause::Signal => ExitCause::Signal, ExitCause::HostStop => ExitCause::HostStop,
        ExitCause::Killed => ExitCause::Killed, ExitCause::Other => ExitCause::Other));
    rt_all(all!(LostReason;
        LostReason::WorkerGone => LostReason::WorkerGone, LostReason::WorkerUnreachable => LostReason::WorkerUnreachable,
        LostReason::WorkerVersion => LostReason::WorkerVersion, LostReason::RegistryCorrupt => LostReason::RegistryCorrupt,
        LostReason::StartInterrupted => LostReason::StartInterrupted, LostReason::GuardianLost => LostReason::GuardianLost,
        LostReason::EpochExhausted => LostReason::EpochExhausted, LostReason::Other => LostReason::Other));
    let states = rt_all(all!(SessionState;
        SessionState::Created => SessionState::Created, SessionState::Starting => SessionState::Starting,
        SessionState::Running => SessionState::Running, SessionState::Stopping => SessionState::Stopping,
        SessionState::Exited(_) => SessionState::Exited(exit()), SessionState::Lost(_) => SessionState::Lost(LostReason::WorkerGone),
        SessionState::Released => SessionState::Released));
    assert_eq!(states[2], json!("Running"));
    rt_all(
        all!(SessionEnd; SessionEnd::Exited(_) => SessionEnd::Exited(exit()), SessionEnd::Lost(_) => SessionEnd::Lost(LostReason::WorkerGone)),
    );
    rt_all(all!(Signal;
        Signal::Term => Signal::Term, Signal::Kill => Signal::Kill, Signal::Int => Signal::Int, Signal::Hup => Signal::Hup,
        Signal::Other(_) => Signal::Other(12)));
    rt_all(all!(StartFailReason;
        StartFailReason::ExecFailed { .. } => StartFailReason::ExecFailed { errno: 2 }, StartFailReason::CwdMissing => StartFailReason::CwdMissing,
        StartFailReason::StartupTimeout => StartFailReason::StartupTimeout, StartFailReason::WorkerFailed => StartFailReason::WorkerFailed,
        StartFailReason::GuardianFailed => StartFailReason::GuardianFailed,
        StartFailReason::Other => StartFailReason::Other));
    rt_all(all!(WorkerCompatibility;
        WorkerCompatibility::Compatible => WorkerCompatibility::Compatible, WorkerCompatibility::Incompatible => WorkerCompatibility::Incompatible,
        WorkerCompatibility::Unknown => WorkerCompatibility::Unknown));
}

// Core 9.3, Core A2-1: every error code, category and reason set.
#[test]
fn error_enums_round_trip() {
    rt_all(
        all!(ErrorCategory; ErrorCategory::Caller => ErrorCategory::Caller, ErrorCategory::State => ErrorCategory::State,
        ErrorCategory::Fault => ErrorCategory::Fault, ErrorCategory::Capacity => ErrorCategory::Capacity),
    );
    rt_all(
        all!(Timing; Timing::Sync => Timing::Sync, Timing::Async => Timing::Async, Timing::Either => Timing::Either),
    );
    rt_all(
        all!(UnsupportedReason; UnsupportedReason::TerminalFormat => UnsupportedReason::TerminalFormat,
        UnsupportedReason::Other => UnsupportedReason::Other),
    );
    let codes = all!(ErrorCode;
        ErrorCode::UnknownSession => ErrorCode::UnknownSession, ErrorCode::UnknownRoute => ErrorCode::UnknownRoute,
        ErrorCode::IdTooLong => ErrorCode::IdTooLong, ErrorCode::IdInUse => ErrorCode::IdInUse, ErrorCode::WrongState => ErrorCode::WrongState,
        ErrorCode::SessionEnded => ErrorCode::SessionEnded, ErrorCode::MissingWorkerPath => ErrorCode::MissingWorkerPath,
        ErrorCode::DataDirInUse => ErrorCode::DataDirInUse, ErrorCode::StartFailed(_) => ErrorCode::StartFailed(StartFailReason::CwdMissing),
        ErrorCode::WorkerLinkFailed => ErrorCode::WorkerLinkFailed, ErrorCode::LaneFull => ErrorCode::LaneFull,
        ErrorCode::InvalidInput { .. } => ErrorCode::InvalidInput { field: Some("text".into()) }, ErrorCode::PayloadTooLarge => ErrorCode::PayloadTooLarge,
        ErrorCode::PendingLimit => ErrorCode::PendingLimit, ErrorCode::SessionLimit => ErrorCode::SessionLimit,
        ErrorCode::RouteLimit => ErrorCode::RouteLimit, ErrorCode::CaptureLimit => ErrorCode::CaptureLimit,
        ErrorCode::ServiceLimit => ErrorCode::ServiceLimit, ErrorCode::Cancelled => ErrorCode::Cancelled,
        ErrorCode::CursorReadUnsupported => ErrorCode::CursorReadUnsupported, ErrorCode::UnknownCapture => ErrorCode::UnknownCapture,
        ErrorCode::PageOutOfRange => ErrorCode::PageOutOfRange,
        ErrorCode::Unsupported { .. } => ErrorCode::Unsupported { what: Some(UnsupportedReason::TerminalFormat) },
        ErrorCode::InvalidConfig { .. } => ErrorCode::InvalidConfig { field: "event_queue".into() },
        ErrorCode::RegistryFailed { .. } => ErrorCode::RegistryFailed { uncertain: true },
        ErrorCode::SnapshotTooLarge => ErrorCode::SnapshotTooLarge,
        ErrorCode::BoundUnavailable { .. } => ErrorCode::BoundUnavailable { bound: Bound::Processes },
        ErrorCode::Internal => ErrorCode::Internal, ErrorCode::UnknownService => ErrorCode::UnknownService);
    let forms = rt_all(codes.clone());
    assert_eq!(forms[0], json!("UnknownSession"));
    assert_eq!(codes.len(), 29);
    // A bare `Unsupported` with no reason is also a code that a receiver can read.
    assert_eq!(
        serde_json::from_value::<ErrorCode>(json!({ "Unsupported": {} })).unwrap(),
        ErrorCode::Unsupported { what: None }
    );
    // Core A7-1, Core A15-1: `InvalidInput` names the refused input in `field`; with no field, the object is empty.
    let named = ErrorCode::InvalidInput {
        field: Some("route_limits.max_frame_bytes".into()),
    };
    assert_eq!(
        round_trip(&named),
        json!({ "InvalidInput": { "field": "route_limits.max_frame_bytes" } })
    );
    assert_eq!(
        round_trip(&ErrorCode::InvalidInput { field: None }),
        json!({ "InvalidInput": {} })
    );
    let err = CoreError::new(ErrorCode::WrongState, "late");
    assert_eq!(
        round_trip(&err),
        json!({ "code": "WrongState", "category": "state", "detail": "late" })
    );
}

// Core 9.3, Core A2-1: category and sync column of every code.
#[test]
fn every_code_has_the_category_and_timing_of_the_table() {
    use ErrorCategory::*;
    use Timing::*;
    let rows: Vec<(ErrorCode, ErrorCategory, Timing)> = vec![
        (ErrorCode::UnknownSession, Caller, Sync),
        (ErrorCode::UnknownRoute, Caller, Sync),
        (ErrorCode::IdTooLong, Caller, Sync),
        (ErrorCode::IdInUse, Caller, Sync),
        (ErrorCode::WrongState, State, Either),
        (ErrorCode::SessionEnded, State, Async),
        (ErrorCode::MissingWorkerPath, Caller, Sync),
        (ErrorCode::DataDirInUse, State, Sync),
        (ErrorCode::StartFailed(StartFailReason::Other), Fault, Async),
        (ErrorCode::WorkerLinkFailed, Fault, Async),
        (ErrorCode::LaneFull, Capacity, Sync),
        (ErrorCode::InvalidInput { field: None }, Caller, Sync),
        (ErrorCode::PayloadTooLarge, Caller, Sync),
        (ErrorCode::PendingLimit, Capacity, Sync),
        (ErrorCode::SessionLimit, Capacity, Sync),
        (ErrorCode::RouteLimit, Capacity, Sync),
        (ErrorCode::CaptureLimit, Capacity, Sync),
        (ErrorCode::ServiceLimit, Capacity, Sync),
        (ErrorCode::Cancelled, State, Async),
        (ErrorCode::CursorReadUnsupported, Caller, Async),
        (ErrorCode::UnknownCapture, Caller, Sync),
        (ErrorCode::PageOutOfRange, Caller, Sync),
        (ErrorCode::Unsupported { what: None }, Caller, Sync),
        (ErrorCode::InvalidConfig { field: "f".into() }, Caller, Sync),
        (ErrorCode::RegistryFailed { uncertain: false }, Fault, Async),
        (ErrorCode::SnapshotTooLarge, Capacity, Async),
        (
            ErrorCode::BoundUnavailable {
                bound: Bound::OpenFiles,
            },
            Caller,
            Sync,
        ),
        (ErrorCode::Internal, Fault, Either),
        (ErrorCode::UnknownService, Caller, Sync),
    ];
    assert_eq!(rows.len(), 29);
    for (code, category, timing) in rows {
        assert_eq!(code.category(), category, "{code:?}");
        assert_eq!(code.timing(), timing, "{code:?}");
        assert_eq!(CoreError::new(code, "").category, category);
    }
}

// Core A2-1: `DeadlineExpired` is removed, and no code names a deadline.
#[test]
fn no_deadline_code_exists() {
    let schema = serde_json::to_string(&schema_of("core-error")).unwrap();
    assert!(!schema.contains("DeadlineExpired"));
    assert!(serde_json::from_value::<ErrorCode>(json!("DeadlineExpired")).is_err());
}

// Core IN-2, Core IN-6, Core IN-9, Core A2-2.
#[test]
fn input_enums_round_trip() {
    rt_all(all!(InputPayload;
        InputPayload::Bytes { .. } => InputPayload::Bytes { bytes: HexBytes(vec![1, 2]) },
        InputPayload::Text { .. } => InputPayload::Text { text: "é".into() },
        InputPayload::Paste { .. } => InputPayload::Paste { bytes: HexBytes(vec![3]), require_bracketed: true },
        InputPayload::Key(_) => InputPayload::Key(key_input()), InputPayload::Mouse(_) => InputPayload::Mouse(mouse_input()),
        InputPayload::Focus { .. } => InputPayload::Focus { focused: true }));
    rt_all(
        all!(SourceClass; SourceClass::Client => SourceClass::Client, SourceClass::Host => SourceClass::Host),
    );
    rt_all(all!(UnsupportedWhat;
        UnsupportedWhat::PixelPosition => UnsupportedWhat::PixelPosition, UnsupportedWhat::ProducedText => UnsupportedWhat::ProducedText,
        UnsupportedWhat::NamedKey => UnsupportedWhat::NamedKey, UnsupportedWhat::WheelDirection => UnsupportedWhat::WheelDirection,
        UnsupportedWhat::Coordinate => UnsupportedWhat::Coordinate, UnsupportedWhat::ReleaseEvent => UnsupportedWhat::ReleaseEvent,
        UnsupportedWhat::Other => UnsupportedWhat::Other));
    rt_all(all!(NotWrittenReason;
        NotWrittenReason::LaneFull => NotWrittenReason::LaneFull, NotWrittenReason::Refused => NotWrittenReason::Refused,
        NotWrittenReason::SessionEnded => NotWrittenReason::SessionEnded, NotWrittenReason::Stopping => NotWrittenReason::Stopping,
        NotWrittenReason::Stale => NotWrittenReason::Stale, NotWrittenReason::PayloadTooLarge => NotWrittenReason::PayloadTooLarge,
        NotWrittenReason::ModePreconditionFailed => NotWrittenReason::ModePreconditionFailed,
        NotWrittenReason::NotReported => NotWrittenReason::NotReported,
        NotWrittenReason::Unsupported { .. } => NotWrittenReason::Unsupported { what: UnsupportedWhat::PixelPosition }));
    let outcomes = rt_all(all!(WriteOutcome;
        WriteOutcome::Written => WriteOutcome::Written, WriteOutcome::NotWritten(_) => WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
        WriteOutcome::Partial => WriteOutcome::Partial, WriteOutcome::Failed => WriteOutcome::Failed,
        WriteOutcome::Cancelled => WriteOutcome::Cancelled, WriteOutcome::Unknown { .. } => WriteOutcome::Unknown { max_payload_bytes: 9 }));
    assert_eq!(outcomes[1], json!({ "NotWritten": "SessionEnded" }));
    rt_all(
        all!(CancelRefusal; CancelRefusal::Full => CancelRefusal::Full, CancelRefusal::NotCancellable => CancelRefusal::NotCancellable),
    );
    rt_all(all!(CancelResult;
        CancelResult::Admitted => CancelResult::Admitted, CancelResult::AlreadyAdmitted => CancelResult::AlreadyAdmitted,
        CancelResult::TooLate => CancelResult::TooLate, CancelResult::UnknownOp => CancelResult::UnknownOp,
        CancelResult::Refused(_) => CancelResult::Refused(CancelRefusal::NotCancellable)));
    round_trip(&input_result());
    round_trip(&Guard {
        input: Some(InputGuard {
            source_class: SourceClass::Client,
            rev: InputRev(3),
        }),
        model_rev: Some(ModelRev(4)),
    });
}

// Core ST-7: the input records of `ReadFacts`.
#[test]
fn read_enums_round_trip() {
    rt_all(
        all!(InputKind; InputKind::Bytes => InputKind::Bytes, InputKind::Text => InputKind::Text, InputKind::Paste => InputKind::Paste,
        InputKind::Key => InputKind::Key, InputKind::Mouse => InputKind::Mouse, InputKind::Focus => InputKind::Focus),
    );
    rt_all(all!(InputSource;
        InputSource::Client { .. } => InputSource::Client { route_tag: Some("t".into()), owner: None }, InputSource::Host => InputSource::Host));
    rt_all(all!(InputRecordState;
        InputRecordState::Pending => InputRecordState::Pending, InputRecordState::Written => InputRecordState::Written,
        InputRecordState::Partial => InputRecordState::Partial,
        InputRecordState::NotWritten { .. } => InputRecordState::NotWritten { reason: NotWrittenReason::Stale },
        InputRecordState::Cancelled => InputRecordState::Cancelled, InputRecordState::Unknown => InputRecordState::Unknown));
    rt_all(all!(InputRecordEntry;
        InputRecordEntry::Record(_) => InputRecordEntry::Record(record_entry()),
        InputRecordEntry::StatusUpdate(_) => InputRecordEntry::StatusUpdate(InputStatusUpdate {
            cursor: InputCursor(2), updates: InputCursor(1), state: InputRecordState::Written, pty_bytes: Some(4) })));
}

// Core OU-1, Core A2-3, Core DP-7.
#[test]
fn route_enums_round_trip() {
    rt_all(
        all!(HistoryMode; HistoryMode::None => HistoryMode::None, HistoryMode::Screen => HistoryMode::Screen, HistoryMode::Full => HistoryMode::Full),
    );
    rt_all(
        all!(DetachReason; DetachReason::Detached => DetachReason::Detached, DetachReason::Replaced => DetachReason::Replaced,
        DetachReason::Revoked => DetachReason::Revoked),
    );
    let reasons = all!(RouteCloseReason;
        RouteCloseReason::Detached => RouteCloseReason::Detached, RouteCloseReason::Replaced => RouteCloseReason::Replaced,
        RouteCloseReason::Revoked => RouteCloseReason::Revoked, RouteCloseReason::SessionEnded { .. } => RouteCloseReason::SessionEnded { exit: exit() },
        RouteCloseReason::SessionRemoved => RouteCloseReason::SessionRemoved, RouteCloseReason::SessionLost => RouteCloseReason::SessionLost,
        RouteCloseReason::PeerClosed => RouteCloseReason::PeerClosed, RouteCloseReason::WriteFailed => RouteCloseReason::WriteFailed,
        RouteCloseReason::BadFrame { .. } => RouteCloseReason::BadFrame { code: botster_route_codec::prelude::ProtocolErrorCode::UnknownFrame },
        RouteCloseReason::SnapshotTooLarge => RouteCloseReason::SnapshotTooLarge, RouteCloseReason::StallTimeout => RouteCloseReason::StallTimeout,
        RouteCloseReason::HandoffFailed => RouteCloseReason::HandoffFailed, RouteCloseReason::Other => RouteCloseReason::Other);
    assert_eq!(reasons.len(), 13);
    rt_all(reasons);
    round_trip(&options());
}

// Core A2-5, Core SV-3, Core SV-5.
#[test]
fn service_enums_round_trip() {
    rt_all(
        all!(Bound; Bound::CpuSeconds => Bound::CpuSeconds, Bound::OpenFiles => Bound::OpenFiles, Bound::Processes => Bound::Processes,
        Bound::AddressSpaceBytes => Bound::AddressSpaceBytes),
    );
    rt_all(all!(ServiceExitCause;
        ServiceExitCause::Normal => ServiceExitCause::Normal, ServiceExitCause::Signal => ServiceExitCause::Signal,
        ServiceExitCause::HostStop => ServiceExitCause::HostStop, ServiceExitCause::StartupTimeout => ServiceExitCause::StartupTimeout,
        ServiceExitCause::OrphanGrace => ServiceExitCause::OrphanGrace, ServiceExitCause::Killed => ServiceExitCause::Killed,
        ServiceExitCause::ChildReported(_) => ServiceExitCause::ChildReported(7), ServiceExitCause::Other => ServiceExitCause::Other));
    let exit = ServiceExit {
        code: None,
        signal: Some(9),
        cause: ServiceExitCause::Killed,
        descendants_may_remain: true,
    };
    rt_all(all!(ServiceState;
        ServiceState::Started => ServiceState::Started, ServiceState::Reconnected { .. } => ServiceState::Reconnected { epoch: 2 },
        ServiceState::Exited(_) => ServiceState::Exited(exit),
        ServiceState::Lost { .. } => ServiceState::Lost { reason: LostReason::GuardianLost, payload_may_remain: true },
        ServiceState::Released => ServiceState::Released));
    rt_all(all!(LaneEndReason;
        LaneEndReason::BadLane => LaneEndReason::BadLane, LaneEndReason::Oversize => LaneEndReason::Oversize,
        LaneEndReason::Truncated => LaneEndReason::Truncated, LaneEndReason::PeerClosed => LaneEndReason::PeerClosed,
        LaneEndReason::EpochEnded => LaneEndReason::EpochEnded, LaneEndReason::Superseded => LaneEndReason::Superseded,
        LaneEndReason::ServiceExited => LaneEndReason::ServiceExited, LaneEndReason::ServiceRemoved => LaneEndReason::ServiceRemoved,
        LaneEndReason::Other => LaneEndReason::Other));
    rt_all(all!(SendError;
        SendError::Backpressured => SendError::Backpressured, SendError::UnknownService => SendError::UnknownService,
        SendError::NoSuchLane => SendError::NoSuchLane, SendError::NotConnected => SendError::NotConnected,
        SendError::ServiceEnded => SendError::ServiceEnded, SendError::Oversize => SendError::Oversize, SendError::Other => SendError::Other));
    rt_all(
        all!(RecvError; RecvError::UnknownService => RecvError::UnknownService, RecvError::NoSuchLane => RecvError::NoSuchLane),
    );
    round_trip(&limits_service());
    round_trip(&SpawnReport {
        applied: vec![Bound::CpuSeconds],
        unsupported: vec![Bound::Processes],
    });
    let frame = Frame {
        epoch: 1,
        frame_type: 2,
        payload: HexBytes(vec![9]),
    };
    assert_eq!(
        round_trip(&frame),
        json!({ "epoch": 1, "type": 2, "payload": { "$bytes_hex": "09" } })
    );
}

// Core 6.2, Core A2-4: every event variant, and the form that a transcript writes.
#[test]
fn every_event_round_trips_and_matches_the_schema() {
    let ids = || (sid("s1"), iid());
    let events = all!(Event;
        Event::Completed { .. } => Event::Completed { op: OpId(1), result: OpResult::Ok(OpOutput::Unit) },
        Event::SessionState { .. } => { let (id, instance) = ids(); Event::SessionState { id, instance, state: SessionState::Running } },
        Event::RouteClosed { .. } => Event::RouteClosed { route: RouteId(1), reason: RouteCloseReason::Detached, route_tag: Some("t".into()) },
        Event::RouteStalled { .. } => Event::RouteStalled { route: RouteId(1) },
        Event::RouteResumed { .. } => Event::RouteResumed { route: RouteId(1) },
        Event::SessionWritable { .. } => { let (id, instance) = ids(); Event::SessionWritable { id, instance } },
        Event::ModesChanged { .. } => { let (id, instance) = ids(); Event::ModesChanged { id, instance, flags: ModeFlags::default() } },
        Event::FocusChanged { .. } => { let (id, instance) = ids(); Event::FocusChanged { id, instance, focused: FocusState::Unknown } },
        Event::SizeChanged { .. } => { let (id, instance) = ids(); Event::SizeChanged { id, instance, size: size() } },
        Event::Activity { .. } => { let (id, instance) = ids(); Event::Activity { id, instance, source: ActivitySource::Client(RouteId(2)), at: 5 } },
        Event::Silent { .. } => { let (id, instance) = ids(); Event::Silent { id, instance, since: 4 } },
        Event::TitleChanged { .. } => { let (id, instance) = ids(); Event::TitleChanged { id, instance, title: "t".into() } },
        Event::CwdChanged { .. } => { let (id, instance) = ids(); Event::CwdChanged { id, instance, cwd: "/".into() } },
        Event::Bell { .. } => { let (id, instance) = ids(); Event::Bell { id, instance, at: 1 } },
        Event::PromptMark { .. } => { let (id, instance) = ids(); Event::PromptMark { id, instance, mark: PromptMarkKind::CommandFinished, exit_code: Some(1), at: 1 } },
        Event::Notification { .. } => { let (id, instance) = ids(); Event::Notification { id, instance, source: NotificationSource::Osc777, title: None, body: "b".into(), truncated: false, at: 1 } },
        Event::ClipboardWrite { .. } => { let (id, instance) = ids(); Event::ClipboardWrite { id, instance, selection: "c".into(), contents: None, total_bytes: 9, reason: Some(ClipboardReason::TooLarge) } },
        Event::EventsLost { .. } => { let (id, instance) = ids(); Event::EventsLost { id, instance, kinds: BTreeSet::from([LostKind::Query, LostKind::Tap]), tap_dropped_bytes: Some(3) } },
        Event::MetadataChanged { .. } => { let (id, instance) = ids(); Event::MetadataChanged { id, instance } },
        Event::RouteAdopted { .. } => Event::RouteAdopted { session: sid("s1"), instance: iid(), route: RouteId(3), route_tag: "t".into(), owner: "o".into(), client: ClientId("c".into()) },
        Event::ServiceState { .. } => Event::ServiceState { id: svc(), state: ServiceState::Started },
        Event::LaneConnected { .. } => Event::LaneConnected { id: svc(), lane: 0, epoch: 1 },
        Event::LaneEnded { .. } => Event::LaneEnded { id: svc(), lane: 0, epoch: 1, reason: LaneEndReason::EpochEnded, dropped_unsent_frames: 1, dropped_unsent_bytes: 2 },
        Event::ServiceWritable { .. } => Event::ServiceWritable { id: svc(), lane: 1 },
        Event::ServiceReadable { .. } => Event::ServiceReadable { id: svc(), lane: 1 });
    assert_eq!(events.len(), 25);
    let schema = schema_of("event");
    for json in rt_all(events) {
        assert!(validator_ok(&schema, &json), "{json}");
    }
    rt_all(
        all!(PromptMarkKind; PromptMarkKind::PromptStart => PromptMarkKind::PromptStart, PromptMarkKind::CommandStart => PromptMarkKind::CommandStart,
        PromptMarkKind::CommandExecuted => PromptMarkKind::CommandExecuted, PromptMarkKind::CommandFinished => PromptMarkKind::CommandFinished),
    );
    rt_all(
        all!(NotificationSource; NotificationSource::Osc9 => NotificationSource::Osc9, NotificationSource::Osc777 => NotificationSource::Osc777),
    );
    rt_all(all!(ClipboardReason; ClipboardReason::TooLarge => ClipboardReason::TooLarge));
    rt_all(
        all!(LostKind; LostKind::Bell => LostKind::Bell, LostKind::PromptMark => LostKind::PromptMark, LostKind::Notification => LostKind::Notification,
        LostKind::ClipboardWrite => LostKind::ClipboardWrite, LostKind::Query => LostKind::Query, LostKind::Tap => LostKind::Tap, LostKind::Other => LostKind::Other),
    );
    rt_all(
        all!(ActivitySource; ActivitySource::Output => ActivitySource::Output, ActivitySource::Client(_) => ActivitySource::Client(RouteId(1))),
    );
    rt_all(
        all!(FocusState; FocusState::Focused => FocusState::Focused, FocusState::Unfocused => FocusState::Unfocused, FocusState::Unknown => FocusState::Unknown),
    );
}

// Core A13-1: `contents` is absent for TooLarge, an empty list clears, and one empty entry is a write of an empty value.
#[test]
fn clipboard_contents_keep_clear_distinct_from_an_empty_value() {
    let write =
        |contents: Option<Vec<ClipboardContent>>, total_bytes, reason| Event::ClipboardWrite {
            id: sid("s1"),
            instance: iid(),
            selection: "c".into(),
            contents,
            total_bytes,
            reason,
        };
    let entry = |mime: &str, bytes: &[u8]| ClipboardContent {
        mime: mime.into(),
        bytes: HexBytes(bytes.to_vec()),
    };
    let schema = schema_of("event");
    let cases = [
        (write(None, 9, Some(ClipboardReason::TooLarge)), None),
        (write(Some(vec![]), 0, None), Some(json!([]))),
        (
            write(Some(vec![entry("text/plain", b"")]), 0, None),
            Some(json!([{ "mime": "text/plain", "bytes": { "$bytes_hex": "" } }])),
        ),
        (
            write(
                Some(vec![entry("text/plain", b"hi"), entry("text/html", b"h")]),
                3,
                None,
            ),
            Some(json!([
                { "mime": "text/plain", "bytes": { "$bytes_hex": "6869" } },
                { "mime": "text/html", "bytes": { "$bytes_hex": "68" } }
            ])),
        ),
    ];
    for (event, contents) in cases {
        let json = round_trip(&event);
        assert!(validator_ok(&schema, &json), "{json}");
        assert_eq!(
            json["ClipboardWrite"].get("contents"),
            contents.as_ref(),
            "{json}"
        );
        assert!(json["ClipboardWrite"].get("bytes").is_none(), "{json}");
    }
}

// Core A16-1, Core A16-2: `StopService` completes with `ServiceEnd`, and `Lost` carries the state's two fields.
#[test]
fn stop_service_completes_with_a_service_end() {
    let exit = ServiceExit {
        code: None,
        signal: Some(15),
        cause: ServiceExitCause::HostStop,
        descendants_may_remain: true,
    };
    let ends = all!(ServiceEnd;
        ServiceEnd::Exited(_) => ServiceEnd::Exited(exit),
        ServiceEnd::Lost { .. } => ServiceEnd::Lost { reason: LostReason::GuardianLost, payload_may_remain: true });
    let schema = schema_of("op-result");
    let forms = [
        json!({ "ok": { "service_end": { "Exited": { "signal": 15, "cause": "HostStop", "descendants_may_remain": true } } } }),
        json!({ "ok": { "service_end": { "Lost": { "reason": "GuardianLost", "payload_may_remain": true } } } }),
    ];
    for (end, form) in ends.into_iter().zip(forms) {
        let json = round_trip(&OpResult::Ok(OpOutput::ServiceEnd(end)));
        assert_eq!(json, form);
        assert!(validator_ok(&schema, &json), "{json}");
    }
    let lost = json!({ "ok": { "service_end": { "Lost": { "reason": "EpochExhausted" } } } });
    assert!(
        serde_json::from_value::<OpResult>(lost.clone()).is_err(),
        "payload_may_remain is required"
    );
    assert!(!validator_ok(&schema, &lost));
}

// Core A2-1, Core A3-3: one `Op` variant per table row (25 rows and the A3 row), and its completion values.
#[test]
fn every_op_has_a_variant_and_a_schema_valid_form() {
    let s = sid("s1");
    let ops = all!(Op;
        Op::Create { .. } => Op::Create { session: s.clone(), request: request() },
        Op::Start { .. } => Op::Start { id: s.clone() }, Op::Stop { .. } => Op::Stop { id: s.clone() },
        Op::Signal { .. } => Op::Signal { id: s.clone(), sig: Signal::Term }, Op::Remove { .. } => Op::Remove { id: s.clone() },
        Op::StopAll => Op::StopAll,
        Op::UpdateMetadata { .. } => Op::UpdateMetadata { id: s.clone(), labels: BTreeMap::from([("k".into(), "v".into())]) },
        Op::SetSizePolicy { .. } => Op::SetSizePolicy { session: s.clone(), policy: SizePolicy::Latest },
        Op::SetColorProfile { .. } => Op::SetColorProfile { session: s.clone(), profile: request().color_profile.unwrap() },
        Op::SetOutputTap { .. } => Op::SetOutputTap { session: s.clone(), enabled: true },
        Op::ReadScreen { .. } => Op::ReadScreen { session: s.clone(), history: true },
        Op::ReadCursor { .. } => Op::ReadCursor { session: s.clone() }, Op::ReadModeFlags { .. } => Op::ReadModeFlags { session: s.clone() },
        Op::CaptureSnapshot { .. } => Op::CaptureSnapshot { session: s.clone(), owner: ClientId("c".into()) },
        Op::ReadFacts { .. } => Op::ReadFacts { session: s.clone(), after: Some(InputCursor(1)) },
        Op::Resize { .. } => Op::Resize { session: s.clone(), size: size() },
        Op::WriteInput { .. } => Op::WriteInput { session: s.clone(), payload: InputPayload::Focus { focused: true }, guard: Some(Guard::default()) },
        Op::Detach { .. } => Op::Detach { route: RouteId(1), reason: DetachReason::Revoked },
        Op::AdoptAll => Op::AdoptAll, Op::Adopt { .. } => Op::Adopt { id: s.clone() },
        Op::SpawnService { .. } => Op::SpawnService { service_id: svc(), argv: vec!["p".into()], env: BTreeMap::new(), cwd: "/".into(), limits: limits_service() },
        Op::StopService { .. } => Op::StopService { id: svc() }, Op::RemoveService { .. } => Op::RemoveService { id: svc() },
        Op::EndEpoch { .. } => Op::EndEpoch { service: svc() },
        Op::SetNotificationPolicy { .. } => Op::SetNotificationPolicy { session: s.clone(), policy: NotificationPolicy::None });
    assert_eq!(ops.len(), 25);
    let names: Vec<String> = ops
        .iter()
        .map(|o| match serde_json::to_value(o).unwrap() {
            Value::String(n) => n,
            Value::Object(m) => m.keys().next().unwrap().clone(),
            other => panic!("{other}"),
        })
        .collect();
    let table = [
        "Create",
        "Start",
        "Stop",
        "Signal",
        "Remove",
        "StopAll",
        "UpdateMetadata",
        "SetSizePolicy",
        "SetColorProfile",
        "SetOutputTap",
        "ReadScreen",
        "ReadCursor",
        "ReadModeFlags",
        "CaptureSnapshot",
        "ReadFacts",
        "Resize",
        "WriteInput",
        "Detach",
        "AdoptAll",
        "Adopt",
        "SpawnService",
        "StopService",
        "RemoveService",
        "EndEpoch",
        "SetNotificationPolicy",
    ];
    assert_eq!(names, table);
    let schema = schema_of("op");
    for json in rt_all(ops) {
        assert!(validator_ok(&schema, &json), "{json}");
    }
    assert_eq!(
        serde_json::to_value(Op::AdoptAll).unwrap(),
        json!("AdoptAll")
    );
}

#[test]
fn every_completion_value_round_trips_and_matches_the_schema() {
    let screen = Screen {
        text: "x".into(),
        rows: 1,
        cols: 1,
        history_unavailable: false,
        model_rev: ModelRev(1),
    };
    let facts = Facts {
        model_rev: ModelRev(1),
        modes: ModeFlags::default(),
        size: size(),
        cursor: FactsCursor {
            row: 0,
            col: 0,
            visible: true,
        },
        input: InputFacts {
            client: ClientFacts {
                rev: InputRev(1),
                age_ms: 3,
            },
            host: HostFacts { rev: InputRev(2) },
            client_tail: HexBytes(vec![1]),
            tail_complete: true,
            input_records: vec![InputRecordEntry::Record(record_entry())],
            next_cursor: InputCursor(2),
            gap: false,
        },
    };
    let outputs = all!(OpOutput;
        OpOutput::Unit => OpOutput::Unit, OpOutput::Record(_) => OpOutput::Record(record()),
        OpOutput::End(_) => OpOutput::End(SessionEnd::Exited(exit())), OpOutput::Screen(_) => OpOutput::Screen(screen),
        OpOutput::Cursor(_) => OpOutput::Cursor(Cursor { row: 0, col: 1, visible: true, row_text: "a".into(), text_before_cursor: "a".into(), model_rev: ModelRev(1) }),
        OpOutput::Modes(_) => OpOutput::Modes(Modes { flags: ModeFlags::default(), model_rev: ModelRev(1) }),
        OpOutput::Capture(_) => OpOutput::Capture(Capture { capture: CaptureId(1), page_count: 2, total_bytes: 9, model_rev: ModelRev(1) }),
        OpOutput::Facts(_) => OpOutput::Facts(facts),
        OpOutput::Resize(_) => OpOutput::Resize(ResizeResult::Applied { actual: size() }),
        OpOutput::Input(_) => OpOutput::Input(input_result()),
        OpOutput::Spawn(_) => OpOutput::Spawn(SpawnReport::default()),
        OpOutput::ServiceEnd(_) => OpOutput::ServiceEnd(ServiceEnd::Exited(ServiceExit { code: Some(0), signal: None, cause: ServiceExitCause::Normal, descendants_may_remain: false })),
        OpOutput::RemoveReport(_) => OpOutput::RemoveReport(RemoveReport { uploads: UploadsOutcome::Deleted }));
    rt_all(all!(ResizeResult;
        ResizeResult::Applied { .. } => ResizeResult::Applied { actual: size() }, ResizeResult::Superseded { .. } => ResizeResult::Superseded { by: OpId(4) }));
    let schema = schema_of("op-result");
    let mut results: Vec<OpResult> = outputs.into_iter().map(OpResult::Ok).collect();
    results.push(OpResult::Err(CoreError::new(
        ErrorCode::WrongState,
        "state changed",
    )));
    assert_eq!(all!(OpResult; OpResult::Ok(_) => results[0].clone(), OpResult::Err(_) => results[13].clone()).len(), 2);
    for json in rt_all(results) {
        assert!(validator_ok(&schema, &json), "{json}");
    }
}

// Core 3, Core AD-2, Core A2-1, Core A2-3, Core A2-5: an unknown value of a non-exhaustive set is `Other`.
#[test]
fn unknown_values_of_open_sets_read_as_other() {
    assert_eq!(
        serde_json::from_value::<LostReason>(json!("FutureReason")).unwrap(),
        LostReason::Other
    );
    assert_eq!(
        serde_json::from_value::<ExitCause>(json!("FutureCause")).unwrap(),
        ExitCause::Other
    );
    assert_eq!(
        serde_json::from_value::<RouteCloseReason>(json!({ "FutureClose": { "x": 1 } })).unwrap(),
        RouteCloseReason::Other
    );
    assert_eq!(
        serde_json::from_value::<LaneEndReason>(json!("FutureLane")).unwrap(),
        LaneEndReason::Other
    );
    assert_eq!(
        serde_json::from_value::<SendError>(json!("FutureSend")).unwrap(),
        SendError::Other
    );
    assert_eq!(
        serde_json::from_value::<LostKind>(json!("FutureKind")).unwrap(),
        LostKind::Other
    );
    assert_eq!(
        serde_json::from_value::<UnsupportedWhat>(json!("future_what")).unwrap(),
        UnsupportedWhat::Other
    );
    assert_eq!(
        serde_json::from_value::<StartFailReason>(json!({ "FutureFail": {} })).unwrap(),
        StartFailReason::Other
    );
    assert_eq!(
        serde_json::from_value::<RouteCloseReason>(json!({ "BadFrame": { "code": "nope" } }))
            .unwrap(),
        RouteCloseReason::BadFrame {
            code: botster_route_codec::prelude::ProtocolErrorCode::Other
        }
    );
    let schema = schema_of("event");
    let event = json!({ "RouteClosed": { "route": 1, "reason": "FutureClose" } });
    assert!(validator_ok(&schema, &event));
}

// Core 11: the trait is object-safe and the runner holds it behind a box. Core TH-1: `Send`.
#[test]
fn core_api_is_object_safe_and_send() {
    fn takes(_: Option<&dyn CoreApi>) {}
    fn send<T: Send + ?Sized>() {}
    takes(None);
    send::<Box<dyn CoreApi>>();
    send::<dyn WakeHandle>();
    fn sync<T: Sync + ?Sized>() {}
    sync::<dyn WakeHandle>();
}

// Core 9B, Core LC-1: `OpenConfig` carries the data directory, the worker path and the limits, and the limits default.
#[test]
fn open_config_defaults_its_limits_and_the_worker_path_may_be_absent() {
    let cfg: OpenConfig = serde_json::from_value(json!({ "data_dir": "/d" })).unwrap();
    assert_eq!(cfg.worker_path, None);
    assert_eq!(cfg.limits, CoreLimits::default());
    round_trip(&cfg);
    let limits_schema = schema_of("core-limits");
    assert!(validator_ok(
        &limits_schema,
        &serde_json::to_value(CoreLimits::default()).unwrap()
    ));
    assert!(validator_ok(&limits_schema, &json!({ "event_queue": 0 })));
    assert!(!validator_ok(&limits_schema, &json!({ "unknown": 1 })));
}

#[test]
fn the_other_schemas_accept_a_sample() {
    for (name, sample) in [
        ("spawn-request", serde_json::to_value(request()).unwrap()),
        ("session-record", serde_json::to_value(record()).unwrap()),
        (
            "input-result",
            serde_json::to_value(input_result()).unwrap(),
        ),
        ("attach-options", serde_json::to_value(options()).unwrap()),
        (
            "service-limits",
            serde_json::to_value(limits_service()).unwrap(),
        ),
        (
            "pump-report",
            serde_json::to_value(PumpReport {
                more: true,
                events_posted: 2,
            })
            .unwrap(),
        ),
        (
            "features",
            serde_json::to_value(Features {
                names: BTreeSet::from([Feature::Silence]),
                service_preamble_versions: vec![1],
            })
            .unwrap(),
        ),
        (
            "cancel-result",
            serde_json::to_value(CancelResult::TooLate).unwrap(),
        ),
    ] {
        assert!(validator_ok(&schema_of(name), &sample), "{name}: {sample}");
    }
    // Core A3-1: the default policy is `All`, and a spawn request may omit the policy.
    let bare: SpawnRequest = serde_json::from_value(
        json!({ "argv": ["sh"], "env": {}, "cwd": "/", "size": { "rows": 1, "cols": 1 } }),
    )
    .unwrap();
    assert_eq!(bare.notification_policy, None);
    assert_eq!(NotificationPolicy::default(), NotificationPolicy::All);
}

// Core A2-5: a service id is 32 bytes and travels as 64 hex digits.
#[test]
fn a_service_id_is_64_hex_digits() {
    let json = round_trip(&svc());
    assert_eq!(json, json!("07".repeat(32)));
    assert!(serde_json::from_value::<ServiceId>(json!("07")).is_err());
    assert!(serde_json::from_value::<ServiceId>(json!("zz".repeat(32))).is_err());
}

// Core SV-3, Core A2-5: a refused send takes no ownership, so the caller retries the same frame without cloning it.
#[test]
fn a_refused_send_can_retry_the_same_frame() {
    #[allow(dead_code)]
    fn retry(core: &mut dyn CoreApi, id: &ServiceId, frame: &OutboundFrame) {
        if core.service_send(id, 0, frame) == Err(SendError::Backpressured) {
            let _ = core.service_send(id, 0, frame);
        }
    }
}

// Core ST-7: the record keeps the original 5.1A fields, and a pending record has a null byte count.
#[test]
fn an_input_record_keeps_every_semantic_field_and_a_pending_count_is_null() {
    let mut key = record_entry();
    key.key = Some(Key::Char("a".into()));
    key.shifted_key = Some(Key::Char("A".into()));
    key.base_layout_key = Some(Key::Char("q".into()));
    key.text = Some("á".into());
    key.repeat = Some(3);
    key.mods = Some(vec![Modifier::Shift]);
    let json = round_trip(&key);
    for field in ["shifted_key", "base_layout_key", "text", "repeat"] {
        assert!(json.get(field).is_some(), "{field}");
    }
    assert_eq!(json["pty_bytes"], Value::Null);
    let mut paste = record_entry();
    paste.kind = InputKind::Paste;
    paste.require_bracketed = Some(true);
    assert_eq!(round_trip(&paste)["require_bracketed"], json!(true));
    let update = InputStatusUpdate {
        cursor: InputCursor(3),
        updates: InputCursor(1),
        state: InputRecordState::Pending,
        pty_bytes: None,
    };
    assert_eq!(round_trip(&update)["pty_bytes"], Value::Null);
}

// Core 3, Core A2-3: the schema and the reader agree on known malformed data and on unknown data of the accepted shapes.
#[test]
fn the_open_set_schema_agrees_with_the_reader() {
    let schema = schema_of("event");
    for reason in [
        json!({ "SessionEnded": {} }),
        json!({ "FutureClose": 7 }),
        json!({ "BadFrame": 3 }),
        json!({ "SessionEnded": {}, "Extra": {} }),
        json!("SessionEnded"),
        json!(7),
    ] {
        let event = json!({ "RouteClosed": { "route": 1, "reason": reason } });
        assert!(!validator_ok(&schema, &event), "schema accepts {event}");
        assert!(
            serde_json::from_value::<Event>(event.clone()).is_err(),
            "reader accepts {event}"
        );
    }
    for reason in [
        json!("FutureClose"),
        json!({ "FutureClose": { "x": 1 } }),
        json!({ "BadFrame": { "code": "nope" } }),
    ] {
        let event = json!({ "RouteClosed": { "route": 1, "reason": reason } });
        assert!(validator_ok(&schema, &event), "schema rejects {event}");
        assert!(
            serde_json::from_value::<Event>(event.clone()).is_ok(),
            "reader rejects {event}"
        );
    }
}

// Core 3, Core 13 (rule 3): every enum has explicit values, and `Wake` has its serde form.
#[test]
fn wake_round_trips_and_the_transport_has_explicit_values() {
    rt_all(all!(Wake; Wake::Woken => Wake::Woken, Wake::TimedOut => Wake::TimedOut));
    assert!(validator_ok(&schema_of("wake"), &json!("Woken")));
    let stream = RouteTransport::Stream(StreamEndpoint::new(5u8));
    assert!(matches!(stream, RouteTransport::Stream(_)));
    assert_eq!(StreamEndpoint::new(9u32).downcast::<u32>().unwrap(), 9);
}

// Core A17-1, A17-4: the WebRTC operation, value, code, start reason, feature, option and limits are gone, so a reader refuses them.
#[test]
fn the_removed_webrtc_names_are_refused() {
    let op = json!({ "AttachWebRtc": { "client": "c", "session": "s", "offer": "o", "expected_fingerprint": "f",
        "options": { "file_directory": "/tmp/f" } } });
    assert!(serde_json::from_value::<Op>(op.clone()).is_err());
    assert!(!validator_ok(&schema_of("op"), &op));
    assert!(serde_json::from_value::<Feature>(json!("route_transport:webrtc")).is_err());
    assert!(serde_json::from_value::<ErrorCode>(
        json!({ "InvalidOffer": { "reason": "bad_offer" } })
    )
    .is_err());
    for limit in [
        "max_connect_deadline",
        "default_route_chunk_bytes",
        "route_send_buffer_bytes",
    ] {
        assert!(
            serde_json::from_value::<CoreLimits>(json!({ limit: 1 })).is_err(),
            "{limit}"
        );
    }
    // The non-exhaustive readers map a removed name to `Other`.
    assert_eq!(
        serde_json::from_value::<StartFailReason>(json!("WebRtcFailed")).unwrap(),
        StartFailReason::Other
    );
    assert_eq!(
        serde_json::from_value::<UnsupportedReason>(json!("binding_auth_unavailable")).unwrap(),
        UnsupportedReason::Other
    );
    for reason in ["BadPeer", "NotConnected"] {
        assert_eq!(
            serde_json::from_value::<RouteCloseReason>(json!(reason)).unwrap(),
            RouteCloseReason::Other
        );
    }
}

// Codec A.1: `ProtocolErrorCode` is read as a bare name only, and the schema says the same.
#[test]
fn a_protocol_error_code_is_string_only_in_schema_and_reader() {
    let schema = schema_of("event");
    for code in [json!({ "FutureCode": {} }), json!(7), json!({ "x": 1 })] {
        let event =
            json!({ "RouteClosed": { "route": 1, "reason": { "BadFrame": { "code": code } } } });
        assert!(!validator_ok(&schema, &event), "schema accepts {event}");
        assert!(
            serde_json::from_value::<Event>(event.clone()).is_err(),
            "reader accepts {event}"
        );
    }
    let event = json!({ "RouteClosed": { "route": 1, "reason": { "BadFrame": { "code": "future_code" } } } });
    assert!(validator_ok(&schema, &event));
    assert!(serde_json::from_value::<Event>(event).is_ok());
}

// Core A6-3, Core A6-4: the spelling of the `Remove` completion. A path list exists only with `delete_failed`.
#[test]
fn remove_report_spelling() {
    let report = |uploads| OpOutput::RemoveReport(RemoveReport { uploads });
    let deleted = serde_json::to_value(report(UploadsOutcome::Deleted)).unwrap();
    assert_eq!(
        deleted,
        json!({ "remove_report": { "uploads": "deleted" } })
    );
    let failed = report(UploadsOutcome::NotDeleted(NotDeleted::DeleteFailed {
        paths: vec!["/u/a".into()],
    }));
    assert_eq!(
        serde_json::to_value(&failed).unwrap(),
        json!({ "remove_report": { "uploads": { "not_deleted": { "reason": "delete_failed", "paths": ["/u/a"] } } } })
    );
    let unknown = report(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown));
    assert_eq!(
        serde_json::to_value(&unknown).unwrap(),
        json!({ "remove_report": { "uploads": { "not_deleted": { "reason": "outcome_unknown" } } } })
    );
    for v in [
        deleted,
        serde_json::to_value(&failed).unwrap(),
        serde_json::to_value(&unknown).unwrap(),
    ] {
        let back: OpOutput = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(back).unwrap(), v);
    }
}

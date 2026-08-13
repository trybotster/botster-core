#![allow(missing_docs)]

use botster_terminal_protocol_client::{
    AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase, TerminalEvent,
    TerminalOutput,
};

#[test]
fn snapshot_and_terminal_output_share_envelope_fields_and_remain_distinct() {
    let snapshot = Snapshot::from_bytes("s", "sub", b"GHOSTSNP", SnapshotPhase::Ready);
    let live = TerminalOutput::from_bytes("s", "sub", b"live");
    let snapshot_json = serde_json::to_value(&snapshot).expect("snapshot json");
    let live_json = serde_json::to_value(&live).expect("live json");
    for field in ["payload_base64", "payload_encoding", "bytes"] {
        assert!(
            snapshot_json.get(field).is_some(),
            "snapshot missing {field}"
        );
        assert!(live_json.get(field).is_some(), "live missing {field}");
    }
    assert_eq!(snapshot_json["payload_encoding"], "base64");
    assert_eq!(live_json["payload_encoding"], "base64");
    assert!(snapshot_json.get("phase").is_some());
    assert!(live_json.get("phase").is_none());
    assert_ne!(
        std::mem::discriminant(&TerminalEvent::Snapshot(snapshot)),
        std::mem::discriminant(&TerminalEvent::TerminalOutput(live))
    );
}

#[test]
fn live_output_rejects_legacy_data_field() {
    let error = serde_json::from_value::<TerminalOutput>(serde_json::json!({
        "session_id": "s",
        "subscription_id": "sub",
        "payload_base64": "bGl2ZQ==",
        "payload_encoding": "base64",
        "bytes": 4,
        "data": "live"
    }))
    .expect_err("legacy data must fail");
    assert!(
        error
            .to_string()
            .contains("legacy terminal_output data field is rejected"),
        "{error}"
    );
}

#[test]
fn snapshot_phase_is_required() {
    let error = serde_json::from_value::<Snapshot>(serde_json::json!({
        "session_id": "s",
        "subscription_id": "sub",
        "payload_base64": "R0hPU1RTTlA=",
        "payload_encoding": "base64",
        "bytes": 8
    }))
    .expect_err("missing phase must fail");
    assert!(error.to_string().contains("phase"), "{error}");
}

#[test]
fn process_exit_omits_code_when_none() {
    let event = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: None,
    };
    let json = serde_json::to_value(&event).expect("json");
    assert!(json.get("code").is_none(), "{json}");
    let with_code = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: Some(1),
    };
    assert_eq!(serde_json::to_value(&with_code).expect("json")["code"], 1);
}

#[test]
fn semantic_events_round_trip_through_opaque_frames() {
    let snapshot = Snapshot::from_bytes("s", "sub", b"GHOSTSNP", SnapshotPhase::History);
    let frame = snapshot.to_frame().expect("frame");
    match TerminalEvent::from_frame(&frame).expect("decode") {
        TerminalEvent::Snapshot(decoded) => {
            assert_eq!(decoded.phase, SnapshotPhase::History);
            assert_eq!(decoded.decoded_bytes().expect("bytes"), b"GHOSTSNP");
        }
        other => panic!("expected snapshot, got {other:?}"),
    }

    let attach = AttachState {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        state: AttachStateKind::SnapshotHistoryIncomplete,
    };
    let frame = attach.to_frame().expect("frame");
    match TerminalEvent::from_frame(&frame).expect("decode") {
        TerminalEvent::AttachState(decoded) => {
            assert_eq!(decoded.state, AttachStateKind::SnapshotHistoryIncomplete);
        }
        other => panic!("expected attach state, got {other:?}"),
    }
}

#![allow(missing_docs)]

use std::path::PathBuf;

use botster_terminal_protocol_client::{
    TerminalEvent, TerminalFrame, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
};

#[test]
fn ready_then_history_fixture_advertises_required_optional_feature() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/ready-then-history-event-order.json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json");
    let features = value["compatibility"]["features"]
        .as_array()
        .expect("features");
    let required = value["required_features"].as_array().expect("required");
    assert!(features
        .iter()
        .any(|feature| feature == FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    assert!(required
        .iter()
        .any(|feature| feature == FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));

    let mut saw_ready = false;
    let mut saw_history = false;
    let mut saw_finish = false;
    for event in value["events"].as_array().expect("events") {
        let frame = TerminalFrame::from_bytes(event.to_string().as_bytes()).expect("frame");
        if let TerminalEvent::Snapshot(snapshot) =
            TerminalEvent::from_frame(&frame).expect("decode")
        {
            match snapshot.phase {
                botster_terminal_protocol_client::SnapshotPhase::Ready => saw_ready = true,
                botster_terminal_protocol_client::SnapshotPhase::History => saw_history = true,
                botster_terminal_protocol_client::SnapshotPhase::Finish => saw_finish = true,
            }
        }
    }
    assert!(saw_ready && saw_history && saw_finish);
}

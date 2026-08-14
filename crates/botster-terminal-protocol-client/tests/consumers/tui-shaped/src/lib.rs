//! Isolated TUI-shaped consumer over the semantic client crate.

use botster_terminal_protocol_client::{
    AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase, TerminalOutput,
};

pub fn encode_representative_events() -> Vec<Vec<u8>> {
    let snapshot = Snapshot::from_bytes("s", "sub", b"ready", SnapshotPhase::Ready)
        .to_frame()
        .expect("snapshot frame")
        .to_bytes()
        .expect("snapshot bytes");
    let live = TerminalOutput::from_bytes("s", "sub", b"out")
        .to_frame()
        .expect("live frame")
        .to_bytes()
        .expect("live bytes");
    let exit = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: None,
    }
    .to_frame()
    .expect("exit frame")
    .to_bytes()
    .expect("exit bytes");
    let attach = AttachState {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        state: AttachStateKind::Attached,
    }
    .to_frame()
    .expect("attach frame")
    .to_bytes()
    .expect("attach bytes");
    vec![snapshot, live, exit, attach]
}

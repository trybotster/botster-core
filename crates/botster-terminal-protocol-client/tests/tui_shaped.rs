#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

use botster_terminal_protocol_client::{
    AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase, TerminalEvent,
    TerminalOutput,
};

#[test]
fn tui_shaped_consumer_constructs_and_serializes_semantic_events() {
    let snapshot = Snapshot::from_bytes("s", "sub", b"ready", SnapshotPhase::Ready);
    let live = TerminalOutput::from_bytes("s", "sub", b"out");
    let exit = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: None,
    };
    let attach = AttachState {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        state: AttachStateKind::Attached,
    };
    for event in [
        TerminalEvent::Snapshot(snapshot),
        TerminalEvent::TerminalOutput(live),
        TerminalEvent::ProcessExit(exit),
        TerminalEvent::AttachState(attach),
    ] {
        let frame = event.to_frame().expect("encode");
        let decoded = TerminalEvent::from_frame(&frame).expect("decode");
        assert_eq!(decoded, event);
    }
}

#[test]
fn isolated_tui_shaped_consumer_compiles_against_client_crate() {
    let consumer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/consumers/tui-shaped");
    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", consumer.join("target"))
        .output()
        .expect("cargo check tui-shaped consumer");
    assert!(
        output.status.success(),
        "tui-shaped consumer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn client_crate_tree_excludes_runtime_and_hub_dependencies() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "botster-terminal-protocol-client",
            "--prefix",
            "none",
        ])
        .current_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|crates| crates.parent())
                .expect("workspace"),
        )
        .output()
        .expect("cargo tree");
    assert!(output.status.success());
    let tree = String::from_utf8_lossy(&output.stdout);
    for crate_name in [
        "botster-core ",
        "botster-core-daemon",
        "botster-hub ",
        "botster-hub-client",
    ] {
        assert!(
            !tree.contains(crate_name),
            "forbidden dependency {crate_name} in tree:\n{tree}"
        );
    }
}

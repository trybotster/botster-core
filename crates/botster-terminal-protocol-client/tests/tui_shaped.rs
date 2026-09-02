#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

use botster_terminal_protocol_client::{
    decode_terminal_input, encode_paste, encode_terminal_input, AttachState, AttachStateKind,
    ProcessExit, Snapshot, SnapshotPhase, TerminalEvent, TerminalInputCommand, TerminalInputKind,
    TerminalInputRejection, TerminalInputResult, TerminalModeFlags, TerminalOutput,
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
        TerminalEvent::InputResult(TerminalInputResult {
            operation_id: None,
            subscription_id: "sub".into(),
            kind: TerminalInputKind::Input,
            admitted: true,
            bytes_written: 1,
            mode_generation: 1,
            mode_revision: 1,
            mode_flags: TerminalModeFlags {
                kitty_enabled: false,
                cursor_visible: true,
                bracketed_paste: false,
                mouse_mode: 0,
                alt_screen: false,
                focus_reporting: false,
                application_cursor: false,
            },
            rejection: None,
        }),
    ] {
        let frame = event.to_frame().expect("encode");
        let decoded = TerminalEvent::from_frame(&frame).expect("decode");
        assert_eq!(decoded, event);
    }

    for command in [
        TerminalInputCommand::Input {
            data: b"hi".to_vec(),
        },
        TerminalInputCommand::ModeGatedInput {
            mode_generation: 2,
            mode_revision: 3,
            data: vec![0xff],
        },
        TerminalInputCommand::Resize { rows: 24, cols: 80 },
    ] {
        let frame = encode_terminal_input(&command).expect("encode command");
        assert_eq!(
            decode_terminal_input(&frame).expect("decode command"),
            command
        );
    }
    assert_eq!(
        TerminalInputRejection::ALL.len(),
        9,
        "published rejection inventory must stay live-owner only"
    );
    assert!(!format!("{:?}", TerminalInputRejection::ALL).contains("Malformed"));
    assert!(!format!("{:?}", TerminalInputRejection::ALL).contains("QueueOverflow"));
    let paste = encode_paste(1, 2, 3, b"shared client helper").expect("paste helper");
    assert_eq!(paste.first().expect("begin").as_bytes()[1], 4);
    assert_eq!(paste.last().expect("commit").as_bytes()[1], 6);
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

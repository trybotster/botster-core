//! Hub-shaped consumer proof against the complete public API.
#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use botster_terminal_protocol::{
    Attach, Detach, Resize, SendInput, TerminalCapabilitySet, TerminalCompatibility,
    TerminalCompatibilityRequirement, TerminalFrame, TerminalInputFrame, FEATURE_RESIZE,
    FEATURE_TERMINAL_STREAMING, FEATURE_TRANSPORT_DUPLEX_BINARY, PROTOCOL,
};

#[test]
fn hub_shaped_consumer_forwards_requests_and_opaque_frames() {
    let attach = Attach {
        session_id: "session-1".into(),
        subscription_id: "sub-1".into(),
    };
    let json = serde_json::to_value(&attach).expect("serialize attach");
    assert_eq!(json["type"], "attach");
    assert_eq!(json["session_id"], "session-1");
    assert!(json.get("phase").is_none());

    let detach = serde_json::to_value(&Detach {
        session_id: "session-1".into(),
        subscription_id: "sub-1".into(),
    })
    .expect("serialize detach");
    assert_eq!(detach["type"], "detach");

    let input = serde_json::to_value(&SendInput {
        session_id: "session-1".into(),
        data: "a".into(),
    })
    .expect("serialize send_input");
    assert_eq!(input["type"], "send_input");

    let resize = serde_json::to_value(&Resize {
        session_id: "session-1".into(),
        rows: 24,
        cols: 80,
    })
    .expect("serialize resize");
    assert_eq!(resize["type"], "resize");

    let snapshot_json = serde_json::json!({
        "type": "snapshot",
        "session_id": "session-1",
        "subscription_id": "sub-1",
        "payload_base64": "R0hPU1RTTlA=",
        "payload_encoding": "base64",
        "bytes": 8,
        "phase": "ready"
    });
    let frame = TerminalFrame::from_bytes(snapshot_json.to_string().as_bytes()).expect("frame");
    let emitted = frame.to_bytes().expect("emit");
    let round_trip: serde_json::Value = serde_json::from_slice(&emitted).expect("json");
    assert_eq!(round_trip["type"], "snapshot");
    assert_eq!(PROTOCOL, "botster-terminal-v1");
    let _ = TerminalCompatibility::current();
    let _ = TerminalCompatibilityRequirement::current();
    let empty = TerminalCapabilitySet::empty();
    assert!(empty.is_empty());
    let negotiated = TerminalCapabilitySet::from_tokens([
        FEATURE_TERMINAL_STREAMING,
        FEATURE_RESIZE,
        FEATURE_TRANSPORT_DUPLEX_BINARY,
    ])
    .expect("Hub can build a set from protocol tokens");
    assert!(negotiated.contains(FEATURE_TERMINAL_STREAMING));
    assert!(negotiated.contains(FEATURE_RESIZE));
    assert!(negotiated.contains(FEATURE_TRANSPORT_DUPLEX_BINARY));

    let input_bytes = {
        let mut bytes = vec![1, 1, 0, 3];
        bytes.extend_from_slice(b"abc");
        bytes
    };
    let input_frame = TerminalInputFrame::from_bytes(&input_bytes).expect("opaque input");
    assert_eq!(input_frame.to_bytes(), input_bytes);
    assert_eq!(input_frame.as_bytes(), input_bytes.as_slice());

    let paste_begin_bytes = vec![
        1, 4, 0, 24, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3,
    ];
    let paste_begin =
        TerminalInputFrame::from_bytes(&paste_begin_bytes).expect("opaque paste begin");
    assert_eq!(paste_begin.to_bytes(), paste_begin_bytes);
}

#[test]
fn isolated_hub_shaped_consumer_compiles_against_opaque_crate_only() {
    let consumer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/consumers/hub-shaped");
    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", consumer.join("target"))
        .output()
        .expect("cargo check hub-shaped consumer");
    assert!(
        output.status.success(),
        "hub-shaped consumer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hub_shaped_complete_public_api_cannot_name_semantic_bodies() {
    let imports = compile_hub_consumer(
        "use botster_terminal_protocol::{AttachState, ProcessExit, Snapshot, SnapshotPhase, TerminalOutput};\n",
    );
    for token in [
        "SnapshotPhase",
        "AttachState",
        "TerminalOutput",
        "Snapshot",
        "ProcessExit",
    ] {
        assert!(
            imports.contains(token),
            "import probe must search `{token}` on the complete public API:\n{imports}"
        );
    }

    let fields = compile_hub_consumer(
        r#"
            fn inspect(frame: botster_terminal_protocol::TerminalFrame) {
                let _ = frame.phase;
                let _ = frame.state;
                let _ = frame.history;
                let _ = frame.payload;
            }
            fn inspect_input(frame: botster_terminal_protocol::TerminalInputFrame) {
                let _ = frame.payload;
                let _ = frame.data;
                let _ = frame.mode_generation;
                let _ = frame.rows;
                let _ = frame.cols;
            }
        "#,
    );
    for token in [
        "phase",
        "state",
        "history",
        "payload",
        "data",
        "mode_generation",
        "rows",
        "cols",
    ] {
        assert!(
            fields.contains(token),
            "field probe must search `{token}` on opaque frames:\n{fields}"
        );
    }

    let client = compile_hub_consumer(
        "fn client_path() { let _ = botster_terminal_protocol_client::SnapshotPhase::Ready; }\n",
    );
    assert!(
        client.contains("botster_terminal_protocol_client"),
        "client-path probe must fail without the client crate:\n{client}"
    );
}

fn compile_hub_consumer(source: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = root.join("target/hub-shaped-forbidden");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(scratch.join("src")).expect("scratch src");
    fs::write(
        scratch.join("Cargo.toml"),
        format!(
            r#"
                [workspace]

                [package]
                name = "hub-shaped-forbidden"
                version = "0.0.0"
                edition = "2021"
                publish = false

                [dependencies]
                botster-terminal-protocol = {{ path = "{}" }}
            "#,
            escape_toml_path(&root)
        ),
    )
    .expect("write manifest");
    fs::write(scratch.join("src/lib.rs"), source).expect("write source");
    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(&scratch)
        .env("CARGO_TARGET_DIR", scratch.join("target"))
        .output()
        .expect("cargo check");
    assert!(
        !output.status.success(),
        "forbidden Hub consumer must fail to compile"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn escape_toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

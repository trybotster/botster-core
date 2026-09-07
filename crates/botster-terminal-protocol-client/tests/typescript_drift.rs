#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use botster_terminal_protocol_client::{
    terminal_protocol_typescript, CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, PROTOCOL, PROTOCOL_VERSION,
};
use serde_json::Value;

#[test]
fn rewrite_generated_typescript_when_requested() {
    if std::env::var("REWRITE_TERMINAL_PROTOCOL_TS").is_err() {
        return;
    }
    let generated = terminal_protocol_typescript();
    std::fs::write(generated_path(), &generated).expect("write generated ts");
    std::fs::write(package_path("terminal-protocol.ts"), generated).expect("write package ts");
}

#[test]
fn generated_typescript_matches_committed_artifact() {
    let committed = std::fs::read_to_string(generated_path()).expect("committed ts");
    assert_eq!(
        terminal_protocol_typescript(),
        committed,
        "generated TypeScript drifted from the committed artifact"
    );
}

#[test]
fn package_mirror_matches_generated_artifact() {
    let generated = std::fs::read_to_string(generated_path()).expect("generated ts");
    let mirrored =
        std::fs::read_to_string(package_path("terminal-protocol.ts")).expect("package ts");
    assert_eq!(generated, mirrored);
}

#[test]
fn emitted_constants_come_from_rust_protocol_constants() {
    let ts = terminal_protocol_typescript();
    assert!(ts.contains(&format!("export const PROTOCOL = \"{PROTOCOL}\";")));
    assert!(ts.contains(&format!(
        "export const PROTOCOL_VERSION = {PROTOCOL_VERSION};"
    )));
    assert!(ts.contains(&format!(
        "export const CONFORMANCE_FIXTURE_REVISION = {CONFORMANCE_FIXTURE_REVISION};"
    )));
    assert!(ts.contains(&format!(
        "export const PACKAGE_VERSION = \"{}\";",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(ts.contains(&format!(
        "export const FEATURE_TERMINAL_STREAMING = \"{FEATURE_TERMINAL_STREAMING}\";"
    )));
    assert!(ts.contains(&format!(
        "export const FEATURE_RESIZE = \"{FEATURE_RESIZE}\";"
    )));
    assert!(ts.contains(&format!(
        "export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = \"{FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY}\";"
    )));
    assert!(ts.contains(&format!(
        "export const FEATURE_TRANSPORT_DUPLEX_BINARY = \"{FEATURE_TRANSPORT_DUPLEX_BINARY}\";"
    )));
    assert!(ts.contains(&format!(
        "export const MAX_PASTE_BYTES = {MAX_PASTE_BYTES};"
    )));
    assert!(ts.contains(&format!(
        "export const MAX_PASTE_CHUNKS = {MAX_PASTE_CHUNKS};"
    )));
    for function in [
        "decodeTerminalBody",
        "encodeTerminalBody",
        "encodeRawBytes",
        "encodeKey",
        "encodeMouse",
        "encodeFocus",
        "encodeResize",
        "encodePaste",
        "encodePasteAbort",
        "terminalKeyFromCode",
        "toOperationId",
    ] {
        assert!(
            ts.contains(&format!("export function {function}(")),
            "generated TypeScript must export {function}"
        );
    }
    assert!(
        ts.contains("Number.isSafeInteger(operation_id)"),
        "numeric operation ids must be rejected when unsafe"
    );
    assert!(
        ts.contains("from_epoch: bodyView.getUint32(0, true)"),
        "route_resync must decode from_epoch and to_epoch"
    );
}

#[test]
fn package_event_order_fixture_mirrors_client_crate() {
    let crate_fixture = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/ready-then-history-event-order.json"),
    )
    .expect("crate fixture");
    let package_fixture =
        std::fs::read_to_string(package_path("fixtures/ready-then-history-event-order.json"))
            .expect("package fixture");
    assert_eq!(crate_fixture, package_fixture);
    let fixture: Value = serde_json::from_str(&crate_fixture).expect("fixture json");
    assert_eq!(
        fixture["compatibility"]["conformance_fixture_revision"],
        CONFORMANCE_FIXTURE_REVISION
    );
    assert_eq!(fixture["stream_epoch"], 0);
}

#[test]
fn package_metadata_matches_rust_protocol_constants() {
    let metadata: Value = serde_json::from_str(
        &std::fs::read_to_string(package_path("metadata.json")).expect("metadata"),
    )
    .expect("json");
    assert_eq!(
        metadata["package_version"],
        env!("CARGO_PKG_VERSION"),
        "package metadata version"
    );
    assert_eq!(metadata["protocol"], PROTOCOL);
    assert_eq!(metadata["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(
        metadata["conformance_fixture_revision"],
        CONFORMANCE_FIXTURE_REVISION
    );
    let features = metadata["features"]
        .as_array()
        .expect("features")
        .iter()
        .map(|value| value.as_str().expect("feature string").to_string())
        .collect::<BTreeSet<_>>();
    assert!(features.contains(FEATURE_TERMINAL_STREAMING));
    assert!(features.contains(FEATURE_RESIZE));
    assert!(features.contains(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    assert!(features.contains(FEATURE_TRANSPORT_DUPLEX_BINARY));
}

fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generated/terminal-protocol.ts")
}

fn package_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/terminal-protocol")
        .join(relative)
}

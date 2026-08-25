#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use botster_terminal_protocol_client::{
    terminal_protocol_typescript, Attach, AttachState, AttachStateKind, Detach, PayloadEncoding,
    ProcessExit, Resize, SendInput, Snapshot, SnapshotPhase, TerminalCompatibility,
    TerminalCompatibilityRequirement, TerminalFrame, TerminalInputKind, TerminalInputRejection,
    TerminalInputResult, TerminalModeFlags, TerminalOutput, CONFORMANCE_FIXTURE_REVISION,
    FEATURE_RESIZE, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, PROTOCOL, PROTOCOL_VERSION,
};
use serde::Serialize;
use serde_json::Value;

#[test]
fn rewrite_generated_typescript_when_requested() {
    if std::env::var("REWRITE_TERMINAL_PROTOCOL_TS").is_err() {
        return;
    }
    let generated = terminal_protocol_typescript();
    std::fs::write(generated_path(), &generated).expect("write generated ts");
    std::fs::write(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/terminal-protocol/terminal-protocol.ts"),
        generated,
    )
    .expect("write package ts");
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
    let mirrored = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/terminal-protocol/terminal-protocol.ts"),
    )
    .expect("package ts");
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
    assert!(ts.contains("export function encodeTerminalInput"));
    assert!(ts.contains("export function encodeModeGatedInput"));
    assert!(ts.contains("export function encodeResize"));
}

#[test]
fn serde_wire_shapes_match_generated_typescript() {
    let ts = terminal_protocol_typescript();
    let phase_values = enum_values(SnapshotPhase::ALL);
    let state_values = enum_values(AttachStateKind::ALL);
    let encoding_values = enum_values(PayloadEncoding::ALL);
    let kind_values = enum_values(TerminalInputKind::ALL);
    let rejection_values = enum_values(TerminalInputRejection::ALL);
    assert_ts_union(&ts, "SnapshotPhase", &phase_values);
    assert_ts_union(&ts, "AttachStateKind", &state_values);
    assert_ts_union(&ts, "PayloadEncoding", &encoding_values);
    assert_ts_union(&ts, "TerminalInputKind", &kind_values);
    assert_ts_union(&ts, "TerminalInputRejection", &rejection_values);

    compare_interface(
        &ts,
        "Attach",
        &json(&Attach {
            session_id: "s".into(),
            subscription_id: "sub".into(),
        }),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "Detach",
        &json(&Detach {
            session_id: "s".into(),
            subscription_id: "sub".into(),
        }),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "SendInput",
        &json(&SendInput {
            session_id: "s".into(),
            data: "x".into(),
        }),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "Resize",
        &json(&Resize {
            session_id: "s".into(),
            rows: 24,
            cols: 80,
        }),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "Snapshot",
        &event_json(
            &Snapshot::from_bytes("s", "sub", b"GHOSTSNP", SnapshotPhase::Ready)
                .to_frame()
                .expect("snapshot frame"),
        ),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "TerminalOutput",
        &event_json(
            &TerminalOutput::from_bytes("s", "sub", b"live")
                .to_frame()
                .expect("live frame"),
        ),
        &[],
        &phase_values,
        &state_values,
    );

    let exit_with_code = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: Some(1),
    };
    let exit_without_code = ProcessExit {
        session_id: "s".into(),
        subscription_id: "sub".into(),
        code: None,
    };
    let present = event_json(&exit_with_code.to_frame().expect("exit frame"));
    let omitted = event_json(&exit_without_code.to_frame().expect("omitted exit frame"));
    assert!(
        present.get("code").is_some(),
        "populated exit must emit code"
    );
    assert!(
        omitted.get("code").is_none(),
        "None exit code must omit the JSON key"
    );
    compare_interface(
        &ts,
        "ProcessExit",
        &present,
        &["code"],
        &phase_values,
        &state_values,
    );

    compare_interface(
        &ts,
        "AttachState",
        &event_json(
            &AttachState {
                session_id: "s".into(),
                subscription_id: "sub".into(),
                state: AttachStateKind::Attached,
            }
            .to_frame()
            .expect("attach frame"),
        ),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "TerminalCompatibility",
        &json(&TerminalCompatibility::current()),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "TerminalCompatibilityRequirement",
        &json(&TerminalCompatibilityRequirement::current()),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "TerminalModeFlags",
        &json(&TerminalModeFlags {
            kitty_enabled: true,
            cursor_visible: true,
            bracketed_paste: true,
            mouse_mode: 1,
            alt_screen: true,
            focus_reporting: true,
            application_cursor: true,
        }),
        &[],
        &phase_values,
        &state_values,
    );
    compare_interface(
        &ts,
        "TerminalInputResult",
        &event_json(
            &TerminalInputResult {
                subscription_id: "sub".into(),
                kind: TerminalInputKind::ModeGatedInput,
                admitted: false,
                bytes_written: 0,
                mode_generation: 1,
                mode_revision: 2,
                mode_flags: TerminalModeFlags {
                    kitty_enabled: false,
                    cursor_visible: false,
                    bracketed_paste: false,
                    mouse_mode: 0,
                    alt_screen: false,
                    focus_reporting: false,
                    application_cursor: false,
                },
                rejection: Some(TerminalInputRejection::StaleMode),
            }
            .to_frame()
            .expect("input_result frame"),
        ),
        &["rejection"],
        &phase_values,
        &state_values,
    );
}

#[test]
fn package_event_order_fixture_mirrors_client_crate() {
    let crate_fixture = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/ready-then-history-event-order.json"),
    )
    .expect("crate fixture");
    let package_fixture = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/terminal-protocol/fixtures/ready-then-history-event-order.json"),
    )
    .expect("package fixture");
    assert_eq!(crate_fixture, package_fixture);
}

#[test]
fn package_metadata_matches_rust_protocol_constants() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/terminal-protocol/metadata.json");
    let metadata: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("metadata")).expect("json");
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

fn json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serialize rust dto")
}

fn event_json(frame: &TerminalFrame) -> Value {
    serde_json::from_slice(&frame.to_bytes().expect("frame bytes")).expect("frame json")
}

fn enum_values<T: Serialize>(variants: &[T]) -> BTreeSet<String> {
    variants
        .iter()
        .map(|variant| match json(variant) {
            Value::String(text) => text,
            other => panic!("expected enum string, got {other}"),
        })
        .collect()
}

fn assert_ts_union(ts: &str, name: &str, expected: &BTreeSet<String>) {
    let header = format!("export type {name} =");
    let start = ts.find(&header).unwrap_or_else(|| panic!("missing {name}"));
    let body = &ts[start + header.len()..];
    let end = body.find("export ").unwrap_or(body.len());
    let mut found = BTreeSet::new();
    for line in body[..end].lines() {
        let line = line
            .trim()
            .trim_start_matches('|')
            .trim()
            .trim_end_matches(';');
        if let Some(value) = line
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        {
            found.insert(value.to_string());
        }
    }
    assert_eq!(found, *expected, "union {name} drifted from rust serde");
}

fn compare_interface(
    ts: &str,
    name: &str,
    populated: &Value,
    optional: &[&str],
    phases: &BTreeSet<String>,
    states: &BTreeSet<String>,
) {
    let object = populated.as_object().expect("json object");
    let rust_fields: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let parsed = parse_interface(ts, name);
    let ts_fields: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
    assert_eq!(
        ts_fields, rust_fields,
        "field set mismatch for {name}: rust={rust_fields:?} ts={ts_fields:?}"
    );
    for (field, value) in object {
        let (ts_optional, ts_type) = parsed
            .get(field)
            .unwrap_or_else(|| panic!("{name}.{field} missing from TypeScript"));
        let expected_optional = optional.contains(&field.as_str());
        assert_eq!(
            *ts_optional, expected_optional,
            "{name}.{field} optionality"
        );
        assert_eq!(
            ts_type.as_str(),
            expected_ts_type(field, value, phases, states),
            "{name}.{field} type"
        );
    }
}

fn expected_ts_type(
    field: &str,
    value: &Value,
    phases: &BTreeSet<String>,
    states: &BTreeSet<String>,
) -> String {
    match value {
        Value::String(text) if field == "type" || field == "payload_encoding" => {
            format!("\"{text}\"")
        }
        Value::String(text) if field == "phase" && phases.contains(text) => {
            "SnapshotPhase".to_string()
        }
        Value::String(text) if field == "state" && states.contains(text) => {
            "AttachStateKind".to_string()
        }
        Value::String(_) if field == "kind" => "TerminalInputKind".to_string(),
        Value::String(_) if field == "rejection" => "TerminalInputRejection".to_string(),
        Value::Object(_) if field == "mode_flags" => "TerminalModeFlags".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Array(items) if items.iter().all(Value::is_string) => "string[]".to_string(),
        other => panic!("unsupported rust json type for {field}: {other}"),
    }
}

fn parse_interface(source: &str, name: &str) -> BTreeMap<String, (bool, String)> {
    let header = format!("export interface {name} {{");
    let start = source
        .find(&header)
        .unwrap_or_else(|| panic!("missing interface {name}"));
    let body = &source[start + header.len()..];
    let end = body.find('}').expect("interface end");
    let mut fields = BTreeMap::new();
    for line in body[..end].lines() {
        let line = line.trim().trim_end_matches(';');
        if line.is_empty() {
            continue;
        }
        let (name_part, ty) = line.split_once(':').expect("field colon");
        let optional = name_part.ends_with('?');
        let name = name_part.trim().trim_end_matches('?').to_string();
        fields.insert(name, (optional, ty.trim().to_string()));
    }
    fields
}

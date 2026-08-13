#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use botster_terminal_protocol_client::terminal_protocol_typescript;

struct FieldSpec {
    name: &'static str,
    ts_type: &'static str,
    optional: bool,
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
fn drift_check_is_bidirectional_with_types_and_optionality() {
    let ts = std::fs::read_to_string(generated_path()).expect("ts");
    let specs = serde_specs();
    for (interface, fields) in specs {
        let parsed = parse_interface(&ts, interface);
        let expected: BTreeSet<&str> = fields.iter().map(|field| field.name).collect();
        let found: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
        assert_eq!(
            found, expected,
            "field set mismatch for {interface}: source={expected:?} ts={found:?}"
        );
        for field in fields {
            let (optional, ty) = parsed
                .get(field.name)
                .unwrap_or_else(|| panic!("{interface}.{} missing", field.name));
            assert_eq!(
                *optional, field.optional,
                "{interface}.{} optionality",
                field.name
            );
            assert_eq!(
                ty.as_str(),
                field.ts_type,
                "{interface}.{} type",
                field.name
            );
        }
    }
}

fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generated/terminal-protocol.ts")
}

fn serde_specs() -> Vec<(&'static str, Vec<FieldSpec>)> {
    vec![
        (
            "Attach",
            vec![
                field("type", "\"attach\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
            ],
        ),
        (
            "Detach",
            vec![
                field("type", "\"detach\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
            ],
        ),
        (
            "SendInput",
            vec![
                field("type", "\"send_input\"", false),
                field("session_id", "string", false),
                field("data", "string", false),
            ],
        ),
        (
            "Resize",
            vec![
                field("type", "\"resize\"", false),
                field("session_id", "string", false),
                field("rows", "number", false),
                field("cols", "number", false),
            ],
        ),
        (
            "Snapshot",
            vec![
                field("type", "\"snapshot\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
                field("payload_base64", "string", false),
                field("payload_encoding", "\"base64\"", false),
                field("bytes", "number", false),
                field("phase", "SnapshotPhase", false),
            ],
        ),
        (
            "TerminalOutput",
            vec![
                field("type", "\"terminal_output\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
                field("payload_base64", "string", false),
                field("payload_encoding", "\"base64\"", false),
                field("bytes", "number", false),
            ],
        ),
        (
            "ProcessExit",
            vec![
                field("type", "\"process_exit\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
                field("code", "number", true),
            ],
        ),
        (
            "AttachState",
            vec![
                field("type", "\"attach_state\"", false),
                field("session_id", "string", false),
                field("subscription_id", "string", false),
                field("state", "AttachStateKind", false),
            ],
        ),
        (
            "TerminalCompatibility",
            vec![
                field("protocol", "string", false),
                field("protocol_version", "number", false),
                field("features", "string[]", false),
                field("conformance_fixture_revision", "number", false),
            ],
        ),
        (
            "TerminalCompatibilityRequirement",
            vec![
                field("protocol", "string", false),
                field("protocol_version", "number", false),
                field("required_features", "string[]", false),
                field("minimum_conformance_fixture_revision", "number", false),
                field("client_name", "string", false),
            ],
        ),
    ]
}

fn field(name: &'static str, ts_type: &'static str, optional: bool) -> FieldSpec {
    FieldSpec {
        name,
        ts_type,
        optional,
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

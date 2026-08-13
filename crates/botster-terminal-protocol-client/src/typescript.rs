//! Deterministic TypeScript emitter for the terminal protocol plane.

use botster_terminal_protocol::{
    CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    FEATURE_TERMINAL_STREAMING, PROTOCOL, PROTOCOL_VERSION,
};
use serde::Serialize;
use serde_json::Value;

use crate::{AttachStateKind, PayloadEncoding, SnapshotPhase};

/// Generate the committed TypeScript artifact from the Rust serde source.
#[must_use]
pub fn terminal_protocol_typescript() -> String {
    let mut output = String::new();
    line(
        &mut output,
        "// Generated from crates/botster-terminal-protocol-client Rust serde DTOs.",
    );
    line(
        &mut output,
        "// Regenerate/check with: cargo test -p botster-terminal-protocol-client typescript",
    );
    line(&mut output, "");
    line(
        &mut output,
        &format!("export const PROTOCOL = \"{PROTOCOL}\";"),
    );
    line(
        &mut output,
        &format!("export const PROTOCOL_VERSION = {PROTOCOL_VERSION};"),
    );
    line(
        &mut output,
        &format!("export const CONFORMANCE_FIXTURE_REVISION = {CONFORMANCE_FIXTURE_REVISION};"),
    );
    line(
        &mut output,
        &format!(
            "export const PACKAGE_VERSION = \"{}\";",
            env!("CARGO_PKG_VERSION")
        ),
    );
    line(
        &mut output,
        &format!("export const FEATURE_TERMINAL_STREAMING = \"{FEATURE_TERMINAL_STREAMING}\";"),
    );
    line(
        &mut output,
        &format!("export const FEATURE_RESIZE = \"{FEATURE_RESIZE}\";"),
    );
    line(
        &mut output,
        &format!(
            "export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = \"{FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY}\";"
        ),
    );
    line(&mut output, "");
    emit_interface(
        &mut output,
        "TerminalCompatibility",
        &[
            ("protocol", "string"),
            ("protocol_version", "number"),
            ("features", "string[]"),
            ("conformance_fixture_revision", "number"),
        ],
    );
    emit_interface(
        &mut output,
        "TerminalCompatibilityRequirement",
        &[
            ("protocol", "string"),
            ("protocol_version", "number"),
            ("required_features", "string[]"),
            ("minimum_conformance_fixture_revision", "number"),
            ("client_name", "string"),
        ],
    );
    emit_interface(
        &mut output,
        "Attach",
        &[
            ("type", "\"attach\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
        ],
    );
    emit_interface(
        &mut output,
        "Detach",
        &[
            ("type", "\"detach\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
        ],
    );
    emit_interface(
        &mut output,
        "SendInput",
        &[
            ("type", "\"send_input\""),
            ("session_id", "string"),
            ("data", "string"),
        ],
    );
    emit_interface(
        &mut output,
        "Resize",
        &[
            ("type", "\"resize\""),
            ("session_id", "string"),
            ("rows", "number"),
            ("cols", "number"),
        ],
    );
    let phases = [
        wire_string(&SnapshotPhase::Ready),
        wire_string(&SnapshotPhase::History),
        wire_string(&SnapshotPhase::Finish),
    ];
    emit_string_union(
        &mut output,
        "SnapshotPhase",
        &phases.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let attach_states = [
        wire_string(&AttachStateKind::Attaching),
        wire_string(&AttachStateKind::Attached),
        wire_string(&AttachStateKind::SnapshotHistoryIncomplete),
        wire_string(&AttachStateKind::AttachFailed),
    ];
    emit_string_union(
        &mut output,
        "AttachStateKind",
        &attach_states.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let encodings = [wire_string(&PayloadEncoding::Base64)];
    emit_string_union(
        &mut output,
        "PayloadEncoding",
        &encodings.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    emit_interface(
        &mut output,
        "Snapshot",
        &[
            ("type", "\"snapshot\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
            ("payload_base64", "string"),
            ("payload_encoding", "\"base64\""),
            ("bytes", "number"),
            ("phase", "SnapshotPhase"),
        ],
    );
    emit_interface(
        &mut output,
        "TerminalOutput",
        &[
            ("type", "\"terminal_output\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
            ("payload_base64", "string"),
            ("payload_encoding", "\"base64\""),
            ("bytes", "number"),
        ],
    );
    emit_interface(
        &mut output,
        "ProcessExit",
        &[
            ("type", "\"process_exit\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
            ("code?", "number"),
        ],
    );
    emit_interface(
        &mut output,
        "AttachState",
        &[
            ("type", "\"attach_state\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
            ("state", "AttachStateKind"),
        ],
    );
    line(
        &mut output,
        "export type TerminalRequest = Attach | Detach | SendInput | Resize;",
    );
    line(&mut output, "");
    line(
        &mut output,
        "export type TerminalEvent = Snapshot | TerminalOutput | ProcessExit | AttachState;",
    );
    output
}

fn wire_string<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => text,
        Ok(other) => panic!("expected string wire value, got {other}"),
        Err(error) => panic!("serialize wire value: {error}"),
    }
}

fn line(output: &mut String, text: &str) {
    output.push_str(text);
    output.push('\n');
}

fn emit_interface(output: &mut String, name: &str, fields: &[(&str, &str)]) {
    line(output, &format!("export interface {name} {{"));
    for (field, ty) in fields {
        line(output, &format!("  {field}: {ty};"));
    }
    line(output, "}");
    line(output, "");
}

fn emit_string_union(output: &mut String, name: &str, values: &[&str]) {
    line(output, &format!("export type {name} ="));
    for (index, value) in values.iter().enumerate() {
        let suffix = if index + 1 == values.len() { ";" } else { "" };
        line(output, &format!("  | \"{value}\"{suffix}"));
    }
    line(output, "");
}

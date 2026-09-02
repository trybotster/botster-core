//! Deterministic TypeScript emitter for the terminal protocol plane.

use botster_terminal_protocol::{
    CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    FEATURE_TERMINAL_STREAMING, FEATURE_TRANSPORT_DUPLEX_BINARY, MAX_INPUT_DATA_BYTES,
    MAX_MODE_GATED_DATA_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, MAX_PASTE_CHUNK_DATA_BYTES,
    PROTOCOL, PROTOCOL_VERSION, TERMINAL_INPUT_SCHEME_VERSION,
};
use serde::Serialize;
use serde_json::Value;

use crate::{
    AttachStateKind, PayloadEncoding, SnapshotPhase, TerminalInputKind, TerminalInputRejection,
};

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
    line(
        &mut output,
        &format!(
            "export const FEATURE_TRANSPORT_DUPLEX_BINARY = \"{FEATURE_TRANSPORT_DUPLEX_BINARY}\";"
        ),
    );
    line(
        &mut output,
        &format!("export const TERMINAL_INPUT_SCHEME_VERSION = {TERMINAL_INPUT_SCHEME_VERSION};"),
    );
    line(
        &mut output,
        &format!("export const MAX_INPUT_DATA_BYTES = {MAX_INPUT_DATA_BYTES};"),
    );
    line(
        &mut output,
        &format!("export const MAX_MODE_GATED_DATA_BYTES = {MAX_MODE_GATED_DATA_BYTES};"),
    );
    line(
        &mut output,
        &format!("export const MAX_PASTE_CHUNK_DATA_BYTES = {MAX_PASTE_CHUNK_DATA_BYTES};"),
    );
    line(
        &mut output,
        &format!("export const MAX_PASTE_BYTES = {MAX_PASTE_BYTES};"),
    );
    line(
        &mut output,
        &format!("export const MAX_PASTE_CHUNKS = {MAX_PASTE_CHUNKS};"),
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
    let phases: Vec<String> = SnapshotPhase::ALL.iter().map(wire_string).collect();
    emit_string_union(
        &mut output,
        "SnapshotPhase",
        &phases.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let attach_states: Vec<String> = AttachStateKind::ALL.iter().map(wire_string).collect();
    emit_string_union(
        &mut output,
        "AttachStateKind",
        &attach_states.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let encodings: Vec<String> = PayloadEncoding::ALL.iter().map(wire_string).collect();
    emit_string_union(
        &mut output,
        "PayloadEncoding",
        &encodings.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let input_kinds: Vec<String> = TerminalInputKind::ALL.iter().map(wire_string).collect();
    emit_string_union(
        &mut output,
        "TerminalInputKind",
        &input_kinds.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let rejections: Vec<String> = TerminalInputRejection::ALL
        .iter()
        .map(wire_string)
        .collect();
    emit_string_union(
        &mut output,
        "TerminalInputRejection",
        &rejections.iter().map(String::as_str).collect::<Vec<_>>(),
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
    emit_interface(
        &mut output,
        "TerminalModeFlags",
        &[
            ("kitty_enabled", "boolean"),
            ("cursor_visible", "boolean"),
            ("bracketed_paste", "boolean"),
            ("mouse_mode", "number"),
            ("alt_screen", "boolean"),
            ("focus_reporting", "boolean"),
            ("application_cursor", "boolean"),
        ],
    );
    emit_interface(
        &mut output,
        "TerminalInputResult",
        &[
            ("type", "\"input_result\""),
            ("subscription_id", "string"),
            ("kind", "TerminalInputKind"),
            ("operation_id?", "number"),
            ("admitted", "boolean"),
            ("bytes_written", "number"),
            ("mode_generation", "number"),
            ("mode_revision", "number"),
            ("mode_flags", "TerminalModeFlags"),
            ("rejection?", "TerminalInputRejection"),
        ],
    );
    line(
        &mut output,
        "export type TerminalRequest = Attach | Detach | SendInput | Resize;",
    );
    line(&mut output, "");
    line(
        &mut output,
        "export type TerminalEvent = Snapshot | TerminalOutput | ProcessExit | AttachState | TerminalInputResult;",
    );
    line(&mut output, "");
    emit_encode_helpers(&mut output);
    output
}

fn emit_encode_helpers(output: &mut String) {
    line(
        output,
        "export function encodeTerminalInput(data: Uint8Array): Uint8Array {",
    );
    line(output, "  if (data.length > MAX_INPUT_DATA_BYTES) {");
    line(
        output,
        "    throw new Error(`PayloadTooLarge kind=input max=${MAX_INPUT_DATA_BYTES} actual=${data.length}`);",
    );
    line(output, "  }");
    line(output, "  return encodeTerminalInputFrame(1, data);");
    line(output, "}");
    line(output, "");
    line(
        output,
        "export function encodeModeGatedInput(mode_generation: bigint | number, mode_revision: bigint | number, data: Uint8Array): Uint8Array {",
    );
    line(output, "  if (data.length > MAX_MODE_GATED_DATA_BYTES) {");
    line(
        output,
        "    throw new Error(`PayloadTooLarge kind=mode_gated_input max=${MAX_MODE_GATED_DATA_BYTES} actual=${data.length}`);",
    );
    line(output, "  }");
    line(output, "  const body = new Uint8Array(16 + data.length);");
    line(output, "  const view = new DataView(body.buffer);");
    line(
        output,
        "  view.setBigUint64(0, BigInt(mode_generation), false);",
    );
    line(
        output,
        "  view.setBigUint64(8, BigInt(mode_revision), false);",
    );
    line(output, "  body.set(data, 16);");
    line(output, "  return encodeTerminalInputFrame(2, body);");
    line(output, "}");
    line(output, "");
    line(
        output,
        "export function encodeResize(rows: number, cols: number): Uint8Array {",
    );
    line(output, "  const body = new Uint8Array(4);");
    line(output, "  const view = new DataView(body.buffer);");
    line(output, "  view.setUint16(0, rows, false);");
    line(output, "  view.setUint16(2, cols, false);");
    line(output, "  return encodeTerminalInputFrame(3, body);");
    line(output, "}");
    line(output, "");
    line(
        output,
        "export function encodePaste(operation_id: number, mode_generation: bigint | number, mode_revision: bigint | number, data: Uint8Array): Uint8Array[] {",
    );
    line(output, "  if (data.length === 0) {");
    line(output, "    throw new Error(\"EmptyPaste\");");
    line(output, "  }");
    line(output, "  if (data.length > MAX_PASTE_BYTES) {");
    line(
        output,
        "    throw new Error(`PayloadTooLarge kind=paste max=${MAX_PASTE_BYTES} actual=${data.length}`);",
    );
    line(output, "  }");
    line(output, "  const begin = new Uint8Array(24);");
    line(output, "  const beginView = new DataView(begin.buffer);");
    line(output, "  beginView.setUint32(0, operation_id, false);");
    line(
        output,
        "  beginView.setBigUint64(4, BigInt(mode_generation), false);",
    );
    line(
        output,
        "  beginView.setBigUint64(12, BigInt(mode_revision), false);",
    );
    line(output, "  beginView.setUint32(20, data.length, false);");
    line(
        output,
        "  const frames = [encodeTerminalInputFrame(4, begin)];",
    );
    line(
        output,
        "  for (let offset = 0, index = 0; offset < data.length; offset += MAX_PASTE_CHUNK_DATA_BYTES, index += 1) {",
    );
    line(
        output,
        "    const chunkData = data.subarray(offset, Math.min(offset + MAX_PASTE_CHUNK_DATA_BYTES, data.length));",
    );
    line(
        output,
        "    const chunk = new Uint8Array(8 + chunkData.length);",
    );
    line(output, "    const chunkView = new DataView(chunk.buffer);");
    line(output, "    chunkView.setUint32(0, operation_id, false);");
    line(output, "    chunkView.setUint32(4, index, false);");
    line(output, "    chunk.set(chunkData, 8);");
    line(
        output,
        "    frames.push(encodeTerminalInputFrame(5, chunk));",
    );
    line(output, "  }");
    line(output, "  const commit = new Uint8Array(4);");
    line(
        output,
        "  new DataView(commit.buffer).setUint32(0, operation_id, false);",
    );
    line(
        output,
        "  frames.push(encodeTerminalInputFrame(6, commit));",
    );
    line(output, "  return frames;");
    line(output, "}");
    line(output, "");
    line(
        output,
        "export function encodePasteAbort(operation_id: number): Uint8Array {",
    );
    line(output, "  const body = new Uint8Array(4);");
    line(
        output,
        "  new DataView(body.buffer).setUint32(0, operation_id, false);",
    );
    line(output, "  return encodeTerminalInputFrame(7, body);");
    line(output, "}");
    line(output, "");
    line(
        output,
        "function encodeTerminalInputFrame(kind: number, body: Uint8Array): Uint8Array {",
    );
    line(output, "  const out = new Uint8Array(4 + body.length);");
    line(output, "  out[0] = TERMINAL_INPUT_SCHEME_VERSION;");
    line(output, "  out[1] = kind;");
    line(output, "  const view = new DataView(out.buffer);");
    line(output, "  view.setUint16(2, body.length, false);");
    line(output, "  out.set(body, 4);");
    line(output, "  return out;");
    line(output, "}");
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

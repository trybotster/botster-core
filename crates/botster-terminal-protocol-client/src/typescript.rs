//! Deterministic TypeScript emitter for the scheme 2 terminal protocol.
//!
//! Every constant, enum table, key table, and byte layout comes from the Rust
//! crates. The committed artifact is
//! `crates/botster-terminal-protocol-client/generated/terminal-protocol.ts`
//! mirrored at `packages/terminal-protocol/terminal-protocol.ts`.

use botster_terminal_protocol::{
    mode_bits, terminal_mods, AttachStateCode, HistoryUnavailableReason, InputOutcome,
    TerminalInputKind, TerminalKey, TerminalKeyAction, TerminalKind, TerminalMouseAction,
    TerminalMouseButton, CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, INPUT_HEADER_BYTES, MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION,
    MAX_ENCODED_INPUT_BYTES, MAX_INPUT_OPERATIONS_PER_CLIENT, MAX_INPUT_OPERATIONS_PER_SESSION,
    MAX_INPUT_RESULT_DETAIL_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, MAX_PASTE_CHUNK_DATA_BYTES,
    MAX_RAW_INPUT_BYTES, MAX_RETAINED_INPUT_BYTES_PER_CLIENT, MAX_RETAINED_INPUT_BYTES_PER_SESSION,
    MAX_ROUTE_EGRESS_BYTES, MAX_ROUTE_EGRESS_FRAMES, MAX_ROUTE_ID_BYTES, MAX_TERMINAL_BODY_BYTES,
    MAX_TERMINAL_INPUT_BODY_BYTES, PROTOCOL, PROTOCOL_VERSION, TERMINAL_BODY_HEADER_BYTES,
    TERMINAL_INPUT_SCHEME_VERSION, TERMINAL_STREAM_SCHEME_VERSION,
};

/// Generate the committed TypeScript artifact from the Rust protocol source.
#[must_use]
pub fn terminal_protocol_typescript() -> String {
    let mut out = String::new();
    line(
        &mut out,
        "// Generated from crates/botster-terminal-protocol-client Rust scheme 2 codecs.",
    );
    line(
        &mut out,
        "// Regenerate/check with: cargo test -p botster-terminal-protocol-client typescript",
    );
    line(&mut out, "");
    emit_constants(&mut out);
    emit_compatibility_interfaces(&mut out);
    emit_enum_tables(&mut out);
    emit_key_table(&mut out);
    out.push_str(STREAM_DECODER);
    out.push_str(INPUT_ENCODER);
    out
}

fn emit_constants(out: &mut String) {
    line(out, &format!("export const PROTOCOL = \"{PROTOCOL}\";"));
    line(
        out,
        &format!("export const PROTOCOL_VERSION = {PROTOCOL_VERSION};"),
    );
    line(
        out,
        &format!("export const CONFORMANCE_FIXTURE_REVISION = {CONFORMANCE_FIXTURE_REVISION};"),
    );
    line(
        out,
        &format!(
            "export const PACKAGE_VERSION = \"{}\";",
            env!("CARGO_PKG_VERSION")
        ),
    );
    line(
        out,
        &format!("export const FEATURE_TERMINAL_STREAMING = \"{FEATURE_TERMINAL_STREAMING}\";"),
    );
    line(
        out,
        &format!("export const FEATURE_RESIZE = \"{FEATURE_RESIZE}\";"),
    );
    line(
        out,
        &format!(
            "export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = \"{FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY}\";"
        ),
    );
    line(
        out,
        &format!(
            "export const FEATURE_TRANSPORT_DUPLEX_BINARY = \"{FEATURE_TRANSPORT_DUPLEX_BINARY}\";"
        ),
    );
    let numbers: &[(&str, usize)] = &[
        (
            "TERMINAL_STREAM_SCHEME_VERSION",
            TERMINAL_STREAM_SCHEME_VERSION as usize,
        ),
        ("TERMINAL_BODY_HEADER_BYTES", TERMINAL_BODY_HEADER_BYTES),
        ("MAX_ROUTE_EGRESS_FRAMES", MAX_ROUTE_EGRESS_FRAMES),
        ("MAX_ROUTE_EGRESS_BYTES", MAX_ROUTE_EGRESS_BYTES),
        ("MAX_TERMINAL_BODY_BYTES", MAX_TERMINAL_BODY_BYTES),
        (
            "MAX_INPUT_RESULT_DETAIL_BYTES",
            MAX_INPUT_RESULT_DETAIL_BYTES,
        ),
        ("MAX_ROUTE_ID_BYTES", MAX_ROUTE_ID_BYTES),
        (
            "TERMINAL_INPUT_SCHEME_VERSION",
            TERMINAL_INPUT_SCHEME_VERSION as usize,
        ),
        ("INPUT_HEADER_BYTES", INPUT_HEADER_BYTES),
        (
            "MAX_TERMINAL_INPUT_BODY_BYTES",
            MAX_TERMINAL_INPUT_BODY_BYTES as usize,
        ),
        ("MAX_RAW_INPUT_BYTES", MAX_RAW_INPUT_BYTES),
        ("MAX_PASTE_CHUNK_DATA_BYTES", MAX_PASTE_CHUNK_DATA_BYTES),
        ("MAX_PASTE_BYTES", MAX_PASTE_BYTES),
        ("MAX_PASTE_CHUNKS", MAX_PASTE_CHUNKS),
        (
            "MAX_INPUT_OPERATIONS_PER_SESSION",
            MAX_INPUT_OPERATIONS_PER_SESSION,
        ),
        (
            "MAX_RETAINED_INPUT_BYTES_PER_SESSION",
            MAX_RETAINED_INPUT_BYTES_PER_SESSION,
        ),
        (
            "MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION",
            MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION,
        ),
        (
            "MAX_INPUT_OPERATIONS_PER_CLIENT",
            MAX_INPUT_OPERATIONS_PER_CLIENT,
        ),
        (
            "MAX_RETAINED_INPUT_BYTES_PER_CLIENT",
            MAX_RETAINED_INPUT_BYTES_PER_CLIENT,
        ),
        ("MAX_ENCODED_INPUT_BYTES", MAX_ENCODED_INPUT_BYTES),
    ];
    for (name, value) in numbers {
        line(out, &format!("export const {name} = {value};"));
    }
    line(out, "");
}

fn emit_compatibility_interfaces(out: &mut String) {
    emit_interface(
        out,
        "TerminalCompatibility",
        &[
            ("protocol", "string"),
            ("protocol_version", "number"),
            ("features", "string[]"),
            ("conformance_fixture_revision", "number"),
        ],
    );
    emit_interface(
        out,
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
        out,
        "Attach",
        &[
            ("type", "\"attach\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
        ],
    );
    emit_interface(
        out,
        "Detach",
        &[
            ("type", "\"detach\""),
            ("session_id", "string"),
            ("subscription_id", "string"),
        ],
    );
    emit_interface(
        out,
        "SendInput",
        &[
            ("type", "\"send_input\""),
            ("session_id", "string"),
            ("data", "string"),
        ],
    );
    emit_interface(
        out,
        "Resize",
        &[
            ("type", "\"resize\""),
            ("session_id", "string"),
            ("rows", "number"),
            ("cols", "number"),
        ],
    );
    line(
        out,
        "export type TerminalRequest = Attach | Detach | SendInput | Resize;",
    );
    line(out, "");
}

fn emit_enum_tables(out: &mut String) {
    emit_value_table(
        out,
        "TerminalKind",
        TerminalKind::ALL
            .iter()
            .map(|kind| (kind.name(), u32::from(kind.as_byte()))),
    );
    emit_value_table(
        out,
        "AttachStateCode",
        AttachStateCode::ALL
            .iter()
            .map(|state| (state.name(), u32::from(state.as_byte()))),
    );
    emit_value_table(
        out,
        "HistoryUnavailableReason",
        HistoryUnavailableReason::ALL
            .iter()
            .map(|reason| (reason.name(), u32::from(reason.as_byte()))),
    );
    emit_value_table(
        out,
        "InputOutcome",
        InputOutcome::ALL
            .iter()
            .map(|outcome| (outcome.name(), u32::from(outcome.as_byte()))),
    );
    emit_value_table(
        out,
        "ModeBits",
        mode_bits::ALL.iter().map(|(name, bit)| (*name, *bit)),
    );
    emit_value_table(
        out,
        "TerminalInputKind",
        TerminalInputKind::ALL
            .iter()
            .map(|kind| (kind.name(), u32::from(kind.as_byte()))),
    );
    emit_value_table(
        out,
        "TerminalKeyAction",
        TerminalKeyAction::ALL
            .iter()
            .map(|action| (action.name(), u32::from(action.as_byte()))),
    );
    emit_value_table(
        out,
        "TerminalMouseAction",
        TerminalMouseAction::ALL
            .iter()
            .map(|action| (action.name(), u32::from(action.as_byte()))),
    );
    emit_value_table(
        out,
        "TerminalMouseButton",
        TerminalMouseButton::ALL
            .iter()
            .map(|button| (button.name(), u32::from(button.as_byte()))),
    );
    emit_value_table(
        out,
        "TerminalMods",
        terminal_mods::ALL
            .iter()
            .map(|(name, bit)| (*name, u32::from(*bit))),
    );
}

fn emit_key_table(out: &mut String) {
    line(
        out,
        "/** Physical keys keyed by W3C UI Events `KeyboardEvent.code`. */",
    );
    line(out, "export const TerminalKey = {");
    for key in TerminalKey::ALL {
        line(out, &format!("  {}: {},", key.code(), key.as_u16()));
    }
    line(out, "} as const;");
    line(
        out,
        "export type TerminalKeyCode = keyof typeof TerminalKey;",
    );
    line(out, "");
}

fn emit_value_table<'a>(
    out: &mut String,
    name: &str,
    values: impl Iterator<Item = (&'a str, u32)>,
) {
    line(out, &format!("export const {name} = {{"));
    for (value_name, value) in values {
        line(out, &format!("  {value_name}: {value},"));
    }
    line(out, "} as const;");
    line(
        out,
        &format!("export type {name}Name = keyof typeof {name};"),
    );
    line(out, "");
}

fn line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

fn emit_interface(out: &mut String, name: &str, fields: &[(&str, &str)]) {
    line(out, &format!("export interface {name} {{"));
    for (field, ty) in fields {
        line(out, &format!("  {field}: {ty};"));
    }
    line(out, "}");
    line(out, "");
}

const STREAM_DECODER: &str = r#"function nameOf<T extends Record<string, number>>(table: T, value: number): keyof T {
  for (const name of Object.keys(table) as (keyof T)[]) {
    if (table[name] === value) {
      return name;
    }
  }
  throw new Error(`UnknownValue value=${value}`);
}

export interface TerminalModeFlags {
  kitty_enabled: boolean;
  cursor_visible: boolean;
  bracketed_paste: boolean;
  mouse_normal: boolean;
  mouse_any: boolean;
  mouse_button: boolean;
  mouse_sgr: boolean;
  alt_screen: boolean;
  focus_reporting: boolean;
  application_cursor: boolean;
}

export function decodeModeFlags(mode_bits: number): TerminalModeFlags {
  return {
    kitty_enabled: (mode_bits & ModeBits.KITTY_KEYBOARD) !== 0,
    cursor_visible: (mode_bits & ModeBits.CURSOR_VISIBLE) !== 0,
    bracketed_paste: (mode_bits & ModeBits.BRACKETED_PASTE) !== 0,
    mouse_normal: (mode_bits & ModeBits.MOUSE_NORMAL) !== 0,
    mouse_any: (mode_bits & ModeBits.MOUSE_ANY) !== 0,
    mouse_button: (mode_bits & ModeBits.MOUSE_BUTTON) !== 0,
    mouse_sgr: (mode_bits & ModeBits.MOUSE_SGR) !== 0,
    alt_screen: (mode_bits & ModeBits.ALT_SCREEN) !== 0,
    focus_reporting: (mode_bits & ModeBits.FOCUS_REPORTING) !== 0,
    application_cursor: (mode_bits & ModeBits.APPLICATION_CURSOR) !== 0,
  };
}

export interface InputResultBody {
  operation_id: bigint;
  outcome: InputOutcomeName;
  accepted_payload_bytes: bigint | null;
  written_pty_bytes: bigint | null;
  mode_bits: number;
  detail: string;
}

export type TerminalEvent =
  | { kind: "output"; payload: Uint8Array }
  | { kind: "snapshot_ready"; payload: Uint8Array }
  | { kind: "snapshot_history"; payload: Uint8Array }
  | { kind: "snapshot_finish" }
  | { kind: "process_exit"; code: number | null }
  | { kind: "modes"; mode_bits: number; rows: number; cols: number }
  | { kind: "attach_state"; state: AttachStateCodeName }
  | { kind: "input_result"; result: InputResultBody }
  | { kind: "history_unavailable"; reason: HistoryUnavailableReasonName }
  | { kind: "route_resync" };

const utf8Decoder = new TextDecoder("utf-8", { fatal: true });

/** Decode one complete TerminalBody. `payload` views share `bytes`; no copy. */
export function decodeTerminalBody(bytes: Uint8Array): TerminalEvent {
  if (bytes.length < TERMINAL_BODY_HEADER_BYTES) {
    throw new Error("TruncatedHeader");
  }
  if (bytes[0] !== TERMINAL_STREAM_SCHEME_VERSION) {
    throw new Error(`WrongSchemeVersion found=${bytes[0]}`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const declared = view.getUint32(4, true);
  const body = bytes.subarray(TERMINAL_BODY_HEADER_BYTES);
  if (declared !== body.length) {
    throw new Error(`BodyLengthMismatch declared=${declared} remaining=${body.length}`);
  }
  const bodyView = new DataView(body.buffer, body.byteOffset, body.byteLength);
  switch (bytes[1]) {
    case TerminalKind.output:
      return { kind: "output", payload: body };
    case TerminalKind.snapshot_ready:
      return { kind: "snapshot_ready", payload: body };
    case TerminalKind.snapshot_history:
      return { kind: "snapshot_history", payload: body };
    case TerminalKind.snapshot_finish:
      expectLength(body, 0);
      return { kind: "snapshot_finish" };
    case TerminalKind.process_exit:
      expectLength(body, 5);
      return { kind: "process_exit", code: body[0] === 1 ? bodyView.getInt32(1, true) : null };
    case TerminalKind.modes:
      expectLength(body, 8);
      return {
        kind: "modes",
        mode_bits: bodyView.getUint32(0, true),
        rows: bodyView.getUint16(4, true),
        cols: bodyView.getUint16(6, true),
      };
    case TerminalKind.attach_state:
      expectLength(body, 1);
      return { kind: "attach_state", state: nameOf(AttachStateCode, body[0]) };
    case TerminalKind.input_result:
      return { kind: "input_result", result: decodeInputResult(body, bodyView) };
    case TerminalKind.history_unavailable:
      expectLength(body, 1);
      return { kind: "history_unavailable", reason: nameOf(HistoryUnavailableReason, body[0]) };
    case TerminalKind.route_resync:
      expectLength(body, 0);
      return { kind: "route_resync" };
    default:
      throw new Error(`UnknownKind found=${bytes[1]}`);
  }
}

function expectLength(body: Uint8Array, expected: number): void {
  if (body.length !== expected) {
    throw new Error(`BodyLength expected=${expected} actual=${body.length}`);
  }
}

function decodeInputResult(body: Uint8Array, view: DataView): InputResultBody {
  if (body.length < 33) {
    throw new Error(`BodyLength expected=33 actual=${body.length}`);
  }
  const detail_len = view.getUint16(31, true);
  const detail_bytes = body.subarray(33);
  if (detail_len !== detail_bytes.length || detail_len > MAX_INPUT_RESULT_DETAIL_BYTES) {
    throw new Error("InvalidDetail");
  }
  return {
    operation_id: view.getBigUint64(0, true),
    outcome: nameOf(InputOutcome, body[8]),
    accepted_payload_bytes: body[9] === 1 ? view.getBigUint64(10, true) : null,
    written_pty_bytes: body[18] === 1 ? view.getBigUint64(19, true) : null,
    mode_bits: view.getUint32(27, true),
    detail: utf8Decoder.decode(detail_bytes),
  };
}

"#;

const INPUT_ENCODER: &str = r#"export type OperationId = bigint | number;

export interface KeyInput {
  action: TerminalKeyActionName;
  key: number;
  mods: number;
  consumed_mods: number;
  composing: boolean;
  unshifted_codepoint: number;
  text: string;
}

export interface MouseInput {
  action: TerminalMouseActionName;
  button: TerminalMouseButtonName | null;
  mods: number;
  col: number;
  row: number;
  x_px: number;
  y_px: number;
}

/** Look up a physical key by W3C `KeyboardEvent.code`; unknown codes are `Unidentified`. */
export function terminalKeyFromCode(code: string): number {
  const value = (TerminalKey as Record<string, number>)[code];
  return value === undefined ? TerminalKey.Unidentified : value;
}

const utf8Encoder = new TextEncoder();

function encodeInputFrame(kind: number, operation_id: OperationId, body: Uint8Array): Uint8Array {
  if (body.length > MAX_TERMINAL_INPUT_BODY_BYTES) {
    throw new Error(`PayloadTooLarge kind=${kind} max=${MAX_TERMINAL_INPUT_BODY_BYTES} actual=${body.length}`);
  }
  const out = new Uint8Array(INPUT_HEADER_BYTES + body.length);
  const view = new DataView(out.buffer);
  out[0] = TERMINAL_INPUT_SCHEME_VERSION;
  out[1] = kind;
  view.setUint16(2, body.length, false);
  view.setBigUint64(4, BigInt(operation_id), false);
  out.set(body, INPUT_HEADER_BYTES);
  return out;
}

export function encodeRawBytes(operation_id: OperationId, data: Uint8Array): Uint8Array {
  if (data.length > MAX_RAW_INPUT_BYTES) {
    throw new Error(`PayloadTooLarge kind=raw_bytes max=${MAX_RAW_INPUT_BYTES} actual=${data.length}`);
  }
  return encodeInputFrame(TerminalInputKind.raw_bytes, operation_id, data);
}

export function encodeKey(operation_id: OperationId, key: KeyInput): Uint8Array {
  const text = utf8Encoder.encode(key.text);
  const body = new Uint8Array(12 + text.length);
  const view = new DataView(body.buffer);
  body[0] = TerminalKeyAction[key.action];
  view.setUint16(1, key.key, false);
  view.setUint16(3, key.mods, false);
  view.setUint16(5, key.consumed_mods, false);
  body[7] = key.composing ? 1 : 0;
  view.setUint32(8, key.unshifted_codepoint, false);
  body.set(text, 12);
  return encodeInputFrame(TerminalInputKind.key, operation_id, body);
}

export function encodeMouse(operation_id: OperationId, mouse: MouseInput): Uint8Array {
  const body = new Uint8Array(17);
  const view = new DataView(body.buffer);
  body[0] = TerminalMouseAction[mouse.action];
  body[1] = mouse.button === null ? 0 : 1;
  body[2] = mouse.button === null ? 0 : TerminalMouseButton[mouse.button];
  view.setUint16(3, mouse.mods, false);
  view.setUint16(5, mouse.col, false);
  view.setUint16(7, mouse.row, false);
  view.setUint32(9, mouse.x_px, false);
  view.setUint32(13, mouse.y_px, false);
  return encodeInputFrame(TerminalInputKind.mouse, operation_id, body);
}

export function encodeFocus(operation_id: OperationId, focused: boolean): Uint8Array {
  return encodeInputFrame(TerminalInputKind.focus, operation_id, new Uint8Array([focused ? 1 : 0]));
}

export function encodeResize(operation_id: OperationId, rows: number, cols: number, width_px: number, height_px: number): Uint8Array {
  const body = new Uint8Array(12);
  const view = new DataView(body.buffer);
  view.setUint16(0, rows, false);
  view.setUint16(2, cols, false);
  view.setUint32(4, width_px, false);
  view.setUint32(8, height_px, false);
  return encodeInputFrame(TerminalInputKind.resize, operation_id, body);
}

/** One paste as PASTE_BEGIN, ordered PASTE_CHUNK frames, and PASTE_COMMIT, all with `operation_id`. */
export function encodePaste(operation_id: OperationId, allow_unsafe: boolean, data: Uint8Array): Uint8Array[] {
  if (data.length === 0) {
    throw new Error("EmptyPaste");
  }
  if (data.length > MAX_PASTE_BYTES) {
    throw new Error(`PayloadTooLarge kind=paste_begin max=${MAX_PASTE_BYTES} actual=${data.length}`);
  }
  const begin = new Uint8Array(5);
  new DataView(begin.buffer).setUint32(0, data.length, false);
  begin[4] = allow_unsafe ? 1 : 0;
  const frames = [encodeInputFrame(TerminalInputKind.paste_begin, operation_id, begin)];
  for (let offset = 0, index = 0; offset < data.length; offset += MAX_PASTE_CHUNK_DATA_BYTES, index += 1) {
    const chunkData = data.subarray(offset, Math.min(offset + MAX_PASTE_CHUNK_DATA_BYTES, data.length));
    const chunk = new Uint8Array(4 + chunkData.length);
    new DataView(chunk.buffer).setUint32(0, index, false);
    chunk.set(chunkData, 4);
    frames.push(encodeInputFrame(TerminalInputKind.paste_chunk, operation_id, chunk));
  }
  frames.push(encodeInputFrame(TerminalInputKind.paste_commit, operation_id, new Uint8Array(0)));
  return frames;
}

export function encodePasteAbort(operation_id: OperationId): Uint8Array {
  return encodeInputFrame(TerminalInputKind.paste_abort, operation_id, new Uint8Array(0));
}
"#;

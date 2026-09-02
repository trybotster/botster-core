// Generated from crates/botster-terminal-protocol-client Rust serde DTOs.
// Regenerate/check with: cargo test -p botster-terminal-protocol-client typescript

export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 2;
export const PACKAGE_VERSION = "0.3.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";
export const TERMINAL_INPUT_SCHEME_VERSION = 1;
export const MAX_INPUT_DATA_BYTES = 65535;
export const MAX_MODE_GATED_DATA_BYTES = 65519;
export const MAX_PASTE_CHUNK_DATA_BYTES = 65527;
export const MAX_PASTE_BYTES = 1048576;
export const MAX_PASTE_CHUNKS = 17;

export interface TerminalCompatibility {
  protocol: string;
  protocol_version: number;
  features: string[];
  conformance_fixture_revision: number;
}

export interface TerminalCompatibilityRequirement {
  protocol: string;
  protocol_version: number;
  required_features: string[];
  minimum_conformance_fixture_revision: number;
  client_name: string;
}

export interface Attach {
  type: "attach";
  session_id: string;
  subscription_id: string;
}

export interface Detach {
  type: "detach";
  session_id: string;
  subscription_id: string;
}

export interface SendInput {
  type: "send_input";
  session_id: string;
  data: string;
}

export interface Resize {
  type: "resize";
  session_id: string;
  rows: number;
  cols: number;
}

export type SnapshotPhase =
  | "ready"
  | "history"
  | "finish";

export type AttachStateKind =
  | "attaching"
  | "attached"
  | "snapshot_history_incomplete"
  | "attach_failed";

export type PayloadEncoding =
  | "base64";

export type TerminalInputKind =
  | "input"
  | "mode_gated_input"
  | "resize"
  | "paste";

export type TerminalInputRejection =
  | "stale_mode"
  | "partial_write"
  | "timeout"
  | "session_not_writable"
  | "duplicate_operation"
  | "operation_in_flight"
  | "operation_out_of_bounds"
  | "operation_incomplete"
  | "aborted";

export interface Snapshot {
  type: "snapshot";
  session_id: string;
  subscription_id: string;
  payload_base64: string;
  payload_encoding: "base64";
  bytes: number;
  phase: SnapshotPhase;
}

export interface TerminalOutput {
  type: "terminal_output";
  session_id: string;
  subscription_id: string;
  payload_base64: string;
  payload_encoding: "base64";
  bytes: number;
}

export interface ProcessExit {
  type: "process_exit";
  session_id: string;
  subscription_id: string;
  code?: number;
}

export interface AttachState {
  type: "attach_state";
  session_id: string;
  subscription_id: string;
  state: AttachStateKind;
}

export interface TerminalModeFlags {
  kitty_enabled: boolean;
  cursor_visible: boolean;
  bracketed_paste: boolean;
  mouse_mode: number;
  alt_screen: boolean;
  focus_reporting: boolean;
  application_cursor: boolean;
}

export interface TerminalInputResult {
  type: "input_result";
  subscription_id: string;
  kind: TerminalInputKind;
  operation_id?: number;
  admitted: boolean;
  bytes_written: number;
  mode_generation: number;
  mode_revision: number;
  mode_flags: TerminalModeFlags;
  rejection?: TerminalInputRejection;
}

export type TerminalRequest = Attach | Detach | SendInput | Resize;

export type TerminalEvent = Snapshot | TerminalOutput | ProcessExit | AttachState | TerminalInputResult;

export function encodeTerminalInput(data: Uint8Array): Uint8Array {
  if (data.length > MAX_INPUT_DATA_BYTES) {
    throw new Error(`PayloadTooLarge kind=input max=${MAX_INPUT_DATA_BYTES} actual=${data.length}`);
  }
  return encodeTerminalInputFrame(1, data);
}

export function encodeModeGatedInput(mode_generation: bigint | number, mode_revision: bigint | number, data: Uint8Array): Uint8Array {
  if (data.length > MAX_MODE_GATED_DATA_BYTES) {
    throw new Error(`PayloadTooLarge kind=mode_gated_input max=${MAX_MODE_GATED_DATA_BYTES} actual=${data.length}`);
  }
  const body = new Uint8Array(16 + data.length);
  const view = new DataView(body.buffer);
  view.setBigUint64(0, BigInt(mode_generation), false);
  view.setBigUint64(8, BigInt(mode_revision), false);
  body.set(data, 16);
  return encodeTerminalInputFrame(2, body);
}

export function encodeResize(rows: number, cols: number): Uint8Array {
  const body = new Uint8Array(4);
  const view = new DataView(body.buffer);
  view.setUint16(0, rows, false);
  view.setUint16(2, cols, false);
  return encodeTerminalInputFrame(3, body);
}

export function encodePaste(operation_id: number, mode_generation: bigint | number, mode_revision: bigint | number, data: Uint8Array): Uint8Array[] {
  if (data.length === 0) {
    throw new Error("EmptyPaste");
  }
  if (data.length > MAX_PASTE_BYTES) {
    throw new Error(`PayloadTooLarge kind=paste max=${MAX_PASTE_BYTES} actual=${data.length}`);
  }
  const begin = new Uint8Array(24);
  const beginView = new DataView(begin.buffer);
  beginView.setUint32(0, operation_id, false);
  beginView.setBigUint64(4, BigInt(mode_generation), false);
  beginView.setBigUint64(12, BigInt(mode_revision), false);
  beginView.setUint32(20, data.length, false);
  const frames = [encodeTerminalInputFrame(4, begin)];
  for (let offset = 0, index = 0; offset < data.length; offset += MAX_PASTE_CHUNK_DATA_BYTES, index += 1) {
    const chunkData = data.subarray(offset, Math.min(offset + MAX_PASTE_CHUNK_DATA_BYTES, data.length));
    const chunk = new Uint8Array(8 + chunkData.length);
    const chunkView = new DataView(chunk.buffer);
    chunkView.setUint32(0, operation_id, false);
    chunkView.setUint32(4, index, false);
    chunk.set(chunkData, 8);
    frames.push(encodeTerminalInputFrame(5, chunk));
  }
  const commit = new Uint8Array(4);
  new DataView(commit.buffer).setUint32(0, operation_id, false);
  frames.push(encodeTerminalInputFrame(6, commit));
  return frames;
}

export function encodePasteAbort(operation_id: number): Uint8Array {
  const body = new Uint8Array(4);
  new DataView(body.buffer).setUint32(0, operation_id, false);
  return encodeTerminalInputFrame(7, body);
}

function encodeTerminalInputFrame(kind: number, body: Uint8Array): Uint8Array {
  const out = new Uint8Array(4 + body.length);
  out[0] = TERMINAL_INPUT_SCHEME_VERSION;
  out[1] = kind;
  const view = new DataView(out.buffer);
  view.setUint16(2, body.length, false);
  out.set(body, 4);
  return out;
}

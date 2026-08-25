// Generated from crates/botster-terminal-protocol-client Rust serde DTOs.
// Regenerate/check with: cargo test -p botster-terminal-protocol-client typescript

export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 2;
export const PACKAGE_VERSION = "0.2.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";
export const TERMINAL_INPUT_SCHEME_VERSION = 1;
export const MAX_INPUT_DATA_BYTES = 65535;
export const MAX_MODE_GATED_DATA_BYTES = 65519;

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
  | "resize";

export type TerminalInputRejection =
  | "stale_mode"
  | "partial_write"
  | "timeout"
  | "session_not_writable";

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

function encodeTerminalInputFrame(kind: number, body: Uint8Array): Uint8Array {
  const out = new Uint8Array(4 + body.length);
  out[0] = TERMINAL_INPUT_SCHEME_VERSION;
  out[1] = kind;
  const view = new DataView(out.buffer);
  view.setUint16(2, body.length, false);
  out.set(body, 4);
  return out;
}

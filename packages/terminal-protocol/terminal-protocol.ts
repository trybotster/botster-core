// Generated from crates/botster-terminal-protocol-client Rust scheme 2 codecs.
// Regenerate with: cargo run -p botster-terminal-protocol-client --example generate_typescript

export const PROTOCOL = "botster-terminal-v2";
export const PROTOCOL_VERSION = 2;
export const CONFORMANCE_FIXTURE_REVISION = 3;
export const PACKAGE_VERSION = "0.4.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";
export const TERMINAL_STREAM_SCHEME_VERSION = 2;
export const TERMINAL_BODY_HEADER_BYTES = 8;
export const MAX_ROUTE_EGRESS_FRAMES = 64;
export const MAX_ROUTE_EGRESS_BYTES = 4194304;
export const MAX_TERMINAL_BODY_BYTES = 4194296;
export const MAX_INPUT_RESULT_DETAIL_BYTES = 1024;
export const MAX_ROUTE_ID_BYTES = 1024;
export const TERMINAL_INPUT_SCHEME_VERSION = 2;
export const INPUT_HEADER_BYTES = 12;
export const MAX_TERMINAL_INPUT_BODY_BYTES = 65535;
export const MAX_RAW_INPUT_BYTES = 65535;
export const MAX_PASTE_CHUNK_DATA_BYTES = 65531;
export const MAX_PASTE_BYTES = 1048576;
export const MAX_PASTE_CHUNKS = 17;
export const MAX_INPUT_OPERATIONS_PER_SESSION = 32;
export const MAX_RETAINED_INPUT_BYTES_PER_SESSION = 2097152;
export const MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION = 1;
export const MAX_INPUT_OPERATIONS_PER_CLIENT = 128;
export const MAX_RETAINED_INPUT_BYTES_PER_CLIENT = 8388608;
export const MAX_ENCODED_INPUT_BYTES = 1048640;

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

export type TerminalRequest = Attach | Detach | SendInput | Resize;

export const TerminalKind = {
  output: 1,
  snapshot_ready: 2,
  snapshot_history: 3,
  snapshot_finish: 4,
  process_exit: 5,
  modes: 6,
  attach_state: 16,
  input_result: 17,
  history_unavailable: 18,
  route_resync: 19,
} as const;
export type TerminalKindName = keyof typeof TerminalKind;

export const AttachStateCode = {
  attaching: 1,
  attached: 2,
  detached: 3,
  failed: 4,
} as const;
export type AttachStateCodeName = keyof typeof AttachStateCode;

export const HistoryUnavailableReason = {
  evicted: 1,
  restart: 2,
  oversize: 3,
  capture_failed: 4,
} as const;
export type HistoryUnavailableReasonName = keyof typeof HistoryUnavailableReason;

export const InputOutcome = {
  written: 1,
  partial_write: 2,
  write_failed: 3,
  cancelled: 4,
  rejected_not_writable: 5,
  rejected_too_large: 6,
  rejected_unsafe_paste: 7,
  rejected_lane_full: 8,
  rejected_protocol: 9,
  session_ended: 10,
  outcome_unknown: 11,
} as const;
export type InputOutcomeName = keyof typeof InputOutcome;

export const ModeBits = {
  KITTY_KEYBOARD: 1,
  CURSOR_VISIBLE: 2,
  BRACKETED_PASTE: 4,
  MOUSE_NORMAL: 8,
  MOUSE_ANY: 16,
  MOUSE_BUTTON: 32,
  MOUSE_SGR: 64,
  ALT_SCREEN: 128,
  FOCUS_REPORTING: 256,
  APPLICATION_CURSOR: 512,
} as const;
export type ModeBitsName = keyof typeof ModeBits;

export const TerminalInputKind = {
  raw_bytes: 1,
  key: 2,
  mouse: 3,
  focus: 4,
  resize: 5,
  paste_begin: 6,
  paste_chunk: 7,
  paste_commit: 8,
  paste_abort: 9,
} as const;
export type TerminalInputKindName = keyof typeof TerminalInputKind;

export const TerminalKeyAction = {
  press: 1,
  release: 2,
  repeat: 3,
} as const;
export type TerminalKeyActionName = keyof typeof TerminalKeyAction;

export const TerminalMouseAction = {
  press: 1,
  release: 2,
  motion: 3,
} as const;
export type TerminalMouseActionName = keyof typeof TerminalMouseAction;

export const TerminalMouseButton = {
  left: 1,
  right: 2,
  middle: 3,
  wheel_up: 4,
  wheel_down: 5,
  wheel_left: 6,
  wheel_right: 7,
  button8: 8,
  button9: 9,
  button10: 10,
  button11: 11,
} as const;
export type TerminalMouseButtonName = keyof typeof TerminalMouseButton;

export const TerminalMods = {
  SHIFT: 1,
  CTRL: 2,
  ALT: 4,
  SUPER: 8,
  CAPS_LOCK: 16,
  NUM_LOCK: 32,
  SHIFT_SIDE: 64,
  CTRL_SIDE: 128,
  ALT_SIDE: 256,
  SUPER_SIDE: 512,
} as const;
export type TerminalModsName = keyof typeof TerminalMods;

/** Physical keys keyed by W3C UI Events `KeyboardEvent.code`. */
export const TerminalKey = {
  Unidentified: 0,
  Backquote: 1,
  Backslash: 2,
  BracketLeft: 3,
  BracketRight: 4,
  Comma: 5,
  Digit0: 6,
  Digit1: 7,
  Digit2: 8,
  Digit3: 9,
  Digit4: 10,
  Digit5: 11,
  Digit6: 12,
  Digit7: 13,
  Digit8: 14,
  Digit9: 15,
  Equal: 16,
  IntlBackslash: 17,
  IntlRo: 18,
  IntlYen: 19,
  KeyA: 20,
  KeyB: 21,
  KeyC: 22,
  KeyD: 23,
  KeyE: 24,
  KeyF: 25,
  KeyG: 26,
  KeyH: 27,
  KeyI: 28,
  KeyJ: 29,
  KeyK: 30,
  KeyL: 31,
  KeyM: 32,
  KeyN: 33,
  KeyO: 34,
  KeyP: 35,
  KeyQ: 36,
  KeyR: 37,
  KeyS: 38,
  KeyT: 39,
  KeyU: 40,
  KeyV: 41,
  KeyW: 42,
  KeyX: 43,
  KeyY: 44,
  KeyZ: 45,
  Minus: 46,
  Period: 47,
  Quote: 48,
  Semicolon: 49,
  Slash: 50,
  AltLeft: 51,
  AltRight: 52,
  Backspace: 53,
  CapsLock: 54,
  ContextMenu: 55,
  ControlLeft: 56,
  ControlRight: 57,
  Enter: 58,
  MetaLeft: 59,
  MetaRight: 60,
  ShiftLeft: 61,
  ShiftRight: 62,
  Space: 63,
  Tab: 64,
  Convert: 65,
  KanaMode: 66,
  NonConvert: 67,
  Delete: 68,
  End: 69,
  Help: 70,
  Home: 71,
  Insert: 72,
  PageDown: 73,
  PageUp: 74,
  ArrowDown: 75,
  ArrowLeft: 76,
  ArrowRight: 77,
  ArrowUp: 78,
  NumLock: 79,
  Numpad0: 80,
  Numpad1: 81,
  Numpad2: 82,
  Numpad3: 83,
  Numpad4: 84,
  Numpad5: 85,
  Numpad6: 86,
  Numpad7: 87,
  Numpad8: 88,
  Numpad9: 89,
  NumpadAdd: 90,
  NumpadBackspace: 91,
  NumpadClear: 92,
  NumpadClearEntry: 93,
  NumpadComma: 94,
  NumpadDecimal: 95,
  NumpadDivide: 96,
  NumpadEnter: 97,
  NumpadEqual: 98,
  NumpadMemoryAdd: 99,
  NumpadMemoryClear: 100,
  NumpadMemoryRecall: 101,
  NumpadMemoryStore: 102,
  NumpadMemorySubtract: 103,
  NumpadMultiply: 104,
  NumpadParenLeft: 105,
  NumpadParenRight: 106,
  NumpadSubtract: 107,
  NumpadSeparator: 108,
  NumpadUp: 109,
  NumpadDown: 110,
  NumpadRight: 111,
  NumpadLeft: 112,
  NumpadBegin: 113,
  NumpadHome: 114,
  NumpadEnd: 115,
  NumpadInsert: 116,
  NumpadDelete: 117,
  NumpadPageUp: 118,
  NumpadPageDown: 119,
  Escape: 120,
  F1: 121,
  F2: 122,
  F3: 123,
  F4: 124,
  F5: 125,
  F6: 126,
  F7: 127,
  F8: 128,
  F9: 129,
  F10: 130,
  F11: 131,
  F12: 132,
  F13: 133,
  F14: 134,
  F15: 135,
  F16: 136,
  F17: 137,
  F18: 138,
  F19: 139,
  F20: 140,
  F21: 141,
  F22: 142,
  F23: 143,
  F24: 144,
  F25: 145,
  Fn: 146,
  FnLock: 147,
  PrintScreen: 148,
  ScrollLock: 149,
  Pause: 150,
  BrowserBack: 151,
  BrowserFavorites: 152,
  BrowserForward: 153,
  BrowserHome: 154,
  BrowserRefresh: 155,
  BrowserSearch: 156,
  BrowserStop: 157,
  Eject: 158,
  LaunchApp1: 159,
  LaunchApp2: 160,
  LaunchMail: 161,
  MediaPlayPause: 162,
  MediaSelect: 163,
  MediaStop: 164,
  MediaTrackNext: 165,
  MediaTrackPrevious: 166,
  Power: 167,
  Sleep: 168,
  AudioVolumeDown: 169,
  AudioVolumeMute: 170,
  AudioVolumeUp: 171,
  WakeUp: 172,
  Copy: 173,
  Cut: 174,
  Paste: 175,
} as const;
export type TerminalKeyCode = keyof typeof TerminalKey;

function nameOf<T extends Record<string, number>>(table: T, value: number): keyof T {
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

export type OperationId = bigint | number;

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

const MAX_OPERATION_ID = (1n << 64n) - 1n;

/** Validate a client operation id. Numbers must be safe integers; bigints must fit u64. */
export function toOperationId(operation_id: OperationId): bigint {
  if (typeof operation_id === "number") {
    if (!Number.isSafeInteger(operation_id) || operation_id < 1) {
      throw new Error(`InvalidOperationId actual=${operation_id}`);
    }
    return BigInt(operation_id);
  }
  if (operation_id < 1n || operation_id > MAX_OPERATION_ID) {
    throw new Error(`InvalidOperationId actual=${operation_id}`);
  }
  return operation_id;
}

function encodeInputFrame(kind: number, operation_id: OperationId, body: Uint8Array): Uint8Array {
  const id = toOperationId(operation_id);
  if (body.length > MAX_TERMINAL_INPUT_BODY_BYTES) {
    throw new Error(`PayloadTooLarge kind=${kind} max=${MAX_TERMINAL_INPUT_BODY_BYTES} actual=${body.length}`);
  }
  const out = new Uint8Array(INPUT_HEADER_BYTES + body.length);
  const view = new DataView(out.buffer);
  out[0] = TERMINAL_INPUT_SCHEME_VERSION;
  out[1] = kind;
  view.setUint16(2, body.length, false);
  view.setBigUint64(4, id, false);
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

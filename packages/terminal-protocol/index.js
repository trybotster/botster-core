export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 2;
export const PACKAGE_VERSION = "0.2.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY =
  "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";
export const TERMINAL_INPUT_SCHEME_VERSION = 1;
export const MAX_INPUT_DATA_BYTES = 65535;
export const MAX_MODE_GATED_DATA_BYTES = 65519;

export const metadata = {
  package_version: PACKAGE_VERSION,
  protocol: PROTOCOL,
  protocol_version: PROTOCOL_VERSION,
  conformance_fixture_revision: CONFORMANCE_FIXTURE_REVISION,
  features: [
    FEATURE_TERMINAL_STREAMING,
    FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    FEATURE_TRANSPORT_DUPLEX_BINARY,
  ],
};

function encodeTerminalInputFrame(kind, body) {
  const out = new Uint8Array(4 + body.length);
  out[0] = TERMINAL_INPUT_SCHEME_VERSION;
  out[1] = kind;
  const view = new DataView(out.buffer);
  view.setUint16(2, body.length, false);
  out.set(body, 4);
  return out;
}

export function encodeTerminalInput(data) {
  if (data.length > MAX_INPUT_DATA_BYTES) {
    throw new Error(
      `PayloadTooLarge kind=input max=${MAX_INPUT_DATA_BYTES} actual=${data.length}`,
    );
  }
  return encodeTerminalInputFrame(1, data);
}

export function encodeModeGatedInput(mode_generation, mode_revision, data) {
  if (data.length > MAX_MODE_GATED_DATA_BYTES) {
    throw new Error(
      `PayloadTooLarge kind=mode_gated_input max=${MAX_MODE_GATED_DATA_BYTES} actual=${data.length}`,
    );
  }
  const body = new Uint8Array(16 + data.length);
  const view = new DataView(body.buffer);
  view.setBigUint64(0, BigInt(mode_generation), false);
  view.setBigUint64(8, BigInt(mode_revision), false);
  body.set(data, 16);
  return encodeTerminalInputFrame(2, body);
}

export function encodeResize(rows, cols) {
  const body = new Uint8Array(4);
  const view = new DataView(body.buffer);
  view.setUint16(0, rows, false);
  view.setUint16(2, cols, false);
  return encodeTerminalInputFrame(3, body);
}

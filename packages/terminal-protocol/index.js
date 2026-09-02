export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 2;
export const PACKAGE_VERSION = "0.3.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY =
  "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";
export const TERMINAL_INPUT_SCHEME_VERSION = 1;
export const MAX_INPUT_DATA_BYTES = 65535;
export const MAX_MODE_GATED_DATA_BYTES = 65519;
export const MAX_PASTE_CHUNK_DATA_BYTES = 65527;
export const MAX_PASTE_BYTES = 1048576;
export const MAX_PASTE_CHUNKS = 17;

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

function assertOperationId(operation_id) {
  if (
    !Number.isInteger(operation_id) ||
    operation_id < 0 ||
    operation_id > 0xffffffff
  ) {
    throw new Error(`InvalidOperationId actual=${operation_id}`);
  }
}

export function encodePaste(operation_id, mode_generation, mode_revision, data) {
  assertOperationId(operation_id);
  if (data.length === 0) {
    throw new Error("EmptyPaste");
  }
  if (data.length > MAX_PASTE_BYTES) {
    throw new Error(
      `PayloadTooLarge kind=paste max=${MAX_PASTE_BYTES} actual=${data.length}`,
    );
  }
  const begin = new Uint8Array(24);
  const beginView = new DataView(begin.buffer);
  beginView.setUint32(0, operation_id, false);
  beginView.setBigUint64(4, BigInt(mode_generation), false);
  beginView.setBigUint64(12, BigInt(mode_revision), false);
  beginView.setUint32(20, data.length, false);
  const frames = [encodeTerminalInputFrame(4, begin)];
  for (
    let offset = 0, index = 0;
    offset < data.length;
    offset += MAX_PASTE_CHUNK_DATA_BYTES, index += 1
  ) {
    const chunkData = data.subarray(
      offset,
      Math.min(offset + MAX_PASTE_CHUNK_DATA_BYTES, data.length),
    );
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

export function encodePasteAbort(operation_id) {
  assertOperationId(operation_id);
  const body = new Uint8Array(4);
  new DataView(body.buffer).setUint32(0, operation_id, false);
  return encodeTerminalInputFrame(7, body);
}

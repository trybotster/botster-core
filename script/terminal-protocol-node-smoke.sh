#!/usr/bin/env bash
# Install the committed @trybotster/terminal-protocol pack in a clean temp dir
# and prove a Web-shaped consumer can import and compile generated types.
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  echo "node is required for the terminal protocol consumer smoke" >&2
  exit 1
fi
if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required for the terminal protocol consumer smoke" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
package_dir="$root/packages/terminal-protocol"
pack_dir="$(mktemp -d "${TMPDIR:-/tmp}/terminal-protocol-pack.XXXXXX")"
consumer_dir="$(mktemp -d "${TMPDIR:-/tmp}/terminal-protocol-consumer.XXXXXX")"
cleanup() {
  rm -rf "$pack_dir" "$consumer_dir"
}
trap cleanup EXIT

cd "$package_dir"
tarball_name="$(npm pack --pack-destination "$pack_dir")"
tarball_path="$pack_dir/$tarball_name"

cd "$consumer_dir"
npm init -y >/dev/null
npm install "$tarball_path" typescript --silent

cat > consumer.ts <<'EOF'
import {
  PROTOCOL,
  PROTOCOL_VERSION,
  FEATURE_TERMINAL_STREAMING,
  FEATURE_RESIZE,
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  FEATURE_TRANSPORT_DUPLEX_BINARY,
  PACKAGE_VERSION,
  encodeTerminalInput,
  encodeModeGatedInput,
  encodeResize,
  encodePaste,
  encodePasteAbort,
  type Attach,
  type Snapshot,
  type SnapshotPhase,
  type TerminalOutput,
  type ProcessExit,
  type AttachState,
  type TerminalCompatibilityRequirement,
  type TerminalEvent,
} from "@trybotster/terminal-protocol";

const phase: SnapshotPhase = "ready";
const attach: Attach = {
  type: "attach",
  session_id: "session",
  subscription_id: "sub",
};
const snapshot: Snapshot = {
  type: "snapshot",
  session_id: "session",
  subscription_id: "sub",
  payload_base64: "R0hPU1RTTlA=",
  payload_encoding: "base64",
  bytes: 8,
  phase,
};
const output: TerminalOutput = {
  type: "terminal_output",
  session_id: "session",
  subscription_id: "sub",
  payload_base64: "bGl2ZQ==",
  payload_encoding: "base64",
  bytes: 4,
};
const exitEvent: ProcessExit = {
  type: "process_exit",
  session_id: "session",
  subscription_id: "sub",
};
const attachState: AttachState = {
  type: "attach_state",
  session_id: "session",
  subscription_id: "sub",
  state: "attached",
};
const requirement: TerminalCompatibilityRequirement = {
  protocol: PROTOCOL,
  protocol_version: PROTOCOL_VERSION,
  required_features: [FEATURE_TERMINAL_STREAMING, FEATURE_RESIZE, FEATURE_TRANSPORT_DUPLEX_BINARY],
  minimum_conformance_fixture_revision: 1,
  client_name: "terminal-protocol-node-smoke",
};
const events: TerminalEvent[] = [snapshot, output, exitEvent, attachState];

void attach;
void snapshot;
void output;
void exitEvent;
void attachState;
void requirement;
void events;
void PROTOCOL;
void PROTOCOL_VERSION;
void FEATURE_TERMINAL_STREAMING;
void FEATURE_RESIZE;
void FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY;
void FEATURE_TRANSPORT_DUPLEX_BINARY;
void PACKAGE_VERSION;
void encodeTerminalInput(new Uint8Array([1]));
void encodeModeGatedInput(1, 1, new Uint8Array([2]));
void encodeResize(24, 80);
void encodePaste(1, 1, 1, new Uint8Array([3]));
void encodePasteAbort(1);
EOF

npx tsc --strict --module nodenext --moduleResolution nodenext --noEmit consumer.ts

node --input-type=module <<'EOF'
import {
  PROTOCOL,
  PROTOCOL_VERSION,
  FEATURE_TERMINAL_STREAMING,
  FEATURE_RESIZE,
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  FEATURE_TRANSPORT_DUPLEX_BINARY,
  PACKAGE_VERSION,
  encodeTerminalInput,
  encodeModeGatedInput,
  encodeResize,
  encodePaste,
  encodePasteAbort,
  metadata,
} from "@trybotster/terminal-protocol";

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, got ${actual}`);
  }
}

assertEqual(PACKAGE_VERSION, "0.3.0", "PACKAGE_VERSION");
assertEqual(PROTOCOL, "botster-terminal-v1", "PROTOCOL");
assertEqual(PROTOCOL_VERSION, 1, "PROTOCOL_VERSION");
assertEqual(FEATURE_TERMINAL_STREAMING, "terminal_streaming", "FEATURE_TERMINAL_STREAMING");
assertEqual(FEATURE_RESIZE, "resize", "FEATURE_RESIZE");
assertEqual(
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  "snapshot_delivery=ready_then_history",
  "FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY",
);
assertEqual(
  FEATURE_TRANSPORT_DUPLEX_BINARY,
  "transport=duplex_binary",
  "FEATURE_TRANSPORT_DUPLEX_BINARY",
);
assertEqual(metadata.package_version, "0.3.0", "metadata.package_version");
assertEqual(metadata.protocol, "botster-terminal-v1", "metadata.protocol");
assertEqual(metadata.protocol_version, 1, "metadata.protocol_version");
assertEqual(metadata.conformance_fixture_revision, 2, "metadata.conformance_fixture_revision");
for (const token of [
  "terminal_streaming",
  "resize",
  "snapshot_delivery=ready_then_history",
  "transport=duplex_binary",
]) {
  if (!metadata.features.includes(token)) {
    throw new Error(`metadata.features missing ${token}`);
  }
}

const input = encodeTerminalInput(new Uint8Array([0, 255]));
if (input[0] !== 1 || input[1] !== 1) {
  throw new Error("encodeTerminalInput header mismatch");
}
const gated = encodeModeGatedInput(1, 2, new Uint8Array([9]));
if (gated[1] !== 2) {
  throw new Error("encodeModeGatedInput kind mismatch");
}
const resize = encodeResize(24, 80);
if (resize.length !== 8) {
  throw new Error("encodeResize length mismatch");
}
const paste = encodePaste(7, 1, 2, new Uint8Array(70000));
assertEqual(paste.length, 4, "70k paste frame count");
assertEqual(paste.map((frame) => frame[1]).join(","), "4,5,5,6", "70k paste kinds");
const maxPaste = encodePaste(8, 1, 2, new Uint8Array(1048576));
assertEqual(maxPaste.length, 19, "maximum paste frame count");
assertEqual(encodePasteAbort(8)[1], 7, "paste abort kind");
for (const invalid of [new Uint8Array(0), new Uint8Array(1048577)]) {
  let rejected = false;
  try {
    encodePaste(9, 1, 2, invalid);
  } catch {
    rejected = true;
  }
  assertEqual(rejected, true, `paste length ${invalid.length} rejection`);
}

const imported = await import("@trybotster/terminal-protocol/metadata", {
  with: { type: "json" },
});
assertEqual(imported.default.package_version, "0.3.0", "imported metadata version");

const fixture = await import(
  "@trybotster/terminal-protocol/ready-then-history-event-order",
  { with: { type: "json" } }
);
if (!Array.isArray(fixture.default.events)) {
  throw new Error("event-order fixture missing events");
}
if (!fixture.default.required_features.includes("snapshot_delivery=ready_then_history")) {
  throw new Error("event-order fixture missing ready-then-history token");
}
console.log("terminal-protocol node smoke passed");
EOF

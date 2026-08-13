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
  PACKAGE_VERSION,
  type Attach,
  type Snapshot,
  type SnapshotPhase,
  type TerminalOutput,
  type ProcessExit,
  type AttachState,
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

void attach;
void snapshot;
void output;
void exitEvent;
void attachState;
void PROTOCOL;
void PROTOCOL_VERSION;
void FEATURE_TERMINAL_STREAMING;
void FEATURE_RESIZE;
void FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY;
void PACKAGE_VERSION;
EOF

npx tsc --strict --module nodenext --moduleResolution nodenext --noEmit consumer.ts

node --input-type=module <<'EOF'
import {
  PROTOCOL,
  PROTOCOL_VERSION,
  FEATURE_TERMINAL_STREAMING,
  FEATURE_RESIZE,
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  PACKAGE_VERSION,
  metadata,
} from "@trybotster/terminal-protocol";

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, got ${actual}`);
  }
}

assertEqual(PACKAGE_VERSION, "0.1.0", "PACKAGE_VERSION");
assertEqual(PROTOCOL, "botster-terminal-v1", "PROTOCOL");
assertEqual(PROTOCOL_VERSION, 1, "PROTOCOL_VERSION");
assertEqual(FEATURE_TERMINAL_STREAMING, "terminal_streaming", "FEATURE_TERMINAL_STREAMING");
assertEqual(FEATURE_RESIZE, "resize", "FEATURE_RESIZE");
assertEqual(
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  "snapshot_delivery=ready_then_history",
  "FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY",
);
assertEqual(metadata.package_version, "0.1.0", "metadata.package_version");
assertEqual(metadata.protocol, "botster-terminal-v1", "metadata.protocol");
assertEqual(metadata.protocol_version, 1, "metadata.protocol_version");
for (const token of [
  "terminal_streaming",
  "resize",
  "snapshot_delivery=ready_then_history",
]) {
  if (!metadata.features.includes(token)) {
    throw new Error(`metadata.features missing ${token}`);
  }
}

const imported = await import("@trybotster/terminal-protocol/metadata", {
  with: { type: "json" },
});
assertEqual(imported.default.package_version, "0.1.0", "imported metadata version");

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

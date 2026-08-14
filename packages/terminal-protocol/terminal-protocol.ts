// Generated from crates/botster-terminal-protocol-client Rust serde DTOs.
// Regenerate/check with: cargo test -p botster-terminal-protocol-client typescript

export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 1;
export const PACKAGE_VERSION = "0.1.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY = "snapshot_delivery=ready_then_history";

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

export type TerminalRequest = Attach | Detach | SendInput | Resize;

export type TerminalEvent = Snapshot | TerminalOutput | ProcessExit | AttachState;

export {
  PROTOCOL,
  PROTOCOL_VERSION,
  CONFORMANCE_FIXTURE_REVISION,
  PACKAGE_VERSION,
  FEATURE_TERMINAL_STREAMING,
  FEATURE_RESIZE,
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  FEATURE_TRANSPORT_DUPLEX_BINARY,
  TERMINAL_INPUT_SCHEME_VERSION,
  MAX_INPUT_DATA_BYTES,
  MAX_MODE_GATED_DATA_BYTES,
  MAX_PASTE_CHUNK_DATA_BYTES,
  MAX_PASTE_BYTES,
  MAX_PASTE_CHUNKS,
  TerminalCompatibility,
  TerminalCompatibilityRequirement,
  Attach,
  Detach,
  SendInput,
  Resize,
  SnapshotPhase,
  AttachStateKind,
  PayloadEncoding,
  Snapshot,
  TerminalOutput,
  ProcessExit,
  AttachState,
  TerminalModeFlags,
  TerminalInputKind,
  TerminalInputRejection,
  TerminalInputResult,
  TerminalRequest,
  TerminalEvent,
  encodeTerminalInput,
  encodeModeGatedInput,
  encodeResize,
  encodePaste,
  encodePasteAbort,
} from "./terminal-protocol.ts";

export const metadata: {
  package_version: string;
  protocol: string;
  protocol_version: number;
  conformance_fixture_revision: number;
  features: string[];
};

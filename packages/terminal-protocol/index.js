// Package metadata only. Codecs, enum tables, and key tables live in the
// generated `terminal-protocol.ts`; import that module directly. This file
// carries no hand-maintained copy of any generated definition.

export const PROTOCOL = "botster-terminal-v2";
export const PROTOCOL_VERSION = 2;
export const CONFORMANCE_FIXTURE_REVISION = 3;
export const PACKAGE_VERSION = "0.4.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY =
  "snapshot_delivery=ready_then_history";
export const FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary";

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

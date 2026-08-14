export const PROTOCOL = "botster-terminal-v1";
export const PROTOCOL_VERSION = 1;
export const CONFORMANCE_FIXTURE_REVISION = 1;
export const PACKAGE_VERSION = "0.1.0";
export const FEATURE_TERMINAL_STREAMING = "terminal_streaming";
export const FEATURE_RESIZE = "resize";
export const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY =
  "snapshot_delivery=ready_then_history";

export const metadata = {
  package_version: PACKAGE_VERSION,
  protocol: PROTOCOL,
  protocol_version: PROTOCOL_VERSION,
  conformance_fixture_revision: CONFORMANCE_FIXTURE_REVISION,
  features: [
    FEATURE_TERMINAL_STREAMING,
    FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  ],
};

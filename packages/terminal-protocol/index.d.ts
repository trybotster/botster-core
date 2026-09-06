export {
  PROTOCOL,
  PROTOCOL_VERSION,
  CONFORMANCE_FIXTURE_REVISION,
  PACKAGE_VERSION,
  FEATURE_TERMINAL_STREAMING,
  FEATURE_RESIZE,
  FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
  FEATURE_TRANSPORT_DUPLEX_BINARY,
} from "./terminal-protocol.ts";

export const metadata: {
  package_version: string;
  protocol: string;
  protocol_version: number;
  conformance_fixture_revision: number;
  features: string[];
};

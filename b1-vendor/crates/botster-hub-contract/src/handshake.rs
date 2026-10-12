//! The handshake (HC section 2): `hello`, `welcome`, the limits, and the typed `close`.

use crate::wire::{lenient, opt_no_null};
use botster_route_codec::prelude::ProtocolErrorCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The protocol name of the first client frame.
///
/// Clause: HC HS-1.
pub const PROTOCOL: &str = "botster-hub";

/// The `ui_embed` feature name (section 6 `view`, ui-contract 1.2) and the prefix of binding features (TB-0).
///
/// Clause: HC section 6, TB-0.
pub const FEATURE_UI_EMBED: &str = "ui_embed";
/// The feature name of ui-contract 1.1 (HC section 6 `kind` row).
///
/// Clause: HC section 6.
pub const FEATURE_UI_1_1: &str = "ui_1_1";
/// A binding feature is `binding:<name>`.
///
/// Clause: HC TB-0.
pub const BINDING_FEATURE_PREFIX: &str = "binding:";
/// The v1 binding names of `transport.binding`. The set is open: a new binding is a new section 9 entry.
///
/// Clause: HC TB-0, TS-1.
pub const BINDINGS: &[&str] = &["local-stream", "webrtc", "mcp"];
/// The label of the Hub peer connection's one reliable, ordered DataChannel for the envelope, watches and event streams.
///
/// Clause: HC TR-2.
pub const CONTROL_DATACHANNEL_LABEL: &str = "control";
/// The label prefix of a route's DataChannel on the same Hub peer connection: the label is `route:<route_id>`.
///
/// Clause: HC A8-2, TB-W2 (A8-7 item 3).
pub const ROUTE_DATACHANNEL_LABEL_PREFIX: &str = "route:";

/// The label of the route channel for `route_id` (A8-2).
///
/// Clause: HC A8-2.
pub fn route_datachannel_label(route_id: &str) -> String {
    format!("{ROUTE_DATACHANNEL_LABEL_PREFIX}{route_id}")
}

/// The route id that a route channel's label names, or `None` when the label is not a route label (A8-2: the Hub closes it).
///
/// Clause: HC A8-2.
pub fn route_id_of_label(label: &str) -> Option<&str> {
    label
        .strip_prefix(ROUTE_DATACHANNEL_LABEL_PREFIX)
        .filter(|id| !id.is_empty())
}

/// A Hub parameter in `hub.toml` that `welcome.limits` does not report: its key, default and inclusive range.
///
/// Clause: HC A8-4, HC A2 configuration table (A8-7 item 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubParameter {
    /// The `hub.toml` table and key.
    pub table: &'static str,
    pub key: &'static str,
    pub default: u64,
    pub min: u64,
    pub max: u64,
    /// The key applies at Hub start only ("restart").
    pub restart: bool,
}

impl HubParameter {
    /// Whether `value` is in the parameter's range.
    ///
    /// Clause: HC A8-4.
    pub fn accepts(&self, value: u64) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// The bound of a bridged route's send queue toward the client: the route payload read from the local stream and not yet
/// acknowledged. Default 1 MiB, range 4 KiB to 64 MiB, restart. It moved from Core 9B (Core A17) and is not in `welcome.limits`.
///
/// Clause: HC A8-4, A8-7 item 14.
pub const ROUTE_SEND_BUFFER_BYTES: HubParameter = HubParameter {
    table: "limits",
    key: "route_send_buffer_bytes",
    default: 1024 * 1024,
    min: 4 * 1024,
    max: 64 * 1024 * 1024,
    restart: true,
};

/// The protocol versions that a client supports.
///
/// Clause: HC HS-1, HS-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VersionRange {
    pub min: u16,
    pub max: u16,
}

/// The client's name and version.
///
/// Clause: HC HS-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// The first envelope frame of a client.
///
/// Clause: HC HS-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hello {
    /// `botster-hub`.
    pub protocol: String,
    pub versions: VersionRange,
    pub client: ClientInfo,
    pub features: Vec<String>,
    /// The route formats that the client can decode, in preference order.
    pub terminal_formats: Vec<String>,
}

/// The numbers of `welcome`. A client uses only these values. `max_chunk_bytes` bounds the chunk payload, without the `flags` byte,
/// on every DataChannel, the route channels included (TR-3, A8-3).
///
/// Clause: HC HS-3, BD-3, TR-3, A8-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Limits {
    pub max_frame_bytes: u64,
    pub max_chunk_bytes: u64,
    pub max_outstanding: u64,
    pub max_watches: u64,
    pub route_connect_seconds: u64,
    pub idempotency_window_seconds: u64,
    pub cursor_idle_seconds: u64,
    pub operation_retention_seconds: u64,
    pub max_paste_bytes: u64,
    pub max_file_bytes: u64,
    pub query_client_deadline_ms: u64,
}

/// The Hub's answer to `hello`.
///
/// Clause: HC HS-3, HS-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Welcome {
    pub hub_id: String,
    pub incarnation: String,
    pub version: u16,
    pub features: Vec<String>,
    /// Advisory: the route format is chosen per route at attach (TS-1). Absent when no listed format is supported now.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub terminal_format: Option<String>,
    pub limits: Limits,
}

/// `close { busy { retry_after_ms } }` data.
///
/// Clause: HC HS-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BusyData {
    pub retry_after_ms: u64,
}

/// `close { version_unsupported { hub_min, hub_max } }` data.
///
/// Clause: HC HS-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VersionUnsupportedData {
    pub hub_min: u16,
    pub hub_max: u16,
}

/// `close { protocol_error { code } }` data.
///
/// Clause: HC HS-9, A.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProtocolErrorData {
    pub code: ProtocolErrorCode,
}

/// `close { hub_shutdown { lock_released } }` data. `lock_released` is true only after the lock is released and the listener is removed.
///
/// Clause: HC HS-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HubShutdownData {
    pub lock_released: bool,
}

/// The reason of a typed `close`. Unit reasons are strings and reasons with data are one-key objects (A.1b). The set is non-exhaustive (A.2).
///
/// Clause: HC HS-9, A.1b, A.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum CloseReason {
    HandshakeOrder,
    VersionUnsupported(VersionUnsupportedData),
    ProtocolError(ProtocolErrorData),
    Busy(BusyData),
    HubShutdown(HubShutdownData),
    AuthFailed,
    RouteTokenUsed,
    RouteTokenInvalid,
    #[serde(other)]
    Other,
}

lenient! {
    CloseReason => ["handshake_order", "version_unsupported", "protocol_error", "busy", "hub_shutdown", "auth_failed", "route_token_used", "route_token_invalid"],
}

/// The `failed` codes of the ready line (non-exhaustive).
///
/// Clause: HC HS-0, A2 HS-0 edit, CF-1.
pub const READY_FAILURE_CODES: &[&str] = &[
    "lock_held",
    "config_invalid",
    "config_unknown_key",
    "socket_path_too_long",
    "socket_unavailable",
    "adoption_failed",
    "internal",
];

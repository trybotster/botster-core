//! Routes: the attach options and result, the transports, and the route close reasons (OU-1, OU-2, DP-2, DP-7, A2-3).

use crate::error::CoreError;
use crate::ids::RouteId;
use crate::types::Exit;
use crate::util::{lenient, opt_duration_ms};
use botster_route_codec::prelude::ProtocolErrorCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::time::Duration;

fn yes() -> bool {
    true
}

/// What the baseline of a route carries.
///
/// Clause: Core OU-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum HistoryMode {
    None = 1,
    Screen = 2,
    Full = 3,
}

/// The route limits that a host may choose at attach.
///
/// Clause: Core OU-1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteLimitChoices {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_screen_frame_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_history_page_bytes: Option<u64>,
}

/// The options of `attach`. Durations travel as milliseconds.
///
/// Clause: Core OU-1, Core EV-8, Core DP-5b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachOptions {
    /// An absolute directory that file input writes into.
    pub file_directory: String,
    /// Unix mode bits. The default is readable and writable by the session's user only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_permissions: Option<u32>,
    #[serde(default)]
    pub route_features: Vec<String>,
    /// In preference order.
    #[serde(default)]
    pub terminal_formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Required and positive when `answers_queries` is true.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "opt_duration_ms"
    )]
    #[schemars(with = "Option<u64>")]
    pub query_deadline: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_limits: Option<RouteLimitChoices>,
    /// OU-1 names no default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "opt_duration_ms"
    )]
    #[schemars(with = "Option<u64>")]
    pub stall_deadline: Option<Duration>,
    #[serde(default = "yes")]
    pub answers_queries: bool,
    #[serde(default = "yes")]
    pub input: bool,
}

/// The route limits that the attach result returns.
///
/// Clause: Core OU-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppliedRouteLimits {
    pub max_frame_bytes: u64,
    pub max_screen_frame_bytes: u64,
    pub max_history_page_bytes: u64,
    pub max_paste_bytes: u64,
    /// Not on the wire (OU-1).
    pub max_query_bytes: u64,
    pub max_query_reply_bytes: u64,
    pub max_file_bytes: u64,
    #[serde(with = "crate::util::duration_ms")]
    #[schemars(with = "u64")]
    pub query_deadline: Duration,
    #[serde(with = "crate::util::duration_ms")]
    #[schemars(with = "u64")]
    pub stall_deadline: Duration,
    #[serde(with = "crate::util::duration_ms")]
    #[schemars(with = "u64")]
    pub stall_close_after: Duration,
    /// Not on the wire (OU-1).
    pub route_input_queue_bytes: u64,
}

/// The result of the sync `attach`.
///
/// Clause: Core OU-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachResult {
    pub route: RouteId,
    pub limits: AppliedRouteLimits,
}

/// A connected stream endpoint that the host hands to Core. The value is the embedder's own stream type,
/// and Core takes it back with [`StreamEndpoint::downcast`]. From a successful `attach` it belongs to Core.
///
/// Clause: Core DP-2, Core OU-1, Core 13 (rule 1).
#[derive(Debug)]
pub struct StreamEndpoint(Box<dyn Any + Send>);

impl StreamEndpoint {
    /// Clause: Core DP-2.
    pub fn new<T: Any + Send>(stream: T) -> Self {
        StreamEndpoint(Box::new(stream))
    }

    /// Clause: Core DP-2.
    pub fn downcast<T: Any + Send>(self) -> Result<T, Self> {
        self.0.downcast::<T>().map(|b| *b).map_err(StreamEndpoint)
    }
}

/// The transport of a route (DP-2). `Stream` is the only one (A17-1).
///
/// Clause: Core DP-2, Core OU-1, Core A2-1, Core A17-1.
#[derive(Debug)]
#[non_exhaustive]
#[repr(u8)]
pub enum RouteTransport {
    Stream(StreamEndpoint) = 1,
}

/// A synchronously refused `attach`: the refusal, and the transport that stays with the caller.
///
/// Clause: Core DP-2 (the transport belongs to Core only from a successful `attach` return), Core AM-4, Core OU-1, steward ruling R-19.
#[derive(Debug)]
#[non_exhaustive]
pub struct AttachRefused {
    /// The refusal. Every sync refusal of OU-1 (`UnknownSession`, `RouteLimit`, `InvalidInput`, `Unsupported`) and the state refusal of A2-1.
    pub error: CoreError,
    /// The transport that the caller passed in. Core neither consumed nor closed it.
    pub transport: RouteTransport,
}

impl AttachRefused {
    /// Clause: Core DP-2, steward ruling R-19.
    pub fn new(error: CoreError, transport: RouteTransport) -> Self {
        AttachRefused { error, transport }
    }
}

/// The reason of `Detach`.
///
/// Clause: Core DP-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum DetachReason {
    Detached = 1,
    Replaced = 2,
    Revoked = 3,
}

/// The route close reasons of A2-3.
///
/// Clause: Core A2-3, Core OU-2, Core OU-2b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum RouteCloseReason {
    Detached = 1,
    Replaced = 2,
    Revoked = 3,
    SessionEnded {
        exit: Exit,
    } = 4,
    SessionRemoved = 5,
    SessionLost = 6,
    PeerClosed = 7,
    WriteFailed = 8,
    /// A2-3 limits the code to four values of the codec's set.
    BadFrame {
        code: ProtocolErrorCode,
    } = 9,
    SnapshotTooLarge = 12,
    StallTimeout = 13,
    HandoffFailed = 14,
    #[serde(other)]
    Other = 255,
}

lenient! {
    RouteCloseReason => ["Detached", "Replaced", "Revoked", "SessionEnded", "SessionRemoved", "SessionLost", "PeerClosed", "WriteFailed", "BadFrame", "SnapshotTooLarge", "StallTimeout", "HandoffFailed"],
}

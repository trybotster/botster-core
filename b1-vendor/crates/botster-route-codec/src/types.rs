//! Shared route types: `ModeFlags` (A.6), `route_limits` (TS-1), the reason sets (A.2, A.2b) and `QueryKind` (A.4).
//!
//! Every reason set is non-exhaustive on the wire (A.2). A receiver decodes an unknown value as `Other`.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Reads a non-exhaustive reason set. A string that this version does not know is `Other`, and so is an unknown one-key tagged value whose data is an object.
/// A known name with bad data, a value of another shape, and an object with several keys stay errors.
///
/// Clause: Codec A.1, A.2, A.4 (extension rules).
macro_rules! lenient {
    ($($t:ty => [$($name:literal),* $(,)?]),* $(,)?) => {$(
        impl Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                <$t>::serialize(self, s)
            }
        }
        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = serde_json::Value::deserialize(d)?;
                match <$t>::deserialize(&value) {
                    Ok(known) => Ok(known),
                    Err(error) => match &value {
                        serde_json::Value::Object(map)
                            if map.len() == 1
                                && map.values().all(serde_json::Value::is_object)
                                && !map.keys().any(|k| [$($name),*].contains(&k.as_str())) =>
                        {
                            Ok(<$t>::Other)
                        }
                        _ => Err(serde::de::Error::custom(error)),
                    },
                }
            }
        }
    )*};
}

/// `mouse_tracking` of A.6.
///
/// Clause: Codec A.6.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum MouseTracking {
    #[default]
    None,
    X10,
    Normal,
    ButtonEvent,
    AnyEvent,
    #[serde(other)]
    Other,
}

/// `mouse_encoding` of A.6.
///
/// Clause: Codec A.6.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum MouseEncoding {
    #[default]
    X10,
    Utf8,
    Sgr,
    Urxvt,
    SgrPixels,
    #[serde(other)]
    Other,
}

/// The session's modes on the route: `ModeFlags` of A.6, which is Core 5.1A verbatim.
///
/// Clause: Codec A.6, TS-4d.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModeFlags {
    pub alt_screen: bool,
    pub cursor_visible: bool,
    pub bracketed_paste: bool,
    pub application_cursor_keys: bool,
    pub application_keypad: bool,
    pub focus_reporting: bool,
    /// A bitset of the kitty flags 1, 2, 4, 8 and 16.
    #[cfg_attr(test, strategy(0u8..32))]
    pub kitty_flags: u8,
    pub mouse_tracking: MouseTracking,
    pub mouse_encoding: MouseEncoding,
    /// Modes outside this list. A receiver ignores names it does not know.
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::btree_map(proptest::prelude::any::<String>(), proptest::prelude::any::<bool>(), 0..3)))]
    pub other_modes: BTreeMap<String, bool>,
}

/// The limits of one route (TS-1), as the worker applies them (A.4 `attached`).
///
/// Clause: Codec TS-1, TS-3, A.4.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteLimits {
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_frame_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_screen_frame_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_history_page_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_paste_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_file_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_query_reply_bytes: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub query_client_deadline_ms: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub stall_deadline_ms: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub stall_close_after_ms: u64,
}

impl RouteLimits {
    /// The three frame bounds of TS-1. A frame size counts the type byte and the body.
    ///
    /// Clause: Codec TS-1, TS-3.
    pub fn bounds(&self) -> FrameBounds {
        FrameBounds {
            max_frame: self.max_frame_bytes,
            max_screen: self.max_screen_frame_bytes,
            max_history: self.max_history_page_bytes,
        }
    }
}

/// The frame size bounds of a route: `screen` and `history` have their own, and every other frame has `max_frame`.
///
/// Clause: Codec TS-1, TS-3, A.1 (`frame_too_large`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBounds {
    pub max_frame: u64,
    pub max_screen: u64,
    pub max_history: u64,
}

/// History `unavailable` reasons (A.2b).
///
/// Clause: Codec A.2b.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum HistoryUnavailable {
    Evicted,
    Restart,
    Oversize,
    CaptureFailed,
    #[serde(other)]
    Other,
}

/// History `incomplete` reasons (A.2b).
///
/// Clause: Codec A.2b.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum HistoryIncomplete {
    Truncated,
    CaptureFailed,
    #[serde(other)]
    Other,
}

/// `baseline_end.history` (A.4). `unavailable` and `incomplete` carry a reason string: the A.1 exception.
///
/// Clause: Codec A.4, A.1, A.2b, TS-4.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoryState {
    NotRequested,
    Complete,
    Unavailable(HistoryUnavailable),
    Incomplete(HistoryIncomplete),
}

/// `attach_failed` reasons (A.2b).
///
/// Clause: Codec A.2b, TS-4.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum AttachFailedReason {
    SessionNotRunning,
    ScreenUnavailable,
    NotAttachable,
    SnapshotTooLarge,
    BadPeer,
    #[serde(other)]
    Other,
}

/// `resync.reason` (A.4).
///
/// Clause: Codec A.4, TS-4c, TS-7.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum ResyncReason {
    Stalled,
    Overflow,
    #[serde(other)]
    Other,
}

/// `route_closed.reason` (A.2, TS-8).
///
/// Clause: Codec A.2, TS-8, A.1.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum CloseReason {
    Detached,
    Replaced,
    SessionEnded,
    Revoked,
    AttachFailed {
        reason: AttachFailedReason,
    },
    ProtocolError {
        code: crate::error::ProtocolErrorCode,
    },
    #[serde(other)]
    Other,
}

/// `route_ended.reason` (A.2, TS-9). It is the cause that the worker reports to the Hub for a failed route.
///
/// Clause: Codec A.2, TS-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum RouteEndReason {
    NotConnected,
    HandoffFailed,
    TransportLost,
    WriteFailed,
    Stalled,
    SessionLost,
    #[serde(other)]
    Other,
}

/// `input_refused.reason` (TS-5b), with the query-reply refusals of A.5. The set is non-exhaustive.
///
/// Clause: Codec TS-5b, A.5, P9.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum RefusalReason {
    NotReady,
    NotWritable,
    TooLarge,
    ModePreconditionFailed,
    Unsupported {
        what: String,
    },
    Refused,
    SessionEnded,
    Failed,
    WriteFailed,
    AlreadyReplied,
    QueryExpired,
    UnknownQuery,
    WrongRoute,
    InvalidInput,
    #[serde(other)]
    Other,
}

/// `terminal_query.kind` (A.4): Core's typed query kinds in snake_case. The set is non-exhaustive.
///
/// Clause: Codec A.4, P9.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum QueryKind {
    ClipboardRead {
        selection: String,
    },
    TextAreaPixels,
    CellPixels,
    ScreenPixels,
    WindowPixels,
    ScreenChars,
    WindowState,
    WindowPosition {
        area: String,
    },
    WindowTitle,
    IconLabel,
    ColorScheme,
    #[serde(other)]
    Other,
}

lenient! {
    MouseTracking => ["none", "x10", "normal", "button_event", "any_event"],
    MouseEncoding => ["x10", "utf8", "sgr", "urxvt", "sgr_pixels"],
    HistoryUnavailable => ["evicted", "restart", "oversize", "capture_failed"],
    HistoryIncomplete => ["truncated", "capture_failed"],
    AttachFailedReason => ["session_not_running", "screen_unavailable", "not_attachable", "snapshot_too_large", "bad_peer"],
    ResyncReason => ["stalled", "overflow"],
    CloseReason => ["detached", "replaced", "session_ended", "revoked", "attach_failed", "protocol_error"],
    RouteEndReason => ["not_connected", "handoff_failed", "transport_lost", "write_failed", "stalled", "session_lost"],
    RefusalReason => ["not_ready", "not_writable", "too_large", "mode_precondition_failed", "unsupported", "refused", "session_ended", "failed", "write_failed", "already_replied", "query_expired", "unknown_query", "wrong_route", "invalid_input"],
    QueryKind => ["clipboard_read", "text_area_pixels", "cell_pixels", "screen_pixels", "window_pixels", "screen_chars", "window_state", "window_position", "window_title", "icon_label", "color_scheme"],
}

/// The `state` of a session as the route codec's lifecycle table names it (A.2c).
///
/// Clause: Codec A.2c.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Includes Core `Created`.
    Starting,
    Running,
    Stopping,
    /// Includes a worker lost with a known termination (Core AD-2 `WorkerGone`, `StartInterrupted`).
    Ended,
    /// Adoption is unresolved (Core AD-2 `WorkerUnreachable`, `WorkerVersion`, `RegistryCorrupt`).
    Lost,
}

/// The `lifecycle_class` of a session (A.2c).
///
/// Clause: Codec A.2c.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleClass {
    Current,
    Ended,
    Indeterminate,
}

impl SessionState {
    /// The class of the state (A.2c).
    ///
    /// Clause: Codec A.2c.
    pub fn lifecycle_class(self) -> LifecycleClass {
        match self {
            SessionState::Starting | SessionState::Running | SessionState::Stopping => {
                LifecycleClass::Current
            }
            SessionState::Ended => LifecycleClass::Ended,
            SessionState::Lost => LifecycleClass::Indeterminate,
        }
    }

    /// Whether input is accepted, on a live route (A.2c).
    ///
    /// Clause: Codec A.2c.
    pub fn accepts_input(self) -> bool {
        self.lifecycle_class() == LifecycleClass::Current
    }

    /// Whether `restart` is allowed: only an ended session that is restartable (A.2c).
    ///
    /// Clause: Codec A.2c.
    pub fn can_restart(self, restartable: bool) -> bool {
        self == SessionState::Ended && restartable
    }

    /// Whether `delete` is allowed: an ended session, and a lost one by operator cleanup. A current session is `wrong_state` (A.2c).
    ///
    /// Clause: Codec A.2c.
    pub fn can_delete(self) -> bool {
        matches!(self, SessionState::Ended | SessionState::Lost)
    }
}

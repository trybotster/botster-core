//! Host input (`WriteInput`), its guards and its results (IN-1 to IN-10, A2-2, section 5.1A).

use crate::ids::{InputRev, ModelRev};
use crate::util::lenient;
use botster_route_codec::prelude::{HexBytes, Key, KeyEvent, Modifier, MouseAction, MouseButton};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A semantic key event (5.1A `Key`). `mods` is a set.
///
/// Clause: Core IN-9, Core 5.1A.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyInput {
    pub key: Key,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifted_key: Option<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_layout_key: Option<Key>,
    #[serde(default)]
    pub mods: Vec<Modifier>,
    pub event: KeyEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Host `WriteInput` only (IN-9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u16>,
}

/// A semantic mouse event (5.1A `Mouse`).
///
/// Clause: Core IN-9, Core 5.1A.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MouseInput {
    pub action: MouseAction,
    pub button: MouseButton,
    pub row: u32,
    pub col: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<u32>,
    #[serde(default)]
    pub mods: Vec<Modifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notches: Option<u32>,
}

/// The payload kinds of `WriteInput`: the same kinds as the route input frames.
///
/// Clause: Core IN-1, Core IN-8, Core IN-9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum InputPayload {
    /// Written as they are.
    Bytes {
        bytes: HexBytes,
    } = 1,
    /// One transaction of UTF-8 text.
    Text {
        text: String,
    } = 2,
    Paste {
        bytes: HexBytes,
        require_bracketed: bool,
    } = 3,
    Key(KeyInput) = 4,
    Mouse(MouseInput) = 5,
    Focus {
        focused: bool,
    } = 6,
}

/// Which source class an input revision counts.
///
/// Clause: Core IN-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum SourceClass {
    Client = 1,
    Host = 2,
}

/// The input guard of a write.
///
/// Clause: Core IN-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputGuard {
    pub source_class: SourceClass,
    pub rev: InputRev,
}

/// The guard of a write.
///
/// Clause: Core IN-10.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Guard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<InputGuard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_rev: Option<ModelRev>,
}

/// What an event had no encoding for. A receiver treats an unknown value as opaque.
///
/// Clause: Core A2-2, Core 5.1A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum UnsupportedWhat {
    PixelPosition = 1,
    ProducedText = 2,
    NamedKey = 3,
    WheelDirection = 4,
    Coordinate = 5,
    ReleaseEvent = 6,
    #[serde(other)]
    Other = 255,
}

/// Why a write is a certain zero.
///
/// Clause: Core IN-2, Core A2-2, Core IN-7, Core IN-8, Core IN-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum NotWrittenReason {
    LaneFull = 1,
    /// Route input (A2-2).
    Refused = 2,
    SessionEnded = 3,
    Stopping = 4,
    Stale = 5,
    /// Route input (IN-2).
    PayloadTooLarge = 6,
    ModePreconditionFailed = 7,
    NotReported = 8,
    Unsupported {
        what: UnsupportedWhat,
    } = 9,
}

/// The outcome of one write. The counts are exact for every outcome except `Unknown`.
///
/// Clause: Core IN-2, Core A2-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum WriteOutcome {
    Written = 1,
    NotWritten(NotWrittenReason) = 2,
    Partial = 3,
    Failed = 4,
    Cancelled = 5,
    Unknown { max_payload_bytes: u64 } = 6,
}

/// The completion value of `WriteInput`. `detail` has no compatibility promise.
///
/// Clause: Core IN-2, Core A2-2, Core IN-9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputResult {
    pub outcome: WriteOutcome,
    pub payload_bytes_written: u64,
    pub pty_bytes_written: u64,
    #[serde(default)]
    pub detail: String,
}

/// Why a cancel was refused.
///
/// Clause: Core IN-6, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum CancelRefusal {
    /// Clause: Core IN-6.
    Full = 1,
    /// Clause: Core A2-1.
    NotCancellable = 2,
}

/// The result of `cancel(op)`.
///
/// Clause: Core IN-6, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum CancelResult {
    Admitted = 1,
    AlreadyAdmitted = 2,
    TooLate = 3,
    UnknownOp = 4,
    Refused(CancelRefusal) = 5,
}

lenient! {
    UnsupportedWhat => ["pixel_position", "produced_text", "named_key", "wheel_direction", "coordinate", "release_event"],
}

//! The results of the reads (section 6.1): the screen, the cursor, the modes, captures, facts and the small sync reads.

use crate::ids::{CaptureId, InputCursor, InputRev, InstanceId, ModelRev};
use crate::input::{MouseInput, NotWrittenReason};
use botster_route_codec::prelude::{HexBytes, Key, KeyEvent, ModeFlags, Modifier};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::Size;

/// The result of `ReadScreen`.
///
/// Clause: Core ST-2, Core A2-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Screen {
    pub text: String,
    pub rows: u32,
    pub cols: u32,
    pub history_unavailable: bool,
    pub model_rev: ModelRev,
}

/// The result of `ReadCursor`. `row` and `col` are zero-based cells.
///
/// Clause: Core ST-3, Core A2-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub row_text: String,
    pub text_before_cursor: String,
    pub model_rev: ModelRev,
}

/// The result of `ReadModeFlags`.
///
/// Clause: Core ST-1, Core A2-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Modes {
    pub flags: ModeFlags,
    pub model_rev: ModelRev,
}

/// The result of `CaptureSnapshot`.
///
/// Clause: Core ST-6, Core A2-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capture {
    pub capture: CaptureId,
    pub page_count: u32,
    pub total_bytes: u64,
    pub model_rev: ModelRev,
}

/// One page of a capture, as `read_page` returns it.
///
/// Clause: Core ST-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    pub index: u32,
    pub bytes: HexBytes,
    pub last: bool,
}

/// A format that a session can emit for a baseline or a capture.
///
/// Clause: Core ST-6, Core OU-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotFormat {
    pub name: String,
    pub version: u32,
}

/// The cursor as `ReadFacts` reports it.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FactsCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
}

/// The client part of the input facts.
///
/// Clause: Core ST-7, Core IN-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientFacts {
    pub rev: InputRev,
    pub age_ms: u64,
}

/// The host side of the input facts.
///
/// Clause: Core ST-7, Core IN-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostFacts {
    pub rev: InputRev,
}

/// The kind of an input record.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum InputKind {
    Bytes = 1,
    Text = 2,
    Paste = 3,
    Key = 4,
    Mouse = 5,
    Focus = 6,
}

/// The source of an input record.
///
/// Clause: Core ST-7, Core IN-4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum InputSource {
    Client {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_tag: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    } = 1,
    Host = 2,
}

/// The state of an input record. `pending` has no byte count yet.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum InputRecordState {
    Pending = 1,
    Written = 2,
    Partial = 3,
    NotWritten { reason: NotWrittenReason } = 4,
    Cancelled = 5,
    Unknown = 6,
}

/// A record of one PTY input, created at the worker's receipt of it.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputRecord {
    pub cursor: InputCursor,
    pub source: InputSource,
    /// The worker's monotonic stamp.
    pub at: u64,
    pub kind: InputKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<KeyEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mods: Option<Vec<Modifier>>,
    /// The original key fields of 5.1A, kept as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifted_key: Option<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_layout_key: Option<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u16>,
    /// The `Paste` field of IN-8, kept as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_bracketed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse: Option<MouseInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<HexBytes>,
    #[serde(default)]
    pub payload_truncated: bool,
    pub state: InputRecordState,
    /// Null while the record is pending (ST-7).
    #[serde(default)]
    pub pty_bytes: Option<u64>,
}

/// The status update of an earlier record.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputStatusUpdate {
    pub cursor: InputCursor,
    pub updates: InputCursor,
    pub state: InputRecordState,
    #[serde(default)]
    pub pty_bytes: Option<u64>,
}

/// One entry of `input_records`: a record or the status update of an earlier record.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
// A boxed record would change the public form for a list that is already heap-allocated.
#[allow(clippy::large_enum_variant)]
pub enum InputRecordEntry {
    Record(InputRecord) = 1,
    StatusUpdate(InputStatusUpdate) = 2,
}

/// The input part of `Facts`.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputFacts {
    pub client: ClientFacts,
    pub host: HostFacts,
    pub client_tail: HexBytes,
    pub tail_complete: bool,
    pub input_records: Vec<InputRecordEntry>,
    pub next_cursor: InputCursor,
    pub gap: bool,
}

/// The result of `ReadFacts`: all facts taken at one point in the worker's ordering.
///
/// Clause: Core ST-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Facts {
    pub model_rev: ModelRev,
    pub modes: ModeFlags,
    pub size: Size,
    pub cursor: FactsCursor,
    pub input: InputFacts,
}

/// The result of `terminal_identity()`.
///
/// Clause: Core A2-8 (TI-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalIdentity {
    pub term: String,
    pub terminfo_source: String,
}

/// One chunk of the raw-output tap. `dropped_before` counts the bytes lost before this chunk.
///
/// Clause: Core TP-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TapChunk {
    pub instance: InstanceId,
    pub bytes: HexBytes,
    pub dropped_before: u64,
}

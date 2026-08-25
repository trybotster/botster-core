//! Semantic terminal event types.

use std::collections::BTreeMap;

use base64::Engine as _;
use botster_terminal_protocol::TerminalFrame;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Define a public wire enum and its complete variant inventory together.
///
/// Adding a variant updates [`$name::ALL`] in the same expansion. The TypeScript
/// generator and drift tests iterate that inventory.
macro_rules! wire_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// Every published variant, in definition order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

wire_enum! {
    /// Snapshot delivery phase. Adding a variant at 0.1.0 is a breaking change.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SnapshotPhase {
        /// Snapshot prefix through READY.
        Ready,
        /// HISTORY records through one PAGE.
        History,
        /// Remaining zero-page HISTORY records through FINISH.
        Finish,
    }
}

wire_enum! {
    /// Wire `attach_state.state` values. `detached` is not published.
    ///
    /// Adding a variant at 0.1.0 is a breaking change.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AttachStateKind {
        /// Attach has been requested.
        Attaching,
        /// Initial snapshot completed and live output may flow.
        Attached,
        /// READY installed, but later snapshot history did not complete.
        SnapshotHistoryIncomplete,
        /// Attach failed before any READY snapshot.
        AttachFailed,
    }
}

wire_enum! {
    /// Envelope encoding. The only published value is the literal `base64`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum PayloadEncoding {
        /// Standard base64.
        Base64,
    }
}

wire_enum! {
    /// Input command kind on `input_result`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TerminalInputKind {
        /// Raw input.
        Input,
        /// Mode-gated input.
        ModeGatedInput,
        /// Resize.
        Resize,
    }
}

wire_enum! {
    /// Client-visible rejection for a live owner.
    ///
    /// `Malformed` and `QueueOverflow` are not published. Those conditions
    /// hard-stop the owner, and close is the report.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TerminalInputRejection {
        /// Worker rejected a stale freshness token.
        StaleMode,
        /// The PTY accepted fewer bytes than submitted.
        PartialWrite,
        /// The gated wait exceeded its deadline.
        Timeout,
        /// The session cannot accept input.
        SessionNotWritable,
    }
}

/// Opaque Snapshot event. Clients must not render these bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Session that produced the snapshot.
    pub session_id: String,
    /// Subscription that receives the snapshot.
    pub subscription_id: String,
    /// Base64-encoded opaque engine state.
    pub payload_base64: String,
    /// Envelope encoding. Always `base64`.
    pub payload_encoding: PayloadEncoding,
    /// Declared decoded byte length.
    pub bytes: usize,
    /// First-class snapshot phase on this plane.
    pub phase: SnapshotPhase,
}

/// Live terminal output. Decoded bytes are renderable PTY data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalOutput {
    /// Session that produced the output.
    pub session_id: String,
    /// Subscription that receives the output.
    pub subscription_id: String,
    /// Base64-encoded live PTY bytes.
    pub payload_base64: String,
    /// Envelope encoding. Always `base64`.
    pub payload_encoding: PayloadEncoding,
    /// Declared decoded byte length.
    pub bytes: usize,
}

/// Process exit event. Wire tag stays `process_exit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExit {
    /// Session that exited.
    pub session_id: String,
    /// Subscription that observes the exit.
    pub subscription_id: String,
    /// Optional exit code. Omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

/// Attach state event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachState {
    /// Session whose attach state changed.
    pub session_id: String,
    /// Subscription that observes the state.
    pub subscription_id: String,
    /// Public attach-state vocabulary.
    pub state: AttachStateKind,
}

/// Mode flags on an `input_result`. Mirrors Core `ModeFlags` one for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalModeFlags {
    /// Kitty keyboard protocol is enabled.
    pub kitty_enabled: bool,
    /// Cursor is visible.
    pub cursor_visible: bool,
    /// Bracketed paste is enabled.
    pub bracketed_paste: bool,
    /// Mouse-mode bit mask.
    pub mouse_mode: u8,
    /// Alternate screen is active.
    pub alt_screen: bool,
    /// Focus reporting is enabled.
    pub focus_reporting: bool,
    /// Application cursor keys are enabled.
    pub application_cursor: bool,
}

/// Per-command input outcome. Wire tag is `input_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalInputResult {
    /// Subscription that submitted the command.
    pub subscription_id: String,
    /// Command kind.
    pub kind: TerminalInputKind,
    /// Whether the command was admitted.
    pub admitted: bool,
    /// Bytes written to the PTY.
    pub bytes_written: usize,
    /// Freshness generation reported with the outcome.
    pub mode_generation: u64,
    /// Freshness revision reported with the outcome.
    pub mode_revision: u64,
    /// Mode flags reported with the outcome.
    pub mode_flags: TerminalModeFlags,
    /// Rejection when the command was not fully admitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection: Option<TerminalInputRejection>,
}

/// Semantic terminal event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Opaque snapshot with a first-class phase.
    Snapshot(Snapshot),
    /// Live renderable output.
    TerminalOutput(TerminalOutput),
    /// Process exit.
    ProcessExit(ProcessExit),
    /// Attach state.
    AttachState(AttachState),
    /// Per-command input outcome.
    InputResult(TerminalInputResult),
}

/// Envelope or frame conversion error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// Base64 payload is invalid.
    #[error("{0}")]
    Invalid(String),
    /// Opaque frame conversion failed.
    #[error("{0}")]
    Frame(String),
}

impl Snapshot {
    /// Build a Snapshot from opaque bytes and a required phase.
    #[must_use]
    pub fn from_bytes(
        session_id: impl Into<String>,
        subscription_id: impl Into<String>,
        payload: &[u8],
        phase: SnapshotPhase,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            subscription_id: subscription_id.into(),
            payload_base64: encode_base64(payload),
            payload_encoding: PayloadEncoding::Base64,
            bytes: payload.len(),
            phase,
        }
    }

    /// Decode and validate the opaque snapshot bytes.
    pub fn decoded_bytes(&self) -> Result<Vec<u8>, EnvelopeError> {
        decode_validated_base64(&self.payload_base64, self.bytes, "opaque snapshot")
    }

    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        frame_from_tagged("snapshot", self)
    }
}

impl TerminalOutput {
    /// Build live output from renderable PTY bytes.
    #[must_use]
    pub fn from_bytes(
        session_id: impl Into<String>,
        subscription_id: impl Into<String>,
        payload: &[u8],
    ) -> Self {
        Self {
            session_id: session_id.into(),
            subscription_id: subscription_id.into(),
            payload_base64: encode_base64(payload),
            payload_encoding: PayloadEncoding::Base64,
            bytes: payload.len(),
        }
    }

    /// Decode and validate the live output bytes.
    pub fn decoded_bytes(&self) -> Result<Vec<u8>, EnvelopeError> {
        decode_validated_base64(&self.payload_base64, self.bytes, "live output")
    }

    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        frame_from_tagged("terminal_output", self)
    }
}

impl ProcessExit {
    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        frame_from_tagged("process_exit", self)
    }
}

impl AttachState {
    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        frame_from_tagged("attach_state", self)
    }
}

impl TerminalInputResult {
    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        frame_from_tagged("input_result", self)
    }
}

impl TerminalEvent {
    /// Encode this event into an opaque [`TerminalFrame`].
    pub fn to_frame(&self) -> Result<TerminalFrame, EnvelopeError> {
        match self {
            Self::Snapshot(event) => event.to_frame(),
            Self::TerminalOutput(event) => event.to_frame(),
            Self::ProcessExit(event) => event.to_frame(),
            Self::AttachState(event) => event.to_frame(),
            Self::InputResult(event) => event.to_frame(),
        }
    }

    /// Decode an opaque frame into a semantic event.
    pub fn from_frame(frame: &TerminalFrame) -> Result<Self, EnvelopeError> {
        let value: Value = serde_json::from_slice(&frame.to_bytes().map_err(frame_err)?)
            .map_err(|error| EnvelopeError::Invalid(error.to_string()))?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| EnvelopeError::Invalid("terminal event is missing type".into()))?;
        match event_type {
            "snapshot" => Ok(Self::Snapshot(
                serde_json::from_value(value).map_err(json_err)?,
            )),
            "terminal_output" => Ok(Self::TerminalOutput(
                serde_json::from_value(value).map_err(json_err)?,
            )),
            "process_exit" => Ok(Self::ProcessExit(
                serde_json::from_value(value).map_err(json_err)?,
            )),
            "attach_state" => Ok(Self::AttachState(
                serde_json::from_value(value).map_err(json_err)?,
            )),
            "input_result" => Ok(Self::InputResult(
                serde_json::from_value(value).map_err(json_err)?,
            )),
            other => Err(EnvelopeError::Invalid(format!(
                "unsupported terminal event type {other}"
            ))),
        }
    }
}

#[derive(Serialize)]
struct Tagged<'a, T> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    body: &'a T,
}

fn frame_from_tagged<T: Serialize>(
    kind: &'static str,
    body: &T,
) -> Result<TerminalFrame, EnvelopeError> {
    let bytes = serde_json::to_vec(&Tagged { kind, body }).map_err(json_err)?;
    TerminalFrame::from_bytes(&bytes).map_err(frame_err)
}

#[derive(Deserialize)]
struct SnapshotWire {
    session_id: String,
    subscription_id: String,
    payload_base64: String,
    payload_encoding: PayloadEncoding,
    bytes: usize,
    phase: SnapshotPhase,
}

impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SnapshotWire::deserialize(deserializer)?;
        decode_validated_base64(&wire.payload_base64, wire.bytes, "opaque snapshot")
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            session_id: wire.session_id,
            subscription_id: wire.subscription_id,
            payload_base64: wire.payload_base64,
            payload_encoding: wire.payload_encoding,
            bytes: wire.bytes,
            phase: wire.phase,
        })
    }
}

#[derive(Deserialize)]
struct TerminalOutputWire {
    session_id: String,
    subscription_id: String,
    payload_base64: String,
    payload_encoding: PayloadEncoding,
    bytes: usize,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for TerminalOutput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TerminalOutputWire::deserialize(deserializer)?;
        if wire.extra.contains_key("data") {
            return Err(serde::de::Error::custom(
                "legacy terminal_output data field is rejected",
            ));
        }
        decode_validated_base64(&wire.payload_base64, wire.bytes, "live output")
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            session_id: wire.session_id,
            subscription_id: wire.subscription_id,
            payload_base64: wire.payload_base64,
            payload_encoding: wire.payload_encoding,
            bytes: wire.bytes,
        })
    }
}

fn encode_base64(payload: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(payload)
}

fn decode_validated_base64(
    payload_base64: &str,
    bytes: usize,
    label: &str,
) -> Result<Vec<u8>, EnvelopeError> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(payload_base64)
        .map_err(|error| EnvelopeError::Invalid(format!("invalid {label} base64: {error}")))?;
    if payload.len() != bytes {
        return Err(EnvelopeError::Invalid(format!(
            "{label} byte length mismatch: declared {bytes}, decoded {}",
            payload.len()
        )));
    }
    Ok(payload)
}

fn json_err(error: serde_json::Error) -> EnvelopeError {
    EnvelopeError::Invalid(error.to_string())
}

fn frame_err(error: botster_terminal_protocol::TerminalFrameError) -> EnvelopeError {
    EnvelopeError::Frame(error.to_string())
}

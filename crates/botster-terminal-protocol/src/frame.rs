//! Opaque terminal event frames.
//!
//! A [`TerminalFrame`] serializes and deserializes JSON. It has no public
//! `phase`, `state`, `history`, `payload`, or Snapshot-body accessor.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const EVENT_TYPES: &[&str] = &[
    "snapshot",
    "terminal_output",
    "process_exit",
    "attach_state",
];

/// Opaque terminal event frame.
///
/// Construct from bytes and emit bytes. Semantic bodies belong in
/// `botster-terminal-protocol-client`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFrame {
    raw: Value,
}

impl TerminalFrame {
    /// Parse JSON bytes into an opaque frame.
    ///
    /// The JSON must be an object with a recognized event `type`. This method
    /// does not decode Snapshot bodies or expose attach phases.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TerminalFrameError> {
        let value: Value = serde_json::from_slice(bytes)?;
        Self::from_value(value)
    }

    /// Parse a JSON value into an opaque frame.
    pub fn from_value(value: Value) -> Result<Self, TerminalFrameError> {
        let object = value.as_object().ok_or(TerminalFrameError::NotAnObject)?;
        let event_type = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(TerminalFrameError::MissingType)?;
        if !EVENT_TYPES.contains(&event_type) {
            return Err(TerminalFrameError::UnknownType {
                found: event_type.to_string(),
            });
        }
        Ok(Self { raw: value })
    }

    /// Serialize this frame to JSON bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TerminalFrameError> {
        Ok(serde_json::to_vec(&self.raw)?)
    }
}

impl Serialize for TerminalFrame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TerminalFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(value).map_err(serde::de::Error::custom)
    }
}

/// Error while parsing or emitting an opaque terminal frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalFrameError {
    /// Frame JSON is not an object.
    #[error("terminal frame must be a JSON object")]
    NotAnObject,
    /// Frame JSON has no string `type` field.
    #[error("terminal frame is missing a string type field")]
    MissingType,
    /// Frame `type` is not a terminal event tag.
    #[error("unsupported terminal frame type {found}")]
    UnknownType {
        /// Observed type tag.
        found: String,
    },
    /// JSON parse or emit failed.
    #[error("terminal frame JSON error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for TerminalFrameError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

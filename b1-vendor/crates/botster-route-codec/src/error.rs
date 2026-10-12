//! Typed decode errors (Codec A.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `protocol_error` codes of A.1. The set is non-exhaustive on the wire (A.2).
///
/// Clause: Codec A.1, A.2.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum ProtocolErrorCode {
    FrameTooLarge,
    MalformedFrame,
    UnknownFrame,
    HandshakeOrder,
    DuplicateRequestId,
    InvalidInput,
    #[serde(other)]
    Other,
}

impl Serialize for ProtocolErrorCode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ProtocolErrorCode::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for ProtocolErrorCode {
    /// An unknown string is `Other`. Any other shape is an error (A.1).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        ProtocolErrorCode::deserialize(value).map_err(serde::de::Error::custom)
    }
}

/// Why a frame cannot be decoded. The wire carries only the [`ProtocolErrorCode`] (A.1); this detail is for the local log and for tests.
/// Each variant has exactly one code, given by [`DecodeDetail::code`].
///
/// Clause: Codec A.1, A.4, TS-3, TR-3, TB-L2.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeDetail {
    /// A frame with no type byte.
    EmptyFrame,
    /// A stream frame of length 0 (TB-L2).
    ZeroLengthFrame,
    /// A DataChannel message with no flags byte (TR-3).
    EmptyMessage,
    /// A DataChannel message with a reserved flag bit set (TR-3).
    ReservedFlagBits,
    /// A final DataChannel message that completes an empty frame (TR-3).
    ZeroSizeFrame,
    /// A frame over the bound of its type (A.1).
    FrameOverBound {
        type_byte: u8,
        size: u64,
        bound: u64,
    },
    /// A DataChannel chunk payload over `welcome.limits.max_chunk_bytes` (TR-3, HC A8-3).
    ChunkOverMax { size: u64, max: u64 },
    /// An unknown frame type without the ignorable bit (A.4).
    UnknownType { type_byte: u8 },
    /// A `live` frame with a body.
    LiveHasBody,
    /// A binary input frame shorter than its `op` (A.5).
    BinaryShorterThanOp,
    /// A paste or file chunk frame with `op` 0 (TS-5d, TS-5f).
    ChunkNeedsOp,
    /// A body that is not valid under the strict JSON rules (A.1).
    InvalidJson(String),
    /// A JSON body that does not have the shape of its frame.
    BodyShape { frame: &'static str, reason: String },
    /// An input body that is not an object.
    InputNotObject,
    /// An input body with no string `kind`.
    InputNoKind,
    /// A paste or file input with no nonzero `op` (TS-5d, TS-5f).
    InputNeedsOp,
    /// An input body that has a known `kind` and a broken shape.
    InputShape(String),
    /// An `output_deflate` stream that is corrupt or inflates over its cap (A.4 0x0E).
    OutputDeflate(String),
}

impl DecodeDetail {
    /// The code that ends the route for this detail (A.1).
    ///
    /// Clause: Codec A.1, A.4.
    pub fn code(&self) -> ProtocolErrorCode {
        use DecodeDetail::*;
        match self {
            FrameOverBound { .. } | ChunkOverMax { .. } => ProtocolErrorCode::FrameTooLarge,
            UnknownType { .. } => ProtocolErrorCode::UnknownFrame,
            BinaryShorterThanOp | OutputDeflate(_) => ProtocolErrorCode::InvalidInput,
            EmptyFrame
            | ZeroLengthFrame
            | EmptyMessage
            | ReservedFlagBits
            | ZeroSizeFrame
            | LiveHasBody
            | ChunkNeedsOp
            | InvalidJson(_)
            | BodyShape { .. }
            | InputNotObject
            | InputNoKind
            | InputNeedsOp
            | InputShape(_) => ProtocolErrorCode::MalformedFrame,
        }
    }
}

impl std::fmt::Display for DecodeDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DecodeDetail::*;
        match self {
            EmptyFrame => f.write_str("empty frame"),
            ZeroLengthFrame => f.write_str("a frame of length 0 has no type byte"),
            EmptyMessage => f.write_str("empty DataChannel message"),
            ReservedFlagBits => f.write_str("reserved flag bits are not 0"),
            ZeroSizeFrame => f.write_str("a frame of size 0 has no type byte"),
            FrameOverBound {
                type_byte,
                size,
                bound,
            } => write!(
                f,
                "frame of {size} bytes is over the bound {bound} of type {type_byte:#04x}"
            ),
            ChunkOverMax { size, max } => {
                write!(f, "chunk of {size} bytes is over max_chunk_bytes {max}")
            }
            UnknownType { type_byte } => write!(f, "unknown frame type {type_byte:#04x}"),
            LiveHasBody => f.write_str("live has no body"),
            BinaryShorterThanOp => f.write_str("binary input frame shorter than its op"),
            ChunkNeedsOp => f.write_str("a paste or file chunk needs a nonzero op"),
            InvalidJson(why) => write!(f, "invalid JSON body: {why}"),
            BodyShape { frame, reason } => write!(f, "{frame} body: {reason}"),
            InputNotObject => f.write_str("input body is not an object"),
            InputNoKind => f.write_str("input body has no string 'kind'"),
            InputNeedsOp => f.write_str("a paste or file frame needs a nonzero op"),
            InputShape(why) => write!(f, "input shape: {why}"),
            OutputDeflate(why) => write!(f, "output_deflate stream: {why}"),
        }
    }
}

/// A frame that cannot be decoded. Each decode error ends the route (Codec A.1).
///
/// Clause: Codec A.1, TS-3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    pub code: ProtocolErrorCode,
    pub detail: DecodeDetail,
}

impl DecodeError {
    /// The error of a detail; its code is `detail.code()`.
    ///
    /// Clause: Codec A.1.
    pub fn new(detail: DecodeDetail) -> Self {
        DecodeError {
            code: detail.code(),
            detail,
        }
    }
}

impl From<DecodeDetail> for DecodeError {
    fn from(detail: DecodeDetail) -> Self {
        DecodeError::new(detail)
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.detail)
    }
}

impl std::error::Error for DecodeError {}

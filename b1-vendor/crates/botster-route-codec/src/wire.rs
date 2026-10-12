//! Wire encoding and decoding of route frames (TS-3, A.4), and the two delimiters (TB-L2, TR-3).
//!
//! A frame is `[u8 type][body]`. A frame's size counts the type byte and the body. Each type has its own bound (A.1).

use crate::bytes::{Op, Payload};
use crate::error::{DecodeDetail, DecodeError};
use crate::frames::*;
use crate::input::{parse_input, ParsedInput};
use crate::strict;
use crate::types::FrameBounds;
use bytes::{Buf, Bytes, BytesMut};
use serde::Serialize;

/// Clause: Codec A.4, TS-3.
pub const TYPE_ATTACHED: u8 = 0x01;
/// Clause: Codec A.4, TS-3.
pub const TYPE_BASELINE_BEGIN: u8 = 0x02;
/// Clause: Codec A.4, TS-3.
pub const TYPE_SCREEN: u8 = 0x03;
/// Clause: Codec A.4, TS-3.
pub const TYPE_HISTORY: u8 = 0x04;
/// Clause: Codec A.4, TS-3.
pub const TYPE_BASELINE_END: u8 = 0x05;
/// Clause: Codec A.4, TS-3.
pub const TYPE_LIVE: u8 = 0x06;
/// Clause: Codec A.4, TS-3.
pub const TYPE_OUTPUT: u8 = 0x07;
/// Clause: Codec A.4, TS-3.
pub const TYPE_MODES: u8 = 0x08;
/// Clause: Codec A.4, TS-3.
pub const TYPE_RESYNC: u8 = 0x09;
/// Clause: Codec A.4, TS-3.
pub const TYPE_INPUT_REFUSED: u8 = 0x0A;
/// Clause: Codec A.4, TS-3.
pub const TYPE_ROUTE_CLOSED: u8 = 0x0B;
/// Clause: Codec A.4, TS-3.
pub const TYPE_TERMINAL_QUERY: u8 = 0x0C;
/// Clause: Codec A.4, TS-3.
pub const TYPE_INPUT_DONE: u8 = 0x0D;
/// Clause: Codec A.4, TS-3.
pub const TYPE_OUTPUT_DEFLATE: u8 = 0x0E;
/// Clause: Codec A.4, TS-3.
pub const TYPE_OUTPUT_DROPPED: u8 = 0x0F;
/// Clause: Codec A.4, TS-3.
pub const TYPE_INPUT: u8 = 0x81;
/// Clause: Codec A.4, TS-3.
pub const TYPE_PASTE_CHUNK: u8 = 0x82;
/// Clause: Codec A.4, TS-3.
pub const TYPE_BYTES: u8 = 0x83;
/// Clause: Codec A.4, TS-3.
pub const TYPE_TEXT: u8 = 0x84;
/// Clause: Codec A.4, TS-3.
pub const TYPE_FILE_CHUNK: u8 = 0x85;

/// The ignorable bit of a frame type (A.4, extension rules).
///
/// Clause: Codec A.4 (extension rules).
pub const IGNORABLE_BIT: u8 = 0x40;

/// The result of decoding one frame.
///
/// Clause: Codec A.4 (extension rules), A.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded<T> {
    Frame(T),
    /// An unknown frame type with the ignorable bit: the receiver drops it.
    Ignored {
        type_byte: u8,
    },
    /// A parseable input that the worker refuses with `unsupported { what }`, and the route stays open (A.5, TS-5b).
    Unsupported {
        op: Option<Op>,
        what: String,
    },
    /// A parseable input that breaks a field constraint (Core IN-9): refused, and the route stays open.
    Invalid {
        op: Option<Op>,
        what: &'static str,
    },
}

fn json_frame<T: Serialize>(type_byte: u8, body: &T) -> Vec<u8> {
    let mut out = vec![type_byte];
    out.extend(serde_json::to_vec(body).expect("a frame body serializes"));
    out
}

fn op_frame(type_byte: u8, op: Op, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![type_byte];
    out.extend(op.0.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// The body of a frame as JSON, without the `frame` tag.
fn untagged<T: Serialize>(frame: &T) -> serde_json::Value {
    let mut value = serde_json::to_value(frame).expect("a frame serializes");
    if let Some(map) = value.as_object_mut() {
        map.remove("frame");
    }
    value
}

impl ToClient {
    /// The type byte of the frame.
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn type_byte(&self) -> u8 {
        match self {
            ToClient::Attached(_) => TYPE_ATTACHED,
            ToClient::BaselineBegin(_) => TYPE_BASELINE_BEGIN,
            ToClient::Screen { .. } => TYPE_SCREEN,
            ToClient::History { .. } => TYPE_HISTORY,
            ToClient::BaselineEnd(_) => TYPE_BASELINE_END,
            ToClient::Live => TYPE_LIVE,
            ToClient::Output { .. } => TYPE_OUTPUT,
            ToClient::OutputDeflate { .. } => TYPE_OUTPUT_DEFLATE,
            ToClient::Modes(_) => TYPE_MODES,
            ToClient::Resync(_) => TYPE_RESYNC,
            ToClient::InputRefused(_) => TYPE_INPUT_REFUSED,
            ToClient::InputDone(_) => TYPE_INPUT_DONE,
            ToClient::TerminalQuery(_) => TYPE_TERMINAL_QUERY,
            ToClient::OutputDropped(_) => TYPE_OUTPUT_DROPPED,
            ToClient::RouteClosed(_) => TYPE_ROUTE_CLOSED,
        }
    }

    /// Encodes the frame as `[u8 type][body]`. The data frames carry raw bytes with no JSON (BP-3).
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn encode(&self) -> Vec<u8> {
        let t = self.type_byte();
        match self {
            ToClient::Screen { payload }
            | ToClient::History { payload }
            | ToClient::Output { payload }
            | ToClient::OutputDeflate { payload, .. } => {
                let mut out = vec![t];
                out.extend_from_slice(payload);
                out
            }
            ToClient::Live => vec![t],
            other => json_frame(t, &untagged(other)),
        }
    }

    /// Encodes the frame once into a shared buffer. Cloning the result clones a reference, not the bytes, so one session event
    /// goes to every route with one allocation (BP-2: every route gets the same bytes; BP-3: data frames are raw).
    ///
    /// Clause: Codec BP-2, BP-3, A.4.
    pub fn encode_shared(&self) -> Bytes {
        Bytes::from(self.encode())
    }

    /// Decodes `[u8 type][body]`. The frame size is checked against the bound of its type first (A.1).
    ///
    /// A payload is copied once out of `frame`; use [`ToClient::decode_shared`] to share the buffer. An `output_deflate` frame is
    /// inflated once, to validate it, and the frame keeps the inflated bytes ([`ToClient::inflated`]).
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn decode(frame: &[u8], bounds: &FrameBounds) -> Result<Decoded<ToClient>, DecodeError> {
        Self::decode_with(frame, bounds, &|from| {
            Bytes::copy_from_slice(&frame[from..])
        })
    }

    /// Like [`ToClient::decode`], and a payload is a slice of `frame`: no byte is copied (BP-2, BP-3).
    ///
    /// Clause: Codec A.4, TS-3, BP-3.
    pub fn decode_shared(
        frame: &Bytes,
        bounds: &FrameBounds,
    ) -> Result<Decoded<ToClient>, DecodeError> {
        Self::decode_with(frame, bounds, &|from| frame.slice(from..))
    }

    /// `body_from(1)` is the body of the frame as `Bytes`.
    fn decode_with(
        frame: &[u8],
        bounds: &FrameBounds,
        body_from: &dyn Fn(usize) -> Bytes,
    ) -> Result<Decoded<ToClient>, DecodeError> {
        let Some((&t, body)) = frame.split_first() else {
            return Err(DecodeDetail::EmptyFrame.into());
        };
        check_bound(t, frame.len(), bounds)?;
        let tagged = |name: &'static str, value: serde_json::Value| {
            let mut value = value;
            value
                .as_object_mut()
                .map(|m| m.insert("frame".to_string(), name.into()));
            serde_json::from_value::<ToClient>(value)
                .map(Decoded::Frame)
                .map_err(|e| {
                    DecodeDetail::BodyShape {
                        frame: name,
                        reason: e.to_string(),
                    }
                    .into()
                })
        };
        let payload = || Payload(body_from(1));
        let data = |f: ToClient| Ok(Decoded::Frame(f));
        match t {
            TYPE_ATTACHED => tagged("attached", strict::parse(body)?),
            TYPE_BASELINE_BEGIN => tagged("baseline_begin", strict::parse(body)?),
            TYPE_BASELINE_END => tagged("baseline_end", strict::parse(body)?),
            TYPE_MODES => tagged("modes", strict::parse(body)?),
            TYPE_RESYNC => tagged("resync", strict::parse(body)?),
            TYPE_INPUT_REFUSED => tagged("input_refused", strict::parse(body)?),
            TYPE_INPUT_DONE => tagged("input_done", strict::parse(body)?),
            TYPE_TERMINAL_QUERY => tagged("terminal_query", strict::parse(body)?),
            TYPE_OUTPUT_DROPPED => tagged("output_dropped", strict::parse(body)?),
            TYPE_ROUTE_CLOSED => tagged("route_closed", strict::parse(body)?),
            TYPE_SCREEN => data(ToClient::Screen { payload: payload() }),
            TYPE_HISTORY => data(ToClient::History { payload: payload() }),
            TYPE_OUTPUT => data(ToClient::Output { payload: payload() }),
            TYPE_OUTPUT_DEFLATE => {
                // Codec A.4 0x0E: the inflated payload is at most the output frame bound minus the type byte.
                let inflated = inflate_output(body, bounds.max_frame.saturating_sub(1))?;
                Ok(Decoded::Frame(ToClient::OutputDeflate {
                    payload: payload(),
                    inflated: Inflated::new(Bytes::from(inflated)),
                }))
            }
            TYPE_LIVE => {
                if body.is_empty() {
                    data(ToClient::Live)
                } else {
                    Err(DecodeDetail::LiveHasBody.into())
                }
            }
            other => unknown_type(other),
        }
    }
}

impl ToWorker {
    /// The type byte of the frame.
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn type_byte(&self) -> u8 {
        match self {
            ToWorker::Input(_) => TYPE_INPUT,
            ToWorker::PasteChunk { .. } => TYPE_PASTE_CHUNK,
            ToWorker::Bytes { .. } => TYPE_BYTES,
            ToWorker::Text { .. } => TYPE_TEXT,
            ToWorker::FileChunk { .. } => TYPE_FILE_CHUNK,
        }
    }

    /// Encodes the frame as `[u8 type][body]`.
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToWorker::Input(input) => json_frame(TYPE_INPUT, input),
            ToWorker::PasteChunk { op, bytes } => op_frame(TYPE_PASTE_CHUNK, *op, bytes),
            ToWorker::Bytes { op, bytes } => op_frame(TYPE_BYTES, *op, bytes),
            ToWorker::Text { op, text } => op_frame(TYPE_TEXT, *op, text.as_bytes()),
            ToWorker::FileChunk { op, bytes } => op_frame(TYPE_FILE_CHUNK, *op, bytes),
        }
    }

    /// Decodes `[u8 type][body]`.
    ///
    /// A binary frame shorter than its 8-byte `op` is `invalid_input`. A `paste_chunk` or `file_chunk` with `op` 0 is `malformed_frame` (TS-5d, TS-5f).
    /// Text that is not valid UTF-8 is `Decoded::Unsupported { what: "utf8" }` (TS-5b). A payload is copied once out of `frame`.
    ///
    /// Clause: Codec A.4, TS-3.
    pub fn decode(frame: &[u8], bounds: &FrameBounds) -> Result<Decoded<ToWorker>, DecodeError> {
        Self::decode_with(frame, bounds, &|from| {
            Bytes::copy_from_slice(&frame[from..])
        })
    }

    /// Like [`ToWorker::decode`], and a payload is a slice of `frame`: no byte is copied (BP-3).
    ///
    /// Clause: Codec A.4, TS-3, BP-3.
    pub fn decode_shared(
        frame: &Bytes,
        bounds: &FrameBounds,
    ) -> Result<Decoded<ToWorker>, DecodeError> {
        Self::decode_with(frame, bounds, &|from| frame.slice(from..))
    }

    /// `body_from(n)` is the frame from offset `n` as `Bytes`.
    fn decode_with(
        frame: &[u8],
        bounds: &FrameBounds,
        body_from: &dyn Fn(usize) -> Bytes,
    ) -> Result<Decoded<ToWorker>, DecodeError> {
        let Some((&t, body)) = frame.split_first() else {
            return Err(DecodeDetail::EmptyFrame.into());
        };
        check_bound(t, frame.len(), bounds)?;
        match t {
            TYPE_INPUT => Ok(match parse_input(body)? {
                ParsedInput::Input(input) => Decoded::Frame(ToWorker::Input(input)),
                ParsedInput::Unsupported { op, what } => Decoded::Unsupported { op, what },
                ParsedInput::Invalid { op, what } => Decoded::Invalid { op, what },
            }),
            TYPE_PASTE_CHUNK | TYPE_BYTES | TYPE_TEXT | TYPE_FILE_CHUNK => {
                let Some((head, rest)) = body.split_at_checked(8) else {
                    return Err(DecodeDetail::BinaryShorterThanOp.into());
                };
                let op = Op(u64::from_be_bytes(head.try_into().expect("8 bytes")));
                let data = || Payload(body_from(9));
                if op.0 == 0 && (t == TYPE_PASTE_CHUNK || t == TYPE_FILE_CHUNK) {
                    return Err(DecodeDetail::ChunkNeedsOp.into());
                }
                Ok(Decoded::Frame(match t {
                    TYPE_PASTE_CHUNK => ToWorker::PasteChunk { op, bytes: data() },
                    TYPE_BYTES => ToWorker::Bytes { op, bytes: data() },
                    TYPE_FILE_CHUNK => ToWorker::FileChunk { op, bytes: data() },
                    _ => match std::str::from_utf8(rest) {
                        Ok(text) => ToWorker::Text {
                            op,
                            text: text.to_string(),
                        },
                        Err(_) => {
                            return Ok(Decoded::Unsupported {
                                op: (op.0 != 0).then_some(op),
                                what: "utf8".to_string(),
                            })
                        }
                    },
                }))
            }
            other => unknown_type(other),
        }
    }
}

fn unknown_type<T>(t: u8) -> Result<Decoded<T>, DecodeError> {
    if t & IGNORABLE_BIT != 0 {
        Ok(Decoded::Ignored { type_byte: t })
    } else {
        Err(DecodeDetail::UnknownType { type_byte: t }.into())
    }
}

/// The size bound of a frame type (A.1 `frame_too_large`). `screen` and `history` have their own.
///
/// Clause: Codec TS-1, TS-3.
pub fn bound_of(type_byte: u8, bounds: &FrameBounds) -> u64 {
    match type_byte {
        TYPE_SCREEN => bounds.max_screen,
        TYPE_HISTORY => bounds.max_history,
        _ => bounds.max_frame,
    }
}

fn check_bound(type_byte: u8, size: usize, bounds: &FrameBounds) -> Result<(), DecodeError> {
    if size as u64 > bound_of(type_byte, bounds) {
        return Err(DecodeDetail::FrameOverBound {
            type_byte,
            size: size as u64,
            bound: bound_of(type_byte, bounds),
        }
        .into());
    }
    Ok(())
}

/// Inflates the raw DEFLATE stream of an `output_deflate` frame (A.4 0x0E).
///
/// A stream that is corrupt, or that inflates to more than `max_inflated` bytes, is `invalid_input`.
///
/// Clause: Codec A.4 (0x0E).
pub fn inflate_output(stream: &[u8], max_inflated: u64) -> Result<Vec<u8>, DecodeError> {
    let limit = usize::try_from(max_inflated).unwrap_or(usize::MAX);
    miniz_oxide::inflate::decompress_to_vec_with_limit(stream, limit)
        .map_err(|e| DecodeDetail::OutputDeflate(format!("{e:?}")).into())
}

/// Deflates one output payload as a raw DEFLATE stream (A.4 0x0E). Each frame is compressed alone and shares no state.
///
/// Clause: Codec A.4 (0x0E).
pub fn deflate_output(payload: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(payload, 6)
}

/// Delimits a frame for a stream binding: `[u32 big-endian length][frame]` (TB-L2).
///
/// Clause: Codec TB-L2, TS-3.
pub fn stream_wrap(frame: &[u8]) -> Vec<u8> {
    let mut out = (frame.len() as u32).to_be_bytes().to_vec();
    out.extend(frame);
    out
}

/// Splits a frame for a DataChannel: `[u8 flags][chunk]` messages. Bit 0 of `flags` is `final` (TR-3).
///
/// `sizes` gives the chunk sizes in order. The last size repeats. A size is at least 1.
///
/// Clause: Codec TR-3.
pub fn chunk_wrap(frame: &[u8], sizes: &[usize]) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut rest = frame;
    let mut i = 0;
    loop {
        let size = sizes
            .get(i)
            .or(sizes.last())
            .copied()
            .unwrap_or(rest.len())
            .max(1);
        let (head, tail) = rest.split_at(size.min(rest.len()));
        let mut message = vec![u8::from(tail.is_empty())];
        message.extend(head);
        messages.push(message);
        rest = tail;
        i += 1;
        if rest.is_empty() {
            return messages;
        }
    }
}

/// Reassembles frames from a byte stream: `[u32 big-endian length][frame]` (TB-L2).
///
/// The bound of the frame's type is checked as soon as the type byte arrives, before the rest of the frame is read.
///
/// Clause: Codec TB-L2, TS-3, A.1.
#[derive(Debug, Default)]
pub struct StreamReader {
    /// `BytesMut` keeps a read cursor: taking a frame moves the start of the buffer and copies nothing, and the memory
    /// of the consumed prefix is reclaimed by `reserve` once no returned frame holds it.
    buffer: BytesMut,
}

impl StreamReader {
    /// Clause: Codec TS-3.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds received bytes.
    ///
    /// Clause: Codec TB-L2.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The number of buffered bytes of an incomplete frame.
    ///
    /// Clause: Codec TS-3.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// The next complete frame (`[u8 type][body]`), or `None` when more bytes are needed. The frame is a slice of the
    /// receive buffer, so no byte is copied and no later byte is moved.
    ///
    /// Clause: Codec TB-L2, TS-3.
    pub fn next_frame(&mut self, bounds: &FrameBounds) -> Result<Option<Bytes>, DecodeError> {
        let Some(head) = self.buffer.first_chunk::<4>() else {
            return Ok(None);
        };
        let length = u32::from_be_bytes(*head) as usize;
        if length == 0 {
            return Err(DecodeDetail::ZeroLengthFrame.into());
        }
        if let Some(&t) = self.buffer.get(4) {
            check_bound(t, length, bounds)?;
        }
        if self.buffer.len() < 4 + length {
            return Ok(None);
        }
        self.buffer.advance(4);
        Ok(Some(self.buffer.split_to(length).freeze()))
    }
}

/// Reassembles frames from DataChannel messages `[u8 flags][chunk]` (TR-3).
///
/// A frame is refused as soon as it passes its bound. A frame that is incomplete when the channel closes is discarded with [`ChunkReader::discard_incomplete`].
///
/// Clause: Codec TR-3, TS-3, A.1.
#[derive(Debug, Default)]
pub struct ChunkReader {
    buffer: BytesMut,
    max_chunk: Option<u64>,
}

impl ChunkReader {
    /// Clause: Codec TS-3.
    pub fn new() -> Self {
        Self::default()
    }

    /// A reader that refuses a chunk over `welcome.limits.max_chunk_bytes`, before it buffers the chunk. The bound is the chunk
    /// payload, without the `flags` byte, on every channel (Codec TR-3, HC A8-3).
    ///
    /// Clause: Codec TR-3.
    pub fn with_max_chunk(max_chunk_bytes: u64) -> Self {
        ChunkReader {
            buffer: BytesMut::new(),
            max_chunk: Some(max_chunk_bytes),
        }
    }

    /// Adds one message. A complete frame is returned when the message has the `final` bit.
    ///
    /// Clause: Codec TR-3.
    pub fn push_message(
        &mut self,
        message: &[u8],
        bounds: &FrameBounds,
    ) -> Result<Option<Bytes>, DecodeError> {
        let Some((&flags, chunk)) = message.split_first() else {
            return Err(DecodeDetail::EmptyMessage.into());
        };
        if flags & !1 != 0 {
            return Err(DecodeDetail::ReservedFlagBits.into());
        }
        if self.max_chunk.is_some_and(|m| chunk.len() as u64 > m) {
            return Err(DecodeDetail::ChunkOverMax {
                size: chunk.len() as u64,
                max: self.max_chunk.unwrap_or(0),
            }
            .into());
        }
        self.buffer.extend_from_slice(chunk);
        if let Some(&t) = self.buffer.first() {
            check_bound(t, self.buffer.len(), bounds)?;
        }
        if flags & 1 == 1 {
            if self.buffer.is_empty() {
                return Err(DecodeDetail::ZeroSizeFrame.into());
            }
            return Ok(Some(self.buffer.split().freeze()));
        }
        Ok(None)
    }

    /// Discards the chunks of an incomplete frame, when the channel closes.
    ///
    /// Clause: Codec TR-3.
    pub fn discard_incomplete(&mut self) {
        self.buffer.clear();
    }

    /// The number of buffered bytes of an incomplete frame.
    ///
    /// Clause: Codec TS-3.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }
}

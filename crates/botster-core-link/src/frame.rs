//! Framing: `[u32 LE len][u8 type][payload]` (plan section 3).
//!
//! `len` counts the payload bytes only; the type byte is not in `len`. The decoder checks `len` against its bound as soon as
//! the five header bytes are present, before it waits for or allocates the payload, so a hostile length costs nothing.
//!
//! Clause: Core AD-4, Core AD-6, Core DP-8 (the link carries the hello and the epoch).

use std::collections::VecDeque;
use std::fmt;

/// The bytes before the payload: the length and the type.
pub const HEADER_LEN: usize = 5;

/// The bound of a decoder that nobody configured. A package that carries bulk data (tap chunks, snapshot pages) chooses its own.
pub const DEFAULT_MAX_PAYLOAD: u32 = 1 << 20;

/// The type byte of a frame. P0 names the hello; the packages that own the other messages name theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameType(pub u8);

impl FrameType {
    /// The first frame of every link (see [`crate::hello`]).
    pub const HELLO: FrameType = FrameType(0x01);
}

/// One frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameType,
    pub payload: Vec<u8>,
}

/// A frame that the link refuses. The stream cannot continue after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The length is above the bound of the decoder or of the encoder.
    TooLarge { len: u32, max: u32 },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLarge { len, max } => {
                write!(
                    f,
                    "frame payload of {len} bytes is above the bound of {max}"
                )
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Appends one frame to `out`. Refuses a payload above `max_payload`.
pub fn encode_frame(
    kind: FrameType,
    payload: &[u8],
    max_payload: u32,
    out: &mut Vec<u8>,
) -> Result<(), FrameError> {
    let too_large = FrameError::TooLarge {
        len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
        max: max_payload,
    };
    let len = u32::try_from(payload.len()).map_err(|_| too_large)?;
    if len > max_payload {
        return Err(too_large);
    }
    out.reserve(HEADER_LEN + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.push(kind.0);
    out.extend_from_slice(payload);
    Ok(())
}

/// A sans-IO frame decoder. The driver gives it the bytes that it read, and takes frames.
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    max_payload: u32,
    buffer: VecDeque<u8>,
    failed: Option<FrameError>,
}

impl FrameDecoder {
    /// A decoder that refuses a payload above `max_payload`.
    pub fn new(max_payload: u32) -> FrameDecoder {
        FrameDecoder {
            max_payload,
            buffer: VecDeque::new(),
            failed: None,
        }
    }

    /// Adds bytes that the driver read. After a refusal the decoder keeps nothing.
    pub fn push(&mut self, bytes: &[u8]) {
        if self.failed.is_none() {
            self.buffer.extend(bytes);
        }
    }

    /// The bytes that wait for a complete frame.
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// The next complete frame, `None` when more bytes are needed, or the refusal. A refusal is final: the decoder returns it
    /// again at every call.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, FrameError> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        if self.buffer.len() < HEADER_LEN {
            return Ok(None);
        }
        let mut header = [0u8; HEADER_LEN];
        for (slot, byte) in header.iter_mut().zip(self.buffer.iter()) {
            *slot = *byte;
        }
        let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        if len > self.max_payload {
            let error = FrameError::TooLarge {
                len,
                max: self.max_payload,
            };
            self.failed = Some(error);
            self.buffer = VecDeque::new();
            return Err(error);
        }
        // `len` is at most `max_payload`, so the size below is bounded by the configured bound.
        let total = HEADER_LEN + len as usize;
        if self.buffer.len() < total {
            return Ok(None);
        }
        self.buffer.drain(..HEADER_LEN);
        let payload: Vec<u8> = self.buffer.drain(..len as usize).collect();
        Ok(Some(Frame {
            kind: FrameType(header[4]),
            payload,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        encode_frame(FrameType(kind), payload, 64, &mut out).unwrap();
        out
    }

    #[test]
    fn the_wire_form_is_length_type_payload() {
        assert_eq!(encoded(7, b"ab"), [2, 0, 0, 0, 7, b'a', b'b']);
        assert_eq!(encoded(9, b""), [0, 0, 0, 0, 9]);
    }

    #[test]
    fn a_frame_in_one_piece_decodes() {
        let mut decoder = FrameDecoder::new(64);
        decoder.push(&encoded(7, b"abc"));
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.kind, FrameType(7));
        assert_eq!(frame.payload, b"abc");
        assert_eq!(decoder.next_frame(), Ok(None));
        assert_eq!(decoder.buffered(), 0);
    }

    #[test]
    fn a_frame_in_single_bytes_decodes_only_when_complete() {
        let wire = encoded(3, b"hello");
        let mut decoder = FrameDecoder::new(64);
        for (i, byte) in wire.iter().enumerate() {
            assert_eq!(decoder.next_frame(), Ok(None), "before byte {i}");
            decoder.push(&[*byte]);
        }
        assert_eq!(decoder.next_frame().unwrap().unwrap().payload, b"hello");
    }

    #[test]
    fn two_frames_decode_in_order() {
        let mut wire = encoded(1, b"x");
        wire.extend(encoded(2, b"yz"));
        let mut decoder = FrameDecoder::new(64);
        decoder.push(&wire);
        assert_eq!(decoder.next_frame().unwrap().unwrap().kind, FrameType(1));
        assert_eq!(decoder.next_frame().unwrap().unwrap().kind, FrameType(2));
        assert_eq!(decoder.next_frame(), Ok(None));
    }

    /// The length is checked from the header alone: the refusal comes with no payload byte present.
    #[test]
    fn a_length_above_the_bound_is_refused_from_the_header() {
        let mut decoder = FrameDecoder::new(16);
        decoder.push(&[17, 0, 0, 0, 5]);
        assert_eq!(
            decoder.next_frame(),
            Err(FrameError::TooLarge { len: 17, max: 16 })
        );
        assert_eq!(decoder.buffered(), 0);
    }

    #[test]
    fn the_largest_length_is_refused_without_waiting_for_it() {
        let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
        decoder.push(&[0xff, 0xff, 0xff, 0xff, 0]);
        assert!(matches!(
            decoder.next_frame(),
            Err(FrameError::TooLarge { len: u32::MAX, .. })
        ));
    }

    #[test]
    fn a_length_at_the_bound_is_accepted() {
        let mut decoder = FrameDecoder::new(16);
        decoder.push(&[16, 0, 0, 0, 5]);
        assert_eq!(decoder.next_frame(), Ok(None));
        decoder.push(&[0; 16]);
        assert_eq!(decoder.next_frame().unwrap().unwrap().payload.len(), 16);
    }

    #[test]
    fn a_refusal_is_final_and_later_bytes_are_dropped() {
        let mut decoder = FrameDecoder::new(4);
        decoder.push(&[5, 0, 0, 0, 1]);
        let error = decoder.next_frame().unwrap_err();
        decoder.push(&encoded(1, b"ok"));
        assert_eq!(decoder.buffered(), 0);
        assert_eq!(decoder.next_frame(), Err(error));
    }

    #[test]
    fn the_encoder_refuses_a_payload_above_the_bound() {
        let mut out = vec![0xAA];
        assert_eq!(
            encode_frame(FrameType(1), &[0; 5], 4, &mut out),
            Err(FrameError::TooLarge { len: 5, max: 4 })
        );
        assert_eq!(out, [0xAA], "a refusal writes nothing");
        assert!(encode_frame(FrameType(1), &[0; 4], 4, &mut out).is_ok());
    }
}

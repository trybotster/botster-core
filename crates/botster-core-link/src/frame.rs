//! Framing: `[u32 LE len][u8 type][payload]` (plan section 3).
//!
//! `len` counts the payload bytes only; the type byte is not in `len`. The decoder checks `len` against its bound as soon as
//! the five header bytes are present, before it buffers any payload byte, so a hostile length costs nothing, and it never
//! holds more than one frame (plan 2.5: one maximal link frame is the receive bound).
//!
//! Clause: Core AD-4, Core AD-6, Core DP-8 (the link carries the hello and the epoch).

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
    out.extend_from_slice(&len.to_le_bytes());
    out.push(kind.0);
    out.extend_from_slice(payload);
    Ok(())
}

/// A sans-IO frame decoder. The driver gives it the bytes that it read, and takes frames.
///
/// The decoder never holds more than one frame: at most [`HEADER_LEN`] header bytes and `max_payload` payload bytes. The
/// header is checked the moment its five bytes are present, and the payload buffer grows only with bytes that the peer
/// really sent, never past the checked length. So [`FrameDecoder::push`] takes only part of its input when a frame is
/// complete: the driver calls [`FrameDecoder::next_frame`], then pushes the rest.
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    max_payload: u32,
    header: [u8; HEADER_LEN],
    header_len: usize,
    payload: Vec<u8>,
    failed: Option<FrameError>,
}

impl FrameDecoder {
    /// A decoder that refuses a payload above `max_payload`.
    pub fn new(max_payload: u32) -> FrameDecoder {
        FrameDecoder {
            max_payload,
            header: [0; HEADER_LEN],
            header_len: 0,
            payload: Vec::new(),
            failed: None,
        }
    }

    /// The payload length that the header announced, once the header is complete and accepted.
    fn announced(&self) -> Option<usize> {
        (self.header_len == HEADER_LEN && self.failed.is_none()).then(|| {
            u32::from_le_bytes([
                self.header[0],
                self.header[1],
                self.header[2],
                self.header[3],
            ]) as usize
        })
    }

    /// Takes bytes that the driver read, and returns how many it took. It takes bytes up to the end of the current
    /// frame and no further: it returns less than `bytes.len()` when a frame is complete and waits for
    /// [`FrameDecoder::next_frame`]. After a refusal it takes and drops everything.
    pub fn push(&mut self, bytes: &[u8]) -> usize {
        if self.failed.is_some() {
            return bytes.len();
        }
        // The header bytes still missing; zero when the header is complete.
        let n = (HEADER_LEN - self.header_len).min(bytes.len());
        self.header[self.header_len..self.header_len + n].copy_from_slice(&bytes[..n]);
        self.header_len += n;
        let mut used = n;
        if self.header_len == HEADER_LEN {
            let len = u32::from_le_bytes([
                self.header[0],
                self.header[1],
                self.header[2],
                self.header[3],
            ]);
            if len > self.max_payload {
                // Refused from the header alone: no payload byte is kept, and the rest of the input is dropped.
                self.failed = Some(FrameError::TooLarge {
                    len,
                    max: self.max_payload,
                });
                self.header_len = 0;
                self.payload = Vec::new();
                return bytes.len();
            }
        }
        if let Some(len) = self.announced() {
            let n = (len - self.payload.len()).min(bytes.len() - used);
            self.payload.extend_from_slice(&bytes[used..used + n]);
            used += n;
        }
        used
    }

    /// The bytes that wait for a complete frame. It is at most `HEADER_LEN + max_payload`.
    pub fn buffered(&self) -> usize {
        self.header_len + self.payload.len()
    }

    /// The capacity that the payload buffer holds. It is zero when no payload is awaited.
    pub fn retained_capacity(&self) -> usize {
        self.payload.capacity()
    }

    /// The next complete frame, `None` when more bytes are needed, or the refusal. A refusal is final: the decoder returns it
    /// again at every call.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, FrameError> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        match self.announced() {
            Some(len) if self.payload.len() == len => {
                self.header_len = 0;
                Ok(Some(Frame {
                    kind: FrameType(self.header[4]),
                    payload: std::mem::take(&mut self.payload),
                }))
            }
            _ => Ok(None),
        }
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

    /// Pushes `bytes` the way a driver does: push, take frames, push the rest.
    fn feed(decoder: &mut FrameDecoder, mut bytes: &[u8]) -> Vec<Result<Frame, FrameError>> {
        let mut out = Vec::new();
        while !bytes.is_empty() {
            let took = decoder.push(bytes);
            bytes = &bytes[took..];
            match decoder.next_frame() {
                Ok(Some(frame)) => out.push(Ok(frame)),
                Ok(None) => {
                    assert!(
                        bytes.is_empty(),
                        "the decoder took too little with no frame to give"
                    );
                }
                Err(error) => {
                    out.push(Err(error));
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn the_default_bound_is_one_mebibyte() {
        assert_eq!(DEFAULT_MAX_PAYLOAD, 1_048_576);
    }

    #[test]
    fn a_refusal_names_the_length_and_the_bound() {
        let text = FrameError::TooLarge { len: 17, max: 16 }.to_string();
        assert!(text.contains("17") && text.contains("16"), "{text}");
    }

    #[test]
    fn buffered_counts_the_bytes_that_wait() {
        let mut decoder = FrameDecoder::new(64);
        assert_eq!(decoder.buffered(), 0);
        assert_eq!(decoder.push(&[3, 0, 0]), 3);
        assert_eq!(decoder.buffered(), 3);
        assert_eq!(decoder.push(&[0, 9, 1]), 3);
        assert_eq!(decoder.buffered(), 6);
    }

    #[test]
    fn the_wire_form_is_length_type_payload() {
        assert_eq!(encoded(7, b"ab"), [2, 0, 0, 0, 7, b'a', b'b']);
        assert_eq!(encoded(9, b""), [0, 0, 0, 0, 9]);
    }

    #[test]
    fn a_frame_in_one_piece_decodes() {
        let mut decoder = FrameDecoder::new(64);
        let frames = feed(&mut decoder, &encoded(7, b"abc"));
        assert_eq!(
            frames,
            [Ok(Frame {
                kind: FrameType(7),
                payload: b"abc".to_vec()
            })]
        );
        assert_eq!(decoder.next_frame(), Ok(None));
        assert_eq!(decoder.buffered(), 0);
    }

    #[test]
    fn a_frame_in_single_bytes_decodes_only_when_complete() {
        let wire = encoded(3, b"hello");
        let mut decoder = FrameDecoder::new(64);
        for (i, byte) in wire.iter().enumerate() {
            assert_eq!(decoder.next_frame(), Ok(None), "before byte {i}");
            assert_eq!(decoder.push(&[*byte]), 1);
        }
        assert_eq!(decoder.next_frame().unwrap().unwrap().payload, b"hello");
    }

    #[test]
    fn two_frames_in_one_chunk_decode_in_order() {
        let mut wire = encoded(1, b"x");
        wire.extend(encoded(2, b"yz"));
        let mut decoder = FrameDecoder::new(64);
        let kinds: Vec<u8> = feed(&mut decoder, &wire)
            .into_iter()
            .map(|f| f.unwrap().kind.0)
            .collect();
        assert_eq!(kinds, [1, 2]);
        assert_eq!(decoder.next_frame(), Ok(None));
    }

    /// The decoder stops at the end of a frame and leaves the rest of the chunk to the driver.
    #[test]
    fn push_takes_no_byte_past_a_complete_frame() {
        let mut wire = encoded(1, b"x");
        let first_len = wire.len();
        wire.extend(encoded(2, b"yz"));
        let mut decoder = FrameDecoder::new(64);
        assert_eq!(decoder.push(&wire), first_len);
        assert_eq!(
            decoder.push(&wire[first_len..]),
            0,
            "a complete frame waits to be taken"
        );
        assert_eq!(decoder.buffered(), first_len);
        decoder.next_frame().unwrap().unwrap();
        assert_eq!(decoder.push(&wire[first_len..]), wire.len() - first_len);
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

    /// F1: an oversized header and its body arrive in one chunk. The body is dropped, never copied.
    #[test]
    fn an_oversized_header_with_a_large_body_in_one_chunk_keeps_nothing() {
        let mut chunk = vec![17, 0, 0, 0, 5];
        chunk.extend(vec![0xAB; 1 << 20]);
        let mut decoder = FrameDecoder::new(16);
        assert_eq!(
            decoder.push(&chunk),
            chunk.len(),
            "the refused input is consumed"
        );
        assert_eq!(decoder.buffered(), 0);
        assert_eq!(decoder.retained_capacity(), 0);
        assert_eq!(
            decoder.next_frame(),
            Err(FrameError::TooLarge { len: 17, max: 16 })
        );
    }

    /// F1: the buffer never exceeds one frame, whatever the driver pushes.
    #[test]
    fn the_buffer_is_bounded_by_one_maximal_frame() {
        let mut decoder = FrameDecoder::new(16);
        let mut chunk = vec![16, 0, 0, 0, 5];
        chunk.extend(vec![1; 1000]);
        let took = decoder.push(&chunk);
        assert_eq!(took, HEADER_LEN + 16);
        assert_eq!(decoder.buffered(), HEADER_LEN + 16);
        assert!(
            decoder.retained_capacity() <= 2 * 16,
            "capacity {}",
            decoder.retained_capacity()
        );
        assert_eq!(decoder.push(&chunk[took..]), 0);
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
        assert_eq!(decoder.push(&[0; 16]), 16);
        assert_eq!(decoder.next_frame().unwrap().unwrap().payload.len(), 16);
    }

    #[test]
    fn an_empty_payload_frame_is_complete_at_its_header() {
        let mut decoder = FrameDecoder::new(16);
        assert_eq!(decoder.push(&[0, 0, 0, 0, 9, 1, 2]), 5);
        assert_eq!(
            decoder.next_frame().unwrap().unwrap(),
            Frame {
                kind: FrameType(9),
                payload: vec![]
            }
        );
    }

    #[test]
    fn a_refusal_is_final_and_later_bytes_are_dropped() {
        let mut decoder = FrameDecoder::new(4);
        decoder.push(&[5, 0, 0, 0, 1]);
        let error = decoder.next_frame().unwrap_err();
        let later = encoded(1, b"ok");
        assert_eq!(decoder.push(&later), later.len());
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

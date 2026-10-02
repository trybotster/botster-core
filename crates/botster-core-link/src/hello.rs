//! The hello (plan section 3): magic, protocol number, `InstanceId`, token proof and host epoch.
//!
//! The hello is the payload of a frame of type [`crate::frame::FrameType::HELLO`]. Layout:
//!
//! | field | size |
//! |---|---|
//! | magic `BCLK` | 4 |
//! | protocol number | 1 |
//! | `InstanceId` length, then its UTF-8 bytes | 2 (LE), then at most [`MAX_INSTANCE_ID_LEN`] |
//! | token proof | 32 |
//! | host epoch | 8 (LE) |
//!
//! The codec checks the form only. Whether the protocol number is adoptable (`Lost(WorkerVersion)`), whether the proof matches
//! the token (AD-6) and whether the epoch is the highest seen (DP-8) are decisions of the host and the worker. How the proof
//! is computed from the token is decided by the package that owns AD-6.
//!
//! Clause: Core AD-4, Core AD-6, Core DP-8, Core A6-2.

use botster_core_contract::prelude::InstanceId;
use std::fmt;

/// The first four bytes of a hello.
pub const MAGIC: [u8; 4] = *b"BCLK";

/// The longest `InstanceId` that a hello carries, in bytes.
pub const MAX_INSTANCE_ID_LEN: usize = 255;

/// The size of a token proof.
pub const PROOF_LEN: usize = 32;

/// Proof that the sender holds the per-worker token (AD-6). The codec treats it as opaque bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TokenProof(pub [u8; PROOF_LEN]);

impl fmt::Debug for TokenProof {
    /// A proof is a credential: it is never printed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TokenProof(..)")
    }
}

/// A decoded hello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// The worker protocol number (A6-2: the first one is 1). Any number decodes; the host judges it.
    pub protocol: u8,
    pub instance: InstanceId,
    pub proof: TokenProof,
    /// The host epoch (DP-8).
    pub host_epoch: u64,
}

/// Why a payload is not a hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloError {
    BadMagic,
    /// The payload ends before the field that the layout needs.
    Truncated,
    /// The `InstanceId` is longer than [`MAX_INSTANCE_ID_LEN`].
    InstanceIdTooLong,
    /// The `InstanceId` bytes are not UTF-8.
    InstanceIdNotUtf8,
    /// Bytes follow the last field.
    TrailingBytes,
}

impl fmt::Display for HelloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HelloError::BadMagic => "the hello does not start with the magic",
            HelloError::Truncated => "the hello ends early",
            HelloError::InstanceIdTooLong => "the instance id is too long",
            HelloError::InstanceIdNotUtf8 => "the instance id is not UTF-8",
            HelloError::TrailingBytes => "bytes follow the hello",
        })
    }
}

impl std::error::Error for HelloError {}

impl Hello {
    /// Appends the hello payload to `out`. Refuses an `InstanceId` that the layout cannot carry.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), HelloError> {
        let id = self.instance.0.as_bytes();
        if id.len() > MAX_INSTANCE_ID_LEN {
            return Err(HelloError::InstanceIdTooLong);
        }
        out.extend_from_slice(&MAGIC);
        out.push(self.protocol);
        // The length is at most 255, so it fits two bytes.
        out.extend_from_slice(&(id.len() as u16).to_le_bytes());
        out.extend_from_slice(id);
        out.extend_from_slice(&self.proof.0);
        out.extend_from_slice(&self.host_epoch.to_le_bytes());
        Ok(())
    }

    /// Decodes a hello payload. Strict: the whole payload is the hello.
    pub fn decode(payload: &[u8]) -> Result<Hello, HelloError> {
        let mut reader = Reader(payload);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(HelloError::BadMagic);
        }
        let protocol = reader.take(1)?[0];
        let len = u16::from_le_bytes(reader.take_array()?) as usize;
        if len > MAX_INSTANCE_ID_LEN {
            return Err(HelloError::InstanceIdTooLong);
        }
        let id = std::str::from_utf8(reader.take(len)?)
            .map_err(|_| HelloError::InstanceIdNotUtf8)?
            .to_owned();
        let proof = TokenProof(reader.take_array()?);
        let host_epoch = u64::from_le_bytes(reader.take_array()?);
        if !reader.0.is_empty() {
            return Err(HelloError::TrailingBytes);
        }
        Ok(Hello {
            protocol,
            instance: InstanceId(id),
            proof,
            host_epoch,
        })
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], HelloError> {
        if self.0.len() < n {
            return Err(HelloError::Truncated);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], HelloError> {
        let mut array = [0u8; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Hello {
        Hello {
            protocol: 1,
            instance: InstanceId("inst-1".into()),
            proof: TokenProof([7; PROOF_LEN]),
            host_epoch: 0x0102_0304_0506_0708,
        }
    }

    fn encoded(hello: &Hello) -> Vec<u8> {
        let mut out = Vec::new();
        hello.encode(&mut out).unwrap();
        out
    }

    #[test]
    fn the_wire_form_is_the_documented_layout() {
        let wire = encoded(&sample());
        let mut expected = b"BCLK".to_vec();
        expected.push(1);
        expected.extend_from_slice(&[6, 0]);
        expected.extend_from_slice(b"inst-1");
        expected.extend_from_slice(&[7; 32]);
        expected.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(wire, expected);
    }

    #[test]
    fn a_hello_round_trips() {
        let hello = sample();
        assert_eq!(Hello::decode(&encoded(&hello)), Ok(hello));
    }

    /// A6-2: an out-of-set protocol number still decodes; the host turns it into `Lost(WorkerVersion)`.
    #[test]
    fn any_protocol_number_decodes() {
        for protocol in [0u8, 1, 2, 255] {
            let hello = Hello {
                protocol,
                ..sample()
            };
            assert_eq!(Hello::decode(&encoded(&hello)).unwrap().protocol, protocol);
        }
    }

    #[test]
    fn the_longest_instance_id_round_trips_and_one_more_is_refused() {
        let longest = Hello {
            instance: InstanceId("a".repeat(MAX_INSTANCE_ID_LEN)),
            ..sample()
        };
        assert_eq!(Hello::decode(&encoded(&longest)), Ok(longest));
        let too_long = Hello {
            instance: InstanceId("a".repeat(MAX_INSTANCE_ID_LEN + 1)),
            ..sample()
        };
        assert_eq!(
            too_long.encode(&mut Vec::new()),
            Err(HelloError::InstanceIdTooLong)
        );
    }

    #[test]
    fn every_refusal_has_its_own_message() {
        let all = [
            HelloError::BadMagic,
            HelloError::Truncated,
            HelloError::InstanceIdTooLong,
            HelloError::InstanceIdNotUtf8,
            HelloError::TrailingBytes,
        ];
        let texts: std::collections::BTreeSet<String> =
            all.iter().map(ToString::to_string).collect();
        assert_eq!(texts.len(), all.len());
        assert!(texts.iter().all(|t| !t.is_empty()));
    }

    #[test]
    fn a_wrong_magic_is_refused() {
        let mut wire = encoded(&sample());
        wire[0] = b'X';
        assert_eq!(Hello::decode(&wire), Err(HelloError::BadMagic));
    }

    #[test]
    fn every_truncation_is_refused() {
        let wire = encoded(&sample());
        for end in 0..wire.len() {
            assert!(Hello::decode(&wire[..end]).is_err(), "prefix of {end}");
        }
        assert_eq!(Hello::decode(&wire[..3]), Err(HelloError::Truncated));
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut wire = encoded(&sample());
        wire.push(0);
        assert_eq!(Hello::decode(&wire), Err(HelloError::TrailingBytes));
    }

    #[test]
    fn an_instance_id_length_above_the_bound_is_refused_before_it_is_read() {
        let mut wire = b"BCLK\x01".to_vec();
        wire.extend_from_slice(&256u16.to_le_bytes());
        assert_eq!(Hello::decode(&wire), Err(HelloError::InstanceIdTooLong));
    }

    #[test]
    fn an_instance_id_that_is_not_utf8_is_refused() {
        let mut wire = b"BCLK\x01\x01\x00".to_vec();
        wire.push(0xff);
        wire.extend_from_slice(&[0; PROOF_LEN + 8]);
        assert_eq!(Hello::decode(&wire), Err(HelloError::InstanceIdNotUtf8));
    }

    #[test]
    fn a_proof_is_never_printed() {
        let shown = format!("{:?}", sample());
        assert!(shown.contains("TokenProof(..)"));
        assert!(!shown.contains("7, 7"));
    }
}

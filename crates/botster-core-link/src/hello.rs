//! The hello (plan section 3): magic, protocol number, `InstanceId`, token proof and host epoch.
//!
//! The hello is the payload of a frame of type [`crate::frame::FrameType::HELLO`]. It is a JSON object, as plan section 3
//! says of control messages, so that a newer worker of the same protocol number can add fields:
//!
//! ```json
//! {"magic":"BCLK","protocol":1,"instance":"…","proof":"<64 lowercase hex digits>","host_epoch":7}
//! ```
//!
//! The decoder requires the five fields with these types and **ignores every other field**. It checks the form only. Whether
//! the protocol number is adoptable (`Lost(WorkerVersion)`), whether the proof matches the token (AD-6) and whether the
//! epoch is the highest seen (DP-8) are decisions of the host and the worker. How the proof is computed from the token is
//! decided by the package that owns AD-6.
//!
//! Clause: Core AD-4, Core AD-6, Core DP-8, Core A6-2.

use botster_core_contract::prelude::InstanceId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The value of the `magic` field.
pub const MAGIC: &str = "BCLK";

/// The longest `InstanceId` that a hello carries, in bytes.
pub const MAX_INSTANCE_ID_LEN: usize = 255;

/// The size of a token proof, in bytes.
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
    /// Not a JSON object, or a required field is missing or has the wrong type.
    Malformed,
    BadMagic,
    /// The `InstanceId` is longer than [`MAX_INSTANCE_ID_LEN`].
    InstanceIdTooLong,
    /// The proof is not 64 lowercase hex digits.
    BadProof,
}

impl fmt::Display for HelloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HelloError::Malformed => "the hello is not a JSON object with the required fields",
            HelloError::BadMagic => "the hello has the wrong magic",
            HelloError::InstanceIdTooLong => "the instance id is too long",
            HelloError::BadProof => "the token proof is not 64 lowercase hex digits",
        })
    }
}

impl std::error::Error for HelloError {}

/// The wire form. Unknown fields are ignored (serde's default).
#[derive(Serialize, Deserialize)]
struct Wire {
    magic: String,
    protocol: u8,
    instance: String,
    proof: String,
    host_epoch: u64,
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<[u8; PROOF_LEN]> {
    let digits = text.as_bytes();
    if digits.len() != PROOF_LEN * 2 {
        return None;
    }
    let nibble = |d: u8| match d {
        b'0'..=b'9' => Some(d - b'0'),
        b'a'..=b'f' => Some(d - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; PROOF_LEN];
    for (byte, pair) in out.iter_mut().zip(digits.chunks(2)) {
        *byte = nibble(pair[0])? << 4 | nibble(pair[1])?;
    }
    Some(out)
}

impl Hello {
    /// Appends the hello payload to `out`. Refuses an `InstanceId` that the codec would refuse to decode.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), HelloError> {
        if self.instance.0.len() > MAX_INSTANCE_ID_LEN {
            return Err(HelloError::InstanceIdTooLong);
        }
        let wire = Wire {
            magic: MAGIC.to_string(),
            protocol: self.protocol,
            instance: self.instance.0.clone(),
            proof: to_hex(&self.proof.0),
            host_epoch: self.host_epoch,
        };
        serde_json::to_writer(&mut *out, &wire).map_err(|_| HelloError::Malformed)
    }

    /// Decodes a hello payload. The five fields are required; other fields are ignored.
    pub fn decode(payload: &[u8]) -> Result<Hello, HelloError> {
        let wire: Wire = serde_json::from_slice(payload).map_err(|_| HelloError::Malformed)?;
        if wire.magic != MAGIC {
            return Err(HelloError::BadMagic);
        }
        if wire.instance.len() > MAX_INSTANCE_ID_LEN {
            return Err(HelloError::InstanceIdTooLong);
        }
        let proof = from_hex(&wire.proof).ok_or(HelloError::BadProof)?;
        Ok(Hello {
            protocol: wire.protocol,
            instance: InstanceId(wire.instance),
            proof: TokenProof(proof),
            host_epoch: wire.host_epoch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Hello {
        Hello {
            protocol: 1,
            instance: InstanceId("inst-1".into()),
            proof: TokenProof([0xAB; PROOF_LEN]),
            host_epoch: u64::MAX,
        }
    }

    fn encoded(hello: &Hello) -> Vec<u8> {
        let mut out = Vec::new();
        hello.encode(&mut out).unwrap();
        out
    }

    fn json(text: &str) -> Result<Hello, HelloError> {
        Hello::decode(text.as_bytes())
    }

    const PROOF: &str = "abababababababababababababababababababababababababababababababab";

    #[test]
    fn the_wire_form_is_the_documented_object() {
        let expected = format!(
            r#"{{"magic":"BCLK","protocol":1,"instance":"inst-1","proof":"{PROOF}","host_epoch":18446744073709551615}}"#
        );
        assert_eq!(String::from_utf8(encoded(&sample())).unwrap(), expected);
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

    /// F2: a newer worker of the same protocol may add fields.
    #[test]
    fn an_unknown_field_is_ignored() {
        let text = format!(
            r#"{{"magic":"BCLK","protocol":1,"instance":"i","proof":"{PROOF}","host_epoch":3,"later":{{"a":[1,2]}},"x":null}}"#
        );
        let hello = json(&text).unwrap();
        assert_eq!(
            (hello.protocol, hello.host_epoch, hello.instance.0.as_str()),
            (1, 3, "i")
        );
    }

    #[test]
    fn every_required_field_is_required() {
        for missing in ["magic", "protocol", "instance", "proof", "host_epoch"] {
            let mut value: serde_json::Value = serde_json::from_slice(&encoded(&sample())).unwrap();
            value.as_object_mut().unwrap().remove(missing);
            assert_eq!(
                Hello::decode(value.to_string().as_bytes()),
                Err(HelloError::Malformed),
                "{missing}"
            );
        }
    }

    #[test]
    fn a_field_of_the_wrong_type_is_malformed() {
        let text = format!(
            r#"{{"magic":"BCLK","protocol":"1","instance":"i","proof":"{PROOF}","host_epoch":3}}"#
        );
        assert_eq!(json(&text), Err(HelloError::Malformed));
        let big = format!(
            r#"{{"magic":"BCLK","protocol":256,"instance":"i","proof":"{PROOF}","host_epoch":3}}"#
        );
        assert_eq!(json(&big), Err(HelloError::Malformed));
        assert_eq!(json("[]"), Err(HelloError::Malformed));
        assert_eq!(json("not json"), Err(HelloError::Malformed));
    }

    #[test]
    fn a_wrong_magic_is_refused() {
        let text = format!(
            r#"{{"magic":"XXXX","protocol":1,"instance":"i","proof":"{PROOF}","host_epoch":3}}"#
        );
        assert_eq!(json(&text), Err(HelloError::BadMagic));
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
        let text = format!(
            r#"{{"magic":"BCLK","protocol":1,"instance":"{}","proof":"{PROOF}","host_epoch":3}}"#,
            "a".repeat(MAX_INSTANCE_ID_LEN + 1)
        );
        assert_eq!(json(&text), Err(HelloError::InstanceIdTooLong));
    }

    #[test]
    fn a_proof_must_be_64_lowercase_hex_digits() {
        for bad in [
            &PROOF[..62],
            &format!("{PROOF}00"),
            &PROOF.to_uppercase(),
            &PROOF.replace('a', "g"),
            "",
        ] {
            let text = format!(
                r#"{{"magic":"BCLK","protocol":1,"instance":"i","proof":"{bad}","host_epoch":3}}"#
            );
            assert_eq!(json(&text), Err(HelloError::BadProof), "{bad}");
        }
    }

    #[test]
    fn hex_covers_every_digit() {
        let all: Vec<u8> = (0..PROOF_LEN as u8).map(|i| i * 8 + 1).collect();
        let mut proof = [0u8; PROOF_LEN];
        proof.copy_from_slice(&all);
        assert_eq!(from_hex(&to_hex(&proof)), Some(proof));
        assert_eq!(
            from_hex(&"0123456789abcdef".repeat(4)).unwrap()[..2],
            [0x01, 0x23]
        );
    }

    #[test]
    fn every_refusal_has_its_own_message() {
        let all = [
            HelloError::Malformed,
            HelloError::BadMagic,
            HelloError::InstanceIdTooLong,
            HelloError::BadProof,
        ];
        let texts: std::collections::BTreeSet<String> =
            all.iter().map(ToString::to_string).collect();
        assert_eq!(texts.len(), all.len());
        assert!(texts.iter().all(|t| !t.is_empty()));
    }

    #[test]
    fn a_proof_is_never_printed() {
        let shown = format!("{:?}", sample());
        assert!(shown.contains("TokenProof(..)"));
        assert!(!shown.to_lowercase().contains("abab"));
    }
}

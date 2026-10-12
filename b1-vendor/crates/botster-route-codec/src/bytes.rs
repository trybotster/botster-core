//! Byte payloads and operation numbers in the JSON forms of the codec.

use bytes::Bytes;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// Binary data in the decoded form of a frame: `{"$bytes_hex": "<lowercase hex>"}`.
///
/// Clause: Codec A.4, BP-3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HexBytes(pub Vec<u8>);

impl HexBytes {
    /// Clause: Codec A.4, BP-3.
    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }
}

/// The payload of a data frame in its decoded form: `{"$bytes_hex": "<lowercase hex>"}`, the same JSON as [`HexBytes`].
///
/// The bytes are a shared [`Bytes`]: a clone shares the buffer, and a frame decoded from a shared buffer
/// ([`crate::prelude::ToClient::decode_shared`]) holds a slice of it, with no copy (BP-2, BP-3).
///
/// Clause: Codec A.4, BP-3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Payload(pub Bytes);

impl From<Vec<u8>> for Payload {
    fn from(bytes: Vec<u8>) -> Self {
        Payload(bytes.into())
    }
}

impl From<&[u8]> for Payload {
    fn from(bytes: &[u8]) -> Self {
        Payload(Bytes::copy_from_slice(bytes))
    }
}

impl From<Bytes> for Payload {
    fn from(bytes: Bytes) -> Self {
        Payload(bytes)
    }
}

impl std::ops::Deref for Payload {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl Payload {
    /// Clause: Codec A.4, BP-3.
    pub fn to_hex(&self) -> String {
        hex_encode(self)
    }
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(1))?;
        map.serialize_entry("$bytes_hex", &self.to_hex())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Payload {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        HexBytes::deserialize(d).map(|h| Payload::from(h.0))
    }
}

impl JsonSchema for Payload {
    fn schema_name() -> Cow<'static, str> {
        HexBytes::schema_name()
    }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        HexBytes::json_schema(g)
    }
}

/// Lowercase hex of a byte slice.
///
/// Clause: Codec A.4, BP-3.
pub fn hex_encode(bytes: &[u8]) -> String {
    // Two table lookups per byte, not one `format!`: a fill payload can be 1 MiB.
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        hex.push(char::from(DIGITS[usize::from(b >> 4)]));
        hex.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    hex
}

/// The value of one hex digit (either case), or `None`.
fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Bytes of a hex string. Spaces are ignored. `Err` carries the reason.
///
/// Clause: Codec A.4, BP-3.
pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return Err("odd number of hex digits".to_string());
    }
    digits
        .chunks(2)
        .map(|pair| match (hex_digit(pair[0]), hex_digit(pair[1])) {
            (Some(h), Some(l)) => Ok(h * 16 + l),
            _ => Err("not a hex digit".to_string()),
        })
        .collect()
}

impl Serialize for HexBytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(1))?;
        map.serialize_entry("$bytes_hex", &self.to_hex())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for HexBytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = HexBytes;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object {\"$bytes_hex\": \"<hex>\"}")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<HexBytes, A::Error> {
                let mut out = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key != "$bytes_hex" || out.is_some() {
                        return Err(de::Error::custom("unexpected key in a bytes value"));
                    }
                    let text: String = map.next_value()?;
                    out = Some(hex_decode(&text).map_err(de::Error::custom)?);
                }
                out.map(HexBytes)
                    .ok_or_else(|| de::Error::custom("missing $bytes_hex"))
            }
        }
        d.deserialize_map(V)
    }
}

impl JsonSchema for HexBytes {
    fn schema_name() -> Cow<'static, str> {
        "HexBytes".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "properties": { "$bytes_hex": { "type": "string", "pattern": "^([0-9a-f]{2})*$" } },
            "required": ["$bytes_hex"],
            "additionalProperties": false
        })
    }
}

/// An operation number. In JSON it is a decimal string (A.1). In a binary frame it is a big-endian `u64`.
///
/// Clause: Codec A.1, A.4, A.5, TS-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Op(pub u64);

impl Serialize for Op {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Op {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Op;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a decimal string of a u64")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Op, E> {
                parse_op(v)
                    .map(Op)
                    .ok_or_else(|| E::custom("op is not a decimal u64 string"))
            }
        }
        d.deserialize_str(V)
    }
}

/// Parses a decimal string without sign, space or leading zero (except `"0"`).
///
/// Clause: Codec A.1.
pub fn parse_op(text: &str) -> Option<u64> {
    let ok = !text.is_empty()
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if ok {
        text.parse().ok()
    } else {
        None
    }
}

impl JsonSchema for Op {
    fn schema_name() -> Cow<'static, str> {
        "Op".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": op_pattern(), "maxLength": 20 })
    }
}

/// The pattern of a decimal `u64`: no sign, no leading zero, and no value over `u64::MAX`.
fn op_pattern() -> String {
    let max = u64::MAX.to_string();
    // Up to 19 digits always fit. A 20-digit value fits when, at the first digit that differs from the maximum, it is smaller.
    let mut alternatives = vec!["0".to_string(), "[1-9][0-9]{0,18}".to_string(), max.clone()];
    for (i, d) in max.bytes().enumerate().skip(1) {
        if d > b'0' {
            alternatives.push(format!(
                "{}[0-{}][0-9]{{{}}}",
                &max[..i],
                (d - 1) as char,
                max.len() - i - 1
            ));
        }
    }
    format!("^({})$", alternatives.join("|"))
}

/// Decodes standard base64. Returns `None` for a bad character.
/// Clause: Codec A.4, A.5.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Encodes standard base64 with padding.
/// Clause: Codec A.4.
pub fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, b)| a | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The file component for an uploaded name (Codec TS-5f): the name reduced to `A-Z a-z 0-9 . _ -`, an empty result becomes `file`,
/// and the whole component (`prefix` included) fits `limit` bytes. The extension stays only when it fits with at least one stem character.
/// Clause: Codec TS-5f.
pub fn file_component(prefix: &str, name: &str, limit: usize) -> String {
    let mut clean: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    if clean.is_empty() {
        clean = "file".to_string();
    }
    let full = format!("{prefix}{clean}");
    if full.len() <= limit {
        return full;
    }
    let room = limit.saturating_sub(prefix.len()).max(1);
    if let Some(dot) = clean.rfind('.') {
        let ext = &clean[dot..];
        if dot > 0 && ext.len() < room {
            let stem_room = room - ext.len();
            return format!("{prefix}{}{ext}", &clean[..dot.min(stem_room)]);
        }
    }
    format!("{prefix}{}", &clean[..room.min(clean.len())])
}

//! Strict JSON parsing for route bodies (Codec A.1).
//!
//! Invalid UTF-8, invalid JSON, a duplicate object key, a null, and an integer outside ±(2^53 − 1) are `malformed_frame`.
//!
//! Clause: Codec A.1.

use crate::error::{DecodeDetail, DecodeError};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

/// The largest integer magnitude that a JSON body may carry.
///
/// Clause: Codec A.1.
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                if v.unsigned_abs() > MAX_SAFE_INTEGER as u64 {
                    return Err(E::custom("integer outside the safe range"));
                }
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                if v > MAX_SAFE_INTEGER as u64 {
                    return Err(E::custom("integer outside the safe range"));
                }
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("not a finite number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.to_string())))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                // A.1: a route body has no field whose schema permits null.
                Err(E::custom("null is not accepted in a route body"))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
                let mut items = Vec::new();
                while let Some(Strict(v)) = seq.next_element()? {
                    items.push(v);
                }
                Ok(Strict(Value::Array(items)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
                let mut out = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate key '{key}'")));
                    }
                    let Strict(value) = map.next_value()?;
                    out.insert(key, value);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}

/// Parses a JSON body under the A.1 rules.
///
/// Clause: Codec A.1.
pub fn parse(bytes: &[u8]) -> Result<Value, DecodeError> {
    serde_json::from_slice::<Strict>(bytes)
        .map(|s| s.0)
        .map_err(|e| DecodeDetail::InvalidJson(e.to_string()).into())
}

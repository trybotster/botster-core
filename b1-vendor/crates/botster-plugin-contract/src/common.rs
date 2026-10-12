//! Shared wire scalars: references, ids, the epoch and the effect state.
//!
//! Clause: HP WF-3, WF-4, WF-6, VC-8, CA-5.

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::borrow::Cow;

/// The largest integer that travels as a JSON number (HP WF-3).
pub const MAX_WIRE_INT: u64 = 1 << 53;

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A session reference (HP WF-6, VC-8).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SessionRef {
    pub hub_id: String,
    pub session_id: String,
}

/// A reference to any other resource (HP WF-6, VC-8).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ResourceRef {
    pub hub_id: String,
    pub kind: String,
    pub id: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An opaque id as it crosses the wire: a JSON integer in range, or a decimal string (HP WF-3, section 3 `RequestId`, `CallId`).
/// The form is kept, so an id echoes back exactly as it was sent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WireId {
    Num(#[cfg_attr(test, strategy(crate::props::wire_int()))] u64),
    Str(String),
}

/// The connection epoch (HP section 2, MD-4, CR-5). It is a u64 that travels as a number up to 2^53 and as a decimal string above (HP WF-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u64);

impl Serialize for Epoch {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0 <= MAX_WIRE_INT {
            s.serialize_u64(self.0)
        } else {
            s.serialize_str(&self.0.to_string())
        }
    }
}

impl<'de> Deserialize<'de> for Epoch {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            N(u64),
            S(String),
        }
        match Raw::deserialize(d)? {
            Raw::N(n) if n <= MAX_WIRE_INT => Ok(Epoch(n)),
            Raw::N(_) => Err(serde::de::Error::custom(
                "integer above 2^53 must be a decimal string (WF-3)",
            )),
            Raw::S(s) => {
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(serde::de::Error::custom("epoch string must be decimal"));
                }
                s.parse().map(Epoch).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl JsonSchema for Epoch {
    fn schema_name() -> Cow<'static, str> {
        "Epoch".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "HP WF-3, MD-4: a u64; a JSON integer up to 2^53, a decimal string above.",
            "oneOf": [
                {"type": "integer", "minimum": 0, "maximum": 9007199254740992u64},
                {"type": "string", "pattern": "^[0-9]+$"}
            ]
        })
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Whether a mutating call had an effect (HP CA-5, ER-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    None,
    Committed,
    Unknown,
}

/// A free JSON object whose keys the contract leaves open (HP section 18, additive rule).
pub type OpenObject = Map<String, Value>;

/// Reads a field that may be absent, `null` or a value: `None` is absent, `Some(None)` is `null` (HP WF-4).
pub fn present_or_null<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Reads an optional field whose contract text does not allow `null`: an absent field is `None`, an explicit `null` is refused (HP WF-4, ST-2).
pub fn opt_no_null<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}

/// An unsigned counter or id that travels as a JSON number up to 2^53 and as a decimal string above (HP WF-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WireU64(pub u64);

impl Serialize for WireU64 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0 <= MAX_WIRE_INT {
            s.serialize_u64(self.0)
        } else {
            s.serialize_str(&self.0.to_string())
        }
    }
}

impl<'de> Deserialize<'de> for WireU64 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Epoch::deserialize(d).map(|e| WireU64(e.0))
    }
}

impl JsonSchema for WireU64 {
    fn schema_name() -> Cow<'static, str> {
        "WireU64".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "HP WF-3: a u64; a JSON integer up to 2^53, a decimal string above.",
            "oneOf": [
                {"type": "integer", "minimum": 0, "maximum": 9007199254740992u64},
                {"type": "string", "pattern": "^[0-9]+$"}
            ]
        })
    }
}

/// The JSON value `true` and nothing else (HP WP-1 `{served: true}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstTrue;

impl Serialize for ConstTrue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for ConstTrue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? {
            Ok(ConstTrue)
        } else {
            Err(serde::de::Error::custom("the value must be true"))
        }
    }
}

impl JsonSchema for ConstTrue {
    fn schema_name() -> Cow<'static, str> {
        "ConstTrue".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"const": true})
    }
}

/// A string-valued marker: one value, one type (used for `kind: "spawn"` and similar fixed words).
macro_rules! const_str {
    ($(#[$m:meta])* $name:ident, $value:literal) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        pub enum $name {
            #[serde(rename = $value)]
            Only,
        }
    };
}

pub(crate) use const_str;

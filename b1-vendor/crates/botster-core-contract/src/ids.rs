//! The identifiers of Core section 3 and the opaque tokens of the reads.

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;

/// The id of a session, chosen by the host (at most `max_session_id_bytes` bytes).
///
/// Clause: Core 3, Core ID-1.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct SessionId(pub String);

/// The id of one incarnation of a session.
///
/// Clause: Core ID-1, Core EV-9.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct InstanceId(pub String);

/// The host's label of a client. Core never interprets it.
///
/// Clause: Core 3, Core P3, Core OU-1.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ClientId(pub String);

/// The id of a route.
///
/// Clause: Core 3, Core ID-1, Core ID-2.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RouteId(pub u64);

/// The id of an operation.
///
/// Clause: Core 3, Core ID-1, Core ID-2.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct OpId(pub u64);

/// The id of a capture, minted by Core.
///
/// Clause: Core 3, Core ST-6, Core ID-1.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CaptureId(pub u64);

/// The id of a service: 32 bytes. In JSON it is 64 lowercase hex digits.
///
/// Clause: Core 3, Core SV-1, Core A2-1.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceId(pub [u8; 32]);

impl fmt::Debug for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceId({self})")
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl Serialize for ServiceId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ServiceId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let digits = text.as_bytes();
        if digits.len() != 64 {
            return Err(de::Error::custom("a service id is 64 hex digits"));
        }
        let mut out = [0u8; 32];
        for (slot, pair) in out.iter_mut().zip(digits.chunks(2)) {
            let hi = (pair[0] as char).to_digit(16);
            let lo = (pair[1] as char).to_digit(16);
            match (hi, lo) {
                (Some(h), Some(l)) => *slot = (h * 16 + l) as u8,
                _ => return Err(de::Error::custom("a service id is 64 hex digits")),
            }
        }
        Ok(ServiceId(out))
    }
}

impl JsonSchema for ServiceId {
    fn schema_name() -> Cow<'static, str> {
        "ServiceId".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": "^[0-9a-f]{64}$" })
    }
}

macro_rules! opaque_token {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    )*};
}

opaque_token! {
    /// An opaque token: equality only.
    ///
    /// Clause: Core ST-1, Core IN-10.
    ModelRev,
    /// An opaque token: equality only.
    ///
    /// Clause: Core IN-10, Core ST-4.
    InputRev,
    /// An opaque cursor: equality and `after` only.
    ///
    /// Clause: Core ST-7.
    InputCursor,
}

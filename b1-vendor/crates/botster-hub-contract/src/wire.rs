//! Shared wire helpers: absent-versus-null readers (HC A.1), the open-enum reader (HC EN-6, A.2), the decimal-string id (HC A.1, EN-1).

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::borrow::Cow;

/// A JSON object whose members this contract does not type (an opaque value, a plugin-owned map).
pub type OpenObject = Map<String, Value>;

/// Unix milliseconds. HC fixes no encoding for `at`, `since`, `created_at` and the like except `expires_at` (Unix seconds, PR-7b).
/// This type is the one choice for every other timestamp; steward ruling H2 may change it.
///
/// Clause: HC EV-1, A.8.
// pending steward ruling H2: the unit of a timestamp.
pub type Timestamp = u64;

/// Reads an optional field whose schema does not allow `null`: absent is `None`, and an explicit `null` is an error.
///
/// Clause: HC A.1.
pub fn opt_no_null<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}

/// Reads a merge-patch member: absent is `None`, `null` is `Some(None)` (delete), a value is `Some(Some(v))`.
///
/// Clause: HC A.1, VB-4.
pub fn present_or_null<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Schema of a merge-patch string member: a string or `null` (the marker keeps the `null`, see `schema`).
pub fn nullable_string(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": ["string", "null"], "x-null-allowed": true })
}

/// A u64 that travels as a decimal string.
///
/// Clause: HC A.1, EN-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(pub u64);

impl Serialize for RequestId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let ok = !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s == "0" || !s.starts_with('0'));
        match (ok, s.parse::<u64>()) {
            (true, Ok(n)) => Ok(RequestId(n)),
            _ => Err(serde::de::Error::custom("an id is a decimal u64 string")),
        }
    }
}

/// A regular expression for the decimal strings of 0 to `u64::MAX` with no leading zero: a number of 1 to 19 digits, or a 20-digit number up to the maximum.
///
/// Clause: HC A.1, EN-1.
pub fn u64_decimal_pattern() -> String {
    let max = u64::MAX.to_string();
    let digits: Vec<u32> = max.chars().filter_map(|c| c.to_digit(10)).collect();
    let mut alternatives = vec!["0".to_string(), "[1-9][0-9]{0,18}".to_string()];
    // A 20-digit number is below the maximum at the first digit that is smaller, or equal to it.
    for (i, d) in digits.iter().enumerate().skip(1) {
        if *d == 0 {
            continue;
        }
        let prefix: String = digits[..i].iter().map(|x| x.to_string()).collect();
        alternatives.push(format!(
            "{prefix}[0-{}][0-9]{{{}}}",
            d - 1,
            digits.len() - i - 1
        ));
    }
    alternatives.push(max);
    format!("^({})$", alternatives.join("|"))
}

impl JsonSchema for RequestId {
    fn schema_name() -> Cow<'static, str> {
        "RequestId".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": u64_decimal_pattern() })
    }
}

/// Reads a non-exhaustive object-or-string set (the data-carrying enums): a string or one-key object that this version does not know is `Other`.
/// A known name with bad data, a value of another shape, and an object with several keys stay errors.
///
/// Clause: HC EN-6, A.1b, A.2.
macro_rules! lenient {
    ($($t:ty => [$($name:literal),* $(,)?]),* $(,)?) => {$(
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                <$t>::serialize(self, s)
            }
        }
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
                match <$t>::deserialize(&value) {
                    Ok(known) => Ok(known),
                    Err(error) => match &value {
                        serde_json::Value::String(s) if ![$($name),*].contains(&s.as_str()) => Ok(<$t>::Other),
                        serde_json::Value::Object(map)
                            if map.len() == 1
                                && map.values().all(serde_json::Value::is_object)
                                && !map.keys().any(|k| [$($name),*].contains(&k.as_str())) =>
                        {
                            Ok(<$t>::Other)
                        }
                        _ => Err(serde::de::Error::custom(error)),
                    },
                }
            }
        }
        impl $t {
            /// The wire names that this version knows, in declaration order. The drift tests and the TypeScript generator read this one list.
            pub const NAMES: &'static [&'static str] = &[$($name),*];
        }
    )*};
}
pub(crate) use lenient;

/// The schema of a non-exhaustive string set: the known names, or any other string (HC EN-6).
pub fn open_string_schema(names: &[&str]) -> Schema {
    json_schema!({
        "anyOf": [{ "type": "string", "enum": names }, { "type": "string" }]
    })
}

/// A set of unit variants that is non-exhaustive on the wire: one declaration gives the enum, its inventory, serde and the schema.
/// An unknown string reads as `Other` (HC EN-6, A.2).
///
/// Clause: HC EN-6, A.2.
macro_rules! open_enum {
    ($(#[$m:meta])* $v:vis enum $name:ident { $( $(#[$vm:meta])* $var:ident = $wire:literal ),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $v enum $name {
            $( $(#[$vm])* $var, )+
            /// A value that this version does not know (EN-6).
            Other,
        }
        impl $name {
            /// Every known variant, in declaration order. One inventory for the schema, the TypeScript union and the drift tests.
            pub const ALL: &'static [$name] = &[ $( $name::$var ),+ ];
            /// The wire name of each known variant, in the order of `ALL`.
            pub const NAMES: &'static [&'static str] = &[ $( $wire ),+ ];
            /// The wire name. `Other` has no name of its own and writes `"other"`.
            pub fn as_str(&self) -> &'static str {
                match self { $( $name::$var => $wire, )+ $name::Other => "other" }
            }
        }
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <String as serde::Deserialize>::deserialize(d)?;
                Ok(match s.as_str() { $( $wire => $name::$var, )+ _ => $name::Other })
            }
        }
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> { stringify!($name).into() }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                $crate::wire::open_string_schema(Self::NAMES)
            }
        }
    };
}
pub(crate) use open_enum;

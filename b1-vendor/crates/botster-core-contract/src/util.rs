//! Serde helpers shared by the contract types: durations as milliseconds, and the readers of the non-exhaustive sets.

/// A `Duration` that travels as an integer of milliseconds.
///
/// Clause: Core 9B, Core TM-1.
pub(crate) mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(d)?))
    }
}

/// An optional `Duration` that travels as an integer of milliseconds.
///
/// Clause: Core OU-1, Core EV-8.
pub(crate) mod opt_duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => s.serialize_some(&u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<u64>::deserialize(d)?.map(Duration::from_millis))
    }
}

/// Reader of an open set: an unknown name, or an unknown one-key object whose value is an object, reads as `Other`. Anything else stays an error.
///
/// Clause: Core 3 (enums are non-exhaustive), Core AD-2, Core A2-1, Core A2-3, Core A2-5.
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
    )*};
}
pub(crate) use lenient;

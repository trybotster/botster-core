//! Forwardable terminal request types.
//!
//! These types are Hub-safe. They carry route identity and input bytes only.

use serde::{Deserialize, Serialize};

/// Attach a subscription to a session.
///
/// Wire tag is `attach`. There is no delivery-mode field. READY-then-history
/// is selected through compatibility, not this request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attach {
    /// Session to attach.
    pub session_id: String,
    /// Subscription that will receive terminal frames.
    pub subscription_id: String,
}

/// Detach a subscription from a session.
///
/// Wire tag is `detach`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detach {
    /// Session to detach.
    pub session_id: String,
    /// Subscription to detach.
    pub subscription_id: String,
}

/// Send terminal input bytes.
///
/// Wire tag stays `send_input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendInput {
    /// Session that receives the input.
    pub session_id: String,
    /// Input payload. Encoding is owned by the caller.
    pub data: String,
}

/// Resize a session PTY.
///
/// Wire tag is `resize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resize {
    /// Session to resize.
    pub session_id: String,
    /// Row count.
    pub rows: u16,
    /// Column count.
    pub cols: u16,
}

#[derive(Serialize)]
struct AttachWire<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    subscription_id: &'a str,
}

#[derive(Deserialize)]
struct AttachWireOwned {
    #[serde(rename = "type")]
    kind: String,
    session_id: String,
    subscription_id: String,
}

impl Serialize for Attach {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AttachWire {
            kind: "attach",
            session_id: &self.session_id,
            subscription_id: &self.subscription_id,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Attach {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AttachWireOwned::deserialize(deserializer)?;
        expect_type::<D>(&wire.kind, "attach")?;
        Ok(Self {
            session_id: wire.session_id,
            subscription_id: wire.subscription_id,
        })
    }
}

#[derive(Serialize)]
struct DetachWire<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    subscription_id: &'a str,
}

#[derive(Deserialize)]
struct DetachWireOwned {
    #[serde(rename = "type")]
    kind: String,
    session_id: String,
    subscription_id: String,
}

impl Serialize for Detach {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DetachWire {
            kind: "detach",
            session_id: &self.session_id,
            subscription_id: &self.subscription_id,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Detach {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = DetachWireOwned::deserialize(deserializer)?;
        expect_type::<D>(&wire.kind, "detach")?;
        Ok(Self {
            session_id: wire.session_id,
            subscription_id: wire.subscription_id,
        })
    }
}

#[derive(Serialize)]
struct SendInputWire<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    data: &'a str,
}

#[derive(Deserialize)]
struct SendInputWireOwned {
    #[serde(rename = "type")]
    kind: String,
    session_id: String,
    data: String,
}

impl Serialize for SendInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SendInputWire {
            kind: "send_input",
            session_id: &self.session_id,
            data: &self.data,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SendInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SendInputWireOwned::deserialize(deserializer)?;
        expect_type::<D>(&wire.kind, "send_input")?;
        Ok(Self {
            session_id: wire.session_id,
            data: wire.data,
        })
    }
}

#[derive(Serialize)]
struct ResizeWire<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    rows: u16,
    cols: u16,
}

#[derive(Deserialize)]
struct ResizeWireOwned {
    #[serde(rename = "type")]
    kind: String,
    session_id: String,
    rows: u16,
    cols: u16,
}

impl Serialize for Resize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ResizeWire {
            kind: "resize",
            session_id: &self.session_id,
            rows: self.rows,
            cols: self.cols,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Resize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ResizeWireOwned::deserialize(deserializer)?;
        expect_type::<D>(&wire.kind, "resize")?;
        Ok(Self {
            session_id: wire.session_id,
            rows: wire.rows,
            cols: wire.cols,
        })
    }
}

fn expect_type<'de, D: serde::Deserializer<'de>>(
    found: &str,
    expected: &str,
) -> Result<(), D::Error> {
    if found == expected {
        Ok(())
    } else {
        Err(serde::de::Error::custom(format!(
            "expected type {expected}, got {found}"
        )))
    }
}

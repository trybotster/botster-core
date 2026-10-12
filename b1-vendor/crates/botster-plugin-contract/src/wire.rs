//! Framing: the lane byte, the type byte, the length field, the encoder and the decoder with typed errors.
//!
//! Clause: HP section 2, MD-1, MD-2, WF-1 to WF-6, VR-2, steward ruling R-4.
//!
//! Wire form: `len` (little-endian u32, the count of the bytes after it) then the lane byte, the type byte and one UTF-8 JSON object (WF-1, WF-2).
//!
//! # Type bytes (steward ruling R-4)
//!
//! The crate pins the type bytes and the checked-in `plugin-frames.schema.json` is the agreement. Four constraints hold, and each has a test:
//! 1. state-changing types are 0x00 to 0x7F, advisory types are 0x80 to 0xFF (section 2). All ten frames of section 2 are state-changing (R-4 P1a);
//! 2. a type byte is unique across all lanes, because `Fault{unsupported_frame, type}` names a frame by type alone;
//! 3. the lane bytes are control 0, invoke 1, call 2, bulk 3 (section 2);
//! 4. a published byte never changes within a `botster_plugin_api` version (A2-43, VR-1).

use crate::common::{Epoch, MAX_WIRE_INT};
use crate::frames::*;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;
use std::fmt;

/// A lane (HP section 2): `control` 0, `invoke` 1, `call` 2, `bulk` 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Control,
    Invoke,
    Call,
    Bulk,
}

impl Lane {
    pub const ALL: [Lane; 4] = [Lane::Control, Lane::Invoke, Lane::Call, Lane::Bulk];

    pub fn byte(self) -> u8 {
        match self {
            Lane::Control => 0,
            Lane::Invoke => 1,
            Lane::Call => 2,
            Lane::Bulk => 3,
        }
    }

    pub fn from_byte(b: u8) -> Option<Lane> {
        Lane::ALL.into_iter().find(|l| l.byte() == b)
    }
}

/// The class of a frame type (HP section 2, VR-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameClass {
    /// 0x00 to 0x7F: an unknown one is answered `Fault{unsupported_frame, type}`.
    StateChanging,
    /// 0x80 to 0xFF: an unknown one is ignored and counted.
    Advisory,
}

impl FrameClass {
    pub fn of_byte(b: u8) -> FrameClass {
        if b < 0x80 {
            FrameClass::StateChanging
        } else {
            FrameClass::Advisory
        }
    }
}

/// The ten frame types of HP section 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FrameType {
    Hello,
    Ready,
    Activated,
    Shutdown,
    Fault,
    Cancel,
    Invoke,
    Result,
    HostCall,
    HostResult,
}

/// Which side sends a frame (HP section 2 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    HubToPlugin,
    PluginToHub,
}

/// One row of the frame table: the type, its byte, its name and its lanes (steward ruling R-4).
pub struct FrameSpec {
    pub ty: FrameType,
    pub byte: u8,
    pub name: &'static str,
    /// The lanes the frame may travel on (HP section 2: HostCall and HostResult use lane 2, or lane 3 for a bulk-class call).
    pub lanes: &'static [Lane],
    pub direction: Direction,
    pub clauses: &'static str,
}

/// The frame table (steward ruling R-4: the crate pins these bytes).
pub const FRAMES: &[FrameSpec] = &[
    FrameSpec {
        ty: FrameType::Hello,
        byte: 0x01,
        name: "Hello",
        lanes: &[Lane::Control],
        direction: Direction::HubToPlugin,
        clauses: "HS-1",
    },
    FrameSpec {
        ty: FrameType::Ready,
        byte: 0x02,
        name: "Ready",
        lanes: &[Lane::Control],
        direction: Direction::PluginToHub,
        clauses: "HS-2",
    },
    FrameSpec {
        ty: FrameType::Activated,
        byte: 0x03,
        name: "Activated",
        lanes: &[Lane::Control],
        direction: Direction::HubToPlugin,
        clauses: "HS-2",
    },
    FrameSpec {
        ty: FrameType::Shutdown,
        byte: 0x04,
        name: "Shutdown",
        lanes: &[Lane::Control],
        direction: Direction::HubToPlugin,
        clauses: "LD-4, HS-2",
    },
    FrameSpec {
        ty: FrameType::Fault,
        byte: 0x05,
        name: "Fault",
        lanes: &[Lane::Control],
        direction: Direction::PluginToHub,
        clauses: "CR-2, VR-2",
    },
    FrameSpec {
        ty: FrameType::Cancel,
        byte: 0x06,
        name: "Cancel",
        lanes: &[Lane::Control],
        direction: Direction::HubToPlugin,
        clauses: "IV-5",
    },
    FrameSpec {
        ty: FrameType::Invoke,
        byte: 0x10,
        name: "Invoke",
        lanes: &[Lane::Invoke],
        direction: Direction::HubToPlugin,
        clauses: "IV-1",
    },
    FrameSpec {
        ty: FrameType::Result,
        byte: 0x11,
        name: "Result",
        lanes: &[Lane::Invoke],
        direction: Direction::PluginToHub,
        clauses: "IV-2",
    },
    FrameSpec {
        ty: FrameType::HostCall,
        byte: 0x20,
        name: "HostCall",
        lanes: &[Lane::Call, Lane::Bulk],
        direction: Direction::PluginToHub,
        clauses: "CA-1",
    },
    FrameSpec {
        ty: FrameType::HostResult,
        byte: 0x21,
        name: "HostResult",
        lanes: &[Lane::Call, Lane::Bulk],
        direction: Direction::HubToPlugin,
        clauses: "CA-2, CA-5",
    },
];

/// The call that travels on the bulk lane (HP section 2, CA-9). It has no frame type of its own (R-4 P1b).
pub const BULK_CALL: &str = "log.write";

impl FrameType {
    pub fn spec(self) -> &'static FrameSpec {
        FRAMES
            .iter()
            .find(|f| f.ty == self)
            .expect("every type has a row")
    }
    pub fn byte(self) -> u8 {
        self.spec().byte
    }
    pub fn class(self) -> FrameClass {
        FrameClass::of_byte(self.byte())
    }
    pub fn from_byte(b: u8) -> Option<FrameType> {
        FRAMES.iter().find(|f| f.byte == b).map(|f| f.ty)
    }
}

/// A decoded frame. The variants differ in size because `Hello` and `Ready` are large; the frame is a plain data type, so it is not boxed.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Hello(Hello),
    Ready(Ready),
    Activated(Activated),
    Shutdown(Shutdown),
    Fault(Fault),
    Cancel(Cancel),
    Invoke(Invoke),
    Result(ResultFrame),
    HostCall(HostCall),
    HostResult(HostResult),
}

impl Frame {
    pub fn frame_type(&self) -> FrameType {
        match self {
            Frame::Hello(_) => FrameType::Hello,
            Frame::Ready(_) => FrameType::Ready,
            Frame::Activated(_) => FrameType::Activated,
            Frame::Shutdown(_) => FrameType::Shutdown,
            Frame::Fault(_) => FrameType::Fault,
            Frame::Cancel(_) => FrameType::Cancel,
            Frame::Invoke(_) => FrameType::Invoke,
            Frame::Result(_) => FrameType::Result,
            Frame::HostCall(_) => FrameType::HostCall,
            Frame::HostResult(_) => FrameType::HostResult,
        }
    }

    /// The connection epoch the frame carries (HP section 2, MD-4).
    pub fn epoch(&self) -> Epoch {
        match self {
            Frame::Hello(f) => f.epoch,
            Frame::Ready(f) => f.epoch,
            Frame::Activated(f) => f.epoch,
            Frame::Shutdown(f) => f.epoch,
            Frame::Fault(f) => f.epoch,
            Frame::Cancel(f) => f.epoch,
            Frame::Invoke(f) => f.epoch,
            Frame::Result(f) => f.epoch,
            Frame::HostCall(f) => f.epoch,
            Frame::HostResult(f) => f.epoch,
        }
    }

    /// The lane a sender uses, when the frame alone decides it. A `HostCall` of `log.write` is bulk (HP section 2, CA-9). A `HostResult` answers on the lane of its `HostCall`, so this gives `Call`; pass the call's lane to `encode` for a bulk answer (R-4 P1b).
    pub fn default_lane(&self) -> Lane {
        match self {
            Frame::HostCall(c) if c.call == BULK_CALL => Lane::Bulk,
            f => f.frame_type().spec().lanes[0],
        }
    }

    pub(crate) fn payload_value(&self) -> Value {
        macro_rules! v {
            ($f:expr) => {
                serde_json::to_value($f).expect("a frame serializes")
            };
        }
        match self {
            Frame::Hello(f) => v!(f),
            Frame::Ready(f) => v!(f),
            Frame::Activated(f) => v!(f),
            Frame::Shutdown(f) => v!(f),
            Frame::Fault(f) => v!(f),
            Frame::Cancel(f) => v!(f),
            Frame::Invoke(f) => v!(f),
            Frame::Result(f) => v!(f),
            Frame::HostCall(f) => v!(f),
            Frame::HostResult(f) => v!(f),
        }
    }
}

/// An encode refusal (HP MD-2, WF-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// `len` would exceed `frame_bytes`: the sender refuses with `frame_too_large` and truncates nothing (MD-2).
    FrameTooLarge { len: u64, limit: u32 },
    /// The frame is not allowed on that lane (section 2).
    WrongLane { frame: &'static str, lane: Lane },
    /// An integer outside ±2^53 that is not a string (WF-3).
    IntegerOutOfRange { pointer: String },
}

/// Encodes one frame on a lane. `frame_bytes` is `limits.frame_bytes`; `len` counts the lane byte, the type byte and the payload.
pub fn encode(lane: Lane, frame: &Frame, frame_bytes: u32) -> Result<Vec<u8>, EncodeError> {
    let spec = frame.frame_type().spec();
    if !spec.lanes.contains(&lane) {
        return Err(EncodeError::WrongLane {
            frame: spec.name,
            lane,
        });
    }
    if let Frame::HostCall(c) = frame {
        let want = if c.call == BULK_CALL {
            Lane::Bulk
        } else {
            Lane::Call
        };
        if lane != want {
            return Err(EncodeError::WrongLane {
                frame: spec.name,
                lane,
            });
        }
    }
    let payload = frame.payload_value();
    if let Some(pointer) = find_out_of_range(&payload, "") {
        return Err(EncodeError::IntegerOutOfRange { pointer });
    }
    let body = serde_json::to_vec(&payload).expect("a value serializes");
    encode_raw(lane.byte(), spec.byte, &body, frame_bytes)
}

/// Encodes raw parts into the wire form. The fakes use it to build frames that the typed encoder refuses.
pub fn encode_raw(
    lane: u8,
    ty: u8,
    payload: &[u8],
    frame_bytes: u32,
) -> Result<Vec<u8>, EncodeError> {
    let len = payload.len() as u64 + 2;
    if len > u64::from(frame_bytes) {
        return Err(EncodeError::FrameTooLarge {
            len,
            limit: frame_bytes,
        });
    }
    let mut out = Vec::with_capacity(len as usize + 4);
    out.extend_from_slice(&(len as u32).to_le_bytes());
    out.push(lane);
    out.push(ty);
    out.extend_from_slice(payload);
    Ok(out)
}

/// What a decode refusal means for the receiver (HP WF-2, R-4 P1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The epoch ends (HP WF-1, WF-2, SV-6).
    EndsEpoch,
    /// The frame is refused as a malformed request: `invalid_request` (HP WF-2 duplicate keys, WF-3, WF-4, R-4 P1b wrong lane). The epoch continues.
    InvalidRequest,
}

/// One refusal per violation, so a transcript can assert each one (HP WF-1 to WF-4, VR-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// `len` is over `frame_bytes`; refused before any allocation (WF-1).
    LenOverLimit { len: u32, limit: u32 },
    /// `len` is below 2: no room for the lane byte and the type byte (WF-1).
    LenTooShort { len: u32 },
    /// The lane byte is not 0 to 3 (section 2).
    UnknownLane { lane: u8 },
    /// The payload is not valid UTF-8 (WF-2).
    InvalidUtf8,
    /// The payload is not valid JSON (WF-2: the payload is one JSON object).
    InvalidJson,
    /// The payload is JSON but not an object (WF-2).
    NotAnObject,
    /// Bytes follow the JSON object inside the payload (WF-2).
    TrailingBytes,
    /// An object holds the same key twice (WF-2).
    DuplicateKey { key: String },
    /// An integer outside ±2^53 travels as a number (WF-3).
    IntegerOutOfRange { pointer: String },
    /// A known type on a lane it may not use (section 2, R-4 P1b).
    WrongLane { ty: u8, lane: u8 },
    /// The payload has no `epoch`, or the epoch is not an integer or a decimal string (HP section 2: every frame carries it).
    EpochMissing { ty: u8 },
    /// The payload does not fit the schema of its frame: a missing field, a wrong type or a forbidden `null` (WF-3, WF-4).
    Schema { ty: u8, message: String },
}

impl WireError {
    pub fn disposition(&self) -> Disposition {
        match self {
            WireError::LenOverLimit { .. }
            | WireError::LenTooShort { .. }
            | WireError::UnknownLane { .. }
            | WireError::InvalidUtf8
            | WireError::InvalidJson
            | WireError::NotAnObject
            | WireError::TrailingBytes => Disposition::EndsEpoch,
            WireError::DuplicateKey { .. }
            | WireError::IntegerOutOfRange { .. }
            | WireError::WrongLane { .. }
            | WireError::EpochMissing { .. }
            | WireError::Schema { .. } => Disposition::InvalidRequest,
        }
    }
}

/// The result of reading one frame from a byte buffer (steward ruling R-6.2).
///
/// The decoder applies the order of R-6.2 to every frame: (1) the WF-1 length, (2) the WF-2 payload (one UTF-8 JSON object, else the epoch ends, whatever the type),
/// (3) the epoch (a stale frame is dropped and counted, whatever the type, and a stale unknown state-changing frame gets no `Fault`), (4) the type.
/// The receiver's actions (end the epoch, count, `Fault`) belong to the receiver; each step has its own outcome so it can follow the order.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// The buffer holds no complete frame yet; `needed` more bytes make progress.
    Incomplete { needed: usize },
    /// A known frame with the current epoch (or any epoch when the connection has none yet), and the bytes it used.
    Frame {
        frame: Frame,
        lane: Lane,
        used: usize,
    },
    /// The frame carries an epoch other than the connection's: drop and count it (HP MD-4, CR-5). It never reaches a handler, whatever its type.
    Stale { ty: u8, epoch: Epoch, used: usize },
    /// An unknown advisory type (0x80 to 0xFF) with the current epoch: ignore and count it (HP VR-2).
    IgnoredAdvisory { ty: u8, used: usize },
    /// An unknown state-changing type (0x00 to 0x7F) with the current epoch: answer `Fault{unsupported_frame, type}` (HP section 2, VR-2).
    UnknownStateChanging { ty: u8, used: usize },
}

/// Decodes the first frame of `buf` in the order of R-6.2. `frame_bytes` is `limits.frame_bytes` (WF-1).
/// `current` is the connection's epoch; `None` means the connection has none yet (the host before `Hello`), and then no frame is stale.
pub fn decode(buf: &[u8], frame_bytes: u32, current: Option<Epoch>) -> Result<Decoded, WireError> {
    if buf.len() < 4 {
        return Ok(Decoded::Incomplete {
            needed: 4 - buf.len(),
        });
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    // R-6.2 (1), WF-1: check len before any allocation or read of the body.
    if len > frame_bytes {
        return Err(WireError::LenOverLimit {
            len,
            limit: frame_bytes,
        });
    }
    if len < 2 {
        return Err(WireError::LenTooShort { len });
    }
    let total = 4 + len as usize;
    if buf.len() < total {
        return Ok(Decoded::Incomplete {
            needed: total - buf.len(),
        });
    }
    let (lane_b, ty) = (buf[4], buf[5]);
    let lane = Lane::from_byte(lane_b).ok_or(WireError::UnknownLane { lane: lane_b })?;
    // R-6.2 (2), WF-2: the payload is checked whatever the type.
    let value = parse_payload(&buf[6..total])?;
    // R-6.2 (3): the epoch, whatever the type. An epoch that is a number above 2^53 breaks WF-3.
    if let Some(e) = value.get("epoch") {
        if let Some(p) = find_out_of_range(e, "/epoch") {
            return Err(WireError::IntegerOutOfRange { pointer: p });
        }
    }
    let epoch = value
        .get("epoch")
        .and_then(|e| serde_json::from_value::<Epoch>(e.clone()).ok())
        .ok_or(WireError::EpochMissing { ty })?;
    if current.is_some_and(|c| c != epoch) {
        return Ok(Decoded::Stale {
            ty,
            epoch,
            used: total,
        });
    }
    // R-6.2 (4): the type.
    let Some(known) = FrameType::from_byte(ty) else {
        return Ok(match FrameClass::of_byte(ty) {
            FrameClass::StateChanging => Decoded::UnknownStateChanging { ty, used: total },
            FrameClass::Advisory => Decoded::IgnoredAdvisory { ty, used: total },
        });
    };
    if !known.spec().lanes.contains(&lane) {
        return Err(WireError::WrongLane { ty, lane: lane_b });
    }
    if let Some(pointer) = find_out_of_range(&value, "") {
        return Err(WireError::IntegerOutOfRange { pointer });
    }
    let schema = |e: serde_json::Error| WireError::Schema {
        ty,
        message: e.to_string(),
    };
    let frame = match known {
        FrameType::Hello => Frame::Hello(serde_json::from_value(value).map_err(schema)?),
        FrameType::Ready => Frame::Ready(serde_json::from_value(value).map_err(schema)?),
        FrameType::Activated => Frame::Activated(serde_json::from_value(value).map_err(schema)?),
        FrameType::Shutdown => Frame::Shutdown(serde_json::from_value(value).map_err(schema)?),
        FrameType::Fault => Frame::Fault(serde_json::from_value(value).map_err(schema)?),
        FrameType::Cancel => Frame::Cancel(serde_json::from_value(value).map_err(schema)?),
        FrameType::Invoke => Frame::Invoke(serde_json::from_value(value).map_err(schema)?),
        FrameType::Result => Frame::Result(serde_json::from_value(value).map_err(schema)?),
        FrameType::HostCall => Frame::HostCall(serde_json::from_value(value).map_err(schema)?),
        FrameType::HostResult => Frame::HostResult(serde_json::from_value(value).map_err(schema)?),
    };
    if let Frame::HostCall(c) = &frame {
        // R-4 P1b: a bulk-class call uses lane 3 and every other call lane 2; the wrong lane is a malformed request.
        let want = if c.call == BULK_CALL {
            Lane::Bulk
        } else {
            Lane::Call
        };
        if lane != want {
            return Err(WireError::WrongLane { ty, lane: lane_b });
        }
    }
    Ok(Decoded::Frame {
        frame,
        lane,
        used: total,
    })
}

/// Parses a payload: UTF-8, one JSON object, no trailing bytes, no duplicate keys (HP WF-2).
fn parse_payload(payload: &[u8]) -> Result<Value, WireError> {
    let text = std::str::from_utf8(payload).map_err(|_| WireError::InvalidUtf8)?;
    let mut de = serde_json::Deserializer::from_str(text);
    let value = match Strict.deserialize(&mut de) {
        Ok(v) => v,
        Err(e) => {
            return Err(match duplicate_key_in(&e.to_string()) {
                Some(key) => WireError::DuplicateKey { key },
                None => WireError::InvalidJson,
            })
        }
    };
    if de.end().is_err() {
        return Err(WireError::TrailingBytes);
    }
    if !value.is_object() {
        return Err(WireError::NotAnObject);
    }
    Ok(value)
}

const DUP_MARK: &str = "duplicate key \u{1}";

fn duplicate_key_in(msg: &str) -> Option<String> {
    let start = msg.find(DUP_MARK)? + DUP_MARK.len();
    let rest = &msg[start..];
    Some(rest.split('\u{1}').next()?.to_string())
}

/// Deserializes any JSON value and refuses a repeated object key (HP WF-2).
struct Strict;

impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        d.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Value, E> {
        Ok(Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null))
    }
    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element_seed(Strict)? {
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut seen = BTreeSet::new();
        let mut out = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!("{DUP_MARK}{key}\u{1}")));
            }
            let v = map.next_value_seed(Strict)?;
            out.insert(key, v);
        }
        Ok(Value::Object(out))
    }
}

/// The JSON pointer of the first integer outside ±2^53 (HP WF-3), if any.
fn find_out_of_range(v: &Value, at: &str) -> Option<String> {
    match v {
        Value::Number(n) => {
            let bad = if let Some(u) = n.as_u64() {
                u > MAX_WIRE_INT
            } else if let Some(i) = n.as_i64() {
                i.unsigned_abs() > MAX_WIRE_INT
            } else {
                n.as_f64()
                    .is_some_and(|f| f.fract() == 0.0 && f.abs() > MAX_WIRE_INT as f64)
            };
            bad.then(|| at.to_string())
        }
        Value::Array(a) => a
            .iter()
            .enumerate()
            .find_map(|(i, x)| find_out_of_range(x, &format!("{at}/{i}"))),
        Value::Object(m) => m
            .iter()
            .find_map(|(k, x)| find_out_of_range(x, &format!("{at}/{k}"))),
        _ => None,
    }
}

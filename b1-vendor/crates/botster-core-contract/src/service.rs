//! Services: limits, calls, states and lane reasons (section 10, A2-5).

use crate::types::LostReason;
use crate::util::{duration_ms, lenient};
use botster_route_codec::prelude::HexBytes;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The value of `CORE_SERVICE_TRANSPORT` today: one authenticated Unix connection per lane. A new value is a new transport.
///
/// Clause: Core SV-1, Core SV-2.
pub const CORE_SERVICE_TRANSPORT_UNIX_LANES_1: &str = "unix-lanes-1";

/// An OS bound of a service (SV-4). The names match the `bound:` features.
///
/// Clause: Core SV-4, Core A2-6.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum Bound {
    CpuSeconds = 1,
    OpenFiles = 2,
    Processes = 3,
    AddressSpaceBytes = 4,
}

/// The limits of one lane (SV-2).
///
/// Clause: Core SV-2, Core SV-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LaneSpec {
    pub max_frame_bytes: u32,
    pub outbound_queue_bytes: u64,
    pub inbound_queue_bytes: u64,
}

/// One requested rlimit.
///
/// Clause: Core A2-5, Core SV-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RlimitSpec {
    pub value: u64,
    pub required: bool,
}

/// The optional rlimits of a service.
///
/// Clause: Core SV-4, Core A2-5.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rlimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_seconds: Option<RlimitSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_files: Option<RlimitSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processes: Option<RlimitSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_space_bytes: Option<RlimitSpec>,
}

/// The limits of a service. Durations travel as milliseconds.
///
/// Clause: Core SV-2, Core SV-4, Core A2-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceLimits {
    pub lanes: Vec<LaneSpec>,
    #[serde(default)]
    pub rlimits: Rlimits,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub startup: Duration,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub stop_grace: Duration,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub orphan_grace: Duration,
    /// The uid that a lane peer must have. Absent means the host's uid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_identity: Option<u32>,
}

/// The completion value of `SpawnService`.
///
/// Clause: Core SV-4, Core A2-1.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnReport {
    pub applied: Vec<Bound>,
    pub unsupported: Vec<Bound>,
}

/// Why a service exited. A host maps an unknown cause to `Other`.
///
/// Clause: Core SV-5, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum ServiceExitCause {
    Normal = 1,
    Signal = 2,
    HostStop = 3,
    StartupTimeout = 4,
    OrphanGrace = 5,
    Killed = 6,
    /// Clause: Core SV-5.
    ChildReported(u8) = 7,
    #[serde(other)]
    Other = 255,
}

/// The `Exited` payload of a service.
///
/// Clause: Core SV-5, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceExit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
    pub cause: ServiceExitCause,
    pub descendants_may_remain: bool,
}

/// The completion of `StopService`: the recorded exit, or the `Lost` state's own payload.
///
/// Clause: Core A16-1, Core A16-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ServiceEnd {
    Exited(ServiceExit) = 1,
    Lost {
        reason: LostReason,
        payload_may_remain: bool,
    } = 2,
}

/// The state of a service. Every value is a mandatory event.
///
/// Clause: Core A2-5, Core SV-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ServiceState {
    Started = 1,
    Reconnected {
        epoch: u64,
    } = 2,
    Exited(ServiceExit) = 3,
    Lost {
        reason: LostReason,
        payload_may_remain: bool,
    } = 4,
    Released = 5,
}

/// Why a lane ended. A receiver treats an unknown value as opaque.
///
/// Clause: Core A2-5, Core SV-3, Core SV-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum LaneEndReason {
    BadLane = 1,
    Oversize = 2,
    Truncated = 3,
    PeerClosed = 4,
    EpochEnded = 5,
    Superseded = 6,
    ServiceExited = 7,
    ServiceRemoved = 8,
    #[serde(other)]
    Other = 255,
}

/// The error of `service_send`.
///
/// Clause: Core A2-5, Core SV-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum SendError {
    Backpressured = 1,
    UnknownService = 2,
    NoSuchLane = 3,
    NotConnected = 4,
    ServiceEnded = 5,
    Oversize = 6,
    #[serde(other)]
    Other = 255,
}

/// The error of `service_recv`.
///
/// Clause: Core A2-5, Core SV-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum RecvError {
    UnknownService = 1,
    NoSuchLane = 2,
}

/// A frame that arrived on a lane. It carries the epoch it arrived in.
///
/// Clause: Core SV-3, Core SV-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Frame {
    pub epoch: u64,
    #[serde(rename = "type")]
    pub frame_type: u8,
    pub payload: HexBytes,
}

/// A frame that the host sends on a lane. Core copies it.
///
/// Clause: Core SV-3, Core A2-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutboundFrame {
    #[serde(rename = "type")]
    pub frame_type: u8,
    pub payload: HexBytes,
}

lenient! {
    ServiceExitCause => ["Normal", "Signal", "HostStop", "StartupTimeout", "OrphanGrace", "Killed", "ChildReported"],
    LaneEndReason => ["BadLane", "Oversize", "Truncated", "PeerClosed", "EpochEnded", "Superseded", "ServiceExited", "ServiceRemoved"],
    SendError => ["Backpressured", "UnknownService", "NoSuchLane", "NotConnected", "ServiceEnded", "Oversize"],
}

//! The events of section 6.2 (with the payloads of A2-4 and the service events of A2-5).

use crate::ids::{ClientId, InstanceId, OpId, RouteId, ServiceId, SessionId};
use crate::op::OpResult;
use crate::route::RouteCloseReason;
use crate::service::{LaneEndReason, ServiceState};
use crate::types::{SessionState, Size, UnixSeconds};
use crate::util::lenient;
use botster_route_codec::prelude::{HexBytes, ModeFlags};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The OSC 133 mark of a `PromptMark`.
///
/// Clause: Core A2-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum PromptMarkKind {
    /// OSC 133 A.
    PromptStart = 1,
    /// OSC 133 B.
    CommandStart = 2,
    /// OSC 133 C.
    CommandExecuted = 3,
    /// OSC 133 D.
    CommandFinished = 4,
}

/// The source of a `Notification`.
///
/// Clause: Core A2-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum NotificationSource {
    Osc9 = 1,
    Osc777 = 2,
}

/// Why a clipboard write has no contents.
///
/// Clause: Core EV-3, Core A13-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ClipboardReason {
    TooLarge = 1,
}

/// One representation of a clipboard write's logical value, already decoded.
///
/// Clause: Core A13-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClipboardContent {
    pub mime: String,
    pub bytes: HexBytes,
}

/// A kind of dropped event in `EventsLost.kinds`. A receiver treats an unknown kind as opaque.
///
/// Clause: Core A2-4, Core EV-2, Core TP-1, Core EV-8.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum LostKind {
    Bell = 1,
    PromptMark = 2,
    Notification = 3,
    ClipboardWrite = 4,
    /// Clause: Core EV-8.
    Query = 5,
    Tap = 6,
    #[serde(other)]
    Other = 255,
}

/// What produced an `Activity`.
///
/// Clause: Core IN-4, Core 6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ActivitySource {
    Output = 1,
    Client(RouteId) = 2,
}

/// The tri-state of `FocusChanged`.
///
/// Clause: Core 6.2 (`FocusChanged`), Core DP-12, Core A3-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum FocusState {
    Focused = 1,
    Unfocused = 2,
    Unknown = 3,
}

/// An event of the one event queue. Every session-scoped event carries its `InstanceId` (EV-9).
///
/// Class M: `Completed`, `SessionState`, `RouteStalled`, `RouteResumed`, `RouteClosed`, `ServiceState`, `LaneConnected`, `LaneEnded`.
/// Class K: `ModesChanged`, `SizeChanged`, `FocusChanged`, `Activity`, `Silent`, `TitleChanged`, `CwdChanged`, `MetadataChanged`,
/// `SessionWritable`, `ServiceWritable`, `ServiceReadable`. Class D: `Bell`, `PromptMark`, `Notification`, `ClipboardWrite`. Class L: `EventsLost`.
/// `RouteAdopted` has no class in 6.2.
///
/// Clause: Core 6.2, Core EV-2, Core EV-5, Core EV-9, Core A2-4, Core A2-5, Core DP-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum Event {
    Completed {
        op: OpId,
        result: OpResult,
    } = 1,
    SessionState {
        id: SessionId,
        instance: InstanceId,
        state: SessionState,
    } = 2,
    RouteClosed {
        route: RouteId,
        reason: RouteCloseReason,
        /// Clause: Core OU-1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route_tag: Option<String>,
    } = 3,
    RouteStalled {
        route: RouteId,
    } = 4,
    RouteResumed {
        route: RouteId,
    } = 5,
    SessionWritable {
        id: SessionId,
        instance: InstanceId,
    } = 6,
    ModesChanged {
        id: SessionId,
        instance: InstanceId,
        flags: ModeFlags,
    } = 7,
    FocusChanged {
        id: SessionId,
        instance: InstanceId,
        focused: FocusState,
    } = 8,
    SizeChanged {
        id: SessionId,
        instance: InstanceId,
        size: Size,
    } = 9,
    Activity {
        id: SessionId,
        instance: InstanceId,
        source: ActivitySource,
        at: UnixSeconds,
    } = 10,
    Silent {
        id: SessionId,
        instance: InstanceId,
        since: UnixSeconds,
    } = 11,
    TitleChanged {
        id: SessionId,
        instance: InstanceId,
        title: String,
    } = 12,
    CwdChanged {
        id: SessionId,
        instance: InstanceId,
        cwd: String,
    } = 13,
    Bell {
        id: SessionId,
        instance: InstanceId,
        at: UnixSeconds,
    } = 14,
    PromptMark {
        id: SessionId,
        instance: InstanceId,
        mark: PromptMarkKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        at: UnixSeconds,
    } = 15,
    /// Clause: Core A2-4.
    Notification {
        id: SessionId,
        instance: InstanceId,
        source: NotificationSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        body: String,
        truncated: bool,
        at: UnixSeconds,
    } = 16,
    ClipboardWrite {
        id: SessionId,
        instance: InstanceId,
        selection: String,
        /// `None` is `TooLarge`; an empty list clears the destination, which differs from one entry with empty `bytes`.
        ///
        /// Clause: Core EV-3, Core A13-1, Core A14-2.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contents: Option<Vec<ClipboardContent>>,
        total_bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<ClipboardReason>,
    } = 17,
    EventsLost {
        id: SessionId,
        instance: InstanceId,
        kinds: BTreeSet<LostKind>,
        /// Clause: Core TP-1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tap_dropped_bytes: Option<u64>,
    } = 18,
    MetadataChanged {
        id: SessionId,
        instance: InstanceId,
    } = 19,
    RouteAdopted {
        session: SessionId,
        instance: InstanceId,
        route: RouteId,
        route_tag: String,
        owner: String,
        client: ClientId,
    } = 20,
    ServiceState {
        id: ServiceId,
        state: ServiceState,
    } = 21,
    LaneConnected {
        id: ServiceId,
        lane: u8,
        epoch: u64,
    } = 22,
    LaneEnded {
        id: ServiceId,
        lane: u8,
        epoch: u64,
        reason: LaneEndReason,
        dropped_unsent_frames: u64,
        dropped_unsent_bytes: u64,
    } = 23,
    ServiceWritable {
        id: ServiceId,
        lane: u8,
    } = 24,
    ServiceReadable {
        id: ServiceId,
        lane: u8,
    } = 25,
}

lenient! {
    LostKind => ["Bell", "PromptMark", "Notification", "ClipboardWrite", "Query", "Tap"],
}

//! Event streams (HC section 8) and the payloads of the Hub's own events (SF-5, SF-5d, TS-9).
//!
//! The layout of `event_gap` (`"frame": "gap"` on an `event` frame) is pending steward ruling H2.

use crate::refs::SessionRef;
use crate::wire::{open_enum, opt_no_null, OpenObject, Timestamp};
use botster_route_codec::prelude::{NotificationSource, RouteEndReason};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

open_enum! {
    /// The priority of an event, fixed per event name.
    ///
    /// Clause: HC EV-1.
    pub enum EventPriority {
        High = "high",
        Normal = "normal",
        Low = "low",
    }
}

open_enum! {
    /// `source.kind`.
    ///
    /// Clause: HC EV-1.
    pub enum SourceKind {
        Hub = "hub",
        Plugin = "plugin",
        Terminal = "terminal",
    }
}

/// Where an event came from. `instance` tells a restarted source from the old one. An absent `hub_id` means the local Hub.
///
/// Clause: HC EV-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventSource {
    pub kind: SourceKind,
    pub instance: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub session: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub hub_id: Option<String>,
}

/// An event on a stream.
///
/// Clause: HC EV-1, EV-2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    pub stream_id: String,
    /// `hub`, `terminal` or a plugin key.
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub subject: Option<Value>,
    pub priority: EventPriority,
    pub source: EventSource,
    pub payload: Value,
    /// Stamped by the Hub.
    pub at: Timestamp,
}

open_enum! {
    /// How a subscriber recovers from a gap (EV-4).
    ///
    /// Clause: HC EV-4.
    pub enum Recovery {
        State = "state",
        Unavailable = "unavailable",
        Partial = "partial",
    }
}

/// The exact read-only read that recovers state after a gap. A descriptor never creates authority.
///
/// Clause: HC EV-4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StateRead {
    pub surface: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub arguments: Option<OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub reference: Option<OpenObject>,
    pub grant: String,
}

/// The tag of an `event_gap` frame.
///
/// Clause: HC EV-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GapTag {
    Gap,
}

/// The loss marker, at the position of the first lost copy. `count` and `lost_bytes` are separate exact counters.
///
/// Clause: HC EV-4.
// pending steward ruling H2: `event_gap` is an `event` frame with `"frame": "gap"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventGap {
    pub frame: GapTag,
    pub stream_id: String,
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub subject: Option<Value>,
    pub count: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub lost_bytes: Option<u64>,
    pub recovery: Recovery,
    /// Only with `recovery: state`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub state_read: Option<StateRead>,
    /// The per-unit counts of an aggregated gap (HP EV-12 owns the shape).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub aggregated: Option<Value>,
}

/// The body of a server `event` frame: an event, or a gap marker.
///
/// Clause: HC EV-2, EV-4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EventFrame {
    Gap(EventGap),
    Event(Event),
}

/// The filter of `watch(event, …)`: `owner` or `name` may be `*`, and `name` may end in `.*` (HP EV-2). `min_priority` defaults to `low`.
///
/// Clause: HC EV-2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventFilter {
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub subjects: Option<Vec<Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub min_priority: Option<EventPriority>,
}

open_enum! {
    /// The severity of a notice.
    ///
    /// Clause: HC SF-5.
    pub enum Severity {
        Info = "info",
        Success = "success",
        Warning = "warning",
        Error = "error",
    }
}

/// The payload of `hub/notice`, and of the push copy.
///
/// Clause: HC SF-5, SF-5b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NoticePayload {
    pub notice_id: String,
    pub package: String,
    pub reaction: String,
    pub severity: Severity,
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub url: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub tag: Option<String>,
    pub ttl_ms: u64,
}

/// The payload of `hub/session_notification`.
///
/// Clause: HC SF-5d.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionNotification {
    pub session: SessionRef,
    pub source: NotificationSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub body: Option<String>,
    pub at: Timestamp,
}

/// The subject of `hub/route.ended`.
///
/// Clause: HC TS-9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteEndedSubject {
    pub route_id: String,
}

/// The payload of `hub/route.ended` on the implicit `routes` stream.
///
/// Clause: HC TS-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteEndedPayload {
    pub reason: RouteEndReason,
}

/// The id of the implicit, always-open stream that carries `route.ended`.
///
/// Clause: HC TS-9.
pub const ROUTES_STREAM_ID: &str = "routes";

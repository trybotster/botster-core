//! Event types: the delivered record, the gap record and the payload table of Hub and terminal events.
//!
//! Clause: HP section 10, EV-1 to EV-15, VC-7, A2-6, A2-23, NT-4, WP-4.

use crate::common::{OpenObject, SessionRef, WireU64};
use crate::frames::EventSourceRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `Invoke.payload` of an `event` handler: one delivered copy (HP EV-15). Every copy carries the Hub-stamped `at` (Unix milliseconds, WF-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventRecord {
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subject: Option<Value>,
    pub priority: String,
    pub source: EventSourceRef,
    pub payload: Value,
    pub at: u64,
}

/// How a gap can be recovered (HP EV-12, EV-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Recovery {
    State,
    Unavailable,
    Partial,
}

/// The exact permitted read of a gap with `recovery: state` (HP EV-12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StateRead {
    pub surface: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub arguments: Option<OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reference: Option<OpenObject>,
    pub grant: String,
}

/// `Invoke.payload` of an `event_gap` handler (HP EV-15, Appendix A). `count` is `lost_events`; `lost_bytes` is separate (EV-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventGapRecord {
    pub gap_id: String,
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subject: Option<Value>,
    pub count: WireU64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub lost_bytes: Option<WireU64>,
    pub recovery: Recovery,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub state_read: Option<StateRead>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub aggregated: Option<bool>,
}

/// The audience of a Hub event (HP EV-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aud {
    Clients,
    Plugins,
    Both,
    /// The contract states no audience for the event.
    Unstated,
}

/// One row of the event payload table (HP EV-9, A2-6): the event name, its priority, audience, grant and payload schema.
pub struct EventSpec {
    pub owner: &'static str,
    pub name: &'static str,
    pub priority: &'static str,
    pub audience: Aud,
    /// The grant a plugin needs to subscribe (HP EV-2, A2-23). `"-"` where the contract states none for plugins.
    pub grant: &'static str,
    pub subject: &'static str,
    pub payload: fn() -> Value,
    pub clauses: &'static str,
}

fn sch<T: JsonSchema>() -> Value {
    crate::schema::inline::<T>()
}

/// `{session, state, exit?, reason?}` of `hub/session.<state>` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionStateEvent {
    pub session: SessionRef,
    pub state: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub exit: Option<ExitInfo>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
}

/// `exit{code, signal, cause}` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExitInfo {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub code: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub signal: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cause: Option<String>,
}

/// `hub/session.attached` and `hub/session.detached` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionAttachEvent {
    pub session: SessionRef,
    pub client_kind: String,
}

/// The source of `hub/session.activity` (HP A2-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    Output,
    Client,
}

/// `hub/session.activity` (HP A2-6, SO-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionActivityEvent {
    pub session: SessionRef,
    pub source: ActivitySource,
    pub at: u64,
}

/// The terminal modes of `hub/session.modes` and `terminal/modes` (HP SO-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Modes {
    pub bracketed_paste: bool,
    pub kitty_flags: u64,
    pub alt_screen: bool,
}

/// `hub/session.modes` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionModesEvent {
    pub session: SessionRef,
    pub modes: Modes,
}

/// `hub/session.size` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSizeEvent {
    pub session: SessionRef,
    pub rows: u32,
    pub cols: u32,
}

/// `hub/session.silent` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSilentEvent {
    pub session: SessionRef,
    pub since: u64,
}

/// `hub/route.ended`: subject `{route_id}`, payload `{reason}` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteEndedEvent {
    pub reason: String,
}

/// `hub/plugin.loaded`, `hub/plugin.unloaded` and `hub/plugin.failed` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PluginLifecycleEvent {
    pub plugin_key: String,
    pub version: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
}

/// `hub/plugin.dependency` (HP PD-8, EV-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PluginDependencyEvent {
    pub interface: String,
    pub provider: String,
    pub state: DependencyState,
}

/// The state in `hub/plugin.dependency` (HP PD-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    Available,
    Unavailable,
    Replaced,
}

/// `hub/config.changed` (HP A2-6, A2-52).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigChangedEvent {
    pub keys: Vec<String>,
    pub restart_required: Vec<String>,
}

/// `hub/package.configuration_changed` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageConfigurationChangedEvent {
    pub plugin_key: String,
    pub names: Vec<String>,
}

/// The source of `hub/session_notification` (HP EV-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSource {
    Bell,
    Osc9,
    Osc777,
}

/// `hub/session_notification` (HP EV-9, CA-22).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionNotificationEvent {
    pub session: SessionRef,
    pub source: NotificationSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub body: Option<String>,
    pub at: u64,
}

/// `hub/notice` and the push payload (HP VW-9, client SF-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NoticeEvent {
    pub notice_id: String,
    pub package: String,
    pub reaction: String,
    pub severity: String,
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub url: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub tag: Option<String>,
    pub ttl_ms: u64,
}

/// `hub/worktree.created`, `.create_failed`, `.deleted` and `.delete_failed` (HP EV-8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeEvent {
    pub worktree_id: String,
    pub target_id: String,
    pub branch: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub session: Option<SessionRef>,
}

/// `terminal/title` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalTitle {
    pub text: String,
}

/// `terminal/cwd` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalCwd {
    pub cwd: String,
}

/// `terminal/bell` carries an empty payload (HP A2-6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalBell {}

/// `terminal/notification` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalNotification {
    pub source: TerminalNotificationSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub title: Option<String>,
    pub body: String,
}

/// The source of `terminal/notification` (HP A2-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminalNotificationSource {
    Osc9,
    Osc777,
}

/// `terminal/prompt_mark` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalPromptMark {
    pub mark: PromptMark,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub exit_code: Option<i64>,
}

/// The mark letter of `terminal/prompt_mark` (HP A2-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromptMark {
    A,
    B,
    C,
    D,
}

/// `terminal/modes` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalModes {
    pub modes: Modes,
}

/// `terminal/exit` (HP A2-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalExit {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub code: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub signal: Option<String>,
    pub cause: String,
}

/// `terminal/output`: the opt-in raw-output tap (HP EV-14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalOutput {
    pub bytes_b64: String,
    pub dropped_before: WireU64,
}

/// `net/ws.message` (HP NT-4): exactly one of `text` and `data_b64`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WsMessageEvent {
    Text(WsMessageText),
    Data(WsMessageData),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsMessageText {
    pub ws_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsMessageData {
    pub ws_id: String,
    pub data_b64: String,
}

/// `net/ws.close` (HP NT-4). `error` is one of the NT-9 codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WsCloseEvent {
    pub ws_id: String,
    pub code: u32,
    pub reason: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub error: Option<String>,
}

macro_rules! ev {
    ($o:literal, $n:literal, $p:literal, $a:expr, $g:literal, $s:literal, $t:ty, $c:literal) => {
        EventSpec {
            owner: $o,
            name: $n,
            priority: $p,
            audience: $a,
            grant: $g,
            subject: $s,
            payload: sch::<$t>,
            clauses: $c,
        }
    };
}

/// The Hub, terminal and `net` events with their payloads (HP EV-9, EV-14, NT-4, A2-6, A2-23).
/// Lifecycle events of the session family states share one payload shape (`hub/session.<state>`).
pub const EVENTS: &[EventSpec] = &[
    ev!(
        "hub",
        "session.created",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.starting",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.running",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.stopping",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.ended",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9, EV-11"
    ),
    ev!(
        "hub",
        "session.lost",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.removed",
        "high",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionStateEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.attached",
        "normal",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionAttachEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.detached",
        "normal",
        Aud::Unstated,
        "events_hub",
        "session",
        SessionAttachEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "session.activity",
        "normal",
        Aud::Unstated,
        "events_hub + session_observe",
        "session",
        SessionActivityEvent,
        "A2-6, A2-23, SO-3"
    ),
    ev!(
        "hub",
        "session.modes",
        "normal",
        Aud::Unstated,
        "events_hub + session_observe",
        "session",
        SessionModesEvent,
        "A2-6, A2-23, SO-3"
    ),
    ev!(
        "hub",
        "session.size",
        "normal",
        Aud::Unstated,
        "events_hub + session_observe",
        "session",
        SessionSizeEvent,
        "A2-6, A2-23, SO-3"
    ),
    ev!(
        "hub",
        "session.silent",
        "normal",
        Aud::Unstated,
        "events_hub + session_observe",
        "session",
        SessionSilentEvent,
        "A2-6, A2-23, SO-3"
    ),
    ev!(
        "hub",
        "route.ended",
        "high",
        Aud::Clients,
        "-",
        "route",
        RouteEndedEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "plugin.loaded",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        PluginLifecycleEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "plugin.unloaded",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        PluginLifecycleEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "plugin.failed",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        PluginLifecycleEvent,
        "A2-6, EV-9"
    ),
    ev!(
        "hub",
        "plugin.dependency",
        "high",
        Aud::Plugins,
        "events_hub",
        "consumer",
        PluginDependencyEvent,
        "PD-8, EV-9"
    ),
    ev!(
        "hub",
        "config.changed",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        ConfigChangedEvent,
        "A2-6, A2-52"
    ),
    ev!(
        "hub",
        "package.configuration_changed",
        "normal",
        Aud::Unstated,
        "events_hub",
        "package",
        PackageConfigurationChangedEvent,
        "A2-6"
    ),
    ev!(
        "hub",
        "notice",
        "normal",
        Aud::Clients,
        "-",
        "-",
        NoticeEvent,
        "VW-9, EV-9"
    ),
    ev!(
        "hub",
        "session_notification",
        "normal",
        Aud::Both,
        "events_terminal",
        "session",
        SessionNotificationEvent,
        "EV-9, CA-22"
    ),
    ev!(
        "hub",
        "worktree.created",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        WorktreeEvent,
        "EV-8"
    ),
    ev!(
        "hub",
        "worktree.create_failed",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        WorktreeEvent,
        "EV-8"
    ),
    ev!(
        "hub",
        "worktree.deleted",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        WorktreeEvent,
        "EV-8"
    ),
    ev!(
        "hub",
        "worktree.delete_failed",
        "normal",
        Aud::Unstated,
        "events_hub",
        "-",
        WorktreeEvent,
        "EV-8"
    ),
    ev!(
        "terminal",
        "title",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalTitle,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "cwd",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalCwd,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "bell",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalBell,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "notification",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalNotification,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "prompt_mark",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalPromptMark,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "modes",
        "normal",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalModes,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "exit",
        "high",
        Aud::Unstated,
        "events_terminal",
        "session",
        TerminalExit,
        "A2-6, EV-14"
    ),
    ev!(
        "terminal",
        "output",
        "low",
        Aud::Unstated,
        "session_output:read",
        "session",
        TerminalOutput,
        "EV-14"
    ),
    ev!(
        "net",
        "ws.message",
        "normal",
        Aud::Plugins,
        "net",
        "-",
        WsMessageEvent,
        "NT-4"
    ),
    ev!(
        "net",
        "ws.close",
        "high",
        Aud::Plugins,
        "net",
        "-",
        WsCloseEvent,
        "NT-4"
    ),
];

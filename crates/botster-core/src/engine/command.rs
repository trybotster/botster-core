//! Policy-free engine command surface vocabulary.
//!
//! [`BotsterEngine`](crate::BotsterEngine) is the canonical command facade for
//! hosts that supply their own runtime adapters. `DefaultBotsterEngine` is the
//! default local PTY-backed instance of the same facade when the `local-runtime`
//! feature is enabled. [`MultiplexerEngine`](crate::MultiplexerEngine)
//! remains the lower-level assembled primitive under those facades.
//!
//! This module names the command boundary without introducing a second router.
//! Requests, results, events, and errors intentionally reuse the existing typed
//! core contracts. Host, hub, CLI, provider, plugin, and UI policy stays outside
//! `botster-core`.
//!
//! | Command | Request shape | Result/event shape | Public entry point |
//! | --- | --- | --- | --- |
//! | Spawn local session | [`SessionSpawnRequest`] plus [`CoreSessionMetadata`] | [`BotsterSpawnOutcome`] | [`BotsterEngine::spawn_session`](crate::BotsterEngine::spawn_session), `DefaultBotsterEngine::spawn_session` |
//! | Attach client | [`ClientId`], [`SessionId`], [`SubscriptionId`] | [`BotsterEngineOutput`] | [`BotsterEngine::attach_client`](crate::BotsterEngine::attach_client) |
//! | Detach client | [`ClientId`], [`SessionId`], [`SubscriptionId`] | [`BotsterEngineOutput`] | [`BotsterEngine::detach_client`](crate::BotsterEngine::detach_client) |
//! | Send input | typed terminal bytes | [`SessionIoRequest::PtyInput`] routed in [`BotsterEngineOutput`] | [`BotsterEngine::write_bytes`](crate::BotsterEngine::write_bytes) |
//! | Resize | rows and columns | [`SessionIoRequest::Resize`] routed in [`BotsterEngineOutput`] | [`BotsterEngine::resize`](crate::BotsterEngine::resize) |
//! | List sessions | none | `Vec<CoreSession>` | [`BotsterEngine::list_sessions`](crate::BotsterEngine::list_sessions) |
//! | Inspect session | [`SessionId`] plus caller clock/threshold | [`EngineSessionInspection`] | [`BotsterEngine::inspect_session`](crate::BotsterEngine::inspect_session) |
//! | Read screen | [`RequestId`] plus [`SessionId`] | [`SessionIoEvent::ScreenReady`] in [`BotsterEngineOutput`] | [`BotsterEngine::read_screen`](crate::BotsterEngine::read_screen) |
//! | Capture snapshot | [`RequestId`] plus [`SessionId`] | [`SessionIoEvent::SnapshotReady`] in [`BotsterEngineOutput`] | [`BotsterEngine::capture_snapshot`](crate::BotsterEngine::capture_snapshot) |
//! | Replay snapshot | [`PreparedSnapshotRequest`] | [`SessionIoEvent::PreparedSnapshotReady`] in [`BotsterEngineOutput`] | [`BotsterEngine::replay_snapshot`](crate::BotsterEngine::replay_snapshot) |
//! | Shutdown | [`SessionId`] plus host reason | [`BotsterEngineOutput`] | [`BotsterEngine::shutdown_session`](crate::BotsterEngine::shutdown_session) |
//! | Notifications | [`NotificationItem`] and [`NotificationTarget`] | [`NotificationId`] / `Vec<NotificationItem>` | [`BotsterEngine::post_notification`](crate::BotsterEngine::post_notification), [`BotsterEngine::drain_notifications`](crate::BotsterEngine::drain_notifications) |
//!
//! Core methods are synchronous and deterministic: they return typed outcomes
//! for the caller to deliver. Hosts own executors, queues, retry policy,
//! persistence, config discovery, auth, cloud/WebRTC/signaling, marketplace
//! policy, CLI UX, TUI/browser rendering, and product workflows.

use crate::actor::{PreparedSnapshotRequest, SessionIoEvent, SessionIoRequest};
use crate::contract::notification::{NotificationId, NotificationItem, NotificationTarget};
use crate::engine::botster::{BotsterEngineOutput, BotsterSpawnOutcome};
use crate::runtime::SessionSpawnRequest;
use crate::session::{
    CoreSession, CoreSessionMetadata, RequestId, SessionActivityStatus, SessionId, SubscriptionId,
};
use crate::ClientId;

/// Stable names for policy-free engine commands currently supported by core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineCommandKind {
    /// Spawn a session from an explicit host-resolved request.
    SpawnSession,
    /// Attach one client subscription to a live session stream.
    AttachClient,
    /// Detach one client subscription from a live session stream.
    DetachClient,
    /// Send terminal input bytes.
    SendInput,
    /// Resize a terminal session.
    Resize,
    /// List sessions currently recorded by the core engine.
    ListSessions,
    /// Inspect one session's lifecycle and activity.
    InspectSession,
    /// Read plain screen state where the runtime adapter supports it.
    ReadScreen,
    /// Capture an opaque terminal snapshot where the runtime adapter supports it.
    CaptureSnapshot,
    /// Replay or prepare an opaque terminal snapshot where the runtime adapter supports it.
    ReplaySnapshot,
    /// Shut down a live session.
    Shutdown,
    /// Queue or drain core notification inbox items.
    Notifications,
}

/// All command names that belong to the current policy-free core surface.
pub const ENGINE_COMMAND_KINDS: &[EngineCommandKind] = &[
    EngineCommandKind::SpawnSession,
    EngineCommandKind::AttachClient,
    EngineCommandKind::DetachClient,
    EngineCommandKind::SendInput,
    EngineCommandKind::Resize,
    EngineCommandKind::ListSessions,
    EngineCommandKind::InspectSession,
    EngineCommandKind::ReadScreen,
    EngineCommandKind::CaptureSnapshot,
    EngineCommandKind::ReplaySnapshot,
    EngineCommandKind::Shutdown,
    EngineCommandKind::Notifications,
];

/// Session inspection returned by the command facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSessionInspection {
    /// Current core session record.
    pub session: CoreSession,
    /// Activity classification at the caller-provided clock value.
    pub activity_status: SessionActivityStatus,
}

/// Spawn request shape used by the command surface.
pub type EngineSpawnSessionRequest = SessionSpawnRequest;

/// Spawn metadata supplied by the host.
pub type EngineSpawnSessionMetadata = CoreSessionMetadata;

/// Spawn result shape returned by the command facade.
pub type EngineSpawnSessionResult = BotsterSpawnOutcome;

/// Mutating command result shape returned by the command facade.
pub type EngineCommandResult = BotsterEngineOutput;

/// Session worker event shape surfaced by command outcomes.
pub type EngineCommandEvent = SessionIoEvent;

/// Direct session I/O request shape used for screen and snapshot commands.
pub type EngineSessionIoRequest = SessionIoRequest;

/// Snapshot replay request shape.
pub type EngineReplaySnapshotRequest = PreparedSnapshotRequest;

/// Notification queue item shape.
pub type EngineNotificationItem = NotificationItem;

/// Notification target shape.
pub type EngineNotificationTarget = NotificationTarget;

/// Notification identifier shape.
pub type EngineNotificationId = NotificationId;

/// Client identifier used by attach, detach, input, and resize commands.
pub type EngineClientId = ClientId;

/// Session identifier used by session-scoped commands.
pub type EngineSessionId = SessionId;

/// Subscription identifier used by attach and detach commands.
pub type EngineSubscriptionId = SubscriptionId;

/// Request identifier used by screen and snapshot commands.
pub type EngineRequestId = RequestId;

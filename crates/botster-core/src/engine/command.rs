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
//!
//! ```
//! use botster_core::{
//!     ClientId, EngineCommand, EngineCommandKind, SessionId, SubscriptionId,
//! };
//!
//! let command: EngineCommand<()> = EngineCommand::AttachClient {
//!     client_id: ClientId("client-a".to_string()),
//!     session_id: SessionId("session-a".to_string()),
//!     subscription_id: SubscriptionId("sub-a".to_string()),
//!     now_seconds: 10,
//! };
//!
//! assert_eq!(command.kind(), EngineCommandKind::AttachClient);
//! ```

use std::error::Error;
use std::fmt;

use crate::actor::{PreparedSnapshotRequest, SessionIoEvent, SessionIoRequest};
use crate::contract::notification::{
    NotificationId, NotificationItem, NotificationTarget, NotificationTimestamp,
};
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

/// Typed command request executed by [`BotsterEngine`](crate::BotsterEngine).
///
/// The generic worker runtime is supplied only by [`SpawnSession`](Self::SpawnSession)
/// because custom host adapters own worker construction.
pub enum EngineCommand<W> {
    /// Spawn a session from explicit host-resolved process details.
    SpawnSession {
        /// Spawn request supplied by the host.
        request: SessionSpawnRequest,
        /// Core session metadata supplied by the host.
        metadata: CoreSessionMetadata,
        /// Host worker runtime for the spawned session.
        worker_runtime: W,
    },
    /// Attach one client subscription to a session.
    AttachClient {
        /// Client being attached.
        client_id: ClientId,
        /// Session receiving the subscription.
        session_id: SessionId,
        /// Subscription identity chosen by the host.
        subscription_id: SubscriptionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Detach one client subscription from a session.
    DetachClient {
        /// Client being detached.
        client_id: ClientId,
        /// Session losing the subscription.
        session_id: SessionId,
        /// Subscription identity chosen by the host.
        subscription_id: SubscriptionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Send terminal bytes from a client to a session.
    SendInput {
        /// Client sending input.
        client_id: ClientId,
        /// Session receiving input.
        session_id: SessionId,
        /// Terminal bytes supplied by the caller.
        data: Vec<u8>,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Resize a session terminal.
    Resize {
        /// Client requesting the resize.
        client_id: ClientId,
        /// Session being resized.
        session_id: SessionId,
        /// Terminal rows.
        rows: u16,
        /// Terminal columns.
        cols: u16,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// List recorded sessions.
    ListSessions,
    /// Inspect one session lifecycle and activity.
    InspectSession {
        /// Session being inspected.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
        /// Maximum idle interval classified as active.
        active_threshold_seconds: u64,
    },
    /// Read plain screen state where the worker supports it.
    ReadScreen {
        /// Caller-supplied request id.
        request_id: RequestId,
        /// Session being read.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Capture an opaque terminal snapshot where the worker supports it.
    CaptureSnapshot {
        /// Caller-supplied request id.
        request_id: RequestId,
        /// Session being snapshotted.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Replay or prepare an opaque terminal snapshot where the worker supports it.
    ReplaySnapshot {
        /// Snapshot request supplied by the caller.
        request: PreparedSnapshotRequest,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Shut down one session.
    Shutdown {
        /// Session being shut down.
        session_id: SessionId,
        /// Host-supplied reason string.
        reason: String,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Queue one notification in the generic engine inbox.
    PostNotification {
        /// Notification item supplied by the caller.
        item: NotificationItem,
    },
    /// Drain deliverable notifications for one target.
    DrainNotifications {
        /// Target to drain.
        target: NotificationTarget,
        /// Caller-supplied notification clock.
        now: NotificationTimestamp,
    },
}

impl<W> EngineCommand<W> {
    /// Return the stable command kind represented by this request.
    #[must_use]
    pub const fn kind(&self) -> EngineCommandKind {
        match self {
            Self::SpawnSession { .. } => EngineCommandKind::SpawnSession,
            Self::AttachClient { .. } => EngineCommandKind::AttachClient,
            Self::DetachClient { .. } => EngineCommandKind::DetachClient,
            Self::SendInput { .. } => EngineCommandKind::SendInput,
            Self::Resize { .. } => EngineCommandKind::Resize,
            Self::ListSessions => EngineCommandKind::ListSessions,
            Self::InspectSession { .. } => EngineCommandKind::InspectSession,
            Self::ReadScreen { .. } => EngineCommandKind::ReadScreen,
            Self::CaptureSnapshot { .. } => EngineCommandKind::CaptureSnapshot,
            Self::ReplaySnapshot { .. } => EngineCommandKind::ReplaySnapshot,
            Self::Shutdown { .. } => EngineCommandKind::Shutdown,
            Self::PostNotification { .. } | Self::DrainNotifications { .. } => {
                EngineCommandKind::Notifications
            }
        }
    }
}

/// Typed command request executed by `DefaultBotsterEngine`.
///
/// Notifications are intentionally absent because the default local facade does
/// not expose notification inbox methods today.
#[cfg(feature = "local-runtime")]
pub enum DefaultEngineCommand {
    /// Spawn a local PTY-backed session from explicit host-resolved details.
    SpawnSession {
        /// Spawn request supplied by the host.
        request: SessionSpawnRequest,
        /// Core session metadata supplied by the host.
        metadata: CoreSessionMetadata,
    },
    /// Attach one client subscription to a session.
    AttachClient {
        /// Client being attached.
        client_id: ClientId,
        /// Session receiving the subscription.
        session_id: SessionId,
        /// Subscription identity chosen by the host.
        subscription_id: SubscriptionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Detach one client subscription from a session.
    DetachClient {
        /// Client being detached.
        client_id: ClientId,
        /// Session losing the subscription.
        session_id: SessionId,
        /// Subscription identity chosen by the host.
        subscription_id: SubscriptionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Send terminal bytes from a client to a session.
    SendInput {
        /// Client sending input.
        client_id: ClientId,
        /// Session receiving input.
        session_id: SessionId,
        /// Terminal bytes supplied by the caller.
        data: Vec<u8>,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Resize a session terminal.
    Resize {
        /// Client requesting the resize.
        client_id: ClientId,
        /// Session being resized.
        session_id: SessionId,
        /// Terminal rows.
        rows: u16,
        /// Terminal columns.
        cols: u16,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// List recorded sessions.
    ListSessions,
    /// Inspect one session lifecycle and activity.
    InspectSession {
        /// Session being inspected.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
        /// Maximum idle interval classified as active.
        active_threshold_seconds: u64,
    },
    /// Read plain screen state where the managed runtime supports it.
    ReadScreen {
        /// Caller-supplied request id.
        request_id: RequestId,
        /// Session being read.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Capture an opaque terminal snapshot where the managed runtime supports it.
    CaptureSnapshot {
        /// Caller-supplied request id.
        request_id: RequestId,
        /// Session being snapshotted.
        session_id: SessionId,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Replay or prepare an opaque terminal snapshot where the managed runtime supports it.
    ReplaySnapshot {
        /// Snapshot request supplied by the caller.
        request: PreparedSnapshotRequest,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
    /// Shut down one session.
    Shutdown {
        /// Session being shut down.
        session_id: SessionId,
        /// Host-supplied reason string.
        reason: String,
        /// Caller-supplied logical clock.
        now_seconds: u64,
    },
}

#[cfg(feature = "local-runtime")]
impl DefaultEngineCommand {
    /// Return the stable command kind represented by this request.
    #[must_use]
    pub const fn kind(&self) -> EngineCommandKind {
        match self {
            Self::SpawnSession { .. } => EngineCommandKind::SpawnSession,
            Self::AttachClient { .. } => EngineCommandKind::AttachClient,
            Self::DetachClient { .. } => EngineCommandKind::DetachClient,
            Self::SendInput { .. } => EngineCommandKind::SendInput,
            Self::Resize { .. } => EngineCommandKind::Resize,
            Self::ListSessions => EngineCommandKind::ListSessions,
            Self::InspectSession { .. } => EngineCommandKind::InspectSession,
            Self::ReadScreen { .. } => EngineCommandKind::ReadScreen,
            Self::CaptureSnapshot { .. } => EngineCommandKind::CaptureSnapshot,
            Self::ReplaySnapshot { .. } => EngineCommandKind::ReplaySnapshot,
            Self::Shutdown { .. } => EngineCommandKind::Shutdown,
        }
    }
}

/// Session inspection returned by the command facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSessionInspection {
    /// Current core session record.
    pub session: CoreSession,
    /// Activity classification at the caller-provided clock value.
    pub activity_status: SessionActivityStatus,
}

/// Typed result returned by a heterogeneous engine command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCommandOutcome {
    /// Session spawn completed.
    SpawnSession(BotsterSpawnOutcome),
    /// A command produced session/client output.
    Output(BotsterEngineOutput),
    /// Recorded sessions were listed.
    Sessions(Vec<CoreSession>),
    /// One session was inspected.
    Inspection(EngineSessionInspection),
    /// One notification was queued.
    NotificationPosted(NotificationId),
    /// Deliverable notifications were drained.
    NotificationsDrained(Vec<NotificationItem>),
}

/// Error returned by typed command execution.
#[derive(Debug)]
pub struct EngineCommandError<E> {
    /// Command kind that failed.
    pub kind: EngineCommandKind,
    /// Typed facade error from the underlying engine.
    pub source: E,
}

impl<E> EngineCommandError<E> {
    /// Wrap a facade error with the command kind that produced it.
    #[must_use]
    pub const fn new(kind: EngineCommandKind, source: E) -> Self {
        Self { kind, source }
    }
}

impl<E> fmt::Display for EngineCommandError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} command failed: {}", self.kind, self.source)
    }
}

impl<E> Error for EngineCommandError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Spawn request shape used by the command surface.
pub type EngineSpawnSessionRequest = SessionSpawnRequest;

/// Spawn metadata supplied by the host.
pub type EngineSpawnSessionMetadata = CoreSessionMetadata;

/// Spawn result shape returned by the command facade.
pub type EngineSpawnSessionResult = BotsterSpawnOutcome;

/// Typed command result shape returned by the command facade.
pub type EngineCommandResult = EngineCommandOutcome;

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

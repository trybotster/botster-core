//! Actor mailbox contract types shared by Botster runtime crates.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::boundary::BoundaryJson;
use crate::client::{ClientId, ClientState};
use crate::session::{RequestId, SessionId, SubscriptionId};
use crate::session_protocol::{
    ModeFlags, NotificationPayload, ProcessExitedPayload, PromptMarkPayload, TerminalColorProfile,
};

/// Bounded actor mailbox metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedQueueConfig {
    /// Stable queue name.
    pub name: String,
    /// Maximum queued messages. Zero is not bounded and should be rejected by
    /// runtime queue builders.
    pub capacity: usize,
}

impl BoundedQueueConfig {
    /// Build queue metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        Self {
            name: name.into(),
            capacity,
        }
    }

    /// Whether this queue has a finite positive capacity.
    #[must_use]
    pub const fn is_bounded(&self) -> bool {
        self.capacity > 0
    }
}

/// Public actor mailbox queues defined by core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSource {
    /// Hub control request queue.
    HubControl,
    /// Per-client worker queue.
    ClientWorker,
    /// Per-session I/O worker queue.
    SessionIo,
    /// Concrete transport adapter queue.
    TransportAdapter,
    /// Per-plugin worker queue.
    PluginWorker,
}

impl QueueSource {
    /// Stable queue name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HubControl => "hub-control",
            Self::ClientWorker => "client-worker",
            Self::SessionIo => "session-io",
            Self::TransportAdapter => "transport-adapter",
            Self::PluginWorker => "plugin-worker",
        }
    }

    /// Default bounded capacity for the queue.
    #[must_use]
    pub const fn default_capacity(self) -> usize {
        match self {
            Self::HubControl => 256,
            Self::ClientWorker => 512,
            Self::SessionIo => 512,
            Self::TransportAdapter => 512,
            Self::PluginWorker => 256,
        }
    }

    /// Build the default bounded queue metadata.
    #[must_use]
    pub fn default_config(self) -> BoundedQueueConfig {
        BoundedQueueConfig::new(self.name(), self.default_capacity())
    }
}

/// Every actor queue that core names as part of the public contract.
pub const PUBLIC_QUEUE_SOURCES: [QueueSource; 5] = [
    QueueSource::HubControl,
    QueueSource::ClientWorker,
    QueueSource::SessionIo,
    QueueSource::TransportAdapter,
    QueueSource::PluginWorker,
];

/// Typed routing context for pressure reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackpressureRoute {
    /// Session involved in the pressure path, when session-scoped.
    pub session_id: Option<SessionId>,
    /// Client involved in the pressure path, when client-scoped.
    pub client_id: Option<ClientId>,
    /// Subscription involved in the pressure path, when stream-scoped.
    pub subscription_id: Option<SubscriptionId>,
    /// Plugin involved in the pressure path, when plugin-scoped.
    pub plugin_key: Option<PluginKey>,
}

impl BackpressureRoute {
    /// Build an empty route for queue-wide pressure.
    #[must_use]
    pub const fn queue_only() -> Self {
        Self {
            session_id: None,
            client_id: None,
            subscription_id: None,
            plugin_key: None,
        }
    }
}

/// Bounded queue pressure with typed routing context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackpressureSummary {
    /// Queue reporting pressure.
    pub source: QueueSource,
    /// Configured queue capacity.
    pub capacity: usize,
    /// Current queued message count.
    pub depth: usize,
    /// Typed path affected by the pressure.
    pub route: BackpressureRoute,
}

/// Accepted-but-slow delivery with typed routing context.
///
/// This intentionally mirrors [`BackpressureSummary`] route metadata while
/// keeping distinct semantics: lag means delivery was accepted but is behind a
/// caller-owned budget, not that a queue is full. Lag observations must not
/// drive [`ClientControlFrame::Backpressure`] health frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryLag {
    /// Queue reporting lag.
    pub source: QueueSource,
    /// Configured queue capacity.
    pub capacity: usize,
    /// Current queued message count or lag depth.
    pub depth: usize,
    /// Typed path affected by the lag.
    pub route: BackpressureRoute,
}

/// Actor mailbox send failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailboxSendFailureReason {
    /// The bounded queue had no remaining capacity.
    QueueFull,
    /// The receiver side was closed.
    QueueClosed,
}

/// Explicit actor mailbox send failure with typed routing context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxSendFailure {
    /// Queue that rejected the message.
    pub source: QueueSource,
    /// Typed path affected by the failure.
    pub route: BackpressureRoute,
    /// Stable failure reason.
    pub reason: MailboxSendFailureReason,
}

/// Origin of a hub-control request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubControlOrigin {
    /// Request came from a concrete client.
    Client(ClientId),
    /// Request came from a session worker.
    Session(SessionId),
    /// Request came from a plugin worker.
    Plugin(PluginKey),
    /// Request came from runtime supervision.
    Runtime,
}

/// Session lifecycle summary visible to hub control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleState {
    /// Session is starting.
    Starting,
    /// Session is running.
    Running,
    /// Session is stopping.
    Stopping,
    /// Session exited.
    Exited {
        /// Process exit code.
        code: Option<i32>,
    },
    /// Session failed before a normal exit.
    Failed {
        /// Human-readable failure.
        reason: String,
    },
}

/// Transport peer liveness state without naming a concrete transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportPeerState {
    /// Peer is connecting.
    Connecting,
    /// Peer is connected.
    Connected,
    /// Peer is reconnecting.
    Reconnecting,
    /// Peer is disconnected.
    Disconnected,
}

/// Requested connection mode for a transport peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportConnectionMode {
    /// Prefer a direct peer route.
    Direct,
    /// Prefer a relay route.
    Relay,
    /// Let the runtime choose.
    Auto,
}

/// Transport disconnect summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportDisconnectReason {
    /// Local side intentionally closed the peer.
    LocalClose,
    /// Remote side closed the peer.
    RemoteClose,
    /// Liveness timeout.
    Timeout,
    /// Runtime replaced this peer with another route.
    Replaced,
    /// Human-readable reason owned by the caller.
    Other(String),
}

/// Relay-owned signaling payload. The payload is opaque to core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportSignal {
    /// Peer-local route id.
    pub peer_id: String,
    /// Requested connection mode.
    pub mode: TransportConnectionMode,
    /// Relay-owned payload.
    pub payload: BoundaryJson,
}

/// Hub-control messages emitted by workers and adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HubControlMessage {
    /// Attach a client to a session stream.
    AttachClient {
        /// Request origin.
        origin: HubControlOrigin,
        /// Request correlation id.
        request_id: RequestId,
        /// Client being attached.
        client_id: ClientId,
        /// Session being attached.
        session_id: SessionId,
        /// Subscription identity for the stream.
        subscription_id: SubscriptionId,
    },
    /// Detach a client from a session stream.
    DetachClient {
        /// Request origin.
        origin: HubControlOrigin,
        /// Client being detached.
        client_id: ClientId,
        /// Session being detached.
        session_id: SessionId,
        /// Subscription identity for the stream.
        subscription_id: SubscriptionId,
    },
    /// Request a fresh session snapshot.
    RequestSnapshot {
        /// Request correlation id.
        request_id: RequestId,
        /// Client requesting the snapshot.
        client_id: ClientId,
        /// Session to snapshot.
        session_id: SessionId,
    },
    /// Publish session lifecycle state.
    SessionLifecycle {
        /// Session whose lifecycle changed.
        session_id: SessionId,
        /// New lifecycle state.
        state: SessionLifecycleState,
    },
    /// Publish transport peer liveness.
    TransportPeer {
        /// Peer-local route id.
        peer_id: String,
        /// New peer state.
        state: TransportPeerState,
    },
    /// Carry a relay-owned signal payload.
    TransportSignal(TransportSignal),
    /// Report bounded queue pressure.
    Backpressure(BackpressureSummary),
    /// Request runtime shutdown.
    Shutdown {
        /// Request correlation id.
        request_id: RequestId,
        /// Human-readable reason.
        reason: String,
    },
}

/// Transport-neutral health of a client worker stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientConnectionHealth {
    /// Client stream is healthy.
    Healthy,
    /// Client stream is alive but slow.
    Backpressured,
    /// Client stream is reconnecting.
    Reconnecting,
    /// Client stream is closed.
    Closed,
}

/// Terminal attach state owned by a client worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalAttachState {
    /// No terminal stream is attached; core does not currently produce this state.
    Detached,
    /// Attach has been requested but initial data has not arrived.
    Attaching,
    /// Initial snapshot has arrived and live output may flow.
    Attached,
}

/// Initial terminal snapshot delivery phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialSnapshotPhase {
    /// Waiting for the authoritative snapshot.
    WaitingForSnapshot,
    /// Snapshot has been delivered and live output may flow.
    LiveOutputActive,
}

/// Typed control frame delivered to a client worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientControlFrame {
    /// Acknowledge an attached stream.
    Attached {
        /// Session that attached.
        session_id: SessionId,
        /// Subscription that attached.
        subscription_id: SubscriptionId,
    },
    /// Publish client liveness state.
    State {
        /// New client state.
        state: ClientState,
    },
    /// Publish stream health.
    Health {
        /// New stream health.
        health: ClientConnectionHealth,
    },
    /// Publish attach state.
    AttachState {
        /// New attach state.
        state: TerminalAttachState,
    },
    /// Report pressure visible to the client worker.
    Backpressure(BackpressureSummary),
}

/// Messages accepted by a client worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientWorkerMessage {
    /// Attach this client to a session stream.
    Attach {
        /// Request correlation id.
        request_id: RequestId,
        /// Session to attach.
        session_id: SessionId,
        /// Subscription identity.
        subscription_id: SubscriptionId,
    },
    /// Detach this client from a session stream.
    Detach {
        /// Session to detach.
        session_id: SessionId,
        /// Subscription identity.
        subscription_id: SubscriptionId,
    },
    /// Deliver a typed control frame.
    Control {
        /// Control frame to deliver.
        frame: ClientControlFrame,
    },
    /// Deliver a session event to this client worker.
    SessionEvent {
        /// Session event to deliver.
        event: SessionIoEvent,
    },
}

/// Error reasons for send-file preparation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendFileErrorReason {
    /// Payload exceeded the runtime limit.
    TooLarge,
    /// Runtime could not prepare storage.
    StorageUnavailable,
    /// Payload was not valid for send-file handling.
    InvalidPayload,
}

/// Request to deliver the authoritative initial snapshot before live output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitialSnapshotRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to snapshot.
    pub session_id: SessionId,
    /// Client receiving the initial snapshot.
    pub client_id: ClientId,
    /// Subscription identity for the terminal stream.
    pub subscription_id: SubscriptionId,
    /// Desired terminal rows.
    pub rows: u16,
    /// Desired terminal columns.
    pub cols: u16,
}

/// Delivered authoritative initial snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitialSnapshotReady {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session that produced the snapshot.
    pub session_id: SessionId,
    /// Client receiving the snapshot.
    pub client_id: ClientId,
    /// Subscription identity for the terminal stream.
    pub subscription_id: SubscriptionId,
    /// Opaque snapshot bytes.
    pub snapshot: Vec<u8>,
    /// Terminal rows represented by this snapshot.
    pub rows: u16,
    /// Terminal columns represented by this snapshot.
    pub cols: u16,
}

/// Ordinary terminal snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotReady {
    /// Correlation id for the snapshot request.
    pub request_id: RequestId,
    /// Session that produced the snapshot.
    pub session_id: SessionId,
    /// Opaque snapshot bytes.
    pub data: Vec<u8>,
    /// Terminal rows represented by this snapshot.
    pub rows: u16,
    /// Terminal columns represented by this snapshot.
    pub cols: u16,
}

/// Request to persist a send-file payload for a session runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendFileRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session receiving the send-file payload.
    pub session_id: SessionId,
    /// Caller-visible filename.
    pub filename: String,
    /// Send-file bytes.
    pub data: Vec<u8>,
}

/// Send-file payload was written by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendFileWritten {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session receiving the send-file payload.
    pub session_id: SessionId,
    /// Number of bytes written.
    pub bytes: usize,
    /// Opaque runtime-owned cleanup handle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_ref: Option<String>,
}

/// Send-file payload could not be written by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendFileFailed {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session receiving the send-file payload.
    pub session_id: SessionId,
    /// Stable failure reason.
    pub reason: SendFileErrorReason,
    /// Optional runtime-owned detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Request to prepare an opaque terminal snapshot payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSnapshotRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to snapshot.
    pub session_id: SessionId,
    /// Raw snapshot bytes.
    pub snapshot: Vec<u8>,
    /// Whether this snapshot is intended for recovery.
    pub recovery: bool,
}

/// Prepared opaque terminal snapshot payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSnapshotReady {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session that produced the snapshot.
    pub session_id: SessionId,
    /// Uncompressed source length.
    pub uncompressed_len: usize,
    /// Opaque prepared payload bytes.
    pub payload: Vec<u8>,
    /// Whether this snapshot is intended for recovery.
    pub recovery: bool,
}

/// Terminal mode flags response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeFlagsReady {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session that produced the mode flags.
    pub session_id: SessionId,
    /// Current terminal mode flags.
    pub mode_flags: ModeFlags,
}

/// Plain terminal screen response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenReady {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session that produced the screen.
    pub session_id: SessionId,
    /// Plain screen contents.
    pub text: String,
}

/// Requests accepted by a session I/O worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionIoRequest {
    /// Subscribe a client worker to terminal output.
    SubscribeTerminal {
        /// Request correlation id.
        request_id: RequestId,
        /// Session being subscribed.
        session_id: SessionId,
        /// Client subscribing.
        client_id: ClientId,
        /// Subscription identity.
        subscription_id: SubscriptionId,
        /// Desired terminal rows.
        rows: u16,
        /// Desired terminal columns.
        cols: u16,
    },
    /// Unsubscribe a client worker.
    UnsubscribeTerminal {
        /// Session being unsubscribed.
        session_id: SessionId,
        /// Subscription identity.
        subscription_id: SubscriptionId,
    },
    /// Write terminal input bytes.
    PtyInput {
        /// Session receiving the input.
        session_id: SessionId,
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Resize the terminal.
    Resize {
        /// Session being resized.
        session_id: SessionId,
        /// New row count.
        rows: u16,
        /// New column count.
        cols: u16,
    },
    /// Request a snapshot.
    GetSnapshot {
        /// Request correlation id.
        request_id: RequestId,
        /// Session to snapshot.
        session_id: SessionId,
    },
    /// Request initial snapshot delivery for an attaching client.
    GetInitialSnapshot(InitialSnapshotRequest),
    /// Prepare a send-file payload.
    SendFile(SendFileRequest),
    /// Prepare an opaque snapshot payload.
    PrepareSnapshot(PreparedSnapshotRequest),
    /// Request terminal mode flags.
    GetModeFlags {
        /// Request correlation id.
        request_id: RequestId,
        /// Session to inspect.
        session_id: SessionId,
    },
    /// Request the plain terminal screen.
    GetScreen {
        /// Request correlation id.
        request_id: RequestId,
        /// Session to inspect.
        session_id: SessionId,
    },
    /// Replace the terminal color profile.
    SetColorProfile {
        /// Session receiving the color profile.
        session_id: SessionId,
        /// Color profile to install.
        color_profile: TerminalColorProfile,
    },
    /// Stop the session I/O worker.
    Shutdown {
        /// Session being shut down.
        session_id: SessionId,
        /// Human-readable reason.
        reason: String,
    },
}

/// Events emitted by a session I/O worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionIoEvent {
    /// Terminal output bytes.
    TerminalBytes {
        /// Session that emitted bytes.
        session_id: SessionId,
        /// Output bytes.
        data: Vec<u8>,
    },
    /// Initial snapshot delivered before live output activation.
    InitialSnapshotReady(InitialSnapshotReady),
    /// Requested snapshot delivered.
    SnapshotReady(SnapshotReady),
    /// Send-file payload was written.
    SendFileWritten(SendFileWritten),
    /// Send-file preparation failed.
    SendFileFailed(SendFileFailed),
    /// Prepared snapshot payload is ready.
    PreparedSnapshotReady(PreparedSnapshotReady),
    /// Terminal mode flags response.
    ModeFlagsReady(ModeFlagsReady),
    /// Plain terminal screen response.
    ScreenReady(ScreenReady),
    /// Terminal title changed.
    TitleChanged {
        /// Session that emitted the title.
        session_id: SessionId,
        /// Current terminal title.
        title: String,
    },
    /// Terminal working directory changed.
    CwdChanged {
        /// Session that emitted the cwd.
        session_id: SessionId,
        /// Current terminal working directory.
        cwd: String,
    },
    /// Semantic prompt action detected.
    PromptMark {
        /// Session that emitted the prompt mark.
        session_id: SessionId,
        /// Prompt payload.
        payload: PromptMarkPayload,
    },
    /// Bell character received.
    Bell {
        /// Session that emitted the bell.
        session_id: SessionId,
    },
    /// OSC notification detected.
    Notification {
        /// Session that emitted the notification.
        session_id: SessionId,
        /// Notification payload.
        payload: NotificationPayload,
    },
    /// Session process exited.
    ProcessExited {
        /// Session that exited.
        session_id: SessionId,
        /// Process exit summary.
        payload: ProcessExitedPayload,
    },
    /// Session I/O worker shut down.
    Shutdown {
        /// Session that shut down.
        session_id: SessionId,
        /// Human-readable reason.
        reason: String,
    },
}

/// Maximum terminal output bytes to coalesce before flushing.
pub const SESSION_IO_MAX_COALESCED_BYTES: usize = 32 * 1024;
/// Maximum terminal output frames to coalesce before flushing.
pub const SESSION_IO_MAX_COALESCED_FRAMES: usize = 16;
/// Maximum terminal output or metadata age to coalesce before flushing.
pub const SESSION_IO_MAX_COALESCED_WINDOW: Duration = Duration::from_millis(4);

/// Public pure coalescing policy for session I/O output and metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIoCoalescingPolicy {
    /// Maximum bytes before output must flush.
    pub max_output_bytes: usize,
    /// Maximum frames before output must flush.
    pub max_output_frames: usize,
    /// Maximum elapsed window before output or metadata must flush.
    pub max_window: Duration,
}

impl Default for SessionIoCoalescingPolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: SESSION_IO_MAX_COALESCED_BYTES,
            max_output_frames: SESSION_IO_MAX_COALESCED_FRAMES,
            max_window: SESSION_IO_MAX_COALESCED_WINDOW,
        }
    }
}

impl SessionIoCoalescingPolicy {
    /// Build a coalescing policy.
    #[must_use]
    pub const fn new(
        max_output_bytes: usize,
        max_output_frames: usize,
        max_window: Duration,
    ) -> Self {
        Self {
            max_output_bytes,
            max_output_frames,
            max_window,
        }
    }

    /// Whether pending PTY output must flush.
    #[must_use]
    pub fn should_flush_output(self, bytes: usize, frames: usize, elapsed: Duration) -> bool {
        bytes >= self.max_output_bytes
            || frames >= self.max_output_frames
            || elapsed >= self.max_window
    }

    /// Whether pending metadata must flush because the coalescing window expired.
    #[must_use]
    pub fn metadata_age_expired(self, elapsed: Duration) -> bool {
        elapsed >= self.max_window
    }
}

/// Ordered session I/O events that require pending output to flush first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionIoOrderedEvent {
    /// Terminal title changed.
    TitleChanged,
    /// Terminal working directory changed.
    CwdChanged,
    /// Semantic prompt action detected.
    PromptMark,
    /// Bell character received.
    Bell,
    /// OSC notification detected.
    Notification,
    /// Child process exited.
    ProcessExited,
    /// Session output reached EOF.
    Eof,
    /// Session protocol stream desynchronized.
    Desynchronized,
    /// Session I/O is shutting down.
    Shutdown,
}

impl SessionIoOrderedEvent {
    /// Whether pending output must flush before this event is delivered.
    #[must_use]
    pub const fn requires_output_flush(self) -> bool {
        matches!(
            self,
            Self::TitleChanged
                | Self::CwdChanged
                | Self::PromptMark
                | Self::Bell
                | Self::Notification
                | Self::ProcessExited
                | Self::Eof
                | Self::Desynchronized
                | Self::Shutdown
        )
    }
}

/// Snapshot-before-live-output barrier for an attaching terminal stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialSnapshotBarrier {
    phase: InitialSnapshotPhase,
    pending_live_output: Vec<Vec<u8>>,
}

impl Default for InitialSnapshotBarrier {
    fn default() -> Self {
        Self::new()
    }
}

impl InitialSnapshotBarrier {
    /// Build a barrier waiting for the initial snapshot.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: InitialSnapshotPhase::WaitingForSnapshot,
            pending_live_output: Vec::new(),
        }
    }

    /// Current delivery phase.
    #[must_use]
    pub const fn phase(&self) -> InitialSnapshotPhase {
        self.phase
    }

    /// Record live output. Output is held until the initial snapshot arrives.
    #[must_use]
    pub fn push_live_output(&mut self, data: Vec<u8>) -> Option<Vec<u8>> {
        if self.phase == InitialSnapshotPhase::LiveOutputActive {
            Some(data)
        } else {
            self.pending_live_output.push(data);
            None
        }
    }

    /// Deliver the initial snapshot, followed by any held live output in order.
    #[must_use]
    pub fn deliver_initial_snapshot(
        &mut self,
        snapshot: InitialSnapshotReady,
    ) -> Vec<SessionIoEvent> {
        self.phase = InitialSnapshotPhase::LiveOutputActive;
        let session_id = snapshot.session_id.clone();
        let mut events = Vec::with_capacity(self.pending_live_output.len() + 1);
        events.push(SessionIoEvent::InitialSnapshotReady(snapshot));
        events.extend(self.pending_live_output.drain(..).map(|data| {
            SessionIoEvent::TerminalBytes {
                session_id: session_id.clone(),
                data,
            }
        }));
        events
    }
}

/// Stable plugin identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginKey(pub String);

/// Capability family for a plugin handler reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHandlerKind {
    /// UI action handler.
    UiAction,
    /// Session action handler.
    SessionAction,
    /// Command handler.
    Command,
    /// Hook handler.
    Hook,
    /// Surface route render handler.
    SurfaceRoute,
    /// Asset message handler.
    AssetMessage,
    /// Timer callback handler.
    Timer,
    /// MCP tool handler.
    McpTool,
    /// MCP prompt handler.
    McpPrompt,
    /// MCP resource handler.
    McpResource,
    /// MCP proxy auth-error recovery handler.
    McpProxyAuthError,
    /// Event handler.
    Event,
    /// HTTP callback handler.
    Http,
    /// File watch callback handler.
    Watch,
    /// ActionCable subscription callback handler.
    ActionCable,
    /// Entity provider handler.
    EntityProvider,
    /// Notification decision handler.
    Notification,
}

/// Stable plugin-owned handler reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginHandlerRef {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Handler capability family.
    pub kind: PluginHandlerKind,
    /// Stable handler id within the plugin.
    pub handler_id: String,
}

/// Plugin-owned descriptor family registered in a parent hub.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDescriptorKind {
    /// UI action descriptor.
    UiAction,
    /// Session action descriptor.
    SessionAction,
    /// Command descriptor.
    Command,
    /// Hook descriptor.
    Hook,
    /// Surface route descriptor.
    SurfaceRoute,
    /// Asset descriptor.
    Asset,
    /// Timer descriptor.
    Timer,
    /// MCP tool descriptor.
    McpTool,
    /// MCP prompt descriptor.
    McpPrompt,
    /// MCP resource descriptor.
    McpResource,
    /// Event subscription descriptor.
    Event,
    /// HTTP callback descriptor.
    Http,
    /// File watch descriptor.
    Watch,
    /// ActionCable subscription descriptor.
    ActionCable,
    /// Entity provider descriptor.
    EntityProvider,
    /// Notification descriptor.
    Notification,
}

/// Stable reference to a plugin-owned descriptor held by the parent hub.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginDescriptorRef {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Descriptor family.
    pub kind: PluginDescriptorKind,
    /// Stable descriptor id within the plugin.
    pub descriptor_id: String,
}

/// Plugin-owned descriptor body plus optional executable handler address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginOwnedDescriptor {
    /// Descriptor identity and owner.
    pub descriptor: PluginDescriptorRef,
    /// Executable handler address, when this descriptor invokes plugin code.
    pub handler: Option<PluginHandlerRef>,
    /// Plugin-owned descriptor body.
    pub body: BoundaryJson,
}

/// Plugin-owned runtime resource family tracked for cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginResourceKind {
    /// Timer resource.
    Timer,
    /// File watch resource.
    Watch,
    /// ActionCable subscription resource.
    ActionCableSubscription,
    /// Local webhook listener resource.
    LocalWebhook,
    /// In-flight HTTP request resource.
    HttpRequest,
    /// Persistent network connection resource.
    NetworkConnection,
    /// In-flight scoped filesystem operation resource.
    FilesystemOperation,
    /// In-flight plugin-store operation resource.
    PluginStoreOperation,
    /// MCP registration resource.
    McpRegistration,
    /// Entity provider resource.
    EntityProvider,
}

/// Stable reference to a plugin-owned runtime resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginResourceRef {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Resource family.
    pub kind: PluginResourceKind,
    /// Stable resource id within the plugin.
    pub resource_id: String,
}

/// Stable reference to a plugin-owned timer resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginTimerId(pub String);

/// Timer scheduling mode owned by core mechanics.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginTimerMode {
    /// Deliver once when due.
    OneShot,
    /// Deliver at a fixed interval after the first due time.
    Interval {
        /// Interval between due ticks in logical milliseconds.
        interval_ms: u64,
    },
    /// Replace prior pending work for the same plugin/key before delivery.
    Debounce {
        /// Plugin-scoped debounce key.
        key: String,
    },
}

/// Request to schedule plugin-owned timer work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginTimerSchedule {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Stable timer id within the plugin.
    pub timer_id: PluginTimerId,
    /// Timer callback handler. Must use [`PluginHandlerKind::Timer`].
    pub handler: PluginHandlerRef,
    /// Logical due time in milliseconds, supplied by the host.
    pub due_at_ms: u64,
    /// Timer mode.
    pub mode: PluginTimerMode,
    /// Runtime timeout in milliseconds for the plugin callback invocation.
    pub timeout_ms: u64,
    /// Serializable invocation context.
    pub context: PluginInvocationContext,
    /// Plugin-owned timer payload.
    pub payload: BoundaryJson,
}

/// Result of cancelling a plugin timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginTimerCancellationResult {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Plugin that owned the timer.
    pub plugin_key: PluginKey,
    /// Timer id requested for cancellation.
    pub timer_id: PluginTimerId,
    /// Whether a pending timer was removed.
    pub cancelled: bool,
    /// Runtime resource removed by cancellation.
    pub removed_resource: Option<PluginResourceRef>,
}

/// Typed scheduler event emitted by timer mechanics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginTimerEvent {
    /// Timer schedule request was rejected before entering scheduler state.
    Rejected {
        /// Request correlation id.
        request_id: RequestId,
        /// Timer that was rejected.
        timer_id: PluginTimerId,
        /// Plugin that would have owned the timer.
        plugin_key: PluginKey,
        /// Human-readable rejection reason.
        reason: String,
    },
    /// Timer was accepted into scheduler state.
    Scheduled {
        /// Request correlation id.
        request_id: RequestId,
        /// Runtime resource now owned by the scheduler.
        resource: PluginResourceRef,
    },
    /// A pending timer was cancelled or replaced.
    Cancelled {
        /// Request correlation id.
        request_id: RequestId,
        /// Runtime resource removed by cancellation.
        resource: PluginResourceRef,
        /// Human-readable cancellation reason.
        reason: String,
    },
    /// Timer callback was delivered through the plugin worker path.
    Fired {
        /// Timer that fired.
        timer_id: PluginTimerId,
        /// Invocation request id used for the callback.
        request_id: RequestId,
        /// Worker invocation result.
        result: PluginInvocationResult,
    },
    /// Repeatable timer work was coalesced instead of queued again.
    Coalesced {
        /// Timer whose due ticks were coalesced.
        timer_id: PluginTimerId,
        /// Owning plugin.
        plugin_key: PluginKey,
        /// Number of due ticks skipped by coalescing.
        skipped_ticks: u64,
        /// Typed pressure route for this repeatable work.
        route: BackpressureRoute,
    },
    /// Worker backpressure was observed while delivering a timer.
    Backpressured {
        /// Timer whose callback hit worker pressure.
        timer_id: PluginTimerId,
        /// Plugin-worker backpressure summary.
        summary: BackpressureSummary,
    },
}

/// Metadata needed to load a plugin worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLoadSpec {
    /// Plugin key.
    pub plugin_key: PluginKey,
    /// Manifest or package name.
    pub package: String,
    /// Entrypoint path or logical id.
    pub entrypoint: String,
    /// Plugin-owned descriptors exposed to the parent hub.
    pub descriptors: Vec<PluginOwnedDescriptor>,
    /// Plugin-owned load metadata.
    pub metadata: Option<BoundaryJson>,
}

/// Serializable context supplied to a plugin handler invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationContext {
    /// Client involved in the invocation, when client-scoped.
    pub client_id: Option<ClientId>,
    /// Session involved in the invocation, when session-scoped.
    pub session_id: Option<SessionId>,
    /// Subscription involved in the invocation, when stream-scoped.
    pub subscription_id: Option<SubscriptionId>,
    /// Surface route or node involved in the invocation, when UI-scoped.
    pub surface_id: Option<String>,
    /// Human-readable source of the invocation.
    pub origin: Option<String>,
    /// Plugin-owned contextual metadata.
    pub metadata: Option<BoundaryJson>,
}

/// Request to invoke a stable plugin-owned handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Handler to invoke.
    pub handler: PluginHandlerRef,
    /// Runtime timeout in milliseconds.
    pub timeout_ms: u64,
    /// Serializable invocation context.
    pub context: PluginInvocationContext,
    /// Plugin-owned invocation payload.
    pub payload: BoundaryJson,
}

/// Handler invocation failure category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInvocationFailureKind {
    /// Handler returned a failure.
    HandlerFailed,
    /// Handler exceeded the configured timeout.
    TimedOut,
    /// Invocation was cancelled by the runtime.
    Cancelled,
    /// Invocation was rejected because the worker queue was pressured.
    Backpressured,
    /// Worker stopped before the invocation completed.
    WorkerStopped,
}

/// Successful plugin handler invocation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationSuccess {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Handler that completed.
    pub handler: PluginHandlerRef,
    /// Plugin-owned response payload.
    pub payload: Option<BoundaryJson>,
}

/// Failed plugin handler invocation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationFailure {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Handler that failed.
    pub handler: PluginHandlerRef,
    /// Failure category.
    pub kind: PluginInvocationFailureKind,
    /// Timeout in milliseconds that applied to this invocation.
    pub timeout_ms: Option<u64>,
    /// Human-readable failure.
    pub reason: String,
}

/// Plugin handler invocation outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PluginInvocationResult {
    /// Invocation completed successfully.
    Completed(PluginInvocationSuccess),
    /// Invocation failed.
    Failed(PluginInvocationFailure),
}

/// Cleanup scope for plugin reload or unload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCleanupScope {
    /// Remove descriptor registrations only.
    Descriptors,
    /// Remove runtime resources only.
    Resources,
    /// Remove descriptor registrations and runtime resources.
    DescriptorsAndResources,
}

/// Request to reload one plugin worker and replace its owned registrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginReloadSpec {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Plugin being replaced.
    pub plugin_key: PluginKey,
    /// New load metadata for the replacement worker.
    pub load: PluginLoadSpec,
    /// Cleanup scope for the old worker's owned state.
    pub cleanup: PluginCleanupScope,
}

/// Request to unload one plugin worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginUnloadSpec {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Plugin being unloaded.
    pub plugin_key: PluginKey,
    /// Cleanup scope for the worker's owned state.
    pub cleanup: PluginCleanupScope,
}

/// Cleanup result for plugin reload or unload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCleanupResult {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Plugin whose owned state was cleaned.
    pub plugin_key: PluginKey,
    /// Descriptors removed by cleanup.
    pub removed_descriptors: Vec<PluginDescriptorRef>,
    /// Runtime resources removed by cleanup.
    pub removed_resources: Vec<PluginResourceRef>,
}

/// Messages accepted by a plugin worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginWorkerMessage {
    /// Load plugin metadata.
    Load {
        /// Request correlation id.
        request_id: RequestId,
        /// Load metadata.
        spec: PluginLoadSpec,
    },
    /// Invoke a stable plugin handler ref.
    Invoke(PluginInvocationRequest),
    /// Reload this plugin worker with replacement metadata.
    Reload(PluginReloadSpec),
    /// Unload this plugin worker and cleanup owned state.
    Unload(PluginUnloadSpec),
    /// Notify plugin worker of queue pressure.
    Backpressure(BackpressureSummary),
    /// Stop plugin worker.
    Shutdown {
        /// Request correlation id.
        request_id: RequestId,
        /// Owning plugin.
        plugin_key: PluginKey,
    },
}

/// Events emitted by a plugin worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginWorkerEvent {
    /// Plugin loaded successfully.
    Loaded {
        /// Request correlation id.
        request_id: RequestId,
        /// Loaded plugin.
        plugin_key: PluginKey,
        /// Registered handlers.
        handlers: Vec<PluginHandlerRef>,
        /// Registered descriptors exposed to the parent hub.
        descriptors: Vec<PluginDescriptorRef>,
    },
    /// Plugin reloaded successfully.
    Reloaded {
        /// Request correlation id.
        request_id: RequestId,
        /// Reloaded plugin.
        plugin_key: PluginKey,
        /// Cleanup performed before replacement.
        cleanup: PluginCleanupResult,
        /// Replacement descriptors exposed to the parent hub.
        descriptors: Vec<PluginDescriptorRef>,
    },
    /// Plugin unloaded successfully.
    Unloaded {
        /// Request correlation id.
        request_id: RequestId,
        /// Unloaded plugin.
        plugin_key: PluginKey,
        /// Cleanup performed during unload.
        cleanup: PluginCleanupResult,
    },
    /// Plugin failed to load or execute.
    Failed {
        /// Request correlation id.
        request_id: RequestId,
        /// Plugin that failed.
        plugin_key: PluginKey,
        /// Human-readable failure.
        reason: String,
    },
    /// Handler invocation completed.
    InvocationCompleted(PluginInvocationSuccess),
    /// Handler invocation failed. Timeout failures are reported exclusively by
    /// `InvocationTimedOut`, so this event must not carry `TimedOut`.
    InvocationFailed(PluginInvocationFailure),
    /// Handler invocation timed out.
    InvocationTimedOut(PluginInvocationFailure),
    /// Plugin worker observed queue pressure.
    Backpressure(BackpressureSummary),
    /// Plugin cleanup completed outside reload or unload.
    CleanupCompleted(PluginCleanupResult),
    /// Plugin worker stopped.
    Stopped {
        /// Plugin that stopped.
        plugin_key: PluginKey,
    },
}

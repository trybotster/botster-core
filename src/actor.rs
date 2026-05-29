//! Actor mailbox contract types shared by Botster runtime crates.

use serde::{Deserialize, Serialize};

use crate::boundary::BoundaryJson;
use crate::client::{ClientId, ClientState};
use crate::session::{RequestId, SessionId, SubscriptionId};

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
    /// No terminal stream is attached.
    Detached,
    /// Attach has been requested but initial data has not arrived.
    Attaching,
    /// Initial snapshot has arrived and live output may flow.
    Attached,
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

/// Terminal color profile summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalColorProfile {
    /// Dark terminal colors.
    Dark,
    /// Light terminal colors.
    Light,
    /// Runtime or client default.
    Default,
}

/// Terminal mode summary owned by session I/O.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalModeSummary {
    /// Whether focus reporting is active.
    pub focus_reporting: bool,
    /// Whether an alternate screen is active.
    pub alternate_screen: bool,
    /// Terminal color profile.
    pub color_profile: TerminalColorProfile,
}

/// Error reasons for paste-file preparation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteFileErrorReason {
    /// Payload exceeded the runtime limit.
    TooLarge,
    /// Runtime could not prepare storage.
    StorageUnavailable,
    /// Payload was not valid for paste handling.
    InvalidPayload,
}

/// Prepared terminal snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSnapshot {
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

/// Requests accepted by a session I/O worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionIoRequest {
    /// Subscribe a client worker to terminal output.
    Subscribe {
        /// Request correlation id.
        request_id: RequestId,
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
    Unsubscribe {
        /// Subscription identity.
        subscription_id: SubscriptionId,
    },
    /// Write terminal input bytes.
    Input {
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Resize the terminal.
    Resize {
        /// New row count.
        rows: u16,
        /// New column count.
        cols: u16,
    },
    /// Request a snapshot.
    Snapshot {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Prepare a paste payload.
    Paste {
        /// Request correlation id.
        request_id: RequestId,
        /// Paste bytes.
        data: Vec<u8>,
    },
    /// Set focus state.
    Focus {
        /// Whether the client is focused.
        focused: bool,
    },
    /// Stop the session I/O worker.
    Shutdown {
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
    /// Initial or requested snapshot.
    Snapshot(PreparedSnapshot),
    /// Terminal mode changed.
    ModeChanged {
        /// Session whose mode changed.
        session_id: SessionId,
        /// New mode summary.
        mode: TerminalModeSummary,
    },
    /// Focus state changed.
    FocusChanged {
        /// Session whose focus changed.
        session_id: SessionId,
        /// Whether focus is active.
        focused: bool,
    },
    /// Paste preparation failed.
    PasteFailed {
        /// Request correlation id.
        request_id: RequestId,
        /// Failure reason.
        reason: PasteFileErrorReason,
    },
    /// Session process exited.
    ProcessExited {
        /// Session that exited.
        session_id: SessionId,
        /// Process exit code.
        code: Option<i32>,
    },
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
    /// MCP handler.
    Mcp,
    /// Event handler.
    Event,
    /// HTTP callback handler.
    Http,
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

/// Metadata needed to load a plugin worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLoadSpec {
    /// Plugin key.
    pub plugin_key: PluginKey,
    /// Manifest or package name.
    pub package: String,
    /// Entrypoint path or logical id.
    pub entrypoint: String,
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
    Invoke {
        /// Request correlation id.
        request_id: RequestId,
        /// Handler to invoke.
        handler: PluginHandlerRef,
        /// Plugin-owned payload.
        payload: BoundaryJson,
    },
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
    Completed {
        /// Request correlation id.
        request_id: RequestId,
        /// Handler that completed.
        handler: PluginHandlerRef,
        /// Plugin-owned response payload.
        payload: Option<BoundaryJson>,
    },
    /// Plugin worker observed queue pressure.
    Backpressure(BackpressureSummary),
    /// Plugin worker stopped.
    Stopped {
        /// Plugin that stopped.
        plugin_key: PluginKey,
    },
}

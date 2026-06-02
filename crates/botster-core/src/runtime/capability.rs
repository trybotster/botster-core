//! Capability-scoped plugin runtime contracts.
//!
//! Core only defines the request, handle, event, and cleanup shapes for
//! non-blocking plugin capability I/O. Host profiles provide concrete HTTP,
//! WebSocket, filesystem, store, watcher, and timer implementations behind a
//! bounded mailbox.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginCleanupResult, PluginHandlerRef, PluginKey,
    PluginResourceKind, PluginResourceRef, QueueSource,
};
use crate::package::{Capability, CapabilitySurface};

/// Stable identifier for one submitted capability operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityOperationId(pub String);

/// Stable identifier for one runtime-owned capability resource.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CapabilityResourceId(pub String);

/// Request submitted by plugin code to a host-provided capability runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRuntimeRequest {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Stable operation identifier assigned before enqueue.
    pub operation_id: CapabilityOperationId,
    /// Requested capability operation.
    pub operation: CapabilityOperation,
    /// Timeout budget in milliseconds for operation completion or first handle.
    pub timeout_ms: u64,
    /// Optional plugin handler for completion or inbound events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback: Option<PluginHandlerRef>,
}

impl CapabilityRuntimeRequest {
    /// Capability required before a host accepts this request.
    #[must_use]
    pub fn required_capability(&self) -> Capability {
        self.operation.required_capability()
    }

    /// Resource kind created or touched by this request.
    #[must_use]
    pub const fn resource_kind(&self) -> PluginResourceKind {
        self.operation.resource_kind()
    }

    /// Build the plugin-scoped resource reference for this request.
    #[must_use]
    pub fn resource_ref(&self, resource_id: CapabilityResourceId) -> PluginResourceRef {
        PluginResourceRef {
            plugin_key: self.plugin_key.clone(),
            kind: self.resource_kind(),
            resource_id: resource_id.0,
        }
    }

    /// Backpressure report for the bounded runtime mailbox that accepted this request family.
    #[must_use]
    pub fn backpressure(&self, capacity: usize, depth: usize) -> BackpressureSummary {
        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity,
            depth,
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(self.plugin_key.clone()),
            },
        }
    }
}

/// Capability operation families supported by the runtime contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityOperation {
    /// Outbound HTTP request.
    Http(HttpCapabilityRequest),
    /// WebSocket connection, send, or close request.
    WebSocket(WebSocketCapabilityRequest),
    /// File watch registration or removal request.
    Watch(WatchCapabilityRequest),
    /// Scoped filesystem operation.
    Filesystem(FilesystemCapabilityRequest),
    /// Plugin-scoped JSON store operation.
    PluginStore(PluginStoreCapabilityRequest),
    /// Timer registration or cancellation request.
    Timer(TimerCapabilityRequest),
}

impl CapabilityOperation {
    /// Capability required before a host accepts this operation.
    #[must_use]
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::Http(_) => scoped_capability(CapabilitySurface::Network, "http"),
            Self::WebSocket(_) => scoped_capability(CapabilitySurface::Network, "websocket"),
            Self::Watch(request) => {
                scoped_capability(CapabilitySurface::Filesystem, request.scope())
            }
            Self::Filesystem(request) => {
                scoped_capability(CapabilitySurface::Filesystem, request.scope_id.clone())
            }
            Self::PluginStore(request) => {
                scoped_capability(CapabilitySurface::PluginDb, request.namespace.clone())
            }
            Self::Timer(_) => scoped_capability(CapabilitySurface::Timers, "callbacks"),
        }
    }

    /// Resource kind created or touched by this operation.
    #[must_use]
    pub const fn resource_kind(&self) -> PluginResourceKind {
        match self {
            Self::Http(_) => PluginResourceKind::HttpRequest,
            Self::WebSocket(_) => PluginResourceKind::NetworkConnection,
            Self::Watch(_) => PluginResourceKind::Watch,
            Self::Filesystem(_) => PluginResourceKind::FilesystemOperation,
            Self::PluginStore(_) => PluginResourceKind::PluginStoreOperation,
            Self::Timer(_) => PluginResourceKind::Timer,
        }
    }
}

/// Outbound HTTP request metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCapabilityRequest {
    /// HTTP method such as `GET` or `POST`.
    pub method: String,
    /// Host-profile-resolved URL or endpoint key.
    pub endpoint: String,
    /// Request headers selected by the host or plugin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<HttpHeader>,
    /// Optional opaque request body bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<u8>,
}

/// HTTP header pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: String,
}

/// WebSocket operation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WebSocketCapabilityRequest {
    /// Open a WebSocket connection.
    Connect {
        /// Host-profile-resolved URL or endpoint key.
        endpoint: String,
        /// Optional subprotocol names.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        protocols: Vec<String>,
    },
    /// Send a message to an existing WebSocket resource.
    Send {
        /// Target WebSocket resource.
        resource_id: CapabilityResourceId,
        /// Message body.
        message: WebSocketMessage,
    },
    /// Close an existing WebSocket resource.
    Close {
        /// Target WebSocket resource.
        resource_id: CapabilityResourceId,
        /// Optional close code.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<u16>,
        /// Optional close reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// WebSocket message body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// Text message.
    Text(String),
    /// Binary message.
    Binary(Vec<u8>),
}

/// File watch operation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WatchCapabilityRequest {
    /// Register a watch below a host-owned filesystem scope.
    Register {
        /// Opaque host-owned filesystem scope id.
        scope_id: String,
        /// Relative path below the scope.
        path: ScopedRelativePath,
        /// Include recursive descendants.
        recursive: bool,
    },
    /// Remove a previously registered watch.
    Unregister {
        /// Opaque host-owned filesystem scope id.
        scope_id: String,
        /// Target watch resource.
        resource_id: CapabilityResourceId,
    },
}

impl WatchCapabilityRequest {
    fn scope(&self) -> String {
        match self {
            Self::Register { scope_id, .. } | Self::Unregister { scope_id, .. } => scope_id.clone(),
        }
    }
}

/// Scoped filesystem operation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemCapabilityRequest {
    /// Opaque host-owned filesystem scope id.
    pub scope_id: String,
    /// Filesystem operation below the scope.
    pub operation: FilesystemOperation,
    /// Optional operation limits requested by plugin code or injected by the host profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<FilesystemCapabilityLimits>,
}

/// Relative path within a host-owned filesystem scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScopedRelativePath(pub String);

impl ScopedRelativePath {
    /// Whether the path is relative and does not contain parent-directory traversal.
    #[must_use]
    pub fn is_scoped_relative(&self) -> bool {
        let path = self.0.as_str();
        !path.is_empty()
            && !path.starts_with('/')
            && !path.starts_with('\\')
            && !has_windows_drive_prefix(path)
            && !path.split('/').any(|segment| segment == "..")
            && !path.split('\\').any(|segment| segment == "..")
    }
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Host/profile grant for one scoped filesystem root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemCapabilityGrant {
    /// Opaque host-owned filesystem scope id.
    pub scope_id: String,
    /// Operations allowed within this scope.
    pub permissions: FilesystemCapabilityPermissions,
    /// Default limits applied by the host profile for this scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<FilesystemCapabilityLimits>,
}

/// Allowed scoped filesystem operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemCapabilityPermissions {
    /// Read file bytes.
    pub read: bool,
    /// Write file bytes.
    pub write: bool,
    /// List child entries.
    pub list: bool,
    /// Read metadata.
    pub stat: bool,
    /// Remove files or empty directories.
    pub remove: bool,
}

impl FilesystemCapabilityPermissions {
    /// Whether this grant permits the requested scoped filesystem operation.
    #[must_use]
    pub const fn allows(&self, operation: &FilesystemOperation) -> bool {
        match operation {
            FilesystemOperation::Read { .. } => self.read,
            FilesystemOperation::Write { .. } => self.write,
            FilesystemOperation::List { .. } => self.list,
            FilesystemOperation::Stat { .. } => self.stat,
            FilesystemOperation::Remove { .. } => self.remove,
        }
    }
}

/// Host/profile limit contract for scoped filesystem operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemCapabilityLimits {
    /// Maximum bytes a read operation may return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_read_bytes: Option<u64>,
    /// Maximum bytes a write operation may accept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_write_bytes: Option<u64>,
    /// Maximum child entries a list operation may return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_list_entries: Option<u64>,
}

/// Scoped filesystem operations named by core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FilesystemOperation {
    /// Read file bytes.
    Read {
        /// Target relative path.
        path: ScopedRelativePath,
    },
    /// Write file bytes.
    Write {
        /// Target relative path.
        path: ScopedRelativePath,
        /// Bytes to write.
        bytes: Vec<u8>,
    },
    /// List child entries.
    List {
        /// Target relative path.
        path: ScopedRelativePath,
    },
    /// Return file metadata.
    Stat {
        /// Target relative path.
        path: ScopedRelativePath,
    },
    /// Remove a file or empty directory.
    Remove {
        /// Target relative path.
        path: ScopedRelativePath,
    },
}

impl FilesystemOperation {
    /// Target relative path for this scoped filesystem operation.
    #[must_use]
    pub const fn path(&self) -> &ScopedRelativePath {
        match self {
            Self::Read { path }
            | Self::Write { path, .. }
            | Self::List { path }
            | Self::Stat { path }
            | Self::Remove { path } => path,
        }
    }
}

/// Plugin-scoped JSON store request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStoreCapabilityRequest {
    /// Host-owned plugin-store namespace.
    pub namespace: String,
    /// Store operation.
    pub operation: PluginStoreOperation,
}

/// Plugin store operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PluginStoreOperation {
    /// Get one JSON value.
    Get {
        /// Target key.
        key: PluginStoreKey,
    },
    /// Set one JSON value.
    Set {
        /// Target key.
        key: PluginStoreKey,
        /// JSON value.
        value: serde_json::Value,
    },
    /// Remove one JSON value.
    Delete {
        /// Target key.
        key: PluginStoreKey,
    },
    /// List keys with an optional prefix.
    List {
        /// Optional key prefix.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
    },
}

/// Stable plugin-store key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluginStoreKey(pub String);

/// Timer registration or cancellation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TimerCapabilityRequest {
    /// Fire once after a delay.
    Once {
        /// Delay in milliseconds.
        delay_ms: u64,
    },
    /// Fire repeatedly at a fixed interval.
    Interval {
        /// Interval in milliseconds.
        interval_ms: u64,
    },
    /// Cancel a timer resource.
    Cancel {
        /// Target timer resource.
        resource_id: CapabilityResourceId,
    },
}

/// Handle returned after a capability request is accepted by the runtime mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRuntimeHandle {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Accepted operation id.
    pub operation_id: CapabilityOperationId,
    /// Runtime resource created or touched by the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<PluginResourceRef>,
    /// Capability checked before acceptance.
    pub required_capability: Capability,
}

/// Event emitted by the host capability runtime after request acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityRuntimeEvent {
    /// One operation completed successfully.
    Completed(CapabilityOperationCompleted),
    /// One runtime resource was opened.
    ResourceOpened(CapabilityResourceEvent),
    /// One runtime resource was released.
    ResourceReleased(CapabilityResourceEvent),
    /// Inbound WebSocket message.
    WebSocketMessage(CapabilityWebSocketEvent),
    /// File watch notification.
    Watch(CapabilityWatchEvent),
    /// Timer fired.
    TimerFired(CapabilityTimerEvent),
    /// Operation timed out.
    TimedOut(CapabilityOperationFailure),
    /// Operation was cancelled.
    Cancelled(CapabilityOperationFailure),
    /// Operation failed.
    Failed(CapabilityOperationFailure),
    /// Bounded runtime mailbox reported pressure.
    Backpressure(BackpressureSummary),
    /// Cleanup completed for one plugin.
    CleanupCompleted(PluginCleanupResult),
}

/// Successful operation completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOperationCompleted {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Completed operation id.
    pub operation_id: CapabilityOperationId,
    /// Optional typed operation result payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapabilityOperationResult>,
}

/// Typed successful operation result payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "result", rename_all = "snake_case")]
pub enum CapabilityOperationResult {
    /// HTTP response metadata and bytes.
    Http(HttpCapabilityResponse),
    /// Scoped filesystem result payload.
    Filesystem(FilesystemCapabilityResult),
}

/// HTTP response metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCapabilityResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<HttpHeader>,
    /// Response body bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<u8>,
}

/// Successful scoped filesystem operation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FilesystemCapabilityResult {
    /// Read file bytes.
    Read {
        /// Target relative path.
        path: ScopedRelativePath,
        /// Returned bytes.
        bytes: Vec<u8>,
    },
    /// Wrote file bytes.
    Write {
        /// Target relative path.
        path: ScopedRelativePath,
        /// Number of bytes accepted by the host runtime.
        bytes_written: u64,
        /// Whether the host runtime completed the write through its atomic-write path.
        atomic: bool,
    },
    /// Listed child entries.
    List {
        /// Target relative path.
        path: ScopedRelativePath,
        /// Returned child entries.
        entries: Vec<FilesystemEntry>,
    },
    /// Returned file metadata.
    Stat {
        /// Target relative path.
        path: ScopedRelativePath,
        /// Returned metadata.
        metadata: FilesystemMetadata,
    },
    /// Removed a file or empty directory.
    Remove {
        /// Target relative path.
        path: ScopedRelativePath,
    },
}

/// One scoped filesystem list entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemEntry {
    /// Entry path relative to the granted scope.
    pub path: ScopedRelativePath,
    /// Entry type.
    pub kind: FilesystemEntryKind,
    /// Size in bytes when the host exposes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// Scoped filesystem entry kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemEntryKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symbolic link.
    Symlink,
    /// Other host-specific file type.
    Other,
}

/// Scoped filesystem metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemMetadata {
    /// File type.
    pub kind: FilesystemEntryKind,
    /// Size in bytes when the host exposes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Whether host metadata reports the entry as readonly.
    pub readonly: bool,
}

/// Runtime resource lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityResourceEvent {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Operation that opened or released the resource.
    pub operation_id: CapabilityOperationId,
    /// Runtime resource.
    pub resource: PluginResourceRef,
}

/// Inbound WebSocket event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWebSocketEvent {
    /// Runtime resource.
    pub resource: PluginResourceRef,
    /// Message body.
    pub message: WebSocketMessage,
}

/// File watch event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWatchEvent {
    /// Runtime resource.
    pub resource: PluginResourceRef,
    /// Path affected inside the watched scope.
    pub path: ScopedRelativePath,
    /// Stable watch event kind.
    pub change: WatchChangeKind,
}

/// Stable watch event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchChangeKind {
    /// File or directory was created.
    Created,
    /// File or directory changed.
    Modified,
    /// File or directory was removed.
    Removed,
    /// Watch backend reported an overflow or lost events.
    Overflow,
}

/// Timer fired event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityTimerEvent {
    /// Runtime resource.
    pub resource: PluginResourceRef,
    /// Monotonic firing sequence for repeated timers.
    pub sequence: u64,
}

/// Failure, timeout, or cancellation event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOperationFailure {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Failed operation id.
    pub operation_id: CapabilityOperationId,
    /// Stable failure kind.
    pub error_kind: CapabilityRuntimeErrorKind,
    /// Human-readable failure reason.
    pub reason: String,
}

/// Host-implemented, non-blocking capability runtime boundary.
pub trait PluginCapabilityRuntime {
    /// Enqueue one operation request without performing blocking I/O inline.
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError>;

    /// Request cancellation for an operation owned by one plugin.
    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError>;

    /// Release one runtime resource.
    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError>;

    /// Drain currently available events for one plugin.
    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError>;

    /// Stop and release all runtime resources owned by one plugin.
    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError>;
}

/// Stable capability runtime error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRuntimeErrorKind {
    /// The bounded request queue was full.
    Backpressured,
    /// The plugin does not have the required capability.
    CapabilityDenied,
    /// The operation id is unknown to the runtime.
    OperationNotFound,
    /// The resource id is unknown to the runtime.
    ResourceNotFound,
    /// The operation exceeded its timeout.
    TimedOut,
    /// Cancellation was requested.
    Cancelled,
    /// The runtime stopped before completion.
    RuntimeStopped,
    /// The request was invalid for its operation family.
    InvalidRequest,
}

/// Typed error returned by a capability runtime implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRuntimeError {
    /// Stable machine-readable error kind.
    pub kind: CapabilityRuntimeErrorKind,
    /// Human-readable error detail.
    pub message: String,
}

impl CapabilityRuntimeError {
    /// Build a typed runtime error.
    #[must_use]
    pub fn new(kind: CapabilityRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for CapabilityRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for CapabilityRuntimeError {}

fn scoped_capability(surface: CapabilitySurface, scope: impl Into<String>) -> Capability {
    Capability {
        surface,
        scope: Some(scope.into()),
    }
}

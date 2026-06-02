//! Capability-scoped plugin runtime contracts.
//!
//! Core defines the request, handle, event, and cleanup shapes for
//! non-blocking plugin capability I/O. The watch family also has a core-owned
//! runtime mechanism for registration state, scoped-path validation,
//! debounce/coalescing, bounded delivery, and cleanup over a host-provided event
//! source. Host profiles still provide concrete HTTP, WebSocket, filesystem,
//! store, timer, and OS watcher adapters behind bounded mailboxes.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::PluginCancellationToken;
use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginCleanupResult, PluginHandlerRef, PluginKey,
    PluginResourceKind, PluginResourceRef, QueueSource,
};
use crate::package::{Capability, CapabilitySet, CapabilitySurface};

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

/// Host-owned endpoint policy for HTTP capability requests.
///
/// Core validates against these supplied policy values, but does not choose
/// default grants, trusted hosts, credentials, redirects, cookies, or retries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCapabilityEndpointPolicy {
    /// Allowed URL schemes such as `http` or `https`.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub allowed_schemes: BTreeSet<String>,
    /// Allowed lower-case host names.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub allowed_hosts: BTreeSet<String>,
}

impl HttpCapabilityEndpointPolicy {
    /// Build a policy from explicit scheme and host allowlists.
    #[must_use]
    pub fn new(
        schemes: impl IntoIterator<Item = impl Into<String>>,
        hosts: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            allowed_schemes: schemes
                .into_iter()
                .map(|scheme| scheme.into().to_ascii_lowercase())
                .collect(),
            allowed_hosts: hosts
                .into_iter()
                .map(|host| host.into().to_ascii_lowercase())
                .collect(),
        }
    }
}

/// Bounded HTTP runtime settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCapabilityRuntimeConfig {
    /// Maximum concurrently accepted HTTP operations.
    pub request_capacity: usize,
    /// Maximum request body bytes accepted before enqueue.
    pub max_request_body_bytes: usize,
    /// Maximum response body bytes retained while collecting transport chunks.
    pub max_response_body_bytes: usize,
    /// Maximum request or response header count.
    pub max_header_count: usize,
    /// Maximum header name length in bytes.
    pub max_header_name_bytes: usize,
    /// Maximum header value length in bytes.
    pub max_header_value_bytes: usize,
}

impl Default for HttpCapabilityRuntimeConfig {
    fn default() -> Self {
        Self {
            request_capacity: 64,
            max_request_body_bytes: 1024 * 1024,
            max_response_body_bytes: 4 * 1024 * 1024,
            max_header_count: 64,
            max_header_name_bytes: 128,
            max_header_value_bytes: 8192,
        }
    }
}

/// Request handed to a host-implemented HTTP transport after core validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTransportRequest {
    /// Original capability runtime request.
    pub runtime_request: CapabilityRuntimeRequest,
    /// Parsed URL scheme.
    pub scheme: String,
    /// Parsed URL host.
    pub host: String,
    /// Runtime response body byte limit.
    pub max_response_body_bytes: usize,
    /// Runtime response header count limit.
    pub max_header_count: usize,
    /// Runtime response header name byte limit.
    pub max_header_name_bytes: usize,
    /// Runtime response header value byte limit.
    pub max_header_value_bytes: usize,
}

/// Host-implemented HTTP transport boundary.
///
/// Implementors perform concrete HTTP I/O outside core. They must poll the
/// provided cancellation token while blocking or collecting response chunks.
pub trait HttpCapabilityTransport: Send + Sync + 'static {
    /// Execute one already-authorized HTTP request.
    fn execute(
        &self,
        request: HttpTransportRequest,
        cancellation: PluginCancellationToken,
    ) -> Result<HttpCapabilityResponse, CapabilityRuntimeError>;
}

/// Non-blocking HTTP implementation of [`PluginCapabilityRuntime`].
///
/// `submit` performs only validation, bounded admission, and worker-thread
/// dispatch. Transport I/O runs on runtime-owned background threads and is
/// observed later through `drain_events`.
pub struct HttpCapabilityRuntime {
    grants: CapabilitySet,
    endpoint_policy: HttpCapabilityEndpointPolicy,
    config: HttpCapabilityRuntimeConfig,
    transport: Arc<dyn HttpCapabilityTransport>,
    in_flight: HashMap<(PluginKey, CapabilityOperationId), HttpInFlightOperation>,
    pending_events: VecDeque<CapabilityRuntimeEvent>,
    completions_sender: mpsc::Sender<HttpWorkerCompletion>,
    completions_receiver: mpsc::Receiver<HttpWorkerCompletion>,
}

impl HttpCapabilityRuntime {
    /// Build a runtime over a host-implemented HTTP transport.
    #[must_use]
    pub fn new(
        grants: CapabilitySet,
        endpoint_policy: HttpCapabilityEndpointPolicy,
        config: HttpCapabilityRuntimeConfig,
        transport: Arc<dyn HttpCapabilityTransport>,
    ) -> Self {
        let (completions_sender, completions_receiver) = mpsc::channel();
        Self {
            grants,
            endpoint_policy,
            config,
            transport,
            in_flight: HashMap::new(),
            pending_events: VecDeque::new(),
            completions_sender,
            completions_receiver,
        }
    }

    fn drain_worker_completions(&mut self) {
        while let Ok(completion) = self.completions_receiver.try_recv() {
            let key = (
                completion.plugin_key.clone(),
                completion.operation_id.clone(),
            );
            if self.in_flight.remove(&key).is_none() {
                continue;
            }

            let event = match completion.result {
                Ok(response) => CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
                    plugin_key: completion.plugin_key,
                    operation_id: completion.operation_id,
                    response: Some(response),
                    plugin_store: None,
                }),
                Err(error) if error.kind == CapabilityRuntimeErrorKind::Cancelled => {
                    CapabilityRuntimeEvent::Cancelled(CapabilityOperationFailure {
                        plugin_key: completion.plugin_key,
                        operation_id: completion.operation_id,
                        error_kind: error.kind,
                        reason: error.message,
                    })
                }
                Err(error) if error.kind == CapabilityRuntimeErrorKind::TimedOut => {
                    CapabilityRuntimeEvent::TimedOut(CapabilityOperationFailure {
                        plugin_key: completion.plugin_key,
                        operation_id: completion.operation_id,
                        error_kind: error.kind,
                        reason: error.message,
                    })
                }
                Err(error) => CapabilityRuntimeEvent::Failed(CapabilityOperationFailure {
                    plugin_key: completion.plugin_key,
                    operation_id: completion.operation_id,
                    error_kind: error.kind,
                    reason: error.message,
                }),
            };
            self.pending_events.push_back(event);
        }
    }

    fn cancel_expired(&mut self) {
        let expired = self
            .in_flight
            .iter()
            .filter_map(|(key, operation)| {
                if operation.started_at.elapsed() >= operation.timeout {
                    Some((key.clone(), operation.cancellation.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for ((plugin_key, operation_id), cancellation) in expired {
            cancellation.cancel();
            self.in_flight
                .remove(&(plugin_key.clone(), operation_id.clone()));
            self.pending_events
                .push_back(CapabilityRuntimeEvent::TimedOut(
                    CapabilityOperationFailure {
                        plugin_key,
                        operation_id,
                        error_kind: CapabilityRuntimeErrorKind::TimedOut,
                        reason: "operation exceeded timeout".to_string(),
                    },
                ));
        }
    }

    fn validate_http_request(
        &self,
        request: &CapabilityRuntimeRequest,
    ) -> Result<HttpValidatedEndpoint, CapabilityRuntimeError> {
        if !self.grants.contains(&request.required_capability()) {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::CapabilityDenied,
                "plugin lacks network:http capability",
            ));
        }

        if request.timeout_ms == 0 {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "timeout_ms must be greater than zero",
            ));
        }

        let CapabilityOperation::Http(http) = &request.operation else {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP runtime only accepts HTTP capability operations",
            ));
        };

        if http.method.trim().is_empty() {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP method is required",
            ));
        }
        if http.body.len() > self.config.max_request_body_bytes {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP request body exceeds configured limit",
            ));
        }
        validate_headers(
            &http.headers,
            self.config.max_header_count,
            self.config.max_header_name_bytes,
            self.config.max_header_value_bytes,
        )?;

        let endpoint = parse_http_endpoint(&http.endpoint)?;
        if !self
            .endpoint_policy
            .allowed_schemes
            .contains(endpoint.scheme.as_str())
        {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::CapabilityDenied,
                "HTTP URL scheme is not allowed",
            ));
        }
        if !self
            .endpoint_policy
            .allowed_hosts
            .contains(endpoint.host.as_str())
        {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::CapabilityDenied,
                "HTTP host is not allowed",
            ));
        }

        Ok(endpoint)
    }

    /// Validate response headers and body size against this runtime config.
    ///
    /// Host transports should call this before returning a response. Transports
    /// that collect chunked bodies should enforce `max_response_body_bytes`
    /// incrementally while collecting chunks.
    pub fn validate_response(
        config: &HttpCapabilityRuntimeConfig,
        response: &HttpCapabilityResponse,
    ) -> Result<(), CapabilityRuntimeError> {
        validate_headers(
            &response.headers,
            config.max_header_count,
            config.max_header_name_bytes,
            config.max_header_value_bytes,
        )?;
        if response.body.len() > config.max_response_body_bytes {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP response body exceeds configured limit",
            ));
        }
        Ok(())
    }
}

impl PluginCapabilityRuntime for HttpCapabilityRuntime {
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        self.drain_worker_completions();
        self.cancel_expired();

        let endpoint = self.validate_http_request(&request)?;
        if self.in_flight.len() >= self.config.request_capacity {
            self.pending_events
                .push_back(CapabilityRuntimeEvent::Backpressure(
                    request.backpressure(self.config.request_capacity, self.in_flight.len()),
                ));
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::Backpressured,
                "HTTP capability runtime request capacity reached",
            ));
        }

        let plugin_key = request.plugin_key.clone();
        let operation_id = request.operation_id.clone();
        let resource = request.resource_ref(CapabilityResourceId(operation_id.0.clone()));
        let cancellation = PluginCancellationToken::new();
        let transport_request = HttpTransportRequest {
            runtime_request: request.clone(),
            scheme: endpoint.scheme,
            host: endpoint.host,
            max_response_body_bytes: self.config.max_response_body_bytes,
            max_header_count: self.config.max_header_count,
            max_header_name_bytes: self.config.max_header_name_bytes,
            max_header_value_bytes: self.config.max_header_value_bytes,
        };
        self.in_flight.insert(
            (plugin_key.clone(), operation_id.clone()),
            HttpInFlightOperation {
                resource: resource.clone(),
                cancellation: cancellation.clone(),
                started_at: Instant::now(),
                timeout: Duration::from_millis(request.timeout_ms),
            },
        );

        let transport = self.transport.clone();
        let completions_sender = self.completions_sender.clone();
        let worker_plugin_key = plugin_key.clone();
        let worker_operation_id = operation_id.clone();
        std::thread::Builder::new()
            .name("botster-http-capability-runtime".to_string())
            .spawn(move || {
                let result = transport.execute(transport_request, cancellation);
                let _ = completions_sender.send(HttpWorkerCompletion {
                    plugin_key: worker_plugin_key,
                    operation_id: worker_operation_id,
                    result,
                });
            })
            .map_err(|error| {
                self.in_flight
                    .remove(&(plugin_key.clone(), operation_id.clone()));
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::RuntimeStopped,
                    format!("failed to start HTTP capability worker: {error}"),
                )
            })?;

        Ok(CapabilityRuntimeHandle {
            plugin_key,
            operation_id,
            resource: Some(resource),
            required_capability: request.required_capability(),
        })
    }

    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        self.drain_worker_completions();
        let key = (plugin_key.clone(), operation_id.clone());
        let Some(operation) = self.in_flight.remove(&key) else {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::OperationNotFound,
                "HTTP operation is not in flight",
            ));
        };
        operation.cancellation.cancel();
        self.pending_events
            .push_back(CapabilityRuntimeEvent::Cancelled(
                CapabilityOperationFailure {
                    plugin_key: plugin_key.clone(),
                    operation_id: operation_id.clone(),
                    error_kind: CapabilityRuntimeErrorKind::Cancelled,
                    reason: "operation cancelled".to_string(),
                },
            ));
        Ok(())
    }

    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError> {
        self.cancel(
            &resource.plugin_key,
            &CapabilityOperationId(resource.resource_id.clone()),
        )
    }

    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError> {
        self.drain_worker_completions();
        self.cancel_expired();

        let mut events = Vec::new();
        let mut retained = VecDeque::new();
        while let Some(event) = self.pending_events.pop_front() {
            if event_plugin_key(&event).as_ref() == Some(plugin_key) {
                events.push(event);
            } else {
                retained.push_back(event);
            }
        }
        self.pending_events = retained;
        Ok(events)
    }

    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError> {
        self.drain_worker_completions();
        let keys = self
            .in_flight
            .keys()
            .filter(|(operation_plugin_key, _)| operation_plugin_key == plugin_key)
            .cloned()
            .collect::<Vec<_>>();
        let mut removed_resources = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(operation) = self.in_flight.remove(&key) {
                operation.cancellation.cancel();
                removed_resources.push(operation.resource);
            }
        }

        self.pending_events
            .retain(|event| event_plugin_key(event).as_ref() != Some(plugin_key));

        Ok(PluginCleanupResult {
            request_id: crate::session::RequestId(format!("capability-cleanup-{}", plugin_key.0)),
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources,
        })
    }
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
            && !path.split('/').any(|segment| segment == "..")
            && !path.split('\\').any(|segment| segment == "..")
    }
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
    /// Get one JSON record.
    Get {
        /// Target key.
        key: PluginStoreKey,
    },
    /// Set one JSON record.
    Set {
        /// Target key.
        key: PluginStoreKey,
        /// Schema version declared by the plugin for this payload.
        schema_version: u64,
        /// Plugin-owned JSON payload.
        payload: serde_json::Value,
        /// Optional compare-and-swap guard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_revision: Option<u64>,
    },
    /// Remove one JSON record.
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
    /// Apply an RFC 7396-style merge patch to one JSON record.
    Patch {
        /// Target key.
        key: PluginStoreKey,
        /// Merge-patch object applied to the plugin-owned payload.
        patch: serde_json::Value,
        /// Optional compare-and-swap guard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_revision: Option<u64>,
    },
}

/// Stable plugin-store key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluginStoreKey(pub String);

impl PluginStoreKey {
    /// Maximum key length in bytes.
    pub const MAX_BYTES: usize = 256;

    /// Whether this key is non-empty, bounded, and does not contain path traversal.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let key = self.0.as_str();
        !key.is_empty()
            && key.len() <= Self::MAX_BYTES
            && !key.starts_with('/')
            && !key.starts_with('\\')
            && !key.split('/').any(|segment| segment == "..")
            && !key.split('\\').any(|segment| segment == "..")
    }
}

/// Plugin-store write limits enforced by core before accepting mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStoreLimits {
    /// Maximum serialized JSON bytes for one payload.
    pub max_record_bytes: usize,
    /// Maximum number of records one plugin namespace may hold.
    pub max_plugin_keys: usize,
    /// Maximum aggregate serialized JSON payload bytes for one plugin namespace.
    pub max_plugin_bytes: usize,
}

impl Default for PluginStoreLimits {
    fn default() -> Self {
        Self {
            max_record_bytes: 64 * 1024,
            max_plugin_keys: 1_024,
            max_plugin_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Stored JSON record envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStoreRecord {
    /// Owning plugin namespace.
    pub plugin_key: PluginKey,
    /// Record key inside the plugin namespace.
    pub key: PluginStoreKey,
    /// Plugin-declared schema version.
    pub schema_version: u64,
    /// Monotonic record revision.
    pub revision: u64,
    /// Plugin-owned JSON payload.
    pub payload: serde_json::Value,
}

impl PluginStoreRecord {
    /// Serialized payload size in bytes.
    #[must_use]
    pub fn payload_bytes(&self) -> usize {
        plugin_store_payload_bytes(&self.payload)
    }
}

/// Lightweight record metadata returned by list operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStoreEntry {
    /// Record key inside the plugin namespace.
    pub key: PluginStoreKey,
    /// Plugin-declared schema version.
    pub schema_version: u64,
    /// Current record revision.
    pub revision: u64,
    /// Serialized payload size in bytes.
    pub bytes: usize,
}

impl From<&PluginStoreRecord> for PluginStoreEntry {
    fn from(record: &PluginStoreRecord) -> Self {
        Self {
            key: record.key.clone(),
            schema_version: record.schema_version,
            revision: record.revision,
            bytes: record.payload_bytes(),
        }
    }
}

/// Typed plugin-store operation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginStoreResult {
    /// Get returned a record.
    Record {
        /// Retrieved record envelope.
        record: PluginStoreRecord,
    },
    /// Set or patch wrote a record.
    Written {
        /// Written record envelope.
        record: PluginStoreRecord,
    },
    /// Delete removed a record.
    Deleted {
        /// Removed record key.
        key: PluginStoreKey,
        /// Removed record revision.
        revision: u64,
    },
    /// List returned deterministic metadata.
    List {
        /// Ordered record metadata entries.
        entries: Vec<PluginStoreEntry>,
    },
}

/// Host-implemented storage backend for plugin-store runtime implementations.
///
/// Core owns the typed operation semantics. Host profiles own concrete storage
/// policy and should call this backend from their non-blocking capability
/// runtime worker, not directly inside plugin handler execution.
pub trait PluginStoreBackend: Send + Sync {
    /// Get one record owned by `plugin_key`.
    fn get(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
    ) -> Result<Option<PluginStoreRecord>, CapabilityRuntimeError>;

    /// Write one record with an optional revision guard.
    fn set(
        &self,
        plugin_key: &PluginKey,
        key: PluginStoreKey,
        schema_version: u64,
        payload: serde_json::Value,
        expected_revision: Option<u64>,
        limits: PluginStoreLimits,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError>;

    /// Delete one record.
    fn delete(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError>;

    /// List deterministic metadata for one plugin namespace.
    fn list(
        &self,
        plugin_key: &PluginKey,
        prefix: Option<&str>,
    ) -> Result<Vec<PluginStoreEntry>, CapabilityRuntimeError>;

    /// Apply an RFC 7396-style merge patch to one record.
    fn patch(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
        patch: serde_json::Value,
        expected_revision: Option<u64>,
        limits: PluginStoreLimits,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError>;
}

/// Serialized JSON payload size used by quota checks.
#[must_use]
pub fn plugin_store_payload_bytes(payload: &serde_json::Value) -> usize {
    serde_json::to_vec(payload)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

/// Apply RFC 7396-style JSON merge patch semantics.
pub fn apply_plugin_store_merge_patch(
    target: &mut serde_json::Value,
    patch: &serde_json::Value,
) -> Result<(), CapabilityRuntimeError> {
    let serde_json::Value::Object(patch_object) = patch else {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::PatchFailed,
            "plugin-store merge patch must be a JSON object",
        ));
    };

    if !target.is_object() {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::PatchFailed,
            "plugin-store merge patch target must be a JSON object",
        ));
    }

    merge_patch_object(target, patch_object);
    Ok(())
}

fn merge_patch_object(
    target: &mut serde_json::Value,
    patch_object: &serde_json::Map<String, serde_json::Value>,
) {
    let target_object = target
        .as_object_mut()
        .expect("target object checked before merge patch");
    for (key, patch_value) in patch_object {
        if patch_value.is_null() {
            target_object.remove(key);
        } else if let serde_json::Value::Object(child_patch) = patch_value {
            let target_value = target_object
                .entry(key.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if !target_value.is_object() {
                *target_value = serde_json::Value::Object(serde_json::Map::new());
            }
            merge_patch_object(target_value, child_patch);
        } else {
            target_object.insert(key.clone(), patch_value.clone());
        }
    }
}

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
    /// Optional HTTP-style response metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<HttpCapabilityResponse>,
    /// Optional plugin-store operation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_store: Option<PluginStoreResult>,
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

/// Non-blocking capability runtime boundary.
///
/// Host profiles implement concrete capability backends. Core also provides a
/// policy-free file watch implementation of this trait over an injected event
/// source, while the concrete OS watcher and directory/root policy stay with
/// the host profile.
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
    /// A plugin-store record was not found.
    StoreNotFound,
    /// A plugin-store expected revision did not match the current revision.
    RevisionConflict,
    /// A plugin-store write would exceed configured limits.
    QuotaExceeded,
    /// A plugin-store merge patch was invalid or could not be applied.
    PatchFailed,
    /// A plugin-store backend failed after request acceptance.
    BackendFailed,
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

#[derive(Debug, Clone)]
struct HttpValidatedEndpoint {
    scheme: String,
    host: String,
}

struct HttpInFlightOperation {
    resource: PluginResourceRef,
    cancellation: PluginCancellationToken,
    started_at: Instant,
    timeout: Duration,
}

struct HttpWorkerCompletion {
    plugin_key: PluginKey,
    operation_id: CapabilityOperationId,
    result: Result<HttpCapabilityResponse, CapabilityRuntimeError>,
}

fn parse_http_endpoint(endpoint: &str) -> Result<HttpValidatedEndpoint, CapabilityRuntimeError> {
    let (scheme, rest) = endpoint.split_once("://").ok_or_else(|| {
        CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "HTTP endpoint must be an absolute URL",
        )
    })?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme.is_empty()
        || !scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
    {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "HTTP endpoint has an invalid URL scheme",
        ));
    }

    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())
        .ok_or_else(|| {
            CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP endpoint must include a host",
            )
        })?;
    if authority.contains('@') {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "HTTP endpoint userinfo is not allowed",
        ));
    }
    let host_port = authority
        .strip_prefix('[')
        .map_or(authority, |without_open| {
            without_open
                .split_once(']')
                .map_or(authority, |(ipv6, _)| ipv6)
        });
    let host = host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "HTTP endpoint host is empty",
        ));
    }

    Ok(HttpValidatedEndpoint { scheme, host })
}

fn validate_headers(
    headers: &[HttpHeader],
    max_count: usize,
    max_name_bytes: usize,
    max_value_bytes: usize,
) -> Result<(), CapabilityRuntimeError> {
    if headers.len() > max_count {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "HTTP header count exceeds configured limit",
        ));
    }

    for header in headers {
        if header.name.is_empty()
            || header.name.len() > max_name_bytes
            || !header.name.bytes().all(is_http_token_byte)
        {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP header name is invalid or too long",
            ));
        }
        if header.value.len() > max_value_bytes
            || header.value.contains('\r')
            || header.value.contains('\n')
        {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "HTTP header value is invalid or too long",
            ));
        }
    }
    Ok(())
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn event_plugin_key(event: &CapabilityRuntimeEvent) -> Option<PluginKey> {
    match event {
        CapabilityRuntimeEvent::Completed(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::ResourceOpened(event)
        | CapabilityRuntimeEvent::ResourceReleased(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::WebSocketMessage(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::Watch(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::TimerFired(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::TimedOut(event)
        | CapabilityRuntimeEvent::Cancelled(event)
        | CapabilityRuntimeEvent::Failed(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::Backpressure(event) => event.route.plugin_key.clone(),
        CapabilityRuntimeEvent::CleanupCompleted(event) => Some(event.plugin_key.clone()),
    }
}

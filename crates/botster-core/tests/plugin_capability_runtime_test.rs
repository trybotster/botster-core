//! Plugin capability runtime contract acceptance tests.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use botster_core::{
    BackpressureSummary, Capability, CapabilityOperation, CapabilityOperationCompleted,
    CapabilityOperationFailure, CapabilityOperationId, CapabilityOperationResult,
    CapabilityResourceEvent, CapabilityResourceId, CapabilityRuntimeEvent,
    CapabilityRuntimeRequest, CapabilitySurface, CapabilityTimerEvent, CapabilityWatchEvent,
    CapabilityWebSocketEvent, FilesystemCapabilityGrant, FilesystemCapabilityLimits,
    FilesystemCapabilityPermissions, FilesystemCapabilityRequest, FilesystemCapabilityResult,
    FilesystemEntry, FilesystemEntryKind, FilesystemMetadata, FilesystemOperation,
    HttpCapabilityEndpointPolicy, HttpCapabilityRequest, HttpCapabilityResponse,
    HttpCapabilityRuntime, HttpCapabilityRuntimeConfig, HttpCapabilityTransport, HttpHeader,
    HttpTransportRequest, InMemoryWebSocketCapabilityRuntime, PluginCancellationToken,
    PluginCapabilityRuntime, PluginCleanupResult, PluginHandlerKind, PluginHandlerRef, PluginKey,
    PluginResourceKind, PluginResourceRef, PluginStoreCapabilityRequest, PluginStoreKey,
    PluginStoreOperation, QueueSource, RequestId, ScopedRelativePath, TimerCapabilityRequest,
    WatchCapabilityRequest, WatchChangeKind, WebSocketCapabilityRequest,
    WebSocketCapabilityRuntimeConfig, WebSocketMessage,
};

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn operation_id(id: &str) -> CapabilityOperationId {
    CapabilityOperationId(id.to_string())
}

fn resource_id(id: &str) -> CapabilityResourceId {
    CapabilityResourceId(id.to_string())
}

fn callback(plugin: &PluginKey, id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin.clone(),
        kind: PluginHandlerKind::Http,
        handler_id: id.to_string(),
    }
}

fn request(
    plugin: &PluginKey,
    id: &str,
    operation: CapabilityOperation,
) -> CapabilityRuntimeRequest {
    CapabilityRuntimeRequest {
        plugin_key: plugin.clone(),
        operation_id: operation_id(id),
        operation,
        timeout_ms: 250,
        callback: Some(callback(plugin, "capability-result")),
    }
}

fn http_request(plugin: &PluginKey, id: &str, endpoint: &str) -> CapabilityRuntimeRequest {
    request(
        plugin,
        id,
        CapabilityOperation::Http(HttpCapabilityRequest {
            method: "GET".to_string(),
            endpoint: endpoint.to_string(),
            headers: vec![HttpHeader {
                name: "Accept".to_string(),
                value: "application/json".to_string(),
            }],
            body: Vec::new(),
        }),
    )
}

fn network_http_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("http".to_string()),
    }
}

fn capability_set(capabilities: Vec<Capability>) -> BTreeSet<Capability> {
    capabilities.into_iter().collect()
}

fn endpoint_policy() -> HttpCapabilityEndpointPolicy {
    HttpCapabilityEndpointPolicy::new(["https"], ["api.example.test"])
}

fn websocket_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("websocket".to_string()),
    }
}

fn websocket_runtime(
    outbound_capacity: usize,
    inbound_capacity: usize,
    event_capacity: usize,
) -> InMemoryWebSocketCapabilityRuntime {
    InMemoryWebSocketCapabilityRuntime::new(WebSocketCapabilityRuntimeConfig::new(
        BTreeSet::from([websocket_capability()]),
        outbound_capacity,
        inbound_capacity,
        event_capacity,
    ))
}

fn connect_request(plugin: &PluginKey, id: &str) -> CapabilityRuntimeRequest {
    request(
        plugin,
        id,
        CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Connect {
            endpoint: "events-feed".to_string(),
            protocols: vec!["botster.events.v1".to_string()],
        }),
    )
}

fn send_request(
    plugin: &PluginKey,
    id: &str,
    resource_id: &str,
    body: &str,
) -> CapabilityRuntimeRequest {
    request(
        plugin,
        id,
        CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Send {
            resource_id: CapabilityResourceId(resource_id.to_string()),
            message: WebSocketMessage::Text(body.to_string()),
        }),
    )
}

fn close_request(plugin: &PluginKey, id: &str, resource_id: &str) -> CapabilityRuntimeRequest {
    request(
        plugin,
        id,
        CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Close {
            resource_id: CapabilityResourceId(resource_id.to_string()),
            code: Some(1000),
            reason: Some("done".to_string()),
        }),
    )
}

fn handle_resource_id(handle: &botster_core::CapabilityRuntimeHandle) -> String {
    handle
        .resource
        .as_ref()
        .expect("websocket connect returns a resource")
        .resource_id
        .clone()
}

#[derive(Clone)]
struct FakeHttpTransport {
    behavior: Arc<Mutex<FakeHttpBehavior>>,
    calls: Arc<AtomicUsize>,
    cancellation_observed: Arc<AtomicUsize>,
    max_retained_body_bytes: Arc<AtomicUsize>,
    started_sender: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

#[derive(Clone)]
enum FakeHttpBehavior {
    Respond(HttpCapabilityResponse),
    Fail(botster_core::CapabilityRuntimeErrorKind),
    BlockUntilCancelled,
    ChunkedBody(Vec<Vec<u8>>),
}

impl FakeHttpTransport {
    fn responding(body: &[u8]) -> Self {
        Self::new(FakeHttpBehavior::Respond(HttpCapabilityResponse {
            status: 200,
            headers: vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            }],
            body: body.to_vec(),
        }))
    }

    fn blocking(started_sender: mpsc::Sender<()>) -> Self {
        let transport = Self::new(FakeHttpBehavior::BlockUntilCancelled);
        *transport
            .started_sender
            .lock()
            .expect("fake transport started lock") = Some(started_sender);
        transport
    }

    fn chunked(chunks: Vec<Vec<u8>>) -> Self {
        Self::new(FakeHttpBehavior::ChunkedBody(chunks))
    }

    fn failing(kind: botster_core::CapabilityRuntimeErrorKind) -> Self {
        Self::new(FakeHttpBehavior::Fail(kind))
    }

    fn new(behavior: FakeHttpBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            calls: Arc::new(AtomicUsize::new(0)),
            cancellation_observed: Arc::new(AtomicUsize::new(0)),
            max_retained_body_bytes: Arc::new(AtomicUsize::new(0)),
            started_sender: Arc::new(Mutex::new(None)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn cancellations_observed(&self) -> usize {
        self.cancellation_observed.load(Ordering::SeqCst)
    }

    fn max_retained_body_bytes(&self) -> usize {
        self.max_retained_body_bytes.load(Ordering::SeqCst)
    }
}

impl HttpCapabilityTransport for FakeHttpTransport {
    fn execute(
        &self,
        request: HttpTransportRequest,
        cancellation: PluginCancellationToken,
    ) -> Result<HttpCapabilityResponse, botster_core::CapabilityRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(started_sender) = self
            .started_sender
            .lock()
            .expect("fake transport started lock")
            .take()
        {
            let _ = started_sender.send(());
        }

        match self
            .behavior
            .lock()
            .expect("fake transport behavior lock")
            .clone()
        {
            FakeHttpBehavior::Respond(response) => {
                HttpCapabilityRuntime::validate_response(
                    &HttpCapabilityRuntimeConfig {
                        request_capacity: 1,
                        max_request_body_bytes: 1024,
                        max_response_body_bytes: request.max_response_body_bytes,
                        max_header_count: request.max_header_count,
                        max_header_name_bytes: request.max_header_name_bytes,
                        max_header_value_bytes: request.max_header_value_bytes,
                    },
                    &response,
                )?;
                Ok(response)
            }
            FakeHttpBehavior::Fail(kind) => Err(botster_core::CapabilityRuntimeError::new(
                kind,
                "fake transport failure",
            )),
            FakeHttpBehavior::BlockUntilCancelled => {
                while !cancellation.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                self.cancellation_observed.fetch_add(1, Ordering::SeqCst);
                Err(botster_core::CapabilityRuntimeError::new(
                    botster_core::CapabilityRuntimeErrorKind::Cancelled,
                    "cancelled by fake transport",
                ))
            }
            FakeHttpBehavior::ChunkedBody(chunks) => {
                let mut body = Vec::new();
                for chunk in chunks {
                    if cancellation.is_cancelled() {
                        self.cancellation_observed.fetch_add(1, Ordering::SeqCst);
                        return Err(botster_core::CapabilityRuntimeError::new(
                            botster_core::CapabilityRuntimeErrorKind::Cancelled,
                            "cancelled by fake transport",
                        ));
                    }
                    if body.len() + chunk.len() > request.max_response_body_bytes {
                        return Err(botster_core::CapabilityRuntimeError::new(
                            botster_core::CapabilityRuntimeErrorKind::InvalidRequest,
                            "HTTP response body exceeds configured limit while collecting chunks",
                        ));
                    }
                    body.extend(chunk);
                    self.max_retained_body_bytes
                        .fetch_max(body.len(), Ordering::SeqCst);
                }
                Ok(HttpCapabilityResponse {
                    status: 200,
                    headers: Vec::new(),
                    body,
                })
            }
        }
    }
}

fn http_runtime(transport: FakeHttpTransport) -> HttpCapabilityRuntime {
    HttpCapabilityRuntime::new(
        capability_set(vec![network_http_capability()]),
        endpoint_policy(),
        HttpCapabilityRuntimeConfig {
            request_capacity: 2,
            max_request_body_bytes: 16,
            max_response_body_bytes: 8,
            max_header_count: 4,
            max_header_name_bytes: 32,
            max_header_value_bytes: 64,
        },
        Arc::new(transport),
    )
}

fn drain_until(
    runtime: &mut HttpCapabilityRuntime,
    plugin: &PluginKey,
    predicate: impl Fn(&[CapabilityRuntimeEvent]) -> bool,
) -> Vec<CapabilityRuntimeEvent> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < Duration::from_millis(500) {
        events.extend(
            runtime
                .drain_events(plugin)
                .expect("drain HTTP capability events"),
        );
        if predicate(&events) {
            return events;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        predicate(&events),
        "expected HTTP runtime event, got {events:?}"
    );
    events
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize capability runtime value");
    serde_json::from_str(&json).expect("deserialize capability runtime value")
}

#[test]
fn every_operation_family_round_trips_and_declares_required_capability() {
    let plugin = plugin_key("project-pipelines");
    let requests = [
        request(
            &plugin,
            "http-1",
            CapabilityOperation::Http(HttpCapabilityRequest {
                method: "GET".to_string(),
                endpoint: "status-api".to_string(),
                headers: vec![HttpHeader {
                    name: "Accept".to_string(),
                    value: "application/json".to_string(),
                }],
                body: Vec::new(),
            }),
        ),
        request(
            &plugin,
            "ws-1",
            CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Connect {
                endpoint: "events-feed".to_string(),
                protocols: vec!["botster.events.v1".to_string()],
            }),
        ),
        request(
            &plugin,
            "watch-1",
            CapabilityOperation::Watch(WatchCapabilityRequest::Register {
                scope_id: "workspace".to_string(),
                path: ScopedRelativePath("src".to_string()),
                recursive: true,
            }),
        ),
        request(
            &plugin,
            "fs-1",
            CapabilityOperation::Filesystem(FilesystemCapabilityRequest {
                scope_id: "workspace".to_string(),
                operation: FilesystemOperation::Read {
                    path: ScopedRelativePath("README.md".to_string()),
                },
                limits: Some(FilesystemCapabilityLimits {
                    max_read_bytes: Some(65_536),
                    max_write_bytes: None,
                    max_list_entries: None,
                }),
            }),
        ),
        request(
            &plugin,
            "store-1",
            CapabilityOperation::PluginStore(PluginStoreCapabilityRequest {
                namespace: "project-pipelines".to_string(),
                operation: PluginStoreOperation::Set {
                    key: PluginStoreKey("runs/active".to_string()),
                    schema_version: 1,
                    payload: serde_json::json!({ "state": "running" }),
                    expected_revision: None,
                },
            }),
        ),
        request(
            &plugin,
            "timer-1",
            CapabilityOperation::Timer(TimerCapabilityRequest::Interval { interval_ms: 1_000 }),
        ),
    ];

    let expected = [
        Capability {
            surface: CapabilitySurface::Network,
            scope: Some("http".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Network,
            scope: Some("websocket".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Filesystem,
            scope: Some("workspace".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Filesystem,
            scope: Some("workspace".to_string()),
        },
        Capability {
            surface: CapabilitySurface::PluginDb,
            scope: Some("project-pipelines".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Timers,
            scope: Some("callbacks".to_string()),
        },
    ];

    let resource_kinds = [
        PluginResourceKind::HttpRequest,
        PluginResourceKind::NetworkConnection,
        PluginResourceKind::Watch,
        PluginResourceKind::FilesystemOperation,
        PluginResourceKind::PluginStoreOperation,
        PluginResourceKind::Timer,
    ];

    for ((request, capability), resource_kind) in requests.iter().zip(expected).zip(resource_kinds)
    {
        assert_eq!(round_trip(request), *request);
        assert_eq!(request.required_capability(), capability);
        assert_eq!(request.resource_kind(), resource_kind);
        assert_eq!(
            request.resource_ref(resource_id("resource-1")).plugin_key,
            plugin
        );
    }
}

#[test]
fn scoped_filesystem_paths_are_relative_contracts_not_host_policy() {
    let scoped = ScopedRelativePath("logs/session.log".to_string());
    let absolute = ScopedRelativePath("/tmp/secret.txt".to_string());
    let traversal = ScopedRelativePath("../outside".to_string());
    let nested_traversal = ScopedRelativePath("logs/../outside".to_string());
    let drive_absolute = ScopedRelativePath("C:\\secret.txt".to_string());
    let drive_relative = ScopedRelativePath("C:secret.txt".to_string());
    let unc = ScopedRelativePath("\\\\server\\share\\secret.txt".to_string());

    assert!(scoped.is_scoped_relative());
    assert!(!absolute.is_scoped_relative());
    assert!(!traversal.is_scoped_relative());
    assert!(!nested_traversal.is_scoped_relative());
    assert!(!drive_absolute.is_scoped_relative());
    assert!(!drive_relative.is_scoped_relative());
    assert!(!unc.is_scoped_relative());

    let request = FilesystemCapabilityRequest {
        scope_id: "workspace".to_string(),
        operation: FilesystemOperation::Write {
            path: scoped.clone(),
            bytes: b"ok".to_vec(),
        },
        limits: Some(FilesystemCapabilityLimits {
            max_read_bytes: None,
            max_write_bytes: Some(1024),
            max_list_entries: None,
        }),
    };

    assert_eq!(request.operation.path(), &scoped);
    assert_eq!(round_trip(&request).operation, request.operation);
}

#[test]
fn scoped_filesystem_grants_limits_and_results_are_typed_contracts() {
    let grant = FilesystemCapabilityGrant {
        scope_id: "workspace".to_string(),
        permissions: FilesystemCapabilityPermissions {
            read: true,
            write: true,
            list: true,
            stat: true,
            remove: false,
        },
        limits: Some(FilesystemCapabilityLimits {
            max_read_bytes: Some(65_536),
            max_write_bytes: Some(16_384),
            max_list_entries: Some(256),
        }),
    };

    assert_eq!(round_trip(&grant), grant);

    let path = ScopedRelativePath("src/lib.rs".to_string());
    let result = CapabilityOperationResult::Filesystem(FilesystemCapabilityResult::List {
        path: ScopedRelativePath("src".to_string()),
        entries: vec![FilesystemEntry {
            path: path.clone(),
            kind: FilesystemEntryKind::File,
            size_bytes: Some(1234),
        }],
    });

    let stat = CapabilityOperationResult::Filesystem(FilesystemCapabilityResult::Stat {
        path,
        metadata: FilesystemMetadata {
            kind: FilesystemEntryKind::File,
            size_bytes: Some(1234),
            readonly: false,
        },
    });

    assert_eq!(round_trip(&result), result);
    assert_eq!(round_trip(&stat), stat);
}

#[test]
fn scoped_filesystem_permissions_gate_each_operation_kind() {
    let read_only = FilesystemCapabilityPermissions {
        read: true,
        write: false,
        list: false,
        stat: false,
        remove: false,
    };
    let path = ScopedRelativePath("README.md".to_string());

    assert!(read_only.allows(&FilesystemOperation::Read { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Write {
        path: path.clone(),
        bytes: b"denied".to_vec(),
    }));
    assert!(!read_only.allows(&FilesystemOperation::List { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Stat { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Remove { path }));

    let list_and_stat = FilesystemCapabilityPermissions {
        read: false,
        write: false,
        list: true,
        stat: true,
        remove: false,
    };
    let path = ScopedRelativePath("src".to_string());

    assert!(!list_and_stat.allows(&FilesystemOperation::Read { path: path.clone() }));
    assert!(list_and_stat.allows(&FilesystemOperation::List { path: path.clone() }));
    assert!(list_and_stat.allows(&FilesystemOperation::Stat { path }));
}

#[test]
fn events_round_trip_with_plugin_identity_operation_ids_and_pressure_route() {
    let plugin = plugin_key("project-pipelines");
    let websocket = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::NetworkConnection,
        resource_id: "ws-1".to_string(),
    };
    let watch = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::Watch,
        resource_id: "watch-1".to_string(),
    };
    let timer = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::Timer,
        resource_id: "timer-1".to_string(),
    };
    let request = request(
        &plugin,
        "http-1",
        CapabilityOperation::Http(HttpCapabilityRequest {
            method: "GET".to_string(),
            endpoint: "status-api".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }),
    );
    let pressure: BackpressureSummary = request.backpressure(256, 250);

    let events = vec![
        CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            plugin_key: plugin.clone(),
            operation_id: operation_id("http-1"),
            result: Some(CapabilityOperationResult::Http(HttpCapabilityResponse {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            })),
        }),
        CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            plugin_key: plugin.clone(),
            operation_id: operation_id("fs-1"),
            result: Some(CapabilityOperationResult::Filesystem(
                FilesystemCapabilityResult::Read {
                    path: ScopedRelativePath("README.md".to_string()),
                    bytes: b"hello".to_vec(),
                },
            )),
        }),
        CapabilityRuntimeEvent::ResourceOpened(CapabilityResourceEvent {
            plugin_key: plugin.clone(),
            operation_id: operation_id("ws-1"),
            resource: websocket.clone(),
        }),
        CapabilityRuntimeEvent::WebSocketMessage(CapabilityWebSocketEvent {
            resource: websocket,
            message: WebSocketMessage::Text("updated".to_string()),
        }),
        CapabilityRuntimeEvent::Watch(CapabilityWatchEvent {
            resource: watch,
            path: ScopedRelativePath("src/main.rs".to_string()),
            change: WatchChangeKind::Modified,
        }),
        CapabilityRuntimeEvent::TimerFired(CapabilityTimerEvent {
            resource: timer,
            sequence: 3,
        }),
        CapabilityRuntimeEvent::TimedOut(CapabilityOperationFailure {
            plugin_key: plugin.clone(),
            operation_id: operation_id("slow-1"),
            error_kind: botster_core::CapabilityRuntimeErrorKind::TimedOut,
            reason: "operation exceeded timeout".to_string(),
        }),
        CapabilityRuntimeEvent::Backpressure(pressure.clone()),
    ];

    for event in events {
        assert_eq!(round_trip(&event), event);
    }
    assert_eq!(pressure.source, QueueSource::PluginWorker);
    assert_eq!(pressure.route.plugin_key, Some(plugin));
}

#[test]
fn cleanup_events_can_target_one_plugins_runtime_resources() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let cleanup = PluginCleanupResult {
        request_id: RequestId("cleanup-1".to_string()),
        plugin_key: plugin_a.clone(),
        removed_descriptors: Vec::new(),
        removed_resources: vec![
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::NetworkConnection,
                resource_id: "ws-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::Watch,
                resource_id: "watch-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::Timer,
                resource_id: "timer-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::PluginStoreOperation,
                resource_id: "store-1".to_string(),
            },
        ],
    };

    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin_a));
    assert!(!cleanup
        .removed_resources
        .iter()
        .any(|resource| resource.plugin_key == plugin_b));
    assert_eq!(
        round_trip(&CapabilityRuntimeEvent::CleanupCompleted(cleanup.clone())),
        CapabilityRuntimeEvent::CleanupCompleted(cleanup)
    );
}

#[test]
fn websocket_runtime_checks_capability_before_opening_connection() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = InMemoryWebSocketCapabilityRuntime::new(
        WebSocketCapabilityRuntimeConfig::new(BTreeSet::new(), 1, 1, 8),
    );

    let error = runtime
        .submit(connect_request(&plugin, "connect-denied"))
        .expect_err("missing websocket grant rejects connection");

    assert_eq!(
        error.kind,
        botster_core::CapabilityRuntimeErrorKind::CapabilityDenied
    );
    assert!(runtime
        .drain_events(&plugin)
        .expect("drain events")
        .is_empty());
}

#[test]
fn websocket_runtime_emits_connect_inbound_send_and_close_lifecycle_events() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(4, 4, 16);

    let connect = runtime
        .submit(connect_request(&plugin, "connect-1"))
        .expect("connect accepted");
    let resource = connect.resource.clone().expect("connect resource");
    let resource_id = handle_resource_id(&connect);

    runtime
        .enqueue_inbound_message(&resource, WebSocketMessage::Text("hello".to_string()))
        .expect("inbound queued");
    runtime
        .submit(send_request(&plugin, "send-1", &resource_id, "ack"))
        .expect("send accepted");
    let outbound = runtime
        .drain_outbound_messages(&resource)
        .expect("host drains outbound");
    runtime
        .submit(close_request(&plugin, "close-1", &resource_id))
        .expect("close accepted");

    let events = runtime.drain_events(&plugin).expect("drain events");

    assert_eq!(outbound, vec![WebSocketMessage::Text("ack".to_string())]);
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::ResourceOpened(opened)
            if opened.operation_id == operation_id("connect-1")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::WebSocketMessage(message)
            if message.message == WebSocketMessage::Text("hello".to_string())
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::ResourceReleased(released)
            if released.operation_id == operation_id("close-1")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::Completed(completed)
            if completed.operation_id == operation_id("send-1")
    )));
}

#[test]
fn websocket_runtime_rejects_saturated_outbound_send_synchronously() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(1, 4, 16);
    let connect = runtime
        .submit(connect_request(&plugin, "connect-1"))
        .expect("connect accepted");
    let resource_id = handle_resource_id(&connect);

    runtime
        .submit(send_request(&plugin, "send-1", &resource_id, "first"))
        .expect("first send accepted");
    let error = runtime
        .submit(send_request(&plugin, "send-2", &resource_id, "second"))
        .expect_err("full outbound queue rejects immediately");
    let events = runtime.drain_events(&plugin).expect("drain events");

    assert_eq!(
        error.kind,
        botster_core::CapabilityRuntimeErrorKind::Backpressured
    );
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::Backpressure(summary)
            if summary.capacity == 1
                && summary.depth == 1
                && summary.route.plugin_key == Some(plugin.clone())
    )));
}

#[test]
fn websocket_runtime_event_backpressure_does_not_enqueue_rejected_send() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(4, 4, 3);
    let connect = runtime
        .submit(connect_request(&plugin, "connect-1"))
        .expect("connect accepted");
    let resource = connect.resource.clone().expect("connect resource");
    let resource_id = handle_resource_id(&connect);

    runtime
        .submit(send_request(&plugin, "send-1", &resource_id, "accepted"))
        .expect("first send fills remaining event capacity");
    assert_eq!(
        runtime
            .drain_outbound_messages(&resource)
            .expect("host drains accepted outbound"),
        vec![WebSocketMessage::Text("accepted".to_string())]
    );

    let error = runtime
        .submit(send_request(&plugin, "send-2", &resource_id, "rejected"))
        .expect_err("full event queue rejects before outbound mutation");

    assert_eq!(
        error.kind,
        botster_core::CapabilityRuntimeErrorKind::Backpressured
    );
    assert!(runtime
        .drain_outbound_messages(&resource)
        .expect("host drains outbound after rejected send")
        .is_empty());
}

#[test]
fn websocket_runtime_bounds_events_only_receive_queue() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(4, 1, 16);
    let connect = runtime
        .submit(connect_request(&plugin, "connect-1"))
        .expect("connect accepted");
    let resource = connect.resource.clone().expect("connect resource");

    runtime
        .enqueue_inbound_message(&resource, WebSocketMessage::Text("one".to_string()))
        .expect("first inbound accepted");
    let error = runtime
        .enqueue_inbound_message(&resource, WebSocketMessage::Text("two".to_string()))
        .expect_err("bounded inbound receive queue rejects pressure");
    let events = runtime.drain_events(&plugin).expect("drain events");

    assert_eq!(
        error.kind,
        botster_core::CapabilityRuntimeErrorKind::Backpressured
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, CapabilityRuntimeEvent::WebSocketMessage(_)))
            .count(),
        1
    );
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::Backpressure(summary)
            if summary.capacity == 1
                && summary.depth == 1
                && summary.route.plugin_key == Some(plugin.clone())
    )));
}

#[test]
fn websocket_runtime_event_backpressure_does_not_drift_inbound_depth() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(4, 1, 2);
    let connect = runtime
        .submit(connect_request(&plugin, "connect-1"))
        .expect("connect fills event capacity");
    let resource = connect.resource.clone().expect("connect resource");

    let error = runtime
        .enqueue_inbound_message(&resource, WebSocketMessage::Text("rejected".to_string()))
        .expect_err("full event queue rejects before inbound depth mutation");

    assert_eq!(
        error.kind,
        botster_core::CapabilityRuntimeErrorKind::Backpressured
    );
    let connect_events = runtime.drain_events(&plugin).expect("drain connect events");
    assert_eq!(
        connect_events
            .iter()
            .filter(|event| matches!(event, CapabilityRuntimeEvent::WebSocketMessage(_)))
            .count(),
        0
    );

    runtime
        .enqueue_inbound_message(&resource, WebSocketMessage::Text("accepted".to_string()))
        .expect("inbound accepts after draining events because depth did not drift");
    let second = runtime.enqueue_inbound_message(
        &resource,
        WebSocketMessage::Text("still-bounded".to_string()),
    );

    assert_eq!(
        second
            .expect_err("real inbound capacity still applies")
            .kind,
        botster_core::CapabilityRuntimeErrorKind::Backpressured
    );
}

#[test]
fn websocket_runtime_reports_timeout_and_cancellation_as_typed_events() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = websocket_runtime(4, 4, 16);
    let mut timed_out = connect_request(&plugin, "connect-timeout");
    timed_out.timeout_ms = 0;

    let timeout = runtime
        .submit(timed_out)
        .expect_err("zero timeout rejects operation");
    let connect = runtime
        .submit(connect_request(&plugin, "connect-cancel"))
        .expect("connect accepted");
    runtime
        .cancel(&plugin, &connect.operation_id)
        .expect("cancel known websocket operation");
    let cancel_send = runtime.submit(send_request(
        &plugin,
        "send-after-cancel",
        &handle_resource_id(&connect),
        "lost",
    ));
    let events = runtime.drain_events(&plugin).expect("drain events");

    assert_eq!(
        timeout.kind,
        botster_core::CapabilityRuntimeErrorKind::TimedOut
    );
    assert_eq!(
        cancel_send.expect_err("cancel releases resource").kind,
        botster_core::CapabilityRuntimeErrorKind::ResourceNotFound
    );
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::TimedOut(failure)
            if failure.operation_id == operation_id("connect-timeout")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::Cancelled(failure)
            if failure.operation_id == operation_id("connect-cancel")
    )));
}

#[test]
fn websocket_runtime_cleanup_targets_only_one_plugins_connections() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let mut runtime = websocket_runtime(4, 4, 16);
    let connection_a = runtime
        .submit(connect_request(&plugin_a, "connect-a"))
        .expect("plugin a connect");
    let connection_b = runtime
        .submit(connect_request(&plugin_b, "connect-b"))
        .expect("plugin b connect");
    let resource_a = connection_a.resource.clone().expect("plugin a resource");
    let resource_b = connection_b.resource.clone().expect("plugin b resource");

    let cleanup = runtime
        .cleanup_plugin(&plugin_a)
        .expect("cleanup plugin a resources");

    assert_eq!(cleanup.plugin_key, plugin_a);
    assert_eq!(cleanup.removed_resources, vec![resource_a.clone()]);
    assert_eq!(
        runtime
            .drain_outbound_messages(&resource_a)
            .expect_err("plugin a connection removed")
            .kind,
        botster_core::CapabilityRuntimeErrorKind::ResourceNotFound
    );
    assert!(runtime
        .drain_outbound_messages(&resource_b)
        .expect("plugin b connection remains")
        .is_empty());
}

#[test]
fn http_runtime_accepts_allowed_request_and_emits_completion_event() {
    let plugin = plugin_key("project-pipelines");
    let transport = FakeHttpTransport::responding(b"ok");
    let mut runtime = http_runtime(transport.clone());

    let handle = runtime
        .submit(http_request(
            &plugin,
            "http-ok",
            "https://api.example.test/status",
        ))
        .expect("allowed HTTP request is accepted");

    assert_eq!(handle.plugin_key, plugin);
    assert_eq!(handle.required_capability, network_http_capability());
    assert_eq!(
        handle.resource.expect("HTTP resource ref").kind,
        PluginResourceKind::HttpRequest
    );

    let events = drain_until(&mut runtime, &plugin, |events| {
        events
            .iter()
            .any(|event| matches!(event, CapabilityRuntimeEvent::Completed(_)))
    });
    assert_eq!(transport.calls(), 1);
    assert!(matches!(
        events.as_slice(),
        [CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            operation_id: completed_operation_id,
            result: Some(CapabilityOperationResult::Http(HttpCapabilityResponse { status: 200, .. })),
            ..
        })] if completed_operation_id == &operation_id("http-ok")
    ));
}

#[test]
fn http_runtime_denies_missing_capability_or_host_before_transport() {
    let plugin = plugin_key("project-pipelines");
    let transport = FakeHttpTransport::responding(b"{}");
    let mut no_grant_runtime = HttpCapabilityRuntime::new(
        capability_set(Vec::new()),
        endpoint_policy(),
        HttpCapabilityRuntimeConfig::default(),
        Arc::new(transport.clone()),
    );

    let denied = no_grant_runtime
        .submit(http_request(
            &plugin,
            "no-grant",
            "https://api.example.test/status",
        ))
        .expect_err("missing network:http capability is denied");
    assert_eq!(
        denied.kind,
        botster_core::CapabilityRuntimeErrorKind::CapabilityDenied
    );

    let mut host_denied_runtime = http_runtime(transport.clone());
    let denied = host_denied_runtime
        .submit(http_request(
            &plugin,
            "bad-host",
            "https://not-allowed.example.test/status",
        ))
        .expect_err("unlisted host is denied");
    assert_eq!(
        denied.kind,
        botster_core::CapabilityRuntimeErrorKind::CapabilityDenied
    );
    assert_eq!(transport.calls(), 0);
}

#[test]
fn http_runtime_rejects_invalid_headers_and_request_body_before_transport() {
    let plugin = plugin_key("project-pipelines");
    let transport = FakeHttpTransport::responding(b"{}");
    let mut runtime = http_runtime(transport.clone());
    let mut request = http_request(&plugin, "bad-header", "https://api.example.test/status");
    let CapabilityOperation::Http(http) = &mut request.operation else {
        panic!("expected HTTP operation");
    };
    http.headers = vec![HttpHeader {
        name: "Bad Header".to_string(),
        value: "ok".to_string(),
    }];

    let invalid = runtime
        .submit(request)
        .expect_err("invalid header is rejected");
    assert_eq!(
        invalid.kind,
        botster_core::CapabilityRuntimeErrorKind::InvalidRequest
    );

    let mut request = http_request(&plugin, "big-body", "https://api.example.test/status");
    let CapabilityOperation::Http(http) = &mut request.operation else {
        panic!("expected HTTP operation");
    };
    http.body = vec![b'x'; 17];

    let invalid = runtime
        .submit(request)
        .expect_err("oversized request body is rejected");
    assert_eq!(
        invalid.kind,
        botster_core::CapabilityRuntimeErrorKind::InvalidRequest
    );
    assert_eq!(transport.calls(), 0);
}

#[test]
fn http_runtime_submit_returns_while_transport_blocks_on_worker_thread() {
    let plugin = plugin_key("project-pipelines");
    let (started_sender, started_receiver) = mpsc::channel();
    let transport = FakeHttpTransport::blocking(started_sender);
    let mut runtime = http_runtime(transport.clone());

    let started_at = Instant::now();
    runtime
        .submit(http_request(
            &plugin,
            "http-blocks",
            "https://api.example.test/status",
        ))
        .expect("blocking transport request is accepted");
    assert!(
        started_at.elapsed() < Duration::from_millis(50),
        "submit must not block on transport execution"
    );
    started_receiver
        .recv_timeout(Duration::from_millis(250))
        .expect("transport runs on background worker");

    runtime
        .cancel(&plugin, &operation_id("http-blocks"))
        .expect("cancel in-flight HTTP request");
    drain_until(&mut runtime, &plugin, |events| {
        events
            .iter()
            .any(|event| matches!(event, CapabilityRuntimeEvent::Cancelled(_)))
    });
    drain_until(&mut runtime, &plugin, |_| {
        transport.cancellations_observed() == 1
    });
}

#[test]
fn http_runtime_timeout_cancels_in_flight_transport_and_releases_capacity() {
    let plugin = plugin_key("project-pipelines");
    let (started_sender, started_receiver) = mpsc::channel();
    let transport = FakeHttpTransport::blocking(started_sender);
    let mut runtime = HttpCapabilityRuntime::new(
        capability_set(vec![network_http_capability()]),
        endpoint_policy(),
        HttpCapabilityRuntimeConfig {
            request_capacity: 1,
            max_request_body_bytes: 16,
            max_response_body_bytes: 8,
            max_header_count: 4,
            max_header_name_bytes: 32,
            max_header_value_bytes: 64,
        },
        Arc::new(transport.clone()),
    );
    let mut request = http_request(&plugin, "http-timeout", "https://api.example.test/status");
    request.timeout_ms = 10;

    runtime.submit(request).expect("HTTP request is accepted");
    started_receiver
        .recv_timeout(Duration::from_millis(250))
        .expect("transport starts before timeout assertion");
    std::thread::sleep(Duration::from_millis(20));

    let events = runtime.drain_events(&plugin).expect("drain timeout event");
    assert!(matches!(
        events.as_slice(),
        [CapabilityRuntimeEvent::TimedOut(
            CapabilityOperationFailure {
                error_kind: botster_core::CapabilityRuntimeErrorKind::TimedOut,
                ..
            }
        )]
    ));

    runtime
        .submit(http_request(
            &plugin,
            "after-timeout",
            "https://api.example.test/status",
        ))
        .expect("timeout releases request capacity");
    drain_until(&mut runtime, &plugin, |_| {
        transport.cancellations_observed() == 1
    });
}

#[test]
fn http_runtime_reports_response_oversize_without_retaining_full_body() {
    let plugin = plugin_key("project-pipelines");
    let transport =
        FakeHttpTransport::chunked(vec![b"1234".to_vec(), b"5678".to_vec(), b"9".to_vec()]);
    let mut runtime = http_runtime(transport.clone());

    runtime
        .submit(http_request(
            &plugin,
            "too-large",
            "https://api.example.test/status",
        ))
        .expect("HTTP request is accepted before response collection");
    let events = drain_until(&mut runtime, &plugin, |events| {
        events
            .iter()
            .any(|event| matches!(event, CapabilityRuntimeEvent::Failed(_)))
    });

    assert!(matches!(
        events.as_slice(),
        [CapabilityRuntimeEvent::Failed(CapabilityOperationFailure {
            error_kind: botster_core::CapabilityRuntimeErrorKind::InvalidRequest,
            ..
        })]
    ));
    assert_eq!(transport.max_retained_body_bytes(), 8);
}

#[test]
fn http_runtime_cleanup_cancels_only_target_plugins_resources() {
    let plugin = plugin_key("project-pipelines");
    let other = plugin_key("preview");
    let shared_transport = FakeHttpTransport::new(FakeHttpBehavior::BlockUntilCancelled);
    let mut runtime = HttpCapabilityRuntime::new(
        capability_set(vec![network_http_capability()]),
        endpoint_policy(),
        HttpCapabilityRuntimeConfig {
            request_capacity: 3,
            max_request_body_bytes: 16,
            max_response_body_bytes: 8,
            max_header_count: 4,
            max_header_name_bytes: 32,
            max_header_value_bytes: 64,
        },
        Arc::new(shared_transport.clone()),
    );

    runtime
        .submit(http_request(
            &plugin,
            "cleanup-a",
            "https://api.example.test/status",
        ))
        .expect("plugin A request accepted");
    runtime
        .submit(http_request(
            &other,
            "cleanup-b",
            "https://api.example.test/status",
        ))
        .expect("plugin B request accepted");

    let cleanup = runtime
        .cleanup_plugin(&plugin)
        .expect("cleanup target plugin");
    assert_eq!(cleanup.plugin_key, plugin);
    assert_eq!(cleanup.removed_resources.len(), 1);
    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin));

    runtime
        .cancel(&other, &operation_id("cleanup-b"))
        .expect("other plugin request remains tracked after plugin A cleanup");
    let events = drain_until(&mut runtime, &other, |events| {
        events
            .iter()
            .any(|event| matches!(event, CapabilityRuntimeEvent::Cancelled(_)))
    });
    assert!(events.iter().all(|event| {
        matches!(
            event,
            CapabilityRuntimeEvent::Cancelled(CapabilityOperationFailure {
                plugin_key,
                ..
            }) if plugin_key == &other
        )
    }));
}

#[test]
fn http_runtime_preserves_typed_transport_failures() {
    let plugin = plugin_key("project-pipelines");
    let transport =
        FakeHttpTransport::failing(botster_core::CapabilityRuntimeErrorKind::RuntimeStopped);
    let mut runtime = http_runtime(transport);

    runtime
        .submit(http_request(
            &plugin,
            "transport-fails",
            "https://api.example.test/status",
        ))
        .expect("HTTP request is accepted before transport failure");
    let events = drain_until(&mut runtime, &plugin, |events| {
        events
            .iter()
            .any(|event| matches!(event, CapabilityRuntimeEvent::Failed(_)))
    });
    assert!(matches!(
        events.as_slice(),
        [CapabilityRuntimeEvent::Failed(CapabilityOperationFailure {
            error_kind: botster_core::CapabilityRuntimeErrorKind::RuntimeStopped,
            ..
        })]
    ));
}

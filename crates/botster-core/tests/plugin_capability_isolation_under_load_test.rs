//! Cross-primitive plugin capability isolation tests.

use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use botster_core::{
    BotsterEngine, BoundaryJson, Capability, CapabilityOperation, CapabilityOperationId,
    CapabilityResourceEvent, CapabilityResourceId, CapabilityRuntimeError,
    CapabilityRuntimeErrorKind, CapabilityRuntimeEvent, CapabilityRuntimeHandle,
    CapabilityRuntimeRequest, CapabilitySurface, CoreSessionMetadata, EngineCommand,
    EngineCommandOutcome, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime,
    FileWatchEventSource, FileWatchRegistration, FileWatchRuntime, FileWatchRuntimeConfig,
    FileWatchSourceError, FileWatchSourceEvent, FilesystemCapabilityLimits,
    FilesystemCapabilityRequest, FilesystemOperation, HttpCapabilityEndpointPolicy,
    HttpCapabilityRequest, HttpCapabilityResponse, HttpCapabilityRuntime,
    HttpCapabilityRuntimeConfig, HttpCapabilityTransport, HttpHeader, HttpTransportRequest,
    InMemoryWebSocketCapabilityRuntime, PackageManifest, PluginCapabilityRuntime,
    PluginCleanupResult, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginLoadSpec, PluginOwnedDescriptor, PluginResourceRef,
    PluginRuntime, PluginStoreCapabilityRequest, PluginStoreKey, PluginStoreOperation,
    PluginTimerEvent, PluginTimerId, PluginTimerMode, PluginTimerSchedule, PluginUnloadSpec,
    PluginWorkerEngineConfig, PluginWorkerRegistration, QueueSource, RequestId, ScopedRelativePath,
    SessionActivityStatus, SessionId, SessionIoEvent, SessionIoRequest, SessionLifecycleState,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    WatchCapabilityRequest, WatchChangeKind, WebSocketCapabilityRequest,
    WebSocketCapabilityRuntimeConfig, WebSocketMessage,
};
use botster_core_test_support::fake::{
    FakePluginBehavior, FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
};

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn plugin_key(value: &str) -> PluginKey {
    PluginKey(value.to_string())
}

fn operation_id(value: &str) -> CapabilityOperationId {
    CapabilityOperationId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("isolation-session".to_string())
}

fn client_id(value: &str) -> botster_core::ClientId {
    botster_core::ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("spawn-isolation"),
        session_id: session_id(),
        executable: "fake-shell".to_string(),
        arguments: vec!["--login".to_string()],
        working_directory: SpawnWorkingDirectory {
            path: "/workspace".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: None,
    }
}

fn capability(surface: CapabilitySurface, scope: &str) -> Capability {
    Capability {
        surface,
        scope: Some(scope.to_string()),
    }
}

fn handler(plugin: &PluginKey, kind: PluginHandlerKind, handler_id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin.clone(),
        kind,
        handler_id: handler_id.to_string(),
    }
}

fn capability_request(
    plugin: &PluginKey,
    id: &str,
    operation: CapabilityOperation,
) -> CapabilityRuntimeRequest {
    CapabilityRuntimeRequest {
        plugin_key: plugin.clone(),
        operation_id: operation_id(id),
        operation,
        timeout_ms: 250,
        callback: Some(handler(
            plugin,
            PluginHandlerKind::Http,
            "capability-result",
        )),
    }
}

fn http_request(plugin: &PluginKey, id: &str) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::Http(HttpCapabilityRequest {
            method: "GET".to_string(),
            endpoint: "https://api.example.test/status".to_string(),
            headers: vec![HttpHeader {
                name: "Accept".to_string(),
                value: "application/json".to_string(),
            }],
            body: Vec::new(),
        }),
    )
}

fn websocket_connect(plugin: &PluginKey, id: &str) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Connect {
            endpoint: "events-feed".to_string(),
            protocols: Vec::new(),
        }),
    )
}

fn websocket_send(
    plugin: &PluginKey,
    id: &str,
    resource_id: &str,
    body: &str,
) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Send {
            resource_id: CapabilityResourceId(resource_id.to_string()),
            message: WebSocketMessage::Text(body.to_string()),
        }),
    )
}

fn watch_request(plugin: &PluginKey, id: &str, path: &str) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::Watch(WatchCapabilityRequest::Register {
            scope_id: "workspace".to_string(),
            path: ScopedRelativePath(path.to_string()),
            recursive: true,
        }),
    )
}

fn filesystem_request(plugin: &PluginKey, id: &str, path: &str) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::Filesystem(FilesystemCapabilityRequest {
            scope_id: "workspace".to_string(),
            operation: FilesystemOperation::Read {
                path: ScopedRelativePath(path.to_string()),
            },
            limits: Some(FilesystemCapabilityLimits {
                max_read_bytes: Some(1024),
                max_write_bytes: None,
                max_list_entries: None,
            }),
        }),
    )
}

fn store_request(plugin: &PluginKey, id: &str, key: &str) -> CapabilityRuntimeRequest {
    capability_request(
        plugin,
        id,
        CapabilityOperation::PluginStore(PluginStoreCapabilityRequest {
            namespace: "project-pipelines".to_string(),
            operation: PluginStoreOperation::Set {
                key: PluginStoreKey(key.to_string()),
                schema_version: 1,
                payload: serde_json::json!({ "state": "running" }),
                expected_revision: None,
            },
        }),
    )
}

fn timer_handler(plugin: &PluginKey, handler_id: &str) -> PluginHandlerRef {
    handler(plugin, PluginHandlerKind::Timer, handler_id)
}

fn command_handler(plugin: &PluginKey, handler_id: &str) -> PluginHandlerRef {
    handler(plugin, PluginHandlerKind::Command, handler_id)
}

fn manifest(plugin: &PluginKey) -> PackageManifest {
    PackageManifest {
        name: plugin.0.clone(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities: Vec::new(),
        entrypoints: vec![ExtensionEntrypoint {
            runtime: ExtensionRuntime::Lua,
            path: "plugin.lua".to_string(),
            bootstrap: false,
        }],
        host_profile: None,
    }
}

fn plugin_registration(
    runtime: FakePluginRuntime,
    plugin: &PluginKey,
    registered_handler: &PluginHandlerRef,
) -> PluginWorkerRegistration {
    let descriptor_kind = match registered_handler.kind {
        PluginHandlerKind::Timer => PluginDescriptorKind::Timer,
        _ => PluginDescriptorKind::Command,
    };

    PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin.clone(),
            package: plugin.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![PluginOwnedDescriptor {
                descriptor: PluginDescriptorRef {
                    plugin_key: plugin.clone(),
                    kind: descriptor_kind,
                    descriptor_id: registered_handler.handler_id.clone(),
                },
                handler: Some(registered_handler.clone()),
                body: BoundaryJson(serde_json::json!({ "title": registered_handler.handler_id })),
            }],
            metadata: None,
        },
        manifest: manifest(plugin),
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: registered_handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    }
}

fn plugin_invocation(
    request: &str,
    registered_handler: PluginHandlerRef,
    timeout_ms: u64,
) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: request_id(request),
        handler: registered_handler,
        timeout_ms,
        context: PluginInvocationContext {
            client_id: Some(client_id("client-hot")),
            session_id: Some(session_id()),
            subscription_id: Some(subscription_id("sub-hot")),
            surface_id: None,
            origin: Some("capability-isolation-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "command": "run" })),
    }
}

fn timer_schedule(
    plugin: &PluginKey,
    handler: PluginHandlerRef,
    timer_id: &str,
) -> PluginTimerSchedule {
    PluginTimerSchedule {
        request_id: request_id("schedule-interval"),
        timer_id: PluginTimerId(timer_id.to_string()),
        handler,
        due_at_ms: 10,
        mode: PluginTimerMode::Interval { interval_ms: 10 },
        timeout_ms: 25,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("capability-isolation-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "value": plugin.0 })),
    }
}

#[derive(Clone, Default)]
struct BlockingHttpTransport {
    started: Arc<AtomicUsize>,
    cancelled: Arc<AtomicUsize>,
}

impl HttpCapabilityTransport for BlockingHttpTransport {
    fn execute(
        &self,
        _request: HttpTransportRequest,
        cancellation: botster_core::PluginCancellationToken,
    ) -> Result<HttpCapabilityResponse, CapabilityRuntimeError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        while !cancellation.is_cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.cancelled.fetch_add(1, Ordering::SeqCst);
        Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::Cancelled,
            "cancelled by blocking test transport",
        ))
    }
}

#[derive(Default)]
struct FakeWatchSource {
    registrations: Vec<FileWatchRegistration>,
    unregistered: Vec<PluginResourceRef>,
    events: Vec<FileWatchSourceEvent>,
}

impl FakeWatchSource {
    fn emit(&mut self, event: FileWatchSourceEvent) {
        self.events.push(event);
    }
}

impl FileWatchEventSource for FakeWatchSource {
    fn register(
        &mut self,
        registration: FileWatchRegistration,
    ) -> Result<(), FileWatchSourceError> {
        self.registrations.push(registration);
        Ok(())
    }

    fn unregister(&mut self, resource: &PluginResourceRef) -> Result<(), FileWatchSourceError> {
        self.unregistered.push(resource.clone());
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<FileWatchSourceEvent>, FileWatchSourceError> {
        Ok(std::mem::take(&mut self.events))
    }
}

#[derive(Debug, Clone)]
struct BoundedFakeCapabilityRuntime {
    capacity: usize,
    pending: VecDeque<CapabilityRuntimeRequest>,
    events: Vec<CapabilityRuntimeEvent>,
}

impl BoundedFakeCapabilityRuntime {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            pending: VecDeque::new(),
            events: Vec::new(),
        }
    }
}

impl PluginCapabilityRuntime for BoundedFakeCapabilityRuntime {
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        if self.pending.len() >= self.capacity {
            self.events.push(CapabilityRuntimeEvent::Backpressure(
                request.backpressure(self.capacity, self.pending.len()),
            ));
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::Backpressured,
                "bounded fake capability runtime is full",
            ));
        }

        let resource = request.resource_ref(CapabilityResourceId(request.operation_id.0.clone()));
        let handle = CapabilityRuntimeHandle {
            plugin_key: request.plugin_key.clone(),
            operation_id: request.operation_id.clone(),
            resource: Some(resource.clone()),
            required_capability: request.required_capability(),
        };
        self.pending.push_back(request.clone());
        self.events.push(CapabilityRuntimeEvent::ResourceOpened(
            CapabilityResourceEvent {
                plugin_key: request.plugin_key,
                operation_id: request.operation_id,
                resource,
            },
        ));
        Ok(handle)
    }

    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        let index = self
            .pending
            .iter()
            .position(|request| {
                &request.plugin_key == plugin_key && &request.operation_id == operation_id
            })
            .ok_or_else(|| {
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::OperationNotFound,
                    "bounded fake operation not found",
                )
            })?;
        self.pending.remove(index);
        Ok(())
    }

    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError> {
        self.cancel(
            &resource.plugin_key,
            &CapabilityOperationId(resource.resource_id),
        )
    }

    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError> {
        let mut drained = Vec::new();
        self.events.retain(|event| {
            if event_plugin_key(event).as_ref() == Some(plugin_key) {
                drained.push(event.clone());
                false
            } else {
                true
            }
        });
        Ok(drained)
    }

    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError> {
        let mut removed_resources = Vec::new();
        self.pending.retain(|request| {
            if &request.plugin_key == plugin_key {
                removed_resources.push(
                    request.resource_ref(CapabilityResourceId(request.operation_id.0.clone())),
                );
                false
            } else {
                true
            }
        });
        Ok(PluginCleanupResult {
            request_id: request_id("bounded-fake-cleanup"),
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources,
        })
    }
}

#[derive(Default)]
struct RecordingRuntime {
    invocations: Arc<Mutex<Vec<PluginInvocationRequest>>>,
    stopped: Arc<Mutex<Vec<PluginKey>>>,
}

impl PluginRuntime for RecordingRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: botster_core::PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.invocations
            .lock()
            .expect("recording runtime invocation lock")
            .push(request.clone());
        PluginInvocationResult::Completed(botster_core::PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: Some(BoundaryJson(serde_json::json!({ "ok": true }))),
        })
    }

    fn stop(&self, plugin_key: &PluginKey) {
        self.stopped
            .lock()
            .expect("recording runtime stopped lock")
            .push(plugin_key.clone());
    }
}

#[test]
fn saturated_capability_primitives_do_not_starve_engine_client_paths_or_unrelated_plugins() {
    let saturated_plugin = plugin_key("project-pipelines");
    let unrelated_plugin = plugin_key("preview");
    let timer_plugin = plugin_key("timer-noise");

    let blocking_transport = BlockingHttpTransport::default();
    let mut http = HttpCapabilityRuntime::new(
        BTreeSet::from([capability(CapabilitySurface::Network, "http")]),
        HttpCapabilityEndpointPolicy::new(["https"], ["api.example.test"]),
        HttpCapabilityRuntimeConfig {
            request_capacity: 1,
            max_request_body_bytes: 1024,
            max_response_body_bytes: 1024,
            max_header_count: 4,
            max_header_name_bytes: 32,
            max_header_value_bytes: 64,
        },
        Arc::new(blocking_transport.clone()),
    );
    let http_handle = http
        .submit(http_request(&saturated_plugin, "http-slow"))
        .expect("first slow HTTP request accepted");
    for _ in 0..50 {
        if blocking_transport.started.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let http_pressure = http
        .submit(http_request(&saturated_plugin, "http-rejected"))
        .expect_err("HTTP capacity rejects instead of waiting");
    assert_eq!(
        http_pressure.kind,
        CapabilityRuntimeErrorKind::Backpressured
    );

    let mut websocket =
        InMemoryWebSocketCapabilityRuntime::new(WebSocketCapabilityRuntimeConfig::new(
            BTreeSet::from([capability(CapabilitySurface::Network, "websocket")]),
            1,
            1,
            16,
        ));
    let websocket_handle = websocket
        .submit(websocket_connect(&saturated_plugin, "ws-connect"))
        .expect("websocket connect accepted");
    let websocket_resource = websocket_handle
        .resource
        .clone()
        .expect("websocket resource");
    websocket
        .submit(websocket_send(
            &saturated_plugin,
            "ws-send-accepted",
            &websocket_resource.resource_id,
            "accepted",
        ))
        .expect("first websocket send accepted");
    assert_eq!(
        websocket
            .submit(websocket_send(
                &saturated_plugin,
                "ws-send-rejected",
                &websocket_resource.resource_id,
                "rejected",
            ))
            .expect_err("bounded websocket outbound rejects")
            .kind,
        CapabilityRuntimeErrorKind::Backpressured
    );
    websocket
        .drain_events(&saturated_plugin)
        .expect("drain websocket lifecycle before inbound pressure");
    websocket
        .enqueue_inbound_message(
            &websocket_resource,
            WebSocketMessage::Text("one".to_string()),
        )
        .expect("first inbound websocket message accepted");
    assert_eq!(
        websocket
            .enqueue_inbound_message(
                &websocket_resource,
                WebSocketMessage::Text("two".to_string())
            )
            .expect_err("bounded websocket inbound rejects")
            .kind,
        CapabilityRuntimeErrorKind::Backpressured
    );

    let mut watch = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 1,
            event_capacity: 1,
            debounce_ms: 0,
        },
    )
    .expect("watch runtime config");
    watch.grant_capability(
        saturated_plugin.clone(),
        capability(CapabilitySurface::Filesystem, "workspace"),
    );
    let watch_resource = watch
        .submit(watch_request(&saturated_plugin, "watch-src", "src"))
        .expect("watch accepted")
        .resource
        .expect("watch resource");
    assert_eq!(
        watch.drain_events(&saturated_plugin).expect("open").len(),
        1
    );
    watch.source_mut().emit(FileWatchSourceEvent::path(
        watch_resource.clone(),
        ScopedRelativePath("src/a.rs".to_string()),
        WatchChangeKind::Modified,
        1,
    ));
    watch.source_mut().emit(FileWatchSourceEvent::path(
        watch_resource,
        ScopedRelativePath("src/b.rs".to_string()),
        WatchChangeKind::Modified,
        1,
    ));
    let watch_events = watch.drain_events(&saturated_plugin).expect("watch drain");
    assert!(watch_events
        .iter()
        .any(|event| matches!(event, CapabilityRuntimeEvent::Watch(_))));
    assert!(watch_events.iter().any(|event| matches!(
        event,
        CapabilityRuntimeEvent::Backpressure(summary)
            if summary.source == QueueSource::PluginWorker
                && summary.capacity == 1
                && summary.route.plugin_key == Some(saturated_plugin.clone())
    )));

    let mut fs_fake = BoundedFakeCapabilityRuntime::new(1);
    fs_fake
        .submit(filesystem_request(
            &saturated_plugin,
            "fs-read",
            "README.md",
        ))
        .expect("first fake filesystem request accepted");
    assert_eq!(
        fs_fake
            .submit(filesystem_request(
                &saturated_plugin,
                "fs-read-2",
                "LICENSE"
            ))
            .expect_err("filesystem fake pressure is bounded")
            .kind,
        CapabilityRuntimeErrorKind::Backpressured
    );

    let mut store_fake = BoundedFakeCapabilityRuntime::new(1);
    store_fake
        .submit(store_request(&saturated_plugin, "store-set", "runs/active"))
        .expect("first fake store request accepted");
    assert_eq!(
        store_fake
            .submit(store_request(
                &saturated_plugin,
                "store-set-2",
                "runs/queued"
            ))
            .expect_err("store fake pressure is bounded")
            .kind,
        CapabilityRuntimeErrorKind::Backpressured
    );

    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::with_plugin_config(
            FakeSessionRuntime::new(),
            PluginWorkerEngineConfig {
                per_plugin_capacity: 1,
            },
        );
    let unrelated_handler = command_handler(&unrelated_plugin, "render");
    let unrelated_runtime = FakePluginRuntime::success("unrelated-ok");
    engine.load_plugin(plugin_registration(
        unrelated_runtime.clone(),
        &unrelated_plugin,
        &unrelated_handler,
    ));
    let timer_handler = timer_handler(&timer_plugin, "tick");
    let timer_runtime = FakePluginRuntime::new(FakePluginBehavior::WaitForCancellation);
    engine.load_plugin(plugin_registration(
        timer_runtime.clone(),
        &timer_plugin,
        &timer_handler,
    ));
    engine.schedule_plugin_timer(timer_schedule(
        &timer_plugin,
        timer_handler.clone(),
        "interval-pressure",
    ));

    let spawn = engine
        .execute_command(EngineCommand::SpawnSession {
            request: spawn_request(),
            metadata: CoreSessionMetadata::new(),
            worker_runtime: FakeSessionWorkerRuntime::new(),
        })
        .expect("spawn through command facade");
    assert!(matches!(spawn, EngineCommandOutcome::SpawnSession(_)));
    engine
        .execute_command(EngineCommand::AttachClient {
            client_id: client_id("client-hot"),
            session_id: session_id(),
            subscription_id: subscription_id("sub-hot"),
            now_seconds: 10,
        })
        .expect("attach through command facade while capabilities saturated");
    let input = engine
        .execute_command(EngineCommand::SendInput {
            client_id: client_id("client-hot"),
            session_id: session_id(),
            data: b"status\n".to_vec(),
            now_seconds: 11,
        })
        .expect("input through command facade while capabilities saturated");
    assert!(matches!(
        input,
        EngineCommandOutcome::Output(output)
            if output.session_requests.iter().any(|(_, request)| {
                matches!(request, SessionIoRequest::PtyInput { data, .. } if data == b"status\n")
            })
    ));
    let sessions = engine
        .execute_command(EngineCommand::ListSessions)
        .expect("list sessions while capabilities saturated");
    assert!(matches!(
        sessions,
        EngineCommandOutcome::Sessions(sessions)
            if sessions.len() == 1 && sessions[0].session_id == session_id()
    ));
    engine
        .receive_output(session_id(), b"ready".to_vec(), 12)
        .expect("session output still drains");
    let inspect = engine
        .execute_command(EngineCommand::InspectSession {
            session_id: session_id(),
            now_seconds: 13,
            active_threshold_seconds: 5,
        })
        .expect("inspect session while capabilities saturated");
    assert!(matches!(
        inspect,
        EngineCommandOutcome::Inspection(inspection)
            if inspection.session.lifecycle == SessionLifecycleState::Running
                && inspection.activity_status == SessionActivityStatus::Active
    ));
    let screen = engine
        .execute_command(EngineCommand::ReadScreen {
            request_id: request_id("screen-under-load"),
            session_id: session_id(),
            now_seconds: 14,
        })
        .expect("read screen while capabilities saturated");
    assert!(matches!(
        screen,
        EngineCommandOutcome::Output(output)
            if matches!(
                output.session_events.first(),
                Some(SessionIoEvent::ScreenReady(screen))
                    if screen.request_id == request_id("screen-under-load")
                        && screen.text == "screen"
            )
    ));
    let snapshot = engine
        .execute_command(EngineCommand::CaptureSnapshot {
            request_id: request_id("snapshot-under-load"),
            session_id: session_id(),
            now_seconds: 15,
        })
        .expect("capture snapshot while capabilities saturated");
    assert!(matches!(
        snapshot,
        EngineCommandOutcome::Output(output)
            if matches!(
                output.session_events.first(),
                Some(SessionIoEvent::SnapshotReady(snapshot))
                    if snapshot.request_id == request_id("snapshot-under-load")
                        && snapshot.data == b"snapshot"
            )
    ));

    let timer_pressure = engine.drain_plugin_timers_due(10);
    let repeated_timer_pressure = engine.drain_plugin_timers_due(30);
    assert!(timer_pressure.events.iter().any(|event| matches!(
        event,
        PluginTimerEvent::Fired {
            result: PluginInvocationResult::Failed(_),
            ..
        }
    )));
    assert!(repeated_timer_pressure.events.iter().any(|event| matches!(
        event,
        PluginTimerEvent::Coalesced {
            timer_id,
            skipped_ticks,
            ..
        } if timer_id.0 == "interval-pressure" && *skipped_ticks > 0
    )));
    let unrelated = engine.invoke_plugin(plugin_invocation(
        "unrelated-under-load",
        unrelated_handler,
        250,
    ));
    assert!(matches!(
        unrelated.result,
        PluginInvocationResult::Completed(_)
    ));
    assert_eq!(unrelated_runtime.invocations().len(), 1);

    let http_pressure_event = http
        .drain_events(&saturated_plugin)
        .expect("HTTP pressure event")
        .into_iter()
        .find(|event| matches!(event, CapabilityRuntimeEvent::Backpressure(_)))
        .expect("HTTP typed backpressure");
    assert!(matches!(
        http_pressure_event,
        CapabilityRuntimeEvent::Backpressure(summary)
            if summary.source == QueueSource::PluginWorker
                && summary.capacity == 1
                && summary.route.plugin_key == Some(saturated_plugin.clone())
    ));
    assert!(fs_fake
        .drain_events(&saturated_plugin)
        .expect("fs events")
        .iter()
        .any(|event| matches!(event, CapabilityRuntimeEvent::Backpressure(_))));
    assert!(store_fake
        .drain_events(&saturated_plugin)
        .expect("store events")
        .iter()
        .any(|event| matches!(event, CapabilityRuntimeEvent::Backpressure(_))));

    http.release_resource(http_handle.resource.expect("HTTP resource"))
        .expect("cleanup slow HTTP resource");
    for _ in 0..50 {
        if blocking_transport.cancelled.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(blocking_transport.cancelled.load(Ordering::SeqCst), 1);
}

#[test]
fn active_noisy_watcher_unload_releases_only_owned_resources_without_blocking_reload() {
    let noisy_plugin = plugin_key("project-pipelines");
    let stable_plugin = plugin_key("preview");
    let mut watch = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 4,
            event_capacity: 2,
            debounce_ms: 0,
        },
    )
    .expect("watch runtime config");
    watch.grant_capability(
        noisy_plugin.clone(),
        capability(CapabilitySurface::Filesystem, "workspace"),
    );
    watch.grant_capability(
        stable_plugin.clone(),
        capability(CapabilitySurface::Filesystem, "workspace"),
    );

    let noisy_watch = watch
        .submit(watch_request(&noisy_plugin, "watch-noisy", "src"))
        .expect("noisy watch accepted")
        .resource
        .expect("noisy watch resource");
    let stable_watch = watch
        .submit(watch_request(&stable_plugin, "watch-stable", "docs"))
        .expect("stable watch accepted")
        .resource
        .expect("stable watch resource");
    watch.source_mut().emit(FileWatchSourceEvent::path(
        noisy_watch.clone(),
        ScopedRelativePath("src/lib.rs".to_string()),
        WatchChangeKind::Modified,
        1,
    ));
    watch
        .source_mut()
        .emit(FileWatchSourceEvent::overflow(noisy_watch.clone(), 2));

    let engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::with_plugin_config(
            FakeSessionRuntime::new(),
            PluginWorkerEngineConfig {
                per_plugin_capacity: 1,
            },
        );
    let noisy_handler = command_handler(&noisy_plugin, "run");
    let stable_handler = command_handler(&stable_plugin, "run");
    let noisy_runtime = RecordingRuntime::default();
    let noisy_invocations = noisy_runtime.invocations.clone();
    let noisy_stopped = noisy_runtime.stopped.clone();
    let stable_runtime = FakePluginRuntime::success("stable");
    engine.load_plugin(PluginWorkerRegistration {
        runtime: Arc::new(noisy_runtime),
        ..plugin_registration(
            FakePluginRuntime::success("placeholder"),
            &noisy_plugin,
            &noisy_handler,
        )
    });
    engine.load_plugin(plugin_registration(
        stable_runtime.clone(),
        &stable_plugin,
        &stable_handler,
    ));

    let cleanup = watch
        .cleanup_plugin(&noisy_plugin)
        .expect("noisy watch cleanup completes");
    let unload = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-noisy"),
        plugin_key: noisy_plugin.clone(),
        cleanup: botster_core::PluginCleanupScope::DescriptorsAndResources,
    });
    engine.load_plugin(PluginWorkerRegistration {
        runtime: Arc::new(RecordingRuntime::default()),
        ..plugin_registration(
            FakePluginRuntime::success("placeholder"),
            &noisy_plugin,
            &noisy_handler,
        )
    });
    let stable = engine.invoke_plugin(plugin_invocation(
        "stable-after-noisy-reload",
        stable_handler,
        250,
    ));

    assert_eq!(cleanup.removed_resources, vec![noisy_watch.clone()]);
    assert_eq!(watch.source().unregistered, vec![noisy_watch]);
    assert!(watch.release_resource(stable_watch).is_ok());
    assert_eq!(unload.request_id, request_id("unload-noisy"));
    assert_eq!(unload.plugin_key, noisy_plugin);
    assert_eq!(
        noisy_stopped.lock().expect("stopped lock").as_slice(),
        std::slice::from_ref(&noisy_plugin)
    );
    assert!(noisy_invocations
        .lock()
        .expect("invocation lock")
        .is_empty());
    assert!(matches!(
        stable.result,
        PluginInvocationResult::Completed(_)
    ));
    assert_eq!(stable_runtime.invocations().len(), 1);
    let late_watch_events = watch
        .drain_events(&noisy_plugin)
        .expect("late noisy watch events removed");
    assert!(late_watch_events
        .iter()
        .all(|event| !matches!(event, CapabilityRuntimeEvent::Watch(_))));
}

fn event_plugin_key(event: &CapabilityRuntimeEvent) -> Option<PluginKey> {
    match event {
        CapabilityRuntimeEvent::Completed(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::ResourceOpened(event)
        | CapabilityRuntimeEvent::ResourceReleased(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::TimedOut(event)
        | CapabilityRuntimeEvent::Cancelled(event)
        | CapabilityRuntimeEvent::Failed(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::WebSocketMessage(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::Watch(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::TimerFired(event) => Some(event.resource.plugin_key.clone()),
        CapabilityRuntimeEvent::CleanupCompleted(event) => Some(event.plugin_key.clone()),
        CapabilityRuntimeEvent::Backpressure(event) => event.route.plugin_key.clone(),
    }
}

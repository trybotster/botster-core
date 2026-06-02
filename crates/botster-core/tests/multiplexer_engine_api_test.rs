//! Public multiplexer engine API acceptance tests.

use std::sync::Arc;

use botster_core::{
    BotsterEngine, BoundaryJson, CoreSessionMetadata, ExtensionEntrypoint, ExtensionKind,
    ExtensionRuntime, MailboxSendFailureReason, MultiplexerEngine, MultiplexerEngineObservation,
    NotificationContent, NotificationItem, NotificationSeverity, NotificationSource,
    NotificationTarget, NotificationTimestamp, PackageManifest, PluginDescriptorKind,
    PluginDescriptorRef, PluginHandlerKind, PluginHandlerRef, PluginHandlerRegistration,
    PluginInvocationContext, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginLoadSpec, PluginOwnedDescriptor, PluginWorkerEvent,
    PluginWorkerRegistration, QueueSource, RequestId, SessionActivityStatus, SessionId,
    SessionLifecycleState, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, SubscriptionMultiplexerObservation, TransportEgress, TransportIngress,
};
use botster_core_test_support::fake::{
    FakePluginBehavior, FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
};

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("session-api-1".to_string())
}

fn client_id(value: &str) -> botster_core::ClientId {
    botster_core::ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("spawn-1"),
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

fn plugin_key() -> PluginKey {
    PluginKey("api-test-plugin".to_string())
}

fn plugin_handler(plugin_key: &PluginKey) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "run".to_string(),
    }
}

fn plugin_manifest(plugin_key: &PluginKey) -> PackageManifest {
    PackageManifest {
        name: plugin_key.0.clone(),
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
    }
}

fn plugin_registration(
    runtime: FakePluginRuntime,
    plugin_key: &PluginKey,
    handler: &PluginHandlerRef,
) -> PluginWorkerRegistration {
    PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: plugin_key.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![PluginOwnedDescriptor {
                descriptor: PluginDescriptorRef {
                    plugin_key: plugin_key.clone(),
                    kind: PluginDescriptorKind::Command,
                    descriptor_id: "run".to_string(),
                },
                handler: Some(handler.clone()),
                body: BoundaryJson(serde_json::json!({ "title": "Run" })),
            }],
            metadata: None,
        },
        manifest: plugin_manifest(plugin_key),
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    }
}

fn plugin_invocation(handler: PluginHandlerRef) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: request_id("plugin-1"),
        handler,
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: Some(client_id("client-a")),
            session_id: Some(session_id()),
            subscription_id: Some(subscription_id("sub-a")),
            surface_id: None,
            origin: Some("multiplexer-engine-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "command": "run" })),
    }
}

fn plugin_invocation_with_timeout(
    request_id_value: &str,
    handler: PluginHandlerRef,
    timeout_ms: u64,
) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: request_id(request_id_value),
        timeout_ms,
        ..plugin_invocation(handler)
    }
}

#[test]
fn multiplexer_invoke_plugin_exposes_timeout_and_backpressure_events() {
    let engine: MultiplexerEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        MultiplexerEngine::with_plugin_config(
            FakeSessionRuntime::new(),
            botster_core::PluginWorkerEngineConfig {
                per_plugin_capacity: 1,
            },
        );
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::new(FakePluginBehavior::WaitForCancellation);
    engine.load_plugin(plugin_registration(
        plugin_runtime.clone(),
        &plugin,
        &handler,
    ));

    let timeout = engine.invoke_plugin(plugin_invocation_with_timeout(
        "plugin-timeout",
        handler.clone(),
        10,
    ));
    assert!(matches!(
        timeout.result,
        PluginInvocationResult::Failed(failure)
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
    assert!(matches!(
        timeout.events.as_slice(),
        [PluginWorkerEvent::InvocationTimedOut(failure)]
            if failure.request_id == request_id("plugin-timeout")
                && failure.kind == PluginInvocationFailureKind::TimedOut
    ));

    let cancellation_deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    while plugin_runtime.cancellations_observed() == 0
        && std::time::Instant::now() < cancellation_deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(plugin_runtime.cancellations_observed(), 1);

    let late_runtime = FakePluginRuntime::new(FakePluginBehavior::Delay {
        duration: std::time::Duration::from_millis(100),
        payload: BoundaryJson(serde_json::json!({ "value": "late" })),
    });
    engine.load_plugin(plugin_registration(late_runtime, &plugin, &handler));
    let first = engine.invoke_plugin(plugin_invocation_with_timeout(
        "plugin-timeout-2",
        handler.clone(),
        10,
    ));
    assert!(matches!(
        first.result,
        PluginInvocationResult::Failed(failure)
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
    let pressured = engine.invoke_plugin(plugin_invocation_with_timeout(
        "plugin-pressured",
        handler,
        10,
    ));
    assert!(matches!(
        pressured.events.as_slice(),
        [PluginWorkerEvent::Backpressure(summary)]
            if summary.capacity == 1 && summary.depth == 1
    ));
}

#[test]
fn multiplexer_engine_drives_spawn_attach_output_notification_plugin_activity_and_shutdown() {
    let mut engine: MultiplexerEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        MultiplexerEngine::new(FakeSessionRuntime::new());
    let spawn = engine
        .spawn_session(
            spawn_request(),
            CoreSessionMetadata::new(),
            FakeSessionWorkerRuntime::new(),
        )
        .expect("spawn through public engine");

    assert_eq!(spawn.handle.session_id, session_id());
    assert_eq!(spawn.session.lifecycle, SessionLifecycleState::Running);
    assert_eq!(
        spawn.observations,
        vec![MultiplexerEngineObservation::SessionLifecycle {
            session_id: session_id(),
            state: SessionLifecycleState::Running,
        }]
    );
    assert_eq!(engine.session_runtime().spawned().len(), 1);

    engine
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::SubscribeSession {
                client_id: client_id("client-a"),
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
            },
            10,
        )
        .expect("client a subscribes");
    engine
        .handle_client_ingress(
            client_id("client-b"),
            TransportIngress::SubscribeSession {
                client_id: client_id("client-b"),
                session_id: session_id(),
                subscription_id: subscription_id("sub-b"),
            },
            10,
        )
        .expect("client b subscribes");

    let output = engine
        .handle_runtime_event(botster_core::SessionWorkerRuntimeEvent::TerminalBytes {
            session_id: session_id(),
            data: b"hello clients".to_vec(),
            last_output_at: 20,
        })
        .expect("runtime output fanout");

    assert_eq!(
        output.client_egress,
        vec![
            (
                client_id("client-a"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-a"),
                    data: b"hello clients".to_vec(),
                },
            ),
            (
                client_id("client-b"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-b"),
                    data: b"hello clients".to_vec(),
                },
            ),
        ]
    );

    let notification = NotificationItem::message(
        botster_core::NotificationId("notice-1".to_string()),
        NotificationTarget::Session(session_id()),
        NotificationSeverity::Info,
        NotificationSource {
            label: "core-test".to_string(),
            plugin_key: None,
        },
        NotificationContent {
            title: "Session notice".to_string(),
            body: Some("Ready".to_string()),
            extension: None,
        },
        NotificationTimestamp(30),
    );
    let notification_id = engine.post_notification(notification);
    let drained = engine.drain_notifications(
        NotificationTarget::Session(session_id()),
        NotificationTimestamp(31),
    );
    assert_eq!(
        notification_id,
        botster_core::NotificationId("notice-1".to_string())
    );
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].id, notification_id);

    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::success("ok");
    engine.load_plugin(plugin_registration(
        plugin_runtime.clone(),
        &plugin,
        &handler,
    ));

    let plugin_result = engine.invoke_plugin(plugin_invocation(handler.clone()));
    assert_eq!(plugin_runtime.invocations().len(), 1);
    match plugin_result.result {
        PluginInvocationResult::Completed(success) => {
            assert_eq!(success.request_id, request_id("plugin-1"));
            assert_eq!(success.handler, handler);
            assert_eq!(
                success.payload,
                Some(BoundaryJson(serde_json::json!({ "value": "ok" })))
            );
        }
        other => panic!("expected plugin success, got {other:?}"),
    }

    assert_eq!(
        engine
            .classify_session_activity(&session_id(), 21, 5)
            .expect("classify activity"),
        SessionActivityStatus::Active
    );

    let shutdown = engine
        .shutdown_session(session_id(), "host requested shutdown", 40)
        .expect("shutdown through public engine");
    assert!(shutdown.observations.iter().any(|observation| {
        observation
            == &MultiplexerEngineObservation::SessionLifecycle {
                session_id: session_id(),
                state: SessionLifecycleState::Stopping,
            }
    }));
    assert_eq!(
        engine
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Stopping)
    );
    assert_eq!(
        engine
            .handle_session_request(
                botster_core::SessionIoRequest::PtyInput {
                    session_id: session_id(),
                    data: b"ignored".to_vec(),
                },
                41,
            )
            .expect("post-shutdown input is ignored by closed worker")
            .session_events,
        Vec::new()
    );

    let post_shutdown = engine
        .handle_runtime_event(botster_core::SessionWorkerRuntimeEvent::TerminalBytes {
            session_id: session_id(),
            data: b"late".to_vec(),
            last_output_at: 42,
        })
        .expect("post-shutdown output is accepted by facade");
    assert!(post_shutdown.client_egress.is_empty());
    assert_eq!(
        engine
            .classify_session_activity(&session_id(), 43, 5)
            .expect("late closed output does not refresh activity"),
        SessionActivityStatus::Idle
    );

    let worker = engine
        .handle_session_request(
            botster_core::SessionIoRequest::Shutdown {
                session_id: session_id(),
                reason: "already closed".to_string(),
            },
            43,
        )
        .expect("closed worker ignores duplicate shutdown");
    assert!(worker.session_events.is_empty());
}

#[test]
fn multiplexer_engine_routes_client_input_to_session_worker() {
    let mut engine: MultiplexerEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        MultiplexerEngine::new(FakeSessionRuntime::new());
    engine
        .spawn_session(
            spawn_request(),
            CoreSessionMetadata::new(),
            FakeSessionWorkerRuntime::new(),
        )
        .expect("spawn through public engine");
    engine
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::SubscribeSession {
                client_id: client_id("client-a"),
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
            },
            10,
        )
        .expect("subscribe before input");

    engine
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::TerminalInput {
                session_id: session_id(),
                data: b"ls\n".to_vec(),
            },
            11,
        )
        .expect("route input through worker");

    let commands = engine
        .handle_session_request(
            botster_core::SessionIoRequest::Shutdown {
                session_id: session_id(),
                reason: "inspect worker command path".to_string(),
            },
            12,
        )
        .expect("shutdown for inspection");
    assert!(commands.session_events.iter().any(|event| {
        matches!(
            event,
            botster_core::SessionIoEvent::Shutdown {
                reason,
                ..
            } if reason == "inspect worker command path"
        )
    }));
    assert_eq!(
        engine
            .session(&session_id())
            .map(|session| session.activity.last_input_at),
        Some(Some(11))
    );
}

#[test]
fn botster_engine_facade_reports_route_pressure_and_preserves_healthy_fanout() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    engine
        .spawn_session(
            spawn_request(),
            CoreSessionMetadata::new(),
            FakeSessionWorkerRuntime::new(),
        )
        .expect("spawn through public engine");
    engine
        .attach_client(
            client_id("client-a"),
            session_id(),
            subscription_id("sub-a"),
            10,
        )
        .expect("client a subscribes");
    engine
        .attach_client(
            client_id("client-b"),
            session_id(),
            subscription_id("sub-b"),
            10,
        )
        .expect("client b subscribes");

    let pressure = engine
        .report_backpressure(
            client_id("client-a"),
            session_id(),
            QueueSource::ClientWorker,
            512,
            511,
        )
        .expect("pressure through public facade");
    let lag = engine
        .report_delivery_lag(
            client_id("client-a"),
            session_id(),
            subscription_id("sub-a"),
            QueueSource::TransportAdapter,
            512,
            128,
        )
        .expect("lag through public facade");
    let drop = engine
        .report_delivery_failure(
            client_id("client-a"),
            session_id(),
            subscription_id("sub-a"),
            QueueSource::ClientWorker,
            MailboxSendFailureReason::QueueFull,
        )
        .expect("drop through public facade");
    let closed = engine
        .report_delivery_failure(
            client_id("client-a"),
            session_id(),
            subscription_id("sub-a"),
            QueueSource::ClientWorker,
            MailboxSendFailureReason::QueueClosed,
        )
        .expect("closed through public facade");

    assert_eq!(pressure.client_control_frames.len(), 1);
    assert!(lag.observations.iter().any(|observation| {
        matches!(
            observation,
            MultiplexerEngineObservation::Subscription(
                SubscriptionMultiplexerObservation::DeliveryLagged {
                    client_id: observed_client_id,
                    lag
                }
            ) if observed_client_id == &client_id("client-a")
                && lag.route.subscription_id == Some(subscription_id("sub-a"))
        )
    }));
    assert!(drop.observations.iter().any(|observation| {
        matches!(
            observation,
            MultiplexerEngineObservation::Subscription(
                SubscriptionMultiplexerObservation::DeliveryFailed { failure, .. }
            ) if failure.reason == MailboxSendFailureReason::QueueFull
                && failure.route.subscription_id == Some(subscription_id("sub-a"))
        )
    }));
    assert!(closed.observations.iter().any(|observation| {
        matches!(
            observation,
            MultiplexerEngineObservation::Subscription(
                SubscriptionMultiplexerObservation::DeliveryFailed { failure, .. }
            ) if failure.reason == MailboxSendFailureReason::QueueClosed
                && failure.route.subscription_id == Some(subscription_id("sub-a"))
        )
    }));

    let output = engine
        .receive_output(session_id(), b"healthy path".to_vec(), 20)
        .expect("runtime output still fans out");
    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-b"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-b"),
                data: b"healthy path".to_vec(),
            },
        )]
    );
}

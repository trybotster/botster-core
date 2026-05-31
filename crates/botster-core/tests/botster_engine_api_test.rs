//! Public ergonomic Botster engine API acceptance tests.

use std::sync::Arc;

use botster_core::{
    BotsterEngine, BotsterEngineObservation, BoundaryJson, CoreSessionMetadata,
    ExtensionEntrypoint, ExtensionKind, ExtensionRuntime, NotificationContent, NotificationItem,
    NotificationSeverity, NotificationSource, NotificationTarget, NotificationTimestamp,
    PackageManifest, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginLoadSpec, PluginOwnedDescriptor,
    PluginWorkerRegistration, RequestId, SessionActivityStatus, SessionId, SessionIoRequest,
    SessionLifecycleState, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TransportEgress,
};
use botster_core_test_support::fake::{
    FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
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
            origin: Some("botster-engine-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "command": "run" })),
    }
}

#[test]
fn botster_engine_consumer_lifecycle_uses_public_api() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let spawn = engine
        .spawn_session(
            spawn_request(),
            CoreSessionMetadata::new(),
            FakeSessionWorkerRuntime::new(),
        )
        .expect("spawn through public BotsterEngine");

    assert_eq!(spawn.handle.session_id, session_id());
    assert_eq!(spawn.session.lifecycle, SessionLifecycleState::Running);
    assert_eq!(
        spawn.observations,
        vec![BotsterEngineObservation::SessionLifecycle {
            session_id: session_id(),
            state: SessionLifecycleState::Running,
        }]
    );
    assert_eq!(engine.session_runtime().spawned().len(), 1);

    engine
        .attach_client(
            client_id("client-a"),
            session_id(),
            subscription_id("sub-a"),
            10,
        )
        .expect("client a attaches through public API");
    engine
        .attach_client(
            client_id("client-b"),
            session_id(),
            subscription_id("sub-b"),
            10,
        )
        .expect("client b attaches through public API");

    let input = engine
        .write_bytes(client_id("client-a"), session_id(), b"ls\n".to_vec(), 11)
        .expect("write bytes through public API");
    assert!(input.session_requests.iter().any(|(_, request)| {
        matches!(request, SessionIoRequest::PtyInput { data, .. } if data == b"ls\n")
    }));

    let resize = engine
        .resize(client_id("client-a"), session_id(), 40, 120, 12)
        .expect("resize through public API");
    assert!(resize.session_requests.iter().any(|(_, request)| {
        matches!(
            request,
            SessionIoRequest::Resize {
                rows: 40,
                cols: 120,
                ..
            }
        )
    }));

    let output = engine
        .receive_output(session_id(), b"hello clients".to_vec(), 20)
        .expect("receive output through public API");
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
    match plugin_result {
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
            .classify_activity(&session_id(), 21, 5)
            .expect("classify activity through public API"),
        SessionActivityStatus::Active
    );

    engine
        .detach_client(
            client_id("client-b"),
            session_id(),
            subscription_id("sub-b"),
            30,
        )
        .expect("detach client through public API");
    let after_detach = engine
        .receive_output(session_id(), b"client a only".to_vec(), 31)
        .expect("post-detach output fanout");
    assert_eq!(
        after_detach.client_egress,
        vec![(
            client_id("client-a"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
                data: b"client a only".to_vec(),
            },
        )]
    );

    let shutdown = engine
        .shutdown_session(session_id(), "host requested shutdown", 40)
        .expect("shutdown through public API");
    assert!(shutdown.observations.iter().any(|observation| {
        observation
            == &BotsterEngineObservation::SessionLifecycle {
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

    let post_shutdown = engine
        .receive_output(session_id(), b"late".to_vec(), 42)
        .expect("closed worker ignores late output");
    assert!(post_shutdown.client_egress.is_empty());
    assert_eq!(
        engine
            .classify_activity(&session_id(), 43, 5)
            .expect("late closed output does not refresh activity"),
        SessionActivityStatus::Idle
    );
}

#[test]
fn botster_engine_returns_typed_error_for_unknown_session() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let error = engine
        .write_bytes(client_id("client-a"), session_id(), b"ls\n".to_vec(), 1)
        .expect_err("unknown session should be typed");

    assert_eq!(
        error,
        botster_core::BotsterEngineError::UnknownSession {
            session_id: session_id()
        }
    );
}

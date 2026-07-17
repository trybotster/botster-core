//! Public ergonomic Botster engine API acceptance tests.

use std::sync::Arc;
#[cfg(feature = "local-runtime")]
use std::thread;
use std::time::Duration;
#[cfg(feature = "local-runtime")]
use std::time::Instant;

use botster_core::{
    BotsterEngine, BotsterEngineObservation, BoundaryJson, Capability, CapabilitySurface,
    CoreSessionMetadata, EngineCommand, EngineCommandError, EngineCommandKind,
    EngineCommandOutcome, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime,
    InitialSnapshotReady, NotificationContent, NotificationItem, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp, PackageManifest,
    PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult, PluginKey,
    PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef,
    PluginUnloadSpec, PluginWorkerEvent, PluginWorkerRegistration, PreparedSnapshotRequest,
    ProcessExitedPayload, QueueSource, RequestId, SessionActivityStatus, SessionId, SessionIoEvent,
    SessionIoRequest, SessionLifecycleState, SessionSpawnRequest, SessionWorkerRuntimeEvent,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TransportEgress, ENGINE_COMMAND_KINDS,
};
#[cfg(feature = "local-runtime")]
use botster_core::{DefaultBotsterEngine, DefaultEngineCommand, ResizePayload};
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
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: None,
        configuration: None,
        runnable_entrypoints: Vec::new(),
        surfaces: Vec::new(),
        navigation: Vec::new(),
    }
}

fn network_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("api".to_string()),
    }
}

fn plugin_resource(
    plugin_key: &PluginKey,
    kind: PluginResourceKind,
    resource_id: &str,
) -> PluginResourceRef {
    PluginResourceRef {
        plugin_key: plugin_key.clone(),
        kind,
        resource_id: resource_id.to_string(),
    }
}

fn plugin_registration(
    runtime: FakePluginRuntime,
    plugin_key: &PluginKey,
    handler: &PluginHandlerRef,
) -> PluginWorkerRegistration {
    plugin_registration_with(runtime, plugin_key, handler, Vec::new(), None, Vec::new())
}

fn plugin_registration_with(
    runtime: FakePluginRuntime,
    plugin_key: &PluginKey,
    handler: &PluginHandlerRef,
    capabilities: Vec<Capability>,
    required_capability: Option<Capability>,
    resources: Vec<PluginResourceRef>,
) -> PluginWorkerRegistration {
    let mut manifest = plugin_manifest(plugin_key);
    manifest.capabilities = capabilities;

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
        manifest,
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability,
        }],
        resources,
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
fn engine_command_load_plugin_then_invoke_dispatches_through_worker() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::success("ok");

    let loaded = engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration(plugin_runtime.clone(), &plugin, &handler),
        })
        .expect("load plugin through command facade");
    assert_eq!(loaded, EngineCommandOutcome::PluginLoaded(plugin.clone()));

    let invoked = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation(handler.clone()),
        })
        .expect("invoke plugin through command facade");

    assert_eq!(plugin_runtime.invocations().len(), 1);
    match invoked {
        EngineCommandOutcome::PluginInvoked(outcome) => match outcome.result {
            PluginInvocationResult::Completed(success) => {
                assert_eq!(success.handler, handler);
                assert_eq!(
                    success.payload,
                    Some(BoundaryJson(serde_json::json!({ "value": "ok" })))
                );
            }
            other => panic!("expected command plugin success, got {other:?}"),
        },
        other => panic!("expected plugin invocation outcome, got {other:?}"),
    }
}

#[test]
fn engine_command_reload_plugin_returns_target_cleanup_and_replaces_runtime() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin = plugin_key();
    let other_plugin = PluginKey("api-other-plugin".to_string());
    let handler = plugin_handler(&plugin);
    let other_handler = plugin_handler(&other_plugin);
    let first_runtime = FakePluginRuntime::success("first");
    let replacement_runtime = FakePluginRuntime::success("replacement");
    let other_runtime = FakePluginRuntime::success("other");

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration_with(
                first_runtime.clone(),
                &plugin,
                &handler,
                Vec::new(),
                None,
                vec![plugin_resource(
                    &plugin,
                    PluginResourceKind::McpRegistration,
                    "resource-a",
                )],
            ),
        })
        .expect("load target plugin");
    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration_with(
                other_runtime.clone(),
                &other_plugin,
                &other_handler,
                Vec::new(),
                None,
                vec![plugin_resource(
                    &other_plugin,
                    PluginResourceKind::McpRegistration,
                    "resource-b",
                )],
            ),
        })
        .expect("load other plugin");

    let replacement_registration =
        plugin_registration(replacement_runtime.clone(), &plugin, &handler);
    let reload = engine
        .execute_command(EngineCommand::ReloadPlugin {
            spec: PluginReloadSpec {
                request_id: request_id("reload-plugin-command"),
                plugin_key: plugin.clone(),
                load: replacement_registration.load.clone(),
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            },
            registration: replacement_registration,
        })
        .expect("reload plugin through command facade");

    match reload {
        EngineCommandOutcome::PluginReloaded(cleanup) => {
            assert_eq!(cleanup.request_id, request_id("reload-plugin-command"));
            assert_eq!(cleanup.plugin_key, plugin);
            assert_eq!(cleanup.removed_descriptors.len(), 1);
            assert_eq!(cleanup.removed_resources.len(), 1);
            assert!(cleanup
                .removed_resources
                .iter()
                .all(|resource| resource.plugin_key == plugin));
        }
        other => panic!("expected plugin reload cleanup, got {other:?}"),
    }
    assert_eq!(first_runtime.stopped(), vec![plugin.clone()]);

    let invoked = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation(handler.clone()),
        })
        .expect("invoke replacement plugin");
    assert!(matches!(
        invoked,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                &outcome.result,
                PluginInvocationResult::Completed(success)
                    if success.payload == Some(BoundaryJson(serde_json::json!({ "value": "replacement" })))
            )
    ));
    assert_eq!(replacement_runtime.invocations().len(), 1);
    assert!(other_runtime.stopped().is_empty());
    assert_eq!(
        engine.plugin_workers().descriptors_for(&other_plugin).len(),
        1
    );
}

#[test]
fn engine_command_unload_plugin_cleans_resources_and_rejects_later_invoke() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::success("ok");

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration_with(
                plugin_runtime.clone(),
                &plugin,
                &handler,
                Vec::new(),
                None,
                vec![plugin_resource(
                    &plugin,
                    PluginResourceKind::NetworkConnection,
                    "socket-1",
                )],
            ),
        })
        .expect("load plugin");

    let unloaded = engine
        .execute_command(EngineCommand::UnloadPlugin {
            spec: PluginUnloadSpec {
                request_id: request_id("unload-plugin-command"),
                plugin_key: plugin.clone(),
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            },
        })
        .expect("unload plugin through command facade");

    match unloaded {
        EngineCommandOutcome::PluginUnloaded(cleanup) => {
            assert_eq!(cleanup.request_id, request_id("unload-plugin-command"));
            assert_eq!(cleanup.plugin_key, plugin);
            assert_eq!(cleanup.removed_descriptors.len(), 1);
            assert_eq!(cleanup.removed_resources.len(), 1);
        }
        other => panic!("expected plugin unload cleanup, got {other:?}"),
    }
    assert_eq!(plugin_runtime.stopped(), vec![plugin.clone()]);

    let invoked = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation(handler),
        })
        .expect("invoke unloaded plugin returns typed worker failure");
    assert!(matches!(
        invoked,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                &outcome.result,
                PluginInvocationResult::Failed(failure)
                    if failure.kind == PluginInvocationFailureKind::WorkerStopped
            )
    ));
}

#[test]
fn engine_command_plugin_capability_rejection_uses_manifest_metadata() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let required = network_capability();
    let plugin_runtime = FakePluginRuntime::success("not-called");

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration_with(
                plugin_runtime.clone(),
                &plugin,
                &handler,
                Vec::new(),
                Some(required),
                Vec::new(),
            ),
        })
        .expect("load plugin with missing capability grant");

    let invoked = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation(handler.clone()),
        })
        .expect("capability rejection is a typed plugin outcome");
    assert!(matches!(
        invoked,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                &outcome.result,
                PluginInvocationResult::Failed(failure)
                    if failure.handler == handler
                        && failure.kind == PluginInvocationFailureKind::HandlerFailed
                        && failure.reason.contains("capability")
            )
    ));
    assert!(plugin_runtime.invocations().is_empty());
}

#[test]
fn engine_command_plugin_timeout_and_backpressure_events_surface() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::with_plugin_config(
            FakeSessionRuntime::new(),
            botster_core::PluginWorkerEngineConfig {
                per_plugin_capacity: 1,
            },
        );
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::new(FakePluginBehavior::Delay {
        duration: Duration::from_millis(100),
        payload: BoundaryJson(serde_json::json!({ "value": "late" })),
    });
    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration(plugin_runtime, &plugin, &handler),
        })
        .expect("load delayed plugin");

    let timeout = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation_with_timeout("command-timeout", handler.clone(), 10),
        })
        .expect("timeout returns plugin outcome");
    assert!(matches!(
        timeout,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                outcome.events.as_slice(),
                [PluginWorkerEvent::InvocationTimedOut(failure)]
                    if failure.request_id == request_id("command-timeout")
                        && failure.kind == PluginInvocationFailureKind::TimedOut
            )
    ));

    let pressured = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation_with_timeout("command-pressured", handler, 10),
        })
        .expect("backpressure returns plugin outcome");
    assert!(matches!(
        pressured,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                outcome.events.as_slice(),
                [PluginWorkerEvent::Backpressure(summary)]
                    if summary.source == QueueSource::PluginWorker
                        && summary.capacity == 1
                        && summary.depth == 1
            )
    ));
}

#[test]
fn engine_command_plugin_invoke_failure_releases_resources() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::failure("expected failure");

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration_with(
                plugin_runtime.clone(),
                &plugin,
                &handler,
                Vec::new(),
                None,
                vec![plugin_resource(
                    &plugin,
                    PluginResourceKind::FilesystemOperation,
                    "operation-1",
                )],
            ),
        })
        .expect("load failing plugin");

    let failed = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation(handler.clone()),
        })
        .expect("handler failure returns plugin outcome");
    assert!(matches!(
        failed,
        EngineCommandOutcome::PluginInvoked(outcome)
            if matches!(
                &outcome.result,
                PluginInvocationResult::Failed(failure)
                    if failure.kind == PluginInvocationFailureKind::HandlerFailed
            )
    ));

    let unload = engine
        .execute_command(EngineCommand::UnloadPlugin {
            spec: PluginUnloadSpec {
                request_id: request_id("unload-after-failure"),
                plugin_key: plugin.clone(),
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            },
        })
        .expect("unload after failed invocation");

    match unload {
        EngineCommandOutcome::PluginUnloaded(cleanup) => {
            assert_eq!(cleanup.plugin_key, plugin);
            assert_eq!(
                cleanup.removed_resources,
                vec![plugin_resource(
                    &plugin,
                    PluginResourceKind::FilesystemOperation,
                    "operation-1",
                )]
            );
        }
        other => panic!("expected unload cleanup after failure, got {other:?}"),
    }
}

#[test]
fn botster_engine_invoke_plugin_exposes_timeout_events() {
    let engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::with_plugin_config(
            FakeSessionRuntime::new(),
            botster_core::PluginWorkerEngineConfig {
                per_plugin_capacity: 1,
            },
        );
    let plugin = plugin_key();
    let handler = plugin_handler(&plugin);
    let plugin_runtime = FakePluginRuntime::new(FakePluginBehavior::Delay {
        duration: Duration::from_millis(100),
        payload: BoundaryJson(serde_json::json!({ "value": "late" })),
    });
    engine.load_plugin(plugin_registration(plugin_runtime, &plugin, &handler));

    let timeout = engine.invoke_plugin(plugin_invocation_with_timeout(
        "botster-plugin-timeout",
        handler,
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
            if failure.request_id == request_id("botster-plugin-timeout")
                && failure.kind == PluginInvocationFailureKind::TimedOut
    ));
}

#[cfg(feature = "local-runtime")]
fn default_spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("default-spawn-1"),
        session_id: SessionId("default-local-session-1".to_string()),
        executable: "/bin/sh".to_string(),
        arguments: vec![
            "-c".to_string(),
            "printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
                .to_string(),
        ],
        working_directory: SpawnWorkingDirectory {
            path: std::env::current_dir()
                .expect("current dir for default engine test")
                .display()
                .to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

#[cfg(feature = "local-runtime")]
fn drain_default_until(
    engine: &mut DefaultBotsterEngine,
    session_id: &SessionId,
    needle: &[u8],
    last_output_at: &mut u64,
) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = Vec::new();

    while Instant::now() < deadline {
        let outcome = engine
            .drain_runtime_once(session_id, *last_output_at)
            .expect("drain default runtime output");
        *last_output_at += 1;

        for (_, frame) in outcome.client_egress {
            if let TransportEgress::TerminalOutput { data, .. } = frame {
                observed.extend(data);
            }
        }

        if observed
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return observed;
        }

        thread::sleep(Duration::from_millis(20));
    }

    panic!(
        "timed out waiting for {:?} in {:?}",
        String::from_utf8_lossy(needle),
        String::from_utf8_lossy(&observed)
    );
}

#[cfg(feature = "local-runtime")]
fn drain_default_all_until(
    engine: &mut DefaultBotsterEngine,
    needle: &[u8],
    last_output_at: &mut u64,
) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = Vec::new();

    while Instant::now() < deadline {
        let outcome = engine
            .drain_runtime_all_once(*last_output_at)
            .expect("fair drain default runtime output");
        *last_output_at += 1;

        for (_, frame) in outcome.client_egress {
            if let TransportEgress::TerminalOutput { data, .. } = frame {
                observed.extend(data);
            }
        }

        if observed
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return observed;
        }

        thread::sleep(Duration::from_millis(20));
    }

    panic!(
        "timed out waiting for {:?} in {:?}",
        String::from_utf8_lossy(needle),
        String::from_utf8_lossy(&observed)
    );
}

#[cfg(feature = "local-runtime")]
#[test]
fn default_botster_engine_spawns_local_session_and_fans_out_output() {
    let mut engine = DefaultBotsterEngine::new();
    let request = default_spawn_request();
    let session_id = request.session_id.clone();
    let client_id = client_id("default-client");
    let subscription_id = subscription_id("default-subscription");
    let mut logical_clock = 20;

    let spawn = engine
        .execute_command(DefaultEngineCommand::SpawnSession {
            request,
            metadata: CoreSessionMetadata::new(),
        })
        .expect("spawn local default session");
    let EngineCommandOutcome::SpawnSession(spawn) = spawn else {
        panic!("expected spawn outcome");
    };
    assert_eq!(spawn.handle.session_id, session_id);
    assert_eq!(spawn.session.lifecycle, SessionLifecycleState::Running);

    engine
        .execute_command(DefaultEngineCommand::AttachClient {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            subscription_id,
            now_seconds: logical_clock,
        })
        .expect("attach client through default engine");
    logical_clock += 1;

    let ready = drain_default_until(&mut engine, &session_id, b"ready", &mut logical_clock);
    assert!(
        ready
            .windows(b"ready".len())
            .any(|window| window == b"ready"),
        "default engine should fan out local PTY startup output"
    );

    engine
        .execute_command(DefaultEngineCommand::SendInput {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            data: b"ping-default\n".to_vec(),
            now_seconds: logical_clock,
        })
        .expect("write input through default engine");
    logical_clock += 1;
    drain_default_until(
        &mut engine,
        &session_id,
        b"echo:ping-default",
        &mut logical_clock,
    );

    engine
        .execute_command(DefaultEngineCommand::Resize {
            client_id,
            session_id: session_id.clone(),
            rows: 30,
            cols: 100,
            now_seconds: logical_clock,
        })
        .expect("resize through default engine");
    logical_clock += 1;

    assert_eq!(
        engine
            .classify_activity(&session_id, logical_clock, 5)
            .expect("classify default runtime activity"),
        SessionActivityStatus::Active
    );

    let shutdown = engine
        .execute_command(DefaultEngineCommand::Shutdown {
            session_id: session_id.clone(),
            reason: "test complete".to_string(),
            now_seconds: logical_clock,
        })
        .expect("shutdown default runtime session");
    let EngineCommandOutcome::Output(shutdown) = shutdown else {
        panic!("expected shutdown output");
    };
    assert!(shutdown.observations.iter().any(|observation| {
        observation
            == &BotsterEngineObservation::SessionLifecycle {
                session_id: session_id.clone(),
                state: SessionLifecycleState::Stopping,
            }
    }));
    assert!(matches!(
        engine
            .session(&session_id)
            .map(|session| &session.lifecycle),
        Some(SessionLifecycleState::Stopping)
    ));
}

#[cfg(feature = "local-runtime")]
#[test]
fn default_botster_engine_exposes_fair_runtime_drain() {
    let mut engine = DefaultBotsterEngine::new();
    let request = default_spawn_request();
    let session_id = request.session_id.clone();
    let client_id = client_id("default-fair-client");
    let subscription_id = subscription_id("default-fair-subscription");
    let mut logical_clock = 20;

    engine
        .spawn_session(request, CoreSessionMetadata::new())
        .expect("spawn local default session");
    engine
        .attach_client(
            client_id.clone(),
            session_id.clone(),
            subscription_id,
            logical_clock,
        )
        .expect("attach client through default engine facade");
    logical_clock += 1;

    let ready = drain_default_all_until(&mut engine, b"ready", &mut logical_clock);
    assert!(
        ready
            .windows(b"ready".len())
            .any(|window| window == b"ready"),
        "fair default drain should fan out local PTY startup output"
    );

    engine
        .write_bytes(
            client_id,
            session_id.clone(),
            b"ping-fair-default\n".to_vec(),
            logical_clock,
        )
        .expect("write input through default engine facade");
    logical_clock += 1;
    drain_default_all_until(&mut engine, b"echo:ping-fair-default", &mut logical_clock);

    engine
        .shutdown_session(session_id, "fair drain test complete", logical_clock)
        .expect("shutdown default runtime session");
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

    engine
        .handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
            InitialSnapshotReady {
                request_id: request_id("initial-a"),
                session_id: session_id(),
                client_id: client_id("client-a"),
                subscription_id: subscription_id("sub-a"),
                snapshot: Vec::new(),
                rows: 24,
                cols: 80,
            },
        ))
        .expect("client a initial snapshot should release live output");
    engine
        .handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
            InitialSnapshotReady {
                request_id: request_id("initial-b"),
                session_id: session_id(),
                client_id: client_id("client-b"),
                subscription_id: subscription_id("sub-b"),
                snapshot: Vec::new(),
                rows: 24,
                cols: 80,
            },
        ))
        .expect("client b initial snapshot should release live output");

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
        .expect("stopping worker routes final output");
    assert_eq!(
        post_shutdown.client_egress,
        vec![(
            client_id("client-a"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
                data: b"late".to_vec(),
            },
        )]
    );
    assert_eq!(
        engine
            .classify_activity(&session_id(), 43, 5)
            .expect("final stopping output refreshes activity"),
        SessionActivityStatus::Active
    );

    engine
        .handle_runtime_event(SessionWorkerRuntimeEvent::ProcessExited {
            session_id: session_id(),
            payload: ProcessExitedPayload {
                exit_code: Some(0),
                signal: None,
            },
        })
        .expect("reader completion releases process exit");
    let after_exit = engine
        .receive_output(session_id(), b"too late".to_vec(), 44)
        .expect("closed worker ignores output after process exit");
    assert!(after_exit.client_egress.is_empty());
}

#[test]
fn engine_command_surface_uses_crate_root_facade_for_all_typed_commands() {
    assert_eq!(
        ENGINE_COMMAND_KINDS,
        &[
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
            EngineCommandKind::LoadPlugin,
            EngineCommandKind::ReloadPlugin,
            EngineCommandKind::UnloadPlugin,
            EngineCommandKind::InvokePlugin,
        ]
    );

    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let spawn = engine
        .execute_command(EngineCommand::SpawnSession {
            request: spawn_request(),
            metadata: CoreSessionMetadata::new(),
            worker_runtime: FakeSessionWorkerRuntime::new(),
        })
        .expect("spawn through command facade");
    let EngineCommandOutcome::SpawnSession(spawn) = spawn else {
        panic!("expected spawn outcome");
    };
    assert_eq!(spawn.handle.session_id, session_id());

    engine
        .execute_command(EngineCommand::AttachClient {
            client_id: client_id("client-command"),
            session_id: session_id(),
            subscription_id: subscription_id("sub-command"),
            now_seconds: 9,
        })
        .expect("attach through typed command");

    let input = engine
        .execute_command(EngineCommand::SendInput {
            client_id: client_id("client-command"),
            session_id: session_id(),
            data: b"command input\n".to_vec(),
            now_seconds: 10,
        })
        .expect("input through typed command");
    assert!(matches!(
        input,
        EngineCommandOutcome::Output(output)
            if output.session_requests.iter().any(|(_, request)| {
                matches!(request, SessionIoRequest::PtyInput { data, .. } if data == b"command input\n")
            })
    ));

    let resize = engine
        .execute_command(EngineCommand::Resize {
            client_id: client_id("client-command"),
            session_id: session_id(),
            rows: 40,
            cols: 120,
            now_seconds: 10,
        })
        .expect("resize through typed command");
    assert!(matches!(
        resize,
        EngineCommandOutcome::Output(output)
            if output.session_requests.iter().any(|(_, request)| {
                matches!(request, SessionIoRequest::Resize { rows: 40, cols: 120, .. })
            })
    ));

    let sessions = engine
        .execute_command(EngineCommand::ListSessions)
        .expect("list through typed command");
    assert!(matches!(
        sessions,
        EngineCommandOutcome::Sessions(sessions)
            if sessions.len() == 1 && sessions[0].session_id == session_id()
    ));

    engine
        .receive_output(session_id(), b"command output".to_vec(), 10)
        .expect("record output before command inspection");
    let inspection = engine
        .execute_command(EngineCommand::InspectSession {
            session_id: session_id(),
            now_seconds: 11,
            active_threshold_seconds: 5,
        })
        .expect("inspect through command facade");
    let EngineCommandOutcome::Inspection(inspection) = inspection else {
        panic!("expected inspection");
    };
    assert_eq!(inspection.session.session_id, session_id());
    assert_eq!(inspection.session.lifecycle, SessionLifecycleState::Running);
    assert_eq!(inspection.activity_status, SessionActivityStatus::Active);

    let screen = engine
        .execute_command(EngineCommand::ReadScreen {
            request_id: request_id("screen-command-1"),
            session_id: session_id(),
            now_seconds: 11,
        })
        .expect("read screen through command facade");
    assert!(matches!(
        screen,
        EngineCommandOutcome::Output(output)
            if matches!(
                output.session_events.first(),
                Some(SessionIoEvent::ScreenReady(screen))
                    if screen.request_id == request_id("screen-command-1") && screen.text == "screen"
            )
    ));

    let snapshot = engine
        .execute_command(EngineCommand::CaptureSnapshot {
            request_id: request_id("snapshot-command-1"),
            session_id: session_id(),
            now_seconds: 12,
        })
        .expect("capture snapshot through command facade");
    assert!(matches!(
        snapshot,
        EngineCommandOutcome::Output(output)
            if matches!(
                output.session_events.first(),
                Some(SessionIoEvent::SnapshotReady(snapshot))
                    if snapshot.request_id == request_id("snapshot-command-1")
                        && snapshot.data == b"snapshot"
            )
    ));

    let replay = engine
        .execute_command(EngineCommand::ReplaySnapshot {
            request: PreparedSnapshotRequest {
                request_id: request_id("replay-command-1"),
                session_id: session_id(),
                snapshot: b"prepared snapshot".to_vec(),
                recovery: true,
            },
            now_seconds: 13,
        })
        .expect("replay snapshot through command facade");
    assert!(matches!(
        replay,
        EngineCommandOutcome::Output(output)
            if matches!(
                output.session_events.first(),
                Some(SessionIoEvent::PreparedSnapshotReady(prepared))
                    if prepared.request_id == request_id("replay-command-1")
                        && prepared.payload == b"prepared snapshot"
                        && prepared.recovery
            )
    ));

    let notification = NotificationItem::message(
        botster_core::NotificationId("typed-notice-1".to_string()),
        NotificationTarget::Session(session_id()),
        NotificationSeverity::Info,
        NotificationSource {
            label: "typed-command-test".to_string(),
            plugin_key: None,
        },
        NotificationContent {
            title: "Typed notice".to_string(),
            body: None,
            extension: None,
        },
        NotificationTimestamp(30),
    );
    let posted = engine
        .execute_command(EngineCommand::PostNotification { item: notification })
        .expect("post notification through typed command");
    assert!(matches!(
        posted,
        EngineCommandOutcome::NotificationPosted(id)
            if id == botster_core::NotificationId("typed-notice-1".to_string())
    ));

    let drained = engine
        .execute_command(EngineCommand::DrainNotifications {
            target: NotificationTarget::Session(session_id()),
            now: NotificationTimestamp(31),
        })
        .expect("drain notifications through typed command");
    assert!(matches!(
        drained,
        EngineCommandOutcome::NotificationsDrained(items)
            if items.len() == 1 && items[0].id == botster_core::NotificationId("typed-notice-1".to_string())
    ));

    engine
        .execute_command(EngineCommand::DetachClient {
            client_id: client_id("client-command"),
            session_id: session_id(),
            subscription_id: subscription_id("sub-command"),
            now_seconds: 32,
        })
        .expect("detach through typed command");

    let shutdown = engine
        .execute_command(EngineCommand::Shutdown {
            session_id: session_id(),
            reason: "typed command complete".to_string(),
            now_seconds: 40,
        })
        .expect("shutdown through typed command");
    assert!(matches!(
        shutdown,
        EngineCommandOutcome::Output(output)
            if output.observations.iter().any(|observation| {
                observation
                    == &BotsterEngineObservation::SessionLifecycle {
                        session_id: session_id(),
                        state: SessionLifecycleState::Stopping,
                    }
            })
    ));
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

#[test]
fn engine_command_error_preserves_command_kind_and_typed_source() {
    let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let error = engine
        .execute_command(EngineCommand::SendInput {
            client_id: client_id("client-a"),
            session_id: session_id(),
            data: b"ls\n".to_vec(),
            now_seconds: 1,
        })
        .expect_err("unknown session should be typed");

    assert_eq!(error.kind, EngineCommandKind::SendInput);
    assert_eq!(
        error.source,
        botster_core::BotsterEngineError::UnknownSession {
            session_id: session_id()
        }
    );
    let _: EngineCommandError<botster_core::BotsterEngineError> = error;
}

#[cfg(feature = "local-runtime")]
#[test]
fn default_engine_command_replay_error_preserves_typed_unsupported_context() {
    let mut engine = DefaultBotsterEngine::new();
    let request = default_spawn_request();
    let session_id = request.session_id.clone();

    engine
        .execute_command(DefaultEngineCommand::SpawnSession {
            request,
            metadata: CoreSessionMetadata::new(),
        })
        .expect("spawn local default session");

    let error = engine
        .execute_command(DefaultEngineCommand::ReplaySnapshot {
            request: PreparedSnapshotRequest {
                request_id: request_id("default-replay-unsupported"),
                session_id,
                snapshot: b"snapshot".to_vec(),
                recovery: true,
            },
            now_seconds: 20,
        })
        .expect_err("default command should preserve unsupported replay");

    assert_eq!(error.kind, EngineCommandKind::ReplaySnapshot);
    assert!(matches!(
        error.source,
        botster_core::DefaultBotsterEngineError::UnsupportedSessionRequest {
            request_kind: "prepare_snapshot",
        }
    ));
}

//! Actor contract acceptance tests.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    PluginCleanupResult, PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef,
    PluginHandlerKind, PluginHandlerRef, PluginInvocationContext, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationSuccess, PluginKey,
    PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef,
    PluginUnloadSpec, PluginWorkerEvent, PluginWorkerMessage, QueueSource, SessionIoRequest,
    TransportConnectionMode, TransportSignal, PUBLIC_QUEUE_SOURCES,
};
use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};

fn request_id() -> RequestId {
    RequestId("req-1".to_string())
}

fn session_id() -> SessionId {
    SessionId("session-1".to_string())
}

fn client_id() -> ClientId {
    ClientId("client-1".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-1".to_string())
}

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn handler(plugin_key: PluginKey, kind: PluginHandlerKind, handler_id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key,
        kind,
        handler_id: handler_id.to_string(),
    }
}

fn descriptor(
    plugin_key: PluginKey,
    kind: PluginDescriptorKind,
    descriptor_id: &str,
    handler: Option<PluginHandlerRef>,
) -> PluginOwnedDescriptor {
    PluginOwnedDescriptor {
        descriptor: PluginDescriptorRef {
            plugin_key,
            kind,
            descriptor_id: descriptor_id.to_string(),
        },
        handler,
        body: BoundaryJson(serde_json::json!({ "label": descriptor_id })),
    }
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize contract value");
    serde_json::from_str(&json).expect("deserialize contract value")
}

#[test]
fn actor_queue_configs_are_bounded() {
    let names: Vec<_> = PUBLIC_QUEUE_SOURCES
        .into_iter()
        .map(QueueSource::default_config)
        .inspect(|config| assert!(config.is_bounded(), "{config:?}"))
        .map(|config| config.name)
        .collect();

    assert!(names.contains(&"hub-control".to_string()));
    assert!(names.contains(&"client-worker".to_string()));
    assert!(names.contains(&"session-io".to_string()));
    assert!(names.contains(&"transport-adapter".to_string()));
    assert!(names.contains(&"plugin-worker".to_string()));

    let unbounded = BoundedQueueConfig::new("test.unbounded", 0);
    assert!(!unbounded.is_bounded());
}

#[test]
fn stable_hub_and_client_controls_are_typed() {
    let attach = HubControlMessage::AttachClient {
        origin: HubControlOrigin::Client(client_id()),
        request_id: request_id(),
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id(),
    };
    let client = ClientWorkerMessage::Control {
        frame: ClientControlFrame::Health {
            health: ClientConnectionHealth::Healthy,
        },
    };
    let session = SessionIoRequest::Resize {
        session_id: session_id(),
        rows: 40,
        cols: 120,
    };
    let ingress = TransportIngress::RequestSnapshot {
        request_id: request_id(),
        session_id: session_id(),
    };
    let output = TransportEgress::TerminalOutput {
        session_id: session_id(),
        data: b"ok".to_vec(),
    };

    assert_eq!(attach, round_trip(&attach));
    assert_eq!(client, round_trip(&client));
    assert_eq!(session, round_trip(&session));
    assert_eq!(ingress, round_trip(&ingress));
    assert_eq!(output, round_trip(&output));
}

#[test]
fn backpressure_summary_round_trips_with_route_context() {
    let summary = BackpressureSummary {
        source: QueueSource::ClientWorker,
        capacity: 512,
        depth: 500,
        route: BackpressureRoute {
            session_id: Some(session_id()),
            client_id: Some(client_id()),
            subscription_id: Some(subscription_id()),
            plugin_key: None,
        },
    };

    let round_trip: BackpressureSummary = round_trip(&summary);

    assert_eq!(round_trip.source, QueueSource::ClientWorker);
    assert_eq!(round_trip.capacity, 512);
    assert_eq!(round_trip.route.session_id, Some(session_id()));
    assert_eq!(round_trip.route.client_id, Some(client_id()));
    assert_eq!(round_trip.route.subscription_id, Some(subscription_id()));
}

#[test]
fn boundary_json_is_reserved_for_lua_plugin_or_relay_payloads() {
    let payload = BoundaryJson(serde_json::json!({ "owned_by": "relay" }));
    let signal = HubControlMessage::TransportSignal(TransportSignal {
        peer_id: "peer-1".to_string(),
        mode: TransportConnectionMode::Relay,
        payload: payload.clone(),
    });
    let plugin = PluginWorkerMessage::Invoke(PluginInvocationRequest {
        request_id: request_id(),
        handler: handler(
            plugin_key("project-pipelines"),
            PluginHandlerKind::UiAction,
            "open",
        ),
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: Some(client_id()),
            session_id: None,
            subscription_id: None,
            surface_id: Some("home".to_string()),
            origin: Some("test".to_string()),
            metadata: None,
        },
        payload: payload.clone(),
    });
    let relay = TransportIngress::BoundaryPayload {
        route_id: "relay-1".to_string(),
        payload,
    };
    let stable_control = HubControlMessage::RequestSnapshot {
        request_id: request_id(),
        client_id: client_id(),
        session_id: session_id(),
    };

    assert!(format!("{signal:?}").contains("BoundaryJson"));
    assert!(format!("{plugin:?}").contains("BoundaryJson"));
    assert!(format!("{relay:?}").contains("BoundaryJson"));
    assert!(!format!("{stable_control:?}").contains("BoundaryJson"));
}

#[test]
fn session_and_client_contracts_do_not_depend_on_transport() {
    let session_source = std::fs::read_to_string("src/session.rs").expect("read session source");
    let client_source = std::fs::read_to_string("src/client.rs").expect("read client source");
    let actor_source = std::fs::read_to_string("src/actor.rs").expect("read actor source");

    assert!(!session_source.contains("crate::transport"));
    assert!(!client_source.contains("crate::transport"));

    for forbidden in [
        "crate::transport",
        "TransportIngress",
        "TransportEgress",
        "WebRtc",
        "Browser",
        "Socket",
        "Tui",
        "Rails",
        "DataChannel",
    ] {
        assert!(
            !actor_source.contains(forbidden),
            "actor session/client contracts must not mention {forbidden}"
        );
    }
}

#[test]
fn plugin_handler_refs_never_contain_function_values() {
    let kinds = [
        PluginHandlerKind::UiAction,
        PluginHandlerKind::SessionAction,
        PluginHandlerKind::Command,
        PluginHandlerKind::Hook,
        PluginHandlerKind::SurfaceRoute,
        PluginHandlerKind::AssetMessage,
        PluginHandlerKind::Timer,
        PluginHandlerKind::McpTool,
        PluginHandlerKind::McpPrompt,
        PluginHandlerKind::McpResource,
        PluginHandlerKind::McpProxyAuthError,
        PluginHandlerKind::Event,
        PluginHandlerKind::Http,
        PluginHandlerKind::Watch,
        PluginHandlerKind::ActionCable,
        PluginHandlerKind::EntityProvider,
        PluginHandlerKind::Notification,
    ];

    for kind in kinds {
        let reference = handler(plugin_key("project-pipelines"), kind, "stable-id");
        let json = serde_json::to_string(&reference).expect("serialize handler ref");
        let rendered = format!("{reference:?} {json}");

        assert!(rendered.contains("project-pipelines"));
        assert!(rendered.contains("stable-id"));
        for forbidden in ["function", "closure", "mlua", "Function"] {
            assert!(
                !rendered.contains(forbidden),
                "handler ref must not contain {forbidden}: {rendered}"
            );
        }
    }

    assert!(!std::fs::read_to_string("Cargo.toml")
        .expect("read Cargo.toml")
        .contains("mlua"));
}

#[test]
fn plugin_invocation_context_is_serializable_and_timeout_attributed() {
    let handler = handler(
        plugin_key("project-pipelines"),
        PluginHandlerKind::McpTool,
        "tickets.next",
    );
    let request = PluginInvocationRequest {
        request_id: request_id(),
        handler: handler.clone(),
        timeout_ms: 2_500,
        context: PluginInvocationContext {
            client_id: Some(client_id()),
            session_id: Some(session_id()),
            subscription_id: Some(subscription_id()),
            surface_id: Some("pipelines".to_string()),
            origin: Some("mcp".to_string()),
            metadata: Some(BoundaryJson(serde_json::json!({ "run_id": "run-1" }))),
        },
        payload: BoundaryJson(serde_json::json!({ "ticket_id": "ticket-1" })),
    };
    let timeout = PluginWorkerEvent::InvocationTimedOut(PluginInvocationFailure {
        request_id: request_id(),
        handler: handler.clone(),
        kind: PluginInvocationFailureKind::TimedOut,
        timeout_ms: Some(2_500),
        reason: "handler exceeded timeout".to_string(),
    });

    let round_trip_request: PluginInvocationRequest = round_trip(&request);
    let round_trip_timeout: PluginWorkerEvent = round_trip(&timeout);

    assert_eq!(round_trip_request.request_id, request_id());
    assert_eq!(
        round_trip_request.handler.plugin_key,
        plugin_key("project-pipelines")
    );
    assert_eq!(round_trip_request.handler.handler_id, "tickets.next");
    assert_eq!(round_trip_request.handler.kind, PluginHandlerKind::McpTool);
    assert_eq!(round_trip_request.timeout_ms, 2_500);

    match round_trip_timeout {
        PluginWorkerEvent::InvocationTimedOut(failure) => {
            assert_eq!(failure.request_id, request_id());
            assert_eq!(failure.handler.plugin_key, plugin_key("project-pipelines"));
            assert_eq!(failure.handler.handler_id, handler.handler_id);
            assert_eq!(failure.kind, PluginInvocationFailureKind::TimedOut);
            assert_eq!(failure.timeout_ms, Some(2_500));
        }
        other => panic!("expected timeout event, got {other:?}"),
    }
}

#[test]
fn plugin_reload_replaces_only_one_plugins_descriptors_in_harness() {
    use std::collections::HashMap;

    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let old_a = descriptor(
        plugin_a.clone(),
        PluginDescriptorKind::Command,
        "advance",
        Some(handler(
            plugin_a.clone(),
            PluginHandlerKind::Command,
            "advance.old",
        )),
    );
    let new_a = descriptor(
        plugin_a.clone(),
        PluginDescriptorKind::Command,
        "advance",
        Some(handler(
            plugin_a.clone(),
            PluginHandlerKind::Command,
            "advance.new",
        )),
    );
    let untouched_b = descriptor(
        plugin_b.clone(),
        PluginDescriptorKind::SurfaceRoute,
        "home",
        Some(handler(
            plugin_b.clone(),
            PluginHandlerKind::SurfaceRoute,
            "home.render",
        )),
    );
    let mut descriptors = HashMap::from([
        (old_a.descriptor.clone(), old_a),
        (untouched_b.descriptor.clone(), untouched_b.clone()),
    ]);
    let reload = PluginReloadSpec {
        request_id: request_id(),
        plugin_key: plugin_a.clone(),
        load: PluginLoadSpec {
            plugin_key: plugin_a.clone(),
            package: "project-pipelines".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![new_a.clone()],
            metadata: None,
        },
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    };

    descriptors.retain(|descriptor, _| descriptor.plugin_key != reload.plugin_key);
    for descriptor in reload.load.descriptors {
        descriptors.insert(descriptor.descriptor.clone(), descriptor);
    }

    assert_eq!(descriptors.len(), 2);
    assert!(descriptors.values().any(|descriptor| descriptor == &new_a));
    assert!(descriptors
        .values()
        .any(|descriptor| descriptor == &untouched_b));
    assert!(descriptors.keys().all(|descriptor| {
        descriptor.plugin_key == plugin_a || descriptor.plugin_key == plugin_b
    }));
}

#[test]
fn plugin_unload_cleanup_is_scoped_to_owner_plugin() {
    let plugin_key = plugin_key("project-pipelines");
    let cleanup = PluginCleanupResult {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        removed_descriptors: vec![PluginDescriptorRef {
            plugin_key: plugin_key.clone(),
            kind: PluginDescriptorKind::Watch,
            descriptor_id: "repo-watch".to_string(),
        }],
        removed_resources: vec![PluginResourceRef {
            plugin_key: plugin_key.clone(),
            kind: PluginResourceKind::Watch,
            resource_id: "repo-watch-1".to_string(),
        }],
    };
    let unload = PluginUnloadSpec {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    };
    let event = PluginWorkerEvent::Unloaded {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        cleanup,
    };

    assert_eq!(unload, round_trip(&unload));

    match round_trip(&event) {
        PluginWorkerEvent::Unloaded { cleanup, .. } => {
            assert!(cleanup
                .removed_descriptors
                .iter()
                .all(|descriptor| descriptor.plugin_key == plugin_key));
            assert!(cleanup
                .removed_resources
                .iter()
                .all(|resource| resource.plugin_key == plugin_key));
        }
        other => panic!("expected unloaded event, got {other:?}"),
    }
}

#[test]
fn plugin_backpressure_is_scoped_to_one_plugin_identity() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let pressure_a = PluginWorkerEvent::Backpressure(BackpressureSummary {
        source: QueueSource::PluginWorker,
        capacity: 256,
        depth: 256,
        route: BackpressureRoute {
            session_id: None,
            client_id: None,
            subscription_id: None,
            plugin_key: Some(plugin_a.clone()),
        },
    });
    let pressure_b = PluginWorkerEvent::Backpressure(BackpressureSummary {
        source: QueueSource::PluginWorker,
        capacity: 256,
        depth: 10,
        route: BackpressureRoute {
            session_id: None,
            client_id: None,
            subscription_id: None,
            plugin_key: Some(plugin_b.clone()),
        },
    });

    let affected = match round_trip(&pressure_a) {
        PluginWorkerEvent::Backpressure(summary) => summary.route.plugin_key,
        other => panic!("expected backpressure event, got {other:?}"),
    };

    assert_eq!(affected, Some(plugin_a));
    assert_eq!(pressure_b, round_trip(&pressure_b));
    assert_ne!(pressure_a, pressure_b);
}

#[test]
fn plugin_worker_messages_cover_load_invoke_reload_unload_shutdown() {
    let plugin_key = plugin_key("project-pipelines");
    let command = handler(plugin_key.clone(), PluginHandlerKind::Command, "advance");
    let command_descriptor = descriptor(
        plugin_key.clone(),
        PluginDescriptorKind::Command,
        "advance",
        Some(command.clone()),
    );
    let load = PluginWorkerMessage::Load {
        request_id: request_id(),
        spec: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: "project-pipelines".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![command_descriptor.clone()],
            metadata: Some(BoundaryJson(serde_json::json!({ "version": 1 }))),
        },
    };
    let invoke = PluginWorkerMessage::Invoke(PluginInvocationRequest {
        request_id: request_id(),
        handler: command.clone(),
        timeout_ms: 1_500,
        context: PluginInvocationContext {
            client_id: Some(client_id()),
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "ticket_id": "ticket-1" })),
    });
    let cleanup = PluginCleanupResult {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        removed_descriptors: vec![command_descriptor.descriptor.clone()],
        removed_resources: vec![PluginResourceRef {
            plugin_key: plugin_key.clone(),
            kind: PluginResourceKind::McpRegistration,
            resource_id: "advance-tool".to_string(),
        }],
    };
    let reload = PluginWorkerMessage::Reload(PluginReloadSpec {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: "project-pipelines".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![command_descriptor.clone()],
            metadata: None,
        },
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    let unload = PluginWorkerMessage::Unload(PluginUnloadSpec {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    let shutdown = PluginWorkerMessage::Shutdown {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
    };
    let loaded = PluginWorkerEvent::Loaded {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        handlers: vec![command.clone()],
        descriptors: vec![command_descriptor.descriptor.clone()],
    };
    let completed = PluginWorkerEvent::InvocationCompleted(PluginInvocationSuccess {
        request_id: request_id(),
        handler: command,
        payload: None,
    });
    let cleanup_completed = PluginWorkerEvent::CleanupCompleted(cleanup.clone());
    let reloaded = PluginWorkerEvent::Reloaded {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        cleanup: cleanup.clone(),
        descriptors: vec![command_descriptor.descriptor],
    };
    let unloaded = PluginWorkerEvent::Unloaded {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        cleanup,
    };
    let pressure = PluginWorkerEvent::Backpressure(BackpressureSummary {
        source: QueueSource::PluginWorker,
        capacity: 256,
        depth: 200,
        route: BackpressureRoute {
            session_id: None,
            client_id: None,
            subscription_id: None,
            plugin_key: Some(plugin_key),
        },
    });

    for message in [load, invoke, reload, unload, shutdown] {
        assert_eq!(message, round_trip(&message));
    }
    let rendered = format!("{completed:?}");
    for event in [
        loaded,
        reloaded,
        unloaded,
        completed,
        cleanup_completed,
        pressure,
    ] {
        assert_eq!(event, round_trip(&event));
    }

    assert!(rendered.contains("PluginHandlerRef"));
    assert!(!rendered.contains("mlua"));
    assert!(!rendered.contains("Function"));
}

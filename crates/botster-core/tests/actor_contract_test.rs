//! Actor contract acceptance tests.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    PluginCleanupResult, PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef,
    PluginHandlerKind, PluginHandlerRef, PluginInvocationContext, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationSuccess, PluginKey,
    PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef,
    PluginUnloadSpec, PluginWorkerEvent, PluginWorkerMessage, QueueSource, SessionIoEvent,
    SessionIoRequest, SessionLifecycleState, TerminalAttachState, TransportConnectionMode,
    TransportSignal, PUBLIC_QUEUE_SOURCES,
};
use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::{ModeFlags, ProcessExitedPayload};

struct BoundaryJsonEscapeHatch {
    path: &'static str,
    owner: &'static str,
    reason: &'static str,
    file: &'static str,
    source_marker: &'static str,
}

// Keep this owner/reason inventory in sync with the recursive source scan in
// boundary_test.rs so new BoundaryJson fields are both classified and detected.
const BOUNDARY_JSON_ESCAPE_HATCHES: [BoundaryJsonEscapeHatch; 10] = [
    BoundaryJsonEscapeHatch {
        path: "TransportSignal.payload",
        owner: "relay",
        reason: "encrypted or relay-owned signaling envelope is opaque to core",
        file: "src/contract/actor.rs",
        source_marker: "pub struct TransportSignal",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginOwnedDescriptor.body",
        owner: "plugin",
        reason: "plugin descriptor schema is owned by the plugin",
        file: "src/contract/actor.rs",
        source_marker: "pub struct PluginOwnedDescriptor",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginLoadSpec.metadata",
        owner: "plugin",
        reason: "plugin load metadata schema is owned by the plugin",
        file: "src/contract/actor.rs",
        source_marker: "pub struct PluginLoadSpec",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginInvocationContext.metadata",
        owner: "plugin",
        reason: "plugin invocation context metadata schema is owned by the plugin",
        file: "src/contract/actor.rs",
        source_marker: "pub struct PluginInvocationContext",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginInvocationRequest.payload",
        owner: "plugin",
        reason: "plugin handler input schema is owned by the plugin",
        file: "src/contract/actor.rs",
        source_marker: "pub struct PluginInvocationRequest",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginInvocationSuccess.payload",
        owner: "plugin",
        reason: "plugin handler response schema is owned by the plugin",
        file: "src/contract/actor.rs",
        source_marker: "pub struct PluginInvocationSuccess",
    },
    BoundaryJsonEscapeHatch {
        path: "TransportIngress::BoundaryPayload.payload",
        owner: "relay/plugin",
        reason: "relay/plugin adapter ingress payload schema is opaque to core",
        file: "src/contract/transport.rs",
        source_marker: "pub enum TransportIngress",
    },
    BoundaryJsonEscapeHatch {
        path: "TransportEgress::BoundaryPayload.payload",
        owner: "relay/plugin",
        reason: "relay/plugin adapter egress payload schema is opaque to core",
        file: "src/contract/transport.rs",
        source_marker: "pub enum TransportEgress",
    },
    BoundaryJsonEscapeHatch {
        path: "NotificationAction.extension",
        owner: "plugin",
        reason: "plugin notification action extension schema is opaque to core",
        file: "src/contract/notification.rs",
        source_marker: "pub struct NotificationAction",
    },
    BoundaryJsonEscapeHatch {
        path: "NotificationContent.extension",
        owner: "plugin",
        reason: "plugin notification content extension schema is opaque to core",
        file: "src/contract/notification.rs",
        source_marker: "pub struct NotificationContent",
    },
];

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
        subscription_id: subscription_id(),
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
fn boundary_json_escape_hatches_are_classified_with_owner_and_reason() {
    assert_eq!(BOUNDARY_JSON_ESCAPE_HATCHES.len(), 10);

    for hatch in BOUNDARY_JSON_ESCAPE_HATCHES {
        assert!(!hatch.path.is_empty(), "{:?}", hatch.path);
        assert!(
            ["relay", "plugin", "relay/plugin"].contains(&hatch.owner),
            "{} has unexpected owner {}",
            hatch.path,
            hatch.owner
        );
        assert!(
            hatch.reason.len() >= 30,
            "{} needs a concrete ownership reason, got {:?}",
            hatch.path,
            hatch.reason
        );
        for owner_term in hatch.owner.split('/') {
            assert!(
                hatch.reason.contains(owner_term),
                "{} reason {:?} must explain owner term {:?}",
                hatch.path,
                hatch.reason,
                owner_term
            );
        }

        let source = std::fs::read_to_string(hatch.file).expect("read contract source");
        assert!(
            source.contains(hatch.source_marker),
            "{} source marker missing: {}",
            hatch.path,
            hatch.source_marker
        );
    }

    let paths: Vec<_> = BOUNDARY_JSON_ESCAPE_HATCHES
        .iter()
        .map(|hatch| hatch.path)
        .collect();
    assert_eq!(
        paths,
        vec![
            "TransportSignal.payload",
            "PluginOwnedDescriptor.body",
            "PluginLoadSpec.metadata",
            "PluginInvocationContext.metadata",
            "PluginInvocationRequest.payload",
            "PluginInvocationSuccess.payload",
            "TransportIngress::BoundaryPayload.payload",
            "TransportEgress::BoundaryPayload.payload",
            "NotificationAction.extension",
            "NotificationContent.extension",
        ]
    );
}

#[test]
fn stable_botster_controls_do_not_use_boundary_json() {
    let stable_controls = [
        format!(
            "{:?}",
            HubControlMessage::AttachClient {
                origin: HubControlOrigin::Client(client_id()),
                request_id: request_id(),
                client_id: client_id(),
                session_id: session_id(),
                subscription_id: subscription_id(),
            }
        ),
        format!(
            "{:?}",
            HubControlMessage::RequestSnapshot {
                request_id: request_id(),
                client_id: client_id(),
                session_id: session_id(),
            }
        ),
        format!(
            "{:?}",
            HubControlMessage::SessionLifecycle {
                session_id: session_id(),
                state: SessionLifecycleState::Exited { code: Some(0) },
            }
        ),
        format!(
            "{:?}",
            HubControlMessage::Backpressure(BackpressureSummary {
                source: QueueSource::ClientWorker,
                capacity: 512,
                depth: 10,
                route: BackpressureRoute::queue_only(),
            })
        ),
        format!(
            "{:?}",
            ClientWorkerMessage::Control {
                frame: ClientControlFrame::AttachState {
                    state: TerminalAttachState::Attached,
                },
            }
        ),
        format!(
            "{:?}",
            ClientWorkerMessage::Control {
                frame: ClientControlFrame::Health {
                    health: ClientConnectionHealth::Healthy,
                },
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::SubscribeTerminal {
                request_id: request_id(),
                session_id: session_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                rows: 40,
                cols: 120,
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::PtyInput {
                session_id: session_id(),
                data: b"hi".to_vec()
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::GetSnapshot {
                request_id: request_id(),
                session_id: session_id(),
            }
        ),
        format!(
            "{:?}",
            SessionIoEvent::TerminalBytes {
                session_id: session_id(),
                data: b"ok".to_vec(),
            }
        ),
        format!(
            "{:?}",
            SessionIoEvent::ProcessExited {
                session_id: session_id(),
                payload: ProcessExitedPayload {
                    exit_code: Some(0),
                    signal: None,
                },
            }
        ),
        format!(
            "{:?}",
            TransportIngress::SubscribeSession {
                client_id: client_id(),
                session_id: session_id(),
                subscription_id: subscription_id(),
            }
        ),
        format!(
            "{:?}",
            TransportIngress::TerminalInput {
                session_id: session_id(),
                data: b"hi".to_vec(),
            }
        ),
        format!(
            "{:?}",
            TransportIngress::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120,
            }
        ),
        format!(
            "{:?}",
            TransportIngress::Focus {
                session_id: session_id(),
                focused: true,
            }
        ),
        format!(
            "{:?}",
            TransportIngress::Ping {
                request_id: request_id()
            }
        ),
        format!(
            "{:?}",
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id(),
                data: b"ok".to_vec(),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::Scrollback {
                session_id: session_id(),
                subscription_id: subscription_id(),
                data: b"page".to_vec(),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::ProcessExit {
                session_id: session_id(),
                subscription_id: subscription_id(),
                code: Some(0),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::AttachState {
                session_id: session_id(),
                subscription_id: subscription_id(),
                state: TerminalAttachState::Attached,
            }
        ),
        format!(
            "{:?}",
            TransportEgress::Pong {
                request_id: request_id()
            }
        ),
        format!(
            "{:?}",
            ModeFlags {
                kitty_enabled: true,
                cursor_visible: true,
                bracketed_paste: true,
                mouse_mode: 1,
                alt_screen: false,
                focus_reporting: true,
                application_cursor: false,
            }
        ),
    ];

    for control in stable_controls {
        assert!(
            !control.contains("BoundaryJson"),
            "stable Botster control used BoundaryJson: {control}"
        );
    }

    let mut boundary_fields = Vec::new();
    for file in [
        "src/contract/actor.rs",
        "src/contract/transport.rs",
        "src/contract/notification.rs",
        "src/lib.rs",
        "src/contract/boundary.rs",
    ] {
        let source = std::fs::read_to_string(file).expect("read source");
        for line in source.lines() {
            let trimmed = line.trim_start();
            if !trimmed.contains("BoundaryJson")
                || trimmed.starts_with("use ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("//")
                || trimmed == "pub struct BoundaryJson(pub serde_json::Value);"
            {
                continue;
            }
            boundary_fields.push(format!("{file}:{}", trimmed.trim()));
        }
    }

    assert_eq!(
        boundary_fields.len(),
        BOUNDARY_JSON_ESCAPE_HATCHES.len(),
        "new public contract BoundaryJson uses must be classified with owner and reason: {boundary_fields:?}"
    );
    assert!(boundary_fields.iter().all(|field| {
        field.starts_with("src/contract/actor.rs:")
            || field.starts_with("src/contract/transport.rs:")
            || field.starts_with("src/contract/notification.rs:")
    }));

    let actor_source = std::fs::read_to_string("src/contract/actor.rs").expect("read actor source");
    let transport_source =
        std::fs::read_to_string("src/contract/transport.rs").expect("read transport source");
    let notification_source =
        std::fs::read_to_string("src/contract/notification.rs").expect("read notification source");
    assert!(!actor_source.contains("serde_json::Value"));
    assert!(!transport_source.contains("serde_json::Value"));
    assert!(!notification_source.contains("serde_json::Value"));
}

#[test]
fn session_and_client_contracts_do_not_depend_on_transport() {
    let session_source =
        std::fs::read_to_string("src/contract/session.rs").expect("read session source");
    let client_source =
        std::fs::read_to_string("src/contract/client.rs").expect("read client source");
    let actor_source = std::fs::read_to_string("src/contract/actor.rs").expect("read actor source");

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
fn terminal_mode_is_not_a_pushed_actor_contract() {
    let actor_source = std::fs::read_to_string("src/contract/actor.rs").expect("read actor source");
    let transport_source =
        std::fs::read_to_string("src/contract/transport.rs").expect("read transport source");

    assert!(
        !actor_source.contains("ModeChanged"),
        "actor contracts must not push terminal mode/color deltas via ModeChanged"
    );
    assert!(
        !transport_source.contains("ModeChanged"),
        "transport contracts must not push terminal mode/color deltas via ModeChanged"
    );
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

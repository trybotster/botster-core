//! Actor contract acceptance tests.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    PluginHandlerKind, PluginHandlerRef, PluginKey, PluginLoadSpec, PluginWorkerEvent,
    PluginWorkerMessage, QueueSource, SessionIoEvent, SessionIoRequest, SessionLifecycleState,
    TerminalAttachState, TransportConnectionMode, TransportSignal, PUBLIC_QUEUE_SOURCES,
};
use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::ModeFlags;

struct BoundaryJsonEscapeHatch {
    path: &'static str,
    owner: &'static str,
    reason: &'static str,
    file: &'static str,
    source_marker: &'static str,
}

const BOUNDARY_JSON_ESCAPE_HATCHES: [BoundaryJsonEscapeHatch; 5] = [
    BoundaryJsonEscapeHatch {
        path: "TransportSignal.payload",
        owner: "relay",
        reason: "encrypted or relay-owned signaling envelope is opaque to core",
        file: "src/actor.rs",
        source_marker: "pub struct TransportSignal",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginWorkerMessage::Invoke.payload",
        owner: "plugin",
        reason: "plugin handler input schema is owned by the plugin",
        file: "src/actor.rs",
        source_marker: "Invoke {",
    },
    BoundaryJsonEscapeHatch {
        path: "PluginWorkerEvent::Completed.payload",
        owner: "plugin",
        reason: "plugin handler response schema is owned by the plugin",
        file: "src/actor.rs",
        source_marker: "Completed {",
    },
    BoundaryJsonEscapeHatch {
        path: "TransportIngress::BoundaryPayload.payload",
        owner: "relay/plugin",
        reason: "relay/plugin adapter ingress payload schema is opaque to core",
        file: "src/transport.rs",
        source_marker: "pub enum TransportIngress",
    },
    BoundaryJsonEscapeHatch {
        path: "TransportEgress::BoundaryPayload.payload",
        owner: "relay/plugin",
        reason: "relay/plugin adapter egress payload schema is opaque to core",
        file: "src/transport.rs",
        source_marker: "pub enum TransportEgress",
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
    let plugin = PluginWorkerMessage::Invoke {
        request_id: request_id(),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("project-pipelines".to_string()),
            kind: PluginHandlerKind::UiAction,
            handler_id: "open".to_string(),
        },
        payload: payload.clone(),
    };
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
    assert_eq!(BOUNDARY_JSON_ESCAPE_HATCHES.len(), 5);

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
            "PluginWorkerMessage::Invoke.payload",
            "PluginWorkerEvent::Completed.payload",
            "TransportIngress::BoundaryPayload.payload",
            "TransportEgress::BoundaryPayload.payload",
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
            SessionIoRequest::Subscribe {
                request_id: request_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                rows: 40,
                cols: 120,
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::Input {
                data: b"hi".to_vec()
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::Resize {
                rows: 40,
                cols: 120
            }
        ),
        format!(
            "{:?}",
            SessionIoRequest::Snapshot {
                request_id: request_id()
            }
        ),
        format!("{:?}", SessionIoRequest::Focus { focused: true }),
        format!(
            "{:?}",
            SessionIoEvent::TerminalBytes {
                session_id: session_id(),
                data: b"ok".to_vec(),
            }
        ),
        format!(
            "{:?}",
            SessionIoEvent::FocusChanged {
                session_id: session_id(),
                focused: true,
            }
        ),
        format!(
            "{:?}",
            SessionIoEvent::ProcessExited {
                session_id: session_id(),
                code: Some(0),
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
                data: b"ok".to_vec(),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::Scrollback {
                session_id: session_id(),
                data: b"page".to_vec(),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::ProcessExit {
                session_id: session_id(),
                code: Some(0),
            }
        ),
        format!(
            "{:?}",
            TransportEgress::AttachState {
                session_id: session_id(),
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
        "src/actor.rs",
        "src/transport.rs",
        "src/lib.rs",
        "src/boundary.rs",
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
    assert!(boundary_fields
        .iter()
        .all(|field| field.starts_with("src/actor.rs:") || field.starts_with("src/transport.rs:")));

    let actor_source = std::fs::read_to_string("src/actor.rs").expect("read actor source");
    let transport_source =
        std::fs::read_to_string("src/transport.rs").expect("read transport source");
    assert!(!actor_source.contains("serde_json::Value"));
    assert!(!transport_source.contains("serde_json::Value"));
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
        "ActionCable",
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
fn plugin_worker_messages_use_handler_refs() {
    let plugin_key = PluginKey("project-pipelines".to_string());
    let handler = PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "advance".to_string(),
    };
    let load = PluginWorkerMessage::Load {
        request_id: request_id(),
        spec: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: "project-pipelines".to_string(),
            entrypoint: "plugin.lua".to_string(),
        },
    };
    let invoke = PluginWorkerMessage::Invoke {
        request_id: request_id(),
        handler: handler.clone(),
        payload: BoundaryJson(serde_json::json!({ "ticket_id": "ticket-1" })),
    };
    let shutdown = PluginWorkerMessage::Shutdown {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
    };
    let loaded = PluginWorkerEvent::Loaded {
        request_id: request_id(),
        plugin_key: plugin_key.clone(),
        handlers: vec![handler.clone()],
    };
    let completed = PluginWorkerEvent::Completed {
        request_id: request_id(),
        handler,
        payload: None,
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

    let rendered =
        format!("{load:?} {invoke:?} {shutdown:?} {loaded:?} {completed:?} {pressure:?}");
    assert!(rendered.contains("PluginHandlerRef"));
    assert!(!rendered.contains("mlua"));
    assert!(!rendered.contains("Function"));
    assert!(!std::fs::read_to_string("Cargo.toml")
        .expect("read Cargo.toml")
        .contains("mlua"));
}

//! Actor contract acceptance tests.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    PluginHandlerKind, PluginHandlerRef, PluginKey, PluginLoadSpec, PluginWorkerEvent,
    PluginWorkerMessage, QueueSource, SessionIoRequest, TransportConnectionMode, TransportSignal,
    PUBLIC_QUEUE_SOURCES,
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

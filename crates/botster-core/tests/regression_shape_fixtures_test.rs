//! Contract tests for reusable regression-shape fixtures.

use botster_core::actor::{
    BackpressureRoute, ClientConnectionHealth, ClientControlFrame, HubControlMessage, PluginKey,
    PluginWorkerEvent, QueueSource, TerminalAttachState,
};
use botster_core::client::ClientId;
use botster_core::entity::{EntityFrame, EntityKind};
use botster_core::session::{SessionId, SubscriptionId};
use botster_core::session_protocol::{encode_frame, FrameDecoder, FRAME_PTY_OUTPUT};
use botster_core::transport::TransportEgress;
use botster_core_test_support::fixtures::regression::regression_shapes;

fn session_id() -> SessionId {
    SessionId("session-regression".to_string())
}

fn client_id() -> ClientId {
    ClientId("client-regression".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-regression".to_string())
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize contract value");
    serde_json::from_str(&json).expect("deserialize contract value")
}

#[test]
fn regression_shape_noisy_pty_replay_is_ordered_opaque_output() {
    let chunks = regression_shapes::noisy_pty_replay(&[
        b"\x1b[?2004hprompt> ",
        b"\x00raw\xffbytes",
        b"\r\nbuild finished\r\n",
    ]);

    let mut encoded = Vec::new();
    for chunk in &chunks {
        encoded.extend_from_slice(
            &encode_frame(FRAME_PTY_OUTPUT, chunk).expect("encode output frame"),
        );
    }
    let mut decoder = FrameDecoder::new();
    let frames = decoder.feed(&encoded).expect("decode output frames");

    assert_eq!(frames.len(), chunks.len());
    assert_eq!(frames[0].payload, chunks[0]);
    assert_eq!(frames[1].payload, chunks[1]);
    assert_eq!(frames[2].payload, chunks[2]);

    let egress: Vec<TransportEgress> = chunks
        .iter()
        .map(|chunk| TransportEgress::TerminalOutput {
            session_id: session_id(),
            subscription_id: subscription_id(),
            data: chunk.clone(),
        })
        .collect();
    assert_eq!(egress, round_trip(&egress));
}

#[test]
fn regression_shape_stale_reconnect_uses_existing_subscription_identity() {
    let stale = SubscriptionId("sub-stale".to_string());
    let current = SubscriptionId("sub-current".to_string());
    let messages = regression_shapes::stale_reconnect_cycle(
        client_id(),
        session_id(),
        stale.clone(),
        current.clone(),
    );
    let health = regression_shapes::reconnecting_health();

    assert_eq!(messages, round_trip(&messages));
    assert_ne!(messages[0], messages[1]);
    assert!(matches!(
        &messages[0],
        HubControlMessage::AttachClient { subscription_id, .. } if subscription_id == &stale
    ));
    assert!(matches!(
        &messages[1],
        HubControlMessage::AttachClient { subscription_id, .. } if subscription_id == &current
    ));
    assert!(matches!(
        health,
        ClientControlFrame::Health {
            health: ClientConnectionHealth::Reconnecting
        }
    ));
}

#[test]
fn regression_shape_bounded_queue_saturation_preserves_typed_pressure_context() {
    let route = BackpressureRoute {
        session_id: Some(session_id()),
        client_id: Some(client_id()),
        subscription_id: Some(SubscriptionId("sub-pressure".to_string())),
        plugin_key: None,
    };
    let summary =
        regression_shapes::bounded_queue_saturation(QueueSource::ClientWorker, 500, route.clone());

    assert_eq!(summary, round_trip(&summary));
    assert_eq!(summary.source, QueueSource::ClientWorker);
    assert_eq!(
        summary.capacity,
        QueueSource::ClientWorker.default_capacity()
    );
    assert_eq!(summary.depth, 500);
    assert_eq!(summary.route, route);
}

#[test]
fn regression_shape_unknown_peer_burst_is_transport_adapter_pressure_not_policy() {
    let summary = regression_shapes::unknown_peer_burst_pressure(&["peer-a", "peer-b", "peer-c"]);

    assert_eq!(summary, round_trip(&summary));
    assert_eq!(summary.source, QueueSource::TransportAdapter);
    assert_eq!(summary.depth, 3);
    assert_eq!(summary.route, BackpressureRoute::queue_only());
}

#[test]
fn regression_shape_snapshot_precedes_live_output_without_mode_event_variants() {
    let egress = regression_shapes::snapshot_before_live_output(session_id(), b"snapshot", b"live");

    assert_eq!(egress, round_trip(&egress));
    assert!(matches!(
        &egress[0],
        TransportEgress::AttachState {
            state: TerminalAttachState::Attaching,
            ..
        }
    ));
    assert!(matches!(&egress[1], TransportEgress::Snapshot { data, .. } if data == b"snapshot"));
    assert!(matches!(
        &egress[2],
        TransportEgress::AttachState {
            state: TerminalAttachState::Attached,
            ..
        }
    ));
    assert!(matches!(&egress[3], TransportEgress::TerminalOutput { data, .. } if data == b"live"));

    let rendered = format!("{egress:?}");
    assert!(!rendered.contains("ModeChanged"));
    assert!(!rendered.contains("terminal-mode-delta"));
}

#[test]
fn regression_shape_entity_scoped_hydration_uses_existing_entity_frames() {
    let frames = regression_shapes::entity_scoped_hydration("project-pipelines", "project-1");

    assert_eq!(frames, round_trip(&frames));
    assert!(matches!(
        &frames[0],
        EntityFrame::Snapshot {
            entity_type,
            items,
            ..
        } if entity_type == &EntityKind("project-pipelines.ticket".to_string())
            && items[0]["scope_id"] == "project-1"
    ));
    assert!(matches!(&frames[1], EntityFrame::Upsert { .. }));
    assert!(matches!(&frames[2], EntityFrame::Patch { .. }));
    assert!(matches!(&frames[3], EntityFrame::Remove { .. }));
}

#[test]
fn regression_shape_plugin_worker_timeout_backpressure_preserves_handler_refs() {
    let events = regression_shapes::plugin_worker_timeout_backpressure(
        PluginKey("project-pipelines".to_string()),
        "advance",
    );

    assert_eq!(events, round_trip(&events));
    assert!(matches!(
        &events[0],
        PluginWorkerEvent::Backpressure(summary)
            if summary.source == QueueSource::PluginWorker
                && summary.depth == QueueSource::PluginWorker.default_capacity()
    ));
    assert!(matches!(
        &events[1],
        PluginWorkerEvent::Failed { reason, .. } if reason.contains("timed out")
    ));
    assert!(matches!(
        &events[2],
        PluginWorkerEvent::InvocationTimedOut(failure)
            if failure.handler.plugin_key == PluginKey("project-pipelines".to_string())
    ));
    assert!(matches!(
        &events[3],
        PluginWorkerEvent::InvocationCompleted(success)
            if success.handler.plugin_key == PluginKey("project-pipelines".to_string())
    ));
}

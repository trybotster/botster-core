//! Routed envelope primitive acceptance tests.

use botster_core::{
    BoundaryJson, EndpointId, EnvelopeCursor, EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget,
    MultiplexerEngine, RoutedEnvelope, RoutedEnvelopeObservation, RoutedEnvelopePayload,
    RoutedEnvelopeQueueConfig, RoutedEnvelopeRouter,
};
use botster_core_test_support::fake::{FakeSessionRuntime, FakeSessionWorkerRuntime};

fn endpoint(id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Endpoint {
        endpoint_id: EndpointId(id.to_string()),
    }
}

fn client(id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Client {
        client_id: botster_core::ClientId(id.to_string()),
    }
}

fn session(id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Session {
        session_id: botster_core::SessionId(id.to_string()),
    }
}

fn subscription(session_id: &str, subscription_id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Subscription {
        session_id: botster_core::SessionId(session_id.to_string()),
        subscription_id: botster_core::SubscriptionId(subscription_id.to_string()),
    }
}

fn topic(name: &str) -> EnvelopeTarget {
    EnvelopeTarget::Topic {
        topic: name.to_string(),
    }
}

fn stream(name: &str) -> EnvelopeTarget {
    EnvelopeTarget::Stream {
        stream: name.to_string(),
    }
}

fn envelope(id: &str, targets: Vec<EnvelopeTarget>) -> RoutedEnvelope {
    RoutedEnvelope::new(
        EnvelopeId(id.to_string()),
        EndpointId("source-endpoint".to_string()),
        targets,
        RoutedEnvelopePayload {
            content_type: "application/octet-stream".to_string(),
            body: format!("payload:{id}").into_bytes(),
            extension: None,
        },
        10,
    )
}

#[test]
fn routes_envelope_to_explicit_client_endpoint_through_multiplexer_engine() {
    let mut engine: MultiplexerEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        MultiplexerEngine::new(FakeSessionRuntime::new());

    let publish = engine.publish_envelope(envelope("env-1", vec![client("client-a")]));
    let drain = engine.drain_envelopes(&client("client-a"), None, 10);

    assert_eq!(publish.deliveries[0].target, client("client-a"));
    assert_eq!(publish.deliveries[0].status, EnvelopeDeliveryStatus::Queued);
    assert_eq!(drain.envelopes[0].id, EnvelopeId("env-1".to_string()));
    assert_eq!(drain.envelopes[0].targets, vec![client("client-a")]);
    assert_eq!(drain.envelopes[0].cursor, Some(EnvelopeCursor(1)));
    assert_eq!(drain.envelopes[0].payload.body, b"payload:env-1".to_vec());
}

#[test]
fn routes_envelope_to_session_and_subscription_targets() {
    let mut router = RoutedEnvelopeRouter::new();

    router.publish(envelope(
        "env-1",
        vec![session("session-a"), subscription("session-a", "sub-a")],
    ));

    assert_eq!(
        router
            .drain(&session("session-a"), None, 10)
            .envelopes
            .len(),
        1
    );
    assert_eq!(
        router
            .drain(&subscription("session-a", "sub-a"), None, 10)
            .envelopes
            .len(),
        1
    );
    assert!(router
        .drain(&subscription("session-a", "sub-b"), None, 10)
        .envelopes
        .is_empty());
}

#[test]
fn topic_and_stream_subscriptions_fan_out_to_current_subscribers() {
    let mut router = RoutedEnvelopeRouter::new();

    router.subscribe(topic("coordination"), endpoint("agent-a"));
    router.subscribe(topic("coordination"), endpoint("agent-b"));
    router.subscribe(stream("status"), endpoint("watcher-a"));
    router.publish(envelope("topic-1", vec![topic("coordination")]));
    router.publish(envelope("stream-1", vec![stream("status")]));
    router.unsubscribe(&topic("coordination"), &endpoint("agent-b"));
    router.publish(envelope("topic-2", vec![topic("coordination")]));

    assert_eq!(
        router
            .drain(&endpoint("agent-a"), None, 10)
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["topic-1", "topic-2"]
    );
    assert_eq!(
        router
            .drain(&endpoint("agent-b"), None, 10)
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["topic-1"]
    );
    assert_eq!(
        router.drain(&endpoint("watcher-a"), None, 10).envelopes[0].id,
        EnvelopeId("stream-1".to_string())
    );
}

#[test]
fn delivery_ack_state_is_per_target() {
    let mut router = RoutedEnvelopeRouter::new();
    router.publish(envelope(
        "env-1",
        vec![endpoint("target-a"), endpoint("target-b")],
    ));
    router.drain(&endpoint("target-a"), None, 10);
    router.drain(&endpoint("target-b"), None, 10);

    let acknowledged = router
        .acknowledge(&endpoint("target-a"), &EnvelopeId("env-1".to_string()))
        .expect("acknowledge target-a");

    assert_eq!(acknowledged.status, EnvelopeDeliveryStatus::Acknowledged);
    assert_eq!(
        router
            .delivery_state(&endpoint("target-b"), &EnvelopeId("env-1".to_string()))
            .expect("target-b delivery")
            .status,
        EnvelopeDeliveryStatus::Delivered
    );
}

#[test]
fn cursoring_resumes_after_last_seen_envelope() {
    let mut router = RoutedEnvelopeRouter::new();
    router.publish(envelope("env-1", vec![endpoint("target-a")]));
    router.publish(envelope("env-2", vec![endpoint("target-a")]));
    router.publish(envelope("env-3", vec![endpoint("target-a")]));

    let first = router.drain(&endpoint("target-a"), None, 2);
    let second = router.drain(&endpoint("target-a"), first.next_cursor, 2);

    assert_eq!(
        first
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-1", "env-2"]
    );
    assert_eq!(
        second
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-3"]
    );
}

#[test]
fn bounded_queue_reports_pressure_without_blocking_other_targets() {
    let mut router = RoutedEnvelopeRouter::with_config(RoutedEnvelopeQueueConfig::new(1));
    router.publish(envelope("env-1", vec![endpoint("slow"), endpoint("fast")]));
    assert_eq!(router.drain(&endpoint("fast"), None, 10).envelopes.len(), 1);
    let pressured = router.publish(envelope("env-2", vec![endpoint("slow"), endpoint("fast")]));

    assert!(pressured.observations.iter().any(|observation| {
        matches!(
            observation,
            RoutedEnvelopeObservation::Backpressured {
                target,
                capacity: 1,
                depth: 1,
                ..
            } if target == &endpoint("slow")
        )
    }));
    assert_eq!(
        router
            .drain(&endpoint("fast"), None, 10)
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-2"]
    );
}

#[test]
fn slow_consumer_isolation_preserves_fast_consumer_delivery() {
    let mut router = RoutedEnvelopeRouter::with_config(RoutedEnvelopeQueueConfig::new(2));
    router.publish(envelope("env-1", vec![endpoint("slow"), endpoint("fast")]));
    assert_eq!(router.drain(&endpoint("fast"), None, 10).envelopes.len(), 1);

    router.publish(envelope("env-2", vec![endpoint("slow"), endpoint("fast")]));
    router.publish(envelope("env-3", vec![endpoint("slow"), endpoint("fast")]));

    assert_eq!(
        router
            .drain(&endpoint("fast"), None, 10)
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-2", "env-3"]
    );
    assert_eq!(router.drain(&endpoint("slow"), None, 10).envelopes.len(), 2);
}

#[test]
fn boundary_json_is_limited_to_extension_payloads() {
    let routed = RoutedEnvelope::new(
        EnvelopeId("env-1".to_string()),
        EndpointId("source".to_string()),
        vec![EnvelopeTarget::Plugin {
            plugin_key: botster_core::PluginKey("plugin-a".to_string()),
        }],
        RoutedEnvelopePayload {
            content_type: "application/json".to_string(),
            body: b"{}".to_vec(),
            extension: Some(BoundaryJson(serde_json::json!({ "owner": "plugin-a" }))),
        },
        10,
    );

    assert!(routed.payload.extension.is_some());
    assert!(!format!("{:?}", routed.id).contains("BoundaryJson"));
    assert!(!format!("{:?}", routed.source).contains("BoundaryJson"));
    assert!(!format!("{:?}", routed.targets).contains("BoundaryJson"));
}

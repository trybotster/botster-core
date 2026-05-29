//! Client stream contract acceptance tests.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, ClientControlFrame, QueueSource, SendFileErrorReason,
    SendFileFailed, SendFileRequest, SessionIoEvent, SessionIoRequest, SnapshotReady,
};
use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::client::ClientState;
use botster_core::client_stream::{
    ClientStreamGeneration, ClientStreamHarness, ClientStreamObservation,
};
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::ProcessExitedPayload;

fn client_id() -> ClientId {
    ClientId("client-1".to_string())
}

fn session_id() -> SessionId {
    SessionId("session-1".to_string())
}

fn request_id() -> RequestId {
    RequestId("req-1".to_string())
}

fn subscription_id(id: &str) -> SubscriptionId {
    SubscriptionId(id.to_string())
}

fn subscribe(harness: &mut ClientStreamHarness, subscription_id: SubscriptionId) {
    harness.handle_ingress(TransportIngress::SubscribeSession {
        client_id: client_id(),
        session_id: session_id(),
        subscription_id,
    });
}

#[test]
fn subscribed_clients_receive_terminal_bytes_and_process_exits() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let output = harness.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"ok".to_vec(),
    });
    let exit = harness.handle_session_event(SessionIoEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });

    assert_eq!(
        output.egress,
        vec![TransportEgress::TerminalOutput {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
            data: b"ok".to_vec(),
        }]
    );
    assert_eq!(
        exit.egress,
        vec![TransportEgress::ProcessExit {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
            code: Some(0),
        }]
    );
}

#[test]
fn unsubscribed_input_send_file_resize_and_snapshot_are_dropped_with_observations() {
    let mut harness = ClientStreamHarness::new(client_id());

    let input = harness.handle_ingress(TransportIngress::TerminalInput {
        session_id: session_id(),
        data: b"ls\n".to_vec(),
    });
    let send_file = harness.handle_ingress(TransportIngress::SendFile {
        request_id: request_id(),
        session_id: session_id(),
        data: b"send-file".to_vec(),
    });
    let resize = harness.handle_ingress(TransportIngress::Resize {
        session_id: session_id(),
        rows: 40,
        cols: 120,
    });
    let snapshot = harness.handle_ingress(TransportIngress::RequestSnapshot {
        request_id: request_id(),
        session_id: session_id(),
    });

    assert!(input.session_requests.is_empty());
    assert!(send_file.session_requests.is_empty());
    assert!(resize.session_requests.is_empty());
    assert!(snapshot.session_requests.is_empty());
    assert_eq!(
        input.observations,
        vec![ClientStreamObservation::DroppedUnsubscribedInput {
            session_id: session_id()
        }]
    );
    assert_eq!(
        send_file.observations,
        vec![ClientStreamObservation::DroppedUnsubscribedSendFile {
            session_id: session_id()
        }]
    );
    assert_eq!(
        resize.observations,
        vec![ClientStreamObservation::DroppedUnsubscribedResize {
            session_id: session_id()
        }]
    );
    assert_eq!(
        snapshot.observations,
        vec![ClientStreamObservation::DroppedUnsubscribedSnapshot {
            session_id: session_id()
        }]
    );
}

#[test]
fn subscribed_input_send_file_resize_and_snapshot_emit_session_requests() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let input = harness.handle_ingress(TransportIngress::TerminalInput {
        session_id: session_id(),
        data: b"ls\n".to_vec(),
    });
    let send_file = harness.handle_ingress(TransportIngress::SendFile {
        request_id: request_id(),
        session_id: session_id(),
        data: b"send-file".to_vec(),
    });
    let resize = harness.handle_ingress(TransportIngress::Resize {
        session_id: session_id(),
        rows: 40,
        cols: 120,
    });
    let snapshot = harness.handle_ingress(TransportIngress::RequestSnapshot {
        request_id: request_id(),
        session_id: session_id(),
    });

    assert_eq!(
        input.session_requests,
        vec![(
            session_id(),
            SessionIoRequest::PtyInput {
                session_id: session_id(),
                data: b"ls\n".to_vec()
            }
        )]
    );
    assert_eq!(
        send_file.session_requests,
        vec![(
            session_id(),
            SessionIoRequest::SendFile(SendFileRequest {
                request_id: request_id(),
                session_id: session_id(),
                filename: "send-file".to_string(),
                data: b"send-file".to_vec(),
            })
        )]
    );
    assert_eq!(
        resize.session_requests,
        vec![(
            session_id(),
            SessionIoRequest::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120
            }
        )]
    );
    assert_eq!(
        snapshot.session_requests,
        vec![(
            session_id(),
            SessionIoRequest::GetSnapshot {
                request_id: request_id(),
                session_id: session_id(),
            }
        )]
    );
}

#[test]
fn subscribed_focus_is_a_typed_noop_without_raw_json() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let outcome = harness.handle_ingress(TransportIngress::Focus {
        session_id: session_id(),
        focused: true,
    });

    assert!(outcome.egress.is_empty());
    assert!(outcome.session_requests.is_empty());
    assert!(outcome.observations.is_empty());
}

#[test]
fn unsubscribed_focus_is_dropped_with_observation() {
    let mut harness = ClientStreamHarness::new(client_id());

    let outcome = harness.handle_ingress(TransportIngress::Focus {
        session_id: session_id(),
        focused: true,
    });

    assert!(outcome.session_requests.is_empty());
    assert_eq!(
        outcome.observations,
        vec![ClientStreamObservation::DroppedUnsubscribedFocus {
            session_id: session_id()
        }]
    );
}

#[test]
fn duplicate_subscriptions_are_idempotent() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let duplicate = harness.handle_ingress(TransportIngress::SubscribeSession {
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id("sub-1"),
    });
    let output = harness.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"one".to_vec(),
    });

    assert_eq!(
        duplicate.observations,
        vec![ClientStreamObservation::DuplicateSubscription {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
        }]
    );
    assert_eq!(output.egress.len(), 1);
    assert_eq!(
        harness.active_subscription(&session_id()),
        Some(&subscription_id("sub-1"))
    );
}

#[test]
fn changed_subscription_ids_replace_old_routes() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-old"));

    let replacement = harness.handle_ingress(TransportIngress::SubscribeSession {
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id("sub-new"),
    });
    let output = harness.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"two".to_vec(),
    });
    let old_unsubscribe = harness.handle_ingress(TransportIngress::UnsubscribeSession {
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id("sub-old"),
    });

    assert_eq!(
        replacement.observations,
        vec![ClientStreamObservation::ReplacedSubscription {
            session_id: session_id(),
            old_subscription_id: subscription_id("sub-old"),
            new_subscription_id: subscription_id("sub-new"),
        }]
    );
    assert_eq!(
        output.egress,
        vec![TransportEgress::TerminalOutput {
            session_id: session_id(),
            subscription_id: subscription_id("sub-new"),
            data: b"two".to_vec(),
        }]
    );
    assert_eq!(
        old_unsubscribe.observations,
        vec![ClientStreamObservation::UnsubscribeIgnored {
            session_id: session_id(),
            subscription_id: subscription_id("sub-old"),
        }]
    );
    assert_eq!(
        harness.active_subscription(&session_id()),
        Some(&subscription_id("sub-new"))
    );

    let new_unsubscribe = harness.handle_ingress(TransportIngress::UnsubscribeSession {
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id("sub-new"),
    });

    assert_eq!(
        new_unsubscribe.observations,
        vec![ClientStreamObservation::Unsubscribed {
            session_id: session_id(),
            subscription_id: subscription_id("sub-new"),
        }]
    );
}

#[test]
fn pong_preserves_request_id() {
    let mut harness = ClientStreamHarness::new(client_id());

    let outcome = harness.handle_ingress(TransportIngress::Ping {
        request_id: request_id(),
    });

    assert_eq!(
        outcome.egress,
        vec![TransportEgress::Pong {
            request_id: request_id()
        }]
    );
}

#[test]
fn heartbeat_pong_preserves_request_id() {
    let mut harness = ClientStreamHarness::new(client_id());

    let outcome = harness.handle_ingress(TransportIngress::Heartbeat {
        request_id: request_id(),
    });

    assert_eq!(
        outcome.egress,
        vec![TransportEgress::Pong {
            request_id: request_id()
        }]
    );
}

#[test]
fn boundary_payload_echoes_to_transport_egress() {
    let mut harness = ClientStreamHarness::new(client_id());
    let payload = BoundaryJson(serde_json::json!({ "owned_by": "adapter" }));

    let outcome = harness.handle_ingress(TransportIngress::BoundaryPayload {
        route_id: "route-1".to_string(),
        payload: payload.clone(),
    });

    assert_eq!(
        outcome.egress,
        vec![TransportEgress::BoundaryPayload {
            route_id: "route-1".to_string(),
            payload,
        }]
    );
}

#[test]
fn client_state_emits_control_frame() {
    let mut harness = ClientStreamHarness::new(client_id());

    let outcome = harness.handle_ingress(TransportIngress::ClientState {
        client_id: client_id(),
        state: ClientState::Ready,
    });

    assert_eq!(
        outcome.control_frames,
        vec![ClientControlFrame::State {
            state: ClientState::Ready,
        }]
    );
}

#[test]
fn generation_gating_drops_stale_deliveries() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));
    let stale = harness.generation();
    let current = harness.advance_generation();

    let outcome = harness.handle_session_event_for_generation(
        stale,
        SessionIoEvent::TerminalBytes {
            session_id: session_id(),
            data: b"stale".to_vec(),
        },
    );

    assert!(outcome.egress.is_empty());
    assert!(outcome.session_requests.is_empty());
    assert_eq!(
        outcome.observations,
        vec![ClientStreamObservation::GenerationStale {
            current,
            received: ClientStreamGeneration(0),
        }]
    );
}

#[test]
fn shutdown_closes_transport_and_stops_routing() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let shutdown = harness.shutdown("done");
    let output = harness.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"late".to_vec(),
    });
    let input = harness.handle_ingress(TransportIngress::TerminalInput {
        session_id: session_id(),
        data: b"late".to_vec(),
    });

    assert_eq!(
        shutdown.egress,
        vec![TransportEgress::Close {
            reason: "done".to_string()
        }]
    );
    assert_eq!(
        shutdown.observations,
        vec![ClientStreamObservation::Shutdown {
            reason: "done".to_string()
        }]
    );
    assert_eq!(output.observations, vec![ClientStreamObservation::Closed]);
    assert_eq!(input.observations, vec![ClientStreamObservation::Closed]);
}

#[test]
fn backpressure_is_observable_with_route_context() {
    let harness = ClientStreamHarness::new(client_id());
    let summary = BackpressureSummary {
        source: QueueSource::ClientWorker,
        capacity: 512,
        depth: 500,
        route: BackpressureRoute {
            session_id: Some(session_id()),
            client_id: Some(client_id()),
            subscription_id: Some(subscription_id("sub-1")),
            plugin_key: None,
        },
    };

    let outcome = harness.report_backpressure(summary.clone());

    assert_eq!(
        outcome.control_frames,
        vec![ClientControlFrame::Backpressure(summary.clone())]
    );
    assert_eq!(
        outcome.observations,
        vec![ClientStreamObservation::Backpressure(summary)]
    );
}

#[test]
fn routed_snapshot_scrollback_attach_and_focus_carry_subscription_id() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let snapshot = harness.handle_session_event(SessionIoEvent::SnapshotReady(SnapshotReady {
        request_id: request_id(),
        session_id: session_id(),
        data: b"snap".to_vec(),
        rows: 24,
        cols: 80,
    }));
    let scrollback = harness.handle_scrollback(session_id(), b"history".to_vec());
    let attach = harness.handle_attach_state(
        session_id(),
        botster_core::actor::TerminalAttachState::Attached,
    );

    assert_eq!(
        snapshot.egress,
        vec![TransportEgress::Snapshot {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
            data: b"snap".to_vec(),
        }]
    );
    assert_eq!(
        scrollback.egress,
        vec![TransportEgress::Scrollback {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
            data: b"history".to_vec(),
        }]
    );
    assert_eq!(
        attach.egress,
        vec![TransportEgress::AttachState {
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
            state: botster_core::actor::TerminalAttachState::Attached,
        }]
    );
}

#[test]
fn send_file_failure_event_does_not_create_new_core_storage_policy() {
    let mut harness = ClientStreamHarness::new(client_id());
    subscribe(&mut harness, subscription_id("sub-1"));

    let outcome = harness.handle_session_event(SessionIoEvent::SendFileFailed(SendFileFailed {
        request_id: request_id(),
        session_id: session_id(),
        reason: SendFileErrorReason::TooLarge,
        detail: None,
    }));

    assert!(outcome.egress.is_empty());
    assert!(outcome.session_requests.is_empty());
    assert!(outcome.observations.is_empty());
}

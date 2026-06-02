//! Subscription multiplexer engine acceptance tests.

use botster_core::actor::{
    BackpressureRoute, DeliveryLag, InitialSnapshotReady, MailboxSendFailure,
    MailboxSendFailureReason, QueueSource, SessionIoEvent, SnapshotReady, TerminalAttachState,
};
use botster_core::client::ClientId;
use botster_core::client_stream::ClientStreamObservation;
use botster_core::engine::{SubscriptionMultiplexer, SubscriptionMultiplexerObservation};
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::{ProcessExitedPayload, SessionIoRequest};

fn client_id(id: &str) -> ClientId {
    ClientId(id.to_string())
}

fn session_id() -> SessionId {
    SessionId("session-1".to_string())
}

fn named_session_id(id: &str) -> SessionId {
    SessionId(id.to_string())
}

fn request_id(id: &str) -> RequestId {
    RequestId(id.to_string())
}

fn subscription_id(id: &str) -> SubscriptionId {
    SubscriptionId(id.to_string())
}

fn subscribe(multiplexer: &mut SubscriptionMultiplexer, client: &str, subscription: &str) {
    subscribe_to(multiplexer, client, session_id(), subscription);
}

fn subscribe_to(
    multiplexer: &mut SubscriptionMultiplexer,
    client: &str,
    session: SessionId,
    subscription: &str,
) {
    multiplexer.handle_client_ingress(
        client_id(client),
        TransportIngress::SubscribeSession {
            client_id: client_id(client),
            session_id: session,
            subscription_id: subscription_id(subscription),
        },
    );
}

fn unsubscribe(multiplexer: &mut SubscriptionMultiplexer, client: &str, subscription: &str) {
    multiplexer.handle_client_ingress(
        client_id(client),
        TransportIngress::UnsubscribeSession {
            client_id: client_id(client),
            session_id: session_id(),
            subscription_id: subscription_id(subscription),
        },
    );
}

#[test]
fn multiple_clients_can_subscribe_to_one_session() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    let outcome = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"hello".to_vec(),
    });

    assert_eq!(
        outcome.client_egress,
        vec![
            (
                client_id("client-1"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-1"),
                    data: b"hello".to_vec(),
                },
            ),
            (
                client_id("client-2"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-2"),
                    data: b"hello".to_vec(),
                },
            ),
        ]
    );
}

#[test]
fn one_client_can_switch_subscriptions() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-old");

    let replacement = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::SubscribeSession {
            client_id: client_id("client-1"),
            session_id: session_id(),
            subscription_id: subscription_id("sub-new"),
        },
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"new".to_vec(),
    });
    let exit = multiplexer.handle_session_event(SessionIoEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });

    assert_eq!(
        replacement.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::ReplacedSubscription {
                session_id: session_id(),
                old_subscription_id: subscription_id("sub-old"),
                new_subscription_id: subscription_id("sub-new"),
            },
        }]
    );
    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-1"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-new"),
                data: b"new".to_vec(),
            },
        )]
    );
    assert_eq!(
        exit.client_egress,
        vec![(
            client_id("client-1"),
            TransportEgress::ProcessExit {
                session_id: session_id(),
                subscription_id: subscription_id("sub-new"),
                code: Some(0),
            },
        )]
    );
}

#[test]
fn duplicate_subscriptions_are_idempotent() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");

    let duplicate = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::SubscribeSession {
            client_id: client_id("client-1"),
            session_id: session_id(),
            subscription_id: subscription_id("sub-1"),
        },
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"one".to_vec(),
    });

    assert_eq!(
        duplicate.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DuplicateSubscription {
                session_id: session_id(),
                subscription_id: subscription_id("sub-1"),
            },
        }]
    );
    assert_eq!(output.client_egress.len(), 1);
}

#[test]
fn unsubscribed_inputs_do_not_reach_session_worker() {
    let mut multiplexer = SubscriptionMultiplexer::new();

    let input = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::TerminalInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        },
    );
    let resize = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::Resize {
            session_id: session_id(),
            rows: 24,
            cols: 80,
        },
    );
    let snapshot = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::RequestSnapshot {
            request_id: request_id("snapshot"),
            session_id: session_id(),
        },
    );
    let send_file = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::SendFile {
            request_id: request_id("send"),
            session_id: session_id(),
            data: b"payload".to_vec(),
        },
    );
    let focus = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::Focus {
            session_id: session_id(),
            focused: true,
        },
    );

    assert!(input.session_requests.is_empty());
    assert!(resize.session_requests.is_empty());
    assert!(snapshot.session_requests.is_empty());
    assert!(send_file.session_requests.is_empty());
    assert!(focus.session_requests.is_empty());
    assert_eq!(
        input.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DroppedUnsubscribedInput {
                session_id: session_id(),
            },
        }]
    );
    assert_eq!(
        resize.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DroppedUnsubscribedResize {
                session_id: session_id(),
            },
        }]
    );
    assert_eq!(
        snapshot.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DroppedUnsubscribedSnapshot {
                session_id: session_id(),
            },
        }]
    );
    assert_eq!(
        send_file.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DroppedUnsubscribedSendFile {
                session_id: session_id(),
            },
        }]
    );
    assert_eq!(
        focus.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::DroppedUnsubscribedFocus {
                session_id: session_id(),
            },
        }]
    );
}

#[test]
fn subscribed_input_routes_to_session_worker() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");

    let input = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::TerminalInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        },
    );

    assert_eq!(
        input.session_requests,
        vec![(
            session_id(),
            SessionIoRequest::PtyInput {
                session_id: session_id(),
                data: b"ls\n".to_vec(),
            },
        )]
    );
}

#[test]
fn current_subscribers_only_receive_terminal_output_and_process_exits() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");
    unsubscribe(&mut multiplexer, "client-1", "sub-1");

    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"after".to_vec(),
    });
    let exit = multiplexer.handle_session_event(SessionIoEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(42),
            signal: None,
        },
    });

    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                data: b"after".to_vec(),
            },
        )]
    );
    assert_eq!(
        exit.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::ProcessExit {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                code: Some(42),
            },
        )]
    );
}

#[test]
fn pong_preserves_request_id_for_requesting_client() {
    let mut multiplexer = SubscriptionMultiplexer::new();

    let outcome = multiplexer.handle_client_ingress(
        client_id("client-1"),
        TransportIngress::Ping {
            request_id: request_id("ping-1"),
        },
    );

    assert_eq!(
        outcome.client_egress,
        vec![(
            client_id("client-1"),
            TransportEgress::Pong {
                request_id: request_id("ping-1"),
            },
        )]
    );
}

#[test]
fn backpressure_includes_typed_route_context() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");

    let outcome = multiplexer.report_backpressure(
        client_id("client-1"),
        session_id(),
        QueueSource::ClientWorker,
        512,
        500,
    );

    assert_eq!(outcome.client_control_frames.len(), 1);
    assert_eq!(
        outcome.observations,
        vec![SubscriptionMultiplexerObservation::ClientStream {
            client_id: client_id("client-1"),
            observation: ClientStreamObservation::Backpressure(botster_core::BackpressureSummary {
                source: QueueSource::ClientWorker,
                capacity: 512,
                depth: 500,
                route: BackpressureRoute {
                    session_id: Some(session_id()),
                    client_id: Some(client_id("client-1")),
                    subscription_id: Some(subscription_id("sub-1")),
                    plugin_key: None,
                },
            },),
        }]
    );
}

#[test]
fn backpressure_is_scoped_to_one_client_and_fanout_continues() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    let pressure = multiplexer.report_backpressure(
        client_id("client-1"),
        session_id(),
        QueueSource::ClientWorker,
        512,
        511,
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"after-pressure".to_vec(),
    });

    assert_eq!(pressure.client_control_frames.len(), 1);
    assert_eq!(pressure.client_control_frames[0].0, client_id("client-1"));
    assert_eq!(
        output.client_egress,
        vec![
            (
                client_id("client-1"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-1"),
                    data: b"after-pressure".to_vec(),
                },
            ),
            (
                client_id("client-2"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-2"),
                    data: b"after-pressure".to_vec(),
                },
            ),
        ]
    );
}

#[test]
fn slow_client_on_one_session_does_not_block_unrelated_session() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    let session_a = named_session_id("session-a");
    let session_b = named_session_id("session-b");
    subscribe_to(&mut multiplexer, "client-a", session_a.clone(), "sub-a");
    subscribe_to(&mut multiplexer, "client-b", session_b.clone(), "sub-b");

    let pressure = multiplexer.report_delivery_failure(
        client_id("client-a"),
        session_a.clone(),
        subscription_id("sub-a"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueFull,
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_b.clone(),
        data: b"other-session".to_vec(),
    });

    assert!(pressure.client_egress.is_empty());
    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-b"),
            TransportEgress::TerminalOutput {
                session_id: session_b,
                subscription_id: subscription_id("sub-b"),
                data: b"other-session".to_vec(),
            },
        )]
    );
}

#[test]
fn lag_drop_and_closed_statuses_are_deterministic() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");

    let lag = multiplexer.report_delivery_lag(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::TransportAdapter,
        512,
        128,
    );
    let drop = multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueFull,
    );
    let closed = multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueClosed,
    );

    let route = BackpressureRoute {
        session_id: Some(session_id()),
        client_id: Some(client_id("client-1")),
        subscription_id: Some(subscription_id("sub-1")),
        plugin_key: None,
    };
    assert_eq!(
        lag.observations,
        vec![SubscriptionMultiplexerObservation::DeliveryLagged {
            client_id: client_id("client-1"),
            lag: DeliveryLag {
                source: QueueSource::TransportAdapter,
                capacity: 512,
                depth: 128,
                route: route.clone(),
            },
        }]
    );
    assert_eq!(
        drop.observations,
        vec![SubscriptionMultiplexerObservation::DeliveryFailed {
            client_id: client_id("client-1"),
            failure: MailboxSendFailure {
                source: QueueSource::ClientWorker,
                route: route.clone(),
                reason: MailboxSendFailureReason::QueueFull,
            },
        }]
    );
    assert!(closed.observations.iter().any(|observation| {
        observation
            == &SubscriptionMultiplexerObservation::DeliveryFailed {
                client_id: client_id("client-1"),
                failure: MailboxSendFailure {
                    source: QueueSource::ClientWorker,
                    route: route.clone(),
                    reason: MailboxSendFailureReason::QueueClosed,
                },
            }
    }));
}

#[test]
fn closed_delivery_route_does_not_remove_unrelated_subscribers() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueClosed,
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"after-close".to_vec(),
    });

    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                data: b"after-close".to_vec(),
            },
        )]
    );
}

#[test]
fn queue_full_delivery_failure_keeps_active_route_subscribed() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");

    let full = multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueFull,
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"after-full".to_vec(),
    });

    assert!(full.client_control_frames.is_empty());
    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-1"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-1"),
                data: b"after-full".to_vec(),
            },
        )]
    );
}

#[test]
fn replacement_subscription_pressure_does_not_revive_old_route() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-old");
    subscribe(&mut multiplexer, "client-1", "sub-new");

    let stale = multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-old"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueFull,
    );
    let output = multiplexer.handle_session_event(SessionIoEvent::TerminalBytes {
        session_id: session_id(),
        data: b"current".to_vec(),
    });

    assert!(stale.observations.iter().any(|observation| {
        matches!(
            observation,
            SubscriptionMultiplexerObservation::DeliveryFailed {
                failure,
                ..
            } if failure.route.subscription_id == Some(subscription_id("sub-old"))
        )
    }));
    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-1"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-new"),
                data: b"current".to_vec(),
            },
        )]
    );
}

#[test]
fn process_exit_and_attach_state_use_independent_delivery_routes() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    multiplexer.report_delivery_failure(
        client_id("client-1"),
        session_id(),
        subscription_id("sub-1"),
        QueueSource::ClientWorker,
        MailboxSendFailureReason::QueueClosed,
    );
    let exit = multiplexer.handle_session_event(SessionIoEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(2),
            signal: None,
        },
    });
    let attach = multiplexer.handle_attach_state(session_id(), TerminalAttachState::Detached);

    assert_eq!(
        exit.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::ProcessExit {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                code: Some(2),
            },
        )]
    );
    assert_eq!(
        attach.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::AttachState {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                state: TerminalAttachState::Detached,
            },
        )]
    );
}

#[test]
fn attach_state_fans_out_to_current_subscribers() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    let attached = multiplexer.handle_attach_state(session_id(), TerminalAttachState::Attached);
    unsubscribe(&mut multiplexer, "client-1", "sub-1");
    let detached = multiplexer.handle_attach_state(session_id(), TerminalAttachState::Detached);

    assert_eq!(
        attached.client_egress,
        vec![
            (
                client_id("client-1"),
                TransportEgress::AttachState {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-1"),
                    state: TerminalAttachState::Attached,
                },
            ),
            (
                client_id("client-2"),
                TransportEgress::AttachState {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-2"),
                    state: TerminalAttachState::Attached,
                },
            ),
        ]
    );
    assert_eq!(
        detached.client_egress,
        vec![(
            client_id("client-2"),
            TransportEgress::AttachState {
                session_id: session_id(),
                subscription_id: subscription_id("sub-2"),
                state: TerminalAttachState::Detached,
            },
        )]
    );
}

#[test]
fn snapshot_initial_snapshot_and_scrollback_are_not_broadcast() {
    let mut multiplexer = SubscriptionMultiplexer::new();
    subscribe(&mut multiplexer, "client-1", "sub-1");
    subscribe(&mut multiplexer, "client-2", "sub-2");

    let snapshot = multiplexer.handle_session_event(SessionIoEvent::SnapshotReady(SnapshotReady {
        request_id: request_id("snapshot"),
        session_id: session_id(),
        data: b"snap".to_vec(),
        rows: 24,
        cols: 80,
    }));
    let initial = multiplexer.handle_session_event(SessionIoEvent::InitialSnapshotReady(
        InitialSnapshotReady {
            request_id: request_id("initial"),
            session_id: session_id(),
            client_id: client_id("client-1"),
            subscription_id: subscription_id("sub-1"),
            snapshot: b"initial".to_vec(),
            rows: 24,
            cols: 80,
        },
    ));

    assert!(snapshot.client_egress.is_empty());
    assert!(initial.client_egress.is_empty());
    assert_eq!(
        snapshot.observations,
        vec![
            SubscriptionMultiplexerObservation::SessionEventNotBroadcast {
                session_id: session_id(),
                event_kind: "snapshot_ready".to_string(),
            }
        ]
    );

    let source = include_str!("../src/engine/subscription_multiplexer.rs");
    assert!(!source.contains("Scrollback"));
}

#[test]
fn engine_contract_excludes_concrete_transport_types() {
    let source = include_str!("../src/engine/subscription_multiplexer.rs");
    for forbidden in [
        "WebRTC",
        "webrtc",
        "browser",
        "TUI",
        "Unix",
        "ActionCable",
        "Rails",
        "permission",
        "reconnect",
        "persistence",
    ] {
        assert!(
            !source.contains(forbidden),
            "engine mentions forbidden boundary term: {forbidden}"
        );
    }
}

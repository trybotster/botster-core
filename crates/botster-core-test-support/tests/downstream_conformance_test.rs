//! Downstream-style tests for the public support crate surface.

use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core_test_support::assertions::assert_terminal_output_round_trips;
use botster_core_test_support::fake::FakeSessionTransport;

fn session_id() -> SessionId {
    SessionId("session-consumer".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-consumer".to_string())
}

#[test]
fn downstream_consumer_can_assert_terminal_output_contract() {
    let egress = assert_terminal_output_round_trips(
        session_id(),
        subscription_id(),
        [b"prompt> ".as_slice(), b"done\r\n".as_slice()],
    );

    assert!(matches!(
        &egress[0],
        TransportEgress::TerminalOutput { data, .. } if data == b"prompt> "
    ));
    assert!(matches!(
        &egress[1],
        TransportEgress::TerminalOutput { data, .. } if data == b"done\r\n"
    ));
}

#[test]
fn downstream_consumer_can_record_public_transport_frames() {
    let mut transport = FakeSessionTransport::new(
        ClientId("client-consumer".to_string()),
        session_id(),
        subscription_id(),
    );

    transport.subscribe();
    transport.terminal_input(b"ls\r".to_vec());
    transport.request_snapshot(RequestId("req-consumer".to_string()));
    transport.terminal_output(b"README.md\r\n".to_vec());

    assert!(matches!(
        &transport.ingress()[0],
        TransportIngress::SubscribeSession { session_id, subscription_id, .. }
            if session_id == transport.session_id()
                && subscription_id == transport.subscription_id()
    ));
    assert!(matches!(
        &transport.ingress()[1],
        TransportIngress::TerminalInput { data, .. } if data == b"ls\r"
    ));
    assert!(matches!(
        &transport.ingress()[2],
        TransportIngress::RequestSnapshot { request_id, .. }
            if request_id == &RequestId("req-consumer".to_string())
    ));
    assert!(matches!(
        &transport.egress()[0],
        TransportEgress::TerminalOutput { data, .. } if data == b"README.md\r\n"
    ));
}

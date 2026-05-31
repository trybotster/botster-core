//! Downstream-style tests for the public support crate surface.

use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::{
    CoreSessionMetadata, LocalProcessRuntime, ManagedSessionRuntime, ResizePayload,
    SessionLifecycleState, TerminalScreenEngine, TerminalScreenHook, TerminalScreenSize,
};
use botster_core_test_support::assertions::assert_terminal_output_round_trips;
use botster_core_test_support::conformance::{
    assert_output_activity, assert_shutdown_requested, assert_terminal_output_fanout,
    local_shell_spawn_request, DisposableManagedLocalSession,
};
use botster_core_test_support::fake::{FakeSessionTransport, FakeTerminalScreenRuntime};

fn session_id() -> SessionId {
    SessionId("session-consumer".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-consumer".to_string())
}

fn client_id(value: &str) -> ClientId {
    ClientId(value.to_string())
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

#[test]
fn downstream_consumer_can_drive_terminal_screen_fake() {
    let mut engine = TerminalScreenEngine::new(FakeTerminalScreenRuntime::new());

    engine.resize(TerminalScreenSize::new(33, 101));
    let output = engine.normalize_output(b"downstream\xff");
    let snapshot = engine.capture_snapshot();

    assert_eq!(
        output.hooks,
        vec![TerminalScreenHook::OutputNormalized {
            bytes: b"downstream\xff".len()
        }]
    );
    assert!(matches!(
        snapshot.snapshot,
        Some(snapshot)
            if snapshot.bytes == b"downstream\xff"
                && snapshot.size == TerminalScreenSize::new(33, 101)
    ));
}

#[cfg(unix)]
#[test]
fn downstream_consumer_can_conform_against_managed_local_runtime() {
    use std::time::Duration;

    let request = local_shell_spawn_request(
        RequestId("req-managed-local".to_string()),
        SessionId("session-managed-local".to_string()),
        "printf 'botster-managed-local-output\\n'; sleep 1",
    );
    let mut harness = DisposableManagedLocalSession::spawn(request, CoreSessionMetadata::new())
        .expect("spawn disposable managed local session");
    let _public_runtime: &ManagedSessionRuntime<LocalProcessRuntime> = harness.runtime();

    harness
        .attach_client(
            client_id("client-managed-a"),
            SubscriptionId("sub-managed-a".to_string()),
            10,
        )
        .expect("attach first downstream client");
    harness
        .attach_client(
            client_id("client-managed-b"),
            SubscriptionId("sub-managed-b".to_string()),
            10,
        )
        .expect("attach second downstream client");

    let output = harness
        .drain_runtime_until_output_contains(
            b"botster-managed-local-output",
            20,
            Duration::from_secs(5),
        )
        .expect("drain real PTY output through managed runtime");

    assert_terminal_output_fanout(
        &output,
        harness.session_id(),
        harness.attached_clients(),
        b"botster-managed-local-output",
    );
    assert_output_activity(harness.session().expect("core session after output"), 20);

    harness
        .write_bytes(client_id("client-managed-a"), b"\n".to_vec(), 21)
        .expect("write through managed runtime ingress");
    harness
        .resize(client_id("client-managed-a"), 33, 120, 22)
        .expect("resize through managed runtime ingress");

    let shutdown = harness
        .shutdown("downstream conformance complete", 23)
        .expect("shutdown through managed runtime");
    assert_shutdown_requested(&shutdown, harness.session_id());
    assert_eq!(
        harness.session().map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Stopping)
    );
}

#[test]
fn downstream_consumer_can_build_explicit_local_spawn_request() {
    let request = local_shell_spawn_request(
        RequestId("req-local-shape".to_string()),
        SessionId("session-local-shape".to_string()),
        "printf 'shape'",
    );

    assert_eq!(request.executable, "sh");
    assert_eq!(request.arguments, vec!["-c", "printf 'shape'"]);
    assert_eq!(request.working_directory.path, ".");
    assert_eq!(
        request.initial_pty_size,
        Some(ResizePayload { rows: 24, cols: 80 })
    );
}

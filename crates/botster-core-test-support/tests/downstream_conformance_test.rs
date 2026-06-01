//! Downstream-style tests for the public support crate surface.

use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
#[cfg(feature = "local-runtime")]
use botster_core::{
    CoreSessionMetadata, LocalProcessRuntime, ManagedSessionRuntime, ResizePayload,
    SessionLifecycleState,
};
use botster_core::{
    ModeFlags, SessionIoEvent, TerminalColorProfile, TerminalOutputChunk, TerminalScreenEngine,
    TerminalScreenHook, TerminalScreenRuntime, TerminalScreenSize, TerminalScreenState,
    TerminalSnapshotPayload,
};
use botster_core_test_support::assertions::{
    assert_initial_snapshot_precedes_live_output,
    assert_terminal_backend_opaque_snapshot_conformance,
    assert_terminal_backend_resize_survives_snapshot_restore,
    assert_terminal_backend_screen_state_matches_output_and_metadata,
    assert_terminal_backend_snapshot_round_trips_opaque_state, assert_terminal_output_round_trips,
};
#[cfg(feature = "local-runtime")]
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

#[cfg(feature = "local-runtime")]
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

#[test]
fn downstream_consumer_can_assert_terminal_backend_shadow_state_contract() {
    assert_terminal_backend_snapshot_round_trips_opaque_state(FakeTerminalScreenRuntime::new());
    assert_terminal_backend_opaque_snapshot_conformance(
        FakeTerminalScreenRuntime::new(),
        Some("fake-opaque-v1"),
    );
    assert_terminal_backend_resize_survives_snapshot_restore(FakeTerminalScreenRuntime::new());
}

#[test]
fn downstream_consumer_can_assert_terminal_backend_screen_state_contract() {
    let mut runtime = FakeTerminalScreenRuntime::new();
    let mode_flags = ModeFlags {
        cursor_visible: true,
        bracketed_paste: true,
        mouse_mode: 1,
        ..ModeFlags::default()
    };
    let color_profile = TerminalColorProfile::default();
    let expected_state = TerminalScreenState {
        size: TerminalScreenSize::new(29, 103),
        plain_text: "metadata-backed-screen".to_string(),
        title: Some("contract shell".to_string()),
        cwd: Some("file:///workspace".to_string()),
        mode_flags: mode_flags.clone(),
        color_profile: Some(color_profile.clone()),
    };

    runtime.set_synced_state(
        expected_state.title.clone(),
        expected_state.cwd.clone(),
        mode_flags,
        Some(color_profile),
    );

    assert_terminal_backend_screen_state_matches_output_and_metadata(runtime, expected_state);
}

#[test]
fn downstream_consumer_can_assert_initial_snapshot_before_live_output_contract() {
    let events = assert_initial_snapshot_precedes_live_output();

    assert!(matches!(
        &events[0],
        SessionIoEvent::InitialSnapshotReady(snapshot)
            if snapshot.snapshot == b"initial-snapshot\x00"
                && snapshot.rows == 45
                && snapshot.cols == 120
    ));
    assert!(matches!(
        &events[1],
        SessionIoEvent::TerminalBytes { data, .. } if data == b"live-before-snapshot\xff"
    ));
}

#[test]
fn terminal_backend_conformance_rejects_broken_restore_runtime() {
    let result = std::panic::catch_unwind(|| {
        assert_terminal_backend_resize_survives_snapshot_restore(BrokenRestoreRuntime::default());
    });

    assert!(
        result.is_err(),
        "resize/restore conformance should fail when replay_snapshot drops state"
    );
}

#[derive(Debug, Clone, Default)]
struct BrokenRestoreRuntime {
    inner: FakeTerminalScreenRuntime,
}

impl TerminalScreenRuntime for BrokenRestoreRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.inner.write_output(bytes)
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.inner.resize(size);
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        self.inner.capture_snapshot()
    }

    fn replay_snapshot(&mut self, _payload: TerminalSnapshotPayload) {}

    fn screen_state(&self) -> TerminalScreenState {
        self.inner.screen_state()
    }
}

#[cfg(all(unix, feature = "local-runtime"))]
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

#[cfg(feature = "local-runtime")]
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

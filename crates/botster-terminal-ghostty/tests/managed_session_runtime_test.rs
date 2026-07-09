//! Feature-gated managed-session integration proof for the libghostty-vt adapter.

#![cfg(feature = "libghostty-vt")]

use botster_core::contract::terminal_screen::TerminalScreenSize;
use botster_core::{
    ClientId, CoreSessionMetadata, ManagedSessionRuntime, RequestId, ResizePayload, SessionId,
    SessionIoEvent, SessionIoRequest, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TransportEgress, TransportIngress,
};
use botster_core_test_support::fake::FakeSessionRuntime;
use botster_terminal_ghostty::GhosttyTerminal;

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("ghostty-managed-session-1".to_string())
}

fn session_id_for(value: &str) -> SessionId {
    SessionId(value.to_string())
}

fn client_id(value: &str) -> ClientId {
    ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    spawn_request_for("spawn-ghostty-managed-1", session_id())
}

fn spawn_request_for(request_id_value: &str, session_id: SessionId) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id(request_id_value),
        session_id,
        executable: "fake-shell".to_string(),
        arguments: Vec::new(),
        working_directory: SpawnWorkingDirectory {
            path: "/workspace".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

fn managed_runtime() -> ManagedSessionRuntime<FakeSessionRuntime, GhosttyTerminal> {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            GhosttyTerminal::new(size)
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session with Ghostty terminal backend");
    runtime
}

fn subscribe_transport(runtime: &mut ManagedSessionRuntime<FakeSessionRuntime, GhosttyTerminal>) {
    subscribe_transport_for(
        runtime,
        client_id("client-a"),
        session_id(),
        subscription_id("sub-a"),
        10,
    );
}

fn subscribe_transport_for(
    runtime: &mut ManagedSessionRuntime<FakeSessionRuntime, GhosttyTerminal>,
    client_id: ClientId,
    session_id: SessionId,
    subscription_id: SubscriptionId,
    now_seconds: u64,
) {
    runtime
        .handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
        .expect("subscribe client");
}

#[test]
fn managed_session_path_uses_ghostty_terminal_backend_for_screen_snapshot_and_fanout() {
    let mut runtime = managed_runtime();
    subscribe_transport(&mut runtime);
    let bytes = b"\x1b[31mghostty red\x1b[0m".to_vec();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), bytes.clone());
    let output = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain fake PTY output through managed session runtime");

    assert_terminal_output(
        &output.client_egress,
        &client_id("client-a"),
        &session_id(),
        &subscription_id("sub-a"),
        &bytes,
    );

    let screen = runtime
        .handle_session_request(
            SessionIoRequest::GetScreen {
                request_id: request_id("screen-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("screen reads core-owned Ghostty state");
    assert!(matches!(
        screen.session_events.first(),
        Some(SessionIoEvent::ScreenReady(screen))
            if screen.text.contains("ghostty red")
                && !screen.text.contains("\u{1b}[31m")
                && !screen.text.contains("\u{1b}[0m")
    ));

    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            22,
        )
        .expect("snapshot reads core-owned Ghostty state");
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.rows == 24
                && snapshot.cols == 80
                && !snapshot.data.is_empty()
                && snapshot.data != bytes
    ));
}

#[test]
fn managed_session_initial_snapshot_still_precedes_held_live_output_with_ghostty_backend() {
    let mut runtime = managed_runtime();
    subscribe_transport(&mut runtime);

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"prior ghostty output".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain prior output into Ghostty state");
    let subscribe_outcome = runtime
        .handle_session_request(
            SessionIoRequest::SubscribeTerminal {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id("client-a"),
                subscription_id: subscription_id("sub-a"),
                rows: 30,
                cols: 100,
            },
            21,
        )
        .expect("request initial snapshot through worker path");
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"held ghostty live".to_vec());

    let drain_outcome = runtime
        .drain_runtime_once(&session_id(), 22)
        .expect("deliver initial snapshot and held output");
    let session_events = subscribe_outcome
        .session_events
        .into_iter()
        .chain(drain_outcome.session_events)
        .collect::<Vec<_>>();
    let client_egress = subscribe_outcome
        .client_egress
        .into_iter()
        .chain(drain_outcome.client_egress)
        .collect::<Vec<_>>();

    let initial_position = session_events
        .iter()
        .position(|event| {
            matches!(
                event,
                SessionIoEvent::InitialSnapshotReady(snapshot)
                    if !snapshot.snapshot.is_empty()
                        && snapshot.snapshot != b"prior ghostty output"
                        && snapshot.rows == 24
                        && snapshot.cols == 80
            )
        })
        .expect("initial Ghostty snapshot should be emitted");
    let live_position = session_events
        .iter()
        .position(|event| {
            matches!(event, SessionIoEvent::TerminalBytes { data, .. } if data == b"held ghostty live")
        })
        .expect("held live terminal bytes should be emitted");
    assert!(
        initial_position < live_position,
        "initial snapshot should precede held live output: {:?}",
        session_events
    );
    assert!(
        client_egress.iter().any(|(_, frame)| {
            matches!(frame, TransportEgress::TerminalOutput { data, .. } if data == b"held ghostty live")
        }),
        "held live output should be delivered to client egress"
    );
}

#[test]
fn managed_session_backend_factory_receives_spawn_size() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            assert_eq!(size, TerminalScreenSize::new(24, 80));
            GhosttyTerminal::new(size)
        });

    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session with requested terminal size");
}

#[test]
fn managed_session_path_keeps_multiple_ghostty_backends_isolated_through_interleaved_operations() {
    let session_a = session_id_for("ghostty-managed-session-a");
    let session_b = session_id_for("ghostty-managed-session-b");
    let client_a = client_id("client-a");
    let client_b = client_id("client-b");
    let subscription_a = subscription_id("sub-a");
    let subscription_b = subscription_id("sub-b");
    let bytes_a = b"\x1b[32msession A green\x1b[0m".to_vec();
    let bytes_b = b"\x1b[34msession B blue\x1b[0m".to_vec();
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            GhosttyTerminal::new(size)
        });

    runtime
        .spawn_session(
            spawn_request_for("spawn-ghostty-managed-a", session_a.clone()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn first Ghostty managed session");
    runtime
        .spawn_session(
            spawn_request_for("spawn-ghostty-managed-b", session_b.clone()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn second Ghostty managed session");
    subscribe_transport_for(
        &mut runtime,
        client_a.clone(),
        session_a.clone(),
        subscription_a.clone(),
        10,
    );
    subscribe_transport_for(
        &mut runtime,
        client_b.clone(),
        session_b.clone(),
        subscription_b.clone(),
        11,
    );

    runtime
        .session_runtime_mut()
        .emit_output(session_a.clone(), bytes_a.clone());
    runtime
        .session_runtime_mut()
        .emit_output(session_b.clone(), bytes_b.clone());
    let output_a = runtime
        .drain_runtime_once(&session_a, 20)
        .expect("drain first Ghostty session output");

    runtime
        .handle_session_request(
            SessionIoRequest::Resize {
                session_id: session_b.clone(),
                rows: 33,
                cols: 101,
            },
            21,
        )
        .expect("resize second Ghostty session before draining it");
    let output_b = runtime
        .drain_runtime_once(&session_b, 22)
        .expect("drain second Ghostty session output");
    runtime
        .handle_client_ingress(
            client_a.clone(),
            TransportIngress::Resize {
                session_id: session_a.clone(),
                rows: 41,
                cols: 111,
            },
            23,
        )
        .expect("resize first Ghostty session through client ingress");

    let screen_a = runtime
        .handle_session_request(
            SessionIoRequest::GetScreen {
                request_id: request_id("screen-a"),
                session_id: session_a.clone(),
            },
            24,
        )
        .expect("screen reads first Ghostty session");
    let screen_b = runtime
        .handle_session_request(
            SessionIoRequest::GetScreen {
                request_id: request_id("screen-b"),
                session_id: session_b.clone(),
            },
            25,
        )
        .expect("screen reads second Ghostty session");
    let snapshot_a = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-a"),
                session_id: session_a.clone(),
            },
            26,
        )
        .expect("snapshot reads first Ghostty session");
    let snapshot_b = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-b"),
                session_id: session_b.clone(),
            },
            27,
        )
        .expect("snapshot reads second Ghostty session");

    assert_terminal_output(
        &output_a.client_egress,
        &client_a,
        &session_a,
        &subscription_a,
        &bytes_a,
    );
    assert_terminal_output(
        &output_b.client_egress,
        &client_b,
        &session_b,
        &subscription_b,
        &bytes_b,
    );
    assert!(matches!(
        screen_a.session_events.first(),
        Some(SessionIoEvent::ScreenReady(screen))
            if screen.session_id == session_a
                && screen.text.contains("session A green")
                && !screen.text.contains("session B blue")
                && !screen.text.contains("\u{1b}[32m")
    ));
    assert!(matches!(
        screen_b.session_events.first(),
        Some(SessionIoEvent::ScreenReady(screen))
            if screen.session_id == session_b
                && screen.text.contains("session B blue")
                && !screen.text.contains("session A green")
                && !screen.text.contains("\u{1b}[34m")
    ));
    assert!(matches!(
        snapshot_a.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.session_id == session_a
                && snapshot.rows == 41
                && snapshot.cols == 111
                && !snapshot.data.is_empty()
                && snapshot.data != bytes_a
    ));
    assert!(matches!(
        snapshot_b.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.session_id == session_b
                && snapshot.rows == 33
                && snapshot.cols == 101
                && !snapshot.data.is_empty()
                && snapshot.data != bytes_b
    ));
}

fn assert_terminal_output(
    frames: &[(ClientId, TransportEgress)],
    expected_client: &ClientId,
    expected_session: &SessionId,
    expected_subscription: &SubscriptionId,
    expected_data: &[u8],
) {
    assert!(
        frames.iter().any(|(client, frame)| {
            matches!(
                frame,
                TransportEgress::TerminalOutput {
                    session_id,
                    subscription_id,
                    data,
                } if client == expected_client
                    && session_id == expected_session
                    && subscription_id == expected_subscription
                    && data == expected_data
            )
        }),
        "expected terminal output frame was not present: {frames:?}"
    );
}

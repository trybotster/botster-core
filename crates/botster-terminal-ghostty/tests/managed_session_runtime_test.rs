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

fn client_id(value: &str) -> ClientId {
    ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("spawn-ghostty-managed-1"),
        session_id: session_id(),
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
    runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::SubscribeSession {
                client_id: client_id("client-a"),
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
            },
            10,
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

    assert_eq!(
        output.client_egress,
        vec![(
            client_id("client-a"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
                data: bytes.clone(),
            },
        )]
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
    runtime
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

    let outcome = runtime
        .drain_runtime_once(&session_id(), 22)
        .expect("deliver initial snapshot and held output");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::InitialSnapshotReady(snapshot))
            if !snapshot.snapshot.is_empty()
                && snapshot.snapshot != b"prior ghostty output"
                && snapshot.rows == 30
                && snapshot.cols == 100
    ));
    assert!(matches!(
        outcome.session_events.get(1),
        Some(SessionIoEvent::TerminalBytes { data, .. }) if data == b"held ghostty live"
    ));
    assert!(matches!(
        outcome.client_egress.first(),
        Some((_, TransportEgress::TerminalOutput { data, .. }))
            if data == b"held ghostty live"
    ));
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

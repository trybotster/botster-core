//! Managed session runtime acceptance tests.

use std::fs;

use botster_core::{
    BackpressureRoute, CoreSessionMetadata, MailboxSendFailureReason, ManagedSessionRuntime,
    ManagedSessionRuntimeError, ProcessExitedPayload, QueueSource, RequestId, ResizePayload,
    SessionId, SessionIoEvent, SessionIoRequest, SessionLifecycleState, SessionRuntimeError,
    SessionRuntimeErrorKind, SessionRuntimeInput, SessionSpawnRequest, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TerminalColorProfile, TransportEgress, TransportIngress,
};
use botster_core_test_support::fake::{FakeSessionIoMailbox, FakeSessionRuntime};

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("managed-session-1".to_string())
}

fn client_id(value: &str) -> botster_core::ClientId {
    botster_core::ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("spawn-managed-1"),
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

fn managed_runtime() -> ManagedSessionRuntime<FakeSessionRuntime> {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");
    runtime
}

fn subscribe(runtime: &mut ManagedSessionRuntime<FakeSessionRuntime>) {
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
fn supervised_session_reader_events_reach_subscription_multiplexer() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    assert!(
        runtime.session_runtime().inputs().is_empty(),
        "SubscribeSession establishes fanout only; it does not hydrate global state or touch the runtime"
    );

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"hello".to_vec());
    let outcome = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output");

    assert_eq!(
        outcome.client_egress,
        vec![(
            client_id("client-a"),
            TransportEgress::TerminalOutput {
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
                data: b"hello".to_vec(),
            },
        )]
    );

    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-after-output"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot after output");
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.data == b"hello" && snapshot.rows == 24 && snapshot.cols == 80
    ));
}

#[test]
fn supervised_session_pty_output_updates_shadow_terminal_snapshot() {
    let mut runtime = managed_runtime();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"shadow bytes".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output");

    let outcome = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot reads core state");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.data == b"shadow bytes" && snapshot.rows == 24 && snapshot.cols == 80
    ));
}

#[test]
fn supervised_session_screen_read_uses_core_shadow_terminal_state() {
    let mut runtime = managed_runtime();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"line one\nline two".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output");

    let outcome = runtime
        .handle_session_request(
            SessionIoRequest::GetScreen {
                request_id: request_id("screen-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("screen reads core state");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::ScreenReady(screen)) if screen.text.contains("line two")
    ));
}

#[test]
fn supervised_session_writer_requests_route_to_session_runtime() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

    runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::TerminalInput {
                session_id: session_id(),
                data: b"ls\n".to_vec(),
            },
            11,
        )
        .expect("terminal input");

    assert!(runtime
        .session_runtime()
        .inputs()
        .contains(&SessionRuntimeInput::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        }));
}

#[test]
fn supervised_session_resize_forwarding_reaches_runtime_before_snapshot() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

    runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120,
            },
            11,
        )
        .expect("resize");
    let outcome = runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::RequestSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            12,
        )
        .expect("snapshot request is supported");

    assert_eq!(
        runtime.session_runtime().inputs().last(),
        Some(&SessionRuntimeInput::Resize {
            session_id: session_id(),
            size: ResizePayload {
                rows: 40,
                cols: 120
            },
        })
    );
    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.rows == 40 && snapshot.cols == 120
    ));
}

#[test]
fn supervised_session_live_output_fanout_still_emits_original_bytes() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let bytes = b"\x1b[31mhello\x00\xff".to_vec();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), bytes.clone());
    let output = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output");
    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot reads same bytes");

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
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot)) if snapshot.data == bytes
    ));
}

#[test]
fn supervised_session_request_snapshot_is_no_longer_unsupported() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"snapshot payload".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output");

    let outcome = runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::RequestSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("request snapshot is supported");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot)) if snapshot.data == b"snapshot payload"
    ));
}

#[test]
fn supervised_session_initial_snapshot_precedes_live_output_from_shadow_state() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

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
            20,
        )
        .expect("subscribe terminal");
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"live after request".to_vec());
    let outcome = runtime
        .drain_runtime_once(&session_id(), 21)
        .expect("deliver initial snapshot and held output");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::InitialSnapshotReady(snapshot))
            if snapshot.snapshot.is_empty() && snapshot.rows == 30 && snapshot.cols == 100
    ));
    assert!(matches!(
        outcome.session_events.get(1),
        Some(SessionIoEvent::TerminalBytes { data, .. }) if data == b"live after request"
    ));
    assert!(matches!(
        outcome.client_egress.first(),
        Some((_, TransportEgress::TerminalOutput { data, .. }))
            if data == b"live after request"
    ));
}

#[test]
fn supervised_session_initial_snapshot_after_prior_output_reflects_shadow_state() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"prior output".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain prior output");
    runtime
        .handle_session_request(
            SessionIoRequest::SubscribeTerminal {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id("client-a"),
                subscription_id: subscription_id("sub-a"),
                rows: 24,
                cols: 80,
            },
            21,
        )
        .expect("subscribe terminal");
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"held live".to_vec());

    let outcome = runtime
        .drain_runtime_once(&session_id(), 22)
        .expect("deliver initial snapshot and held output");

    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::InitialSnapshotReady(snapshot))
            if snapshot.snapshot == b"prior output"
    ));
    assert!(matches!(
        outcome.session_events.get(1),
        Some(SessionIoEvent::TerminalBytes { data, .. }) if data == b"held live"
    ));
}

#[test]
fn supervised_session_resize_updates_runtime_and_shadow_before_snapshot() {
    let mut runtime = managed_runtime();

    runtime
        .handle_session_request(
            SessionIoRequest::Resize {
                session_id: session_id(),
                rows: 50,
                cols: 132,
            },
            20,
        )
        .expect("resize");
    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot");

    assert_eq!(
        runtime.session_runtime().inputs().last(),
        Some(&SessionRuntimeInput::Resize {
            session_id: session_id(),
            size: ResizePayload {
                rows: 50,
                cols: 132
            },
        })
    );
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.rows == 50 && snapshot.cols == 132
    ));
}

#[test]
fn supervised_session_mode_and_color_paths_stay_explicitly_unsupported_or_are_backed() {
    let mut runtime = managed_runtime();

    let mode_error = runtime
        .handle_session_request(
            SessionIoRequest::GetModeFlags {
                request_id: request_id("mode-1"),
                session_id: session_id(),
            },
            20,
        )
        .expect_err("mode flags remain explicitly unsupported");
    assert!(matches!(
        mode_error,
        ManagedSessionRuntimeError::UnsupportedSessionRequest {
            request_kind: "get_mode_flags",
        }
    ));

    let color_error = runtime
        .handle_session_request(
            SessionIoRequest::SetColorProfile {
                session_id: session_id(),
                color_profile: TerminalColorProfile::default(),
            },
            21,
        )
        .expect_err("color profile remains explicitly unsupported");
    assert!(matches!(
        color_error,
        ManagedSessionRuntimeError::UnsupportedSessionRequest {
            request_kind: "set_color_profile",
        }
    ));
}

#[test]
fn supervised_session_shutdown_closes_worker_and_runtime_exactly_once() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

    runtime
        .shutdown_session(session_id(), "host shutdown", 20)
        .expect("first shutdown");
    runtime
        .shutdown_session(session_id(), "duplicate shutdown", 21)
        .expect("duplicate shutdown is idempotent");
    runtime
        .handle_session_request(
            SessionIoRequest::PtyInput {
                session_id: session_id(),
                data: b"ignored".to_vec(),
            },
            22,
        )
        .expect("closed worker ignores later input");

    assert_eq!(
        runtime
            .session_runtime()
            .inputs()
            .iter()
            .filter(|input| matches!(input, SessionRuntimeInput::Shutdown { .. }))
            .count(),
        1
    );
    assert!(!runtime
        .session_runtime()
        .inputs()
        .contains(&SessionRuntimeInput::PtyInput {
            session_id: session_id(),
            data: b"ignored".to_vec(),
        }));
}

#[test]
fn supervised_session_process_exit_flushes_ordered_output_before_lifecycle_exit() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"tail".to_vec());
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );

    let outcome = runtime
        .drain_runtime_once(&session_id(), 30)
        .expect("drain output then exit");

    assert!(matches!(
        outcome.client_egress.first(),
        Some((
            _,
            TransportEgress::TerminalOutput {
                data,
                ..
            }
        )) if data == b"tail"
    ));
    assert!(matches!(
        outcome.client_egress.get(1),
        Some((_, TransportEgress::ProcessExit { code: Some(0), .. }))
    ));
    assert_eq!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Exited { code: Some(0) })
    );
}

#[test]
fn supervised_session_runtime_drain_error_returns_typed_managed_runtime_error_for_hosts() {
    let mut runtime = managed_runtime();
    runtime
        .session_runtime_mut()
        .fail_next_drain(SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            "read failed",
        ));

    let error = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect_err("drain should return typed error");

    match error {
        ManagedSessionRuntimeError::Runtime(error) => {
            assert_eq!(error.kind, SessionRuntimeErrorKind::OutputFailed);
        }
        other => panic!("expected runtime error, got {other:?}"),
    }
}

#[test]
fn supervised_session_surfaces_session_side_queue_full_and_closed_failures() {
    let route = BackpressureRoute {
        session_id: Some(session_id()),
        client_id: None,
        subscription_id: None,
        plugin_key: None,
    };
    let mut full = FakeSessionIoMailbox::new(0, route.clone());
    let full_failure = full
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "full".to_string(),
        })
        .expect_err("queue full");
    assert_eq!(full_failure.source, QueueSource::SessionIo);
    assert_eq!(full_failure.reason, MailboxSendFailureReason::QueueFull);
    assert_eq!(full_failure.route.client_id, None);

    let mut closed = FakeSessionIoMailbox::new(1, route);
    closed.close();
    let closed_failure = closed
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "closed".to_string(),
        })
        .expect_err("queue closed");
    assert_eq!(closed_failure.source, QueueSource::SessionIo);
    assert_eq!(closed_failure.reason, MailboxSendFailureReason::QueueClosed);
}

#[test]
fn supervised_session_contract_excludes_concrete_transport_and_product_policy() {
    let source = fs::read_to_string("src/engine/managed_session_runtime.rs")
        .expect("read managed runtime source");

    for forbidden in [
        "WebRTC",
        "browser",
        "TUI",
        "ActionCable",
        "Rails",
        "ProjectPipelines",
        "auth",
        "retention",
        "reconnect",
        "cloud",
        "restty",
        "Ghostty",
    ] {
        assert!(
            !source.contains(forbidden),
            "managed session runtime must not contain {forbidden}"
        );
    }
}

#[test]
fn supervised_session_does_not_add_pushed_terminal_mode_event_variants() {
    let actor_source = fs::read_to_string("src/contract/actor.rs").expect("read actor source");
    let transport_source =
        fs::read_to_string("src/contract/transport.rs").expect("read transport source");

    for pushed_variant in ["ModeChanged", "ColorChanged", "TerminalModeChanged"] {
        assert!(!actor_source.contains(pushed_variant));
        assert!(!transport_source.contains(pushed_variant));
    }
}

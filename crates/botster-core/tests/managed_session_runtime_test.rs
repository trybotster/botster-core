//! Managed session runtime acceptance tests.

use std::fs;

use botster_core::{
    BackpressureRoute, CoreSessionMetadata, MailboxSendFailureReason, ManagedSessionRuntime,
    ManagedSessionRuntimeError, ProcessExitedPayload, QueueSource, RequestId, ResizePayload,
    SessionId, SessionIoRequest, SessionLifecycleState, SessionRuntimeError,
    SessionRuntimeErrorKind, SessionRuntimeInput, SessionSpawnRequest, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TransportEgress, TransportIngress,
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
    let error = runtime
        .handle_client_ingress(
            client_id("client-a"),
            TransportIngress::RequestSnapshot {
                request_id: request_id("snapshot-1"),
                session_id: session_id(),
            },
            12,
        )
        .expect_err("managed runtime does not fabricate snapshots");
    assert!(matches!(
        error,
        ManagedSessionRuntimeError::UnsupportedSessionRequest {
            request_kind: "request_snapshot",
        }
    ));

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

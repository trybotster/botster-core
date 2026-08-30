//! Managed session runtime acceptance tests.

use std::cell::{Cell, RefCell};
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use botster_core::{
    BackpressureRoute, BackpressureSummary, CoreSessionMetadata, MailboxSendFailureReason,
    ManagedSessionRuntime, ManagedSessionRuntimeError, ModeFlags, MultiplexerEngineObservation,
    ProcessExitedPayload, QueueSource, RequestId, ResizePayload, SessionId, SessionIoEvent,
    SessionIoRequest, SessionLifecycleState, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionRuntimeInput, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TerminalAttachState, TerminalBackendError, TerminalColorProfile,
    TerminalOutputChunk, TerminalScreenRuntime, TerminalScreenSize, TerminalScreenState,
    TerminalSnapshotPayload, TransportEgress, TransportIngress,
};
use botster_core_test_support::fake::{FakeSessionIoMailbox, FakeSessionRuntime};

use botster_core::engine::terminal_screen::PLAIN_TERMINAL_SCREEN_MAX_BYTES;

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("managed-session-1".to_string())
}

fn numbered_session_id(index: usize) -> SessionId {
    SessionId(format!("managed-session-{index}"))
}

fn client_id(value: &str) -> botster_core::ClientId {
    botster_core::ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    spawn_request_for(session_id(), "spawn-managed-1")
}

fn spawn_request_for(session_id: SessionId, request_id_value: &str) -> SessionSpawnRequest {
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

fn oversized_plain_bytes() -> Vec<u8> {
    let mut bytes = vec![b'a'; PLAIN_TERMINAL_SCREEN_MAX_BYTES];
    bytes.extend_from_slice(b"managed-bounded-tail");
    bytes
}

fn managed_runtime() -> ManagedSessionRuntime<FakeSessionRuntime> {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");
    runtime
}

fn subscribe(runtime: &mut ManagedSessionRuntime<FakeSessionRuntime>) {
    subscribe_to(
        runtime,
        client_id("client-a"),
        session_id(),
        subscription_id("sub-a"),
    );
}

fn subscribe_to(
    runtime: &mut ManagedSessionRuntime<FakeSessionRuntime>,
    client_id: botster_core::ClientId,
    session_id: SessionId,
    subscription_id: SubscriptionId,
) {
    runtime
        .handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            10,
        )
        .expect("subscribe client");
}

fn spawn_numbered_sessions(
    runtime: &mut ManagedSessionRuntime<FakeSessionRuntime>,
    count: usize,
) -> Vec<(SessionId, botster_core::ClientId, SubscriptionId)> {
    (0..count)
        .map(|index| {
            let session_id = numbered_session_id(index);
            let client_id = client_id(&format!("client-{index}"));
            let subscription_id = subscription_id(&format!("sub-{index}"));
            runtime
                .spawn_session(
                    spawn_request_for(session_id.clone(), &format!("spawn-managed-{index}")),
                    CoreSessionMetadata::new(),
                )
                .expect("spawn numbered session");
            subscribe_to(
                runtime,
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            );
            (session_id, client_id, subscription_id)
        })
        .collect()
}

fn terminal_output_bytes_for(
    outcome: &botster_core::MultiplexerEngineOutcome,
    client_id: &botster_core::ClientId,
    subscription_id: &SubscriptionId,
    session_id: &SessionId,
) -> Vec<u8> {
    outcome
        .client_egress
        .iter()
        .filter_map(|(received_client, egress)| match egress {
            TransportEgress::TerminalOutput {
                session_id: received_session_id,
                subscription_id: received_subscription_id,
                data,
            } if received_client == client_id
                && received_session_id == session_id
                && received_subscription_id == subscription_id =>
            {
                Some(data.as_slice())
            }
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

#[derive(Debug, Clone)]
struct SpyTerminalRuntime {
    size: TerminalScreenSize,
    writes: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl SpyTerminalRuntime {
    fn new(size: TerminalScreenSize, writes: Rc<RefCell<Vec<Vec<u8>>>>) -> Self {
        Self { size, writes }
    }
}

impl TerminalScreenRuntime for SpyTerminalRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.writes.borrow_mut().push(bytes.to_vec());
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.size = size;
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        TerminalSnapshotPayload::new(
            self.writes.borrow().concat(),
            self.size,
            Some("spy-terminal-snapshot-v1".to_string()),
        )
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.size = payload.size;
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState::new(
            self.size,
            String::from_utf8_lossy(&self.writes.borrow().concat()).into_owned(),
        )
    }

    fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
        Ok(ModeFlags {
            mouse_mode: 9,
            ..ModeFlags::default()
        })
    }
}

#[derive(Debug, Clone)]
struct FailingTerminalRuntime {
    size: TerminalScreenSize,
    message: &'static str,
}

#[derive(Debug, Clone)]
struct ControlledFailingTerminalRuntime {
    size: TerminalScreenSize,
    fail_resize: Rc<Cell<bool>>,
    fail_snapshot: Rc<Cell<bool>>,
    last_error: Option<String>,
}

impl ControlledFailingTerminalRuntime {
    fn new(
        size: TerminalScreenSize,
        fail_resize: Rc<Cell<bool>>,
        fail_snapshot: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            size,
            fail_resize,
            fail_snapshot,
            last_error: None,
        }
    }
}

impl TerminalScreenRuntime for ControlledFailingTerminalRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        if self.fail_resize.get() {
            self.last_error = Some("controlled resize failure".to_string());
        } else {
            self.size = size;
            self.last_error = None;
        }
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        if self.fail_snapshot.get() {
            self.last_error = Some("controlled snapshot_export failure".to_string());
            TerminalSnapshotPayload::new(
                Vec::new(),
                self.size,
                Some("controlled-opaque-v1".to_string()),
            )
        } else {
            self.last_error = None;
            TerminalSnapshotPayload::new(
                b"controlled snapshot".to_vec(),
                self.size,
                Some("controlled-opaque-v1".to_string()),
            )
        }
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.size = payload.size;
        self.last_error = None;
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState::new(self.size, String::new())
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.clone()
    }
}

impl FailingTerminalRuntime {
    const fn new(size: TerminalScreenSize, message: &'static str) -> Self {
        Self { size, message }
    }
}

impl TerminalScreenRuntime for FailingTerminalRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.size = size;
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        TerminalSnapshotPayload::new(Vec::new(), self.size, Some("failing-opaque-v1".to_string()))
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.size = payload.size;
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState::new(self.size, String::new())
    }

    fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
        Err(TerminalBackendError::operation_failed(
            "mode_flags",
            self.message,
        ))
    }

    fn last_error(&self) -> Option<String> {
        Some(self.message.to_string())
    }
}

#[test]
fn managed_session_runtime_fair_drain_visits_each_active_session_once_per_tick() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    let sessions = spawn_numbered_sessions(&mut runtime, 3);

    for (index, (session_id, _, _)) in sessions.iter().enumerate() {
        runtime
            .session_runtime_mut()
            .emit_output(session_id.clone(), format!("fair:{index}\n").into_bytes());
    }

    let outcome = runtime
        .drain_runtime_all_once(20)
        .expect("fair drain all sessions");

    for (index, (session_id, client_id, subscription_id)) in sessions.iter().enumerate() {
        assert_eq!(
            terminal_output_bytes_for(&outcome, client_id, subscription_id, session_id),
            format!("fair:{index}\n").into_bytes(),
            "each active session should deliver output in the same aggregate tick"
        );
        assert_eq!(
            runtime
                .session_runtime()
                .drain_attempts()
                .iter()
                .filter(|attempted| *attempted == session_id)
                .count(),
            1,
            "each active session should be drained exactly once in one fair tick"
        );
    }
}

#[test]
fn managed_session_runtime_fair_drain_bounds_work_to_one_drain_per_session_per_tick() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    let sessions = spawn_numbered_sessions(&mut runtime, 3);
    let noisy_session = &sessions[0].0;

    for line in 0..64 {
        runtime.session_runtime_mut().emit_output(
            noisy_session.clone(),
            format!("noisy:{line}\n").into_bytes(),
        );
    }
    for (index, (session_id, _, _)) in sessions.iter().enumerate().skip(1) {
        runtime
            .session_runtime_mut()
            .emit_output(session_id.clone(), format!("quiet:{index}\n").into_bytes());
    }

    let outcome = runtime
        .drain_runtime_all_once(20)
        .expect("fair drain noisy and quiet sessions");

    assert!(
        terminal_output_bytes_for(&outcome, &sessions[0].1, &sessions[0].2, &sessions[0].0)
            .windows(b"noisy:63\n".len())
            .any(|window| window == b"noisy:63\n"),
        "byte volume remains runtime-defined, but noisy session gets one drain call"
    );
    for (index, (session_id, client_id, subscription_id)) in sessions.iter().enumerate().skip(1) {
        assert_eq!(
            terminal_output_bytes_for(&outcome, client_id, subscription_id, session_id),
            format!("quiet:{index}\n").into_bytes(),
            "quiet sessions must deliver in the same aggregate tick as noisy output"
        );
    }
    for (session_id, _, _) in &sessions {
        assert_eq!(
            runtime
                .session_runtime()
                .drain_attempts()
                .iter()
                .filter(|attempted| *attempted == session_id)
                .count(),
            1,
            "fair drain bound is one drain_output call per session per tick, not bytes per tick"
        );
    }
}

#[test]
fn managed_session_runtime_fair_drain_skips_empty_sessions_without_starving_later_output() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    let sessions = spawn_numbered_sessions(&mut runtime, 3);
    let later = &sessions[2];
    runtime
        .session_runtime_mut()
        .emit_output(later.0.clone(), b"later output".to_vec());

    let outcome = runtime
        .drain_runtime_all_once(20)
        .expect("fair drain with empty earlier sessions");

    assert_eq!(
        terminal_output_bytes_for(&outcome, &later.1, &later.2, &later.0),
        b"later output".to_vec()
    );
    for (session_id, _, _) in &sessions {
        assert_eq!(
            runtime
                .session_runtime()
                .drain_attempts()
                .iter()
                .filter(|attempted| *attempted == session_id)
                .count(),
            1,
            "empty sessions should be attempted once without blocking later sessions"
        );
    }
}

#[test]
fn managed_session_runtime_fair_drain_tolerates_exited_session_and_preserves_per_session_order() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    let sessions = spawn_numbered_sessions(&mut runtime, 2);
    let exited = &sessions[0];
    let live = &sessions[1];

    runtime
        .session_runtime_mut()
        .emit_output(exited.0.clone(), b"tail before exit".to_vec());
    runtime.session_runtime_mut().emit_exit(
        exited.0.clone(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );
    runtime
        .session_runtime_mut()
        .emit_output(live.0.clone(), b"live after exit".to_vec());

    let outcome = runtime
        .drain_runtime_all_once(20)
        .expect("fair drain with one exited session");

    let exited_frames = outcome
        .client_egress
        .iter()
        .filter_map(|(client_id, egress)| {
            if client_id == &exited.1 {
                Some(egress)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        exited_frames.first(),
        Some(TransportEgress::TerminalOutput { data, .. }) if data == b"tail before exit"
    ));
    assert!(matches!(
        exited_frames.get(1),
        Some(TransportEgress::ProcessExit { code: Some(0), .. })
    ));
    assert_eq!(
        terminal_output_bytes_for(&outcome, &live.1, &live.2, &live.0),
        b"live after exit".to_vec(),
        "another session's output should still deliver in the same aggregate tick"
    );

    let followup = runtime
        .drain_runtime_all_once(21)
        .expect("fair drain should skip runtime-removed exited sessions");
    assert!(followup.client_egress.is_empty());
}

#[test]
fn managed_session_runtime_fair_drain_routes_pending_runtime_events_once() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    let sessions = spawn_numbered_sessions(&mut runtime, 2);
    let initial = &sessions[0];

    runtime
        .handle_session_request(
            SessionIoRequest::SubscribeTerminal {
                request_id: request_id("initial-fair"),
                session_id: initial.0.clone(),
                client_id: initial.1.clone(),
                subscription_id: initial.2.clone(),
                rows: 30,
                cols: 100,
            },
            20,
        )
        .expect("queue initial snapshot");

    let outcome = runtime
        .drain_runtime_all_once(21)
        .expect("fair drain routes pending runtime events once");

    assert_eq!(
        outcome
            .session_events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    SessionIoEvent::InitialSnapshotReady(snapshot)
                        if snapshot.request_id == request_id("initial-fair")
                )
            })
            .count(),
        1,
        "pending worker runtime events should be routed once per aggregate tick, not once per session"
    );
}

#[test]
fn managed_session_runtime_fair_drain_error_returns_typed_runtime_error() {
    let mut runtime = ManagedSessionRuntime::new(FakeSessionRuntime::new());
    spawn_numbered_sessions(&mut runtime, 2);
    runtime
        .session_runtime_mut()
        .fail_next_drain(SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            "aggregate read failed",
        ));

    let error = runtime
        .drain_runtime_all_once(20)
        .expect_err("aggregate drain should abort on typed runtime error");

    match error {
        ManagedSessionRuntimeError::Runtime(error) => {
            assert_eq!(error.kind, SessionRuntimeErrorKind::OutputFailed);
        }
        other => panic!("expected runtime error, got {other:?}"),
    }
}

#[test]
fn supervised_session_reader_events_reach_subscription_multiplexer() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let initial = runtime
        .drain_runtime_once(&session_id(), 19)
        .expect("drain subscribe-triggered initial snapshot");
    assert!(matches!(
        initial.session_events.first(),
        Some(SessionIoEvent::InitialSnapshotReady(snapshot)) if snapshot.snapshot.is_empty()
    ));
    assert_eq!(
        initial.client_egress,
        vec![(
            client_id("client-a"),
            TransportEgress::AttachState {
                session_id: session_id(),
                subscription_id: subscription_id("sub-a"),
                state: TerminalAttachState::Attached,
            },
        )]
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
fn supervised_session_reader_backpressure_reaches_managed_runtime_observations() {
    let mut runtime = managed_runtime();
    let summary = BackpressureSummary {
        source: QueueSource::SessionIo,
        capacity: 1,
        depth: 1,
        route: BackpressureRoute {
            session_id: Some(session_id()),
            client_id: None,
            subscription_id: None,
            plugin_key: None,
        },
    };
    runtime
        .session_runtime_mut()
        .emit_backpressure(summary.clone());

    let outcome = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime backpressure");

    assert_eq!(
        outcome.observations,
        vec![MultiplexerEngineObservation::Backpressure(summary)]
    );
    assert!(
        outcome.client_egress.is_empty(),
        "reader pressure is host-visible metadata, not terminal bytes"
    );
    assert!(
        outcome.session_events.is_empty(),
        "reader pressure is not a process/session lifecycle event"
    );
}

#[test]
fn supervised_session_can_inject_non_plain_terminal_backend_factory() {
    let writes = Rc::new(RefCell::new(Vec::new()));
    let factory_writes = Rc::clone(&writes);
    let mut runtime = ManagedSessionRuntime::with_terminal_backend_factory(
        FakeSessionRuntime::new(),
        move |size| {
            Ok::<_, std::convert::Infallible>(SpyTerminalRuntime::new(
                size,
                Rc::clone(&factory_writes),
            ))
        },
    );
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session with injected terminal backend");

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"injected backend bytes".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain runtime output through injected backend");

    assert_eq!(
        writes.borrow().as_slice(),
        [b"injected backend bytes".to_vec()]
    );

    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-injected"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot reads injected backend");

    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.data == b"injected backend bytes"
    ));
}

#[test]
fn supervised_session_backend_factory_error_surfaces_as_typed_error() {
    let mut runtime = ManagedSessionRuntime::with_terminal_backend_factory(
        FakeSessionRuntime::new(),
        |_size| -> Result<SpyTerminalRuntime, std::io::Error> {
            Err(std::io::Error::other("backend factory failed"))
        },
    );

    let error = runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect_err("spawn should fail when terminal backend construction fails");

    match error {
        ManagedSessionRuntimeError::TerminalBackendConstruction { source } => {
            assert_eq!(source.to_string(), "backend factory failed");
        }
        other => panic!("expected terminal backend construction error, got {other:?}"),
    }
    assert!(
        runtime.session(&session_id()).is_none(),
        "failed backend construction must not register the session"
    );
}

#[test]
fn supervised_session_screen_read_fails_loudly_when_terminal_backend_records_error() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            Ok::<_, std::convert::Infallible>(FailingTerminalRuntime::new(size, "formatter failed"))
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session with failing terminal backend");

    let error = runtime
        .read_screen(request_id("screen-error"), session_id(), 21)
        .expect_err("screen read should surface terminal backend error");

    match error {
        ManagedSessionRuntimeError::TerminalBackendOperation { operation, message } => {
            assert_eq!(operation, "screen_state");
            assert_eq!(message, "formatter failed");
        }
        other => panic!("expected terminal backend operation error, got {other:?}"),
    }
}

#[test]
fn supervised_session_snapshot_fails_loudly_when_terminal_backend_records_error() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            Ok::<_, std::convert::Infallible>(FailingTerminalRuntime::new(
                size,
                "snapshot export failed",
            ))
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session with failing terminal backend");

    let request_error = runtime
        .capture_snapshot(request_id("snapshot-error"), session_id(), 21)
        .expect_err("snapshot request should surface terminal backend error");
    match request_error {
        ManagedSessionRuntimeError::TerminalBackendOperation { operation, message } => {
            assert_eq!(operation, "capture_snapshot");
            assert_eq!(message, "snapshot export failed");
        }
        other => panic!("expected terminal backend operation error, got {other:?}"),
    }

    let payload_error = runtime
        .capture_snapshot_payload(&session_id())
        .expect_err("direct snapshot payload should surface terminal backend error");
    match payload_error {
        ManagedSessionRuntimeError::TerminalBackendOperation { operation, message } => {
            assert_eq!(operation, "capture_snapshot");
            assert_eq!(message, "snapshot export failed");
        }
        other => panic!("expected terminal backend operation error, got {other:?}"),
    }
}

#[test]
fn supervised_session_resize_failure_surfaces_before_runtime_resize_is_queued() {
    let fail_resize = Rc::new(Cell::new(false));
    let fail_snapshot = Rc::new(Cell::new(false));
    let factory_fail_resize = Rc::clone(&fail_resize);
    let factory_fail_snapshot = Rc::clone(&fail_snapshot);
    let mut runtime = ManagedSessionRuntime::with_terminal_backend_factory(
        FakeSessionRuntime::new(),
        move |size| {
            Ok::<_, std::convert::Infallible>(ControlledFailingTerminalRuntime::new(
                size,
                Rc::clone(&factory_fail_resize),
                Rc::clone(&factory_fail_snapshot),
            ))
        },
    );
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");
    runtime
        .handle_client_ingress(
            client_id("resize-client"),
            TransportIngress::SubscribeSession {
                client_id: client_id("resize-client"),
                session_id: session_id(),
                subscription_id: subscription_id("resize-subscription"),
            },
            10,
        )
        .expect("subscribe client");

    fail_resize.set(true);
    let error = runtime
        .handle_client_ingress(
            client_id("resize-client"),
            TransportIngress::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120,
            },
            20,
        )
        .expect_err("resize failure should surface");

    assert!(matches!(
        error,
        ManagedSessionRuntimeError::TerminalBackendOperation {
            operation: "resize",
            ref message,
        } if message == "controlled resize failure"
    ));
    assert!(
        runtime.session_runtime_mut().inputs().is_empty(),
        "failed shadow resize must not enqueue a PTY resize"
    );

    fail_resize.set(false);
    runtime
        .handle_client_ingress(
            client_id("resize-client"),
            TransportIngress::Resize {
                session_id: session_id(),
                rows: 40,
                cols: 120,
            },
            21,
        )
        .expect("successful retry should clear the backend error");
}

#[test]
fn supervised_session_failed_initial_snapshot_rolls_back_route_and_allows_fresh_retry() {
    let fail_resize = Rc::new(Cell::new(false));
    let fail_snapshot = Rc::new(Cell::new(false));
    let factory_fail_resize = Rc::clone(&fail_resize);
    let factory_fail_snapshot = Rc::clone(&fail_snapshot);
    let mut runtime = ManagedSessionRuntime::with_terminal_backend_factory(
        FakeSessionRuntime::new(),
        move |size| {
            Ok::<_, std::convert::Infallible>(ControlledFailingTerminalRuntime::new(
                size,
                Rc::clone(&factory_fail_resize),
                Rc::clone(&factory_fail_snapshot),
            ))
        },
    );
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");

    fail_snapshot.set(true);
    let error = runtime
        .handle_client_ingress(
            client_id("attach-client"),
            TransportIngress::SubscribeSession {
                client_id: client_id("attach-client"),
                session_id: session_id(),
                subscription_id: subscription_id("failed-subscription"),
            },
            20,
        )
        .expect_err("snapshot export failure should fail attach");
    assert!(matches!(
        error,
        ManagedSessionRuntimeError::TerminalBackendOperation {
            operation: "capture_snapshot",
            ref message,
        } if message == "controlled snapshot_export failure"
    ));
    assert!(matches!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(SessionLifecycleState::Running)
    ));

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"held while unattached".to_vec());
    let unattached = runtime
        .drain_runtime_once(&session_id(), 21)
        .expect("terminal session remains live after failed attach");
    assert!(unattached.client_egress.is_empty());
    assert!(unattached.session_events.iter().any(
        |event| matches!(event, SessionIoEvent::TerminalBytes { data, .. } if data == b"held while unattached")
    ));

    fail_snapshot.set(false);
    let retry = runtime
        .handle_client_ingress(
            client_id("attach-client"),
            TransportIngress::SubscribeSession {
                client_id: client_id("attach-client"),
                session_id: session_id(),
                subscription_id: subscription_id("fresh-subscription"),
            },
            22,
        )
        .expect("fresh subscription should attach after recovery");
    assert_eq!(
        retry.client_egress,
        vec![(
            client_id("attach-client"),
            TransportEgress::AttachState {
                session_id: session_id(),
                subscription_id: subscription_id("fresh-subscription"),
                state: TerminalAttachState::Attaching,
            },
        )]
    );
    let attached = runtime
        .drain_runtime_once(&session_id(), 23)
        .expect("route recovered initial snapshot");
    assert!(attached.client_egress.iter().any(|(_, frame)| matches!(
        frame,
        TransportEgress::AttachState {
            subscription_id: received_subscription_id,
            state: TerminalAttachState::Attached,
            ..
        } if received_subscription_id == &subscription_id("fresh-subscription")
    )));

    fail_snapshot.set(true);
    runtime
        .handle_client_ingress(
            client_id("attach-client"),
            TransportIngress::SubscribeSession {
                client_id: client_id("attach-client"),
                session_id: session_id(),
                subscription_id: subscription_id("failed-replacement"),
            },
            24,
        )
        .expect_err("failed replacement attach should restore the prior route");
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"still on prior route".to_vec());
    let restored = runtime
        .drain_runtime_once(&session_id(), 25)
        .expect("prior subscription remains usable");
    assert!(restored.client_egress.iter().any(|(_, frame)| matches!(
        frame,
        TransportEgress::TerminalOutput {
            subscription_id: received_subscription_id,
            data,
            ..
        } if received_subscription_id == &subscription_id("fresh-subscription")
            && data == b"still on prior route"
    )));
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
fn supervised_session_subscribe_snapshot_preserves_existing_terminal_size() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let _ = runtime
        .drain_runtime_once(&session_id(), 10)
        .expect("drain initial subscription snapshot");

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
        .expect("primary client resize");
    runtime
        .handle_client_ingress(
            client_id("client-b"),
            TransportIngress::SubscribeSession {
                client_id: client_id("client-b"),
                session_id: session_id(),
                subscription_id: subscription_id("sub-b"),
            },
            12,
        )
        .expect("late subscribe should not resize shared terminal");

    let outcome = runtime
        .drain_runtime_once(&session_id(), 13)
        .expect("drain late subscribe snapshot");

    let resize_inputs = runtime
        .session_runtime()
        .inputs()
        .iter()
        .filter(|input| matches!(input, SessionRuntimeInput::Resize { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        resize_inputs,
        vec![&SessionRuntimeInput::Resize {
            session_id: session_id(),
            size: ResizePayload {
                rows: 40,
                cols: 120
            },
        }],
        "late subscribe must snapshot current state without forcing a second resize"
    );
    assert!(matches!(
        outcome.session_events.first(),
        Some(SessionIoEvent::InitialSnapshotReady(snapshot))
            if snapshot.client_id == client_id("client-b")
                && snapshot.subscription_id == subscription_id("sub-b")
                && snapshot.rows == 40
                && snapshot.cols == 120
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
        vec![
            (
                client_id("client-a"),
                TransportEgress::AttachState {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-a"),
                    state: TerminalAttachState::Attached,
                },
            ),
            (
                client_id("client-a"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-a"),
                    data: bytes.clone(),
                },
            ),
        ]
    );
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot)) if snapshot.data == bytes
    ));
}

#[test]
fn supervised_session_default_plain_backend_bounds_retained_state_without_truncating_fanout() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let bytes = oversized_plain_bytes();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), bytes.clone());
    let output = runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain oversized runtime output");
    let snapshot = runtime
        .handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id: request_id("snapshot-bounded"),
                session_id: session_id(),
            },
            21,
        )
        .expect("snapshot reads bounded retained state");
    let screen = runtime
        .handle_session_request(
            SessionIoRequest::GetScreen {
                request_id: request_id("screen-bounded"),
                session_id: session_id(),
            },
            22,
        )
        .expect("screen reads bounded retained state");

    assert_eq!(
        output.client_egress,
        vec![
            (
                client_id("client-a"),
                TransportEgress::AttachState {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-a"),
                    state: TerminalAttachState::Attached,
                },
            ),
            (
                client_id("client-a"),
                TransportEgress::TerminalOutput {
                    session_id: session_id(),
                    subscription_id: subscription_id("sub-a"),
                    data: bytes.clone(),
                },
            ),
        ]
    );
    assert!(matches!(
        snapshot.session_events.first(),
        Some(SessionIoEvent::SnapshotReady(snapshot))
            if snapshot.data.len() == PLAIN_TERMINAL_SCREEN_MAX_BYTES
                && snapshot.data == bytes[bytes.len() - PLAIN_TERMINAL_SCREEN_MAX_BYTES..]
    ));
    assert!(matches!(
        screen.session_events.first(),
        Some(SessionIoEvent::ScreenReady(screen))
            if screen.text.len() == PLAIN_TERMINAL_SCREEN_MAX_BYTES
                && screen.text.ends_with("managed-bounded-tail")
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
    let _ = runtime
        .drain_runtime_once(&session_id(), 19)
        .expect("drain subscribe-triggered initial snapshot");

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
            if snapshot.snapshot.is_empty() && snapshot.rows == 24 && snapshot.cols == 80
    ));
    assert!(matches!(
        outcome.session_events.get(1),
        Some(SessionIoEvent::TerminalBytes { data, .. }) if data == b"live after request"
    ));
    assert!(matches!(
        outcome.client_egress.first(),
        Some((_, TransportEgress::AttachState { state, .. }))
            if state == &TerminalAttachState::Attached
    ));
    assert!(matches!(
        outcome.client_egress.get(1),
        Some((_, TransportEgress::TerminalOutput { data, .. }))
            if data == b"live after request"
    ));
}

#[test]
fn supervised_session_initial_snapshot_after_prior_output_reflects_shadow_state() {
    let mut runtime = managed_runtime();

    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"prior output".to_vec());
    runtime
        .drain_runtime_once(&session_id(), 20)
        .expect("drain prior output");
    subscribe(&mut runtime);
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
    assert!(matches!(
        outcome.client_egress.first(),
        Some((
            received_client,
            TransportEgress::Snapshot {
                subscription_id: received_subscription_id,
                data,
                ..
            }
        )) if received_client == &client_id("client-a")
            && received_subscription_id == &subscription_id("sub-a")
            && data == b"prior output"
    ));
    assert!(matches!(
        outcome.client_egress.get(1),
        Some((
            received_client,
            TransportEgress::AttachState {
                subscription_id: received_subscription_id,
                state: TerminalAttachState::Attached,
                ..
            }
        )) if received_client == &client_id("client-a")
            && received_subscription_id == &subscription_id("sub-a")
    ));
    assert!(matches!(
        outcome.client_egress.get(2),
        Some((
            received_client,
            TransportEgress::TerminalOutput {
                subscription_id: received_subscription_id,
                data,
                ..
            }
        )) if received_client == &client_id("client-a")
            && received_subscription_id == &subscription_id("sub-a")
            && data == b"held live"
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
        .expect_err("plain library harness mode flags remain explicitly unsupported");
    assert!(matches!(
        mode_error,
        ManagedSessionRuntimeError::UnsupportedSessionRequest {
            request_kind: "mode_flags",
        }
    ));

    // Plain harness has no palette authority; production Ghostty backends own
    // set_color_profile through TerminalScreenRuntime.
    let color_error = runtime
        .handle_session_request(
            SessionIoRequest::SetColorProfile {
                session_id: session_id(),
                color_profile: TerminalColorProfile::default(),
            },
            21,
        )
        .expect_err("plain library harness color profile remains explicitly unsupported");
    assert!(matches!(
        color_error,
        ManagedSessionRuntimeError::UnsupportedSessionRequest {
            request_kind: "set_color_profile",
        }
    ));
}

#[test]
fn supervised_session_mode_flags_use_authoritative_backend_and_preserve_correlation() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            Ok::<_, std::convert::Infallible>(SpyTerminalRuntime::new(
                size,
                Rc::new(RefCell::new(Vec::new())),
            ))
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");

    let output = runtime
        .handle_session_request(
            SessionIoRequest::GetModeFlags {
                request_id: request_id("mode-authoritative"),
                session_id: session_id(),
            },
            20,
        )
        .expect("read authoritative mode flags");

    assert!(matches!(
        output.session_events.as_slice(),
        [SessionIoEvent::ModeFlagsReady(response)]
            if response.request_id == request_id("mode-authoritative")
                && response.session_id == session_id()
                && response.mode_flags.mouse_mode == 9
    ));
}

#[test]
fn supervised_session_mode_flag_failure_preserves_operation_and_message() {
    let mut runtime =
        ManagedSessionRuntime::with_terminal_backend_factory(FakeSessionRuntime::new(), |size| {
            Ok::<_, std::convert::Infallible>(FailingTerminalRuntime::new(
                size,
                "forced mode query failure",
            ))
        });
    runtime
        .spawn_session(spawn_request(), CoreSessionMetadata::new())
        .expect("spawn managed session");

    let error = runtime
        .handle_session_request(
            SessionIoRequest::GetModeFlags {
                request_id: request_id("mode-failure"),
                session_id: session_id(),
            },
            20,
        )
        .expect_err("mode query failure");

    assert!(matches!(
        error,
        ManagedSessionRuntimeError::TerminalBackendOperation {
            operation: "mode_flags",
            ref message,
        } if message == "forced mode query failure"
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
fn failed_deferred_shutdown_restores_retryability_and_sends_one_fresh_retry() {
    let mut runtime = managed_runtime();
    let shutdown = SessionRuntimeInput::Shutdown {
        session_id: session_id(),
    };
    runtime.session_runtime_mut().fail_next_input(
        shutdown.clone(),
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::InputFailed,
            "forced shutdown delivery failure",
        ),
    );

    let error = runtime
        .shutdown_session(session_id(), "first shutdown", 20)
        .expect_err("unrecovered delivery failure remains typed");
    assert!(matches!(
        error,
        ManagedSessionRuntimeError::Runtime(SessionRuntimeError {
            kind: SessionRuntimeErrorKind::InputFailed,
            ref message,
        }) if message == "forced shutdown delivery failure"
    ));
    assert_eq!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Running)
    );

    let retry = runtime
        .shutdown_session(session_id(), "retry shutdown", 21)
        .expect("retry should enqueue and deliver a fresh shutdown");
    assert!(retry.observations.iter().any(|observation| {
        observation
            == &MultiplexerEngineObservation::SessionLifecycle {
                session_id: session_id(),
                state: SessionLifecycleState::Stopping,
            }
    }));
    assert_eq!(
        runtime
            .session_runtime()
            .input_attempts()
            .iter()
            .filter(|input| *input == &shutdown)
            .count(),
        2
    );
    assert_eq!(
        runtime
            .session_runtime()
            .inputs()
            .iter()
            .filter(|input| *input == &shutdown)
            .count(),
        1
    );
}

#[test]
fn failed_deferred_shutdown_leaves_natural_exit_for_the_host_drain_loop() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let shutdown = SessionRuntimeInput::Shutdown {
        session_id: session_id(),
    };
    runtime.session_runtime_mut().fail_next_input(
        shutdown.clone(),
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::InputFailed,
            "worker control route closed",
        ),
    );
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"final-natural-exit-output".to_vec());
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );

    let error = runtime
        .shutdown_session(session_id(), "natural exit cleanup", 30)
        .expect_err("managed runtime should preserve the delivery failure");
    assert!(matches!(
        error,
        ManagedSessionRuntimeError::Runtime(SessionRuntimeError {
            kind: SessionRuntimeErrorKind::InputFailed,
            ref message,
        }) if message == "worker control route closed"
    ));

    let outcome = runtime
        .drain_runtime_once(&session_id(), 30)
        .expect("host recovery drain should receive terminal evidence");

    assert_eq!(
        terminal_output_bytes_for(
            &outcome,
            &client_id("client-a"),
            &subscription_id("sub-a"),
            &session_id(),
        ),
        b"final-natural-exit-output"
    );
    assert!(outcome.session_events.iter().any(|event| {
        matches!(
            event,
            SessionIoEvent::ProcessExited {
                session_id: observed_session_id,
                payload: ProcessExitedPayload {
                    exit_code: Some(0),
                    ..
                },
            } if observed_session_id == &session_id()
        )
    }));
    assert!(!outcome.observations.iter().any(|observation| {
        matches!(
            observation,
            MultiplexerEngineObservation::SessionLifecycle {
                session_id: observed_session_id,
                state: SessionLifecycleState::Stopping,
            } if observed_session_id == &session_id()
        )
    }));
    assert_eq!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Exited { code: Some(0) })
    );
    assert_eq!(
        runtime
            .session_runtime()
            .input_attempts()
            .iter()
            .filter(|input| *input == &shutdown)
            .count(),
        1
    );
}

#[test]
fn failed_deferred_shutdown_does_not_consume_live_output_before_returning_error() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let shutdown = SessionRuntimeInput::Shutdown {
        session_id: session_id(),
    };
    runtime.session_runtime_mut().fail_next_input(
        shutdown,
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::InputFailed,
            "worker control route closed",
        ),
    );
    runtime
        .session_runtime_mut()
        .emit_output(session_id(), b"live-output-before-exit-evidence".to_vec());

    runtime
        .shutdown_session(session_id(), "failed cleanup", 30)
        .expect_err("delivery failure without exit evidence should remain an error");

    let outcome = runtime
        .drain_runtime_once(&session_id(), 31)
        .expect("live output should remain reachable after the error");
    assert_eq!(
        terminal_output_bytes_for(
            &outcome,
            &client_id("client-a"),
            &subscription_id("sub-a"),
            &session_id(),
        ),
        b"live-output-before-exit-evidence"
    );
    assert_eq!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Running)
    );
}

#[test]
fn failed_shutdown_does_not_retire_session_wake() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let source = runtime.wake_source().clone();
    let handle = source.session_handle(session_id());
    handle.notify();
    let _ = source.wait_wakes(Duration::from_millis(0));
    let shutdown = SessionRuntimeInput::Shutdown {
        session_id: session_id(),
    };
    runtime.session_runtime_mut().fail_next_input(
        shutdown,
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::InputFailed,
            "worker control route closed",
        ),
    );
    runtime
        .shutdown_session(session_id(), "failed cleanup", 30)
        .expect_err("delivery failure without exit evidence should remain an error");
    handle.notify();
    assert_eq!(
        source.occupancy(),
        1,
        "shutdown rollback must keep the live session wake handle"
    );
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert!(batch.ingress_sessions.contains(&session_id()));
}

#[test]
fn process_exit_keeps_wake_until_the_commit_owner_retires_it() {
    let mut runtime = managed_runtime();
    subscribe(&mut runtime);
    let source = runtime.wake_source().clone();
    let handle = source.session_handle(session_id());
    runtime
        .shutdown_session(session_id(), "host shutdown", 20)
        .expect("shutdown");
    assert_eq!(
        runtime
            .session(&session_id())
            .map(|session| &session.lifecycle),
        Some(&SessionLifecycleState::Stopping)
    );
    assert_eq!(source.session_registry_len(), 1);
    handle.notify();
    assert_eq!(
        source.occupancy(),
        1,
        "Stopping must keep the live session wake"
    );
    let _ = source.wait_wakes(Duration::from_millis(0));
    runtime.session_runtime_mut().emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    );
    runtime
        .drain_runtime_once(&session_id(), 21)
        .expect("route ProcessExited");
    handle.notify();
    source.notify_session(&session_id());
    assert_eq!(source.occupancy(), 1);
    assert_eq!(source.session_registry_len(), 1);
    source.forget_session(&session_id());
    assert_eq!(source.session_registry_len(), 0);
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

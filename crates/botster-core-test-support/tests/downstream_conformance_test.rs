//! Downstream-style tests for the public support crate surface.

use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::{
    CapabilityOperation, CapabilityOperationResult, CapabilityRuntimeEvent,
    CapabilityRuntimeRequest, FilesystemCapabilityRequest, FilesystemCapabilityResult,
    FilesystemOperation, ModeFlags, PluginCapabilityRuntime, PluginKey, PluginStoreBackend,
    PluginStoreKey, PluginStoreLimits, ScopedRelativePath, SessionIoEvent, TerminalColorProfile,
    TerminalOutputChunk, TerminalScreenEngine, TerminalScreenHook, TerminalScreenRuntime,
    TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
};
#[cfg(feature = "local-runtime")]
use botster_core::{
    CoreSessionMetadata, DefaultBotsterEngine, EngineCommandKind, EngineCommandOutcome,
    LocalProcessRuntime, ManagedSessionRuntime, PreparedSnapshotRequest, ResizePayload,
    SessionActivityStatus, SessionIoRequest, SessionLifecycleState,
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
    assert_command_inspection_activity, assert_command_output_fanout,
    assert_command_replay_snapshot_behavior, assert_command_screen_ready,
    assert_command_sessions_include, assert_command_snapshot_ready, assert_output_activity,
    assert_shutdown_requested, assert_terminal_output_fanout, local_shell_spawn_request,
    run_adversarial_hot_path_load, run_many_pty_load, AdversarialHotPathConfig,
    DisposableCommandLocalSession, DisposableManagedLocalSession, ManyPtyLoadConfig,
};
use botster_core_test_support::fake::{
    FakeCapabilityRuntime, FakePluginStoreBackend, FakeSessionTransport, FakeTerminalScreenRuntime,
};
use botster_core_test_support::ui_conformance::{
    assert_ui_renderer_conformance_fixture, assert_ui_renderer_conformance_fixtures,
    ui_renderer_conformance_fixtures,
};

fn session_id() -> SessionId {
    SessionId("session-consumer".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-consumer".to_string())
}

fn plugin_key(value: &str) -> PluginKey {
    PluginKey(value.to_string())
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
fn downstream_consumer_can_drive_plugin_store_fake_backend() {
    let backend = FakePluginStoreBackend::new();
    let plugin = plugin_key("consumer");
    let key = PluginStoreKey("settings".to_string());

    let record = backend
        .set(
            &plugin,
            key.clone(),
            1,
            serde_json::json!({ "enabled": true }),
            None,
            PluginStoreLimits::default(),
        )
        .expect("fake plugin-store set");

    assert_eq!(record.revision, 1);
    assert_eq!(
        backend
            .get(&plugin, &key)
            .expect("fake plugin-store get")
            .expect("record exists")
            .payload,
        serde_json::json!({ "enabled": true })
    );
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
fn downstream_consumer_can_import_ui_renderer_conformance_helpers() {
    assert_ui_renderer_conformance_fixtures();

    let fixtures = ui_renderer_conformance_fixtures();
    assert!(
        fixtures.iter().any(|fixture| fixture.name == "bindings"),
        "fixture set should include binding grammar coverage"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.name == "responsive_fallbacks"),
        "fixture set should include capability downgrade coverage"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.name == "application_dashboard"),
        "fixture set should include application dashboard coverage"
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.name == "custom_fallback"),
        "fixture set should include custom fallback coverage"
    );
}

#[test]
fn downstream_consumer_can_run_one_ui_renderer_fixture() {
    let fixture = ui_renderer_conformance_fixtures()
        .into_iter()
        .find(|fixture| fixture.name == "action_metadata")
        .expect("action metadata fixture should exist");

    assert_ui_renderer_conformance_fixture(&fixture);
    assert_eq!(fixture.action_requests.len(), 1);
    assert_eq!(fixture.action_results.len(), 1);
}

#[test]
fn downstream_consumer_can_run_custom_fallback_ui_fixture() {
    let fixture = ui_renderer_conformance_fixtures()
        .into_iter()
        .find(|fixture| fixture.name == "custom_fallback")
        .expect("custom fallback fixture should exist");

    assert_ui_renderer_conformance_fixture(&fixture);
    let custom = fixture.nodes.first().expect("custom node fixture");
    assert_eq!(custom.kind, botster_core::ui::UiNodeKind::Custom);
    assert_eq!(
        custom.custom_fallback().expect("custom fallback").kind,
        botster_core::ui::UiNodeKind::EmptyState
    );
}

#[test]
fn downstream_consumer_can_prove_capability_submit_precedes_completion() {
    let plugin = plugin_key("consumer-plugin");
    let mut runtime = FakeCapabilityRuntime::new();
    let request = CapabilityRuntimeRequest {
        plugin_key: plugin.clone(),
        operation_id: botster_core::CapabilityOperationId("fs-read-1".to_string()),
        operation: CapabilityOperation::Filesystem(FilesystemCapabilityRequest {
            scope_id: "workspace".to_string(),
            operation: FilesystemOperation::Read {
                path: ScopedRelativePath("README.md".to_string()),
            },
            limits: None,
        }),
        timeout_ms: 100,
        callback: None,
    };

    let handle = runtime.submit(request.clone()).expect("submit capability");

    assert_eq!(handle.operation_id, request.operation_id);
    assert_eq!(runtime.submitted(), &[request]);
    assert_eq!(runtime.pending_len(), 1);
    assert!(
        runtime
            .drain_events(&plugin)
            .expect("drain events before completion")
            .is_empty(),
        "submit must not complete filesystem work inline"
    );

    runtime
        .complete_next(Some(CapabilityOperationResult::Filesystem(
            FilesystemCapabilityResult::Read {
                path: ScopedRelativePath("README.md".to_string()),
                bytes: b"ok".to_vec(),
            },
        )))
        .expect("complete pending operation");

    let events = runtime
        .drain_events(&plugin)
        .expect("drain events after completion");
    assert!(matches!(
        &events[..],
        [CapabilityRuntimeEvent::Completed(completed)]
            if completed.operation_id == handle.operation_id
                && matches!(
                    &completed.result,
                    Some(CapabilityOperationResult::Filesystem(
                        FilesystemCapabilityResult::Read { .. }
                    ))
                )
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

#[cfg(all(unix, feature = "local-runtime"))]
#[test]
fn downstream_consumer_can_conform_against_default_engine_commands() {
    use std::time::Duration;

    let request = local_shell_spawn_request(
        RequestId("req-command-local".to_string()),
        SessionId("session-command-local".to_string()),
        "printf 'botster-command-local-output\\n'; read line; printf \"command-input:%s\\n\" \"$line\"; sleep 1",
    );
    let mut harness = DisposableCommandLocalSession::spawn(request, CoreSessionMetadata::new())
        .expect("spawn disposable command local session");
    let _public_engine: &DefaultBotsterEngine = harness.engine();

    harness
        .attach_client(
            client_id("client-command-a"),
            SubscriptionId("sub-command-a".to_string()),
            10,
        )
        .expect("attach first downstream client through typed command");
    harness
        .attach_client(
            client_id("client-command-b"),
            SubscriptionId("sub-command-b".to_string()),
            11,
        )
        .expect("attach second downstream client through typed command");

    let output = harness
        .drain_runtime_until_output_contains(
            b"botster-command-local-output",
            20,
            Duration::from_secs(5),
        )
        .expect("drain real PTY output through default command engine");
    let output = EngineCommandOutcome::Output(output);

    assert_command_output_fanout(
        &output,
        harness.session_id(),
        harness.attached_clients(),
        b"botster-command-local-output",
    );

    let active = harness
        .inspect_session(21, 5)
        .expect("inspect active session through typed command");
    assert_command_inspection_activity(
        &active,
        harness.session_id(),
        SessionActivityStatus::Active,
    );

    let idle = harness
        .inspect_session(200, 5)
        .expect("inspect idle session through typed command");
    assert_command_inspection_activity(&idle, harness.session_id(), SessionActivityStatus::Idle);

    let sessions = harness
        .list_sessions()
        .expect("list sessions through typed command");
    assert_command_sessions_include(&sessions, harness.session_id());

    harness
        .write_bytes(client_id("client-command-a"), b"typed\n".to_vec(), 22)
        .expect("write through typed command");
    let input_output = harness
        .drain_runtime_until_output_contains(b"command-input:typed", 23, Duration::from_secs(5))
        .expect("drain input response through default command engine");
    let input_output = EngineCommandOutcome::Output(input_output);
    assert_command_output_fanout(
        &input_output,
        harness.session_id(),
        harness.attached_clients(),
        b"command-input:typed",
    );

    let resize = harness
        .resize(client_id("client-command-a"), 33, 120, 24)
        .expect("resize through typed command");
    assert!(matches!(
        resize,
        EngineCommandOutcome::Output(ref output)
            if output.session_requests.iter().any(|(_, request)| matches!(
                request,
                SessionIoRequest::Resize {
                    session_id,
                    rows: 33,
                    cols: 120,
                } if session_id == harness.session_id()
            ))
    ));

    let screen_request_id = RequestId("req-command-screen".to_string());
    let screen = harness
        .read_screen(screen_request_id.clone(), 25)
        .expect("read screen through typed command");
    assert_command_screen_ready(&screen, &screen_request_id, harness.session_id());

    let snapshot_request_id = RequestId("req-command-snapshot".to_string());
    let snapshot = harness
        .capture_snapshot(snapshot_request_id.clone(), 26)
        .expect("capture snapshot through typed command");
    let snapshot_bytes =
        assert_command_snapshot_ready(&snapshot, &snapshot_request_id, harness.session_id());

    let replay_request_id = RequestId("req-command-replay".to_string());
    let replay = harness.replay_snapshot(
        PreparedSnapshotRequest {
            request_id: replay_request_id.clone(),
            session_id: harness.session_id().clone(),
            snapshot: snapshot_bytes,
            recovery: false,
        },
        27,
    );
    assert_command_replay_snapshot_behavior(&replay, &replay_request_id, harness.session_id());
    if let Err(botster_core_test_support::conformance::EngineConformanceError::Command(error)) =
        replay
    {
        assert_eq!(error.kind, EngineCommandKind::ReplaySnapshot);
    }

    harness
        .detach_client(
            client_id("client-command-b"),
            SubscriptionId("sub-command-b".to_string()),
            28,
        )
        .expect("detach downstream client through typed command");

    let shutdown = harness
        .shutdown("downstream command conformance complete", 29)
        .expect("shutdown through typed command");
    match shutdown {
        EngineCommandOutcome::Output(output) => {
            assert_shutdown_requested(&output, harness.session_id());
        }
        outcome => panic!("expected shutdown output, got {outcome:?}"),
    }
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

#[cfg(all(unix, feature = "local-runtime"))]
#[test]
fn many_pty_load_default() {
    let config = match std::env::var("BOTSTER_CORE_LOAD_SESSIONS") {
        Ok(value) => match value
            .parse::<usize>()
            .expect("BOTSTER_CORE_LOAD_SESSIONS must be a positive integer")
        {
            50 => ManyPtyLoadConfig::local_50(),
            count => {
                let mut config = ManyPtyLoadConfig::ci_default();
                config.session_count = count;
                config
            }
        },
        Err(_) => ManyPtyLoadConfig::ci_default(),
    };

    let report = run_many_pty_load(config).expect("run many-PTY load harness");

    assert!(
        report.session_count >= 20,
        "default many-PTY load should cover at least 20 sessions; report={report:?}"
    );
    assert_eq!(
        report.outputs_completed, report.session_count,
        "output hot path regressed; report={report:?}"
    );
    assert_eq!(
        report.exits_observed, report.session_count,
        "process-exit hot path regressed; report={report:?}"
    );
    assert!(
        report.drain_rounds >= 1,
        "round-robin drain path was not exercised; report={report:?}"
    );
    assert!(
        report.total_output_bytes > 0,
        "terminal-output hot path was not exercised; report={report:?}"
    );
    assert!(
        !report.queue_backpressure_observations.is_empty(),
        "report should name public queue/backpressure observations; report={report:?}"
    );
}

#[cfg(all(unix, feature = "local-runtime"))]
#[test]
fn many_pty_load_adversarial_noisy_reports_reader_backpressure() {
    let mut config = ManyPtyLoadConfig::ci_default().with_noisy_session(0);
    config.timeout = std::time::Duration::from_secs(35);
    config.normal_output_lines = 2;
    config.noisy_output_lines = 24_000;

    let report = run_many_pty_load(config).expect("run adversarial many-PTY load harness");

    assert!(
        report.outputs_completed >= report.session_count.saturating_sub(1),
        "quiet sessions should complete while the noisy session exercises backpressure; report={report:?}"
    );
    assert_eq!(
        report.exits_observed, report.session_count,
        "process-exit hot path regressed under noisy output; report={report:?}"
    );
    assert!(
        report.noisy_session_id.is_some(),
        "adversarial report should name the noisy session; report={report:?}"
    );
    assert!(
        report
            .queue_backpressure_observations
            .iter()
            .any(|observation| {
                observation.contains("source=session-io")
                    && observation.contains("capacity=64")
                    && observation.contains("depth=64")
            }),
        "report should include typed session-io reader pressure; report={report:?}"
    );
}

#[cfg(all(unix, feature = "local-runtime"))]
#[test]
fn adversarial_hot_path_commands_remain_bounded_under_noisy_load() {
    let report = run_adversarial_hot_path_load(AdversarialHotPathConfig::ci_default())
        .expect("run adversarial hot-path proof");

    assert_eq!(
        report.session_count, 20,
        "CI-safe hot-path proof should use the default 20-session load; report={report:?}"
    );
    assert!(
        report.noisy_output_active_during_probes,
        "hot-path probes must overlap active noisy output; report={report:?}"
    );
    assert!(
        report.quiet_sessions_completed_before_probes > 0,
        "at least one quiet session should complete before probes while noisy output is active; report={report:?}"
    );
    assert!(
        report.drain_rounds_before_probes > 0,
        "background PTY drain path was not exercised before probes; report={report:?}"
    );

    for expected_phase in [
        "list",
        "inspect",
        "attach",
        "detach",
        "resize",
        "input",
        "read-screen",
        "capture-snapshot",
        "shutdown-control",
    ] {
        assert!(
            report
                .phase_timings
                .iter()
                .any(|timing| timing.phase == expected_phase),
            "missing hot-path phase {expected_phase}; report={report:?}"
        );
    }

    assert!(
        report
            .hot_path_budget_observation
            .contains("phase_count=9 expected_phases=9")
            && report
                .hot_path_budget_observation
                .contains("fair_drain_rounds_before_probes=")
            && report
                .hot_path_budget_observation
                .contains("total_drain_rounds="),
        "report should document deterministic phase/drain budgets, not only wall-clock timings; report={report:?}"
    );
    assert!(
        report.live_sessions_after_cleanup.is_empty(),
        "cleanup should leave no live synthetic PTY sessions; report={report:?}"
    );
    assert_eq!(
        report.cleanup_exited_sessions,
        report.session_count + 1,
        "all load sessions should exit and the control session should be cleaned up; report={report:?}"
    );
    assert!(
        !report.queue_backpressure_observations.is_empty(),
        "report should name public queue/backpressure observation boundary; report={report:?}"
    );
    assert!(
        report
            .slow_client_plugin_observation
            .contains("subscription_multiplexer_engine_test")
            && report
                .slow_client_plugin_observation
                .contains("plugin_worker_engine_test"),
        "report should state focused slow-client/plugin proof boundary; report={report:?}"
    );
}

#[cfg(all(unix, feature = "local-runtime"))]
#[test]
#[ignore = "opt-in many-PTY pressure check for local hardening runs"]
fn many_pty_load_100() {
    let report = run_many_pty_load(ManyPtyLoadConfig::opt_in_100())
        .expect("run opt-in 100-session many-PTY load harness");

    assert_eq!(
        report.session_count, 100,
        "100-session opt-in path did not run the requested load; report={report:?}"
    );
    assert_eq!(
        report.outputs_completed, 100,
        "output hot path regressed under 100 sessions; report={report:?}"
    );
    assert_eq!(
        report.exits_observed, 100,
        "process-exit hot path regressed under 100 sessions; report={report:?}"
    );
}

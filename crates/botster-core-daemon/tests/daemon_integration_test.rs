#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::contract::terminal_adapter::TerminalAdapterPressure;
use botster_core::contract::terminal_wake::TerminalWakeBatch;
use botster_core::TerminalScreenSize;
use botster_core::{
    BindTerminalAdapterError, BotsterEngineObservation, ClientId, ClientStreamObservation,
    CoreSessionMetadata, EndpointId, EnvelopeCursor, EnvelopeDeliveryStatus, EnvelopeId,
    EnvelopeTarget, ModeFlags, NotificationContent, NotificationDeliveryStatus, NotificationId,
    NotificationItem, NotificationSeverity, NotificationSource, NotificationTarget,
    NotificationTimestamp, RequestId, ResizePayload, RoutedEnvelope, RoutedEnvelopeObservation,
    RoutedEnvelopePayload, RoutedEnvelopeQueueConfig, SessionId, SessionLifecycleState,
    SessionSpawnRequest, SessionWorkerHealthReason, SessionWorkerStaleReason, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, SubscriptionMultiplexerObservation, TerminalAttachState,
    TerminalCapabilitySet, TransportEgress, MAX_CORE_SESSION_METADATA_LEN,
};
use botster_core::{PtyOutputRouting, TerminalWakeKind, WorkerRouteProbe, WorkerRouteProbeEvent};
use botster_core_daemon::{
    reserved_observe_slice_error, sanitize_observe_slice_error_message,
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, CoreDaemon, CoreDaemonConfig,
    CoreDaemonError, DaemonSession, DrainNotificationsRequest, DrainRoutedEnvelopesRequest,
    GuardedWriteDecision, GuardedWriteDeliveryState, GuardedWriteRequest, LifecycleBaselineBudget,
    LifecycleBaselineStop, ObserveLifecycleBudget, ObserveLifecycleCursor, ObserveLifecyclePassId,
    ObserveLifecycleSlice, ObserveLifecycleStop, PostNotificationRequest,
    PublishRoutedEnvelopeRequest, ReadinessEvidence, RegistryRecord, RegistrySessionState,
    SafeWriteIndicator, SessionAdoptionState, SessionLifecycleBaseline, SessionLifecycleChangeKind,
    SessionLifecycleChanges, SessionLifecycleCursor, SessionLifecycleLookup, SessionLifecyclePage,
    SessionLifecyclePageError, SessionLifecycleRecord, SessionLifecycleResyncReason,
    SessionLifecycleSourceId, SessionRegistryStateLookup, SpawnSessionRequest,
    TerminalSubscriptionGeneration, OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES,
};
use botster_core_daemon::{
    DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY,
};
use botster_core_test_support::bounded_wait::{wait_for, HANG_GUARD};
use botster_core_test_support::fixture_gate::{wait_pid_exit, Fifo};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttyClientProjection, GhosttySnapshotDecodeProgress, GhosttyTerminal,
};
use botster_terminal_protocol::{
    decode_attach_state, decode_modes, mode_bits, AttachStateCode, ModesBody, TerminalFrame,
    TerminalKind,
};
use botster_terminal_protocol_client::{encode_terminal_input, TerminalInputCommand};

const EXPECTED_SNAPSHOT_FORMAT: &str = "ghostty-terminal-snapshot-v1";
const EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING: usize = 16 * 1024 * 1024;
const EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS: usize = 4_000;
const EXPECTED_GHOSTTY_DROPPED_MARKER: &str = "echo:scrollback-line-00000";
const LOW_GHOSTTY_MAX_SCROLLBACK_BYTES: usize = 1_000_000;
const REAL_WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const REAL_WORKER_COMPLETION_TIMEOUT: Duration = Duration::from_secs(180);

#[test]
fn daemon_config_defaults_to_production_ghostty_scrollback_byte_budget() {
    let config = CoreDaemonConfig::new("daemon-config-default");

    assert_eq!(
        config.ghostty_max_scrollback_bytes,
        DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES
    );
    assert_eq!(
        config.lifecycle_journal_capacity,
        DEFAULT_LIFECYCLE_JOURNAL_CAPACITY
    );
}

#[cfg(unix)]
#[test]
fn pump_commits_exited_before_return() {
    let data_dir = short_temp_data_dir("pump-commits-exited");
    let session_id = SessionId("pump-commits-exited".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived worker");

    pump_until_registry_exited(&mut daemon, &session_id, 20);

    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("exact non-progress lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_commit_visible_without_second_call() {
    let data_dir = short_temp_data_dir("pump-visible-without-second-call");
    let session_id = SessionId("pump-visible-without-second-call".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived worker");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    pump_until_registry_exited(&mut daemon, &session_id, 20);

    let baseline = daemon
        .lifecycle_baseline_page(
            None,
            None,
            LifecycleBaselineBudget {
                max_rows: 8,
                max_bytes: 64 * 1024,
                max_elapsed: Duration::MAX,
            },
        )
        .expect("paged baseline");
    assert!(baseline.sessions.iter().any(|record| {
        record.session.session_id == session_id
            && record.session.registry_state == RegistrySessionState::Exited
            && matches!(record.lifecycle, Some(SessionLifecycleState::Exited { .. }))
    }));
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("exact non-progress lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("journal page");
    assert!(page_contains_exited(&changes, &session_id));
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| {
                matches!(
                    &change.kind,
                    SessionLifecycleChangeKind::Upsert { record }
                        if record.session.session_id == session_id
                            && record.session.registry_state == RegistrySessionState::Exited
                )
            })
            .count(),
        1,
        "one exit must append one Exited journal entry"
    );
    let _ = daemon
        .drain(&session_id, 21)
        .expect("later drain must not replay lifecycle history");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("exact lookup after later drain"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    let after_drain = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("journal page after later drain");
    assert_eq!(
        after_drain
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            ))
            .count(),
        1
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn absent_session_attach_cancels_pre_attach_declaration() {
    let data_dir = short_temp_data_dir("absent-attach-cancels-declaration");
    let session_id = SessionId("absent-attach-session".to_string());
    let client_id = ClientId("absent-attach-client".to_string());
    let subscription_id = SubscriptionId("absent-attach-sub".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare adapter");

    assert!(matches!(
        daemon.attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            10,
        ),
        Err(CoreDaemonError::UnknownSession(id)) if id == session_id
    ));

    daemon
        .spawn(spawn_request(&session_id), 20)
        .expect("spawn reused session id");
    let attached = daemon
        .attach(client_id, session_id.clone(), subscription_id, 21)
        .expect("fresh unbound attach");
    assert!(
        !attached.client_egress.is_empty(),
        "the rejected declaration must not hold a later unbound bootstrap"
    );
    daemon
        .shutdown(Some(session_id.clone()), 30)
        .expect("shutdown reused session");
    assert!(daemon
        .remove_session(&session_id)
        .expect("remove reused session"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn terminal_session_attach_cancels_pre_attach_declaration() {
    let data_dir = short_temp_data_dir("terminal-attach-cancels-declaration");
    let session_id = SessionId("terminal-attach-session".to_string());
    let client_id = ClientId("terminal-attach-client".to_string());
    let subscription_id = SubscriptionId("terminal-attach-sub".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn terminal session");
    pump_until_registry_exited(&mut daemon, &session_id, 20);
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare adapter");

    assert!(matches!(
        daemon.attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            30,
        ),
        Err(CoreDaemonError::SessionNotReadable(id)) if id == session_id
    ));

    assert!(daemon
        .remove_session(&session_id)
        .expect("remove terminal session"));
    daemon
        .spawn(spawn_request(&session_id), 40)
        .expect("spawn reused session id");
    let attached = daemon
        .attach(client_id, session_id.clone(), subscription_id, 41)
        .expect("fresh unbound attach");
    assert!(
        !attached.client_egress.is_empty(),
        "the rejected declaration must not hold a later unbound bootstrap"
    );
    daemon
        .shutdown(Some(session_id.clone()), 50)
        .expect("shutdown reused session");
    assert!(daemon
        .remove_session(&session_id)
        .expect("remove reused session"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_ignore_payload_hub_shape() {
    let data_dir = temp_data_dir("pump-ignore-payload");
    let session_id = SessionId("pump-ignore-payload".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let fixture = GatedOutputExit::new();
    daemon
        .spawn(
            gated_output_exit_spawn_request(&session_id, &fixture, "PUMP-RETAINED"),
            10,
        )
        .expect("spawn output worker");
    daemon
        .attach(
            ClientId("pump-ignore-client".to_string()),
            session_id.clone(),
            SubscriptionId("pump-ignore-subscription".to_string()),
            11,
        )
        .expect("attach unbound consumer");
    let _ = fixture.release();

    pump_until_registry_exited(&mut daemon, &session_id, 20);

    let first = daemon.drain(&session_id, 30).expect("retained drain");
    let first_text = first
        .client_egress
        .iter()
        .filter_map(|(_, frame)| renderable_frame_data(frame))
        .collect::<String>();
    assert!(
        first_text.contains("PUMP-RETAINED"),
        "unmatched output must survive an ignored pump outcome: {first_text:?}"
    );
    let second = daemon.drain(&session_id, 31).expect("second drain");
    assert!(
        second
            .client_egress
            .iter()
            .filter_map(|(_, frame)| renderable_frame_data(frame))
            .all(|text| !text.contains("PUMP-RETAINED")),
        "retained output must drain exactly once"
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn mixed_session_batch_retains_output_per_session() {
    let data_dir = temp_data_dir("mixed-pump-retention");
    let session_a = SessionId("mixed-pump-a".to_string());
    let session_b = SessionId("mixed-pump-b".to_string());
    let client_a = ClientId("mixed-client-a".to_string());
    let client_b = ClientId("mixed-client-b".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let fixture_a = GatedOutputExit::new();
    let fixture_b = GatedOutputExit::new();
    for (session_id, fixture, marker) in [
        (&session_a, &fixture_a, "MIXED-A"),
        (&session_b, &fixture_b, "MIXED-B"),
    ] {
        daemon
            .spawn(
                gated_output_exit_spawn_request(session_id, fixture, marker),
                10,
            )
            .expect("spawn mixed session");
    }
    daemon
        .attach(
            client_a.clone(),
            session_a.clone(),
            SubscriptionId("mixed-sub-a".to_string()),
            11,
        )
        .expect("attach session A");
    daemon
        .attach(
            client_b.clone(),
            session_b.clone(),
            SubscriptionId("mixed-sub-b".to_string()),
            12,
        )
        .expect("attach session B");

    for fixture in [&fixture_a, &fixture_b] {
        let pid = fixture.release();
        assert!(
            wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
            "mixed fixture exit"
        );
    }
    // Both sessions have exited; gather their wakes into one mixed batch.
    let mut batch = TerminalWakeBatch::default();
    wait_for(
        "wakes from both mixed sessions",
        Duration::from_secs(1),
        |remaining| {
            // timer: deadline — wait_for's bound limits this wait
            let next = daemon.wait_wakes(remaining);
            for session in next.ingress_sessions {
                if !batch.ingress_sessions.contains(&session) {
                    batch.ingress_sessions.push(session);
                }
            }
            batch.adapter_routes.extend(next.adapter_routes);
            (batch.ingress_sessions.contains(&session_a)
                && batch.ingress_sessions.contains(&session_b))
            .then_some(())
        },
    );
    let _ = daemon.pump_woken(&batch, 20).expect("pump mixed batch");
    pump_until_registry_exited(&mut daemon, &session_a, 20);
    pump_until_registry_exited(&mut daemon, &session_b, 20);

    let first_a = daemon.drain(&session_a, 21).expect("drain session A");
    let first_b = daemon.drain(&session_b, 22).expect("drain session B");
    let text_a = terminal_output(&first_a.client_egress);
    let text_b = terminal_output(&first_b.client_egress);
    assert!(text_a.contains("MIXED-A"));
    assert!(!text_a.contains("MIXED-B"));
    assert!(text_b.contains("MIXED-B"));
    assert!(!text_b.contains("MIXED-A"));
    assert_no_duplicate_exit_output(
        &daemon.drain(&session_a, 23).expect("second drain A"),
        "MIXED-A",
    );
    assert_no_duplicate_exit_output(
        &daemon.drain(&session_b, 24).expect("second drain B"),
        "MIXED-B",
    );

    let _ = daemon.remove_session(&session_a);
    let _ = daemon.remove_session(&session_b);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn persistence_failure_rearms_and_later_commit_retires_the_wake() {
    let data_dir = temp_data_dir("pump-persistence-retry");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("pump-persistence-retry".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let fixture = GatedOutputExit::new();
    daemon
        .spawn(
            gated_output_exit_spawn_request(&session_id, &fixture, "PUMP-RETAINED"),
            10,
        )
        .expect("spawn output process");
    daemon
        .attach(
            ClientId("pump-persistence-client".to_string()),
            session_id.clone(),
            SubscriptionId("pump-persistence-sub".to_string()),
            11,
        )
        .expect("attach persistence session");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    let original_mode = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions()
        .mode();
    let mut read_only = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    read_only.set_mode(0o500);
    fs::set_permissions(&sessions_dir, read_only).expect("make sessions directory read-only");
    let probe = fs::write(sessions_dir.join("write-probe"), b"probe");
    if probe.is_ok() {
        let mut restored = fs::metadata(&sessions_dir)
            .expect("sessions directory metadata")
            .permissions();
        restored.set_mode(original_mode);
        fs::set_permissions(&sessions_dir, restored).expect("restore sessions permissions");
        panic!("read-only sessions directory accepted a probe write");
    }
    // The exit happens only now, so its commit meets the read-only directory.
    let _ = fixture.release();

    let deadline = Instant::now() + Duration::from_secs(15);
    let failure = loop {
        assert!(
            Instant::now() < deadline,
            "registry persistence did not fail"
        );
        // timer: deadline — the loop's bound on the next wake
        let batch = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
        if !batch.ingress_sessions.contains(&session_id) {
            continue;
        }
        if let Err(error) = daemon.pump_woken(&batch, 20) {
            break error;
        }
    };
    let mut restored = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(&sessions_dir, restored).expect("restore sessions permissions");

    assert!(failure.to_string().contains("Permission denied"));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    // timer: deadline — expiry fails the test
    let retry = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(retry.ingress_sessions, vec![session_id.clone()]);
    let _ = daemon
        .pump_woken(&retry, 21)
        .expect("retry persistence commit");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("persistence retry exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("persistence retry journal page");
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            ))
            .count(),
        1
    );
    let retained = daemon
        .drain(&session_id, 22)
        .expect("retained persistence output");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 23)
            .expect("second persistence drain"),
        "PUMP-RETAINED",
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_daemon_error_after_engine_output_retains_once() {
    let data_dir = temp_data_dir("shutdown-registry-error-retention");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("shutdown-registry-error-retention".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let fixture = GatedOutputExit::new();
    daemon
        .spawn(
            gated_output_exit_spawn_request(&session_id, &fixture, "PUMP-RETAINED"),
            10,
        )
        .expect("spawn shutdown registry process");
    daemon
        .attach(
            ClientId("shutdown-registry-client".to_string()),
            session_id.clone(),
            SubscriptionId("shutdown-registry-sub".to_string()),
            11,
        )
        .expect("attach shutdown registry session");
    // Shutdown must meet engine output from a process that already exited.
    let pid = fixture.release();
    assert!(
        wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "shutdown fixture exit"
    );

    let original_mode = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions()
        .mode();
    let mut read_only = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    read_only.set_mode(0o500);
    fs::set_permissions(&sessions_dir, read_only).expect("make sessions directory read-only");
    let probe = fs::write(sessions_dir.join("write-probe"), b"probe");
    if probe.is_ok() {
        let mut restored = fs::metadata(&sessions_dir)
            .expect("sessions directory metadata")
            .permissions();
        restored.set_mode(original_mode);
        fs::set_permissions(&sessions_dir, restored).expect("restore sessions permissions");
        panic!("read-only sessions directory accepted a probe write");
    }

    let error = daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect_err("shutdown registry save must fail");
    let mut restored = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(&sessions_dir, restored).expect("restore sessions permissions");
    assert!(error.to_string().contains("Permission denied"));

    let retained = daemon
        .drain(&session_id, 21)
        .expect("drain retained shutdown output");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 22)
            .expect("second shutdown registry drain"),
        "PUMP-RETAINED",
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn direct_drain_exit_commits_and_retires_once() {
    let data_dir = short_temp_data_dir("direct-drain-exit-commit");
    let session_id = SessionId("direct-drain-exit-commit".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived worker");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "direct drain did not commit exit"
        );
        daemon.drain(&session_id, 20).expect("direct drain");
        if matches!(
            daemon
                .session_registry_state(&session_id)
                .expect("direct drain exact lookup"),
            SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
        ) {
            break;
        }
        // timer: deadline — the loop's bound; the exit wakes the session
        let _ = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
    }

    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("direct drain journal page");
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            ))
            .count(),
        1
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_lifecycle_exit_commits_and_retires_once() {
    let data_dir = short_temp_data_dir("observe-exit-commit");
    let session_id = SessionId("observe-exit-commit".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived worker");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "observe did not commit exit");
        daemon.observe_lifecycle(20).expect("observe lifecycle");
        if matches!(
            daemon
                .session_registry_state(&session_id)
                .expect("observe exact lookup"),
            SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
        ) {
            break;
        }
        // timer: deadline — the loop's bound; the exit wakes the session
        let _ = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
    }

    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("observe journal page");
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            ))
            .count(),
        1
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_changes_reject_a_cursor_ahead_of_the_source() {
    let data_dir = temp_data_dir("lifecycle-cursor-ahead");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let mut ahead = daemon
        .lifecycle_baseline()
        .expect("empty lifecycle baseline")
        .cursor;
    ahead.sequence += 1;

    let changes = daemon.lifecycle_changes(&ahead);

    assert!(changes.changes.is_empty());
    assert_eq!(
        changes.resync_required,
        Some(SessionLifecycleResyncReason::CursorAhead)
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_page_validates_cursor_before_budget_and_rejects_undersized_success() {
    let data_dir = temp_data_dir("lifecycle-page-budget");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let cursor = daemon
        .lifecycle_baseline()
        .expect("empty lifecycle baseline")
        .cursor;

    match daemon.lifecycle_changes_page(&cursor, 8, 0) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => {
            assert!(minimum_bytes > 0);
            let minus_one = daemon
                .lifecycle_changes_page(&cursor, 8, minimum_bytes - 1)
                .expect_err("minimum minus one must not encode a successful page");
            assert!(matches!(
                minus_one,
                SessionLifecyclePageError::BudgetTooSmall {
                    minimum_bytes: again
                } if again == minimum_bytes
            ));
            let exact = daemon
                .lifecycle_changes_page(&cursor, 0, minimum_bytes)
                .expect("exact minimum returns the empty successful page");
            assert_successful_page_within_budget(&exact, minimum_bytes);
            assert!(exact.changes.is_empty());
            assert_eq!(exact.next, cursor);
            assert_eq!(exact.source_watermark, cursor);
        }
        other => panic!("expected BudgetTooSmall, got {other:?}"),
    }

    let mut ahead = cursor.clone();
    ahead.sequence += 1;
    let foreign = SessionLifecycleCursor {
        source_id: SessionLifecycleSourceId("foreign".to_string()),
        sequence: 0,
    };
    for (after, expected) in [
        (ahead, SessionLifecycleResyncReason::CursorAhead),
        (foreign, SessionLifecycleResyncReason::SourceChanged),
    ] {
        let page = daemon
            .lifecycle_changes_page(&after, 0, 0)
            .expect("resync must win over an undersized budget");
        assert!(page.changes.is_empty());
        assert_eq!(page.resync_required, Some(expected));
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_api_types_are_control_plane_only() {
    let api = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/api.rs"));
    let start = api
        .find("pub struct SessionLifecycleSourceId")
        .expect("lifecycle types start");
    let end = api
        .find("pub struct AttachedSession")
        .expect("lifecycle types end before attach");
    let section = &api[start..end];
    for forbidden in [
        "TransportEgress",
        "TerminalSnapshotPayload",
        "TerminalAttachState",
        "GHOSTSNP",
        "client_egress",
    ] {
        assert!(
            !section.contains(forbidden),
            "lifecycle API must stay control-plane-only; found {forbidden}"
        );
    }
    assert!(section.contains("pub struct SessionLifecyclePage"));
    assert!(section.contains("pub struct ObserveLifecycleSlice"));
    assert!(section.contains("pub struct SessionLifecycleBaselinePage"));
    assert!(section.contains("pub struct LifecycleBaselineBudget"));
    assert!(section.contains("pub enum SessionLifecycleLookup"));
    assert!(section.contains("pub enum SessionRegistryStateLookup"));
    assert!(section.contains("#[non_exhaustive]"));
    assert!(section.contains("BudgetTooSmall"));
}

#[test]
fn exact_query_methods_are_control_plane_and_work_bound() {
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/daemon.rs"));
    for method in [
        "pub fn terminal_subscription_generation",
        "pub fn session_registry_state",
    ] {
        let start = source.find(method).expect("CoreDaemon method");
        let body = method_body(source, start);
        for forbidden in [
            "list_terminal_subscriptions",
            "load_all",
            "sort",
            "observe_session",
            "append_lifecycle",
            "TransportEgress",
            "TerminalSnapshotPayload",
            "client_egress",
        ] {
            assert!(
                !body.contains(forbidden),
                "{method} must not mention {forbidden}: {body}"
            );
        }
    }
}

fn method_body(source: &str, start: usize) -> &str {
    let relative_brace = source[start..].find('{').expect("method body");
    let body_start = start + relative_brace;
    let bytes = source.as_bytes();
    let mut depth = 0_i32;
    for (offset, &byte) in bytes[body_start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..=body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced method body starting at {start}");
}

#[cfg(unix)]
#[test]
fn daemon_spawns_lists_attaches_drains_inputs_resizes_and_shuts_down() {
    let data_dir = temp_data_dir("daemon-api");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-api-session".to_string());
    let client_id = ClientId("daemon-api-client".to_string());
    let subscription_id = SubscriptionId("daemon-api-subscription".to_string());

    let session = daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn through core engine");
    assert_eq!(session.session_id, session_id);

    let listed = daemon.list().expect("registry list should load");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, session_id);
    assert_eq!(listed[0].registry_state, RegistrySessionState::Running);
    assert!(
        daemon
            .registry()
            .load(&session_id)
            .expect("load spawned record")
            .is_some(),
        "spawn should persist a non-PII registry record"
    );

    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("attach should use core subscription path");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"ping\n".to_vec(),
            12,
        )
        .expect("input should use core write path");
    daemon
        .resize(client_id, session_id.clone(), 30, 100, 13)
        .expect("resize should use core resize path");

    let drained = drain_until(&mut daemon, &session_id, "echo:ping");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:ping"),
        "input should echo through daemon-drained client egress: {output:?}"
    );

    let listed = daemon
        .list()
        .expect("registry list should load after resize");
    assert_eq!(listed[0].size.rows, 30);
    assert_eq!(listed[0].size.cols, 100);

    daemon
        .shutdown(Some(session_id.clone()), 30)
        .expect("shutdown should route through core shutdown");
    let listed = daemon
        .list()
        .expect("registry list should load after shutdown");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_late_attach_drains_initial_history_before_later_live_output() {
    let data_dir = temp_data_dir("daemon-late-attach-history");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-late-history-session".to_string());
    let primary_client = ClientId("daemon-late-history-primary".to_string());
    let primary_subscription =
        SubscriptionId("daemon-late-history-primary-subscription".to_string());
    let late_client = ClientId("daemon-late-history-late".to_string());
    let late_subscription = SubscriptionId("daemon-late-history-late-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            primary_subscription,
            11,
        )
        .expect("initial attach should subscribe through CoreDaemon");

    daemon
        .input(
            primary_client,
            session_id.clone(),
            b"before-late-attach\n".to_vec(),
            12,
        )
        .expect("prior marker should write through CoreDaemon input");
    let primary_replay_source = drain_until(&mut daemon, &session_id, "echo:before-late-attach");
    assert!(
        renderable_output(&primary_replay_source.client_egress).contains("echo:before-late-attach"),
        "fixture must prove prior marker reached core terminal state before late attach"
    );

    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            13,
        )
        .expect("late attach should return initial route output");
    daemon
        .input(
            late_client.clone(),
            session_id.clone(),
            b"after-late-attach\n".to_vec(),
            14,
        )
        .expect("later live marker should still write after late attach");

    let late_drain = drain_until_for_client(
        &mut daemon,
        &session_id,
        &late_client,
        "echo:after-late-attach",
    );
    let mut combined_egress = late_attach.client_egress;
    combined_egress.extend(late_drain.client_egress);
    {
        let (snapshot_index, snapshot) = first_snapshot_for_client(&combined_egress, &late_client)
            .expect("late Ghostty attach should deliver an opaque snapshot replay");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:before-late-attach");
        let live_index = first_terminal_output_index_for_client_containing(
            &combined_egress,
            &late_client,
            "echo:after-late-attach",
        )
        .expect("late client should receive later live output");
        assert!(
            snapshot_index < live_index,
            "late Ghostty snapshot replay should precede later live output: {:?}",
            combined_egress
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn rapid_reattach_drops_pending_egress_for_detached_subscription() {
    let data_dir = temp_data_dir("rapid-reattach-drops-stale-egress");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("rapid-reattach-session".to_string());
    let client_id = ClientId("rapid-reattach-client".to_string());
    let old_subscription = SubscriptionId("rapid-reattach-old".to_string());
    let new_subscription = SubscriptionId("rapid-reattach-new".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            old_subscription.clone(),
            11,
        )
        .expect("first attach should succeed");
    daemon
        .detach(
            client_id.clone(),
            session_id.clone(),
            old_subscription.clone(),
            12,
        )
        .expect("detach should succeed before drain");
    let replacement = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            new_subscription.clone(),
            13,
        )
        .expect("replacement attach should succeed");

    let drained = daemon.drain(&session_id, 14).expect("drain attach egress");
    assert!(replacement
        .client_egress
        .iter()
        .all(|(received_client, frame)| {
            received_client != &client_id
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &old_subscription
                )
        }));
    assert!(drained
        .client_egress
        .iter()
        .all(|(received_client, frame)| {
            received_client != &client_id
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &new_subscription
                )
        }));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn guarded_write_states_are_fail_closed_and_write_through_input_path() {
    let data_dir = temp_data_dir("daemon-guarded");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-guarded-session".to_string());
    let client_id = ClientId("daemon-guarded-client".to_string());
    let subscription_id = SubscriptionId("daemon-guarded-subscription".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("daemon should attach");

    let mode_flags = ModeFlags {
        cursor_visible: true,
        ..ModeFlags::default()
    };
    let ready = daemon
        .guarded_write(GuardedWriteRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
            data: b"guarded\n".to_vec(),
            readiness: ReadinessEvidence::ready(mode_flags),
            now_seconds: 12,
        })
        .expect("ready guarded write should run");
    assert!(matches!(ready.decision, GuardedWriteDecision::Write));
    assert_eq!(
        ready.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Written
        ],
        "plain PTY injection must not fabricate delivered or acknowledged"
    );

    let drained = drain_until(&mut daemon, &session_id, "echo:guarded");
    assert!(terminal_output(&drained.client_egress).contains("echo:guarded"));

    let deferred = daemon
        .guarded_write(GuardedWriteRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
            data: b"deferred\n".to_vec(),
            readiness: ReadinessEvidence::default(),
            now_seconds: 13,
        })
        .expect("absent evidence should defer");
    assert!(matches!(
        deferred.decision,
        GuardedWriteDecision::Defer { .. }
    ));
    assert_eq!(
        deferred.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Deferred
        ]
    );

    let rejected = daemon
        .guarded_write(GuardedWriteRequest {
            session_id,
            client_id,
            data: b"rejected\n".to_vec(),
            readiness: ReadinessEvidence {
                safe_write: SafeWriteIndicator::Unsafe,
                ..ReadinessEvidence::default()
            },
            now_seconds: 14,
        })
        .expect("unsafe evidence should reject");
    assert!(matches!(
        rejected.decision,
        GuardedWriteDecision::Reject { .. }
    ));
    assert_eq!(
        rejected.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Rejected
        ]
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_posts_drains_and_acknowledges_notifications() {
    let data_dir = temp_data_dir("daemon-notification-ack");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let id = NotificationId("daemon-notification-1".to_string());
    let target = notification_session_target("daemon-notification-session");

    let posted = daemon
        .post_notification(PostNotificationRequest {
            item: notification("daemon-notification-1", target.clone(), 10),
        })
        .expect("daemon should queue notification through CoreDaemon");
    assert_eq!(posted.id, id);
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Queued)
    );

    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("daemon should drain notification target");
    assert_eq!(drained.items.len(), 1);
    assert_eq!(drained.items[0].id, id);
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Delivered)
    );

    let acknowledged = daemon
        .acknowledge_notification(AcknowledgeNotificationRequest { id: id.clone() })
        .expect("daemon should acknowledge notification");
    assert_eq!(
        acknowledged.status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_drain_is_target_scoped_and_once_only() {
    let data_dir = temp_data_dir("daemon-notification-target-scope");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_target = notification_session_target("target-scope-session");
    let client_target = notification_client_target("target-scope-client");

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("session-notification", session_target.clone(), 10),
        })
        .expect("session notification should queue");
    daemon
        .post_notification(PostNotificationRequest {
            item: notification("client-notification", client_target.clone(), 10),
        })
        .expect("client notification should queue");

    let session_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: session_target.clone(),
            now: NotificationTimestamp(12),
        })
        .expect("session target should drain");
    let second_session_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: session_target,
            now: NotificationTimestamp(12),
        })
        .expect("session target second drain should run");
    let client_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: client_target,
            now: NotificationTimestamp(12),
        })
        .expect("client target should drain independently");

    assert_eq!(
        session_drain
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["session-notification"]
    );
    assert!(
        second_session_drain.items.is_empty(),
        "notification drains are one-shot per target"
    );
    assert_eq!(
        client_drain
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["client-notification"]
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_expiry_matches_core_inbox() {
    let data_dir = temp_data_dir("daemon-notification-expiry");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let target = notification_session_target("expiry-session");
    let expired_id = NotificationId("expired-notification".to_string());
    let live_id = NotificationId("live-notification".to_string());

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("expired-notification", target.clone(), 10)
                .with_expiry(NotificationTimestamp(20)),
        })
        .expect("expired fixture should queue");
    daemon
        .post_notification(PostNotificationRequest {
            item: notification("live-notification", target.clone(), 10)
                .with_expiry(NotificationTimestamp(40)),
        })
        .expect("live fixture should queue");

    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(30),
        })
        .expect("expiry drain should run");

    assert_eq!(
        drained
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["live-notification"]
    );
    assert_eq!(
        daemon.notification_status(&expired_id).status,
        Some(NotificationDeliveryStatus::Expired)
    );
    assert_eq!(
        daemon.notification_status(&live_id).status,
        Some(NotificationDeliveryStatus::Delivered)
    );

    let _ = fs::remove_dir_all(data_dir);
}

fn session_target(session_id: &SessionId) -> EnvelopeTarget {
    EnvelopeTarget::Session {
        session_id: session_id.clone(),
    }
}

fn subscription_target(session_id: &SessionId, subscription_id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Subscription {
        session_id: session_id.clone(),
        subscription_id: SubscriptionId(subscription_id.to_string()),
    }
}

/// Publish one envelope to `targets`, then drain each once so every copy is
/// both queued and delivered-but-unacknowledged.
fn publish_and_deliver(daemon: &mut CoreDaemon, id: &str, targets: &[EnvelopeTarget]) {
    daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope(id, targets.to_vec()),
        })
        .expect("publish to the session targets");
    for target in targets {
        let drained = daemon
            .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
                target: target.clone(),
                after: None,
                limit: 8,
            })
            .expect("drain one target");
        assert_eq!(drained.envelopes.len(), 1, "{target:?} holds the copy");
    }
}

fn holds_nothing(daemon: &mut CoreDaemon, target: &EnvelopeTarget, id: &str) -> bool {
    let drained = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: target.clone(),
            after: None,
            limit: 8,
        })
        .expect("drain a forgotten target");
    drained.envelopes.is_empty()
        && daemon
            .routed_envelope_delivery_state(target, &EnvelopeId(id.to_string()))
            .state
            .is_none()
}

/// Removing a session forgets every envelope target of it, the session
/// target and each subscription on it, and leaves other sessions' targets.
#[test]
fn removing_a_session_forgets_its_session_and_subscription_envelope_targets() {
    let data_dir = temp_data_dir("remove-session-envelope-targets");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let gone = SessionId("gone-envelope-session".to_string());
    let other = SessionId("other-envelope-session".to_string());
    for session_id in [&gone, &other] {
        let mut record = RegistryRecord::running(
            session_id.clone(),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "seed".to_string(),
            1,
        );
        record.mark(RegistrySessionState::Exited, 1);
        daemon.registry().save(&record).expect("seed exited record");
    }
    let gone_targets = [
        session_target(&gone),
        subscription_target(&gone, "a"),
        subscription_target(&gone, "b"),
    ];
    let kept = subscription_target(&other, "a");
    let mut targets = gone_targets.to_vec();
    targets.push(kept.clone());
    publish_and_deliver(&mut daemon, "env-remove", &targets);

    assert!(daemon
        .remove_session(&gone)
        .expect("remove the exited session"));

    for target in &gone_targets {
        assert!(
            holds_nothing(&mut daemon, target, "env-remove"),
            "{target:?} still holds envelope state after its session was removed"
        );
    }
    assert!(
        daemon
            .routed_envelope_delivery_state(&kept, &EnvelopeId("env-remove".to_string()))
            .state
            .is_some(),
        "another session's target keeps its delivery"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// The same cleanup is public, for a host that retires a session's targets
/// before the session is removed.
#[test]
fn forget_session_envelope_targets_forgets_only_that_sessions_targets() {
    let data_dir = temp_data_dir("forget-session-envelope-targets");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let ended = SessionId("ended-envelope-session".to_string());
    let other = SessionId("live-envelope-session".to_string());
    let ended_targets = [session_target(&ended), subscription_target(&ended, "a")];
    let kept = [session_target(&other), subscription_target(&other, "a")];
    let mut targets = ended_targets.to_vec();
    targets.extend(kept.iter().cloned());
    publish_and_deliver(&mut daemon, "env-forget", &targets);

    daemon
        .forget_session_envelope_targets(&ended)
        .expect("forget the ended session's targets");

    for target in &ended_targets {
        assert!(
            holds_nothing(&mut daemon, target, "env-forget"),
            "{target:?}"
        );
    }
    for target in &kept {
        assert!(
            daemon
                .routed_envelope_delivery_state(target, &EnvelopeId("env-forget".to_string()))
                .state
                .is_some(),
            "{target:?} keeps its delivery"
        );
    }
    let _ = fs::remove_dir_all(data_dir);
}

/// session_metadata reads one registry row and changes nothing: an unknown
/// id is None, and the lifecycle journal gains no change.
#[test]
fn session_metadata_reads_the_registry_row_without_side_effects() {
    let data_dir = temp_data_dir("session-metadata-read");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let known = SessionId("metadata-session".to_string());
    let metadata = CoreSessionMetadata::from_entries(
        [
            ("session_type".to_string(), "agent".to_string()),
            ("token_digest".to_string(), "sha256:abc".to_string()),
        ]
        .into_iter()
        .collect(),
    );
    let mut record = RegistryRecord::running(
        known.clone(),
        None,
        ResizePayload { rows: 24, cols: 80 },
        "seed".to_string(),
        1,
    );
    record.metadata = metadata.clone();
    daemon
        .registry()
        .save(&record)
        .expect("seed the registry row");
    let cursor = daemon.lifecycle_baseline().expect("baseline").cursor;

    assert_eq!(
        daemon.session_metadata(&known).expect("read metadata"),
        Some(metadata)
    );
    assert_eq!(
        daemon
            .session_metadata(&SessionId("never-seen".to_string()))
            .expect("read an unknown id"),
        None
    );

    let after = daemon.lifecycle_changes(&cursor);
    assert!(after.resync_required.is_none());
    assert!(
        after.changes.is_empty(),
        "the read appended no lifecycle change"
    );
    assert_eq!(after.cursor, cursor, "the journal did not advance");
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_routed_envelope_cursor_ack_and_backpressure_are_exposed_when_needed() {
    let data_dir = temp_data_dir("daemon-routed-envelope");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_routed_envelope_queue(RoutedEnvelopeQueueConfig::new(1)),
    );
    let slow = envelope_endpoint("slow");
    let fast = envelope_endpoint("fast");

    let first = daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope("env-1", vec![slow.clone(), fast.clone()]),
        })
        .expect("first envelope should publish");
    assert_eq!(first.deliveries.len(), 2);

    let fast_first = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: fast.clone(),
            after: None,
            limit: 1,
        })
        .expect("fast target should drain first envelope");
    assert_eq!(fast_first.envelopes[0].id, EnvelopeId("env-1".to_string()));
    assert_eq!(fast_first.next_cursor, Some(EnvelopeCursor(2)));
    // At-least-once: a drained envelope holds its slot until the ack.
    daemon
        .acknowledge_routed_envelope(AcknowledgeRoutedEnvelopeRequest {
            target: fast.clone(),
            envelope_id: EnvelopeId("env-1".to_string()),
        })
        .expect("fast target acks the first envelope");

    let second = daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope("env-2", vec![slow.clone(), fast.clone()]),
        })
        .expect("second envelope should publish with slow pressure");
    assert!(second.observations.iter().any(|observation| {
        matches!(
            observation,
            RoutedEnvelopeObservation::Backpressured {
                envelope_id,
                target,
                capacity: 1,
                depth: 1
            } if envelope_id == &EnvelopeId("env-2".to_string()) && target == &slow
        )
    }));

    let fast_second = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: fast.clone(),
            after: fast_first.next_cursor,
            limit: 1,
        })
        .expect("cursor drain should deliver second fast envelope");
    assert_eq!(
        fast_second
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-2"]
    );

    let acknowledged = daemon
        .acknowledge_routed_envelope(AcknowledgeRoutedEnvelopeRequest {
            target: fast.clone(),
            envelope_id: EnvelopeId("env-2".to_string()),
        })
        .expect("fast envelope should acknowledge");
    assert_eq!(
        acknowledged
            .state
            .expect("delivery state should exist")
            .status,
        EnvelopeDeliveryStatus::Acknowledged
    );
    // A backpressured copy is reported by publish only; nothing of it is
    // kept, and the acknowledged copy's record is gone with its ack.
    assert!(daemon
        .routed_envelope_delivery_state(&slow, &EnvelopeId("env-2".to_string()))
        .state
        .is_none());
    assert!(daemon
        .routed_envelope_delivery_state(&fast, &EnvelopeId("env-2".to_string()))
        .state
        .is_none());

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notifications_work_for_worker_backed_daemon_without_worker_engine_notification_methods() {
    let data_dir = temp_data_dir("daemon-worker-notification");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let id = NotificationId("worker-notification".to_string());
    let target = notification_session_target("worker-notification-session");

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("worker-notification", target.clone(), 10),
        })
        .expect("worker-backed daemon should queue notification");
    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("worker-backed daemon should drain notification");
    let acknowledged = daemon
        .acknowledge_notification(AcknowledgeNotificationRequest { id: id.clone() })
        .expect("worker-backed daemon should acknowledge notification");

    assert_eq!(drained.items.len(), 1);
    assert_eq!(drained.items[0].id, id);
    assert_eq!(
        acknowledged.status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_blank_history_is_ready_finish_attached() {
    let data_dir = temp_data_dir("worker-incremental-blank");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-incremental-blank-session".to_string());
    let client_id = ClientId("worker-incremental-blank-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-blank-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn blank worker");
    let initial = daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("start blank attach");
    let drained = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut frames = initial.client_egress;
    frames.extend(drained.client_egress);

    let mut projection =
        GhosttyClientProjection::new(TerminalScreenSize::new(24, 80)).expect("blank client");
    let mut sequence = Vec::new();
    for (target, frame) in frames {
        if target != client_id {
            continue;
        }
        match frame {
            TransportEgress::AttachState {
                state: TerminalAttachState::Attaching,
                ..
            } => sequence.push("attaching"),
            TransportEgress::Snapshot { data, .. } if sequence == ["attaching"] => {
                assert_eq!(
                    projection
                        .install_ghostsnp_ready(&data[..])
                        .expect("blank READY"),
                    GhosttySnapshotDecodeProgress::Ready
                );
                sequence.push("ready");
            }
            TransportEgress::Snapshot { data, .. } => {
                assert_eq!(
                    projection
                        .apply_ghostsnp_history(&data[..])
                        .expect("blank FINISH"),
                    GhosttySnapshotDecodeProgress::Finish
                );
                sequence.push("finish");
            }
            TransportEgress::AttachState {
                state: TerminalAttachState::Attached,
                ..
            } => sequence.push("attached"),
            _ => {}
        }
    }
    assert_eq!(sequence, ["attaching", "ready", "finish", "attached"]);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_bound_adapter_receives_ready_finish_without_drain_snapshots() {
    let data_dir = temp_data_dir("worker-bound-adapter");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-bound-adapter-session".to_string());
    let client_id = ClientId("worker-bound-adapter-client".to_string());
    let subscription_id = SubscriptionId("worker-bound-adapter-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string();
    daemon.spawn(request, 10).expect("spawn bound worker");
    let initial = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("start bound attach");
    assert!(matches!(
        &initial.client_egress[0],
        (_, TransportEgress::AttachState { .. })
    ));
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after attach")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::from_tokens(["snapshot_delivery=ready_then_history"])
                .expect("advertised optional token"),
            Box::new(adapter.clone()),
        )
        .expect("bind worker adapter");

    let started = Instant::now();
    let mut phases = Vec::new();
    let mut sent_live_input = false;
    let mut saw_live = false;
    let mut live_output = Vec::new();
    let mut seen_frames = 0;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        let batch =
            // timer: deadline — the loop's bound on the next wake
            daemon.wait_wakes(REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()));
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            let _ = daemon
                .pump_woken(&batch, 20)
                .expect("pump bound worker wake");
        }
        let delivered = adapter.snapshot_delivered_frame_bytes();
        for bytes in &delivered[seen_frames..] {
            let frame = adapter_terminal_frame(bytes);
            match frame.kind() {
                TerminalKind::SnapshotReady => {
                    assert!(
                        frame.body().starts_with(b"GHOSTSNP"),
                        "READY carries the GHOSTSNP prefix"
                    );
                    if !phases.iter().any(|seen| seen == "ready") {
                        phases.push("ready".to_string());
                    }
                }
                TerminalKind::SnapshotHistory => {
                    if !phases.iter().any(|seen| seen == "history") {
                        phases.push("history".to_string());
                    }
                }
                TerminalKind::SnapshotFinish => {
                    assert!(frame.body().is_empty(), "FINISH carries no body");
                    if !phases.iter().any(|seen| seen == "finish") {
                        phases.push("finish".to_string());
                    }
                }
                TerminalKind::AttachState => {
                    if decode_attach_state(&frame).expect("attach state body")
                        == AttachStateCode::Attached
                        && !phases.iter().any(|seen| seen == "attached")
                    {
                        phases.push("attached".to_string());
                    }
                }
                TerminalKind::Output if sent_live_input => {
                    live_output.extend_from_slice(frame.body());
                    saw_live = live_output
                        .windows(b"echo:BOUND-LIVE".len())
                        .any(|window| window == b"echo:BOUND-LIVE");
                }
                _ => {}
            }
        }
        seen_frames = delivered.len();
        if phases.iter().any(|phase| phase == "attached") && !sent_live_input {
            daemon
                .input(
                    client_id.clone(),
                    session_id.clone(),
                    b"BOUND-LIVE\n".to_vec(),
                    21,
                )
                .expect("post-attach live input");
            sent_live_input = true;
        }
        let ready = phases.iter().position(|phase| phase == "ready");
        let finish = phases.iter().position(|phase| phase == "finish");
        if ready
            .zip(finish)
            .is_some_and(|(ready, finish)| ready < finish)
            && saw_live
        {
            break;
        }
    }
    let ready = phases.iter().position(|phase| phase == "ready");
    let finish = phases.iter().position(|phase| phase == "finish");
    assert!(
        ready
            .zip(finish)
            .is_some_and(|(ready, finish)| ready < finish),
        "bound adapter must receive READY before FINISH: {phases:?}"
    );
    assert!(
        saw_live,
        "bound adapter must receive the live echo `echo:BOUND-LIVE`; live output so far: {:?}",
        String::from_utf8_lossy(&live_output)
    );
    assert!(daemon
        .list()
        .expect("list")
        .iter()
        .any(|row| row.session_id == session_id));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn bound_adapter_keeps_live_bytes_across_repeated_process_exited_rounds() {
    // Never written: each round's worker holds after its exit frame until
    // the reaper ends it.
    let exit_hold = Fifo::new("bound-exit-rounds-hold");
    let data_dir = temp_data_dir("bound-exit-rounds");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0)
            .with_test_hold_before_exit_gate(Some(exit_hold.path().to_path_buf())),
    );

    for round in 0..3 {
        let session_id = SessionId(format!("bound-exit-round-{round}"));
        let client_id = ClientId(format!("bound-exit-client-{round}"));
        let subscription_id = SubscriptionId(format!("bound-exit-sub-{round}"));
        let mut request = spawn_request(&session_id);
        request.request.arguments[1] =
            "printf ready; while IFS= read -r line; do printf LIVE; exit 0; done".to_string();
        daemon
            .spawn(request, 10 + round)
            .unwrap_or_else(|error| panic!("round {round} spawn: {error:?}"));
        let (_worker_pid, pty_child_pid, _) = worker_process_evidence(&daemon, &session_id);
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                11 + round,
            )
            .unwrap_or_else(|error| panic!("round {round} attach: {error:?}"));
        let _ = drain_until_attached(&mut daemon, &session_id, &client_id);
        let generation = daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .into_iter()
            .find(|row| row.subscription_id == subscription_id)
            .unwrap_or_else(|| panic!("round {round} inventory"))
            .generation;
        let adapter = SharedFakeTerminalAdapter::new();
        daemon
            .bind_waking_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(adapter.clone()),
            )
            .unwrap_or_else(|error| panic!("round {round} bind: {error:?}"));

        daemon
            .input(
                client_id.clone(),
                session_id.clone(),
                b"go\n".to_vec(),
                12 + round,
            )
            .unwrap_or_else(|error| panic!("round {round} release: {error:?}"));
        assert!(
            wait_pid_exit(pty_child_pid, REAL_WORKER_COMPLETION_TIMEOUT),
            "round {round} PTY child exit"
        );
        let mut saw_live = false;
        let mut now = 13 + round;
        wait_for(
            "LIVE and process_exit at the one-slot adapter",
            // The former 80 wake rounds of 250 ms each.
            Duration::from_secs(20),
            |remaining| {
                complete_one_slot_and_wake(&adapter);
                saw_live |= adapter_has_live(&adapter);
                if saw_live && adapter_has_process_exit(&adapter) {
                    return Some(());
                }
                // timer: deadline — wait_for's bound limits this wait
                let batch = daemon.wait_wakes(remaining);
                if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                    now += 1;
                    let _ = daemon
                        .pump_woken(&batch, now)
                        .unwrap_or_else(|error| panic!("round {round} pump: {error:?}"));
                }
                None
            },
        );
        let delivered = adapter.snapshot_delivered_frame_bytes();
        let types: Vec<String> = delivered
            .iter()
            .map(|bytes| adapter_frame_type(bytes))
            .collect();
        let payloads: Vec<String> = delivered
            .iter()
            .map(|bytes| adapter_payload_text(bytes))
            .collect();
        assert!(
            saw_live,
            "round {round}: LIVE bytes must reach the one-slot adapter before close: types={types:?} payloads={payloads:?}"
        );
        assert!(
            types.iter().any(|kind| kind == "process_exit") || adapter_has_process_exit(&adapter),
            "round {round}: process_exit must reach the adapter before close: {types:?}"
        );
        if let (Some(live_at), Some(exit_at)) = (
            delivered.iter().position(|bytes| {
                adapter_frame_type(bytes) == "terminal_output"
                    && adapter_payload_text(bytes).contains("LIVE")
            }),
            types.iter().position(|kind| kind == "process_exit"),
        ) {
            assert!(
                live_at < exit_at,
                "round {round}: LIVE must precede process_exit: {types:?}"
            );
        }

        daemon
            .shutdown(Some(session_id), 15 + round)
            .unwrap_or_else(|error| panic!("round {round} shutdown: {error:?}"));
        assert_eq!(
            adapter.snapshot_pressure(),
            TerminalAdapterPressure::Closed,
            "round {round}: shutdown teardown must close after the flush window"
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn bound_adapter_receives_live_bytes_when_process_exits_during_incremental_attach() {
    // The former tick loop's bound: 400 passes of a 250 ms wake wait.
    const TICK_LOOP_BOUND: Duration = Duration::from_secs(100);
    let data_dir = temp_data_dir("bound-exit-during-attach");
    let (probe, probe_events) = WorkerRouteProbe::channel();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1))
            .with_test_route_probe(Some(probe)),
    );
    let session_id = SessionId("bound-exit-during-attach".to_string());
    let client_id = ClientId("bound-exit-during-attach-client".to_string());
    let subscription_id = SubscriptionId("bound-exit-during-attach-sub".to_string());
    // The child signals the test and waits for its release on named pipes,
    // opened by external commands, which a signal the shell handles cannot
    // interrupt. It prints LIVE and exits the moment the test releases it.
    let ready = Fifo::new("history-ready");
    let release = Fifo::new("live-release");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'PRE-BARRIER-MARKER'; /bin/echo ready > '{}'; ",
            "/bin/cat '{}' >/dev/null; ",
            // A post-fence burst the one-slot parent queue cannot hold,
            // then LIVE.
            "/usr/bin/head -c 262144 /dev/zero | /usr/bin/tr '\\0' f; ",
            "printf LIVE; exit 0"
        ),
        ready.path().display(),
        release.path().display()
    );

    daemon.spawn(request, 10).expect("spawn history then wait");
    let _ = ready.read_signal(Duration::from_secs(10));
    drain_pre_attach_producer_output(&mut daemon, &session_id, 11);
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start incremental attach");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after attach")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
    daemon
        .bind_waking_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind during incremental attach");

    wait_for(
        "the incremental attach's first wake",
        Duration::from_secs(5),
        |remaining| {
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
                return None;
            }
            let _ = daemon
                .pump_woken(&batch, 13)
                .expect("pace unfinished attach through a wake");
            Some(())
        },
    );
    assert!(
        !adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| { adapter_phase(bytes) == Some("attached") }),
        "bind must happen before incremental attach finishes"
    );

    // Pump until Core queues the capture's barrier release, and stop there:
    // the capture stays registered, and the worker's release reply fills the
    // one-event parent queue.
    let release_sent = |events: &mpsc::Receiver<WorkerRouteProbeEvent>| {
        events.try_iter().any(|event| {
            matches!(
                event,
                WorkerRouteProbeEvent::CaptureReleaseSent { session_id: ref released }
                    if *released == session_id
            )
        })
    };
    let mut now = 14;
    wait_for(
        "the capture's barrier release",
        TICK_LOOP_BOUND,
        |remaining| {
            if release_sent(&probe_events) {
                return Some(());
            }
            complete_one_slot_and_wake(&adapter);
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump incremental attach wake");
            }
            release_sent(&probe_events).then_some(())
        },
    );
    // Only now does the child print LIVE: after the fence, and while Core
    // does not pump. The child's post-fence burst fills the one-slot parent
    // queue, whatever that pump already took, and the next chunk meets it
    // full with the capture still registered: the bound route is a
    // consumer, so the reader stalls. Without it, the reader drops the chunk.
    release.release(Duration::from_secs(10));
    let routing_deadline = Instant::now() + TICK_LOOP_BOUND;
    let full_queue_routing = loop {
        match probe_events
            // timer: deadline — the full-queue routing decision must arrive; expiry fails the test
            .recv_timeout(routing_deadline.saturating_duration_since(Instant::now()))
            .expect("the parent reader routes the post-fence burst")
        {
            WorkerRouteProbeEvent::PtyOutputRouted {
                session_id: ref routed,
                routing,
            } if *routed == session_id && routing != PtyOutputRouting::Enqueued => break routing,
            _ => {}
        }
    };
    assert_eq!(
        full_queue_routing,
        PtyOutputRouting::Stalled,
        "post-fence output meeting a full queue must stall the reader, not be dropped"
    );

    wait_for(
        "process_exit at the one-slot adapter",
        TICK_LOOP_BOUND,
        |remaining| {
            complete_one_slot_and_wake(&adapter);
            if adapter_has_process_exit(&adapter) {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump incremental attach wake");
            }
            None
        },
    );
    let frames = adapter.snapshot_delivered_frame_bytes();
    let described: Vec<String> = frames
        .iter()
        .map(|bytes| {
            let frame = adapter_terminal_frame(bytes);
            format!(
                "{:?}:{}:live={}",
                frame.kind(),
                frame.body().len(),
                frame.body().windows(4).any(|window| window == b"LIVE")
            )
        })
        .collect();
    // Bytes before the capture fence are in the snapshot only; bytes after
    // it are output after FINISH. LIVE is in exactly one of the two.
    let snapshot_live = frames.iter().any(|bytes| {
        let frame = adapter_terminal_frame(bytes);
        matches!(
            frame.kind(),
            TerminalKind::SnapshotReady | TerminalKind::SnapshotHistory
        ) && frame.body().windows(4).any(|window| window == b"LIVE")
    });
    let output_text: String = frames
        .iter()
        .filter(|bytes| adapter_frame_type(bytes) == "terminal_output")
        .map(|bytes| adapter_payload_text(bytes))
        .collect();
    let output_live = output_text.matches("LIVE").count();
    assert_eq!(
        usize::from(snapshot_live) + output_live,
        1,
        "LIVE must reach the route exactly once, in the snapshot or as output: {described:?}"
    );
    assert!(
        !output_text.contains("history-") && !output_text.contains("PRE-BARRIER-MARKER"),
        "pre-fence bytes must not be repeated as output: {output_text:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

/// Pump a bound route on wakes until its adapter closes, completing each
/// adapter write as a transport does.
fn pump_until_route_closed(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    first_now: u64,
) {
    let mut now = first_now;
    wait_for(
        "the route's close",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            complete_one_slot_and_wake(adapter);
            if adapter.close_reason().is_some() {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon.pump_woken(&batch, now).expect("pump the route");
            }
            None
        },
    );
}

/// The route stream of an attach during or after an exit: ATTACHED, one
/// snapshot (READY, history, FINISH), then output, then PROCESS_EXIT last.
/// Returns the snapshot bytes and the output text.
fn assert_snapshot_then_exit(frames: &[Vec<u8>]) -> (Vec<u8>, String) {
    let decoded: Vec<TerminalFrame> = frames
        .iter()
        .map(|bytes| adapter_terminal_frame(bytes))
        .collect();
    let kinds: Vec<TerminalKind> = decoded.iter().map(TerminalFrame::kind).collect();
    let position = |kind: TerminalKind| kinds.iter().position(|seen| *seen == kind);
    let attached = frames
        .iter()
        .position(|bytes| adapter_phase(bytes) == Some("attached"))
        .unwrap_or_else(|| panic!("the route must be ATTACHED: {kinds:?}"));
    let ready = position(TerminalKind::SnapshotReady)
        .unwrap_or_else(|| panic!("the route must receive SNAPSHOT_READY: {kinds:?}"));
    let finish = position(TerminalKind::SnapshotFinish)
        .unwrap_or_else(|| panic!("the route must receive SNAPSHOT_FINISH: {kinds:?}"));
    let exits: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == TerminalKind::ProcessExit)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(exits.len(), 1, "exactly one PROCESS_EXIT: {kinds:?}");
    assert_eq!(
        exits[0],
        kinds.len() - 1,
        "PROCESS_EXIT must be last: {kinds:?}"
    );
    assert!(
        attached < ready && ready < finish,
        "ATTACHED, READY, FINISH in order: {kinds:?}"
    );
    let mut snapshot = Vec::new();
    for frame in &decoded[ready..=finish] {
        snapshot.extend_from_slice(frame.body());
    }
    let output: String = decoded[finish..]
        .iter()
        .filter(|frame| frame.kind() == TerminalKind::Output)
        .map(|frame| String::from_utf8_lossy(frame.body()).into_owned())
        .collect();
    (snapshot, output)
}

/// A route attached while its child exits: the capture is in flight when
/// the exit lands, and the client is slow while the child writes and exits.
/// The route receives its whole snapshot, the output behind its fence, and
/// PROCESS_EXIT last; LIVE arrives exactly once.
#[cfg(unix)]
#[test]
fn a_route_attached_while_its_child_exits_receives_its_snapshot_then_the_exit() {
    let data_dir = temp_data_dir("attach-during-exit");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1)),
    );
    let session_id = SessionId("attach-during-exit".to_string());
    let client_id = ClientId("attach-during-exit-client".to_string());
    let subscription_id = SubscriptionId("attach-during-exit-sub".to_string());
    let ready = Fifo::new("exit-attach-ready");
    let release = Fifo::new("exit-attach-release");
    let printed = Fifo::new("exit-attach-printed");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "/bin/echo ready > '{}'; /bin/cat '{}' >/dev/null; ",
            "printf LIVE; /bin/echo printed > '{}'; exit 0"
        ),
        ready.path().display(),
        release.path().display(),
        printed.path().display()
    );
    daemon.spawn(request, 10).expect("spawn history then exit");
    let _ = ready.read_signal(REAL_WORKER_COMPLETION_TIMEOUT);
    drain_pre_attach_producer_output(&mut daemon, &session_id, 11);
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("inventory")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind while the capture starts");
    // The child writes LIVE and exits while Core is not pumping.
    release.release(REAL_WORKER_COMPLETION_TIMEOUT);
    let _ = printed.read_signal(REAL_WORKER_COMPLETION_TIMEOUT);
    pump_until_route_closed(&mut daemon, &adapter, 13);

    let (snapshot, output) = assert_snapshot_then_exit(&adapter.snapshot_delivered_frame_bytes());
    let in_snapshot = snapshot
        .windows(4)
        .filter(|window| *window == b"LIVE")
        .count();
    assert_eq!(
        in_snapshot + output.matches("LIVE").count(),
        1,
        "LIVE exactly once, in the snapshot or as output after FINISH"
    );
    assert!(
        !output.contains("history-"),
        "no pre-fence history as output"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// Path 2: the capture request reaches the worker after it has already sent
/// PROCESS_EXITED. The worker serves it from its final terminal model.
#[cfg(unix)]
#[test]
fn a_capture_requested_after_the_worker_saw_the_exit_is_served_from_the_final_screen() {
    let data_dir = temp_data_dir("capture-after-exit");
    let (probe, probe_events) = WorkerRouteProbe::channel();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_route_probe(Some(probe)),
    );
    let session_id = SessionId("capture-after-exit".to_string());
    let client_id = ClientId("capture-after-exit-client".to_string());
    let subscription_id = SubscriptionId("capture-after-exit-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf FINAL-SCREEN; exit 0".to_string();
    daemon
        .spawn(request, 10)
        .expect("spawn a child that exits at once");
    // Without pumping, wait until the parent reader has the worker's exit.
    let probe_deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    loop {
        match probe_events
            // timer: deadline — the worker must report the exit; expiry fails the test
            .recv_timeout(probe_deadline.saturating_duration_since(Instant::now()))
            .expect("the worker reports the exit")
        {
            WorkerRouteProbeEvent::ProcessExitRead {
                session_id: ref exited,
            } if *exited == session_id => break,
            _ => {}
        }
    }
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach after the worker saw the exit");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("inventory")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    pump_until_route_closed(&mut daemon, &adapter, 12);

    let (snapshot, output) = assert_snapshot_then_exit(&adapter.snapshot_delivered_frame_bytes());
    assert!(
        snapshot
            .windows(b"FINAL-SCREEN".len())
            .any(|window| window == b"FINAL-SCREEN")
            || output.contains("FINAL-SCREEN"),
        "the final screen must reach the route"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// With no capture owed, the parent removes an exited session at once and
/// shuts its worker down: no worker lingers.
#[cfg(unix)]
#[test]
fn an_exit_with_no_capture_leaves_no_worker_behind() {
    let data_dir = temp_data_dir("exit-no-capture");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("exit-no-capture".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "exit 0".to_string();
    daemon
        .spawn(request, 10)
        .expect("spawn a child that exits at once");
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &session_id);
    pump_until_registry_exited(&mut daemon, &session_id, 11);
    assert!(
        wait_pid_exit(worker_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "the worker of an exited session with no capture must exit"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// Shutdown never hands a route that still waits for its attach snapshot a
/// bare PROCESS_EXIT: the owed snapshot ends explicitly first (its FINISH,
/// or the typed ATTACH_STATE failed), and is never lost silently.
#[cfg(unix)]
#[test]
fn shutdown_ends_an_owed_attach_snapshot_before_any_exit() {
    let data_dir = temp_data_dir("shutdown-owed-snapshot");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("shutdown-owed-snapshot".to_string());
    let client_id = ClientId("shutdown-owed-snapshot-client".to_string());
    let subscription_id = SubscriptionId("shutdown-owed-snapshot-sub".to_string());
    daemon.spawn(spawn_request(&session_id), 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("inventory")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind while the attach capture is owed");

    // Nothing pumps, so the attach capture cannot progress: it is still
    // owed when shutdown drops it.
    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown with an owed attach capture");
    assert!(
        pump_wakes_until(&mut daemon, || adapter.close_reason().is_some()),
        "the route ends after shutdown"
    );

    let frames: Vec<TerminalFrame> = adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .map(|bytes| adapter_terminal_frame(bytes))
        .collect();
    let ended_explicitly = |frame: &TerminalFrame| {
        frame.kind() == TerminalKind::SnapshotFinish
            || (frame.kind() == TerminalKind::AttachState
                && decode_attach_state(frame).expect("attach state body")
                    == AttachStateCode::Failed)
    };
    let kinds: Vec<TerminalKind> = frames.iter().map(TerminalFrame::kind).collect();
    let snapshot_end = frames.iter().position(ended_explicitly);
    assert!(
        snapshot_end.is_some(),
        "the owed snapshot must end with FINISH or ATTACH_STATE failed: {kinds:?}"
    );
    if let Some(exit) = kinds
        .iter()
        .position(|kind| *kind == TerminalKind::ProcessExit)
    {
        assert!(
            snapshot_end.is_some_and(|end| end < exit),
            "PROCESS_EXIT must not overtake the owed snapshot: {kinds:?}"
        );
    }
    let _ = fs::remove_dir_all(data_dir);
}

/// The production shutdown grace (500 ms) ends a process group that ignores
/// TERM: shutdown waits the whole grace, then kills the group. Tests whose
/// property is order or preservation may lengthen the grace; this test keeps
/// the default and proves it.
#[cfg(unix)]
#[test]
fn the_default_shutdown_grace_kills_a_group_that_ignores_term() {
    let data_dir = temp_data_dir("grace-kills-ignoring-group");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("grace-kills-ignoring-group".to_string());
    let ready = Fifo::new("grace-ready");
    let hold = Fifo::new("grace-hold");
    let mut request = spawn_request(&session_id);
    // The shell and its cat both ignore TERM; the hold pipe is never written.
    request.request.arguments[1] = format!(
        "trap '' TERM; /bin/echo ready > '{}'; /bin/cat '{}' >/dev/null",
        ready.path().display(),
        hold.path().display()
    );
    daemon
        .spawn(request, 10)
        .expect("spawn TERM-ignoring group");
    let _ = ready.read_signal(HANG_GUARD);
    let (_, pty_child_pid, _) = worker_process_evidence(&daemon, &session_id);

    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown ends a group that ignores TERM");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(500),
        "the group ignores TERM, so shutdown waits the whole 500 ms grace: {elapsed:?}"
    );
    assert!(
        wait_pid_exit(pty_child_pid, HANG_GUARD),
        "the grace's group kill ends the shell"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// Wait, without pumping, until the parent reader of `session_id` reaches
/// its worker's end of stream.
fn wait_for_reader_end(events: &mpsc::Receiver<WorkerRouteProbeEvent>, session_id: &SessionId) {
    let probe_deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    loop {
        match events
            // timer: deadline — the reader must end; expiry fails the test
            .recv_timeout(probe_deadline.saturating_duration_since(Instant::now()))
            .expect("the parent reader reaches the worker's end of stream")
        {
            WorkerRouteProbeEvent::ReaderEnded {
                session_id: ref ended,
            } if ended == session_id => {
                return;
            }
            _ => {}
        }
    }
}

/// A worker killed without PROCESS_EXITED ends its session from its reader's
/// end of stream alone: no input, only wakes. Its owner is swept with the
/// typed worker-link close, and the session fails as worker_lost.
#[cfg(unix)]
#[test]
fn a_killed_worker_is_detected_from_its_reader_end_without_input() {
    let data_dir = temp_data_dir("worker-lost");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-lost".to_string());
    let subscription_id = SubscriptionId("worker-lost-sub".to_string());
    let adapter = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        ClientId("worker-lost-client".to_string()),
        subscription_id.clone(),
        "exec cat >/dev/null",
        10,
    );
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &session_id);
    let _ = Command::new("kill")
        .args(["-9", &worker_pid.to_string()])
        .status()
        .expect("kill the worker");
    assert!(
        wait_pid_exit(worker_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "worker exits"
    );

    // Only wakes drive the pump; no input is sent. Two independent
    // detections follow the kill, in either order: a control write that
    // fails sweeps the route (WorkerLinkFailed) and leaves the session
    // running, and the reader's end of stream reports WorkerLost, which
    // sweeps any route left and fails the session. Wait for both.
    let mut now = 20;
    wait_for(
        "the lost worker's owner swept and its session failed",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            let gone = daemon
                .list_terminal_subscriptions(1024 * 1024)
                .expect("inventory")
                .records
                .iter()
                .all(|row| row.subscription_id != subscription_id);
            let failed = matches!(
                daemon.engine_session_lifecycle(&session_id),
                Some(SessionLifecycleState::Failed { .. })
            );
            if gone && failed {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump the lost worker");
            }
            None
        },
    );
    assert_eq!(
        adapter.close_reason(),
        Some(botster_core::contract::terminal_adapter::TerminalRouteCloseReason::WorkerLinkFailed),
        "the route ends with the typed worker-link close"
    );
    assert_eq!(
        daemon.engine_session_lifecycle(&session_id),
        Some(SessionLifecycleState::Failed {
            reason: "worker_lost".to_string()
        }),
        "the session reports a typed terminal outcome"
    );
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("registry lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Stale)
    ));
    let _ = fs::remove_dir_all(data_dir);
}

/// A capture whose barrier release cannot be queued (the worker stopped
/// taking control frames, so the queue is full) fails its route typed and
/// never fails the host's pump batch.
#[cfg(unix)]
#[test]
fn a_capture_release_that_cannot_be_queued_never_fails_the_pump() {
    let data_dir = temp_data_dir("release-queue-full");
    let (probe, probe_events) = WorkerRouteProbe::channel();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_route_probe(Some(probe)),
    );
    let session_id = SessionId("release-queue-full".to_string());
    let client_id = ClientId("release-queue-full-client".to_string());
    let subscription_id = SubscriptionId("release-queue-full-sub".to_string());
    daemon.spawn(spawn_request(&session_id), 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("inventory")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind while the capture is in flight");
    // Without pumping, wait until the worker has sent the capture's FINISH:
    // the release is owed, and nothing has tried to queue it yet.
    let probe_deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    loop {
        match probe_events
            // timer: deadline — the capture must finish at the worker; expiry fails the test
            .recv_timeout(probe_deadline.saturating_duration_since(Instant::now()))
            .expect("the worker finishes the capture")
        {
            WorkerRouteProbeEvent::SnapshotFinishRead {
                session_id: ref finished,
            } if *finished == session_id => {
                break;
            }
            _ => {}
        }
    }
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &session_id);
    let stopped = StoppedProcess::new(worker_pid);
    wait_for(
        "the control queue full",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |_| {
            daemon
                .input(
                    client_id.clone(),
                    session_id.clone(),
                    vec![b'x'; 64 * 1024],
                    12,
                )
                .err()
                .filter(|error| error.to_string().contains("control queue full"))
                .map(|_| ())
        },
    );
    // This pump handles FINISH and cannot queue the release.
    let batch = botster_core::contract::terminal_wake::TerminalWakeBatch {
        adapter_routes: Vec::new(),
        ingress_sessions: vec![session_id.clone()],
    };
    let _ = daemon
        .pump_woken(&batch, 13)
        .expect("a release that cannot be queued must not fail the pump");
    // The capturing route ends with ATTACH_STATE failed, typed.
    let mut now = 14;
    wait_for(
        "the capturing route's close",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            if adapter.close_reason().is_some() {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump the failed route");
            }
            None
        },
    );
    let last = adapter_terminal_frame(
        adapter
            .snapshot_delivered_frame_bytes()
            .last()
            .expect("the route received frames"),
    );
    assert!(
        last.kind() == TerminalKind::AttachState
            && decode_attach_state(&last).expect("attach state body") == AttachStateCode::Failed,
        "the capturing route ends with ATTACH_STATE failed"
    );
    drop(stopped);
    let _ = Command::new("kill")
        .args(["-9", &worker_pid.to_string()])
        .status();
    let _ = fs::remove_dir_all(data_dir);
}

/// Host input into a failed control plane is refused, typed, at the call:
/// never buffered and then silently dropped.
#[cfg(unix)]
#[test]
fn input_into_a_failed_control_plane_is_refused_typed() {
    let data_dir = temp_data_dir("sealed-input-refused");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("sealed-input-refused".to_string());
    let client_id = ClientId("sealed-input-refused-client".to_string());
    daemon.spawn(spawn_request(&session_id), 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("sealed-input-refused-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &session_id);
    // A stopped worker takes no control frames: the queue fills, and the
    // control writer misses its production write deadline.
    let stopped = StoppedProcess::new(worker_pid);
    wait_for(
        "the control queue full",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |_| {
            daemon
                .input(
                    client_id.clone(),
                    session_id.clone(),
                    vec![b'x'; 64 * 1024],
                    12,
                )
                .err()
                .filter(|error| error.to_string().contains("control queue full"))
                .map(|_| ())
        },
    );
    let mut now = 13;
    wait_for(
        "the control plane failed",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            if matches!(
                daemon.control_plane_state(&session_id),
                botster_core::runtime::ControlPlaneState::Failed(_)
            ) {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump the writer failure");
            }
            None
        },
    );

    let refused = daemon.input(
        client_id,
        session_id.clone(),
        b"after-failure\n".to_vec(),
        now + 1,
    );
    assert!(
        matches!(&refused, Err(CoreDaemonError::ControlPlaneFailed(id)) if *id == session_id),
        "input into a failed control plane must be refused typed: {refused:?}"
    );
    drop(stopped);
    let _ = Command::new("kill")
        .args(["-9", &worker_pid.to_string()])
        .status();
    let _ = fs::remove_dir_all(data_dir);
}

/// Host input buffered for a session whose worker is then lost never fails
/// another session's input: flushing every session's buffered input drops
/// the lost session's bytes, and the other session's input and output
/// complete.
#[cfg(unix)]
#[test]
fn input_for_a_lost_worker_never_fails_other_work_in_the_batch() {
    let data_dir = temp_data_dir("lost-input-batch");
    let (probe, probe_events) = WorkerRouteProbe::channel();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_route_probe(Some(probe)),
    );
    let lost = SessionId("lost-input-a".to_string());
    let live = SessionId("lost-input-b".to_string());
    let lost_client = ClientId("lost-input-a-client".to_string());
    let live_client = ClientId("lost-input-b-client".to_string());
    for (session_id, client_id, subscription) in [
        (&lost, &lost_client, "lost-input-a-sub"),
        (&live, &live_client, "lost-input-b-sub"),
    ] {
        daemon.spawn(spawn_request(session_id), 10).expect("spawn");
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                SubscriptionId(subscription.to_string()),
                11,
            )
            .expect("attach");
    }
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &lost);

    // A stopped worker stops reading its control socket, so A's control
    // queue fills and its further input stays buffered in Core.
    let stopped = StoppedProcess::new(worker_pid);
    wait_for(
        "A's control queue full",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |_| {
            daemon
                .input(lost_client.clone(), lost.clone(), vec![b'x'; 64 * 1024], 12)
                .err()
                .filter(|error| error.to_string().contains("control queue full"))
                .map(|_| ())
        },
    );

    // The stopped worker dies: its reader ends without an exit report, and
    // the pump reports the loss and removes A.
    let _ = Command::new("kill")
        .args(["-9", &worker_pid.to_string()])
        .status()
        .expect("kill worker A");
    drop(stopped);
    assert!(
        wait_pid_exit(worker_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "worker A exits"
    );
    wait_for_reader_end(&probe_events, &lost);
    let batch = botster_core::contract::terminal_wake::TerminalWakeBatch {
        adapter_routes: Vec::new(),
        ingress_sessions: vec![lost.clone()],
    };
    let _ = daemon.pump_woken(&batch, 13).expect("pump A's loss");
    assert_eq!(
        daemon.engine_session_lifecycle(&lost),
        Some(SessionLifecycleState::Failed {
            reason: "worker_lost".to_string()
        })
    );

    // B's input flushes every session's buffered input, A's included.
    daemon
        .input(live_client, live.clone(), b"still-live\n".to_vec(), 14)
        .expect("A's lost input must not fail B's input");
    let drained = drain_until(&mut daemon, &live, "echo:still-live");
    assert!(terminal_output(&drained.client_egress).contains("echo:still-live"));
    let _ = fs::remove_dir_all(data_dir);
}

/// Complete the adapter's one in-flight write and wake Core, as a real
/// transport does when its write finishes and capacity returns.
fn complete_one_slot_and_wake(adapter: &SharedFakeTerminalAdapter) {
    if adapter.snapshot_pressure() == TerminalAdapterPressure::Full {
        adapter.complete_write();
        let _ = adapter.wake(TerminalWakeKind::Writable);
    }
}

fn adapter_has_live(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| {
            adapter_frame_type(bytes) == "terminal_output"
                && adapter_payload_text(bytes).contains("LIVE")
        })
}

fn adapter_has_process_exit(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| adapter_frame_type(bytes) == "process_exit")
}

/// Decode one bound-adapter delivery as the scheme 2 `TerminalBody` it is.
fn adapter_terminal_frame(bytes: &[u8]) -> TerminalFrame {
    TerminalFrame::from_bytes(bytes)
        .expect("bound adapter deliveries are scheme 2 TerminalBody frames")
}

/// Snapshot phase or attached transition carried by one bound-adapter delivery.
fn adapter_phase(bytes: &[u8]) -> Option<&'static str> {
    let frame = adapter_terminal_frame(bytes);
    match frame.kind() {
        TerminalKind::SnapshotReady => Some("ready"),
        TerminalKind::SnapshotHistory => Some("history"),
        TerminalKind::SnapshotFinish => Some("finish"),
        TerminalKind::AttachState => (decode_attach_state(&frame).expect("attach state body")
            == AttachStateCode::Attached)
            .then_some("attached"),
        _ => None,
    }
}

fn adapter_frame_type(bytes: &[u8]) -> String {
    match adapter_terminal_frame(bytes).kind() {
        TerminalKind::Output => "terminal_output",
        TerminalKind::ProcessExit => "process_exit",
        _ => "other",
    }
    .to_string()
}

fn adapter_payload_text(bytes: &[u8]) -> String {
    let frame = adapter_terminal_frame(bytes);
    if frame.kind() == TerminalKind::Output {
        String::from_utf8_lossy(frame.body()).into_owned()
    } else {
        String::new()
    }
}

#[cfg(unix)]
#[test]
fn worker_pending_replacement_does_not_start_the_old_subscription() {
    let data_dir = temp_data_dir("worker-pending-replace");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-pending-replace-session".to_string());
    let active = ClientId("worker-pending-replace-active".to_string());
    let pending = ClientId("worker-pending-replace-pending".to_string());
    let active_sub = SubscriptionId("worker-pending-replace-active-sub".to_string());
    let old_sub = SubscriptionId("worker-pending-replace-old-sub".to_string());
    let new_sub = SubscriptionId("worker-pending-replace-new-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(active.clone(), session_id.clone(), active_sub.clone(), 11)
        .expect("active attach");
    daemon
        .attach(pending.clone(), session_id.clone(), old_sub.clone(), 12)
        .expect("queue old pending");
    daemon
        .attach(pending, session_id.clone(), new_sub.clone(), 13)
        .expect("replace pending");
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .map(|row| row.subscription_id)
        .collect();
    assert!(live.contains(&active_sub));
    assert!(live.contains(&new_sub));
    assert!(
        !live.contains(&old_sub),
        "replaced pending subscription must not stay in inventory: {live:?}"
    );

    // Captures on the session run in order, so the replacement's own
    // snapshot is the positive event: until it arrives, and with it, the
    // old subscription must have emitted nothing.
    let mut saw_old_after_replace = false;
    on_wakes_until(
        &mut daemon,
        "the replacement subscription's attach snapshot",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let drained = daemon.drain(&session_id, 20).expect("drain");
            let mut replacement_attached = false;
            for (_, frame) in drained.client_egress {
                match frame {
                    TransportEgress::Snapshot {
                        subscription_id, ..
                    }
                    | TransportEgress::AttachState {
                        subscription_id, ..
                    } if subscription_id == old_sub => saw_old_after_replace = true,
                    TransportEgress::Snapshot {
                        subscription_id, ..
                    } if subscription_id == new_sub => replacement_attached = true,
                    _ => {}
                }
            }
            replacement_attached.then_some(())
        },
    );
    assert!(
        !saw_old_after_replace,
        "old pending subscription must never start a snapshot boundary"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_owner_replacement_cancels_the_active_boundary() {
    let data_dir = temp_data_dir("worker-same-key-replace");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-same-key-replace-session".to_string());
    let first = ClientId("worker-same-key-replace-a".to_string());
    let second = ClientId("worker-same-key-replace-b".to_string());
    let subscription = SubscriptionId("worker-same-key-replace-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf ready; while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), subscription.clone(), 11)
        .expect("attach first owner");
    let first_gen = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("first inventory")
        .generation;
    let replaced = daemon
        .attach(second.clone(), session_id.clone(), subscription.clone(), 12)
        .expect("replace owner before first boundary finishes");
    assert!(
        replaced.client_egress.iter().any(|(client_id, frame)| {
            client_id == &second
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &subscription
                )
        }),
        "replacement must start its attach immediately: {:?}",
        replaced.client_egress
    );
    assert!(
        replaced
            .client_egress
            .iter()
            .all(|(client_id, _)| client_id != &first),
        "cancelled owner must not receive the replacement attach frames"
    );
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].client_id, second);
    assert_eq!(live[0].subscription_id, subscription);
    assert_eq!(
        live[0].generation,
        botster_core::TerminalSubscriptionGeneration(first_gen.0 + 1)
    );

    let mut saw_first_after_replace = false;
    let replacement = drain_until_attached(&mut daemon, &session_id, &second);
    for (client_id, frame) in &replacement.client_egress {
        if client_id == &first
            && matches!(
                frame,
                TransportEgress::Snapshot { .. } | TransportEgress::AttachState { .. }
            )
        {
            saw_first_after_replace = true;
        }
    }
    assert!(
        !saw_first_after_replace,
        "cancelled owner must not receive later snapshot or attach frames: {:?}",
        replacement.client_egress
    );
    assert!(
        replacement.client_egress.iter().any(|(client_id, frame)| {
            client_id == &second && matches!(frame, TransportEgress::Snapshot { .. })
        }),
        "replacement must start its own snapshot boundary: {:?}",
        replacement.client_egress
    );
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].client_id, second);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_takeover_preserves_pending_sibling_input_and_resize() {
    let data_dir = temp_data_dir("worker-takeover-sibling");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-takeover-sibling-session".to_string());
    let first = ClientId("worker-takeover-sibling-a".to_string());
    let second = ClientId("worker-takeover-sibling-b".to_string());
    let sibling = ClientId("worker-takeover-sibling-c".to_string());
    let first_sub = SubscriptionId("worker-takeover-sibling-sub-a".to_string());
    let sibling_sub = SubscriptionId("worker-takeover-sibling-sub-c".to_string());
    // The child reads its input only after the gate opens, so the echo of
    // the sibling's pending input follows both attaches as live output.
    let gate = Fifo::new("takeover-sibling-gate");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; /bin/cat '{}' >/dev/null; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        gate.path().display()
    );
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first owner");
    daemon
        .attach(sibling.clone(), session_id.clone(), sibling_sub.clone(), 12)
        .expect("queue sibling");
    daemon
        .input(
            sibling.clone(),
            session_id.clone(),
            b"SIBLING-KEEP\n".to_vec(),
            13,
        )
        .expect("queue sibling input");
    daemon
        .resize(sibling.clone(), session_id.clone(), 30, 100, 14)
        .expect("queue sibling resize");
    daemon
        .attach(second.clone(), session_id.clone(), first_sub.clone(), 15)
        .expect("take over first key");
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
    assert!(live
        .iter()
        .any(|row| row.client_id == second && row.subscription_id == first_sub));
    assert!(live
        .iter()
        .any(|row| row.client_id == sibling && row.subscription_id == sibling_sub));
    assert!(live.iter().all(|row| row.client_id != first));
    // One drain can carry both Attached frames and the echo, so keep every
    // drained frame and wait only for what has not arrived yet.
    let mut seen = drain_until_attached(&mut daemon, &session_id, &second);
    if !client_attached(&seen, &sibling) {
        let more = drain_until_attached(&mut daemon, &session_id, &sibling);
        seen.client_egress.extend(more.client_egress);
    }
    gate.release(Duration::from_secs(5));
    if !terminal_output(&seen.client_egress).contains("echo:SIBLING-KEEP") {
        drain_until_terminal_marker(&mut daemon, &session_id, "echo:SIBLING-KEEP", 30);
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_takeover_drops_the_new_owners_obsolete_pending_subscription() {
    let data_dir = temp_data_dir("worker-takeover-stale-pending");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-takeover-stale-pending-session".to_string());
    let first = ClientId("worker-takeover-stale-pending-a".to_string());
    let second = ClientId("worker-takeover-stale-pending-b".to_string());
    let first_sub = SubscriptionId("worker-takeover-stale-pending-x".to_string());
    let stale_sub = SubscriptionId("worker-takeover-stale-pending-y".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first owner");
    daemon
        .attach(second.clone(), session_id.clone(), stale_sub.clone(), 12)
        .expect("queue obsolete pending");
    daemon
        .attach(second.clone(), session_id.clone(), first_sub.clone(), 13)
        .expect("take over first key");
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
    assert!(live
        .iter()
        .any(|row| row.client_id == second && row.subscription_id == first_sub));
    assert!(
        live.iter().all(|row| row.subscription_id != stale_sub),
        "obsolete pending subscription must leave inventory: {live:?}"
    );
    let mut saw_stale = false;
    let replacement = drain_until_attached(&mut daemon, &session_id, &second);
    for (_, frame) in &replacement.client_egress {
        if matches!(
            frame,
            TransportEgress::Snapshot {
                subscription_id,
                ..
            }
            | TransportEgress::AttachState {
                subscription_id,
                ..
            } if subscription_id == &stale_sub
        ) {
            saw_stale = true;
        }
    }
    assert!(
        !saw_stale,
        "obsolete pending subscription must never start a snapshot boundary: {:?}",
        replacement.client_egress
    );
    let live: Vec<_> = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
    assert!(live.iter().all(|row| row.subscription_id != stale_sub));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_subscription_drain_retains_foreign_route_frames() {
    let data_dir = temp_data_dir("worker-route-drain");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-route-drain-session".to_string());
    let client_a = ClientId("worker-route-drain-a".to_string());
    let client_b = ClientId("worker-route-drain-b".to_string());
    let subscription_a = SubscriptionId("worker-route-drain-sub-a".to_string());
    let subscription_b = SubscriptionId("worker-route-drain-sub-b".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn route worker");
    daemon
        .attach(
            client_a.clone(),
            session_id.clone(),
            subscription_a.clone(),
            11,
        )
        .expect("attach route A");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_a);
    daemon
        .attach(
            client_b.clone(),
            session_id.clone(),
            subscription_b.clone(),
            12,
        )
        .expect("attach route B");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_b);
    daemon
        .input(
            client_a.clone(),
            session_id.clone(),
            b"ROUTE-DRAIN-MARKER\n".to_vec(),
            13,
        )
        .expect("write route marker");

    let mut route_a = botster_core_daemon::DrainResult::default();
    let tick_deadline = Instant::now() + HANG_GUARD;
    for tick in 0.. {
        let drained = daemon
            .drain_subscription(&client_a, &session_id, &subscription_a, 20 + tick)
            .expect("drain route A");
        assert!(drained.client_egress.iter().all(|(target, frame)| {
            target == &client_a
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    }
                    | TransportEgress::Snapshot {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    }
                    | TransportEgress::AttachState {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    } if routed_session == &session_id
                        && routed_subscription == &subscription_a
                )
        }));
        route_a.client_egress.extend(drained.client_egress);
        if renderable_output_for_client(&route_a.client_egress, &client_a)
            .contains("echo:ROUTE-DRAIN-MARKER")
        {
            break;
        }
        assert!(
            Instant::now() < tick_deadline,
            "the route-drain marker did not arrive within {HANG_GUARD:?}"
        );
        // timer: deadline — HANG_GUARD bounds this wait
        let _ = daemon.wait_wakes(tick_deadline.saturating_duration_since(Instant::now()));
    }
    assert!(
        renderable_output_for_client(&route_a.client_egress, &client_a)
            .contains("echo:ROUTE-DRAIN-MARKER")
    );

    let route_b = daemon
        .drain_subscription(&client_b, &session_id, &subscription_b, 200)
        .expect("drain retained route B");
    assert!(
        renderable_output_for_client(&route_b.client_egress, &client_b)
            .contains("echo:ROUTE-DRAIN-MARKER")
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_concurrent_attaches_serialize_without_pre_attached_live_output() {
    let data_dir = temp_data_dir("worker-concurrent-attach");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-concurrent-attach-session".to_string());
    let client_a = ClientId("worker-concurrent-attach-a".to_string());
    let client_b = ClientId("worker-concurrent-attach-b".to_string());
    let sub_a = SubscriptionId("worker-concurrent-attach-sub-a".to_string());
    let sub_b = SubscriptionId("worker-concurrent-attach-sub-b".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn concurrent worker");
    let first = daemon
        .attach(client_a.clone(), session_id.clone(), sub_a, 11)
        .expect("start first attach");
    let second = daemon
        .attach(client_b.clone(), session_id.clone(), sub_b, 12)
        .expect("queue second attach");
    daemon
        .input(
            client_b.clone(),
            session_id.clone(),
            b"CONCURRENT-POST\n".to_vec(),
            13,
        )
        .expect("queue second client input");

    let mut egress = first.client_egress;
    egress.extend(second.client_egress);
    let mut attached_a = false;
    let mut attached_b = false;
    let tick_deadline = Instant::now() + HANG_GUARD;
    for tick in 0.. {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("drain serialized attaches");
        for (target, frame) in &drained.client_egress {
            match frame {
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } if target == &client_a => attached_a = true,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } if target == &client_b => {
                    assert!(attached_a, "the first worker encode must finish first");
                    attached_b = true;
                }
                TransportEgress::TerminalOutput { .. } if target == &client_a => {
                    assert!(attached_a, "client A output must follow Attached")
                }
                TransportEgress::TerminalOutput { .. } if target == &client_b => {
                    assert!(attached_b, "client B output must follow Attached")
                }
                _ => {}
            }
        }
        egress.extend(drained.client_egress);
        if attached_b
            && renderable_output_for_client(&egress, &client_b).contains("echo:CONCURRENT-POST")
        {
            break;
        }
        assert!(Instant::now() < tick_deadline, "both serialized attaches and the post-attach echo did not arrive within {HANG_GUARD:?}");
        // timer: deadline — HANG_GUARD bounds this wait
        let _ = daemon.wait_wakes(tick_deadline.saturating_duration_since(Instant::now()));
    }
    assert!(attached_a && attached_b);
    assert!(renderable_output_for_client(&egress, &client_b).contains("echo:CONCURRENT-POST"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_honors_host_ghostty_scrollback_byte_budget() {
    let data_dir = temp_data_dir("dwgs-override");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(LOW_GHOSTTY_MAX_SCROLLBACK_BYTES),
    );
    let session_id = SessionId("dwgs-override-session".to_string());
    let primary_client = ClientId("dwgs-override-primary-client".to_string());
    let late_client = ClientId("dwgs-override-late-client".to_string());
    let marker_count = 4_500;

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-override-primary-subscription".to_string()),
            11,
        )
        .expect("primary attach should succeed");
    let _ = drain_until_attached(&mut daemon, &session_id, &primary_client);

    for chunk_start in (0..marker_count).step_by(10) {
        let chunk_end = (chunk_start + 10).min(marker_count);
        let mut scrollback_input = Vec::new();
        for line in chunk_start..chunk_end {
            scrollback_input.extend_from_slice(format!("scrollback-line-{line:05}\n").as_bytes());
        }
        daemon
            .input(
                primary_client.clone(),
                session_id.clone(),
                scrollback_input,
                12 + chunk_start as u64,
            )
            .expect("scrollback generator chunk should write");
        let chunk_marker = format!("echo:scrollback-line-{:05}", chunk_end - 1);
        drain_until_terminal_marker(
            &mut daemon,
            &session_id,
            &chunk_marker,
            20 + chunk_start as u64,
        );
    }
    let newest_marker = format!("echo:scrollback-line-{:05}", marker_count - 1);

    let late_subscription = SubscriptionId("dwgs-override-late-subscription".to_string());
    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            101,
        )
        .expect("late attach should receive a scrollback snapshot");
    let late_drain = drain_until_attached(&mut daemon, &session_id, &late_client);
    let mut late_egress = late_attach.client_egress;
    late_egress.extend(late_drain.client_egress);
    let (_, snapshot) = first_snapshot_for_client(&late_egress, &late_client)
        .expect("late Ghostty attach should include a snapshot frame");
    let plain_text = ghostty_snapshot_plain_text(&snapshot);
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, marker_count);
    let retained_marker_count = retained_markers.len();
    let replayed_text_length = plain_text.len();
    let retains_newest_marker = plain_text.contains(&newest_marker);

    daemon
        .shutdown(Some(session_id), 103)
        .expect("worker-backed daemon should shut down");
    let _ = fs::remove_dir_all(data_dir);

    assert!(
        retained_marker_count > 0,
        "low Ghostty byte budget should remain above the page-allocation floor"
    );
    assert!(
        retained_marker_count < EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
        "host override should retain fewer markers than the default budget's pinned minimum; retained markers: {}; replayed text length: {}",
        retained_marker_count,
        replayed_text_length
    );
    assert!(
        retains_newest_marker,
        "low Ghostty byte budget should retain the newest marker"
    );
}

#[cfg(unix)]
#[test]
fn daemon_default_ghostty_scrollback_byte_budget_pins_effective_window() {
    let mut terminal = GhosttyTerminal::with_config(
        TerminalScreenSize::new(24, 80),
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty terminal with daemon default config");
    for chunk_start in (0..12_000).step_by(100) {
        let mut output = Vec::new();
        for line in chunk_start..chunk_start + 100 {
            output.extend_from_slice(format!("echo:scrollback-line-{line:05}\n").as_bytes());
        }
        terminal.write_output_bytes(&output);
    }

    let plain_text = terminal
        .plain_text()
        .expect("test should format Ghostty terminal text");
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, 12_000);
    assert!(
        retained_markers.len() >= EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
        "default Ghostty byte budget should retain a material history window at 24x80; retained markers: {}; text length: {}",
        retained_markers.len(),
        plain_text.len()
    );
    assert!(
        !plain_text.contains(EXPECTED_GHOSTTY_DROPPED_MARKER),
        "default Ghostty byte budget should drop history beyond the configured window at 24x80; text length: {}",
        plain_text.len()
    );

    let snapshot = terminal
        .export_snapshot()
        .expect("test should export Ghostty snapshot");
    assert_ghostty_snapshot_replays_minimum_markers(
        &snapshot,
        12_000,
        EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
    );
    assert_ghostty_snapshot_does_not_replay_marker(&snapshot, EXPECTED_GHOSTTY_DROPPED_MARKER);
}

#[cfg(unix)]
#[test]
fn worker_backed_duplicate_attach_refreshes_same_subscription_with_current_snapshot() {
    let data_dir = temp_data_dir("worker-duplicate-attach-refresh");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-duplicate-attach-session".to_string());
    let client_id = ClientId("worker-duplicate-attach-client".to_string());
    let subscription_id = SubscriptionId("worker-duplicate-attach-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn real worker");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("first attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"duplicate-current-state\n".to_vec(),
            12,
        )
        .expect("write current-state marker");
    let _ = drain_until(&mut daemon, &session_id, "echo:duplicate-current-state");

    let duplicate_attach = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            13,
        )
        .expect("duplicate attach must refresh the route");
    let duplicate_drain = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut duplicate_egress = duplicate_attach.client_egress;
    duplicate_egress.extend(duplicate_drain.client_egress);
    let (_, snapshot) = first_snapshot_for_client(&duplicate_egress, &client_id)
        .expect("duplicate attach must deliver a fresh GHOSTSNP");
    assert_ghostty_snapshot_replays_marker(&snapshot, "echo:duplicate-current-state");
    assert!(duplicate_egress.iter().all(|(received_client, frame)| {
        received_client == &client_id
            && matches!(
                frame,
                TransportEgress::Snapshot {
                    subscription_id: received_subscription,
                    ..
                } | TransportEgress::AttachState {
                    subscription_id: received_subscription,
                    ..
                } if received_subscription == &subscription_id
            )
    }));
    let attaching_index = duplicate_egress
        .iter()
        .position(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attaching,
                    ..
                }
            )
        })
        .expect("duplicate attach must return Attaching");
    let snapshot_index = duplicate_egress
        .iter()
        .position(|(_, frame)| matches!(frame, TransportEgress::Snapshot { .. }))
        .expect("duplicate attach must return Snapshot");
    let attached_index = duplicate_egress
        .iter()
        .position(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                }
            )
        })
        .expect("duplicate attach must return Attached");
    assert!(attaching_index < snapshot_index && snapshot_index < attached_index);
    assert!(duplicate_egress.iter().any(|(received_client, frame)| {
        received_client == &client_id
            && matches!(
                frame,
                TransportEgress::AttachState {
                    subscription_id: received_subscription,
                    state: TerminalAttachState::Attached,
                    ..
                } if received_subscription == &subscription_id
            )
    }));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_state_is_not_restart_durable_today() {
    let data_dir = temp_data_dir("daemon-notification-not-durable");
    let target = notification_session_target("non-durable-session");
    {
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        daemon
            .post_notification(PostNotificationRequest {
                item: notification("non-durable-notification", target.clone(), 10),
            })
            .expect("first daemon should queue in-memory notification");
    }

    let mut restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let drained = restarted
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("fresh daemon should have an empty in-memory inbox");

    assert!(
        drained.items.is_empty(),
        "daemon notification/envelope state is in-memory and not restart durable today"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_restart_adopts_live_worker_and_reattaches() {
    let data_dir = temp_data_dir("daemon-restart-adopts-live-worker");
    let session_id = SessionId("daemon-restart-session".to_string());
    let client_id = ClientId("daemon-restart-client".to_string());
    let subscription_id = SubscriptionId("daemon-restart-subscription".to_string());

    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("first daemon should spawn live session");
        let mut record = daemon
            .registry()
            .load(&session_id)
            .expect("registry load should succeed")
            .expect("spawn should persist restart evidence");
        assert!(
            record
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("worker_control_socket"))
                .is_some(),
            "worker-backed daemon should persist a reconnectable worker endpoint"
        );
        record
            .recovery_identity
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("recovery identity object")
            .remove("atomic_snapshot_boundary");
        daemon
            .registry()
            .save(&record)
            .expect("persist a legacy v2 worker record without the capability");
        daemon.release_for_restart();
    }

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let reports = restarted
        .adoption_scan()
        .expect("restarted daemon should scan registry");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].state, SessionAdoptionState::Adoptable);
    restarted
        .adopt_session(&session_id, 12)
        .expect("fresh daemon should adopt live worker process");

    let listed = restarted
        .list()
        .expect("restarted daemon should list durable sessions");
    assert_eq!(listed[0].session_id, session_id);
    assert_eq!(listed[0].registry_state, RegistrySessionState::Running);

    restarted
        .attach(client_id.clone(), session_id.clone(), subscription_id, 13)
        .expect("restarted daemon should attach through live engine route");
    restarted
        .input(
            client_id.clone(),
            session_id.clone(),
            b"after-restart\n".to_vec(),
            14,
        )
        .expect("restarted daemon should send input through adopted route");
    let drained = drain_until(&mut restarted, &session_id, "echo:after-restart");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:after-restart"),
        "reattached daemon should drain live worker output: {output:?}"
    );

    restarted
        .shutdown(Some(session_id.clone()), 30)
        .expect("restarted daemon should shut down adopted session");
    let listed = restarted
        .list()
        .expect("registry list should load after adopted shutdown");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);

    let _ = fs::remove_dir_all(data_dir);
}

/// A successor daemon that adopts a live worker, and only adopts it, must
/// end that worker and its child at shutdown. The child blocks reading its
/// PTY and never exits alone: only the shutdown can end it.
#[cfg(unix)]
#[test]
fn a_successor_shutdown_ends_an_adopted_worker_and_its_child() {
    let data_dir = temp_data_dir("successor-shutdown-ends-adopted-worker");
    let session_id = SessionId("successor-shutdown-session".to_string());
    let request = spawn_request(&session_id);
    let (worker_pid, child_pid) = {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon.spawn(request, 10).expect("first daemon spawns");
        let (worker_pid, child_pid, _) = worker_process_evidence(&daemon, &session_id);
        daemon.release_for_restart();
        (worker_pid, child_pid)
    };

    let mut successor =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    // Run the successor as Hub does: a wake pump that observes its stop,
    // then a whole-daemon shutdown.
    let control = successor.wake_pump_control();
    let reports = successor.adoption_scan().expect("successor scans");
    assert_eq!(reports[0].state, SessionAdoptionState::Adoptable);
    successor
        .adopt_session(&session_id, 11)
        .expect("successor adopts the live worker");
    control.request_stop();
    // After a stop the pump returns at most one final collision batch, and
    // every later call returns Stopped.
    match successor.wait_pump(Duration::ZERO) {
        botster_core_daemon::WakePumpWait::Stopped => {}
        botster_core_daemon::WakePumpWait::Wakes(_) => assert_eq!(
            successor.wait_pump(Duration::ZERO),
            botster_core_daemon::WakePumpWait::Stopped,
            "the call after the collision batch observes the stop"
        ),
        other => panic!("a stopped pump returns its collision batch or Stopped: {other:?}"),
    }
    successor
        .shutdown(None, 12)
        .expect("successor shuts every session down");

    let live: Vec<u32> = [worker_pid, child_pid]
        .into_iter()
        .filter(|pid| !wait_pid_exit(*pid, REAL_WORKER_COMPLETION_TIMEOUT))
        .collect();
    // This test started these processes; end any survivor before failing.
    for pid in &live {
        signal_process(*pid, "KILL");
    }
    let _ = fs::remove_dir_all(data_dir);
    assert!(
        live.is_empty(),
        "adopted worker {worker_pid} and child {child_pid} end at the successor's shutdown; still live: {live:?}"
    );
}

#[cfg(unix)]
#[test]
fn production_worker_root_handles_canonical_and_long_session_ids() {
    let data_dir = temp_data_dir("production-worker-id-length");
    let canonical = SessionId("123e4567-e89b-12d3-a456-426614174000".to_string());
    // The longest id Core accepts: far longer than a socket path allows.
    let long = SessionId(format!(
        "sess-long-{}",
        "x".repeat(botster_core::MAX_SESSION_ID_BYTES - "sess-long-".len())
    ));
    assert_eq!(long.0.len(), botster_core::MAX_SESSION_ID_BYTES);
    let canonical_client = ClientId("canonical-client".to_string());
    let long_client = ClientId("long-client".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(spawn_request(&canonical), 10)
        .expect("spawn canonical worker-backed session");
    daemon
        .spawn(spawn_request(&long), 11)
        .expect("spawn long-id worker-backed session");
    let over_cap = SessionId(format!("{}x", long.0));
    assert!(matches!(
        daemon.spawn(spawn_request(&over_cap), 12),
        Err(CoreDaemonError::SessionReservation(
            botster_core::SessionReservationRefusal::SessionIdTooLong
        ))
    ));
    let listed = daemon.list().expect("list both worker-backed sessions");
    assert!(listed.iter().any(|session| session.session_id == canonical));
    assert!(listed.iter().any(|session| session.session_id == long));

    let (canonical_worker, canonical_pty, canonical_socket) =
        worker_process_evidence(&daemon, &canonical);
    let (long_worker, long_pty, long_socket) = worker_process_evidence(&daemon, &long);
    let worker_root = canonical_socket
        .parent()
        .expect("canonical worker socket root")
        .to_path_buf();
    assert_eq!(long_socket.parent(), Some(worker_root.as_path()));
    assert!(worker_root.starts_with(std::env::temp_dir()));
    assert_ne!(canonical_socket, long_socket);
    assert_eq!(
        canonical_socket
            .file_name()
            .expect("canonical basename")
            .len(),
        27
    );
    assert_eq!(long_socket.file_name().expect("long basename").len(), 27);
    assert!(
        canonical_socket.as_os_str().len() <= 103,
        "canonical production endpoint must fit macOS SUN_LEN: {canonical_socket:?}"
    );
    assert!(
        long_socket.as_os_str().len() <= 103,
        "long production endpoint must fit macOS SUN_LEN: {long_socket:?}"
    );

    for (session, client, subscription, marker, now) in [
        (
            &canonical,
            &canonical_client,
            "canonical-subscription",
            "canonical-production-marker",
            12,
        ),
        (
            &long,
            &long_client,
            "long-subscription",
            "long-production-marker",
            13,
        ),
    ] {
        daemon
            .attach(
                client.clone(),
                session.clone(),
                SubscriptionId(subscription.to_string()),
                now,
            )
            .expect("attach worker-backed session");
        daemon
            .input(
                client.clone(),
                session.clone(),
                format!("{marker}\n").into_bytes(),
                now + 1,
            )
            .expect("send marker through worker-backed session");
        let drained = drain_until_for_client(&mut daemon, session, client, marker);
        assert!(
            renderable_output_for_client(&drained.client_egress, client).contains(marker),
            "worker-backed session should read back its own marker"
        );
    }

    daemon
        .shutdown(Some(long.clone()), 30)
        .expect("shut down long-id session");
    daemon
        .shutdown(Some(canonical.clone()), 31)
        .expect("shut down canonical session");
    for pid in [canonical_worker, long_worker, canonical_pty, long_pty] {
        assert!(
            wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
            "production worker and PTY cleanup: pid {pid}"
        );
    }
    // Each worker removes its socket before it exits.
    assert!(
        !canonical_socket.exists() && !long_socket.exists(),
        "worker sockets removed"
    );
    assert!(
        !worker_root.exists(),
        "worker-owned production root should be removed when empty"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn adoption_of_live_process_with_reaped_socket_fails_without_rebinding() {
    let data_dir = temp_data_dir("reaped-worker-socket");
    let session_id = SessionId("reaped-worker-session".to_string());
    let mut original =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    original
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker before simulated socket reaping");
    let (worker_pid, pty_pid, socket_path) = worker_process_evidence(&original, &session_id);
    original.release_for_restart();
    assert!(process_exists(worker_pid));
    assert!(process_exists(pty_pid));
    fs::remove_file(&socket_path).expect("simulate macOS reaping the live socket pathname");

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let error = restarted
        .adopt_session(&session_id, 12)
        .expect_err("missing persisted endpoint must not create a replacement worker");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(
            botster_core::SessionRuntimeError {
                kind: botster_core::SessionRuntimeErrorKind::SpawnFailed,
                ref message,
            }
        )) if message.starts_with("connect worker control socket failed: ")
    ));
    assert!(
        !socket_path.exists(),
        "adoption must not bind a replacement endpoint"
    );
    assert!(
        process_exists(worker_pid),
        "adoption must not replace or kill worker"
    );

    original
        .shutdown(Some(session_id.clone()), 20)
        .expect("the connected owner should still shut down the reaped worker");
    for pid in [worker_pid, pty_pid] {
        assert!(
            wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
            "bounded reap after owner shutdown: {pid}"
        );
    }
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_lifecycle_source_drives_projection_through_exit_and_removal() {
    let data_dir = temp_data_dir("lifecycle-source-exit-removal");
    let session_id = SessionId("lifecycle-source-session".to_string());
    let client_id = ClientId("lifecycle-source-client".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(8),
    );

    let baseline = daemon
        .lifecycle_baseline()
        .expect("empty lifecycle baseline");
    assert!(baseline.sessions.is_empty());

    daemon
        .spawn(self_exit_spawn_request(&session_id), 10)
        .expect("worker-backed lifecycle fixture should spawn");
    let (_, _, lifecycle_socket) = worker_process_evidence(&daemon, &session_id);
    let lifecycle_worker_root = lifecycle_socket
        .parent()
        .expect("worker socket parent")
        .to_path_buf();
    let running = daemon.lifecycle_changes(&baseline.cursor);
    assert!(running.resync_required.is_none());
    assert_eq!(running.changes.len(), 1);
    assert!(matches!(
        &running.changes[0].kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
                && matches!(record.lifecycle, Some(SessionLifecycleState::Running))
    ));
    assert!(!daemon
        .remove_session(&session_id)
        .expect("live session removal should be rejected without mutation"));

    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("lifecycle-source-subscription".to_string()),
            11,
        )
        .expect("fixture should attach through the production daemon facade");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should cause natural process exit");

    let mut terminal_drain = botster_core_daemon::DrainResult::default();
    let tick_deadline = Instant::now() + HANG_GUARD;
    let exited = (0..).find_map(|tick| {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("natural-exit drain should succeed");
        terminal_drain.client_egress.extend(drained.client_egress);
        terminal_drain.observations.extend(drained.observations);
        terminal_drain.backpressure.extend(drained.backpressure);
        let changes = daemon.lifecycle_changes(&running.cursor);
        let observed_exit = changes.changes.iter().any(|change| {
            matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
                        && matches!(
                            record.lifecycle,
                            Some(SessionLifecycleState::Exited { code: Some(0) })
                        )
            )
        });
        if observed_exit {
            Some(changes)
        } else {
            assert!(Instant::now() < tick_deadline, "the natural exit publishing its lifecycle upsert did not arrive within {HANG_GUARD:?}");
            // timer: deadline — HANG_GUARD bounds this wait
            let _ = daemon.wait_wakes(tick_deadline.saturating_duration_since(Instant::now()));
            None
        }
    });
    let exited = exited.expect("natural exit should publish one terminal lifecycle upsert");
    assert!(terminal_output(&terminal_drain.client_egress).contains("echo:finish"));
    assert_eq!(exited.changes.len(), 1);
    assert!(
        !lifecycle_worker_root.exists(),
        "natural terminal transition should remove the empty worker root"
    );

    let empty = daemon.lifecycle_changes(&exited.cursor);
    assert!(empty.changes.is_empty());
    assert!(empty.resync_required.is_none());
    let _ = daemon
        .drain(&session_id, 200)
        .expect("repeat terminal drain should remain reachable");
    assert!(daemon.lifecycle_changes(&exited.cursor).changes.is_empty());

    assert!(daemon
        .remove_session(&session_id)
        .expect("host should explicitly forget the terminal session"));
    let removed = daemon.lifecycle_changes(&exited.cursor);
    assert_eq!(removed.changes.len(), 1);
    assert!(matches!(
        &removed.changes[0].kind,
        SessionLifecycleChangeKind::Removed { session_id: removed_id }
            if removed_id == &session_id
    ));
    assert!(daemon
        .lifecycle_baseline()
        .expect("post-removal baseline")
        .sessions
        .is_empty());
    assert!(matches!(
        daemon.drain(&session_id, 201),
        Err(CoreDaemonError::UnknownSession(id)) if id == session_id
    ));

    daemon
        .spawn(spawn_request(&session_id), 210)
        .expect("same stable id should be reusable after complete removal");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("lifecycle-source-reused-subscription".to_string()),
            211,
        )
        .expect("removed subscription state must not block a fresh attach");
    let fresh = drain_until_attached(&mut daemon, &session_id, &client_id);
    assert!(!terminal_output(&fresh.client_egress).contains("echo:finish"));
    assert!(fresh.observations.iter().all(|observation| !matches!(
        observation,
        BotsterEngineObservation::Subscription(
            botster_core::SubscriptionMultiplexerObservation::ClientStream {
                observation: botster_core::ClientStreamObservation::ReplacedSubscription { .. },
                ..
            }
        )
    )));

    daemon
        .shutdown(Some(session_id.clone()), 220)
        .expect("fresh worker should shut down cleanly");
    assert!(daemon
        .remove_session(&session_id)
        .expect("fresh terminal worker should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_observe_advances_exit_without_attach_or_drain() {
    let data_dir = temp_data_dir("lifecycle-observe-zero-client");
    let session_id = SessionId("lifecycle-observe-zero-client".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(8),
    );
    let baseline = daemon
        .lifecycle_baseline()
        .expect("empty observe baseline")
        .cursor;
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("zero-client fixture should spawn");
    let running = daemon
        .lifecycle_changes_page(&baseline, 8, 16 * 1024)
        .expect("spawn page");
    assert_successful_page_within_budget(&running, 16 * 1024);
    assert_eq!(running.changes.len(), 1);

    let exited = observe_until_exited(&mut daemon, &session_id, &running.next, 20);
    assert_successful_page_within_budget(&exited, 16 * 1024);
    assert!(journal_advanced(&mut daemon));
    assert!(matches!(
        daemon
            .list()
            .expect("registry after observe")
            .first()
            .map(|session| session.registry_state.clone()),
        Some(RegistrySessionState::Exited)
    ));

    daemon
        .shutdown(Some(session_id), 40)
        .expect("exited worker should shut down");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_dropped_wake_still_converges_by_page() {
    let data_dir = temp_data_dir("lifecycle-dropped-wake");
    let session_id = SessionId("lifecycle-dropped-wake".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let baseline = daemon
        .lifecycle_baseline()
        .expect("dropped-wake baseline")
        .cursor;
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("dropped-wake spawn");
    let _ = journal_advanced(&mut daemon);
    let exited = observe_until_exited(&mut daemon, &session_id, &baseline, 20);
    let _discarded = journal_advanced(&mut daemon);
    let later = daemon
        .lifecycle_changes_page(&baseline, 8, 16 * 1024)
        .expect("later page after discarded wake");
    assert_successful_page_within_budget(&later, 16 * 1024);
    assert!(page_contains_exited(&later, &session_id));
    assert!(page_contains_exited(&exited, &session_id));
    daemon
        .shutdown(Some(session_id), 40)
        .expect("dropped-wake shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn lifecycle_wakes_coalesce_and_page_does_not_clear_them() {
    let data_dir = temp_data_dir("lifecycle-wake-coalesce");
    let session_id = SessionId("lifecycle-wake-coalesce".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    assert!(!journal_advanced(&mut daemon));
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("first append sets the wake");
    // Only a subscribed client may resize (an unsubscribed one is refused,
    // typed).
    daemon
        .attach(
            ClientId("wake-client".to_string()),
            session_id.clone(),
            SubscriptionId("wake-sub".to_string()),
            11,
        )
        .expect("attach before resize");
    daemon
        .resize(
            ClientId("wake-client".to_string()),
            session_id.clone(),
            25,
            80,
            11,
        )
        .expect("second append stays one bit");
    let cursor = daemon.lifecycle_baseline().expect("wake baseline").cursor;
    let _ = daemon
        .lifecycle_changes_page(&cursor, 8, 16 * 1024)
        .expect("page must not clear the wake");
    assert!(journal_advanced(&mut daemon));
    assert!(!journal_advanced(&mut daemon));
    daemon
        .shutdown(Some(session_id), 20)
        .expect("wake fixture shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn lifecycle_page_stops_on_item_count_and_encoded_bytes() {
    let data_dir = temp_data_dir("lifecycle-page-limits");
    let first = SessionId("lifecycle-page-a".to_string());
    let second = SessionId("lifecycle-page-b".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_lifecycle_journal_capacity(8));
    let baseline = daemon
        .lifecycle_baseline()
        .expect("page-limit baseline")
        .cursor;
    daemon
        .spawn(spawn_request(&first), 10)
        .expect("first spawn");
    daemon
        .spawn(spawn_request(&second), 11)
        .expect("second spawn");

    let one = daemon
        .lifecycle_changes_page(&baseline, 1, 16 * 1024)
        .expect("item-count stop");
    assert_successful_page_within_budget(&one, 16 * 1024);
    assert_eq!(one.changes.len(), 1);
    assert_ne!(one.next, one.source_watermark);

    let first_change_bytes = serde_json::to_vec(&one).expect("encode one-change page");
    let two = daemon
        .lifecycle_changes_page(&baseline, 8, 16 * 1024)
        .expect("both changes");
    assert_successful_page_within_budget(&two, 16 * 1024);
    assert_eq!(two.changes.len(), 2);
    let two_bytes = serde_json::to_vec(&two).expect("encode two-change page");
    assert!(two_bytes.len() > first_change_bytes.len());
    let byte_stopped = daemon
        .lifecycle_changes_page(&baseline, 8, two_bytes.len() - 1)
        .expect("encoded-page stop");
    assert_successful_page_within_budget(&byte_stopped, two_bytes.len() - 1);
    assert_eq!(byte_stopped.changes.len(), 1);

    daemon.shutdown(Some(first), 20).expect("first shutdown");
    daemon.shutdown(Some(second), 21).expect("second shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn lifecycle_page_expired_cursor_resyncs_before_budget() {
    let data_dir = temp_data_dir("lifecycle-page-expired");
    let session_id = SessionId("lifecycle-page-expired".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_lifecycle_journal_capacity(1));
    let baseline = daemon
        .lifecycle_baseline()
        .expect("expired baseline")
        .cursor;
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("first append");
    // Only a subscribed client may resize (an unsubscribed one is refused,
    // typed).
    daemon
        .attach(
            ClientId("expired-client".to_string()),
            session_id.clone(),
            SubscriptionId("expired-sub".to_string()),
            11,
        )
        .expect("attach before resize");
    daemon
        .resize(
            ClientId("expired-client".to_string()),
            session_id.clone(),
            26,
            80,
            11,
        )
        .expect("second append evicts the first");
    let page = daemon
        .lifecycle_changes_page(&baseline, 0, 0)
        .expect("expired resync wins over zero budget");
    assert!(page.changes.is_empty());
    assert!(matches!(
        page.resync_required,
        Some(SessionLifecycleResyncReason::CursorExpired { .. })
    ));
    daemon
        .shutdown(Some(session_id), 20)
        .expect("expired fixture shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn observe_slice_error_messages_use_a_json_safe_alphabet() {
    let cases = [
        "a".repeat(300),
        "\u{0000}".repeat(256),
        r#"quote " and backslash \"#.to_string(),
        "controls \n\t\r\u{0007}".to_string(),
        "multibyte café 日本語".to_string(),
    ];
    for raw in cases {
        let sanitized = sanitize_observe_slice_error_message(&raw);
        assert!(sanitized.len() <= OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES);
        assert!(
            sanitized
                .bytes()
                .all(botster_core_daemon::is_observe_slice_error_message_byte),
            "unsafe alphabet survived sanitization: {sanitized:?}"
        );
        let session_id = SessionId("escape-session".to_string());
        let actual = botster_core_daemon::ObserveLifecycleSliceError {
            session_id: session_id.clone(),
            message: sanitized,
        };
        let reserved = reserved_observe_slice_error(session_id);
        let actual_len = serde_json::to_vec(&actual).expect("actual error").len();
        let reserved_len = serde_json::to_vec(&reserved).expect("reserved error").len();
        assert!(
            actual_len <= reserved_len,
            "encoded actual {actual_len} exceeded reserved {reserved_len} for {raw:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn observe_slice_resumes_after_item_budget_without_revisiting() {
    let data_dir = temp_data_dir("lifecycle-observe-slice-resume");
    let first = SessionId("a-slice-resume".to_string());
    let second = SessionId("b-slice-resume".to_string());
    let third = SessionId("c-slice-resume".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    for session_id in [&first, &second, &third] {
        daemon
            .spawn(spawn_request(session_id), 10)
            .expect("slice resume spawn");
    }
    let first_slice = daemon
        .observe_lifecycle_slice(11, None, observe_item_budget(1))
        .expect("first slice");
    assert_eq!(first_slice.last_visited.as_ref(), Some(&first));
    assert_eq!(first_slice.stop, ObserveLifecycleStop::SessionBudget);
    let second_slice = daemon
        .observe_lifecycle_slice(
            12,
            Some(&observe_resume(&first_slice)),
            observe_item_budget(1),
        )
        .expect("resume slice");
    assert_eq!(second_slice.last_visited.as_ref(), Some(&second));
    assert_eq!(second_slice.pass_id, first_slice.pass_id);
    assert_eq!(second_slice.stop, ObserveLifecycleStop::SessionBudget);
    let third_slice = daemon
        .observe_lifecycle_slice(
            13,
            Some(&observe_resume(&second_slice)),
            observe_item_budget(1),
        )
        .expect("final slice");
    assert_eq!(third_slice.last_visited.as_ref(), Some(&third));
    assert_eq!(third_slice.stop, ObserveLifecycleStop::Complete);

    for session_id in [first, second, third] {
        daemon.shutdown(Some(session_id), 20).expect("shutdown");
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_each_budget_stops_remaining_visits() {
    let data_dir = temp_data_dir("lifecycle-observe-slice-budgets");
    let first = SessionId("a-slice-budget".to_string());
    let second = SessionId("b-slice-budget".to_string());
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    daemon
        .spawn(spawn_request(&first), 10)
        .expect("budget first");
    daemon
        .spawn(spawn_request(&second), 11)
        .expect("budget second");

    let empty = CoreDaemon::new(CoreDaemonConfig::new(temp_data_dir(
        "lifecycle-observe-slice-empty",
    )))
    .observe_lifecycle_slice(1, None, observe_item_budget(8))
    .expect("empty slice");
    let empty_bytes = serde_json::to_vec(&empty).expect("encode empty").len();
    match daemon.observe_lifecycle_slice(
        12,
        None,
        ObserveLifecycleBudget {
            max_sessions: 8,
            max_encoded_result_bytes: empty_bytes,
            max_elapsed: Duration::MAX,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => {
            assert!(minimum_bytes > empty_bytes);
        }
        other => panic!("empty envelope must not admit a reserved visit: {other:?}"),
    }

    let one = daemon
        .observe_lifecycle_slice(13, None, observe_item_budget(1))
        .expect("item budget visits one");
    assert_eq!(one.last_visited.as_ref(), Some(&first));
    assert_eq!(one.stop, ObserveLifecycleStop::SessionBudget);

    let timed_out = daemon
        .observe_lifecycle_slice(
            14,
            Some(&observe_resume(&one)),
            ObserveLifecycleBudget {
                max_sessions: 8,
                max_encoded_result_bytes: 16 * 1024,
                max_elapsed: EXPIRED,
            },
        )
        .expect("an expired elapsed budget visits none remaining");
    assert_eq!(timed_out.last_visited.as_ref(), Some(&first));
    assert_eq!(timed_out.stop, ObserveLifecycleStop::Elapsed);

    daemon.shutdown(Some(first), 20).expect("shutdown first");
    daemon.shutdown(Some(second), 21).expect("shutdown second");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_enforces_bytes_on_empty_and_no_visit_results() {
    let empty_data_dir = temp_data_dir("lifecycle-observe-empty-result-budget");
    let mut empty_daemon = CoreDaemon::new(CoreDaemonConfig::new(&empty_data_dir));
    let empty_minimum = match empty_daemon.observe_lifecycle_slice(
        1,
        None,
        ObserveLifecycleBudget {
            max_sessions: 1,
            max_encoded_result_bytes: 0,
            max_elapsed: Duration::MAX,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
        other => panic!("empty slice must enforce its byte budget: {other:?}"),
    };
    let empty = empty_daemon
        .observe_lifecycle_slice(
            2,
            None,
            ObserveLifecycleBudget {
                max_sessions: 1,
                max_encoded_result_bytes: empty_minimum,
                max_elapsed: Duration::MAX,
            },
        )
        .expect("exact empty budget");
    assert_eq!(empty.stop, ObserveLifecycleStop::Complete);
    assert_eq!(
        serde_json::to_vec(&empty).expect("encode").len(),
        empty_minimum
    );

    let data_dir = temp_data_dir("lifecycle-observe-no-visit-result-budget");
    let first = SessionId("a-no-visit-budget".to_string());
    let second = SessionId("b-no-visit-budget".to_string());
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    daemon
        .spawn(spawn_request(&first), 10)
        .expect("first spawn");
    daemon
        .spawn(spawn_request(&second), 11)
        .expect("second spawn");

    // A slice that may visit no session could never advance: it is a typed
    // error whatever its byte budget.
    for max_encoded_result_bytes in [0, 16 * 1024] {
        assert_eq!(
            daemon.observe_lifecycle_slice(
                12,
                None,
                ObserveLifecycleBudget {
                    max_sessions: 0,
                    max_encoded_result_bytes,
                    max_elapsed: Duration::MAX,
                },
            ),
            Err(SessionLifecyclePageError::SessionBudgetZero)
        );
    }
    assert_eq!(
        daemon.observe_lifecycle_slice(
            13,
            None,
            ObserveLifecycleBudget {
                max_sessions: 1,
                max_encoded_result_bytes: 16 * 1024,
                max_elapsed: Duration::ZERO,
            },
        ),
        Err(SessionLifecyclePageError::ElapsedBudgetZero)
    );

    let first_elapsed_minimum = match daemon.observe_lifecycle_slice(
        14,
        None,
        ObserveLifecycleBudget {
            max_sessions: usize::MAX,
            max_encoded_result_bytes: 0,
            max_elapsed: EXPIRED,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
        other => panic!("first elapsed yield must enforce its byte budget: {other:?}"),
    };
    let first_elapsed = daemon
        .observe_lifecycle_slice(
            15,
            None,
            ObserveLifecycleBudget {
                max_sessions: usize::MAX,
                max_encoded_result_bytes: first_elapsed_minimum,
                max_elapsed: EXPIRED,
            },
        )
        .expect("exact first elapsed budget");
    assert_eq!(first_elapsed.stop, ObserveLifecycleStop::Elapsed);
    assert!(first_elapsed.last_visited.is_none());
    assert_eq!(
        serde_json::to_vec(&first_elapsed).expect("encode").len(),
        first_elapsed_minimum
    );

    let progressed = daemon
        .observe_lifecycle_slice(16, None, observe_item_budget(1))
        .expect("progress before resumed yield");
    let resume = observe_resume(&progressed);
    let resumed_minimum = match daemon.observe_lifecycle_slice(
        17,
        Some(&resume),
        ObserveLifecycleBudget {
            max_sessions: usize::MAX,
            max_encoded_result_bytes: 0,
            max_elapsed: EXPIRED,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
        other => panic!("resumed elapsed yield must enforce its byte budget: {other:?}"),
    };
    let resumed = daemon
        .observe_lifecycle_slice(
            18,
            Some(&resume),
            ObserveLifecycleBudget {
                max_sessions: usize::MAX,
                max_encoded_result_bytes: resumed_minimum,
                max_elapsed: EXPIRED,
            },
        )
        .expect("exact resumed elapsed budget preserves the pass");
    assert_eq!(resumed.pass_id, progressed.pass_id);
    assert_eq!(resumed.last_visited, progressed.last_visited);
    assert_eq!(resumed.stop, ObserveLifecycleStop::Elapsed);
    assert_eq!(
        serde_json::to_vec(&resumed).expect("encode").len(),
        resumed_minimum
    );

    daemon.shutdown(Some(first), 20).ok();
    daemon.shutdown(Some(second), 21).ok();
    let _ = fs::remove_dir_all(empty_data_dir);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_dropped_cursor_is_resync_not_a_complete_suffix() {
    let data_dir = temp_data_dir("lifecycle-observe-dropped-cursor");
    let first = SessionId("a-dropped-cursor".to_string());
    let second = SessionId("b-dropped-cursor".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon.spawn(spawn_request(&first), 10).expect("first");
    daemon.spawn(spawn_request(&second), 11).expect("second");
    let partial = daemon
        .observe_lifecycle_slice(12, None, observe_item_budget(1))
        .expect("partial");
    let foreign = ObserveLifecycleCursor {
        pass_id: ObserveLifecyclePassId("foreign-pass".to_string()),
        last_visited: Some(first.clone()),
    };
    let dropped = daemon
        .observe_lifecycle_slice(13, Some(&foreign), observe_item_budget(8))
        .expect("foreign pass");
    assert!(dropped.last_visited.is_none());
    assert_eq!(
        dropped.stop,
        ObserveLifecycleStop::Resync {
            reason: SessionLifecycleResyncReason::ObservePassUnavailable
        }
    );

    let restarted = daemon
        .observe_lifecycle_slice(14, None, observe_item_budget(8))
        .expect("new pass restarts");
    assert_eq!(restarted.stop, ObserveLifecycleStop::Complete);
    assert_eq!(restarted.last_visited.as_ref(), Some(&second));
    assert_ne!(restarted.pass_id, partial.pass_id);

    daemon.shutdown(Some(first), 20).ok();
    daemon.shutdown(Some(second), 21).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_same_pass_cursor_must_match_last_visited() {
    let data_dir = temp_data_dir("lifecycle-observe-cursor-identity");
    let first = SessionId("a-cursor-identity".to_string());
    let second = SessionId("b-cursor-identity".to_string());
    let third = SessionId("c-cursor-identity".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    for session_id in [&first, &second, &third] {
        daemon
            .spawn(spawn_request(session_id), 10)
            .expect("identity spawn");
    }
    let first_slice = daemon
        .observe_lifecycle_slice(11, None, observe_item_budget(1))
        .expect("first");
    let second_slice = daemon
        .observe_lifecycle_slice(
            12,
            Some(&observe_resume(&first_slice)),
            observe_item_budget(1),
        )
        .expect("second");
    assert_eq!(second_slice.last_visited.as_ref(), Some(&second));

    let stale_earlier = ObserveLifecycleCursor {
        pass_id: second_slice.pass_id.clone(),
        last_visited: Some(first.clone()),
    };
    let earlier = daemon
        .observe_lifecycle_slice(13, Some(&stale_earlier), observe_item_budget(8))
        .expect("stale earlier");
    assert!(earlier.last_visited.is_none());
    assert_eq!(
        earlier.stop,
        ObserveLifecycleStop::Resync {
            reason: SessionLifecycleResyncReason::ObservePassUnavailable
        }
    );

    let forged_later = ObserveLifecycleCursor {
        pass_id: second_slice.pass_id.clone(),
        last_visited: Some(third.clone()),
    };
    let later = daemon
        .observe_lifecycle_slice(14, Some(&forged_later), observe_item_budget(8))
        .expect("forged later");
    assert!(later.last_visited.is_none());
    assert_eq!(
        later.stop,
        ObserveLifecycleStop::Resync {
            reason: SessionLifecycleResyncReason::ObservePassUnavailable
        }
    );

    let resumed = daemon
        .observe_lifecycle_slice(
            15,
            Some(&observe_resume(&second_slice)),
            observe_item_budget(8),
        )
        .expect("honest resume still works");
    assert_eq!(resumed.stop, ObserveLifecycleStop::Complete);
    assert_eq!(resumed.last_visited.as_ref(), Some(&third));

    for session_id in [first, second, third] {
        daemon.shutdown(Some(session_id), 20).ok();
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn lifecycle_baseline_pages_reconstruct_the_full_snapshot() {
    let data_dir = temp_data_dir("lifecycle-baseline-pages");
    let first = SessionId("a-baseline-page".to_string());
    let second = SessionId("b-baseline-page".to_string());
    let third = SessionId("c-baseline-page".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    for session_id in [&first, &second, &third] {
        daemon
            .spawn(spawn_request(session_id), 10)
            .expect("baseline spawn");
    }
    let full = daemon.lifecycle_baseline().expect("full baseline");
    let mut rows = Vec::new();
    let mut snapshot = None;
    let mut after = None;
    loop {
        let page = daemon
            .lifecycle_baseline_page(snapshot.as_ref(), after.as_ref(), baseline_item_budget(1))
            .expect("baseline page");
        rows.extend(page.sessions.iter().cloned());
        if page.stop == LifecycleBaselineStop::Complete {
            assert!(page.next.is_none());
            assert_eq!(rows, full.sessions);
            break;
        }
        assert_eq!(page.stop, LifecycleBaselineStop::RowBudget);
        snapshot = Some(page.snapshot_sequence);
        after = page.next;
    }

    daemon.shutdown(Some(first), 20).ok();
    daemon.shutdown(Some(second), 21).ok();
    daemon.shutdown(Some(third), 22).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// Begin one operation that completes synchronously on the in-process
/// engine and return its completion.
fn complete_now(
    daemon: &mut CoreDaemon,
    operation: botster_core_daemon::CoreOperation,
) -> botster_core_daemon::CoreCompletion {
    let id = daemon.begin(operation).expect("begin the operation");
    let mut completions = daemon.take_completions();
    let index = completions
        .iter()
        .position(|completion| completion.id() == id)
        .expect("the operation completed synchronously");
    completions.swap_remove(index)
}

/// Wait for the session's PTY child to be gone. The registry row reads
/// Exited when the exit commits; the child's process group can still probe
/// as present for a moment after that, and `release_ended_session` then
/// keeps the admission entry (`false`). The exit event of the child is the
/// point after which a release can succeed.
fn wait_pty_child_gone(daemon: &CoreDaemon, session_id: &SessionId) {
    let pid = daemon
        .registry()
        .load(session_id)
        .expect("load the ended record")
        .expect("the ended record")
        .process
        .and_then(|process| process.pid)
        .expect("the ended record keeps the PTY child pid");
    assert!(
        wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "the PTY child {pid} is gone"
    );
}

/// Spawn a session that waits on a gate and prints nothing, attach a route
/// while it runs, release the gate so it exits, and wait until its registry row is Exited
/// and the child is gone. Returns the route's generation.
fn spawn_until_exited(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
    subscription_id: &SubscriptionId,
) -> TerminalSubscriptionGeneration {
    let fixture = GatedOutputExit::new();
    daemon
        .spawn(
            gated_output_exit_spawn_request(session_id, &fixture, "''"),
            10,
        )
        .expect("spawn the first run");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach the first run");
    let generation = daemon
        .terminal_subscription_generation(session_id, subscription_id)
        .expect("the first run's route generation");
    fixture.release();
    on_wakes_until(
        daemon,
        "the first run's exit committed",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let _ = daemon.observe_lifecycle(12).expect("observe");
            matches!(
                daemon.session_registry_state(session_id).expect("state"),
                SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
            )
            .then_some(())
        },
    );
    wait_pty_child_gone(daemon, session_id);
    generation
}

/// An ended session is released and spawned again under the same id: the
/// row goes from Exited to Running in one Upsert, nothing is Removed, the
/// route gets a fresh generation, and an unacknowledged envelope reaches the
/// new run.
#[cfg(unix)]
#[test]
fn a_released_ended_session_respawns_in_place_under_the_same_id() {
    use botster_core_daemon::operation::ReservedSpawnResult;
    use botster_core_daemon::{CoreCompletion, CoreOperation};
    let data_dir = temp_data_dir("release-ended-respawn");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("respawned-session".to_string());
    let client_id = ClientId("respawn-client".to_string());
    let subscription_id = SubscriptionId("respawn-route".to_string());
    let first_generation =
        spawn_until_exited(&mut daemon, &session_id, &client_id, &subscription_id);
    daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope("env-inbox", vec![session_target(&session_id)]),
        })
        .expect("queue a message for the ended session");
    let cursor = daemon.lifecycle_baseline().expect("baseline").cursor;

    match complete_now(
        &mut daemon,
        CoreOperation::ReleaseEndedSession(session_id.clone()),
    ) {
        CoreCompletion::ReleaseEndedSession { result, .. } => {
            assert!(result.expect("release"), "an ended session is released");
        }
        other => panic!("unexpected completion {other:?}"),
    }
    let reservation = match complete_now(
        &mut daemon,
        CoreOperation::ReserveSession(session_id.clone()),
    ) {
        CoreCompletion::ReserveSession { result, .. } => {
            result.expect("the released id can be reserved again")
        }
        other => panic!("unexpected completion {other:?}"),
    };
    match complete_now(
        &mut daemon,
        CoreOperation::SpawnReserved {
            reservation,
            request: spawn_request(&session_id),
        },
    ) {
        CoreCompletion::SpawnReserved {
            result: ReservedSpawnResult::Installed { .. },
            ..
        } => {}
        other => panic!("the respawn must install: {other:?}"),
    }

    let changes = daemon.lifecycle_changes(&cursor);
    assert!(changes.resync_required.is_none());
    assert!(
        changes
            .changes
            .iter()
            .all(|change| !matches!(change.kind, SessionLifecycleChangeKind::Removed { .. })),
        "a respawn journals no Removed: {:?}",
        changes.changes
    );
    assert!(changes.changes.iter().any(|change| matches!(
        &change.kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
    )));
    assert_eq!(
        daemon.session_registry_state(&session_id).expect("state"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Running)
    );
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            20,
        )
        .expect("attach the new run");
    let second_generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("the new run's route generation");
    assert!(second_generation > first_generation, "a fresh generation");
    let inbox = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: session_target(&session_id),
            after: None,
            limit: 8,
        })
        .expect("drain the session inbox");
    assert_eq!(
        inbox
            .envelopes
            .iter()
            .map(|e| e.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-inbox"],
        "the unacknowledged envelope survives the restart"
    );
    daemon.shutdown(Some(session_id), 30).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// Reserve `session_id` and spawn `request` into that explicit reservation.
fn reserve_and_spawn(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    request: SpawnSessionRequest,
) {
    use botster_core_daemon::operation::ReservedSpawnResult;
    use botster_core_daemon::{CoreCompletion, CoreOperation};
    let reservation = match complete_now(daemon, CoreOperation::ReserveSession(session_id.clone()))
    {
        CoreCompletion::ReserveSession { result, .. } => result.expect("the id can be reserved"),
        other => panic!("unexpected completion {other:?}"),
    };
    match complete_now(
        daemon,
        CoreOperation::SpawnReserved {
            reservation,
            request,
        },
    ) {
        CoreCompletion::SpawnReserved {
            result: ReservedSpawnResult::Installed { .. },
            ..
        } => {}
        other => panic!("the reserved spawn must install: {other:?}"),
    }
}

fn wait_registry_exited(daemon: &mut CoreDaemon, session_id: &SessionId) {
    on_wakes_until(
        daemon,
        "the run's exit committed",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let _ = daemon.observe_lifecycle(12).expect("observe");
            matches!(
                daemon.session_registry_state(session_id).expect("state"),
                SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
            )
            .then_some(())
        },
    );
    wait_pty_child_gone(daemon, session_id);
}

/// A session whose runs all start through ReserveSession and SpawnReserved
/// holds an explicit reservation. Release frees it, so the documented
/// sequence restarts the same id again and again.
#[cfg(unix)]
#[test]
fn an_explicitly_reserved_session_restarts_in_place_repeatedly() {
    let data_dir = temp_data_dir("release-explicit-respawn");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("explicit-respawn-session".to_string());
    reserve_and_spawn(
        &mut daemon,
        &session_id,
        immediate_exit_spawn_request(&session_id),
    );
    for run in 1..=2 {
        wait_registry_exited(&mut daemon, &session_id);
        assert!(
            daemon
                .release_ended_session(&session_id)
                .expect("release the ended run"),
            "run {run}: the explicit reservation is released"
        );
        reserve_and_spawn(
            &mut daemon,
            &session_id,
            immediate_exit_spawn_request(&session_id),
        );
    }
    wait_registry_exited(&mut daemon, &session_id);
    let _ = fs::remove_dir_all(data_dir);
}

/// Pump every wake until a pump outcome satisfies `done`.
fn pump_until_outcome(
    daemon: &mut CoreDaemon,
    label: &str,
    mut done: impl FnMut(&botster_core_daemon::PumpWokenOutcome) -> bool,
) {
    wait_for(label, REAL_WORKER_COMPLETION_TIMEOUT, |remaining| {
        // timer: deadline — wait_for's bound limits this wait
        let batch = daemon.wait_wakes(remaining);
        let outcome = daemon.pump_woken(&batch, 11).expect("pump");
        done(&outcome).then_some(())
    });
}

/// Output of a session with no attached route advances its edges; the
/// host learns it from the pump outcome and reads the counters exactly.
#[cfg(unix)]
#[test]
fn output_of_an_unattached_session_advances_its_edges() {
    let data_dir = temp_data_dir("edges-unattached-output");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("edges-output-session".to_string());
    daemon.spawn(spawn_request(&session_id), 10).expect("spawn");

    pump_until_outcome(&mut daemon, "the session's output advanced", |outcome| {
        outcome.output_advanced.contains(&session_id)
    });

    let edges = daemon
        .session_edges(&session_id)
        .expect("read the edges")
        .expect("a spawned session has a record");
    assert!(edges.output_seq >= 1, "{edges:?}");
    assert_eq!(
        (edges.input_seq, edges.composing),
        (0, false),
        "no client input yet"
    );
    assert_eq!(
        daemon
            .session_edges(&SessionId("never-spawned".to_string()))
            .expect("read an unknown id"),
        None
    );
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// A mode change advances modes_epoch, and the query carries the flags.
#[cfg(unix)]
#[test]
fn a_mode_change_advances_modes_epoch_with_its_flags() {
    let data_dir = temp_data_dir("edges-mode-change");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("edges-modes-session".to_string());
    let mut request = spawn_request(&session_id);
    // Hide the cursor, then wait on input.
    request.request.arguments[1] =
        "printf '\\033[?25l'; while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn");

    pump_until_outcome(&mut daemon, "the session's modes advanced", |outcome| {
        outcome.modes_advanced.contains(&session_id)
    });

    let edges = daemon
        .session_edges(&session_id)
        .expect("read the edges")
        .expect("a record");
    assert!(edges.modes_epoch >= 1);
    assert_eq!(
        edges.mode_flags.map(|flags| flags.cursor_visible),
        Some(false),
        "the query carries the changed flags"
    );
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// Release keeps an ended id's counters for a respawn; removing the id
/// after that release still drops them, though the engine holds no session
/// for it any more.
#[cfg(unix)]
#[test]
fn removing_a_released_session_drops_its_edge_counters() {
    let data_dir = temp_data_dir("edges-release-remove");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("edges-release-remove-session".to_string());
    daemon.spawn(spawn_request(&session_id), 10).expect("spawn");
    pump_until_outcome(&mut daemon, "the run's output advanced", |outcome| {
        outcome.output_advanced.contains(&session_id)
    });
    assert!(
        daemon
            .session_edges(&session_id)
            .expect("read")
            .expect("a record")
            .output_seq
            > 0
    );
    daemon.shutdown(Some(session_id.clone()), 20).ok();
    wait_registry_exited(&mut daemon, &session_id);
    assert!(daemon.release_ended_session(&session_id).expect("release"));
    assert!(daemon.remove_session(&session_id).expect("remove"));

    daemon
        .spawn(spawn_request(&session_id), 30)
        .expect("spawn the removed id again");
    let fresh = daemon
        .session_edges(&session_id)
        .expect("read")
        .expect("a record");
    assert_eq!(
        (fresh.output_seq, fresh.modes_epoch),
        (0, 0),
        "the removed run's counters do not carry over: {fresh:?}"
    );
    daemon.shutdown(Some(session_id), 40).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// The counters follow the id: a release keeps them for the respawn, and
/// removal drops them.
#[cfg(unix)]
#[test]
fn session_edges_continue_across_a_respawn_and_end_at_removal() {
    let data_dir = temp_data_dir("edges-lifetime");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("edges-lifetime-session".to_string());
    spawn_until_exited(
        &mut daemon,
        &session_id,
        &ClientId("edges-client".to_string()),
        &SubscriptionId("edges-route".to_string()),
    );
    let before = daemon
        .session_edges(&session_id)
        .expect("read")
        .expect("an ended session keeps its record");

    assert!(daemon.release_ended_session(&session_id).expect("release"));
    assert_eq!(
        daemon.session_edges(&session_id).expect("read"),
        Some(before.clone()),
        "a release keeps the counters for the next run"
    );
    daemon
        .spawn(spawn_request(&session_id), 20)
        .expect("respawn");
    pump_until_outcome(&mut daemon, "the new run's output advanced", |outcome| {
        outcome.output_advanced.contains(&session_id)
    });
    let after = daemon
        .session_edges(&session_id)
        .expect("read")
        .expect("a record");
    assert!(
        after.output_seq > before.output_seq,
        "the respawn continues the output sequence: {before:?} then {after:?}"
    );

    daemon.shutdown(Some(session_id.clone()), 30).ok();
    wait_registry_exited(&mut daemon, &session_id);
    assert!(daemon.remove_session(&session_id).expect("remove"));
    assert_eq!(
        daemon.session_edges(&session_id).expect("read"),
        None,
        "removal drops the record"
    );
    // After a removal the id is a new entity: a fresh spawn starts at 0.
    daemon
        .spawn(spawn_request(&session_id), 40)
        .expect("spawn again");
    assert_eq!(
        daemon
            .session_edges(&session_id)
            .expect("read")
            .expect("a record")
            .output_seq,
        0,
        "the removed run's counters do not carry over"
    );
    daemon.shutdown(Some(session_id), 50).ok();
    let _ = fs::remove_dir_all(data_dir);
}

/// Release refuses a live session, like remove_session. A respawn that
/// fails after release leaves the row ended, and removal still works.
#[cfg(unix)]
#[test]
fn a_failed_respawn_after_release_leaves_the_row_ended_and_removable() {
    use botster_core_daemon::operation::ReservedSpawnResult;
    use botster_core_daemon::{CoreCompletion, CoreOperation};
    let data_dir = temp_data_dir("release-ended-failed-respawn");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let live = SessionId("still-live-session".to_string());
    daemon
        .spawn(spawn_request(&live), 5)
        .expect("spawn a live session");
    assert!(
        !daemon
            .release_ended_session(&live)
            .expect("release a live session"),
        "a live session is not released"
    );

    let session_id = SessionId("failed-respawn-session".to_string());
    spawn_until_exited(
        &mut daemon,
        &session_id,
        &ClientId("failed-client".to_string()),
        &SubscriptionId("failed-route".to_string()),
    );
    assert!(daemon.release_ended_session(&session_id).expect("release"));
    let reservation = match complete_now(
        &mut daemon,
        CoreOperation::ReserveSession(session_id.clone()),
    ) {
        CoreCompletion::ReserveSession { result, .. } => result.expect("reserve"),
        other => panic!("unexpected completion {other:?}"),
    };
    let mut failing = spawn_request(&session_id);
    failing.request.executable = "/nonexistent/botster-respawn-binary".to_string();
    match complete_now(
        &mut daemon,
        CoreOperation::SpawnReserved {
            reservation: reservation.clone(),
            request: failing,
        },
    ) {
        CoreCompletion::SpawnReserved {
            result: ReservedSpawnResult::Installed { .. },
            ..
        } => panic!("a missing executable must not install"),
        CoreCompletion::SpawnReserved { .. } => {}
        other => panic!("unexpected completion {other:?}"),
    }
    let _ = complete_now(
        &mut daemon,
        CoreOperation::ReleaseSessionReservation(reservation),
    );

    assert_eq!(
        daemon.session_registry_state(&session_id).expect("state"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited),
        "the failed respawn leaves the row ended"
    );
    assert!(
        daemon.remove_session(&session_id).expect("remove"),
        "the ended row is still removable"
    );
    daemon.shutdown(Some(live), 30).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn lifecycle_baseline_pages_ignore_observe_mutations() {
    let data_dir = temp_data_dir("lifecycle-baseline-freeze");
    let first = SessionId("a-baseline-freeze".to_string());
    let second = SessionId("b-baseline-freeze".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon.spawn(spawn_request(&first), 10).expect("first");
    daemon
        .spawn(immediate_exit_spawn_request(&second), 11)
        .expect("second");
    let mut snapshot = None;
    let mut after = None;
    let first_page = loop {
        let page = daemon
            .lifecycle_baseline_page(snapshot.as_ref(), after.as_ref(), baseline_item_budget(1))
            .expect("mint");
        assert!(!matches!(page.stop, LifecycleBaselineStop::Resync { .. }));
        snapshot = Some(page.snapshot_sequence.clone());
        if !page.sessions.is_empty() || page.stop == LifecycleBaselineStop::Complete {
            break page;
        }
        after = page.next;
    };
    assert_eq!(first_page.stop, LifecycleBaselineStop::RowBudget);
    let snapshot = first_page.snapshot_sequence.clone();
    // Observe after the mint until it commits the second session's exit, so
    // the frozen page below must ignore a real mutation.
    on_wakes_until(
        &mut daemon,
        "observe committing the second session's exit",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let _ = daemon.observe_lifecycle(20).expect("observe after mint");
            matches!(
                daemon
                    .session_registry_state(&second)
                    .expect("second registry state"),
                SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
            )
            .then_some(())
        },
    );
    let second_page = daemon
        .lifecycle_baseline_page(
            Some(&snapshot),
            first_page.next.as_ref(),
            baseline_item_budget(8),
        )
        .expect("frozen second page");
    assert_eq!(second_page.stop, LifecycleBaselineStop::Complete);
    assert_eq!(second_page.sessions.len(), 1);
    assert_eq!(second_page.sessions[0].session.session_id, second);
    assert_eq!(
        second_page.sessions[0].session.registry_state,
        RegistrySessionState::Running,
        "freeze must not absorb post-mint observe upserts"
    );

    let unknown = daemon
        .lifecycle_baseline_page(Some(&snapshot), None, baseline_item_budget(8))
        .expect("dropped freeze");
    assert!(unknown.sessions.is_empty());
    assert_eq!(
        unknown.stop,
        LifecycleBaselineStop::Resync {
            reason: SessionLifecycleResyncReason::SnapshotUnavailable
        }
    );

    daemon.shutdown(Some(first), 30).ok();
    daemon.shutdown(Some(second), 31).ok();
    let _ = fs::remove_dir_all(data_dir);
}

fn baseline_item_budget(max_rows: usize) -> LifecycleBaselineBudget {
    LifecycleBaselineBudget {
        max_rows,
        max_bytes: 64 * 1024,
        max_elapsed: Duration::MAX,
    }
}

fn seed_registry_records(daemon: &CoreDaemon, ids: &[&SessionId], now: u64) {
    for session_id in ids {
        let record = botster_core_daemon::RegistryRecord::running(
            (*session_id).clone(),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "seed".to_string(),
            now,
        );
        daemon
            .registry()
            .save(&record)
            .expect("seed registry record");
    }
}

fn assemble_baseline_pages(
    daemon: &mut CoreDaemon,
    snapshot: Option<SessionLifecycleCursor>,
    budget: LifecycleBaselineBudget,
) -> (
    SessionLifecycleCursor,
    Vec<botster_core_daemon::SessionLifecycleRecord>,
) {
    let mut rows = Vec::new();
    let mut snapshot = snapshot;
    let mut after = None;
    loop {
        let page = daemon
            .lifecycle_baseline_page(snapshot.as_ref(), after.as_ref(), budget)
            .expect("baseline page");
        assert!(!matches!(page.stop, LifecycleBaselineStop::Resync { .. }));
        snapshot = Some(page.snapshot_sequence.clone());
        rows.extend(page.sessions.iter().cloned());
        if page.stop == LifecycleBaselineStop::Complete {
            return (page.snapshot_sequence, rows);
        }
        after = page.next;
    }
}

#[test]
fn lifecycle_baseline_pages_preserve_colliding_sanitizer_ids_with_digest_filenames() {
    let data_dir = temp_data_dir("baseline-registry-identities");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let ids = [
        SessionId("audit:a".to_string()),
        SessionId("audit_a".to_string()),
    ];
    seed_registry_records(&daemon, &[&ids[0], &ids[1]], 1);
    let mut snapshot = None;
    let mut after = None;
    let mut rows = Vec::new();
    let mut complete = false;
    for _ in 0..16 {
        let page = daemon
            .lifecycle_baseline_page(snapshot.as_ref(), after.as_ref(), baseline_item_budget(1))
            .expect("one-item baseline page");
        assert!(!matches!(page.stop, LifecycleBaselineStop::Resync { .. }));
        assert!(
            page.sessions.len() <= 1,
            "one-item budget must remain bounded"
        );
        if let Some(ref expected) = snapshot {
            assert_eq!(&page.snapshot_sequence, expected);
        }
        snapshot = Some(page.snapshot_sequence);
        let stop = page.stop.clone();
        rows.extend(page.sessions);
        if stop == LifecycleBaselineStop::Complete {
            complete = true;
            break;
        }
        after = page.next;
    }
    assert!(
        complete,
        "two identities must complete within bounded pages"
    );
    assert_eq!(
        rows.iter()
            .map(|row| &row.session.session_id)
            .collect::<Vec<_>>(),
        ids.iter().collect::<Vec<_>>()
    );
    drop(daemon);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_rejects_foreign_registry_identity_as_source_changed() {
    let data_dir = temp_data_dir("baseline-foreign-identity");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let ids = [
        SessionId("audit:a".to_string()),
        SessionId("audit_a".to_string()),
    ];
    seed_registry_records(&daemon, &[&ids[0], &ids[1]], 1);
    let path = registry_fixture_path(&data_dir, &ids[0]);
    let foreign =
        fs::read(registry_fixture_path(&data_dir, &ids[1])).expect("foreign fixture bytes");
    fs::write(&path, &foreign).expect("place foreign identity at first path");
    let page = daemon
        .lifecycle_baseline_page(None, None, baseline_item_budget(8))
        .expect("baseline reports resync");
    assert_eq!(
        page.stop,
        LifecycleBaselineStop::Resync {
            reason: SessionLifecycleResyncReason::SourceChanged
        }
    );
    assert!(page.sessions.is_empty());
    assert_eq!(fs::read(path).expect("foreign bytes remain"), foreign);
    drop(daemon);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_setup_only_elapsed_keeps_freeze_identity() {
    let data_dir = temp_data_dir("lifecycle-baseline-setup-only");
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    let first = SessionId("a-setup-only".to_string());
    let second = SessionId("b-setup-only".to_string());
    seed_registry_records(&daemon, &[&first, &second], 1);
    let page = daemon
        .lifecycle_baseline_page(
            None,
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: 64 * 1024,
                max_elapsed: EXPIRED,
            },
        )
        .expect("setup-only mint");
    assert_eq!(page.stop, LifecycleBaselineStop::Elapsed);
    assert!(page.sessions.is_empty());
    assert!(page.next.is_none());
    let (snapshot, rows) = assemble_baseline_pages(
        &mut daemon,
        Some(page.snapshot_sequence.clone()),
        baseline_item_budget(8),
    );
    assert_eq!(snapshot, page.snapshot_sequence);
    assert_eq!(
        rows.iter()
            .map(|record| record.session.session_id.clone())
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_spawn_after_open_is_excluded() {
    let data_dir = temp_data_dir("lifecycle-baseline-spawn-fence");
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    let first = SessionId("a-spawn-fence".to_string());
    let second = SessionId("b-spawn-fence".to_string());
    seed_registry_records(&daemon, &[&first, &second], 1);
    let minted = daemon
        .lifecycle_baseline_page(
            None,
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: 64 * 1024,
                max_elapsed: EXPIRED,
            },
        )
        .expect("mint before spawn");
    let born = SessionId("c-spawn-fence".to_string());
    daemon
        .spawn(spawn_request(&born), 2)
        .expect("post-mint spawn");
    let (snapshot, rows) = assemble_baseline_pages(
        &mut daemon,
        Some(minted.snapshot_sequence.clone()),
        baseline_item_budget(8),
    );
    assert_eq!(snapshot, minted.snapshot_sequence);
    let ids: Vec<_> = rows
        .iter()
        .map(|record| record.session.session_id.0.clone())
        .collect();
    assert_eq!(ids, vec![first.0, second.0]);
    assert!(!ids.contains(&born.0));
    daemon.shutdown(Some(born), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_remove_before_visit_keeps_pre_change_row() {
    let data_dir = temp_data_dir("lifecycle-baseline-remove-fence");
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    let first = SessionId("a-remove-fence".to_string());
    let second = SessionId("b-remove-fence".to_string());
    for session_id in [&first, &second] {
        let mut record = botster_core_daemon::RegistryRecord::running(
            session_id.clone(),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "seed".to_string(),
            1,
        );
        record.mark(RegistrySessionState::Exited, 1);
        daemon.registry().save(&record).expect("seed exited record");
    }
    let minted = daemon
        .lifecycle_baseline_page(
            None,
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: 64 * 1024,
                max_elapsed: EXPIRED,
            },
        )
        .expect("mint before remove");
    assert!(daemon.remove_session(&second).expect("remove unseen"));
    let (snapshot, rows) = assemble_baseline_pages(
        &mut daemon,
        Some(minted.snapshot_sequence.clone()),
        baseline_item_budget(8),
    );
    assert_eq!(snapshot, minted.snapshot_sequence);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].session.session_id, second);
    assert_eq!(rows[1].session.registry_state, RegistrySessionState::Exited);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_skips_malformed_records_without_blocking_good_rows() {
    let data_dir = temp_data_dir("lifecycle-baseline-malformed");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let good = SessionId("a-good-baseline".to_string());
    seed_registry_records(&daemon, &[&good], 1);
    let bad = SessionId("b-bad-baseline".to_string());
    seed_registry_records(&daemon, &[&bad], 1);
    fs::write(registry_fixture_path(&data_dir, &bad), b"not json")
        .expect("malformed registry fixture");
    let (_, rows) = assemble_baseline_pages(&mut daemon, None, baseline_item_budget(8));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session.session_id, good);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_byte_budget_stops_before_remaining_rows() {
    let data_dir = temp_data_dir("lifecycle-baseline-bytes");
    let mut daemon = CoreDaemon::new(expired_elapsed_config(&data_dir));
    let first = SessionId("a-byte-budget".to_string());
    let second = SessionId("b-byte-budget".to_string());
    seed_registry_records(&daemon, &[&first, &second], 1);
    let setup = daemon
        .lifecycle_baseline_page(
            None,
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: 64 * 1024,
                max_elapsed: EXPIRED,
            },
        )
        .expect("setup mint");
    let minimum = match daemon.lifecycle_baseline_page(
        Some(&setup.snapshot_sequence),
        None,
        LifecycleBaselineBudget {
            max_rows: usize::MAX,
            max_bytes: 0,
            max_elapsed: Duration::MAX,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
        other => panic!("expected BudgetTooSmall, got {other:?}"),
    };
    // The empty page fits `minimum`; the first row does not. The call names
    // the budget of the page with that row, never an empty page at the same
    // position, and that budget returns exactly that row.
    let one_row = match daemon.lifecycle_baseline_page(
        Some(&setup.snapshot_sequence),
        None,
        LifecycleBaselineBudget {
            max_rows: usize::MAX,
            max_bytes: minimum,
            max_elapsed: Duration::MAX,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => {
            assert!(minimum_bytes > minimum);
            minimum_bytes
        }
        other => panic!("a first row that cannot fit must be named, got {other:?}"),
    };
    let indexed = daemon
        .lifecycle_baseline_page(
            Some(&setup.snapshot_sequence),
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: one_row,
                max_elapsed: Duration::MAX,
            },
        )
        .expect("the named budget");
    assert_eq!(indexed.stop, LifecycleBaselineStop::ByteBudget);
    assert_eq!(indexed.sessions.len(), 1);
    assert_eq!(indexed.sessions[0].session.session_id, first);
    let encoded = serde_json::to_vec(&indexed)
        .expect("one-row page must serialize")
        .len();
    assert_eq!(encoded, one_row);
    let (snapshot, rows) = assemble_baseline_pages(
        &mut daemon,
        Some(setup.snapshot_sequence.clone()),
        baseline_item_budget(8),
    );
    assert_eq!(snapshot, setup.snapshot_sequence);
    assert_eq!(rows.len(), 2);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_publishes_zero_client_exit_without_drain() {
    let data_dir = temp_data_dir("lifecycle-observe-slice-exit");
    let session_id = SessionId("slice-zero-client-exit".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("self-exit spawn");
    let after = daemon.lifecycle_baseline().expect("baseline").cursor;
    // Each round runs one whole sliced pass. The child's exit and its PTY
    // end wake the session once visible, so a pass that ran too early is
    // followed by a wake. No drain runs.
    let mut tick = 0;
    on_wakes_until(
        &mut daemon,
        "a sliced observe pass publishing Exited",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let mut resume = None;
            let mut complete = false;
            for _ in 0..50 {
                tick += 1;
                let slice = daemon
                    .observe_lifecycle_slice(20 + tick, resume.as_ref(), observe_item_budget(1))
                    .expect("slice");
                assert!(!matches!(slice.stop, ObserveLifecycleStop::Resync { .. }));
                complete = slice.stop == ObserveLifecycleStop::Complete;
                resume = slice
                    .last_visited
                    .as_ref()
                    .map(|last_visited| ObserveLifecycleCursor {
                        pass_id: slice.pass_id.clone(),
                        last_visited: Some(last_visited.clone()),
                    });
                if complete {
                    break;
                }
            }
            assert!(complete, "sliced observe must finish the pass");
            let page = daemon
                .lifecycle_changes_page(&after, 16, 16 * 1024)
                .expect("page");
            page_contains_exited(&page, &session_id).then_some(())
        },
    );
    daemon.shutdown(Some(session_id), 40).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_then_drain_still_delivers_terminal_process_exited() {
    let data_dir = temp_data_dir("lifecycle-observe-then-process-exit");
    let session_id = SessionId("lifecycle-observe-process-exit".to_string());
    let client_id = ClientId("lifecycle-observe-process-exit-client".to_string());
    let subscription_id = SubscriptionId("lifecycle-observe-process-exit-sub".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(self_exit_spawn_request(&session_id), 10)
        .expect("process-exit fixture spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach before observe");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("input should cause natural exit");
    let baseline = daemon
        .lifecycle_baseline()
        .expect("process-exit baseline")
        .cursor;
    let _ = observe_until_exited(&mut daemon, &session_id, &baseline, 20);
    let drained = daemon
        .drain(&session_id, 40)
        .expect("terminal drain remains available after observe");
    assert!(
        drained.client_egress.iter().any(|(target, frame)| {
            target == &client_id
                && matches!(
                    frame,
                    TransportEgress::ProcessExit {
                        session_id: frame_session,
                        subscription_id: frame_subscription,
                        ..
                    } if frame_session == &session_id && frame_subscription == &subscription_id
                )
        }),
        "unbound drain must still deliver ProcessExited after control-plane Exited: {:?}",
        drained.client_egress
    );
    daemon
        .shutdown(Some(session_id), 50)
        .expect("process-exit shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_session_lifecycle_finds_a_row_beyond_256_without_scans() {
    let data_dir = temp_data_dir("exact-observe-large-registry");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let live = SessionId("z-exact-observe-live".to_string());
    daemon
        .spawn(immediate_exit_spawn_request(&live), 10)
        .expect("one live session so an observe walk would increment scans");
    for index in 0..257_u32 {
        let dummy = SessionId(format!("a-dummy-{index:03}"));
        daemon
            .registry()
            .save(&RegistryRecord::running(
                dummy,
                None,
                ResizePayload { rows: 24, cols: 80 },
                "dummy".to_string(),
                10,
            ))
            .expect("dummy registry row");
    }
    let target = SessionId("a-dummy-256".to_string());
    let looked_up = daemon
        .observe_session_lifecycle(&target, 20)
        .expect("exact query");
    match looked_up {
        SessionLifecycleLookup::Found(record) => {
            assert_eq!(record.session.session_id, target);
        }
        other => panic!("expected Found for the 257th dummy row, got {other:?}"),
    }
    daemon.shutdown(Some(live), 40).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_delivers_process_exited_during_worker_hold_before_exit() {
    let data_dir = short_temp_data_dir("w1-hold");
    let session_id = SessionId("w1-hold-session".to_string());
    // Never written: the worker holds after its exit frame until the
    // reaper ends it.
    let exit_hold = Fifo::new("w1-exit-hold");
    let (probe, probe_events) = WorkerRouteProbe::channel();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_hold_before_exit_gate(Some(exit_hold.path().to_path_buf()))
            .with_test_route_probe(Some(probe)),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("W1 session should spawn");
    let after_spawn = daemon.lifecycle_baseline().expect("W1 baseline").cursor;
    let (worker_pid, pty_child_pid, _) = worker_process_evidence(&daemon, &session_id);
    assert!(
        wait_pid_exit(pty_child_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "W1 session process exit"
    );
    assert!(process_exists(worker_pid), "W1 worker still alive");
    // The worker sends FRAME_PROCESS_EXITED and then holds: the parent
    // reading that frame proves the hold has begun.
    let probe_deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    loop {
        match probe_events
            // timer: deadline — the parent must read the exit frame; expiry fails the test
            .recv_timeout(probe_deadline.saturating_duration_since(Instant::now()))
            .expect("the parent reads the worker's exit frame")
        {
            WorkerRouteProbeEvent::ProcessExitRead {
                session_id: ref exited,
            } if *exited == session_id => break,
            _ => {}
        }
    }
    assert!(
        process_exists(worker_pid),
        "W1 hold must still own the worker child before blind shutdown"
    );

    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("blind ShutdownSession must complete while the worker holds stdout open");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "W1 shutdown must finish inside the 2s daemon deadline, got {elapsed:?}"
    );
    assert!(
        process_exists(worker_pid),
        "W1 hold must keep the worker child alive after shutdown Ok"
    );

    let looked_up = daemon
        .observe_session_lifecycle(&session_id, 21)
        .expect("exact-session query after W1 delivery");
    match &looked_up {
        SessionLifecycleLookup::Found(record) => {
            assert_eq!(record.session.registry_state, RegistrySessionState::Exited);
            assert!(
                matches!(record.lifecycle, Some(SessionLifecycleState::Exited { .. })),
                "observe_session_lifecycle must report the exited row during the hold: {record:?}"
            );
        }
        other => panic!("expected Found Exited during W1 hold, got {other:?}"),
    }
    assert_eq!(
        daemon.list().expect("list W1 session")[0].registry_state,
        RegistrySessionState::Exited
    );
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("W1 journal page");
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            ))
            .count(),
        1
    );
    assert!(
        wait_pid_exit(worker_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "W1 bounded reaper"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_delivers_process_exited_when_worker_exits_nonzero() {
    let data_dir = short_temp_data_dir("w2-exit");
    let session_id = SessionId("w2-exit-session".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_exit_code(Some(1)),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("W2 session should spawn");

    let looked_up = wait_for_exact_session_exited(&mut daemon, &session_id, 20);
    match &looked_up {
        SessionLifecycleLookup::Found(record) => {
            assert_eq!(record.session.registry_state, RegistrySessionState::Exited);
            assert!(
                matches!(
                    record.lifecycle,
                    Some(SessionLifecycleState::Exited { code: Some(0) })
                ),
                "W2 must keep the session process payload, not the worker status: {record:?}"
            );
        }
        other => panic!("expected Found Exited after W2 worker exit, got {other:?}"),
    }
    daemon
        .shutdown(Some(session_id.clone()), 30)
        .expect("shutdown after W2 delivery must succeed");
    assert_eq!(
        daemon.list().expect("list W2 session")[0].registry_state,
        RegistrySessionState::Exited
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_session_lifecycle_reconciles_parked_process_exited() {
    let data_dir = temp_data_dir("exact-observe-parked-exit");
    let session_id = SessionId("exact-observe-parked-exit".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("finite producer spawn");
    let first = wait_for_exact_session_exited(&mut daemon, &session_id, 20);
    let second = daemon
        .observe_session_lifecycle(&session_id, 21)
        .expect("second exact query");
    assert_eq!(first, second);
    match &first {
        SessionLifecycleLookup::Found(record) => {
            assert_eq!(record.session.registry_state, RegistrySessionState::Exited);
            assert!(
                matches!(record.lifecycle, Some(SessionLifecycleState::Exited { .. })),
                "first query must reconcile parked ProcessExited: {record:?}"
            );
        }
        other => panic!("expected Found Exited, got {other:?}"),
    }
    assert!(daemon
        .remove_session(&session_id)
        .expect("exited session is removable"));
    assert!(matches!(
        daemon.observe_session_lifecycle(&session_id, 30),
        Ok(SessionLifecycleLookup::Absent)
    ));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn observe_session_lifecycle_unknown_id_is_absent() {
    let data_dir = temp_data_dir("exact-observe-absent");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let result = daemon.observe_session_lifecycle(&SessionId("missing".to_string()), 10);
    assert!(
        matches!(result, Ok(SessionLifecycleLookup::Absent)),
        "absence must be Ok(Absent), got {result:?}"
    );
    assert!(!matches!(result, Err(CoreDaemonError::UnknownSession(_))));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn terminal_subscription_generation_is_exact_membership() {
    let data_dir = temp_data_dir("exact-sub-generation");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("exact-sub-generation".to_string());
    let client_id = ClientId("exact-sub-generation-client".to_string());
    let subscription_id = SubscriptionId("exact-sub-generation-sub".to_string());
    let missing_session = SessionId("exact-sub-generation-missing".to_string());
    let missing_subscription = SubscriptionId("exact-sub-generation-other".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn for exact membership");
    assert_eq!(
        daemon.terminal_subscription_generation(&session_id, &subscription_id),
        None,
        "unknown subscription before attach is None"
    );
    assert_eq!(
        daemon.terminal_subscription_generation(&missing_session, &subscription_id),
        None,
        "unknown session is None"
    );
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach owner");
    let inventory = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.session_id == session_id && row.subscription_id == subscription_id)
        .expect("inventory row after attach");
    let live = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("live generation");
    assert_eq!(live, inventory.generation);
    assert_eq!(
        daemon.terminal_subscription_generation(&session_id, &missing_subscription),
        None,
        "other subscription on a live session is None"
    );
    daemon
        .detach_terminal_subscription(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            live,
            12,
        )
        .expect("detach owner");
    assert_eq!(
        daemon.terminal_subscription_generation(&session_id, &subscription_id),
        None,
        "detached subscription is None"
    );
    daemon
        .attach(client_id, session_id.clone(), subscription_id.clone(), 13)
        .expect("re-attach owner");
    let next = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation after re-attach");
    assert!(
        next > live,
        "re-attach must increment generation: live={live:?} next={next:?}"
    );
    assert_ne!(next, TerminalSubscriptionGeneration(0));
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn session_registry_state_unknown_id_is_absent() {
    let data_dir = temp_data_dir("exact-registry-state-absent");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let result = daemon.session_registry_state(&SessionId("missing".to_string()));
    assert!(
        matches!(result, Ok(SessionRegistryStateLookup::Absent)),
        "absence must be Ok(Absent), got {result:?}"
    );
    assert!(!matches!(result, Err(CoreDaemonError::UnknownSession(_))));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn session_registry_state_after_shutdown_is_err() {
    let data_dir = temp_data_dir("exact-registry-state-shutdown");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon.shutdown(None, 10).expect("full daemon shutdown");
    let result = daemon.session_registry_state(&SessionId("after-shutdown".to_string()));
    assert!(
        matches!(result, Err(CoreDaemonError::Shutdown)),
        "shutdown must be Err, got {result:?}"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn session_registry_state_engine_live_without_registry_is_unknown_session() {
    let data_dir = temp_data_dir("exact-registry-state-unknown");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("exact-registry-state-unknown".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn live engine session");
    daemon
        .registry()
        .remove(&session_id)
        .expect("drop registry row while engine still owns the session");
    let result = daemon.session_registry_state(&session_id);
    assert!(
        matches!(result, Err(CoreDaemonError::UnknownSession(ref id) ) if id == &session_id),
        "registry-missing engine-live must be UnknownSession, got {result:?}"
    );
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn session_registry_state_does_not_reconcile_parked_exit() {
    let data_dir = temp_data_dir("exact-registry-state-parked");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("exact-registry-state-parked".to_string());
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("finite producer spawn");
    let pid = daemon
        .registry()
        .load(&session_id)
        .expect("load spawned record")
        .expect("spawned record")
        .process
        .and_then(|process| process.pid)
        .expect("PTY child pid");
    assert!(journal_advanced(&mut daemon), "spawn sets the wake");
    let cursor = daemon
        .lifecycle_baseline()
        .expect("watermark after spawn")
        .cursor;
    assert!(
        wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "OS-level finite-producer exit"
    );
    let looked_up = daemon
        .session_registry_state(&session_id)
        .expect("non-mutating query");
    assert!(
        matches!(
            looked_up,
            SessionRegistryStateLookup::Found(RegistrySessionState::Running)
        ),
        "parked exit must stay Found(Running): {looked_up:?}"
    );
    assert!(
        !journal_advanced(&mut daemon),
        "registry-state query must not raise the journal-advanced wake"
    );
    let page = daemon
        .lifecycle_changes_page(&cursor, 8, 16 * 1024)
        .expect("page after non-mutating query");
    assert!(page.resync_required.is_none());
    assert!(
        page.changes.is_empty(),
        "registry-state query must not append lifecycle changes: {:?}",
        page.changes
    );
    // OS-level termination does not prove PTY reader finalization. The local
    // runtime publishes ProcessExited only after its reader observes EOF, so
    // one observe pass may run before that and leave the record Running.
    // Observe until the runtime reconciles the parked exit.
    let observed = on_wakes_until(
        &mut daemon,
        "observe reconciles the parked exit",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |daemon| {
            let lookup = daemon
                .observe_session_lifecycle(&session_id, 20)
                .expect("positive observe control");
            matches!(
                &lookup,
                SessionLifecycleLookup::Found(record)
                    if record.session.registry_state == RegistrySessionState::Exited
            )
            .then_some(lookup)
        },
    );
    match &observed {
        SessionLifecycleLookup::Found(record) => {
            assert_eq!(record.session.registry_state, RegistrySessionState::Exited);
            assert!(
                matches!(record.lifecycle, Some(SessionLifecycleState::Exited { .. })),
                "observe must reconcile parked ProcessExited: {record:?}"
            );
        }
        other => panic!("expected Found Exited after observe, got {other:?}"),
    }
    assert!(
        journal_advanced(&mut daemon),
        "observe_session_lifecycle must raise the journal-advanced wake"
    );
    let after_observe = daemon
        .lifecycle_changes_page(&cursor, 8, 16 * 1024)
        .expect("page after observe");
    assert!(
        !after_observe.changes.is_empty(),
        "observe must append a lifecycle change so the negative half is meaningful"
    );
    daemon.shutdown(Some(session_id), 30).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_lifecycle_source_orders_shutdown_and_requires_overflow_resync() {
    let data_dir = temp_data_dir("lifecycle-source-overflow");
    let session_id = SessionId("lifecycle-overflow-session".to_string());
    let client_id = ClientId("lifecycle-overflow-client".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(2),
    );
    let baseline = daemon
        .lifecycle_baseline()
        .expect("empty overflow baseline");

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("overflow fixture should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("lifecycle-overflow-subscription".to_string()),
            11,
        )
        .expect("overflow fixture should attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);
    daemon
        .resize(client_id.clone(), session_id.clone(), 25, 80, 12)
        .expect("first material row update");
    daemon
        .resize(client_id, session_id.clone(), 26, 81, 13)
        .expect("second material row update");

    let overflow = daemon.lifecycle_changes(&baseline.cursor);
    assert!(overflow.changes.is_empty());
    assert!(matches!(
        overflow.resync_required,
        Some(SessionLifecycleResyncReason::CursorExpired { .. })
    ));
    let refreshed = daemon
        .lifecycle_baseline()
        .expect("overflow recovery baseline");
    assert_eq!(refreshed.sessions.len(), 1);
    assert_eq!(refreshed.sessions[0].session.size.rows, 26);
    assert_eq!(refreshed.sessions[0].session.size.cols, 81);

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("explicit shutdown should complete");
    let terminal = daemon.lifecycle_changes(&refreshed.cursor);
    assert!(terminal.resync_required.is_none());
    let terminal_states: Vec<_> = terminal
        .changes
        .iter()
        .filter_map(|change| match &change.kind {
            SessionLifecycleChangeKind::Upsert { record } => {
                Some(record.session.registry_state.clone())
            }
            _ => None,
        })
        .collect();
    assert!(matches!(
        terminal_states.as_slice(),
        [RegistrySessionState::Exited]
            | [RegistrySessionState::Stopping, RegistrySessionState::Exited]
    ));
    assert!(terminal
        .changes
        .windows(2)
        .all(|pair| pair[0].cursor.sequence < pair[1].cursor.sequence));

    assert!(daemon
        .remove_session(&session_id)
        .expect("shutdown session should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_restart_invalidates_cursor_and_adopts_same_session_id() {
    let data_dir = temp_data_dir("lifecycle-source-restart");
    let session_id = SessionId("lcr".to_string());
    let metadata = CoreSessionMetadata::from_entries(BTreeMap::from([
        ("host.example/class".to_string(), "interactive".to_string()),
        ("host.example/source".to_string(), "embedded".to_string()),
    ]));
    let mut projection = BTreeMap::new();
    let old_cursor = {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        let baseline = serialized_lifecycle_baseline(
            &daemon
                .lifecycle_baseline()
                .expect("first-generation baseline"),
        );
        replace_lifecycle_projection(&mut projection, &baseline);
        let mut request = spawn_request(&session_id);
        request.metadata = metadata.clone();
        let spawned = daemon
            .spawn(request, 10)
            .expect("first generation should spawn worker");
        assert_eq!(spawned.metadata, metadata);
        let running = serialized_lifecycle_changes(&daemon.lifecycle_changes(&baseline.cursor));
        apply_lifecycle_changes(&mut projection, &running);
        assert_eq!(
            projection
                .get(&session_id.0)
                .expect("spawn upsert should populate consumer projection")
                .metadata,
            metadata
        );
        let current = serialized_lifecycle_baseline(
            &daemon
                .lifecycle_baseline()
                .expect("current first-generation baseline"),
        );
        assert_eq!(current.sessions[0].metadata, metadata);
        let cursor = running.cursor;
        daemon.release_for_restart();
        cursor
    };

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let restarted_baseline = serialized_lifecycle_baseline(
        &restarted
            .lifecycle_baseline()
            .expect("restart baseline should expose durable registry truth"),
    );
    replace_lifecycle_projection(&mut projection, &restarted_baseline);
    assert_eq!(restarted_baseline.sessions.len(), 1);
    assert_eq!(
        restarted_baseline.sessions[0].session.session_id,
        session_id
    );
    assert_eq!(restarted_baseline.sessions[0].metadata, metadata);
    assert!(restarted_baseline.sessions[0].lifecycle.is_none());
    let foreign = restarted.lifecycle_changes(&old_cursor);
    assert!(foreign.changes.is_empty());
    assert_eq!(
        foreign.resync_required,
        Some(SessionLifecycleResyncReason::SourceChanged)
    );

    let adopted_session = restarted
        .adopt_session(&session_id, 12)
        .expect("fresh daemon should adopt from real worker protocol evidence");
    assert_eq!(adopted_session.metadata, metadata);
    let adopted =
        serialized_lifecycle_changes(&restarted.lifecycle_changes(&restarted_baseline.cursor));
    apply_lifecycle_changes(&mut projection, &adopted);
    assert_eq!(adopted.changes.len(), 1);
    assert!(matches!(
        &adopted.changes[0].kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
                && matches!(record.lifecycle, Some(SessionLifecycleState::Running))
                && record.metadata == metadata
    ));
    let post_adoption = serialized_lifecycle_baseline(
        &restarted
            .lifecycle_baseline()
            .expect("post-adoption baseline"),
    );
    assert_eq!(
        post_adoption.sessions.len(),
        1,
        "adoption must not fabricate a duplicate session"
    );
    assert_eq!(post_adoption.sessions[0].metadata, metadata);
    assert_eq!(projection.len(), 1);
    assert_eq!(
        projection
            .get(&session_id.0)
            .expect("adoption upsert should update the same projected row")
            .metadata,
        metadata
    );

    restarted
        .shutdown(Some(session_id.clone()), 20)
        .expect("adopted worker should shut down cleanly");
    assert!(restarted
        .remove_session(&session_id)
        .expect("adopted terminal worker should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn colliding_sanitizer_ids_restart_adopt_and_remove_independently() {
    let data_dir = short_temp_data_dir("registry-id");
    let ids = [
        SessionId("audit:a".to_string()),
        SessionId("audit_a".to_string()),
    ];
    let mut original =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    for (index, id) in ids.iter().enumerate() {
        let mut request = spawn_request(id);
        request.metadata = CoreSessionMetadata::from_entries(BTreeMap::from([(
            "host.example/identity".to_string(),
            format!("record-{index}"),
        )]));
        original
            .spawn(request, 10 + index as u64)
            .expect("spawn distinct worker");
    }
    let before: Vec<_> = ids
        .iter()
        .map(|id| {
            original
                .registry()
                .load(id)
                .expect("load exact record")
                .expect("record exists")
        })
        .collect();
    assert_eq!(before[0].session_id, ids[0]);
    assert_eq!(before[1].session_id, ids[1]);
    assert_ne!(before[0].metadata, before[1].metadata);
    let evidence: Vec<_> = ids
        .iter()
        .map(|id| worker_process_evidence(&original, id))
        .collect();
    assert_ne!(evidence[0].0, evidence[1].0);
    assert_ne!(evidence[0].2, evidence[1].2);
    let (_, rows) = assemble_baseline_pages(&mut original, None, baseline_item_budget(1));
    assert_eq!(
        rows.iter()
            .map(|row| &row.session.session_id)
            .collect::<Vec<_>>(),
        ids.iter().collect::<Vec<_>>()
    );

    original.release_for_restart();
    drop(original);
    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let mut adopted = [false; 2];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let reports = restarted
            .adoption_scan()
            .expect("scan both restart identities");
        assert_eq!(
            reports
                .iter()
                .map(|report| &report.record.session_id)
                .collect::<Vec<_>>(),
            ids.iter().collect::<Vec<_>>()
        );
        for (index, id) in ids.iter().enumerate() {
            restarted
                .adopt_session(id, 20 + index as u64)
                .expect("adopt exact worker identity");
            adopted[index] = true;
            let after = restarted
                .registry()
                .load(id)
                .expect("load adopted record")
                .expect("adopted record exists");
            assert_eq!(after.session_id, *id);
            assert_eq!(after.metadata, before[index].metadata);
            assert_eq!(after.recovery_identity, before[index].recovery_identity);
            assert_eq!(worker_process_evidence(&restarted, id), evidence[index]);
        }
        restarted
            .shutdown(Some(ids[0].clone()), 30)
            .expect("shutdown only punctuation id");
        assert!(restarted
            .remove_session(&ids[0])
            .expect("remove punctuation record"));
        assert!(restarted
            .registry()
            .load(&ids[0])
            .expect("first removed")
            .is_none());
        let sibling = restarted
            .registry()
            .load(&ids[1])
            .expect("load surviving sibling")
            .expect("sibling persists");
        assert_eq!(sibling.session_id, ids[1]);
        assert_eq!(sibling.state, RegistrySessionState::Running);
        assert_eq!(sibling.recovery_identity, before[1].recovery_identity);
        let client = ClientId("registry-sibling-client".to_string());
        restarted
            .attach(
                client.clone(),
                ids[1].clone(),
                SubscriptionId("registry-sibling-sub".to_string()),
                31,
            )
            .expect("attach surviving worker");
        let _ = drain_until_attached(&mut restarted, &ids[1], &client);
        restarted
            .input(
                client,
                ids[1].clone(),
                b"registry-sibling-live\n".to_vec(),
                32,
            )
            .expect("input to surviving sibling");
        let output = drain_until(&mut restarted, &ids[1], "echo:registry-sibling-live");
        assert!(terminal_output(&output.client_egress).contains("echo:registry-sibling-live"));
        restarted
            .shutdown(Some(ids[1].clone()), 33)
            .expect("shutdown sibling");
        assert!(restarted
            .remove_session(&ids[1])
            .expect("remove sibling record"));
        assert!(restarted.list().expect("empty registry").is_empty());
    }));
    // A failed assertion must not leave a released worker without a daemon owner.
    for (index, id) in ids.iter().enumerate() {
        if restarted.registry().load(id).ok().flatten().is_some() {
            if !adopted[index] {
                let _ = restarted.adopt_session(id, 40);
            }
            let _ = restarted.shutdown(Some(id.clone()), 41);
        }
    }
    drop(restarted);
    let deadline = Instant::now() + Duration::from_secs(5);
    for (worker, child, socket) in &evidence {
        for pid in [*worker, *child] {
            assert!(
                wait_pid_exit(pid, deadline.saturating_duration_since(Instant::now())),
                "registry identity workers must stop within the cleanup bound"
            );
        }
        // The worker removes its socket before it exits.
        assert!(
            !socket.exists(),
            "a stopped worker leaves no control socket"
        );
    }
    let _ = fs::remove_dir_all(&data_dir);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn adoption_rejects_legacy_registry_without_inventing_terminal_state() {
    let data_dir = temp_data_dir("legacy-registry-rejection");
    let registry = botster_core_daemon::SessionRegistry::new(&data_dir);
    fs::create_dir_all(registry.root()).expect("legacy registry directory");
    let id = SessionId("legacy-session".to_string());
    let bytes = serde_json::to_vec(&adoptable_record(&id, 10)).expect("legacy record bytes");
    let path = registry.root().join("legacy-session.json");
    fs::write(&path, &bytes).expect("legacy fixture");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    assert!(matches!(
        daemon.adoption_scan(),
        Err(CoreDaemonError::Registry(
            botster_core_daemon::SessionRegistryError::UnsupportedFormat
        ))
    ));
    assert_eq!(fs::read(path).expect("legacy bytes remain"), bytes);
    drop(daemon);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn metadata_free_registry_and_lifecycle_json_default_to_empty_metadata() {
    let registry_record = botster_core_daemon::RegistryRecord::running(
        SessionId("legacy-registry".to_string()),
        None,
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        1,
    );
    let mut registry_json = serde_json::to_value(&registry_record).expect("serialize registry");
    registry_json
        .as_object_mut()
        .expect("registry JSON object")
        .remove("metadata");
    let decoded_registry: botster_core_daemon::RegistryRecord =
        serde_json::from_value(registry_json).expect("decode legacy registry JSON");
    assert_eq!(decoded_registry.metadata, CoreSessionMetadata::new());

    let lifecycle_record = SessionLifecycleRecord {
        session: DaemonSession::from(&registry_record),
        metadata: CoreSessionMetadata::new(),
        lifecycle: None,
    };
    let mut lifecycle_json =
        serde_json::to_value(&lifecycle_record).expect("serialize lifecycle record");
    lifecycle_json
        .as_object_mut()
        .expect("lifecycle JSON object")
        .remove("metadata");
    let decoded_lifecycle: SessionLifecycleRecord =
        serde_json::from_value(lifecycle_json).expect("decode legacy lifecycle JSON");
    assert_eq!(decoded_lifecycle.metadata, CoreSessionMetadata::new());
}

#[cfg(unix)]
#[test]
fn oversized_persisted_metadata_fails_adoption_without_touching_the_live_worker() {
    let data_dir = temp_data_dir("oversized-adoption-metadata");
    let session_id = SessionId("oversized-adoption".to_string());
    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("spawn worker before editing persisted metadata");
        daemon.release_for_restart();
    }

    let record_path = registry_fixture_path(&data_dir, &session_id);
    let valid_record = fs::read(&record_path).expect("read valid registry record for cleanup");
    let mut record: botster_core_daemon::RegistryRecord =
        serde_json::from_slice(&valid_record).expect("decode persisted registry record");
    record.metadata = CoreSessionMetadata::from_entries(BTreeMap::from([(
        "host.example/oversized".to_string(),
        "x".repeat(MAX_CORE_SESSION_METADATA_LEN + 1),
    )]));
    botster_core_daemon::SessionRegistry::new(&data_dir)
        .save(&record)
        .expect("write oversized registry record through its private storage format");
    let oversized_record = fs::read(&record_path).expect("read oversized registry bytes");

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let cursor = restarted
        .lifecycle_baseline()
        .expect("baseline before failed adoption")
        .cursor;
    let error = restarted
        .adopt_session(&session_id, 12)
        .expect_err("oversized persisted metadata must fail loudly");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Multiplexer(
            botster_core::MultiplexerEngineError::MetadataTooLarge
        ))
    ));
    assert_eq!(
        fs::read(&record_path).expect("read registry after failed adoption"),
        oversized_record,
        "failed adoption must not mutate persisted metadata"
    );
    assert!(restarted.lifecycle_changes(&cursor).changes.is_empty());

    fs::write(&record_path, valid_record).expect("repair persisted metadata after rejection");
    drop(restarted);
    let mut cleanup =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    cleanup
        .adopt_session(&session_id, 13)
        .expect("rejected adoption must leave the repaired worker adoptable");
    cleanup
        .shutdown(Some(session_id.clone()), 14)
        .expect("cleanup adopted worker");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_shutdown_timeout_is_typed_and_keeps_non_exited_cleanup_ownership() {
    let data_dir = short_temp_data_dir("timeout");
    let session_id = SessionId("timeout".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker-backed session");
    let (worker_pid, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    let mut stopped = StoppedProcess::new(worker_pid);

    // This test keeps the production 2 s shutdown deadline: the stopped
    // worker never completes, so shutdown waits the whole deadline and
    // ends typed. Tests whose property is order may lengthen it.
    let started = Instant::now();
    let error = daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect_err("stopped worker must not produce truthful completion");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_secs(2),
        "shutdown waits the whole production 2 s deadline: {elapsed:?}"
    );
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(
            botster_core::SessionRuntimeError {
                kind: botster_core::SessionRuntimeErrorKind::ShutdownFailed,
                ..
            }
        ))
    ));
    let listed = daemon.list().expect("list timed-out worker session");
    assert_ne!(listed[0].registry_state, RegistrySessionState::Exited);
    assert!(process_exists(worker_pid));
    assert!(socket_path.exists());

    stopped.resume();
    let tick_deadline = Instant::now() + HANG_GUARD;
    for tick in 0.. {
        let _ = daemon.drain(&session_id, 30 + tick);
        let listed = daemon.list().expect("list resumed worker session");
        if listed[0].registry_state == RegistrySessionState::Exited {
            break;
        }
        assert!(
            Instant::now() < tick_deadline,
            "the resumed worker session exiting did not arrive within {HANG_GUARD:?}"
        );
        // timer: deadline — HANG_GUARD bounds this wait
        let _ = daemon.wait_wakes(tick_deadline.saturating_duration_since(Instant::now()));
    }
    assert_eq!(
        daemon.list().expect("list cleaned worker session")[0].registry_state,
        RegistrySessionState::Exited
    );

    daemon.release_for_restart();
    drop(daemon);
    for pid in [worker_pid, pty_child_pid] {
        assert!(
            wait_pid_exit(pid, REAL_WORKER_COMPLETION_TIMEOUT),
            "bounded reap after timeout resume: {pid}"
        );
    }
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_registry_reopened_without_worker_path_is_not_restart_durable() {
    let data_dir = temp_data_dir("local-reopen");
    let session_id = SessionId("local-reopen-session".to_string());

    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("worker-backed daemon should spawn live session");
        let record = daemon
            .registry()
            .load(&session_id)
            .expect("registry load should succeed")
            .expect("spawn should persist restart evidence");
        assert!(
            record
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("worker_control_socket"))
                .is_some(),
            "worker-backed daemon should persist worker control socket evidence"
        );
        daemon.release_for_restart();
    }

    let mut local = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = local
        .adoption_scan()
        .expect("local daemon should scan worker-created registry");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::InProcessDaemonNotRestartDurable
    );

    let error = local
        .adopt_session(&session_id, 12)
        .expect_err("local daemon should fail loudly before registry adoption");
    assert!(
        matches!(error, CoreDaemonError::MissingWorkerPath),
        "expected MissingWorkerPath, got {error:?}"
    );

    let mut cleanup =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    cleanup
        .adopt_session(&session_id, 13)
        .expect("cleanup daemon should adopt released worker");
    cleanup
        .shutdown(Some(session_id.clone()), 14)
        .expect("cleanup daemon should shut down released worker");

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn in_process_non_restart_durable_adoption_state_has_stable_json_tag() {
    let json = serde_json::to_string(&SessionAdoptionState::InProcessDaemonNotRestartDurable)
        .expect("adoption state should serialize");
    assert_eq!(json, "\"in_process_daemon_not_restart_durable\"");
}

#[test]
fn registry_records_are_durable_enough_for_adoption_scan() {
    let data_dir = temp_data_dir("daemon-adoption");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-adoption-session".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");

    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read persisted records");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].record.session_id, session_id);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::MissingProtocolEvidence
    );

    let mut record = daemon
        .registry()
        .load(&session_id)
        .expect("registry record should load")
        .expect("spawn should persist a record");
    record.observe_restart_contract(serde_json::json!({"session": "daemon-adoption"}), 11);
    daemon
        .registry()
        .save(&record)
        .expect("observed restart-contract evidence should persist");

    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read persisted records");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::WorkerDied
        }
    );

    daemon
        .shutdown(Some(SessionId("daemon-adoption-session".to_string())), 20)
        .expect("shutdown should update registry lifecycle");
    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read shut down records");
    assert_eq!(reports[0].state, SessionAdoptionState::Terminal);

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_dead_worker_with_live_registry_record() {
    let data_dir = temp_data_dir("daemon-dead-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-dead-worker-session".to_string());
    let record = adoptable_record(&session_id, 10);
    daemon
        .registry()
        .save(&record)
        .expect("dead-worker fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify dead worker");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::WorkerDied
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_incompatible_protocol_version() {
    let data_dir = temp_data_dir("daemon-incompatible-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-incompatible-worker-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.protocol_version = botster_core::PROTOCOL_VERSION.saturating_sub(1);
    daemon
        .registry()
        .save(&record)
        .expect("incompatible fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify incompatible protocol");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::IncompatibleProtocol
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_missed_heartbeat() {
    let data_dir = temp_data_dir("daemon-missed-heartbeat");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-missed-heartbeat-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.ping_pong_supported = false;
    daemon
        .registry()
        .save(&record)
        .expect("heartbeat fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify heartbeat failure");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::UnhealthyWorker {
            reason: SessionWorkerHealthReason::MissedHeartbeat
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_duplicate_worker_for_session() {
    let data_dir = temp_data_dir("daemon-duplicate-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-duplicate-worker-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.duplicate_worker_candidates = 1;
    daemon
        .registry()
        .save(&record)
        .expect("duplicate fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify duplicate candidates");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::DuplicateWorker { candidates: 2 }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_is_read_only_until_explicit_mark_stale() {
    let data_dir = temp_data_dir("daemon-adoption-read-only");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-adoption-read-only-session".to_string());
    let record = adoptable_record(&session_id, 10);
    daemon
        .registry()
        .save(&record)
        .expect("read-only fixture should save");
    let record_path = registry_fixture_path(&data_dir, &session_id);
    let before = fs::read(&record_path).expect("record bytes should load before scan");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify without mutation");
    assert!(matches!(
        reports[0].state,
        SessionAdoptionState::StaleWorker { .. }
    ));
    let after_scan = fs::read(&record_path).expect("record bytes should load after scan");
    assert_eq!(before, after_scan, "adoption_scan must be read-only");

    daemon
        .mark_stale(&session_id, 20)
        .expect("explicit stale mark should persist");
    let marked = daemon
        .registry()
        .load(&session_id)
        .expect("marked record should load")
        .expect("marked record should exist");
    assert_eq!(marked.state, RegistrySessionState::Stale);
    assert_eq!(marked.updated_at, 20);

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn registry_load_all_skips_malformed_records_without_blocking_good_records() {
    let data_dir = temp_data_dir("daemon-corrupt-registry");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-good-record".to_string());
    let mut record = botster_core_daemon::RegistryRecord::running(
        session_id.clone(),
        None,
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        1,
    );
    record.observe_restart_contract(serde_json::json!({"session": "daemon-good-record"}), 2);
    daemon
        .registry()
        .save(&record)
        .expect("good registry record should save");
    let bad = SessionId("daemon-bad-record".to_string());
    seed_registry_records(&daemon, &[&bad], 1);
    fs::write(registry_fixture_path(&data_dir, &bad), b"not json")
        .expect("malformed registry fixture should be written");

    let listed = daemon.list().expect("bad record should not block listing");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, session_id);

    let _ = fs::remove_dir_all(data_dir);
}

fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
                    .to_string(),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

#[cfg(unix)]
#[test]
fn worker_binary_is_hosted_by_daemon_package_not_core() {
    // Packaging proof: session worker builds from botster-core-daemon and
    // botster-core does not depend on botster-terminal-ghostty.
    let daemon_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let core_toml = fs::read_to_string(daemon_manifest.join("../botster-core/Cargo.toml"))
        .expect("read core cargo");
    assert!(
        !core_toml.contains("botster-terminal-ghostty"),
        "botster-core must remain Ghostty-free"
    );
    assert!(
        !core_toml.contains("name = \"botster-session-worker\""),
        "session-worker binary must not remain in botster-core"
    );
    let daemon_toml =
        fs::read_to_string(daemon_manifest.join("Cargo.toml")).expect("read daemon cargo");
    assert!(
        daemon_toml.contains("name = \"botster-session-worker\""),
        "daemon package must host botster-session-worker"
    );
    assert!(
        daemon_toml.contains("botster-terminal-ghostty"),
        "daemon package hosts Ghostty for the worker binary"
    );
    let path = worker_path();
    assert!(
        path.exists(),
        "daemon-hosted worker binary must resolve at {}",
        path.display()
    );
}

fn immediate_exit_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec!["-c".to_string(), "exit 0".to_string()],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

/// A shell fixture that reports its pid, blocks until the test releases it,
/// prints its marker, and exits. The test chooses when the output and exit
/// happen instead of racing a fixed delay.
struct GatedOutputExit {
    pid: Fifo,
    gate: Fifo,
}

impl GatedOutputExit {
    fn new() -> Self {
        Self {
            pid: Fifo::new("gated-exit-pid"),
            gate: Fifo::new("gated-exit-gate"),
        }
    }

    /// Release the fixture to print and exit; returns its pid.
    fn release(&self) -> u32 {
        let pid = String::from_utf8(self.pid.read_signal(REAL_WORKER_COMPLETION_TIMEOUT))
            .expect("fixture pid is text");
        self.gate.release(REAL_WORKER_COMPLETION_TIMEOUT);
        pid.trim().parse().expect("fixture pid")
    }
}

fn gated_output_exit_spawn_request(
    session_id: &SessionId,
    fixture: &GatedOutputExit,
    marker: &str,
) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                format!(
                    "/bin/echo $$ > '{}'; /bin/cat '{}' >/dev/null; printf {marker}; exit 0",
                    fixture.pid.path().display(),
                    fixture.gate.path().display()
                ),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

fn pump_until_registry_exited(daemon: &mut CoreDaemon, session_id: &SessionId, now_seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            daemon
                .session_registry_state(session_id)
                .expect("non-progress registry lookup"),
            SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
        ) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "targeted pump did not commit Exited for {}",
            session_id.0
        );
        // timer: deadline — the loop's bound on the next wake
        let batch = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            continue;
        }
        let _ = daemon
            .pump_woken(&batch, now_seconds)
            .expect("targeted pump");
    }
}

/// Daemons built with [`expired_elapsed_config`] start every lifecycle
/// elapsed reading at this, so a page or slice given this budget yields at
/// entry, while one given `Duration::MAX` runs.
const EXPIRED: Duration = Duration::from_secs(60);

fn expired_elapsed_config(data_dir: &std::path::Path) -> CoreDaemonConfig {
    CoreDaemonConfig::new(data_dir).with_test_elapsed_at_entry(EXPIRED)
}

fn observe_item_budget(max_sessions: usize) -> ObserveLifecycleBudget {
    ObserveLifecycleBudget {
        max_sessions,
        max_encoded_result_bytes: 16 * 1024,
        max_elapsed: Duration::MAX,
    }
}

fn observe_resume(slice: &ObserveLifecycleSlice) -> ObserveLifecycleCursor {
    ObserveLifecycleCursor {
        pass_id: slice.pass_id.clone(),
        last_visited: slice.last_visited.clone(),
    }
}

fn assert_successful_page_within_budget(page: &SessionLifecyclePage, max_bytes: usize) {
    assert!(page.resync_required.is_none());
    let encoded = serde_json::to_vec(page).expect("successful page must serialize");
    assert!(
        encoded.len() <= max_bytes,
        "successful page encoded {} bytes, budget {max_bytes}",
        encoded.len()
    );
}

fn page_contains_exited(page: &SessionLifecyclePage, session_id: &SessionId) -> bool {
    page.changes.iter().any(|change| {
        matches!(
            &change.kind,
            SessionLifecycleChangeKind::Upsert { record }
                if record.session.session_id == *session_id
                    && record.session.registry_state == RegistrySessionState::Exited
                    && matches!(
                        record.lifecycle,
                        Some(SessionLifecycleState::Exited { code: Some(0) })
                    )
        )
    })
}

fn observe_until_exited(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    after: &SessionLifecycleCursor,
    now_seconds: u64,
) -> SessionLifecyclePage {
    let mut tick = 0;
    on_wakes_until(
        daemon,
        "observe_lifecycle publishing Exited",
        Duration::from_secs(1),
        |daemon| {
            tick += 1;
            daemon
                .observe_lifecycle(now_seconds + tick)
                .expect("observe_lifecycle should succeed");
            let page = daemon
                .lifecycle_changes_page(after, 16, 16 * 1024)
                .expect("page after observe");
            assert_successful_page_within_budget(&page, 16 * 1024);
            page_contains_exited(&page, session_id).then_some(page)
        },
    )
}

fn self_exit_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "printf ready; IFS= read -r line; printf \"echo:%s\\n\" \"$line\"; exit 0"
                    .to_string(),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

fn notification_session_target(id: &str) -> NotificationTarget {
    NotificationTarget::Session(SessionId(id.to_string()))
}

fn notification_client_target(id: &str) -> NotificationTarget {
    NotificationTarget::Client(ClientId(id.to_string()))
}

fn notification(id: &str, target: NotificationTarget, created_at: u64) -> NotificationItem {
    NotificationItem::message(
        NotificationId(id.to_string()),
        target,
        NotificationSeverity::Info,
        NotificationSource {
            label: "daemon-test".to_string(),
            plugin_key: None,
        },
        NotificationContent {
            title: format!("Notification {id}"),
            body: Some("Synthetic daemon test notification.".to_string()),
            extension: None,
        },
        NotificationTimestamp(created_at),
    )
}

fn envelope_endpoint(id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Endpoint {
        endpoint_id: EndpointId(id.to_string()),
    }
}

fn envelope(id: &str, targets: Vec<EnvelopeTarget>) -> RoutedEnvelope {
    RoutedEnvelope::new(
        EnvelopeId(id.to_string()),
        EndpointId("daemon-test-source".to_string()),
        targets,
        RoutedEnvelopePayload {
            content_type: "application/octet-stream".to_string(),
            body: format!("payload:{id}").into_bytes(),
            extension: None,
        },
        10,
    )
}

fn adoptable_record(
    session_id: &SessionId,
    now_seconds: u64,
) -> botster_core_daemon::RegistryRecord {
    let mut record = botster_core_daemon::RegistryRecord::running(
        session_id.clone(),
        Some(botster_core::ProcessIdentity {
            pid: Some(42),
            runtime_id: Some(format!("{}-runtime", session_id.0)),
        }),
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        now_seconds,
    );
    record.observe_restart_contract(
        serde_json::json!({"session": session_id.0}),
        now_seconds + 1,
    );
    record
}

fn serialized_lifecycle_baseline(baseline: &SessionLifecycleBaseline) -> SessionLifecycleBaseline {
    serde_json::from_slice(
        &serde_json::to_vec(baseline).expect("serialize lifecycle baseline for consumer"),
    )
    .expect("deserialize lifecycle baseline for consumer")
}

fn serialized_lifecycle_changes(changes: &SessionLifecycleChanges) -> SessionLifecycleChanges {
    serde_json::from_slice(
        &serde_json::to_vec(changes).expect("serialize lifecycle changes for consumer"),
    )
    .expect("deserialize lifecycle changes for consumer")
}

fn replace_lifecycle_projection(
    projection: &mut BTreeMap<String, SessionLifecycleRecord>,
    baseline: &SessionLifecycleBaseline,
) {
    projection.clear();
    projection.extend(
        baseline
            .sessions
            .iter()
            .cloned()
            .map(|record| (record.session.session_id.0.clone(), record)),
    );
}

fn apply_lifecycle_changes(
    projection: &mut BTreeMap<String, SessionLifecycleRecord>,
    changes: &SessionLifecycleChanges,
) {
    for change in &changes.changes {
        match &change.kind {
            SessionLifecycleChangeKind::Upsert { record } => {
                projection.insert(record.session.session_id.0.clone(), record.clone());
            }
            SessionLifecycleChangeKind::Removed { session_id } => {
                projection.remove(&session_id.0);
            }
            _ => {}
        }
    }
}

fn drain_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
) -> botster_core_daemon::DrainResult {
    // As before, the drain returns what it has after its 1 s bound (the
    // former 100 steps of 10 ms); callers assert on the result.
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut aggregate = botster_core_daemon::DrainResult::default();
    let mut tick = 0;
    loop {
        tick += 1;
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let left = deadline.saturating_duration_since(Instant::now());
        if terminal_output(&aggregate.client_egress).contains(expected) || left.is_zero() {
            return aggregate;
        }
        // timer: deadline — the drain's 1 s bound; expiry returns what was drained
        let _ = daemon.wait_wakes(left);
    }
}

fn drain_pre_attach_producer_output(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    now_seconds: u64,
) {
    let mut idle = 0;
    for tick in 0..256 {
        let drained = daemon
            .drain(session_id, now_seconds + tick)
            .expect("pre-attach producer drain should succeed");
        if drained.client_egress.is_empty()
            && drained.observations.is_empty()
            && drained.backpressure.is_empty()
        {
            idle += 1;
            if idle >= 3 {
                return;
            }
        } else {
            idle = 0;
        }
    }
}

fn client_attached(drained: &botster_core_daemon::DrainResult, client_id: &ClientId) -> bool {
    drained.client_egress.iter().any(|(target, frame)| {
        target == client_id
            && matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                }
            )
    })
}

fn drain_until_attached(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    let mut tick = 0;
    on_wakes_until(
        daemon,
        "the worker attach reaching Attached",
        Duration::from_secs(10),
        |daemon| {
            tick += 1;
            let drained = daemon
                .drain(session_id, 20 + tick)
                .expect("daemon attach drain should succeed");
            let attached = client_attached(&drained, client_id);
            aggregate.client_egress.extend(drained.client_egress);
            aggregate.observations.extend(drained.observations);
            aggregate.backpressure.extend(drained.backpressure);
            attached.then_some(())
        },
    );
    aggregate
}

fn drain_until_for_client(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
    expected: &str,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    let mut tick = 0;
    on_wakes_until_idle(daemon, "queued client output", |daemon| {
        tick += 1;
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = renderable_output_for_client(&aggregate.client_egress, client_id);
        if output.contains(expected) {
            Ok(())
        } else {
            Err(output.len())
        }
    });
    aggregate
}

#[cfg(unix)]
fn drain_until_terminal_marker(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    let mut tick = 0;
    on_wakes_until_idle(daemon, "the terminal output marker", |daemon| {
        let drained = daemon
            .drain(session_id, start_tick + tick)
            .expect("daemon drain should succeed");
        tick += 1;
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = terminal_output(&aggregate.client_egress);
        if output.contains(expected) {
            Ok(())
        } else {
            Err(output.len())
        }
    });
}

fn assert_snapshot_format(payload: &botster_core::TerminalSnapshotPayload) {
    assert_eq!(
        payload.format.as_deref(),
        Some(EXPECTED_SNAPSHOT_FORMAT),
        "snapshot format should match the active daemon terminal backend"
    );
}

fn assert_ghostty_snapshot_replays_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    assert!(
        plain_text.contains(expected),
        "Ghostty snapshot replay should contain {expected:?}; replayed text length: {}",
        plain_text.len()
    );
    assert!(
        payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        payload.bytes.len()
    );
}

fn assert_ghostty_snapshot_replays_minimum_markers(
    payload: &botster_core::TerminalSnapshotPayload,
    marker_count: usize,
    minimum: usize,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, marker_count);
    assert!(
        retained_markers.len() >= minimum,
        "Ghostty snapshot replay should retain at least {minimum} generated markers; retained markers: {}; replayed text length: {}",
        retained_markers.len(),
        plain_text.len()
    );
    assert!(
        payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        payload.bytes.len()
    );
}

fn assert_ghostty_snapshot_does_not_replay_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    unexpected: &str,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    assert!(
        !plain_text.contains(unexpected),
        "Ghostty snapshot replay should not contain {unexpected:?}; replayed text length: {}",
        plain_text.len()
    );
}

fn ghostty_snapshot_plain_text(payload: &botster_core::TerminalSnapshotPayload) -> String {
    assert_snapshot_format(payload);
    let mut terminal = GhosttyTerminal::with_config(
        payload.size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty replay terminal");
    terminal
        .import_snapshot(payload)
        .expect("test should import daemon Ghostty snapshot");
    terminal
        .plain_text()
        .expect("test should format replayed Ghostty snapshot")
}

fn retained_ghostty_scrollback_markers(plain_text: &str, marker_count: usize) -> Vec<usize> {
    (0..marker_count)
        .filter(|line| plain_text.contains(&format!("echo:scrollback-line-{line:05}")))
        .collect()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

fn assert_retained_exit_output(
    drained: &botster_core_daemon::DrainResult,
    session_id: &SessionId,
    expected: &str,
) {
    let output = terminal_output(&drained.client_egress);
    assert_eq!(
        count_occurrences(&output, expected),
        1,
        "retained final terminal output should be delivered exactly once: {output:?}"
    );
    assert!(
        drained.observations.iter().any(|observation| {
            matches!(
                observation,
                BotsterEngineObservation::SessionLifecycle {
                    session_id: observed_session,
                    state: SessionLifecycleState::Exited { .. },
                } if observed_session == session_id
            )
        }),
        "retained drain should include the process-exit lifecycle observation: {:?}",
        drained.observations
    );
}

fn assert_no_duplicate_exit_output(drained: &botster_core_daemon::DrainResult, expected: &str) {
    let output = terminal_output(&drained.client_egress);
    assert_eq!(count_occurrences(&output, expected), 0);
    assert!(drained.observations.iter().all(|observation| !matches!(
        observation,
        BotsterEngineObservation::SessionLifecycle {
            state: SessionLifecycleState::Exited { .. },
            ..
        }
    )));
}

fn terminal_output(frames: &[(ClientId, TransportEgress)]) -> String {
    frames
        .iter()
        .filter_map(|(_, frame)| match frame {
            TransportEgress::TerminalOutput { data, .. } => {
                Some(String::from_utf8_lossy(data).to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn first_snapshot_for_client(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
) -> Option<(usize, botster_core::TerminalSnapshotPayload)> {
    let index = frames
        .iter()
        .enumerate()
        .find_map(|(index, (target, frame))| {
            (target == client_id && matches!(frame, TransportEgress::Snapshot { .. }))
                .then_some(index)
        })?;
    let bytes = frames
        .iter()
        .filter_map(|(target, frame)| match frame {
            TransportEgress::Snapshot { data, .. } if target == client_id => Some(data.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    Some((
        index,
        botster_core::TerminalSnapshotPayload::new(
            bytes,
            TerminalScreenSize::new(24, 80),
            Some(EXPECTED_SNAPSHOT_FORMAT.to_string()),
        ),
    ))
}

fn first_terminal_output_index_for_client_containing(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
    expected: &str,
) -> Option<usize> {
    frames
        .iter()
        .position(|(frame_client_id, frame)| match frame {
            TransportEgress::TerminalOutput { data, .. } if frame_client_id == client_id => {
                String::from_utf8_lossy(data).contains(expected)
            }
            _ => false,
        })
}

fn renderable_output(frames: &[(ClientId, TransportEgress)]) -> String {
    frames
        .iter()
        .filter_map(|(_, frame)| renderable_frame_data(frame))
        .collect::<Vec<_>>()
        .join("")
}

fn renderable_output_for_client(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
) -> String {
    frames
        .iter()
        .filter(|(frame_client_id, _)| frame_client_id == client_id)
        .filter_map(|(_, frame)| renderable_frame_data(frame))
        .collect::<Vec<_>>()
        .join("")
}

fn renderable_frame_data(frame: &TransportEgress) -> Option<String> {
    match frame {
        TransportEgress::TerminalOutput { data, .. }
        | TransportEgress::Snapshot { data, .. }
        | TransportEgress::Scrollback { data, .. } => {
            Some(String::from_utf8_lossy(data).to_string())
        }
        _ => None,
    }
}

#[cfg(unix)]
fn wait_for_exact_session_exited(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    now_seconds: u64,
) -> SessionLifecycleLookup {
    let mut tick = 0;
    on_wakes_until(
        daemon,
        "observe_session_lifecycle reconciling the parked exit",
        Duration::from_secs(1),
        |daemon| {
            tick += 1;
            let looked_up = daemon
                .observe_session_lifecycle(session_id, now_seconds + tick)
                .expect("exact query");
            matches!(
                &looked_up,
                SessionLifecycleLookup::Found(record)
                    if record.session.registry_state == RegistrySessionState::Exited
                        && matches!(
                            record.lifecycle,
                            Some(SessionLifecycleState::Exited { .. })
                        )
            )
            .then_some(looked_up)
        },
    )
}

/// Run `step` now and after each wake, under one deadline, until it returns
/// a value. Wakes queue until taken, so none is lost between steps.
fn on_wakes_until<T>(
    daemon: &mut CoreDaemon,
    label: &str,
    bound: Duration,
    mut step: impl FnMut(&mut CoreDaemon) -> Option<T>,
) -> T {
    wait_for(label, bound, |remaining| {
        if let Some(value) = step(daemon) {
            return Some(value);
        }
        // timer: deadline — wait_for's bound limits this wait
        let _ = daemon.wait_wakes(remaining);
        step(daemon)
    })
}

/// [`on_wakes_until`] under [`REAL_WORKER_COMPLETION_TIMEOUT`] that also
/// fails when `step`'s progress measure (`Err(progress)`) stays unchanged for
/// [`REAL_WORKER_IDLE_TIMEOUT`].
fn on_wakes_until_idle<T>(
    daemon: &mut CoreDaemon,
    label: &str,
    mut step: impl FnMut(&mut CoreDaemon) -> Result<T, usize>,
) -> T {
    let mut last = None;
    let last_progress = std::cell::Cell::new(Instant::now());
    wait_for(label, REAL_WORKER_COMPLETION_TIMEOUT, |remaining| {
        let mut check = |daemon: &mut CoreDaemon| match step(daemon) {
            Ok(value) => Some(value),
            Err(progress) => {
                if last != Some(progress) {
                    last = Some(progress);
                    last_progress.set(Instant::now());
                }
                assert!(
                    last_progress.get().elapsed() < REAL_WORKER_IDLE_TIMEOUT,
                    "{label}: no progress for {REAL_WORKER_IDLE_TIMEOUT:?}"
                );
                None
            }
        };
        if let Some(value) = check(daemon) {
            return Some(value);
        }
        let idle_left = REAL_WORKER_IDLE_TIMEOUT.saturating_sub(last_progress.get().elapsed());
        // timer: deadline — the completion bound or the idle bound, whichever ends first
        let _ = daemon.wait_wakes(remaining.min(idle_left));
        check(daemon)
    })
}

// Locate an existing test fixture by its verified record, without copying the filename encoding.
fn registry_fixture_path(data_dir: &std::path::Path, session_id: &SessionId) -> PathBuf {
    let registry = botster_core_daemon::SessionRegistry::new(data_dir);
    let expected = registry
        .load(session_id)
        .expect("load fixture identity")
        .expect("fixture exists");
    let matches: Vec<_> = fs::read_dir(registry.root())
        .expect("list registry fixtures")
        .map(|entry| entry.expect("registry fixture entry").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .filter(|path| {
            fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<RegistryRecord>(&bytes).ok())
                .is_some_and(|record| record == expected)
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "one file must contain the verified fixture"
    );
    matches.into_iter().next().expect("one fixture path")
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "botster-core-daemon-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn short_temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::path::PathBuf::from("/tmp").join(format!("bcd-{label}-{}-{nanos}", std::process::id()))
}

#[cfg(unix)]
fn worker_process_evidence(
    daemon: &CoreDaemon,
    session_id: &SessionId,
) -> (u32, u32, std::path::PathBuf) {
    let record = daemon
        .registry()
        .load(session_id)
        .expect("load worker registry record")
        .expect("worker registry record");
    let worker_pid = record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32;
    let pty_child_pid = record
        .process
        .and_then(|process| process.pid)
        .expect("PTY child pid in registry process identity");
    let socket_path = record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("worker socket in recovery identity");
    (worker_pid, pty_child_pid, socket_path)
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .expect("run process signal command");
    assert!(status.success(), "signal {signal} to process {pid}");
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
struct StoppedProcess {
    pid: u32,
    resumed: bool,
}

#[cfg(unix)]
impl StoppedProcess {
    fn new(pid: u32) -> Self {
        signal_process(pid, "STOP");
        Self {
            pid,
            resumed: false,
        }
    }

    fn resume(&mut self) {
        if !self.resumed {
            signal_process(self.pid, "CONT");
            self.resumed = true;
        }
    }
}

#[cfg(unix)]
impl Drop for StoppedProcess {
    fn drop(&mut self) {
        if !self.resumed && process_exists(self.pid) {
            let _ = Command::new("kill")
                .arg("-CONT")
                .arg(self.pid.to_string())
                .status();
        }
    }
}

fn worker_path() -> std::path::PathBuf {
    botster_core_test_support::real_worker::WorkerBinary::from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn compact_input_frame(data: &[u8]) -> Vec<u8> {
    encode_terminal_input(&TerminalInputCommand::RawBytes {
        operation_id: 1,
        data: data.to_vec(),
    })
    .expect("encode raw input")
    .into_bytes()
}

/// Route of each `INPUT_RESULT` the bound adapter delivered.
fn adapter_input_result_routes(adapter: &SharedFakeTerminalAdapter) -> Vec<String> {
    adapter
        .snapshot_delivered_frames()
        .into_iter()
        .filter(|delivered| {
            adapter_terminal_frame(&delivered.bytes).kind() == TerminalKind::InputResult
        })
        .map(|delivered| delivered.route)
        .collect()
}

fn wait_until_bound_attached(
    daemon: &mut CoreDaemon,
    _session_id: &SessionId,
    adapter: &SharedFakeTerminalAdapter,
) {
    let _ = pump_on_wakes_until(daemon, "the bound adapter reaching attached", 20, |_| {
        complete_one_slot_and_wake(adapter);
        adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_phase(bytes) == Some("attached"))
    });
}

/// Pump on wakes under one deadline until `done` holds. Returns whether any
/// pump reported a terminal inventory change.
fn pump_on_wakes_until(
    daemon: &mut CoreDaemon,
    label: &str,
    first_now: u64,
    mut done: impl FnMut(&mut CoreDaemon) -> bool,
) -> bool {
    let mut now = first_now;
    let mut inventory_changed = false;
    wait_for(label, REAL_WORKER_COMPLETION_TIMEOUT, |remaining| {
        if done(daemon) {
            return Some(());
        }
        // timer: deadline — wait_for's bound limits this wait
        let batch = daemon.wait_wakes(remaining);
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            now += 1;
            let outcome = daemon.pump_woken(&batch, now).expect("pump a woken batch");
            inventory_changed |= outcome.terminal_inventory_changed;
        }
        done(daemon).then_some(())
    });
    inventory_changed
}

fn pump_next_available_wake(daemon: &mut CoreDaemon, now_seconds: u64, within: Duration) -> bool {
    // timer: deadline — the caller's bound on the next wake
    let batch = daemon.wait_wakes(within);
    if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
        return false;
    }
    let _ = daemon
        .pump_woken(&batch, now_seconds)
        .expect("pump targeted wake");
    true
}

fn bind_echo_worker(
    daemon: &mut CoreDaemon,
    session_id: SessionId,
    client_id: ClientId,
    subscription_id: SubscriptionId,
    script: &str,
    now: u64,
) -> SharedFakeTerminalAdapter {
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = script.to_string();
    daemon.spawn(request, now).expect("spawn echo worker");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            now + 1,
        )
        .expect("attach echo worker");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after attach")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind echo worker");
    wait_until_bound_attached(daemon, &session_id, &adapter);
    adapter
}

/// Pump daemon wakes until `done` holds. Every step waits on the wake source.
fn pump_wakes_until(daemon: &mut CoreDaemon, mut done: impl FnMut() -> bool) -> bool {
    // timer: deadline — the condition must arrive through daemon wakes; expiry fails the caller
    let deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    while !done() {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        // timer: deadline — expiry fails the test
        let batch = daemon.wait_wakes(remaining);
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            daemon.pump_woken(&batch, 30).expect("pump woken");
        }
    }
    true
}

/// Delivered frame kinds with the MODES body where there is one.
fn adapter_frames(
    adapter: &SharedFakeTerminalAdapter,
) -> Vec<(TerminalKind, Option<ModesBody>, String)> {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .map(|bytes| {
            let frame = adapter_terminal_frame(bytes);
            let modes = (frame.kind() == TerminalKind::Modes)
                .then(|| decode_modes(&frame).expect("modes body"));
            (frame.kind(), modes, adapter_payload_text(bytes))
        })
        .collect()
}

/// Spawn a line-echo session whose echo enables bracketed paste, declare and
/// bind one auto-completing adapter, and pump until the route is attached.
fn bind_declared_mode_echo(
    daemon: &mut CoreDaemon,
    label: &str,
) -> (SessionId, ClientId, SharedFakeTerminalAdapter) {
    let session_id = SessionId(format!("{label}-session"));
    let client_id = ClientId(format!("{label}-client"));
    let subscription_id = SubscriptionId(format!("{label}-sub"));
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "while IFS= read -r line; do printf '\\033[?2004hMODE:%s\\n' \"$line\"; done".to_string();
    daemon.spawn(request, 10).expect("spawn mode echo");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare adapter");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach mode echo");
    let generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind mode echo");
    assert!(
        pump_wakes_until(daemon, || adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_phase(bytes) == Some("attached"))),
        "route must attach"
    );
    (session_id, client_id, adapter)
}

fn modes_daemon(worker: bool, label: &str) -> (CoreDaemon, PathBuf) {
    let data_dir = temp_data_dir(label);
    let config = CoreDaemonConfig::new(&data_dir).with_ghostty_max_scrollback_bytes(0);
    let config = if worker {
        config.with_worker_path(worker_path())
    } else {
        config
    };
    (CoreDaemon::new(config), data_dir)
}

/// A resize that changes only rows and cols publishes MODES.
fn assert_resize_only_publishes_modes(worker: bool, label: &str) {
    let (mut daemon, data_dir) = modes_daemon(worker, label);
    let (session_id, client_id, adapter) = bind_declared_mode_echo(&mut daemon, label);
    daemon
        .resize(client_id, session_id, 31, 101, 20)
        .expect("resize only");
    assert!(
        pump_wakes_until(&mut daemon, || {
            adapter_frames(&adapter)
                .iter()
                .any(|(_, modes, _)| modes.is_some_and(|m| (m.rows, m.cols) == (31, 101)))
        }),
        "a resize-only change must publish MODES: {:?}",
        adapter_frames(&adapter)
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// A mode change carried by PTY output reaches the route before that output.
fn assert_modes_precede_mode_changing_output(worker: bool, label: &str) {
    let (mut daemon, data_dir) = modes_daemon(worker, label);
    let (_, _, adapter) = bind_declared_mode_echo(&mut daemon, label);
    adapter.inject_ingress_frame(compact_input_frame(b"on\n"));
    assert!(
        pump_wakes_until(&mut daemon, || adapter_frames(&adapter).iter().any(
            |(kind, _, text)| *kind == TerminalKind::Output && text.contains("MODE:on")
        )),
        "the mode-changing output must arrive: {:?}",
        adapter_frames(&adapter)
    );
    let frames = adapter_frames(&adapter);
    let modes_at = frames
        .iter()
        .position(|(_, modes, _)| {
            modes.is_some_and(|m| m.mode_bits & mode_bits::BRACKETED_PASTE != 0)
        })
        .unwrap_or(usize::MAX);
    let output_at = frames
        .iter()
        .position(|(kind, _, text)| *kind == TerminalKind::Output && text.contains("\u{1b}[?2004h"))
        .expect("output carrying the mode change");
    assert!(
        modes_at < output_at,
        "MODES must precede the output that changed the mode: {frames:?}"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn in_process_resize_only_publishes_modes() {
    assert_resize_only_publishes_modes(false, "modes-resize-in-process");
}

#[cfg(unix)]
#[test]
fn in_process_modes_precede_mode_changing_output() {
    assert_modes_precede_mode_changing_output(false, "modes-order-in-process");
}

#[cfg(unix)]
#[test]
fn worker_resize_only_publishes_modes() {
    assert_resize_only_publishes_modes(true, "modes-resize-worker");
}

#[cfg(unix)]
#[test]
fn worker_modes_precede_mode_changing_output() {
    assert_modes_precede_mode_changing_output(true, "modes-order-worker");
}

#[cfg(unix)]
#[test]
fn pump_woken_applies_injected_duplex_input_through_real_worker_pty() {
    let data_dir = temp_data_dir("duplex-byte-oracle");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("duplex-byte-oracle-session".to_string());
    let client_id = ClientId("duplex-byte-oracle-client".to_string());
    let subscription_id = SubscriptionId("duplex-byte-oracle-sub".to_string());
    let adapter = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        client_id,
        subscription_id.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    adapter.inject_ingress_frame(compact_input_frame(b"ORACLE\n"));
    let started = Instant::now();
    let mut saw_echo = false;
    let mut saw_result_id = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            21,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
        );
        for bytes in adapter.snapshot_delivered_frame_bytes() {
            if adapter_frame_type(&bytes) == "terminal_output"
                && adapter_payload_text(&bytes).contains("echo:ORACLE")
            {
                saw_echo = true;
            }
        }
        if adapter_input_result_routes(&adapter).contains(&subscription_id.0) {
            saw_result_id = true;
        }
        if saw_echo && saw_result_id {
            break;
        }
    }
    assert!(saw_echo, "injected adapter bytes must reach the worker PTY");
    assert!(
        saw_result_id,
        "input_result must carry the live subscription id"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_reconnects_and_rejects_stale_generation_ingress() {
    let data_dir = temp_data_dir("duplex-reconnect");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("duplex-reconnect-session".to_string());
    let client_id = ClientId("duplex-reconnect-client".to_string());
    let subscription_id = SubscriptionId("duplex-reconnect-sub".to_string());
    let stale = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        client_id.clone(),
        subscription_id.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    stale.inject_ingress_frame(compact_input_frame(b"STALE\n"));
    daemon
        .detach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("detach generation N");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            13,
        )
        .expect("attach generation N+1");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("N+1 inventory")
        .generation;
    let fresh = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(fresh.clone()),
        )
        .expect("bind N+1");
    wait_until_bound_attached(&mut daemon, &session_id, &fresh);
    fresh.inject_ingress_frame(compact_input_frame(b"FRESH\n"));
    let started = Instant::now();
    let mut saw_fresh = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            14,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
        );
        let stale_bytes = stale
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:STALE"));
        assert!(!stale_bytes, "generation N ingress must not reach the PTY");
        if fresh
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:FRESH"))
        {
            saw_fresh = true;
            break;
        }
    }
    assert!(saw_fresh, "generation N+1 must apply fresh adapter input");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_teardown_session_clears_ingress_and_inventory() {
    let data_dir = temp_data_dir("duplex-teardown");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("duplex-teardown-session".to_string());
    let subscription_id = SubscriptionId("duplex-teardown-sub".to_string());
    let adapter = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        ClientId("duplex-teardown-client".to_string()),
        subscription_id.clone(),
        "exec cat >/dev/null",
        10,
    );
    adapter.inject_ingress_frame(compact_input_frame(b"gone\n"));
    daemon
        .shutdown(Some(session_id.clone()), 12)
        .expect("teardown session");
    let _ = daemon.drain(&session_id, 13);
    assert!(daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .all(|row| row.session_id != session_id));
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_writer_failure_sweeps_idle_same_session_owner() {
    let data_dir = temp_data_dir("duplex-writer-sweep");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let failed = SessionId("duplex-writer-failed".to_string());
    let other = SessionId("duplex-writer-other".to_string());
    let idle_sub = SubscriptionId("duplex-writer-idle".to_string());
    let active_sub = SubscriptionId("duplex-writer-active".to_string());
    let other_sub = SubscriptionId("duplex-writer-other-sub".to_string());
    let _idle = bind_echo_worker(
        &mut daemon,
        failed.clone(),
        ClientId("duplex-writer-idle-client".to_string()),
        idle_sub.clone(),
        "exec cat >/dev/null",
        10,
    );
    daemon
        .attach(
            ClientId("duplex-writer-active-client".to_string()),
            failed.clone(),
            active_sub.clone(),
            12,
        )
        .expect("attach second same-session owner");
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == active_sub)
        .expect("active inventory")
        .generation;
    let active = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            ClientId("duplex-writer-active-client".to_string()),
            failed.clone(),
            active_sub.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(active.clone()),
        )
        .expect("bind active owner");
    let other_adapter = bind_echo_worker(
        &mut daemon,
        other.clone(),
        ClientId("duplex-writer-other-client".to_string()),
        other_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        20,
    );
    let (worker_pid, _, _) = worker_process_evidence(&daemon, &failed);
    let _ = Command::new("kill")
        .args(["-9", &worker_pid.to_string()])
        .status()
        .expect("kill worker");
    assert!(
        wait_pid_exit(worker_pid, REAL_WORKER_COMPLETION_TIMEOUT),
        "failed worker exits"
    );
    // No input: the dead worker's reader end wakes the session, and the
    // pump that handles it sweeps every owner of the session.
    let mut saw_inventory_change = false;
    let mut now = 30;
    wait_for(
        "the dead worker's owners swept",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            let gone = daemon
                .list_terminal_subscriptions(1024 * 1024)
                .expect("test inventory allowance")
                .records
                .iter()
                .all(|row| row.subscription_id != idle_sub && row.subscription_id != active_sub);
            if gone {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let outcome = daemon
                    .pump_woken(&batch, now)
                    .expect("pump worker-link failure");
                saw_inventory_change |= outcome.terminal_inventory_changed;
            }
            None
        },
    );
    assert!(
        saw_inventory_change,
        "worker-link failure must report the inventory removal"
    );
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .all(|row| row.subscription_id != idle_sub && row.subscription_id != active_sub),
        "writer failure must sweep every same-session owner"
    );
    // Ingress is work for Core: a transport wakes it. The echo may arrive
    // split across output frames.
    other_adapter.inject_ingress_frame(compact_input_frame(b"LIVE\n"));
    let _ = other_adapter.wake(TerminalWakeKind::Writable);
    wait_for(
        "a different session surviving the loss",
        REAL_WORKER_COMPLETION_TIMEOUT,
        |remaining| {
            let echoed = other_adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .filter(|bytes| adapter_frame_type(bytes) == "terminal_output")
                .map(|bytes| adapter_payload_text(bytes))
                .collect::<String>()
                .contains("echo:LIVE");
            if echoed {
                return Some(());
            }
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                now += 1;
                let _ = daemon
                    .pump_woken(&batch, now)
                    .expect("pump the surviving session");
            }
            None
        },
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_ingress_loss_and_malformed_input_remove_the_route() {
    for (label, inject) in [
        (
            "lost",
            Box::new(|adapter: &SharedFakeTerminalAdapter| {
                adapter.inject_ingress_frame(compact_input_frame(b"keep"));
                adapter.drop_buffered_ingress_frame();
            }) as Box<dyn Fn(&SharedFakeTerminalAdapter)>,
        ),
        (
            "malformed",
            Box::new(|adapter: &SharedFakeTerminalAdapter| {
                adapter.inject_ingress_frame(vec![0xff, 0xff, 0xff]);
            }),
        ),
    ] {
        let data_dir = temp_data_dir(&format!("duplex-{label}"));
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir)
                .with_worker_path(worker_path())
                .with_ghostty_max_scrollback_bytes(0),
        );
        let failed = SessionId(format!("duplex-{label}-fail"));
        let sibling = SessionId(format!("duplex-{label}-sib"));
        let failed_sub = SubscriptionId(format!("duplex-{label}-fail-sub"));
        let sibling_sub = SubscriptionId(format!("duplex-{label}-sib-sub"));
        let adapter = bind_echo_worker(
            &mut daemon,
            failed.clone(),
            ClientId(format!("duplex-{label}-fail-c")),
            failed_sub.clone(),
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
            10,
        );
        let sibling_adapter = bind_echo_worker(
            &mut daemon,
            sibling.clone(),
            ClientId(format!("duplex-{label}-sib-c")),
            sibling_sub.clone(),
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
            20,
        );
        inject(&adapter);
        assert!(pump_next_available_wake(
            &mut daemon,
            30,
            Duration::from_millis(250)
        ));
        assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
        assert!(daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .all(|row| row.subscription_id != failed_sub));
        let after = daemon.drain(&failed, 31).expect("drain after hard-stop");
        assert!(after.client_egress.iter().all(|(_, frame)| {
            !matches!(
                frame,
                TransportEgress::TerminalOutput {
                    subscription_id,
                    ..
                } if subscription_id == &failed_sub
            )
        }));
        sibling_adapter.inject_ingress_frame(compact_input_frame(b"SIB\n"));
        let started = Instant::now();
        let mut saw_sibling = false;
        while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
            pump_next_available_wake(
                &mut daemon,
                32,
                REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
            );
            if sibling_adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .any(|bytes| adapter_payload_text(bytes).contains("echo:SIB"))
            {
                saw_sibling = true;
                break;
            }
        }
        assert!(saw_sibling, "{label} must leave the sibling session live");
        let _ = fs::remove_dir_all(data_dir);
    }
}

fn advertised_ready_then_history() -> TerminalCapabilitySet {
    TerminalCapabilitySet::from_tokens(["snapshot_delivery=ready_then_history"])
        .expect("advertised optional token")
}

fn route_terminal_frames<'a>(
    frames: &'a [(ClientId, TransportEgress)],
    client_id: &ClientId,
    session_id: &SessionId,
    subscription_id: &SubscriptionId,
) -> Vec<&'a TransportEgress> {
    frames
        .iter()
        .filter(|(client, frame)| {
            client == client_id
                && match frame {
                    TransportEgress::TerminalOutput {
                        session_id: routed_session,
                        subscription_id: routed_sub,
                        ..
                    }
                    | TransportEgress::Snapshot {
                        session_id: routed_session,
                        subscription_id: routed_sub,
                        ..
                    }
                    | TransportEgress::Scrollback {
                        session_id: routed_session,
                        subscription_id: routed_sub,
                        ..
                    }
                    | TransportEgress::ProcessExit {
                        session_id: routed_session,
                        subscription_id: routed_sub,
                        ..
                    }
                    | TransportEgress::AttachState {
                        session_id: routed_session,
                        subscription_id: routed_sub,
                        ..
                    } => routed_session == session_id && routed_sub == subscription_id,
                    _ => false,
                }
        })
        .map(|(_, frame)| frame)
        .collect()
}

fn count_production_unsubscribe(
    observations: &[BotsterEngineObservation],
    client_id: &ClientId,
    session_id: &SessionId,
    subscription_id: &SubscriptionId,
) -> usize {
    observations
        .iter()
        .filter(|observation| {
            matches!(
                observation,
                BotsterEngineObservation::Subscription(
                    SubscriptionMultiplexerObservation::ClientStream {
                        client_id: observed_client,
                        observation: ClientStreamObservation::Unsubscribed {
                            session_id: observed_session,
                            subscription_id: observed_sub,
                        },
                    }
                ) if observed_client == client_id
                    && observed_session == session_id
                    && observed_sub == subscription_id
            )
        })
        .count()
}

#[cfg(unix)]
#[test]
fn declared_attach_retains_frames_until_bind_then_delivers_ready_history_finish() {
    let data_dir = temp_data_dir("hold-until-bound-order");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("hold-order-session".to_string());
    let client_id = ClientId("hold-order-client".to_string());
    let subscription_id = SubscriptionId("hold-order-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf 'hold-order-live\\n'; exec cat >/dev/null".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("expect adapter");
    let attached = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("attach");
    assert!(
        route_terminal_frames(
            &attached.client_egress,
            &client_id,
            &session_id,
            &subscription_id
        )
        .is_empty(),
        "declared attach must not extract route frames: {:?}",
        attached.client_egress
    );
    let pre_bind = daemon.drain(&session_id, 12).expect("pre-bind drain");
    assert!(
        route_terminal_frames(
            &pre_bind.client_egress,
            &client_id,
            &session_id,
            &subscription_id
        )
        .is_empty(),
        "declared route must not leak on drain before bind: {:?}",
        pre_bind.client_egress
    );
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after attach")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            advertised_ready_then_history(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    // Attached arrives first on a live attach; history finishes later and
    // live output may interleave. Wait for FINISH within the same bound.
    let started = Instant::now();
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            20,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
        );
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_phase(bytes) == Some("finish"))
        {
            break;
        }
    }
    let mut phases = Vec::new();
    for bytes in adapter.snapshot_delivered_frame_bytes() {
        let frame = adapter_terminal_frame(&bytes);
        match frame.kind() {
            TerminalKind::SnapshotReady => assert!(
                frame.body().starts_with(b"GHOSTSNP"),
                "READY carries the GHOSTSNP prefix"
            ),
            TerminalKind::SnapshotFinish => {
                assert!(frame.body().is_empty(), "FINISH carries no body");
            }
            _ => {}
        }
        if let Some(phase) = adapter_phase(&bytes) {
            phases.push(phase.to_string());
        }
    }
    let ready = phases.iter().position(|phase| phase == "ready");
    let finish = phases.iter().position(|phase| phase == "finish");
    let attached_at = phases.iter().position(|phase| phase == "attached");
    assert!(
        ready.is_some() && finish.is_some() && attached_at.is_some(),
        "declared bind must deliver Attached, READY, and FINISH: {phases:?}"
    );
    let ready = ready.expect("ready");
    let finish = finish.expect("finish");
    let attached_at = attached_at.expect("attached");
    assert!(
        attached_at < ready && ready < finish,
        "live attach order is Attached, READY, FINISH: {phases:?}"
    );
    assert!(
        phases[ready + 1..finish]
            .iter()
            .all(|phase| phase == "history"),
        "only HISTORY pages sit between READY and FINISH: {phases:?}"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn hold_overflow_unsubscribes_through_production_path_and_keeps_sibling() {
    let data_dir = temp_data_dir("hold-overflow-prod");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("hold-overflow-session".to_string());
    let holder = ClientId("hold-overflow-holder".to_string());
    let sibling = ClientId("hold-overflow-sibling".to_string());
    let holder_sub = SubscriptionId("hold-overflow-holder-sub".to_string());
    let sibling_sub = SubscriptionId("hold-overflow-sibling-sub".to_string());
    let sibling_adapter = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        sibling.clone(),
        sibling_sub.clone(),
        "yes hold-overflow",
        10,
    );
    daemon
        .expect_terminal_adapter(holder.clone(), session_id.clone(), holder_sub.clone())
        .expect("expect holder");
    let attached = daemon
        .attach(holder.clone(), session_id.clone(), holder_sub.clone(), 20)
        .expect("attach holder");
    assert!(
        route_terminal_frames(&attached.client_egress, &holder, &session_id, &holder_sub)
            .is_empty()
    );
    let holder_generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == holder_sub)
        .expect("holder inventory after attach")
        .generation;

    // The `yes` flood overflows the never-bound holder's pre-bind hold on
    // the first full queue. The bound below is a failure bound only; the
    // overflow itself arrives within the first few pumps.
    let started = Instant::now();
    let mut unsubscribe_count = 0;
    let mut pumps = 0;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            30,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
        );
        pumps += 1;
        let drained = daemon.drain(&session_id, 30).expect("read overflow result");
        unsubscribe_count +=
            count_production_unsubscribe(&drained.observations, &holder, &session_id, &holder_sub);
        let holder_live = daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.subscription_id == holder_sub);
        if !holder_live {
            break;
        }
    }
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .all(|row| row.subscription_id != holder_sub),
        "first pre-bind overflow must remove the never-bound holder (after {pumps} pumps)"
    );
    assert_eq!(
        unsubscribe_count, 1,
        "the one teardown yields exactly one production unsubscribe observation"
    );
    // Public failure signal: the holder's generation is no longer bindable.
    let late = SharedFakeTerminalAdapter::auto_complete();
    assert!(matches!(
        daemon.bind_waking_terminal_adapter(
            holder.clone(),
            session_id.clone(),
            holder_sub.clone(),
            holder_generation,
            TerminalCapabilitySet::empty(),
            Box::new(late),
        ),
        Err(CoreDaemonError::BindTerminalAdapter(
            BindTerminalAdapterError::UnknownSubscription { .. }
        ))
    ));
    // The failed route remains absent while the flood continues and the
    // sibling keeps receiving live output. (Capture work itself is measured
    // by the unit regression: no resync request remains for the route.)
    let before = sibling_adapter.snapshot_delivered_frame_bytes().len();
    let sibling_started = Instant::now();
    let mut sibling_progress = false;
    let mut extra_unsubscribes = 0;
    while sibling_started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            31,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(sibling_started.elapsed()),
        );
        let drained = daemon.drain(&session_id, 31).expect("drain after overflow");
        extra_unsubscribes +=
            count_production_unsubscribe(&drained.observations, &holder, &session_id, &holder_sub);
        assert!(
            daemon
                .list_terminal_subscriptions(1024 * 1024)
                .expect("test inventory allowance")
                .records
                .iter()
                .all(|row| row.subscription_id != holder_sub),
            "the failed never-bound route remains absent"
        );
        let delivered = sibling_adapter.snapshot_delivered_frame_bytes();
        if delivered[before.min(delivered.len())..]
            .iter()
            .any(|bytes| adapter_terminal_frame(bytes).kind() == TerminalKind::Output)
        {
            sibling_progress = true;
            break;
        }
    }
    assert!(
        sibling_progress,
        "sibling must receive new live OUTPUT after the holder failed"
    );
    assert_eq!(
        extra_unsubscribes, 0,
        "one teardown, one production unsubscribe: none may follow"
    );
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.subscription_id == sibling_sub && row.adapter_bound),
        "sibling must remain bound"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn closed_adapter_at_bind_discards_hold_and_unsubscribes_through_production_path() {
    let data_dir = temp_data_dir("hold-closed-adapter");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("hold-closed-session".to_string());
    let holder = ClientId("hold-closed-holder".to_string());
    let sibling = ClientId("hold-closed-sibling".to_string());
    let holder_sub = SubscriptionId("hold-closed-holder-sub".to_string());
    let sibling_sub = SubscriptionId("hold-closed-sibling-sub".to_string());
    let sibling_adapter = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        sibling.clone(),
        sibling_sub.clone(),
        "printf sibling-live\\n; exec cat >/dev/null",
        10,
    );
    daemon
        .expect_terminal_adapter(holder.clone(), session_id.clone(), holder_sub.clone())
        .expect("expect holder");
    let _ = daemon
        .attach(holder.clone(), session_id.clone(), holder_sub.clone(), 20)
        .expect("attach holder");
    for now in 21..28 {
        let drained = daemon.drain(&session_id, now).expect("accumulate hold");
        assert!(
            route_terminal_frames(&drained.client_egress, &holder, &session_id, &holder_sub)
                .is_empty(),
            "held dump must not leak before bind"
        );
    }
    let generation = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == holder_sub)
        .expect("holder inventory")
        .generation;
    let closed = SharedFakeTerminalAdapter::new();
    closed.close_transport();
    daemon
        .bind_waking_terminal_adapter(
            holder.clone(),
            session_id.clone(),
            holder_sub.clone(),
            generation,
            advertised_ready_then_history(),
            Box::new(closed.clone()),
        )
        .expect("bind closed adapter");
    let started = Instant::now();
    let mut unsubscribe_count = 0;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            40,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(started.elapsed()),
        );
        let drained = daemon
            .drain(&session_id, 40)
            .expect("read closed-bind result");
        unsubscribe_count +=
            count_production_unsubscribe(&drained.observations, &holder, &session_id, &holder_sub);
        if !daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.subscription_id == holder_sub)
            && unsubscribe_count > 0
        {
            break;
        }
    }
    let extra = daemon
        .drain(&session_id, 40)
        .expect("drain after closed bind");
    unsubscribe_count +=
        count_production_unsubscribe(&extra.observations, &holder, &session_id, &holder_sub);
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .all(|row| row.subscription_id != holder_sub),
        "closed adapter must remove the owner"
    );
    assert_eq!(
        unsubscribe_count, 1,
        "closed adapter must run production UnsubscribeSession exactly once"
    );
    assert!(closed.snapshot_delivered_frame_bytes().is_empty());
    assert_eq!(closed.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.subscription_id == sibling_sub && row.adapter_bound),
        "sibling must remain bound"
    );
    let before = sibling_adapter.snapshot_delivered_frame_bytes().len();
    sibling_adapter.inject_ingress_frame(compact_input_frame(b"SIB\n"));
    let sibling_started = Instant::now();
    let mut sibling_progress = false;
    while sibling_started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(
            &mut daemon,
            41,
            REAL_WORKER_COMPLETION_TIMEOUT.saturating_sub(sibling_started.elapsed()),
        );
        if sibling_adapter.snapshot_delivered_frame_bytes().len() > before {
            sibling_progress = true;
            break;
        }
    }
    assert!(
        sibling_progress,
        "sibling must keep delivering after closed-adapter teardown"
    );
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.subscription_id == sibling_sub),
        "sibling must survive closed-adapter teardown"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn foreign_route_drains_while_another_route_holds() {
    let data_dir = temp_data_dir("hold-foreign-drain");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("hold-foreign-session".to_string());
    let holder = ClientId("hold-foreign-holder".to_string());
    let other = ClientId("hold-foreign-other".to_string());
    let holder_sub = SubscriptionId("hold-foreign-holder-sub".to_string());
    let other_sub = SubscriptionId("hold-foreign-other-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf 'foreign-live\\n'; exec cat >/dev/null".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .expect_terminal_adapter(holder.clone(), session_id.clone(), holder_sub.clone())
        .expect("expect holder");
    let _ = daemon
        .attach(holder.clone(), session_id.clone(), holder_sub.clone(), 11)
        .expect("attach holder");
    let other_attached = daemon
        .attach(other.clone(), session_id.clone(), other_sub.clone(), 12)
        .expect("attach foreign");
    assert!(
        !route_terminal_frames(
            &other_attached.client_egress,
            &other,
            &session_id,
            &other_sub
        )
        .is_empty()
            || {
                let drained = daemon
                    .drain_subscription(&other, &session_id, &other_sub, 13)
                    .expect("drain foreign");
                !route_terminal_frames(&drained.client_egress, &other, &session_id, &other_sub)
                    .is_empty()
                    && route_terminal_frames(
                        &drained.client_egress,
                        &holder,
                        &session_id,
                        &holder_sub,
                    )
                    .is_empty()
            },
        "foreign route must drain while the declared route holds"
    );
    let holder_drain = daemon
        .drain_subscription(&holder, &session_id, &holder_sub, 14)
        .expect("drain holder");
    assert!(
        route_terminal_frames(
            &holder_drain.client_egress,
            &holder,
            &session_id,
            &holder_sub
        )
        .is_empty(),
        "holding route must stay empty on drain_subscription"
    );
    let _ = fs::remove_dir_all(data_dir);
}

/// The journal bit, as a host learns it: from its next pump's outcome.
fn journal_advanced(daemon: &mut CoreDaemon) -> bool {
    daemon
        .pump_woken(&TerminalWakeBatch::default(), 1)
        .expect("an empty pump")
        .journal_advanced
}

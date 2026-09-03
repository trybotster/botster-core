#![allow(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::contract::terminal_adapter::TerminalAdapterPressure;
use botster_core::TerminalScreenSize;
use botster_core::{
    BotsterEngineObservation, ClientId, ClientStreamObservation, CoreSessionMetadata, EndpointId,
    EnvelopeCursor, EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget, ModeFlags,
    NotificationContent, NotificationDeliveryStatus, NotificationId, NotificationItem,
    NotificationSeverity, NotificationSource, NotificationTarget, NotificationTimestamp, RequestId,
    ResizePayload, Rgb, RoutedEnvelope, RoutedEnvelopeObservation, RoutedEnvelopePayload,
    RoutedEnvelopeQueueConfig, SessionId, SessionLifecycleState, SessionRuntimeErrorKind,
    SessionSpawnRequest, SessionWorkerHealthReason, SessionWorkerStaleReason, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, SubscriptionMultiplexerObservation, TerminalAttachState,
    TerminalCapabilitySet, TerminalColorProfile, TransportEgress, MAX_CORE_SESSION_METADATA_LEN,
};
use botster_core_daemon::{
    reserved_observe_slice_error, sanitize_observe_slice_error_message,
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest,
    CaptureColorAndSnapshotRequest, CaptureSnapshotRequest, CoreDaemon, CoreDaemonConfig,
    CoreDaemonError, DaemonSession, DrainNotificationsRequest, DrainRoutedEnvelopesRequest,
    GuardedWriteDecision, GuardedWriteDeliveryState, GuardedWriteRequest, LifecycleBaselineBudget,
    ModeGatedInputOutcome, ObserveLifecycleBudget, ObserveLifecycleCursor, ObserveLifecyclePassId,
    ObserveLifecycleSlice, PostNotificationRequest, PublishRoutedEnvelopeRequest,
    ReadModeFlagsRequest, ReadScreenRequest, ReadinessEvidence, RegistryRecord,
    RegistrySessionState, SafeWriteIndicator, SessionAdoptionState, SessionLifecycleBaseline,
    SessionLifecycleChangeKind, SessionLifecycleChanges, SessionLifecycleCursor,
    SessionLifecycleLookup, SessionLifecyclePage, SessionLifecyclePageError,
    SessionLifecycleRecord, SessionLifecycleResyncReason, SessionLifecycleSourceId,
    SessionRegistryStateLookup, SpawnSessionRequest, TerminalSubscriptionGeneration,
    OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES,
};
use botster_core_daemon::{
    DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY,
};
use botster_core_test_support::conformance::{
    assert_color_profile_authority, assert_ghostty_snapshot_authority, assert_mode_flags_authority,
    GHOSTSNP_MAGIC,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttyClientProjection, GhosttySnapshotDecodeProgress, GhosttyTerminal,
    COLOR_INDEX_BACKGROUND, COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND,
};

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
fn pump_woken_ignore_payload_hub_shape() {
    let data_dir = temp_data_dir("pump-ignore-payload");
    let session_id = SessionId("pump-ignore-payload".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn output worker");
    daemon
        .attach(
            ClientId("pump-ignore-client".to_string()),
            session_id.clone(),
            SubscriptionId("pump-ignore-subscription".to_string()),
            11,
        )
        .expect("attach unbound consumer");

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
    for (session_id, marker) in [(&session_a, "MIXED-A"), (&session_b, "MIXED-B")] {
        let mut request = delayed_output_exit_spawn_request(session_id);
        request.request.arguments[1] = format!("sleep 0.1; printf {marker}; exit 0");
        daemon.spawn(request, 10).expect("spawn mixed session");
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

    thread::sleep(Duration::from_millis(200));
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert!(batch.ingress_sessions.contains(&session_a));
    assert!(batch.ingress_sessions.contains(&session_b));
    let _ = daemon.pump_woken(&batch, 20).expect("pump mixed batch");

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

#[test]
fn pump_failure_retains_failing_session_output_once() {
    let data_dir = temp_data_dir("pump-failure-retention");
    let session_id = SessionId("pump-failure-retention".to_string());
    let client_id = ClientId("pump-failure-retention-client".to_string());
    let failure_text = "test-injected pump retention failure";
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_runtime_drain_for(Some(session_id.clone()))
            .with_test_fail_runtime_drain_message(Some(failure_text.to_string())),
    );
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn output session");
    daemon
        .attach(
            client_id,
            session_id.clone(),
            SubscriptionId("pump-failure-retention-sub".to_string()),
            11,
        )
        .expect("attach output session");

    thread::sleep(Duration::from_millis(300));
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    let error = daemon
        .pump_woken(&batch, 20)
        .expect_err("inject pump failure");
    assert!(error.to_string().contains(failure_text));

    let first = daemon
        .drain(&session_id, 21)
        .expect("drain retained output");
    assert_retained_exit_output(&first, &session_id, "PUMP-RETAINED");
    let second = daemon
        .drain(&session_id, 22)
        .expect("second retained drain");
    assert_no_duplicate_exit_output(&second, "PUMP-RETAINED");
    let _ = daemon.remove_session(&session_id);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn final_state_retention_failure_rearms_and_later_commit_retires_the_wake() {
    let data_dir = short_temp_data_dir("pump-final-retention-retry");
    let session_id = SessionId("pump-final-retention-retry".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_fail_retain_final_terminal_state_for(Some(session_id.clone())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived worker");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    let deadline = Instant::now() + Duration::from_secs(5);
    let injected = loop {
        assert!(
            Instant::now() < deadline,
            "injected final-state failure did not run"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if !batch.ingress_sessions.contains(&session_id) {
            continue;
        }
        match daemon.pump_woken(&batch, 20) {
            Err(error)
                if error
                    .to_string()
                    .contains("test-injected final terminal state retention failure") =>
            {
                break error;
            }
            Ok(_) => {}
            Err(error) => panic!("unexpected pump error: {error}"),
        }
    };
    assert!(injected
        .to_string()
        .contains("test-injected final terminal state retention failure"));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    let retry = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(retry.ingress_sessions, vec![session_id.clone()]);
    let _ = daemon.pump_woken(&retry, 21).expect("retry commit");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("retry exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("final-retention journal page");
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
fn local_final_state_failure_rearms_and_later_commit_retires_the_wake() {
    let data_dir = temp_data_dir("local-pump-final-retention-retry");
    let session_id = SessionId("local-pump-final-retention-retry".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_retain_final_terminal_state_for(Some(session_id.clone())),
    );
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived local process");
    daemon
        .attach(
            ClientId("local-pump-final-retention-client".to_string()),
            session_id.clone(),
            SubscriptionId("local-pump-final-retention-sub".to_string()),
            11,
        )
        .expect("attach local final-retention session");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "injected local final-state failure did not run"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if !batch.ingress_sessions.contains(&session_id) {
            continue;
        }
        match daemon.pump_woken(&batch, 20) {
            Err(error)
                if error
                    .to_string()
                    .contains("test-injected final terminal state retention failure") =>
            {
                break;
            }
            Ok(_) => {}
            Err(error) => panic!("unexpected local pump error: {error}"),
        }
    }
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    let retry = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(retry.ingress_sessions, vec![session_id.clone()]);
    let _ = daemon.pump_woken(&retry, 21).expect("retry local commit");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("local retry exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let retained = daemon
        .drain(&session_id, 22)
        .expect("retained local final output");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 23)
            .expect("second local final drain"),
        "PUMP-RETAINED",
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn obligation_registry_read_failure_rearms_the_exact_session() {
    let data_dir = temp_data_dir("obligation-registry-read-retry");
    let session_id = SessionId("obligation-registry-read-retry".to_string());
    let record_path = data_dir
        .join("sessions")
        .join(format!("{}.json", session_id.0));
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_retain_final_terminal_state_for(Some(session_id.clone())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn obligation read process");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "initial final-state failure did not run"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if !batch.ingress_sessions.contains(&session_id) {
            continue;
        }
        let error = daemon
            .pump_woken(&batch, 20)
            .expect_err("inject initial final-state failure");
        assert!(error
            .to_string()
            .contains("test-injected final terminal state retention failure"));
        break;
    }

    let retry = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(retry.ingress_sessions, vec![session_id.clone()]);
    let original_mode = fs::metadata(&record_path)
        .expect("registry record metadata")
        .permissions()
        .mode();
    let mut unreadable = fs::metadata(&record_path)
        .expect("registry record metadata")
        .permissions();
    unreadable.set_mode(0o000);
    fs::set_permissions(&record_path, unreadable).expect("make registry record unreadable");
    let probe = fs::read(&record_path);
    if probe.is_ok() {
        let mut restored = fs::metadata(&record_path)
            .expect("registry record metadata")
            .permissions();
        restored.set_mode(original_mode);
        fs::set_permissions(&record_path, restored).expect("restore registry permissions");
        panic!("unreadable registry record accepted a probe read");
    }

    let error = daemon
        .pump_woken(&retry, 21)
        .expect_err("obligation registry read must fail");
    let mut restored = fs::metadata(&record_path)
        .expect("registry record metadata")
        .permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(&record_path, restored).expect("restore registry permissions");
    assert!(error.to_string().contains("Permission denied"));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    let second_retry = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(second_retry.ingress_sessions, vec![session_id.clone()]);
    daemon
        .pump_woken(&second_retry, 22)
        .expect("commit preserved obligation");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("obligation retry exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn persistence_failure_rearms_and_later_commit_retires_the_wake() {
    let data_dir = temp_data_dir("pump-persistence-retry");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("pump-persistence-retry".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
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

    let deadline = Instant::now() + Duration::from_secs(15);
    let failure = loop {
        assert!(
            Instant::now() < deadline,
            "registry persistence did not fail"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
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
fn pump_persist_resize_error_retains_once() {
    let data_dir = temp_data_dir("pump-resize-persistence");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("pump-resize-persistence".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_applied_attach_resize(Some((
            session_id.clone(),
            40,
            120,
            12,
        ))),
    );
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn resize process");
    daemon
        .attach(
            ClientId("pump-resize-client".to_string()),
            session_id.clone(),
            SubscriptionId("pump-resize-sub".to_string()),
            11,
        )
        .expect("attach resize session");

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
    thread::sleep(Duration::from_millis(300));
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(batch.ingress_sessions, vec![session_id.clone()]);
    let failure = daemon
        .pump_woken(&batch, 20)
        .expect_err("resize persistence must fail");
    let mut restored = fs::metadata(&sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(&sessions_dir, restored).expect("restore sessions permissions");

    assert!(failure.to_string().contains("Permission denied"));
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resize record")
        .expect("resize registry row");
    assert_eq!((record.rows, record.cols), (24, 80));
    let retained = daemon
        .drain(&session_id, 30)
        .expect("drain retained resize output");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 31)
            .expect("second retained resize drain"),
        "PUMP-RETAINED",
    );
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn shutdown_watchdog_timeout_retains_once() {
    let data_dir = temp_data_dir("shutdown-watchdog-retention");
    let session_id = SessionId("shutdown-watchdog-retention".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_force_shutdown_watchdog_for(Some(session_id.clone())),
    );
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn watchdog process");
    daemon
        .attach(
            ClientId("shutdown-watchdog-client".to_string()),
            session_id.clone(),
            SubscriptionId("shutdown-watchdog-sub".to_string()),
            11,
        )
        .expect("attach watchdog session");
    thread::sleep(Duration::from_millis(300));

    let error = daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect_err("inject daemon shutdown watchdog");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(
            botster_core::SessionRuntimeError {
                kind: botster_core::SessionRuntimeErrorKind::ShutdownFailed,
                ref message,
            }
        )) if message.contains("test-injected daemon shutdown watchdog timeout")
    ));
    let retained = daemon
        .drain(&session_id, 21)
        .expect("drain watchdog output");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 22)
            .expect("second watchdog drain"),
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
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn shutdown registry process");
    daemon
        .attach(
            ClientId("shutdown-registry-client".to_string()),
            session_id.clone(),
            SubscriptionId("shutdown-registry-sub".to_string()),
            11,
        )
        .expect("attach shutdown registry session");
    thread::sleep(Duration::from_millis(300));

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
        thread::sleep(Duration::from_millis(10));
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

#[test]
fn direct_drain_failure_preserves_obligation_and_wake() {
    let data_dir = temp_data_dir("direct-drain-failure");
    let session_id = SessionId("direct-drain-failure".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_retain_final_terminal_state_for(Some(session_id.clone())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn direct failure process");
    thread::sleep(Duration::from_millis(50));

    let error = daemon
        .drain(&session_id, 20)
        .expect_err("direct drain final-state failure");
    assert!(error
        .to_string()
        .contains("test-injected final terminal state retention failure"));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    daemon
        .drain(&session_id, 21)
        .expect("direct drain retry commit");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("direct retry exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
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
        thread::sleep(Duration::from_millis(10));
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
fn readback_exit_records_obligation_and_rearms() {
    let data_dir = temp_data_dir("readback-exit-obligation");
    let session_id = SessionId("readback-exit-obligation".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(delayed_output_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived local process");
    daemon
        .attach(
            ClientId("readback-exit-client".to_string()),
            session_id.clone(),
            SubscriptionId("readback-exit-sub".to_string()),
            11,
        )
        .expect("attach readback session");
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;

    thread::sleep(Duration::from_millis(300));
    daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("readback-exit-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 20,
        })
        .expect("readback consumes process exit");
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(batch.ingress_sessions, vec![session_id.clone()]);
    let _ = daemon
        .pump_woken(&batch, 21)
        .expect("commit readback obligation");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("readback exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let changes = daemon
        .lifecycle_changes_page(&after_spawn, 16, 64 * 1024)
        .expect("readback journal page");
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
        .expect("readback retained drain");
    assert_retained_exit_output(&retained, &session_id, "PUMP-RETAINED");
    assert_no_duplicate_exit_output(
        &daemon
            .drain(&session_id, 23)
            .expect("second readback drain"),
        "PUMP-RETAINED",
    );
    assert!(daemon.remove_session(&session_id).expect("remove exited"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn pump_failure_does_not_block_a_later_sibling() {
    let data_dir = temp_data_dir("pump-failure-sibling");
    let earlier = SessionId("a-pump-failure".to_string());
    let later = SessionId("b-pump-exit".to_string());
    let failure_text = "test-injected targeted pump failure";
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_runtime_drain_for(Some(earlier.clone()))
            .with_test_fail_runtime_drain_message(Some(failure_text.to_string())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&earlier), 10)
        .expect("spawn earlier process");
    daemon
        .spawn(immediate_exit_spawn_request(&later), 11)
        .expect("spawn later process");

    thread::sleep(Duration::from_millis(50));
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert!(batch.ingress_sessions.contains(&earlier));
    assert!(batch.ingress_sessions.contains(&later));
    let error = daemon
        .pump_woken(&batch, 20)
        .expect_err("earlier session must fail");
    assert!(error.to_string().contains(failure_text));
    assert!(matches!(
        daemon
            .session_registry_state(&later)
            .expect("later exact lookup"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    daemon.remove_session(&earlier).expect("remove earlier");
    assert!(daemon.remove_session(&later).expect("remove later"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn repeated_failure_stops_rearming_at_the_bound() {
    let data_dir = temp_data_dir("pump-failure-bound");
    let session_id = SessionId("pump-failure-bound".to_string());
    let failure_text = "test-injected bounded pump failure";
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_test_fail_runtime_drain_for(Some(session_id.clone()))
            .with_test_fail_runtime_drain_message(Some(failure_text.to_string())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn short-lived process");

    for attempt in 1..=3 {
        let batch = daemon.wait_wakes(Duration::from_secs(1));
        assert_eq!(
            batch.ingress_sessions,
            vec![session_id.clone()],
            "attempt {attempt} must receive one exact session wake"
        );
        let error = daemon
            .pump_woken(&batch, 20 + attempt)
            .expect_err("injected pump failure");
        assert!(error.to_string().contains(failure_text));
    }

    let after_bound = daemon.wait_wakes(Duration::from_millis(50));
    assert!(after_bound.ingress_sessions.is_empty());
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    daemon
        .remove_session(&session_id)
        .expect("remove failed session");
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn reused_session_id_starts_with_a_clean_failure_counter() {
    let data_dir = temp_data_dir("reused-pump-failure-counter");
    let session_id = SessionId("reused-pump-failure-counter".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_fail_runtime_drain_for(Some(session_id.clone())),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("spawn first session");
    for attempt in 1..=3 {
        let batch = daemon.wait_wakes(Duration::from_secs(1));
        assert!(batch.ingress_sessions.contains(&session_id));
        daemon
            .pump_woken(&batch, 20 + attempt)
            .expect_err("reach the first session failure bound");
    }
    assert!(daemon
        .wait_wakes(Duration::from_millis(50))
        .ingress_sessions
        .is_empty());
    let mut record = daemon
        .registry()
        .load(&session_id)
        .expect("load first session")
        .expect("first registry row");
    record.mark(RegistrySessionState::Stale, 29);
    daemon
        .registry()
        .save(&record)
        .expect("mark first session stale");
    assert!(daemon
        .remove_session(&session_id)
        .expect("remove first session"));

    let mut reused_request = spawn_request(&session_id);
    reused_request.request.arguments[1] = "sleep 30".to_string();
    daemon
        .spawn(reused_request, 30)
        .expect("spawn reused session id");
    daemon.wake_source().notify_session(&session_id);
    let first = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&first, 31)
        .expect_err("first reused-session failure");
    let rearmed = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(rearmed.ingress_sessions, vec![session_id.clone()]);

    daemon
        .remove_session(&session_id)
        .expect("remove reused session");
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn genuine_adapter_wake_after_the_bound_applies_teardown() {
    let data_dir = temp_data_dir("genuine-wake-after-bound");
    let session_id = SessionId("genuine-wake-after-bound".to_string());
    let client_id = ClientId("genuine-wake-client".to_string());
    let subscription_id = SubscriptionId("genuine-wake-sub".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_fail_runtime_drain_for(Some(session_id.clone())),
    );
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "sleep 30".to_string();
    daemon.spawn(request, 10).expect("spawn live session");
    daemon.wake_source().notify_session(&session_id);
    for attempt in 1..=3 {
        let batch = daemon.wait_wakes(Duration::from_secs(1));
        daemon
            .pump_woken(&batch, 20 + attempt)
            .expect_err("reach the re-arm bound");
    }
    assert!(daemon
        .wait_wakes(Duration::from_millis(50))
        .ingress_sessions
        .is_empty());

    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            30,
        )
        .expect("attach after bound");
    let generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("live generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");
    adapter.close_transport();

    let genuine = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(genuine.adapter_routes.len(), 1);
    assert_eq!(genuine.adapter_routes[0].session_id, session_id);
    daemon
        .pump_woken(&genuine, 31)
        .expect_err("configured failure follows adapter teardown");
    assert!(daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .is_none());

    daemon
        .remove_session(&session_id)
        .expect("remove live session");
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
        data_dir
            .join("sessions")
            .join("daemon-api-session.json")
            .exists(),
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
    let _ = drain_until(&mut daemon, &session_id, "ready");

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
    assert_eq!(
        daemon
            .routed_envelope_delivery_state(&slow, &EnvelopeId("env-2".to_string()))
            .state
            .expect("slow delivery state should exist")
            .status,
        EnvelopeDeliveryStatus::Backpressured
    );

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
fn worker_backed_read_screen_drains_before_read_and_preserves_client_egress_once() {
    let data_dir = temp_data_dir("dwrs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwrs-session".to_string());
    let client_id = ClientId("dwrs-client".to_string());
    let subscription_id = SubscriptionId("dwrs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"screen-marker\n".to_vec(),
            12,
        )
        .expect("marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:screen-marker", 13);
    assert_eq!(
        screen.screen.request_id,
        RequestId("read-screen-marker".to_string())
    );
    assert_eq!(screen.screen.session_id, session_id);
    assert!(
        screen.screen.text.contains("echo:screen-marker"),
        "read_screen should internally drain before reading: {:?}",
        screen.screen.text
    );

    let drained = daemon
        .drain(&session_id, 30)
        .expect("drain after read_screen should succeed");
    let output = terminal_output(&drained.client_egress);
    assert_eq!(
        count_occurrences(&output, "echo:screen-marker"),
        1,
        "internal read_screen drain must retain client egress exactly once: {output:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_capture_snapshot_drains_before_capture_and_preserves_client_egress_once() {
    let data_dir = temp_data_dir("dwcs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwcs-session".to_string());
    let client_id = ClientId("dwcs-client".to_string());
    let subscription_id = SubscriptionId("dwcs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"snapshot-marker\n".to_vec(),
            12,
        )
        .expect("marker input should write");

    let (captured, output) = capture_snapshot_and_retained_output_until(
        &mut daemon,
        &session_id,
        "echo:snapshot-marker",
        13,
    );
    assert_eq!(
        captured.snapshot.request_id,
        RequestId("capture-snapshot-marker".to_string())
    );
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_snapshot_format(&captured.payload);
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert_snapshot_payload_observed_marker_when_plain(&captured.payload, "echo:snapshot-marker");
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:snapshot-marker");
    assert_eq!(
        count_occurrences(&output, "echo:snapshot-marker"),
        1,
        "internal capture_snapshot drain must retain client egress exactly once: {output:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn local_daemon_read_screen_and_capture_snapshot_use_configured_terminal_backend() {
    let data_dir = temp_data_dir("dlrs");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("dlrs-session".to_string());
    let client_id = ClientId("dlrs-client".to_string());
    let subscription_id = SubscriptionId("dlrs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("local daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("local daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"local-marker\n".to_vec(),
            12,
        )
        .expect("local marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:local-marker", 13);
    assert_eq!(screen.screen.session_id, session_id);
    assert!(
        screen.screen.text.contains("echo:local-marker"),
        "local daemon read_screen should use the configured terminal backend"
    );

    let captured = capture_snapshot_until(&mut daemon, &session_id, "echo:local-marker", 14);
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_snapshot_format(&captured.payload);
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert_snapshot_payload_observed_marker_when_plain(&captured.payload, "echo:local-marker");
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:local-marker");

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_default_path_uses_ghostty_terminal_fidelity() {
    let data_dir = temp_data_dir("dwgf");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgf-session".to_string());
    let client_id = ClientId("dwgf-client".to_string());
    let subscription_id = SubscriptionId("dwgf-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"\x1b[31mghostty-red\x1b[0m\n".to_vec(),
            12,
        )
        .expect("styled marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:ghostty-red", 13);
    assert!(
        !screen.screen.text.contains("\x1b["),
        "Ghostty screen reads should format terminal state as plain text, not raw VT bytes: {:?}",
        screen.screen.text
    );

    let captured = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("ghostty-fidelity-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("Ghostty-backed daemon should capture snapshot");
    assert_snapshot_format(&captured.payload);
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:ghostty-red");
    assert!(
        captured.payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        captured.payload.bytes.len()
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_ghostty_same_session_reattach_restores_retained_history() {
    let data_dir = temp_data_dir("dwgrh");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgrh-session".to_string());
    let original_client = ClientId("dwgrh-original-client".to_string());
    let original_subscription = SubscriptionId("dwgrh-original-subscription".to_string());
    let reattached_client = ClientId("dwgrh-reattached-client".to_string());
    let reattached_subscription = SubscriptionId("dwgrh-reattached-subscription".to_string());
    let prior_marker = "echo:dwgrh-prior-marker";
    let visible_marker = "echo:dwgrh-visible-marker";
    let live_marker = "echo:dwgrh-live-marker";

    daemon
        .spawn(retained_history_spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    let authoritative = read_screen_until(&mut daemon, &session_id, prior_marker, 11);
    assert!(
        authoritative.screen.text.contains(prior_marker),
        "authoritative terminal state must contain the startup marker before attach: {:?}",
        authoritative.screen.text
    );
    daemon
        .attach(
            original_client.clone(),
            session_id.clone(),
            original_subscription.clone(),
            12,
        )
        .expect("original client should attach");
    let _ = daemon
        .drain(&session_id, 13)
        .expect("original attach bootstrap should drain");
    daemon
        .input(
            original_client.clone(),
            session_id.clone(),
            b"dwgrh-visible-marker\n".to_vec(),
            14,
        )
        .expect("second visible marker should write");
    let _ = drain_until(&mut daemon, &session_id, visible_marker);

    let authoritative = read_screen_until(&mut daemon, &session_id, visible_marker, 15);
    assert!(
        authoritative.screen.text.contains(prior_marker),
        "authoritative terminal state must contain the prior marker before detach: {:?}",
        authoritative.screen.text
    );

    daemon
        .detach(
            original_client.clone(),
            session_id.clone(),
            original_subscription.clone(),
            16,
        )
        .expect("original client should detach");
    let reattached = daemon
        .attach(
            reattached_client.clone(),
            session_id.clone(),
            reattached_subscription.clone(),
            17,
        )
        .expect("fresh client and subscription should reattach the running session");
    let reattach_drain = drain_until_attached(&mut daemon, &session_id, &reattached_client);
    let mut reattach_egress = reattached.client_egress;
    reattach_egress.extend(reattach_drain.client_egress);
    let (snapshot_index, snapshot) =
        first_snapshot_for_client(&reattach_egress, &reattached_client)
            .expect("reattach should deliver the authoritative Ghostty snapshot");
    let snapshot_subscription = match &reattach_egress[snapshot_index] {
        (
            _,
            TransportEgress::Snapshot {
                subscription_id, ..
            },
        ) => subscription_id,
        _ => unreachable!("snapshot helper returned a non-snapshot frame"),
    };
    assert_eq!(
        snapshot_subscription, &reattached_subscription,
        "retained snapshot must target the fresh reattach subscription"
    );
    let replayed = ghostty_snapshot_plain_text(&snapshot);
    assert_eq!(
        count_occurrences(&replayed, prior_marker),
        1,
        "reattached snapshot should replay the prior marker exactly once: {replayed:?}"
    );
    assert_eq!(
        count_occurrences(&replayed, visible_marker),
        1,
        "reattached snapshot should replay the second visible marker exactly once: {replayed:?}"
    );
    assert!(
        replayed.find(prior_marker) < replayed.find(visible_marker),
        "reattached snapshot should preserve marker order: {replayed:?}"
    );

    let attaching_index = reattach_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &reattached_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &reattached_subscription
                )
        })
        .expect("fresh subscription should receive Attaching");
    let attached_index = reattach_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &reattached_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attached,
                        ..
                    } if subscription_id == &reattached_subscription
                )
        })
        .expect("fresh subscription should receive Attached");
    assert!(
        attaching_index < snapshot_index && snapshot_index < attached_index,
        "reattach bootstrap must order Attaching, retained snapshot, then Attached: {:?}",
        reattach_egress
    );
    assert!(
        reattach_egress
            .iter()
            .all(|(client_id, _)| client_id != &original_client),
        "detached client must not receive reattach-only delivery: {:?}",
        reattach_egress
    );

    let post_attach_drain = daemon
        .drain(&session_id, 18)
        .expect("post-attach drain should succeed");
    assert!(post_attach_drain
        .client_egress
        .iter()
        .all(|(client_id, frame)| {
            client_id != &reattached_client
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &reattached_subscription
                )
        }));

    daemon
        .input(
            reattached_client.clone(),
            session_id.clone(),
            b"dwgrh-live-marker\n".to_vec(),
            19,
        )
        .expect("post-attach live marker should write");
    let live_drain =
        drain_until_for_client(&mut daemon, &session_id, &reattached_client, live_marker);
    assert_eq!(
        count_occurrences(
            &renderable_output_for_client(&live_drain.client_egress, &reattached_client),
            live_marker,
        ),
        1,
        "post-attach live marker should be delivered exactly once"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output() {
    let data_dir = temp_data_dir("worker-incremental-contract");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1)),
    );
    let session_id = SessionId("worker-incremental-contract-session".to_string());
    let client_id = ClientId("worker-incremental-contract-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-contract-sub".to_string());
    let ready_path = data_dir.join("history-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'PRE-BARRIER-MARKER'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );

    daemon.spawn(request, 10).expect("spawn real worker");
    wait_for_file(&ready_path);
    // Capacity-one worker egress can block the child on leftover producer
    // output. Drain that output before attach so later queued input can
    // reach the child's read loop instead of only echoing on the terminal.
    drain_pre_attach_producer_output(&mut daemon, &session_id, 11);
    let recovery = daemon
        .registry()
        .load(&session_id)
        .expect("load worker record")
        .and_then(|record| record.recovery_identity)
        .expect("worker recovery identity");
    assert_eq!(
        recovery
            .get("snapshot_delivery")
            .and_then(serde_json::Value::as_str),
        Some("ready_then_history")
    );

    let attached = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start incremental attach");
    assert_eq!(attached.client_egress.len(), 1);
    assert!(matches!(
        &attached.client_egress[0],
        (
            target,
            TransportEgress::AttachState {
                state: TerminalAttachState::Attaching,
                ..
            }
        ) if target == &client_id
    ));

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"POST-BARRIER-MARKER\n".to_vec(),
            13,
        )
        .expect("queue input");
    daemon
        .resize(client_id.clone(), session_id.clone(), 30, 90, 14)
        .expect("queue first resize");
    daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 15)
        .expect("replace queued resize");

    let mut projection = GhosttyClientProjection::new(TerminalScreenSize::new(24, 80))
        .expect("create incremental client");
    let mut sequence = vec!["attaching"];
    let mut history_frames = 0;
    let mut saw_ready = false;
    let mut saw_finish = false;
    let mut saw_attached = false;
    let mut all_egress = attached.client_egress;
    for tick in 0..10_000 {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("drain one incremental frame");
        let snapshots_in_drain = drained
            .client_egress
            .iter()
            .filter(|(target, frame)| {
                target == &client_id && matches!(frame, TransportEgress::Snapshot { .. })
            })
            .count();
        assert!(snapshots_in_drain <= 1, "one client-paced frame per drain");
        for (target, frame) in &drained.client_egress {
            if target != &client_id {
                continue;
            }
            match frame {
                TransportEgress::Snapshot { data, .. } if !saw_ready => {
                    assert_eq!(
                        projection
                            .install_ghostsnp_ready(data.clone())
                            .expect("READY must decode"),
                        GhosttySnapshotDecodeProgress::Ready
                    );
                    let ready_viewport = projection.project_viewport().expect("paint at READY");
                    assert_eq!(ready_viewport.rows, 24);
                    assert_eq!(ready_viewport.cols, 80);
                    assert!(viewport_contains_marker(
                        &ready_viewport,
                        "PRE-BARRIER-MARKER"
                    ));
                    saw_ready = true;
                    sequence.push("ready");
                }
                TransportEgress::Snapshot { data, .. } => {
                    match projection
                        .apply_ghostsnp_history(data.clone())
                        .expect("decode one PAGE or FINISH")
                    {
                        GhosttySnapshotDecodeProgress::History => {
                            history_frames += 1;
                            sequence.push("history");
                        }
                        GhosttySnapshotDecodeProgress::Finish => {
                            saw_finish = true;
                            sequence.push("finish");
                        }
                        GhosttySnapshotDecodeProgress::Ready => {
                            panic!("READY must occur once")
                        }
                    }
                }
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } => {
                    assert!(saw_finish, "Attached must follow FINISH");
                    saw_attached = true;
                    sequence.push("attached");
                }
                TransportEgress::TerminalOutput { .. } => {
                    assert!(saw_attached, "live output must follow Attached")
                }
                _ => {}
            }
        }
        all_egress.extend(drained.client_egress);
        if saw_attached {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(saw_ready, "worker must emit READY");
    assert!(history_frames > 0, "large history must emit PAGE frames");
    assert!(saw_finish, "FINISH must map from decoder NO_VALUE");
    assert!(saw_attached, "attach must complete");
    assert_eq!(sequence.first(), Some(&"attaching"));
    assert_eq!(sequence.last(), Some(&"attached"));

    let barrier_resized = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("incremental-barrier-resize-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 49,
        })
        .expect("capture immediately after Attached");
    assert_eq!(
        barrier_resized.payload.size,
        TerminalScreenSize::new(40, 120),
        "the worker must apply the latest resize before Attached"
    );

    let live = drain_until_for_client(
        &mut daemon,
        &session_id,
        &client_id,
        "echo:POST-BARRIER-MARKER",
    );
    all_egress.extend(live.client_egress);
    let attached_index = all_egress
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
        .expect("Attached index");
    let post_index = first_terminal_output_index_for_client_containing(
        &all_egress,
        &client_id,
        "echo:POST-BARRIER-MARKER",
    )
    .expect("queued input output");
    assert!(attached_index < post_index);

    let resized = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("incremental-resize-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 50,
        })
        .expect("capture after queued resize");
    assert_eq!(resized.payload.size, TerminalScreenSize::new(40, 120));
    assert_ghostty_snapshot_replays_marker(&resized.payload, "echo:POST-BARRIER-MARKER");
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
                        .install_ghostsnp_ready(data)
                        .expect("blank READY"),
                    GhosttySnapshotDecodeProgress::Ready
                );
                sequence.push("ready");
            }
            TransportEgress::Snapshot { data, .. } => {
                assert_eq!(
                    projection
                        .apply_ghostsnp_history(data)
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
        .list_terminal_subscriptions()
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
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            let _ = daemon
                .pump_woken(&batch, 20)
                .expect("pump bound worker wake");
        }
        for bytes in adapter.snapshot_delivered_frame_bytes() {
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("opaque frame is JSON");
            match value.get("type").and_then(serde_json::Value::as_str) {
                Some("snapshot") => {
                    if let Some(phase) = value.get("phase").and_then(serde_json::Value::as_str) {
                        if !phases.iter().any(|seen| seen == phase) {
                            phases.push(phase.to_string());
                        }
                    }
                }
                Some("attach_state") => {
                    if value.get("state").and_then(serde_json::Value::as_str) == Some("attached")
                        && !phases.iter().any(|seen| seen == "attached")
                    {
                        phases.push("attached".to_string());
                    }
                }
                Some("terminal_output") => {
                    if sent_live_input {
                        saw_live = true;
                    }
                }
                _ => {}
            }
        }
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
        if phases
            .windows(2)
            .any(|window| window == ["ready", "finish"])
            && saw_live
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        phases
            .windows(2)
            .any(|window| window == ["ready", "finish"]),
        "bound adapter must receive READY then FINISH: {phases:?}"
    );
    assert!(saw_live, "bound adapter must receive live output");
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
    const LIVE_B64: &str = "TElWRQ==";
    let data_dir = temp_data_dir("bound-exit-rounds");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0)
            .with_test_hold_before_exit_ms(Some(2_000)),
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
            .list_terminal_subscriptions()
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
        wait_for_condition(&format!("round {round} PTY child exit"), || {
            !process_exists(pty_child_pid)
        });
        // Worker writer emits FRAME_PROCESS_EXITED before the hold starts.
        thread::sleep(Duration::from_millis(150));

        let mut saw_live = false;
        for tick in 0..80 {
            let batch = daemon.wait_wakes(Duration::from_millis(250));
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                let _ = daemon
                    .pump_woken(&batch, 13 + round + tick)
                    .unwrap_or_else(|error| panic!("round {round} pump: {error:?}"));
            }
            complete_one_slot_if_full(&adapter);
            if adapter_has_live(&adapter, LIVE_B64) {
                saw_live = true;
                if adapter_has_process_exit(&adapter) {
                    break;
                }
            }
        }
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
                    && (adapter_payload_b64(bytes) == LIVE_B64
                        || adapter_payload_text(bytes).contains("LIVE"))
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
    const LIVE_B64: &str = "TElWRQ==";
    let data_dir = temp_data_dir("bound-exit-during-attach");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1))
            .with_test_hold_before_exit_ms(Some(2_000)),
    );
    let session_id = SessionId("bound-exit-during-attach".to_string());
    let client_id = ClientId("bound-exit-during-attach-client".to_string());
    let subscription_id = SubscriptionId("bound-exit-during-attach-sub".to_string());
    let ready_path = data_dir.join("history-ready");
    let release_path = data_dir.join("go");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'PRE-BARRIER-MARKER'; : > '{}'; ",
            "while [ ! -f '{}' ]; do sleep 0.05; done; ",
            "printf LIVE; exit 0"
        ),
        ready_path.display(),
        release_path.display()
    );

    daemon.spawn(request, 10).expect("spawn history then wait");
    wait_for_file(&ready_path);
    drain_pre_attach_producer_output(&mut daemon, &session_id, 11);
    let (_worker_pid, pty_child_pid, _) = worker_process_evidence(&daemon, &session_id);
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start incremental attach");
    let generation = daemon
        .list_terminal_subscriptions()
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

    let first_wake_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < first_wake_deadline,
            "incremental attach did not wake"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            continue;
        }
        let _ = daemon
            .pump_woken(&batch, 13)
            .expect("pace unfinished attach through a wake");
        break;
    }
    assert!(
        !adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| {
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .ok()
                    .is_some_and(|value| {
                        value.get("type").and_then(serde_json::Value::as_str)
                            == Some("attach_state")
                            && value.get("state").and_then(serde_json::Value::as_str)
                                == Some("attached")
                    })
            }),
        "bind must happen before incremental attach finishes"
    );

    fs::write(&release_path, b"go").expect("release live exit");

    let mut saw_live = false;
    for tick in 0..400 {
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            let _ = daemon
                .pump_woken(&batch, 14 + tick)
                .expect("pump incremental attach wake");
        }
        complete_one_slot_if_full(&adapter);
        if adapter_has_live(&adapter, LIVE_B64) {
            saw_live = true;
            break;
        }
        if !process_exists(pty_child_pid) {
            thread::sleep(Duration::from_millis(20));
        }
    }
    assert!(
        saw_live,
        "LIVE bytes must reach the bound adapter when ProcessExited arrives during incremental attach: payloads={:?}",
        adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .map(|bytes| adapter_payload_text(bytes))
            .collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(data_dir);
}

fn complete_one_slot_if_full(adapter: &SharedFakeTerminalAdapter) {
    if adapter.snapshot_pressure() == TerminalAdapterPressure::Full {
        adapter.complete_write();
    }
}

fn adapter_has_live(adapter: &SharedFakeTerminalAdapter, live_b64: &str) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| {
            adapter_frame_type(bytes) == "terminal_output"
                && (adapter_payload_b64(bytes) == live_b64
                    || adapter_payload_text(bytes).contains("LIVE"))
        })
}

fn adapter_has_process_exit(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| adapter_frame_type(bytes) == "process_exit")
}

fn adapter_frame_type(bytes: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("type")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn adapter_payload_b64(bytes: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("payload_base64")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn adapter_payload_text(bytes: &[u8]) -> String {
    decode_std_base64(&adapter_payload_b64(bytes))
        .map(|payload| String::from_utf8_lossy(&payload).into_owned())
        .unwrap_or_default()
}

fn decode_std_base64(input: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    }
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !cleaned.len().is_multiple_of(4) {
        return None;
    }
    let pads = cleaned
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    if pads > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks_exact(4) {
        let a = value(chunk[0])?;
        let b = value(chunk[1])?;
        let c = value(chunk[2])?;
        let d = value(chunk[3])?;
        out.push((a << 2) | (b >> 4));
        out.push((b << 4) | (c >> 2));
        out.push((c << 6) | d);
    }
    out.truncate(out.len().saturating_sub(pads));
    Some(out)
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
        .list_terminal_subscriptions()
        .into_iter()
        .map(|row| row.subscription_id)
        .collect();
    assert!(live.contains(&active_sub));
    assert!(live.contains(&new_sub));
    assert!(
        !live.contains(&old_sub),
        "replaced pending subscription must not stay in inventory: {live:?}"
    );

    let started = Instant::now();
    let mut saw_old_after_replace = false;
    while started.elapsed() < Duration::from_secs(2) {
        let drained = daemon.drain(&session_id, 20).expect("drain");
        for (_, frame) in drained.client_egress {
            if matches!(
                frame,
                TransportEgress::Snapshot {
                    subscription_id,
                    ..
                }
                | TransportEgress::AttachState {
                    subscription_id,
                    ..
                } if subscription_id == old_sub
            ) {
                saw_old_after_replace = true;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
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
        .list_terminal_subscriptions()
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
    let live: Vec<_> = daemon.list_terminal_subscriptions();
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
    let live: Vec<_> = daemon.list_terminal_subscriptions();
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
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
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
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert!(live
        .iter()
        .any(|row| row.client_id == second && row.subscription_id == first_sub));
    assert!(live
        .iter()
        .any(|row| row.client_id == sibling && row.subscription_id == sibling_sub));
    assert!(live.iter().all(|row| row.client_id != first));
    let _ = drain_until_attached(&mut daemon, &session_id, &second);
    let _ = drain_until_attached(&mut daemon, &session_id, &sibling);
    drain_until_terminal_marker(&mut daemon, &session_id, "echo:SIBLING-KEEP", 30);
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
    let live: Vec<_> = daemon.list_terminal_subscriptions();
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
    let live: Vec<_> = daemon.list_terminal_subscriptions();
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
    for tick in 0..100 {
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
        std::thread::sleep(Duration::from_millis(1));
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
    for tick in 0..10_000 {
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
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(attached_a && attached_b);
    assert!(renderable_output_for_client(&egress, &client_b).contains("echo:CONCURRENT-POST"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_history_failure_reports_incomplete_then_attached() {
    let data_dir = temp_data_dir("worker-incremental-history-failure");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_fail_snapshot_history_after_ready(true),
    );
    let session_id = SessionId("worker-incremental-history-failure-session".to_string());
    let client_id = ClientId("worker-incremental-history-failure-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-history-failure-sub".to_string());
    let ready_path = data_dir.join("failure-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 1000 ]; do printf 'failure-history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'FAILURE-READY'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );
    daemon.spawn(request, 10).expect("spawn failure worker");
    wait_for_file(&ready_path);
    let initial = daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 12)
        .expect("start failure attach");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"FAILURE-POST\n".to_vec(),
            13,
        )
        .expect("queue post-failure input");
    let drained = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut frames = initial.client_egress;
    frames.extend(drained.client_egress);
    let states = frames
        .iter()
        .filter_map(|(target, frame)| match frame {
            TransportEgress::AttachState { state, .. } if target == &client_id => Some(state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        [
            &TerminalAttachState::Attaching,
            &TerminalAttachState::SnapshotHistoryIncomplete,
            &TerminalAttachState::Attached,
        ]
    );
    assert_eq!(
        frames
            .iter()
            .filter(|(target, frame)| {
                target == &client_id && matches!(frame, TransportEgress::Snapshot { .. })
            })
            .count(),
        1,
        "history failure must occur after READY and before any PAGE delivery"
    );
    let live = drain_until_for_client(&mut daemon, &session_id, &client_id, "echo:FAILURE-POST");
    assert!(
        renderable_output_for_client(&live.client_egress, &client_id).contains("echo:FAILURE-POST")
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_cancel_releases_snapshot_barrier() {
    let data_dir = temp_data_dir("worker-incremental-cancel");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1)),
    );
    let session_id = SessionId("worker-incremental-cancel-session".to_string());
    let client_id = ClientId("worker-incremental-cancel-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-cancel-sub".to_string());
    let ready_path = data_dir.join("cancel-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'cancel-history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'CANCEL-READY'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );
    daemon.spawn(request, 10).expect("spawn cancel worker");
    wait_for_file(&ready_path);
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start cancel attach");
    let ready = drain_until_snapshot(&mut daemon, &session_id, &client_id);
    assert_eq!(
        ready
            .client_egress
            .iter()
            .filter(|(_, frame)| matches!(frame, TransportEgress::Snapshot { .. }))
            .count(),
        1
    );
    daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 13)
        .expect("queue resize before cancel");
    daemon
        .detach(client_id, session_id.clone(), subscription_id, 14)
        .expect("cancel attach");

    let started = Instant::now();
    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("cancel-release-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("capture after cancel");
    assert!(snapshot.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_eq!(snapshot.payload.size, TerminalScreenSize::new(24, 80));
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load registry after cancelled resize")
        .expect("worker registry record");
    assert_eq!((record.rows, record.cols), (24, 80));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancel must release the worker barrier"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_default_ghostty_path_replays_configured_scrollback_window() {
    let data_dir = temp_data_dir("dwgs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgs-session".to_string());
    let primary_client = ClientId("dwgs-primary-client".to_string());
    let late_client = ClientId("dwgs-late-client".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-primary-subscription".to_string()),
            11,
        )
        .expect("primary attach should succeed");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let mut scrollback_input = Vec::new();
    for line in 0..80 {
        scrollback_input.extend_from_slice(format!("scrollback-line-{line:04}\n").as_bytes());
    }
    daemon
        .input(
            primary_client.clone(),
            session_id.clone(),
            scrollback_input,
            12,
        )
        .expect("scrollback generator should write");
    let _ = drain_until(&mut daemon, &session_id, "echo:scrollback-line-0079");

    let visible = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("visible-scrollback-check".to_string()),
            session_id: session_id.clone(),
            now_seconds: 100,
        })
        .expect("daemon read_screen should succeed");
    assert!(
        visible.screen.text.contains("echo:scrollback-line-0000"),
        "default Ghostty read_screen should retain scrollback history beyond visible rows: {:?}",
        visible.screen.text
    );

    let late_subscription = SubscriptionId("dwgs-late-subscription".to_string());
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
    assert_ghostty_snapshot_replays_marker(&snapshot, "echo:scrollback-line-0000");

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
    let _ = drain_until(&mut daemon, &session_id, "ready");

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
        std::thread::sleep(Duration::from_millis(1));
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
fn worker_backed_late_attach_and_read_screen_pending_drains_merge_in_order() {
    let data_dir = temp_data_dir("dwrsl");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwrsl-session".to_string());
    let primary_client = ClientId("dwrsl-primary".to_string());
    let primary_subscription = SubscriptionId("dwrsl-primary-sub".to_string());
    let late_client = ClientId("dwrsl-late".to_string());
    let late_subscription = SubscriptionId("dwrsl-late-sub".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            primary_subscription,
            11,
        )
        .expect("primary attach should subscribe");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            primary_client,
            session_id.clone(),
            b"worker-before-late\n".to_vec(),
            12,
        )
        .expect("history marker should write");
    let _ = drain_until(&mut daemon, &session_id, "echo:worker-before-late");

    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription.clone(),
            13,
        )
        .expect("late attach should return initial history");
    daemon
        .input(
            late_client.clone(),
            session_id.clone(),
            b"worker-after-read\n".to_vec(),
            14,
        )
        .expect("post-read marker should write");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:worker-after-read", 15);

    let late_drain = drain_until_for_client(
        &mut daemon,
        &session_id,
        &late_client,
        "echo:worker-after-read",
    );
    let mut combined_egress = late_attach.client_egress;
    combined_egress.extend(late_drain.client_egress);
    let attaching_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should emit Attaching");
    let snapshot_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::Snapshot {
                        subscription_id,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should deliver initial history");
    let attached_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attached,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should emit Attached after history");
    let live_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        subscription_id,
                        data,
                        ..
                    } if subscription_id == &late_subscription
                        && String::from_utf8_lossy(data).contains("echo:worker-after-read")
                )
        })
        .expect("late worker-backed client should receive later live output");
    assert!(
        attaching_index < snapshot_index
            && snapshot_index < attached_index
            && attached_index < live_index,
        "worker-backed readiness must order Attaching before history before Attached before live output: {:?}",
        combined_egress
    );
    {
        let (snapshot_index, snapshot) = first_snapshot_for_client(&combined_egress, &late_client)
            .expect("late Ghostty attach should return an opaque snapshot replay");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:worker-before-late");
        let live_index = first_terminal_output_index_for_client_containing(
            &combined_egress,
            &late_client,
            "echo:worker-after-read",
        )
        .expect("read_screen internal drain should remain pending");
        assert!(
            snapshot_index < live_index,
            "attach snapshot replay should merge before read_screen pending drain: {:?}",
            combined_egress
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_attach_during_alt_screen_resize_gets_complete_current_state() {
    for iteration in 0..5 {
        let data_dir = temp_data_dir(&format!("atomic-resize-attach-{iteration}"));
        let midpoint = data_dir.join("redraw-midpoint");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir)
                .with_worker_path(worker_path())
                .with_pty_reader_chunk_capacity(Some(1))
                .with_test_hold_after_read_ms(Some(2))
                .with_test_worker_egress_capacity(Some(4)),
        );
        let session_id = SessionId(format!("atomic-resize-attach-{iteration}"));
        let primary_client = ClientId(format!("atomic-primary-{iteration}"));
        let late_client = ClientId(format!("atomic-late-{iteration}"));
        let mut request = spawn_request(&session_id);
        request.request.arguments[1] = format!(
            concat!(
                "trap 'printf \"\\033[?1049h\\033[2J\"; ",
                "i=0; while [ $i -lt 100 ]; do ",
                "printf \"\\033]0;filler-%03d\\007\" \"$i\"; i=$((i+1)); done; ",
                "i=1; while [ $i -le 49 ]; do ",
                "printf \"\\033[%d;1HROW-%02d-COMPLETE\" \"$i\" \"$i\"; ",
                "sleep 0.003; i=$((i+1)); done; ",
                "printf done > \"{}\"; sleep 1; ",
                "printf \"\\033[50;1HROW-50-COMPLETE\"' WINCH; ",
                "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            ),
            midpoint.display()
        );

        daemon.spawn(request, 10).expect("spawn real worker");
        daemon
            .attach(
                primary_client.clone(),
                session_id.clone(),
                SubscriptionId(format!("atomic-primary-sub-{iteration}")),
                11,
            )
            .expect("primary attach");
        let _ = drain_until(&mut daemon, &session_id, "ready");
        daemon
            .resize(primary_client.clone(), session_id.clone(), 50, 152, 12)
            .expect("production resize");

        let deadline = Instant::now() + Duration::from_secs(10);
        while !midpoint.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            midpoint.exists(),
            "child must reach the controlled redraw midpoint"
        );

        let late_attach = daemon
            .attach(
                late_client.clone(),
                session_id.clone(),
                SubscriptionId(format!("atomic-late-sub-{iteration}")),
                13,
            )
            .expect("attach during alternate-screen redraw");

        let drained =
            drain_until_for_client(&mut daemon, &session_id, &late_client, "ROW-50-COMPLETE");
        assert!(
            drained.client_egress.iter().any(|(client_id, frame)| {
                client_id == &primary_client
                    && matches!(frame, TransportEgress::TerminalOutput { .. })
            }),
            "attach must retain pre-boundary live output for the existing client"
        );
        let mut combined_egress = late_attach.client_egress;
        combined_egress.extend(drained.client_egress);
        let (snapshot_index, snapshot) = first_snapshot_for_client_at_size(
            &combined_egress,
            &late_client,
            TerminalScreenSize::new(50, 152),
        )
        .expect("late attach must receive GHOSTSNP");
        assert_ghostty_snapshot_authority(&snapshot);
        let snapshot_text = ghostty_snapshot_plain_text(&snapshot);
        assert!(
            snapshot_text.contains("ROW-49-COMPLETE"),
            "snapshot must contain the pre-boundary redraw prefix"
        );
        assert!(
            !snapshot_text.contains("ROW-50-COMPLETE"),
            "row 50 must remain a post-boundary live-output suffix"
        );
        let mut client = GhosttyTerminal::with_config(
            snapshot.size,
            GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
        )
        .expect("client Ghostty");
        client.import_snapshot(&snapshot).expect("install GHOSTSNP");
        for (index, (client_id, frame)) in combined_egress.iter().enumerate() {
            if index <= snapshot_index {
                continue;
            }
            if client_id == &late_client {
                if let TransportEgress::TerminalOutput { data, .. } = frame {
                    client.write_output_bytes(data);
                }
            }
        }
        let text = client.plain_text().expect("render client state");
        for row in 1..=50 {
            assert!(
                text.contains(&format!("ROW-{row:02}-COMPLETE")),
                "iteration {iteration} missed row {row}; text={text:?}"
            );
        }
        assert_eq!(client.size(), TerminalScreenSize::new(50, 152));
        assert!(client.read_mode_flags().expect("mode flags").alt_screen);

        let screen = daemon
            .read_screen(ReadScreenRequest {
                request_id: RequestId(format!("atomic-screen-{iteration}")),
                session_id: session_id.clone(),
                now_seconds: 14,
            })
            .expect("parent screen after attach boundary");
        assert!(screen.screen.text.contains("ROW-50-COMPLETE"));
        daemon.shutdown(Some(session_id), 15).ok();
        let _ = fs::remove_dir_all(data_dir);
    }
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
    let _ = drain_until(&mut daemon, &session_id, "ready");
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

#[cfg(unix)]
#[test]
fn worker_backed_rapid_switch_attach_always_builds_complete_current_screen() {
    let data_dir = temp_data_dir("worker-rapid-switch-attach");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-rapid-switch-session".to_string());
    let client_id = ClientId("worker-rapid-switch-client".to_string());
    let prefix_ack = data_dir.join("prefix-ack");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "stty -echo; printf '\\033[?1049h\\033[2J'; ",
            "i=1; while [ $i -le 23 ]; do ",
            "printf '\\033[%d;1HSWITCH-ROW-%02d-COMPLETE' \"$i\" \"$i\"; ",
            "i=$((i+1)); done; ",
            "printf '\\033[24;1HREADY'; ",
            "while IFS= read -r line; do ",
            "printf '\\033[24;1H%-40s' \"$line\"; ",
            "printf '%s' \"$line\" > \"{}\"; done"
        ),
        prefix_ack.display()
    );

    daemon.spawn(request, 10).expect("spawn real worker");
    let initial_subscription = SubscriptionId("rapid-switch-a".to_string());
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            initial_subscription,
            11,
        )
        .expect("initial attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);

    for iteration in 0..20 {
        let subscription_id = SubscriptionId(format!("rapid-switch-{}", (iteration / 2) % 2));
        let prefix_marker = format!("PREFIX-SWITCH-{iteration:02}");
        daemon
            .input(
                client_id.clone(),
                session_id.clone(),
                format!("{prefix_marker}\n").into_bytes(),
                19 + iteration,
            )
            .expect("pre-attach marker");
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(&prefix_ack).ok().as_deref() != Some(prefix_marker.as_str())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            fs::read_to_string(&prefix_ack).ok().as_deref(),
            Some(prefix_marker.as_str()),
            "child must publish the pre-attach marker before attach"
        );
        if iteration % 3 == 0 {
            let transient = SubscriptionId(format!("rapid-switch-transient-{iteration}"));
            daemon
                .attach(
                    client_id.clone(),
                    session_id.clone(),
                    transient.clone(),
                    20 + iteration,
                )
                .expect("transient attach");
            daemon
                .detach(
                    client_id.clone(),
                    session_id.clone(),
                    transient,
                    21 + iteration,
                )
                .expect("transient detach before drain");
        }
        let attached = daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                22 + iteration,
            )
            .expect("rapid attach");

        let live_marker = format!("LIVE-SWITCH-{iteration:02}");
        daemon
            .input(
                client_id.clone(),
                session_id.clone(),
                format!("{live_marker}\n").into_bytes(),
                23 + iteration,
            )
            .expect("post-attach live marker");
        let drained = drain_until_for_client(&mut daemon, &session_id, &client_id, &live_marker);
        let mut combined_egress = attached.client_egress;
        combined_egress.extend(drained.client_egress);
        assert!(combined_egress.iter().all(|(received_client, frame)| {
            received_client != &client_id
                || match frame {
                    TransportEgress::TerminalOutput {
                        subscription_id: received,
                        ..
                    }
                    | TransportEgress::Snapshot {
                        subscription_id: received,
                        ..
                    }
                    | TransportEgress::AttachState {
                        subscription_id: received,
                        ..
                    } => received == &subscription_id,
                    _ => true,
                }
        }));

        let (snapshot_index, snapshot) = first_snapshot_for_client_at_size(
            &combined_egress,
            &client_id,
            TerminalScreenSize::new(24, 80),
        )
        .expect("each attach must deliver a current GHOSTSNP");
        assert!(ghostty_snapshot_plain_text(&snapshot).contains(&prefix_marker));
        assert!(combined_egress[..snapshot_index]
            .iter()
            .all(|(received_client, frame)| received_client != &client_id
                || !matches!(frame, TransportEgress::TerminalOutput { .. })));
        assert_ghostty_snapshot_authority(&snapshot);
        let mut client = GhosttyTerminal::with_config(
            snapshot.size,
            GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
        )
        .expect("fresh client Ghostty");
        client.import_snapshot(&snapshot).expect("install GHOSTSNP");
        for (index, (received_client, frame)) in combined_egress.iter().enumerate() {
            if index > snapshot_index && received_client == &client_id {
                if let TransportEgress::TerminalOutput { data, .. } = frame {
                    client.write_output_bytes(data);
                }
            }
        }
        let text = client.plain_text().expect("render client state");
        for row in 1..=23 {
            assert!(
                text.contains(&format!("SWITCH-ROW-{row:02}-COMPLETE")),
                "iteration {iteration} missed row {row}; text={text:?}"
            );
        }
        assert!(text.contains(&live_marker));
        assert!(client.read_mode_flags().expect("mode flags").alt_screen);
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_screen_and_snapshot_retain_shutdown_truth_and_keep_negative_paths() {
    let data_dir = temp_data_dir("dssn");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let missing_session = SessionId("missing-rb-session".to_string());

    assert!(matches!(
        daemon.read_screen(ReadScreenRequest {
            request_id: RequestId("missing-screen".to_string()),
            session_id: missing_session.clone(),
            now_seconds: 10,
        }),
        Err(CoreDaemonError::UnknownSession(session)) if session == missing_session
    ));
    assert!(matches!(
        daemon.capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("missing-snapshot".to_string()),
            session_id: missing_session.clone(),
            now_seconds: 10,
        }),
        Err(CoreDaemonError::UnknownSession(session)) if session == missing_session
    ));

    let session_id = SessionId("dssn-empty-session".to_string());
    let client_id = ClientId("dssn-client".to_string());
    daemon
        .spawn(mode_flags_spawn_request(&session_id), 11)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("dssn-subscription".to_string()),
            12,
        )
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"shutdown-final\n".to_vec(),
            13,
        )
        .expect("shutdown marker input should write");
    let screen = read_screen_until(&mut daemon, &session_id, "echo:shutdown-final", 14);
    assert_eq!(screen.screen.session_id, session_id);
    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("empty-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("spawned never explicitly drained session should still capture snapshot");
    assert_eq!(snapshot.snapshot.session_id, session_id);
    assert_snapshot_format(&snapshot.payload);
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"enable-mouse\n".to_vec(),
            16,
        )
        .expect("mouse mode DECSET should write");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-mouse", 17);
    let live_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("live-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 18,
        })
        .expect("live mode flags should be authoritative");
    assert_eq!(live_mode_flags.mode_flags.mode_flags.mouse_mode, 9);

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should succeed");
    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("shutdown screen should serve retained terminal truth");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-snapshot-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("shutdown snapshot should serve retained terminal truth");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 23,
        })
        .expect("shutdown screen should be repeatable");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-snapshot-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 24,
        })
        .expect("shutdown snapshot should be repeatable");
    let retained_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("retained-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 25,
        })
        .expect("shutdown mode flags should serve retained terminal truth");

    assert!(first_screen.screen.text.contains("echo:shutdown-final"));
    assert_eq!(retained_mode_flags.mode_flags.mode_flags.mouse_mode, 9);
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_ne!(
        first_screen.screen.request_id,
        second_screen.screen.request_id
    );
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_ne!(
        first_snapshot.snapshot.request_id,
        second_snapshot.snapshot.request_id
    );

    assert!(daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"late-input\n".to_vec(),
            25,
        )
        .is_err());
    assert!(daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 26)
        .is_err());
    assert!(daemon
        .attach(
            client_id,
            session_id.clone(),
            SubscriptionId("dssn-late-subscription".to_string()),
            27,
        )
        .is_err());
    let unchanged = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-unchanged".to_string()),
            session_id: session_id.clone(),
            now_seconds: 28,
        })
        .expect("failed mutation attempts must not change retained truth");
    assert_eq!(unchanged.screen.text, first_screen.screen.text);

    daemon
        .mark_stale(&session_id, 29)
        .expect("test should mark retained registry record stale");
    assert!(matches!(
        daemon.read_screen(ReadScreenRequest {
            request_id: RequestId("stale-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 30,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
    ));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn natural_exit_read_screen_and_capture_snapshot_freeze_repeatable_truth() {
    let data_dir = temp_data_dir("dnex");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    let screen_session = SessionId("dnex-screen-session".to_string());
    daemon
        .spawn(self_exit_spawn_request(&screen_session), 10)
        .expect("screen self-exit session should spawn");
    daemon
        .attach(
            ClientId("dnex-screen-client".to_string()),
            screen_session.clone(),
            SubscriptionId("dnex-screen-subscription".to_string()),
            11,
        )
        .expect("screen self-exit session should attach");
    let _ = drain_until(&mut daemon, &screen_session, "ready");
    daemon
        .input(
            ClientId("dnex-screen-client".to_string()),
            screen_session.clone(),
            b"screen-exit\n".to_vec(),
            12,
        )
        .expect("screen self-exit input should write");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-screen-1".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 13,
        })
        .expect("first natural-exit read_screen should freeze and serve terminal truth");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-screen-snapshot-1".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 14,
        })
        .expect("natural-exit snapshot should reuse retained truth");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-screen-2".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 15,
        })
        .expect("natural-exit screen should be repeatable");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-screen-snapshot-2".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 16,
        })
        .expect("natural-exit snapshot should be repeatable");
    assert!(first_screen.screen.text.contains("echo:screen-exit"));
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_ne!(
        first_screen.screen.request_id,
        second_screen.screen.request_id
    );
    assert_ne!(
        first_snapshot.snapshot.request_id,
        second_snapshot.snapshot.request_id
    );
    let screen_drain = daemon
        .drain(&screen_session, 17)
        .expect("drain after retained read_screen should return final output");
    assert_retained_exit_output(&screen_drain, &screen_session, "echo:screen-exit");
    let second_screen_drain = daemon
        .drain(&screen_session, 18)
        .expect("second drain after retained readback should succeed");
    assert_no_duplicate_exit_output(&second_screen_drain, "echo:screen-exit");

    let snapshot_session = SessionId("dnex-snapshot-session".to_string());
    daemon
        .spawn(self_exit_spawn_request(&snapshot_session), 20)
        .expect("snapshot self-exit session should spawn");
    daemon
        .attach(
            ClientId("dnex-snapshot-client".to_string()),
            snapshot_session.clone(),
            SubscriptionId("dnex-snapshot-subscription".to_string()),
            21,
        )
        .expect("snapshot self-exit session should attach");
    let _ = drain_until(&mut daemon, &snapshot_session, "ready");
    daemon
        .input(
            ClientId("dnex-snapshot-client".to_string()),
            snapshot_session.clone(),
            b"snapshot-exit\n".to_vec(),
            22,
        )
        .expect("snapshot self-exit input should write");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let snapshot_result = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-1".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 23,
        })
        .expect("first natural-exit capture_snapshot should freeze and serve terminal truth");
    let snapshot_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-snapshot-screen".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 24,
        })
        .expect("screen should share snapshot-triggered retained truth");
    let repeated_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-2".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 25,
        })
        .expect("snapshot-triggered retained truth should be repeatable");
    assert!(snapshot_screen.screen.text.contains("echo:snapshot-exit"));
    assert_eq!(snapshot_result.payload, repeated_snapshot.payload);
    assert_ne!(
        snapshot_result.snapshot.request_id,
        repeated_snapshot.snapshot.request_id
    );
    let snapshot_drain = daemon
        .drain(&snapshot_session, 26)
        .expect("drain after retained capture_snapshot should return final output");
    assert_retained_exit_output(&snapshot_drain, &snapshot_session, "echo:snapshot-exit");
    let second_snapshot_drain = daemon
        .drain(&snapshot_session, 27)
        .expect("second drain after retained snapshot should succeed");
    assert_no_duplicate_exit_output(&second_snapshot_drain, "echo:snapshot-exit");

    let registry_json =
        fs::read_to_string(data_dir.join("sessions").join("dnex-snapshot-session.json"))
            .expect("registry JSON should remain readable");
    assert!(!registry_json.contains("echo:snapshot-exit"));
    drop(daemon);
    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    assert!(matches!(
        restarted.read_screen(ReadScreenRequest {
            request_id: RequestId("restart-screen".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 28,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == snapshot_session
    ));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_reconciles_a_worker_natural_exit_after_output_is_observable() {
    let data_dir = short_temp_data_dir("shutdown-race");
    let session_id = SessionId("shutdown-race-session".to_string());
    let client_id = ClientId("shutdown-race-client".to_string());
    let subscription_id = SubscriptionId("shutdown-race-subscription".to_string());
    let marker_path = data_dir.join("marker");
    let release_path = data_dir.join("release");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(
            controlled_exit_spawn_request(&session_id, &marker_path, &release_path),
            10,
        )
        .expect("controlled natural-exit session should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("controlled natural-exit session should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should emit the marker");
    wait_for_condition("fixture marker file", || marker_path.exists());
    let marker_screen = read_screen_until(
        &mut daemon,
        &session_id,
        "botster-core-natural-exit-marker",
        13,
    );
    assert!(marker_screen
        .screen
        .text
        .contains("botster-core-natural-exit-marker"));
    let observed_output = daemon
        .drain(&session_id, 14)
        .expect("observable marker output should remain drainable");
    assert!(terminal_output(&observed_output.client_egress)
        .contains("botster-core-natural-exit-marker"));
    assert_eq!(
        daemon.list().expect("list pre-exit session")[0].registry_state,
        RegistrySessionState::Running
    );

    let (_, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    fs::write(&release_path, b"release").expect("release controlled natural exit");
    wait_for_condition("worker process and control route completion", || {
        !process_exists(pty_child_pid) && UnixStream::connect(&socket_path).is_err()
    });
    assert_eq!(
        daemon.list().expect("list unreconciled session")[0].registry_state,
        RegistrySessionState::Running
    );

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should reconcile the completed worker");
    daemon
        .shutdown(Some(session_id.clone()), 21)
        .expect("repeated terminal shutdown should remain idempotent");

    let recovered = daemon
        .drain(&session_id, 22)
        .expect("attached client should drain shutdown recovery egress");
    assert!(
        recovered
            .client_egress
            .iter()
            .all(|(frame_client_id, _)| frame_client_id == &client_id),
        "shutdown recovery egress must remain targeted to the attached client: {:?}",
        recovered.client_egress
    );
    let tail_index = recovered
        .client_egress
        .iter()
        .position(|(frame_client_id, frame)| {
            frame_client_id == &client_id
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        session_id: frame_session_id,
                        subscription_id: frame_subscription_id,
                        data,
                    } if frame_session_id == &session_id
                        && frame_subscription_id == &subscription_id
                        && String::from_utf8_lossy(data)
                            .contains("botster-core-natural-exit-tail")
                )
        })
        .expect("shutdown recovery should preserve the final terminal tail");
    let process_exits = recovered
        .client_egress
        .iter()
        .enumerate()
        .filter_map(|(index, (frame_client_id, frame))| match frame {
            TransportEgress::ProcessExit {
                session_id: frame_session_id,
                subscription_id: frame_subscription_id,
                code,
            } if frame_client_id == &client_id
                && frame_session_id == &session_id
                && frame_subscription_id == &subscription_id =>
            {
                Some((index, *code))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        process_exits.len(),
        1,
        "shutdown recovery should deliver exactly one ProcessExit: {:?}",
        recovered.client_egress
    );
    assert_eq!(
        process_exits[0].1,
        Some(0),
        "shutdown recovery should deliver the successful exit code"
    );
    assert!(
        tail_index < process_exits[0].0,
        "final terminal output must precede ProcessExit: {:?}",
        recovered.client_egress
    );

    let reconciled = daemon.list().expect("list reconciled session");
    assert_eq!(reconciled[0].registry_state, RegistrySessionState::Exited);
    assert!(matches!(
        daemon
            .lifecycle_baseline()
            .expect("lifecycle baseline")
            .sessions[0]
            .lifecycle,
        Some(SessionLifecycleState::Exited { code: Some(0) })
    ));

    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-race-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 23,
        })
        .expect("retained terminal screen");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-race-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 24,
        })
        .expect("repeat retained terminal screen");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-race-snapshot-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 25,
        })
        .expect("retained terminal snapshot");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-race-snapshot-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 26,
        })
        .expect("repeat retained terminal snapshot");

    assert!(first_screen
        .screen
        .text
        .contains("botster-core-natural-exit-tail"));
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_snapshot_format(&first_snapshot.payload);
    assert_snapshot_payload_observed_marker_when_plain(
        &first_snapshot.payload,
        "botster-core-natural-exit-tail",
    );
    assert_ghostty_snapshot_replays_marker(
        &first_snapshot.payload,
        "botster-core-natural-exit-tail",
    );
    let repeated_drain = daemon
        .drain(&session_id, 27)
        .expect("shutdown recovery egress should be delivered exactly once");
    assert!(
        repeated_drain.client_egress.iter().all(|(_, frame)| {
            !matches!(
                frame,
                TransportEgress::TerminalOutput { data, .. }
                    if String::from_utf8_lossy(data)
                        .contains("botster-core-natural-exit-tail")
            ) && !matches!(frame, TransportEgress::ProcessExit { .. })
        }),
        "second drain must not duplicate the recovered tail or ProcessExit: {:?}",
        repeated_drain.client_egress
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_all_continues_after_a_raced_natural_exit_and_cleans_a_live_session() {
    let data_dir = short_temp_data_dir("shutdown-all-race");
    let raced_session = SessionId("shutdown-all-raced".to_string());
    let live_session = SessionId("shutdown-all-live".to_string());
    let marker_path = data_dir.join("marker");
    let release_path = data_dir.join("release");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(
            controlled_exit_spawn_request(&raced_session, &marker_path, &release_path),
            10,
        )
        .expect("controlled natural-exit session should spawn");
    daemon
        .attach(
            ClientId("shutdown-all-raced-client".to_string()),
            raced_session.clone(),
            SubscriptionId("shutdown-all-raced-subscription".to_string()),
            11,
        )
        .expect("controlled natural-exit session should attach");
    let _ = drain_until(&mut daemon, &raced_session, "ready");
    daemon
        .input(
            ClientId("shutdown-all-raced-client".to_string()),
            raced_session.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should emit the marker");
    wait_for_condition("shutdown-all fixture marker", || marker_path.exists());
    let _ = read_screen_until(
        &mut daemon,
        &raced_session,
        "botster-core-natural-exit-marker",
        13,
    );
    let (_, raced_child_pid, raced_socket_path) = worker_process_evidence(&daemon, &raced_session);
    fs::write(&release_path, b"release").expect("release controlled natural exit");
    wait_for_condition("shutdown-all raced worker completion", || {
        !process_exists(raced_child_pid) && UnixStream::connect(&raced_socket_path).is_err()
    });

    daemon
        .spawn(spawn_request(&live_session), 20)
        .expect("live session should spawn");
    let (_, live_child_pid, live_socket_path) = worker_process_evidence(&daemon, &live_session);

    daemon
        .shutdown(None, 30)
        .expect("raced and live sessions should both shut down");
    let listed = daemon.list().expect("list shutdown-all sessions");
    assert_eq!(listed.len(), 2);
    assert!(listed
        .iter()
        .all(|record| record.registry_state == RegistrySessionState::Exited));
    assert!(!process_exists(live_child_pid));
    assert!(!live_socket_path.exists());

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

#[cfg(unix)]
#[test]
fn production_worker_root_handles_canonical_and_long_session_ids() {
    let data_dir = temp_data_dir("production-worker-id-length");
    let canonical = SessionId("123e4567-e89b-12d3-a456-426614174000".to_string());
    let long = SessionId(format!("sess-long-{}", "x".repeat(180)));
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
    wait_for_condition("production worker and PTY cleanup", || {
        !process_exists(canonical_worker)
            && !process_exists(long_worker)
            && !process_exists(canonical_pty)
            && !process_exists(long_pty)
            && !canonical_socket.exists()
            && !long_socket.exists()
    });
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
    wait_for_condition("bounded reap after owner shutdown", || {
        !process_exists(worker_pid) && !process_exists(pty_pid)
    });
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
    let exited = (0..100).find_map(|tick| {
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
            std::thread::sleep(Duration::from_millis(10));
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
            client_id,
            session_id.clone(),
            SubscriptionId("lifecycle-source-reused-subscription".to_string()),
            211,
        )
        .expect("removed subscription state must not block a fresh attach");
    let fresh = drain_until(&mut daemon, &session_id, "ready");
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
    assert!(daemon.take_journal_advanced_wake());
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
    let _ = daemon.take_journal_advanced_wake();
    let exited = observe_until_exited(&mut daemon, &session_id, &baseline, 20);
    let _discarded = daemon.take_journal_advanced_wake();
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
    assert!(!daemon.take_journal_advanced_wake());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("first append sets the wake");
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
    assert!(daemon.take_journal_advanced_wake());
    assert!(!daemon.take_journal_advanced_wake());
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

#[cfg(unix)]
#[test]
fn observe_retains_one_session_error_and_still_exits_a_later_sibling() {
    let data_dir = temp_data_dir("lifecycle-observe-sibling");
    let earlier = SessionId("a-observe-fail".to_string());
    let later = SessionId("b-observe-exit".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_fail_runtime_drain_for(Some(earlier.clone())),
    );
    daemon
        .spawn(spawn_request(&earlier), 10)
        .expect("earlier sibling stays live");
    daemon
        .spawn(immediate_exit_spawn_request(&later), 11)
        .expect("later sibling should spawn");
    let after_spawn = daemon
        .lifecycle_baseline()
        .expect("sibling spawn watermark")
        .cursor;
    // `exit 0` becomes a zombie until the observe drain reaps it. Do not wait
    // on kill -0; that stays true until this tick.
    std::thread::sleep(Duration::from_millis(50));

    let observed = daemon
        .observe_lifecycle(20)
        .expect("observe must finish the full pass");
    assert_eq!(observed.session_errors.len(), 1);
    assert_eq!(observed.session_errors[0].session_id, earlier);
    assert!(matches!(
        &observed.session_errors[0].error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(error))
            if error.kind == SessionRuntimeErrorKind::OutputFailed
    ));
    let page = daemon
        .lifecycle_changes_page(&after_spawn, 8, 16 * 1024)
        .expect("sibling page");
    assert!(
        page_contains_exited(&page, &later),
        "later sibling must publish Exited on the same observe tick: {page:?}"
    );

    daemon.shutdown(Some(earlier), 30).ok();
    daemon.shutdown(Some(later), 31).expect("later shutdown");
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
    assert!(!first_slice.complete);
    let second_slice = daemon
        .observe_lifecycle_slice(
            12,
            Some(&observe_resume(&first_slice)),
            observe_item_budget(1),
        )
        .expect("resume slice");
    assert_eq!(second_slice.last_visited.as_ref(), Some(&second));
    assert_eq!(second_slice.pass_id, first_slice.pass_id);
    assert!(!second_slice.complete);
    let third_slice = daemon
        .observe_lifecycle_slice(
            13,
            Some(&observe_resume(&second_slice)),
            observe_item_budget(1),
        )
        .expect("final slice");
    assert_eq!(third_slice.last_visited.as_ref(), Some(&third));
    assert!(third_slice.complete);

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
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
    assert!(!one.complete);

    let timed_out = daemon
        .observe_lifecycle_slice(
            14,
            Some(&observe_resume(&one)),
            ObserveLifecycleBudget {
                max_sessions: 8,
                max_encoded_result_bytes: 16 * 1024,
                max_elapsed: Duration::ZERO,
            },
        )
        .expect("zero elapsed visits none remaining");
    assert_eq!(timed_out.last_visited.as_ref(), Some(&first));
    assert!(!timed_out.complete);

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
    assert!(empty.complete);
    assert_eq!(
        serde_json::to_vec(&empty).expect("encode").len(),
        empty_minimum
    );

    let data_dir = temp_data_dir("lifecycle-observe-no-visit-result-budget");
    let first = SessionId("a-no-visit-budget".to_string());
    let second = SessionId("b-no-visit-budget".to_string());
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon
        .spawn(spawn_request(&first), 10)
        .expect("first spawn");
    daemon
        .spawn(spawn_request(&second), 11)
        .expect("second spawn");

    let zero_items_minimum = match daemon.observe_lifecycle_slice(
        12,
        None,
        ObserveLifecycleBudget {
            max_sessions: 0,
            max_encoded_result_bytes: 0,
            max_elapsed: Duration::MAX,
        },
    ) {
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
        other => panic!("zero-item slice must enforce its byte budget: {other:?}"),
    };
    let zero_items = daemon
        .observe_lifecycle_slice(
            13,
            None,
            ObserveLifecycleBudget {
                max_sessions: 0,
                max_encoded_result_bytes: zero_items_minimum,
                max_elapsed: Duration::MAX,
            },
        )
        .expect("exact zero-item budget");
    assert!(!zero_items.complete);
    assert!(zero_items.last_visited.is_none());
    assert_eq!(
        serde_json::to_vec(&zero_items).expect("encode").len(),
        zero_items_minimum
    );

    let first_elapsed_minimum = match daemon.observe_lifecycle_slice(
        14,
        None,
        ObserveLifecycleBudget {
            max_sessions: usize::MAX,
            max_encoded_result_bytes: 0,
            max_elapsed: Duration::ZERO,
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
                max_elapsed: Duration::ZERO,
            },
        )
        .expect("exact first elapsed budget");
    assert!(!first_elapsed.complete);
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
            max_elapsed: Duration::ZERO,
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
                max_elapsed: Duration::ZERO,
            },
        )
        .expect("exact resumed elapsed budget preserves the pass");
    assert_eq!(resumed.pass_id, progressed.pass_id);
    assert_eq!(resumed.last_visited, progressed.last_visited);
    assert!(!resumed.complete);
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
fn observe_slice_preserves_typed_wrapper_errors_and_blocks_over_budget_visits() {
    let messages = [
        "a".repeat(300),
        "\u{0000}".repeat(256),
        r#"quote " and backslash \"#.to_string(),
        "controls \n\t\r\u{0007}".to_string(),
        "multibyte café 日本語".to_string(),
    ];
    for (index, message) in messages.into_iter().enumerate() {
        let data_dir = temp_data_dir(&format!("lifecycle-observe-long-error-{index}"));
        let earlier = SessionId(format!("a-long-error-{index}"));
        let later = SessionId(format!("b-long-error-{index}"));
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir)
                .with_test_fail_runtime_drain_for(Some(earlier.clone()))
                .with_test_fail_runtime_drain_message(Some(message.to_string())),
        );
        daemon
            .spawn(spawn_request(&earlier), 10)
            .expect("failing session");
        daemon
            .spawn(immediate_exit_spawn_request(&later), 11)
            .expect("later sibling");

        let reserved_min = match daemon.observe_lifecycle_slice(
            19,
            None,
            ObserveLifecycleBudget {
                max_sessions: 8,
                max_encoded_result_bytes: 0,
                max_elapsed: Duration::MAX,
            },
        ) {
            Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
            other => panic!("expected reserved-error BudgetTooSmall, got {other:?}"),
        };
        let first = daemon
            .observe_lifecycle_slice(
                20,
                None,
                ObserveLifecycleBudget {
                    max_sessions: 8,
                    max_encoded_result_bytes: reserved_min,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("reserved budget admits the first visit only");
        assert_eq!(first.last_visited.as_ref(), Some(&earlier));
        assert!(!first.complete);
        assert_eq!(first.session_errors.len(), 1);
        assert_eq!(first.session_errors[0].session_id, earlier);
        assert!(first.session_errors[0]
            .message
            .bytes()
            .all(botster_core_daemon::is_observe_slice_error_message_byte));
        assert!(first.session_errors[0].message.len() <= 256);
        let encoded = serde_json::to_vec(&first).expect("encode slice");
        assert!(encoded.len() <= reserved_min);
        assert!(
            serde_json::to_vec(&first.session_errors[0])
                .expect("actual")
                .len()
                <= serde_json::to_vec(&reserved_observe_slice_error(earlier.clone()))
                    .expect("reserved")
                    .len()
        );

        let mut wrapper_daemon = CoreDaemon::new(
            CoreDaemonConfig::new(temp_data_dir(&format!(
                "lifecycle-observe-wrapper-error-{index}"
            )))
            .with_test_fail_runtime_drain_for(Some(earlier.clone()))
            .with_test_fail_runtime_drain_message(Some(message.to_string())),
        );
        wrapper_daemon
            .spawn(spawn_request(&earlier), 10)
            .expect("wrapper spawn");
        let wrapped = wrapper_daemon
            .observe_lifecycle(22)
            .expect("wrapper keeps typed errors");
        assert_eq!(wrapped.session_errors.len(), 1);
        assert!(matches!(
            &wrapped.session_errors[0].error,
            CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(error))
                if error.kind == SessionRuntimeErrorKind::OutputFailed
        ));
        assert_ne!(
            wrapped.session_errors[0].error.to_string(),
            first.session_errors[0].message,
            "wrapper must not reconstruct CoreDaemonError from the sanitized slice message"
        );

        let finished = daemon
            .observe_lifecycle_slice(23, Some(&observe_resume(&first)), observe_item_budget(8))
            .expect("later slice still visits the sibling");
        assert!(finished.complete || finished.last_visited.as_ref() == Some(&later));

        daemon.shutdown(Some(earlier.clone()), 30).ok();
        daemon.shutdown(Some(later), 31).ok();
        wrapper_daemon.shutdown(Some(earlier), 32).ok();
        let _ = fs::remove_dir_all(data_dir);
    }
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
    assert!(!dropped.complete);
    assert!(dropped.last_visited.is_none());
    assert!(matches!(
        dropped.resync_required,
        Some(SessionLifecycleResyncReason::ObservePassUnavailable)
    ));

    let restarted = daemon
        .observe_lifecycle_slice(14, None, observe_item_budget(8))
        .expect("new pass restarts");
    assert!(restarted.complete);
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
    assert!(!earlier.complete);
    assert!(earlier.last_visited.is_none());
    assert!(matches!(
        earlier.resync_required,
        Some(SessionLifecycleResyncReason::ObservePassUnavailable)
    ));

    let forged_later = ObserveLifecycleCursor {
        pass_id: second_slice.pass_id.clone(),
        last_visited: Some(third.clone()),
    };
    let later = daemon
        .observe_lifecycle_slice(14, Some(&forged_later), observe_item_budget(8))
        .expect("forged later");
    assert!(!later.complete);
    assert!(later.last_visited.is_none());
    assert!(matches!(
        later.resync_required,
        Some(SessionLifecycleResyncReason::ObservePassUnavailable)
    ));

    let resumed = daemon
        .observe_lifecycle_slice(
            15,
            Some(&observe_resume(&second_slice)),
            observe_item_budget(8),
        )
        .expect("honest resume still works");
    assert!(resumed.complete);
    assert_eq!(resumed.last_visited.as_ref(), Some(&third));

    for session_id in [first, second, third] {
        daemon.shutdown(Some(session_id), 20).ok();
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_slice_resume_does_not_rescan_or_absorb_mid_pass_births() {
    let data_dir = temp_data_dir("lifecycle-observe-frozen-remaining");
    let born = SessionId("s05a-mid-pass-birth".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_fail_runtime_drain_for(Some(born.clone())),
    );
    let mut ids = Vec::new();
    for index in 0..12 {
        let session_id = SessionId(format!("s{index:02}-frozen-remaining"));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("bulk spawn");
        ids.push(session_id);
    }
    let first = daemon
        .observe_lifecycle_slice(11, None, observe_item_budget(1))
        .expect("mint snapshot");
    daemon
        .spawn(spawn_request(&born), 12)
        .expect("mid-pass birth");
    let mut resume = observe_resume(&first);
    for tick in 0..11 {
        let slice = daemon
            .observe_lifecycle_slice(13 + tick, Some(&resume), observe_item_budget(1))
            .expect("resume from snapshot");
        assert_ne!(slice.last_visited.as_ref(), Some(&born));
        assert!(slice
            .session_errors
            .iter()
            .all(|error| error.session_id != born));
        if slice.complete {
            assert_eq!(slice.last_visited.as_ref(), ids.last());
            break;
        }
        resume = observe_resume(&slice);
    }
    let new_pass = daemon
        .observe_lifecycle_slice(30, None, observe_item_budget(32))
        .expect("new pass sees the birth");
    assert!(new_pass.complete);
    assert!(new_pass
        .session_errors
        .iter()
        .any(|error| error.session_id == born));

    for session_id in ids {
        daemon.shutdown(Some(session_id), 40).ok();
    }
    daemon.shutdown(Some(born), 41).ok();
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
        assert!(page.resync_required.is_none());
        rows.extend(page.sessions.iter().cloned());
        if page.complete {
            assert!(page.next.is_none());
            assert_eq!(rows, full.sessions);
            break;
        }
        assert!(!page.complete);
        snapshot = Some(page.snapshot_sequence);
        after = page.next;
    }

    daemon.shutdown(Some(first), 20).ok();
    daemon.shutdown(Some(second), 21).ok();
    daemon.shutdown(Some(third), 22).ok();
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
        assert!(page.resync_required.is_none());
        snapshot = Some(page.snapshot_sequence.clone());
        if !page.sessions.is_empty() || page.complete {
            break page;
        }
        after = page.next;
    };
    assert!(!first_page.complete);
    let snapshot = first_page.snapshot_sequence.clone();
    std::thread::sleep(Duration::from_millis(50));
    let _ = daemon.observe_lifecycle(20).expect("observe after mint");
    let second_page = daemon
        .lifecycle_baseline_page(
            Some(&snapshot),
            first_page.next.as_ref(),
            baseline_item_budget(8),
        )
        .expect("frozen second page");
    assert!(second_page.complete);
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
    assert!(!unknown.complete);
    assert!(unknown.sessions.is_empty());
    assert!(matches!(
        unknown.resync_required,
        Some(SessionLifecycleResyncReason::SnapshotUnavailable)
    ));

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
        assert!(page.resync_required.is_none());
        snapshot = Some(page.snapshot_sequence.clone());
        rows.extend(page.sessions.iter().cloned());
        if page.complete {
            return (page.snapshot_sequence, rows);
        }
        after = page.next;
    }
}

#[test]
fn lifecycle_baseline_page_setup_only_elapsed_keeps_freeze_identity() {
    let data_dir = temp_data_dir("lifecycle-baseline-setup-only");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
                max_elapsed: Duration::ZERO,
            },
        )
        .expect("setup-only mint");
    assert!(page.resync_required.is_none());
    assert!(!page.complete);
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
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
                max_elapsed: Duration::ZERO,
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
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
                max_elapsed: Duration::ZERO,
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
    fs::write(
        data_dir.join("sessions").join("b-bad-baseline.json"),
        b"not json",
    )
    .expect("malformed registry fixture");
    let (_, rows) = assemble_baseline_pages(&mut daemon, None, baseline_item_budget(8));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session.session_id, good);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn lifecycle_baseline_page_byte_budget_stops_before_remaining_rows() {
    let data_dir = temp_data_dir("lifecycle-baseline-bytes");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
                max_elapsed: Duration::ZERO,
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
    let indexed = match daemon.lifecycle_baseline_page(
        Some(&setup.snapshot_sequence),
        None,
        LifecycleBaselineBudget {
            max_rows: usize::MAX,
            max_bytes: minimum,
            max_elapsed: Duration::MAX,
        },
    ) {
        Ok(page) => {
            assert!(!page.complete);
            assert!(page.sessions.is_empty());
            page
        }
        Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => {
            assert!(minimum_bytes > minimum);
            let page = daemon
                .lifecycle_baseline_page(
                    Some(&setup.snapshot_sequence),
                    None,
                    LifecycleBaselineBudget {
                        max_rows: usize::MAX,
                        max_bytes: minimum_bytes,
                        max_elapsed: Duration::MAX,
                    },
                )
                .expect("exact continuation budget");
            assert!(!page.complete);
            assert!(page.sessions.is_empty());
            page
        }
        other => panic!("expected incomplete page or BudgetTooSmall, got {other:?}"),
    };
    let encoded = serde_json::to_vec(&indexed)
        .expect("continuation page must serialize")
        .len();
    assert!(encoded <= 64 * 1024);
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
    std::thread::sleep(Duration::from_millis(50));
    let mut resume = None;
    let mut complete = false;
    for tick in 0..50 {
        let slice = daemon
            .observe_lifecycle_slice(20 + tick, resume.as_ref(), observe_item_budget(1))
            .expect("slice");
        assert!(slice.resync_required.is_none());
        complete = slice.complete;
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
    assert!(
        page_contains_exited(&page, &session_id),
        "sliced observe must publish Exited without Drain: {page:?}"
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
    let hold_ms = 8_000;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_hold_before_exit_ms(Some(hold_ms)),
    );
    daemon
        .spawn(immediate_exit_spawn_request(&session_id), 10)
        .expect("W1 session should spawn");
    let after_spawn = daemon.lifecycle_baseline().expect("W1 baseline").cursor;
    let (worker_pid, pty_child_pid, _) = worker_process_evidence(&daemon, &session_id);
    wait_for_condition("W1 session process exit with worker still alive", || {
        !process_exists(pty_child_pid) && process_exists(worker_pid)
    });
    // Worker loop + writer need a short beat after the PTY child exits to
    // queue FRAME_PROCESS_EXITED before the hold starts.
    thread::sleep(Duration::from_millis(150));
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
    wait_for_condition("W1 bounded reaper", || !process_exists(worker_pid));

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
fn observe_session_lifecycle_injected_drain_failure_is_err() {
    let data_dir = temp_data_dir("exact-observe-fail-drain");
    let session_id = SessionId("exact-observe-fail-drain".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_fail_runtime_drain_for(Some(session_id.clone())),
    );
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("live session for injected drain failure");
    let result = daemon.observe_session_lifecycle(&session_id, 20);
    assert!(
        result.is_err(),
        "injected drain failure must be Err: {result:?}"
    );
    assert!(
        !matches!(
            result,
            Ok(SessionLifecycleLookup::Found(ref record))
                if matches!(record.lifecycle, Some(SessionLifecycleState::Running))
        ),
        "injected drain failure must not return Found Running: {result:?}"
    );
    daemon.shutdown(Some(session_id), 30).ok();
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
        .list_terminal_subscriptions()
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
    assert!(daemon.take_journal_advanced_wake(), "spawn sets the wake");
    let cursor = daemon
        .lifecycle_baseline()
        .expect("watermark after spawn")
        .cursor;
    wait_for_condition("OS-level finite-producer exit", || process_has_exited(pid));
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
        !daemon.take_journal_advanced_wake(),
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
    let observed = daemon
        .observe_session_lifecycle(&session_id, 20)
        .expect("positive observe control");
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
        daemon.take_journal_advanced_wake(),
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

    let record_path = data_dir.join("sessions").join("oversized-adoption.json");
    let valid_record = fs::read(&record_path).expect("read valid registry record for cleanup");
    let mut record: botster_core_daemon::RegistryRecord =
        serde_json::from_slice(&valid_record).expect("decode persisted registry record");
    record.metadata = CoreSessionMetadata::from_entries(BTreeMap::from([(
        "host.example/oversized".to_string(),
        "x".repeat(MAX_CORE_SESSION_METADATA_LEN + 1),
    )]));
    fs::write(
        &record_path,
        serde_json::to_vec_pretty(&record).expect("encode oversized registry record"),
    )
    .expect("write oversized registry record");
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
fn worker_shutdown_waits_for_delayed_progress_before_release_can_preserve_nothing() {
    let data_dir = short_temp_data_dir("delayed");
    let session_id = SessionId("delay".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker-backed session");
    let (worker_pid, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    let mut stopped = StoppedProcess::new(worker_pid);
    let resume = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        signal_process(worker_pid, "CONT");
    });

    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should complete after worker progress resumes");
    resume.join().expect("resume worker thread");
    stopped.resumed = true;

    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "shutdown must not report success while the worker is stopped"
    );
    let listed = daemon.list().expect("list completed worker session");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);
    let first = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("delayed-final-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("read retained final screen");
    let second = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("delayed-final-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("repeat retained final screen");
    assert_eq!(first.screen.text, second.screen.text);

    daemon.release_for_restart();
    drop(daemon);
    wait_for_condition("bounded reap after delayed shutdown", || {
        !process_exists(worker_pid) && !process_exists(pty_child_pid)
    });
    assert!(!socket_path.exists());
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

    let error = daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect_err("stopped worker must not produce truthful completion");
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
    for tick in 0..500 {
        let _ = daemon.drain(&session_id, 30 + tick);
        let listed = daemon.list().expect("list resumed worker session");
        if listed[0].registry_state == RegistrySessionState::Exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        daemon.list().expect("list cleaned worker session")[0].registry_state,
        RegistrySessionState::Exited
    );

    daemon.release_for_restart();
    drop(daemon);
    wait_for_condition("bounded reap after timeout resume", || {
        !process_exists(worker_pid) && !process_exists(pty_child_pid)
    });
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
    let record_path = data_dir
        .join("sessions")
        .join("daemon-adoption-read-only-session.json");
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
    fs::write(
        data_dir.join("sessions").join("daemon-bad-record.json"),
        b"not json",
    )
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
fn worker_backed_capture_color_and_snapshot_agrees_with_ghostsnp_import_after_osc_mutations() {
    // Production path: worker-backed CoreDaemon atomic dual-return after OSC
    // palette/special mutations, with GHOSTSNP import equality and stability.
    let data_dir = temp_data_dir("color-snap-atomic");
    let host_color_profile = host_test_color_profile();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_terminal_color_profile(host_color_profile.clone()),
    );
    let session_id = SessionId("color-snap-session".to_string());
    let client_id = ClientId("color-snap-client".to_string());

    // Child applies OSC 4 (palette index 3), OSC 10/11/12 specials, then echoes
    // a marker so drain-before-read has terminal truth to observe.
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = [
        "printf ready; ",
        // palette index 3 -> rgb 0x11/0x22/0x33",
        "printf '\\033]4;3;rgb:1111/2222/3333\\033\\\\'; ",
        // FG 0xaa/0xbb/0xcc, BG 0x01/0x02/0x03, cursor 0xfe/0xfd/0xfc",
        "printf '\\033]10;rgb:aaaa/bbbb/cccc\\033\\\\'; ",
        "printf '\\033]11;rgb:0101/0202/0303\\033\\\\'; ",
        "printf '\\033]12;rgb:fefe/fdfd/fcfc\\033\\\\'; ",
        "printf 'echo:color-mutated\\n'; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = mutate-again ]; then ",
        "printf '\\033]4;3;rgb:4444/5555/6666\\033\\\\'; ",
        "printf '\\033]10;rgb:1212/3434/5656\\033\\\\'; ",
        "printf 'echo:color-mutated-again\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done",
    ]
    .concat();

    daemon.spawn(request, 10).expect("spawn color-snap session");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("color-snap-sub".to_string()),
            11,
        )
        .expect("attach color-snap session");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    // Wait until OSC mutations reach the Ghostty shadow via drain-before-read.
    let first = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-mutated",
        12,
        |profile| {
            profile.colors.get(&3)
                == Some(&Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0xaa,
                        g: 0xbb,
                        b: 0xcc,
                    })
                && profile.colors.get(&COLOR_INDEX_BACKGROUND)
                    == Some(&Rgb {
                        r: 0x01,
                        g: 0x02,
                        b: 0x03,
                    })
                && profile.colors.get(&COLOR_INDEX_CURSOR)
                    == Some(&Rgb {
                        r: 0xfe,
                        g: 0xfd,
                        b: 0xfc,
                    })
        },
    );

    assert_eq!(
        first.snapshot.request_id,
        RequestId("capture-color-and-snapshot".to_string())
    );
    assert_eq!(first.snapshot.session_id, session_id);
    assert_eq!(first.snapshot.data, first.payload.bytes);
    assert_ghostty_snapshot_authority(&first.payload);
    assert!(first.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_color_profile_authority(&first.color_profile);
    assert_eq!(
        first.color_profile.colors.get(&3),
        Some(&Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        Some(&Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        Some(&Rgb {
            r: 0x01,
            g: 0x02,
            b: 0x03
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        Some(&Rgb {
            r: 0xfe,
            g: 0xfd,
            b: 0xfc
        })
    );
    // Host baseline must not rewrite live OSC-owned colors after start.
    assert_ne!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        host_color_profile.colors.get(&COLOR_INDEX_FOREGROUND)
    );

    // GHOSTSNP content proof: import into a fresh Ghostty terminal and fully
    // compare the restored profile with the paired atomic result (all 256
    // palette entries + specials), while keeping targeted mutation diagnostics.
    let restored = ghostty_snapshot_color_profile(&first.payload);
    assert_eq!(
        restored.colors.get(&3),
        first.color_profile.colors.get(&3),
        "imported GHOSTSNP palette index 3 must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_FOREGROUND),
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        "imported GHOSTSNP foreground must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_BACKGROUND),
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        "imported GHOSTSNP background must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_CURSOR),
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        "imported GHOSTSNP cursor must match atomic pair"
    );
    assert_eq!(
        restored, first.color_profile,
        "imported GHOSTSNP full color profile must equal the paired atomic profile"
    );

    // Hold the first pair; mutate live; prove held pair is stable and a new
    // capture differs.
    let held_profile = first.color_profile.clone();
    let held_payload = first.payload.clone();
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"mutate-again\n".to_vec(),
            50,
        )
        .expect("second OSC mutation input");
    let second = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-mutated-again",
        51,
        |profile| {
            profile.colors.get(&3)
                == Some(&Rgb {
                    r: 0x44,
                    g: 0x55,
                    b: 0x66,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0x12,
                        g: 0x34,
                        b: 0x56,
                    })
        },
    );
    assert_eq!(
        held_profile, first.color_profile,
        "held atomic color pair must not mutate with live terminal"
    );
    assert_eq!(
        held_payload, first.payload,
        "held atomic snapshot pair must not mutate with live terminal"
    );
    assert_ne!(
        second.color_profile.colors.get(&3),
        held_profile.colors.get(&3),
        "live re-capture must observe post-mutation palette"
    );
    assert_ne!(
        second.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        held_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        "live re-capture must observe post-mutation foreground"
    );
    let second_restored = ghostty_snapshot_color_profile(&second.payload);
    assert_eq!(
        second_restored.colors.get(&3),
        second.color_profile.colors.get(&3)
    );
    assert_eq!(
        second_restored.colors.get(&COLOR_INDEX_FOREGROUND),
        second.color_profile.colors.get(&COLOR_INDEX_FOREGROUND)
    );
    assert_eq!(
        second_restored, second.color_profile,
        "second capture GHOSTSNP full profile must equal its paired atomic profile"
    );

    // Drain-before-read egress retention parity: next explicit drain returns
    // client egress retained from internal drains exactly once for markers.
    let drained = daemon
        .drain(&session_id, 80)
        .expect("drain after atomic captures should succeed");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:color-mutated") || output.contains("echo:color-mutated-again"),
        "internal atomic capture drain must retain client egress: {output:?}"
    );

    daemon.shutdown(Some(session_id), 90).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn natural_exit_capture_color_and_snapshot_freezes_repeatable_pair() {
    // Direct retained-path proof for the new public dual-return: first
    // reconciling read after natural exit is capture_color_and_snapshot.
    let data_dir = temp_data_dir("color-snap-natural-exit");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("color-snap-exit-session".to_string());
    let client_id = ClientId("color-snap-exit-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = [
        "printf ready; ",
        "printf '\\033]4;5;rgb:5555/6666/7777\\033\\\\'; ",
        "printf '\\033]10;rgb:1010/2020/3030\\033\\\\'; ",
        "printf '\\033]11;rgb:4040/5050/6060\\033\\\\'; ",
        "printf '\\033]12;rgb:7070/8080/9090\\033\\\\'; ",
        "printf 'echo:color-exit-ready\\n'; ",
        "IFS= read -r line; ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "exit 0",
    ]
    .concat();

    daemon
        .spawn(request, 10)
        .expect("natural-exit color session should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("color-snap-exit-sub".to_string()),
            11,
        )
        .expect("natural-exit color session should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    // Ensure OSC mutations reach the Ghostty shadow while the session is live.
    let _ = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-exit-ready",
        12,
        |profile| {
            profile.colors.get(&5)
                == Some(&Rgb {
                    r: 0x55,
                    g: 0x66,
                    b: 0x77,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0x10,
                        g: 0x20,
                        b: 0x30,
                    })
        },
    );

    // Capture worker identity before exit so we can wait for exact process and
    // control-socket death without sleeping past a race into the live path.
    let (_, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"color-exit\n".to_vec(),
            20,
        )
        .expect("natural-exit trigger input should write");
    wait_for_condition("worker process and control route completion", || {
        !process_exists(pty_child_pid) && UnixStream::connect(&socket_path).is_err()
    });
    // Process is gone, but the daemon has not yet reconciled lifecycle.
    assert_eq!(
        daemon.list().expect("list unreconciled session")[0].registry_state,
        RegistrySessionState::Running,
        "registry must still report Running before the first reconciling capture"
    );

    // First daemon lifecycle reconciliation after process exit: freeze retained
    // color+snapshot pair through capture_color_and_snapshot (not drain/read_screen).
    // resolve_readback drains the dead worker, freezes retained terminal state, and
    // returns that frozen pair — the live dual-return branch is not used.
    let first = daemon
        .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
            request_id: RequestId("natural-exit-color-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("first natural-exit capture_color_and_snapshot should freeze retained truth");

    // Public lifecycle discriminator: after the first reconciling capture, mutable
    // session paths must reject with SessionNotReadable before any later readback.
    // This proves capture_color_and_snapshot reconciled exit without relying only
    // on a later drain.
    assert!(
        matches!(
            daemon.input(
                client_id,
                session_id.clone(),
                b"should-not-write\n".to_vec(),
                21,
            ),
            Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
        ),
        "first reconciling capture must make subsequent input SessionNotReadable"
    );

    // Second call serves the frozen pair without re-entering a live terminal.
    // This is the retained branch: process/socket are already dead, so a live
    // re-capture could not produce a fresh authoritative Ghostty pair.
    let second = daemon
        .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
            request_id: RequestId("natural-exit-color-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("retained capture_color_and_snapshot should be repeatable");

    assert_eq!(
        first.color_profile, second.color_profile,
        "retained freeze must serve the same color profile"
    );
    assert_eq!(
        first.payload, second.payload,
        "retained freeze must serve the same GHOSTSNP payload"
    );
    assert_ne!(first.snapshot.request_id, second.snapshot.request_id);
    // Symmetric retained snapshot path must agree with the dual-return freeze.
    let retained_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-from-retained".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("capture_snapshot should serve the same retained freeze");
    assert_eq!(
        retained_snapshot.payload, first.payload,
        "retained snapshot projection must match the frozen dual-return pair"
    );
    assert_eq!(
        first.snapshot.request_id,
        RequestId("natural-exit-color-1".to_string())
    );
    assert_eq!(
        second.snapshot.request_id,
        RequestId("natural-exit-color-2".to_string())
    );
    assert_ghostty_snapshot_authority(&first.payload);
    assert!(first.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_color_profile_authority(&first.color_profile);
    assert_eq!(
        first.color_profile.colors.get(&5),
        Some(&Rgb {
            r: 0x55,
            g: 0x66,
            b: 0x77
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        Some(&Rgb {
            r: 0x10,
            g: 0x20,
            b: 0x30
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        Some(&Rgb {
            r: 0x40,
            g: 0x50,
            b: 0x60
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        Some(&Rgb {
            r: 0x70,
            g: 0x80,
            b: 0x90
        })
    );

    let restored = ghostty_snapshot_color_profile(&first.payload);
    assert_eq!(
        restored, first.color_profile,
        "retained GHOSTSNP full profile must equal the frozen atomic pair"
    );

    let exit_drain = daemon
        .drain(&session_id, 23)
        .expect("drain after retained color capture should return final output");
    assert_retained_exit_output(&exit_drain, &session_id, "echo:color-exit");
    let second_drain = daemon
        .drain(&session_id, 24)
        .expect("second drain after retained color capture should succeed");
    assert_no_duplicate_exit_output(&second_drain, "echo:color-exit");

    daemon
        .shutdown(Some(session_id), 25)
        .expect("shutdown after retained color capture");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_kitty_and_mouse_input_reaches_child_pty() {
    let data_dir = temp_data_dir("kitty-mouse-input-pty");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("kitty-mouse-input-session".to_string());
    let client_id = ClientId("kitty-mouse-input-client".to_string());

    // Child hex-encodes exact raw stdin bytes so the production input path can
    // prove Kitty and mouse encodings reach the PTY unchanged.
    let kitty_bytes = b"\x1b[27u".to_vec();
    let mouse_bytes = b"\x1b[<0;10;20M".to_vec();
    let mut expected = kitty_bytes.clone();
    expected.extend_from_slice(&mouse_bytes);
    let expected_len = expected.len();

    let mut request = spawn_request(&session_id);
    // Use dd for an exact-length raw read and od for hex so we prove the
    // production CoreDaemon::input path without depending on Python startup.
    request.request.arguments[1] = format!(
        "stty -echo raw 2>/dev/null; printf ready; dd bs=1 count={len} 2>/dev/null | od -An -tx1 | tr -d ' \n'; printf '\n'",
        len = expected_len
    );

    daemon
        .spawn(request, 10)
        .expect("spawn input-proof session");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("kitty-mouse-sub".to_string()),
            11,
        )
        .expect("attach input-proof session");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    daemon
        .input(client_id.clone(), session_id.clone(), kitty_bytes, 12)
        .expect("kitty input should write through CoreDaemon::input");
    daemon
        .input(client_id.clone(), session_id.clone(), mouse_bytes, 13)
        .expect("mouse input should write through CoreDaemon::input");

    let mut expected_hex = String::new();
    for byte in &expected {
        expected_hex.push_str(&format!("{byte:02x}"));
    }
    let screen = read_screen_until(&mut daemon, &session_id, &expected_hex, 14);
    assert!(
        screen.screen.text.contains(&expected_hex),
        "child PTY must receive exact Kitty+mouse input bytes; text={}",
        screen.screen.text
    );

    daemon
        .shutdown(Some(session_id), 15)
        .expect("shutdown input-proof session");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_flags_include_kitty_and_mouse_from_ghostty_authority() {
    let data_dir = temp_data_dir("mode-flags-full-authority");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("mode-flags-full-session".to_string());
    let client_id = ClientId("mode-flags-full-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-modes ]; then ",
        "printf '\\033[?1000h\\033[?1006h\\033[=1;1u\\033[?2004h\\033[?1004h\\033[?1h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("mode-full-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"enable-modes\n".to_vec(),
            12,
        )
        .expect("enable modes");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-modes", 13);

    let mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-full".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("read production mode flags");
    assert_mode_flags_authority(
        &mode_flags.mode_flags.mode_flags,
        ModeFlags {
            kitty_enabled: true,
            mouse_mode: 9,
            bracketed_paste: true,
            focus_reporting: true,
            application_cursor: true,
            ..ModeFlags::default()
        },
    );

    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("mode-full-snap".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("capture snapshot");
    assert_ghostty_snapshot_authority(&snapshot.payload);
    assert!(snapshot.payload.bytes.starts_with(GHOSTSNP_MAGIC));

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_input_admits_matching_token_and_rejects_stale() {
    let data_dir = temp_data_dir("mode-gated-admit");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("mode-gated-session".to_string());
    let client_id = ClientId("mode-gated-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-modes ]; then ",
        "printf '\\033[?1000h\\033[?1006h\\033[=1;1u'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("mode-gated-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-gated-baseline".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline modes");
    let baseline_token = baseline.mode_flags.mode_freshness;
    assert_ne!(baseline_token.mode_generation, 0);

    let admitted = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"enable-modes\n".to_vec(),
            Some(baseline_token),
            13,
        )
        .expect("matching gated input");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-modes", 14);

    let after_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-gated-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("modes after enable");
    assert!(after_modes.mode_flags.mode_flags.kitty_enabled);
    assert_eq!(after_modes.mode_flags.mode_flags.mouse_mode, 9);

    let stale = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"stale-input\n".to_vec(),
            Some(baseline_token),
            16,
        )
        .expect("stale gated input returns typed result");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "stale token must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    thread::sleep(Duration::from_millis(100));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("mode-gated-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 17,
        })
        .expect("screen after stale reject");
    assert!(
        !screen.screen.text.contains("echo:stale-input"),
        "stale gated input must write zero PTY bytes; screen={}",
        screen.screen.text
    );

    let current = after_modes.mode_flags.mode_freshness;
    let again = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"fresh-input\n".to_vec(),
            Some(current),
            18,
        )
        .expect("fresh gated input");
    match again {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    let _ = read_screen_until(&mut daemon, &session_id, "echo:fresh-input", 19);

    daemon
        .mode_gated_input(
            ClientId("mode-gated-client".to_string()),
            session_id.clone(),
            b"plain-path\n".to_vec(),
            None,
            20,
        )
        .expect("plain path");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:plain-path", 21);

    daemon.shutdown(Some(session_id), 22).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_race_a_queued_before_probe_rejects() {
    // Race (a): mode-changing output is available before the probe forms a
    // reply but must not be admitted against a token that missed that output.
    // Hold delays the worker barrier drain so a concurrent flip is applied only
    // at admit time under the reader fence.
    let data_dir = temp_data_dir("mode-gated-race-a");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(200)),
    );
    let session_id = SessionId("mode-gated-race-a".to_string());
    let client_id = ClientId("mode-gated-race-a-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("race-a-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("race-a-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    // Emit mode-changing output after probe, then admit under hold so the
    // worker applies that output after the hold but before the write.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-race-a\n".to_vec(),
            Some(token),
            14,
        )
        .expect("race-a gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "race (a) must reject stale token");
            assert!(
                result.mode_freshness != token || result.mode_flags.mouse_mode != 0,
                "worker must surface post-flip mode state"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(150));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("race-a-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-race-a"),
        "race (a) must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_race_b_after_probe_before_admit_rejects() {
    // Race (b): mode change after probe reply, before admission. Do not wait for
    // parent screen echo before admitting — the worker fence must catch it.
    let data_dir = temp_data_dir("mode-gated-race-b");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(150)),
    );
    let session_id = SessionId("mode-gated-race-b".to_string());
    let client_id = ClientId("mode-gated-race-b-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("race-b-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("race-b-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip modes");
    // Immediately admit with pre-flip token while hold keeps the fence open.
    let stale = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-race-b\n".to_vec(),
            Some(token),
            14,
        )
        .expect("race-b gated");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "race (b) must reject stale token");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(150));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("race-b-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-race-b"),
        "race (b) must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_post_final_drain_hold_rejects() {
    // Hold after the initial queue drain under the reader fence, then release
    // mode-changing output into the OS PTY buffer before the final drain/write.
    let data_dir = temp_data_dir("mode-gated-hold");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(200)),
    );
    let session_id = SessionId("mode-gated-hold".to_string());
    let client_id = ClientId("mode-gated-hold-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("hold-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("hold-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"held-stale\n".to_vec(),
            Some(token),
            14,
        )
        .expect("held gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "post-final-drain hold must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(200));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("hold-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:held-stale"),
        "held race must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_timeout_fails_closed_without_late_write() {
    // Parent wait is write-timeout + 1s reply grace. Hold past that so the
    // parent clears the wait slot as a true timeout (not a correlated deadline
    // result). The worker still must not write after its wall-clock deadline.
    let data_dir = temp_data_dir("mode-gated-timeout");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(Duration::from_millis(120))
            .with_test_mode_gated_hold_ms(Some(2_000)),
    );
    let session_id = SessionId(format!(
        "mode-gated-timeout-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let client_id = ClientId("mode-gated-timeout-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("timeout-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("timeout-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let error = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"timeout-bytes\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect_err("short timeout must fail closed");
    let message = error.to_string();
    assert!(
        message.contains("timed out") || message.contains("timeout"),
        "expected timeout failure, got {message}"
    );

    // Wait past the worker hold so a non-fail-closed worker would write.
    thread::sleep(Duration::from_millis(1_500));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("timeout-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("screen after timeout");
    assert!(
        !screen.screen.text.contains("echo:timeout-bytes"),
        "timeout must not allow late PTY write; screen={}",
        screen.screen.text
    );

    daemon.shutdown(Some(session_id), 15).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_interleaved_output_during_wait() {
    let data_dir = temp_data_dir("mode-gated-interleave");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(250)),
    );
    let session_id = SessionId("mode-gated-interleave".to_string());
    let client_id = ClientId("mode-gated-interleave-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("interleave-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("interleave-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    // Fire plain input that produces output while a gated wait is held.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"interleaved\n".to_vec(),
            13,
        )
        .expect("interleaved input");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"after-interleave\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            14,
        )
        .expect("gated during interleave");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    let screen = read_screen_until(&mut daemon, &session_id, "echo:after-interleave", 15);
    assert!(
        screen.screen.text.contains("echo:interleaved"),
        "interleaved output must still demux to parent; screen={}",
        screen.screen.text
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_unpublished_reader_chunk_window_rejects() {
    // Hold after the background reader captures mode bytes and before fence
    // enqueue onto the single fence-owned queue. The barrier must wait for
    // publication, apply the modes, and reject the stale token — not write while
    // the chunk is unpublished.
    let data_dir = temp_data_dir("mode-gated-unpub-chunk");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_hold_after_read_ms(Some(250)),
    );
    let session_id = SessionId("mode-gated-unpub".to_string());
    let client_id = ClientId("mode-gated-unpub-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("unpub-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("unpub-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    // Give the reader time to capture mode output into the after-read hold.
    thread::sleep(Duration::from_millis(50));
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-unpub\n".to_vec(),
            Some(token),
            14,
        )
        .expect("gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "unpublished chunk window must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(300));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("unpub-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-unpub"),
        "must write zero stale bytes; screen={}",
        screen.screen.text
    );
    daemon.shutdown(Some(session_id), 16).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_write_deadline_bounds_complete_write() {
    // Force write WouldBlock past the worker write deadline so the worker must
    // return a correlated deadline result without delivering the payload. The
    // parent wait includes reply grace so this test must observe that result
    // (parent timeout is not accepted). After the block lifts, re-check screen
    // so a non-enforcing worker cannot pass by writing late.
    let data_dir = temp_data_dir("mode-gated-write-deadline");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    // Block long enough for spawn/probe/scheduling plus the 1s write deadline.
    // Keep the window short enough that waiting past it is practical in CI.
    let write_timeout = Duration::from_secs(1);
    let block_until = now_ms + 8_000;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(write_timeout)
            .with_test_write_block_until_unix_ms(Some(block_until)),
    );
    let session_id = SessionId(format!("mode-gated-wdeadline-{now_ms}"));
    let client_id = ClientId("mode-gated-wdeadline-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("wdeadline-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("wdeadline-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"deadline-bytes\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect("must receive correlated worker gated result (not parent timeout)");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "deadline must not admit");
            assert_eq!(result.bytes_written, 0, "deadline must write zero bytes");
            let kind = result.error_kind.unwrap_or_default();
            assert!(
                kind.contains("deadline"),
                "expected deadline error_kind from worker, got {kind}"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated fail-closed outcome"),
    }

    // Wait until the forced WouldBlock window has ended so a worker that
    // ignored the deadline would complete the write and echo the payload.
    let after_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    if after_ms < block_until + 250 {
        thread::sleep(Duration::from_millis(block_until + 250 - after_ms));
    }
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("wdeadline-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:deadline-bytes"),
        "deadline-bounded write must deliver zero payload bytes even after block lifts; screen={}",
        screen.screen.text
    );
    daemon.shutdown(Some(session_id), 15).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_fence_queue_preserves_mode_order() {
    // Capacity-1 fence queue + hold-after-read: opposite mode transitions are
    // enqueued in order on the single ownership queue. Barrier/probe drain must
    // keep FIFO enable then disable → mouse off. Reversed order leaves mouse on.
    let data_dir = temp_data_dir("mode-gated-fence-order");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_mode_gated_input_timeout(Duration::from_secs(5))
            .with_test_hold_after_read_ms(Some(60)),
    );
    let session_id = SessionId("mode-gated-fence-order".to_string());
    let client_id = ClientId("mode-gated-fence-order-client".to_string());
    let mut request = spawn_request(&session_id);
    // Separate writes + sleep so the single fence queue captures enable then
    // disable as distinct chunks under hold-after-read.
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = seq ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "sleep 0.05; ",
        "printf '\\033[?1000l\\033[?1006l'; ",
        "printf 'seq-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("fence-order-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-base".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline");
    let baseline_token = baseline.mode_flags.mode_freshness;
    assert_eq!(baseline.mode_flags.mode_flags.mouse_mode, 0);

    daemon
        .input(client_id.clone(), session_id.clone(), b"seq\n".to_vec(), 13)
        .expect("seq");
    // Wait for enable hold (~60ms) + disable capture on the single fence queue,
    // then drain via probe (barrier path).
    thread::sleep(Duration::from_millis(120));
    let start = Instant::now();
    let after_seq = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("probe must drain fence queue without hang");
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "mode probe must not hang when the fence queue holds mode bytes"
    );
    assert_eq!(
        after_seq.mode_flags.mode_flags.mouse_mode, 0,
        "FIFO enable-then-disable must leave mouse off; reversed drain leaves mouse on"
    );

    // Wait for shell marker so both transitions are definitely applied, then
    // re-probe for a stable token used in admit/reject assertions.
    let _ = drain_until(&mut daemon, &session_id, "seq-done");
    let final_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-final".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("final modes");
    assert_eq!(
        final_modes.mode_flags.mode_flags.mouse_mode, 0,
        "final mouse mode must remain off after ordered sequence"
    );

    let stale = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"order-stale\n".to_vec(),
            Some(baseline_token),
            16,
        )
        .expect("stale gated");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            // If the sequence advanced modes, baseline is stale. If timing
            // applied both transitions with a net-zero mode delta before any
            // revision bump observation, admit is still safe only with final
            // mouse-off (asserted above); require reject when revision moved.
            if final_modes.mode_flags.mode_freshness != baseline_token {
                assert!(!result.admitted, "stale baseline token must reject");
            }
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    let admitted = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"fresh-after-order\n".to_vec(),
            Some(final_modes.mode_flags.mode_freshness),
            17,
        )
        .expect("current token");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(
                result.admitted,
                "current token after ordered apply must admit"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 17).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_ordinary_pressure_stays_lossless() {
    // Tiny public capacity + tiny fence pending + flood: ordinary pressure must
    // wait/backpressure, not latch sticky mode-authority failure. After the
    // flood drains, probes and current-token gated admit still succeed.
    // True-loss sticky fail-closed is covered by internal unit seams
    // (set_overflow_error / Failed event) without a public production flag.
    let data_dir = temp_data_dir("mode-gated-pressure-lossless");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_test_pending_capacity(Some(1))
            .with_mode_gated_input_timeout(Duration::from_secs(5)),
    );
    let session_id = SessionId("mode-gated-pressure-lossless".to_string());
    let client_id = ClientId("mode-gated-pressure-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = flood ]; then ",
        "i=0; while [ $i -lt 40 ]; do ",
        "printf 'flood-%s:%04000d\\n' \"$i\" 0; ",
        "i=$((i+1)); ",
        "done; ",
        "printf 'flood-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("pressure-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flood\n".to_vec(),
            13,
        )
        .expect("flood");
    let _ = drain_until(&mut daemon, &session_id, "flood-done");
    let after = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("pressure-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("probe must succeed after ordinary pressure (not sticky authority fail)");
    let admitted = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"after-pressure\n".to_vec(),
            Some(after.mode_flags.mode_freshness),
            15,
        )
        .expect("current token after pressure");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(
                result.admitted,
                "ordinary pressure must not latch sticky authority failure"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 16).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_normal_drain_preserves_mode_fifo() {
    // hold_after_enqueue keeps the reader critical while opposite mode chunks
    // sit on the single fence queue. Normal (non-barrier) drain via the worker
    // control loop + parent drain, then a later probe, must keep enable→disable
    // FIFO (mouse off) after both mode transitions apply. Complements the
    // dual-buffer source guard in local_process unit tests (that guard fails on
    // df38c218; this path proves production normal-drain + Ghostty apply order).
    //
    // Deterministic producer/worker-application boundary: the child emits
    // enable + an "enabled" marker, then blocks until the parent releases
    // disable. The test observes worker-applied mouse-on (and a revision bump)
    // before releasing disable. Without that boundary, enable+disable can
    // coalesce into one PTY read; the worker samples ModeFlags once per chunk
    // and net-zero final modes leave mode_revision unchanged (production
    // contract), which made the old sleep-separated form flake.
    let data_dir = temp_data_dir("mode-gated-normal-drain-fifo");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_test_hold_after_enqueue_ms(Some(80))
            .with_mode_gated_input_timeout(Duration::from_secs(5)),
    );
    let session_id = SessionId("mode-gated-normal-drain".to_string());
    let client_id = ClientId("mode-gated-normal-drain-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = seq ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "printf 'enabled\\n'; ",
        "IFS= read -r _release; ",
        "printf '\\033[?1000l\\033[?1006l'; ",
        "printf 'seq-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("normal-drain-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("normal-drain-base".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline");
    assert_eq!(
        baseline.mode_flags.mode_flags.mouse_mode, 0,
        "baseline mouse must be off before the sequence"
    );
    daemon
        .input(client_id.clone(), session_id.clone(), b"seq\n".to_vec(), 13)
        .expect("seq");
    // Normal-path drains while enqueue holds may still be active (no fixed sleep).
    let start = Instant::now();
    let _ = daemon
        .drain(&session_id, 14)
        .expect("normal drain during hold");
    let _ = drain_until(&mut daemon, &session_id, "enabled");

    // Require worker application of enable before the child may emit disable.
    // Polling is condition-driven; mode sampling (not wall-clock) is the barrier.
    let mut enable_modes = None;
    for tick in 0..200u64 {
        let probe = daemon
            .read_mode_flags(ReadModeFlagsRequest {
                request_id: RequestId(format!("normal-drain-enable-{tick}")),
                session_id: session_id.clone(),
                now_seconds: 15 + tick,
            })
            .expect("enable probe");
        if probe.mode_flags.mode_flags.mouse_mode != 0 {
            enable_modes = Some(probe);
            break;
        }
        let _ = daemon
            .drain(&session_id, 200 + tick)
            .expect("normal drain while waiting for enable apply");
        thread::sleep(Duration::from_millis(10));
    }
    let enable_modes = enable_modes
        .expect("worker must apply enable (mouse on) before release; missing application boundary");
    assert!(
        enable_modes.mode_flags.mode_freshness.mode_revision
            >= baseline
                .mode_flags
                .mode_freshness
                .mode_revision
                .saturating_add(1),
        "enable application must advance revision; baseline={:?} enable={:?}",
        baseline.mode_flags.mode_freshness,
        enable_modes.mode_flags.mode_freshness
    );

    // Release disable only after enable was observed as its own mode sample.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"release\n".to_vec(),
            16,
        )
        .expect("release");
    let _ = daemon
        .drain(&session_id, 17)
        .expect("normal drain during second transition");
    let _ = drain_until(&mut daemon, &session_id, "seq-done");
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "normal drain path must not hang under enqueue hold"
    );
    let final_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("normal-drain-final".to_string()),
            session_id: session_id.clone(),
            now_seconds: 18,
        })
        .expect("final");
    assert_eq!(
        final_modes.mode_flags.mode_flags.mouse_mode, 0,
        "normal-drain FIFO must keep enable→disable (mouse off)"
    );
    // Both transitions applied as distinct observations: enable then disable
    // advances revision by ≥2. Mouse-off alone is insufficient if only disable
    // (or neither) applied. Valid because enable was observed before release.
    assert!(
        final_modes.mode_flags.mode_freshness.mode_revision
            >= baseline
                .mode_flags
                .mode_freshness
                .mode_revision
                .saturating_add(2),
        "expected ≥2 mode revisions (enable then disable); baseline={:?} enable={:?} final={:?}",
        baseline.mode_flags.mode_freshness,
        enable_modes.mode_flags.mode_freshness,
        final_modes.mode_flags.mode_freshness
    );
    assert_ne!(
        final_modes.mode_flags.mode_freshness, baseline.mode_flags.mode_freshness,
        "mode sequence must change freshness so stale admit is meaningful"
    );
    let stale = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-normal-drain\n".to_vec(),
            Some(baseline.mode_flags.mode_freshness),
            19,
        )
        .expect("stale");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "stale must reject after mode sequence");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_partial_write_reports_bytes_written() {
    // First write chunk succeeds (1 byte), then force WouldBlock past the
    // deadline. Public outcome must be admitted=false with nonzero
    // bytes_written and partial_write error_kind — never a clean reject.
    let data_dir = temp_data_dir("mode-gated-partial-write");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    let block_until = now_ms + 30_000;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(Duration::from_millis(800))
            .with_test_write_max_chunk(Some(1))
            .with_test_write_block_until_unix_ms(Some(block_until)),
    );
    let session_id = SessionId(format!("mode-gated-partial-{now_ms}"));
    let client_id = ClientId("mode-gated-partial-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("partial-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("partial-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let payload = b"partial-payload\n".to_vec();
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            payload.clone(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect("partial path returns outcome");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "partial must not report full admit");
            assert!(
                result.bytes_written > 0,
                "partial must report nonzero bytes_written, got {}",
                result.bytes_written
            );
            assert!(
                result.bytes_written < payload.len(),
                "partial bytes_written must be less than payload; written={} len={}",
                result.bytes_written,
                payload.len()
            );
            let kind = result.error_kind.unwrap_or_default();
            assert!(
                kind.contains("partial_write") || kind.contains("deadline"),
                "expected partial_write/deadline error_kind, got {kind}"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated partial outcome"),
    }
    daemon.shutdown(Some(session_id), 14).ok();
    let _ = fs::remove_dir_all(data_dir);
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

#[cfg(unix)]
#[test]
fn worker_backed_osc_color_queries_receive_session_side_write_pty_replies() {
    let data_dir = temp_data_dir("osc-color-write-pty");
    // Host outside Core supplies presentation policy through the policy-free
    // CoreDaemonConfig seam. CoreDaemon itself invents no color defaults.
    let host_color_profile = host_test_color_profile();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_terminal_color_profile(host_color_profile),
    );
    let session_id = SessionId("osc-color-session".to_string());

    // No client attaches. Child emits OSC 10/11/12 queries; session Ghostty must
    // answer via write_pty using the host-supplied color profile. Replies are
    // injected into the child PTY and appear on the production screen path
    // (line-discipline echo of PTY input), proving pre-attach session-side
    // ownership of OSC 10/11/12 for a supplied profile.
    let script_path = data_dir.join("osc_query_child.py");
    fs::create_dir_all(&data_dir).expect("create data dir for script");
    fs::write(
        &script_path,
        r#"#!/usr/bin/env python3
import sys
import time

sys.stdout.write("ready\n")
sys.stdout.flush()
sys.stdout.buffer.write(b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\")
sys.stdout.flush()
# Stay alive long enough for the parent to drain PTY output, run write_pty,
# and inject replies into this process's PTY input.
time.sleep(2)
sys.stdout.write("done\n")
sys.stdout.flush()
"#,
    )
    .expect("write osc child script");

    let mut request = spawn_request(&session_id);
    request.request.executable = "python3".to_string();
    request.request.arguments = vec![script_path.to_string_lossy().into_owned()];

    daemon
        .spawn(request, 10)
        .expect("spawn color query session");
    // Wait until all three host-supplied OSC replies are visible on the session
    // path without any client attach.
    let screen = read_screen_until(&mut daemon, &session_id, "]12;", 11);
    let text = screen.screen.text;
    // Host test profile:
    // FG 0xdd/0xdd/0xdd, BG 0x1e/0x1e/0x2e, cursor 0xf5/0xe0/0xdc
    let seq10 = assert_osc_color_reply_sequence(&text, 10, 0xdd, 0xdd, 0xdd);
    let seq11 = assert_osc_color_reply_sequence(&text, 11, 0x1e, 0x1e, 0x2e);
    let seq12 = assert_osc_color_reply_sequence(&text, 12, 0xf5, 0xe0, 0xdc);
    assert!(
        seq10 < seq11 && seq11 < seq12,
        "expected OSC 10,11,12 bound reply sequences in order; text={text}"
    );

    daemon.shutdown(Some(session_id), 12).ok();
    let _ = fs::remove_dir_all(data_dir);
}

fn mode_flags_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-mouse ]; then printf '\\033[?1000h\\033[?1006h'; fi; ",
        "done"
    )
    .to_string();
    request
}

fn retained_history_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = "printf 'echo:dwgrh-prior-marker\nready'; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string();
    request
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

fn delayed_output_exit_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "sleep 0.2; printf PUMP-RETAINED; exit 0".to_string(),
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
        assert!(
            Instant::now() < deadline,
            "targeted pump did not commit Exited for {}",
            session_id.0
        );
        let batch = daemon.wait_wakes(Duration::from_millis(250));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            continue;
        }
        let _ = daemon
            .pump_woken(&batch, now_seconds)
            .expect("targeted pump");
        if matches!(
            daemon
                .session_registry_state(session_id)
                .expect("non-progress registry lookup"),
            SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
        ) {
            return;
        }
    }
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
    for tick in 0..100 {
        daemon
            .observe_lifecycle(now_seconds + tick)
            .expect("observe_lifecycle should succeed");
        let page = daemon
            .lifecycle_changes_page(after, 16, 16 * 1024)
            .expect("page after observe");
        assert_successful_page_within_budget(&page, 16 * 1024);
        if page_contains_exited(&page, session_id) {
            return page;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "observe_lifecycle did not publish Exited for {}",
        session_id.0
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

#[cfg(unix)]
fn controlled_exit_spawn_request(
    session_id: &SessionId,
    marker_path: &std::path::Path,
    release_path: &std::path::Path,
) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                concat!(
                    "printf ready; IFS= read -r line; ",
                    "printf 'botster-core-natural-exit-marker:%s\\n' \"$line\"; ",
                    "printf observed > \"$1\"; ",
                    "while [ ! -e \"$2\" ]; do sleep 0.01; done; ",
                    "printf 'botster-core-natural-exit-tail\\n'; exit 0"
                )
                .to_string(),
                "botster-controlled-exit".to_string(),
                marker_path.to_string_lossy().into_owned(),
                release_path.to_string_lossy().into_owned(),
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
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..100 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if terminal_output(&aggregate.client_egress).contains(expected) {
            return aggregate;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    aggregate
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

fn wait_for_file(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("child readiness file did not appear: {}", path.display());
}

fn drain_until_attached(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..10_000 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon attach drain should succeed");
        let attached = drained.client_egress.iter().any(|(target, frame)| {
            target == client_id
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        state: TerminalAttachState::Attached,
                        ..
                    }
                )
        });
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if attached {
            return aggregate;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker attach did not reach Attached");
}

fn drain_until_snapshot(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..10_000 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon snapshot drain should succeed");
        let snapshot = drained.client_egress.iter().any(|(target, frame)| {
            target == client_id && matches!(frame, TransportEgress::Snapshot { .. })
        });
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if snapshot {
            return aggregate;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker attach did not emit READY");
}

fn viewport_contains_marker(
    projection: &botster_terminal_ghostty::ViewportProjection,
    marker: &str,
) -> bool {
    let mut row = String::new();
    for (index, cell) in projection.cells.iter().enumerate() {
        if index > 0 && index % projection.cols as usize == 0 {
            if row.contains(marker) {
                return true;
            }
            row.clear();
        }
        row.push_str(&cell.grapheme);
    }
    row.contains(marker)
}

fn drain_until_for_client(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
    expected: &str,
) -> botster_core_daemon::DrainResult {
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_output_length = 0;
    let mut tick = 0;
    let mut aggregate = botster_core_daemon::DrainResult::default();
    loop {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = renderable_output_for_client(&aggregate.client_egress, client_id);
        if output.contains(expected) {
            return aggregate;
        }

        let now = Instant::now();
        if output.len() != last_output_length {
            last_progress = now;
            last_output_length = output.len();
        }
        assert!(
            now.duration_since(last_progress) < REAL_WORKER_IDLE_TIMEOUT
                && now.duration_since(started) < REAL_WORKER_COMPLETION_TIMEOUT,
            "queued client output never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last output: {output:?}"
        );
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_screen_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> botster_core_daemon::ReadScreenResult {
    let request_id = RequestId("read-screen-marker".to_string());
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_text = String::new();
    let mut tick = 0;
    let last = loop {
        let read = daemon
            .read_screen(ReadScreenRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon read_screen should succeed");
        if read.screen.text.contains(expected) {
            return read;
        }
        let now = Instant::now();
        if read.screen.text != last_text {
            last_progress = now;
        }
        if now.duration_since(last_progress) >= REAL_WORKER_IDLE_TIMEOUT
            || now.duration_since(started) >= REAL_WORKER_COMPLETION_TIMEOUT
        {
            break read;
        }
        last_text.clone_from(&read.screen.text);
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    };
    panic!(
        "read_screen never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last text: {:?}",
        last.screen.text
    )
}

#[cfg(unix)]
fn drain_until_terminal_marker(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) {
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_output_length = 0;
    let mut tick = 0;
    let mut aggregate = botster_core_daemon::DrainResult::default();
    loop {
        let drained = daemon
            .drain(session_id, start_tick + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = terminal_output(&aggregate.client_egress);
        if output.contains(expected) {
            return;
        }

        let now = Instant::now();
        if output.len() != last_output_length {
            last_progress = now;
            last_output_length = output.len();
        }
        assert!(
            now.duration_since(last_progress) < REAL_WORKER_IDLE_TIMEOUT
                && now.duration_since(started) < REAL_WORKER_COMPLETION_TIMEOUT,
            "terminal output never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last output: {output:?}"
        );
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn capture_color_and_snapshot_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected_marker: &str,
    start_tick: u64,
    profile_ready: impl Fn(&TerminalColorProfile) -> bool,
) -> botster_core_daemon::CaptureColorAndSnapshotResult {
    let request_id = RequestId("capture-color-and-snapshot".to_string());
    let mut last = None;
    for tick in 0..150 {
        let captured = daemon
            .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_color_and_snapshot should succeed");
        let marker_ready = ghostty_snapshot_replays_marker(&captured.payload, expected_marker);
        if marker_ready && profile_ready(&captured.color_profile) {
            return captured;
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let last = last.expect("at least one atomic capture should complete");
    panic!(
        "capture_color_and_snapshot never observed marker {expected_marker:?} with expected colors; \
         palette[3]={:?} fg={:?} bg={:?} cursor={:?}; format={:?}",
        last.color_profile.colors.get(&3),
        last.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        last.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        last.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        last.payload.format,
    );
}

fn ghostty_snapshot_color_profile(
    payload: &botster_core::TerminalSnapshotPayload,
) -> TerminalColorProfile {
    assert_snapshot_format(payload);
    let mut terminal = GhosttyTerminal::with_config(
        payload.size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty import terminal");
    terminal
        .import_snapshot(payload)
        .expect("test should import daemon Ghostty snapshot");
    terminal
        .read_color_profile()
        .expect("imported GHOSTSNP should expose a color profile")
}

fn capture_snapshot_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> botster_core_daemon::CaptureSnapshotResult {
    let request_id = RequestId("capture-snapshot-marker".to_string());
    let mut last = None;
    for tick in 0..100 {
        let captured = daemon
            .capture_snapshot(CaptureSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_snapshot should succeed");
        if ghostty_snapshot_replays_marker(&captured.payload, expected) {
            return captured;
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "capture_snapshot never observed {expected:?}; last bytes: {:?}",
        last.map(|captured| String::from_utf8_lossy(&captured.payload.bytes).to_string())
    )
}

fn capture_snapshot_and_retained_output_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> (botster_core_daemon::CaptureSnapshotResult, String) {
    let request_id = RequestId("capture-snapshot-marker".to_string());
    let mut aggregate_output = String::new();
    let mut last = None;
    for tick in 0..100 {
        let captured = daemon
            .capture_snapshot(CaptureSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_snapshot should succeed");
        let drained = daemon
            .drain(session_id, start_tick + tick + 100)
            .expect("drain after capture_snapshot should succeed");
        aggregate_output.push_str(&terminal_output(&drained.client_egress));
        if aggregate_output.contains(expected)
            && ghostty_snapshot_replays_marker(&captured.payload, expected)
        {
            return (captured, aggregate_output);
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "capture_snapshot never retained {expected:?}; last format: {:?}; output: {:?}",
        last.and_then(|captured| captured.payload.format),
        aggregate_output
    )
}

/// Host-owned test color profile used only by daemon integration proofs.
///
/// Presentation policy is intentionally not hardcoded inside CoreDaemon.
fn host_test_color_profile() -> TerminalColorProfile {
    let mut colors = HashMap::new();
    colors.insert(
        COLOR_INDEX_FOREGROUND,
        Rgb {
            r: 0xdd,
            g: 0xdd,
            b: 0xdd,
        },
    );
    colors.insert(
        COLOR_INDEX_BACKGROUND,
        Rgb {
            r: 0x1e,
            g: 0x1e,
            b: 0x2e,
        },
    );
    colors.insert(
        COLOR_INDEX_CURSOR,
        Rgb {
            r: 0xf5,
            g: 0xe0,
            b: 0xdc,
        },
    );
    TerminalColorProfile { colors }
}

/// Assert one complete OSC identifier+value sequence and return its index.
///
/// Ghostty encodes OSC color replies as `]Ps;rgb:RRRR/GGGG/BBBB` with 16-bit
/// channels (high byte == low byte for 8-bit source colors). The identifier and
/// RGB value must appear as one bound sequence so mismatched OSC/RGB pairs fail.
fn assert_osc_color_reply_sequence(reply_text: &str, osc: u8, r: u8, g: u8, b: u8) -> usize {
    let rr = format!("{r:02x}{r:02x}");
    let gg = format!("{g:02x}{g:02x}");
    let bb = format!("{b:02x}{b:02x}");
    let expected = format!("]{osc};rgb:{rr}/{gg}/{bb}");
    let lowered = reply_text.to_ascii_lowercase();
    lowered
        .find(&expected)
        .unwrap_or_else(|| panic!("expected bound OSC sequence {expected} in reply {reply_text}"))
}

fn assert_snapshot_format(payload: &botster_core::TerminalSnapshotPayload) {
    assert_eq!(
        payload.format.as_deref(),
        Some(EXPECTED_SNAPSHOT_FORMAT),
        "snapshot format should match the active daemon terminal backend"
    );
}

fn assert_snapshot_payload_observed_marker_when_plain(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) {
    // Plain fallback snapshots are no longer a production path. Keep the helper
    // as a no-op so historical call sites stay readable next to Ghostty asserts.
    let _ = (payload, expected);
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

fn ghostty_snapshot_replays_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) -> bool {
    ghostty_snapshot_plain_text(payload).contains(expected)
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

fn first_snapshot_for_client_at_size(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
    size: TerminalScreenSize,
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
            size,
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
    let mut last = None;
    for tick in 0..100 {
        let looked_up = daemon
            .observe_session_lifecycle(session_id, now_seconds + tick)
            .expect("exact query");
        if matches!(
            &looked_up,
            SessionLifecycleLookup::Found(record)
                if record.session.registry_state == RegistrySessionState::Exited
                    && matches!(
                        record.lifecycle,
                        Some(SessionLifecycleState::Exited { .. })
                    )
        ) {
            return looked_up;
        }
        last = Some(looked_up);
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "observe_session_lifecycle did not reconcile parked ProcessExited for {}: {last:?}",
        session_id.0
    );
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-daemon-{label}-{nanos}"))
}

fn short_temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::path::PathBuf::from("/tmp").join(format!("bcd-{label}-{nanos}"))
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
fn process_has_exited(pid: u32) -> bool {
    if !process_exists(pid) {
        return true;
    }
    let output = Command::new("ps")
        .args(["-o", "state=", "-p", &pid.to_string()])
        .output();
    match output {
        Ok(output) => {
            let state = String::from_utf8_lossy(&output.stdout);
            let state = state.trim();
            state.is_empty() || state.starts_with('Z')
        }
        Err(_) => !process_exists(pid),
    }
}

#[cfg(unix)]
fn wait_for_condition(label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{label} was not observed within {REAL_WORKER_COMPLETION_TIMEOUT:?}");
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
    static BUILD_WORKER: Once = Once::new();
    BUILD_WORKER.call_once(|| {
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "botster-core-daemon",
                "--bin",
                "botster-session-worker",
            ])
            .status()
            .expect("worker binary build command should run");
        assert!(
            status.success(),
            "worker binary should build for daemon restart test"
        );
    });

    let mut path = std::env::current_exe().expect("test executable path should resolve");
    while path.file_name().and_then(|name| name.to_str()) != Some("debug") {
        assert!(path.pop(), "test executable should live under target/debug");
    }
    path.join("botster-session-worker")
}

fn compact_input_frame(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).expect("input fits u16");
    let mut bytes = vec![1, 1];
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn compact_mode_gated_frame(mode_generation: u64, mode_revision: u64, data: &[u8]) -> Vec<u8> {
    let body_len = u16::try_from(16 + data.len()).expect("gated body fits u16");
    let mut bytes = vec![1, 2];
    bytes.extend_from_slice(&body_len.to_be_bytes());
    bytes.extend_from_slice(&mode_generation.to_be_bytes());
    bytes.extend_from_slice(&mode_revision.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn adapter_input_result_subscription(bytes: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    if value.get("type")?.as_str()? != "input_result" {
        return None;
    }
    value
        .get("subscription_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn wait_until_bound_attached(
    daemon: &mut CoreDaemon,
    _session_id: &SessionId,
    adapter: &SharedFakeTerminalAdapter,
) {
    let started = Instant::now();
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(daemon, 20);
        let attached = adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| {
                let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
                    return false;
                };
                value.get("type").and_then(serde_json::Value::as_str) == Some("attach_state")
                    && value.get("state").and_then(serde_json::Value::as_str) == Some("attached")
            });
        if attached {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "bound adapter never reached attached: {:?}",
        adapter.snapshot_delivered_frame_bytes()
    );
}

fn pump_next_available_wake(daemon: &mut CoreDaemon, now_seconds: u64) -> bool {
    let batch = daemon.wait_wakes(Duration::from_millis(250));
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
        .list_terminal_subscriptions()
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

fn attach_bound_adapter(
    daemon: &mut CoreDaemon,
    session_id: SessionId,
    client_id: ClientId,
    subscription_id: SubscriptionId,
    now: u64,
) -> SharedFakeTerminalAdapter {
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            now,
        )
        .expect("attach replacement owner");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after replacement attach")
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
        .expect("bind replacement owner");
    wait_until_bound_attached(daemon, &session_id, &adapter);
    adapter
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
        pump_next_available_wake(&mut daemon, 21);
        for bytes in adapter.snapshot_delivered_frame_bytes() {
            if adapter_frame_type(&bytes) == "terminal_output"
                && adapter_payload_text(&bytes).contains("echo:ORACLE")
            {
                saw_echo = true;
            }
            if adapter_input_result_subscription(&bytes).as_deref()
                == Some(subscription_id.0.as_str())
            {
                saw_result_id = true;
            }
        }
        if saw_echo && saw_result_id {
            break;
        }
        thread::sleep(Duration::from_millis(10));
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
fn pump_woken_queue_overflow_tears_down_one_owner_and_keeps_a_sibling_session() {
    let data_dir = temp_data_dir("duplex-overflow");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0)
            .with_test_mode_gated_hold_ms(Some(10_000)),
    );
    let flooded = SessionId("duplex-overflow-flood".to_string());
    let sibling = SessionId("duplex-overflow-sibling".to_string());
    let flood_sub = SubscriptionId("duplex-overflow-flood-sub".to_string());
    let sibling_sub = SubscriptionId("duplex-overflow-sibling-sub".to_string());
    let flood_adapter = bind_echo_worker(
        &mut daemon,
        flooded.clone(),
        ClientId("duplex-overflow-flood-client".to_string()),
        flood_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    let sibling_adapter = bind_echo_worker(
        &mut daemon,
        sibling.clone(),
        ClientId("duplex-overflow-sibling-client".to_string()),
        sibling_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        20,
    );
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("duplex-overflow-probe".to_string()),
            session_id: flooded.clone(),
            now_seconds: 25,
        })
        .expect("probe flooded session");
    flood_adapter.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"hold\n",
    ));
    assert!(pump_next_available_wake(&mut daemon, 26));
    for _ in 0..5 {
        for _ in 0..64 {
            flood_adapter.inject_ingress_frame(compact_input_frame(b"flood\n"));
        }
        pump_next_available_wake(&mut daemon, 30);
        pump_next_available_wake(&mut daemon, 31);
    }
    assert_eq!(
        flood_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Closed
    );
    assert!(daemon
        .list_terminal_subscriptions()
        .iter()
        .all(|row| row.subscription_id != flood_sub));
    assert!(daemon
        .list_terminal_subscriptions()
        .iter()
        .any(|row| row.subscription_id == sibling_sub));
    let after = daemon.drain(&flooded, 31).expect("drain after overflow");
    assert!(
        after.client_egress.iter().all(|(target, frame)| {
            target.0 != "duplex-overflow-flood-client"
                || !matches!(frame, TransportEgress::TerminalOutput { .. })
        }),
        "removed overflow owner must not receive later client_egress"
    );
    sibling_adapter.inject_ingress_frame(compact_input_frame(b"SIB\n"));
    let started = Instant::now();
    let mut saw_sibling = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 32);
        if sibling_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:SIB"))
        {
            saw_sibling = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_sibling, "sibling session must keep applying input");
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
        .list_terminal_subscriptions()
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
        pump_next_available_wake(&mut daemon, 14);
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
        thread::sleep(Duration::from_millis(10));
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
        "sleep 30",
        10,
    );
    adapter.inject_ingress_frame(compact_input_frame(b"gone\n"));
    daemon
        .shutdown(Some(session_id.clone()), 12)
        .expect("teardown session");
    let _ = daemon.drain(&session_id, 13);
    assert!(daemon
        .list_terminal_subscriptions()
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
        "sleep 30",
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
        .list_terminal_subscriptions()
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
    wait_for_condition("failed worker exits", || process_has_exited(worker_pid));
    let started = Instant::now();
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        let _ = daemon.input(
            ClientId("duplex-writer-idle-client".to_string()),
            failed.clone(),
            vec![b'X'; 4_096],
            29,
        );
        pump_next_available_wake(&mut daemon, 30);
        let gone = daemon
            .list_terminal_subscriptions()
            .iter()
            .all(|row| row.subscription_id != idle_sub && row.subscription_id != active_sub);
        if gone {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        daemon
            .list_terminal_subscriptions()
            .iter()
            .all(|row| row.subscription_id != idle_sub && row.subscription_id != active_sub),
        "writer failure must sweep every same-session owner"
    );
    other_adapter.inject_ingress_frame(compact_input_frame(b"LIVE\n"));
    let started = Instant::now();
    let mut saw_other = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 31);
        if other_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:LIVE"))
        {
            saw_other = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_other, "a different session must survive writer failure");
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
        assert!(pump_next_available_wake(&mut daemon, 30));
        assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
        assert!(daemon
            .list_terminal_subscriptions()
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
            pump_next_available_wake(&mut daemon, 32);
            if sibling_adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .any(|bytes| adapter_payload_text(bytes).contains("echo:SIB"))
            {
                saw_sibling = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(saw_sibling, "{label} must leave the sibling session live");
        let _ = fs::remove_dir_all(data_dir);
    }
}

#[cfg(unix)]
#[test]
fn pump_woken_detach_cancels_held_gated_and_leaves_sibling() {
    let data_dir = temp_data_dir("duplex-detach-gated");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0)
            .with_test_mode_gated_hold_ms(Some(10_000)),
    );
    let held = SessionId("duplex-detach-gated-held".to_string());
    let sibling = SessionId("duplex-detach-gated-sib".to_string());
    let held_sub = SubscriptionId("duplex-detach-gated-held-sub".to_string());
    let sibling_sub = SubscriptionId("duplex-detach-gated-sib-sub".to_string());
    let held_client = ClientId("duplex-detach-gated-held-c".to_string());
    let held_adapter = bind_echo_worker(
        &mut daemon,
        held.clone(),
        held_client.clone(),
        held_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    let sibling_adapter = bind_echo_worker(
        &mut daemon,
        sibling.clone(),
        ClientId("duplex-detach-gated-sib-c".to_string()),
        sibling_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        20,
    );
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("duplex-detach-gated-probe".to_string()),
            session_id: held.clone(),
            now_seconds: 25,
        })
        .expect("probe held session");
    held_adapter.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"hold\n",
    ));
    assert!(pump_next_available_wake(&mut daemon, 26));
    daemon
        .detach(held_client, held.clone(), held_sub.clone(), 27)
        .expect("detach held owner");
    let _ = daemon.drain(&held, 28);
    assert_eq!(
        held_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Closed
    );
    assert!(daemon
        .list_terminal_subscriptions()
        .iter()
        .all(|row| row.subscription_id != held_sub));
    assert!(
        held_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .all(|bytes| adapter_input_result_subscription(bytes).is_none()),
        "detach must not synthesize a late input_result for the held request"
    );
    let replacement_client = ClientId("duplex-detach-gated-next-c".to_string());
    let replacement_sub = SubscriptionId("duplex-detach-gated-next-sub".to_string());
    let replacement = attach_bound_adapter(
        &mut daemon,
        held.clone(),
        replacement_client,
        replacement_sub.clone(),
        30,
    );
    // Replacement probe and echo must complete well before the uncancelled
    // hold (10s) and the parent timeout (5s). Sleep-past-hold is not a cancel
    // oracle.
    const CANCEL_RELEASE_BOUND: Duration = Duration::from_secs(2);
    let started_release = Instant::now();
    let replacement_probe = loop {
        pump_next_available_wake(&mut daemon, 29);
        match daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("duplex-detach-gated-next-probe".to_string()),
            session_id: held.clone(),
            now_seconds: 31,
        }) {
            Ok(probe) => break probe,
            Err(error)
                if error
                    .to_string()
                    .contains("mode-gated request already in flight") =>
            {
                assert!(
                    started_release.elapsed() < CANCEL_RELEASE_BOUND,
                    "cancel must release the held lane before the uncancelled hold: {error}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("probe replacement session: {error}"),
        }
    };
    replacement.inject_ingress_frame(compact_mode_gated_frame(
        replacement_probe.mode_flags.mode_freshness.mode_generation,
        replacement_probe.mode_flags.mode_freshness.mode_revision,
        b"next\n",
    ));
    let started_next = Instant::now();
    let mut saw_replacement = false;
    while started_next.elapsed() < CANCEL_RELEASE_BOUND {
        pump_next_available_wake(&mut daemon, 32);
        if replacement
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:next"))
        {
            saw_replacement = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        saw_replacement,
        "cancel must release the held lane so a replacement gated request completes before the uncancelled hold; replacement frames: {:?}",
        replacement
            .snapshot_delivered_frame_bytes()
            .iter()
            .map(|bytes| adapter_payload_text(bytes))
            .collect::<Vec<_>>()
    );
    assert!(
        held_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .all(|bytes| !adapter_payload_text(bytes).contains("echo:hold")),
        "cancelled hold must not write the abandoned gated payload"
    );
    sibling_adapter.inject_ingress_frame(compact_input_frame(b"SIB\n"));
    let started = Instant::now();
    let mut saw_sibling = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 29);
        if sibling_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| adapter_payload_text(bytes).contains("echo:SIB"))
        {
            saw_sibling = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_sibling, "detach must leave the sibling session live");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_other_session_leaves_queued_gated_on_a_held_sibling() {
    let data_dir = temp_data_dir("duplex-hold-cross-session");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0)
            .with_test_mode_gated_hold_ms(Some(10_000)),
    );
    let drained = SessionId("duplex-hold-cross-drained".to_string());
    let held = SessionId("duplex-hold-cross-held".to_string());
    let drained_sub = SubscriptionId("duplex-hold-cross-drained-sub".to_string());
    let holder_sub = SubscriptionId("duplex-hold-cross-holder-sub".to_string());
    let sibling_sub = SubscriptionId("duplex-hold-cross-sibling-sub".to_string());
    let _drained_adapter = bind_echo_worker(
        &mut daemon,
        drained.clone(),
        ClientId("duplex-hold-cross-drained-c".to_string()),
        drained_sub,
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    let holder = bind_echo_worker(
        &mut daemon,
        held.clone(),
        ClientId("duplex-hold-cross-holder-c".to_string()),
        holder_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        20,
    );
    let sibling = attach_bound_adapter(
        &mut daemon,
        held.clone(),
        ClientId("duplex-hold-cross-sibling-c".to_string()),
        sibling_sub.clone(),
        30,
    );
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("duplex-hold-cross-probe".to_string()),
            session_id: held.clone(),
            now_seconds: 31,
        })
        .expect("probe held session");
    holder.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"hold\n",
    ));
    assert!(pump_next_available_wake(&mut daemon, 32));
    sibling.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"queued\n",
    ));
    let _ = daemon
        .drain(&drained, 33)
        .expect("read back the other session");
    assert!(
        sibling
            .snapshot_delivered_frame_bytes()
            .iter()
            .all(|bytes| adapter_input_result_subscription(bytes).is_none()),
        "draining another session must not reject a queued ModeGatedInput on a held session: {:?}",
        sibling.snapshot_delivered_frame_bytes()
    );
    pump_next_available_wake(&mut daemon, 34);
    assert!(
        sibling
            .snapshot_delivered_frame_bytes()
            .iter()
            .all(|bytes| adapter_input_result_subscription(bytes).is_none()),
        "the held session must keep the sibling ModeGatedInput queued: {:?}",
        sibling.snapshot_delivered_frame_bytes()
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_one_batch_grants_one_gated_and_leaves_the_sibling_queued() {
    let data_dir = temp_data_dir("duplex-hold-same-tick");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("duplex-hold-same-tick".to_string());
    let first_sub = SubscriptionId("duplex-hold-same-tick-first-sub".to_string());
    let second_sub = SubscriptionId("duplex-hold-same-tick-second-sub".to_string());
    let first = bind_echo_worker(
        &mut daemon,
        session_id.clone(),
        ClientId("duplex-hold-same-tick-first-c".to_string()),
        first_sub.clone(),
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        10,
    );
    let second = attach_bound_adapter(
        &mut daemon,
        session_id.clone(),
        ClientId("duplex-hold-same-tick-second-c".to_string()),
        second_sub.clone(),
        20,
    );
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("duplex-hold-same-tick-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("probe shared session");
    first.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"first\n",
    ));
    second.inject_ingress_frame(compact_mode_gated_frame(
        probe.mode_flags.mode_freshness.mode_generation,
        probe.mode_flags.mode_freshness.mode_revision,
        b"second\n",
    ));
    assert!(pump_next_available_wake(&mut daemon, 22));
    let first_results: Vec<_> = first
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| adapter_input_result_subscription(bytes))
        .collect();
    let second_results: Vec<_> = second
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| adapter_input_result_subscription(bytes))
        .collect();
    assert!(
        first_results.is_empty() && second_results.is_empty(),
        "one drain tick must not reject the ungranted sibling: first={first_results:?} second={second_results:?} first_frames={:?} second_frames={:?}",
        first.snapshot_delivered_frame_bytes(),
        second.snapshot_delivered_frame_bytes()
    );
    let _ = fs::remove_dir_all(data_dir);
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
    request.request.arguments[1] = "printf 'hold-order-live\\n'; sleep 30".to_string();
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
        .list_terminal_subscriptions()
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
    wait_until_bound_attached(&mut daemon, &session_id, &adapter);
    let mut phases = Vec::new();
    for bytes in adapter.snapshot_delivered_frame_bytes() {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("snapshot") => {
                if let Some(phase) = value.get("phase").and_then(serde_json::Value::as_str) {
                    phases.push(phase.to_string());
                }
            }
            Some("attach_state")
                if value.get("state").and_then(serde_json::Value::as_str) == Some("attached") =>
            {
                phases.push("attached".to_string());
            }
            _ => {}
        }
    }
    let ready = phases.iter().position(|phase| phase == "ready");
    let finish = phases.iter().position(|phase| phase == "finish");
    let attached_at = phases.iter().position(|phase| phase == "attached");
    assert!(
        ready.is_some() && finish.is_some() && attached_at.is_some(),
        "declared bind must deliver READY, FINISH, and Attached: {phases:?}"
    );
    let ready = ready.expect("ready");
    let finish = finish.expect("finish");
    let attached_at = attached_at.expect("attached");
    assert!(ready < finish && finish < attached_at, "{phases:?}");
    assert!(
        phases[ready + 1..finish]
            .iter()
            .all(|phase| phase == "history"),
        "HISTORY pages must sit between READY and FINISH: {phases:?}"
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

    let started = Instant::now();
    let mut unsubscribe_count = 0;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 30);
        let drained = daemon.drain(&session_id, 30).expect("read overflow result");
        unsubscribe_count +=
            count_production_unsubscribe(&drained.observations, &holder, &session_id, &holder_sub);
        let holder_live = daemon
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.subscription_id == holder_sub);
        if !holder_live && unsubscribe_count > 0 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let extra = daemon.drain(&session_id, 30).expect("drain after overflow");
    unsubscribe_count +=
        count_production_unsubscribe(&extra.observations, &holder, &session_id, &holder_sub);
    assert!(
        daemon
            .list_terminal_subscriptions()
            .iter()
            .all(|row| row.subscription_id != holder_sub),
        "overflow must remove the holding owner"
    );
    assert_eq!(
        unsubscribe_count, 1,
        "overflow must run production UnsubscribeSession exactly once"
    );
    assert!(
        daemon
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.subscription_id == sibling_sub && row.adapter_bound),
        "sibling must remain bound"
    );
    let before = sibling_adapter.snapshot_delivered_frame_bytes().len();
    let sibling_started = Instant::now();
    let mut sibling_progress = false;
    while sibling_started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 31);
        if sibling_adapter.snapshot_delivered_frame_bytes().len() > before {
            sibling_progress = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        sibling_progress,
        "sibling must keep delivering after overflow"
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
        "printf sibling-live\\n; sleep 30",
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
        .list_terminal_subscriptions()
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
        pump_next_available_wake(&mut daemon, 40);
        let drained = daemon
            .drain(&session_id, 40)
            .expect("read closed-bind result");
        unsubscribe_count +=
            count_production_unsubscribe(&drained.observations, &holder, &session_id, &holder_sub);
        if !daemon
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.subscription_id == holder_sub)
            && unsubscribe_count > 0
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let extra = daemon
        .drain(&session_id, 40)
        .expect("drain after closed bind");
    unsubscribe_count +=
        count_production_unsubscribe(&extra.observations, &holder, &session_id, &holder_sub);
    assert!(
        daemon
            .list_terminal_subscriptions()
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
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.subscription_id == sibling_sub && row.adapter_bound),
        "sibling must remain bound"
    );
    let before = sibling_adapter.snapshot_delivered_frame_bytes().len();
    sibling_adapter.inject_ingress_frame(compact_input_frame(b"SIB\n"));
    let sibling_started = Instant::now();
    let mut sibling_progress = false;
    while sibling_started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        pump_next_available_wake(&mut daemon, 41);
        if sibling_adapter.snapshot_delivered_frame_bytes().len() > before {
            sibling_progress = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        sibling_progress,
        "sibling must keep delivering after closed-adapter teardown"
    );
    assert!(
        daemon
            .list_terminal_subscriptions()
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
    request.request.arguments[1] = "printf 'foreign-live\\n'; sleep 30".to_string();
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

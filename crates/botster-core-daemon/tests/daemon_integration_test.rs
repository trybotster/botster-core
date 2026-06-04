#![allow(missing_docs)]

use std::fs;
use std::process::Command;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use botster_core::{
    ClientId, CoreSessionMetadata, ModeFlags, RequestId, ResizePayload, SessionId,
    SessionSpawnRequest, SessionWorkerHealthReason, SessionWorkerStaleReason, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TransportEgress,
};
use botster_core_daemon::{
    CoreDaemon, CoreDaemonConfig, GuardedWriteDecision, GuardedWriteDeliveryState,
    GuardedWriteRequest, ReadinessEvidence, RegistrySessionState, SafeWriteIndicator,
    SessionAdoptionState, SpawnSessionRequest,
};

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
            "worker-backed daemon should persist a reconnectable worker endpoint"
        );
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
    record.protocol_version = botster_core::PROTOCOL_VERSION.saturating_add(1);
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
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-daemon-{label}-{nanos}"))
}

fn worker_path() -> std::path::PathBuf {
    static BUILD_WORKER: Once = Once::new();
    BUILD_WORKER.call_once(|| {
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "botster-core",
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

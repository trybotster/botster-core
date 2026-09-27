//! Every pending daemon operation progresses through daemon wakes alone.
//!
//! These tests pump only when `wait_wakes` returns a real wake. An operation
//! whose progress source raises no wake stalls until the deadline and fails,
//! so a host needs no periodic pump.
#![cfg(unix)]

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::{
    CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory,
};
use botster_core_daemon::{
    CaptureOwner, CaptureSnapshotRequest, CoreCompletion, CoreDaemon, CoreDaemonConfig,
    CoreOperation, PendingOperationId, ReadModeFlagsRequest, ReadScreenRequest,
    SpawnSessionRequest,
};

fn worker_path() -> std::path::PathBuf {
    botster_core_test_support::real_worker::WorkerBinary::from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "botster-core-progress-wake-{label}-{}-{nanos}",
        std::process::id()
    ))
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

/// Pump only on real wakes until operation `id` completes.
fn complete_by_wakes(daemon: &mut CoreDaemon, id: PendingOperationId) -> CoreCompletion {
    // timer: deadline — the operation must progress through daemon wakes alone; expiry fails the test
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut now_seconds = 100;
    loop {
        if let Some(completion) = daemon
            .take_completions()
            .into_iter()
            .find(|completion| completion.id() == id)
        {
            return completion;
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or_else(|| panic!("operation {id:?} stalled: no wake carried its progress"));
        let batch = daemon.wait_wakes(remaining);
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            continue;
        }
        now_seconds += 1;
        // A wake that carries progress must pump cleanly: a host stops its
        // turn on a pump error.
        daemon
            .pump_woken(&batch, now_seconds)
            .unwrap_or_else(|error| panic!("pump for {id:?} failed: {error}"));
    }
}

fn worker_daemon(label: &str) -> (CoreDaemon, std::path::PathBuf) {
    let data_dir = temp_data_dir(label);
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    (daemon, data_dir)
}

/// Spawn one worker session through `begin`, completed by wakes alone.
fn spawned_session(daemon: &mut CoreDaemon, label: &str) -> SessionId {
    let session_id = SessionId(format!("{label}-session"));
    let id = daemon
        .begin(CoreOperation::Spawn(spawn_request(&session_id)))
        .expect("begin spawn");
    let completion = complete_by_wakes(daemon, id);
    assert!(
        matches!(completion, CoreCompletion::Spawn { result: Ok(_), .. }),
        "spawn: {completion:?}"
    );
    session_id
}

#[test]
fn spawn_completes_through_wakes_alone() {
    let (mut daemon, data_dir) = worker_daemon("spawn");
    spawned_session(&mut daemon, "spawn");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn read_screen_completes_through_wakes_alone() {
    let (mut daemon, data_dir) = worker_daemon("read-screen");
    let session_id = spawned_session(&mut daemon, "read-screen");
    let id = daemon
        .begin(CoreOperation::ReadScreen(ReadScreenRequest {
            request_id: RequestId("read-screen".into()),
            session_id,
            now_seconds: 200,
        }))
        .expect("begin read screen");
    let completion = complete_by_wakes(&mut daemon, id);
    assert!(
        matches!(completion, CoreCompletion::ReadScreen { result: Ok(_), .. }),
        "read screen: {completion:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn read_mode_flags_completes_through_wakes_alone() {
    let (mut daemon, data_dir) = worker_daemon("read-modes");
    let session_id = spawned_session(&mut daemon, "read-modes");
    let id = daemon
        .begin(CoreOperation::ReadModeFlags(ReadModeFlagsRequest {
            request_id: RequestId("read-modes".into()),
            session_id,
            now_seconds: 200,
        }))
        .expect("begin read mode flags");
    let completion = complete_by_wakes(&mut daemon, id);
    assert!(
        matches!(
            completion,
            CoreCompletion::ReadModeFlags { result: Ok(_), .. }
        ),
        "read mode flags: {completion:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn capture_snapshot_completes_through_wakes_alone() {
    let (mut daemon, data_dir) = worker_daemon("capture");
    let session_id = spawned_session(&mut daemon, "capture");
    let id = daemon
        .begin(CoreOperation::CaptureSnapshot {
            request: CaptureSnapshotRequest {
                request_id: RequestId("capture".into()),
                session_id,
                now_seconds: 200,
            },
            owner: CaptureOwner("capture-owner".into()),
        })
        .expect("begin capture");
    let completion = complete_by_wakes(&mut daemon, id);
    assert!(
        matches!(
            completion,
            CoreCompletion::CaptureSnapshot { result: Ok(_), .. }
        ),
        "capture: {completion:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn shutdown_completes_through_wakes_alone() {
    let (mut daemon, data_dir) = worker_daemon("shutdown");
    let session_id = spawned_session(&mut daemon, "shutdown");
    let id = daemon
        .begin(CoreOperation::ShutdownSession(session_id))
        .expect("begin shutdown");
    let completion = complete_by_wakes(&mut daemon, id);
    assert!(
        matches!(
            completion,
            CoreCompletion::ShutdownSession { result: Ok(_), .. }
        ),
        "shutdown: {completion:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

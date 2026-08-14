use std::process::Command;
use std::sync::Once;

use super::{ClientId, SessionId, SubscriptionId, WorkerBackedBotsterEngine};
use crate::runtime::SessionSpawnRequest;
use crate::session::CoreSessionMetadata;
use crate::{RequestId, ResizePayload, SpawnEnvironment, SpawnWorkingDirectory};

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
            "worker binary should build for takeover tests"
        );
    });
    let mut path = std::env::current_exe().expect("test executable path should resolve");
    while path.file_name().and_then(|name| name.to_str()) != Some("debug")
        && path.file_name().and_then(|name| name.to_str()) != Some("release")
    {
        assert!(
            path.pop(),
            "test executable should live under target/debug or target/release"
        );
    }
    path.join("botster-session-worker")
}

fn spawn_request(session_id: &SessionId) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: RequestId(format!("{}-spawn", session_id.0)),
        session_id: session_id.clone(),
        executable: "sh".to_string(),
        arguments: vec![
            "-c".to_string(),
            "printf ready; while IFS= read -r line; do :; done".to_string(),
        ],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

fn live_clients(engine: &WorkerBackedBotsterEngine) -> Vec<ClientId> {
    engine
        .list_terminal_subscriptions()
        .into_iter()
        .map(|row| row.client_id)
        .collect()
}

#[test]
fn cancel_failure_does_not_publish_the_new_owner() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("takeover-cancel-fail".to_string());
    let first = ClientId("takeover-cancel-a".to_string());
    let second = ClientId("takeover-cancel-b".to_string());
    let subscription = SubscriptionId("takeover-cancel-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), subscription.clone(), 11)
        .expect("attach first");
    engine.session_runtime_mut().fail_next_snapshot_cancel();
    let error = engine
        .attach_client(second.clone(), session_id.clone(), subscription.clone(), 12)
        .expect_err("cancel failure must fail takeover");
    assert!(
        error
            .to_string()
            .contains("injected snapshot cancel failure"),
        "unexpected error: {error}"
    );
    let live = live_clients(&engine);
    assert_eq!(live, vec![first]);
}

#[test]
fn begin_failure_does_not_publish_the_new_owner() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("takeover-begin-fail".to_string());
    let first = ClientId("takeover-begin-a".to_string());
    let second = ClientId("takeover-begin-b".to_string());
    let subscription = SubscriptionId("takeover-begin-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), subscription.clone(), 11)
        .expect("attach first");
    engine.session_runtime_mut().fail_next_snapshot_begins(1);
    let error = engine
        .attach_client(second.clone(), session_id.clone(), subscription.clone(), 12)
        .expect_err("begin failure must fail takeover");
    assert!(
        error
            .to_string()
            .contains("injected snapshot begin failure"),
        "unexpected error: {error}"
    );
    let live = live_clients(&engine);
    assert!(!live.contains(&second), "new owner published: {live:?}");
    assert!(
        !live.contains(&first),
        "cancelled owner stayed published: {live:?}"
    );
}

#[test]
fn two_begin_failures_detach_the_pending_sibling() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("takeover-two-begin-fail".to_string());
    let first = ClientId("takeover-two-begin-a".to_string());
    let second = ClientId("takeover-two-begin-b".to_string());
    let sibling = ClientId("takeover-two-begin-c".to_string());
    let first_sub = SubscriptionId("takeover-two-begin-x".to_string());
    let sibling_sub = SubscriptionId("takeover-two-begin-c-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first");
    engine
        .attach_client(sibling.clone(), session_id.clone(), sibling_sub.clone(), 12)
        .expect("queue sibling");
    assert!(live_clients(&engine).contains(&sibling));
    engine.session_runtime_mut().fail_next_snapshot_begins(2);
    let error = engine
        .attach_client(second.clone(), session_id.clone(), first_sub, 13)
        .expect_err("two begin failures must fail takeover");
    assert!(
        error
            .to_string()
            .contains("injected snapshot begin failure"),
        "unexpected error: {error}"
    );
    let live = engine.list_terminal_subscriptions();
    assert!(
        live.iter().all(|row| row.client_id != second),
        "new owner published: {live:?}"
    );
    assert!(
        live.iter().all(|row| row.client_id != first),
        "cancelled owner stayed published: {live:?}"
    );
    assert!(
        live.iter().all(|row| row.client_id != sibling),
        "pending sibling stayed published without a tracked boundary: {live:?}"
    );
    assert!(
        live.iter().all(|row| row.subscription_id != sibling_sub),
        "pending sibling route remained: {live:?}"
    );
}

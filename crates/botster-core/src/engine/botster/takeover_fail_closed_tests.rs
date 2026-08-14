use std::process::Command;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};

use super::{ClientId, SessionId, SubscriptionId, WorkerBackedBotsterEngine};
use crate::contract::transport::TransportEgress;
use crate::runtime::{SessionRuntime, SessionSpawnRequest};
use crate::session::CoreSessionMetadata;
use crate::{
    QueueSource, RequestId, ResizePayload, SessionRuntimeInput, SessionRuntimeOutput,
    SpawnEnvironment, SpawnWorkingDirectory, TerminalAttachState, TerminalScreenSize,
    WorkerProcessRuntimeOptions,
};

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
            "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
                .to_string(),
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
fn initial_begin_failure_restores_detached_overflow() {
    let mut options = WorkerProcessRuntimeOptions::new(worker_path());
    options.egress_capacity = 1;
    let mut engine = WorkerBackedBotsterEngine::with_options(options);
    let session_id = SessionId("initial-begin-fail".to_string());
    let client = ClientId("initial-begin-fail-client".to_string());
    let subscription = SubscriptionId("initial-begin-fail-sub".to_string());
    engine
        .spawn_session(
            SessionSpawnRequest {
                request_id: RequestId(format!("{}-spawn", session_id.0)),
                session_id: session_id.clone(),
                executable: "sh".to_string(),
                arguments: vec![
                    "-c".to_string(),
                    "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string(),
                ],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_string(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine.session_runtime_mut().fail_next_snapshot_begins(1);
    let error = engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect_err("initial begin failure must fail attach");
    assert!(
        error
            .to_string()
            .contains("injected snapshot begin failure"),
        "unexpected error: {error}"
    );
    let live = engine.list_terminal_subscriptions();
    assert!(
        live.is_empty(),
        "failed pre-boundary attach must leave empty inventory: {live:?}"
    );

    engine
        .session_runtime_mut()
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session_id.clone(),
            data: b"FILL-SLOT\n".to_vec(),
        })
        .expect("fill the one-slot parent channel");
    thread::sleep(Duration::from_millis(80));
    engine
        .session_runtime_mut()
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session_id.clone(),
            data: b"OVERFLOW-MARKER\n".to_vec(),
        })
        .expect("second write under capacity one");
    thread::sleep(Duration::from_millis(80));

    let health = engine
        .session_runtime_mut()
        .ping(&session_id)
        .expect("worker must progress without a parent drain");
    assert_eq!(health.session_id, session_id);

    let detached = engine
        .session_runtime_mut()
        .drain_output(&session_id)
        .expect("detached drain after failed attach");
    assert!(
        detached.iter().any(|event| matches!(
            event,
            SessionRuntimeOutput::Backpressure(summary)
                if summary.source == QueueSource::SessionIo
        )),
        "failed initial attach must restore typed detached overflow; drained={detached:?}"
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

fn drain_until_attached(
    engine: &mut WorkerBackedBotsterEngine,
    session_id: &SessionId,
    client_id: &ClientId,
) -> Vec<(ClientId, TransportEgress)> {
    let started = Instant::now();
    let mut frames = Vec::new();
    let mut tick = 20;
    while started.elapsed() < Duration::from_secs(8) {
        let output = engine.drain_runtime_once(session_id, tick).expect("drain");
        tick += 1;
        let attached = output.client_egress.iter().any(|(target, frame)| {
            target == client_id
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        state: TerminalAttachState::Attached,
                        ..
                    }
                )
        });
        frames.extend(output.client_egress);
        if attached {
            return frames;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("client {client_id:?} did not reach Attached");
}

fn output_text(frames: &[(ClientId, TransportEgress)]) -> String {
    let mut text = String::new();
    for (_, frame) in frames {
        if let TransportEgress::TerminalOutput { data, .. } = frame {
            text.push_str(&String::from_utf8_lossy(data));
        }
    }
    text
}

#[test]
fn failed_pending_owner_queues_do_not_follow_a_fresh_reattach() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("takeover-stale-queue".to_string());
    let first = ClientId("takeover-stale-a".to_string());
    let second = ClientId("takeover-stale-b".to_string());
    let failed = ClientId("takeover-stale-c".to_string());
    let recovered = ClientId("takeover-stale-d".to_string());
    let first_sub = SubscriptionId("takeover-stale-x".to_string());
    let failed_sub = SubscriptionId("takeover-stale-c-old".to_string());
    let recovered_sub = SubscriptionId("takeover-stale-d-sub".to_string());
    let fresh_sub = SubscriptionId("takeover-stale-c-new".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first");
    engine
        .attach_client(failed.clone(), session_id.clone(), failed_sub, 12)
        .expect("queue failed sibling");
    engine
        .write_bytes(
            failed.clone(),
            session_id.clone(),
            b"STALE-C\n".to_vec(),
            13,
        )
        .expect("queue stale input");
    engine
        .resize(failed.clone(), session_id.clone(), 30, 100, 14)
        .expect("queue stale resize");
    engine
        .attach_client(
            recovered.clone(),
            session_id.clone(),
            recovered_sub.clone(),
            15,
        )
        .expect("queue recovery sibling");
    engine.session_runtime_mut().fail_next_snapshot_begins(2);
    engine
        .attach_client(second, session_id.clone(), first_sub, 16)
        .expect_err("takeover begin failures");
    let live = engine.list_terminal_subscriptions();
    assert!(live
        .iter()
        .any(|row| row.client_id == recovered && row.subscription_id == recovered_sub));
    assert!(live.iter().all(|row| row.client_id != failed));
    engine
        .attach_client(failed.clone(), session_id.clone(), fresh_sub, 17)
        .expect("fresh reattach while recovered boundary is still active");
    let mut frames = drain_until_attached(&mut engine, &session_id, &recovered);
    let (screen, _, _) = engine
        .capture_terminal_state(&session_id)
        .expect("screen after recovery");
    assert_eq!(
        screen.size,
        TerminalScreenSize { rows: 24, cols: 80 },
        "failed sibling resize must not apply to the recovered owner"
    );
    frames.extend(drain_until_attached(&mut engine, &session_id, &failed));
    engine
        .write_bytes(failed, session_id.clone(), b"FRESH-C\n".to_vec(), 18)
        .expect("fresh input");
    let started = Instant::now();
    let mut tick = 40;
    while started.elapsed() < Duration::from_secs(5) {
        let output = engine
            .drain_runtime_once(&session_id, tick)
            .expect("drain live");
        tick += 1;
        frames.extend(output.client_egress);
        if output_text(&frames).contains("echo:FRESH-C") {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let text = output_text(&frames);
    assert!(
        !text.contains("echo:STALE-C"),
        "stale failed-owner input reached the PTY: {text:?}"
    );
    assert!(
        text.contains("echo:FRESH-C"),
        "fresh reattach input never reached the PTY: {text:?}"
    );
}

#[test]
fn finish_promotion_begin_failure_detaches_the_pending_owner() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("finish-promote-fail".to_string());
    let first = ClientId("finish-promote-a".to_string());
    let pending = ClientId("finish-promote-c".to_string());
    let first_sub = SubscriptionId("finish-promote-x".to_string());
    let pending_sub = SubscriptionId("finish-promote-c-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), first_sub, 11)
        .expect("attach first");
    engine
        .attach_client(pending.clone(), session_id.clone(), pending_sub.clone(), 12)
        .expect("queue pending");
    engine
        .write_bytes(
            pending.clone(),
            session_id.clone(),
            b"STALE-C\n".to_vec(),
            13,
        )
        .expect("queue stale input");
    engine.session_runtime_mut().fail_next_snapshot_begins(1);
    let mut frames = drain_until_attached(&mut engine, &session_id, &first);
    for tick in 30..40 {
        let output = engine
            .drain_runtime_once(&session_id, tick)
            .expect("drain after finish");
        frames.extend(output.client_egress);
    }
    let live = engine.list_terminal_subscriptions();
    assert!(
        live.iter().all(|row| row.client_id != pending),
        "failed FINISH promotion left the pending owner published: {live:?}"
    );
    assert!(
        live.iter().all(|row| row.subscription_id != pending_sub),
        "failed FINISH promotion left the pending route: {live:?}"
    );
    assert!(
        !output_text(&frames).contains("echo:STALE-C"),
        "pending input bypassed the attach barrier: {:?}",
        output_text(&frames)
    );
}

#[test]
fn detach_promotion_begin_failure_detaches_the_pending_owner() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("detach-promote-fail".to_string());
    let first = ClientId("detach-promote-a".to_string());
    let pending = ClientId("detach-promote-c".to_string());
    let first_sub = SubscriptionId("detach-promote-x".to_string());
    let pending_sub = SubscriptionId("detach-promote-c-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first");
    engine
        .attach_client(pending.clone(), session_id.clone(), pending_sub.clone(), 12)
        .expect("queue pending");
    engine
        .write_bytes(
            pending.clone(),
            session_id.clone(),
            b"STALE-C\n".to_vec(),
            13,
        )
        .expect("queue stale input");
    engine.session_runtime_mut().fail_next_snapshot_begins(1);
    engine
        .detach_client(first, session_id.clone(), first_sub, 14)
        .expect("detach current owner");
    let live = engine.list_terminal_subscriptions();
    assert!(
        live.iter().all(|row| row.client_id != pending),
        "failed detach promotion left the pending owner published: {live:?}"
    );
    assert!(
        live.iter().all(|row| row.subscription_id != pending_sub),
        "failed detach promotion left the pending route: {live:?}"
    );
    let output = engine
        .drain_runtime_once(&session_id, 20)
        .expect("drain after detach");
    assert!(
        !output_text(&output.client_egress).contains("echo:STALE-C"),
        "pending input bypassed the attach barrier after detach"
    );
}

fn setup_active_stale_and_pending(
    engine: &mut WorkerBackedBotsterEngine,
    label: &str,
) -> (
    SessionId,
    ClientId,
    ClientId,
    SubscriptionId,
    SubscriptionId,
) {
    let session_id = SessionId(format!("{label}-session"));
    let first = ClientId(format!("{label}-a"));
    let pending = ClientId(format!("{label}-b"));
    let first_sub = SubscriptionId(format!("{label}-sub-a"));
    let pending_sub = SubscriptionId(format!("{label}-sub-b"));
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first");
    engine
        .write_bytes(first.clone(), session_id.clone(), b"STALE-A\n".to_vec(), 12)
        .expect("queue stale input");
    engine
        .resize(first.clone(), session_id.clone(), 30, 100, 14)
        .expect("queue stale resize");
    engine
        .attach_client(pending.clone(), session_id.clone(), pending_sub.clone(), 15)
        .expect("queue pending sibling");
    (session_id, first, pending, first_sub, pending_sub)
}

fn assert_promoted_sibling_did_not_inherit_stale(
    engine: &mut WorkerBackedBotsterEngine,
    session_id: &SessionId,
    pending: &ClientId,
    pending_sub: &SubscriptionId,
) {
    let frames = drain_until_attached(engine, session_id, pending);
    assert!(
        engine.take_applied_attach_resize(session_id).is_none(),
        "removed owner resize applied to the promoted sibling"
    );
    let (screen, _, _) = engine
        .capture_terminal_state(session_id)
        .expect("screen after promotion");
    assert_eq!(
        screen.size,
        TerminalScreenSize { rows: 24, cols: 80 },
        "removed owner resize changed the promoted sibling screen"
    );
    assert!(
        !output_text(&frames).contains("echo:STALE-A"),
        "removed owner input reached the PTY: {:?}",
        output_text(&frames)
    );
    let live = engine.list_terminal_subscriptions();
    assert!(live
        .iter()
        .any(|row| &row.client_id == pending && &row.subscription_id == pending_sub));
}

#[test]
fn generation_detach_discards_removed_owner_queues() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let (session_id, first, pending, first_sub, pending_sub) =
        setup_active_stale_and_pending(&mut engine, "gen-detach");
    let generation = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.client_id == first && row.subscription_id == first_sub)
        .expect("first inventory")
        .generation;
    engine
        .detach_terminal_subscription(first, session_id.clone(), first_sub, generation, 16)
        .expect("generation detach");
    assert_promoted_sibling_did_not_inherit_stale(&mut engine, &session_id, &pending, &pending_sub);
}

#[test]
fn pre_ready_failure_discards_removed_owner_queues() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let (session_id, _, pending, _, pending_sub) =
        setup_active_stale_and_pending(&mut engine, "pre-ready");
    engine.session_runtime_mut().fail_next_pre_ready_snapshot();
    let drain_error = engine
        .drain_runtime_once(&session_id, 16)
        .expect_err("pre-ready failure");
    assert!(
        drain_error
            .to_string()
            .contains("injected pre-ready failure"),
        "unexpected drain error: {drain_error}"
    );
    assert_promoted_sibling_did_not_inherit_stale(&mut engine, &session_id, &pending, &pending_sub);
}

#[test]
fn teardown_reconcile_discards_removed_owner_queues() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let (session_id, first, pending, first_sub, pending_sub) =
        setup_active_stale_and_pending(&mut engine, "reconcile");
    engine
        .runtime
        .detach_live_subscription(first, session_id.clone(), first_sub, 16)
        .expect("inventory teardown without IncrementalAttach sweep");
    engine
        .session_runtime_mut()
        .cancel_outstanding_snapshot(&session_id)
        .expect("stop the removed owner encode before reconcile");
    engine
        .drain_runtime_once(&session_id, 17)
        .expect("reconcile after inventory teardown");
    assert_promoted_sibling_did_not_inherit_stale(&mut engine, &session_id, &pending, &pending_sub);
}

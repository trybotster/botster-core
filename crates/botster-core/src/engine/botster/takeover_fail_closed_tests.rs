use std::collections::VecDeque;
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;

use super::{ClientId, SessionId, SubscriptionId, WorkerBackedBotsterEngine};
use crate::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeKind, TerminalWakeSink, WakingTerminalAdapter,
};
use crate::contract::transport::TransportEgress;
use crate::runtime::{
    ControlFrameClass, ControlQueue, SessionRuntime, SessionSpawnRequest,
    WORKER_CONTROL_QUEUE_FRAMES, WORKER_CONTROL_RESERVED_SLOTS,
};
use crate::session::CoreSessionMetadata;
use crate::{
    QueueSource, RequestId, ResizePayload, SessionIoEvent, SessionRuntimeInput,
    SessionRuntimeOutput, SpawnEnvironment, SpawnWorkingDirectory, TerminalAttachState,
    TerminalCapabilitySet, TerminalScreenSize, WorkerProcessRuntimeOptions, FRAME_PTY_INPUT,
    FRAME_RESIZE,
};
use botster_terminal_protocol::TerminalFrame;

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

#[derive(Clone)]
struct InjectAdapter {
    ingress: Arc<Mutex<VecDeque<Vec<u8>>>>,
    closed: Arc<Mutex<bool>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    wake_sink: Arc<Mutex<Option<TerminalWakeSink>>>,
}

impl InjectAdapter {
    fn new() -> Self {
        Self {
            ingress: Arc::new(Mutex::new(VecDeque::new())),
            closed: Arc::new(Mutex::new(false)),
            writes: Arc::new(Mutex::new(Vec::new())),
            wake_sink: Arc::new(Mutex::new(None)),
        }
    }

    fn inject(&self, bytes: Vec<u8>) {
        self.ingress
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push_back(bytes);
        if let Some(sink) = self
            .wake_sink
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
        {
            let _ = sink.wake(TerminalWakeKind::Writable);
        }
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

impl TerminalAdapter for InjectAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        if *self
            .closed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
        {
            return Err(TerminalAdapterWriteError::Closed);
        }
        self.writes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(frame.to_bytes().expect("terminal frame bytes"));
        Ok(())
    }

    fn close(&mut self) {
        *self
            .closed
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = true;
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        if *self
            .closed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
        {
            TerminalAdapterPressure::Closed
        } else {
            TerminalAdapterPressure::Ready
        }
    }

    fn try_read(&mut self) -> TerminalIngress {
        if *self
            .closed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
        {
            return TerminalIngress::Closed;
        }
        match self
            .ingress
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop_front()
        {
            Some(bytes) => TerminalIngress::Frame(bytes),
            None => TerminalIngress::Empty,
        }
    }
}

impl WakingTerminalAdapter for InjectAdapter {
    fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        *self
            .wake_sink
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(sink);
    }
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

fn compact_input_frame(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).expect("input fits u16");
    let mut bytes = vec![1, 1];
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn compact_resize_frame(rows: u16, cols: u16) -> Vec<u8> {
    let mut bytes = vec![1, 3, 0, 4];
    bytes.extend_from_slice(&rows.to_be_bytes());
    bytes.extend_from_slice(&cols.to_be_bytes());
    bytes
}

fn input_result_count(adapter: &InjectAdapter, kind: &str) -> usize {
    adapter
        .writes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("input_result")
                && value.get("kind").and_then(|field| field.as_str()) == Some(kind)
        })
        .count()
}

fn terminal_output_bytes(frames: &[serde_json::Value]) -> Vec<u8> {
    frames
        .iter()
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("terminal_output")
        })
        .filter_map(|value| value.get("payload_base64").and_then(|field| field.as_str()))
        .flat_map(|payload| {
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .expect("terminal output base64")
        })
        .collect()
}

fn settle_wakes(engine: &mut WorkerBackedBotsterEngine, tick: u64) {
    for _ in 0..4 {
        let batch = engine.wait_wakes(Duration::from_millis(50));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            break;
        }
        engine.pump_woken(&batch, tick).expect("settle prior wakes");
    }
}

fn held_resize_engine(
    label: &str,
) -> (
    WorkerBackedBotsterEngine,
    SessionId,
    SubscriptionId,
    InjectAdapter,
    ControlQueue,
) {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId(format!("{label}-session"));
    let client = ClientId(format!("{label}-client"));
    let subscription = SubscriptionId(format!("{label}-sub"));
    let mut request = spawn_request(&session_id);
    request.arguments[1] = "stty -echo; printf ready; while IFS= read -r line; do printf 'echo:%s\n' \"$line\"; if [ \"$line\" = MIXED-ADMISSION-RACE ]; then stty size; exit 0; fi; done".to_string();
    engine
        .spawn_session(request, CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("generation");
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client,
            session_id.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    settle_wakes(&mut engine, 19);
    let queue = engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("control queue");
    queue.hold_pops(true);
    assert!(
        queue.held_frames().is_empty(),
        "settled queue must be empty"
    );
    (engine, session_id, subscription, adapter, queue)
}

fn pump_adapter_wake(
    engine: &mut WorkerBackedBotsterEngine,
    adapter: &InjectAdapter,
    frame: Vec<u8>,
    tick: u64,
) {
    adapter.inject(frame);
    let batch = engine.wait_wakes(Duration::from_secs(1));
    assert!(!batch.adapter_routes.is_empty(), "adapter wake must arrive");
    engine.pump_woken(&batch, tick).expect("targeted pump");
}

fn held_resize_payloads(queue: &ControlQueue) -> Vec<ResizePayload> {
    queue
        .held_frames()
        .into_iter()
        .filter(|(frame_type, _)| *frame_type == FRAME_RESIZE)
        .map(|(_, payload)| serde_json::from_slice(&payload).expect("resize payload"))
        .collect()
}

#[test]
fn pump_woken_admits_one_worker_resize_frame_for_one_request() {
    let (mut engine, _, _, adapter, queue) = held_resize_engine("woken-resize-once");
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(31, 101), 20);

    assert_eq!(
        held_resize_payloads(&queue),
        vec![ResizePayload {
            rows: 31,
            cols: 101
        }]
    );
    assert_eq!(input_result_count(&adapter, "resize"), 1);
    queue.hold_pops(false);
}

#[test]
fn pump_woken_preserves_mixed_resize_and_input_across_a_queue_admission_race() {
    let (mut engine, session_id, subscription, adapter, queue) =
        held_resize_engine("woken-mixed-admission-race");
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("live generation");
    assert!(engine.list_terminal_subscriptions().iter().any(|row| {
        row.session_id == session_id
            && row.subscription_id == subscription
            && row.generation == generation
    }));
    for _ in 0..(WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS - 1) {
        engine
            .session_runtime_mut()
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: Vec::new(),
            })
            .expect("leave one ordinary slot");
    }

    adapter.inject(compact_resize_frame(31, 101));
    adapter.inject(compact_input_frame(b"MIXED-ADMISSION-RACE\n"));
    let mixed_batch = engine.wait_wakes(Duration::from_secs(1));
    let (probe_reached, release_probe) = queue.pause_after_next_ready_probe();
    let competing_queue = queue.clone();
    let competing_admission = thread::spawn(move || {
        probe_reached
            .recv_timeout(Duration::from_secs(1))
            .expect("targeted apply must reach a ready queue probe");
        competing_queue
            .admit(
                ControlFrameClass::Ordinary,
                crate::encode_frame(FRAME_PTY_INPUT, &[]).expect("encode competing PTY input"),
            )
            .expect("a concurrent producer must use the final ordinary slot");
        release_probe.send(()).expect("release targeted apply");
    });
    engine
        .pump_woken(&mixed_batch, 20)
        .expect("queue pressure must not fail the public pump");
    competing_admission
        .join()
        .expect("join competing admission");

    queue.hold_pops(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut exit_code = None;
    while exit_code.is_none() {
        assert!(
            Instant::now() < deadline,
            "live worker did not exit after the raced mixed batch"
        );
        let batch = engine.wait_wakes(Duration::from_millis(100));
        let output = engine
            .pump_woken(&batch, 21)
            .expect("drain the raced mixed batch");
        exit_code = output.session_events.iter().find_map(|event| match event {
            SessionIoEvent::ProcessExited {
                session_id: exited,
                payload,
            } if exited == &session_id => payload.exit_code,
            _ => None,
        });
    }

    assert_eq!(exit_code, Some(0));
    assert_eq!(input_result_count(&adapter, "resize"), 1);
    assert_eq!(input_result_count(&adapter, "input"), 1);
    let frames: Vec<serde_json::Value> = adapter
        .writes()
        .iter()
        .map(|bytes| serde_json::from_slice(bytes).expect("adapter frame JSON"))
        .collect();
    let process_exit_index = frames
        .iter()
        .position(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("process_exit")
                && value.get("code").and_then(|field| field.as_i64()) == Some(0)
        })
        .expect("adapter code-zero process exit");
    let before_exit = &frames[..process_exit_index];
    assert!(before_exit
        .iter()
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("input_result")
        })
        .all(|value| {
            value
                .get("subscription_id")
                .and_then(|field| field.as_str())
                == Some(subscription.0.as_str())
        }));
    assert!(before_exit
        .iter()
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("terminal_output")
        })
        .all(|value| {
            value
                .get("subscription_id")
                .and_then(|field| field.as_str())
                == Some(subscription.0.as_str())
        }));
    let terminal_output = terminal_output_bytes(before_exit);
    assert!(
        terminal_output
            .windows(b"echo:MIXED-ADMISSION-RACE\r\n".len())
            .any(|window| window == b"echo:MIXED-ADMISSION-RACE\r\n"),
        "raced input must reach the live PTY: {terminal_output:?}"
    );
    assert!(
        terminal_output
            .windows(b"31 101\r\n".len())
            .any(|window| window == b"31 101\r\n"),
        "live worker PTY must report the raced 31x101 resize: {terminal_output:?}"
    );
    assert!(frames
        .iter()
        .all(|value| !value.to_string().contains("core_adapter_closed")));
    assert_eq!(adapter.pressure(), TerminalAdapterPressure::Closed);
}

#[test]
fn pump_woken_admits_one_worker_resize_frame_per_identical_request() {
    let (mut engine, _, _, adapter, queue) = held_resize_engine("woken-resize-identical");
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(31, 101), 20);
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(31, 101), 21);

    assert_eq!(
        held_resize_payloads(&queue),
        vec![
            ResizePayload {
                rows: 31,
                cols: 101
            },
            ResizePayload {
                rows: 31,
                cols: 101
            },
        ]
    );
    assert_eq!(input_result_count(&adapter, "resize"), 2);
    queue.hold_pops(false);
}

#[test]
fn pump_woken_preserves_worker_resize_frame_order() {
    let (mut engine, _, _, adapter, queue) = held_resize_engine("woken-resize-order");
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(31, 101), 20);
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(32, 102), 21);

    assert_eq!(
        held_resize_payloads(&queue),
        vec![
            ResizePayload {
                rows: 31,
                cols: 101
            },
            ResizePayload {
                rows: 32,
                cols: 102
            },
        ]
    );
    queue.hold_pops(false);
}

#[test]
fn pump_woken_ingress_only_wake_admits_no_worker_resize_frame() {
    let (mut engine, session_id, _, _, queue) = held_resize_engine("woken-resize-ingress");
    engine
        .pump_woken(
            &TerminalWakeBatch {
                adapter_routes: Vec::new(),
                ingress_sessions: vec![session_id],
            },
            20,
        )
        .expect("ingress-only pump");

    assert!(held_resize_payloads(&queue).is_empty());
    queue.hold_pops(false);
}

#[test]
fn pump_woken_capacity_parked_resize_is_admitted_once_after_capacity_returns() {
    let (mut engine, session_id, _, adapter, queue) = held_resize_engine("woken-resize-capacity");
    for _ in 0..(WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS) {
        engine
            .session_runtime_mut()
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: Vec::new(),
            })
            .expect("fill ordinary queue");
    }
    pump_adapter_wake(&mut engine, &adapter, compact_resize_frame(31, 101), 20);
    assert!(held_resize_payloads(&queue).is_empty());
    assert_eq!(input_result_count(&adapter, "resize"), 0);

    queue.hold_pops(false);
    let capacity_batch = engine.wait_wakes(Duration::from_secs(1));
    assert!(capacity_batch.ingress_sessions.contains(&session_id));
    queue.hold_pops(true);
    engine
        .pump_woken(&capacity_batch, 21)
        .expect("capacity retry pump");

    assert_eq!(
        held_resize_payloads(&queue),
        vec![ResizePayload {
            rows: 31,
            cols: 101
        }]
    );
    assert_eq!(input_result_count(&adapter, "resize"), 1);
    queue.hold_pops(false);
}

#[test]
fn pump_woken_retries_capacity_parked_input_once_after_the_capacity_wake() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("woken-capacity-retry".to_string());
    let client = ClientId("woken-capacity-client".to_string());
    let subscription = SubscriptionId("woken-capacity-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("inventory")
        .generation;
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client,
            session_id.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    for _ in 0..4 {
        let batch = engine.wait_wakes(Duration::from_millis(50));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            break;
        }
        engine.pump_woken(&batch, 19).expect("settle prior wakes");
    }
    let queue = engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("control queue");
    queue.hold_pops(true);
    for _ in 0..(WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS) {
        engine
            .session_runtime_mut()
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: Vec::new(),
            })
            .expect("fill ordinary queue");
    }
    adapter.inject(compact_input_frame(b"CAPACITY-ONCE\n"));
    let route_batch = engine.wait_wakes(Duration::from_secs(1));
    assert!(route_batch
        .adapter_routes
        .iter()
        .any(|route| { route.session_id == session_id && route.subscription_id == subscription }));
    engine
        .pump_woken(&route_batch, 20)
        .expect("park full input");
    assert!(
        engine
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.subscription_id == subscription),
        "transient full must keep the exact owner live"
    );
    assert!(
        adapter
            .writes()
            .iter()
            .all(|bytes| { !String::from_utf8_lossy(bytes).contains("input_result") }),
        "a parked frame must not report completion"
    );

    queue.hold_pops(false);
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let batch = engine.wait_wakes(Duration::from_millis(100));
        engine.pump_woken(&batch, 21).expect("capacity retry pump");
        let writes = adapter.writes();
        let results = writes
            .iter()
            .filter(|bytes| String::from_utf8_lossy(bytes).contains("input_result"))
            .count();
        let echoes = writes
            .iter()
            .filter(|bytes| String::from_utf8_lossy(bytes).contains("Q0FQQUNJVFktT05DRQ"))
            .count();
        if results == 1 && echoes == 1 {
            return;
        }
    }
    panic!(
        "capacity retry did not deliver once: {:?}",
        adapter.writes()
    );
}

#[test]
fn pump_woken_uses_only_one_free_slot_for_two_input_commands() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("woken-one-slot".to_string());
    let client = ClientId("woken-one-slot-client".to_string());
    let subscription = SubscriptionId("woken-one-slot-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("generation");
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client,
            session_id.clone(),
            subscription,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    settle_wakes(&mut engine, 19);
    let queue = engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("control queue");
    queue.hold_pops(true);
    for _ in 0..(WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS - 1) {
        engine
            .session_runtime_mut()
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: Vec::new(),
            })
            .expect("leave one ordinary slot");
    }
    adapter.inject(compact_input_frame(b"FIRST\n"));
    adapter.inject(compact_input_frame(b"SECOND\n"));
    let route_batch = engine.wait_wakes(Duration::from_secs(1));
    engine
        .pump_woken(&route_batch, 20)
        .expect("use one free slot");
    assert_eq!(input_result_count(&adapter, "input"), 1);
    assert_eq!(
        queue.class_counts().0,
        WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS
    );

    queue.hold_pops(false);
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let batch = engine.wait_wakes(Duration::from_millis(100));
        engine.pump_woken(&batch, 21).expect("retry second input");
        if input_result_count(&adapter, "input") == 2 {
            return;
        }
    }
    panic!("second input did not complete once: {:?}", adapter.writes());
}

#[test]
fn pump_woken_preserves_all_command_kinds_under_full_pressure() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("woken-pressure-matrix".to_string());
    let client = ClientId("woken-pressure-client".to_string());
    let subscription = SubscriptionId("woken-pressure-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("generation");
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client,
            session_id.clone(),
            subscription,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    settle_wakes(&mut engine, 19);
    let flags = engine
        .read_mode_flags(
            RequestId("woken-pressure-modes".to_string()),
            session_id.clone(),
            20,
        )
        .expect("mode flags");
    let (mode_generation, mode_revision) = flags
        .session_events
        .iter()
        .find_map(|event| match event {
            SessionIoEvent::ModeFlagsReady(ready) => Some((
                ready.mode_freshness.mode_generation,
                ready.mode_freshness.mode_revision,
            )),
            _ => None,
        })
        .expect("authoritative mode freshness");
    let queue = engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("control queue");
    queue.hold_pops(true);
    for _ in 0..(WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS) {
        engine
            .session_runtime_mut()
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: Vec::new(),
            })
            .expect("fill ordinary queue");
    }
    adapter.inject(compact_input_frame(b"PRESSURE\n"));
    adapter.inject(compact_mode_gated_frame(
        mode_generation,
        mode_revision,
        b"GATED\n",
    ));
    adapter.inject(compact_resize_frame(35, 95));
    let route_batch = engine.wait_wakes(Duration::from_secs(1));
    engine
        .pump_woken(&route_batch, 21)
        .expect("park all command kinds");
    assert_eq!(input_result_count(&adapter, "input"), 0);
    assert_eq!(input_result_count(&adapter, "mode_gated_input"), 0);
    assert_eq!(input_result_count(&adapter, "resize"), 0);

    queue.hold_pops(false);
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let batch = engine.wait_wakes(Duration::from_millis(100));
        engine
            .pump_woken(&batch, 22)
            .expect("retry pressured commands");
        if input_result_count(&adapter, "input") == 1
            && input_result_count(&adapter, "mode_gated_input") == 1
            && input_result_count(&adapter, "resize") == 1
        {
            return;
        }
    }
    panic!(
        "pressure matrix did not complete once: {:?}",
        adapter.writes()
    );
}

#[test]
fn pump_woken_hard_stops_input_owner_after_clean_queue_seal() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("woken-clean-seal".to_string());
    let client = ClientId("woken-clean-seal-client".to_string());
    let subscription = SubscriptionId("woken-clean-seal-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("generation");
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client,
            session_id.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    settle_wakes(&mut engine, 19);
    engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("control queue")
        .seal();
    adapter.inject(compact_input_frame(b"SEALED\n"));
    let route_batch = engine.wait_wakes(Duration::from_secs(1));
    engine
        .pump_woken(&route_batch, 20)
        .expect("hard-stop sealed owner");
    assert!(engine
        .list_terminal_subscriptions()
        .iter()
        .all(|row| row.subscription_id != subscription));
    assert_eq!(input_result_count(&adapter, "input"), 0);
}

#[test]
fn pump_woken_defers_adapter_input_during_incremental_attach() {
    let mut engine = WorkerBackedBotsterEngine::new(worker_path());
    let session_id = SessionId("woken-attach-deferral".to_string());
    let client = ClientId("woken-attach-client".to_string());
    let subscription = SubscriptionId("woken-attach-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("start attach");
    assert!(engine.incremental_attach_active(&session_id));
    let generation = engine
        .terminal_subscription_generation(&session_id, &subscription)
        .expect("generation");
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client.clone(),
            session_id.clone(),
            subscription,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind during attach");
    adapter.inject(compact_input_frame(b"AFTER-ATTACH\n"));
    let route_batch = engine.wait_wakes(Duration::from_secs(1));
    engine
        .pump_woken(&route_batch, 11)
        .expect("defer during attach");
    assert!(engine.incremental_attach_active(&session_id));
    assert_eq!(input_result_count(&adapter, "input"), 0);

    let started = Instant::now();
    let mut tick = 12;
    while engine.incremental_attach_active(&session_id) {
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "incremental attach did not complete"
        );
        engine
            .drain_runtime_once(&session_id, tick)
            .expect("drain attach boundary");
        tick += 1;
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!engine.incremental_attach_active(&session_id));
    let capacity_batch = TerminalWakeBatch {
        adapter_routes: Vec::new(),
        ingress_sessions: vec![session_id],
    };
    engine
        .pump_woken(&capacity_batch, 20)
        .expect("apply after attach");
    assert_eq!(input_result_count(&adapter, "input"), 1);
}

#[test]
fn owner_teardown_enqueues_one_cancel_and_leaves_the_shutdown_slot() {
    let mut options = WorkerProcessRuntimeOptions::new(worker_path());
    options.test_mode_gated_hold_ms = Some(10_000);
    let mut engine = WorkerBackedBotsterEngine::with_options(options);
    let session_id = SessionId("queue-bound-one-cancel".to_string());
    let client = ClientId("queue-bound-one-cancel-c".to_string());
    let subscription = SubscriptionId("queue-bound-one-cancel-sub".to_string());
    engine
        .spawn_session(spawn_request(&session_id), CoreSessionMetadata::new())
        .expect("spawn");
    engine
        .attach_client(client.clone(), session_id.clone(), subscription.clone(), 10)
        .expect("attach");
    let _ = drain_until_attached(&mut engine, &session_id, &client);
    let generation = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("inventory after attach")
        .generation;
    let adapter = InjectAdapter::new();
    engine
        .bind_waking_terminal_adapter(
            client.clone(),
            session_id.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let flags = engine
        .read_mode_flags(
            RequestId("queue-bound-probe".to_string()),
            session_id.clone(),
            20,
        )
        .expect("mode flags");
    let (mode_generation, mode_revision) = flags
        .session_events
        .iter()
        .find_map(|event| match event {
            SessionIoEvent::ModeFlagsReady(ready) => Some((
                ready.mode_freshness.mode_generation,
                ready.mode_freshness.mode_revision,
            )),
            _ => None,
        })
        .expect("worker freshness");
    let queue = engine
        .session_runtime()
        .test_control_queue(&session_id)
        .expect("live control queue");
    queue.hold_pops(true);
    adapter.inject(compact_mode_gated_frame(
        mode_generation,
        mode_revision,
        b"hold\n",
    ));
    let batch = engine.wait_wakes(Duration::from_secs(5));
    engine
        .pump_woken(&batch, 21)
        .expect("submit gated hold through targeted pump");
    let (ordinary, cancel, terminal) = queue.class_counts();
    assert_eq!(
        cancel, 0,
        "gated submit is ordinary, not cancel: {ordinary}"
    );
    assert_eq!(terminal, 0);
    let ordinary_capacity = WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS;
    assert!(ordinary >= 1, "gated submit must occupy one ordinary slot");
    for _ in ordinary..ordinary_capacity {
        queue
            .admit(ControlFrameClass::Ordinary, vec![1])
            .expect("fill ordinary capacity");
    }
    assert_eq!(queue.class_counts(), (ordinary_capacity, 0, 0));
    engine
        .detach_client(client, session_id.clone(), subscription, 22)
        .expect("detach held owner");
    assert_eq!(
        queue.class_counts(),
        (ordinary_capacity, 1, 0),
        "one owner teardown must enqueue exactly one cancel"
    );
    queue
        .admit(ControlFrameClass::Terminal, vec![0])
        .expect("one cancel must leave the reserved shutdown slot");
    assert_eq!(queue.class_counts(), (ordinary_capacity, 1, 1));
    queue.hold_pops(false);
}

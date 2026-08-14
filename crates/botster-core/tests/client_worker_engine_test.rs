//! Production ClientWorker bind, inventory, and teardown proofs.

#![cfg(all(unix, feature = "local-runtime"))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_core::{
    BindTerminalAdapterError, ClientId, ClientWorker, CoreSessionMetadata, DefaultBotsterEngine,
    DetachTerminalSubscriptionResult, QueueSource, RequestId, ResizePayload, SessionId,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TerminalSubscriptionGeneration, TransportEgress,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_protocol::TerminalFrame;
use serde_json::Value;

fn session(name: &str) -> SessionId {
    SessionId(name.to_string())
}

fn client(name: &str) -> ClientId {
    ClientId(name.to_string())
}

fn sub(name: &str) -> SubscriptionId {
    SubscriptionId(name.to_string())
}

fn shell_request(session_id: SessionId, script: &str) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: RequestId(format!("spawn-{}", session_id.0)),
        session_id,
        executable: "sh".to_string(),
        arguments: vec!["-c".to_string(), script.to_string()],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

fn process_thread_count() -> usize {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-M", "-p", &pid])
        .output()
        .expect("ps -M should list threads");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn json_type(bytes: &[u8]) -> String {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| value.get("type")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn frame_payload_text(bytes: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return String::new();
    };
    value
        .get("payload_base64")
        .and_then(Value::as_str)
        .and_then(|encoded| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).ok()
        })
        .map(|payload| String::from_utf8_lossy(&payload).into_owned())
        .unwrap_or_default()
}

#[derive(Clone)]
struct DropProbeAdapter {
    closed: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
    inner: SharedFakeTerminalAdapter,
}

impl Drop for DropProbeAdapter {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl TerminalAdapter for DropProbeAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        self.inner.close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }
}

#[test]
fn bind_before_attach_is_a_typed_error() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("bind-before-attach");
    engine
        .spawn_session(
            shell_request(session.clone(), "sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    let error = engine
        .bind_terminal_adapter(
            client("c"),
            session,
            sub("s"),
            TerminalSubscriptionGeneration(1),
            Box::new(SharedFakeTerminalAdapter::auto_complete()),
        )
        .expect_err("pre-attach bind");
    assert!(matches!(
        error,
        BindTerminalAdapterError::BindBeforeAttach { .. }
    ));
}

#[test]
fn attach_then_bind_assigns_generation_and_strips_bound_drain_frames() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("bind-after-attach");
    let client = client("bind-client");
    let subscription = sub("bind-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'bind-live\\n'; sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach");
    let row = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("inventory row after attach");
    assert!(!row.adapter_bound);
    assert_eq!(row.generation, TerminalSubscriptionGeneration(1));

    let adapter = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            client.clone(),
            session.clone(),
            subscription.clone(),
            row.generation,
            Box::new(adapter.clone()),
        )
        .expect("bind");
    assert!(engine
        .list_terminal_subscriptions()
        .iter()
        .any(|row| row.adapter_bound));

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let drained = engine.drain_runtime_once(&session, 2).expect("drain");
        assert!(
            drained.client_egress.iter().all(|(_, frame)| {
                !matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        subscription_id,
                        ..
                    }
                    | TransportEgress::ProcessExit {
                        subscription_id,
                        ..
                    } if subscription_id == &subscription
                )
            }),
            "bound terminal frames must not appear on drain"
        );
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| frame_payload_text(bytes).contains("bind-live"))
        {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "bound adapter never delivered live output: {:?}",
        adapter.snapshot_delivered_frame_bytes()
    );
}

#[test]
fn process_exit_is_delivered_before_close_and_session_stays() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("process-exit-order");
    let client = client("exit-client");
    let subscription = sub("exit-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'exit-marker\\n'; exit 4"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach");
    let generation = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("generation");
    let closed = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let adapter = SharedFakeTerminalAdapter::default();
    let probe = DropProbeAdapter {
        closed: Arc::clone(&closed),
        dropped: Arc::clone(&dropped),
        inner: adapter.clone(),
    };
    let threads_before = process_thread_count();
    engine
        .bind_terminal_adapter(
            client,
            session.clone(),
            subscription.clone(),
            generation,
            Box::new(probe),
        )
        .expect("bind");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_process_exit = false;
    while Instant::now() < deadline {
        let drained = engine.drain_runtime_once(&session, 2).expect("drain");
        assert!(
            drained
                .client_egress
                .iter()
                .all(|(_, frame)| !matches!(frame, TransportEgress::ProcessExit { .. })),
            "bound ProcessExit must not appear on drain"
        );
        adapter.complete_write();
        let delivered = adapter.snapshot_delivered_frame_bytes();
        if delivered
            .iter()
            .any(|bytes| json_type(bytes) == "process_exit")
        {
            saw_process_exit = true;
            assert!(
                delivered.iter().any(|bytes| {
                    json_type(bytes) == "terminal_output"
                        && frame_payload_text(bytes).contains("exit-marker")
                }),
                "remaining output must precede process_exit: {delivered:?}"
            );
        }
        if saw_process_exit && !engine.adapter_is_bound(&session, &subscription) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        saw_process_exit,
        "process_exit must be in delivered_frame_bytes"
    );
    assert!(
        closed.load(Ordering::SeqCst),
        "close() must run on the same path"
    );
    assert!(dropped.load(Ordering::SeqCst), "adapter must be dropped");
    assert!(engine
        .list_terminal_subscriptions()
        .iter()
        .all(|row| row.subscription_id != subscription));
    assert!(
        engine.session(&session).is_some(),
        "host session stays after ProcessExited"
    );
    assert!(
        process_thread_count() <= threads_before,
        "close+drop must not grow process thread count"
    );
}

#[test]
fn adapter_closed_is_one_effective_detach() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("adapter-closed");
    let client_a = client("closed-a");
    let client_b = client("closed-b");
    let sub_a = sub("closed-a");
    let sub_b = sub("closed-b");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'sib\\n'; sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client_a.clone(), session.clone(), sub_a.clone(), 1)
        .expect("attach a");
    engine
        .attach_client(client_b.clone(), session.clone(), sub_b.clone(), 1)
        .expect("attach b");
    let gen_a = engine
        .terminal_subscription_generation(&session, &sub_a)
        .expect("gen a");
    let gen_b = engine
        .terminal_subscription_generation(&session, &sub_b)
        .expect("gen b");
    let adapter_a = SharedFakeTerminalAdapter::auto_complete();
    let adapter_b = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            client_a,
            session.clone(),
            sub_a.clone(),
            gen_a,
            Box::new(adapter_a.clone()),
        )
        .expect("bind a");
    engine
        .bind_terminal_adapter(
            client_b,
            session.clone(),
            sub_b.clone(),
            gen_b,
            Box::new(adapter_b.clone()),
        )
        .expect("bind b");

    adapter_a.close_transport();
    engine
        .drain_runtime_once(&session, 2)
        .expect("drain closed");
    assert!(!engine.has_live(&session, &sub_a));
    assert!(engine.adapter_is_bound(&session, &sub_b));

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        engine
            .drain_runtime_once(&session, 3)
            .expect("sibling drain");
        if adapter_b
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| frame_payload_text(bytes).contains("sib"))
        {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("sibling adapter stopped pumping after Closed");
}

trait LiveInventory {
    fn has_live(&self, session: &SessionId, subscription: &SubscriptionId) -> bool;
}

impl LiveInventory for DefaultBotsterEngine {
    fn has_live(&self, session: &SessionId, subscription: &SubscriptionId) -> bool {
        self.list_terminal_subscriptions()
            .iter()
            .any(|row| &row.session_id == session && &row.subscription_id == subscription)
    }
}

#[test]
fn detach_is_idempotent_by_generation_and_reuse_increments() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("gen-reuse");
    let client = client("gen-client");
    let subscription = sub("gen-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), "sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach");
    let first = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("first gen");
    let (first_result, _) = engine
        .detach_terminal_subscription(
            client.clone(),
            session.clone(),
            subscription.clone(),
            first,
            2,
        )
        .expect("detach first");
    assert!(matches!(
        first_result,
        DetachTerminalSubscriptionResult::Detached { .. }
    ));
    let second_result = engine
        .detach_terminal_subscription(
            client.clone(),
            session.clone(),
            subscription.clone(),
            first,
            3,
        )
        .expect("second detach")
        .0;
    assert!(matches!(
        second_result,
        DetachTerminalSubscriptionResult::AlreadyGone
    ));

    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 4)
        .expect("reattach");
    let second = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("second gen");
    assert_eq!(second, TerminalSubscriptionGeneration(first.0 + 1));
    let stale = engine
        .detach_terminal_subscription(client, session, subscription, first, 5)
        .expect("stale detach")
        .0;
    assert!(matches!(
        stale,
        DetachTerminalSubscriptionResult::GenerationMismatch { .. }
    ));
}

#[test]
fn write_budget_fails_only_the_stalled_subscription() {
    let mut worker = ClientWorker::new();
    let session = session("budget");
    let live = client("live");
    let stalled = client("stalled");
    let live_sub = sub("live");
    let stalled_sub = sub("stalled");
    worker.record_attach(live.clone(), session.clone(), live_sub.clone());
    worker.record_attach(stalled.clone(), session.clone(), stalled_sub.clone());
    let live_gen = worker
        .live_generation(&session, &live_sub)
        .expect("live gen");
    let stalled_gen = worker
        .live_generation(&session, &stalled_sub)
        .expect("stalled gen");
    let live_adapter = SharedFakeTerminalAdapter::auto_complete();
    let stalled_adapter = SharedFakeTerminalAdapter::new();
    worker
        .bind_terminal_adapter(
            &live,
            session.clone(),
            live_sub.clone(),
            live_gen,
            Box::new(live_adapter.clone()),
        )
        .expect("bind live");
    worker
        .bind_terminal_adapter(
            &stalled,
            session.clone(),
            stalled_sub.clone(),
            stalled_gen,
            Box::new(stalled_adapter.clone()),
        )
        .expect("bind stalled");
    stalled_adapter.block_writes();

    let mut frames = vec![
        (
            stalled.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: stalled_sub.clone(),
                data: b"head".to_vec(),
            },
        ),
        (
            live.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: live_sub.clone(),
                data: b"ok".to_vec(),
            },
        ),
    ];
    let _ = worker.ingest_bound_terminal_frames(&mut frames);
    assert!(frames.is_empty());

    let mut torn = false;
    for _ in 0..QueueSource::ClientWorker.default_capacity() {
        let teardowns = worker.pump();
        if teardowns
            .iter()
            .any(|teardown| teardown.subscription_id == stalled_sub)
        {
            torn = true;
            break;
        }
    }
    assert!(
        torn,
        "512 unsuccessful writes must fail the stalled subscription"
    );
    assert!(worker.has_subscription(&session, &live_sub));
    assert!(!worker.has_subscription(&session, &stalled_sub));
    assert!(live_adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| frame_payload_text(bytes).contains("ok")));
}

#[test]
fn lost_snapshot_fails_the_subscription_without_replay() {
    let mut worker = ClientWorker::new();
    let session = session("lost-snapshot");
    let client = client("lost");
    let subscription = sub("lost");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            Box::new(SharedFakeTerminalAdapter::new()),
        )
        .expect("bind");

    let sibling_client = ClientId("kept".to_string());
    let sibling_sub = sub("kept");
    worker.record_attach(sibling_client.clone(), session.clone(), sibling_sub.clone());
    let sibling_gen = worker
        .live_generation(&session, &sibling_sub)
        .expect("sibling gen");
    worker
        .bind_terminal_adapter(
            &sibling_client,
            session.clone(),
            sibling_sub.clone(),
            sibling_gen,
            Box::new(SharedFakeTerminalAdapter::auto_complete()),
        )
        .expect("bind sibling");

    let capacity = QueueSource::ClientWorker.default_capacity();
    let mut frames = Vec::new();
    for index in 0..=capacity + 1 {
        frames.push((
            client.clone(),
            TransportEgress::Snapshot {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                data: vec![index as u8],
            },
        ));
    }
    frames.push((
        sibling_client.clone(),
        TransportEgress::TerminalOutput {
            session_id: session.clone(),
            subscription_id: sibling_sub.clone(),
            data: b"kept".to_vec(),
        },
    ));
    let teardowns = worker.ingest_bound_terminal_frames(&mut frames);
    assert!(teardowns
        .iter()
        .any(|teardown| teardown.subscription_id == subscription));
    assert!(!worker.has_subscription(&session, &subscription));
    assert!(
        frames.iter().all(|(_, frame)| {
            !matches!(
                frame,
                TransportEgress::Snapshot {
                    subscription_id,
                    ..
                } if subscription_id == &subscription
            )
        }),
        "failed-route frames must not escape to drain: {frames:?}"
    );
    assert!(worker.has_subscription(&session, &sibling_sub));
    let _ = worker.pump();
}

#[test]
fn close_is_observed_without_a_closer_thread() {
    let mut worker = ClientWorker::new();
    let session = session("close-idle");
    let client = client("close");
    let subscription = sub("close");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    let dropped = Arc::new(AtomicUsize::new(0));
    struct CountDrop(Arc<AtomicUsize>, SharedFakeTerminalAdapter);
    impl Drop for CountDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl TerminalAdapter for CountDrop {
        fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
            self.1.try_write(frame)
        }
        fn close(&mut self) {
            self.1.close();
        }
        fn pressure(&self) -> TerminalAdapterPressure {
            self.1.pressure()
        }
    }
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            Box::new(CountDrop(
                Arc::clone(&dropped),
                SharedFakeTerminalAdapter::new(),
            )),
        )
        .expect("bind");
    let _ = worker.detach_live(&session, &subscription);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(!worker.adapter_is_bound(&session, &subscription));
}

#[test]
fn inventory_has_no_terminal_state_fields() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/contract/terminal_subscription.rs"
    ));
    let struct_body = source
        .split("pub struct TerminalSubscriptionRecord")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("record struct");
    assert!(struct_body.contains("adapter_bound"));
    assert!(struct_body.contains("generation"));
    assert!(!struct_body.contains("phase"));
    assert!(!struct_body.contains("snapshot"));
    assert!(!struct_body.contains("queue"));
}

#[test]
fn accepted_in_flight_write_counts_toward_the_write_budget() {
    let mut worker = ClientWorker::new();
    let session = session("in-flight-stall");
    let stalled = client("stalled");
    let live = client("live");
    let stalled_sub = sub("stalled");
    let live_sub = sub("live");
    worker.record_attach(stalled.clone(), session.clone(), stalled_sub.clone());
    worker.record_attach(live.clone(), session.clone(), live_sub.clone());
    let stalled_gen = worker
        .live_generation(&session, &stalled_sub)
        .expect("stalled gen");
    let live_gen = worker
        .live_generation(&session, &live_sub)
        .expect("live gen");
    let stalled_adapter = SharedFakeTerminalAdapter::new();
    let live_adapter = SharedFakeTerminalAdapter::auto_complete();
    let dropped = Arc::new(AtomicBool::new(false));
    struct DropFlag(Arc<AtomicBool>, SharedFakeTerminalAdapter);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    impl TerminalAdapter for DropFlag {
        fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
            self.1.try_write(frame)
        }
        fn close(&mut self) {
            self.1.close();
        }
        fn pressure(&self) -> TerminalAdapterPressure {
            self.1.pressure()
        }
    }
    worker
        .bind_terminal_adapter(
            &stalled,
            session.clone(),
            stalled_sub.clone(),
            stalled_gen,
            Box::new(DropFlag(Arc::clone(&dropped), stalled_adapter.clone())),
        )
        .expect("bind stalled");
    worker
        .bind_terminal_adapter(
            &live,
            session.clone(),
            live_sub.clone(),
            live_gen,
            Box::new(live_adapter.clone()),
        )
        .expect("bind live");

    let mut frames = vec![
        (
            stalled.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: stalled_sub.clone(),
                data: b"accepted".to_vec(),
            },
        ),
        (
            live.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: live_sub.clone(),
                data: b"ok".to_vec(),
            },
        ),
    ];
    let _ = worker.ingest_bound_terminal_frames(&mut frames);
    let first = worker.pump();
    assert!(
        first.is_empty(),
        "accepted in-flight write must not fail on the first tick"
    );
    assert_eq!(
        stalled_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Full
    );

    let mut torn = false;
    for _ in 0..QueueSource::ClientWorker.default_capacity() {
        let teardowns = worker.pump();
        if teardowns
            .iter()
            .any(|teardown| teardown.subscription_id == stalled_sub)
        {
            torn = true;
            break;
        }
    }
    assert!(
        torn,
        "stalled completion must fail after 512 in-flight ticks"
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert!(!worker.has_subscription(&session, &stalled_sub));
    assert!(worker.has_subscription(&session, &live_sub));
    assert!(live_adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| frame_payload_text(bytes).contains("ok")));
}

#[test]
fn replacement_attach_hard_stops_the_old_owner() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("replace-attach");
    let owner = client("same-client");
    let old_sub = sub("old");
    let new_sub = sub("new");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'after-replace\\n'; sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(owner.clone(), session.clone(), old_sub.clone(), 1)
        .expect("attach old");
    let old_gen = engine
        .terminal_subscription_generation(&session, &old_sub)
        .expect("old gen");
    let dropped = Arc::new(AtomicBool::new(false));
    let old_adapter = SharedFakeTerminalAdapter::auto_complete();
    struct DropFlag(Arc<AtomicBool>, SharedFakeTerminalAdapter);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    impl TerminalAdapter for DropFlag {
        fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
            self.1.try_write(frame)
        }
        fn close(&mut self) {
            self.1.close();
        }
        fn pressure(&self) -> TerminalAdapterPressure {
            self.1.pressure()
        }
    }
    engine
        .bind_terminal_adapter(
            owner.clone(),
            session.clone(),
            old_sub.clone(),
            old_gen,
            Box::new(DropFlag(Arc::clone(&dropped), old_adapter)),
        )
        .expect("bind old");
    engine
        .attach_client(owner.clone(), session.clone(), new_sub.clone(), 2)
        .expect("replace attach");
    assert!(dropped.load(Ordering::SeqCst), "old adapter must drop");
    assert!(!engine.has_live(&session, &old_sub));
    assert!(engine.has_live(&session, &new_sub));

    let new_gen = engine
        .terminal_subscription_generation(&session, &new_sub)
        .expect("new gen");
    let new_adapter = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            owner,
            session.clone(),
            new_sub.clone(),
            new_gen,
            Box::new(new_adapter.clone()),
        )
        .expect("bind new");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        engine.drain_runtime_once(&session, 3).expect("drain");
        if new_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| frame_payload_text(bytes).contains("after-replace"))
        {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("replacement route never delivered live output");
}

#[test]
fn unbound_process_exit_removes_inventory_and_keeps_the_session() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("unbound-exit");
    let client = client("unbound");
    let subscription = sub("unbound");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'unbound-exit\\n'; exit 3"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach");
    assert!(engine.has_live(&session, &subscription));

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_exit = false;
    while Instant::now() < deadline {
        let drained = engine.drain_runtime_once(&session, 2).expect("drain");
        if drained.client_egress.iter().any(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::ProcessExit {
                    subscription_id,
                    code: Some(3),
                    ..
                } if subscription_id == &subscription
            )
        }) {
            saw_exit = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(saw_exit, "unbound ProcessExit must remain on drain");
    assert!(
        !engine.has_live(&session, &subscription),
        "unbound ProcessExit must remove the inventory row"
    );
    assert!(engine.session(&session).is_some(), "host session stays");
}

#[test]
fn rejected_attach_does_not_publish_inventory() {
    let mut engine = DefaultBotsterEngine::new();
    let error = engine
        .attach_client(
            client("missing"),
            session("missing-session"),
            sub("missing-sub"),
            1,
        )
        .expect_err("unknown session");
    assert!(engine.list_terminal_subscriptions().is_empty());
    let _ = error;
}

#[allow(dead_code)]
fn _mutex_keeps_shared_adapter_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(Mutex::new(SharedFakeTerminalAdapter::new()));
}

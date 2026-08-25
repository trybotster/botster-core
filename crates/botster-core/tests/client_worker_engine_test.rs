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
    DetachTerminalSubscriptionResult, LocalProcessRuntimeOptions, QueueSource, RequestId,
    ResizePayload, SessionId, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TerminalCapabilitySet, TerminalSubscriptionGeneration, TransportEgress,
    WorkerSnapshotPhase,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_protocol::{TerminalFrame, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY};
use serde_json::Value;

fn advertised_capabilities() -> TerminalCapabilitySet {
    TerminalCapabilitySet::from_tokens([FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY])
        .expect("advertised optional token")
}

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

    fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
        self.inner.try_read()
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
            advertised_capabilities(),
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
    assert_eq!(row.capabilities, None);

    let adapter = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            client.clone(),
            session.clone(),
            subscription.clone(),
            row.generation,
            advertised_capabilities(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let bound = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("bound inventory row");
    assert!(bound.adapter_bound);
    assert_eq!(bound.capabilities, Some(advertised_capabilities()));

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
            advertised_capabilities(),
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
            advertised_capabilities(),
            Box::new(adapter_a.clone()),
        )
        .expect("bind a");
    engine
        .bind_terminal_adapter(
            client_b,
            session.clone(),
            sub_b.clone(),
            gen_b,
            advertised_capabilities(),
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
            advertised_capabilities(),
            Box::new(live_adapter.clone()),
        )
        .expect("bind live");
    worker
        .bind_terminal_adapter(
            &stalled,
            session.clone(),
            stalled_sub.clone(),
            stalled_gen,
            advertised_capabilities(),
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
            advertised_capabilities(),
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
            advertised_capabilities(),
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
        fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
            self.1.try_read()
        }
    }
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            advertised_capabilities(),
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
    assert!(struct_body.contains("capabilities"));
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
        fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
            self.1.try_read()
        }
    }
    worker
        .bind_terminal_adapter(
            &stalled,
            session.clone(),
            stalled_sub.clone(),
            stalled_gen,
            advertised_capabilities(),
            Box::new(DropFlag(Arc::clone(&dropped), stalled_adapter.clone())),
        )
        .expect("bind stalled");
    worker
        .bind_terminal_adapter(
            &live,
            session.clone(),
            live_sub.clone(),
            live_gen,
            advertised_capabilities(),
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
        fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
            self.1.try_read()
        }
    }
    engine
        .bind_terminal_adapter(
            owner.clone(),
            session.clone(),
            old_sub.clone(),
            old_gen,
            advertised_capabilities(),
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
            advertised_capabilities(),
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

#[test]
fn second_client_same_subscription_hard_stops_the_first_owner() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("shared-sub");
    let first = client("first");
    let second = client("second");
    let subscription = sub("shared");
    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'second-owner\\n'; sleep 30"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(first.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach first");
    let first_gen = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("first gen");
    let first_dropped = Arc::new(AtomicBool::new(false));
    let first_adapter = SharedFakeTerminalAdapter::auto_complete();
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
        fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
            self.1.try_read()
        }
    }
    engine
        .bind_terminal_adapter(
            first,
            session.clone(),
            subscription.clone(),
            first_gen,
            advertised_capabilities(),
            Box::new(DropFlag(Arc::clone(&first_dropped), first_adapter.clone())),
        )
        .expect("bind first");
    let first_before = first_adapter.snapshot_delivered_frame_bytes().len();
    engine
        .attach_client(second.clone(), session.clone(), subscription.clone(), 2)
        .expect("attach second");
    assert!(first_dropped.load(Ordering::SeqCst));
    let second_gen = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("second gen");
    assert_eq!(second_gen, TerminalSubscriptionGeneration(first_gen.0 + 1));
    let row = engine
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("live row");
    assert_eq!(row.client_id, second);
    let second_adapter = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            second,
            session.clone(),
            subscription,
            second_gen,
            advertised_capabilities(),
            Box::new(second_adapter.clone()),
        )
        .expect("bind second");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        engine.drain_runtime_once(&session, 3).expect("drain");
        if second_adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| frame_payload_text(bytes).contains("second-owner"))
        {
            assert_eq!(
                first_adapter.snapshot_delivered_frame_bytes().len(),
                first_before,
                "first adapter must not receive the second client's frames"
            );
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("second owner never received live output");
}

#[test]
fn unbound_snapshot_phase_does_not_survive_teardown() {
    let mut worker = ClientWorker::new();
    let session = session("stale-phase");
    let owner = client("phase");
    let subscription = sub("phase");
    worker.record_attach(owner.clone(), session.clone(), subscription.clone());
    worker.note_snapshot_phase(&session, &subscription, WorkerSnapshotPhase::History);
    let mut frames = vec![(
        owner.clone(),
        TransportEgress::Snapshot {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
            data: b"unbound-ready".to_vec(),
        },
    )];
    let _ = worker.ingest_bound_terminal_frames(&mut frames);
    assert_eq!(frames.len(), 1);
    let _ = worker.detach_live(&session, &subscription);

    worker.record_attach(owner.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("new gen");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    worker
        .bind_terminal_adapter(
            &owner,
            session.clone(),
            subscription.clone(),
            generation,
            advertised_capabilities(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let mut reuse = vec![(
        owner,
        TransportEgress::Snapshot {
            session_id: session,
            subscription_id: subscription,
            data: b"reused".to_vec(),
        },
    )];
    let _ = worker.ingest_bound_terminal_frames(&mut reuse);
    let _ = worker.pump();
    let phases: Vec<String> = adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|value| value.get("phase")?.as_str().map(str::to_string))
        })
        .collect();
    assert_eq!(
        phases,
        vec!["ready".to_string()],
        "reused Snapshot must not inherit a leftover History phase: {phases:?}"
    );
}

#[test]
fn empty_capability_set_binds_and_round_trips_inventory() {
    let mut worker = ClientWorker::new();
    let session = session("empty-caps");
    let client = client("empty");
    let subscription = sub("empty");
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
            TerminalCapabilitySet::empty(),
            Box::new(SharedFakeTerminalAdapter::new()),
        )
        .expect("empty set binds");
    let row = worker
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("bound row");
    assert!(row.adapter_bound);
    let capabilities = row.capabilities.expect("bound empty is Some");
    assert!(capabilities.is_empty());
}

#[test]
fn second_bind_is_already_bound_even_when_the_set_differs() {
    let mut worker = ClientWorker::new();
    let session = session("already-bound");
    let client = client("bound");
    let subscription = sub("bound");
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
            TerminalCapabilitySet::empty(),
            Box::new(SharedFakeTerminalAdapter::new()),
        )
        .expect("first bind");
    let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let error = worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            advertised_capabilities(),
            Box::new(DropProbeAdapter {
                closed: std::sync::Arc::clone(&closed),
                dropped: std::sync::Arc::clone(&dropped),
                inner: SharedFakeTerminalAdapter::new(),
            }),
        )
        .expect_err("second bind");
    assert!(matches!(
        error,
        BindTerminalAdapterError::AlreadyBound { .. }
    ));
    assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    let row = worker
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("still bound");
    assert_eq!(row.capabilities, Some(TerminalCapabilitySet::empty()));
}

#[test]
fn empty_set_encodes_live_output_and_skips_snapshots() {
    let mut worker = ClientWorker::new();
    let session = session("empty-stream");
    let client = client("empty-stream");
    let subscription = sub("empty-stream");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind empty");
    worker.note_snapshot_phase(&session, &subscription, WorkerSnapshotPhase::Ready);
    let mut frames = vec![
        (
            client.clone(),
            TransportEgress::Snapshot {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                data: b"GHOSTSNP-skip".to_vec(),
            },
        ),
        (
            client,
            TransportEgress::TerminalOutput {
                session_id: session,
                subscription_id: subscription,
                data: b"live-empty".to_vec(),
            },
        ),
    ];
    let teardowns = worker.ingest_bound_terminal_frames(&mut frames);
    assert!(
        teardowns.is_empty(),
        "skipped snapshots must not fail the route"
    );
    assert!(
        frames.is_empty(),
        "skipped snapshots must not return on drain: {frames:?}"
    );
    let _ = worker.pump();
    let delivered = adapter.snapshot_delivered_frame_bytes();
    assert!(
        delivered
            .iter()
            .any(|bytes| json_type(bytes) == "terminal_output"
                && frame_payload_text(bytes).contains("live-empty")),
        "empty set must still encode live output: {delivered:?}"
    );
    assert!(
        delivered.iter().all(|bytes| json_type(bytes) != "snapshot"),
        "empty set must not emit snapshot tags: {delivered:?}"
    );
}

#[test]
fn ready_then_history_set_encodes_incremental_snapshot() {
    let mut worker = ClientWorker::new();
    let session = session("rth-stream");
    let client = client("rth-stream");
    let subscription = sub("rth-stream");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            advertised_capabilities(),
            Box::new(adapter.clone()),
        )
        .expect("bind ready-then-history");
    worker.note_snapshot_phase(&session, &subscription, WorkerSnapshotPhase::Ready);
    let mut frames = vec![
        (
            client.clone(),
            TransportEgress::Snapshot {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                data: b"GHOSTSNP-ready".to_vec(),
            },
        ),
        (
            client,
            TransportEgress::TerminalOutput {
                session_id: session,
                subscription_id: subscription,
                data: b"live-rth".to_vec(),
            },
        ),
    ];
    let _ = worker.ingest_bound_terminal_frames(&mut frames);
    let _ = worker.pump();
    let delivered = adapter.snapshot_delivered_frame_bytes();
    assert!(
        delivered.iter().any(|bytes| json_type(bytes) == "snapshot"),
        "ready-then-history must encode snapshot: {delivered:?}"
    );
    assert!(
        delivered
            .iter()
            .any(|bytes| json_type(bytes) == "terminal_output"
                && frame_payload_text(bytes).contains("live-rth")),
        "ready-then-history must still encode live output: {delivered:?}"
    );
}

#[test]
fn bind_error_variants_remain_the_shipped_cases() {
    fn classify(error: BindTerminalAdapterError) -> &'static str {
        match error {
            BindTerminalAdapterError::BindBeforeAttach { .. } => "bind_before_attach",
            BindTerminalAdapterError::UnknownSubscription { .. } => "unknown_subscription",
            BindTerminalAdapterError::StaleGeneration { .. } => "stale_generation",
            BindTerminalAdapterError::AlreadyBound { .. } => "already_bound",
            BindTerminalAdapterError::ControlPlaneFailed { .. } => "control_plane_failed",
        }
    }
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/contract/terminal_subscription.rs"
    ));
    assert!(!source.contains("UnsupportedCapabilities"));
    assert!(!source.contains("MissingCapabilities"));
    let _ = classify;
}

#[test]
fn one_slot_adapter_delivers_live_bytes_then_process_exit_before_close() {
    let mut worker = ClientWorker::new();
    let session = session("one-slot-flush");
    let client = client("one-slot");
    let subscription = sub("one-slot");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::new();
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");

    let mut frames = vec![
        (
            client.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                data: b"LIVE".to_vec(),
            },
        ),
        (
            client,
            TransportEgress::ProcessExit {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                code: Some(0),
            },
        ),
    ];
    let ingest = worker.ingest_bound_terminal_frames(&mut frames);
    assert!(
        ingest.is_empty(),
        "bound ingest must not hard-stop: {ingest:?}"
    );
    assert!(
        frames.is_empty(),
        "bound frames must leave drain: {frames:?}"
    );

    let first = worker.pump();
    assert!(
        first.is_empty(),
        "must not close while the one-slot write is in flight: {first:?}"
    );
    assert_eq!(
        adapter.snapshot_pressure(),
        TerminalAdapterPressure::Full,
        "accepted LIVE must occupy the one write slot"
    );
    assert!(
        adapter.snapshot_delivered_frame_bytes().is_empty(),
        "in-flight LIVE is not delivered until complete"
    );

    adapter.complete_write();
    assert!(
        adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| json_type(bytes) == "terminal_output"
                && frame_payload_text(bytes).contains("LIVE")),
        "LIVE must complete before process_exit occupies the slot: {:?}",
        adapter.snapshot_delivered_frame_bytes()
    );

    let second = worker.pump();
    assert!(
        second.is_empty(),
        "must not close while process_exit is in flight: {second:?}"
    );
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Full);
    adapter.complete_write();
    let delivered = adapter.snapshot_delivered_frame_bytes();
    let types: Vec<String> = delivered.iter().map(|bytes| json_type(bytes)).collect();
    assert!(
        types.iter().any(|kind| kind == "process_exit"),
        "process_exit must complete before close: {types:?}"
    );
    let output_at = types
        .iter()
        .position(|kind| kind == "terminal_output")
        .expect("terminal_output");
    let exit_at = types
        .iter()
        .position(|kind| kind == "process_exit")
        .expect("process_exit");
    assert!(
        output_at < exit_at,
        "LIVE bytes must precede process_exit: {types:?}"
    );

    let third = worker.pump();
    assert_eq!(
        third.len(),
        1,
        "close on the tick that observes completed process_exit: {third:?}"
    );
    assert!(!worker.adapter_is_bound(&session, &subscription));
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
}

#[test]
fn unbound_process_exit_rejects_late_bind_and_closes_the_presented_adapter() {
    let mut worker = ClientWorker::new();
    let session = session("unbound-exit-bind");
    let client = client("unbound-exit");
    let subscription = sub("unbound-exit");
    worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");

    let mut frames = vec![
        (
            client.clone(),
            TransportEgress::TerminalOutput {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                data: b"LIVE".to_vec(),
            },
        ),
        (
            client.clone(),
            TransportEgress::ProcessExit {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                code: Some(0),
            },
        ),
    ];
    let teardowns = worker.ingest_bound_terminal_frames(&mut frames);
    assert_eq!(teardowns.len(), 1, "unbound ProcessExit must hard-stop");
    assert!(
        frames.iter().any(|(_, frame)| matches!(
            frame,
            TransportEgress::TerminalOutput { data, .. } if data == b"LIVE"
        )),
        "unbound LIVE bytes must stay on the drain path: {frames:?}"
    );
    assert!(!worker.has_subscription(&session, &subscription));

    let presented = SharedFakeTerminalAdapter::new();
    let error = worker
        .bind_terminal_adapter(
            &client,
            session,
            subscription,
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(presented.clone()),
        )
        .expect_err("late bind after teardown");
    assert!(matches!(
        error,
        BindTerminalAdapterError::UnknownSubscription { .. }
    ));
    assert_eq!(
        presented.snapshot_pressure(),
        TerminalAdapterPressure::Closed,
        "failed bind must close the presented adapter"
    );
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

fn compact_mode_gated_frame(mode_generation: u64, mode_revision: u64, data: &[u8]) -> Vec<u8> {
    let body_len = u16::try_from(16 + data.len()).expect("gated body fits u16");
    let mut bytes = vec![1, 2];
    bytes.extend_from_slice(&body_len.to_be_bytes());
    bytes.extend_from_slice(&mode_generation.to_be_bytes());
    bytes.extend_from_slice(&mode_revision.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn input_result_fields(bytes: &[u8]) -> Option<(String, String, bool)> {
    let value = serde_json::from_slice::<Value>(bytes).ok()?;
    if value.get("type")?.as_str()? != "input_result" {
        return None;
    }
    Some((
        value.get("subscription_id")?.as_str()?.to_string(),
        value.get("kind")?.as_str()?.to_string(),
        value.get("admitted")?.as_bool()?,
    ))
}

fn bind_local_pair(
    engine: &mut DefaultBotsterEngine,
    session: &SessionId,
    client: &ClientId,
    subscription: &SubscriptionId,
    script: &str,
) -> SharedFakeTerminalAdapter {
    engine
        .spawn_session(
            shell_request(session.clone(), script),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach");
    let generation = engine
        .terminal_subscription_generation(session, subscription)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    engine
        .bind_terminal_adapter(
            client.clone(),
            session.clone(),
            subscription.clone(),
            generation,
            advertised_capabilities(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    adapter
}

fn apply_and_pump(engine: &mut DefaultBotsterEngine, session: &SessionId) {
    engine
        .apply_terminal_input(session, 2)
        .expect("apply terminal input");
    let _ = engine.drain_runtime_once(session, 2).expect("pump");
}

#[test]
fn local_input_result_carries_the_live_subscription_id() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("local-input-result-id");
    let client = client("local-input-result-client");
    let subscription = sub("local-input-result-sub");
    let adapter = bind_local_pair(
        &mut engine,
        &session,
        &client,
        &subscription,
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
    );
    adapter.inject_ingress_frame(compact_input_frame(b"hello\n"));
    apply_and_pump(&mut engine, &session);
    let delivered = adapter.snapshot_delivered_frame_bytes();
    assert!(
        delivered.iter().any(|bytes| {
            input_result_fields(bytes) == Some((subscription.0.clone(), "input".to_string(), true))
        }),
        "input_result must carry the live subscription id: {delivered:?}"
    );
}

#[test]
fn local_mode_gated_input_result_carries_the_live_subscription_id() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("local-gated-result-id");
    let client = client("local-gated-result-client");
    let subscription = sub("local-gated-result-sub");
    let adapter = bind_local_pair(&mut engine, &session, &client, &subscription, "sleep 30");
    adapter.inject_ingress_frame(compact_mode_gated_frame(1, 1, b"x"));
    apply_and_pump(&mut engine, &session);
    let delivered = adapter.snapshot_delivered_frame_bytes();
    assert!(
        delivered.iter().any(|bytes| {
            input_result_fields(bytes)
                == Some((
                    subscription.0.clone(),
                    "mode_gated_input".to_string(),
                    false,
                ))
        }),
        "rejected gated input_result must still name the live subscription: {delivered:?}"
    );
    assert!(
        engine.adapter_is_bound(&session, &subscription),
        "SessionNotWritable keeps the owner live"
    );
}

#[test]
fn local_apply_errors_fail_closed_and_leave_siblings() {
    let mut engine = DefaultBotsterEngine::with_local_options(LocalProcessRuntimeOptions {
        test_fail_pty_writes: true,
        ..LocalProcessRuntimeOptions::default()
    });
    let failed_session = session("local-apply-fail");
    let sibling_session = session("local-apply-sibling");
    let failed_client = client("local-apply-fail-client");
    let sibling_client = client("local-apply-sibling-client");
    let failed_sub = sub("local-apply-fail-sub");
    let sibling_sub = sub("local-apply-sibling-sub");
    let failed_adapter = bind_local_pair(
        &mut engine,
        &failed_session,
        &failed_client,
        &failed_sub,
        "sleep 30",
    );
    let sibling_adapter = bind_local_pair(
        &mut engine,
        &sibling_session,
        &sibling_client,
        &sibling_sub,
        "sleep 30",
    );

    failed_adapter.inject_ingress_frame(compact_input_frame(b"die\n"));
    apply_and_pump(&mut engine, &failed_session);
    assert_eq!(
        failed_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Closed
    );
    assert!(!engine.adapter_is_bound(&failed_session, &failed_sub));
    assert!(engine.adapter_is_bound(&sibling_session, &sibling_sub));
    assert_eq!(
        sibling_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Ready
    );

    let mut engine = DefaultBotsterEngine::with_local_options(LocalProcessRuntimeOptions {
        test_fail_pty_writes: true,
        ..LocalProcessRuntimeOptions::default()
    });
    let resize_session = session("local-resize-fail");
    let resize_client = client("local-resize-fail-client");
    let resize_sub = sub("local-resize-fail-sub");
    let resize_adapter = bind_local_pair(
        &mut engine,
        &resize_session,
        &resize_client,
        &resize_sub,
        "sleep 30",
    );
    resize_adapter.inject_ingress_frame(compact_resize_frame(24, 80));
    apply_and_pump(&mut engine, &resize_session);
    assert_eq!(
        resize_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Closed
    );
    assert!(!engine.adapter_is_bound(&resize_session, &resize_sub));
}

#[test]
fn intake_refuses_the_command_that_would_exceed_capacity() {
    let mut worker = ClientWorker::default();
    let session = session("queue-cap");
    let client = client("queue-cap-client");
    let subscription = sub("queue-cap-sub");
    let _ = worker.record_attach(client.clone(), session.clone(), subscription.clone());
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::new();
    worker
        .bind_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let capacity = botster_core::engine::client_worker::INPUT_QUEUE_CAPACITY;
    let intake = botster_core::engine::client_worker::INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK;
    assert_eq!(capacity % intake, 0);
    for _ in 0..(capacity / intake) {
        for _ in 0..intake {
            adapter.inject_ingress_frame(compact_input_frame(b"x"));
        }
        let teardowns = worker.intake_terminal_input();
        assert!(teardowns.is_empty(), "capacity fill must stay live");
    }
    assert_eq!(
        worker.input_queue_len(&session, &subscription),
        Some(capacity)
    );
    adapter.inject_ingress_frame(compact_input_frame(b"overflow"));
    let teardowns = worker.intake_terminal_input();
    assert_eq!(teardowns.len(), 1);
    assert_eq!(teardowns[0].subscription_id, subscription);
    assert!(!worker.has_subscription(&session, &subscription));
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
}

#[allow(dead_code)]
fn _mutex_keeps_shared_adapter_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(Mutex::new(SharedFakeTerminalAdapter::new()));
}

//! Production ClientWorker bind, inventory, and teardown proofs.

#![cfg(all(unix, feature = "local-runtime"))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalRouteCloseReason,
};
use botster_core::{
    BindTerminalAdapterError, BotsterEngineOutput, ClientId, ClientWorker, CoreSessionMetadata,
    DefaultBotsterEngine, DetachTerminalSubscriptionResult, RequestId, ResizePayload, SessionId,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TerminalCapabilitySet, TerminalSubscriptionGeneration, TerminalWakeKind, TerminalWakeSink,
    TransportEgress, WakingTerminalAdapter,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_protocol::{
    decode_input_result, InputOutcome, RoutedTerminalFrame, TerminalFrame, TerminalKind,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
};
use botster_terminal_protocol_client::encode_paste;

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
    fn try_write(&mut self, frame: &RoutedTerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self, reason: TerminalRouteCloseReason) {
        self.closed.store(true, Ordering::SeqCst);
        self.inner.close(reason);
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }

    fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
        self.inner.try_read()
    }
}

impl WakingTerminalAdapter for DropProbeAdapter {
    fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        self.inner.set_wake_sink(sink);
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
        .bind_waking_terminal_adapter(
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

trait LiveInventory {
    fn has_live(&self, session: &SessionId, subscription: &SubscriptionId) -> bool;
}

impl LiveInventory for DefaultBotsterEngine {
    fn has_live(&self, session: &SessionId, subscription: &SubscriptionId) -> bool {
        self.list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
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
fn close_is_observed_without_a_closer_thread() {
    let mut worker = ClientWorker::new();
    let session = session("close-idle");
    let client = client("close");
    let subscription = sub("close");
    worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("attach");
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
        fn try_write(
            &mut self,
            frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            self.1.try_write(frame)
        }
        fn close(&mut self, reason: TerminalRouteCloseReason) {
            self.1.close(reason);
        }
        fn pressure(&self) -> TerminalAdapterPressure {
            self.1.pressure()
        }
        fn try_read(&mut self) -> botster_core::contract::terminal_adapter::TerminalIngress {
            self.1.try_read()
        }
    }
    impl WakingTerminalAdapter for CountDrop {
        fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
            self.1.set_wake_sink(sink);
        }
    }
    worker
        .bind_waking_terminal_adapter(
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
fn stale_adapter_close_after_reattach_does_not_stop_the_live_generation() {
    let mut worker = ClientWorker::new();
    let session = session("stale-close-session");
    let client = client("stale-close-client");
    let subscription = sub("stale-close-route");
    let (first_generation, _) = worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("first attach");
    let first_adapter = SharedFakeTerminalAdapter::auto_complete();
    worker
        .bind_waking_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            first_generation,
            TerminalCapabilitySet::empty(),
            Box::new(first_adapter.clone()),
        )
        .expect("first bind");

    assert!(matches!(
        worker.detach_generation(&session, &subscription, first_generation),
        DetachTerminalSubscriptionResult::Detached { .. }
    ));
    let (second_generation, _) = worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("second attach");
    assert_ne!(first_generation, second_generation);
    let second_adapter = SharedFakeTerminalAdapter::auto_complete();
    worker
        .bind_waking_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            second_generation,
            TerminalCapabilitySet::empty(),
            Box::new(second_adapter.clone()),
        )
        .expect("second bind");

    first_adapter.close_transport();
    assert!(
        !first_adapter.wake(TerminalWakeKind::Closed),
        "the retired adapter sink must not wake the reused route"
    );
    assert!(worker.adapter_is_bound(&session, &subscription));

    worker
        .push_route_frame(
            &session,
            &subscription,
            botster_terminal_protocol::encode_snapshot_ready(b"GHOSTSNP").expect("ready frame"),
        )
        .expect("queue ready for the live generation");
    assert!(second_adapter.wake(TerminalWakeKind::Writable));
    let batch = worker.wake_source().wait_wakes(Duration::from_secs(1));
    assert!(worker.pump_woken(&batch).is_empty());
    let delivered = second_adapter.snapshot_delivered_frames();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].generation, second_generation.0);
    assert!(worker.adapter_is_bound(&session, &subscription));
    assert_eq!(
        second_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Ready
    );
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
    assert!(engine
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .is_empty());
    let _ = error;
}

#[test]
fn empty_capability_set_binds_and_round_trips_inventory() {
    let mut worker = ClientWorker::new();
    let session = session("empty-caps");
    let client = client("empty");
    let subscription = sub("empty");
    worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("attach");
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    worker
        .bind_waking_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            TerminalCapabilitySet::empty(),
            Box::new(SharedFakeTerminalAdapter::new()),
        )
        .expect("empty set binds");
    let row = worker
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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
    worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("attach");
    let generation = worker
        .live_generation(&session, &subscription)
        .expect("generation");
    worker
        .bind_waking_terminal_adapter(
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
        .bind_waking_terminal_adapter(
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("still bound");
    assert_eq!(row.capabilities, Some(TerminalCapabilitySet::empty()));
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

fn compact_input_frame(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).expect("input fits u16");
    let mut bytes = vec![1, 1];
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
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
        .bind_waking_terminal_adapter(
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

fn apply_and_pump(engine: &mut DefaultBotsterEngine, _session: &SessionId) {
    let batch = engine.wait_wakes(Duration::from_secs(5));
    assert!(
        !batch.adapter_routes.is_empty(),
        "adapter input must wake its route"
    );
    let _ = engine.pump_woken(&batch, 2).expect("targeted pump");
}

#[test]
fn local_unsafe_paste_rejects_with_zero_counts_and_exact_route_identity() {
    let mut engine = DefaultBotsterEngine::new();
    let session = session("local-paste-result-id");
    let client = client("local-paste-result-client");
    let subscription = sub("local-paste-result-sub");
    let adapter = bind_local_pair(&mut engine, &session, &client, &subscription, "sleep 30");
    let generation = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("bound generation");
    for frame in encode_paste(77, false, b"line1\nline2").expect("encode paste") {
        adapter.inject_ingress_frame(frame.as_bytes().to_vec());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = 'completion: loop {
        for delivered in adapter.snapshot_delivered_frames() {
            let frame = TerminalFrame::from_bytes(&delivered.bytes).expect("terminal frame");
            if frame.kind() != TerminalKind::InputResult {
                continue;
            }
            let result = decode_input_result(&frame).expect("input result");
            if result.operation_id == 77 {
                assert_eq!(delivered.route, subscription.0);
                assert_eq!(delivered.generation, generation.0);
                break 'completion result;
            }
        }
        assert!(engine.adapter_is_bound(&session, &subscription));
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "paste result deadline");
        let batch = engine.wait_wakes(remaining);
        engine.pump_woken(&batch, 2).expect("targeted pump");
    };
    assert_eq!(result.operation_id, 77);
    assert_eq!(result.outcome, InputOutcome::RejectedUnsafePaste);
    // These counts are the typed result, not an independent PTY write measurement.
    assert_eq!(result.accepted_payload_bytes, Some(0));
    assert_eq!(result.written_pty_bytes, Some(0));
    let inventory = engine
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance");
    let row = inventory
        .records
        .iter()
        .find(|row| row.subscription_id == subscription)
        .expect("subscription remains live");
    assert_eq!(row.session_id, session);
    assert_eq!(row.client_id, client);
    assert_eq!(row.subscription_id, subscription);
    assert_eq!(row.generation, generation);
    assert!(row.adapter_bound);
    assert!(engine.adapter_is_bound(&session, &subscription));
}

fn subscription_live(engine: &DefaultBotsterEngine, subscription: &SubscriptionId) -> bool {
    engine
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .any(|row| &row.subscription_id == subscription)
}

fn drain_has_removed_route(drained: &BotsterEngineOutput, subscription: &SubscriptionId) -> bool {
    drained.client_egress.iter().any(|(_, frame)| {
        matches!(
            frame,
            TransportEgress::TerminalOutput {
                subscription_id,
                ..
            }
            | TransportEgress::Snapshot {
                subscription_id,
                ..
            } if subscription_id == subscription
        )
    })
}

fn assert_input_hard_stop(
    engine: &mut DefaultBotsterEngine,
    adapter: &SharedFakeTerminalAdapter,
    session: &SessionId,
    subscription: &SubscriptionId,
    sibling_session: &SessionId,
    sibling_sub: &SubscriptionId,
    sibling_adapter: &SharedFakeTerminalAdapter,
) {
    apply_and_pump(engine, session);
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert!(!engine.adapter_is_bound(session, subscription));
    assert!(!subscription_live(engine, subscription));
    assert!(engine.adapter_is_bound(sibling_session, sibling_sub));
    assert_eq!(
        sibling_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Ready
    );
    let drained = engine
        .drain_runtime_once(session, 3)
        .expect("drain after hard-stop");
    assert!(
        !drain_has_removed_route(&drained, subscription),
        "removed generation must not receive later client_egress"
    );
}

fn bind_hard_stop_pair(
    label: &str,
) -> (
    DefaultBotsterEngine,
    SessionId,
    SubscriptionId,
    SharedFakeTerminalAdapter,
    SessionId,
    SubscriptionId,
    SharedFakeTerminalAdapter,
) {
    let mut engine = DefaultBotsterEngine::new();
    let failed = session(&format!("route-{label}"));
    let sibling = session(&format!("route-{label}-sib"));
    let failed_sub = sub(&format!("route-{label}-sub"));
    let sibling_sub = sub(&format!("route-{label}-sib-sub"));
    let adapter = bind_local_pair(
        &mut engine,
        &failed,
        &client(&format!("route-{label}-c")),
        &failed_sub,
        "sleep 30",
    );
    let sibling_adapter = bind_local_pair(
        &mut engine,
        &sibling,
        &client(&format!("route-{label}-sib-c")),
        &sibling_sub,
        "sleep 30",
    );
    (
        engine,
        failed,
        failed_sub,
        adapter,
        sibling,
        sibling_sub,
        sibling_adapter,
    )
}

#[test]
fn input_path_hard_stop_unsubscribes_the_multiplexer_route() {
    let (mut engine, failed, failed_sub, adapter, sibling, sibling_sub, sibling_adapter) =
        bind_hard_stop_pair("malformed");
    adapter.inject_ingress_frame(vec![0xff, 0xff, 0xff]);
    assert_input_hard_stop(
        &mut engine,
        &adapter,
        &failed,
        &failed_sub,
        &sibling,
        &sibling_sub,
        &sibling_adapter,
    );

    let (mut engine, failed, failed_sub, adapter, sibling, sibling_sub, sibling_adapter) =
        bind_hard_stop_pair("lost");
    adapter.inject_ingress_frame(compact_input_frame(b"keep"));
    adapter.drop_buffered_ingress_frame();
    assert_input_hard_stop(
        &mut engine,
        &adapter,
        &failed,
        &failed_sub,
        &sibling,
        &sibling_sub,
        &sibling_adapter,
    );
}

#[test]
fn owner_removal_matrix_closes_adapter_and_route() {
    let mut engine = DefaultBotsterEngine::new();
    let live = session("owner-detach-live");
    let gen = session("owner-detach-gen");
    let torn = session("owner-teardown-session");
    let sibling = session("owner-matrix-sib");
    let live_sub = sub("owner-detach-live-sub");
    let gen_sub = sub("owner-detach-gen-sub");
    let torn_sub = sub("owner-teardown-session-sub");
    let sibling_sub = sub("owner-matrix-sib-sub");
    let live_client = client("owner-detach-live-c");
    let gen_client = client("owner-detach-gen-c");
    let torn_client = client("owner-teardown-session-c");
    let live_adapter = bind_local_pair(&mut engine, &live, &live_client, &live_sub, "sleep 30");
    let gen_adapter = bind_local_pair(&mut engine, &gen, &gen_client, &gen_sub, "sleep 30");
    let torn_adapter = bind_local_pair(&mut engine, &torn, &torn_client, &torn_sub, "sleep 30");
    let sibling_adapter = bind_local_pair(
        &mut engine,
        &sibling,
        &client("owner-matrix-sib-c"),
        &sibling_sub,
        "sleep 30",
    );

    engine
        .detach_client(live_client, live.clone(), live_sub.clone(), 4)
        .expect("detach_live");
    let generation = engine
        .terminal_subscription_generation(&gen, &gen_sub)
        .expect("generation");
    engine
        .detach_terminal_subscription(gen_client, gen.clone(), gen_sub.clone(), generation, 5)
        .expect("detach_generation");
    engine
        .shutdown_session(torn.clone(), "matrix", 6)
        .expect("teardown_session");
    let deadline = Instant::now() + Duration::from_secs(5);
    while engine.adapter_is_bound(&torn, &torn_sub) && Instant::now() < deadline {
        let batch = engine.wait_wakes(Duration::from_secs(5));
        engine
            .pump_woken(&batch, 6)
            .expect("shutdown wake delivers ProcessExit then closes the owner");
        torn_adapter.complete_write();
    }
    engine.forget_terminal_session(&live);
    engine.forget_terminal_session(&gen);

    for (adapter, session, subscription) in [
        (&live_adapter, &live, &live_sub),
        (&gen_adapter, &gen, &gen_sub),
        (&torn_adapter, &torn, &torn_sub),
    ] {
        assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
        assert!(!engine.adapter_is_bound(session, subscription));
        assert!(!subscription_live(&engine, subscription));
        if engine.session(session).is_some() {
            let drained = engine.drain_runtime_once(session, 7).expect("drain");
            assert!(!drain_has_removed_route(&drained, subscription));
        }
    }
    assert!(engine.adapter_is_bound(&sibling, &sibling_sub));
    assert_eq!(
        sibling_adapter.snapshot_pressure(),
        TerminalAdapterPressure::Ready
    );
}

#[allow(dead_code)]
fn _mutex_keeps_shared_adapter_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(Mutex::new(SharedFakeTerminalAdapter::new()));
}

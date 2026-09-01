#![allow(missing_docs)]

use std::fs;
use std::process::Command;
use std::sync::{mpsc, Once};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet,
    TerminalWakeBatch, TerminalWakeKind, WAKE_QUEUE_CAPACITY,
};
use botster_core_daemon::{
    CaptureColorAndSnapshotRequest, CaptureSnapshotRequest, CoreDaemon, CoreDaemonConfig,
    CoreDaemonError, ReadModeFlagsRequest, ReadScreenRequest, SessionLifecycleChangeKind,
    SpawnSessionRequest, WakePumpControl, WakePumpError, WakePumpWait,
};
use botster_core_test_support::terminal_adapter::{
    SharedFakeTerminalAdapter, TerminalAdapterHarnessDriver,
};

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-wake-{label}-{nanos}"))
}

#[cfg(unix)]
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
            .expect("worker build command");
        assert!(status.success(), "worker binary must build");
    });
    let mut path = std::env::current_exe().expect("test executable path");
    while !matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("debug" | "release")
    ) {
        assert!(path.pop(), "test executable must be under target");
    }
    path.join("botster-session-worker")
}

#[cfg(unix)]
fn bind_size_reporting_worker(
    daemon: &mut CoreDaemon,
    label: &str,
) -> (SessionId, SharedFakeTerminalAdapter) {
    let session_id = SessionId(format!("{label}-session"));
    let client_id = ClientId(format!("{label}-client"));
    let subscription_id = SubscriptionId(format!("{label}-sub"));
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "stty -echo; printf ready; while IFS= read -r _; do stty size; done".into();
    daemon.spawn(request, 1).expect("spawn size reporter");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "worker attach did not finish");
        daemon.drain(&session_id, 2).expect("drain attach boundary");
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
            .any(|value| {
                value.get("type").and_then(serde_json::Value::as_str) == Some("attach_state")
                    && value.get("state").and_then(serde_json::Value::as_str) == Some("attached")
            })
        {
            return (session_id, adapter);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn pump_until_encoded_output(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    encoded: &str,
    tick: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "worker output did not arrive");
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon.pump_woken(&batch, tick).expect("pump worker output");
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| String::from_utf8_lossy(bytes).contains(encoded))
        {
            return;
        }
    }
}

fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec!["-c".to_string(), "printf ready; sleep 2".to_string()],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

fn empty_caps() -> TerminalCapabilitySet {
    TerminalCapabilitySet::empty()
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
    let body_len = u16::try_from(16 + data.len()).expect("gated input fits u16");
    let mut bytes = vec![1, 2];
    bytes.extend_from_slice(&body_len.to_be_bytes());
    bytes.extend_from_slice(&mode_generation.to_be_bytes());
    bytes.extend_from_slice(&mode_revision.to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn delivered_input_result_count(adapter: &SharedFakeTerminalAdapter, kind: &str) -> usize {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter(|bytes| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .is_some_and(|value| {
                    value.get("type").and_then(|field| field.as_str()) == Some("input_result")
                        && value.get("kind").and_then(|field| field.as_str()) == Some(kind)
                })
        })
        .count()
}

fn delivered_admitted_input_results(
    adapter: &SharedFakeTerminalAdapter,
    kind: &str,
) -> Vec<serde_json::Value> {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("input_result")
                && value.get("kind").and_then(|field| field.as_str()) == Some(kind)
                && value.get("admitted").and_then(|field| field.as_bool()) == Some(true)
        })
        .collect()
}

fn assert_send_sync_clone<T: Send + Sync + Clone>() {}

#[test]
fn wake_pump_control_interrupts_a_daemon_constructed_on_its_owner_thread() {
    assert_send_sync_clone::<WakePumpControl>();
    let data_dir = temp_data_dir("owner-thread");
    let (control_tx, control_rx) = mpsc::sync_channel(1);
    let (interrupted_tx, interrupted_rx) = mpsc::sync_channel(1);
    let owner = std::thread::spawn(move || {
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
        control_tx
            .send(daemon.wake_pump_control())
            .expect("publish control");
        assert!(matches!(
            daemon.wait_pump(Duration::from_secs(5)),
            WakePumpWait::Interrupted
        ));
        interrupted_tx.send(()).expect("publish interrupt result");
        assert!(matches!(
            daemon.wait_pump(Duration::from_secs(5)),
            WakePumpWait::Stopped
        ));
        daemon.shutdown(None, 1).expect("ordered shutdown");
    });
    let control = control_rx.recv().expect("receive control");
    control.interrupt();
    interrupted_rx.recv().expect("interrupt was observed");
    control.request_stop();
    owner.join().expect("owner thread");
}

#[test]
fn shutdown_without_a_pump_control_keeps_existing_behavior() {
    let data_dir = temp_data_dir("no-control");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
    daemon.shutdown(None, 1).expect("ordinary shutdown");
}

#[test]
fn pump_hosted_shutdown_fails_closed_until_stop_is_observed() {
    let data_dir = temp_data_dir("stop-required");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
    let control = daemon.wake_pump_control();
    control.request_stop();
    assert!(matches!(
        daemon.shutdown(None, 1),
        Err(CoreDaemonError::WakePump(WakePumpError::StopNotObserved))
    ));
    assert!(matches!(
        daemon.wait_pump(Duration::ZERO),
        WakePumpWait::Stopped
    ));
    daemon
        .shutdown(None, 1)
        .expect("shutdown after observed stop");
}

#[test]
fn stop_collision_returns_one_real_batch_then_stops() {
    let data_dir = temp_data_dir("stop-collision");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let (session_id, _, subscription_id, _) = bind_probe(
        &mut daemon,
        "stop-collision-session",
        "stop-collision-client",
        "stop-collision-sub",
        adapter.clone(),
    );
    let _ = daemon.wait_wakes(Duration::ZERO);
    let ingress = daemon.wake_source().session_handle(session_id.clone());
    let control = daemon.wake_pump_control();
    control.request_stop();
    assert!(adapter.wake(TerminalWakeKind::Writable));
    ingress.notify();

    let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::ZERO) else {
        panic!("stop collision must return already queued work");
    };
    assert_eq!(batch.adapter_routes.len(), 1);
    assert_eq!(batch.adapter_routes[0].subscription_id, subscription_id);
    assert_eq!(batch.ingress_sessions, vec![session_id]);
    assert!(matches!(
        daemon.shutdown(None, 3),
        Err(CoreDaemonError::WakePump(WakePumpError::StopNotObserved))
    ));
    let reads_before = adapter.try_read_count();
    let outcome = daemon
        .pump_woken(&batch, 3)
        .expect("pump the stop collision batch");
    assert_eq!(outcome.pumped_routes, 1);
    assert!(adapter.try_read_count() > reads_before);
    assert!(matches!(
        daemon.shutdown(None, 3),
        Err(CoreDaemonError::WakePump(WakePumpError::StopNotObserved))
    ));
    assert!(matches!(
        daemon.wait_pump(Duration::from_secs(1)),
        WakePumpWait::Stopped
    ));
    daemon.shutdown(None, 1).expect("ordered shutdown");
}

#[test]
fn sustained_wake_producer_cannot_extend_the_post_stop_loop() {
    let data_dir = temp_data_dir("bounded-stop");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
    let session_id = SessionId("bounded-stop-session".into());
    let ingress = daemon.wake_source().session_handle(session_id);
    let control = daemon.wake_pump_control();
    let producer_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let producer_flag = std::sync::Arc::clone(&producer_stop);
    let producer = std::thread::spawn(move || {
        while !producer_flag.load(std::sync::atomic::Ordering::Acquire) {
            ingress.notify();
            std::hint::spin_loop();
        }
    });
    control.request_stop();
    let first = daemon.wait_pump(Duration::ZERO);
    assert!(matches!(
        first,
        WakePumpWait::Wakes(_) | WakePumpWait::Stopped
    ));
    assert!(matches!(
        daemon.wait_pump(Duration::from_secs(1)),
        WakePumpWait::Stopped
    ));
    producer_stop.store(true, std::sync::atomic::Ordering::Release);
    producer.join().expect("producer");
    daemon.shutdown(None, 1).expect("ordered shutdown");
}

#[cfg(unix)]
#[test]
fn interrupt_during_shutdown_preserves_final_output_and_exit() {
    let data_dir = temp_data_dir("interrupt-shutdown");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("interrupt-shutdown-session".into());
    let client_id = ClientId("interrupt-shutdown-client".into());
    let subscription_id = SubscriptionId("interrupt-shutdown-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "trap 'sleep 0.2; printf final; exit 0' TERM; printf ready; while :; do sleep 1; done"
            .into();
    daemon.spawn(request, 1).expect("spawn shutdown fixture");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    pump_until_encoded_output(&mut daemon, &adapter, "cmVhZHk=", 3);

    let control = daemon.wake_pump_control();
    control.request_stop();
    if let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::ZERO) {
        daemon
            .pump_woken(&batch, 3)
            .expect("pump the shutdown collision batch");
        assert!(matches!(
            daemon.wait_pump(Duration::ZERO),
            WakePumpWait::Stopped
        ));
    }

    let shutdown_active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let active = std::sync::Arc::clone(&shutdown_active);
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = std::sync::Arc::clone(&interrupt_count);
    let interrupt_control = control.clone();
    let interrupter = std::thread::spawn(move || {
        while active.load(std::sync::atomic::Ordering::Acquire) {
            interrupt_control.interrupt();
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::thread::yield_now();
        }
    });
    while interrupt_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
        std::thread::yield_now();
    }
    let before_shutdown = interrupt_count.load(std::sync::atomic::Ordering::Acquire);
    let started = Instant::now();
    daemon
        .shutdown(Some(session_id), 4)
        .expect("bounded shutdown while interrupted");
    let elapsed = started.elapsed();
    shutdown_active.store(false, std::sync::atomic::Ordering::Release);
    interrupter.join().expect("interrupter");

    assert!(
        interrupt_count.load(std::sync::atomic::Ordering::Acquire) > before_shutdown,
        "the control thread must raise an interrupt during shutdown"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "shutdown spun: {elapsed:?}"
    );
    let frames = adapter.snapshot_delivered_frame_bytes();
    assert!(
        frames
            .iter()
            .any(|bytes| String::from_utf8_lossy(bytes).contains("ZmluYWw=")),
        "shutdown must deliver the final terminal output: {frames:?}"
    );
    assert!(
        frames.iter().any(|bytes| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .is_some_and(|value| {
                    value.get("type").and_then(|field| field.as_str()) == Some("process_exit")
                })
        }),
        "shutdown must deliver the process exit: {frames:?}"
    );
}

#[cfg(unix)]
#[test]
fn sustained_worker_and_adapter_producers_still_reach_shutdown_bound() {
    let data_dir = temp_data_dir("bounded-live-stop");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("bounded-live-stop-session".into());
    let client_id = ClientId("bounded-live-stop-client".into());
    let subscription_id = SubscriptionId("bounded-live-stop-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "while :; do printf x; sleep 0.01; done".into();
    daemon.spawn(request, 1).expect("spawn live producer");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind");

    let control = daemon.wake_pump_control();
    let producer_control = control.clone();
    let producer_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let producer_flag = std::sync::Arc::clone(&producer_stop);
    let producer_adapter = adapter.clone();
    let producer = std::thread::spawn(move || {
        while !producer_flag.load(std::sync::atomic::Ordering::Acquire) {
            let _ = producer_adapter.wake(TerminalWakeKind::Writable);
            producer_control.interrupt();
            std::hint::spin_loop();
        }
    });

    control.request_stop();
    let mut post_stop_wakes = 0;
    if let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::ZERO) {
        post_stop_wakes += 1;
        daemon.pump_woken(&batch, 3).expect("collision pump");
    }
    let refill_deadline = Instant::now() + Duration::from_secs(1);
    while daemon.wake_source().occupancy() == 0 {
        assert!(
            Instant::now() < refill_deadline,
            "live producers did not refill the wake channel"
        );
        std::thread::yield_now();
    }
    assert!(matches!(
        daemon.wait_pump(Duration::from_secs(1)),
        WakePumpWait::Stopped
    ));
    assert!(post_stop_wakes <= 1);

    let shutdown_started = Instant::now();
    daemon.shutdown(None, 4).expect("bounded live shutdown");
    assert!(shutdown_started.elapsed() < Duration::from_secs(3));
    producer_stop.store(true, std::sync::atomic::Ordering::Release);
    producer.join().expect("adapter producer");
}

#[cfg(unix)]
#[test]
fn pump_woken_applies_named_duplex_input_through_the_pty_once() {
    let data_dir = temp_data_dir("pump-input");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("pump-input-session".into());
    let client_id = ClientId("pump-input-client".into());
    let subscription_id = SubscriptionId("pump-input-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done".into();
    daemon.spawn(request, 1).expect("spawn");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    adapter.inject_ingress_frame(compact_input_frame(b"WAKE-INPUT\n"));
    let first = daemon.wait_wakes(Duration::from_secs(1));
    assert!(first.adapter_routes.iter().any(|route| {
        route.session_id == session_id && route.subscription_id == subscription_id
    }));
    daemon.pump_woken(&first, 3).expect("apply input wake");

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon.pump_woken(&batch, 4).expect("pump PTY echo");
        let delivered = adapter.snapshot_delivered_frame_bytes();
        let input_results = delivered
            .iter()
            .filter(|bytes| {
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("type")
                            .and_then(|kind| kind.as_str())
                            .map(str::to_owned)
                    })
                    .as_deref()
                    == Some("input_result")
            })
            .count();
        let echoes = delivered
            .iter()
            .filter(|bytes| {
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("payload_base64")
                            .and_then(|payload| payload.as_str())
                            .map(str::to_owned)
                    })
                    .is_some_and(|payload| {
                        matches!(
                            payload.as_str(),
                            "ZWNobzpXQUtFLUlOUFVUDQo=" | "V0FLRS1JTlBVVA0KZWNobzpXQUtFLUlOUFVUDQo="
                        )
                    })
            })
            .count();
        if input_results == 1 && echoes == 1 {
            let _ = fs::remove_dir_all(data_dir);
            return;
        }
    }
    panic!(
        "targeted pump must deliver one result and one PTY echo: {:?}",
        adapter.snapshot_delivered_frame_bytes()
    );
}

#[cfg(unix)]
#[test]
fn pump_woken_applies_authoritative_gated_input_and_clears_the_wait() {
    let data_dir = temp_data_dir("pump-gated");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("pump-gated-session".into());
    let client_id = ClientId("pump-gated-client".into());
    let subscription_id = SubscriptionId("pump-gated-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done".into();
    daemon.spawn(request, 1).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    let started = Instant::now();
    let mut probe = 0;
    let token = loop {
        probe += 1;
        daemon
            .drain(&session_id, 2 + probe)
            .expect("drain attach boundary");
        if let Ok(result) = daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId(format!("pump-gated-modes-{probe}")),
            session_id: session_id.clone(),
            now_seconds: 20 + probe,
        }) {
            break result.mode_flags.mode_freshness;
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "worker mode authority did not become ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    for (index, input) in [b"GATED-ONE\n".as_slice(), b"GATED-TWO\n".as_slice()]
        .into_iter()
        .enumerate()
    {
        adapter.inject_ingress_frame(compact_mode_gated_frame(
            token.mode_generation,
            token.mode_revision,
            input,
        ));
        let route_batch = daemon.wait_wakes(Duration::from_secs(1));
        daemon
            .pump_woken(&route_batch, 5 + index as u64)
            .expect("submit gated input");
        let started = Instant::now();
        while delivered_admitted_input_results(&adapter, "mode_gated_input").len() < index + 1 {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "gated input did not complete"
            );
            let batch = daemon.wait_wakes(Duration::from_millis(100));
            daemon
                .pump_woken(&batch, 10 + index as u64)
                .expect("complete gated input");
        }
    }
    let results = delivered_admitted_input_results(&adapter, "mode_gated_input");
    assert_eq!(results.len(), 2, "each gated input must complete once");
    assert_eq!(
        results
            .iter()
            .map(|result| result["bytes_written"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(10), Some(10)]
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_worker_resize_updates_live_pty_registry_and_one_patch() {
    let data_dir = temp_data_dir("pump-result-egress");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(16),
    );
    let session_id = SessionId("pump-result-session".into());
    let client_id = ClientId("pump-result-client".into());
    let subscription_id = SubscriptionId("pump-result-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "stty -echo; printf ready; while IFS= read -r _; do stty size; done".into();
    daemon.spawn(request, 1).expect("spawn");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    let attach_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < attach_deadline,
            "worker attach did not finish"
        );
        daemon.drain(&session_id, 2).expect("drain attach boundary");
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
            .any(|value| {
                value.get("type").and_then(serde_json::Value::as_str) == Some("attach_state")
                    && value.get("state").and_then(serde_json::Value::as_str) == Some("attached")
            })
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let before_resize = daemon.lifecycle_baseline().expect("baseline").cursor;

    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("resize apply tick");
    assert_eq!(delivered_input_result_count(&adapter, "resize"), 1);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("registry load")
        .expect("registry record");
    assert_eq!((record.rows, record.cols), (31, 91));

    adapter.inject_ingress_frame(compact_input_frame(b"report-size\n"));
    let input_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&input_batch, 4)
        .expect("size request apply tick");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "worker did not report live PTY size"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon.pump_woken(&batch, 5).expect("pump size report");
        if adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .any(|bytes| String::from_utf8_lossy(bytes).contains("MzEgOTENCg=="))
        {
            break;
        }
    }

    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    let repeated_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&repeated_batch, 6)
        .expect("identical resize apply tick");
    assert_eq!(delivered_input_result_count(&adapter, "resize"), 3);
    let resize_changes = daemon
        .lifecycle_changes_page(&before_resize, 16, 64 * 1024)
        .expect("resize journal page")
        .changes
        .into_iter()
        .filter(|change| {
            matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.size.rows == 31
                        && record.session.size.cols == 91
            )
        })
        .count();
    assert_eq!(
        resize_changes, 1,
        "identical resize must not append a patch"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_worker_resize_isolates_the_named_sibling() {
    let data_dir = temp_data_dir("pump-resize-sibling");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(32),
    );
    let (session_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "resize-sibling-a");
    let (session_b, adapter_b) = bind_size_reporting_worker(&mut daemon, "resize-sibling-b");
    let before_resize = daemon.lifecycle_baseline().expect("baseline").cursor;

    adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("resize named worker");
    assert_eq!(delivered_input_result_count(&adapter_a, "resize"), 1);
    assert_eq!(delivered_input_result_count(&adapter_b, "resize"), 0);

    let record_a = daemon
        .registry()
        .load(&session_a)
        .expect("load A")
        .expect("record A");
    let record_b = daemon
        .registry()
        .load(&session_b)
        .expect("load B")
        .expect("record B");
    assert_eq!((record_a.rows, record_a.cols), (31, 101));
    assert_eq!((record_b.rows, record_b.cols), (24, 80));

    adapter_a.inject_ingress_frame(compact_input_frame(b"report-a\n"));
    let input_a = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&input_a, 4).expect("request A size");
    pump_until_encoded_output(&mut daemon, &adapter_a, "MzEgMTAxDQo=", 5);
    adapter_b.inject_ingress_frame(compact_input_frame(b"report-b\n"));
    let input_b = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&input_b, 6).expect("request B size");
    pump_until_encoded_output(&mut daemon, &adapter_b, "MjQgODANCg==", 7);

    let changes = daemon
        .lifecycle_changes_page(&before_resize, 32, 64 * 1024)
        .expect("resize journal page");
    assert_eq!(
        changes
            .changes
            .iter()
            .filter(|change| matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_a
                        && record.session.size.rows == 31
                        && record.session.size.cols == 101
            ))
            .count(),
        1
    );
    assert!(changes.changes.iter().all(|change| !matches!(
        &change.kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_b
                && (record.session.size.rows != 24 || record.session.size.cols != 80)
    )));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn stalled_resize_acknowledgment_does_not_block_a_later_named_sibling() {
    let data_dir = temp_data_dir("pump-resize-stalled-sibling");
    let acknowledgment_timeout = Duration::from_secs(1);
    let mut config = CoreDaemonConfig::new(&data_dir)
        .with_worker_path(worker_path())
        .with_mode_gated_input_timeout(acknowledgment_timeout);
    config.test_omit_resize_applied = true;
    let mut daemon = CoreDaemon::new(config);
    let (session_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-stalled-resize");
    let (session_b, adapter_b) = bind_size_reporting_worker(&mut daemon, "z-live-input");

    adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    adapter_b.inject_ingress_frame(compact_input_frame(b"report-b\n"));
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(batch.adapter_routes.len(), 2);

    let observer_started = Instant::now();
    let observed_adapter = adapter_b.clone();
    let (observed_tx, observed_rx) = mpsc::sync_channel(1);
    let observer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            assert!(Instant::now() < deadline, "sibling input was not delivered");
            if delivered_input_result_count(&observed_adapter, "input") == 1 {
                observed_tx
                    .send(observer_started.elapsed())
                    .expect("send sibling delivery time");
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let error = daemon
        .pump_woken(&batch, 3)
        .expect_err("missing resize acknowledgment must fail");
    let sibling_delivery_elapsed = observed_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("receive sibling delivery time");
    observer.join().expect("sibling delivery observer");
    assert!(
        error
            .to_string()
            .contains("resize acknowledgment timed out"),
        "unexpected error: {error}"
    );
    assert!(
        sibling_delivery_elapsed < acknowledgment_timeout,
        "sibling input arrived after the resize acknowledgment wait: {sibling_delivery_elapsed:?}"
    );
    assert_eq!(delivered_input_result_count(&adapter_a, "resize"), 1);
    assert_eq!(delivered_input_result_count(&adapter_b, "input"), 1);

    let sibling_wake = daemon.wait_wakes(Duration::from_secs(1));
    assert!(sibling_wake.ingress_sessions.contains(&session_b));
    let stalled_record = daemon
        .registry()
        .load(&session_a)
        .expect("load stalled record")
        .expect("stalled record");
    assert_eq!((stalled_record.rows, stalled_record.cols), (24, 80));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_delivers_resize_and_rejected_gated_results_on_the_apply_tick() {
    let data_dir = temp_data_dir("pump-result-egress");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("pump-result-session".into());
    let client_id = ClientId("pump-result-client".into());
    let subscription_id = SubscriptionId("pump-result-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "stty -echo; while IFS= read -r _; do :; done".into();
    daemon.spawn(request, 1).expect("spawn");
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
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("resize apply tick");
    assert_eq!(delivered_input_result_count(&adapter, "resize"), 1);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load local resize record")
        .expect("local resize record");
    assert_eq!((record.rows, record.cols), (31, 91));

    adapter.inject_ingress_frame(compact_mode_gated_frame(0, 0, b"rejected"));
    let gated_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&gated_batch, 4)
        .expect("gated rejection tick");
    assert_eq!(
        delivered_input_result_count(&adapter, "mode_gated_input"),
        1
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn attach_without_waking_bind_allocates_no_registry_entry() {
    let data_dir = temp_data_dir("unbound");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("unbound-session".into());
    let client_id = ClientId("unbound-client".into());
    let subscription_id = SubscriptionId("unbound-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(client_id, session_id.clone(), subscription_id.clone(), 2)
        .expect("attach");
    assert!(
        !daemon
            .wake_source()
            .registry_contains(&session_id, &subscription_id),
        "attached-but-unbound routes must not enter the waking-adapter registry"
    );
    assert_eq!(
        daemon.wake_source().registry_len(),
        0,
        "attach must not allocate RouteWakeState"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn bind_rejection_allocates_nothing() {
    let data_dir = temp_data_dir("reject");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("reject-session".into());
    let client_id = ClientId("reject-client".into());
    let subscription_id = SubscriptionId("reject-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    let before = daemon.wake_source().registry_len();
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let err = daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            botster_core_daemon::TerminalSubscriptionGeneration(1),
            empty_caps(),
            Box::new(adapter),
        )
        .expect_err("bind before attach");
    let _ = err;
    assert_eq!(daemon.wake_source().registry_len(), before);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn late_spawn_and_waking_bind_after_shutdown_allocate_no_core_state() {
    let data_dir = temp_data_dir("late-after-shutdown");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon.shutdown(None, 1).expect("shutdown");
    let before_routes = daemon.wake_source().registry_len();
    let before_sessions = daemon.wake_source().session_registry_len();
    let session_id = SessionId("late-after-shutdown-session".into());
    assert!(matches!(
        daemon.spawn(spawn_request(&session_id), 2),
        Err(CoreDaemonError::Shutdown)
    ));
    assert!(matches!(
        daemon.bind_waking_terminal_adapter(
            ClientId("late-client".into()),
            session_id,
            SubscriptionId("late-sub".into()),
            botster_core_daemon::TerminalSubscriptionGeneration(1),
            empty_caps(),
            Box::new(SharedFakeTerminalAdapter::auto_complete()),
        ),
        Err(CoreDaemonError::Shutdown)
    ));
    assert_eq!(daemon.wake_source().registry_len(), before_routes);
    assert_eq!(daemon.wake_source().session_registry_len(), before_sessions);
}

#[test]
fn waking_bind_then_writable_wake_pumps_one_route() {
    let data_dir = temp_data_dir("pump-one");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("pump-session".into());
    let client_id = ClientId("pump-client".into());
    let subscription_id = SubscriptionId("pump-sub".into());
    let other_sub = SubscriptionId("other-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    daemon
        .attach(
            ClientId("other-client".into()),
            session_id.clone(),
            other_sub,
            3,
        )
        .expect("other attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter),
        )
        .expect("bind");
    assert_eq!(daemon.wake_source().registry_len(), 1);
    assert!(daemon
        .wake_source()
        .registry_contains(&session_id, &subscription_id));
    let _ = daemon.wait_wakes(Duration::from_millis(0));
    let batch = daemon.wait_wakes(Duration::from_millis(50));
    let outcome = daemon.pump_woken(&batch, 4).expect("pump");
    assert!(
        outcome.pumped_routes <= 1,
        "one waking bind must name at most one adapter route, got {}",
        outcome.pumped_routes
    );
    daemon
        .detach_terminal_subscription(
            ClientId("pump-client".into()),
            session_id,
            subscription_id,
            generation,
            5,
        )
        .ok();
    assert_eq!(daemon.wake_source().registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn readback_does_not_advance_bound_adapter() {
    let data_dir = temp_data_dir("readback");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("readback-session".into());
    let client_id = ClientId("readback-client".into());
    let subscription_id = SubscriptionId("readback-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let before = adapter.snapshot_delivered_frame_bytes().len();
    let _ = daemon.read_screen(ReadScreenRequest {
        request_id: RequestId("screen".into()),
        session_id: session_id.clone(),
        now_seconds: 3,
    });
    let _ = daemon.read_mode_flags(ReadModeFlagsRequest {
        request_id: RequestId("modes".into()),
        session_id: session_id.clone(),
        now_seconds: 4,
    });
    let _ = daemon.capture_snapshot(CaptureSnapshotRequest {
        request_id: RequestId("snap".into()),
        session_id: session_id.clone(),
        now_seconds: 5,
    });
    let _ = daemon.capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
        request_id: RequestId("color".into()),
        session_id: session_id.clone(),
        now_seconds: 6,
    });
    assert_eq!(adapter.snapshot_delivered_frame_bytes().len(), before);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn retained_sink_clone_does_not_pin_allocation() {
    let source = botster_core::TerminalWakeSource::new();
    let session = SessionId("retain".into());
    let sub = SubscriptionId("sub".into());
    let sink = source.bind_route(
        session.clone(),
        sub.clone(),
        botster_core::TerminalSubscriptionGeneration(1),
    );
    let clone = sink.clone();
    assert!(sink.wake(TerminalWakeKind::Writable));
    source.retire_route(&session, &sub);
    let _ = source.wait_wakes(Duration::from_millis(0));
    assert_eq!(source.registry_len(), 0);
    assert_eq!(clone.strong_count(), 0);
    assert!(!clone.wake(TerminalWakeKind::Writable));
    assert!(source.live_allocation_bound() <= WAKE_QUEUE_CAPACITY);
}

#[test]
fn overflow_reconcile_visits_only_registry() {
    let source = botster_core::TerminalWakeSource::new();
    let mut sinks = Vec::new();
    for n in 0..=WAKE_QUEUE_CAPACITY {
        let sink = source.bind_route(
            SessionId(format!("s{n}")),
            SubscriptionId(format!("sub{n}")),
            botster_core::TerminalSubscriptionGeneration(1),
        );
        let _ = sink.wake(TerminalWakeKind::Writable);
        sinks.push(sink);
    }
    let before = source.visit_count();
    let _ = source.wait_wakes(Duration::from_millis(0));
    let visits = source.visit_count().saturating_sub(before);
    assert!(visits <= source.registry_len() + WAKE_QUEUE_CAPACITY + 1);
    drop(sinks);
}

fn bind_probe(
    daemon: &mut CoreDaemon,
    session: &str,
    client: &str,
    sub: &str,
    adapter: SharedFakeTerminalAdapter,
) -> (
    SessionId,
    ClientId,
    SubscriptionId,
    botster_core_daemon::TerminalSubscriptionGeneration,
) {
    let session_id = SessionId(session.into());
    let client_id = ClientId(client.into());
    let subscription_id = SubscriptionId(sub.into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("row")
        .generation;
    daemon
        .bind_waking_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter),
        )
        .expect("bind");
    (session_id, client_id, subscription_id, generation)
}

#[test]
fn pump_woken_does_not_try_read_unrelated_adapter() {
    let data_dir = temp_data_dir("two-session");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let woken = SharedFakeTerminalAdapter::new();
    let sibling = SharedFakeTerminalAdapter::new();
    let (session_1, _, sub_1, _) =
        bind_probe(&mut daemon, "session-1", "client-1", "sub-1", woken.clone());
    let _ = bind_probe(
        &mut daemon,
        "session-2",
        "client-2",
        "sub-2",
        sibling.clone(),
    );
    let _ = daemon.wait_wakes(Duration::from_millis(0));
    let sibling_reads_before = sibling.try_read_count();
    assert!(woken.wake(TerminalWakeKind::Writable));
    let batch = daemon.wait_wakes(Duration::from_millis(0));
    assert_eq!(
        batch
            .adapter_routes
            .iter()
            .filter(|route| route.session_id == session_1 && route.subscription_id == sub_1)
            .count(),
        1
    );
    daemon.pump_woken(&batch, 10).expect("pump");
    assert_eq!(
        sibling.try_read_count(),
        sibling_reads_before,
        "unrelated adapter must not receive try_read"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn ingress_only_wake_does_not_apply_sibling_route_input() {
    let data_dir = temp_data_dir("ingress-route-isolation");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("ingress-route-session".into());
    let first_client = ClientId("ingress-route-first-client".into());
    let sibling_client = ClientId("ingress-route-sibling-client".into());
    let first_sub = SubscriptionId("ingress-route-first-sub".into());
    let sibling_sub = SubscriptionId("ingress-route-sibling-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    for (client, subscription) in [
        (first_client.clone(), first_sub.clone()),
        (sibling_client.clone(), sibling_sub.clone()),
    ] {
        daemon
            .attach(client, session_id.clone(), subscription, 2)
            .expect("attach route");
    }
    let first = SharedFakeTerminalAdapter::auto_complete();
    let sibling = SharedFakeTerminalAdapter::auto_complete();
    for (client, subscription, adapter) in [
        (first_client, first_sub, first.clone()),
        (sibling_client, sibling_sub.clone(), sibling.clone()),
    ] {
        let generation = daemon
            .list_terminal_subscriptions()
            .into_iter()
            .find(|row| row.subscription_id == subscription)
            .expect("inventory")
            .generation;
        daemon
            .bind_waking_terminal_adapter(
                client,
                session_id.clone(),
                subscription,
                generation,
                empty_caps(),
                Box::new(adapter),
            )
            .expect("bind route");
    }
    sibling.inject_ingress_frame(compact_input_frame(b"MUST-STAY-QUEUED\n"));
    let reads_before = sibling.try_read_count();
    daemon
        .pump_woken(
            &TerminalWakeBatch {
                adapter_routes: Vec::new(),
                ingress_sessions: vec![session_id],
            },
            3,
        )
        .expect("ingress-only pump");
    assert_eq!(
        sibling.try_read_count(),
        reads_before,
        "session ingress must not intake a sibling adapter route"
    );
    assert_eq!(delivered_input_result_count(&sibling, "input"), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn spurious_writable_wakes_hard_stop_one_route() {
    let data_dir = temp_data_dir("spurious");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let mut blocked = SharedFakeTerminalAdapter::new();
    blocked.force_would_block();
    let sibling = SharedFakeTerminalAdapter::auto_complete();
    let (session, client, sub, generation) = bind_probe(
        &mut daemon,
        "blocked-session",
        "blocked-client",
        "blocked-sub",
        blocked.clone(),
    );
    let (sibling_session, _, sibling_sub, _) = bind_probe(
        &mut daemon,
        "ok-session",
        "ok-client",
        "ok-sub",
        sibling.clone(),
    );
    let _ = daemon.wait_wakes(Duration::from_millis(0));
    for tick in 0..512 {
        let _ = blocked.wake(TerminalWakeKind::Writable);
        let batch = daemon.wait_wakes(Duration::from_millis(0));
        let _ = daemon.pump_woken(&batch, 20 + tick);
    }
    assert!(
        !daemon
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.session_id == session && row.subscription_id == sub),
        "512 rejected Writable pumps must UnsubscribeSession the blocked route"
    );
    assert!(
        daemon
            .list_terminal_subscriptions()
            .iter()
            .any(|row| row.session_id == sibling_session && row.subscription_id == sibling_sub),
        "sibling must survive the spurious-wake hard-stop"
    );
    let _ = (client, generation);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn ingress_overflow_then_bind_still_recovers() {
    let source = botster_core::TerminalWakeSource::new();
    let mut sinks = Vec::new();
    for n in 0..WAKE_QUEUE_CAPACITY {
        let sink = source.bind_route(
            SessionId(format!("cap{n}")),
            SubscriptionId(format!("sub{n}")),
            botster_core::TerminalSubscriptionGeneration(1),
        );
        assert!(sink.wake(TerminalWakeKind::Writable));
        sinks.push(sink);
    }
    let late = SessionId("late-ingress".into());
    let handle = source.session_handle(late.clone());
    handle.notify();
    let sink = source.bind_route(
        late.clone(),
        SubscriptionId("later-sub".into()),
        botster_core::TerminalSubscriptionGeneration(1),
    );
    let batch = source.wait_wakes(Duration::from_millis(0));
    assert!(
        batch.ingress_sessions.contains(&late),
        "bind after overflow must not drop the ingress-only session"
    );
    drop(sink);
    drop(sinks);
}

#[test]
fn public_ingress_overflow_does_not_fabricate_idle_adapter_route() {
    let data_dir = temp_data_dir("idle-overflow");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let source = daemon.wake_source().clone();
    let idle_session = SessionId("idle-route".into());
    let idle_sub = SubscriptionId("idle-sub".into());
    let idle = source.bind_route(
        idle_session.clone(),
        idle_sub.clone(),
        botster_core::TerminalSubscriptionGeneration(1),
    );
    let mut handles = Vec::new();
    for n in 0..=WAKE_QUEUE_CAPACITY {
        let handle = source.session_handle(SessionId(format!("ingress{n}")));
        handle.notify();
        handles.push(handle);
    }
    let batch = daemon.wait_wakes(Duration::from_millis(0));
    assert!(
        !batch
            .adapter_routes
            .iter()
            .any(|route| route.session_id == idle_session && route.subscription_id == idle_sub),
        "ingress-only overflow must not name an idle adapter route"
    );
    assert_eq!(batch.ingress_sessions.len(), WAKE_QUEUE_CAPACITY + 1);
    drop(idle);
    drop(handles);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn public_occupancy_is_exact_after_quiesce() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    let data_dir = temp_data_dir("occupancy-quiesce");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let source = daemon.wake_source().clone();
    let idle = source.bind_route(
        SessionId("idle-bound".into()),
        SubscriptionId("idle-sub".into()),
        botster_core::TerminalSubscriptionGeneration(1),
    );
    let handle = source.session_handle(SessionId("quiesce".into()));
    let stop = Arc::new(AtomicBool::new(false));
    let drain_source = source.clone();
    let drain_stop = Arc::clone(&stop);
    let drainer = thread::spawn(move || {
        let mut worst = 0usize;
        while !drain_stop.load(Ordering::Relaxed) {
            let _ = drain_source.wait_wakes(Duration::from_millis(1));
            let seen = drain_source.occupancy();
            if seen > worst {
                worst = seen;
            }
        }
        worst
    });
    let deadline = Instant::now() + Duration::from_millis(400);
    let mut producer_worst = 0usize;
    while Instant::now() < deadline {
        handle.notify();
        let seen = source.occupancy();
        if seen > producer_worst {
            producer_worst = seen;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let drain_worst = drainer.join().expect("drain thread");
    assert!(
        producer_worst <= WAKE_QUEUE_CAPACITY && drain_worst <= WAKE_QUEUE_CAPACITY,
        "occupancy wrapped or exceeded the channel: producer_worst={producer_worst} drain_worst={drain_worst}"
    );
    for _ in 0..64 {
        let batch = daemon.wait_wakes(Duration::from_millis(0));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            break;
        }
    }
    assert_eq!(
        source.occupancy(),
        0,
        "occupancy must be exact after producers stop and the channel is drained"
    );
    assert_eq!(
        source.live_allocation_bound(),
        source.registry_len(),
        "live allocation bound must equal registry size when occupancy is zero"
    );
    drop(idle);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn public_session_wakes_coalesce_by_session() {
    let data_dir = temp_data_dir("coalesce");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let source = daemon.wake_source().clone();
    let session = SessionId("coalesce".into());
    let first = source.session_handle(session.clone());
    let second = source.session_handle(session.clone());
    first.notify();
    second.notify();
    source.notify_session(&session);
    source.notify_session(&session);
    assert_eq!(source.occupancy(), 1);
    assert_eq!(source.session_registry_len(), 1);
    let batch = daemon.wait_wakes(Duration::from_millis(0));
    assert_eq!(batch.ingress_sessions, vec![session]);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn public_forget_session_retires_retained_handle() {
    let data_dir = temp_data_dir("late-forget");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let source = daemon.wake_source().clone();
    let mut sinks = Vec::new();
    for n in 0..WAKE_QUEUE_CAPACITY {
        let sink = source.bind_route(
            SessionId(format!("cap{n}")),
            SubscriptionId(format!("sub{n}")),
            botster_core::TerminalSubscriptionGeneration(1),
        );
        assert!(sink.wake(TerminalWakeKind::Writable));
        sinks.push(sink);
    }
    let session = SessionId("doomed".into());
    let handle = source.session_handle(session.clone());
    handle.notify();
    assert_eq!(source.ingress_overflow_len(), 1);
    source.forget_session(&session);
    handle.notify();
    source.notify_session(&session);
    assert_eq!(source.ingress_overflow_len(), 0);
    let batch = daemon.wait_wakes(Duration::from_millis(0));
    assert!(
        !batch.ingress_sessions.contains(&session),
        "a retained reader handle must not resurrect a forgotten SessionId"
    );
    drop(sinks);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn public_overflow_wait_does_not_depend_on_timeout() {
    let data_dir = temp_data_dir("overflow-wait");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let source = daemon.wake_source().clone();
    let mut sinks = Vec::new();
    for n in 0..WAKE_QUEUE_CAPACITY {
        let sink = source.bind_route(
            SessionId(format!("cap{n}")),
            SubscriptionId(format!("sub{n}")),
            botster_core::TerminalSubscriptionGeneration(1),
        );
        assert!(sink.wake(TerminalWakeKind::Writable));
        sinks.push(sink);
    }
    let session = SessionId("overflow-ingress".into());
    let handle = source.session_handle(session.clone());
    handle.notify();
    let started = Instant::now();
    let batch = daemon.wait_wakes(Duration::from_secs(5));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "a full ready channel plus overflow must not wait out the timeout"
    );
    assert!(batch.ingress_sessions.contains(&session));
    drop(sinks);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn shutdown_completion_arrives_through_wait_wakes() {
    let data_dir = temp_data_dir("shutdown-wake");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("shutdown-wake-session".into());
    daemon
        .spawn(
            SpawnSessionRequest {
                request: SessionSpawnRequest {
                    request_id: RequestId("shutdown-wake-spawn".into()),
                    session_id: session_id.clone(),
                    executable: "sh".to_string(),
                    arguments: vec!["-c".to_string(), "printf FINAL; exec sleep 30".to_string()],
                    working_directory: SpawnWorkingDirectory {
                        path: ".".to_string(),
                    },
                    environment: SpawnEnvironment::default(),
                    initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                },
                metadata: CoreSessionMetadata::new(),
            },
            1,
        )
        .expect("spawn");
    let source = daemon.wake_source().clone();
    assert_eq!(source.session_registry_len(), 1);
    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 3)
        .expect("shutdown through wait_wakes");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "shutdown must complete from wakes, not the watchdog timeout"
    );
    assert_eq!(source.session_registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

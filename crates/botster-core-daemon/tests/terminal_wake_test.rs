#![allow(missing_docs)]

use std::any::Any;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::process::Command;
use std::sync::{mpsc, Once};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::engine::managed_session_runtime::PENDING_INGRESS_RESIZE_CAP;
use botster_core::runtime::{ControlPlaneState, ControlWriterError};
use botster_core::terminal_adapter::TerminalAdapterPressure;
use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet,
    TerminalWakeBatch, TerminalWakeKind, WAKE_QUEUE_CAPACITY,
};
use botster_core_daemon::{
    CaptureColorAndSnapshotRequest, CaptureSnapshotRequest, CoreDaemon, CoreDaemonConfig,
    CoreDaemonError, LifecycleBaselineBudget, ObserveLifecycleBudget, ReadModeFlagsRequest,
    ReadScreenRequest, RegistrySessionState, ResizeAckHold, SessionLifecycleChangeKind,
    SessionLifecycleLookup, SessionRegistryStateLookup, SpawnSessionRequest, WakePumpControl,
    WakePumpError, WakePumpWait,
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

fn pump_next(daemon: &mut CoreDaemon, now_seconds: u64) {
    let batch = daemon.wait_wakes(Duration::from_secs(5));
    assert!(
        !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty(),
        "targeted progress requires a wake"
    );
    daemon
        .pump_woken(&batch, now_seconds)
        .expect("targeted wake pump");
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
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "worker attach did not finish");
        pump_next(daemon, 2);
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
fn wait_session_ingress_wake(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    tick: u64,
) -> TerminalWakeBatch {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        assert!(
            Instant::now() < deadline,
            "missing resize-completion session wake"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(20));
        if batch.ingress_sessions.contains(session_id) {
            return batch;
        }
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            continue;
        }
        daemon
            .pump_woken(&batch, tick)
            .expect("pump leftover wake while waiting for resize completion");
    }
}

fn pump_until_registry_size(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    rows: u16,
    cols: u16,
    tick: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let record = daemon
            .registry()
            .load(session_id)
            .expect("registry load")
            .expect("registry record");
        if record.rows == rows && record.cols == cols {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "registry geometry did not follow acknowledgement"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, tick)
            .expect("pump resize completion");
    }
}

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

fn compact_paste_frames(
    operation_id: u32,
    mode_generation: u64,
    mode_revision: u64,
    data: &[u8],
) -> Vec<Vec<u8>> {
    const CHUNK_BYTES: usize = 65_527;
    let mut begin = vec![1, 4, 0, 24];
    begin.extend_from_slice(&operation_id.to_be_bytes());
    begin.extend_from_slice(&mode_generation.to_be_bytes());
    begin.extend_from_slice(&mode_revision.to_be_bytes());
    begin.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut frames = vec![begin];
    for (index, data) in data.chunks(CHUNK_BYTES).enumerate() {
        let body_len = u16::try_from(8 + data.len()).expect("paste chunk body fits");
        let mut chunk = vec![1, 5];
        chunk.extend_from_slice(&body_len.to_be_bytes());
        chunk.extend_from_slice(&operation_id.to_be_bytes());
        chunk.extend_from_slice(&(index as u32).to_be_bytes());
        chunk.extend_from_slice(data);
        frames.push(chunk);
    }
    let mut commit = vec![1, 6, 0, 4];
    commit.extend_from_slice(&operation_id.to_be_bytes());
    frames.push(commit);
    frames
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

fn delivered_input_results(
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
        })
        .collect()
}

/// Decode standard base64 with `=` padding. Test-only: panics on bad input.
fn decode_base64(text: &str) -> Vec<u8> {
    fn sextet(byte: u8) -> u32 {
        match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            other => panic!("invalid base64 byte {other:#x}"),
        }
    }
    let raw = text.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(raw.len() * 3 / 4);
    for chunk in raw.chunks(4) {
        let mut accumulator = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            accumulator |= sextet(*byte) << (18 - 6 * index);
        }
        let produced = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => panic!("invalid base64 length"),
        };
        for index in 0..produced {
            out.push(((accumulator >> (16 - 8 * index)) & 0xff) as u8);
        }
    }
    out
}

/// All delivered `terminal_output` bytes, decoded and joined in delivery order.
/// Frame boundaries are not the contract, so callers match the joined bytes.
fn delivered_terminal_output_bytes(adapter: &SharedFakeTerminalAdapter) -> Vec<u8> {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .filter(|value| {
            value.get("type").and_then(|field| field.as_str()) == Some("terminal_output")
        })
        .filter_map(|value| {
            value
                .get("payload_base64")
                .and_then(serde_json::Value::as_str)
                .map(decode_base64)
        })
        .flatten()
        .collect()
}

/// True when the joined terminal output contains one complete line whose
/// trimmed text equals `expected`.
fn terminal_output_has_count_line(adapter: &SharedFakeTerminalAdapter, expected: &str) -> bool {
    let joined = delivered_terminal_output_bytes(adapter);
    String::from_utf8_lossy(&joined)
        .split('\n')
        .any(|line| line.trim() == expected)
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
    let ready_path = data_dir.join("shutdown-ready");
    let release_path = data_dir.join("shutdown-release");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "trap '' TERM; : > {}; while [ ! -f {} ]; do sleep 0.01; done; printf final",
        ready_path.display(),
        release_path.display()
    );
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
    let ready_deadline = Instant::now() + Duration::from_secs(15);
    while !ready_path.exists() {
        assert!(
            Instant::now() < ready_deadline,
            "shutdown fixture did not publish readiness"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let control = daemon.wake_pump_control();
    control.request_stop();
    match daemon.wait_pump(Duration::ZERO) {
        WakePumpWait::Wakes(batch) => {
            daemon
                .pump_woken(&batch, 3)
                .expect("pump the shutdown collision batch");
            assert!(matches!(
                daemon.wait_pump(Duration::ZERO),
                WakePumpWait::Stopped
            ));
        }
        WakePumpWait::Stopped => {}
        other => panic!("stop must return a collision batch or Stopped: {other:?}"),
    }

    let shutdown_active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let active = std::sync::Arc::clone(&shutdown_active);
    let interrupt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = std::sync::Arc::clone(&interrupt_count);
    let interrupt_control = control.clone();
    let release = release_path.clone();
    let interrupter = std::thread::spawn(move || {
        let release_at = Instant::now() + Duration::from_millis(50);
        let mut released = false;
        while active.load(std::sync::atomic::Ordering::Acquire) {
            interrupt_control.interrupt();
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if !released && Instant::now() >= release_at {
                fs::write(&release, b"release").expect("release shutdown fixture");
                released = true;
            }
            std::thread::yield_now();
        }
    });
    while interrupt_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
        std::thread::yield_now();
    }
    let before_shutdown = interrupt_count.load(std::sync::atomic::Ordering::Acquire);
    let started = Instant::now();
    let shutdown_result = daemon.shutdown(Some(session_id), 4);
    let elapsed = started.elapsed();
    shutdown_active.store(false, std::sync::atomic::Ordering::Release);
    let interrupter_result = interrupter.join();

    interrupter_result.expect("interrupter");
    shutdown_result.expect("bounded shutdown while interrupted");
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
                        && value.get("code").and_then(|field| field.as_i64()) == Some(0)
                })
        }),
        "shutdown must deliver a successful process exit: {frames:?}"
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
fn pump_woken_preserves_mixed_resize_and_input_with_same_session_sibling() {
    let data_dir = temp_data_dir("pump-mixed-resize-input");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("pump-mixed-resize-input-session".into());
    let owner_client = ClientId("pump-mixed-resize-input-owner-client".into());
    let owner_subscription = SubscriptionId("pump-mixed-resize-input-owner-sub".into());
    let sibling_client = ClientId("pump-mixed-resize-input-sibling-client".into());
    let sibling_subscription = SubscriptionId("pump-mixed-resize-input-sibling-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "stty -echo; printf ready; while IFS= read -r line; do if [ \"$line\" = REPORT-SIZE ]; then stty size; else printf 'echo:%s\n' \"$line\"; fi; done".into();
    daemon.spawn(request, 1).expect("spawn worker");

    for (client, subscription) in [
        (owner_client.clone(), owner_subscription.clone()),
        (sibling_client.clone(), sibling_subscription.clone()),
    ] {
        daemon
            .expect_terminal_adapter(client.clone(), session_id.clone(), subscription.clone())
            .expect("declare adapter");
        daemon
            .attach(client, session_id.clone(), subscription, 2)
            .expect("attach route");
    }

    let subscriptions = daemon.list_terminal_subscriptions();
    let owner_generation = subscriptions
        .iter()
        .find(|row| row.subscription_id == owner_subscription)
        .expect("owner subscription")
        .generation;
    let sibling_generation = subscriptions
        .iter()
        .find(|row| row.subscription_id == sibling_subscription)
        .expect("sibling subscription")
        .generation;
    let owner = SharedFakeTerminalAdapter::auto_complete();
    let sibling = SharedFakeTerminalAdapter::auto_complete();
    for (client, subscription, generation, adapter) in [
        (
            owner_client,
            owner_subscription.clone(),
            owner_generation,
            owner.clone(),
        ),
        (
            sibling_client,
            sibling_subscription.clone(),
            sibling_generation,
            sibling.clone(),
        ),
    ] {
        daemon
            .bind_waking_terminal_adapter(
                client,
                session_id.clone(),
                subscription,
                generation,
                empty_caps(),
                Box::new(adapter),
            )
            .expect("bind waking adapter");
    }

    let attach_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < attach_deadline,
            "same-session routes did not finish attaching"
        );
        pump_next(&mut daemon, 2);
        let attached = [&owner, &sibling].iter().all(|adapter| {
            adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
                .any(|value| {
                    value.get("type").and_then(serde_json::Value::as_str) == Some("attach_state")
                        && value.get("state").and_then(serde_json::Value::as_str)
                            == Some("attached")
                })
        });
        if attached {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let settled_batch = daemon.wait_wakes(Duration::ZERO);
    daemon
        .pump_woken(&settled_batch, 2)
        .expect("settle attach wakes");

    owner.inject_ingress_frame(compact_resize_frame(31, 91));
    owner.inject_ingress_frame(compact_input_frame(b"OWNER\n"));
    let mixed_batch = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(
        mixed_batch
            .adapter_routes
            .iter()
            .filter(|route| {
                route.session_id == session_id && route.subscription_id == owner_subscription
            })
            .count(),
        1,
        "back-to-back frames must share one coalesced route wake"
    );
    daemon
        .pump_woken(&mixed_batch, 3)
        .expect("apply mixed wake batch");

    let resize_results = delivered_admitted_input_results(&owner, "resize");
    let input_results = delivered_admitted_input_results(&owner, "input");
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for result in resize_results.iter().chain(&input_results) {
        assert_eq!(
            result["subscription_id"].as_str(),
            Some(owner_subscription.0.as_str()),
            "each result must identify the live owner"
        );
    }
    let completion = wait_session_ingress_wake(&mut daemon, &session_id, 3);
    daemon
        .pump_woken(&completion, 3)
        .expect("pump mixed-batch resize completion");
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker")
        .expect("worker registry record");
    assert_eq!((record.rows, record.cols), (31, 91));
    for (subscription, generation) in [
        (&owner_subscription, owner_generation),
        (&sibling_subscription, sibling_generation),
    ] {
        assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
            row.session_id == session_id
                && row.subscription_id == *subscription
                && row.generation == generation
        }));
    }

    pump_until_encoded_output(&mut daemon, &owner, "ZWNobzpPV05FUg0K", 4);
    owner.inject_ingress_frame(compact_input_frame(b"REPORT-SIZE\n"));
    let size_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&size_batch, 5)
        .expect("request worker size after mixed batch");
    pump_until_encoded_output(&mut daemon, &owner, "MzEgOTENCg==", 6);
    sibling.inject_ingress_frame(compact_input_frame(b"SIBLING\n"));
    let sibling_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&sibling_batch, 7)
        .expect("apply sibling input after mixed batch");
    assert_eq!(
        delivered_admitted_input_results(&sibling, "input")
            .iter()
            .map(|result| result["subscription_id"].as_str())
            .collect::<Vec<_>>(),
        vec![Some(sibling_subscription.0.as_str())]
    );
    pump_until_encoded_output(&mut daemon, &sibling, "ZWNobzpTSUJMSU5HDQo=", 8);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pump_woken_same_wake_resize_then_input_survives_resize_completion() {
    let data_dir = temp_data_dir("pump-resize-then-input-wake");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("pump-resize-then-input-wake-session".into());
    let client_id = ClientId("pump-resize-then-input-wake-client".into());
    let subscription_id = SubscriptionId("pump-resize-then-input-wake-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "stty -echo; printf ready; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
            .into();
    daemon.spawn(request, 1).expect("spawn worker");
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
        .expect("attach route");
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

    let attach_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < attach_deadline,
            "worker attach did not finish"
        );
        pump_next(&mut daemon, 2);
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
    }
    loop {
        let settled = daemon.wait_wakes(Duration::from_millis(50));
        if settled.adapter_routes.is_empty() && settled.ingress_sessions.is_empty() {
            break;
        }
        daemon.pump_woken(&settled, 2).expect("settle wakes");
    }
    assert_eq!(daemon.wake_source().occupancy(), 0);

    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    adapter.inject_ingress_frame(compact_input_frame(b"SCRATCH\n"));

    let mixed = daemon.wait_wakes(Duration::from_secs(5));
    assert_eq!(mixed.adapter_routes.len(), 1);
    assert_eq!(mixed.adapter_routes[0].session_id, session_id);
    assert_eq!(mixed.adapter_routes[0].subscription_id, subscription_id);
    assert!(mixed.ingress_sessions.is_empty());
    daemon.pump_woken(&mixed, 3).expect("pump mixed wake");

    assert_eq!(
        delivered_input_result_count(&adapter, "resize"),
        1,
        "resize must emit one total result"
    );
    assert_eq!(
        delivered_input_result_count(&adapter, "input"),
        1,
        "input must emit one total result"
    );
    let resize_results = delivered_admitted_input_results(&adapter, "resize");
    let input_results = delivered_admitted_input_results(&adapter, "input");
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for result in resize_results.iter().chain(&input_results) {
        assert_eq!(
            result["subscription_id"].as_str(),
            Some(subscription_id.0.as_str()),
            "each result must identify the live owner"
        );
    }
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker")
        .expect("worker registry record");
    assert_eq!(
        (record.rows, record.cols),
        (24, 80),
        "registry geometry follows the completion wake, not accept"
    );
    assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
        row.session_id == session_id
            && row.subscription_id == subscription_id
            && row.generation == generation
    }));

    let retained = wait_session_ingress_wake(&mut daemon, &session_id, 3);
    assert_eq!(retained.ingress_sessions, vec![session_id.clone()]);
    daemon
        .pump_woken(&retained, 4)
        .expect("pump retained resize-completion wake");
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker after completion")
        .expect("worker registry record after completion");
    assert_eq!((record.rows, record.cols), (31, 91));
    assert!(!adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .any(|value| value
            .get("payload_base64")
            .and_then(serde_json::Value::as_str)
            == Some("ZWNobzpTQ1JBVENIDQo=")));

    let echo = daemon.wait_wakes(Duration::from_secs(5));
    assert!(echo.adapter_routes.is_empty());
    assert_eq!(echo.ingress_sessions, vec![session_id.clone()]);
    daemon.pump_woken(&echo, 5).expect("pump worker echo wake");
    let exact_echoes = adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .filter(|value| {
            value
                .get("payload_base64")
                .and_then(serde_json::Value::as_str)
                == Some("ZWNobzpTQ1JBVENIDQo=")
        })
        .count();
    assert_eq!(exact_echoes, 1, "exact PTY echo must arrive once");
    assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
        row.session_id == session_id
            && row.subscription_id == subscription_id
            && row.generation == generation
    }));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn one_slot_adapter_preserves_resize_input_and_echo_wake_obligations() {
    let data_dir = temp_data_dir("one-slot-resize-input-wakes");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("one-slot-resize-input-wakes-session".into());
    let client_id = ClientId("one-slot-resize-input-wakes-client".into());
    let subscription_id = SubscriptionId("one-slot-resize-input-wakes-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "stty -echo; printf ready; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
            .into();
    daemon.spawn(request, 1).expect("spawn worker");
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
        .expect("attach route");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("subscription")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
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

    let attach_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < attach_deadline,
            "one-slot worker attach did not finish"
        );
        pump_next(&mut daemon, 2);
        adapter.complete_write();
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
    }

    let settle_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < settle_deadline,
            "one-slot attach wakes did not settle"
        );
        adapter.complete_write();
        let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::from_millis(50)) else {
            panic!("uncontrolled wake pump must return wakes");
        };
        if batch.adapter_routes.is_empty()
            && batch.ingress_sessions.is_empty()
            && adapter.snapshot_pressure() == TerminalAdapterPressure::Ready
            && daemon.wake_source().occupancy() == 0
        {
            break;
        }
        daemon.pump_woken(&batch, 2).expect("settle attach wakes");
    }

    adapter.inject_ingress_frame(compact_resize_frame(31, 91));
    adapter.inject_ingress_frame(compact_input_frame(b"SCRATCH\n"));

    let WakePumpWait::Wakes(mixed) = daemon.wait_pump(Duration::from_secs(5)) else {
        panic!("uncontrolled wake pump must return the mixed wake");
    };
    assert_eq!(mixed.adapter_routes.len(), 1);
    assert_eq!(mixed.adapter_routes[0].session_id, session_id);
    assert_eq!(mixed.adapter_routes[0].subscription_id, subscription_id);
    assert!(mixed.ingress_sessions.is_empty());
    daemon.pump_woken(&mixed, 3).expect("pump mixed wake");

    assert_eq!(
        adapter.snapshot_pressure(),
        TerminalAdapterPressure::Full,
        "the first input result must occupy the only output slot"
    );
    assert_eq!(delivered_input_result_count(&adapter, "resize"), 0);
    assert_eq!(delivered_input_result_count(&adapter, "input"), 0);
    assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
        row.session_id == session_id
            && row.subscription_id == subscription_id
            && row.generation == generation
    }));

    let completion_deadline = Instant::now() + Duration::from_secs(5);
    let mut retained_resize_wake_observed = false;
    while delivered_input_result_count(&adapter, "resize")
        + delivered_input_result_count(&adapter, "input")
        < 2
    {
        assert!(
            Instant::now() < completion_deadline,
            "one-slot input results did not complete"
        );
        adapter.complete_write();
        let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::from_secs(5)) else {
            panic!("uncontrolled wake pump must return a completion wake");
        };
        assert_eq!(batch.adapter_routes.len(), 1);
        assert_eq!(batch.adapter_routes[0].session_id, session_id);
        assert_eq!(batch.adapter_routes[0].subscription_id, subscription_id);
        assert!(
            batch
                .ingress_sessions
                .iter()
                .all(|session| session == &session_id),
            "completion can coalesce only with the named session"
        );
        daemon
            .pump_woken(&batch, 4)
            .expect("pump one-slot completion wake");
        if !batch.ingress_sessions.is_empty() {
            retained_resize_wake_observed = true;
            assert!(!adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
                .any(|value| value
                    .get("payload_base64")
                    .and_then(serde_json::Value::as_str)
                    == Some("ZWNobzpTQ1JBVENIDQo=")));
        }
        assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
            row.session_id == session_id
                && row.subscription_id == subscription_id
                && row.generation == generation
        }));
        assert_ne!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    }

    assert_eq!(delivered_input_result_count(&adapter, "resize"), 1);
    assert_eq!(delivered_input_result_count(&adapter, "input"), 1);
    let resize_results = delivered_admitted_input_results(&adapter, "resize");
    let input_results = delivered_admitted_input_results(&adapter, "input");
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for result in resize_results.iter().chain(&input_results) {
        assert_eq!(
            result["subscription_id"].as_str(),
            Some(subscription_id.0.as_str()),
            "each result must identify the live owner"
        );
    }

    if !retained_resize_wake_observed {
        let retained_deadline = Instant::now() + Duration::from_secs(1);
        let retained = loop {
            assert!(
                Instant::now() < retained_deadline,
                "missing one-slot resize-completion session wake"
            );
            let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::from_millis(20)) else {
                panic!("uncontrolled wake pump must return the retained wake");
            };
            if batch.ingress_sessions.contains(&session_id) {
                break batch;
            }
            assert!(
                batch.adapter_routes.is_empty(),
                "unexpected adapter wake while waiting for one-slot completion: {batch:?}"
            );
        };
        assert!(retained.adapter_routes.is_empty());
        assert_eq!(retained.ingress_sessions, vec![session_id.clone()]);
        daemon
            .pump_woken(&retained, 5)
            .expect("pump retained resize-completion wake");
        retained_resize_wake_observed = true;
        assert!(!adapter
            .snapshot_delivered_frame_bytes()
            .iter()
            .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
            .any(|value| value
                .get("payload_base64")
                .and_then(serde_json::Value::as_str)
                == Some("ZWNobzpTQ1JBVENIDQo=")));
    }
    assert!(retained_resize_wake_observed);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker")
        .expect("worker registry record");
    assert_eq!((record.rows, record.cols), (31, 91));

    let WakePumpWait::Wakes(echo) = daemon.wait_pump(Duration::from_secs(5)) else {
        panic!("uncontrolled wake pump must return the echo wake");
    };
    assert!(echo.adapter_routes.is_empty());
    assert_eq!(echo.ingress_sessions, vec![session_id.clone()]);
    daemon.pump_woken(&echo, 6).expect("pump worker echo wake");
    adapter.complete_write();

    let delivered = adapter.snapshot_delivered_frame_bytes();
    let exact_echoes = delivered
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .filter(|value| {
            value
                .get("payload_base64")
                .and_then(serde_json::Value::as_str)
                == Some("ZWNobzpTQ1JBVENIDQo=")
        })
        .count();
    assert_eq!(exact_echoes, 1, "exact PTY echo must arrive once");
    assert!(!delivered
        .iter()
        .any(|bytes| { String::from_utf8_lossy(bytes).contains("core_adapter_closed") }));
    assert_ne!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert!(daemon.list_terminal_subscriptions().iter().any(|row| {
        row.session_id == session_id
            && row.subscription_id == subscription_id
            && row.generation == generation
    }));

    let _ = fs::remove_dir_all(data_dir);
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
    for result in &results {
        assert_eq!(
            result["mode_generation"].as_u64(),
            Some(token.mode_generation),
            "admitted gated input must carry the worker's current generation: {result}"
        );
        assert_eq!(
            result["mode_revision"].as_u64(),
            Some(token.mode_revision),
            "admitted gated input must carry the worker's current revision: {result}"
        );
        assert!(
            result["mode_flags"].is_object(),
            "admitted gated input must carry the worker's mode flags: {result}"
        );
        assert!(
            result.get("rejection").is_none(),
            "admitted gated input must not carry a rejection: {result}"
        );
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn paste_above_one_frame_delivers_one_atomic_worker_write_and_result() {
    let data_dir = temp_data_dir("paste-above-frame");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("paste-above-frame-session".into());
    let client_id = ClientId("paste-above-frame-client".into());
    let subscription_id = SubscriptionId("paste-above-frame-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "stty raw -echo; printf '\\033[?2004hready'; dd bs=1 count=70012 2>/dev/null | wc -c; sleep 30"
            .into();
    daemon.spawn(request, 1).expect("spawn paste byte counter");
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
            .expect("drain attach and mode output");
        if let Ok(result) = daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId(format!("paste-modes-{probe}")),
            session_id: session_id.clone(),
            now_seconds: 20 + probe,
        }) {
            if result.mode_flags.mode_flags.bracketed_paste {
                break result.mode_flags.mode_freshness;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "bracketed-paste mode did not become ready"
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
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind waking adapter");

    let content = vec![b'A'; 70_000];
    for frame in compact_paste_frames(41, token.mode_generation, token.mode_revision, &content) {
        adapter.inject_ingress_frame(frame);
    }
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&batch, 30)
        .expect("assemble and submit paste");

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < deadline,
            "paste result and PTY count did not arrive: {:?}",
            adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .collect::<Vec<_>>()
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon.pump_woken(&batch, 31).expect("pump paste result");
        let delivered = adapter.snapshot_delivered_frame_bytes();
        let counted = delivered
            .iter()
            .any(|bytes| String::from_utf8_lossy(bytes).contains("NzAwMTIK"));
        if delivered_input_results(&adapter, "paste").len() == 1 && counted {
            break;
        }
    }
    let results = delivered_input_results(&adapter, "paste");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["subscription_id"], subscription_id.0);
    assert_eq!(results[0]["operation_id"], 41);
    assert_eq!(results[0]["admitted"], true);
    assert_eq!(results[0]["bytes_written"], 70_012);
    assert!(daemon
        .list_terminal_subscriptions()
        .iter()
        .any(|row| { row.session_id == session_id && row.subscription_id == subscription_id }));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn incomplete_paste_times_out_through_targeted_wait_without_later_input() {
    let data_dir = temp_data_dir("paste-timeout");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("paste-timeout-session".into());
    let client_id = ClientId("paste-timeout-client".into());
    let subscription_id = SubscriptionId("paste-timeout-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "sleep 30".into();
    daemon.spawn(request, 1).expect("spawn idle child");
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

    let begin = compact_paste_frames(51, 1, 1, b"unfinished")
        .into_iter()
        .next()
        .expect("begin");
    adapter.inject_ingress_frame(begin.clone());
    adapter.inject_ingress_frame(begin.clone());
    let intake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&intake, 3).expect("accept begin");
    let _control = daemon.wake_pump_control();
    let started = Instant::now();
    let WakePumpWait::Wakes(expired) = daemon.wait_pump(Duration::from_secs(30)) else {
        panic!("paste deadline must return a wake batch");
    };
    assert!(started.elapsed() <= Duration::from_secs(6));
    assert_eq!(expired.ingress_sessions, Vec::<SessionId>::new());
    assert_eq!(expired.adapter_routes.len(), 1);
    assert_eq!(expired.adapter_routes[0].session_id, session_id);
    assert_eq!(expired.adapter_routes[0].subscription_id, subscription_id);
    daemon.pump_woken(&expired, 4).expect("deliver timeout");
    let results = delivered_input_results(&adapter, "paste");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["operation_id"], 51);
    assert_eq!(results[0]["bytes_written"], 0);
    assert_eq!(results[0]["rejection"], "timeout");

    adapter.inject_ingress_frame(begin);
    let replay = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&replay, 5)
        .expect("drop completed begin replay");
    assert_eq!(
        delivered_input_results(&adapter, "paste").len(),
        1,
        "active and completed Begin replays must not add a second result"
    );

    let mut commit = vec![1, 6, 0, 4];
    commit.extend_from_slice(&51_u32.to_be_bytes());
    adapter.inject_ingress_frame(commit);
    let late = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&late, 6).expect("drop late commit");
    assert_eq!(delivered_input_results(&adapter, "paste").len(), 1);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn partial_paste_write_delivers_result_then_hard_stops_only_that_owner() {
    let data_dir = temp_data_dir("paste-partial");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(Duration::from_millis(800))
            .with_test_write_max_chunk(Some(1))
            .with_test_write_block_until_unix_ms(Some(now_ms + 30_000)),
    );
    let session_id = SessionId("paste-partial-session".into());
    let owner_client = ClientId("paste-partial-client".into());
    let owner_subscription = SubscriptionId("paste-partial-owner".into());
    let sibling_client = ClientId("paste-partial-sibling-client".into());
    let sibling_subscription = SubscriptionId("paste-partial-sibling".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf ready; sleep 30".into();
    daemon.spawn(request, 1).expect("spawn");
    for (client, subscription) in [
        (owner_client.clone(), owner_subscription.clone()),
        (sibling_client.clone(), sibling_subscription.clone()),
    ] {
        daemon
            .attach(client, session_id.clone(), subscription, 2)
            .expect("attach owner");
    }
    let started = Instant::now();
    let mut probe = 0;
    let token = loop {
        probe += 1;
        daemon.drain(&session_id, 2 + probe).expect("drain ready");
        if let Ok(result) = daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId(format!("paste-partial-modes-{probe}")),
            session_id: session_id.clone(),
            now_seconds: 20 + probe,
        }) {
            break result.mode_flags.mode_freshness;
        }
        assert!(started.elapsed() < Duration::from_secs(8));
    };
    let records = daemon.list_terminal_subscriptions();
    let owner_generation = records
        .iter()
        .find(|row| row.subscription_id == owner_subscription)
        .expect("owner")
        .generation;
    let sibling_generation = records
        .iter()
        .find(|row| row.subscription_id == sibling_subscription)
        .expect("sibling")
        .generation;
    let owner_adapter = SharedFakeTerminalAdapter::auto_complete();
    let sibling_adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            owner_client,
            session_id.clone(),
            owner_subscription.clone(),
            owner_generation,
            empty_caps(),
            Box::new(owner_adapter.clone()),
        )
        .expect("bind owner");
    daemon
        .bind_waking_terminal_adapter(
            sibling_client,
            session_id.clone(),
            sibling_subscription.clone(),
            sibling_generation,
            empty_caps(),
            Box::new(sibling_adapter.clone()),
        )
        .expect("bind sibling");

    for frame in compact_paste_frames(
        61,
        token.mode_generation,
        token.mode_revision,
        b"partial-paste",
    ) {
        owner_adapter.inject_ingress_frame(frame);
    }
    let input = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&input, 30).expect("submit partial paste");
    let deadline = Instant::now() + Duration::from_secs(5);
    while delivered_input_results(&owner_adapter, "paste").is_empty() {
        assert!(
            Instant::now() < deadline,
            "partial paste result did not arrive"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, 31)
            .expect("complete partial paste");
    }
    let results = delivered_input_results(&owner_adapter, "paste");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["operation_id"], 61);
    assert_eq!(results[0]["rejection"], "partial_write");
    let written = results[0]["bytes_written"].as_u64().expect("written count");
    assert!(written > 0 && written < 13);
    assert!(!daemon
        .list_terminal_subscriptions()
        .iter()
        .any(|row| row.subscription_id == owner_subscription));

    sibling_adapter.inject_ingress_frame(compact_resize_frame(25, 81));
    let sibling_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&sibling_wake, 32)
        .expect("sibling resize progresses");
    assert_eq!(delivered_input_result_count(&sibling_adapter, "resize"), 1);
    assert!(daemon
        .list_terminal_subscriptions()
        .iter()
        .any(|row| row.subscription_id == sibling_subscription));
    let _ = fs::remove_dir_all(data_dir);
}

/// A real worker-side mode change makes an earlier token stale. Core rejects
/// the stale paste with the worker's current token and flags. A retry with
/// exactly the returned token is admitted, and the admitted bracketed paste
/// counts the content plus the 12 framing bytes.
#[cfg(unix)]
#[test]
fn stale_paste_token_reports_current_mode_and_returned_token_retry_admits() {
    let data_dir = temp_data_dir("paste-stale");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("paste-stale-session".into());
    let client_id = ClientId("paste-stale-client".into());
    let subscription_id = SubscriptionId("paste-stale-sub".into());
    let mut request = spawn_request(&session_id);
    // Phase 1: plain mode, print ready, wait for one trigger byte.
    // Phase 2: enable bracketed paste, then report the exact next 13 received
    // bytes as one hex line. A leaked stale byte shifts and fails that line.
    request.request.arguments[1] = "stty raw -echo; printf ready; \
         dd bs=1 count=1 2>/dev/null >/dev/null; printf '\\033[?2004h'; \
         dd bs=1 count=13 2>/dev/null | od -An -tx1 | tr -d ' \\n'; printf '\\n'; \
         sleep 30"
        .into();
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
    let old_token = loop {
        probe += 1;
        daemon.drain(&session_id, 2 + probe).expect("drain ready");
        if let Ok(result) = daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId(format!("paste-stale-modes-{probe}")),
            session_id: session_id.clone(),
            now_seconds: 20 + probe,
        }) {
            assert!(
                !result.mode_flags.mode_flags.bracketed_paste,
                "bracketed paste must be off before the trigger byte"
            );
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
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind");

    // Wait for the child's ready marker. `printf ready` runs after `stty raw`,
    // so a visible marker proves raw mode is active before the trigger byte.
    // The marker can land in the opaque attach snapshot instead of a later
    // terminal_output frame, so the screen text is the oracle.
    let started = Instant::now();
    loop {
        probe += 1;
        let screen = daemon
            .read_screen(ReadScreenRequest {
                request_id: RequestId(format!("paste-stale-screen-{probe}")),
                session_id: session_id.clone(),
                now_seconds: 20 + probe,
            })
            .expect("read screen while waiting for ready");
        if screen.screen.text.contains("ready") {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "child ready marker did not appear: {:?}",
            screen.screen.text
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, 20 + probe)
            .expect("pump while waiting for ready");
    }

    // Trigger the worker-side mode change with one plain input byte.
    adapter.inject_ingress_frame(compact_input_frame(b"k"));
    let trigger_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&trigger_wake, 10)
        .expect("apply trigger input");

    // Refresh Core's stored authority through a worker probe. The worker
    // emits no spontaneous mode frame, so the probe is the only refresh path.
    let started = Instant::now();
    let current_token = loop {
        probe += 1;
        let result = daemon
            .read_mode_flags(ReadModeFlagsRequest {
                request_id: RequestId(format!("paste-stale-modes-{probe}")),
                session_id: session_id.clone(),
                now_seconds: 20 + probe,
            })
            .expect("probe after trigger");
        if result.mode_flags.mode_flags.bracketed_paste {
            break result.mode_flags.mode_freshness;
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "bracketed-paste mode did not become ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_ne!(
        current_token, old_token,
        "a worker-side mode change must advance the freshness token"
    );

    for frame in compact_paste_frames(71, old_token.mode_generation, old_token.mode_revision, b"X")
    {
        adapter.inject_ingress_frame(frame);
    }
    let stale_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&stale_wake, 30)
        .expect("reject stale paste");
    let stale = delivered_input_results(&adapter, "paste");
    assert_eq!(stale.len(), 1, "stale paste must emit exactly one result");
    assert_eq!(stale[0]["operation_id"], 71);
    assert_eq!(stale[0]["admitted"], false);
    assert_eq!(stale[0]["bytes_written"], 0);
    assert_eq!(stale[0]["rejection"], "stale_mode");
    assert_eq!(
        stale[0]["mode_generation"].as_u64(),
        Some(current_token.mode_generation),
        "stale rejection must carry the current generation: {}",
        stale[0]
    );
    assert_eq!(
        stale[0]["mode_revision"].as_u64(),
        Some(current_token.mode_revision),
        "stale rejection must carry the current revision: {}",
        stale[0]
    );
    assert_eq!(
        stale[0]["mode_flags"]["bracketed_paste"], true,
        "stale rejection must carry the current flags: {}",
        stale[0]
    );

    // Retry with exactly the token the rejection returned.
    let returned = botster_core::ModeFreshnessToken {
        mode_generation: stale[0]["mode_generation"]
            .as_u64()
            .expect("returned generation"),
        mode_revision: stale[0]["mode_revision"]
            .as_u64()
            .expect("returned revision"),
    };
    for frame in compact_paste_frames(72, returned.mode_generation, returned.mode_revision, b"Y") {
        adapter.inject_ingress_frame(frame);
    }
    let recovery_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&recovery_wake, 31)
        .expect("submit recovered paste");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < deadline,
            "recovered paste result and receiver hex line did not arrive: {:?}",
            String::from_utf8_lossy(&delivered_terminal_output_bytes(&adapter))
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, 32)
            .expect("complete recovered paste");
        // The child received exactly ESC[200~ Y ESC[201~ and nothing earlier.
        // A leaked stale `X` (0x58) would shift this line and fail the match.
        let received = terminal_output_has_count_line(&adapter, "1b5b3230307e591b5b3230317e");
        if delivered_input_results(&adapter, "paste").len() == 2 && received {
            break;
        }
    }
    let results = delivered_input_results(&adapter, "paste");
    assert_eq!(results.len(), 2);
    assert_eq!(results[1]["operation_id"], 72);
    assert_eq!(results[1]["admitted"], true);
    assert_eq!(
        results[1]["bytes_written"], 13,
        "admitted bracketed paste counts content plus 12 framing bytes"
    );
    assert_eq!(
        results[1]["mode_generation"].as_u64(),
        Some(current_token.mode_generation)
    );
    assert_eq!(
        results[1]["mode_revision"].as_u64(),
        Some(current_token.mode_revision)
    );
    assert_eq!(results[1]["mode_flags"]["bracketed_paste"], true);
    assert!(results[1].get("rejection").is_none());
    let _ = fs::remove_dir_all(data_dir);
}

/// Before any worker mode reply reaches Core, no mode authority exists. A
/// paste then reports `session_not_writable` with no invented token. After one
/// probe establishes authority, a paste with that token is admitted.
#[cfg(unix)]
#[test]
fn paste_before_worker_mode_authority_is_session_not_writable_not_stale() {
    let data_dir = temp_data_dir("paste-no-authority");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("paste-no-authority-session".into());
    let client_id = ClientId("paste-no-authority-client".into());
    let subscription_id = SubscriptionId("paste-no-authority-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "stty raw -echo; printf ready; sleep 30".into();
    daemon.spawn(request, 1).expect("spawn");
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
        .expect("bind");

    // No `read_mode_flags` and no gated input has run: Core holds no pair.
    for frame in compact_paste_frames(81, 1, 1, b"X") {
        adapter.inject_ingress_frame(frame);
    }
    let unavailable_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&unavailable_wake, 30)
        .expect("reject paste without authority");
    let unavailable = delivered_input_results(&adapter, "paste");
    assert_eq!(unavailable.len(), 1);
    assert_eq!(unavailable[0]["operation_id"], 81);
    assert_eq!(unavailable[0]["admitted"], false);
    assert_eq!(unavailable[0]["bytes_written"], 0);
    assert_eq!(
        unavailable[0]["rejection"], "session_not_writable",
        "missing authority is not a stale token: {}",
        unavailable[0]
    );
    assert_eq!(unavailable[0]["mode_generation"], 0);
    assert_eq!(unavailable[0]["mode_revision"], 0);

    let started = Instant::now();
    let mut probe = 0;
    let token = loop {
        probe += 1;
        daemon.drain(&session_id, 2 + probe).expect("drain ready");
        if let Ok(result) = daemon.read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId(format!("paste-no-authority-modes-{probe}")),
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
    for frame in compact_paste_frames(82, token.mode_generation, token.mode_revision, b"Y") {
        adapter.inject_ingress_frame(frame);
    }
    let admit_wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&admit_wake, 40)
        .expect("submit paste with authority");
    let deadline = Instant::now() + Duration::from_secs(5);
    while delivered_input_results(&adapter, "paste").len() < 2 {
        assert!(
            Instant::now() < deadline,
            "admitted paste result did not arrive"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, 41)
            .expect("complete paste with authority");
    }
    let results = delivered_input_results(&adapter, "paste");
    assert_eq!(results[1]["operation_id"], 82);
    assert_eq!(results[1]["admitted"], true);
    assert_eq!(results[1]["bytes_written"], 1);
    assert_eq!(
        results[1]["mode_generation"].as_u64(),
        Some(token.mode_generation)
    );
    assert_eq!(
        results[1]["mode_revision"].as_u64(),
        Some(token.mode_revision)
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
        pump_next(&mut daemon, 2);
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
    pump_until_registry_size(&mut daemon, &session_id, 31, 91, 3);
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
    pump_until_registry_size(&mut daemon, &session_a, 31, 101, 3);

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

struct ReleaseResizeAckHoldOnDrop(ResizeAckHold);

impl Drop for ReleaseResizeAckHoldOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[derive(Debug)]
enum DelayedArrivalStep {
    Attached,
    ResizePumped,
    SiblingProgressed,
    Completed,
    Failed(String),
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
        })
        .unwrap_or_else(|| "scenario thread panicked".to_string())
}

fn recv_delayed_arrival_step(
    rx: &mpsc::Receiver<DelayedArrivalStep>,
    bound: Duration,
    timeout_message: &str,
    hold: &ResizeAckHold,
) -> DelayedArrivalStep {
    match rx.recv_timeout(bound) {
        Ok(DelayedArrivalStep::Failed(message)) => {
            hold.release();
            panic!("{message}");
        }
        Ok(step) => step,
        Err(_) => {
            hold.release();
            panic!("{timeout_message}");
        }
    }
}

#[cfg(unix)]
#[test]
fn delayed_sibling_arrival_progresses_while_resize_acknowledgement_is_held() {
    let data_dir = temp_data_dir("delayed-sibling-resize");
    let acknowledgment_timeout = Duration::from_secs(2);
    let liveness_bound = Duration::from_millis(400);
    let session_a = SessionId("a-held-resize-session".into());
    let hold = ResizeAckHold::for_session(session_a.clone());
    let _release = ReleaseResizeAckHoldOnDrop(hold.clone());
    let worker = worker_path();
    let (tx, rx) = mpsc::sync_channel(8);
    let scenario_hold = hold.clone();
    let scenario_dir = data_dir.clone();
    let scenario_session_a = session_a.clone();
    let scenario = std::thread::spawn(move || {
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut config = CoreDaemonConfig::new(&scenario_dir)
                .with_worker_path(worker)
                .with_mode_gated_input_timeout(acknowledgment_timeout);
            config.test_resize_ack_hold = Some(scenario_hold.clone());
            let mut daemon = CoreDaemon::new(config);
            let (bound_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-held-resize");
            assert_eq!(bound_a, scenario_session_a);
            let (session_b, adapter_b) = bind_size_reporting_worker(&mut daemon, "z-live-sibling");
            scenario_hold.arm();
            let record_b = daemon
                .registry()
                .load(&session_b)
                .expect("load B")
                .expect("record B");
            assert_eq!((record_b.rows, record_b.cols), (24, 80));
            tx.send(DelayedArrivalStep::Attached)
                .expect("send attached");

            adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
            let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
            assert!(
                resize_batch
                    .adapter_routes
                    .iter()
                    .any(|route| route.session_id == scenario_session_a),
                "A's resize must produce an adapter wake before the held acknowledgement"
            );
            let resize_pump = daemon.pump_woken(&resize_batch, 3);
            tx.send(DelayedArrivalStep::ResizePumped)
                .expect("send resize pumped");
            resize_pump.expect("held resize acknowledgement must not block the pump");
            assert_eq!(delivered_input_result_count(&adapter_a, "resize"), 1);
            let record_a = daemon
                .registry()
                .load(&scenario_session_a)
                .expect("load A")
                .expect("record A");
            assert_eq!(
                (record_a.rows, record_a.cols),
                (24, 80),
                "registry must keep last confirmed geometry until acknowledgement"
            );

            adapter_b.inject_ingress_frame(compact_input_frame(b"report-b\n"));
            let input_b = daemon.wait_wakes(liveness_bound);
            daemon
                .pump_woken(&input_b, 4)
                .expect("B input must apply while A's acknowledgement is held");
            assert_eq!(delivered_input_result_count(&adapter_b, "input"), 1);
            pump_until_encoded_output(&mut daemon, &adapter_b, "MjQgODANCg==", 5);
            tx.send(DelayedArrivalStep::SiblingProgressed)
                .expect("send sibling progress");

            scenario_hold.release();
            let completion = wait_session_ingress_wake(&mut daemon, &scenario_session_a, 6);
            daemon
                .pump_woken(&completion, 6)
                .expect("pump A's completion wake");
            let record_a = daemon
                .registry()
                .load(&scenario_session_a)
                .expect("load A after ack")
                .expect("record A after ack");
            assert_eq!((record_a.rows, record_a.cols), (31, 101));

            adapter_a.inject_ingress_frame(compact_input_frame(b"report-a\n"));
            let later_a = daemon.wait_wakes(Duration::from_secs(1));
            daemon
                .pump_woken(&later_a, 7)
                .expect("later A input after completion");
            pump_until_encoded_output(&mut daemon, &adapter_a, "MzEgMTAxDQo=", 8);
        }));
        match outcome {
            Ok(()) => {
                let _ = tx.send(DelayedArrivalStep::Completed);
            }
            Err(payload) => {
                let _ = tx.send(DelayedArrivalStep::Failed(panic_payload_message(payload)));
            }
        }
    });

    match recv_delayed_arrival_step(
        &rx,
        Duration::from_secs(20),
        "worker attach did not finish",
        &hold,
    ) {
        DelayedArrivalStep::Attached => {}
        other => panic!("expected attach, got {other:?}"),
    }
    match recv_delayed_arrival_step(
        &rx,
        liveness_bound,
        "session A resize pump did not return while its acknowledgement remained held",
        &hold,
    ) {
        DelayedArrivalStep::ResizePumped => {}
        other => panic!("expected resize pump return, got {other:?}"),
    }
    match recv_delayed_arrival_step(
        &rx,
        acknowledgment_timeout,
        "B echo must arrive before A's resize deadline",
        &hold,
    ) {
        DelayedArrivalStep::SiblingProgressed => {}
        other => panic!("expected sibling progress, got {other:?}"),
    }
    match recv_delayed_arrival_step(
        &rx,
        Duration::from_secs(8),
        "delayed-arrival scenario did not finish",
        &hold,
    ) {
        DelayedArrivalStep::Completed => {}
        other => panic!("expected completion, got {other:?}"),
    }
    scenario
        .join()
        .expect("scenario thread finished after Completed");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn stalled_resize_acknowledgment_does_not_block_a_later_named_sibling() {
    let data_dir = temp_data_dir("pump-resize-stalled-sibling");
    let acknowledgment_timeout = Duration::from_millis(400);
    let mut config = CoreDaemonConfig::new(&data_dir)
        .with_worker_path(worker_path())
        .with_mode_gated_input_timeout(acknowledgment_timeout);
    config.test_omit_resize_applied = true;
    let mut daemon = CoreDaemon::new(config);
    let (session_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-stalled-resize");
    let (session_b, adapter_b) = bind_size_reporting_worker(&mut daemon, "z-live-input");
    assert_ne!(session_a, session_b);

    adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("omitted acknowledgement must not block the accept pump");
    assert_eq!(delivered_input_result_count(&adapter_a, "resize"), 1);
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 1);

    let deadline = Instant::now() + acknowledgment_timeout + Duration::from_millis(400);
    let mut b_inputs = 0_u32;
    while Instant::now() < deadline
        || !matches!(
            daemon.control_plane_state(&session_a),
            ControlPlaneState::Failed(ControlWriterError::ResizeAckTimeout)
        )
    {
        b_inputs += 1;
        adapter_b.inject_ingress_frame(compact_input_frame(format!("b{b_inputs}\n").as_bytes()));
        let batch = daemon.wait_wakes(Duration::from_millis(50));
        assert!(
            !batch.ingress_sessions.is_empty() || !batch.adapter_routes.is_empty(),
            "sibling traffic or A's deadline must keep producing wakes"
        );
        daemon
            .pump_woken(&batch, 4)
            .expect("sibling traffic must not return a pump error at A's deadline");
        if matches!(
            daemon.control_plane_state(&session_a),
            ControlPlaneState::Failed(ControlWriterError::ResizeAckTimeout)
        ) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "A's resize deadline did not fail the session under sibling traffic"
        );
    }
    assert!(
        delivered_input_result_count(&adapter_b, "input") >= 1,
        "B must remain live across A's deadline"
    );
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 0);
    let stalled_record = daemon
        .registry()
        .load(&session_a)
        .expect("load stalled record")
        .expect("stalled record");
    assert_eq!((stalled_record.rows, stalled_record.cols), (24, 80));

    adapter_b.inject_ingress_frame(compact_input_frame(b"after-a-failed\n"));
    let after = daemon.wait_wakes(Duration::from_secs(1));
    assert!(
        !after.ingress_sessions.contains(&session_a),
        "deadline wakes must stop after pending state is removed, got {after:?}"
    );
    daemon
        .pump_woken(&after, 5)
        .expect("B remains live after A's control-plane failure");
    assert!(delivered_input_result_count(&adapter_b, "input") >= 2);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn pending_resize_cap_parks_the_next_resize_and_resumes_on_acknowledgement() {
    let data_dir = temp_data_dir("pending-resize-cap");
    let session_a = SessionId("a-cap-resize-session".into());
    let hold = ResizeAckHold::for_session(session_a.clone());
    let _release = ReleaseResizeAckHoldOnDrop(hold.clone());
    let mut config = CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path());
    config.test_resize_ack_hold = Some(hold.clone());
    let mut daemon = CoreDaemon::new(config);
    let (bound_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-cap-resize");
    assert_eq!(bound_a, session_a);
    hold.arm();

    let first_batch = 16;
    assert!(first_batch < PENDING_INGRESS_RESIZE_CAP);
    for _ in 0..first_batch {
        adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    }
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&wake, 3)
        .expect("accept first resize batch");
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), first_batch);

    for _ in 0..(PENDING_INGRESS_RESIZE_CAP - first_batch) {
        adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    }
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&wake, 4).expect("fill pending cap");
    assert_eq!(
        daemon.pending_terminal_resize_len(&session_a),
        PENDING_INGRESS_RESIZE_CAP
    );
    assert_eq!(
        delivered_input_result_count(&adapter_a, "resize"),
        PENDING_INGRESS_RESIZE_CAP
    );

    adapter_a.inject_ingress_frame(compact_resize_frame(32, 102));
    adapter_a.inject_ingress_frame(compact_input_frame(b"behind-resize\n"));
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&wake, 5)
        .expect("park the overflowing resize");
    assert_eq!(
        daemon.pending_terminal_resize_len(&session_a),
        PENDING_INGRESS_RESIZE_CAP
    );
    assert_eq!(
        delivered_input_result_count(&adapter_a, "resize"),
        PENDING_INGRESS_RESIZE_CAP
    );
    assert_eq!(delivered_input_result_count(&adapter_a, "input"), 0);

    hold.release();
    let resume_deadline = Instant::now() + Duration::from_secs(5);
    while daemon.pending_terminal_resize_len(&session_a) > 0
        || delivered_input_result_count(&adapter_a, "resize") < PENDING_INGRESS_RESIZE_CAP + 1
        || delivered_input_result_count(&adapter_a, "input") < 1
    {
        assert!(
            Instant::now() < resume_deadline,
            "parked owner did not resume from acknowledgement wakes"
        );
        let batch = daemon.wait_wakes(Duration::from_millis(200));
        daemon
            .pump_woken(&batch, 6)
            .expect("resume parked resize from acknowledgement");
        assert!(
            daemon.pending_terminal_resize_len(&session_a) <= PENDING_INGRESS_RESIZE_CAP,
            "pending collection must stay at the ordinary-lane cap"
        );
    }
    assert_eq!(
        delivered_input_result_count(&adapter_a, "resize"),
        PENDING_INGRESS_RESIZE_CAP + 1
    );
    assert_eq!(delivered_input_result_count(&adapter_a, "input"), 1);
    pump_until_registry_size(&mut daemon, &session_a, 32, 102, 7);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn repeated_equal_resizes_complete_in_acknowledgement_order() {
    let data_dir = temp_data_dir("repeated-equal-resize");
    let session_a = SessionId("a-repeat-resize-session".into());
    let hold = ResizeAckHold::for_session(session_a.clone());
    let _release = ReleaseResizeAckHoldOnDrop(hold.clone());
    let mut config = CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path());
    config.test_resize_ack_hold = Some(hold.clone());
    let mut daemon = CoreDaemon::new(config);
    let (bound_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-repeat-resize");
    assert_eq!(bound_a, session_a);
    hold.arm();

    adapter_a.inject_ingress_frame(compact_resize_frame(24, 80));
    adapter_a.inject_ingress_frame(compact_resize_frame(24, 80));
    adapter_a.inject_ingress_frame(compact_resize_frame(31, 91));
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&wake, 3)
        .expect("accept repeated resizes");
    assert_eq!(delivered_input_result_count(&adapter_a, "resize"), 3);
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 3);
    let record = daemon
        .registry()
        .load(&session_a)
        .expect("load")
        .expect("record");
    assert_eq!((record.rows, record.cols), (24, 80));

    hold.release();
    pump_until_registry_size(&mut daemon, &session_a, 31, 91, 4);
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 0);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn explicit_resize_is_busy_while_ingress_resize_is_pending() {
    let data_dir = temp_data_dir("explicit-resize-busy");
    let session_a = SessionId("a-busy-resize-session".into());
    let hold = ResizeAckHold::for_session(session_a.clone());
    let _release = ReleaseResizeAckHoldOnDrop(hold.clone());
    let mut config = CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path());
    config.test_resize_ack_hold = Some(hold.clone());
    let mut daemon = CoreDaemon::new(config);
    let (bound_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-busy-resize");
    assert_eq!(bound_a, session_a);
    let (session_b, _adapter_b) = bind_size_reporting_worker(&mut daemon, "z-busy-sibling");
    hold.arm();

    adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&wake, 3).expect("accept ingress resize");
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 1);

    let busy = daemon
        .resize(
            ClientId("a-busy-resize-client".into()),
            session_a.clone(),
            40,
            120,
            4,
        )
        .expect_err("explicit resize must be busy while ingress is pending");
    assert!(
        matches!(busy, CoreDaemonError::ExplicitResizeBusy(ref id) if id == &session_a),
        "expected ExplicitResizeBusy, got {busy}"
    );
    let record_a = daemon
        .registry()
        .load(&session_a)
        .expect("load A")
        .expect("record A");
    assert_eq!((record_a.rows, record_a.cols), (24, 80));

    daemon
        .resize(
            ClientId("z-busy-sibling-client".into()),
            session_b.clone(),
            30,
            90,
            5,
        )
        .expect("sibling explicit resize is unaffected");
    let record_b = daemon
        .registry()
        .load(&session_b)
        .expect("load B")
        .expect("record B");
    assert_eq!((record_b.rows, record_b.cols), (30, 90));

    hold.release();
    pump_until_registry_size(&mut daemon, &session_a, 31, 101, 6);
    daemon
        .resize(
            ClientId("a-busy-resize-client".into()),
            session_a.clone(),
            40,
            120,
            7,
        )
        .expect("explicit resize succeeds after ingress completion");
    let record_a = daemon
        .registry()
        .load(&session_a)
        .expect("load A after explicit")
        .expect("record A after explicit");
    assert_eq!((record_a.rows, record_a.cols), (40, 120));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn teardown_clears_pending_resize_and_ignores_late_acknowledgement() {
    let data_dir = temp_data_dir("teardown-pending-resize");
    let session_a = SessionId("a-teardown-resize-session".into());
    let hold = ResizeAckHold::for_session(session_a.clone());
    let _release = ReleaseResizeAckHoldOnDrop(hold.clone());
    let mut config = CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path());
    config.test_resize_ack_hold = Some(hold.clone());
    let mut daemon = CoreDaemon::new(config);
    let (bound_a, adapter_a) = bind_size_reporting_worker(&mut daemon, "a-teardown-resize");
    assert_eq!(bound_a, session_a);
    hold.arm();

    adapter_a.inject_ingress_frame(compact_resize_frame(31, 101));
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&wake, 3).expect("accept pending resize");
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 1);

    // ResizeAckHold blocks the parent reader after it reads FRAME_RESIZE_APPLIED
    // and before it queues the acknowledgement. Shutdown then cannot drain worker
    // stdout, and the daemon watchdog returns ShutdownFailed:
    // "worker session shutdown did not complete before the daemon deadline".
    // Releasing after teardown therefore deadlocks this gate. Release first.
    // This proves pending cleanup and a harmless later pump. It does not prove
    // that an acknowledgement arrived after teardown.
    hold.release();
    daemon
        .shutdown(Some(session_a.clone()), 4)
        .expect("shutdown after releasing the acknowledgement hold");
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), 0);
    let late = daemon.wait_wakes(Duration::from_millis(200));
    let _ = daemon.pump_woken(&late, 5);
    if let Some(record) = daemon
        .registry()
        .load(&session_a)
        .expect("load after teardown")
    {
        assert_eq!((record.rows, record.cols), (24, 80));
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn expired_pending_resize_on_exited_session_still_delivers_process_exit() {
    // Exit drain removes the worker map entry before reconcile. This test
    // covers ProcessExited delivery through the SessionNotFound pending-clear
    // branch, not the Stopping/Exited guard that runs after take_resize_applied
    // returns Ok. That guard is proven in
    // managed_session_runtime::tests::expired_pending_resize_guard.
    let data_dir = temp_data_dir("expired-resize-exit-delivery");
    let acknowledgment_timeout = Duration::from_millis(300);
    let session_id = SessionId("expired-resize-exit-session".into());
    let client_id = ClientId("expired-resize-exit-client".into());
    let subscription_id = SubscriptionId("expired-resize-exit-sub".into());
    let go = data_dir.join("go");
    let mut config = CoreDaemonConfig::new(&data_dir)
        .with_worker_path(worker_path())
        .with_mode_gated_input_timeout(acknowledgment_timeout);
    config.test_omit_resize_applied = true;
    let mut daemon = CoreDaemon::new(config);
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "stty -echo; printf ready; while [ ! -f '{}' ]; do sleep 0.05; done; exit 0",
        go.display()
    );
    daemon.spawn(request, 1).expect("spawn");
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
        .expect("bind");
    let attach_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < attach_deadline,
            "worker attach did not finish"
        );
        pump_next(&mut daemon, 2);
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
    pump_available_wakes_until_quiet(&mut daemon, 3);

    adapter.inject_ingress_frame(compact_resize_frame(31, 101));
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 4)
        .expect("accept omitted-ack resize");
    assert_eq!(daemon.pending_terminal_resize_len(&session_id), 1);
    let resize_deadline = Instant::now() + acknowledgment_timeout;
    assert!(
        matches!(
            daemon.engine_session_lifecycle(&session_id),
            Some(
                botster_core::SessionLifecycleState::Running
                    | botster_core::SessionLifecycleState::Starting
            )
        ),
        "expiry skip must key off engine lifecycle, not a registry projection"
    );

    fs::write(&go, b"go").expect("release child");
    observe_until_exited_without_pump(&mut daemon, &session_id, 20);
    assert!(
        matches!(
            daemon.engine_session_lifecycle(&session_id),
            Some(botster_core::SessionLifecycleState::Exited { .. })
        ),
        "observe must commit engine Exited before the targeted delivery pump"
    );
    assert!(!adapter_has_process_exit(&adapter));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);

    while Instant::now() < resize_deadline + Duration::from_millis(20) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == &session_id),
        "expired pending resize must still name the exited session for delivery, got {batch:?}"
    );
    daemon
        .pump_woken(&batch, 23)
        .expect("deliver ProcessExited after resize deadline");
    assert!(
        adapter_has_process_exit(&adapter),
        "resize-timeout failure must not drop an undelivered ProcessExited frame"
    );
    assert_eq!(daemon.pending_terminal_resize_len(&session_id), 0);
    assert!(!matches!(
        daemon.control_plane_state(&session_id),
        ControlPlaneState::Failed(ControlWriterError::ResizeAckTimeout)
    ));

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
fn waking_bind_after_shutdown_closes_and_drops_adapter() {
    let data_dir = temp_data_dir("bind-after-shutdown-close-drop");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    daemon.shutdown(None, 1).expect("shutdown");
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let presented = adapter.clone();
    assert_eq!(adapter.shared_owner_count(), 2);

    assert!(matches!(
        daemon.bind_waking_terminal_adapter(
            ClientId("late-client".into()),
            SessionId("late-session".into()),
            SubscriptionId("late-sub".into()),
            botster_core_daemon::TerminalSubscriptionGeneration(1),
            empty_caps(),
            Box::new(presented),
        ),
        Err(CoreDaemonError::Shutdown)
    ));

    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert_eq!(
        adapter.shared_owner_count(),
        1,
        "Core must drop the rejected adapter after close"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn waking_bind_for_unknown_session_closes_and_drops_adapter() {
    let data_dir = temp_data_dir("bind-unknown-close-drop");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("unknown-session".into());
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let presented = adapter.clone();
    assert_eq!(adapter.shared_owner_count(), 2);

    assert!(matches!(
        daemon.bind_waking_terminal_adapter(
            ClientId("unknown-client".into()),
            session_id.clone(),
            SubscriptionId("unknown-sub".into()),
            botster_core_daemon::TerminalSubscriptionGeneration(1),
            empty_caps(),
            Box::new(presented),
        ),
        Err(CoreDaemonError::UnknownSession(id)) if id == session_id
    ));

    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert_eq!(
        adapter.shared_owner_count(),
        1,
        "Core must drop the rejected adapter after close"
    );
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

fn short_lived_spawn_request(
    session_id: &SessionId,
    done: &std::path::Path,
) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = format!("printf ready; : > '{}'; exit 0", done.display());
    request
}

fn wait_for_done_file(done: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if done.exists() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "child did not write done file {}",
            done.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn drain_follow_up_wakes(daemon: &mut CoreDaemon) {
    let quiet_deadline = Instant::now() + Duration::from_secs(5);
    let mut empty_streak = 0;
    while Instant::now() < quiet_deadline {
        let extra = daemon.wait_wakes(Duration::from_millis(200));
        if extra.adapter_routes.is_empty() && extra.ingress_sessions.is_empty() {
            empty_streak += 1;
            if empty_streak >= 3 {
                return;
            }
        } else {
            empty_streak = 0;
        }
    }
    panic!("runtime wake channel did not go quiet before the target drain");
}

fn pump_available_wakes_until_quiet(daemon: &mut CoreDaemon, now_seconds: u64) {
    let quiet_deadline = Instant::now() + Duration::from_secs(5);
    let mut empty_streak = 0;
    while Instant::now() < quiet_deadline {
        let batch = daemon.wait_wakes(Duration::from_millis(200));
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            empty_streak += 1;
            if empty_streak >= 3 {
                return;
            }
        } else {
            empty_streak = 0;
            daemon
                .pump_woken(&batch, now_seconds)
                .expect("pump leftover attach wakes");
        }
    }
    panic!("worker attach did not go quiet before child release");
}

fn consume_runtime_ingress_wakes(daemon: &mut CoreDaemon, session_id: &SessionId) {
    let batch = daemon.wait_wakes(Duration::from_secs(5));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == session_id),
        "setup must consume a runtime session ingress wake before the target drain, got {batch:?}"
    );
    drain_follow_up_wakes(daemon);
}

fn finish_short_lived_runtime_setup(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    done: &std::path::Path,
) {
    wait_for_done_file(done);
    consume_runtime_ingress_wakes(daemon, session_id);
}

fn observe_until_exited_without_pump(daemon: &mut CoreDaemon, session_id: &SessionId, now: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "observe did not commit Exited without a pump"
        );
        daemon
            .observe_session_lifecycle(session_id, now)
            .expect("observe until exit");
        if matches!(
            daemon
                .session_registry_state(session_id)
                .expect("registry after observe"),
            SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
        ) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn adapter_has_process_exit(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .any(|value| value.get("type").and_then(serde_json::Value::as_str) == Some("process_exit"))
}

fn bind_short_lived_session(
    daemon: &mut CoreDaemon,
    label: &str,
    adapter: SharedFakeTerminalAdapter,
) -> (SessionId, ClientId, SubscriptionId) {
    let session_id = SessionId(format!("{label}-session"));
    let client_id = ClientId(format!("{label}-client"));
    let subscription_id = SubscriptionId(format!("{label}-sub"));
    let done = std::env::temp_dir().join(format!("botster-core-wake-done-{}", session_id.0));
    let _ = fs::remove_file(&done);
    daemon
        .spawn(short_lived_spawn_request(&session_id, &done), 1)
        .expect("spawn");
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
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation");
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
    finish_short_lived_runtime_setup(daemon, &session_id, &done);
    let _ = fs::remove_file(&done);
    (session_id, client_id, subscription_id)
}

#[test]
fn readback_does_not_advance_bound_adapter() {
    let data_dir = temp_data_dir("readback");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let (session_id, client_id, subscription_id) =
        bind_short_lived_session(&mut daemon, "readback", adapter.clone());
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    let before = adapter.try_write_count();
    let _ = daemon.list().expect("list sessions");
    let _ = daemon.list_terminal_subscriptions();
    let cursor = daemon.lifecycle_baseline().expect("baseline").cursor;
    let _ = daemon
        .lifecycle_changes_page(&cursor, 16, 64 * 1024)
        .expect("changes page");
    let _ = daemon.lifecycle_baseline_page(
        None,
        None,
        LifecycleBaselineBudget {
            max_rows: 8,
            max_bytes: 16 * 1024,
            max_elapsed: Duration::from_secs(1),
        },
    );
    let _ = daemon
        .session_registry_state(&session_id)
        .expect("registry state");
    let _ = daemon.observe_lifecycle(5);
    let _ = daemon.observe_lifecycle_slice(
        6,
        None,
        ObserveLifecycleBudget {
            max_sessions: 1,
            max_encoded_result_bytes: 16 * 1024,
            max_elapsed: Duration::from_secs(1),
        },
    );
    let _ = daemon.observe_session_lifecycle(&session_id, 7);
    let _ = daemon.read_screen(ReadScreenRequest {
        request_id: RequestId("screen".into()),
        session_id: session_id.clone(),
        now_seconds: 8,
    });
    let _ = daemon.read_mode_flags(ReadModeFlagsRequest {
        request_id: RequestId("modes".into()),
        session_id: session_id.clone(),
        now_seconds: 9,
    });
    let _ = daemon.capture_snapshot(CaptureSnapshotRequest {
        request_id: RequestId("snap".into()),
        session_id: session_id.clone(),
        now_seconds: 10,
    });
    let _ = daemon.capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
        request_id: RequestId("color".into()),
        session_id: session_id.clone(),
        now_seconds: 11,
    });
    let _ = client_id;
    let _ = subscription_id;
    assert_eq!(adapter.try_write_count(), before);
    let _ = fs::remove_dir_all(data_dir);
}

fn assert_observe_then_targeted_process_exit(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    session_id: &SessionId,
) {
    let after_spawn = daemon.lifecycle_baseline().expect("spawn baseline").cursor;
    observe_until_exited_without_pump(daemon, session_id, 20);
    assert!(matches!(
        daemon
            .observe_session_lifecycle(session_id, 21)
            .expect("exact observe"),
        SessionLifecycleLookup::Found(_)
    ));
    let _ = daemon.observe_lifecycle_slice(
        22,
        None,
        ObserveLifecycleBudget {
            max_sessions: 1,
            max_encoded_result_bytes: 16 * 1024,
            max_elapsed: Duration::from_secs(1),
        },
    );
    assert!(matches!(
        daemon
            .session_registry_state(session_id)
            .expect("exited registry"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
    ));
    let exited = daemon
        .lifecycle_changes_page(&after_spawn, 32, 64 * 1024)
        .expect("journal")
        .changes
        .iter()
        .filter(|change| {
            matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == *session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            )
        })
        .count();
    assert_eq!(exited, 1);
    let writes_before = adapter.try_write_count();
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == session_id),
        "observe must emit a session ingress wake, got {batch:?}"
    );
    daemon.pump_woken(&batch, 23).expect("targeted pump");
    assert!(adapter_has_process_exit(adapter));
    assert!(adapter.try_write_count() > writes_before);
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let exited_after = daemon
        .lifecycle_changes_page(&after_spawn, 32, 64 * 1024)
        .expect("journal after pump")
        .changes
        .iter()
        .filter(|change| {
            matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == *session_id
                        && record.session.registry_state == RegistrySessionState::Exited
            )
        })
        .count();
    assert_eq!(exited_after, 1);
}

#[test]
fn observe_queues_process_exit_until_wait_wakes_and_pump_woken() {
    let data_dir = temp_data_dir("observe-exit-wake");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let (session_id, _, _) =
        bind_short_lived_session(&mut daemon, "observe-exit-wake", adapter.clone());
    assert_observe_then_targeted_process_exit(&mut daemon, &adapter, &session_id);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_observe_queues_process_exit_until_wait_wakes_and_pump_woken() {
    let data_dir = temp_data_dir("observe-exit-wake-worker");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("observe-exit-wake-worker-session".into());
    let client_id = ClientId("observe-exit-wake-worker-client".into());
    let subscription_id = SubscriptionId("observe-exit-wake-worker-sub".into());
    let go = data_dir.join("go");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; while [ ! -f '{}' ]; do sleep 0.05; done; exit 0",
        go.display()
    );
    daemon.spawn(request, 1).expect("spawn");
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
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "worker attach did not finish");
        pump_next(&mut daemon, 2);
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
    pump_available_wakes_until_quiet(&mut daemon, 3);
    fs::write(&go, b"go").expect("release child");
    consume_runtime_ingress_wakes(&mut daemon, &session_id);
    assert_observe_then_targeted_process_exit(&mut daemon, &adapter, &session_id);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn declared_unbound_exit_keeps_session_wake_until_bind_and_pump() {
    let data_dir = temp_data_dir("declared-unbound-exit");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("declared-unbound-session".into());
    let client_id = ClientId("declared-unbound-client".into());
    let subscription_id = SubscriptionId("declared-unbound-sub".into());
    let done = data_dir.join("child-done");
    let after_spawn = {
        daemon
            .spawn(short_lived_spawn_request(&session_id, &done), 1)
            .expect("spawn");
        daemon.lifecycle_baseline().expect("baseline").cursor
    };
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
    finish_short_lived_runtime_setup(&mut daemon, &session_id, &done);
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    assert_eq!(
        daemon
            .lifecycle_changes_page(&after_spawn, 32, 64 * 1024)
            .expect("journal")
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
        1
    );
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
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == &session_id),
        "bind with held frames must notify the live session wake, got {batch:?}"
    );
    daemon.pump_woken(&batch, 4).expect("pump after bind");
    assert!(adapter_has_process_exit(&adapter));
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn observe_then_force_closed_adapter_still_retires_session_wake() {
    let data_dir = temp_data_dir("observe-hard-stop");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let (session_id, _, _) =
        bind_short_lived_session(&mut daemon, "observe-hard-stop", adapter.clone());
    let after_spawn = daemon.lifecycle_baseline().expect("baseline").cursor;
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    adapter.close_transport();
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    daemon.pump_woken(&batch, 4).expect("pump closed adapter");
    assert_eq!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    assert_eq!(
        daemon
            .lifecycle_changes_page(&after_spawn, 32, 64 * 1024)
            .expect("journal")
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
        1
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn abandoned_declaration_observe_retires_session_wake() {
    let data_dir = temp_data_dir("abandoned-declaration");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("abandoned-session".into());
    let client_id = ClientId("abandoned-client".into());
    let subscription_id = SubscriptionId("abandoned-sub".into());
    let done = data_dir.join("child-done");
    daemon
        .spawn(short_lived_spawn_request(&session_id, &done), 1)
        .expect("spawn");
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
    finish_short_lived_runtime_setup(&mut daemon, &session_id, &done);
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    daemon
        .detach(client_id, session_id.clone(), subscription_id, 4)
        .expect("unsubscribe before bind");
    daemon
        .observe_session_lifecycle(&session_id, 5)
        .expect("observe after unsubscribe");
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn observe_does_not_try_write_a_blocked_bound_adapter() {
    let data_dir = temp_data_dir("observe-block-writes");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    adapter.block_writes();
    let (session_id, _, _) =
        bind_short_lived_session(&mut daemon, "observe-block-writes", adapter.clone());
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    let before = adapter.try_write_count();
    daemon
        .observe_session_lifecycle(&session_id, 4)
        .expect("observe blocked adapter");
    assert_eq!(adapter.try_write_count(), before);
    assert_ne!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
fn bind_named_short_lived(
    daemon: &mut CoreDaemon,
    session_id: SessionId,
    adapter: SharedFakeTerminalAdapter,
) -> SessionId {
    let client_id = ClientId(format!("{}-client", session_id.0));
    let subscription_id = SubscriptionId(format!("{}-sub", session_id.0));
    let done = std::env::temp_dir().join(format!("botster-core-wake-done-{}", session_id.0));
    let _ = fs::remove_file(&done);
    daemon
        .spawn(short_lived_spawn_request(&session_id, &done), 1)
        .expect("spawn");
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
    let generation = daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("generation");
    daemon
        .bind_waking_terminal_adapter(
            ClientId(format!("{}-client", session_id.0)),
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter),
        )
        .expect("bind");
    finish_short_lived_runtime_setup(daemon, &session_id, &done);
    let _ = fs::remove_file(&done);
    session_id
}

#[cfg(unix)]
fn lock_sessions_directory(sessions_dir: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    let original_mode = fs::metadata(sessions_dir)
        .expect("sessions directory metadata")
        .permissions()
        .mode();
    let mut read_only = fs::metadata(sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    read_only.set_mode(0o500);
    fs::set_permissions(sessions_dir, read_only).expect("make sessions directory read-only");
    let probe = fs::write(sessions_dir.join("write-probe"), b"probe");
    if probe.is_ok() {
        let mut restored = fs::metadata(sessions_dir)
            .expect("sessions directory metadata")
            .permissions();
        restored.set_mode(original_mode);
        fs::set_permissions(sessions_dir, restored).expect("restore sessions permissions");
        panic!("read-only sessions directory accepted a probe write");
    }
    original_mode
}

#[cfg(unix)]
fn unlock_sessions_directory(sessions_dir: &std::path::Path, original_mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let mut restored = fs::metadata(sessions_dir)
        .expect("sessions directory metadata")
        .permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(sessions_dir, restored).expect("restore sessions permissions");
}

#[cfg(unix)]
#[test]
fn drain_resize_persist_failure_still_emits_bound_queue_wake() {
    let data_dir = temp_data_dir("drain-resize-persist");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("drain-resize-persist-session".into());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_applied_attach_resize(Some((
            session_id.clone(),
            40,
            120,
            12,
        ))),
    );
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let session_id = bind_named_short_lived(&mut daemon, session_id, adapter);
    let original_mode = lock_sessions_directory(&sessions_dir);
    let failure = daemon
        .drain(&session_id, 3)
        .expect_err("resize persistence must fail");
    unlock_sessions_directory(&sessions_dir, original_mode);
    assert!(
        failure.to_string().contains("Permission denied"),
        "expected permission failure, got {failure}"
    );
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == &session_id),
        "failed persist must not swallow the bound-queue wake, got {batch:?}"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn observe_resize_persist_failure_still_emits_bound_queue_wake() {
    let data_dir = temp_data_dir("observe-resize-persist");
    let sessions_dir = data_dir.join("sessions");
    let session_id = SessionId("observe-resize-persist-session".into());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir).with_test_applied_attach_resize(Some((
            session_id.clone(),
            40,
            120,
            12,
        ))),
    );
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let session_id = bind_named_short_lived(&mut daemon, session_id, adapter);
    let original_mode = lock_sessions_directory(&sessions_dir);
    let failure = daemon
        .observe_session_lifecycle(&session_id, 3)
        .expect_err("resize persistence must fail");
    unlock_sessions_directory(&sessions_dir, original_mode);
    assert!(
        failure.to_string().contains("Permission denied"),
        "expected permission failure, got {failure}"
    );
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == &session_id),
        "failed persist must not swallow the bound-queue wake, got {batch:?}"
    );
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

#[cfg(unix)]
#[test]
fn stale_registry_then_shutdown_completes_through_wait_wakes() {
    let data_dir = temp_data_dir("stale-shutdown-wake");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("stale-shutdown-wake-session".into());
    let client_id = ClientId("stale-shutdown-wake-client".into());
    let subscription_id = SubscriptionId("stale-shutdown-wake-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf ready; exec sleep 30".into();
    daemon.spawn(request, 1).expect("spawn");
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
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "worker attach did not finish");
        pump_next(&mut daemon, 2);
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
    pump_available_wakes_until_quiet(&mut daemon, 3);
    daemon
        .mark_stale(&session_id, 10)
        .expect("mark registry stale");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("stale registry"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Stale)
    ));
    daemon
        .observe_session_lifecycle(&session_id, 11)
        .expect("observe after stale");
    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 12)
        .expect("shutdown through wait_wakes");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stale registry shutdown must complete from wakes, not the watchdog timeout"
    );
    assert_eq!(daemon.wake_source().session_registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn stale_registry_with_live_worker_still_delivers_process_exit_through_targeted_wake() {
    let data_dir = temp_data_dir("stale-live-worker-exit");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("stale-live-worker-exit-session".into());
    let client_id = ClientId("stale-live-worker-exit-client".into());
    let subscription_id = SubscriptionId("stale-live-worker-exit-sub".into());
    let go = data_dir.join("go");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; while [ ! -f '{}' ]; do sleep 0.05; done; exit 0",
        go.display()
    );
    daemon.spawn(request, 1).expect("spawn");
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
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "worker attach did not finish");
        pump_next(&mut daemon, 2);
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
    pump_available_wakes_until_quiet(&mut daemon, 3);
    daemon
        .mark_stale(&session_id, 10)
        .expect("mark registry stale");
    daemon
        .observe_session_lifecycle(&session_id, 11)
        .expect("observe after stale");
    assert!(matches!(
        daemon
            .session_registry_state(&session_id)
            .expect("stale registry"),
        SessionRegistryStateLookup::Found(RegistrySessionState::Stale)
    ));
    assert_eq!(daemon.wake_source().session_registry_len(), 1);
    fs::write(&go, b"go").expect("release child");
    consume_runtime_ingress_wakes(&mut daemon, &session_id);
    assert_observe_then_targeted_process_exit(&mut daemon, &adapter, &session_id);
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

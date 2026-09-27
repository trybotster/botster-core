#![allow(missing_docs)]

use std::fs;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::engine::managed_session_runtime::PENDING_INGRESS_RESIZE_CAP;
use botster_core::terminal_adapter::TerminalAdapterPressure;
use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory, SubscriptionId,
    TerminalCapabilitySet, TerminalWakeBatch, TerminalWakeKind, WAKE_QUEUE_CAPACITY,
};
use botster_core_daemon::{
    CoreDaemon, CoreDaemonConfig, CoreDaemonError, ObserveLifecycleBudget, RegistrySessionState,
    ResizeAckHold, SessionLifecycleChangeKind, SessionLifecycleLookup, SessionRegistryStateLookup,
    SpawnSessionRequest, WakePumpControl, WakePumpError, WakePumpWait,
};
use botster_core_test_support::bounded_wait::{wait_for, HANG_GUARD};
use botster_core_test_support::fixture_gate::Fifo;
use botster_core_test_support::terminal_adapter::{
    DeliveredFrame, SharedFakeTerminalAdapter, TerminalAdapterHarnessDriver,
};
use botster_terminal_protocol::{
    decode_attach_state, decode_input_result, decode_process_exit, AttachStateCode, InputOutcome,
    InputResultBody, TerminalFrame, TerminalKind,
};
use botster_terminal_protocol_client::{encode_paste, encode_terminal_input, TerminalInputCommand};

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "botster-core-wake-{label}-{}-{nanos}",
        std::process::id()
    ))
}

/// Pump real wakes until `done` holds, waiting for each wake with the time
/// left under one bound; expiry fails the test naming `what`.
fn pump_until(
    daemon: &mut CoreDaemon,
    what: &str,
    bound: Duration,
    tick: u64,
    mut done: impl FnMut(&mut CoreDaemon) -> bool,
) {
    if done(daemon) {
        return;
    }
    wait_for(what, bound, |remaining| {
        // timer: deadline — wait_for's bound limits this wait
        let batch = daemon.wait_wakes(remaining);
        if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
            daemon.pump_woken(&batch, tick).expect("pump woken work");
        }
        done(daemon).then_some(())
    });
}

fn pump_next(daemon: &mut CoreDaemon, now_seconds: u64) {
    // timer: deadline — expiry fails the checks that follow
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
    // Prebuilt and verified; tests never build the worker.
    botster_core_test_support::real_worker::WorkerBinary::from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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

    // The reporter's setup ends when its ready line has reached the adapter,
    // in the snapshot or as live output; after that it prints only on input.
    pump_until(
        daemon,
        "worker attach and ready",
        Duration::from_secs(8),
        2,
        |daemon| {
            adapter_settled(&adapter)
                && !daemon.capture_active(&session_id)
                && adapter_has_seen(&adapter, b"ready")
        },
    );
    pump_queued_wakes(daemon, 2);
    (session_id, adapter)
}

#[cfg(unix)]
fn wait_session_ingress_wake(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    tick: u64,
) -> TerminalWakeBatch {
    wait_for(
        "resize-completion session wake",
        Duration::from_secs(1),
        |remaining| {
            // timer: deadline — wait_for's bound limits this wait
            let batch = daemon.wait_wakes(remaining);
            if batch.ingress_sessions.contains(session_id) {
                return Some(batch);
            }
            if !batch.adapter_routes.is_empty() || !batch.ingress_sessions.is_empty() {
                daemon
                    .pump_woken(&batch, tick)
                    .expect("pump leftover wake while waiting for resize completion");
            }
            None
        },
    )
}

fn pump_until_registry_size(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    rows: u16,
    cols: u16,
    tick: u64,
) {
    pump_until(
        daemon,
        "registry geometry following the acknowledgement",
        Duration::from_secs(2),
        tick,
        |daemon| {
            let record = daemon
                .registry()
                .load(session_id)
                .expect("registry load")
                .expect("registry record");
            record.rows == rows && record.cols == cols
        },
    );
}

fn pump_until_output(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    expected: &[u8],
    tick: u64,
) {
    pump_until(
        daemon,
        "worker output",
        Duration::from_secs(5),
        tick,
        |_| adapter_output_contains(adapter, expected),
    );
}

fn pump_until_input_result_count(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    operation_ids: std::ops::RangeInclusive<u64>,
    expected: usize,
    tick: u64,
) {
    pump_until(
        daemon,
        "worker input results",
        Duration::from_secs(5),
        tick,
        |_| delivered_input_result_count(adapter, operation_ids.clone()) == expected,
    );
}

fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "printf ready; exec cat >/dev/null".to_string(),
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

fn empty_caps() -> TerminalCapabilitySet {
    TerminalCapabilitySet::empty()
}

struct ShutdownSessionOnDrop<'a> {
    daemon: &'a mut CoreDaemon,
    session_id: SessionId,
    complete: bool,
}

impl<'a> ShutdownSessionOnDrop<'a> {
    fn new(daemon: &'a mut CoreDaemon, session_id: SessionId) -> Self {
        Self {
            daemon,
            session_id,
            complete: false,
        }
    }

    fn daemon(&mut self) -> &mut CoreDaemon {
        self.daemon
    }

    fn shutdown(&mut self, now_seconds: u64) {
        self.daemon
            .shutdown(Some(self.session_id.clone()), now_seconds)
            .expect("shut down test session");
        self.complete = true;
    }
}

impl Drop for ShutdownSessionOnDrop<'_> {
    fn drop(&mut self) {
        if !self.complete {
            let _ = self
                .daemon
                .shutdown(Some(self.session_id.clone()), u64::MAX);
        }
    }
}

fn compact_input_frame(operation_id: u64, data: &[u8]) -> Vec<u8> {
    encode_terminal_input(&TerminalInputCommand::RawBytes {
        operation_id,
        data: data.to_vec(),
    })
    .expect("encode raw input")
    .into_bytes()
}

fn compact_resize_frame(operation_id: u64, rows: u16, cols: u16) -> Vec<u8> {
    encode_terminal_input(&TerminalInputCommand::Resize {
        operation_id,
        rows,
        cols,
        width_px: 0,
        height_px: 0,
    })
    .expect("encode resize input")
    .into_bytes()
}

fn compact_paste_frames(operation_id: u64, data: &[u8]) -> Vec<Vec<u8>> {
    encode_paste(operation_id, false, data)
        .expect("encode paste input")
        .into_iter()
        .map(|frame| frame.into_bytes())
        .collect()
}

fn delivered_input_result_count(
    adapter: &SharedFakeTerminalAdapter,
    operation_ids: std::ops::RangeInclusive<u64>,
) -> usize {
    delivered_input_results(adapter)
        .iter()
        .filter(|(_, result)| operation_ids.contains(&result.operation_id))
        .count()
}

fn delivered_written_input_results(
    adapter: &SharedFakeTerminalAdapter,
    operation_ids: std::ops::RangeInclusive<u64>,
) -> Vec<(DeliveredFrame, InputResultBody)> {
    delivered_input_results(adapter)
        .into_iter()
        .filter(|(_, result)| {
            operation_ids.contains(&result.operation_id) && result.outcome == InputOutcome::Written
        })
        .collect()
}

fn delivered_input_results(
    adapter: &SharedFakeTerminalAdapter,
) -> Vec<(DeliveredFrame, InputResultBody)> {
    adapter
        .snapshot_delivered_frames()
        .into_iter()
        .filter_map(|delivery| {
            let frame = TerminalFrame::from_bytes(&delivery.bytes).ok()?;
            let result = decode_input_result(&frame).ok()?;
            Some((delivery, result))
        })
        .collect()
}

fn adapter_output_count(adapter: &SharedFakeTerminalAdapter, needle: &[u8]) -> usize {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
        .filter(|frame| frame.kind() == TerminalKind::Output)
        .filter(|frame| {
            frame
                .body()
                .windows(needle.len())
                .any(|window| window == needle)
        })
        .count()
}

fn adapter_output_contains(adapter: &SharedFakeTerminalAdapter, needle: &[u8]) -> bool {
    adapter_output_count(adapter, needle) > 0
}

/// The adapter side of attach is complete: the adapter holds the Attached
/// state and its snapshot write has finished. The worker's capture can
/// still be open; `CoreDaemon::capture_active` reports that side.
fn adapter_settled(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter_has_attached(adapter) && adapter.snapshot_pressure() == TerminalAdapterPressure::Ready
}

/// Whether any frame the adapter received, the attach snapshot included,
/// carries `needle`. Output printed before the attach reaches the adapter
/// in the snapshot; output printed after it arrives as live output.
fn adapter_has_seen(adapter: &SharedFakeTerminalAdapter, needle: &[u8]) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .any(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
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
            // timer: deadline — expiry fails the checks that follow
            daemon.wait_pump(Duration::from_secs(5)),
            WakePumpWait::Interrupted
        ));
        interrupted_tx.send(()).expect("publish interrupt result");
        assert!(matches!(
            // timer: deadline — expiry fails the checks that follow
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
        // timer: deadline — expiry fails the checks that follow
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
        // timer: deadline — expiry fails the checks that follow
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
    // The property is order and preservation under interrupts, and that
    // shutdown stays bounded (never spins) while interrupts arrive: the
    // grace, the daemon's shutdown deadline, and the watchdog are hang
    // guards.
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_shutdown_grace_ms(Some(HANG_GUARD.as_millis() as u64))
            .with_test_shutdown_deadline(Some(HANG_GUARD)),
    );
    let session_id = SessionId("interrupt-shutdown-session".into());
    let client_id = ClientId("interrupt-shutdown-client".into());
    let subscription_id = SubscriptionId("interrupt-shutdown-sub".into());
    let ready = Fifo::new("shutdown-ready");
    let hold = Fifo::new("shutdown-hold");
    let terminated = Fifo::new("shutdown-term");
    let gate = Fifo::new("shutdown-gate");
    let mut request = spawn_request(&session_id);
    // Shutdown's own TERM runs the trap, which reports it and then waits on
    // the gate: shutdown is provably in progress until the test releases the
    // fixture to print its final output and exit. `wait` returns for a
    // trapped signal. The holder ignores TERM, so only the trap can end the
    // wait; the hold pipe is never written, and the group kill after the
    // leader's exit ends the holder. FIFO opens run in external commands,
    // whose redirections happen in the forked child: a signal the shell
    // handles cannot interrupt them.
    request.request.arguments[1] = format!(
        "trap '/bin/echo term > \"$TERMINATED\"; /bin/cat \"$GATE\" >/dev/null; printf final; exit 0' TERM; \
         /bin/echo ready > '{}'; (trap '' TERM; exec cat '{}') & wait $!",
        ready.path().display(),
        hold.path().display()
    );
    request.request.environment = SpawnEnvironment {
        variables: vec![
            SpawnEnvironmentVariable {
                name: "TERMINATED".into(),
                value: terminated.path().display().to_string(),
            },
            SpawnEnvironmentVariable {
                name: "GATE".into(),
                value: gate.path().display().to_string(),
            },
        ],
    };
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
    let _ = ready.read_signal(Duration::from_secs(15));
    // The property is order and preservation for an attached route: finish
    // the attach capture before shutdown begins.
    pump_until(
        &mut daemon,
        "the route's attach snapshot finishing",
        Duration::from_secs(15),
        2,
        |_| {
            adapter
                .snapshot_delivered_frame_bytes()
                .iter()
                .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
                .any(|frame| frame.kind() == TerminalKind::SnapshotFinish)
        },
    );

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
    let (first_sender, first_interrupt) = std::sync::mpsc::channel();
    // The adversary: interrupts keep arriving for as long as shutdown runs.
    let interrupter = std::thread::spawn(move || {
        let mut first = Some(first_sender);
        while active.load(std::sync::atomic::Ordering::Acquire) {
            interrupt_control.interrupt();
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(sender) = first.take() {
                let _ = sender.send(());
            }
            std::thread::yield_now();
        }
    });
    // timer: deadline — the interrupter must start; expiry fails the test
    first_interrupt
        // timer: deadline — expiry fails the checks that follow
        .recv_timeout(Duration::from_secs(5))
        .expect("the interrupter raised its first interrupt");
    let before_shutdown = interrupt_count.load(std::sync::atomic::Ordering::Acquire);
    // While the fixture holds shutdown open, raise one interrupt, then
    // release the fixture.
    let releaser_control = control.clone();
    let during = std::sync::Arc::clone(&interrupt_count);
    let releaser = std::thread::spawn(move || {
        let _ = terminated.read_signal(HANG_GUARD);
        releaser_control.interrupt();
        during.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        gate.release(HANG_GUARD);
    });
    let (returned_sender, returned) = std::sync::mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        // timer: deadline — shutdown must return (never spin) while interrupted
        returned.recv_timeout(HANG_GUARD)
    });
    let shutdown_result = daemon.shutdown(Some(session_id), 4);
    let _ = returned_sender.send(());
    shutdown_active.store(false, std::sync::atomic::Ordering::Release);
    let interrupter_result = interrupter.join();
    // Report the shutdown result first: a shutdown that failed before its
    // TERM leaves the releaser waiting for a receipt that never comes.
    shutdown_result.expect("bounded shutdown while interrupted");
    releaser.join().expect("releaser");
    let bounded = watchdog.join().expect("watchdog");

    interrupter_result.expect("interrupter");
    assert!(
        interrupt_count.load(std::sync::atomic::Ordering::Acquire) > before_shutdown,
        "the control thread must raise an interrupt during shutdown"
    );
    assert!(
        bounded.is_ok(),
        "shutdown spun past its bound while interrupted"
    );
    let frames = adapter.snapshot_delivered_frame_bytes();
    let decoded: Vec<_> = frames
        .iter()
        .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
        .collect();
    let final_output = decoded
        .iter()
        .position(|frame| {
            frame.kind() == TerminalKind::Output
                && frame
                    .body()
                    .windows(b"final".len())
                    .any(|body| body == b"final")
        })
        .unwrap_or_else(|| panic!("shutdown must deliver the final terminal output: {frames:?}"));
    let process_exit = decoded
        .iter()
        .position(|frame| {
            frame.kind() == TerminalKind::ProcessExit
                && decode_process_exit(frame).ok().and_then(|body| body.code) == Some(0)
        })
        .unwrap_or_else(|| panic!("shutdown must deliver a successful process exit: {frames:?}"));
    assert!(
        final_output < process_exit,
        "final output must precede process exit: {frames:?}"
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
        // timer: deadline — expiry fails the checks that follow
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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

    adapter.inject_ingress_frame(compact_input_frame(1, b"WAKE-INPUT\n"));
    // timer: deadline — expiry fails the checks that follow
    let first = daemon.wait_wakes(Duration::from_secs(1));
    assert!(first.adapter_routes.iter().any(|route| {
        route.session_id == session_id && route.subscription_id == subscription_id
    }));
    let input_outcome = daemon.pump_woken(&first, 3).expect("apply input wake");
    assert_eq!(input_outcome.pumped_routes, first.adapter_routes.len());
    assert!(
        !input_outcome.terminal_inventory_changed,
        "valid terminal input must not report an inventory change"
    );

    // timer: deadline — the echo and its input result must arrive within 5 s
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        // timer: deadline — wait for the next wake with the time left
        let batch = daemon.wait_wakes(Duration::from_secs(5).saturating_sub(started.elapsed()));
        let output_outcome = daemon.pump_woken(&batch, 4).expect("pump PTY echo");
        assert!(
            !output_outcome.terminal_inventory_changed,
            "ordinary PTY output must not report an inventory change"
        );
        let input_results = delivered_input_result_count(&adapter, 1..=1);
        let echoes = adapter_output_count(&adapter, b"echo:WAKE-INPUT\r\n");
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

    let subscriptions = daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records;
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

    pump_until(
        &mut daemon,
        "same-session routes attaching",
        Duration::from_secs(8),
        2,
        |daemon| {
            [&owner, &sibling]
                .iter()
                .all(|adapter| adapter_settled(adapter))
                && !daemon.capture_active(&session_id)
        },
    );
    pump_queued_wakes(&mut daemon, 2);

    owner.inject_ingress_frame(compact_resize_frame(1, 31, 91));
    owner.inject_ingress_frame(compact_input_frame(2, b"OWNER\n"));
    // timer: deadline — expiry fails the checks that follow
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

    let completion = wait_session_ingress_wake(&mut daemon, &session_id, 3);
    daemon
        .pump_woken(&completion, 3)
        .expect("pump mixed-batch resize completion");
    pump_until_input_result_count(&mut daemon, &owner, 1..=2, 2, 3);
    let resize_results = delivered_written_input_results(&owner, 1..=1);
    let input_results = delivered_written_input_results(&owner, 2..=2);
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for (delivery, _) in resize_results.iter().chain(&input_results) {
        assert_eq!(
            delivery.route.as_str(),
            owner_subscription.0.as_str(),
            "each result must identify the live owner"
        );
    }
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
        assert!(daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| {
                row.session_id == session_id
                    && row.subscription_id == *subscription
                    && row.generation == generation
            }));
    }

    pump_until_output(&mut daemon, &owner, b"echo:OWNER\r\n", 4);
    owner.inject_ingress_frame(compact_input_frame(3, b"REPORT-SIZE\n"));
    // timer: deadline — expiry fails the checks that follow
    let size_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&size_batch, 5)
        .expect("request worker size after mixed batch");
    pump_until_output(&mut daemon, &owner, b"31 91\r\n", 6);
    sibling.inject_ingress_frame(compact_input_frame(1, b"SIBLING\n"));
    // timer: deadline — expiry fails the checks that follow
    let sibling_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&sibling_batch, 7)
        .expect("apply sibling input after mixed batch");
    pump_until_output(&mut daemon, &sibling, b"echo:SIBLING\r\n", 8);
    assert_eq!(
        delivered_written_input_results(&sibling, 1..=1)
            .iter()
            .map(|(delivery, _)| delivery.route.as_str())
            .collect::<Vec<_>>(),
        vec![sibling_subscription.0.as_str()]
    );

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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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

    pump_until(
        &mut daemon,
        "worker attach",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    // Attached is the positive end of setup; pump what it left queued.
    pump_queued_wakes(&mut daemon, 2);
    assert_eq!(daemon.wake_source().occupancy(), 0);

    adapter.inject_ingress_frame(compact_resize_frame(1, 31, 91));
    adapter.inject_ingress_frame(compact_input_frame(2, b"SCRATCH\n"));

    // timer: deadline — expiry fails the checks that follow
    let mixed = daemon.wait_wakes(Duration::from_secs(5));
    assert_eq!(mixed.adapter_routes.len(), 1);
    assert_eq!(mixed.adapter_routes[0].session_id, session_id);
    assert_eq!(mixed.adapter_routes[0].subscription_id, subscription_id);
    assert!(mixed.ingress_sessions.is_empty());
    daemon.pump_woken(&mixed, 3).expect("pump mixed wake");

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
    assert!(daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .any(|row| {
            row.session_id == session_id
                && row.subscription_id == subscription_id
                && row.generation == generation
        }));

    let retained = wait_session_ingress_wake(&mut daemon, &session_id, 3);
    assert_eq!(retained.ingress_sessions, vec![session_id.clone()]);
    daemon
        .pump_woken(&retained, 4)
        .expect("pump retained resize-completion wake");
    pump_until_input_result_count(&mut daemon, &adapter, 1..=2, 2, 4);
    assert_eq!(
        delivered_input_result_count(&adapter, 1..=1),
        1,
        "resize must emit one total result"
    );
    assert_eq!(
        delivered_input_result_count(&adapter, 2..=2),
        1,
        "input must emit one total result"
    );
    let resize_results = delivered_written_input_results(&adapter, 1..=1);
    let input_results = delivered_written_input_results(&adapter, 2..=2);
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for (delivery, _) in resize_results.iter().chain(&input_results) {
        assert_eq!(
            delivery.route.as_str(),
            subscription_id.0.as_str(),
            "each result must identify the live owner"
        );
    }
    // The results and the registry geometry are separate events.
    pump_until_registry_size(&mut daemon, &session_id, 31, 91, 4);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker after completion")
        .expect("worker registry record after completion");
    assert_eq!((record.rows, record.cols), (31, 91));
    pump_until_output(&mut daemon, &adapter, b"echo:SCRATCH\r\n", 5);
    let exact_echoes = adapter_output_count(&adapter, b"echo:SCRATCH\r\n");
    assert_eq!(exact_echoes, 1, "exact PTY echo must arrive once");
    assert!(daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .any(|row| {
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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
        if adapter_has_attached(&adapter) {
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
        let WakePumpWait::Wakes(mut batch) = daemon.wait_pump(Duration::ZERO) else {
            panic!("uncontrolled wake pump must return wakes");
        };
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            if adapter.snapshot_pressure() == TerminalAdapterPressure::Ready
                && !daemon.capture_active(&session_id)
                && daemon.wake_source().occupancy() == 0
            {
                break;
            }
            // Not settled and nothing queued: wait for the next wake.
            let WakePumpWait::Wakes(next) =
                // timer: deadline — the settle loop's remaining bound; expiry fails the assert above
                daemon.wait_pump(settle_deadline.saturating_duration_since(Instant::now()))
            else {
                panic!("uncontrolled wake pump must return wakes");
            };
            batch = next;
        }
        daemon.pump_woken(&batch, 2).expect("settle attach wakes");
    }

    adapter.inject_ingress_frame(compact_resize_frame(1, 31, 91));
    adapter.inject_ingress_frame(compact_input_frame(2, b"SCRATCH\n"));

    // timer: deadline — expiry fails the checks that follow
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
        TerminalAdapterPressure::Ready,
        "worker results must arrive through a later ingress wake"
    );
    assert_eq!(delivered_input_result_count(&adapter, 1..=1), 0);
    assert_eq!(delivered_input_result_count(&adapter, 2..=2), 0);
    assert!(daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .any(|row| {
            row.session_id == session_id
                && row.subscription_id == subscription_id
                && row.generation == generation
        }));

    let completion_deadline = Instant::now() + Duration::from_secs(5);
    let mut retained_resize_wake_observed = false;
    while delivered_input_result_count(&adapter, 1..=1)
        + delivered_input_result_count(&adapter, 2..=2)
        < 2
    {
        assert!(
            Instant::now() < completion_deadline,
            "one-slot input results did not complete"
        );
        adapter.complete_write();
        // timer: deadline — expiry fails the checks that follow
        let WakePumpWait::Wakes(batch) = daemon.wait_pump(Duration::from_secs(5)) else {
            panic!("uncontrolled wake pump must return a completion wake");
        };
        assert!(batch.adapter_routes.iter().all(|route| {
            route.session_id == session_id && route.subscription_id == subscription_id
        }));
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
            assert!(!adapter_output_contains(&adapter, b"echo:SCRATCH\r\n"));
        }
        assert!(daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| {
                row.session_id == session_id
                    && row.subscription_id == subscription_id
                    && row.generation == generation
            }));
        assert_ne!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    }

    assert_eq!(delivered_input_result_count(&adapter, 1..=1), 1);
    assert_eq!(delivered_input_result_count(&adapter, 2..=2), 1);
    let resize_results = delivered_written_input_results(&adapter, 1..=1);
    let input_results = delivered_written_input_results(&adapter, 2..=2);
    assert_eq!(resize_results.len(), 1, "resize must complete once");
    assert_eq!(input_results.len(), 1, "input must complete once");
    for (delivery, _) in resize_results.iter().chain(&input_results) {
        assert_eq!(
            delivery.route.as_str(),
            subscription_id.0.as_str(),
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
            let WakePumpWait::Wakes(batch) =
                // timer: deadline — the loop's remaining bound; expiry fails the assert above
                daemon.wait_pump(retained_deadline.saturating_duration_since(Instant::now()))
            else {
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
        assert!(!adapter_output_contains(&adapter, b"echo:SCRATCH\r\n"));
    }
    assert!(retained_resize_wake_observed);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load resized worker")
        .expect("worker registry record");
    assert_eq!((record.rows, record.cols), (31, 91));

    let echo_deadline = Instant::now() + Duration::from_secs(5);
    while !adapter_output_contains(&adapter, b"echo:SCRATCH\r\n") {
        assert!(Instant::now() < echo_deadline, "worker echo did not arrive");
        adapter.complete_write();
        // timer: deadline — expiry fails the checks that follow
        let WakePumpWait::Wakes(echo) = daemon.wait_pump(Duration::from_secs(5)) else {
            panic!("uncontrolled wake pump must return the echo wake");
        };
        assert!(!echo.adapter_routes.is_empty() || !echo.ingress_sessions.is_empty());
        assert!(echo.adapter_routes.iter().all(|route| {
            route.session_id == session_id && route.subscription_id == subscription_id
        }));
        assert!(echo
            .ingress_sessions
            .iter()
            .all(|woken_session| woken_session == &session_id));
        daemon.pump_woken(&echo, 6).expect("pump worker echo wake");
        adapter.complete_write();
    }

    let exact_echoes = adapter_output_count(&adapter, b"echo:SCRATCH\r\n");
    assert_eq!(exact_echoes, 1, "exact PTY echo must arrive once");
    assert!(!delivered_input_results(&adapter)
        .iter()
        .any(|(_, result)| result.detail.contains("core_adapter_closed")));
    assert_ne!(adapter.snapshot_pressure(), TerminalAdapterPressure::Closed);
    assert!(daemon
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
        .iter()
        .any(|row| {
            row.session_id == session_id
                && row.subscription_id == subscription_id
                && row.generation == generation
        }));

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
    request.request.arguments[1] = "exec cat >/dev/null".into();
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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

    let begin = compact_paste_frames(51, b"unfinished")
        .into_iter()
        .next()
        .expect("begin");
    adapter.inject_ingress_frame(begin.clone());
    adapter.inject_ingress_frame(begin.clone());
    // timer: deadline — expiry fails the checks that follow
    let intake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&intake, 3).expect("accept begin");
    let _control = daemon.wake_pump_control();
    let started = Instant::now();
    let expired = loop {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the paste deadline's wake did not arrive"
        );
        let WakePumpWait::Wakes(batch) =
            // timer: deadline — a hang guard; Core ends the wait at its paste deadline
            daemon.wait_pump(Duration::from_secs(30))
        else {
            panic!("paste deadline must return a wake batch");
        };
        daemon
            .pump_woken(&batch, 4)
            .expect("deliver replay rejection or timeout");
        if delivered_input_results(&adapter)
            .iter()
            .any(|(_, result)| result.detail == "paste assembly timed out")
        {
            break batch;
        }
    };
    assert!(started.elapsed() <= Duration::from_secs(6));
    assert_eq!(expired.ingress_sessions, Vec::<SessionId>::new());
    assert_eq!(expired.adapter_routes.len(), 1);
    assert_eq!(expired.adapter_routes[0].session_id, session_id);
    assert_eq!(expired.adapter_routes[0].subscription_id, subscription_id);
    pump_until(
        &mut daemon,
        "both paste results",
        Duration::from_secs(5),
        4,
        |_| delivered_input_results(&adapter).len() >= 2,
    );
    pump_queued_wakes(&mut daemon, 4);
    let results = delivered_input_results(&adapter);
    assert_eq!(results.len(), 2);
    let timeout = results
        .iter()
        .find(|(_, result)| result.detail == "paste assembly timed out")
        .expect("one timeout result");
    assert_eq!(timeout.1.operation_id, 51);
    assert_eq!(timeout.1.outcome, InputOutcome::RejectedProtocol);
    assert_eq!(timeout.1.accepted_payload_bytes, Some(0));
    assert_eq!(timeout.1.written_pty_bytes, Some(0));
    let active_replay = results
        .iter()
        .find(|(_, result)| result.detail == "operation id is not strictly increasing")
        .expect("active Begin replay rejection");
    assert_eq!(active_replay.1.operation_id, 51);
    assert_eq!(active_replay.1.outcome, InputOutcome::RejectedProtocol);
    assert!(
        !adapter_output_contains(&adapter, b"unfinished"),
        "an incomplete paste must not reach the PTY"
    );

    adapter.inject_ingress_frame(begin);
    // timer: deadline — expiry fails the checks that follow
    let replay = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&replay, 5)
        .expect("reject completed begin replay");
    pump_until_input_result_count(&mut daemon, &adapter, 51..=51, 3, 5);
    let results = delivered_input_results(&adapter);
    assert_eq!(
        results
            .iter()
            .filter(|(_, result)| result.detail == "operation id is not strictly increasing")
            .count(),
        2,
        "each invalid Begin replay must receive one typed rejection"
    );
    assert_eq!(
        results
            .iter()
            .filter(|(_, result)| result.detail == "paste assembly timed out")
            .count(),
        1,
        "the completed replay must not alter the original timeout result"
    );
    assert!(
        !adapter_output_contains(&adapter, b"unfinished"),
        "Begin replays must not write paste content to the PTY"
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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

    pump_until(
        &mut daemon,
        "worker attach",
        Duration::from_secs(8),
        2,
        |daemon| {
            adapter_settled(&adapter)
                && !daemon.capture_active(&session_id)
                && adapter_has_seen(&adapter, b"ready")
        },
    );
    pump_queued_wakes(&mut daemon, 2);
    let before_resize = daemon.lifecycle_baseline().expect("baseline").cursor;

    adapter.inject_ingress_frame(compact_resize_frame(1, 31, 91));
    // timer: deadline — expiry fails the checks that follow
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("resize apply tick");
    pump_until_registry_size(&mut daemon, &session_id, 31, 91, 3);
    // The registry and the adapter's result are separate events.
    pump_until(
        &mut daemon,
        "the resize result",
        Duration::from_secs(5),
        3,
        |_| delivered_input_result_count(&adapter, 1..=1) == 1,
    );
    assert_eq!(delivered_input_result_count(&adapter, 1..=1), 1);
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("registry load")
        .expect("registry record");
    assert_eq!((record.rows, record.cols), (31, 91));

    adapter.inject_ingress_frame(compact_input_frame(2, b"report-size\n"));
    // timer: deadline — expiry fails the checks that follow
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
        // timer: deadline — wait for the next wake with the time left
        let batch = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
        daemon.pump_woken(&batch, 5).expect("pump size report");
        if adapter_output_contains(&adapter, b"31 91\r\n") {
            break;
        }
    }

    adapter.inject_ingress_frame(compact_resize_frame(3, 31, 91));
    adapter.inject_ingress_frame(compact_resize_frame(4, 31, 91));
    // timer: deadline — expiry fails the checks that follow
    let repeated_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&repeated_batch, 6)
        .expect("identical resize apply tick");
    pump_until_input_result_count(&mut daemon, &adapter, 1..=4, 4, 6);
    assert_eq!(
        delivered_input_result_count(&adapter, 1..=1)
            + delivered_input_result_count(&adapter, 3..=4),
        3
    );
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

    adapter_a.inject_ingress_frame(compact_resize_frame(1, 31, 101));
    // timer: deadline — expiry fails the checks that follow
    let resize_batch = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&resize_batch, 3)
        .expect("resize named worker");
    assert_eq!(delivered_input_result_count(&adapter_b, 1..=1), 0);
    pump_until_registry_size(&mut daemon, &session_a, 31, 101, 3);
    // The registry and the adapter's result are separate events.
    pump_until(
        &mut daemon,
        "the named sibling's resize result",
        Duration::from_secs(5),
        3,
        |_| delivered_input_result_count(&adapter_a, 1..=1) == 1,
    );
    assert_eq!(delivered_input_result_count(&adapter_a, 1..=1), 1);

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

    adapter_a.inject_ingress_frame(compact_input_frame(2, b"report-a\n"));
    // timer: deadline — expiry fails the checks that follow
    let input_a = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&input_a, 4).expect("request A size");
    pump_until_output(&mut daemon, &adapter_a, b"31 101\r\n", 5);
    adapter_b.inject_ingress_frame(compact_input_frame(1, b"report-b\n"));
    // timer: deadline — expiry fails the checks that follow
    let input_b = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&input_b, 6).expect("request B size");
    pump_until_output(&mut daemon, &adapter_b, b"24 80\r\n", 7);

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
    for operation_id in 1..=first_batch {
        adapter_a.inject_ingress_frame(compact_resize_frame(operation_id as u64, 31, 101));
    }
    // timer: deadline — expiry fails the checks that follow
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&wake, 3)
        .expect("accept first resize batch");
    assert_eq!(daemon.pending_terminal_resize_len(&session_a), first_batch);

    for operation_id in (first_batch + 1)..=PENDING_INGRESS_RESIZE_CAP {
        adapter_a.inject_ingress_frame(compact_resize_frame(operation_id as u64, 31, 101));
    }
    // timer: deadline — expiry fails the checks that follow
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon.pump_woken(&wake, 4).expect("fill pending cap");
    assert_eq!(
        daemon.pending_terminal_resize_len(&session_a),
        PENDING_INGRESS_RESIZE_CAP
    );
    assert_eq!(
        delivered_input_result_count(&adapter_a, 1..=PENDING_INGRESS_RESIZE_CAP as u64),
        0
    );

    adapter_a.inject_ingress_frame(compact_resize_frame(
        PENDING_INGRESS_RESIZE_CAP as u64 + 1,
        32,
        102,
    ));
    adapter_a.inject_ingress_frame(compact_input_frame(
        PENDING_INGRESS_RESIZE_CAP as u64 + 2,
        b"behind-resize\n",
    ));
    // timer: deadline — expiry fails the checks that follow
    let wake = daemon.wait_wakes(Duration::from_secs(1));
    daemon
        .pump_woken(&wake, 5)
        .expect("park the overflowing resize");
    assert_eq!(
        daemon.pending_terminal_resize_len(&session_a),
        PENDING_INGRESS_RESIZE_CAP
    );
    assert_eq!(
        delivered_input_result_count(&adapter_a, 1..=PENDING_INGRESS_RESIZE_CAP as u64),
        0
    );
    assert_eq!(
        delivered_input_result_count(
            &adapter_a,
            PENDING_INGRESS_RESIZE_CAP as u64 + 2..=PENDING_INGRESS_RESIZE_CAP as u64 + 2,
        ),
        0
    );

    hold.release();
    let resume_deadline = Instant::now() + Duration::from_secs(5);
    while daemon.pending_terminal_resize_len(&session_a) > 0
        || delivered_input_result_count(&adapter_a, 1..=PENDING_INGRESS_RESIZE_CAP as u64 + 1)
            < PENDING_INGRESS_RESIZE_CAP + 1
        || delivered_input_result_count(
            &adapter_a,
            PENDING_INGRESS_RESIZE_CAP as u64 + 2..=PENDING_INGRESS_RESIZE_CAP as u64 + 2,
        ) < 1
    {
        assert!(
            Instant::now() < resume_deadline,
            "parked owner did not resume from acknowledgement wakes"
        );
        // timer: deadline — wait for the next wake with the time left
        let batch = daemon.wait_wakes(resume_deadline.saturating_duration_since(Instant::now()));
        daemon
            .pump_woken(&batch, 6)
            .expect("resume parked resize from acknowledgement");
        assert!(
            daemon.pending_terminal_resize_len(&session_a) <= PENDING_INGRESS_RESIZE_CAP,
            "pending collection must stay at the ordinary-lane cap"
        );
    }
    assert_eq!(
        delivered_input_result_count(&adapter_a, 1..=PENDING_INGRESS_RESIZE_CAP as u64 + 1,),
        PENDING_INGRESS_RESIZE_CAP + 1
    );
    assert_eq!(
        delivered_input_result_count(
            &adapter_a,
            PENDING_INGRESS_RESIZE_CAP as u64 + 2..=PENDING_INGRESS_RESIZE_CAP as u64 + 2,
        ),
        1
    );
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

    adapter_a.inject_ingress_frame(compact_resize_frame(1, 24, 80));
    adapter_a.inject_ingress_frame(compact_resize_frame(2, 24, 80));
    adapter_a.inject_ingress_frame(compact_resize_frame(3, 31, 91));
    // A full control queue parks a resize until the writer frees space and
    // wakes the session, so the three may take more than one pump.
    pump_until(
        &mut daemon,
        "all three resizes submitted and pending",
        Duration::from_secs(1),
        3,
        |daemon| daemon.pending_terminal_resize_len(&session_a) == 3,
    );
    assert_eq!(delivered_input_result_count(&adapter_a, 1..=3), 0);
    let record = daemon
        .registry()
        .load(&session_a)
        .expect("load")
        .expect("record");
    assert_eq!((record.rows, record.cols), (24, 80));

    hold.release();
    pump_until_registry_size(&mut daemon, &session_a, 31, 91, 4);
    assert_eq!(delivered_input_result_count(&adapter_a, 1..=3), 3);
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

    adapter_a.inject_ingress_frame(compact_resize_frame(1, 31, 101));
    // timer: deadline — expiry fails the checks that follow
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
    pump_until_registry_size(&mut daemon, &session_b, 30, 90, 5);
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

    adapter_a.inject_ingress_frame(compact_resize_frame(1, 31, 101));
    // timer: deadline — expiry fails the checks that follow
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
    // Pump whatever the teardown left queued; nothing waits for more.
    let late = daemon.wait_wakes(Duration::ZERO);
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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
            Box::new(adapter.clone()),
        )
        .expect("bind");
    assert_eq!(daemon.wake_source().registry_len(), 1);
    assert!(daemon
        .wake_source()
        .registry_contains(&session_id, &subscription_id));
    // The adapter raises a writable wake; wait for the batch that names it.
    assert!(adapter.wake(TerminalWakeKind::Writable));
    let batch = wait_for(
        "the bound route's wake",
        Duration::from_secs(5),
        |remaining| {
            // timer: deadline — wait_for's remaining bound; expiry fails the test
            let batch = daemon.wait_wakes(remaining);
            (!batch.adapter_routes.is_empty()).then_some(batch)
        },
    );
    assert_eq!(
        batch
            .adapter_routes
            .iter()
            .map(|route| &route.subscription_id)
            .collect::<Vec<_>>(),
        vec![&subscription_id],
        "one waking bind names exactly its own route"
    );
    let outcome = daemon.pump_woken(&batch, 4).expect("pump");
    assert_eq!(
        outcome.pumped_routes, 1,
        "one waking bind must pump exactly one adapter route"
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

fn short_lived_spawn_request(session_id: &SessionId, done: &Fifo) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = format!(
        "printf ready; /bin/echo done > '{}'; exit 0",
        done.path().display()
    );
    request
}

/// Wait for the child to signal that it reached its marker.
fn wait_for_done_signal(done: &Fifo) {
    let _ = done.read_signal(Duration::from_secs(5));
}

/// Take the wakes already queued, without waiting and without pumping.
///
/// A caller first waits for a positive event that ends its setup (the
/// child's signal, an attach, a delivered result). Wakes coalesce into one
/// node per session and per route, so a wake that arrives after this drain
/// merges into the node the test reads next instead of adding a new one.
fn drain_follow_up_wakes(daemon: &mut CoreDaemon) {
    loop {
        let extra = daemon.wait_wakes(Duration::ZERO);
        if extra.adapter_routes.is_empty() && extra.ingress_sessions.is_empty() {
            return;
        }
    }
}

/// Pump the wakes already queued, without waiting. As with
/// [`drain_follow_up_wakes`], the caller first waits for the positive event
/// that ends its setup.
fn pump_queued_wakes(daemon: &mut CoreDaemon, now_seconds: u64) {
    loop {
        let batch = daemon.wait_wakes(Duration::ZERO);
        if batch.adapter_routes.is_empty() && batch.ingress_sessions.is_empty() {
            return;
        }
        daemon
            .pump_woken(&batch, now_seconds)
            .expect("pump queued setup wakes");
    }
}

fn consume_runtime_ingress_wakes(daemon: &mut CoreDaemon, session_id: &SessionId) {
    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(5));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == session_id),
        "setup must consume a runtime session ingress wake before the target drain, got {batch:?}"
    );
    drain_follow_up_wakes(daemon);
}

fn finish_short_lived_runtime_setup(daemon: &mut CoreDaemon, session_id: &SessionId, done: &Fifo) {
    wait_for_done_signal(done);
    consume_runtime_ingress_wakes(daemon, session_id);
}

/// Observe `session_id` without pumping until `done`, waiting for a wake
/// between observes. The child's exit and its PTY end each wake the session
/// once visible, so an observe that misses them is followed by a wake.
fn observe_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    now: u64,
    what: &str,
    mut done: impl FnMut(&mut CoreDaemon) -> bool,
) {
    wait_for(what, Duration::from_secs(8), |remaining| {
        daemon
            .observe_session_lifecycle(session_id, now)
            .expect("observe without a pump");
        if done(daemon) {
            return Some(());
        }
        // timer: deadline — wait_for's bound limits this wait
        let _ = daemon.wait_wakes(remaining);
        None
    });
}

fn observe_until_exited_without_pump(daemon: &mut CoreDaemon, session_id: &SessionId, now: u64) {
    observe_until(
        daemon,
        session_id,
        now,
        "observe committing Exited without a pump",
        |daemon| {
            matches!(
                daemon
                    .session_registry_state(session_id)
                    .expect("registry after observe"),
                SessionRegistryStateLookup::Found(RegistrySessionState::Exited)
            )
        },
    );
}

fn adapter_has_process_exit(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
        .any(|frame| frame.kind() == TerminalKind::ProcessExit)
}

fn adapter_has_attached(adapter: &SharedFakeTerminalAdapter) -> bool {
    adapter
        .snapshot_delivered_frame_bytes()
        .iter()
        .filter_map(|bytes| TerminalFrame::from_bytes(bytes).ok())
        .any(|frame| {
            frame.kind() == TerminalKind::AttachState
                && decode_attach_state(&frame).ok() == Some(AttachStateCode::Attached)
        })
}

fn bind_short_lived_session(
    daemon: &mut CoreDaemon,
    label: &str,
    adapter: SharedFakeTerminalAdapter,
) -> (SessionId, ClientId, SubscriptionId) {
    let session_id = SessionId(format!("{label}-session"));
    let client_id = ClientId(format!("{label}-client"));
    let subscription_id = SubscriptionId(format!("{label}-sub"));
    let done = Fifo::new("wake-done");
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
    (session_id, client_id, subscription_id)
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
    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(
        batch.ingress_sessions.iter().any(|id| id == session_id),
        "observe must emit a session ingress wake, got {batch:?}"
    );
    let outcome = daemon.pump_woken(&batch, 23).expect("targeted pump");
    assert!(outcome.terminal_inventory_changed);
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
    let go = Fifo::new("go");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; /bin/cat '{}' >/dev/null; exit 0",
        go.path().display()
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
    pump_until(
        &mut daemon,
        "worker attach",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    pump_queued_wakes(&mut daemon, 3);
    go.release(Duration::from_secs(5));
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
    let done = Fifo::new("child-done");
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
    // timer: deadline — expiry fails the checks that follow
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
    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    let outcome = daemon.pump_woken(&batch, 4).expect("pump closed adapter");
    assert_eq!(outcome.pumped_routes, batch.adapter_routes.len());
    assert!(outcome.terminal_inventory_changed);
    let unchanged = daemon
        .pump_woken(&TerminalWakeBatch::default(), 5)
        .expect("unchanged follow-up pump");
    assert!(!unchanged.terminal_inventory_changed);
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
fn natural_exit_coalesces_sibling_removals_and_later_pump_is_unchanged() {
    let data_dir = temp_data_dir("natural-exit-siblings");
    let done = Fifo::new("done");
    let go = Fifo::new("go");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("natural-exit-siblings-session".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; /bin/cat '{}' >/dev/null; /bin/echo done > '{}'; exit 0",
        go.path().display(),
        done.path().display()
    );
    daemon.spawn(request, 1).expect("spawn gated session");

    let routes = [
        (
            ClientId("natural-exit-siblings-client-a".into()),
            SubscriptionId("natural-exit-siblings-sub-a".into()),
            SharedFakeTerminalAdapter::auto_complete(),
        ),
        (
            ClientId("natural-exit-siblings-client-b".into()),
            SubscriptionId("natural-exit-siblings-sub-b".into()),
            SharedFakeTerminalAdapter::auto_complete(),
        ),
    ];
    for (client_id, subscription_id, adapter) in &routes {
        daemon
            .expect_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            )
            .expect("declare sibling adapter");
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                2,
            )
            .expect("attach sibling route");
        let generation = daemon
            .terminal_subscription_generation(&session_id, subscription_id)
            .expect("sibling generation");
        daemon
            .bind_waking_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                generation,
                empty_caps(),
                Box::new(adapter.clone()),
            )
            .expect("bind sibling adapter");
    }
    pump_until(
        &mut daemon,
        "sibling attaches",
        Duration::from_secs(8),
        2,
        |daemon| {
            routes
                .iter()
                .all(|(_, _, adapter)| adapter_settled(adapter))
                && !daemon.capture_active(&session_id)
        },
    );
    pump_queued_wakes(&mut daemon, 2);

    go.release(Duration::from_secs(5));
    wait_for_done_signal(&done);
    consume_runtime_ingress_wakes(&mut daemon, &session_id);
    observe_until_exited_without_pump(&mut daemon, &session_id, 3);
    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(2));
    assert!(batch.ingress_sessions.contains(&session_id));
    let outcome = daemon.pump_woken(&batch, 4).expect("deliver process exit");
    assert_eq!(outcome.pumped_routes, batch.adapter_routes.len());
    assert!(outcome.terminal_inventory_changed);
    assert!(routes.iter().all(|(_, subscription_id, adapter)| {
        adapter.snapshot_pressure() == TerminalAdapterPressure::Closed
            && daemon
                .terminal_subscription_generation(&session_id, subscription_id)
                .is_none()
    }));

    let unchanged = daemon
        .pump_woken(&TerminalWakeBatch::default(), 5)
        .expect("later unchanged pump");
    assert!(!unchanged.terminal_inventory_changed);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn ordinary_pty_output_does_not_report_inventory_change() {
    let data_dir = temp_data_dir("ordinary-output-inventory");
    let go = Fifo::new("go");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("ordinary-output-inventory-session".into());
    let client_id = ClientId("ordinary-output-inventory-client".into());
    let subscription_id = SubscriptionId("ordinary-output-inventory-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "/bin/cat '{}' >/dev/null; printf ordinary-output; exec cat >/dev/null",
        go.path().display()
    );
    daemon.spawn(request, 1).expect("spawn gated session");
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
        .terminal_subscription_generation(&session_id, &subscription_id)
        .expect("route generation");
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
        .expect("bind adapter");
    pump_until(
        &mut daemon,
        "attach before the gated output",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    pump_queued_wakes(&mut daemon, 2);

    go.release(Duration::from_secs(5));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "ordinary output did not arrive");
        // timer: deadline — wait for the next wake with the time left
        let batch = daemon.wait_wakes(deadline.saturating_duration_since(Instant::now()));
        let outcome = daemon.pump_woken(&batch, 3).expect("pump ordinary output");
        assert_eq!(outcome.pumped_routes, batch.adapter_routes.len());
        assert!(!outcome.terminal_inventory_changed);
        if adapter_output_contains(&adapter, b"ordinary-output") {
            break;
        }
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn abandoned_declaration_observe_retires_session_wake() {
    let data_dir = temp_data_dir("abandoned-declaration");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("abandoned-session".into());
    let client_id = ClientId("abandoned-client".into());
    let subscription_id = SubscriptionId("abandoned-sub".into());
    let done = Fifo::new("child-done");
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
        .list_terminal_subscriptions(1024 * 1024)
        .expect("test inventory allowance")
        .records
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
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
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
    sibling.inject_ingress_frame(compact_input_frame(1, b"MUST-STAY-QUEUED\n"));
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
    assert_eq!(delivered_input_result_count(&sibling, 1..=1), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn spurious_writable_wakes_resync_then_hard_stop_one_route() {
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
    let blocked_listed = |daemon: &CoreDaemon| {
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.session_id == session && row.subscription_id == sub)
    };
    let mut inventory_changes = 0;
    // The first exhausted write budget resyncs the route; the second, with
    // no successful write since that resync, ends it.
    for tick in 0..1024 {
        let _ = blocked.wake(TerminalWakeKind::Writable);
        let batch = daemon.wait_wakes(Duration::from_millis(0));
        let outcome = daemon
            .pump_woken(&batch, 20 + tick)
            .expect("pump blocked route");
        assert_eq!(outcome.pumped_routes, batch.adapter_routes.len());
        inventory_changes += usize::from(outcome.terminal_inventory_changed);
        if tick == 511 {
            assert!(
                blocked_listed(&daemon),
                "the first exhausted budget must resync, not end, the blocked route"
            );
        }
    }
    assert_eq!(inventory_changes, 1);
    assert!(
        !blocked_listed(&daemon),
        "1024 rejected Writable pumps must UnsubscribeSession the blocked route"
    );
    assert!(
        daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.session_id == sibling_session && row.subscription_id == sibling_sub),
        "sibling must survive the spurious-wake hard-stop"
    );
    let _ = (client, generation);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn malformed_input_reports_inventory_change() {
    let data_dir = temp_data_dir("malformed-inventory-change");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let (session_id, _, subscription_id, _) = bind_probe(
        &mut daemon,
        "malformed-inventory-session",
        "malformed-inventory-client",
        "malformed-inventory-sub",
        adapter.clone(),
    );
    pump_until(
        &mut daemon,
        "probe attach",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    pump_queued_wakes(&mut daemon, 2);

    adapter.inject_ingress_frame(vec![0xff, 0xff, 0xff]);
    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    let outcome = daemon.pump_woken(&batch, 3).expect("pump malformed input");
    assert_eq!(outcome.pumped_routes, batch.adapter_routes.len());
    assert!(outcome.terminal_inventory_changed);
    assert!(daemon
        .terminal_subscription_generation(&session_id, &subscription_id)
        .is_none());
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn outside_pump_replacement_wakes_and_failed_pump_does_not_acknowledge() {
    let data_dir = temp_data_dir("outside-pump-replacement");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("outside-pump-replacement-session".into());
    let subscription_id = SubscriptionId("outside-pump-replacement-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .attach(
            ClientId("outside-pump-client-a".into()),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach first owner");
    pump_queued_wakes(&mut daemon, 2);

    daemon
        .attach(
            ClientId("outside-pump-client-b".into()),
            session_id.clone(),
            subscription_id.clone(),
            3,
        )
        .expect("replace owner outside pump");
    assert!(daemon
        .pump_woken(
            &TerminalWakeBatch {
                adapter_routes: Vec::new(),
                ingress_sessions: vec![SessionId("unknown-session".into())],
            },
            4,
        )
        .is_err());

    // timer: deadline — expiry fails the checks that follow
    let batch = daemon.wait_wakes(Duration::from_secs(1));
    assert_eq!(batch.ingress_sessions, vec![session_id]);
    let outcome = daemon
        .pump_woken(&batch, 5)
        .expect("pump teardown notification");
    assert_eq!(outcome.pumped_routes, 0);
    assert!(outcome.terminal_inventory_changed);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn outside_pump_observe_hard_stop_wakes_without_later_traffic() {
    let data_dir = temp_data_dir("outside-pump-observe-hard-stop");
    let go = Fifo::new("go");
    let produced = Fifo::new("produced");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("outside-pump-observe-session".into());
    let client_id = ClientId("outside-pump-observe-client".into());
    let subscription_id = SubscriptionId("outside-pump-observe-sub".into());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "/bin/cat '{}' >/dev/null; /bin/echo produced > '{}'; dd if=/dev/zero bs=5242880 count=1 2>/dev/null; exec cat >/dev/null",
        go.path().display(),
        produced.path().display()
    );
    daemon.spawn(request, 1).expect("spawn output producer");
    let mut cleanup = ShutdownSessionOnDrop::new(&mut daemon, session_id.clone());
    cleanup
        .daemon()
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare held adapter");
    cleanup
        .daemon()
        .attach(client_id, session_id.clone(), subscription_id.clone(), 2)
        .expect("attach held route");
    drain_follow_up_wakes(cleanup.daemon());

    go.release(Duration::from_secs(5));
    wait_for_done_signal(&produced);
    drain_follow_up_wakes(cleanup.daemon());
    observe_until(
        cleanup.daemon(),
        &session_id,
        3,
        "observe hard-stopping the route",
        |daemon| {
            daemon
                .terminal_subscription_generation(&session_id, &subscription_id)
                .is_none()
        },
    );
    assert!(cleanup
        .daemon()
        .terminal_subscription_generation(&session_id, &subscription_id)
        .is_none());

    // timer: deadline — expiry fails the checks that follow
    let batch = cleanup.daemon().wait_wakes(Duration::from_secs(1));
    assert_eq!(batch.ingress_sessions, vec![session_id]);
    let outcome = cleanup
        .daemon()
        .pump_woken(&batch, 4)
        .expect("pump observe teardown notification");
    assert_eq!(outcome.pumped_routes, 0);
    assert!(outcome.terminal_inventory_changed);
    cleanup.shutdown(5);
    drop(cleanup);
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
    let stop_drainer = source.interrupt_handle();
    let drainer = thread::spawn(move || {
        drain_until_interrupted(&drain_source, &drain_stop, Duration::from_secs(5))
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
    stop.store(true, Ordering::Release);
    // An interrupt sent before the drainer waits is kept as pending.
    stop_drainer.interrupt();
    let drain_worst = drainer
        .join()
        .expect("drain thread")
        .expect("the stop interrupt ends the drainer");
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
    // timer: deadline — expiry fails the checks that follow
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
    request.request.arguments[1] = "printf ready; exec cat >/dev/null".into();
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
    pump_until(
        &mut daemon,
        "worker attach",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    pump_queued_wakes(&mut daemon, 3);
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
    let go = Fifo::new("go");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        "printf ready; /bin/cat '{}' >/dev/null; exit 0",
        go.path().display()
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
    pump_until(
        &mut daemon,
        "worker attach",
        Duration::from_secs(8),
        2,
        |daemon| adapter_settled(&adapter) && !daemon.capture_active(&session_id),
    );
    pump_queued_wakes(&mut daemon, 3);
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
    go.release(Duration::from_secs(5));
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
                    arguments: vec![
                        "-c".to_string(),
                        "printf FINAL; exec cat >/dev/null".to_string(),
                    ],
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

/// Drain wakes, recording the worst occupancy, until the stop interrupt.
/// Expiry of the hang guard is a failure, never an exit: a lost stop
/// interrupt cannot pass through the deadline.
fn drain_until_interrupted(
    source: &botster_core::TerminalWakeSource,
    stop: &std::sync::atomic::AtomicBool,
    guard: Duration,
) -> Result<usize, &'static str> {
    let mut worst = 0usize;
    loop {
        // timer: deadline — a hang guard only; expiry fails the test
        match source.wait_wakes_interruptible(guard) {
            botster_core::TerminalWakeWait::Wakes(_) => {}
            botster_core::TerminalWakeWait::Interrupted
                if stop.load(std::sync::atomic::Ordering::Acquire) =>
            {
                return Ok(worst.max(source.occupancy()));
            }
            botster_core::TerminalWakeWait::Interrupted => {}
            botster_core::TerminalWakeWait::TimedOut => {
                return Err("the hang guard expired: the stop interrupt was lost");
            }
            _ => return Err("an unknown wait outcome"),
        }
        worst = worst.max(source.occupancy());
    }
}

/// With the stop flag set and no interrupt sent, the drainer reports its
/// hang guard's expiry instead of treating it as a stop.
#[test]
fn a_lost_stop_interrupt_fails_the_public_drainer_instead_of_passing() {
    let source = botster_core::TerminalWakeSource::new();
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert_eq!(
        drain_until_interrupted(&source, &stop, Duration::ZERO),
        Err("the hang guard expired: the stop interrupt was lost")
    );
}

#![cfg(unix)]
#![allow(missing_docs)]

use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet,
};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig, SpawnSessionRequest};
use botster_core_test_support::diagnostics::StepFailure;
use botster_core_test_support::fixture_gate::Fifo;
use botster_core_test_support::fixtures::paste::{
    control_byte_payload, encode_unbracketed_paste_for_pty, printable_payload,
};
use botster_core_test_support::route_observer::{RouteObserver, RouteState};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_ghostty::GhosttyTerminal;
use botster_terminal_protocol::{InputOutcome, TerminalFrame, TerminalKind};
use botster_terminal_protocol_client::{encode_paste, encode_terminal_input, TerminalInputCommand};

const STEP_DEADLINE: Duration = Duration::from_secs(8);
const PASTE_LEN: usize = 65_536;

struct WorkerHarness {
    daemon: CoreDaemon,
    data_dir: std::path::PathBuf,
}

impl WorkerHarness {
    fn new(label: &str) -> Self {
        let data_dir = temp_data_dir(label);
        let worker = botster_core_test_support::real_worker::WorkerBinary::from_env()
            .unwrap_or_else(|failure| panic!("{failure}"));
        Self {
            daemon: CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker.path)),
            data_dir,
        }
    }

    fn spawn_and_attach(
        &mut self,
        label: &str,
        script: String,
    ) -> (
        SessionId,
        SubscriptionId,
        SharedFakeTerminalAdapter,
        RouteObserver,
    ) {
        let session_id = SessionId(format!("{label}-session"));
        let client_id = ClientId(format!("{label}-client"));
        let subscription_id = SubscriptionId(format!("{label}-route"));
        let request = SpawnSessionRequest {
            request: SessionSpawnRequest {
                request_id: RequestId(format!("{label}-spawn")),
                session_id: session_id.clone(),
                executable: "sh".to_owned(),
                arguments: vec!["-c".to_owned(), script],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_owned(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            metadata: CoreSessionMetadata::new(),
        };
        self.daemon
            .spawn(request, 1)
            .expect("spawn real worker session");
        self.daemon
            .expect_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            )
            .expect("declare route adapter");
        self.daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                2,
            )
            .expect("attach route");
        let generation = self
            .daemon
            .terminal_subscription_generation(&session_id, &subscription_id)
            .expect("attached generation");
        let adapter = SharedFakeTerminalAdapter::auto_complete();
        self.daemon
            .bind_waking_terminal_adapter(
                client_id,
                session_id.clone(),
                subscription_id.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(adapter.clone()),
            )
            .expect("bind route adapter");
        let observer = RouteObserver::new("core", &session_id.0, &subscription_id.0, generation.0);
        (session_id, subscription_id, adapter, observer)
    }
}

impl Drop for WorkerHarness {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown(None, 99);
        let _ = fs::remove_dir_all(&self.data_dir);
    }
}

#[test]
fn c_s1_real_worker_raw_echo_and_natural_exit_end_with_process_exit() {
    let mut harness = WorkerHarness::new("c-s1");
    let gate = Fifo::new("c-s1-input-gate");
    let script = format!(
        "/bin/cat '{}' >/dev/null; stty -echo; printf C-S1-INPUT-READY; IFS= read -r line; printf 'C-S1-ECHO:%s\\n' \"$line\"; exit 0",
        gate.path().display()
    );
    let (session_id, subscription_id, adapter, mut observer) =
        harness.spawn_and_attach("c-s1", script);
    let mut cursor = 0;
    wait_for_state(
        &mut harness.daemon,
        &adapter,
        &subscription_id,
        &mut observer,
        &mut cursor,
        "attach_ready",
        |state| state.attached() && state.ready_seen,
    )
    .unwrap_or_else(|failure| panic!("{failure}"));

    observer.set_marker(b"C-S1-INPUT-READY");
    gate.release(Duration::from_secs(5));
    wait_for_state(
        &mut harness.daemon,
        &adapter,
        &subscription_id,
        &mut observer,
        &mut cursor,
        "shell_ready",
        RouteState::marker_seen,
    )
    .unwrap_or_else(|failure| panic!("{failure}"));

    let operation_id = 1;
    observer
        .expect_result(operation_id)
        .unwrap_or_else(|failure| panic!("{failure}"));
    observer.set_marker(b"C-S1-ECHO:raw-echo");
    let input = encode_terminal_input(&TerminalInputCommand::RawBytes {
        operation_id,
        data: b"raw-echo\n".to_vec(),
    })
    .expect("encode raw input");
    adapter.inject_ingress_frame(input.into_bytes());
    wait_for_state(
        &mut harness.daemon,
        &adapter,
        &subscription_id,
        &mut observer,
        &mut cursor,
        "raw_echo_exit",
        |state| {
            state.marker_seen() && state.has_result(operation_id) && state.process_exit.is_some()
        },
    )
    .unwrap_or_else(|failure| panic!("{failure}"));

    let result = observer
        .take_result(operation_id)
        .expect("raw input result");
    assert_eq!(result.outcome, InputOutcome::Written);
    assert_eq!(
        result.accepted_payload_bytes,
        Some(b"raw-echo\n".len() as u64)
    );
    assert_eq!(result.written_pty_bytes, Some(b"raw-echo\n".len() as u64));
    assert_eq!(observer.state().process_exit, Some(Some(0)));
    assert_eq!(
        observer.state().last_kinds().last(),
        Some(&TerminalKind::ProcessExit)
    );
    assert!(
        !harness
            .daemon
            .list_terminal_subscriptions(1024 * 1024)
            .expect("test inventory allowance")
            .records
            .iter()
            .any(|row| row.session_id == session_id),
        "the terminal route must end after PROCESS_EXIT"
    );
}

#[test]
fn c_s2_real_worker_paste_table_uses_production_ghostty_encoding() {
    struct Row {
        name: &'static str,
        payload: Vec<u8>,
        allow_unsafe: bool,
        outcome: InputOutcome,
    }

    let rows = [
        Row {
            name: "printable-safe",
            payload: printable_payload(PASTE_LEN),
            allow_unsafe: false,
            outcome: InputOutcome::Written,
        },
        Row {
            name: "control-rejected",
            payload: control_byte_payload(PASTE_LEN),
            allow_unsafe: false,
            outcome: InputOutcome::RejectedUnsafePaste,
        },
        Row {
            name: "control-allowed",
            payload: control_byte_payload(PASTE_LEN),
            allow_unsafe: true,
            outcome: InputOutcome::Written,
        },
    ];

    for (row_index, row) in rows.into_iter().enumerate() {
        let mut harness = WorkerHarness::new(row.name);
        let gate = Fifo::new("paste-gate");
        let sink = harness.data_dir.join("paste-sink");
        let script = format!(
            "/bin/cat '{}' >/dev/null; stty raw -echo; printf C-S2-SINK-READY; dd of='{}' bs=1 count={} 2>/dev/null; exit 0",
            gate.path().display(),
            sink.display(),
            PASTE_LEN
        );
        let (session_id, subscription_id, adapter, mut observer) =
            harness.spawn_and_attach(row.name, script);
        let mut cursor = 0;
        wait_for_state(
            &mut harness.daemon,
            &adapter,
            &subscription_id,
            &mut observer,
            &mut cursor,
            "attach_ready",
            |state| state.attached() && state.ready_seen,
        )
        .unwrap_or_else(|failure| panic!("row={} {failure}", row.name));
        observer.set_marker(b"C-S2-SINK-READY");
        gate.release(Duration::from_secs(5));
        wait_for_state(
            &mut harness.daemon,
            &adapter,
            &subscription_id,
            &mut observer,
            &mut cursor,
            "sink_ready",
            RouteState::marker_seen,
        )
        .unwrap_or_else(|failure| panic!("row={} {failure}", row.name));

        assert_eq!(
            GhosttyTerminal::paste_is_safe(&row.payload),
            row.name == "printable-safe"
        );
        let operation_id = row_index as u64 + 1;
        observer
            .expect_result(operation_id)
            .unwrap_or_else(|failure| panic!("row={} {failure}", row.name));
        for frame in encode_paste(operation_id, row.allow_unsafe, &row.payload)
            .expect("production client paste encoder")
        {
            adapter.inject_ingress_frame(frame.into_bytes());
        }
        wait_for_state(
            &mut harness.daemon,
            &adapter,
            &subscription_id,
            &mut observer,
            &mut cursor,
            "paste_result",
            |state| state.has_result(operation_id),
        )
        .unwrap_or_else(|failure| panic!("row={} {failure}", row.name));
        let result = observer.take_result(operation_id).expect("paste result");
        assert_eq!(result.outcome, row.outcome, "row={}", row.name);

        if row.outcome == InputOutcome::Written {
            wait_for_state(
                &mut harness.daemon,
                &adapter,
                &subscription_id,
                &mut observer,
                &mut cursor,
                "paste_exit",
                |state| state.process_exit.is_some(),
            )
            .unwrap_or_else(|failure| panic!("row={} {failure}", row.name));
            let expected_sink = encode_unbracketed_paste_for_pty(&row.payload)
                .expect("production Ghostty paste encoder");
            assert_eq!(
                fs::read(&sink).expect("paste sink bytes"),
                expected_sink,
                "row={}",
                row.name
            );
            assert_eq!(result.accepted_payload_bytes, Some(PASTE_LEN as u64));
            assert_eq!(result.written_pty_bytes, Some(expected_sink.len() as u64));
            assert_eq!(
                observer.state().last_kinds().last(),
                Some(&TerminalKind::ProcessExit)
            );
        } else {
            assert_eq!(result.written_pty_bytes, Some(0));
            assert_eq!(
                fs::metadata(&sink)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
                0
            );
            harness
                .daemon
                .shutdown(Some(session_id), 20)
                .expect("stop the rejected-paste row");
        }
    }
}

#[expect(
    clippy::result_large_err,
    reason = "Route failures deliberately retain rich harness diagnostics by value"
)]
fn wait_for_state(
    daemon: &mut CoreDaemon,
    adapter: &SharedFakeTerminalAdapter,
    subscription_id: &SubscriptionId,
    observer: &mut RouteObserver,
    cursor: &mut usize,
    step: &'static str,
    predicate: impl Fn(&RouteState) -> bool,
) -> Result<(), StepFailure> {
    let started = Instant::now();
    loop {
        observe_new_frames(adapter, subscription_id, observer, cursor)?;
        if predicate(observer.state()) {
            return Ok(());
        }
        if started.elapsed() >= STEP_DEADLINE {
            return Err(observer
                .failure(step, "deadline passed")
                .with_timing(STEP_DEADLINE, started));
        }
        let batch = daemon.wait_wakes(Duration::from_millis(100));
        daemon
            .pump_woken(&batch, 10)
            .map_err(|error| observer.failure(step, format!("wake pump failed: {error}")))?;
    }
}

#[expect(
    clippy::result_large_err,
    reason = "Route failures deliberately retain rich harness diagnostics by value"
)]
fn observe_new_frames(
    adapter: &SharedFakeTerminalAdapter,
    subscription_id: &SubscriptionId,
    observer: &mut RouteObserver,
    cursor: &mut usize,
) -> Result<(), StepFailure> {
    let delivered = adapter.delivered_frames_from(*cursor);
    *cursor += delivered.len();
    for delivered in delivered {
        if delivered.route != subscription_id.0 {
            return Err(observer.failure(
                "observe_route",
                format!(
                    "delivered route {} does not equal expected route {}",
                    delivered.route, subscription_id.0
                ),
            ));
        }
        if delivered.generation != observer.generation() {
            return Err(observer.failure(
                "observe_generation",
                format!(
                    "delivered generation {} does not equal expected generation {}",
                    delivered.generation,
                    observer.generation()
                ),
            ));
        }
        let frame = TerminalFrame::from_bytes(&delivered.bytes).map_err(|error| {
            observer.failure(
                "decode_container",
                format!("invalid terminal body: {error}"),
            )
        })?;
        observer.observe(delivered.stream_epoch, &frame)?;
    }
    Ok(())
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-harness-{label}-{nanos}"))
}

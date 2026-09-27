//! Local session worker process acceptance tests.
#![cfg(all(unix, feature = "local-runtime"))]

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use botster_core::{
    write_startup_failure, BackpressureSummary, CoreSessionMetadata, DefaultBotsterEngine,
    NotificationPayload, PromptMarkPayload, QueueSource, RequestId, ReservedSessionSpawnError,
    ResizePayload, SessionId, SessionMetadata, SessionReservationRelease, SessionReservationState,
    SessionRuntime, SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, StartupFailureOutcome,
    StartupFailureReport, SubscriptionId, TerminalMetadataShapingObservation,
    TerminalMetadataShapingOutcome, TerminalWakeSource, TransportEgress, WorkerBackedBotsterEngine,
    WorkerProcessRuntime, WorkerProcessRuntimeOptions, WorkerRouteProbe, WorkerRouteProbeEvent,
};
use botster_core_test_support::fixture_gate::wait_pid_exit;
use sha2::{Digest, Sha256};

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn worker_path() -> std::path::PathBuf {
    botster_core_test_support::real_worker::WorkerBinary::from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id(value: &str) -> SessionId {
    SessionId(value.to_string())
}

fn client_id(value: &str) -> botster_core::ClientId {
    botster_core::ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn process_exists(pid: u32) -> bool {
    // SAFETY: signal 0 performs the POSIX existence check without delivering a
    // signal to the target process.
    unsafe { kill(pid as i32, 0) == 0 }
}

fn process_argv(pid: u32) -> String {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .or_else(|_| {
            Command::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .arg("-o")
                .arg("command=")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        })
        .unwrap_or_default()
}

fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

fn shell_request(session_id: SessionId, script: &str) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("worker-spawn"),
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

fn worker_options() -> WorkerProcessRuntimeOptions {
    WorkerProcessRuntimeOptions {
        worker_path: worker_path(),
        egress_capacity: 64,
        pty_reader_chunk_capacity: 8,
        shutdown_grace_ms: 80,
        control_socket_dir: None,
        worker_reply_timeout: botster_core::DEFAULT_WORKER_REPLY_TIMEOUT,
        test_hold_after_read_ms: None,
        test_pending_capacity: None,
        test_hold_after_enqueue_ms: None,
        test_resize_ack_hold: None,
        test_route_probe: None,
        test_hold_before_exit_ms: None,
        test_exit_code: None,
        ghostty_max_scrollback_bytes: 10_000_000,
        terminal_color_profile: None,
    }
}

fn collect_until<F>(
    runtime: &mut dyn SessionRuntime,
    wakes: &TerminalWakeSource,
    session_id: &SessionId,
    mut predicate: F,
) -> Vec<SessionRuntimeOutput>
where
    F: FnMut(&[SessionRuntimeOutput]) -> bool,
{
    // timer: deadline — the output must arrive; the caller asserts on what came
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();

    loop {
        match runtime.drain_output(session_id) {
            Ok(drained) => output.extend(drained),
            Err(error) if error.kind == SessionRuntimeErrorKind::SessionNotFound => {}
            Err(error) => panic!("drain worker runtime output: {error}"),
        }
        if predicate(&output) {
            return output;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return output;
        };
        // timer: deadline — wait for the session's next worker event wake
        let _ = wakes.wait_wakes(remaining);
    }
}

fn output_text(output: &[SessionRuntimeOutput]) -> String {
    let bytes: Vec<u8> = output
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeOutput::PtyOutput { data, .. } => Some(data.as_slice()),
            SessionRuntimeOutput::ProcessExited { .. }
            | SessionRuntimeOutput::WorkerLost { .. }
            | SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::Backpressure(_)
            | SessionRuntimeOutput::MetadataShaping(_)
            | SessionRuntimeOutput::ModesChanged { .. } => None,
        })
        .flatten()
        .copied()
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn has_process_exit(output: &[SessionRuntimeOutput]) -> bool {
    output
        .iter()
        .any(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
}

fn backpressure(output: &[SessionRuntimeOutput]) -> Vec<BackpressureSummary> {
    output
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeOutput::Backpressure(summary) => Some(summary.clone()),
            _ => None,
        })
        .collect()
}

fn metadata_shaping(output: &[SessionRuntimeOutput]) -> Vec<TerminalMetadataShapingObservation> {
    output
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeOutput::MetadataShaping(observation) => Some(observation.clone()),
            _ => None,
        })
        .collect()
}

fn output_event_texts(output: &[SessionRuntimeOutput]) -> Vec<String> {
    output
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeOutput::PtyOutput { data, .. } => {
                Some(String::from_utf8_lossy(data).into_owned())
            }
            SessionRuntimeOutput::ProcessExited { .. }
            | SessionRuntimeOutput::WorkerLost { .. }
            | SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::Backpressure(_)
            | SessionRuntimeOutput::MetadataShaping(_)
            | SessionRuntimeOutput::ModesChanged { .. } => None,
        })
        .collect()
}

fn terminal_output_bytes(
    outcome: &botster_core::BotsterEngineOutput,
    client_id: &botster_core::ClientId,
    subscription_id: &SubscriptionId,
    session_id: &SessionId,
) -> Vec<u8> {
    outcome
        .client_egress
        .iter()
        .filter_map(|(received_client, egress)| match egress {
            TransportEgress::TerminalOutput {
                session_id: received_session,
                subscription_id: received_subscription,
                data,
            } if received_client == client_id
                && received_session == session_id
                && received_subscription == subscription_id =>
            {
                Some(data.as_slice())
            }
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn drain_engine_until<F>(
    engine: &mut WorkerBackedBotsterEngine,
    session_id: &SessionId,
    mut predicate: F,
) -> Vec<u8>
where
    F: FnMut(&[u8]) -> bool,
{
    let client = client_id("worker-client");
    let subscription = subscription_id("worker-sub");
    // timer: deadline — the output must arrive; the caller asserts on what came
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();

    loop {
        let outcome = engine
            .drain_runtime_once(session_id, 20)
            .expect("drain worker-backed engine");
        bytes.extend(terminal_output_bytes(
            &outcome,
            &client,
            &subscription,
            session_id,
        ));
        if predicate(&bytes) {
            return bytes;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return bytes;
        };
        // timer: deadline — wait for the engine's next session wake
        let _ = engine.wait_wakes(remaining);
    }
}

#[test]
fn failed_worker_start_does_not_leave_wake_registry_residue() {
    let source = TerminalWakeSource::new();
    let mut runtime =
        WorkerProcessRuntime::with_options(worker_options()).with_wake_source(source.clone());
    runtime.fail_next_start_writer();
    let session = session_id("failed-start-wake");
    let before = source.session_registry_len();
    let error = runtime
        .spawn_session(shell_request(session.clone(), "sleep 0.2"))
        .expect_err("injected start_writer failure");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert_eq!(source.session_registry_len(), before);
    assert!(
        !runtime.is_worker_process(&session),
        "failed start must not insert a live worker session"
    );
}

#[test]
fn worker_process_runtime_crosses_os_process_boundary_and_handles_protocol_commands() {
    let wakes = TerminalWakeSource::new();
    let mut runtime =
        WorkerProcessRuntime::with_options(worker_options()).with_wake_source(wakes.clone());
    let session = session_id("worker-process-protocol");

    runtime
        .spawn_session(shell_request(session.clone(), "sleep 0.2; stty size; cat"))
        .expect("spawn worker-owned session");

    let metadata = runtime
        .metadata(&session)
        .expect("worker welcome metadata should be recorded")
        .clone();
    assert!(
        metadata.recovery_identity.is_some(),
        "worker welcome should expose recovery_identity"
    );
    assert!(
        runtime.is_worker_process(&session),
        "runtime should own a live worker process instead of a local PTY handle"
    );

    runtime
        .set_reconnect_timeout(&session, 7)
        .expect("send FRAME_SET_TIMEOUT to worker");
    let health = runtime.ping(&session).expect("worker responds to ping");
    assert_eq!(health.session_id, session);
    assert_eq!(health.reconnect_timeout_seconds, Some(7));

    runtime
        .send_input(SessionRuntimeInput::Resize {
            session_id: session.clone(),
            size: ResizePayload { rows: 31, cols: 91 },
        })
        .expect("send resize frame");
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"hello\n".to_vec(),
        })
        .expect("send input frame");

    let echoed = collect_until(&mut runtime, &wakes, &session, |output| {
        let text = output_text(output);
        text.contains("31 91") && text.contains("hello")
    });
    let text = output_text(&echoed);
    assert!(text.contains("31 91"), "resize should reach worker PTY");
    assert!(text.contains("hello"), "input should reach worker PTY");

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown worker-owned PTY");
    let finished = collect_until(&mut runtime, &wakes, &session, has_process_exit);
    assert!(
        has_process_exit(&finished),
        "worker should emit process exit over protocol"
    );
}

#[test]
fn worker_process_runtime_emits_semantic_metadata_from_session_worker_output() {
    let mut options = worker_options();
    options.pty_reader_chunk_capacity = 8;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-semantic-metadata");
    let script =
        "printf '\\033]2;Build\\007\\033]7;file://host/work/repo\\007\\033]133;A\\007\\007\\033]9;Notice;Body\\007'; sleep 0.1";

    runtime
        .spawn_session(shell_request(session.clone(), script))
        .expect("spawn metadata worker session");

    let output = collect_until(&mut runtime, &wakes, &session, |output| {
        output.iter().any(|event| {
            matches!(
                event,
                SessionRuntimeOutput::TitleChanged { title, .. } if title == "Build"
            )
        }) && output.iter().any(|event| {
            matches!(
                event,
                SessionRuntimeOutput::CwdChanged { cwd, .. } if cwd == "/work/repo"
            )
        }) && output.iter().any(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PromptMark {
                    payload: PromptMarkPayload { mark },
                    ..
                } if mark == "A"
            )
        }) && output
            .iter()
            .any(|event| matches!(event, SessionRuntimeOutput::Bell { .. }))
            && output.iter().any(|event| {
                matches!(
                    event,
                    SessionRuntimeOutput::Notification {
                        payload: NotificationPayload { title, body },
                        ..
                    } if title == "Notice" && body == "Body"
                )
            })
    });

    let raw = output_text(&output);
    assert!(
        raw.contains("\u{1b}]2;Build\u{7}"),
        "raw PTY output should retain OSC title bytes"
    );
    assert!(
        raw.contains("\u{1b}]7;file://host/work/repo\u{7}"),
        "raw PTY output should retain OSC cwd bytes"
    );
}

#[test]
fn metadata_flood_reports_shaping_without_blocking_terminal_or_control_paths() {
    let mut options = worker_options();
    options.egress_capacity = 4;
    options.pty_reader_chunk_capacity = 4096;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-metadata-shaping");
    let titles = (0..40)
        .map(|index| format!("\\033]2;title-{index}\\007"))
        .collect::<String>();
    let script = format!("printf 'protected-ready\\n{titles}'; cat");

    runtime
        .spawn_session(shell_request(session.clone(), &script))
        .expect("spawn metadata flood worker session");

    let output = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("protected-ready")
            && metadata_shaping(output).iter().any(|observation| {
                matches!(
                    observation.outcome,
                    TerminalMetadataShapingOutcome::Accepted
                        | TerminalMetadataShapingOutcome::LatestWin
                ) && observation.count > 0
            })
    });
    let text = output_text(&output);
    assert!(
        text.contains("protected-ready"),
        "PTY output should still cross the worker path during metadata flood"
    );
    assert!(
        metadata_shaping(&output).iter().any(|observation| {
            matches!(
                observation.outcome,
                TerminalMetadataShapingOutcome::Accepted
                    | TerminalMetadataShapingOutcome::LatestWin
            ) && observation.count > 0
        }),
        "metadata flood should emit typed accepted/latest-win shaping observations"
    );

    let health = runtime
        .ping(&session)
        .expect("ping should not be blocked by metadata flood");
    assert_eq!(health.session_id, session);

    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"after-flood\n".to_vec(),
        })
        .expect("send input after metadata flood");
    let after_flood = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("after-flood")
    });
    assert!(
        output_text(&after_flood).contains("after-flood"),
        "future PTY traffic should still flow after metadata shaping"
    );
}

#[test]
fn metadata_overflow_reports_typed_drop_without_starving_worker_paths() {
    let mut options = worker_options();
    options.egress_capacity = 8;
    options.pty_reader_chunk_capacity = 4096;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-metadata-drop");
    let prompts = (0..20)
        .map(|index| format!("\\033]133;P{index}\\007"))
        .collect::<String>();
    let script = format!("printf 'drop-ready\\n{prompts}'; cat");

    runtime
        .spawn_session(shell_request(session.clone(), &script))
        .expect("spawn metadata drop worker session");

    let output = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("drop-ready")
            && metadata_shaping(output).iter().any(|observation| {
                observation.outcome == TerminalMetadataShapingOutcome::Dropped
                    && observation.count > 0
            })
    });
    assert!(
        output_text(&output).contains("drop-ready"),
        "PTY output should cross the worker path while metadata is dropped"
    );
    assert!(
        metadata_shaping(&output).iter().any(|observation| {
            observation.outcome == TerminalMetadataShapingOutcome::Dropped && observation.count > 0
        }),
        "bounded metadata overflow should emit typed dropped observations"
    );

    let health = runtime
        .ping(&session)
        .expect("ping should survive metadata overflow");
    assert_eq!(health.session_id, session);
}

#[test]
fn ordering_significant_metadata_flushes_before_later_pty_output() {
    let mut options = worker_options();
    options.pty_reader_chunk_capacity = 16;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-metadata-order");
    let script = "printf '\\033]133;A\\007'; sleep 0.1; printf 'after-prompt\\n'; cat";

    runtime
        .spawn_session(shell_request(session.clone(), script))
        .expect("spawn metadata ordering worker session");

    let output = collect_until(&mut runtime, &wakes, &session, |output| {
        output.iter().any(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PromptMark {
                    payload: PromptMarkPayload { mark },
                    ..
                } if mark == "A"
            )
        }) && output_text(output).contains("after-prompt")
    });

    let prompt_index = output
        .iter()
        .position(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PromptMark {
                    payload: PromptMarkPayload { mark },
                    ..
                } if mark == "A"
            )
        })
        .expect("prompt mark metadata event");
    let later_output_index = output
        .iter()
        .position(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PtyOutput { data, .. }
                    if String::from_utf8_lossy(data).contains("after-prompt")
            )
        })
        .expect("later PTY output event");
    assert!(
        prompt_index < later_output_index,
        "ordering-significant side-band metadata should flush before later PTY output"
    );
}

#[test]
fn worker_backed_public_engine_path_routes_spawn_input_resize_output_and_shutdown() {
    let mut engine = DefaultBotsterEngine::worker_backed(worker_path());
    let session = session_id("worker-public-path");
    let client = client_id("worker-client");
    let subscription = subscription_id("worker-sub");

    engine
        .spawn_session(
            shell_request(
                session.clone(),
                "printf 'public-ready\\n'; sleep 0.2; stty size; cat",
            ),
            CoreSessionMetadata::new(),
        )
        .expect("spawn through public worker-backed facade");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 10)
        .expect("attach public path consumer");

    engine
        .resize(client.clone(), session.clone(), 28, 88, 11)
        .expect("resize through public worker-backed path");
    engine
        .write_bytes(client, session.clone(), b"world\n".to_vec(), 12)
        .expect("write input through public worker-backed path");

    let output = drain_engine_until(&mut engine, &session, |bytes| {
        let text = String::from_utf8_lossy(bytes);
        text.contains("28 88") && text.contains("world")
    });
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("28 88"));
    assert!(text.contains("world"));

    let health = engine
        .session_runtime_mut()
        .ping(&session)
        .expect("public facade runtime sends ping to worker process");
    assert_eq!(health.session_id, session);

    engine
        .shutdown_session(health.session_id, "test shutdown", 13)
        .expect("shutdown through public worker-backed path");
}

#[test]
fn detaching_one_client_does_not_starve_other_subscribers() {
    let mut engine = DefaultBotsterEngine::worker_backed(worker_path());
    let session = session_id("worker-multi-client");
    let client_a = client_id("worker-client-a");
    let client_b = client_id("worker-client-b");
    let sub_a = subscription_id("worker-sub-a");
    let sub_b = subscription_id("worker-sub-b");

    engine
        .spawn_session(
            shell_request(session.clone(), "printf 'multi-ready\\n'; cat"),
            CoreSessionMetadata::new(),
        )
        .expect("spawn worker-backed session");
    engine
        .attach_client(client_a.clone(), session.clone(), sub_a.clone(), 10)
        .expect("attach first client");
    engine
        .attach_client(client_b.clone(), session.clone(), sub_b.clone(), 10)
        .expect("attach second client");
    engine
        .detach_client(client_a.clone(), session.clone(), sub_a.clone(), 11)
        .expect("detach first client only");
    engine
        .write_bytes(
            client_b.clone(),
            session.clone(),
            b"still-attached\n".to_vec(),
            12,
        )
        .expect("write after one client detaches");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut client_b_bytes = Vec::new();
    while Instant::now() < deadline {
        let outcome = engine
            .drain_runtime_once(&session, 20)
            .expect("drain multi-client worker-backed session");
        client_b_bytes.extend(terminal_output_bytes(&outcome, &client_b, &sub_b, &session));
        assert!(
            terminal_output_bytes(&outcome, &client_a, &sub_a, &session).is_empty(),
            "detached client should not receive terminal output"
        );
        if String::from_utf8_lossy(&client_b_bytes).contains("still-attached") {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        String::from_utf8_lossy(&client_b_bytes).contains("still-attached"),
        "still-attached subscriber should receive output after another client detaches"
    );
    engine
        .shutdown_session(session, "test shutdown", 13)
        .expect("shutdown multi-client worker-backed session");
}

#[test]
fn detach_reattach_keeps_worker_live_and_bounded_egress_reports_pressure() {
    let mut options = worker_options();
    options.egress_capacity = 2;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-detach-reattach");

    runtime
        .spawn_session(shell_request(
            session.clone(),
            "i=0; while [ $i -lt 2000 ]; do printf \"tick:$i\\n\"; i=$((i+1)); done; cat",
        ))
        .expect("spawn noisy worker session");

    runtime
        .detach_consumer(&session)
        .expect("detach parent-side consumer without protocol frame");
    thread::sleep(Duration::from_millis(500));
    let detached = runtime
        .drain_output(&session)
        .expect("detached drain should not block worker");
    assert!(
        backpressure(&detached)
            .iter()
            .any(|summary| summary.source == QueueSource::SessionIo),
        "bounded parent egress should report typed pressure while detached"
    );
    let retained = output_event_texts(&detached).join("");
    assert!(
        retained.contains("tick:0"),
        "bounded egress should retain the earliest PTY output"
    );
    if let (Some(first), Some(second)) = (retained.find("tick:0"), retained.find("tick:1")) {
        assert!(
            first <= second,
            "retained PTY bytes should preserve source order"
        );
    }

    let health = runtime
        .ping(&session)
        .expect("worker event loop stays live");
    assert_eq!(health.session_id, session);

    runtime
        .attach_consumer(&session)
        .expect("reattach parent-side consumer");
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"again\n".to_vec(),
        })
        .expect("send input after reattach");

    let reattached = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("again")
    });
    assert!(
        output_text(&reattached).contains("again"),
        "reattach should receive future worker PTY output"
    );

    let protocol_source = include_str!("../src/contract/session_protocol.rs");
    assert!(!protocol_source.contains("FRAME_ATTACH"));
    assert!(!protocol_source.contains("FRAME_DETACH"));
}

#[test]
fn attached_capacity_one_retains_process_echo_after_terminal_echo() {
    let mut options = worker_options();
    options.egress_capacity = 1;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-attached-process-echo");

    runtime
        .spawn_session(shell_request(
            session.clone(),
            "printf 'ready\\n'; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done",
        ))
        .expect("spawn capacity-one echo worker");
    runtime
        .attach_consumer(&session)
        .expect("attach parent consumer before live output");

    let ready = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("ready")
    });
    assert!(
        output_text(&ready).contains("ready"),
        "pre-input ready marker should drain"
    );

    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"FILL-SLOT\n".to_vec(),
        })
        .expect("fill the one-slot parent channel");
    thread::sleep(Duration::from_millis(80));
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"POST-BARRIER-MARKER\n".to_vec(),
        })
        .expect("write queued marker into the PTY");
    // Hold the parent drain so the marker races a full one-slot channel.
    // A try_send drop keeps FILL-SLOT and loses echo:POST-BARRIER-MARKER.
    thread::sleep(Duration::from_millis(80));

    let live = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("echo:POST-BARRIER-MARKER")
    });
    let text = output_text(&live);
    assert!(
        text.contains("echo:POST-BARRIER-MARKER"),
        "attached capacity-one egress must retain process echo after terminal echo; last output: {text:?}"
    );
}

fn capacity_one_engine() -> WorkerBackedBotsterEngine {
    let mut options = worker_options();
    options.egress_capacity = 1;
    WorkerBackedBotsterEngine::with_options(options)
}

fn echo_script() -> &'static str {
    "printf 'ready\\n'; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
}

fn drain_until_attached(
    engine: &mut WorkerBackedBotsterEngine,
    session: &SessionId,
    client: &botster_core::ClientId,
) {
    // timer: deadline — the attach must reach Attached; expiry fails the test
    let deadline = Instant::now() + Duration::from_secs(10);
    for tick in 0u64.. {
        let outcome = engine
            .drain_runtime_once(session, 20 + tick)
            .expect("drain until Attached");
        let attached = outcome.client_egress.iter().any(|(target, frame)| {
            target == client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        state: botster_core::TerminalAttachState::Attached,
                        ..
                    }
                )
        });
        if attached {
            return;
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("attach did not reach Attached");
        // timer: deadline — wait for the engine's next session wake
        let _ = engine.wait_wakes(remaining);
    }
}

fn drain_engine_text_for(
    engine: &mut WorkerBackedBotsterEngine,
    session: &SessionId,
    client: &botster_core::ClientId,
    subscription: &SubscriptionId,
    expected: &str,
) -> String {
    // timer: deadline — the text must arrive; the caller asserts on what came
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    loop {
        let outcome = engine
            .drain_runtime_once(session, 20)
            .expect("drain worker-backed engine");
        bytes.extend(terminal_output_bytes(
            &outcome,
            client,
            subscription,
            session,
        ));
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.contains(expected) {
            return text;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return text;
        };
        // timer: deadline — wait for the engine's next session wake
        let _ = engine.wait_wakes(remaining);
    }
}

fn write_and_hold(
    engine: &mut WorkerBackedBotsterEngine,
    client: &botster_core::ClientId,
    session: &SessionId,
    data: &[u8],
) {
    engine
        .write_bytes(client.clone(), session.clone(), data.to_vec(), 30)
        .expect("write bytes");
    thread::sleep(Duration::from_millis(80));
}

#[test]
fn takeover_then_full_detach_restores_overflow_progress() {
    let mut engine = capacity_one_engine();
    let session = session_id("owner-takeover-detach");
    let first = client_id("owner-takeover-a");
    let second = client_id("owner-takeover-b");
    let subscription = subscription_id("owner-takeover-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), echo_script()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(first, session.clone(), subscription.clone(), 10)
        .expect("attach first");
    drain_until_attached(&mut engine, &session, &client_id("owner-takeover-a"));
    engine
        .attach_client(second.clone(), session.clone(), subscription.clone(), 11)
        .expect("same-key takeover");
    drain_until_attached(&mut engine, &session, &second);
    engine
        .detach_client(second.clone(), session.clone(), subscription, 12)
        .expect("full detach after takeover");

    write_and_hold(&mut engine, &second, &session, b"FILL-SLOT\n");
    write_and_hold(&mut engine, &second, &session, b"POST-BARRIER-MARKER\n");
    let started = Instant::now();
    let _ = engine
        .drain_runtime_once(&session, 40)
        .expect("drain after detach");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "detached overflow must not stall the parent drain"
    );
}

#[test]
fn generation_detach_restores_overflow_progress() {
    let mut engine = capacity_one_engine();
    let session = session_id("owner-generation-detach");
    let client = client_id("owner-generation-client");
    let subscription = subscription_id("owner-generation-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), echo_script()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 10)
        .expect("attach");
    drain_until_attached(&mut engine, &session, &client);
    let generation = engine
        .terminal_subscription_generation(&session, &subscription)
        .expect("live generation");
    engine
        .detach_terminal_subscription(
            client.clone(),
            session.clone(),
            subscription,
            generation,
            11,
        )
        .expect("generation detach");

    write_and_hold(&mut engine, &client, &session, b"FILL-SLOT\n");
    write_and_hold(&mut engine, &client, &session, b"POST-BARRIER-MARKER\n");
    let started = Instant::now();
    let _ = engine.drain_runtime_once(&session, 40).expect("drain");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "generation detach must not leave a leaked stall"
    );
}

#[test]
fn stale_detach_keeps_sibling_process_echo() {
    let mut engine = capacity_one_engine();
    let session = session_id("owner-stale-sibling");
    let first = client_id("owner-stale-a");
    let sibling = client_id("owner-stale-b");
    let first_sub = subscription_id("owner-stale-a-sub");
    let sibling_sub = subscription_id("owner-stale-b-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), echo_script()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(first.clone(), session.clone(), first_sub.clone(), 10)
        .expect("attach first");
    engine
        .attach_client(sibling.clone(), session.clone(), sibling_sub.clone(), 11)
        .expect("attach sibling");
    drain_until_attached(&mut engine, &session, &first);
    drain_until_attached(&mut engine, &session, &sibling);
    engine
        .detach_client(first.clone(), session.clone(), first_sub.clone(), 12)
        .expect("detach first");
    engine
        .detach_client(first, session.clone(), first_sub, 13)
        .expect("stale second detach");

    write_and_hold(&mut engine, &sibling, &session, b"FILL-SLOT\n");
    write_and_hold(&mut engine, &sibling, &session, b"POST-BARRIER-MARKER\n");
    let text = drain_engine_text_for(
        &mut engine,
        &session,
        &sibling,
        &sibling_sub,
        "echo:POST-BARRIER-MARKER",
    );
    assert!(
        text.contains("echo:POST-BARRIER-MARKER"),
        "live sibling must keep process echo after stale detach; last output: {text:?}"
    );
}

#[test]
fn detach_while_stalled_unblocks_parent() {
    let mut engine = capacity_one_engine();
    let session = session_id("owner-detach-stalled");
    let client = client_id("owner-detach-stalled-client");
    let subscription = subscription_id("owner-detach-stalled-sub");
    engine
        .spawn_session(
            shell_request(session.clone(), echo_script()),
            CoreSessionMetadata::new(),
        )
        .expect("spawn");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 10)
        .expect("attach");
    drain_until_attached(&mut engine, &session, &client);
    write_and_hold(&mut engine, &client, &session, b"FILL-SLOT\n");
    engine
        .write_bytes(
            client.clone(),
            session.clone(),
            b"POST-BARRIER-MARKER\n".to_vec(),
            31,
        )
        .expect("second write while the one-slot channel is full");
    let started = Instant::now();
    engine
        .detach_client(client, session.clone(), subscription, 32)
        .expect("detach while sender is stalled");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "detach must stop the attached stall"
    );
    let _ = engine
        .drain_runtime_once(&session, 40)
        .expect("drain after stalled detach");
}

#[test]
fn attached_pty_stall_waits_on_drain_or_detach_not_fixed_sleep() {
    let source = include_str!("../src/runtime/worker_process.rs");
    let start = source
        .find("fn send_worker_event(")
        .expect("send_worker_event must exist");
    // Bound the scan at the function that follows send_worker_event, and
    // fail if that boundary moves, so the scan cannot widen silently.
    let end = source[start..]
        .find("fn next_request_id(")
        .expect("next_request_id must follow send_worker_event");
    let body = &source[start..start + end];
    assert!(
        !body.contains("thread::sleep"),
        "attached PtyOutput stall must not poll with a fixed sleep"
    );
    assert!(
        !body.contains("from_millis(1)"),
        "attached PtyOutput stall must not use a 1 ms interval"
    );
    assert!(
        source.contains("wait_for_space_or_detach"),
        "attached stall must wait on the drain/detach condvar"
    );
    assert!(
        source.contains("struct EgressStall"),
        "attached stall must use the std Condvar gate"
    );
    let drop_impl = source
        .split("impl Drop for WorkerProcessRuntime")
        .nth(1)
        .and_then(|rest| rest.split("impl WorkerProcessSession").next())
        .expect("WorkerProcessRuntime Drop must exist");
    let close_at = drop_impl
        .find("close_before_blocking_shutdown")
        .expect("runtime Drop must notify EgressStall before close");
    let kill_at = drop_impl
        .find("child.kill()")
        .expect("runtime Drop must kill the worker child");
    assert!(
        drop_impl.contains("reap_worker_child_in_background"),
        "runtime Drop must background-reap instead of blocking on child.wait"
    );
    assert!(
        !drop_impl.contains("child.wait()"),
        "runtime Drop must not block on child.wait while the writer can still hold stdin"
    );
    assert!(
        close_at < kill_at,
        "EgressStall close must run before kill so attached pressure cannot deadlock shutdown"
    );
}

#[test]
fn dropping_parent_runtime_reaps_worker_and_pty_child() {
    let session = session_id("worker-parent-drop-cleanup");
    let (worker_pid, pty_child_pid) = {
        let mut runtime = WorkerProcessRuntime::with_options(worker_options());
        runtime
            .spawn_session(shell_request(session.clone(), "cat"))
            .expect("spawn long-lived worker session");
        let metadata = runtime.metadata(&session).expect("worker metadata").clone();
        let worker_pid = metadata
            .recovery_identity
            .as_ref()
            .and_then(|identity| identity.get("worker_pid"))
            .and_then(serde_json::Value::as_u64)
            .expect("worker pid in recovery identity") as u32;
        (worker_pid, metadata.pid)
    };

    assert!(
        wait_pid_exit(worker_pid, Duration::from_secs(5)),
        "dropping parent runtime should reap worker process {worker_pid}"
    );
    assert!(
        wait_pid_exit(pty_child_pid, Duration::from_secs(5)),
        "dropping parent runtime should clean worker PTY child {pty_child_pid}"
    );
}

#[test]
fn attached_capacity_one_close_reaps_stalled_worker_and_pty_child() {
    let mut options = worker_options();
    options.egress_capacity = 1;
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-attached-close-stall");

    runtime
        .spawn_session(shell_request(
            session.clone(),
            "i=0; while :; do printf \"tick:%s\\n\" \"$i\"; i=$((i+1)); done",
        ))
        .expect("spawn sustained PTY producer");
    runtime
        .attach_consumer(&session)
        .expect("attach parent consumer so live output stalls");

    let started_output = collect_until(&mut runtime, &wakes, &session, |output| {
        output_text(output).contains("tick:")
    });
    assert!(
        output_text(&started_output).contains("tick:"),
        "sustained producer must emit live PTY bytes before close"
    );
    // Stop draining so the one-slot channel stays full and the attached
    // stdout reader waits on EgressStall. Keep producing until the worker
    // pipe fills; that is the shutdown cycle the close notification breaks.
    thread::sleep(Duration::from_millis(300));

    let metadata = runtime.metadata(&session).expect("worker metadata").clone();
    let worker_pid = metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32;
    let pty_child_pid = metadata.pid;

    let dropped = Arc::new(AtomicBool::new(false));
    let drop_flag = Arc::clone(&dropped);
    let started = Instant::now();
    thread::spawn(move || {
        drop(runtime);
        drop_flag.store(true, Ordering::SeqCst);
    });

    let drop_finished = wait_until(|| dropped.load(Ordering::SeqCst));
    if !drop_finished {
        // SAFETY: signal 9 is SIGKILL. The parent drop is stuck, so the test
        // must reap leftovers before later cases observe leaked PIDs.
        unsafe {
            let _ = kill(worker_pid as i32, 9);
            let _ = kill(pty_child_pid as i32, 9);
        }
    }
    assert!(
        drop_finished,
        "parent drop must finish while attached capacity-one stall is under sustained PTY pressure"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "parent drop must stay bounded; elapsed={:?}",
        started.elapsed()
    );
    assert!(
        wait_pid_exit(worker_pid, Duration::from_secs(5)),
        "dropping parent runtime should reap worker process {worker_pid}"
    );
    assert!(
        wait_pid_exit(pty_child_pid, Duration::from_secs(5)),
        "dropping parent runtime should clean worker PTY child {pty_child_pid}"
    );
}

#[test]
fn worker_control_endpoints_are_bounded_for_canonical_and_long_session_ids() {
    let control_dir = temp_control_dir("bwid");
    let canonical = session_id("123e4567-e89b-12d3-a456-426614174000");
    let long = session_id(&format!("sess-long-{}", "identifier-".repeat(100)));
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());

    let canonical_handle = runtime
        .spawn_session(shell_request(canonical.clone(), "cat"))
        .expect("spawn canonical session id");
    let long_handle = runtime
        .spawn_session(shell_request(long.clone(), "cat"))
        .expect("spawn deliberately long session id");
    assert_eq!(canonical_handle.session_id, canonical);
    assert_eq!(long_handle.session_id, long);
    assert_eq!(
        std::fs::metadata(&control_dir)
            .expect("worker-created control root")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let canonical_metadata = runtime
        .metadata(&canonical)
        .expect("canonical metadata")
        .clone();
    let long_metadata = runtime.metadata(&long).expect("long metadata").clone();
    assert_eq!(canonical_metadata.session_uuid, canonical.0);
    assert_eq!(long_metadata.session_uuid, long.0);
    let canonical_socket = worker_control_socket(&canonical_metadata);
    let long_socket = worker_control_socket(&long_metadata);
    assert_eq!(canonical_socket.parent(), Some(control_dir.as_path()));
    assert_eq!(long_socket.parent(), Some(control_dir.as_path()));
    assert_ne!(canonical_socket, long_socket);
    assert_eq!(
        canonical_socket
            .file_name()
            .expect("canonical socket basename")
            .len(),
        long_socket.file_name().expect("long socket basename").len()
    );
    assert!(
        canonical_socket.as_os_str().len() <= 103,
        "macOS-shaped endpoint must fit: {canonical_socket:?}"
    );
    assert!(
        long_socket.as_os_str().len() <= 103,
        "long-id endpoint must fit: {long_socket:?}"
    );

    for (session, marker) in [(&canonical, "canonical-marker"), (&long, "long-marker")] {
        runtime
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session.clone(),
                data: format!("{marker}\n").into_bytes(),
            })
            .expect("send marker input");
        let output = collect_until(&mut runtime, &wakes, session, |output| {
            output_text(output).contains(marker)
        });
        assert!(
            output_text(&output).contains(marker),
            "session {session:?} should round-trip its own marker"
        );
    }

    let canonical_worker_pid = worker_pid(&canonical_metadata);
    let long_worker_pid = worker_pid(&long_metadata);
    let canonical_pty_pid = canonical_metadata.pid;
    let long_pty_pid = long_metadata.pid;
    for session in [&long, &canonical] {
        runtime
            .send_input(SessionRuntimeInput::Shutdown {
                session_id: session.clone(),
            })
            .expect("request worker shutdown");
        let output = collect_until(&mut runtime, &wakes, session, has_process_exit);
        assert!(has_process_exit(&output));
    }
    assert!(wait_pid_exit(canonical_worker_pid, Duration::from_secs(5)));
    assert!(wait_pid_exit(long_worker_pid, Duration::from_secs(5)));
    assert!(wait_pid_exit(canonical_pty_pid, Duration::from_secs(5)));
    assert!(wait_pid_exit(long_pty_pid, Duration::from_secs(5)));
    assert!(!canonical_socket.exists());
    assert!(!long_socket.exists());
    assert!(
        control_dir.exists(),
        "caller-supplied control root must remain caller-owned"
    );
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn worker_socket_failures_are_typed_before_spawn_or_after_worker_exit() {
    let overlong_root = std::path::PathBuf::from(format!("/tmp/{}", "x".repeat(120)));
    let mut overlong_options = worker_options();
    overlong_options.worker_path = "/definitely/missing/botster-session-worker".into();
    overlong_options.control_socket_dir = Some(overlong_root.clone());
    let mut overlong_runtime = WorkerProcessRuntime::with_options(overlong_options);
    let error = overlong_runtime
        .spawn_session(shell_request(
            session_id("123e4567-e89b-12d3-a456-426614174000"),
            "cat",
        ))
        .expect_err("overlong endpoint must fail before worker spawn");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(error.message.starts_with("worker control socket path is "));
    assert!(!error
        .message
        .starts_with("connect worker control socket failed: "));
    assert!(!overlong_root.exists());

    let missing_socket_root = temp_control_dir("bwcf");
    let mut connect_options = worker_options();
    connect_options.worker_path = "/usr/bin/false".into();
    connect_options.control_socket_dir = Some(missing_socket_root.clone());
    let mut connect_runtime = WorkerProcessRuntime::with_options(connect_options);
    let error = connect_runtime
        .spawn_session(shell_request(session_id("connect-failure"), "cat"))
        .expect_err("worker that exits before bind must retain connect error contract");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(error
        .message
        .starts_with("connect worker control socket failed: "));
    assert!(error
        .message
        .contains("worker process exited before startup completed"));
    assert!(!missing_socket_root.exists());
}

#[test]
fn occupied_worker_endpoint_fails_without_contacting_or_replacing_the_live_worker() {
    let control_dir = temp_control_dir("bwoe");
    let session = session_id("occupied-worker-endpoint");
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
    let owner_wakes = TerminalWakeSource::new();
    let mut owner =
        WorkerProcessRuntime::with_options(options.clone()).with_wake_source(owner_wakes.clone());
    owner
        .spawn_session(shell_request(session.clone(), "cat"))
        .expect("spawn endpoint owner");
    let owner_metadata = owner.metadata(&session).expect("owner metadata").clone();
    let owner_worker_pid = worker_pid(&owner_metadata);
    let owner_pty_pid = owner_metadata.pid;
    let owner_socket = worker_control_socket(&owner_metadata);

    let started = Instant::now();
    let mut contender = WorkerProcessRuntime::with_options(options);
    let error = contender
        .spawn_session(shell_request(session.clone(), "cat"))
        .expect_err("occupied endpoint must fail rather than contact its owner");

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(error
        .message
        .starts_with("connect worker control socket failed: "));
    assert!(error.message.contains("already active"), "{error}");
    let failed_worker_pid = failed_worker_pid(&error.message);
    assert!(!process_exists(failed_worker_pid));
    assert!(process_exists(owner_worker_pid));
    assert!(process_exists(owner_pty_pid));
    assert!(owner_socket.exists());

    owner
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"owner-still-connected\n".to_vec(),
        })
        .expect("live owner remains writable");
    let output = collect_until(&mut owner, &owner_wakes, &session, |output| {
        output_text(output).contains("owner-still-connected")
    });
    assert!(output_text(&output).contains("owner-still-connected"));
    owner
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown endpoint owner");
    assert!(has_process_exit(&collect_until(
        &mut owner,
        &owner_wakes,
        &session,
        has_process_exit
    )));
    assert!(wait_pid_exit(owner_worker_pid, Duration::from_secs(5)));
    assert!(wait_pid_exit(owner_pty_pid, Duration::from_secs(5)));
    assert!(!owner_socket.exists());
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn public_worker_root_failure_is_typed_visible_and_reaped() {
    let control_dir = temp_control_dir("bwpr");
    std::fs::create_dir_all(&control_dir).expect("create public control root");
    std::fs::set_permissions(&control_dir, std::fs::Permissions::from_mode(0o777))
        .expect("make control root public");
    let session = session_id("public-worker-root");
    let socket_path = derived_worker_socket(&control_dir, &session);
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);

    let error = runtime
        .spawn_session(shell_request(session, "cat"))
        .expect_err("public worker root must fail visibly");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(error
        .message
        .starts_with("connect worker control socket failed: "));
    assert!(
        error
            .message
            .contains("owned by the effective user with private permissions"),
        "{error}"
    );
    let worker_pid = failed_worker_pid(&error.message);
    assert!(!process_exists(worker_pid));
    assert!(!socket_path.exists());
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn handshake_failure_reaps_the_spawned_worker_and_its_socket() {
    let control_dir = temp_control_dir("bwhf");
    std::fs::create_dir_all(&control_dir).expect("create control root");
    std::fs::set_permissions(&control_dir, std::fs::Permissions::from_mode(0o700))
        .expect("make control root private");
    let session = session_id("handshake-failure");
    let socket_path = derived_worker_socket(&control_dir, &session);
    let worker_script = control_dir.join("sleeping-worker");
    let worker_pid_path = control_dir.join("sleeping-worker.pid");
    std::fs::write(
        &worker_script,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nprintf 'botster-session-worker-ready %s\\n' \"$$\"\nexec sleep 30\n",
            worker_pid_path.display()
        ),
    )
    .expect("write sleeping worker");
    std::fs::set_permissions(&worker_script, std::fs::Permissions::from_mode(0o700))
        .expect("make sleeping worker executable");

    let server_pid_path = worker_pid_path.clone();
    let server_socket = socket_path.clone();
    let server = thread::spawn(move || {
        assert!(wait_until(|| server_pid_path.exists()));
        let listener = UnixListener::bind(&server_socket).expect("bind fake worker endpoint");
        let (mut stream, _) = listener.accept().expect("accept startup connection");
        let mut hello = [0_u8; 16];
        let _ = stream.read(&mut hello);
    });

    let mut options = worker_options();
    options.worker_path = worker_script.clone();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let error = runtime
        .spawn_session(shell_request(session, "cat"))
        .expect_err("peer that closes before welcome must fail startup");
    server.join().expect("fake worker server");
    let worker_pid = std::fs::read_to_string(&worker_pid_path)
        .expect("read sleeping worker pid")
        .parse::<u32>()
        .expect("parse sleeping worker pid");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(wait_pid_exit(worker_pid, Duration::from_secs(5)));
    assert!(!socket_path.exists());
    assert!(control_dir.exists(), "caller-owned root must remain");
    let _ = std::fs::remove_file(worker_pid_path);
    let _ = std::fs::remove_file(worker_script);
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn welcome_must_identify_the_exact_spawned_worker() {
    let control_dir = temp_control_dir("bwpid");
    std::fs::create_dir_all(&control_dir).expect("create control root");
    std::fs::set_permissions(&control_dir, std::fs::Permissions::from_mode(0o700))
        .expect("make control root private");
    let session = session_id("wrong-worker-pid");
    let socket_path = derived_worker_socket(&control_dir, &session);
    let worker_script = control_dir.join("signaling-worker");
    let worker_pid_path = control_dir.join("signaling-worker.pid");
    std::fs::write(
        &worker_script,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nprintf 'botster-session-worker-ready %s\\n' \"$$\"\nexec sleep 30\n",
            worker_pid_path.display()
        ),
    )
    .expect("write signaling worker");
    std::fs::set_permissions(&worker_script, std::fs::Permissions::from_mode(0o700))
        .expect("make signaling worker executable");

    let server_pid_path = worker_pid_path.clone();
    let server_socket = socket_path.clone();
    let server = thread::spawn(move || {
        assert!(wait_until(|| server_pid_path.exists()));
        let listener = UnixListener::bind(&server_socket).expect("bind fake worker endpoint");
        let (mut stream, _) = listener.accept().expect("accept startup connection");
        botster_core::read_hello(&mut stream).expect("read hello");
        let mut frame_len = [0_u8; 4];
        stream
            .read_exact(&mut frame_len)
            .expect("read spawn frame length");
        let frame_len = u32::from_le_bytes(frame_len) as usize;
        assert!(frame_len > 0 && frame_len <= botster_core::MAX_FRAME_LEN);
        let mut frame = vec![0_u8; frame_len];
        stream
            .read_exact(&mut frame)
            .expect("read complete spawn frame");
        assert_eq!(frame[0], botster_core::FRAME_SPAWN_SESSION);
        let request: SessionSpawnRequest =
            serde_json::from_slice(&frame[1..]).expect("decode spawn request");
        assert_eq!(request.session_id.0, "wrong-worker-pid");
        let metadata = SessionMetadata {
            session_uuid: "wrong-worker-pid".to_string(),
            pid: 1,
            rows: 24,
            cols: 80,
            last_output_at: 0,
            title: None,
            cwd: None,
            port: None,
            mode_flags: Default::default(),
            recovery_identity: Some(serde_json::json!({"worker_pid": 1})),
        };
        botster_core::write_welcome(&mut stream, &metadata).expect("write foreign welcome");
    });

    let mut options = worker_options();
    options.worker_path = worker_script.clone();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let error = runtime
        .spawn_session(shell_request(session, "cat"))
        .expect_err("foreign welcome identity must fail startup");
    server.join().expect("fake worker server");
    let worker_pid = std::fs::read_to_string(&worker_pid_path)
        .expect("read signaling worker pid")
        .parse::<u32>()
        .expect("parse signaling worker pid");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert_eq!(
        error.message,
        "worker welcome did not identify the spawned child"
    );
    assert!(wait_pid_exit(worker_pid, Duration::from_secs(5)));
    assert!(!socket_path.exists());
    let _ = std::fs::remove_file(worker_pid_path);
    let _ = std::fs::remove_file(worker_script);
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn killed_worker_stale_socket_is_reclaimed_for_same_session_id() {
    let control_dir = temp_control_dir("bwkr");
    let session = session_id("same-session-after-worker-kill");
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
    let (stale_socket, first_worker_pid, first_pty_pid) = {
        let mut runtime = WorkerProcessRuntime::with_options(options.clone());
        runtime
            .spawn_session(shell_request(session.clone(), "cat"))
            .expect("spawn first worker");
        let metadata = runtime.metadata(&session).expect("first metadata").clone();
        let socket = worker_control_socket(&metadata);
        let worker_pid = worker_pid(&metadata);
        let status = Command::new("kill")
            .arg("-KILL")
            .arg(worker_pid.to_string())
            .status()
            .expect("kill first worker");
        assert!(status.success());
        assert!(wait_until(|| !runtime.is_worker_process(&session)));
        assert!(!process_exists(worker_pid));
        assert!(socket.exists(), "SIGKILL should leave a stale socket entry");
        runtime.release_for_restart();
        (socket, worker_pid, metadata.pid)
    };
    assert!(!process_exists(first_worker_pid));
    assert!(wait_pid_exit(first_pty_pid, Duration::from_secs(5)));
    assert!(stale_socket.exists());

    let replacement_wakes = TerminalWakeSource::new();

    let mut replacement =
        WorkerProcessRuntime::with_options(options).with_wake_source(replacement_wakes.clone());
    replacement
        .spawn_session(shell_request(session.clone(), "cat"))
        .expect("same session id should reclaim refused stale socket");
    let replacement_metadata = replacement
        .metadata(&session)
        .expect("replacement metadata")
        .clone();
    assert_ne!(worker_pid(&replacement_metadata), first_worker_pid);
    assert_eq!(worker_control_socket(&replacement_metadata), stale_socket);
    replacement
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"replacement-marker\n".to_vec(),
        })
        .expect("send replacement marker");
    let output = collect_until(&mut replacement, &replacement_wakes, &session, |output| {
        output_text(output).contains("replacement-marker")
    });
    assert!(output_text(&output).contains("replacement-marker"));
    replacement
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown replacement worker");
    assert!(has_process_exit(&collect_until(
        &mut replacement,
        &replacement_wakes,
        &session,
        has_process_exit
    )));
    assert!(!stale_socket.exists());
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn loaded_bounded_egress_publishes_exit_only_after_worker_and_control_teardown() {
    let control_dir = temp_control_dir("bwc");
    create_private_control_dir(&control_dir);

    let mut options = worker_options();
    options.egress_capacity = 1;
    options.control_socket_dir = Some(control_dir.clone());
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-loaded-completion");
    runtime
        .spawn_session(shell_request(
            session.clone(),
            "printf 'terminal-before-exit\\n'",
        ))
        .expect("spawn bounded worker session");

    let metadata = runtime.metadata(&session).expect("worker metadata").clone();
    let worker_pid = metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32;
    let socket_path = metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("worker socket in recovery identity");
    let pty_child_pid = metadata.pid;

    thread::sleep(Duration::from_millis(250));
    let output = collect_until(&mut runtime, &wakes, &session, has_process_exit);
    let terminal_index = output
        .iter()
        .position(|event| matches!(event, SessionRuntimeOutput::PtyOutput { .. }))
        .expect("full bounded queue should retain terminal output");
    let exit_index = output
        .iter()
        .position(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
        .expect("terminal completion must not be dropped with a full bounded queue");

    assert!(
        terminal_index < exit_index,
        "exit must follow retained output"
    );
    assert!(output_text(&output).contains("terminal-before-exit"));
    assert!(
        wait_pid_exit(worker_pid, Duration::from_secs(5)),
        "worker must be reaped after ProcessExited delivery"
    );
    assert!(
        !process_exists(pty_child_pid),
        "PTY child must be terminal before exit"
    );
    assert!(
        !socket_path.exists(),
        "control socket must be removed before exit"
    );
    let error = runtime
        .drain_output(&session)
        .expect_err("completed session should be removed from runtime ownership");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);

    let _ = std::fs::remove_dir_all(control_dir);
}

#[test]
fn drain_output_delivers_process_exited_while_worker_holds_stdout_open() {
    let hold_ms = 8_000;
    let control_dir = temp_control_dir("w1h");
    create_private_control_dir(&control_dir);
    let mut options = worker_options();
    options.test_hold_before_exit_ms = Some(hold_ms);
    options.control_socket_dir = Some(control_dir.clone());
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-w1-hold-before-exit");
    runtime
        .spawn_session(shell_request(
            session.clone(),
            "printf 'w1-process-exited-hold\\n'",
        ))
        .expect("spawn worker for W1 hold");
    let worker_pid = worker_pid(runtime.metadata(&session).expect("worker metadata"));

    let started = Instant::now();
    let output = collect_until(&mut runtime, &wakes, &session, has_process_exit);
    let elapsed = started.elapsed();

    assert!(
        has_process_exit(&output),
        "received ProcessExited payload is session-exit truth: {output:?}"
    );
    assert!(
        output_text(&output).contains("w1-process-exited-hold"),
        "re-pump must keep final PTY bytes ahead of ProcessExited: {}",
        output_text(&output)
    );
    assert!(
        elapsed < Duration::from_millis(hold_ms / 2),
        "delivery must not wait for the worker hold ({elapsed:?} vs {hold_ms}ms)"
    );
    assert!(
        process_exists(worker_pid),
        "W1 hold keeps the worker child alive after ProcessExited"
    );
    let error = runtime
        .drain_output(&session)
        .expect_err("delivered session must be removed from the runtime map");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);
    assert!(
        wait_pid_exit(worker_pid, Duration::from_secs(5)),
        "bounded reaper must eventually reap worker {worker_pid}"
    );
    let _ = std::fs::remove_dir_all(control_dir);
}

#[test]
fn drain_output_delivers_process_exited_when_worker_exits_nonzero() {
    let mut options = worker_options();
    options.test_exit_code = Some(1);
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let session = session_id("worker-w2-nonzero-exit");
    runtime
        .spawn_session(shell_request(
            session.clone(),
            "printf 'w2-process-exited-nonzero\\n'",
        ))
        .expect("spawn worker for W2 nonzero exit");

    let output = collect_until(&mut runtime, &wakes, &session, has_process_exit);
    let payload = output.iter().find_map(|event| match event {
        SessionRuntimeOutput::ProcessExited { payload, .. } => Some(payload.clone()),
        _ => None,
    });
    assert!(
        has_process_exit(&output),
        "nonzero worker exit must not suppress ProcessExited: {output:?}"
    );
    assert_eq!(
        payload.and_then(|payload| payload.exit_code),
        Some(0),
        "delivered payload is the session process exit, not the worker status"
    );
    assert!(
        output_text(&output).contains("w2-process-exited-nonzero"),
        "final session output must survive W2 delivery: {}",
        output_text(&output)
    );
    let error = runtime
        .drain_output(&session)
        .expect_err("delivered session must be removed from the runtime map");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);
}

#[test]
fn reaper_window_leaves_a_sibling_session_live() {
    let hold_ms = 8_000;
    let control_dir = temp_control_dir("sib");
    create_private_control_dir(&control_dir);
    let mut options = worker_options();
    options.test_hold_before_exit_ms = Some(hold_ms);
    options.control_socket_dir = Some(control_dir.clone());
    let wakes = TerminalWakeSource::new();
    let mut runtime = WorkerProcessRuntime::with_options(options).with_wake_source(wakes.clone());
    let exiting = session_id("worker-reaper-sibling-exit");
    let sibling = session_id("worker-reaper-sibling-live");
    runtime
        .spawn_session(shell_request(
            exiting.clone(),
            "printf 'sibling-exit-marker\\n'",
        ))
        .expect("spawn exiting sibling");
    runtime
        .spawn_session(shell_request(sibling.clone(), "cat"))
        .expect("spawn live sibling");
    let exiting_pid = worker_pid(runtime.metadata(&exiting).expect("exiting metadata"));
    let sibling_pid = worker_pid(runtime.metadata(&sibling).expect("sibling metadata"));

    let output = collect_until(&mut runtime, &wakes, &exiting, has_process_exit);
    assert!(has_process_exit(&output));
    assert!(
        process_exists(exiting_pid),
        "reaper window must still own the exiting worker"
    );
    assert!(
        process_exists(sibling_pid),
        "sibling worker must stay alive during the reaper window"
    );

    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: sibling.clone(),
            data: b"sibling-still-live\n".to_vec(),
        })
        .expect("sibling must still accept input");
    let sibling_output = collect_until(&mut runtime, &wakes, &sibling, |output| {
        output_text(output).contains("sibling-still-live")
    });
    assert!(
        output_text(&sibling_output).contains("sibling-still-live"),
        "sibling session must keep working while the exiting child is reaped: {}",
        output_text(&sibling_output)
    );

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: sibling.clone(),
        })
        .expect("shutdown live sibling");
    assert!(has_process_exit(&collect_until(
        &mut runtime,
        &wakes,
        &sibling,
        has_process_exit
    )));
    assert!(wait_pid_exit(exiting_pid, Duration::from_secs(5)));
    assert!(wait_pid_exit(sibling_pid, Duration::from_secs(5)));
    let _ = std::fs::remove_dir_all(control_dir);
}

#[test]
fn unexpected_control_eof_without_clean_exit_reports_the_worker_lost() {
    let control_dir = temp_control_dir("bwe");
    create_private_control_dir(&control_dir);
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
    let (probe, probe_events) = WorkerRouteProbe::channel();
    options.test_route_probe = Some(probe);
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let session = session_id("worker-unexpected-eof");
    runtime
        .spawn_session(shell_request(session.clone(), "cat"))
        .expect("spawn worker for unexpected EOF");
    let metadata = runtime.metadata(&session).expect("worker metadata").clone();
    let worker_pid = metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32;
    let socket_path = metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("worker socket in recovery identity");

    let status = Command::new("kill")
        .arg("-KILL")
        .arg(worker_pid.to_string())
        .status()
        .expect("kill worker process");
    assert!(status.success());
    // The parent reader records its end before it reports ReaderEnded, so
    // the next drain sees the finished reader with no exit report.
    loop {
        // timer: deadline — the killed worker's reader must end within the bound
        let event = probe_events
            .recv_timeout(Duration::from_secs(5))
            .expect("the killed worker's reader ends");
        if matches!(&event, WorkerRouteProbeEvent::ReaderEnded { session_id } if *session_id == session)
        {
            break;
        }
    }
    // The worker ended without an exit report: the session ends as
    // WorkerLost, never as a completed exit, and is removed.
    let output = runtime
        .drain_output(&session)
        .expect("drain reports the lost worker");
    assert!(!has_process_exit(&output));
    assert!(output
        .iter()
        .any(|event| matches!(event, SessionRuntimeOutput::WorkerLost { session_id } if *session_id == session)));
    assert!(runtime.metadata(&session).is_none());

    drop(runtime);
    assert!(wait_pid_exit(metadata.pid, Duration::from_secs(5)));
    assert!(!socket_path.exists());
    let _ = std::fs::remove_dir_all(control_dir);
}

fn temp_control_dir(prefix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/tmp").join(format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow unix epoch")
            .as_nanos()
    ))
}

fn create_private_control_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).expect("create worker control directory");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("make worker control directory private");
}

fn worker_pid(metadata: &botster_core::SessionMetadata) -> u32 {
    metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32
}

fn worker_control_socket(metadata: &botster_core::SessionMetadata) -> std::path::PathBuf {
    metadata
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("worker socket in recovery identity")
}

fn derived_worker_socket(root: &std::path::Path, session_id: &SessionId) -> std::path::PathBuf {
    let digest = Sha256::digest(session_id.0.as_bytes());
    root.join(format!("{}.sock", URL_SAFE_NO_PAD.encode(&digest[..16])))
}

fn failed_worker_pid(message: &str) -> u32 {
    message
        .split_once("botster-session-worker ")
        .and_then(|(_, suffix)| suffix.split_whitespace().next())
        .and_then(|value| value.parse().ok())
        .expect("failed worker pid in captured diagnostic")
}

#[test]
fn worker_process_argv_does_not_expose_spawn_environment_or_working_directory() {
    let secret = "botster-secret-env-value";
    let cwd = "botster-sensitive-working-directory";
    let request = SessionSpawnRequest {
        environment: SpawnEnvironment {
            variables: vec![botster_core::SpawnEnvironmentVariable {
                name: "BOTSTER_SECRET_TEST".to_string(),
                value: secret.to_string(),
            }],
        },
        working_directory: SpawnWorkingDirectory {
            path: cwd.to_string(),
        },
        ..shell_request(session_id("argv-safety"), "cat")
    };

    let mut child = Command::new(worker_path())
        .arg("--egress-capacity")
        .arg("2")
        .arg("--pty-reader-capacity")
        .arg("2")
        .arg("--shutdown-grace-ms")
        .arg("80")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn worker directly");
    let argv = process_argv(child.id());
    assert!(!argv.contains(secret));
    assert!(!argv.contains(cwd));

    let mut stdin = child.stdin.take().expect("worker stdin");
    botster_core::write_hello(&mut stdin).expect("write hello");
    let spawn = botster_core::encode_json(botster_core::FRAME_SPAWN_SESSION, &request)
        .expect("encode spawn frame");
    use std::io::Write;
    stdin.write_all(&spawn).expect("write spawn frame");
    stdin.flush().expect("flush spawn frame");
    drop(stdin);
    let _ = child.wait();
}

fn write_signaling_worker(
    dir: &std::path::Path,
    name: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let script = dir.join(name);
    let pid_path = dir.join(format!("{name}.pid"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nprintf 'botster-session-worker-ready %s\\n' \"$$\"\nexec sleep 30\n",
            pid_path.display()
        ),
    )
    .expect("write signaling worker");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
        .expect("make signaling worker executable");
    (script, pid_path)
}

fn reserved_spawn(
    runtime: &mut WorkerProcessRuntime,
    request: SessionSpawnRequest,
) -> (
    botster_core::SessionReservation,
    Result<botster_core::SessionRuntimeHandle, ReservedSessionSpawnError>,
) {
    let admission = runtime
        .session_admission()
        .expect("worker runtime admission")
        .clone();
    let reservation = admission
        .reserve(request.session_id.clone())
        .expect("prelaunch reservation");
    let result = runtime.spawn_reserved(&reservation, request);
    (reservation, result)
}

#[test]
fn real_worker_missing_executable_is_not_created_and_releases() {
    let mut runtime = WorkerProcessRuntime::with_options(worker_options());
    let session = session_id("spf1-missing-child");
    let mut request = shell_request(session, "exit 0");
    request.executable = "/no/such/botster-session-child".to_string();
    request.arguments.clear();
    let (reservation, result) = reserved_spawn(&mut runtime, request);
    result.expect_err("missing child executable");
    assert!(!reservation.startup_created_child());
    assert_eq!(reservation.state(), SessionReservationState::Ended);
    assert_eq!(
        runtime
            .session_admission()
            .expect("admission")
            .release(&reservation)
            .expect("release not created"),
        SessionReservationRelease::Released
    );
}

#[test]
fn spf1_wrong_worker_pid_is_unknown() {
    let control_dir = temp_control_dir("spf1-pid");
    create_private_control_dir(&control_dir);
    let session = session_id("spf1-wrong-pid");
    let socket_path = derived_worker_socket(&control_dir, &session);
    let (worker_script, worker_pid_path) = write_signaling_worker(&control_dir, "spf1-worker");
    let server_pid_path = worker_pid_path.clone();
    let server_socket = socket_path.clone();
    let server = thread::spawn(move || {
        assert!(wait_until(|| server_pid_path.exists()));
        let listener = UnixListener::bind(&server_socket).expect("bind fake worker endpoint");
        let (mut stream, _) = listener.accept().expect("accept startup connection");
        botster_core::read_hello(&mut stream).expect("read hello");
        let mut frame_len = [0_u8; 4];
        stream
            .read_exact(&mut frame_len)
            .expect("read spawn frame length");
        let frame_len = u32::from_le_bytes(frame_len) as usize;
        let mut frame = vec![0_u8; frame_len];
        stream
            .read_exact(&mut frame)
            .expect("read complete spawn frame");
        assert_eq!(frame[0], botster_core::FRAME_SPAWN_SESSION);
        let request: SessionSpawnRequest =
            serde_json::from_slice(&frame[1..]).expect("decode spawn request");
        write_startup_failure(
            &mut stream,
            &StartupFailureReport {
                request_id: request.request_id.0,
                session_id: request.session_id.0,
                worker_pid: 1,
                message: "wrong pid".to_string(),
                outcome: StartupFailureOutcome::NotCreated,
            },
        )
        .expect("write mismatched spf1");
    });

    let mut options = worker_options();
    options.worker_path = worker_script.clone();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let (reservation, result) = reserved_spawn(&mut runtime, shell_request(session, "cat"));
    result.expect_err("mismatched worker pid");
    server.join().expect("fake worker server");
    assert_eq!(
        reservation.state(),
        SessionReservationState::CleanupUnconfirmed
    );
    assert_eq!(
        runtime
            .session_admission()
            .expect("admission")
            .release(&reservation)
            .expect("retain unknown"),
        SessionReservationRelease::RetainedUnconfirmed
    );
    let _ = std::fs::remove_file(worker_pid_path);
    let _ = std::fs::remove_file(worker_script);
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn old_worker_exits_before_connect_is_not_created() {
    let control_dir = temp_control_dir("spf1-old-before");
    create_private_control_dir(&control_dir);
    let session = session_id("spf1-old-before-connect");
    let worker_script = control_dir.join("old-worker");
    std::fs::write(
        &worker_script,
        "#!/bin/sh\nprintf 'botster-session-worker-ready %s\\n' \"$$\"\necho old-worker-stderr >&2\nexit 1\n",
    )
    .expect("write old worker");
    std::fs::set_permissions(&worker_script, std::fs::Permissions::from_mode(0o700))
        .expect("make old worker executable");

    let mut options = worker_options();
    options.worker_path = worker_script.clone();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let (reservation, result) = reserved_spawn(&mut runtime, shell_request(session, "cat"));
    result.expect_err("old worker before connect");
    assert!(!reservation.startup_created_child());
    assert_eq!(reservation.state(), SessionReservationState::Ended);
    let _ = std::fs::remove_file(worker_script);
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn old_worker_reads_spawn_frame_then_exits_without_spf1_is_unknown() {
    let control_dir = temp_control_dir("spf1-old-after");
    create_private_control_dir(&control_dir);
    let session = session_id("spf1-old-after-spawn");
    let socket_path = derived_worker_socket(&control_dir, &session);
    let (worker_script, worker_pid_path) = write_signaling_worker(&control_dir, "old-after");
    let server_pid_path = worker_pid_path.clone();
    let server_socket = socket_path.clone();
    let server = thread::spawn(move || {
        assert!(wait_until(|| server_pid_path.exists()));
        let listener = UnixListener::bind(&server_socket).expect("bind fake worker endpoint");
        let (mut stream, _) = listener.accept().expect("accept startup connection");
        botster_core::read_hello(&mut stream).expect("read hello");
        let mut frame_len = [0_u8; 4];
        stream
            .read_exact(&mut frame_len)
            .expect("read spawn frame length");
        let frame_len = u32::from_le_bytes(frame_len) as usize;
        let mut frame = vec![0_u8; frame_len];
        stream
            .read_exact(&mut frame)
            .expect("read complete spawn frame");
        assert_eq!(frame[0], botster_core::FRAME_SPAWN_SESSION);
        let _ = std::io::Write::write_all(&mut std::io::stderr(), b"old-worker-stderr\n");
    });

    let mut options = worker_options();
    options.worker_path = worker_script.clone();
    options.control_socket_dir = Some(control_dir.clone());
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let (reservation, result) = reserved_spawn(&mut runtime, shell_request(session, "cat"));
    result.expect_err("old worker after spawn frame");
    server.join().expect("fake worker server");
    assert_eq!(
        reservation.state(),
        SessionReservationState::CleanupUnconfirmed
    );
    let _ = std::fs::remove_file(worker_pid_path);
    let _ = std::fs::remove_file(worker_script);
    let _ = std::fs::remove_dir(control_dir);
}

#[test]
fn crash_before_hello_is_not_created_crash_after_spawn_frame_is_unknown() {
    let before_dir = temp_control_dir("spf1-before");
    create_private_control_dir(&before_dir);
    let before_session = session_id("spf1-before-hello");
    let before_script = before_dir.join("crash-before");
    std::fs::write(
        &before_script,
        "#!/bin/sh\nprintf 'botster-session-worker-ready %s\\n' \"$$\"\nexit 1\n",
    )
    .expect("write crash-before worker");
    std::fs::set_permissions(&before_script, std::fs::Permissions::from_mode(0o700))
        .expect("make crash-before executable");
    let mut before_options = worker_options();
    before_options.worker_path = before_script.clone();
    before_options.control_socket_dir = Some(before_dir.clone());
    let mut before_runtime = WorkerProcessRuntime::with_options(before_options);
    let (before_reservation, before_result) =
        reserved_spawn(&mut before_runtime, shell_request(before_session, "cat"));
    before_result.expect_err("crash before hello");
    assert!(!before_reservation.startup_created_child());
    assert_eq!(before_reservation.state(), SessionReservationState::Ended);

    let after_dir = temp_control_dir("spf1-after");
    create_private_control_dir(&after_dir);
    let after_session = session_id("spf1-after-spawn");
    let socket_path = derived_worker_socket(&after_dir, &after_session);
    let (after_script, after_pid_path) = write_signaling_worker(&after_dir, "crash-after");
    let server_pid_path = after_pid_path.clone();
    let server_socket = socket_path.clone();
    let server = thread::spawn(move || {
        assert!(wait_until(|| server_pid_path.exists()));
        let listener = UnixListener::bind(&server_socket).expect("bind fake worker endpoint");
        let (mut stream, _) = listener.accept().expect("accept startup connection");
        botster_core::read_hello(&mut stream).expect("read hello");
        let mut frame_len = [0_u8; 4];
        stream
            .read_exact(&mut frame_len)
            .expect("read spawn frame length");
        let frame_len = u32::from_le_bytes(frame_len) as usize;
        let mut frame = vec![0_u8; frame_len];
        stream
            .read_exact(&mut frame)
            .expect("read complete spawn frame");
        assert_eq!(frame[0], botster_core::FRAME_SPAWN_SESSION);
    });
    let mut after_options = worker_options();
    after_options.worker_path = after_script.clone();
    after_options.control_socket_dir = Some(after_dir.clone());
    let mut after_runtime = WorkerProcessRuntime::with_options(after_options);
    let (after_reservation, after_result) =
        reserved_spawn(&mut after_runtime, shell_request(after_session, "cat"));
    after_result.expect_err("crash after spawn frame");
    server.join().expect("fake worker server");
    assert_eq!(
        after_reservation.state(),
        SessionReservationState::CleanupUnconfirmed
    );
    let _ = std::fs::remove_file(before_script);
    let _ = std::fs::remove_dir(before_dir);
    let _ = std::fs::remove_file(after_pid_path);
    let _ = std::fs::remove_file(after_script);
    let _ = std::fs::remove_dir(after_dir);
}

/// After its child exits, a socket worker keeps serving its parent (captures
/// from the final terminal model). When its last control connection closes,
/// the parent is gone, and the worker exits.
#[cfg(unix)]
#[test]
fn an_exited_socket_worker_exits_when_its_parent_connection_closes() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let dir = std::env::temp_dir().join(format!(
        "bw-exit-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir(&dir).expect("create the socket directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .expect("make the socket directory private");
    let socket = dir.join("w.sock");
    let mut worker = Command::new(worker_path())
        .arg("--control-socket")
        .arg(&socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn a socket worker");
    let mut ready = String::new();
    BufReader::new(worker.stdout.take().expect("worker stdout"))
        .read_line(&mut ready)
        .expect("read worker readiness");
    assert!(
        ready.starts_with("botster-session-worker-ready"),
        "{ready:?}"
    );

    let mut stream = UnixStream::connect(&socket).expect("connect to the worker");
    botster_core::write_hello(&mut stream).expect("write hello");
    let spawn = botster_core::encode_json(
        botster_core::FRAME_SPAWN_SESSION,
        &shell_request(session_id("socket-exit"), "exit 0"),
    )
    .expect("encode spawn");
    stream.write_all(&spawn).expect("write spawn");
    let _ = botster_core::read_welcome(&mut stream).expect("read welcome");
    // timer: deadline — the worker must report the exit; expiry fails the test
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("bound the reads");
    let mut decoder = botster_core::FrameDecoder::new();
    let mut buffer = [0_u8; 4096];
    'exited: loop {
        let read = stream.read(&mut buffer).expect("read worker frames");
        assert!(read > 0, "the worker closed before reporting the exit");
        for frame in decoder.feed(&buffer[..read]).expect("decode worker frames") {
            if frame.frame_type == botster_core::FRAME_PROCESS_EXITED {
                break 'exited;
            }
        }
    }

    // The child has exited; the worker is serving. The parent goes away.
    drop(stream);
    let exited = wait_pid_exit(worker.id(), Duration::from_secs(30));
    if !exited {
        let _ = worker.kill();
    }
    let _ = worker.wait();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        exited,
        "an exited worker must exit when its parent's connection closes"
    );
}

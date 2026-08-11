//! Local session worker process acceptance tests.
#![cfg(all(unix, feature = "local-runtime"))]

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use botster_core::{
    BackpressureSummary, CoreSessionMetadata, DefaultBotsterEngine, NotificationPayload,
    PromptMarkPayload, QueueSource, RequestId, ResizePayload, SessionId, SessionMetadata,
    SessionRuntime, SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TerminalMetadataShapingObservation, TerminalMetadataShapingOutcome, TransportEgress,
    WorkerBackedBotsterEngine, WorkerProcessRuntime, WorkerProcessRuntimeOptions,
};
use sha2::{Digest, Sha256};

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn worker_path() -> std::path::PathBuf {
    use std::process::Command;
    use std::sync::Once;
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
            "worker binary should build for core worker tests"
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
        poll_interval_ms: 5,
        control_socket_dir: None,
        mode_gated_input_timeout: botster_core::DEFAULT_MODE_GATED_INPUT_TIMEOUT,
        test_mode_gated_hold_ms: None,
        test_hold_after_read_ms: None,
        test_write_block_until_unix_ms: None,
        test_write_max_chunk: None,
        test_pending_capacity: None,
        test_hold_after_flush_ms: None,
    }
}

fn collect_until<F>(
    runtime: &mut dyn SessionRuntime,
    session_id: &SessionId,
    mut predicate: F,
) -> Vec<SessionRuntimeOutput>
where
    F: FnMut(&[SessionRuntimeOutput]) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();

    while Instant::now() < deadline {
        match runtime.drain_output(session_id) {
            Ok(drained) => output.extend(drained),
            Err(error) if error.kind == SessionRuntimeErrorKind::SessionNotFound => {}
            Err(error) => panic!("drain worker runtime output: {error}"),
        }
        if predicate(&output) {
            return output;
        }
        thread::sleep(Duration::from_millis(20));
    }

    output
}

fn output_text(output: &[SessionRuntimeOutput]) -> String {
    let bytes: Vec<u8> = output
        .iter()
        .filter_map(|event| match event {
            SessionRuntimeOutput::PtyOutput { data, .. } => Some(data.as_slice()),
            SessionRuntimeOutput::ProcessExited { .. }
            | SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::Backpressure(_)
            | SessionRuntimeOutput::MetadataShaping(_) => None,
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
            | SessionRuntimeOutput::TitleChanged { .. }
            | SessionRuntimeOutput::CwdChanged { .. }
            | SessionRuntimeOutput::PromptMark { .. }
            | SessionRuntimeOutput::Bell { .. }
            | SessionRuntimeOutput::Notification { .. }
            | SessionRuntimeOutput::Backpressure(_)
            | SessionRuntimeOutput::MetadataShaping(_) => None,
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
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();

    while Instant::now() < deadline {
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
        thread::sleep(Duration::from_millis(20));
    }

    bytes
}

#[test]
fn worker_process_runtime_crosses_os_process_boundary_and_handles_protocol_commands() {
    let mut runtime = WorkerProcessRuntime::with_options(worker_options());
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

    let echoed = collect_until(&mut runtime, &session, |output| {
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
    let finished = collect_until(&mut runtime, &session, has_process_exit);
    assert!(
        has_process_exit(&finished),
        "worker should emit process exit over protocol"
    );
}

#[test]
fn worker_process_runtime_emits_semantic_metadata_from_session_worker_output() {
    let mut options = worker_options();
    options.pty_reader_chunk_capacity = 8;
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let session = session_id("worker-semantic-metadata");
    let script =
        "printf '\\033]2;Build\\007\\033]7;file://host/work/repo\\007\\033]133;A\\007\\007\\033]9;Notice;Body\\007'; sleep 0.1";

    runtime
        .spawn_session(shell_request(session.clone(), script))
        .expect("spawn metadata worker session");

    let output = collect_until(&mut runtime, &session, |output| {
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
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let session = session_id("worker-metadata-shaping");
    let titles = (0..40)
        .map(|index| format!("\\033]2;title-{index}\\007"))
        .collect::<String>();
    let script = format!("printf 'protected-ready\\n{titles}'; cat");

    runtime
        .spawn_session(shell_request(session.clone(), &script))
        .expect("spawn metadata flood worker session");

    let output = collect_until(&mut runtime, &session, |output| {
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
    let after_flood = collect_until(&mut runtime, &session, |output| {
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
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let session = session_id("worker-metadata-drop");
    let prompts = (0..20)
        .map(|index| format!("\\033]133;P{index}\\007"))
        .collect::<String>();
    let script = format!("printf 'drop-ready\\n{prompts}'; cat");

    runtime
        .spawn_session(shell_request(session.clone(), &script))
        .expect("spawn metadata drop worker session");

    let output = collect_until(&mut runtime, &session, |output| {
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
    let mut runtime = WorkerProcessRuntime::with_options(options);
    let session = session_id("worker-metadata-order");
    let script = "printf '\\033]133;A\\007'; sleep 0.1; printf 'after-prompt\\n'; cat";

    runtime
        .spawn_session(shell_request(session.clone(), script))
        .expect("spawn metadata ordering worker session");

    let output = collect_until(&mut runtime, &session, |output| {
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

    let ready = drain_engine_until(&mut engine, &session, |bytes| {
        String::from_utf8_lossy(bytes).contains("public-ready")
    });
    assert!(String::from_utf8_lossy(&ready).contains("public-ready"));

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
    let mut runtime = WorkerProcessRuntime::with_options(options);
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

    let reattached = collect_until(&mut runtime, &session, |output| {
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
        wait_until(|| !process_exists(worker_pid)),
        "dropping parent runtime should reap worker process {worker_pid}"
    );
    assert!(
        wait_until(|| !process_exists(pty_child_pid)),
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
    let mut runtime = WorkerProcessRuntime::with_options(options);

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
        let output = collect_until(&mut runtime, session, |output| {
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
        let output = collect_until(&mut runtime, session, has_process_exit);
        assert!(has_process_exit(&output));
    }
    assert!(wait_until(|| !process_exists(canonical_worker_pid)));
    assert!(wait_until(|| !process_exists(long_worker_pid)));
    assert!(wait_until(|| !process_exists(canonical_pty_pid)));
    assert!(wait_until(|| !process_exists(long_pty_pid)));
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
    let mut owner = WorkerProcessRuntime::with_options(options.clone());
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
    let output = collect_until(&mut owner, &session, |output| {
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
        &session,
        has_process_exit
    )));
    assert!(wait_until(|| !process_exists(owner_worker_pid)));
    assert!(wait_until(|| !process_exists(owner_pty_pid)));
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
    assert!(wait_until(|| !process_exists(worker_pid)));
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
    assert!(wait_until(|| !process_exists(worker_pid)));
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
    assert!(wait_until(|| !process_exists(first_pty_pid)));
    assert!(stale_socket.exists());

    let mut replacement = WorkerProcessRuntime::with_options(options);
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
    let output = collect_until(&mut replacement, &session, |output| {
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
    let mut runtime = WorkerProcessRuntime::with_options(options);
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
    let output = collect_until(&mut runtime, &session, has_process_exit);
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
        !process_exists(worker_pid),
        "worker must be reaped before exit"
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
fn unexpected_control_eof_without_clean_exit_does_not_publish_completion() {
    let control_dir = temp_control_dir("bwe");
    create_private_control_dir(&control_dir);
    let mut options = worker_options();
    options.control_socket_dir = Some(control_dir.clone());
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
    assert!(wait_until(|| !runtime.is_worker_process(&session)));
    thread::sleep(Duration::from_millis(50));
    let output = runtime
        .drain_output(&session)
        .expect("unexpected EOF remains fail-closed runtime state");
    assert!(!has_process_exit(&output));
    assert!(runtime.metadata(&session).is_some());

    drop(runtime);
    assert!(wait_until(|| !process_exists(metadata.pid)));
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
        .arg("--poll-interval-ms")
        .arg("5")
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

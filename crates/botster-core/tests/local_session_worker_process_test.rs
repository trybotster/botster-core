//! Local session worker process acceptance tests.
#![cfg(all(unix, feature = "local-runtime"))]

use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use botster_core::{
    BackpressureSummary, CoreSessionMetadata, DefaultBotsterEngine, NotificationPayload,
    PromptMarkPayload, QueueSource, RequestId, ResizePayload, SessionId, SessionRuntime,
    SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TransportEgress,
    WorkerBackedBotsterEngine, WorkerProcessRuntime, WorkerProcessRuntimeOptions,
};

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn worker_path() -> &'static str {
    env!("CARGO_BIN_EXE_botster-session-worker")
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
        worker_path: worker_path().into(),
        egress_capacity: 64,
        pty_reader_chunk_capacity: 8,
        shutdown_grace_ms: 80,
        poll_interval_ms: 5,
        control_socket_dir: None,
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
            | SessionRuntimeOutput::Backpressure(_) => None,
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
            | SessionRuntimeOutput::Backpressure(_) => None,
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
            "i=0; while [ $i -lt 200 ]; do printf \"tick:$i\\n\"; i=$((i+1)); done; cat",
        ))
        .expect("spawn noisy worker session");

    runtime
        .detach_consumer(&session)
        .expect("detach parent-side consumer without protocol frame");
    thread::sleep(Duration::from_millis(250));
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

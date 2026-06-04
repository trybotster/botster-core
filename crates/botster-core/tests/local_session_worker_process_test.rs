//! Local session worker process acceptance tests.
#![cfg(all(unix, feature = "local-runtime"))]

use std::thread;
use std::time::{Duration, Instant};

use botster_core::{
    BackpressureSummary, CoreSessionMetadata, DefaultBotsterEngine, QueueSource, RequestId,
    ResizePayload, SessionId, SessionRuntime, SessionRuntimeErrorKind, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TransportEgress, WorkerBackedBotsterEngine, WorkerProcessRuntime,
    WorkerProcessRuntimeOptions,
};

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
            SessionRuntimeOutput::ProcessExited { .. } | SessionRuntimeOutput::Backpressure(_) => {
                None
            }
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

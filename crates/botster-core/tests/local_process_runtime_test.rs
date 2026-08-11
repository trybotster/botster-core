//! Local process runtime acceptance and shutdown behavior tests.
#![cfg(all(unix, feature = "local-runtime"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::{
    ClientId, CoreSessionMetadata, DefaultBotsterEngine, LocalProcessRuntime,
    LocalProcessRuntimeOptions, MultiplexerEngine, ProcessExitedPayload, QueueSource, RequestId,
    ResizePayload, SessionId, SessionLifecycleState, SessionRuntime, SessionRuntimeErrorKind,
    SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest, SpawnEnvironment,
    SpawnEnvironmentVariable, SpawnWorkingDirectory, SubscriptionId, TransportEgress,
    DEFAULT_PTY_READER_CHUNK_CAPACITY,
};

const SIGKILL: i32 = 9;
static LOCAL_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn runtime_options() -> LocalProcessRuntimeOptions {
    LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(50),
        poll_interval: Duration::from_millis(5),
        pty_reader_chunk_capacity: DEFAULT_PTY_READER_CHUNK_CAPACITY,
        test_hold_after_read_ms: None,
        test_write_block_until_unix_ms: None,
        test_write_max_chunk: None,
    }
}

fn slow_shutdown_runtime_options() -> LocalProcessRuntimeOptions {
    LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(700),
        poll_interval: Duration::from_millis(20),
        pty_reader_chunk_capacity: DEFAULT_PTY_READER_CHUNK_CAPACITY,
        test_hold_after_read_ms: None,
        test_write_block_until_unix_ms: None,
        test_write_max_chunk: None,
    }
}

fn term_ignoring_process_group_script() -> &'static str {
    "trap '' TERM; sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!"
}

fn session_id(value: &str) -> SessionId {
    SessionId(value.to_string())
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn client_id(value: &str) -> ClientId {
    ClientId(value.to_string())
}

fn subscription_id(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}

fn local_process_test_lock() -> MutexGuard<'static, ()> {
    LOCAL_PROCESS_TEST_LOCK
        .lock()
        .expect("local process test lock should not be poisoned")
}

fn shell_request(session_id: SessionId, script: &str) -> SessionSpawnRequest {
    shell_request_with_env(session_id, script, SpawnEnvironment::default())
}

fn shell_request_with_env(
    session_id: SessionId,
    script: &str,
    environment: SpawnEnvironment,
) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("local-runtime-request"),
        session_id,
        executable: "sh".to_string(),
        arguments: vec!["-c".to_string(), script.to_string()],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment,
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

fn env_var(name: &str, value: impl Into<String>) -> SpawnEnvironmentVariable {
    SpawnEnvironmentVariable {
        name: name.to_string(),
        value: value.into(),
    }
}

fn unique_temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-{name}-{nanos}"))
}

fn process_exists(pid: u32) -> bool {
    // SAFETY: signal 0 performs the POSIX existence check without delivering a
    // signal to the target process.
    unsafe { kill(pid as i32, 0) == 0 }
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

fn wait_for_child_pid(path: &Path) -> u32 {
    assert!(
        wait_until(|| path.exists()),
        "child pid file should be written"
    );
    fs::read_to_string(path)
        .expect("read child pid")
        .trim()
        .parse()
        .expect("child pid is numeric")
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
            Err(error) => panic!("drain local process runtime output: {error}"),
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

fn has_exit(output: &[SessionRuntimeOutput]) -> bool {
    output
        .iter()
        .any(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
}

fn exit_signal(output: &[SessionRuntimeOutput], expected_session: &SessionId) -> Option<i32> {
    output.iter().find_map(|event| match event {
        SessionRuntimeOutput::ProcessExited {
            session_id,
            payload,
        } if session_id == expected_session => payload.signal,
        _ => None,
    })
}

fn source_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.as_ref().to_path_buf()];

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("read source directory") {
            let entry = entry.expect("read source entry");
            let path = entry.path();

            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "rs" || extension == "md")
            {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

#[test]
fn local_process_runtime_spawns_simple_command_and_drains_output() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::new();
    let session = session_id("local-runtime-output");

    runtime
        .spawn_session(shell_request(
            session.clone(),
            "printf 'botster-local-output\\n'",
        ))
        .expect("spawn local command");

    let output = collect_until(&mut runtime, &session, |output| {
        output_text(output).contains("botster-local-output") && has_exit(output)
    });

    assert!(output_text(&output).contains("botster-local-output"));
    assert!(has_exit(&output), "expected process exit, got {output:?}");
}

#[test]
fn local_process_runtime_reports_bounded_reader_backpressure_out_of_band() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(LocalProcessRuntimeOptions {
        pty_reader_chunk_capacity: 1,
        ..runtime_options()
    });
    let session = session_id("local-runtime-reader-pressure");

    runtime
        .spawn_session(shell_request(
            session.clone(),
            "i=0; while [ \"$i\" -lt 128 ]; do printf 'reader-pressure:%03d:%08000d\\n' \"$i\" 0; i=$((i + 1)); done",
        ))
        .expect("spawn noisy local command");

    thread::sleep(Duration::from_millis(100));
    let output = runtime
        .drain_output(&session)
        .expect("drain local process pressure");

    let summary = output
        .iter()
        .find_map(|event| match event {
            SessionRuntimeOutput::Backpressure(summary) => Some(summary),
            _ => None,
        })
        .expect("bounded reader pressure should be reported");
    assert_eq!(summary.source, QueueSource::SessionIo);
    assert_eq!(summary.capacity, 1);
    assert_eq!(summary.depth, 1);
    assert_eq!(summary.route.session_id, Some(session.clone()));
    assert_eq!(summary.route.client_id, None);
    assert!(output.iter().any(|event| {
        matches!(
            event,
            SessionRuntimeOutput::PtyOutput {
                session_id,
                data,
            } if session_id == &session && !data.is_empty()
        )
    }));

    let _ = runtime.send_input(SessionRuntimeInput::Shutdown {
        session_id: session,
    });
}

#[test]
fn local_process_runtime_spawns_and_captures_process_exit_status() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let session = session_id("local-exit");
    runtime
        .spawn_session(shell_request(session.clone(), "exit 7"))
        .expect("spawn short-lived local process");

    let output = collect_until(&mut runtime, &session, has_exit);

    assert!(output.iter().any(|event| {
        event
            == &SessionRuntimeOutput::ProcessExited {
                session_id: session.clone(),
                payload: ProcessExitedPayload {
                    exit_code: Some(7),
                    signal: None,
                },
            }
    }));
}

#[test]
fn local_process_runtime_drains_final_output_before_exit_and_removal() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let session = session_id("local-final-reader-egress");
    let ready_file = unique_temp_path("final-reader-ready");
    let child_pid_file = unique_temp_path("final-reader-child-pid");
    let environment = SpawnEnvironment {
        variables: vec![
            env_var("READY_FILE", ready_file.display().to_string()),
            env_var("CHILD_PID_FILE", child_pid_file.display().to_string()),
        ],
    };
    let script = "sh -c 'trap \"\" TERM; printf ready > \"$READY_FILE\"; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; while [ ! -s \"$READY_FILE\" ]; do :; done; printf 'final-reader-marker\\n'; exit 7";

    runtime
        .spawn_session(shell_request_with_env(session.clone(), script, environment))
        .expect("spawn leader with PTY-holding descendant");
    let descendant_pid = wait_for_child_pid(&child_pid_file);
    assert!(ready_file.exists(), "descendant should confirm readiness");

    let output = collect_until(&mut runtime, &session, has_exit);
    let marker_index = output
        .iter()
        .position(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PtyOutput { data, .. }
                    if String::from_utf8_lossy(data).contains("final-reader-marker")
            )
        })
        .expect("final PTY marker should be published");
    let exit_index = output
        .iter()
        .position(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
        .expect("process exit should be published");

    assert!(
        marker_index < exit_index,
        "final PTY marker must precede exit"
    );
    assert!(output.iter().any(|event| {
        event
            == &SessionRuntimeOutput::ProcessExited {
                session_id: session.clone(),
                payload: ProcessExitedPayload {
                    exit_code: Some(7),
                    signal: None,
                },
            }
    }));
    assert!(wait_until(|| !process_exists(descendant_pid)));
    let error = runtime
        .drain_output(&session)
        .expect_err("session should be removed only after final egress");
    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);

    let _ = fs::remove_file(ready_file);
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn default_engine_subscription_publishes_final_terminal_output_before_process_exit() {
    let _guard = local_process_test_lock();
    let mut engine = DefaultBotsterEngine::new();
    let session = session_id("default-engine-final-egress");
    let client = client_id("default-engine-final-client");
    let subscription = subscription_id("default-engine-final-subscription");

    engine
        .spawn_session(
            shell_request(
                session.clone(),
                "printf 'default-engine-final-marker\\n'; exit 9",
            ),
            CoreSessionMetadata::new(),
        )
        .expect("spawn default-engine local session");
    engine
        .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
        .expect("attach default-engine subscription");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut egress = Vec::new();
    while Instant::now() < deadline {
        let outcome = engine
            .drain_runtime_once(&session, 2)
            .expect("drain default-engine runtime");
        egress.extend(
            outcome
                .client_egress
                .into_iter()
                .filter_map(|(received_client, event)| {
                    (received_client == client).then_some(event)
                }),
        );
        if egress
            .iter()
            .any(|event| matches!(event, TransportEgress::ProcessExit { .. }))
        {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    let marker_index = egress
        .iter()
        .position(|event| {
            matches!(
                event,
                TransportEgress::TerminalOutput {
                    session_id,
                    subscription_id,
                    data,
                } if session_id == &session
                    && subscription_id == &subscription
                    && String::from_utf8_lossy(data).contains("default-engine-final-marker")
            )
        })
        .expect("subscription should receive the final terminal marker");
    let exit_index = egress
        .iter()
        .position(|event| {
            matches!(
                event,
                TransportEgress::ProcessExit {
                    session_id,
                    subscription_id,
                    code: Some(9),
                } if session_id == &session && subscription_id == &subscription
            )
        })
        .expect("subscription should receive the leader exit");
    assert!(
        marker_index < exit_index,
        "terminal egress must precede exit"
    );
}

#[test]
fn local_process_runtime_writes_input_to_pty() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::new();
    let session = session_id("local-runtime-input");

    runtime
        .spawn_session(shell_request(session.clone(), "cat"))
        .expect("spawn echoing local command");
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"botster-input-marker\n".to_vec(),
        })
        .expect("write local pty input");

    let output = collect_until(&mut runtime, &session, |output| {
        output_text(output).contains("botster-input-marker")
    });
    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown echoing local command");

    assert!(output_text(&output).contains("botster-input-marker"));
}

#[test]
fn local_process_runtime_resizes_pty_when_supported() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::new();
    let session = session_id("local-runtime-resize");

    runtime
        .spawn_session(shell_request(session.clone(), "sleep 0.2; stty size"))
        .expect("spawn resizable local command");

    runtime
        .send_input(SessionRuntimeInput::Resize {
            session_id: session.clone(),
            size: ResizePayload {
                rows: 33,
                cols: 120,
            },
        })
        .expect("resize local pty");

    let output = collect_until(&mut runtime, &session, |output| {
        output_text(output).contains("33 120") && has_exit(output)
    });

    assert!(
        output_text(&output).contains("33 120"),
        "expected resized terminal dimensions, got {output:?}"
    );
}

#[test]
fn local_process_runtime_graceful_shutdown_records_exit() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let session = session_id("local-graceful");
    runtime
        .spawn_session(shell_request(
            session.clone(),
            "trap 'exit 0' TERM; while true; do sleep 1; done",
        ))
        .expect("spawn graceful local process");

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown graceful process");

    let output = collect_until(&mut runtime, &session, has_exit);
    assert!(output.iter().any(|event| {
        matches!(
            event,
            SessionRuntimeOutput::ProcessExited {
                session_id,
                payload: ProcessExitedPayload { .. },
            } if session_id == &session
        )
    }));
}

#[test]
fn local_process_runtime_graceful_leader_exit_still_kills_ignoring_child_group() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let child_pid_file = unique_temp_path("graceful-child-pid");
    let session = session_id("local-graceful-child");
    runtime
        .spawn_session(shell_request_with_env(
            session.clone(),
            "trap 'exit 0' TERM; sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
            SpawnEnvironment {
                variables: vec![env_var(
                    "CHILD_PID_FILE",
                    child_pid_file.display().to_string(),
                )],
            },
        ))
        .expect("spawn graceful leader with TERM-ignoring child");
    let child_pid = wait_for_child_pid(&child_pid_file);

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown graceful leader process group");

    let output = collect_until(&mut runtime, &session, has_exit);
    assert!(output.iter().any(|event| {
        event
            == &SessionRuntimeOutput::ProcessExited {
                session_id: session.clone(),
                payload: ProcessExitedPayload {
                    exit_code: Some(0),
                    signal: None,
                },
            }
    }));
    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_forced_shutdown_kills_ignoring_child_group() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let child_pid_file = unique_temp_path("child-pid");
    let session = session_id("local-forced");
    let handle = runtime
        .spawn_session(shell_request_with_env(
            session.clone(),
            "trap '' TERM; sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
            SpawnEnvironment {
                variables: vec![env_var(
                    "CHILD_PID_FILE",
                    child_pid_file.display().to_string(),
                )],
            },
        ))
        .expect("spawn process group that ignores graceful termination");
    let parent_pid = handle.process.pid.expect("local process exposes pid");
    let child_pid = wait_for_child_pid(&child_pid_file);

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("force shutdown process group");

    let output = collect_until(&mut runtime, &session, has_exit);
    assert!(output.iter().any(|event| {
        matches!(
            event,
            SessionRuntimeOutput::ProcessExited {
                payload: ProcessExitedPayload {
                    signal: Some(SIGKILL),
                    ..
                },
                ..
            }
        )
    }));
    assert!(wait_until(|| !process_exists(parent_pid)));
    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_shutdown_is_idempotent() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());
    let session = session_id("local-idempotent");
    runtime
        .spawn_session(shell_request(
            session.clone(),
            "trap 'exit 0' TERM; while true; do sleep 1; done",
        ))
        .expect("spawn process for idempotent shutdown");

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("first shutdown succeeds");
    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("second shutdown is a no-op");

    let output = collect_until(&mut runtime, &session, has_exit);
    assert_eq!(
        output
            .iter()
            .filter(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
            .count(),
        1
    );
}

#[test]
fn local_process_runtime_shutdown_does_not_block_unrelated_session_io() {
    let _guard = local_process_test_lock();
    let options = slow_shutdown_runtime_options();
    let mut runtime = LocalProcessRuntime::with_options(options);
    let child_pid_file = unique_temp_path("nonblocking-child-pid");
    let stubborn = session_id("local-nonblocking-stubborn");
    let peer = session_id("local-nonblocking-peer");

    runtime
        .spawn_session(shell_request_with_env(
            stubborn.clone(),
            term_ignoring_process_group_script(),
            SpawnEnvironment {
                variables: vec![env_var(
                    "CHILD_PID_FILE",
                    child_pid_file.display().to_string(),
                )],
            },
        ))
        .expect("spawn stubborn process");
    let _child_pid = wait_for_child_pid(&child_pid_file);
    runtime
        .spawn_session(shell_request(peer.clone(), "cat"))
        .expect("spawn peer process");

    let mut shutdown_runtime = runtime.clone();
    let shutdown_session = stubborn.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let shutdown = thread::spawn(move || {
        started_tx.send(()).expect("notify shutdown thread started");
        let started = Instant::now();
        shutdown_runtime
            .send_input(SessionRuntimeInput::Shutdown {
                session_id: shutdown_session,
            })
            .expect("shutdown stubborn process");
        started.elapsed()
    });

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("shutdown thread should start");
    thread::sleep(Duration::from_millis(100));

    let started = Instant::now();
    runtime
        .send_input(SessionRuntimeInput::Resize {
            session_id: peer.clone(),
            size: ResizePayload { rows: 31, cols: 90 },
        })
        .expect("resize peer while stubborn shutdown waits");
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: peer.clone(),
            data: b"peer-still-live\n".to_vec(),
        })
        .expect("write peer while stubborn shutdown waits");
    let output = collect_until(&mut runtime, &peer, |output| {
        output_text(output).contains("peer-still-live")
    });
    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: peer.clone(),
        })
        .expect("shutdown peer while stubborn shutdown waits");
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_millis(250),
        "peer operations were blocked for {elapsed:?}"
    );
    assert!(output_text(&output).contains("peer-still-live"));

    let shutdown_elapsed = shutdown.join().expect("join stubborn shutdown thread");
    assert!(
        shutdown_elapsed >= options.shutdown_grace,
        "stubborn shutdown completed too quickly: {shutdown_elapsed:?}"
    );
    let stubborn_output = collect_until(&mut runtime, &stubborn, has_exit);
    assert_eq!(
        exit_signal(&stubborn_output, &stubborn),
        Some(SIGKILL),
        "stubborn process should require forced cleanup"
    );
    assert_eq!(
        stubborn_output
            .iter()
            .filter(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
            .count(),
        1,
        "stubborn shutdown should queue exactly one process exit"
    );
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_drop_cleans_live_child_group() {
    let _guard = local_process_test_lock();
    let child_pid_file = unique_temp_path("drop-child-pid");
    let child_pid = {
        let mut runtime = LocalProcessRuntime::with_options(runtime_options());
        runtime
            .spawn_session(shell_request_with_env(
                session_id("local-drop"),
                "sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
                SpawnEnvironment {
                    variables: vec![env_var(
                        "CHILD_PID_FILE",
                        child_pid_file.display().to_string(),
                    )],
                },
            ))
            .expect("spawn process for drop cleanup");
        wait_for_child_pid(&child_pid_file)
    };

    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_reports_spawn_failure() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::new();
    let mut request = shell_request(session_id("local-runtime-spawn-failure"), "exit 0");
    request.executable = "definitely-missing-botster-core-runtime-test-command".to_string();

    let error = runtime
        .spawn_session(request)
        .expect_err("missing executable should fail");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert!(
        error
            .message
            .contains("definitely-missing-botster-core-runtime-test-command"),
        "spawn failure should include requested executable"
    );
}

#[test]
fn local_process_runtime_reports_session_not_found() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::new();
    let session = session_id("local-runtime-missing");

    let input_error = runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session.clone(),
            data: b"ignored".to_vec(),
        })
        .expect_err("missing session input should fail");
    let output_error = runtime
        .drain_output(&session)
        .expect_err("missing session output should fail");

    assert_eq!(input_error.kind, SessionRuntimeErrorKind::SessionNotFound);
    assert_eq!(output_error.kind, SessionRuntimeErrorKind::SessionNotFound);
}

#[test]
fn local_process_runtime_unknown_session_shutdown_returns_typed_error() {
    let _guard = local_process_test_lock();
    let mut runtime = LocalProcessRuntime::with_options(runtime_options());

    let error = runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session_id("missing-local"),
        })
        .expect_err("unknown shutdown should fail");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);
}

#[test]
fn local_process_runtime_can_be_used_through_public_session_runtime_trait() {
    let _guard = local_process_test_lock();
    let mut runtime: Box<dyn SessionRuntime> = Box::new(LocalProcessRuntime::new());
    let session = session_id("local-runtime-trait-object");
    let mut request = shell_request(
        session.clone(),
        "printf 'trait-runtime env-%s\\n' \"$BOTSTER_CORE_LOCAL_RUNTIME_TEST\"",
    );
    request
        .environment
        .variables
        .push(SpawnEnvironmentVariable {
            name: "BOTSTER_CORE_LOCAL_RUNTIME_TEST".to_string(),
            value: "1".to_string(),
        });

    let handle = runtime
        .spawn_session(request)
        .expect("spawn through public trait object");
    assert_eq!(handle.session_id, session);

    let output = collect_until(runtime.as_mut(), &handle.session_id, |output| {
        output_text(output).contains("trait-runtime env-1") && has_exit(output)
    });

    assert!(output_text(&output).contains("trait-runtime env-1"));
    assert!(has_exit(&output), "expected process exit, got {output:?}");
}

#[test]
fn botster_engine_shutdown_uses_runtime_cleanup_path() {
    let _guard = local_process_test_lock();
    let runtime = LocalProcessRuntime::with_options(runtime_options());
    let worker_runtime = runtime.worker_runtime();
    let mut engine = MultiplexerEngine::new(runtime);
    let session = session_id("engine-local");

    let spawn = engine
        .spawn_session(
            shell_request(
                session.clone(),
                "trap 'printf \"shutdown-final-marker\\n\"; exit 0' TERM; printf 'shutdown-ready\\n'; while true; do sleep 1; done",
            ),
            CoreSessionMetadata::new(),
            worker_runtime,
        )
        .expect("spawn local process through engine");
    let pid = spawn.handle.process.pid.expect("local process exposes pid");
    let ready = collect_until(engine.session_runtime_mut(), &session, |output| {
        output_text(output).contains("shutdown-ready")
    });
    assert!(output_text(&ready).contains("shutdown-ready"));

    let shutdown = engine
        .shutdown_session(session.clone(), "engine shutdown", 10)
        .expect("shutdown through public engine path");

    assert!(
        !shutdown
            .session_events
            .iter()
            .any(|event| matches!(event, botster_core::SessionIoEvent::ProcessExited { .. })),
        "engine shutdown must withhold ProcessExited until reader completion"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut routed_exit = false;
    let mut final_output = Vec::new();
    while Instant::now() < deadline && !routed_exit {
        let output = engine
            .session_runtime_mut()
            .drain_output(&session)
            .expect("drain final local runtime output");
        final_output.extend(output.iter().cloned());
        for event in output {
            if let SessionRuntimeOutput::ProcessExited {
                session_id,
                payload,
            } = event
            {
                let outcome = engine
                    .handle_runtime_event(botster_core::SessionWorkerRuntimeEvent::ProcessExited {
                        session_id,
                        payload,
                    })
                    .expect("route delayed process exit");
                routed_exit = outcome.session_events.iter().any(|event| {
                    matches!(event, botster_core::SessionIoEvent::ProcessExited { .. })
                });
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        routed_exit,
        "reader completion should release ProcessExited"
    );
    let marker_index = final_output
        .iter()
        .position(|event| {
            matches!(
                event,
                SessionRuntimeOutput::PtyOutput { data, .. }
                    if String::from_utf8_lossy(data).contains("shutdown-final-marker")
            )
        })
        .expect("shutdown should retain the final PTY marker");
    let exit_index = final_output
        .iter()
        .position(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
        .expect("shutdown should eventually publish ProcessExited");
    assert!(marker_index < exit_index);
    assert!(matches!(
        engine.session(&session).map(|session| &session.lifecycle),
        Some(SessionLifecycleState::Exited { .. })
    ));
    assert!(wait_until(|| !process_exists(pid)));
}

#[test]
fn botster_engine_shutdown_does_not_hold_registry_lock_for_unrelated_session() {
    let _guard = local_process_test_lock();
    let options = slow_shutdown_runtime_options();
    let mut runtime = LocalProcessRuntime::with_options(options);
    let mut engine = MultiplexerEngine::new(runtime.clone());
    let child_pid_file = unique_temp_path("engine-nonblocking-child-pid");
    let stubborn = session_id("engine-nonblocking-stubborn");
    let peer = session_id("engine-nonblocking-peer");

    engine
        .spawn_session(
            shell_request_with_env(
                stubborn.clone(),
                term_ignoring_process_group_script(),
                SpawnEnvironment {
                    variables: vec![env_var(
                        "CHILD_PID_FILE",
                        child_pid_file.display().to_string(),
                    )],
                },
            ),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect("spawn stubborn process through engine");
    let _child_pid = wait_for_child_pid(&child_pid_file);
    engine
        .spawn_session(
            shell_request(peer.clone(), "cat"),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect("spawn peer process through engine");

    let shutdown_session = stubborn.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let shutdown = thread::spawn(move || {
        started_tx
            .send(())
            .expect("notify engine shutdown thread started");
        let started = Instant::now();
        let outcome = engine
            .shutdown_session(shutdown_session, "engine slow shutdown", 10)
            .expect("shutdown stubborn session through engine");
        (started.elapsed(), outcome, engine)
    });

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("engine shutdown thread should start");
    thread::sleep(Duration::from_millis(100));

    let started = Instant::now();
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: peer.clone(),
            data: b"engine-peer-still-live\n".to_vec(),
        })
        .expect("write peer while engine shutdown waits");
    let output = collect_until(&mut runtime, &peer, |output| {
        output_text(output).contains("engine-peer-still-live")
    });
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_millis(250),
        "peer runtime access was blocked by engine shutdown for {elapsed:?}"
    );
    assert!(output_text(&output).contains("engine-peer-still-live"));

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: peer.clone(),
        })
        .expect("shutdown peer after engine contention proof");
    let (shutdown_elapsed, shutdown_outcome, mut engine) =
        shutdown.join().expect("join engine shutdown thread");
    assert!(
        shutdown_elapsed >= options.shutdown_grace,
        "engine stubborn shutdown completed too quickly: {shutdown_elapsed:?}"
    );
    assert!(
        !shutdown_outcome
            .session_events
            .iter()
            .any(|event| matches!(event, botster_core::SessionIoEvent::ProcessExited { .. })),
        "engine shutdown must withhold exit until reader completion"
    );
    let output = collect_until(engine.session_runtime_mut(), &stubborn, has_exit);
    assert_eq!(exit_signal(&output, &stubborn), Some(SIGKILL));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_runtime_docs_and_tests_do_not_embed_private_paths_or_pii() {
    let _guard = local_process_test_lock();
    let banned_terms = [
        ["/", "Users", "/"].concat(),
        ["jason", "conigliari"].concat(),
        ["Project", "Pipelines"].concat(),
    ];
    let mut files = source_files("src/runtime");
    files.push(PathBuf::from("tests/local_process_runtime_test.rs"));
    files.push(PathBuf::from("../../README.md"));
    files.push(PathBuf::from(
        "../../docs/archive/plans/default-local-pty-process-runtime.md",
    ));
    files.push(PathBuf::from(
        "../../docs/archive/plans/process-group-cleanup-shutdown-guarantees.md",
    ));

    for source_file in files {
        let source = fs::read_to_string(&source_file).expect("read source file");

        for term in &banned_terms {
            assert!(
                !source.contains(term.as_str()),
                "local runtime file {} must not contain banned term {term}",
                source_file.display()
            );
        }
    }
}

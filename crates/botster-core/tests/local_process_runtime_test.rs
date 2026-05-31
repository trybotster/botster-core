#![cfg(unix)]

//! Local process runtime shutdown behavior tests.

use std::fs;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::{
    CoreSessionMetadata, LocalProcessRuntimeOptions, LocalProcessSessionRuntime, MultiplexerEngine,
    ProcessExitedPayload, RequestId, SessionId, SessionLifecycleState, SessionRuntime,
    SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory,
};

const SIGKILL: i32 = 9;

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn runtime_options() -> LocalProcessRuntimeOptions {
    LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(50),
        poll_interval: Duration::from_millis(5),
    }
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id(value: &str) -> SessionId {
    SessionId(value.to_string())
}

fn spawn_request(
    request: &str,
    session: &str,
    script: impl Into<String>,
    environment: Vec<SpawnEnvironmentVariable>,
) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id(request),
        session_id: session_id(session),
        executable: "sh".to_string(),
        arguments: vec!["-c".to_string(), script.into()],
        working_directory: SpawnWorkingDirectory {
            path: std::env::current_dir()
                .expect("current test directory")
                .display()
                .to_string(),
        },
        environment: SpawnEnvironment {
            variables: environment,
        },
        initial_pty_size: None,
    }
}

fn env_var(name: &str, value: impl Into<String>) -> SpawnEnvironmentVariable {
    SpawnEnvironmentVariable {
        name: name.to_string(),
        value: value.into(),
    }
}

fn unique_temp_path(name: &str) -> std::path::PathBuf {
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
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn wait_for_child_pid(path: &std::path::Path) -> u32 {
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

fn drain_until_exit(
    runtime: &mut LocalProcessSessionRuntime,
    session_id: &SessionId,
) -> Vec<SessionRuntimeOutput> {
    let mut output = Vec::new();
    assert!(
        wait_until(|| {
            output = runtime
                .drain_output(session_id)
                .expect("drain local process output");
            !output.is_empty()
        }),
        "process exit output should be observed"
    );
    output
}

#[test]
fn local_process_runtime_spawns_and_captures_process_exit_status() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let session = session_id("local-exit");
    runtime
        .spawn_session(spawn_request(
            "spawn-exit",
            &session.0,
            "exit 7",
            Vec::new(),
        ))
        .expect("spawn short-lived local process");

    let output = drain_until_exit(&mut runtime, &session);

    assert_eq!(
        output,
        vec![SessionRuntimeOutput::ProcessExited {
            session_id: session,
            payload: ProcessExitedPayload {
                exit_code: Some(7),
                signal: None,
            },
        }]
    );
}

#[test]
fn local_process_runtime_graceful_shutdown_records_exit() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let session = session_id("local-graceful");
    runtime
        .spawn_session(spawn_request(
            "spawn-graceful",
            &session.0,
            "trap 'exit 0' TERM; while true; do sleep 1; done",
            Vec::new(),
        ))
        .expect("spawn graceful local process");

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown graceful process");

    assert!(matches!(
        runtime
            .drain_output(&session)
            .expect("drain graceful shutdown")
            .as_slice(),
        [SessionRuntimeOutput::ProcessExited {
            session_id,
            payload: ProcessExitedPayload { .. },
        }] if session_id == &session
    ));
}

#[test]
fn local_process_runtime_graceful_leader_exit_still_kills_ignoring_child_group() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let child_pid_file = unique_temp_path("graceful-child-pid");
    let session = session_id("local-graceful-child");
    runtime
        .spawn_session(spawn_request(
            "spawn-graceful-child",
            &session.0,
            "trap 'exit 0' TERM; sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
            vec![env_var(
                "CHILD_PID_FILE",
                child_pid_file.display().to_string(),
            )],
        ))
        .expect("spawn graceful leader with TERM-ignoring child");
    let child_pid = wait_for_child_pid(&child_pid_file);

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("shutdown graceful leader process group");

    assert_eq!(
        runtime
            .drain_output(&session)
            .expect("drain graceful leader shutdown"),
        vec![SessionRuntimeOutput::ProcessExited {
            session_id: session,
            payload: ProcessExitedPayload {
                exit_code: Some(0),
                signal: None,
            },
        }]
    );
    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_forced_shutdown_kills_ignoring_child_group() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let child_pid_file = unique_temp_path("child-pid");
    let session = session_id("local-forced");
    let handle = runtime
        .spawn_session(spawn_request(
            "spawn-forced",
            &session.0,
            "trap '' TERM; sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
            vec![env_var(
                "CHILD_PID_FILE",
                child_pid_file.display().to_string(),
            )],
        ))
        .expect("spawn process group that ignores graceful termination");
    let parent_pid = handle.process.pid.expect("local process exposes pid");
    let child_pid = wait_for_child_pid(&child_pid_file);

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session.clone(),
        })
        .expect("force shutdown process group");

    let output = runtime
        .drain_output(&session)
        .expect("drain forced shutdown");
    assert!(matches!(
        output.as_slice(),
        [SessionRuntimeOutput::ProcessExited {
            payload: ProcessExitedPayload {
                signal: Some(SIGKILL),
                ..
            },
            ..
        }]
    ));
    assert!(wait_until(|| !process_exists(parent_pid)));
    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_shutdown_is_idempotent() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let session = session_id("local-idempotent");
    runtime
        .spawn_session(spawn_request(
            "spawn-idempotent",
            &session.0,
            "trap 'exit 0' TERM; while true; do sleep 1; done",
            Vec::new(),
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

    assert_eq!(
        runtime
            .drain_output(&session)
            .expect("drain idempotent shutdown")
            .len(),
        1
    );
}

#[test]
fn local_process_runtime_drop_cleans_live_child_group() {
    let child_pid_file = unique_temp_path("drop-child-pid");
    let child_pid = {
        let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());
        runtime
            .spawn_session(spawn_request(
                "spawn-drop",
                "local-drop",
                "sh -c 'trap \"\" TERM; while true; do sleep 1; done' & echo $! > \"$CHILD_PID_FILE\"; wait $!",
                vec![env_var(
                    "CHILD_PID_FILE",
                    child_pid_file.display().to_string(),
                )],
            ))
            .expect("spawn process for drop cleanup");
        wait_for_child_pid(&child_pid_file)
    };

    assert!(wait_until(|| !process_exists(child_pid)));
    let _ = fs::remove_file(child_pid_file);
}

#[test]
fn local_process_runtime_unknown_session_shutdown_returns_typed_error() {
    let mut runtime = LocalProcessSessionRuntime::with_options(runtime_options());

    let error = runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session_id("missing-local"),
        })
        .expect_err("unknown shutdown should fail");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SessionNotFound);
}

#[test]
fn botster_engine_shutdown_uses_runtime_cleanup_path() {
    let runtime = LocalProcessSessionRuntime::with_options(runtime_options());
    let worker_runtime = runtime.worker_runtime();
    let mut engine = MultiplexerEngine::new(runtime);
    let session = session_id("engine-local");

    let spawn = engine
        .spawn_session(
            spawn_request(
                "spawn-engine-local",
                &session.0,
                "trap 'exit 0' TERM; while true; do sleep 1; done",
                Vec::new(),
            ),
            CoreSessionMetadata::new(),
            worker_runtime,
        )
        .expect("spawn local process through engine");
    let pid = spawn.handle.process.pid.expect("local process exposes pid");

    let shutdown = engine
        .shutdown_session(session.clone(), "engine shutdown", 10)
        .expect("shutdown through public engine path");

    assert!(
        shutdown.session_events.iter().any(|event| {
            matches!(
                event,
                botster_core::SessionIoEvent::ProcessExited {
                    session_id,
                    payload: ProcessExitedPayload { .. },
                } if session_id == &session
            )
        }),
        "engine shutdown should route ProcessExited through session worker events"
    );
    assert!(matches!(
        engine.session(&session).map(|session| &session.lifecycle),
        Some(SessionLifecycleState::Exited { .. })
    ));
    assert!(wait_until(|| !process_exists(pid)));
}

//! Session runtime contract acceptance tests.

use std::fs;
use std::path::{Path, PathBuf};

use botster_core::{
    ProcessExitedPayload, RequestId, ResizePayload, SessionId, SessionRuntime, SessionRuntimeError,
    SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory,
};
use botster_core_test_support::fake::FakeSessionRuntime;

fn request_id() -> RequestId {
    RequestId("req-runtime-1".to_string())
}

fn session_id() -> SessionId {
    SessionId("session-runtime-1".to_string())
}

fn spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id(),
        session_id: session_id(),
        executable: "botster-session".to_string(),
        arguments: vec!["--mode".to_string(), "agent".to_string()],
        working_directory: SpawnWorkingDirectory {
            path: "/work/repo".to_string(),
        },
        environment: SpawnEnvironment {
            variables: vec![SpawnEnvironmentVariable {
                name: "BOTSTER_ENV".to_string(),
                value: "test".to_string(),
            }],
        },
        initial_pty_size: Some(ResizePayload {
            rows: 30,
            cols: 100,
        }),
    }
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize runtime contract value");
    serde_json::from_str(&json).expect("deserialize runtime contract value")
}

fn runtime_source_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.as_ref().to_path_buf()];

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("read runtime source directory") {
            let entry = entry.expect("read runtime source entry");
            let path = entry.path();

            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

#[test]
fn session_runtime_trait_can_spawn_successfully_with_explicit_contracts() {
    let mut runtime = FakeSessionRuntime::new();

    let handle = runtime
        .spawn_session(spawn_request())
        .expect("fake runtime spawn should succeed");

    assert_eq!(handle.request_id, request_id());
    assert_eq!(handle.session_id, session_id());
    assert_eq!(handle.process.pid, Some(1));
    assert_eq!(
        handle.process.runtime_id,
        Some("fake-process-1".to_string())
    );
    assert_eq!(runtime.spawned(), &[spawn_request()]);
    assert_eq!(
        runtime.spawned()[0].environment.variables,
        vec![SpawnEnvironmentVariable {
            name: "BOTSTER_ENV".to_string(),
            value: "test".to_string(),
        }]
    );
    assert_eq!(
        runtime.spawned()[0].working_directory,
        SpawnWorkingDirectory {
            path: "/work/repo".to_string(),
        }
    );
}

#[test]
fn session_runtime_spawn_failure_returns_typed_error() {
    let mut runtime = FakeSessionRuntime::new();
    runtime.fail_next_spawn(SessionRuntimeError::new(
        SessionRuntimeErrorKind::SpawnFailed,
        "binary not available",
    ));

    let error = runtime
        .spawn_session(spawn_request())
        .expect_err("fake runtime spawn should fail");

    assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
    assert_eq!(error.message, "binary not available");
    assert_eq!(error, round_trip(&error));
}

#[test]
fn session_runtime_error_kinds_pin_all_serialized_variants() {
    let variants = [
        (SessionRuntimeErrorKind::SpawnFailed, "\"SpawnFailed\""),
        (
            SessionRuntimeErrorKind::SessionNotFound,
            "\"SessionNotFound\"",
        ),
        (SessionRuntimeErrorKind::InputFailed, "\"InputFailed\""),
        (SessionRuntimeErrorKind::OutputFailed, "\"OutputFailed\""),
    ];

    for (kind, json) in variants {
        assert_eq!(serde_json::to_string(&kind).expect("serialize kind"), json);
        assert_eq!(round_trip(&kind), kind);
    }
}

#[test]
fn session_runtime_reports_exit_status_through_contract() {
    let mut runtime = FakeSessionRuntime::new();
    runtime
        .spawn_session(spawn_request())
        .expect("fake runtime spawn should succeed");

    runtime.emit_exit(
        session_id(),
        ProcessExitedPayload {
            exit_code: Some(7),
            signal: None,
        },
    );

    let output = runtime
        .drain_output(&session_id())
        .expect("fake runtime output should drain");

    assert_eq!(
        output,
        vec![SessionRuntimeOutput::ProcessExited {
            session_id: session_id(),
            payload: ProcessExitedPayload {
                exit_code: Some(7),
                signal: None,
            }
        }]
    );
}

#[test]
fn session_runtime_io_wires_input_and_output() {
    let mut runtime = FakeSessionRuntime::new();
    runtime
        .spawn_session(spawn_request())
        .expect("fake runtime spawn should succeed");

    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        })
        .expect("fake runtime input should succeed");
    runtime.emit_output(session_id(), b"ok\n".to_vec());

    assert_eq!(
        runtime.inputs(),
        &[SessionRuntimeInput::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        }]
    );
    assert_eq!(
        runtime
            .drain_output(&session_id())
            .expect("fake runtime output should drain"),
        vec![SessionRuntimeOutput::PtyOutput {
            session_id: session_id(),
            data: b"ok\n".to_vec(),
        }]
    );
}

#[test]
fn fake_runtime_reports_session_not_found_for_unknown_session_io() {
    let mut runtime = FakeSessionRuntime::new();

    let input_error = runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        })
        .expect_err("unknown session input should fail");
    let output_error = runtime
        .drain_output(&session_id())
        .expect_err("unknown session output drain should fail");

    assert_eq!(input_error.kind, SessionRuntimeErrorKind::SessionNotFound);
    assert_eq!(output_error.kind, SessionRuntimeErrorKind::SessionNotFound);
}

#[test]
fn session_runtime_pty_size_is_rows_and_columns_only() {
    let request = spawn_request();

    assert_eq!(
        request.initial_pty_size,
        Some(ResizePayload {
            rows: 30,
            cols: 100
        })
    );
}

#[test]
fn session_runtime_contract_excludes_product_and_transport_policy() {
    let banned_terms = [
        "Rails",
        "WebRtc",
        "WebRTC",
        "Tui",
        "React",
        "ProjectPipelines",
        "/Users/",
    ];

    let source_files = runtime_source_files("src/runtime");
    assert!(
        !source_files.is_empty(),
        "runtime source guard found no files"
    );

    for source_file in source_files {
        let source = fs::read_to_string(&source_file).expect("read runtime source");

        for term in banned_terms {
            assert!(
                !source.contains(term),
                "runtime contract file {} must not contain banned term {term}",
                source_file.display()
            );
        }
    }
}

#[test]
fn fake_runtime_is_usable_from_test_support() {
    let mut runtime: Box<dyn SessionRuntime> = Box::new(FakeSessionRuntime::new());
    let handle = runtime
        .spawn_session(spawn_request())
        .expect("fake runtime spawn should succeed through trait object");

    assert_eq!(handle.session_id, session_id());
}

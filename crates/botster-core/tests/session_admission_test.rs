//! Public reservation exclusion across spawn and adoption paths.
#![cfg(all(unix, feature = "local-runtime"))]

use std::sync::Mutex;
use std::time::Duration;

use botster_core::runtime::{SessionReservationRefusal, SessionReservationRelease};
use botster_core::{
    CoreSessionMetadata, LocalProcessRuntime, LocalProcessRuntimeOptions,
    LocalProcessWorkerRuntime, MultiplexerEngine, MultiplexerEngineError, SessionId,
    SessionRuntimeErrorKind, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    DEFAULT_PTY_READER_CHUNK_CAPACITY,
};

type TestEngine = MultiplexerEngine<LocalProcessRuntime, LocalProcessWorkerRuntime>;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn runtime_options() -> LocalProcessRuntimeOptions {
    LocalProcessRuntimeOptions {
        shutdown_grace: Duration::from_millis(50),
        pty_reader_chunk_capacity: DEFAULT_PTY_READER_CHUNK_CAPACITY,
        test_hold_after_read_ms: None,
        test_pending_capacity: None,
        test_hold_after_enqueue_ms: None,
    }
}

fn session_id(value: &str) -> SessionId {
    SessionId(value.to_string())
}

fn shell_request(session_id: SessionId) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: botster_core::RequestId("reservation-spawn".to_string()),
        session_id,
        executable: "sh".to_string(),
        arguments: vec!["-c".to_string(), "exit 0".to_string()],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: None,
    }
}

fn occupied(error: MultiplexerEngineError) -> bool {
    matches!(
        error,
        MultiplexerEngineError::Runtime(error)
            if error.kind == SessionRuntimeErrorKind::SpawnFailed
                && error.message == "session identity is occupied"
    )
}

#[test]
fn reserved_identity_excludes_ordinary_spawn_and_shared_runtime_peers() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    let runtime = LocalProcessRuntime::with_options(runtime_options());
    let mut engine = TestEngine::new(runtime.clone());
    let mut peer = TestEngine::new(runtime.clone());
    let session = session_id("reserved-exclusion");
    let reservation = engine
        .reserve_session(session.clone())
        .expect("prelaunch reservation");

    assert!(occupied(
        engine
            .spawn_session(
                shell_request(session.clone()),
                CoreSessionMetadata::new(),
                runtime.worker_runtime(),
            )
            .expect_err("ordinary spawn")
    ));
    assert!(occupied(
        peer.spawn_session(
            shell_request(session.clone()),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect_err("peer spawn")
    ));
    assert_eq!(
        peer.release_session_reservation(&reservation)
            .expect_err("peer release"),
        SessionReservationRefusal::InvalidToken
    );
    assert_eq!(
        engine
            .release_session_reservation(&reservation)
            .expect("owner release"),
        SessionReservationRelease::Released
    );

    engine
        .spawn_session(
            shell_request(session),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect("spawn after definitive release");
}

#[test]
fn reserved_launch_installs_only_the_matching_token() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    let runtime = LocalProcessRuntime::with_options(runtime_options());
    let mut engine = TestEngine::new(runtime.clone());
    let session = session_id("reserved-launch");
    let other = engine
        .reserve_session(session_id("other-token"))
        .expect("other reservation");
    let reservation = engine
        .reserve_session(session.clone())
        .expect("matching reservation");

    engine
        .spawn_reserved_session(
            &other,
            shell_request(session.clone()),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect_err("mismatched token");
    engine
        .spawn_reserved_session(
            &reservation,
            shell_request(session.clone()),
            CoreSessionMetadata::new(),
            runtime.worker_runtime(),
        )
        .expect("reserved launch");
    assert!(engine.session(&session).is_some());
}

#[test]
fn lookup_recovers_only_the_original_reserve_operation() {
    let runtime = LocalProcessRuntime::with_options(runtime_options());
    let engine = TestEngine::new(runtime);
    let session = session_id("lookup");
    let reserved = engine
        .reserve_session_for_request(session.clone(), 9, 4)
        .expect("reserve");
    assert_eq!(
        engine
            .session_reservation_for_request(&session, 9)
            .expect("lookup")
            .as_ref(),
        Some(&reserved)
    );
    assert!(engine
        .session_reservation_for_request(&session, 8)
        .expect("wrong operation")
        .is_none());
}

#[test]
fn public_capacity_refusal_uses_pending_spawn_limit() {
    let runtime = LocalProcessRuntime::with_options(runtime_options());
    let engine = TestEngine::new(runtime);
    engine
        .reserve_session_for_request(session_id("cap-1"), 1, 1)
        .expect("first");
    assert_eq!(
        engine
            .reserve_session_for_request(session_id("cap-2"), 2, 1)
            .expect_err("capacity"),
        SessionReservationRefusal::Capacity
    );
}

//! Dev-only smoke harnesses for `botster-core`.

use std::error::Error;
use std::fmt;

#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use botster_core::{
    BotsterEngineObservation, ClientId, CoreSessionMetadata, DefaultBotsterEngine,
    DefaultEngineCommand, EngineCommandOutcome, RequestId, ResizePayload, SessionActivityStatus,
    SessionId, SessionIoEvent, SessionLifecycleState, SessionSpawnRequest, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TransportEgress,
};

/// Deterministic report emitted by the dev-only real embedder smoke harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSmokeReport {
    /// Whether this host ran the real local PTY example.
    pub ran_real_embedder: bool,
    /// Session spawned through the public default local engine.
    pub spawned_session_id: SessionId,
    /// Client attached through the public subscription path.
    pub attached_client_id: ClientId,
    /// Explicit executable selected by the embedding host.
    pub executable: String,
    /// Explicit arguments selected by the embedding host.
    pub arguments: Vec<String>,
    /// Working directory selected without embedding user-specific host paths.
    pub working_directory: String,
    /// Startup output observed through subscribed client egress.
    pub startup_output: String,
    /// Terminal input sent through the client-facing path.
    pub terminal_input: String,
    /// Echoed output observed through subscribed client egress.
    pub echoed_output: String,
    /// Resize dimensions sent through the client-facing path.
    pub resized_to: Option<(u16, u16)>,
    /// Plain terminal screen contents returned by the public read-screen command.
    pub screen_text: String,
    /// Snapshot byte length returned by the public capture-snapshot command.
    pub snapshot_bytes: usize,
    /// Snapshot dimensions returned by the public capture-snapshot command.
    pub snapshot_size: Option<(u16, u16)>,
    /// Activity classification after real PTY output.
    pub activity_status: SessionActivityStatus,
    /// Whether shutdown moved the managed session toward stopping.
    pub shutdown_observed: bool,
}

impl EngineSmokeReport {
    /// Render deterministic, scrubbed lines for the dev executable.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        vec![
            "botster-core real embedder smoke".to_string(),
            format!("real embedder ran: {}", self.ran_real_embedder),
            format!("session spawned: {}", self.spawned_session_id.0),
            format!("client attached: {}", self.attached_client_id.0),
            format!("explicit command: {} {:?}", self.executable, self.arguments),
            format!("working directory: {}", self.working_directory),
            format!("startup output observed: {:?}", self.startup_output),
            format!("terminal input routed: {:?}", self.terminal_input),
            format!("echoed output observed: {:?}", self.echoed_output),
            format!("resize requested: {:?}", self.resized_to),
            format!("screen text observed: {:?}", self.screen_text),
            format!("snapshot bytes observed: {}", self.snapshot_bytes),
            format!("snapshot size observed: {:?}", self.snapshot_size),
            format!("activity status: {:?}", self.activity_status),
            format!("shutdown observed: {}", self.shutdown_observed),
        ]
    }
}

/// Error returned when the dev smoke path fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSmokeError {
    message: String,
}

impl EngineSmokeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EngineSmokeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EngineSmokeError {}

/// Run the dev-only real embedder smoke scenario used by both the binary and tests.
pub fn run_engine_smoke() -> Result<EngineSmokeReport, EngineSmokeError> {
    run_real_embedder_smoke()
}

#[cfg(unix)]
fn run_real_embedder_smoke() -> Result<EngineSmokeReport, EngineSmokeError> {
    let mut engine = DefaultBotsterEngine::new();
    let request = real_embedder_spawn_request();
    let session_id = request.session_id.clone();
    let client_id = ClientId("real-embedder-client".to_string());
    let subscription_id = SubscriptionId("real-embedder-subscription".to_string());
    let mut logical_clock = 20;

    let spawn = engine
        .execute_command(DefaultEngineCommand::SpawnSession {
            request: request.clone(),
            metadata: CoreSessionMetadata::new(),
        })
        .map_err(|error| EngineSmokeError::new(format!("spawn failed: {error}")))?;
    let EngineCommandOutcome::SpawnSession(spawn) = spawn else {
        return Err(EngineSmokeError::new("spawn command returned wrong result"));
    };
    if spawn.handle.session_id != session_id {
        return Err(EngineSmokeError::new(
            "spawned session id did not match request",
        ));
    }

    let smoke_result = run_spawned_embedder_smoke(
        &mut engine,
        request.clone(),
        session_id.clone(),
        client_id,
        subscription_id,
        &mut logical_clock,
    );
    if let Err(error) = smoke_result {
        let _ = shutdown_session(&mut engine, &session_id, logical_clock);
        return Err(error);
    }

    let mut report = smoke_result.expect("error branch returned above");
    let shutdown = shutdown_session(&mut engine, &session_id, logical_clock)?;
    report.shutdown_observed = shutdown.observations.iter().any(|observation| {
        observation
            == &BotsterEngineObservation::SessionLifecycle {
                session_id: session_id.clone(),
                state: SessionLifecycleState::Stopping,
            }
    });

    Ok(report)
}

#[cfg(unix)]
fn run_spawned_embedder_smoke(
    engine: &mut DefaultBotsterEngine,
    request: SessionSpawnRequest,
    session_id: SessionId,
    client_id: ClientId,
    subscription_id: SubscriptionId,
    logical_clock: &mut u64,
) -> Result<EngineSmokeReport, EngineSmokeError> {
    engine
        .execute_command(DefaultEngineCommand::AttachClient {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            subscription_id,
            now_seconds: *logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("attach failed: {error}")))?;
    *logical_clock += 1;

    let startup_output = drain_until_text(engine, &session_id, b"ready", logical_clock)?;

    let input = "ping-embedder\n";
    engine
        .execute_command(DefaultEngineCommand::SendInput {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            data: input.as_bytes().to_vec(),
            now_seconds: *logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("input failed: {error}")))?;
    *logical_clock += 1;

    let echoed_output =
        drain_until_text(engine, &session_id, b"echo:ping-embedder", logical_clock)?;

    let resized_to = (30, 100);
    engine
        .execute_command(DefaultEngineCommand::Resize {
            client_id,
            session_id: session_id.clone(),
            rows: resized_to.0,
            cols: resized_to.1,
            now_seconds: *logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("resize failed: {error}")))?;
    *logical_clock += 1;

    let screen_text = read_screen(engine, &session_id, logical_clock)?;
    let snapshot = capture_snapshot(engine, &session_id, logical_clock)?;

    let activity_status = engine
        .classify_activity(&session_id, *logical_clock, 5)
        .map_err(|error| EngineSmokeError::new(format!("classify failed: {error}")))?;

    Ok(EngineSmokeReport {
        ran_real_embedder: true,
        spawned_session_id: session_id,
        attached_client_id: ClientId("real-embedder-client".to_string()),
        executable: request.executable,
        arguments: request.arguments,
        working_directory: request.working_directory.path,
        startup_output,
        terminal_input: input.to_string(),
        echoed_output,
        resized_to: Some(resized_to),
        screen_text,
        snapshot_bytes: snapshot.bytes,
        snapshot_size: Some(snapshot.size),
        activity_status,
        shutdown_observed: false,
    })
}

#[cfg(not(unix))]
fn run_real_embedder_smoke() -> Result<EngineSmokeReport, EngineSmokeError> {
    Ok(EngineSmokeReport {
        ran_real_embedder: false,
        spawned_session_id: SessionId("real-embedder-session".to_string()),
        attached_client_id: ClientId("real-embedder-client".to_string()),
        executable: "sh".to_string(),
        arguments: Vec::new(),
        working_directory: ".".to_string(),
        startup_output: "skipped: local PTY example requires Unix".to_string(),
        terminal_input: String::new(),
        echoed_output: String::new(),
        resized_to: None,
        screen_text: String::new(),
        snapshot_bytes: 0,
        snapshot_size: None,
        activity_status: SessionActivityStatus::Idle,
        shutdown_observed: false,
    })
}

#[cfg(unix)]
fn real_embedder_spawn_request() -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id: request_id("spawn"),
        session_id: SessionId("real-embedder-session".to_string()),
        executable: "sh".to_string(),
        arguments: vec![
            "-c".to_string(),
            "printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"
                .to_string(),
        ],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

#[cfg(unix)]
fn drain_until_text(
    engine: &mut DefaultBotsterEngine,
    session_id: &SessionId,
    needle: &[u8],
    logical_clock: &mut u64,
) -> Result<String, EngineSmokeError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = Vec::new();

    while Instant::now() < deadline {
        let output = engine
            .drain_runtime_once(session_id, *logical_clock)
            .map_err(|error| EngineSmokeError::new(format!("drain failed: {error}")))?;
        *logical_clock += 1;

        for (_, frame) in output.client_egress {
            if let TransportEgress::TerminalOutput { data, .. } = frame {
                observed.extend(data);
            }
        }

        if observed
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return String::from_utf8(observed)
                .map_err(|error| EngineSmokeError::new(format!("output was not utf-8: {error}")));
        }

        thread::sleep(Duration::from_millis(20));
    }

    Err(EngineSmokeError::new(format!(
        "timed out waiting for {:?} in {:?}",
        String::from_utf8_lossy(needle),
        String::from_utf8_lossy(&observed)
    )))
}

#[cfg(unix)]
fn read_screen(
    engine: &mut DefaultBotsterEngine,
    session_id: &SessionId,
    logical_clock: &mut u64,
) -> Result<String, EngineSmokeError> {
    let output = engine
        .execute_command(DefaultEngineCommand::ReadScreen {
            request_id: request_id("read-screen"),
            session_id: session_id.clone(),
            now_seconds: *logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("read screen failed: {error}")))?;
    *logical_clock += 1;

    let EngineCommandOutcome::Output(output) = output else {
        return Err(EngineSmokeError::new(
            "read screen command returned wrong result",
        ));
    };
    output
        .session_events
        .into_iter()
        .find_map(|event| match event {
            SessionIoEvent::ScreenReady(screen) => Some(screen.text),
            _ => None,
        })
        .ok_or_else(|| EngineSmokeError::new("read screen did not return ScreenReady"))
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct SnapshotEvidence {
    bytes: usize,
    size: (u16, u16),
}

#[cfg(unix)]
fn capture_snapshot(
    engine: &mut DefaultBotsterEngine,
    session_id: &SessionId,
    logical_clock: &mut u64,
) -> Result<SnapshotEvidence, EngineSmokeError> {
    let output = engine
        .execute_command(DefaultEngineCommand::CaptureSnapshot {
            request_id: request_id("capture-snapshot"),
            session_id: session_id.clone(),
            now_seconds: *logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("capture snapshot failed: {error}")))?;
    *logical_clock += 1;

    let EngineCommandOutcome::Output(output) = output else {
        return Err(EngineSmokeError::new(
            "capture snapshot command returned wrong result",
        ));
    };
    output
        .session_events
        .into_iter()
        .find_map(|event| match event {
            SessionIoEvent::SnapshotReady(snapshot) => Some(SnapshotEvidence {
                bytes: snapshot.data.len(),
                size: (snapshot.rows, snapshot.cols),
            }),
            _ => None,
        })
        .ok_or_else(|| EngineSmokeError::new("capture snapshot did not return SnapshotReady"))
}

#[cfg(unix)]
fn shutdown_session(
    engine: &mut DefaultBotsterEngine,
    session_id: &SessionId,
    logical_clock: u64,
) -> Result<botster_core::BotsterEngineOutput, EngineSmokeError> {
    let shutdown = engine
        .execute_command(DefaultEngineCommand::Shutdown {
            session_id: session_id.clone(),
            reason: "real embedder smoke complete".to_string(),
            now_seconds: logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("shutdown failed: {error}")))?;
    let EngineCommandOutcome::Output(shutdown) = shutdown else {
        return Err(EngineSmokeError::new(
            "shutdown command returned wrong result",
        ));
    };
    Ok(shutdown)
}

#[cfg(unix)]
fn request_id(value: &str) -> RequestId {
    RequestId(format!("real-embedder-{value}"))
}

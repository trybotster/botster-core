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
    SessionId, SessionLifecycleState, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TransportEgress,
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

    engine
        .execute_command(DefaultEngineCommand::AttachClient {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            subscription_id,
            now_seconds: logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("attach failed: {error}")))?;
    logical_clock += 1;

    let startup_output = drain_until_text(&mut engine, &session_id, b"ready", &mut logical_clock)?;

    let input = "ping-embedder\n";
    engine
        .execute_command(DefaultEngineCommand::SendInput {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            data: input.as_bytes().to_vec(),
            now_seconds: logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("input failed: {error}")))?;
    logical_clock += 1;

    let echoed_output = drain_until_text(
        &mut engine,
        &session_id,
        b"echo:ping-embedder",
        &mut logical_clock,
    )?;

    let resized_to = (30, 100);
    engine
        .execute_command(DefaultEngineCommand::Resize {
            client_id,
            session_id: session_id.clone(),
            rows: resized_to.0,
            cols: resized_to.1,
            now_seconds: logical_clock,
        })
        .map_err(|error| EngineSmokeError::new(format!("resize failed: {error}")))?;
    logical_clock += 1;

    let activity_status = engine
        .classify_activity(&session_id, logical_clock, 5)
        .map_err(|error| EngineSmokeError::new(format!("classify failed: {error}")))?;

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
    let shutdown_observed = shutdown.observations.iter().any(|observation| {
        observation
            == &BotsterEngineObservation::SessionLifecycle {
                session_id: session_id.clone(),
                state: SessionLifecycleState::Stopping,
            }
    });

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
        activity_status,
        shutdown_observed,
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
fn request_id(value: &str) -> RequestId {
    RequestId(format!("real-embedder-{value}"))
}

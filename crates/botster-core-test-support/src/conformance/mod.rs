//! Downstream conformance helpers for the managed local session runtime.
//!
//! These helpers intentionally wrap public `botster_core` APIs. They do not
//! remap runtime output into worker events; real PTY output is drained through
//! `ManagedSessionRuntime::drain_runtime_once`.

use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use botster_core::{
    ClientId, CoreSession, CoreSessionMetadata, LocalProcessRuntime, ManagedSessionRuntime,
    ManagedSessionRuntimeError, MultiplexerEngineOutcome, ResizePayload, SessionId,
    SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TransportEgress,
    TransportIngress,
};
#[cfg(feature = "local-runtime")]
use botster_core::{
    DefaultBotsterEngine, DefaultBotsterEngineError, DefaultEngineCommand, EngineCommandError,
    EngineCommandKind, EngineCommandOutcome, EngineSessionInspection, PreparedSnapshotRequest,
    ProcessExitedPayload, RequestId, SessionActivityStatus, SessionIoEvent,
};

/// Explicit reason a local PTY conformance test cannot run on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipReason {
    /// Human-readable skip reason.
    pub reason: String,
}

impl SkipReason {
    /// Build a skip reason.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Return whether the host supports local PTY-backed conformance tests.
pub fn require_local_pty() -> Result<(), SkipReason> {
    if cfg!(unix) {
        Ok(())
    } else {
        Err(SkipReason::new(
            "local PTY conformance tests require a Unix host",
        ))
    }
}

/// Error returned by managed local conformance helpers.
#[derive(Debug)]
pub enum EngineConformanceError {
    /// The host cannot run this conformance path.
    Skipped(SkipReason),
    /// The public managed runtime returned an error.
    ManagedRuntime(ManagedSessionRuntimeError),
    /// The typed command facade returned an error.
    #[cfg(feature = "local-runtime")]
    Command(EngineCommandError<DefaultBotsterEngineError>),
    /// The expected runtime output did not arrive before the deadline.
    Timeout {
        /// Bytes observed before timing out.
        observed_output: String,
    },
    /// A many-PTY load harness assertion failed.
    #[cfg(feature = "local-runtime")]
    ManyPtyLoad {
        /// Hot-path phase label.
        phase: &'static str,
        /// Session id involved in the failure, when available.
        session_id: Option<SessionId>,
        /// Client id involved in the failure, when available.
        client_id: Option<ClientId>,
        /// Failure details.
        details: String,
    },
}

impl fmt::Display for EngineConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skipped(reason) => write!(formatter, "{}", reason.reason),
            Self::ManagedRuntime(error) => write!(formatter, "{error}"),
            #[cfg(feature = "local-runtime")]
            Self::Command(error) => write!(formatter, "{error}"),
            Self::Timeout { observed_output } => {
                write!(
                    formatter,
                    "timed out waiting for output; observed {observed_output:?}"
                )
            }
            #[cfg(feature = "local-runtime")]
            Self::ManyPtyLoad {
                phase,
                session_id,
                client_id,
                details,
            } => write!(
                formatter,
                "many-PTY load failure in {phase}; session={session_id:?}; client={client_id:?}; {details}"
            ),
        }
    }
}

impl Error for EngineConformanceError {}

impl From<ManagedSessionRuntimeError> for EngineConformanceError {
    fn from(error: ManagedSessionRuntimeError) -> Self {
        Self::ManagedRuntime(error)
    }
}

#[cfg(feature = "local-runtime")]
impl From<EngineCommandError<DefaultBotsterEngineError>> for EngineConformanceError {
    fn from(error: EngineCommandError<DefaultBotsterEngineError>) -> Self {
        Self::Command(error)
    }
}

/// Disposable local session that drives the public typed command facade.
#[cfg(feature = "local-runtime")]
pub struct DisposableCommandLocalSession {
    engine: DefaultBotsterEngine,
    session_id: SessionId,
    attached_clients: Vec<(ClientId, SubscriptionId)>,
}

#[cfg(feature = "local-runtime")]
impl DisposableCommandLocalSession {
    /// Spawn a disposable local session through `DefaultEngineCommand`.
    pub fn spawn(
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<Self, EngineConformanceError> {
        require_local_pty().map_err(EngineConformanceError::Skipped)?;

        let session_id = request.session_id.clone();
        let mut engine = DefaultBotsterEngine::new();
        engine.execute_command(DefaultEngineCommand::SpawnSession { request, metadata })?;

        Ok(Self {
            engine,
            session_id,
            attached_clients: Vec::new(),
        })
    }

    /// Return the session id owned by this disposable harness.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Return the core session state recorded by the public command facade.
    #[must_use]
    pub fn session(&self) -> Option<&CoreSession> {
        self.engine.session(&self.session_id)
    }

    /// Return the wrapped default command engine.
    #[must_use]
    pub const fn engine(&self) -> &DefaultBotsterEngine {
        &self.engine
    }

    /// Return the wrapped default command engine mutably.
    pub fn engine_mut(&mut self) -> &mut DefaultBotsterEngine {
        &mut self.engine
    }

    /// Attach one fake downstream client through `DefaultEngineCommand`.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        let outcome = self
            .engine
            .execute_command(DefaultEngineCommand::AttachClient {
                client_id: client_id.clone(),
                session_id: self.session_id.clone(),
                subscription_id: subscription_id.clone(),
                now_seconds,
            })?;
        self.attached_clients.push((client_id, subscription_id));
        Ok(outcome)
    }

    /// Detach one fake downstream client through `DefaultEngineCommand`.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::DetachClient {
                client_id,
                session_id: self.session_id.clone(),
                subscription_id,
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Write terminal bytes through `DefaultEngineCommand`.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::SendInput {
                client_id,
                session_id: self.session_id.clone(),
                data: data.into(),
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Resize the terminal through `DefaultEngineCommand`.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::Resize {
                client_id,
                session_id: self.session_id.clone(),
                rows,
                cols,
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// List sessions through `DefaultEngineCommand`.
    pub fn list_sessions(&mut self) -> Result<Vec<CoreSession>, EngineConformanceError> {
        match self
            .engine
            .execute_command(DefaultEngineCommand::ListSessions)?
        {
            EngineCommandOutcome::Sessions(sessions) => Ok(sessions),
            outcome => panic!("ListSessions returned unexpected outcome: {outcome:?}"),
        }
    }

    /// Inspect this session through `DefaultEngineCommand`.
    pub fn inspect_session(
        &mut self,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, EngineConformanceError> {
        match self
            .engine
            .execute_command(DefaultEngineCommand::InspectSession {
                session_id: self.session_id.clone(),
                now_seconds,
                active_threshold_seconds,
            })? {
            EngineCommandOutcome::Inspection(inspection) => Ok(inspection),
            outcome => panic!("InspectSession returned unexpected outcome: {outcome:?}"),
        }
    }

    /// Read the current plain screen through `DefaultEngineCommand`.
    pub fn read_screen(
        &mut self,
        request_id: RequestId,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::ReadScreen {
                request_id,
                session_id: self.session_id.clone(),
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Capture an opaque snapshot through `DefaultEngineCommand`.
    pub fn capture_snapshot(
        &mut self,
        request_id: RequestId,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::CaptureSnapshot {
                request_id,
                session_id: self.session_id.clone(),
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Replay or prepare an opaque snapshot through `DefaultEngineCommand`.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::ReplaySnapshot {
                request,
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Drain runtime output once through the public default engine bridge.
    ///
    /// Runtime draining is not itself a command; this pumps the real local PTY
    /// so typed attach/input/resize commands can be observed.
    pub fn drain_runtime_once(
        &mut self,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        Ok(self
            .engine
            .drain_runtime_once(&self.session_id, last_output_at)?)
    }

    /// Drain through `DefaultBotsterEngine::drain_runtime_once` until output matches.
    pub fn drain_runtime_until_output_contains(
        &mut self,
        needle: &[u8],
        last_output_at: u64,
        timeout: Duration,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        let deadline = Instant::now() + timeout;
        let mut combined = MultiplexerEngineOutcome::empty();

        while Instant::now() < deadline {
            let outcome = self.drain_runtime_once(last_output_at)?;
            append_outcome(&mut combined, outcome);
            if terminal_output_bytes(&combined)
                .windows(needle.len())
                .any(|window| window == needle)
            {
                return Ok(combined);
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        Err(EngineConformanceError::Timeout {
            observed_output: String::from_utf8_lossy(&terminal_output_bytes(&combined))
                .into_owned(),
        })
    }

    /// Shut down the local session through `DefaultEngineCommand`.
    pub fn shutdown(
        &mut self,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<EngineCommandOutcome, EngineConformanceError> {
        self.engine
            .execute_command(DefaultEngineCommand::Shutdown {
                session_id: self.session_id.clone(),
                reason: reason.into(),
                now_seconds,
            })
            .map_err(EngineConformanceError::from)
    }

    /// Return clients attached through this harness.
    #[must_use]
    pub fn attached_clients(&self) -> &[(ClientId, SubscriptionId)] {
        &self.attached_clients
    }
}

#[cfg(feature = "local-runtime")]
impl Drop for DisposableCommandLocalSession {
    fn drop(&mut self) {
        let _ = self.shutdown("disposable command local session dropped", 0);
    }
}

/// Configuration for the public many-PTY load harness.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone)]
pub struct ManyPtyLoadConfig {
    /// Number of local PTY sessions to spawn.
    pub session_count: usize,
    /// Overall deadline for round-robin draining.
    pub timeout: Duration,
    /// Number of bounded output lines printed by normal sessions.
    pub normal_output_lines: usize,
    /// Optional index of one noisier bounded-output session.
    pub noisy_session_index: Option<usize>,
    /// Number of bounded output lines printed by the noisy session.
    pub noisy_output_lines: usize,
}

#[cfg(feature = "local-runtime")]
impl ManyPtyLoadConfig {
    /// CI-safe default load: 20 real local PTY sessions with bounded output.
    #[must_use]
    pub fn ci_default() -> Self {
        Self {
            session_count: 20,
            timeout: Duration::from_secs(20),
            normal_output_lines: 3,
            noisy_session_index: None,
            noisy_output_lines: 64,
        }
    }

    /// Build a local 50-session configuration.
    #[must_use]
    pub fn local_50() -> Self {
        Self {
            session_count: 50,
            timeout: Duration::from_secs(45),
            ..Self::ci_default()
        }
    }

    /// Build an opt-in 100-session configuration.
    #[must_use]
    pub fn opt_in_100() -> Self {
        Self {
            session_count: 100,
            timeout: Duration::from_secs(90),
            ..Self::ci_default()
        }
    }

    /// Enable one bounded noisy session at the selected index.
    #[must_use]
    pub const fn with_noisy_session(mut self, index: usize) -> Self {
        self.noisy_session_index = Some(index);
        self
    }
}

/// Rough timing and delivery observations from the many-PTY load harness.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone)]
pub struct ManyPtyLoadReport {
    /// Number of sessions requested.
    pub session_count: usize,
    /// Total elapsed wall-clock time.
    pub elapsed: Duration,
    /// Number of round-robin drain passes performed.
    pub drain_rounds: usize,
    /// Total terminal-output bytes delivered to attached clients.
    pub total_output_bytes: usize,
    /// Number of sessions whose ready and done markers reached every client.
    pub outputs_completed: usize,
    /// Number of sessions with observed process-exit events.
    pub exits_observed: usize,
    /// Optional noisy session id.
    pub noisy_session_id: Option<SessionId>,
    /// Queue or backpressure observations exposed by the current public API.
    pub queue_backpressure_observations: Vec<String>,
    /// Slow-client or plugin-pressure limitation for this run.
    pub slow_client_observation: String,
}

#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone)]
struct ManyPtySessionState {
    session_id: SessionId,
    client_id: ClientId,
    subscription_id: SubscriptionId,
    ready_marker: Vec<u8>,
    done_marker: Vec<u8>,
    output: Vec<u8>,
    ready_seen: bool,
    done_seen: bool,
    exit_seen: bool,
}

#[cfg(feature = "local-runtime")]
impl ManyPtySessionState {
    fn complete(&self) -> bool {
        self.ready_seen && self.done_seen && self.exit_seen
    }
}

#[cfg(feature = "local-runtime")]
struct ManyPtyLoadHarness {
    engine: DefaultBotsterEngine,
    sessions: Vec<ManyPtySessionState>,
}

#[cfg(feature = "local-runtime")]
impl ManyPtyLoadHarness {
    fn new() -> Self {
        Self {
            engine: DefaultBotsterEngine::new(),
            sessions: Vec::new(),
        }
    }

    fn shutdown_all(&mut self, now_seconds: u64) {
        for session in &self.sessions {
            let _ = self.engine.execute_command(DefaultEngineCommand::Shutdown {
                session_id: session.session_id.clone(),
                reason: "many-PTY load harness cleanup".to_string(),
                now_seconds,
            });
        }
    }
}

#[cfg(feature = "local-runtime")]
impl Drop for ManyPtyLoadHarness {
    fn drop(&mut self) {
        self.shutdown_all(0);
    }
}

/// Run a many-session local PTY load check through the public default engine.
///
/// The harness uses one `DefaultBotsterEngine`, spawns explicit bounded shell
/// commands, attaches one client per session, drains sessions in round-robin
/// passes, and asserts both terminal output fanout and process-exit delivery.
#[cfg(feature = "local-runtime")]
pub fn run_many_pty_load(
    config: ManyPtyLoadConfig,
) -> Result<ManyPtyLoadReport, EngineConformanceError> {
    require_local_pty().map_err(EngineConformanceError::Skipped)?;
    if config.session_count == 0 {
        return Err(many_pty_error(
            "config",
            None,
            None,
            "session_count must be greater than zero",
        ));
    }

    let started = Instant::now();
    let mut harness = ManyPtyLoadHarness::new();
    spawn_many_pty_sessions(&mut harness, &config)?;
    attach_many_pty_clients(&mut harness)?;
    let report = drain_many_pty_sessions(&mut harness, &config, started)?;
    harness.shutdown_all(10_000);

    Ok(report)
}

#[cfg(feature = "local-runtime")]
fn spawn_many_pty_sessions(
    harness: &mut ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
) -> Result<(), EngineConformanceError> {
    for index in 0..config.session_count {
        let session_id = SessionId(format!("many-pty-session-{index:03}"));
        let client_id = ClientId(format!("many-pty-client-{index:03}"));
        let subscription_id = SubscriptionId(format!("many-pty-subscription-{index:03}"));
        let ready_marker = format!("many-pty:{index}:ready").into_bytes();
        let done_marker = format!("many-pty:{index}:done").into_bytes();
        let line_count = if config.noisy_session_index == Some(index) {
            config.noisy_output_lines
        } else {
            config.normal_output_lines
        };
        let script = many_pty_script(index, line_count);
        let request = local_shell_spawn_request(
            RequestId(format!("many-pty-spawn-{index:03}")),
            session_id.clone(),
            script,
        );

        harness
            .engine
            .execute_command(DefaultEngineCommand::SpawnSession {
                request,
                metadata: CoreSessionMetadata::new(),
            })
            .map_err(EngineConformanceError::from)
            .map_err(|error| wrap_many_pty_error("spawn", &session_id, None, error))?;

        harness.sessions.push(ManyPtySessionState {
            session_id,
            client_id,
            subscription_id,
            ready_marker,
            done_marker,
            output: Vec::new(),
            ready_seen: false,
            done_seen: false,
            exit_seen: false,
        });
    }

    Ok(())
}

#[cfg(feature = "local-runtime")]
fn attach_many_pty_clients(harness: &mut ManyPtyLoadHarness) -> Result<(), EngineConformanceError> {
    for session in &harness.sessions {
        harness
            .engine
            .execute_command(DefaultEngineCommand::AttachClient {
                client_id: session.client_id.clone(),
                session_id: session.session_id.clone(),
                subscription_id: session.subscription_id.clone(),
                now_seconds: 1,
            })
            .map_err(EngineConformanceError::from)
            .map_err(|error| {
                wrap_many_pty_error(
                    "attach",
                    &session.session_id,
                    Some(&session.client_id),
                    error,
                )
            })?;
    }

    Ok(())
}

#[cfg(feature = "local-runtime")]
fn drain_many_pty_sessions(
    harness: &mut ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
    started: Instant,
) -> Result<ManyPtyLoadReport, EngineConformanceError> {
    let deadline = started + config.timeout;
    let mut drain_rounds = 0;
    let mut total_output_bytes = 0;

    while Instant::now() < deadline {
        let mut made_progress = false;
        drain_rounds += 1;

        for index in 0..harness.sessions.len() {
            if harness.sessions[index].complete() {
                continue;
            }

            let session_id = harness.sessions[index].session_id.clone();
            let output = harness
                .engine
                .drain_runtime_once(&session_id, drain_rounds as u64 + 1)
                .map_err(EngineConformanceError::from)
                .map_err(|error| wrap_many_pty_error("drain", &session_id, None, error))?;

            if !output.client_egress.is_empty() || !output.session_events.is_empty() {
                made_progress = true;
            }

            let delivered = terminal_output_bytes_for(
                &output,
                &harness.sessions[index].client_id,
                &harness.sessions[index].subscription_id,
                &session_id,
            );
            total_output_bytes += delivered.len();
            harness.sessions[index].output.extend(delivered);
            update_many_pty_completion(&mut harness.sessions[index], &output);
        }

        if harness.sessions.iter().all(ManyPtySessionState::complete) {
            return Ok(many_pty_report(
                harness,
                config,
                started.elapsed(),
                drain_rounds,
                total_output_bytes,
            ));
        }

        if !made_progress {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    let pending = harness
        .sessions
        .iter()
        .filter(|session| !session.complete())
        .map(|session| {
            format!(
                "{} ready={} done={} exit={} observed={:?}",
                session.session_id.0,
                session.ready_seen,
                session.done_seen,
                session.exit_seen,
                String::from_utf8_lossy(&bounded_tail(&session.output, 160))
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    Err(many_pty_error(
        "timeout",
        None,
        None,
        format!(
            "deadline {:?} elapsed before all sessions completed; pending: {pending}",
            config.timeout
        ),
    ))
}

#[cfg(feature = "local-runtime")]
fn update_many_pty_completion(
    session: &mut ManyPtySessionState,
    output: &MultiplexerEngineOutcome,
) {
    session.ready_seen = session
        .output
        .windows(session.ready_marker.len())
        .any(|window| window == session.ready_marker);
    session.done_seen = session
        .output
        .windows(session.done_marker.len())
        .any(|window| window == session.done_marker);
    session.exit_seen |= output.session_events.iter().any(|event| {
        matches!(
            event,
            SessionIoEvent::ProcessExited {
                session_id,
                payload: ProcessExitedPayload {
                    exit_code: Some(0),
                    ..
                },
            } if session_id == &session.session_id
        )
    });
}

#[cfg(feature = "local-runtime")]
fn many_pty_report(
    harness: &ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
    elapsed: Duration,
    drain_rounds: usize,
    total_output_bytes: usize,
) -> ManyPtyLoadReport {
    let outputs_completed = harness
        .sessions
        .iter()
        .filter(|session| session.ready_seen && session.done_seen)
        .count();
    let exits_observed = harness
        .sessions
        .iter()
        .filter(|session| session.exit_seen)
        .count();
    let noisy_session_id = config
        .noisy_session_index
        .and_then(|index| harness.sessions.get(index))
        .map(|session| session.session_id.clone());

    ManyPtyLoadReport {
        session_count: config.session_count,
        elapsed,
        drain_rounds,
        total_output_bytes,
        outputs_completed,
        exits_observed,
        noisy_session_id,
        queue_backpressure_observations: vec![
            "DefaultBotsterEngine exposes delivered client egress and typed session events; it does not expose queue-depth or backpressure counters on this public path.".to_string(),
        ],
        slow_client_observation:
            "No public slow-client/plugin-pressure primitive is available through DefaultBotsterEngine; adversarial coverage is limited to one bounded noisy PTY session."
                .to_string(),
    }
}

#[cfg(feature = "local-runtime")]
fn many_pty_script(index: usize, line_count: usize) -> String {
    format!(
        "printf 'many-pty:{index}:ready\\n'; i=0; while [ \"$i\" -lt {line_count} ]; do printf 'many-pty:{index}:line:%03d\\n' \"$i\"; i=$((i + 1)); done; printf 'many-pty:{index}:done\\n'"
    )
}

#[cfg(feature = "local-runtime")]
fn bounded_tail(bytes: &[u8], max_len: usize) -> Vec<u8> {
    let start = bytes.len().saturating_sub(max_len);
    bytes[start..].to_vec()
}

#[cfg(feature = "local-runtime")]
fn wrap_many_pty_error(
    phase: &'static str,
    session_id: &SessionId,
    client_id: Option<&ClientId>,
    error: EngineConformanceError,
) -> EngineConformanceError {
    many_pty_error(
        phase,
        Some(session_id.clone()),
        client_id.cloned(),
        error.to_string(),
    )
}

#[cfg(feature = "local-runtime")]
fn many_pty_error(
    phase: &'static str,
    session_id: Option<SessionId>,
    client_id: Option<ClientId>,
    details: impl Into<String>,
) -> EngineConformanceError {
    EngineConformanceError::ManyPtyLoad {
        phase,
        session_id,
        client_id,
        details: details.into(),
    }
}

/// Disposable managed session backed by the public local PTY runtime.
pub struct DisposableManagedLocalSession {
    runtime: ManagedSessionRuntime<LocalProcessRuntime>,
    session_id: SessionId,
    attached_clients: Vec<(ClientId, SubscriptionId)>,
}

impl DisposableManagedLocalSession {
    /// Spawn a disposable local session through `ManagedSessionRuntime`.
    pub fn spawn(
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<Self, EngineConformanceError> {
        require_local_pty().map_err(EngineConformanceError::Skipped)?;

        let session_id = request.session_id.clone();
        let mut runtime = ManagedSessionRuntime::new(LocalProcessRuntime::new());
        runtime.spawn_session(request, metadata)?;

        Ok(Self {
            runtime,
            session_id,
            attached_clients: Vec::new(),
        })
    }

    /// Return the session id owned by this disposable harness.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Return the core session state recorded by the public runtime facade.
    #[must_use]
    pub fn session(&self) -> Option<&CoreSession> {
        self.runtime.session(&self.session_id)
    }

    /// Return the wrapped public managed runtime.
    #[must_use]
    pub const fn runtime(&self) -> &ManagedSessionRuntime<LocalProcessRuntime> {
        &self.runtime
    }

    /// Return the wrapped public managed runtime mutably.
    pub fn runtime_mut(&mut self) -> &mut ManagedSessionRuntime<LocalProcessRuntime> {
        &mut self.runtime
    }

    /// Attach one fake downstream client through public transport ingress.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        let outcome = self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id: client_id.clone(),
                session_id: self.session_id.clone(),
                subscription_id: subscription_id.clone(),
            },
            now_seconds,
        )?;
        self.attached_clients.push((client_id, subscription_id));
        Ok(outcome)
    }

    /// Write terminal bytes through public client ingress.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        Ok(self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id: self.session_id.clone(),
                data: data.into(),
            },
            now_seconds,
        )?)
    }

    /// Resize the terminal through public client ingress.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        Ok(self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id: self.session_id.clone(),
                rows,
                cols,
            },
            now_seconds,
        )?)
    }

    /// Drain runtime output once through the public managed-runtime bridge.
    pub fn drain_runtime_once(
        &mut self,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        Ok(self
            .runtime
            .drain_runtime_once(&self.session_id, last_output_at)?)
    }

    /// Drain through `ManagedSessionRuntime::drain_runtime_once` until output matches.
    pub fn drain_runtime_until_output_contains(
        &mut self,
        needle: &[u8],
        last_output_at: u64,
        timeout: Duration,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        let deadline = Instant::now() + timeout;
        let mut combined = MultiplexerEngineOutcome::empty();

        while Instant::now() < deadline {
            let outcome = self.drain_runtime_once(last_output_at)?;
            append_outcome(&mut combined, outcome);
            if terminal_output_bytes(&combined)
                .windows(needle.len())
                .any(|window| window == needle)
            {
                return Ok(combined);
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        Err(EngineConformanceError::Timeout {
            observed_output: String::from_utf8_lossy(&terminal_output_bytes(&combined))
                .into_owned(),
        })
    }

    /// Shut down the managed session through the public runtime facade.
    pub fn shutdown(
        &mut self,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
        Ok(self
            .runtime
            .shutdown_session(self.session_id.clone(), reason, now_seconds)?)
    }

    /// Return clients attached through this harness.
    #[must_use]
    pub fn attached_clients(&self) -> &[(ClientId, SubscriptionId)] {
        &self.attached_clients
    }
}

/// Build an explicit local shell spawn request for disposable tests.
#[must_use]
pub fn local_shell_spawn_request(
    request_id: botster_core::RequestId,
    session_id: SessionId,
    script: impl Into<String>,
) -> SessionSpawnRequest {
    SessionSpawnRequest {
        request_id,
        session_id,
        executable: "sh".to_string(),
        arguments: vec!["-c".to_string(), script.into()],
        working_directory: SpawnWorkingDirectory {
            path: ".".to_string(),
        },
        environment: SpawnEnvironment::default(),
        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
    }
}

/// Assert that each expected client received the same terminal output bytes.
pub fn assert_terminal_output_fanout(
    outcome: &MultiplexerEngineOutcome,
    session_id: &SessionId,
    clients: &[(ClientId, SubscriptionId)],
    expected: &[u8],
) {
    for (client_id, subscription_id) in clients {
        let received = terminal_output_bytes_for(outcome, client_id, subscription_id, session_id);
        assert!(
            received
                .windows(expected.len())
                .any(|window| window == expected),
            "expected client {client_id:?} subscription {subscription_id:?} to receive {expected:?}; got {received:?}"
        );
    }
}

/// Assert that public core session state recorded output activity.
pub fn assert_output_activity(session: &CoreSession, expected_last_output_at: u64) {
    assert_eq!(
        session.activity.last_output_at,
        Some(expected_last_output_at)
    );
}

/// Assert that a shutdown operation emitted lifecycle observations.
pub fn assert_shutdown_requested(outcome: &MultiplexerEngineOutcome, session_id: &SessionId) {
    assert!(
        outcome.observations.iter().any(|observation| matches!(
            observation,
            botster_core::MultiplexerEngineObservation::SessionLifecycle {
                session_id: observed_session_id,
                state: botster_core::SessionLifecycleState::Stopping,
            } if observed_session_id == session_id
        )),
        "expected shutdown lifecycle observation for {session_id:?}; got {:?}",
        outcome.observations
    );
}

/// Assert that one typed command outcome contains fanout output for each client.
#[cfg(feature = "local-runtime")]
pub fn assert_command_output_fanout(
    outcome: &EngineCommandOutcome,
    session_id: &SessionId,
    clients: &[(ClientId, SubscriptionId)],
    expected: &[u8],
) {
    match outcome {
        EngineCommandOutcome::Output(output) => {
            assert_terminal_output_fanout(output, session_id, clients, expected);
        }
        _ => panic!("expected output command outcome, got {outcome:?}"),
    }
}

/// Assert that a typed list command includes the disposable session.
#[cfg(feature = "local-runtime")]
pub fn assert_command_sessions_include(sessions: &[CoreSession], session_id: &SessionId) {
    assert!(
        sessions
            .iter()
            .any(|session| &session.session_id == session_id),
        "expected listed sessions to include {session_id:?}; got {sessions:?}"
    );
}

/// Assert that typed inspection reports the expected activity state.
#[cfg(feature = "local-runtime")]
pub fn assert_command_inspection_activity(
    inspection: &EngineSessionInspection,
    session_id: &SessionId,
    expected_status: SessionActivityStatus,
) {
    assert_eq!(&inspection.session.session_id, session_id);
    assert_eq!(inspection.activity_status, expected_status);
}

/// Assert that a screen command returned a typed screen event for the session.
#[cfg(feature = "local-runtime")]
pub fn assert_command_screen_ready(
    outcome: &EngineCommandOutcome,
    request_id: &RequestId,
    session_id: &SessionId,
) {
    let output = command_output(outcome);
    assert!(
        output.session_events.iter().any(|event| matches!(
            event,
            SessionIoEvent::ScreenReady(screen)
                if &screen.request_id == request_id && &screen.session_id == session_id
        )),
        "expected screen-ready event for {session_id:?}; got {:?}",
        output.session_events
    );
}

/// Assert that a snapshot command returned a typed snapshot event for the session.
#[cfg(feature = "local-runtime")]
pub fn assert_command_snapshot_ready(
    outcome: &EngineCommandOutcome,
    request_id: &RequestId,
    session_id: &SessionId,
) -> Vec<u8> {
    let output = command_output(outcome);
    output
        .session_events
        .iter()
        .find_map(|event| match event {
            SessionIoEvent::SnapshotReady(snapshot)
                if &snapshot.request_id == request_id && &snapshot.session_id == session_id =>
            {
                Some(snapshot.data.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "expected snapshot-ready event for {session_id:?}; got {:?}",
                output.session_events
            )
        })
}

/// Assert that snapshot replay succeeded or failed with a typed replay command error.
#[cfg(feature = "local-runtime")]
pub fn assert_command_replay_snapshot_behavior(
    result: &Result<EngineCommandOutcome, EngineConformanceError>,
    request_id: &RequestId,
    session_id: &SessionId,
) {
    match result {
        Ok(outcome) => {
            let output = command_output(outcome);
            assert!(
                output.session_events.iter().any(|event| matches!(
                    event,
                    SessionIoEvent::PreparedSnapshotReady(snapshot)
                        if &snapshot.request_id == request_id && &snapshot.session_id == session_id
                )),
                "expected prepared-snapshot event for {session_id:?}; got {:?}",
                output.session_events
            );
        }
        Err(EngineConformanceError::Command(error)) => {
            assert_eq!(error.kind, EngineCommandKind::ReplaySnapshot);
        }
        Err(error) => {
            panic!("expected replay snapshot outcome or typed command error, got {error}")
        }
    }
}

#[cfg(feature = "local-runtime")]
fn command_output(outcome: &EngineCommandOutcome) -> &MultiplexerEngineOutcome {
    match outcome {
        EngineCommandOutcome::Output(output) => output,
        _ => panic!("expected output command outcome, got {outcome:?}"),
    }
}

fn append_outcome(target: &mut MultiplexerEngineOutcome, source: MultiplexerEngineOutcome) {
    target.client_egress.extend(source.client_egress);
    target.session_requests.extend(source.session_requests);
    target
        .client_control_frames
        .extend(source.client_control_frames);
    target.session_events.extend(source.session_events);
    target.observations.extend(source.observations);
}

fn terminal_output_bytes(outcome: &MultiplexerEngineOutcome) -> Vec<u8> {
    outcome
        .client_egress
        .iter()
        .filter_map(|(_, egress)| match egress {
            TransportEgress::TerminalOutput { data, .. } => Some(data.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn terminal_output_bytes_for(
    outcome: &MultiplexerEngineOutcome,
    client_id: &ClientId,
    subscription_id: &SubscriptionId,
    session_id: &SessionId,
) -> Vec<u8> {
    outcome
        .client_egress
        .iter()
        .filter_map(|(received_client, egress)| match egress {
            TransportEgress::TerminalOutput {
                session_id: received_session_id,
                subscription_id: received_subscription_id,
                data,
            } if received_client == client_id
                && received_session_id == session_id
                && received_subscription_id == subscription_id =>
            {
                Some(data.as_slice())
            }
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

//! Downstream conformance helpers for the managed local session runtime.
//!
//! These helpers intentionally wrap public `botster_core` APIs. They do not
//! remap runtime output into worker events; real PTY output is drained through
//! `ManagedSessionRuntime::drain_runtime_once` or the fair aggregate drain
//! helper.

use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use botster_core::{
    ClientId, CoreSession, CoreSessionMetadata, EndpointId, EnvelopeId, EnvelopeTarget,
    LocalProcessRuntime, ManagedSessionRuntime, ManagedSessionRuntimeError,
    MultiplexerEngineOutcome, ResizePayload, RoutedEnvelope, RoutedEnvelopePayload, SessionId,
    SessionRuntimeErrorKind, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, TransportEgress, TransportIngress,
};
#[cfg(feature = "local-runtime")]
use botster_core::{
    DefaultBotsterEngine, DefaultBotsterEngineError, DefaultEngineCommand, EngineCommandError,
    EngineCommandKind, EngineCommandOutcome, EngineSessionInspection, MultiplexerEngineObservation,
    PreparedSnapshotRequest, ProcessExitedPayload, RequestId, SessionActivityStatus,
    SessionIoEvent, SessionIoRequest, SessionLifecycleState,
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

/// Build a synthetic host-owned coordination envelope over the generic primitive.
///
/// The returned envelope intentionally keeps workflow meaning in the payload
/// body. Core can route it by typed endpoint and topic metadata without knowing
/// what the host will do with the payload.
#[must_use]
pub fn host_coordination_envelope_fixture(
    id: impl Into<String>,
    source: impl Into<String>,
    topic: impl Into<String>,
    body: impl Into<Vec<u8>>,
) -> RoutedEnvelope {
    RoutedEnvelope::new(
        EnvelopeId(id.into()),
        EndpointId(source.into()),
        vec![EnvelopeTarget::Topic {
            topic: topic.into(),
        }],
        RoutedEnvelopePayload {
            content_type: "application/vnd.botster.host-coordination+json".to_string(),
            body: body.into(),
            extension: None,
        },
        1,
    )
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

/// Configuration for adversarial public command hot-path proof.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone)]
pub struct AdversarialHotPathConfig {
    /// Many-PTY load configuration used as the background pressure.
    pub load: ManyPtyLoadConfig,
    /// Per-command responsiveness bound. This is a regression bound, not a benchmark target.
    pub phase_budget: Duration,
    /// Deadline for observing the control session's input echo.
    pub input_timeout: Duration,
}

#[cfg(feature = "local-runtime")]
impl AdversarialHotPathConfig {
    /// CI-safe default: 20 PTYs, one bounded noisy PTY, and generous hot-path budgets.
    #[must_use]
    pub fn ci_default() -> Self {
        let mut load = ManyPtyLoadConfig::ci_default().with_noisy_session(0);
        load.timeout = Duration::from_secs(35);
        load.normal_output_lines = 2;
        load.noisy_output_lines = 24_000;

        Self {
            load,
            phase_budget: Duration::from_secs(2),
            input_timeout: Duration::from_secs(5),
        }
    }
}

/// One timed public command phase from the adversarial hot-path proof.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotPathPhaseTiming {
    /// Stable phase label.
    pub phase: &'static str,
    /// Elapsed time for this phase.
    pub elapsed: Duration,
}

/// Report produced by the adversarial public command hot-path proof.
#[cfg(feature = "local-runtime")]
#[derive(Debug, Clone)]
pub struct AdversarialHotPathReport {
    /// Number of many-PTY load sessions requested, excluding the input control session.
    pub session_count: usize,
    /// Noisy background session used for overlap proof.
    pub noisy_session_id: SessionId,
    /// Quiet load session that completed before probes ran.
    pub quiet_session_id: SessionId,
    /// Interactive control session used to prove `SendInput` delivery.
    pub control_session_id: SessionId,
    /// Number of quiet load sessions completed before probes ran.
    pub quiet_sessions_completed_before_probes: usize,
    /// Drain rounds completed before probes ran.
    pub drain_rounds_before_probes: usize,
    /// Total drain rounds, including cleanup.
    pub total_drain_rounds: usize,
    /// Whether the noisy session was still mid-output when hot-path probes ran.
    pub noisy_output_active_during_probes: bool,
    /// Timed hot-path command phases.
    pub phase_timings: Vec<HotPathPhaseTiming>,
    /// Deterministic command-phase budget exercised by this run.
    pub hot_path_budget_observation: String,
    /// Queue or backpressure observations exposed by the current public API.
    pub queue_backpressure_observations: Vec<String>,
    /// Number of synthetic sessions that either reached `Exited` or lost their runtime handle after shutdown.
    pub cleanup_exited_sessions: usize,
    /// Synthetic session ids still live after cleanup.
    pub live_sessions_after_cleanup: Vec<SessionId>,
    /// Slow-client/plugin proof boundary for the current public default-engine API.
    pub slow_client_plugin_observation: String,
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
/// commands, attaches one client per session, drains every live session once
/// per pass through the reusable fair helper, and asserts both terminal output
/// fanout and process-exit delivery.
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

/// Run an adversarial public-command hot-path proof through `DefaultBotsterEngine`.
///
/// Probes run while one noisy PTY is still mid-output, and `SendInput` is
/// proven through an observable echo from a live interactive control session.
#[cfg(feature = "local-runtime")]
pub fn run_adversarial_hot_path_load(
    config: AdversarialHotPathConfig,
) -> Result<AdversarialHotPathReport, EngineConformanceError> {
    require_local_pty().map_err(EngineConformanceError::Skipped)?;
    validate_adversarial_hot_path_config(&config)?;

    let started = Instant::now();
    let deadline = started + config.load.timeout;
    let noisy_index = config
        .load
        .noisy_session_index
        .expect("validated noisy index");
    let mut harness = ManyPtyLoadHarness::new();
    spawn_many_pty_sessions(&mut harness, &config.load)?;
    attach_many_pty_clients(&mut harness)?;

    let control = spawn_adversarial_control_session(&mut harness)?;
    let mut drain_rounds = 0;
    let mut total_output_bytes = 0;
    let mut queue_backpressure_observations = Vec::new();
    let mut probe_report = None;

    while Instant::now() < deadline {
        drain_rounds += 1;
        let output = harness
            .engine
            .drain_runtime_all_once(drain_rounds as u64 + 1)
            .map_err(EngineConformanceError::from)
            .map_err(|error| {
                many_pty_error(
                    "drain",
                    None,
                    None,
                    format!("fair aggregate drain failed: {error}"),
                )
            })?;

        queue_backpressure_observations.extend(queue_backpressure_observations_for(&output));
        for session in &mut harness.sessions {
            let delivered = terminal_output_bytes_for(
                &output,
                &session.client_id,
                &session.subscription_id,
                &session.session_id,
            );
            total_output_bytes += delivered.len();
            session.output.extend(delivered);
            update_many_pty_completion(session, &output);
        }

        if probe_report.is_none() && adversarial_probe_window_open(&harness, noisy_index) {
            let quiet_session = quiet_probe_session(&harness, noisy_index)?;
            probe_report = Some(run_hot_path_probes(
                &mut harness,
                &config,
                noisy_index,
                quiet_session,
                &control,
                drain_rounds,
            )?);
            break;
        }

        if output.client_egress.is_empty() && output.session_events.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    let mut report = probe_report.ok_or_else(|| {
        let noisy = &harness.sessions[noisy_index];
        many_pty_error(
            "probe-window",
            Some(noisy.session_id.clone()),
            None,
            format!(
                "deadline {:?} elapsed before quiet sessions completed while noisy output remained active; noisy ready={} done={} exit={} total_output_bytes={}",
                config.load.timeout,
                noisy.ready_seen,
                noisy.done_seen,
                noisy.exit_seen,
                total_output_bytes
            ),
        )
    })?;

    drain_remaining_adversarial_load(
        &mut harness,
        &config.load,
        deadline,
        &mut drain_rounds,
        &mut queue_backpressure_observations,
    )?;
    cleanup_adversarial_sessions(&mut harness, &control, &mut report, deadline)?;
    report.total_drain_rounds = drain_rounds.max(report.total_drain_rounds);
    report.hot_path_budget_observation = hot_path_budget_observation(
        report.phase_timings.len(),
        report.drain_rounds_before_probes,
        report.total_drain_rounds,
        config.phase_budget,
    );
    report.queue_backpressure_observations =
        dedupe_preserving_order(queue_backpressure_observations);
    if report.queue_backpressure_observations.is_empty() {
        report.queue_backpressure_observations.push(
            "DefaultBotsterEngine exposes typed reader backpressure observations; no reader pressure was observed in this run."
                .to_string(),
        );
    }

    Ok(report)
}

#[cfg(feature = "local-runtime")]
fn validate_adversarial_hot_path_config(
    config: &AdversarialHotPathConfig,
) -> Result<(), EngineConformanceError> {
    if config.load.session_count < 2 {
        return Err(many_pty_error(
            "config",
            None,
            None,
            "adversarial hot-path proof requires at least one noisy and one quiet load session",
        ));
    }
    match config.load.noisy_session_index {
        Some(index) if index < config.load.session_count => Ok(()),
        _ => Err(many_pty_error(
            "config",
            None,
            None,
            "adversarial hot-path proof requires a valid noisy_session_index",
        )),
    }
}

#[cfg(feature = "local-runtime")]
fn drain_remaining_adversarial_load(
    harness: &mut ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
    deadline: Instant,
    drain_rounds: &mut usize,
    queue_backpressure_observations: &mut Vec<String>,
) -> Result<(), EngineConformanceError> {
    while Instant::now() < deadline {
        if many_pty_completion_satisfied(harness, config, queue_backpressure_observations) {
            return Ok(());
        }

        *drain_rounds += 1;
        let output = drain_many_pty_harness_once(harness, *drain_rounds as u64 + 1)?;
        queue_backpressure_observations.extend(queue_backpressure_observations_for(&output));
        for session in &mut harness.sessions {
            let delivered = terminal_output_bytes_for(
                &output,
                &session.client_id,
                &session.subscription_id,
                &session.session_id,
            );
            session.output.extend(delivered);
            update_many_pty_completion(session, &output);
        }

        if output.client_egress.is_empty() && output.session_events.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    Err(many_pty_error(
        "post-probe-drain",
        None,
        None,
        format!(
            "deadline {:?} elapsed before background sessions reached completion",
            config.timeout
        ),
    ))
}

#[cfg(feature = "local-runtime")]
fn drain_many_pty_harness_once(
    harness: &mut ManyPtyLoadHarness,
    last_output_at: u64,
) -> Result<MultiplexerEngineOutcome, EngineConformanceError> {
    let mut combined = MultiplexerEngineOutcome::empty();
    let session_ids = harness
        .sessions
        .iter()
        .filter(|session| !session.complete())
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();

    for session_id in session_ids {
        let output = harness
            .engine
            .drain_runtime_once(&session_id, last_output_at)
            .map_err(EngineConformanceError::from)
            .map_err(|error| {
                many_pty_error(
                    "drain",
                    Some(session_id.clone()),
                    None,
                    format!("post-probe session drain failed: {error}"),
                )
            })?;
        append_outcome(&mut combined, output);
    }

    Ok(combined)
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
#[derive(Debug, Clone)]
struct AdversarialControlSession {
    session_id: SessionId,
    client_id: ClientId,
    subscription_id: SubscriptionId,
}

#[cfg(feature = "local-runtime")]
fn spawn_adversarial_control_session(
    harness: &mut ManyPtyLoadHarness,
) -> Result<AdversarialControlSession, EngineConformanceError> {
    let control = AdversarialControlSession {
        session_id: SessionId("adversarial-control-session".to_string()),
        client_id: ClientId("adversarial-control-client".to_string()),
        subscription_id: SubscriptionId("adversarial-control-subscription".to_string()),
    };
    let request = local_shell_spawn_request(
        RequestId("adversarial-control-spawn".to_string()),
        control.session_id.clone(),
        "printf 'adversarial-control:ready\\n'; IFS= read line; printf 'adversarial-control:input:%s\\n' \"$line\"; sleep 30",
    );

    harness
        .engine
        .execute_command(DefaultEngineCommand::SpawnSession {
            request,
            metadata: CoreSessionMetadata::new(),
        })
        .map_err(EngineConformanceError::from)
        .map_err(|error| wrap_many_pty_error("spawn-control", &control.session_id, None, error))?;
    harness
        .engine
        .execute_command(DefaultEngineCommand::AttachClient {
            client_id: control.client_id.clone(),
            session_id: control.session_id.clone(),
            subscription_id: control.subscription_id.clone(),
            now_seconds: 1,
        })
        .map_err(EngineConformanceError::from)
        .map_err(|error| {
            wrap_many_pty_error(
                "attach-control",
                &control.session_id,
                Some(&control.client_id),
                error,
            )
        })?;

    Ok(control)
}

#[cfg(feature = "local-runtime")]
fn adversarial_probe_window_open(harness: &ManyPtyLoadHarness, noisy_index: usize) -> bool {
    let Some(noisy_session) = harness.sessions.get(noisy_index) else {
        return false;
    };

    noisy_session.ready_seen
        && !noisy_session.done_seen
        && quiet_sessions_completed(harness, noisy_index) > 0
}

#[cfg(feature = "local-runtime")]
fn quiet_sessions_completed(harness: &ManyPtyLoadHarness, noisy_index: usize) -> usize {
    harness
        .sessions
        .iter()
        .enumerate()
        .filter(|(index, session)| *index != noisy_index && session.complete())
        .count()
}

#[cfg(feature = "local-runtime")]
fn quiet_probe_session(
    harness: &ManyPtyLoadHarness,
    noisy_index: usize,
) -> Result<ManyPtySessionState, EngineConformanceError> {
    harness
        .sessions
        .iter()
        .enumerate()
        .find(|(index, session)| *index != noisy_index && session.complete())
        .map(|(_, session)| session.clone())
        .ok_or_else(|| many_pty_error("quiet-session", None, None, "no quiet session completed"))
}

#[cfg(feature = "local-runtime")]
fn run_hot_path_probes(
    harness: &mut ManyPtyLoadHarness,
    config: &AdversarialHotPathConfig,
    noisy_index: usize,
    quiet_session: ManyPtySessionState,
    control: &AdversarialControlSession,
    drain_rounds_before_probes: usize,
) -> Result<AdversarialHotPathReport, EngineConformanceError> {
    let noisy_session = harness.sessions[noisy_index].clone();
    let quiet_sessions_completed_before_probes = quiet_sessions_completed(harness, noisy_index);
    let noisy_output_active_during_probes = noisy_session.ready_seen
        && !noisy_session.done_seen
        && quiet_sessions_completed_before_probes > 0;
    let mut phase_timings = Vec::new();

    let sessions = timed_phase(
        &mut phase_timings,
        "list",
        config.phase_budget,
        || match harness
            .engine
            .execute_command(DefaultEngineCommand::ListSessions)?
        {
            EngineCommandOutcome::Sessions(sessions) => Ok(sessions),
            outcome => Err(many_pty_error(
                "list",
                None,
                None,
                format!("unexpected outcome: {outcome:?}"),
            )),
        },
    )?;
    assert_session_list_contains(&sessions, &quiet_session.session_id, "list")?;
    assert_session_list_contains(&sessions, &noisy_session.session_id, "list")?;
    assert_session_list_contains(&sessions, &control.session_id, "list")?;

    let inspection =
        timed_phase(
            &mut phase_timings,
            "inspect",
            config.phase_budget,
            || match harness
                .engine
                .execute_command(DefaultEngineCommand::InspectSession {
                    session_id: control.session_id.clone(),
                    now_seconds: 20,
                    active_threshold_seconds: 5,
                })? {
                EngineCommandOutcome::Inspection(inspection) => Ok(inspection),
                outcome => Err(many_pty_error(
                    "inspect",
                    Some(control.session_id.clone()),
                    None,
                    format!("unexpected outcome: {outcome:?}"),
                )),
            },
        )?;
    if inspection.session.session_id != control.session_id {
        return Err(many_pty_error(
            "inspect",
            Some(control.session_id.clone()),
            None,
            format!("inspected wrong session: {inspection:?}"),
        ));
    }

    let probe_client = ClientId("adversarial-probe-client".to_string());
    let probe_subscription = SubscriptionId("adversarial-probe-subscription".to_string());
    timed_phase(&mut phase_timings, "attach", config.phase_budget, || {
        harness
            .engine
            .execute_command(DefaultEngineCommand::AttachClient {
                client_id: probe_client.clone(),
                session_id: control.session_id.clone(),
                subscription_id: probe_subscription.clone(),
                now_seconds: 21,
            })
            .map(|_| ())
            .map_err(EngineConformanceError::from)
    })?;
    timed_phase(&mut phase_timings, "detach", config.phase_budget, || {
        harness
            .engine
            .execute_command(DefaultEngineCommand::DetachClient {
                client_id: probe_client.clone(),
                session_id: control.session_id.clone(),
                subscription_id: probe_subscription.clone(),
                now_seconds: 22,
            })
            .map(|_| ())
            .map_err(EngineConformanceError::from)
    })?;

    timed_phase(&mut phase_timings, "resize", config.phase_budget, || {
        let outcome = harness
            .engine
            .execute_command(DefaultEngineCommand::Resize {
                client_id: control.client_id.clone(),
                session_id: control.session_id.clone(),
                rows: 33,
                cols: 120,
                now_seconds: 23,
            })?;
        let output = command_output(&outcome);
        if output.session_requests.iter().any(|(_, request)| {
            matches!(
                request,
                SessionIoRequest::Resize {
                    session_id,
                    rows: 33,
                    cols: 120,
                } if session_id == &control.session_id
            )
        }) {
            Ok(())
        } else {
            Err(many_pty_error(
                "resize",
                Some(control.session_id.clone()),
                Some(control.client_id.clone()),
                format!("resize did not route typed session request: {output:?}"),
            ))
        }
    })?;

    timed_phase(&mut phase_timings, "input", config.input_timeout, || {
        harness
            .engine
            .execute_command(DefaultEngineCommand::SendInput {
                client_id: control.client_id.clone(),
                session_id: control.session_id.clone(),
                data: b"typed-hot-path\n".to_vec(),
                now_seconds: 24,
            })
            .map_err(EngineConformanceError::from)?;
        drain_control_until_input_echo(
            harness,
            control,
            b"adversarial-control:input:typed-hot-path",
            config.input_timeout,
        )
    })?;

    timed_phase(
        &mut phase_timings,
        "read-screen",
        config.phase_budget,
        || {
            let request_id = RequestId("adversarial-read-screen".to_string());
            let outcome = harness
                .engine
                .execute_command(DefaultEngineCommand::ReadScreen {
                    request_id: request_id.clone(),
                    session_id: control.session_id.clone(),
                    now_seconds: 25,
                })?;
            let output = command_output(&outcome);
            if output.session_events.iter().any(|event| matches!(
            event,
            SessionIoEvent::ScreenReady(screen)
                if screen.request_id == request_id && screen.session_id == control.session_id
        )) {
            Ok(())
        } else {
            Err(many_pty_error(
                "read-screen",
                Some(control.session_id.clone()),
                None,
                format!("screen-ready event missing: {output:?}"),
            ))
        }
        },
    )?;

    timed_phase(
        &mut phase_timings,
        "capture-snapshot",
        config.phase_budget,
        || {
            let request_id = RequestId("adversarial-capture-snapshot".to_string());
            let outcome =
                harness
                    .engine
                    .execute_command(DefaultEngineCommand::CaptureSnapshot {
                        request_id: request_id.clone(),
                        session_id: control.session_id.clone(),
                        now_seconds: 26,
                    })?;
            let output = command_output(&outcome);
            if output.session_events.iter().any(|event| matches!(
                event,
                SessionIoEvent::SnapshotReady(snapshot)
                    if snapshot.request_id == request_id && snapshot.session_id == control.session_id
            )) {
                Ok(())
            } else {
                Err(many_pty_error(
                    "capture-snapshot",
                    Some(control.session_id.clone()),
                    None,
                    format!("snapshot-ready event missing: {output:?}"),
                ))
            }
        },
    )?;

    timed_phase(
        &mut phase_timings,
        "shutdown-control",
        config.phase_budget,
        || {
            harness
                .engine
                .execute_command(DefaultEngineCommand::Shutdown {
                    session_id: control.session_id.clone(),
                    reason: "adversarial hot-path control complete".to_string(),
                    now_seconds: 27,
                })
                .map(|_| ())
                .map_err(EngineConformanceError::from)
        },
    )?;

    let hot_path_budget_observation = hot_path_budget_observation(
        phase_timings.len(),
        drain_rounds_before_probes,
        drain_rounds_before_probes,
        config.phase_budget,
    );

    Ok(AdversarialHotPathReport {
        session_count: config.load.session_count,
        noisy_session_id: noisy_session.session_id,
        quiet_session_id: quiet_session.session_id,
        control_session_id: control.session_id.clone(),
        quiet_sessions_completed_before_probes,
        drain_rounds_before_probes,
        total_drain_rounds: drain_rounds_before_probes,
        noisy_output_active_during_probes,
        phase_timings,
        hot_path_budget_observation,
        queue_backpressure_observations: Vec::new(),
        cleanup_exited_sessions: 0,
        live_sessions_after_cleanup: Vec::new(),
        slow_client_plugin_observation:
            "DefaultBotsterEngine hot-path proof composes with focused subscription_multiplexer_engine_test and plugin_worker_engine_test for slow-client/plugin isolation; no combined slow-client/plugin counter is exposed by the public default engine."
                .to_string(),
    })
}

#[cfg(feature = "local-runtime")]
const EXPECTED_HOT_PATH_PHASES: &[&str] = &[
    "list",
    "inspect",
    "attach",
    "detach",
    "resize",
    "input",
    "read-screen",
    "capture-snapshot",
    "shutdown-control",
];

#[cfg(feature = "local-runtime")]
fn hot_path_budget_observation(
    phase_count: usize,
    drain_rounds_before_probes: usize,
    total_drain_rounds: usize,
    phase_budget: Duration,
) -> String {
    format!(
        "phase_count={} expected_phases={} fair_drain_rounds_before_probes={} total_drain_rounds={} per_phase_regression_budget={:?}",
        phase_count,
        EXPECTED_HOT_PATH_PHASES.len(),
        drain_rounds_before_probes,
        total_drain_rounds,
        phase_budget
    )
}

#[cfg(feature = "local-runtime")]
fn timed_phase<T>(
    phase_timings: &mut Vec<HotPathPhaseTiming>,
    phase: &'static str,
    budget: Duration,
    operation: impl FnOnce() -> Result<T, EngineConformanceError>,
) -> Result<T, EngineConformanceError> {
    let started = Instant::now();
    let result = operation();
    let elapsed = started.elapsed();
    phase_timings.push(HotPathPhaseTiming { phase, elapsed });
    if elapsed > budget {
        return Err(many_pty_error(
            phase,
            None,
            None,
            format!("phase exceeded budget {budget:?}; elapsed {elapsed:?}"),
        ));
    }
    result
}

#[cfg(feature = "local-runtime")]
fn assert_session_list_contains(
    sessions: &[CoreSession],
    session_id: &SessionId,
    phase: &'static str,
) -> Result<(), EngineConformanceError> {
    if sessions
        .iter()
        .any(|session| &session.session_id == session_id)
    {
        Ok(())
    } else {
        Err(many_pty_error(
            phase,
            Some(session_id.clone()),
            None,
            format!("listed sessions did not include session; got {sessions:?}"),
        ))
    }
}

#[cfg(feature = "local-runtime")]
fn drain_control_until_input_echo(
    harness: &mut ManyPtyLoadHarness,
    control: &AdversarialControlSession,
    expected: &[u8],
    timeout: Duration,
) -> Result<(), EngineConformanceError> {
    let deadline = Instant::now() + timeout;
    let mut combined = MultiplexerEngineOutcome::empty();

    while Instant::now() < deadline {
        let output = harness
            .engine
            .drain_runtime_once(&control.session_id, 24)
            .map_err(EngineConformanceError::from)?;
        append_outcome(&mut combined, output);
        let delivered = terminal_output_bytes_for(
            &combined,
            &control.client_id,
            &control.subscription_id,
            &control.session_id,
        );
        if delivered
            .windows(expected.len())
            .any(|window| window == expected)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Err(EngineConformanceError::Timeout {
        observed_output: String::from_utf8_lossy(&terminal_output_bytes_for(
            &combined,
            &control.client_id,
            &control.subscription_id,
            &control.session_id,
        ))
        .into_owned(),
    })
}

#[cfg(feature = "local-runtime")]
fn cleanup_adversarial_sessions(
    harness: &mut ManyPtyLoadHarness,
    control: &AdversarialControlSession,
    report: &mut AdversarialHotPathReport,
    deadline: Instant,
) -> Result<(), EngineConformanceError> {
    harness.shutdown_all(30_000);
    let _ = harness
        .engine
        .execute_command(DefaultEngineCommand::Shutdown {
            session_id: control.session_id.clone(),
            reason: "adversarial hot-path cleanup".to_string(),
            now_seconds: 30_000,
        });

    let mut control_cleaned_up = session_record_exited(&harness.engine, &control.session_id);
    while Instant::now() < deadline {
        if control_cleaned_up {
            break;
        }
        control_cleaned_up = drain_control_cleanup_once(harness, control, report)?;
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut live = harness
        .sessions
        .iter()
        .filter(|session| !session.exit_seen)
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    if !control_cleaned_up {
        live.push(control.session_id.clone());
    }

    report.cleanup_exited_sessions = harness
        .sessions
        .iter()
        .filter(|session| session.exit_seen)
        .count()
        + usize::from(control_cleaned_up);
    report.live_sessions_after_cleanup = live;

    if report.live_sessions_after_cleanup.is_empty() {
        Ok(())
    } else {
        Err(many_pty_error(
            "cleanup",
            None,
            None,
            format!(
                "sessions remained live after cleanup: {:?}",
                report.live_sessions_after_cleanup
            ),
        ))
    }
}

#[cfg(feature = "local-runtime")]
fn drain_control_cleanup_once(
    harness: &mut ManyPtyLoadHarness,
    control: &AdversarialControlSession,
    report: &mut AdversarialHotPathReport,
) -> Result<bool, EngineConformanceError> {
    report.total_drain_rounds += 1;
    match harness.engine.drain_runtime_once(
        &control.session_id,
        30_000 + report.total_drain_rounds as u64,
    ) {
        Ok(_) => Ok(session_record_exited(&harness.engine, &control.session_id)),
        Err(ManagedSessionRuntimeError::Runtime(error))
            if error.kind == SessionRuntimeErrorKind::SessionNotFound =>
        {
            Ok(true)
        }
        Err(error) => Err(EngineConformanceError::from(error)),
    }
}

#[cfg(feature = "local-runtime")]
fn session_record_exited(engine: &DefaultBotsterEngine, session_id: &SessionId) -> bool {
    matches!(
        engine.session(session_id).map(|session| &session.lifecycle),
        Some(SessionLifecycleState::Exited { .. })
    )
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
    let mut queue_backpressure_observations = Vec::new();

    while Instant::now() < deadline {
        let mut made_progress = false;
        drain_rounds += 1;

        let output = harness
            .engine
            .drain_runtime_all_once(drain_rounds as u64 + 1)
            .map_err(EngineConformanceError::from)
            .map_err(|error| {
                many_pty_error(
                    "drain",
                    None,
                    None,
                    format!("fair aggregate drain failed: {error}"),
                )
            })?;

        if !output.client_egress.is_empty() || !output.session_events.is_empty() {
            made_progress = true;
        }
        queue_backpressure_observations.extend(queue_backpressure_observations_for(&output));

        for session in &mut harness.sessions {
            if session.complete() {
                continue;
            }
            let delivered = terminal_output_bytes_for(
                &output,
                &session.client_id,
                &session.subscription_id,
                &session.session_id,
            );
            total_output_bytes += delivered.len();
            session.output.extend(delivered);
            update_many_pty_completion(session, &output);
        }

        if many_pty_completion_satisfied(harness, config, &queue_backpressure_observations) {
            return Ok(many_pty_report(
                harness,
                config,
                started.elapsed(),
                drain_rounds,
                total_output_bytes,
                queue_backpressure_observations,
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
fn many_pty_completion_satisfied(
    harness: &ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
    queue_backpressure_observations: &[String],
) -> bool {
    if harness.sessions.iter().all(ManyPtySessionState::complete) {
        return true;
    }

    let Some(noisy_session_index) = config.noisy_session_index else {
        return false;
    };

    let Some(noisy_session) = harness.sessions.get(noisy_session_index) else {
        return false;
    };

    !queue_backpressure_observations.is_empty()
        && noisy_session.ready_seen
        && noisy_session.exit_seen
        && harness
            .sessions
            .iter()
            .enumerate()
            .all(|(index, session)| index == noisy_session_index || session.complete())
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
fn queue_backpressure_observations_for(output: &MultiplexerEngineOutcome) -> Vec<String> {
    output
        .observations
        .iter()
        .filter_map(|observation| match observation {
            MultiplexerEngineObservation::Backpressure(summary) => Some(format!(
                "source={} capacity={} depth={} session_id={}",
                summary.source.name(),
                summary.capacity,
                summary.depth,
                summary
                    .route
                    .session_id
                    .as_ref()
                    .map_or("none", |session_id| session_id.0.as_str())
            )),
            _ => None,
        })
        .collect()
}

#[cfg(feature = "local-runtime")]
fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(&value) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(feature = "local-runtime")]
fn many_pty_report(
    harness: &ManyPtyLoadHarness,
    config: &ManyPtyLoadConfig,
    elapsed: Duration,
    drain_rounds: usize,
    total_output_bytes: usize,
    queue_backpressure_observations: Vec<String>,
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

    let mut queue_backpressure_observations =
        dedupe_preserving_order(queue_backpressure_observations);
    if queue_backpressure_observations.is_empty() {
        queue_backpressure_observations.push(
            "DefaultBotsterEngine exposes typed reader backpressure observations; no reader pressure was observed in this run."
                .to_string(),
        );
    }

    ManyPtyLoadReport {
        session_count: config.session_count,
        elapsed,
        drain_rounds,
        total_output_bytes,
        outputs_completed,
        exits_observed,
        noisy_session_id,
        queue_backpressure_observations,
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

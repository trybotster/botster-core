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
    /// The expected runtime output did not arrive before the deadline.
    Timeout {
        /// Bytes observed before timing out.
        observed_output: String,
    },
}

impl fmt::Display for EngineConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skipped(reason) => write!(formatter, "{}", reason.reason),
            Self::ManagedRuntime(error) => write!(formatter, "{error}"),
            Self::Timeout { observed_output } => {
                write!(
                    formatter,
                    "timed out waiting for output; observed {observed_output:?}"
                )
            }
        }
    }
}

impl Error for EngineConformanceError {}

impl From<ManagedSessionRuntimeError> for EngineConformanceError {
    fn from(error: ManagedSessionRuntimeError) -> Self {
        Self::ManagedRuntime(error)
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

//! Dev-only smoke harnesses for `botster-core`.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use botster_core::{
    admit_host_profile, BotsterEngineObservation, CoreSessionMetadata, DefaultBotsterEngine,
    DefaultEngineCommand, EngineCommand, EngineCommandOutcome, HostProfilePolicySection,
    LocalProcessRuntime, LocalProcessWorkerRuntime, PackageSource, PluginDescriptorKind,
    PluginDescriptorRef, PluginOwnedDescriptor, PluginWorkerEngineConfig, ResizePayload,
    SessionIoEvent, SessionLifecycleState, SessionSpawnRequest, SpawnEnvironment,
    SpawnWorkingDirectory, SubscriptionId, TransportEgress,
};
use botster_core::{
    BotsterEngine, BoundaryJson, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime,
    PackageManifest, PluginAdmissionResult, PluginCompletion, PluginHandlerKind, PluginHandlerRef,
    PluginHandlerRegistration, PluginInvocationClass, PluginInvocationContext,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult, PluginKey,
    PluginLoadSpec, PluginWorkerRegistration, RequestId,
};
use botster_core::{Capability, CapabilitySurface, ClientId, SessionActivityStatus, SessionId};
use botster_core_test_support::fake::{
    FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
};

/// Deterministic report emitted by the dev-only real embedder smoke harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSmokeReport {
    /// Whether this host ran the real local PTY example.
    pub ran_real_embedder: bool,
    /// Host profile admitted before entering core runtime/plugin paths.
    pub admitted_host_profile_id: String,
    /// Capability declared by the admitted host profile and reused by plugin assertions.
    pub admitted_required_capability: Capability,
    /// Whether the exact admitted capability was used as the plugin handler requirement.
    pub admitted_capability_drove_plugin_handler: bool,
    /// Public engine surface selected for the no-hub proof.
    pub engine_surface: String,
    /// Whether one generic engine performed real session and plugin work.
    pub single_engine_session_and_plugin: bool,
    /// Whether an ordinary plugin with the admitted capability completed.
    pub plugin_invocation_completed: bool,
    /// Payload value returned by the allowed ordinary plugin.
    pub plugin_invocation_value: String,
    /// Whether the missing-capability plugin was rejected before its runtime was called.
    pub plugin_missing_capability_rejected: bool,
    /// Typed failure kind from the denied plugin invocation.
    pub plugin_denial_failure_kind: String,
    /// Whether the denied plugin runtime observed zero invocations.
    pub denied_plugin_runtime_not_called: bool,
    /// Queue capacity passed through the public engine facade.
    pub plugin_queue_capacity: usize,
    /// Executor concurrency passed through the public engine facade.
    pub plugin_executor_concurrency: usize,
    /// Loaded plugin executors reported by the public debug snapshot.
    pub live_plugin_executors: usize,
    /// Live workers reported by the public debug snapshot.
    pub live_plugin_executor_workers: usize,
    /// Requirements this dev proof shows a custom host must provide before entering core.
    pub custom_host_requirements: Vec<String>,
    /// Session spawned through the public generic local engine.
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
    /// Whether the selected engine returned typed shutdown evidence.
    pub shutdown_observed: bool,
}

impl EngineSmokeReport {
    /// Render deterministic, scrubbed lines for the dev executable.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        vec![
            "botster-core real embedder smoke".to_string(),
            format!("real embedder ran: {}", self.ran_real_embedder),
            format!("admitted host profile: {}", self.admitted_host_profile_id),
            format!(
                "admitted required capability: {:?}",
                self.admitted_required_capability
            ),
            format!(
                "admitted capability drove plugin handler: {}",
                self.admitted_capability_drove_plugin_handler
            ),
            format!("engine surface: {}", self.engine_surface),
            format!(
                "single engine session and plugin: {}",
                self.single_engine_session_and_plugin
            ),
            format!(
                "plugin invocation completed: {}",
                self.plugin_invocation_completed
            ),
            format!("plugin invocation value: {}", self.plugin_invocation_value),
            format!(
                "plugin missing capability rejected: {}",
                self.plugin_missing_capability_rejected
            ),
            format!(
                "plugin denial failure kind: {}",
                self.plugin_denial_failure_kind
            ),
            format!(
                "denied plugin runtime not called: {}",
                self.denied_plugin_runtime_not_called
            ),
            format!("plugin queue capacity: {}", self.plugin_queue_capacity),
            format!(
                "plugin executor concurrency: {}",
                self.plugin_executor_concurrency
            ),
            format!("live plugin executors: {}", self.live_plugin_executors),
            format!(
                "live plugin executor workers: {}",
                self.live_plugin_executor_workers
            ),
            format!(
                "custom host requirements: {}",
                self.custom_host_requirements.join(", ")
            ),
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

/// Separate-crate proof that a public facade consumer can admit Background
/// work, drain a typed completion, and read live class snapshot fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAdmissionProof {
    /// Whether `try_admit_plugin` accepted the Background request.
    pub admitted: bool,
    /// Whether the drained completion was a typed timeout.
    pub timed_out: bool,
    /// Whether the completion recorded the Background admission class.
    pub class_is_background: bool,
    /// Reserved RequestResponse executor slots from the live snapshot.
    pub reserved_executors: usize,
    /// Whether the live snapshot reported class-specific fields.
    pub live_class_fields_present: bool,
}

/// Admit Background work through `BotsterEngine` and drain one typed completion.
pub fn run_plugin_admission_proof() -> Result<PluginAdmissionProof, EngineSmokeError> {
    let engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
        BotsterEngine::new(FakeSessionRuntime::new());
    let plugin_key = PluginKey("core-dev-admission".to_string());
    let handler = PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "slow".to_string(),
    };
    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: plugin_key.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: PackageManifest {
            name: plugin_key.0.clone(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::Plugin,
            botster: ">=0.1.0".to_string(),
            source: None,
            capabilities: Vec::new(),
            entrypoints: vec![ExtensionEntrypoint {
                runtime: ExtensionRuntime::Lua,
                path: "plugin.lua".to_string(),
                bootstrap: false,
            }],
            dependencies: Vec::new(),
            features: Vec::new(),
            host_profile: None,
            configuration: None,
            runnable_entrypoints: Vec::new(),
        },
        runtime: Arc::new(FakePluginRuntime::delayed(Duration::from_millis(200))),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    });

    let request = PluginInvocationRequest {
        request_id: RequestId("core-dev-timeout".to_string()),
        handler,
        timeout_ms: 10,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("core-dev".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "op": "slow" })),
    };
    let admit_started = Instant::now();
    let admitted = loop {
        match engine.try_admit_plugin(PluginInvocationClass::Background, request.clone()) {
            PluginAdmissionResult::Queued { .. } => break true,
            PluginAdmissionResult::Backpressured { reason, .. }
                if reason == "admission lock busy"
                    && admit_started.elapsed() < Duration::from_millis(100) =>
            {
                std::thread::yield_now();
            }
            _ => break false,
        }
    };

    let started = Instant::now();
    let mut completion = None;
    while started.elapsed() < Duration::from_secs(1) {
        let drain = engine.drain_plugin_completions(8, usize::MAX);
        if let Some(item) = drain.completions.into_iter().next() {
            completion = Some(item);
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let snapshot = engine.plugin_workers().debug_snapshot();
    let completion = completion.ok_or_else(|| {
        EngineSmokeError::new("did not drain a Background completion through the public facade")
    })?;

    Ok(PluginAdmissionProof {
        admitted,
        timed_out: matches!(
            &completion,
            PluginCompletion {
                result: PluginInvocationResult::Failed(failure),
                ..
            } if failure.kind == PluginInvocationFailureKind::TimedOut
        ),
        class_is_background: matches!(completion.class, PluginInvocationClass::Background),
        reserved_executors: snapshot.configured_reserved_request_response_executors,
        live_class_fields_present: snapshot.configured_background_queue_capacity > 0
            && snapshot.plugins.iter().any(|plugin| {
                plugin.reserved_request_response_executors
                    == snapshot.configured_reserved_request_response_executors
            }),
    })
}

#[cfg(unix)]
fn run_real_embedder_smoke() -> Result<EngineSmokeReport, EngineSmokeError> {
    let admitted =
        admit_host_profile(&trusted_host_profile_manifest(), true, "0.1.0").map_err(|error| {
            EngineSmokeError::new(format!("host profile admission failed: {error}"))
        })?;
    let admitted_capability = admitted
        .metadata
        .required_capabilities
        .first()
        .cloned()
        .ok_or_else(|| {
            EngineSmokeError::new("admitted host profile did not declare a required capability")
        })?;

    let mut session_engine = DefaultBotsterEngine::new();
    let request = real_embedder_spawn_request();
    let session_id = request.session_id.clone();
    let client_id = ClientId("real-embedder-client".to_string());
    let subscription_id = SubscriptionId("real-embedder-subscription".to_string());
    let mut logical_clock = 20;

    let spawn = session_engine
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
        &mut session_engine,
        request.clone(),
        session_id.clone(),
        client_id,
        subscription_id,
        &mut logical_clock,
    );
    if let Err(error) = smoke_result {
        let _ = shutdown_session(&mut session_engine, &session_id, logical_clock);
        return Err(error);
    }

    let mut report = smoke_result.expect("error branch returned above");
    let mut plugin_engine: BotsterEngine<LocalProcessRuntime, LocalProcessWorkerRuntime> =
        BotsterEngine::with_plugin_config(
            LocalProcessRuntime::new(),
            PluginWorkerEngineConfig {
                per_plugin_queue_capacity: 32,
                per_plugin_executor_concurrency: 2,
                ..PluginWorkerEngineConfig::default()
            },
        );
    let plugin_proof = run_plugin_proof(&mut plugin_engine, &admitted_capability)?;
    let shutdown = shutdown_session(&mut session_engine, &session_id, logical_clock)?;
    report.shutdown_observed = shutdown.session_events.iter().any(|event| {
        matches!(
            event,
            SessionIoEvent::Shutdown {
                session_id: shutdown_session_id,
                ..
            } if shutdown_session_id == &session_id
        )
    }) || shutdown.observations.iter().any(|observation| {
        observation
            == &BotsterEngineObservation::SessionLifecycle {
                session_id: session_id.clone(),
                state: SessionLifecycleState::Stopping,
            }
    });
    report.admitted_host_profile_id = admitted.metadata.profile_id;
    report.admitted_required_capability = admitted_capability;
    report.admitted_capability_drove_plugin_handler =
        plugin_proof.handler_required_admitted_capability;
    report.engine_surface =
        "DefaultBotsterEngine session runtime + BotsterEngine plugin facade".to_string();
    report.single_engine_session_and_plugin = false;
    report.plugin_invocation_completed = plugin_proof.allowed_completed;
    report.plugin_invocation_value = plugin_proof.allowed_value;
    report.plugin_missing_capability_rejected = plugin_proof.denied_rejected;
    report.plugin_denial_failure_kind = plugin_proof.denial_failure_kind;
    report.denied_plugin_runtime_not_called = plugin_proof.denied_runtime_not_called;
    report.plugin_queue_capacity = plugin_proof.queue_capacity;
    report.plugin_executor_concurrency = plugin_proof.executor_concurrency;
    report.live_plugin_executors = plugin_proof.live_executors;
    report.live_plugin_executor_workers = plugin_proof.live_executor_workers;
    report.custom_host_requirements = custom_host_requirements();

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
        admitted_host_profile_id: String::new(),
        admitted_required_capability: host_profile_capability(),
        admitted_capability_drove_plugin_handler: false,
        engine_surface: String::new(),
        single_engine_session_and_plugin: false,
        plugin_invocation_completed: false,
        plugin_invocation_value: String::new(),
        plugin_missing_capability_rejected: false,
        plugin_denial_failure_kind: String::new(),
        denied_plugin_runtime_not_called: false,
        plugin_queue_capacity: 0,
        plugin_executor_concurrency: 0,
        live_plugin_executors: 0,
        live_plugin_executor_workers: 0,
        custom_host_requirements: Vec::new(),
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
        admitted_host_profile_id: "minimal-test-host".to_string(),
        admitted_required_capability: host_profile_capability(),
        admitted_capability_drove_plugin_handler: false,
        engine_surface: "skipped: local PTY example requires Unix".to_string(),
        single_engine_session_and_plugin: false,
        plugin_invocation_completed: false,
        plugin_invocation_value: String::new(),
        plugin_missing_capability_rejected: false,
        plugin_denial_failure_kind: String::new(),
        denied_plugin_runtime_not_called: false,
        plugin_queue_capacity: 0,
        plugin_executor_concurrency: 0,
        live_plugin_executors: 0,
        live_plugin_executor_workers: 0,
        custom_host_requirements: custom_host_requirements(),
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginProof {
    handler_required_admitted_capability: bool,
    allowed_completed: bool,
    allowed_value: String,
    denied_rejected: bool,
    denial_failure_kind: String,
    denied_runtime_not_called: bool,
    queue_capacity: usize,
    executor_concurrency: usize,
    live_executors: usize,
    live_executor_workers: usize,
}

#[cfg(unix)]
fn run_plugin_proof(
    engine: &mut BotsterEngine<LocalProcessRuntime, LocalProcessWorkerRuntime>,
    admitted_capability: &Capability,
) -> Result<PluginProof, EngineSmokeError> {
    let allowed_plugin = PluginKey("minimal-host-ordinary-plugin".to_string());
    let denied_plugin = PluginKey("minimal-host-denied-plugin".to_string());
    let allowed_handler = plugin_handler(&allowed_plugin);
    let denied_handler = plugin_handler(&denied_plugin);
    let allowed_runtime = FakePluginRuntime::success("allowed");
    let denied_runtime = FakePluginRuntime::success("denied-runtime-should-not-run");
    let allowed_required_capability = Some(admitted_capability.clone());
    let denied_required_capability = Some(admitted_capability.clone());
    let handler_required_admitted_capability = allowed_required_capability.as_ref()
        == Some(admitted_capability)
        && denied_required_capability.as_ref() == Some(admitted_capability);

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration(
                allowed_runtime,
                &allowed_plugin,
                &allowed_handler,
                vec![admitted_capability.clone()],
                allowed_required_capability,
            ),
        })
        .map_err(|error| EngineSmokeError::new(format!("load allowed plugin failed: {error}")))?;

    engine
        .execute_command(EngineCommand::LoadPlugin {
            registration: plugin_registration(
                denied_runtime.clone(),
                &denied_plugin,
                &denied_handler,
                Vec::new(),
                denied_required_capability,
            ),
        })
        .map_err(|error| EngineSmokeError::new(format!("load denied plugin failed: {error}")))?;

    let allowed = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation("allowed-plugin-request", allowed_handler),
        })
        .map_err(|error| {
            EngineSmokeError::new(format!(
                "invoke allowed plugin failed unexpectedly: {error}"
            ))
        })?;
    let EngineCommandOutcome::PluginInvoked(allowed) = allowed else {
        return Err(EngineSmokeError::new(
            "allowed plugin invocation returned wrong result",
        ));
    };
    let (allowed_completed, allowed_value) = match allowed.result {
        PluginInvocationResult::Completed(success) => {
            let value = success
                .payload
                .and_then(|payload| {
                    payload
                        .0
                        .get("value")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_default();
            (true, value)
        }
        _ => (false, String::new()),
    };

    let denied = engine
        .execute_command(EngineCommand::InvokePlugin {
            request: plugin_invocation("denied-plugin-request", denied_handler),
        })
        .map_err(|error| {
            EngineSmokeError::new(format!("invoke denied plugin failed unexpectedly: {error}"))
        })?;
    let EngineCommandOutcome::PluginInvoked(denied) = denied else {
        return Err(EngineSmokeError::new(
            "denied plugin invocation returned wrong result",
        ));
    };
    let (denied_rejected, denial_failure_kind) = match denied.result {
        PluginInvocationResult::Failed(failure) => (
            failure.kind == PluginInvocationFailureKind::HandlerFailed
                && failure.reason.contains("capability"),
            format!("{:?}", failure.kind),
        ),
        _ => (false, String::new()),
    };

    let debug = engine.plugin_workers().debug_snapshot();
    Ok(PluginProof {
        handler_required_admitted_capability,
        allowed_completed,
        allowed_value,
        denied_rejected,
        denial_failure_kind,
        denied_runtime_not_called: denied_runtime.invocations().is_empty(),
        queue_capacity: debug.configured_queue_capacity,
        executor_concurrency: debug.configured_executor_concurrency,
        live_executors: debug.live_plugin_executors,
        live_executor_workers: debug.live_executor_workers,
    })
}

#[cfg(unix)]
fn trusted_host_profile_manifest() -> PackageManifest {
    PackageManifest {
        name: "minimal-test-host-provider".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Provider,
        botster: ">=0.1.0".to_string(),
        source: Some(PackageSource::Git {
            repo: "https://example.invalid/botster/minimal-test-host".to_string(),
            reference: "v0.1.0".to_string(),
        }),
        capabilities: vec![host_profile_capability()],
        entrypoints: vec![ExtensionEntrypoint {
            runtime: ExtensionRuntime::Lua,
            path: "bootstrap.lua".to_string(),
            bootstrap: true,
        }],
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: Some(botster_core::HostProfileMetadata {
            profile_id: "minimal-test-host".to_string(),
            compatibility: ">=0.1.0".to_string(),
            precedence: 10,
            required_providers: vec!["local-process-provider".to_string()],
            required_capabilities: vec![host_profile_capability()],
            policy_sections: vec![
                HostProfilePolicySection::Startup,
                HostProfilePolicySection::Providers,
                HostProfilePolicySection::Capabilities,
                HostProfilePolicySection::ClientAdmission,
            ],
        }),
        configuration: None,
        runnable_entrypoints: Vec::new(),
    }
}

fn host_profile_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Mcp,
        scope: Some("minimal-test-host.run".to_string()),
    }
}

fn custom_host_requirements() -> Vec<String> {
    vec![
        "host Botster version".to_string(),
        "enablement decision".to_string(),
        "source provenance".to_string(),
        "bootstrap entrypoint".to_string(),
        "required provider names".to_string(),
        "required capabilities".to_string(),
        "explicit spawn request fields".to_string(),
        "client and subscription ids".to_string(),
        "logical clocks".to_string(),
        "plugin worker registration and runtime".to_string(),
    ]
}

#[cfg(unix)]
fn plugin_registration(
    runtime: FakePluginRuntime,
    plugin_key: &PluginKey,
    handler: &PluginHandlerRef,
    capabilities: Vec<Capability>,
    required_capability: Option<Capability>,
) -> PluginWorkerRegistration {
    PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: plugin_key.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![PluginOwnedDescriptor {
                descriptor: PluginDescriptorRef {
                    plugin_key: plugin_key.clone(),
                    kind: PluginDescriptorKind::Command,
                    descriptor_id: "run".to_string(),
                },
                handler: Some(handler.clone()),
                body: BoundaryJson(serde_json::json!({ "title": "Run" })),
            }],
            metadata: None,
        },
        manifest: plugin_manifest(plugin_key, capabilities),
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability,
        }],
        resources: Vec::new(),
    }
}

#[cfg(unix)]
fn plugin_manifest(plugin_key: &PluginKey, capabilities: Vec<Capability>) -> PackageManifest {
    PackageManifest {
        name: plugin_key.0.clone(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities,
        entrypoints: vec![ExtensionEntrypoint {
            runtime: ExtensionRuntime::Lua,
            path: "plugin.lua".to_string(),
            bootstrap: false,
        }],
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: None,
        configuration: None,
        runnable_entrypoints: Vec::new(),
    }
}

#[cfg(unix)]
fn plugin_handler(plugin_key: &PluginKey) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "run".to_string(),
    }
}

#[cfg(unix)]
fn plugin_invocation(request: &str, handler: PluginHandlerRef) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(format!("real-embedder-{request}")),
        handler,
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: Some(ClientId("real-embedder-client".to_string())),
            session_id: Some(SessionId("real-embedder-session".to_string())),
            subscription_id: Some(SubscriptionId("real-embedder-subscription".to_string())),
            surface_id: None,
            origin: Some("minimal-test-host".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "command": "run" })),
    }
}

#[cfg(unix)]
fn request_id(value: &str) -> RequestId {
    RequestId(format!("real-embedder-{value}"))
}

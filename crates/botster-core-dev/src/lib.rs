//! Dev-only smoke harnesses for `botster-core`.

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use botster_core::{
    BoundaryJson, ClientId, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime,
    NotificationContent, NotificationId, NotificationInbox, NotificationItem, NotificationPayload,
    NotificationSeverity, NotificationSource, NotificationTarget, NotificationTimestamp,
    PackageManifest, PluginHandlerKind, PluginHandlerRef, PluginHandlerRegistration,
    PluginInvocationContext, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginKey,
    PluginLoadSpec, PluginRuntime, PluginWorkerEngine, PluginWorkerRegistration, RequestId,
    SessionId, SessionIoEvent, SessionIoRequest, SessionRuntime, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory,
    SubscriptionId, SubscriptionMultiplexer, TransportEgress, TransportIngress,
};
use botster_core_test_support::fake::FakeSessionRuntime;

/// Deterministic report emitted by the dev-only engine smoke harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSmokeReport {
    /// Session spawned through the host runtime boundary.
    pub spawned_session_id: SessionId,
    /// Client attached through the subscription multiplexer.
    pub attached_client_id: ClientId,
    /// Terminal input routed from the client path to the session runtime.
    pub terminal_input: String,
    /// Terminal output delivered back to the subscribed client.
    pub client_output: String,
    /// Notification titles drained from the typed notification inbox.
    pub notifications: Vec<String>,
    /// Whether the typed session notification path stayed out of client egress.
    pub session_notification_routed: bool,
    /// Summary returned by the fake plugin handler through `PluginWorkerEngine`.
    pub plugin_result: String,
    /// Whether shutdown was requested through the runtime boundary.
    pub shutdown_requested: bool,
}

impl EngineSmokeReport {
    /// Render deterministic, scrubbed lines for the dev executable.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        vec![
            "botster-core dev engine smoke".to_string(),
            format!("session spawned: {}", self.spawned_session_id.0),
            format!("client attached: {}", self.attached_client_id.0),
            format!("terminal input routed: {:?}", self.terminal_input),
            format!("client observed output: {:?}", self.client_output),
            format!("notifications drained: {}", self.notifications.join(", ")),
            format!(
                "session notification routed: {}",
                self.session_notification_routed
            ),
            format!("plugin result: {}", self.plugin_result),
            format!("shutdown requested: {}", self.shutdown_requested),
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

/// Run the dev-only engine smoke scenario used by both the binary and tests.
pub fn run_engine_smoke() -> Result<EngineSmokeReport, EngineSmokeError> {
    let session_id = SessionId("engine-smoke-session".to_string());
    let client_id = ClientId("engine-smoke-client".to_string());
    let subscription_id = SubscriptionId("engine-smoke-subscription".to_string());

    let mut runtime = FakeSessionRuntime::new();
    runtime
        .spawn_session(SessionSpawnRequest {
            request_id: request_id("spawn"),
            session_id: session_id.clone(),
            executable: "fake-session".to_string(),
            arguments: vec!["--engine-smoke".to_string()],
            working_directory: SpawnWorkingDirectory {
                path: "dev-harness-workspace".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: None,
        })
        .map_err(|error| EngineSmokeError::new(format!("spawn failed: {error}")))?;

    let mut multiplexer = SubscriptionMultiplexer::new();
    let subscribe = multiplexer.handle_client_ingress(
        client_id.clone(),
        TransportIngress::SubscribeSession {
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        },
    );
    if subscribe.observations.is_empty() {
        return Err(EngineSmokeError::new(
            "client subscription was not observed",
        ));
    }

    let input_bytes = b"echo engine-smoke\n".to_vec();
    let input = multiplexer.handle_client_ingress(
        client_id.clone(),
        TransportIngress::TerminalInput {
            session_id: session_id.clone(),
            data: input_bytes.clone(),
        },
    );
    let routed_input = input
        .session_requests
        .iter()
        .find_map(|(_, request)| match request {
            SessionIoRequest::PtyInput { session_id, data } => Some((session_id, data)),
            _ => None,
        })
        .ok_or_else(|| {
            EngineSmokeError::new("terminal input did not reach session request path")
        })?;
    runtime
        .send_input(SessionRuntimeInput::PtyInput {
            session_id: routed_input.0.clone(),
            data: routed_input.1.clone(),
        })
        .map_err(|error| EngineSmokeError::new(format!("input failed: {error}")))?;

    let output_bytes = b"engine-smoke-output\n".to_vec();
    runtime.emit_output(session_id.clone(), output_bytes.clone());
    let mut client_output = String::new();
    for event in drain_runtime_events(&mut runtime, &session_id)? {
        let output = multiplexer.handle_session_event(event);
        client_output.push_str(&terminal_output_text(&output.client_egress)?);
    }

    let notifications = post_and_drain_notification(&session_id);
    let session_notification = multiplexer.handle_session_event(SessionIoEvent::Notification {
        session_id: session_id.clone(),
        payload: NotificationPayload {
            title: "Engine smoke notice".to_string(),
            body: "typed session notification routed".to_string(),
        },
    });
    let session_notification_routed = session_notification.client_egress.is_empty()
        && !session_notification.observations.is_empty();

    let plugin_result = invoke_fake_plugin()?;

    runtime
        .send_input(SessionRuntimeInput::Shutdown {
            session_id: session_id.clone(),
        })
        .map_err(|error| EngineSmokeError::new(format!("shutdown failed: {error}")))?;
    let shutdown_requested = runtime
        .inputs()
        .iter()
        .any(|input| matches!(input, SessionRuntimeInput::Shutdown { session_id: id } if id == &session_id));

    Ok(EngineSmokeReport {
        spawned_session_id: session_id,
        attached_client_id: client_id,
        terminal_input: String::from_utf8(input_bytes)
            .map_err(|error| EngineSmokeError::new(format!("input was not utf-8: {error}")))?,
        client_output,
        notifications,
        session_notification_routed,
        plugin_result,
        shutdown_requested,
    })
}

fn drain_runtime_events(
    runtime: &mut FakeSessionRuntime,
    session_id: &SessionId,
) -> Result<Vec<SessionIoEvent>, EngineSmokeError> {
    runtime
        .drain_output(session_id)
        .map_err(|error| EngineSmokeError::new(format!("output drain failed: {error}")))?
        .into_iter()
        .map(|output| match output {
            SessionRuntimeOutput::PtyOutput { session_id, data } => {
                Ok(SessionIoEvent::TerminalBytes { session_id, data })
            }
            SessionRuntimeOutput::ProcessExited {
                session_id,
                payload,
            } => Ok(SessionIoEvent::ProcessExited {
                session_id,
                payload,
            }),
        })
        .collect()
}

fn terminal_output_text(
    egress: &[(ClientId, TransportEgress)],
) -> Result<String, EngineSmokeError> {
    let mut output = String::new();
    for (_, frame) in egress {
        if let TransportEgress::TerminalOutput { data, .. } = frame {
            output.push_str(&String::from_utf8(data.clone()).map_err(|error| {
                EngineSmokeError::new(format!("output was not utf-8: {error}"))
            })?);
        }
    }

    if output.is_empty() {
        Err(EngineSmokeError::new(
            "session output did not reach subscribed client",
        ))
    } else {
        Ok(output)
    }
}

fn post_and_drain_notification(session_id: &SessionId) -> Vec<String> {
    let mut inbox = NotificationInbox::new();
    inbox.post(NotificationItem::message(
        NotificationId("engine-smoke-notification".to_string()),
        NotificationTarget::Session(session_id.clone()),
        NotificationSeverity::Info,
        NotificationSource {
            label: "botster-core-dev".to_string(),
            plugin_key: None,
        },
        NotificationContent {
            title: "Engine smoke notice".to_string(),
            body: Some("The dev harness posted a typed notification.".to_string()),
            extension: None,
        },
        NotificationTimestamp(1),
    ));

    inbox
        .drain(
            &NotificationTarget::Session(session_id.clone()),
            NotificationTimestamp(2),
        )
        .into_iter()
        .map(|item| item.content.title)
        .collect()
}

fn invoke_fake_plugin() -> Result<String, EngineSmokeError> {
    let plugin_key = PluginKey("engine-smoke-plugin".to_string());
    let handler = PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "handle-smoke".to_string(),
    };
    let engine = PluginWorkerEngine::new();
    let runtime = FakePluginRuntime::default();

    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: "engine-smoke-plugin".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: PackageManifest {
            name: "engine-smoke-plugin".to_string(),
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
        },
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    });

    match engine.invoke(PluginInvocationRequest {
        request_id: request_id("plugin-smoke"),
        handler,
        timeout_ms: 1_000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("botster-core-dev".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "command": "smoke" })),
    }) {
        PluginInvocationResult::Completed(success) => success
            .payload
            .and_then(|payload| {
                payload
                    .0
                    .get("result")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .ok_or_else(|| EngineSmokeError::new("plugin returned no result payload")),
        PluginInvocationResult::Failed(failure) => Err(EngineSmokeError::new(format!(
            "plugin failed: {}",
            failure.reason
        ))),
    }
}

#[derive(Clone, Default)]
struct FakePluginRuntime {
    invocations: Arc<Mutex<Vec<RequestId>>>,
}

impl PluginRuntime for FakePluginRuntime {
    fn invoke(&self, request: PluginInvocationRequest) -> PluginInvocationResult {
        match self.invocations.lock() {
            Ok(mut invocations) => invocations.push(request.request_id.clone()),
            Err(_) => {
                return PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::HandlerFailed,
                    timeout_ms: None,
                    reason: "fake plugin invocation recorder was poisoned".to_string(),
                });
            }
        }

        PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: Some(BoundaryJson(serde_json::json!({
                "result": "fake plugin handler invoked"
            }))),
        })
    }
}

fn request_id(value: &str) -> RequestId {
    RequestId(format!("engine-smoke-{value}"))
}

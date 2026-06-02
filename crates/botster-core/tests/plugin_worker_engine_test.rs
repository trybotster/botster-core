//! Plugin worker engine acceptance tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use botster_core::{
    BoundaryJson, Capability, CapabilitySurface, ExtensionEntrypoint, ExtensionKind,
    ExtensionRuntime, PackageManifest, PluginCancellationToken, PluginCleanupScope,
    PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind, PluginHandlerRef,
    PluginHandlerRegistration, PluginInvocationContext, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult,
    PluginInvocationSuccess, PluginKey, PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec,
    PluginResourceKind, PluginResourceRef, PluginRuntime, PluginUnloadSpec, PluginWorkerEngine,
    PluginWorkerEngineConfig, PluginWorkerEvent, PluginWorkerRegistration, RequestId,
};

#[derive(Clone)]
struct FakeRuntime {
    behavior: Arc<Mutex<FakeBehavior>>,
    invocations: Arc<Mutex<Vec<PluginInvocationRequest>>>,
    stopped: Arc<Mutex<Vec<PluginKey>>>,
    cancellations_observed: Arc<Mutex<usize>>,
}

#[derive(Clone)]
enum FakeBehavior {
    Success(BoundaryJson),
    Failure(String),
    Delay {
        duration: Duration,
        payload: BoundaryJson,
    },
    WaitForCancellation,
    IgnoreCancellationThenReturn {
        duration: Duration,
        payload: BoundaryJson,
    },
    NeverReturn,
}

impl FakeRuntime {
    fn success(value: &str) -> Self {
        Self::new(FakeBehavior::Success(BoundaryJson(
            serde_json::json!({ "value": value }),
        )))
    }

    fn failure(reason: &str) -> Self {
        Self::new(FakeBehavior::Failure(reason.to_string()))
    }

    fn delayed(duration: Duration) -> Self {
        Self::new(FakeBehavior::Delay {
            duration,
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    fn waits_for_cancellation() -> Self {
        Self::new(FakeBehavior::WaitForCancellation)
    }

    fn ignores_cancellation_then_returns(duration: Duration) -> Self {
        Self::new(FakeBehavior::IgnoreCancellationThenReturn {
            duration,
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    fn never_returns() -> Self {
        Self::new(FakeBehavior::NeverReturn)
    }

    fn new(behavior: FakeBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            invocations: Arc::new(Mutex::new(Vec::new())),
            stopped: Arc::new(Mutex::new(Vec::new())),
            cancellations_observed: Arc::new(Mutex::new(0)),
        }
    }

    fn invocations(&self) -> Vec<PluginInvocationRequest> {
        self.invocations
            .lock()
            .expect("fake runtime invocations lock")
            .clone()
    }

    fn stopped(&self) -> Vec<PluginKey> {
        self.stopped
            .lock()
            .expect("fake runtime stopped lock")
            .clone()
    }

    fn cancellations_observed(&self) -> usize {
        *self
            .cancellations_observed
            .lock()
            .expect("fake runtime cancellations lock")
    }

    fn set_behavior(&self, behavior: FakeBehavior) {
        *self.behavior.lock().expect("fake runtime behavior lock") = behavior;
    }
}

impl PluginRuntime for FakeRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.invocations
            .lock()
            .expect("fake runtime invocations lock")
            .push(request.clone());

        match self
            .behavior
            .lock()
            .expect("fake runtime behavior lock")
            .clone()
        {
            FakeBehavior::Success(payload) => {
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakeBehavior::Failure(reason) => {
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::HandlerFailed,
                    timeout_ms: None,
                    reason,
                })
            }
            FakeBehavior::Delay { duration, payload } => {
                std::thread::sleep(duration);
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakeBehavior::WaitForCancellation => {
                while !cancellation.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                *self
                    .cancellations_observed
                    .lock()
                    .expect("fake runtime cancellations lock") += 1;
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::Cancelled,
                    timeout_ms: None,
                    reason: "cancelled by fake runtime".to_string(),
                })
            }
            FakeBehavior::IgnoreCancellationThenReturn { duration, payload } => {
                std::thread::sleep(duration);
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakeBehavior::NeverReturn => loop {
                std::thread::sleep(Duration::from_secs(60));
            },
        }
    }

    fn stop(&self, plugin_key: &PluginKey) {
        self.stopped
            .lock()
            .expect("fake runtime stopped lock")
            .push(plugin_key.clone());
    }
}

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn handler(plugin_key: &PluginKey, id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: id.to_string(),
    }
}

fn descriptor(
    plugin_key: &PluginKey,
    id: &str,
    handler: PluginHandlerRef,
) -> PluginOwnedDescriptor {
    PluginOwnedDescriptor {
        descriptor: PluginDescriptorRef {
            plugin_key: plugin_key.clone(),
            kind: PluginDescriptorKind::Command,
            descriptor_id: id.to_string(),
        },
        handler: Some(handler),
        body: BoundaryJson(serde_json::json!({ "id": id })),
    }
}

fn manifest(plugin_key: &PluginKey, capabilities: Vec<Capability>) -> PackageManifest {
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
    }
}

fn load_spec(plugin_key: &PluginKey, descriptors: Vec<PluginOwnedDescriptor>) -> PluginLoadSpec {
    PluginLoadSpec {
        plugin_key: plugin_key.clone(),
        package: plugin_key.0.clone(),
        entrypoint: "plugin.lua".to_string(),
        descriptors,
        metadata: None,
    }
}

fn registration(
    plugin_key: &PluginKey,
    runtime: FakeRuntime,
    handler: PluginHandlerRef,
    descriptors: Vec<PluginOwnedDescriptor>,
    capabilities: Vec<Capability>,
    required_capability: Option<Capability>,
) -> PluginWorkerRegistration {
    PluginWorkerRegistration {
        load: load_spec(plugin_key, descriptors),
        manifest: manifest(plugin_key, capabilities),
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler,
            required_capability,
        }],
        resources: Vec::new(),
    }
}

fn invocation(
    request_id: &str,
    handler: PluginHandlerRef,
    timeout_ms: u64,
) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(request_id.to_string()),
        handler,
        timeout_ms,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "input": request_id })),
    }
}

fn network_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("api".to_string()),
    }
}

fn wait_until(deadline: Duration, predicate: impl Fn() -> bool) {
    let started = std::time::Instant::now();
    while started.elapsed() < deadline {
        if predicate() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(predicate(), "condition did not become true before deadline");
}

#[test]
fn handler_invocation_dispatches_to_registered_runtime() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "advance");
    let runtime = FakeRuntime::success("ok");
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "advance", command.clone())],
        Vec::new(),
        None,
    ));

    let result = engine.invoke(invocation("req-1", command.clone(), 1_000));

    match result.result {
        PluginInvocationResult::Completed(success) => {
            assert_eq!(success.request_id, request_id("req-1"));
            assert_eq!(success.handler, command);
            assert_eq!(
                success.payload,
                Some(BoundaryJson(serde_json::json!({ "value": "ok" })))
            );
        }
        other => panic!("expected successful invocation, got {other:?}"),
    }
    assert_eq!(runtime.invocations().len(), 1);
}

#[test]
fn invocation_timeout_is_attributed_to_request_handler_and_plugin() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "slow");
    let runtime = FakeRuntime::delayed(Duration::from_millis(100));
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin,
        runtime,
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));

    let result = engine.invoke(invocation("req-timeout", command.clone(), 10));

    match result.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.request_id, request_id("req-timeout"));
            assert_eq!(failure.handler, command);
            assert_eq!(failure.handler.plugin_key, plugin);
            assert_eq!(failure.kind, PluginInvocationFailureKind::TimedOut);
            assert_eq!(failure.timeout_ms, Some(10));
        }
        other => panic!("expected timed out invocation, got {other:?}"),
    }
}

#[test]
fn timeout_cancels_runtime_invocation_and_releases_plugin_capacity() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "slow");
    let runtime = FakeRuntime::waits_for_cancellation();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));

    let timeout = engine.invoke(invocation("req-timeout", command.clone(), 10));
    match &timeout.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.kind, PluginInvocationFailureKind::TimedOut);
            assert_eq!(failure.timeout_ms, Some(10));
        }
        other => panic!("expected timeout, got {other:?}"),
    }
    assert!(matches!(
        timeout.events.as_slice(),
        [PluginWorkerEvent::InvocationTimedOut(failure)]
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));

    wait_until(Duration::from_millis(250), || {
        runtime.cancellations_observed() == 1 && engine.backpressure_for(&plugin).depth == 0
    });

    runtime.set_behavior(FakeBehavior::Success(BoundaryJson(
        serde_json::json!({ "value": "after-timeout" }),
    )));
    assert!(matches!(
        engine
            .invoke(invocation("req-after-timeout", command, 1_000))
            .result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn unload_cancels_in_flight_invocations_before_cleanup() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "slow");
    let runtime = FakeRuntime::waits_for_cancellation();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    engine.record_resource(PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::Watch,
        resource_id: "watch-1".to_string(),
    });

    let in_flight_engine = engine.clone();
    let in_flight_command = command.clone();
    let in_flight_handle = std::thread::spawn(move || {
        in_flight_engine.invoke(invocation("req-in-flight", in_flight_command, 1_000))
    });

    wait_until(Duration::from_millis(250), || {
        !runtime.invocations().is_empty()
    });

    let cleanup = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-a"),
        plugin_key: plugin.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });

    wait_until(Duration::from_millis(250), || {
        runtime.cancellations_observed() == 1
    });
    let outcome = in_flight_handle.join().expect("in-flight invoke thread");
    assert!(matches!(
        outcome.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::Cancelled,
            ..
        })
    ));
    assert_eq!(runtime.stopped(), vec![plugin.clone()]);
    assert_eq!(cleanup.removed_descriptors.len(), 1);
    assert_eq!(cleanup.removed_resources.len(), 1);
    assert!(matches!(
        engine
            .invoke(invocation("req-after-unload", command, 10))
            .result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::WorkerStopped,
            ..
        })
    ));
}

#[test]
fn runtime_failure_is_attributed_without_corrupting_other_plugins() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "fail");
    let handler_b = handler(&plugin_b, "ok");
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin_a,
        FakeRuntime::failure("boom"),
        handler_a.clone(),
        vec![descriptor(&plugin_a, "fail", handler_a.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("still-ok"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "ok", handler_b.clone())],
        Vec::new(),
        None,
    ));

    let failed = engine.invoke(invocation("req-a", handler_a.clone(), 1_000));
    let completed = engine.invoke(invocation("req-b", handler_b.clone(), 1_000));

    match failed.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.handler, handler_a);
            assert_eq!(failure.handler.plugin_key, plugin_a);
            assert_eq!(failure.kind, PluginInvocationFailureKind::HandlerFailed);
        }
        other => panic!("expected plugin A failure, got {other:?}"),
    }
    assert!(matches!(
        completed.result,
        PluginInvocationResult::Completed(_)
    ));
    assert_eq!(engine.descriptors_for(&plugin_b).len(), 1);
}

#[test]
fn reload_cleanup_replaces_one_plugin_descriptors_only() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let old_a = handler(&plugin_a, "old");
    let new_a = handler(&plugin_a, "new");
    let handler_b = handler(&plugin_b, "render");
    let runtime_a = FakeRuntime::success("old");
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin_a,
        runtime_a.clone(),
        old_a.clone(),
        vec![descriptor(&plugin_a, "old", old_a)],
        Vec::new(),
        None,
    ));
    engine.record_resource(PluginResourceRef {
        plugin_key: plugin_a.clone(),
        kind: PluginResourceKind::McpRegistration,
        resource_id: "old-tool".to_string(),
    });
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("b"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "home", handler_b.clone())],
        Vec::new(),
        None,
    ));

    let cleanup = engine.reload_plugin(
        PluginReloadSpec {
            request_id: request_id("reload-a"),
            plugin_key: plugin_a.clone(),
            load: load_spec(&plugin_a, vec![descriptor(&plugin_a, "new", new_a.clone())]),
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        },
        registration(
            &plugin_a,
            FakeRuntime::success("new"),
            new_a.clone(),
            vec![descriptor(&plugin_a, "new", new_a.clone())],
            Vec::new(),
            None,
        ),
    );

    assert_eq!(cleanup.plugin_key, plugin_a);
    assert_eq!(cleanup.removed_descriptors.len(), 1);
    assert_eq!(cleanup.removed_resources.len(), 1);
    assert_eq!(runtime_a.stopped(), vec![plugin_key("project-pipelines")]);
    assert_eq!(engine.descriptors_for(&plugin_a)[0].descriptor_id, "new");
    assert_eq!(engine.descriptors_for(&plugin_b)[0].descriptor_id, "home");
}

#[test]
fn reload_cancels_only_replaced_plugin_and_keeps_neighbor_alive() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let old_a = handler(&plugin_a, "old");
    let new_a = handler(&plugin_a, "new");
    let handler_b = handler(&plugin_b, "render");
    let runtime_a = FakeRuntime::waits_for_cancellation();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin_a,
        runtime_a.clone(),
        old_a.clone(),
        vec![descriptor(&plugin_a, "old", old_a.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("b"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "home", handler_b.clone())],
        Vec::new(),
        None,
    ));

    let in_flight_engine = engine.clone();
    let in_flight_old_a = old_a.clone();
    let old_invocation_handle = std::thread::spawn(move || {
        in_flight_engine.invoke(invocation("req-old-a", in_flight_old_a, 1_000))
    });
    wait_until(Duration::from_millis(250), || {
        !runtime_a.invocations().is_empty()
    });

    let cleanup = engine.reload_plugin(
        PluginReloadSpec {
            request_id: request_id("reload-a"),
            plugin_key: plugin_a.clone(),
            load: load_spec(&plugin_a, vec![descriptor(&plugin_a, "new", new_a.clone())]),
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        },
        registration(
            &plugin_a,
            FakeRuntime::success("new"),
            new_a.clone(),
            vec![descriptor(&plugin_a, "new", new_a.clone())],
            Vec::new(),
            None,
        ),
    );

    wait_until(Duration::from_millis(250), || {
        runtime_a.cancellations_observed() == 1
    });
    assert!(matches!(
        old_invocation_handle.join().expect("old invocation").result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::Cancelled,
            ..
        })
    ));
    assert_eq!(cleanup.plugin_key, plugin_a);
    assert_eq!(engine.descriptors_for(&plugin_a)[0].descriptor_id, "new");
    assert!(matches!(
        engine.invoke(invocation("req-b", handler_b, 1_000)).result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn reload_drops_stale_results_from_previous_plugin_generation() {
    let plugin = plugin_key("project-pipelines");
    let old_handler = handler(&plugin, "old");
    let new_handler = handler(&plugin, "new");
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::ignores_cancellation_then_returns(Duration::from_millis(80)),
        old_handler.clone(),
        vec![descriptor(&plugin, "old", old_handler.clone())],
        Vec::new(),
        None,
    ));

    let timeout = engine.invoke(invocation("req-old-timeout", old_handler.clone(), 10));
    assert!(matches!(
        timeout.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::TimedOut,
            ..
        })
    ));

    engine.reload_plugin(
        PluginReloadSpec {
            request_id: request_id("reload-a"),
            plugin_key: plugin.clone(),
            load: load_spec(
                &plugin,
                vec![descriptor(&plugin, "new", new_handler.clone())],
            ),
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        },
        registration(
            &plugin,
            FakeRuntime::success("new"),
            new_handler.clone(),
            vec![descriptor(&plugin, "new", new_handler.clone())],
            Vec::new(),
            None,
        ),
    );

    wait_until(Duration::from_millis(250), || {
        engine.backpressure_for(&plugin).depth == 0
    });
    assert!(matches!(
        engine
            .invoke(invocation("req-new", new_handler, 1_000))
            .result,
        PluginInvocationResult::Completed(PluginInvocationSuccess { payload, .. })
            if payload == Some(BoundaryJson(serde_json::json!({ "value": "new" })))
    ));
    assert!(matches!(
        engine
            .invoke(invocation("req-old", old_handler, 1_000))
            .result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::HandlerFailed,
            ..
        })
    ));
}

#[test]
fn unload_cleanup_removes_only_owner_plugin() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "advance");
    let handler_b = handler(&plugin_b, "render");
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin_a,
        FakeRuntime::success("a"),
        handler_a.clone(),
        vec![descriptor(&plugin_a, "advance", handler_a.clone())],
        Vec::new(),
        None,
    ));
    engine.record_resource(PluginResourceRef {
        plugin_key: plugin_a.clone(),
        kind: PluginResourceKind::Watch,
        resource_id: "watch-1".to_string(),
    });
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("b"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "home", handler_b.clone())],
        Vec::new(),
        None,
    ));

    let cleanup = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-a"),
        plugin_key: plugin_a.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });

    assert!(cleanup
        .removed_descriptors
        .iter()
        .all(|descriptor| descriptor.plugin_key == plugin_a));
    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin_a));
    assert!(engine.descriptors_for(&plugin_a).is_empty());
    assert_eq!(engine.descriptors_for(&plugin_b).len(), 1);
    assert!(matches!(
        engine.invoke(invocation("req-b", handler_b, 1_000)).result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn capability_checks_use_declared_package_metadata_for_rejection_and_grant() {
    let plugin = plugin_key("networked");
    let command = handler(&plugin, "fetch");
    let required = network_capability();
    let engine = PluginWorkerEngine::new();
    let runtime = FakeRuntime::success("allowed");

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "fetch", command.clone())],
        Vec::new(),
        Some(required.clone()),
    ));

    let rejected = engine.invoke(invocation("req-denied", command.clone(), 1_000));
    match rejected.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.handler, command);
            assert_eq!(failure.kind, PluginInvocationFailureKind::HandlerFailed);
            assert!(failure.reason.contains("capability"));
        }
        other => panic!("expected capability rejection, got {other:?}"),
    }
    assert!(runtime.invocations().is_empty());

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        handler(&plugin, "fetch"),
        vec![descriptor(&plugin, "fetch", handler(&plugin, "fetch"))],
        vec![required.clone()],
        Some(required),
    ));

    assert!(matches!(
        engine
            .invoke(invocation("req-allowed", handler(&plugin, "fetch"), 1_000))
            .result,
        PluginInvocationResult::Completed(_)
    ));
    assert_eq!(runtime.invocations().len(), 1);
}

#[test]
fn backpressure_is_isolated_by_plugin_identity() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "slow");
    let handler_b = handler(&plugin_b, "fast");
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin_a,
        FakeRuntime::delayed(Duration::from_millis(100)),
        handler_a.clone(),
        vec![descriptor(&plugin_a, "slow", handler_a.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("b"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "fast", handler_b.clone())],
        Vec::new(),
        None,
    ));

    let timeout = engine.invoke(invocation("req-a-1", handler_a.clone(), 10));
    assert!(matches!(
        timeout.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::TimedOut,
            ..
        })
    ));

    let pressured = engine.invoke(invocation("req-a-2", handler_a, 1_000));
    match pressured.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.kind, PluginInvocationFailureKind::Backpressured);
            assert_eq!(failure.handler.plugin_key, plugin_a);
        }
        other => panic!("expected backpressure for plugin A, got {other:?}"),
    }

    let pressure = engine.backpressure_for(&plugin_a);
    assert_eq!(pressure.route.plugin_key, Some(plugin_a));
    assert_eq!(pressure.capacity, 1);
    assert_eq!(pressure.depth, 1);
    assert!(matches!(
        engine.invoke(invocation("req-b", handler_b, 1_000)).result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn late_runtime_completion_after_timeout_does_not_double_release_capacity() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "late");
    let runtime = FakeRuntime::ignores_cancellation_then_returns(Duration::from_millis(80));
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin,
        runtime,
        command.clone(),
        vec![descriptor(&plugin, "late", command.clone())],
        Vec::new(),
        None,
    ));

    let timeout = engine.invoke(invocation("req-timeout", command.clone(), 10));
    assert!(matches!(
        timeout.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::TimedOut,
            ..
        })
    ));
    assert_eq!(engine.backpressure_for(&plugin).depth, 1);

    wait_until(Duration::from_millis(250), || {
        engine.backpressure_for(&plugin).depth == 0
    });
    assert_eq!(engine.backpressure_for(&plugin).depth, 0);
    assert!(matches!(
        engine
            .invoke(invocation("req-after-late", command, 1_000))
            .result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn repeated_timeouts_do_not_spawn_unbounded_live_threads() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "never");
    let handler_b = handler(&plugin_b, "fast");
    let runtime_a = FakeRuntime::never_returns();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 2,
    });

    engine.load_plugin(registration(
        &plugin_a,
        runtime_a.clone(),
        handler_a.clone(),
        vec![descriptor(&plugin_a, "never", handler_a.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &plugin_b,
        FakeRuntime::success("b"),
        handler_b.clone(),
        vec![descriptor(&plugin_b, "fast", handler_b.clone())],
        Vec::new(),
        None,
    ));

    for request in ["req-a-1", "req-a-2"] {
        let timeout = engine.invoke(invocation(request, handler_a.clone(), 10));
        assert!(matches!(
            timeout.result,
            PluginInvocationResult::Failed(PluginInvocationFailure {
                kind: PluginInvocationFailureKind::TimedOut,
                ..
            })
        ));
    }

    let overflow = engine.invoke(invocation("req-a-3", handler_a, 10));
    assert!(matches!(
        overflow.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::Backpressured,
            ..
        })
    ));
    assert!(matches!(
        overflow.events.as_slice(),
        [PluginWorkerEvent::Backpressure(summary)]
            if summary.capacity == 2 && summary.depth == 2
    ));
    assert_eq!(runtime_a.invocations().len(), 2);
    assert_eq!(engine.backpressure_for(&plugin_a).depth, 2);
    assert!(matches!(
        engine.invoke(invocation("req-b", handler_b, 1_000)).result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn timeout_and_backpressure_emit_typed_plugin_worker_events() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "slow");
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });

    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::never_returns(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));

    let timeout = engine.invoke(invocation("req-timeout", command.clone(), 10));
    assert!(matches!(
        timeout.events.as_slice(),
        [PluginWorkerEvent::InvocationTimedOut(failure)]
            if failure.request_id == request_id("req-timeout")
                && failure.handler == command
                && failure.kind == PluginInvocationFailureKind::TimedOut
                && failure.timeout_ms == Some(10)
    ));

    let pressured = engine.invoke(invocation("req-pressured", command.clone(), 10));
    assert!(matches!(
        pressured.events.as_slice(),
        [PluginWorkerEvent::Backpressure(summary)]
            if summary.capacity == 1
                && summary.depth == 1
                && summary.route.plugin_key == Some(plugin)
    ));
}

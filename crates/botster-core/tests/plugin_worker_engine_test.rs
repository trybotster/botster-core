//! Plugin worker engine acceptance tests.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use botster_core::{
    BoundaryJson, Capability, CapabilitySurface, ExtensionEntrypoint, ExtensionKind,
    ExtensionRuntime, HostProfileMetadata, HostProfilePolicySection, PackageManifest,
    PluginCancellationToken, PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef,
    PluginHandlerKind, PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext,
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginInvocationSuccess, PluginKey, PluginLoadSpec,
    PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef, PluginRuntime,
    PluginUnloadSpec, PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerEvent,
    PluginWorkerRegistration, RequestId,
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
        }
    }

    fn stop(&self, plugin_key: &PluginKey) {
        self.stopped
            .lock()
            .expect("fake runtime stopped lock")
            .push(plugin_key.clone());
    }
}

#[derive(Clone, Default)]
struct GatedRuntime {
    state: Arc<GatedRuntimeState>,
}

#[derive(Default)]
struct GatedRuntimeState {
    started: AtomicUsize,
    executing: AtomicUsize,
    max_executing: AtomicUsize,
    gate: (Mutex<bool>, Condvar),
}

impl GatedRuntime {
    fn release(&self) {
        let (released, condition) = &self.state.gate;
        *released.lock().expect("gated runtime release lock") = true;
        condition.notify_all();
    }

    fn started(&self) -> usize {
        self.state.started.load(Ordering::SeqCst)
    }

    fn max_executing(&self) -> usize {
        self.state.max_executing.load(Ordering::SeqCst)
    }
}

impl PluginRuntime for GatedRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.state.started.fetch_add(1, Ordering::SeqCst);
        let executing = self.state.executing.fetch_add(1, Ordering::SeqCst) + 1;
        self.state
            .max_executing
            .fetch_max(executing, Ordering::SeqCst);

        let (released, condition) = &self.state.gate;
        let mut released = released.lock().expect("gated runtime gate lock");
        while !*released && !cancellation.is_cancelled() {
            let (next, _) = condition
                .wait_timeout(released, Duration::from_millis(1))
                .expect("gated runtime condition wait");
            released = next;
        }
        self.state.executing.fetch_sub(1, Ordering::SeqCst);

        PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        self.release();
    }
}

#[derive(Clone, Default)]
struct RetirementGatedRuntime {
    state: Arc<RetirementGatedRuntimeState>,
}

#[derive(Default)]
struct RetirementGatedRuntimeState {
    started: AtomicBool,
    stop_called: AtomicBool,
    gate: (Mutex<bool>, Condvar),
}

impl RetirementGatedRuntime {
    fn release(&self) {
        let (released, condition) = &self.state.gate;
        *released.lock().expect("retirement gate release lock") = true;
        condition.notify_all();
    }

    fn started(&self) -> bool {
        self.state.started.load(Ordering::SeqCst)
    }

    fn stop_called(&self) -> bool {
        self.state.stop_called.load(Ordering::SeqCst)
    }
}

impl PluginRuntime for RetirementGatedRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.state.started.store(true, Ordering::SeqCst);
        let (released, condition) = &self.state.gate;
        let mut released = released.lock().expect("retirement gate lock");
        while !*released {
            released = condition
                .wait(released)
                .expect("retirement gate condition wait");
        }

        PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        self.state.stop_called.store(true, Ordering::SeqCst);
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
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: None,
        configuration: None,
        runnable_entrypoints: Vec::new(),
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
    runtime: impl PluginRuntime,
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
fn default_queue_capacity_is_independent_from_executor_concurrency() {
    let engine = PluginWorkerEngine::new();
    for name in ["one", "two", "three", "four"] {
        let plugin = plugin_key(name);
        let command = handler(&plugin, "run");
        engine.load_plugin(registration(
            &plugin,
            FakeRuntime::success(name),
            command.clone(),
            vec![descriptor(&plugin, "run", command)],
            Vec::new(),
            None,
        ));
    }

    let snapshot = engine.debug_snapshot();
    assert_eq!(snapshot.configured_queue_capacity, 256);
    assert_eq!(snapshot.configured_executor_concurrency, 2);
    assert_eq!(snapshot.live_plugin_executors, 4);
    assert_eq!(snapshot.live_executor_workers, 8);
    assert_eq!(snapshot.queued_jobs, 0);
    assert_eq!(snapshot.in_flight_jobs, 0);
    assert!(snapshot
        .plugins
        .iter()
        .all(|plugin| plugin.live_executor_workers == 2));
    assert_eq!(
        snapshot
            .plugins
            .iter()
            .map(|plugin| plugin.plugin_key.0.as_str())
            .collect::<Vec<_>>(),
        vec!["four", "one", "three", "two"]
    );
}

#[test]
fn queue_capacity_and_executor_concurrency_must_be_positive() {
    assert!(std::panic::catch_unwind(|| {
        PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 0,
            per_plugin_executor_concurrency: 1,
        })
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 1,
            per_plugin_executor_concurrency: 0,
        })
    })
    .is_err());
}

#[test]
fn bounded_waiting_queue_reports_attributed_backpressure_and_neighbor_isolation() {
    let slow_plugin = plugin_key("slow");
    let fast_plugin = plugin_key("fast");
    let slow_handler = handler(&slow_plugin, "run");
    let fast_handler = handler(&fast_plugin, "run");
    let slow_runtime = GatedRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 4,
        per_plugin_executor_concurrency: 1,
    });
    engine.load_plugin(registration(
        &slow_plugin,
        slow_runtime.clone(),
        slow_handler.clone(),
        vec![descriptor(&slow_plugin, "run", slow_handler.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &fast_plugin,
        FakeRuntime::success("fast"),
        fast_handler.clone(),
        vec![descriptor(&fast_plugin, "run", fast_handler.clone())],
        Vec::new(),
        None,
    ));

    let mut callers = Vec::new();
    for index in 0..5 {
        let caller_engine = engine.clone();
        let caller_handler = slow_handler.clone();
        callers.push(std::thread::spawn(move || {
            caller_engine.invoke(invocation(
                &format!("queued-{index}"),
                caller_handler,
                2_000,
            ))
        }));
        if index == 0 {
            wait_until(Duration::from_millis(250), || slow_runtime.started() == 1);
        }
    }
    wait_until(Duration::from_millis(250), || {
        let snapshot = engine.debug_snapshot();
        snapshot.queued_jobs == 4 && snapshot.in_flight_jobs == 1
    });

    let pressured = engine.invoke(invocation("overflow", slow_handler, 2_000));
    assert!(matches!(
        pressured.events.as_slice(),
        [PluginWorkerEvent::Backpressure(summary)]
            if summary.capacity == 4
                && summary.depth == 4
                && summary.route.plugin_key == Some(slow_plugin.clone())
    ));
    assert!(matches!(
        engine
            .invoke(invocation("fast", fast_handler, 1_000))
            .result,
        PluginInvocationResult::Completed(_)
    ));

    slow_runtime.release();
    for caller in callers {
        assert!(matches!(
            caller.join().expect("slow caller should join").result,
            PluginInvocationResult::Completed(_)
        ));
    }
    assert_eq!(engine.debug_snapshot().queued_jobs, 0);
    assert_eq!(engine.debug_snapshot().in_flight_jobs, 0);
}

#[test]
fn executor_concurrency_allows_two_slow_invocations_to_overlap() {
    let plugin = plugin_key("slow");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 4,
        per_plugin_executor_concurrency: 2,
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    let mut callers = Vec::new();
    for index in 0..2 {
        let caller_engine = engine.clone();
        let caller_handler = command.clone();
        callers.push(std::thread::spawn(move || {
            caller_engine.invoke(invocation(
                &format!("concurrent-{index}"),
                caller_handler,
                2_000,
            ))
        }));
    }
    wait_until(Duration::from_millis(250), || runtime.started() == 2);
    assert_eq!(runtime.max_executing(), 2);
    assert_eq!(engine.debug_snapshot().in_flight_jobs, 2);

    runtime.release();
    for caller in callers {
        assert!(matches!(
            caller.join().expect("concurrent caller should join").result,
            PluginInvocationResult::Completed(_)
        ));
    }
}

#[test]
fn timed_out_queued_job_is_skipped_before_runtime_execution() {
    let plugin = plugin_key("slow");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    let active_engine = engine.clone();
    let active_handler = command.clone();
    let active = std::thread::spawn(move || {
        active_engine.invoke(invocation("active", active_handler, 2_000))
    });
    wait_until(Duration::from_millis(250), || runtime.started() == 1);

    let queued = engine.invoke(invocation("queued-timeout", command, 10));
    assert!(matches!(
        queued.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::TimedOut,
            ..
        })
    ));
    assert_eq!(engine.debug_snapshot().queued_jobs, 1);

    runtime.release();
    active.join().expect("active caller should join");
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().queued_jobs == 0
    });
    assert_eq!(runtime.started(), 1);
}

#[test]
fn repeated_load_unload_cycles_join_workers_and_return_debug_counts_to_zero() {
    let plugin = plugin_key("reloadable");
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
    });

    for cycle in 0..5 {
        let command = handler(&plugin, "run");
        let runtime = FakeRuntime::success("ok");
        engine.load_plugin(registration(
            &plugin,
            runtime.clone(),
            command.clone(),
            vec![descriptor(&plugin, "run", command)],
            Vec::new(),
            None,
        ));
        assert_eq!(engine.debug_snapshot().live_plugin_executors, 1);
        assert_eq!(engine.debug_snapshot().live_executor_workers, 2);

        engine.unload_plugin(PluginUnloadSpec {
            request_id: request_id(&format!("unload-{cycle}")),
            plugin_key: plugin.clone(),
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        });
        assert_eq!(runtime.stopped(), vec![plugin.clone()]);
        assert_eq!(engine.debug_snapshot().live_plugin_executors, 0);
        assert_eq!(engine.debug_snapshot().live_executor_workers, 0);
    }
}

#[test]
fn retiring_generation_remains_observable_until_unload_joins_its_worker() {
    let plugin = plugin_key("retiring");
    let command = handler(&plugin, "run");
    let runtime = RetirementGatedRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    let outcome = engine.invoke(invocation("retiring-timeout", command, 10));
    assert!(matches!(
        outcome.result,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            kind: PluginInvocationFailureKind::TimedOut,
            ..
        })
    ));
    wait_until(Duration::from_millis(250), || runtime.started());

    let unload_engine = engine.clone();
    let unload_plugin = plugin.clone();
    let unload_handle = std::thread::spawn(move || {
        unload_engine.unload_plugin(PluginUnloadSpec {
            request_id: request_id("retiring-unload"),
            plugin_key: unload_plugin,
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        })
    });
    wait_until(Duration::from_millis(250), || runtime.stop_called());

    let snapshot = engine.debug_snapshot();
    let unload_finished_before_release = unload_handle.is_finished();
    runtime.release();
    unload_handle.join().expect("retiring unload should join");

    assert!(
        !unload_finished_before_release,
        "unload returned before its executor worker retired"
    );
    assert!(snapshot.plugins.is_empty());
    assert_eq!(snapshot.live_plugin_executors, 1);
    assert_eq!(snapshot.live_executor_workers, 1);
    assert_eq!(snapshot.in_flight_jobs, 1);
    let retired = engine.debug_snapshot();
    assert_eq!(retired.live_plugin_executors, 0);
    assert_eq!(retired.live_executor_workers, 0);
    assert_eq!(retired.in_flight_jobs, 0);
}

#[test]
fn final_engine_drop_stops_runtime_and_joins_idle_workers() {
    let plugin = plugin_key("drop");
    let command = handler(&plugin, "run");
    let runtime = FakeRuntime::success("ok");
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 256,
        per_plugin_executor_concurrency: 2,
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command)],
        Vec::new(),
        None,
    ));
    assert_eq!(engine.debug_snapshot().live_executor_workers, 2);

    drop(engine);

    assert_eq!(runtime.stopped(), vec![plugin]);
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
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
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
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
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

    assert_eq!(runtime.cancellations_observed(), 1);
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
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
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
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
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
fn unload_cleanup_tracks_capability_runtime_resource_kinds() {
    let plugin = plugin_key("project-pipelines");
    let other_plugin = plugin_key("preview");
    let command = handler(&plugin, "advance");
    let other_command = handler(&other_plugin, "render");
    let engine = PluginWorkerEngine::new();

    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("ok"),
        command.clone(),
        vec![descriptor(&plugin, "advance", command)],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &other_plugin,
        FakeRuntime::success("ok"),
        other_command.clone(),
        vec![descriptor(&other_plugin, "render", other_command)],
        Vec::new(),
        None,
    ));

    for kind in [
        PluginResourceKind::HttpRequest,
        PluginResourceKind::NetworkConnection,
        PluginResourceKind::Watch,
        PluginResourceKind::FilesystemOperation,
        PluginResourceKind::PluginStoreOperation,
        PluginResourceKind::Timer,
    ] {
        let resource_id = format!("{kind:?}");
        engine.record_resource(PluginResourceRef {
            plugin_key: plugin.clone(),
            kind,
            resource_id,
        });
    }
    engine.record_resource(PluginResourceRef {
        plugin_key: other_plugin.clone(),
        kind: PluginResourceKind::NetworkConnection,
        resource_id: "other-ws".to_string(),
    });

    let cleanup = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("cleanup-runtime"),
        plugin_key: plugin.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });

    assert_eq!(cleanup.plugin_key, plugin);
    assert_eq!(cleanup.removed_resources.len(), 6);
    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin));
    assert!(!cleanup
        .removed_resources
        .iter()
        .any(|resource| resource.plugin_key == other_plugin));
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
fn host_profile_metadata_is_not_a_plugin_worker_capability_grant() {
    let plugin = plugin_key("networked-profile");
    let command = handler(&plugin, "fetch");
    let required = network_capability();
    let engine = PluginWorkerEngine::new();
    let runtime = FakeRuntime::success("not-called");
    let mut manifest = manifest(&plugin, Vec::new());

    manifest.host_profile = Some(HostProfileMetadata {
        profile_id: "botster-hub".to_string(),
        compatibility: ">=0.1.0".to_string(),
        precedence: 10,
        required_providers: vec!["network-provider".to_string()],
        required_capabilities: vec![required.clone()],
        policy_sections: vec![HostProfilePolicySection::Capabilities],
    });

    engine.load_plugin(PluginWorkerRegistration {
        load: load_spec(&plugin, vec![descriptor(&plugin, "fetch", command.clone())]),
        manifest,
        runtime: Arc::new(runtime.clone()),
        handlers: vec![PluginHandlerRegistration {
            handler: command.clone(),
            required_capability: Some(required),
        }],
        resources: Vec::new(),
    });

    let rejected = engine.invoke(invocation("req-denied-profile", command.clone(), 1_000));
    match rejected.result {
        PluginInvocationResult::Failed(failure) => {
            assert_eq!(failure.handler, command);
            assert_eq!(failure.kind, PluginInvocationFailureKind::HandlerFailed);
            assert!(failure.reason.contains("capability"));
        }
        other => panic!("expected capability rejection, got {other:?}"),
    }
    assert!(
        runtime.invocations().is_empty(),
        "metadata must not bypass manifest.capabilities"
    );
}

#[test]
fn backpressure_is_isolated_by_plugin_identity() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "slow");
    let handler_b = handler(&plugin_b, "fast");
    let runtime_a = GatedRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
    });

    engine.load_plugin(registration(
        &plugin_a,
        runtime_a.clone(),
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

    let active_engine = engine.clone();
    let active_handler = handler_a.clone();
    let active = std::thread::spawn(move || {
        active_engine.invoke(invocation("req-a-active", active_handler, 2_000))
    });
    wait_until(Duration::from_millis(250), || runtime_a.started() == 1);
    let queued_engine = engine.clone();
    let queued_handler = handler_a.clone();
    let queued = std::thread::spawn(move || {
        queued_engine.invoke(invocation("req-a-queued", queued_handler, 2_000))
    });
    wait_until(Duration::from_millis(250), || {
        engine.backpressure_for(&plugin_a).depth == 1
    });

    let pressured = engine.invoke(invocation("req-a-pressured", handler_a, 1_000));
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
    runtime_a.release();
    active.join().expect("active plugin A caller should join");
    queued.join().expect("queued plugin A caller should join");
}

#[test]
fn late_runtime_completion_after_timeout_does_not_double_release_capacity() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "late");
    let runtime = FakeRuntime::ignores_cancellation_then_returns(Duration::from_millis(80));
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
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
    assert_eq!(engine.backpressure_for(&plugin).depth, 0);
    assert_eq!(engine.debug_snapshot().in_flight_jobs, 1);

    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().in_flight_jobs == 0
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
fn repeated_timeouts_keep_fixed_executor_worker_count() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let handler_a = handler(&plugin_a, "slow");
    let handler_b = handler(&plugin_b, "fast");
    let runtime_a = FakeRuntime::waits_for_cancellation();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 2,
        per_plugin_executor_concurrency: 1,
    });

    engine.load_plugin(registration(
        &plugin_a,
        runtime_a.clone(),
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

    for (index, request) in ["req-a-1", "req-a-2", "req-a-3"].into_iter().enumerate() {
        let timeout = engine.invoke(invocation(request, handler_a.clone(), 10));
        assert!(matches!(
            timeout.result,
            PluginInvocationResult::Failed(PluginInvocationFailure {
                kind: PluginInvocationFailureKind::TimedOut,
                ..
            })
        ));
        wait_until(Duration::from_millis(250), || {
            runtime_a.cancellations_observed() == index + 1
        });
    }
    wait_until(Duration::from_millis(250), || {
        let snapshot = engine.debug_snapshot();
        snapshot.queued_jobs == 0 && snapshot.in_flight_jobs == 0
    });
    let snapshot = engine.debug_snapshot();
    assert_eq!(runtime_a.invocations().len(), 3);
    assert_eq!(snapshot.live_plugin_executors, 2);
    assert_eq!(snapshot.live_executor_workers, 2);
    assert_eq!(snapshot.queued_jobs, 0);
    assert_eq!(snapshot.in_flight_jobs, 0);
    assert!(matches!(
        engine.invoke(invocation("req-b", handler_b, 1_000)).result,
        PluginInvocationResult::Completed(_)
    ));
}

#[test]
fn timeout_and_backpressure_emit_typed_plugin_worker_events() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "slow");
    let timeout_engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
    });

    timeout_engine.load_plugin(registration(
        &plugin,
        FakeRuntime::waits_for_cancellation(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));

    let timeout = timeout_engine.invoke(invocation("req-timeout", command.clone(), 10));
    assert!(matches!(
        timeout.events.as_slice(),
        [PluginWorkerEvent::InvocationTimedOut(failure)]
            if failure.request_id == request_id("req-timeout")
                && failure.handler == command
                && failure.kind == PluginInvocationFailureKind::TimedOut
                && failure.timeout_ms == Some(10)
    ));

    let pressure_runtime = GatedRuntime::default();
    let pressure_engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 1,
    });
    pressure_engine.load_plugin(registration(
        &plugin,
        pressure_runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    let active_engine = pressure_engine.clone();
    let active_handler = command.clone();
    let active = std::thread::spawn(move || {
        active_engine.invoke(invocation("req-active", active_handler, 2_000))
    });
    wait_until(Duration::from_millis(250), || {
        pressure_runtime.started() == 1
    });
    let queued_engine = pressure_engine.clone();
    let queued_handler = command.clone();
    let queued = std::thread::spawn(move || {
        queued_engine.invoke(invocation("req-queued", queued_handler, 2_000))
    });
    wait_until(Duration::from_millis(250), || {
        pressure_engine.debug_snapshot().queued_jobs == 1
    });

    let pressured = pressure_engine.invoke(invocation("req-pressured", command.clone(), 10));
    assert!(matches!(
        pressured.events.as_slice(),
        [PluginWorkerEvent::Backpressure(summary)]
            if summary.capacity == 1
                && summary.depth == 1
                && summary.route.plugin_key == Some(plugin)
    ));
    pressure_runtime.release();
    active.join().expect("active caller should join");
    queued.join().expect("queued caller should join");
}

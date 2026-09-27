//! Plugin worker engine acceptance tests.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use botster_core::{
    BoundaryJson, Capability, CapabilitySurface, ExtensionEntrypoint, ExtensionKind,
    ExtensionRuntime, HostProfileMetadata, HostProfilePolicySection, PackageManifest,
    PluginAdmissionResult, PluginCancellationToken, PluginCleanupScope, PluginCompletion,
    PluginCompletionItem, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginHandlerRegistration, PluginInvocationClass, PluginInvocationContext,
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginInvocationSuccess, PluginKey, PluginLoadSpec,
    PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef, PluginRuntime,
    PluginUnloadSpec, PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerEvent,
    PluginWorkerRegistration, RequestId,
};
use botster_core::{PluginQueueProbe, PluginQueueProbeEvent};
use botster_core_test_support::bounded_wait::HANG_GUARD;

#[derive(Clone)]
struct FakeRuntime {
    behavior: Arc<Mutex<FakeBehavior>>,
    invocations: Arc<Mutex<Vec<PluginInvocationRequest>>>,
    stopped: Arc<Mutex<Vec<PluginKey>>>,
    cancellations_observed: Arc<Mutex<usize>>,
    /// Released by the test (or, for slow work, by cancellation).
    gate: Arc<(Mutex<bool>, Condvar)>,
}

#[derive(Clone)]
enum FakeBehavior {
    Success(BoundaryJson),
    Failure(String),
    /// Slow work: runs until the test releases it or the engine cancels it.
    Slow {
        payload: BoundaryJson,
    },
    WaitForCancellation,
    /// Ignores cancellation: runs until the test releases it, then returns
    /// its (possibly stale) result.
    IgnoreCancellationUntilReleased {
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

    fn slow() -> Self {
        Self::new(FakeBehavior::Slow {
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    fn waits_for_cancellation() -> Self {
        Self::new(FakeBehavior::WaitForCancellation)
    }

    fn ignores_cancellation_until_released() -> Self {
        Self::new(FakeBehavior::IgnoreCancellationUntilReleased {
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    /// Let held work finish.
    fn release(&self) {
        let (released, changed) = &*self.gate;
        *released.lock().expect("fake runtime gate lock") = true;
        changed.notify_all();
    }

    /// Block until released, or also until cancelled when `cancellable`.
    fn hold(&self, cancellation: &PluginCancellationToken, cancellable: bool) {
        if cancellable {
            let gate = Arc::clone(&self.gate);
            cancellation.on_cancel(move || {
                // Take the gate's own lock so the waiter cannot miss this.
                let _released = gate.0.lock().expect("fake runtime gate lock");
                gate.1.notify_all();
            });
        }
        let (released, changed) = &*self.gate;
        let mut released = released.lock().expect("fake runtime gate lock");
        while !*released && !(cancellable && cancellation.is_cancelled()) {
            released = changed.wait(released).expect("fake runtime gate wait");
        }
    }

    fn new(behavior: FakeBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            invocations: Arc::new(Mutex::new(Vec::new())),
            stopped: Arc::new(Mutex::new(Vec::new())),
            cancellations_observed: Arc::new(Mutex::new(0)),
            gate: Arc::new((Mutex::new(false), Condvar::new())),
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
        CHANGES.bump();

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
            FakeBehavior::Slow { payload } => {
                self.hold(&cancellation, true);
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakeBehavior::WaitForCancellation => {
                let (cancelled, on_cancel) = mpsc::channel();
                cancellation.on_cancel(move || {
                    let _ = cancelled.send(());
                });
                let _ = on_cancel.recv();
                *self
                    .cancellations_observed
                    .lock()
                    .expect("fake runtime cancellations lock") += 1;
                CHANGES.bump();
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::Cancelled,
                    timeout_ms: None,
                    reason: "cancelled by fake runtime".to_string(),
                })
            }
            FakeBehavior::IgnoreCancellationUntilReleased { payload } => {
                self.hold(&cancellation, false);
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
        CHANGES.bump();
        // The engine stops a runtime when it retires its generation: held
        // work ends then, and its (stale) result comes back.
        self.release();
    }
}

#[derive(Clone, Default)]
struct GatedRuntime {
    state: Arc<GatedRuntimeState>,
}

#[derive(Default)]
struct GatedRuntimeState {
    /// Reports each start, when the test asked for start events.
    start_events: Option<Mutex<mpsc::Sender<()>>>,
    started: AtomicUsize,
    executing: AtomicUsize,
    max_executing: AtomicUsize,
    gate: (Mutex<bool>, Condvar),
}

impl GatedRuntime {
    /// A gated runtime that reports each invocation start on the receiver.
    fn with_start_events() -> (Self, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        let runtime = Self {
            state: Arc::new(GatedRuntimeState {
                start_events: Some(Mutex::new(sender)),
                ..GatedRuntimeState::default()
            }),
        };
        (runtime, receiver)
    }

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
        // Report only after every counter a waiter reads has moved.
        CHANGES.bump();
        if let Some(events) = &self.state.start_events {
            let _ = events.lock().expect("gated runtime start events").send(());
        }

        let state = Arc::clone(&self.state);
        cancellation.on_cancel(move || {
            // Take the gate's own lock so the waiter cannot miss this.
            let _released = state.gate.0.lock().expect("gated runtime gate lock");
            state.gate.1.notify_all();
        });
        let (released, condition) = &self.state.gate;
        let mut released = released.lock().expect("gated runtime gate lock");
        while !*released && !cancellation.is_cancelled() {
            released = condition
                .wait(released)
                .expect("gated runtime condition wait");
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
        CHANGES.bump();
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
        CHANGES.bump();
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

const ADMISSION_LOCK_BUSY: &str = "admission lock busy";

/// One change counter for this test binary. Every engine built by [`probed`]
/// bumps it through its job-state probe, and the test runtimes bump it when
/// their own observable state changes. Tests running in parallel share it,
/// which only adds spurious wakeups: a waiter rechecks its predicate.
struct Changes {
    count: Mutex<u64>,
    changed: Condvar,
}

static CHANGES: Changes = Changes {
    count: Mutex::new(0),
    changed: Condvar::new(),
};

impl Changes {
    fn bump(&self) {
        *self.count.lock().expect("changes lock") += 1;
        self.changed.notify_all();
    }

    fn current(&self) -> u64 {
        *self.count.lock().expect("changes lock")
    }
}

/// An engine whose job-state changes wake [`wait_until`]. A config that
/// already carries its own probe keeps it.
fn probed(mut config: PluginWorkerEngineConfig) -> PluginWorkerEngine {
    if config.test_queue_probe.is_none() {
        config.test_queue_probe = Some(PluginQueueProbe::from_fn(|_| CHANGES.bump()));
    }
    PluginWorkerEngine::with_config(config)
}

/// Wait until `predicate` holds, for at most `deadline`. The predicate is
/// rechecked after each change the engines or the test runtimes report, so
/// no change is missed: the counter is read before each check.
fn wait_until(deadline: Duration, predicate: impl Fn() -> bool) {
    let end = Instant::now() + deadline;
    loop {
        let seen = CHANGES.current();
        if predicate() {
            return;
        }
        let count = CHANGES.count.lock().expect("changes lock");
        // timer: deadline — the caller's bound; expiry fails the wait below
        let (count, _) = CHANGES
            .changed
            .wait_timeout_while(
                count,
                end.saturating_duration_since(Instant::now()),
                |count| *count == seen,
            )
            .expect("changes wait");
        if *count == seen {
            drop(count);
            assert!(predicate(), "condition did not become true before deadline");
            return;
        }
    }
}

fn admit(
    engine: &PluginWorkerEngine,
    class: PluginInvocationClass,
    request: PluginInvocationRequest,
) -> PluginAdmissionResult {
    engine.admit(class, request, 1)
}

fn engine_admit_with_reservation(
    engine: &PluginWorkerEngine,
    class: PluginInvocationClass,
    request: PluginInvocationRequest,
    completion_reservation_bytes: usize,
) -> PluginAdmissionResult {
    engine.admit(class, request, completion_reservation_bytes)
}

#[test]
fn default_queue_capacity_is_independent_from_executor_concurrency() {
    let engine = probed(PluginWorkerEngineConfig::default());
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
    let defaults = PluginWorkerEngineConfig::default();
    assert_eq!(defaults.completion_reservation_byte_capacity, 1024 * 1024);
    assert_eq!(defaults.completion_queue_byte_capacity, 1024 * 1024);
    assert!(std::panic::catch_unwind(|| {
        probed(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 0,
            per_plugin_executor_concurrency: 2,
            ..PluginWorkerEngineConfig::default()
        })
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        probed(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 1,
            per_plugin_executor_concurrency: 0,
            ..PluginWorkerEngineConfig::default()
        })
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        probed(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 1,
            per_plugin_executor_concurrency: 2,
            reserved_request_response_executors: 0,
            ..PluginWorkerEngineConfig::default()
        })
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        probed(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 1,
            per_plugin_executor_concurrency: 2,
            reserved_request_response_executors: 2,
            ..PluginWorkerEngineConfig::default()
        })
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        probed(PluginWorkerEngineConfig {
            completion_reservation_byte_capacity: 0,
            ..PluginWorkerEngineConfig::default()
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
    let (slow_runtime, slow_starts) = GatedRuntime::with_start_events();
    let (probe, queued) = PluginQueueProbe::channel();
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 4,
        per_plugin_executor_concurrency: 2,
        test_queue_probe: Some(probe),
        ..PluginWorkerEngineConfig::default()
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
    for index in 0..6 {
        let caller_engine = engine.clone();
        let caller_handler = slow_handler.clone();
        callers.push(std::thread::spawn(move || {
            caller_engine.invoke(invocation(
                &format!("queued-{index}"),
                caller_handler,
                2_000,
            ))
        }));
        if index < 2 {
            // Both executors take their job before the next caller arrives,
            // so the four later callers fill the queue exactly.
            // timer: deadline — the slow job must start; expiry fails the test
            slow_starts
                .recv_timeout(HANG_GUARD)
                .expect("a slow job starts on each executor");
        }
    }
    // All six slow jobs were queued by the engine: two run (gated), four wait.
    let deadline = Instant::now() + HANG_GUARD;
    let mut slow_queued = 0;
    while slow_queued < 6 {
        // timer: deadline — the engine must queue every slow job within the bound
        match queued
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the engine queues every slow job")
        {
            PluginQueueProbeEvent::JobQueued { plugin_key, .. } if plugin_key == slow_plugin => {
                slow_queued += 1;
            }
            _ => {}
        }
    }
    let snapshot = engine.debug_snapshot();
    assert_eq!((snapshot.queued_jobs, snapshot.in_flight_jobs), (4, 2));

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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 4,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
                HANG_GUARD.as_millis() as u64,
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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    let mut actives = Vec::new();
    for index in 0..2 {
        let active_engine = engine.clone();
        let active_handler = command.clone();
        actives.push(std::thread::spawn(move || {
            active_engine.invoke(invocation(
                &format!("active-{index}"),
                active_handler,
                2_000,
            ))
        }));
        let expected = index + 1;
        wait_until(Duration::from_secs(2), || {
            runtime.started() == expected && engine.debug_snapshot().in_flight_jobs == expected
        });
    }

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
    for active in actives {
        active.join().expect("active caller should join");
    }
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().queued_jobs == 0
    });
    assert_eq!(runtime.started(), 2);
}

#[test]
fn repeated_load_unload_cycles_join_workers_and_return_debug_counts_to_zero() {
    let plugin = plugin_key("reloadable");
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    // Retirement stops the runtime and joins the idle executor; the busy
    // one stays until the runtime is released.
    wait_until(Duration::from_millis(250), || {
        runtime.stop_called() && engine.debug_snapshot().live_executor_workers == 1
    });

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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 256,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let runtime = FakeRuntime::slow();
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
    });

    let old_runtime = FakeRuntime::ignores_cancellation_until_released();
    engine.load_plugin(registration(
        &plugin,
        old_runtime.clone(),
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
    // The reload stopped the old runtime, which released its held work: its
    // stale result came back while the old generation retired.
    assert!(old_runtime.stopped().contains(&plugin));

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
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let engine = probed(PluginWorkerEngineConfig::default());

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
    let engine = probed(PluginWorkerEngineConfig::default());
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
    let engine = probed(PluginWorkerEngineConfig::default());
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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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

    let mut actives = Vec::new();
    for index in 0..2 {
        let active_engine = engine.clone();
        let active_handler = handler_a.clone();
        actives.push(std::thread::spawn(move || {
            active_engine.invoke(invocation(
                &format!("req-a-active-{index}"),
                active_handler,
                2_000,
            ))
        }));
        let expected = index + 1;
        wait_until(Duration::from_secs(2), || {
            runtime_a.started() == expected && engine.debug_snapshot().in_flight_jobs == expected
        });
    }
    let queued_engine = engine.clone();
    let queued_handler = handler_a.clone();
    let queued = std::thread::spawn(move || {
        queued_engine.invoke(invocation("req-a-queued", queued_handler, 2_000))
    });
    wait_until(Duration::from_secs(2), || {
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
    for active in actives {
        active.join().expect("active plugin A caller should join");
    }
    queued.join().expect("queued plugin A caller should join");
}

#[test]
fn late_runtime_completion_after_timeout_does_not_double_release_capacity() {
    let plugin = plugin_key("project-pipelines");
    let command = handler(&plugin, "late");
    let runtime = FakeRuntime::ignores_cancellation_until_released();
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
    });

    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
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
    // The runtime ignored the cancellation; its late completion comes now.
    runtime.release();

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
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 2,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
                && engine.debug_snapshot().in_flight_jobs == 0
                && engine.debug_snapshot().queued_jobs == 0
        });
    }
    wait_until(Duration::from_millis(250), || {
        let snapshot = engine.debug_snapshot();
        snapshot.queued_jobs == 0 && snapshot.in_flight_jobs == 0
    });
    let snapshot = engine.debug_snapshot();
    assert_eq!(runtime_a.invocations().len(), 3);
    assert_eq!(snapshot.live_plugin_executors, 2);
    assert_eq!(snapshot.live_executor_workers, 4);
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
    let timeout_engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
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
    let pressure_engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 1,
        per_plugin_executor_concurrency: 2,
        ..PluginWorkerEngineConfig::default()
    });
    pressure_engine.load_plugin(registration(
        &plugin,
        pressure_runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    let mut actives = Vec::new();
    for index in 0..2 {
        let active_engine = pressure_engine.clone();
        let active_handler = command.clone();
        actives.push(std::thread::spawn(move || {
            active_engine.invoke(invocation(
                &format!("req-active-{index}"),
                active_handler,
                2_000,
            ))
        }));
        let expected = index + 1;
        wait_until(Duration::from_secs(2), || {
            pressure_runtime.started() == expected
                && pressure_engine.debug_snapshot().in_flight_jobs == expected
        });
    }
    let queued_engine = pressure_engine.clone();
    let queued_handler = command.clone();
    let queued = std::thread::spawn(move || {
        queued_engine.invoke(invocation("req-queued", queued_handler, 2_000))
    });
    wait_until(Duration::from_secs(1), || {
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
    for active in actives {
        active.join().expect("active caller should join");
    }
    queued.join().expect("queued caller should join");
}

/// Drain until `request` completes, waking on the engine's completion
/// publications (its probe bumps [`CHANGES`]), within 1 s.
fn wait_for_completion(engine: &PluginWorkerEngine, request: &str) -> PluginCompletion {
    let found = std::cell::RefCell::new(None);
    wait_until(Duration::from_secs(1), || {
        if found.borrow().is_some() {
            return true;
        }
        let drain = engine.drain_completions(8, usize::MAX);
        let item = drain
            .completions
            .into_iter()
            .find(|item| match &item.completion.result {
                PluginInvocationResult::Completed(success) => success.request_id.0 == request,
                PluginInvocationResult::Failed(failure) => failure.request_id.0 == request,
            });
        let done = item.is_some();
        if let Some(item) = item {
            *found.borrow_mut() = Some(item.completion);
        }
        done
    });
    found
        .into_inner()
        .unwrap_or_else(|| panic!("did not drain completion for {request}"))
}

#[test]
fn saturated_background_cannot_occupy_reserved_request_response_executor() {
    let plugin = plugin_key("reserved");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        reserved_request_response_executors: 1,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    for index in 0..2 {
        assert!(matches!(
            admit(
                &engine,
                PluginInvocationClass::Background,
                invocation(&format!("bg-{index}"), command.clone(), 2_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
    }
    wait_until(Duration::from_millis(250), || runtime.started() == 1);
    let snapshot = engine.debug_snapshot();
    assert_eq!(snapshot.background_in_flight_jobs, 1);
    assert_eq!(snapshot.background_queued_jobs, 1);
    assert_eq!(snapshot.request_response_in_flight_jobs, 0);

    let rr_engine = engine.clone();
    let rr_handler = command.clone();
    let rr =
        std::thread::spawn(move || rr_engine.invoke(invocation("rr-reserved", rr_handler, 2_000)));
    wait_until(Duration::from_millis(250), || runtime.started() == 2);
    let snapshot = engine.debug_snapshot();
    assert_eq!(snapshot.request_response_in_flight_jobs, 1);
    assert_eq!(snapshot.background_in_flight_jobs, 1);
    assert_eq!(snapshot.background_queued_jobs, 1);

    runtime.release();
    assert!(matches!(
        rr.join().expect("reserved request-response caller").result,
        PluginInvocationResult::Completed(_)
    ));
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().background_queued_jobs == 0
            && engine.debug_snapshot().in_flight_jobs == 0
    });
}

#[test]
fn try_admit_never_waits_on_slow_in_flight_work() {
    let plugin = plugin_key("non-blocking");
    let command = handler(&plugin, "run");
    let runtime = FakeRuntime::slow();
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        runtime,
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("slow", command.clone(), 1_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    let started = std::time::Instant::now();
    loop {
        let call_started = std::time::Instant::now();
        match engine.try_admit(
            PluginInvocationClass::Background,
            invocation("second", command.clone(), 1_000),
            1,
        ) {
            PluginAdmissionResult::Queued { .. } => {
                assert!(call_started.elapsed() < Duration::from_millis(50));
                break;
            }
            PluginAdmissionResult::Backpressured { reason, .. }
                if reason == ADMISSION_LOCK_BUSY =>
            {
                assert!(call_started.elapsed() < Duration::from_millis(50));
                assert!(
                    started.elapsed() < Duration::from_millis(100),
                    "typed admission lock busy persisted"
                );
            }
            other => panic!("expected queued second admission, got {other:?}"),
        }
    }
}

#[test]
fn drain_completions_honors_item_and_byte_caps() {
    let plugin = plugin_key("drain");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("ok"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    for index in 0..3 {
        assert!(matches!(
            admit(
                &engine,
                PluginInvocationClass::Background,
                invocation(&format!("drain-{index}"), command.clone(), 1_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
    }
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().undrained_completions == 3
    });
    let first = engine.drain_completions(1, usize::MAX);
    assert_eq!(first.item_count, 1);
    assert_eq!(first.completions.len(), 1);
    assert_eq!(first.byte_count, first.completions[0].encoded_len);
    assert_eq!(
        first.completions[0].encoded_len,
        serde_json::to_vec(&first.completions[0].completion)
            .expect("completion encodes")
            .len()
    );
    assert!(first.has_remaining);
    assert_eq!(engine.debug_snapshot().undrained_completions, 2);
    let too_small = engine.drain_completions(8, 1);
    assert_eq!(too_small.item_count, 0);
    assert_eq!(too_small.byte_count, 0);
    assert!(too_small.completions.is_empty());
    assert!(too_small.has_remaining);
    assert_eq!(engine.debug_snapshot().undrained_completions, 2);
    let rest = engine.drain_completions(8, usize::MAX);
    assert_eq!(rest.item_count, 2);
    assert_eq!(
        rest.byte_count,
        rest.completions
            .iter()
            .map(|item| item.encoded_len)
            .sum::<usize>()
    );
    assert!(!rest.has_remaining);
}

#[test]
fn admitted_slow_job_times_out_through_engine_deadline_waiter() {
    let plugin = plugin_key("deadline");
    let command = handler(&plugin, "slow");
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::slow(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("engine-timeout", command, 10),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    let completion = wait_for_completion(&engine, "engine-timeout");
    assert!(matches!(
        completion,
        PluginCompletion {
            class: PluginInvocationClass::Background,
            result: PluginInvocationResult::Failed(failure),
        } if failure.kind == PluginInvocationFailureKind::TimedOut && failure.timeout_ms == Some(10)
    ));
}

#[test]
fn completion_reservation_is_one_slot_until_drained() {
    let plugin = plugin_key("reserve");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        completion_queue_capacity: 1,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("first", command.clone(), 2_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("second", command.clone(), 2_000),
        ),
        PluginAdmissionResult::Backpressured { .. }
    ));
    runtime.release();
    let _ = wait_for_completion(&engine, "first");
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("after-drain", command, 1_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn unload_of_open_job_publishes_worker_stopped_and_does_not_rewrite_drained_timeout() {
    let plugin = plugin_key("stopped");
    let command = handler(&plugin, "slow");
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::slow(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("open-job", command.clone(), 5_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-open"),
        plugin_key: plugin.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    let pending = engine.drain_completions(0, usize::MAX);
    assert!(pending.completions.is_empty());
    assert!(
        pending.has_remaining,
        "shutdown migration preserves remainder"
    );
    let stopped = engine.drain_completions(8, usize::MAX);
    assert!(matches!(
        stopped.completions.as_slice(),
        [PluginCompletionItem {
            completion: PluginCompletion {
                result: PluginInvocationResult::Failed(failure),
                ..
            },
            ..
        }] if failure.kind == PluginInvocationFailureKind::WorkerStopped
            && failure.request_id.0 == "open-job"
    ));
    assert!(!stopped.has_remaining);

    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::slow(),
        command.clone(),
        vec![descriptor(&plugin, "slow", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("timeout-then-unload", command, 10),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    let _ = wait_for_completion(&engine, "timeout-then-unload");
    engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-after-timeout"),
        plugin_key: plugin,
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    assert!(engine
        .drain_completions(8, usize::MAX)
        .completions
        .is_empty());
}

#[test]
fn reload_reused_request_id_is_not_sealed_by_prior_generation_deadline() {
    let plugin = plugin_key("reload-same-id");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::slow(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("same", command.clone(), 40),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::slow(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("same", command, 5_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    let first = engine.drain_completions(8, usize::MAX);
    assert!(first.completions.iter().all(|item| {
        matches!(
            item.completion.result,
            PluginInvocationResult::Failed(ref failure)
                if failure.kind == PluginInvocationFailureKind::WorkerStopped
        )
    }));
    // The reload removed the prior generation's 40 ms deadline with it, so it
    // can never seal the reused id: only the new job's deadline remains.
    assert_eq!(engine.debug_snapshot().tracked_deadlines, 1);
    assert!(engine
        .drain_completions(8, usize::MAX)
        .completions
        .is_empty());
}

#[test]
fn large_context_metadata_is_counted_in_class_byte_budget() {
    let plugin = plugin_key("bytes");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        background_queue_byte_capacity: 256,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("ok"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    let mut huge = invocation("huge-meta", command, 1_000);
    huge.payload = BoundaryJson(serde_json::json!({ "ok": true }));
    huge.context.metadata = Some(BoundaryJson(serde_json::json!({
        "blob": "m".repeat(512)
    })));
    assert!(matches!(
        admit(&engine, PluginInvocationClass::Background, huge),
        PluginAdmissionResult::RejectedBudget { .. }
    ));
}

#[test]
fn short_and_long_correlation_fields_reserve_fitting_fallbacks() {
    let plugin = plugin_key("corr");
    let short_handler = handler(&plugin, "s");
    let long_handler = PluginHandlerRef {
        plugin_key: plugin.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "h".repeat(128),
    };
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(PluginWorkerRegistration {
        handlers: vec![
            PluginHandlerRegistration {
                handler: short_handler.clone(),
                required_capability: None,
            },
            PluginHandlerRegistration {
                handler: long_handler.clone(),
                required_capability: None,
            },
        ],
        ..registration(
            &plugin,
            FakeRuntime::success("ok"),
            short_handler.clone(),
            vec![descriptor(&plugin, "s", short_handler.clone())],
            Vec::new(),
            None,
        )
    });

    let short = admit(
        &engine,
        PluginInvocationClass::Background,
        invocation("a", short_handler, 1_000),
    );
    let long = admit(
        &engine,
        PluginInvocationClass::Background,
        invocation(&"r".repeat(128), long_handler, 1_000),
    );
    match (short, long) {
        (
            PluginAdmissionResult::Queued {
                reservation_bytes: short_bytes,
                ..
            },
            PluginAdmissionResult::Queued {
                reservation_bytes: long_bytes,
                ..
            },
        ) => {
            assert!(long_bytes > short_bytes);
        }
        other => panic!("expected queued reservations, got {other:?}"),
    }
}

#[test]
fn oversize_handler_result_uses_prebuilt_compact_failure() {
    let plugin = plugin_key("oversize");
    let command = handler(&plugin, "run");
    let huge = BoundaryJson(serde_json::json!({ "blob": "x".repeat(8_192) }));
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        background_queue_byte_capacity: 2_048,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::new(FakeBehavior::Success(huge)),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    let mut request = invocation("oversize-result", command, 1_000);
    request.payload = BoundaryJson(serde_json::json!({ "tiny": true }));
    assert!(matches!(
        admit(&engine, PluginInvocationClass::Background, request),
        PluginAdmissionResult::Queued { .. }
    ));
    let completion = wait_for_completion(&engine, "oversize-result");
    assert!(matches!(
        completion.result,
        PluginInvocationResult::Failed(failure)
            if failure.kind == PluginInvocationFailureKind::CompletionTooLarge
                && failure.reason == "completion exceeded reserved byte budget"
    ));
}

#[test]
fn explicit_completion_reservation_allows_a_larger_bounded_result() {
    let plugin = plugin_key("explicit-reservation");
    let command = handler(&plugin, "run");
    let result_value = "x".repeat(512);
    let engine = probed(PluginWorkerEngineConfig {
        completion_queue_byte_capacity: 2_048,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success(&result_value),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    let admitted = engine_admit_with_reservation(
        &engine,
        PluginInvocationClass::Background,
        invocation("explicit-reservation", command, 1_000),
        1_024,
    );
    assert!(matches!(
        admitted,
        PluginAdmissionResult::Queued {
            reservation_bytes,
            ..
        } if reservation_bytes == 1_024 + PluginWorkerEngine::completion_reservation_metadata_bytes()
    ));
    let completion = wait_for_completion(&engine, "explicit-reservation");
    assert!(matches!(
        completion.result,
        PluginInvocationResult::Completed(PluginInvocationSuccess {
            payload: Some(BoundaryJson(payload)),
            ..
        }) if payload == serde_json::json!({ "value": result_value })
    ));
}

#[test]
fn completion_metadata_charge_does_not_enlarge_the_payload_allowance() {
    let plugin = plugin_key("payload-only-limit");
    let command = handler(&plugin, "run");
    let value = "x".repeat(512);
    let expected = PluginCompletion {
        class: PluginInvocationClass::Background,
        result: PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request_id("payload-limit"),
            handler: command.clone(),
            payload: Some(BoundaryJson(serde_json::json!({ "value": value }))),
        }),
    };
    let payload_bytes = 512;
    let charged_bytes = payload_bytes + PluginWorkerEngine::completion_reservation_metadata_bytes();
    let encoded_len = serde_json::to_vec(&expected)
        .expect("encoded completion")
        .len();
    assert!(encoded_len > payload_bytes && encoded_len < charged_bytes);
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success(&value),
        command.clone(),
        Vec::new(),
        Vec::new(),
        None,
    ));
    assert!(
        matches!(engine_admit_with_reservation(&engine, PluginInvocationClass::Background,
        invocation("payload-limit", command, 1000), payload_bytes), PluginAdmissionResult::Queued { reservation_bytes, .. } if reservation_bytes == charged_bytes)
    );
    assert!(
        matches!(wait_for_completion(&engine, "payload-limit").result, PluginInvocationResult::Failed(failure)
        if failure.kind == PluginInvocationFailureKind::CompletionTooLarge)
    );
    assert_eq!(engine.debug_snapshot().reserved_completion_bytes, 0);
}

#[test]
fn completion_reservation_requires_a_positive_allowance_and_fits_fallback_overhead() {
    let plugin = plugin_key("positive-reservation");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("unused"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            invocation("zero-reservation", command.clone(), 1_000),
            0,
        ),
        PluginAdmissionResult::RejectedBudget { reason, .. }
            if reason == "completion reservation must be positive"
    ));
    let admitted = engine_admit_with_reservation(
        &engine,
        PluginInvocationClass::Background,
        invocation("fallback-overhead", command, 0),
        1,
    );
    assert!(matches!(
        admitted,
        PluginAdmissionResult::Queued {
            reservation_bytes,
            ..
        } if reservation_bytes > 1
    ));
    let completion = wait_for_completion(&engine, "fallback-overhead");
    assert!(matches!(
        completion.result,
        PluginInvocationResult::Failed(failure)
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
}

#[test]
fn per_completion_ceiling_checks_both_admission_paths() {
    let plugin = plugin_key("per-completion-ceiling");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        completion_reservation_byte_capacity: 1_024,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("small"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("exact-ceiling", command.clone(), 1_000),
            1_024,
        ),
        PluginAdmissionResult::Queued {
            reservation_bytes,
            ..
        } if reservation_bytes == 1_024 + PluginWorkerEngine::completion_reservation_metadata_bytes()
    ));
    let _ = wait_for_completion(&engine, "exact-ceiling");
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("above-ceiling", command, 1_000),
            1_025,
        ),
        PluginAdmissionResult::RejectedBudget { reason, .. }
            if reason == "completion reservation exceeds per-completion byte capacity"
    ));

    let missing = PluginHandlerRef {
        plugin_key: plugin,
        kind: PluginHandlerKind::Command,
        handler_id: "missing".to_string(),
    };
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("immediate-above-ceiling", missing, 1_000),
            1_025,
        ),
        PluginAdmissionResult::RejectedBudget { reason, .. }
            if reason == "completion reservation exceeds per-completion byte capacity"
    ));
}

#[test]
fn immediate_failure_rejects_a_request_above_the_class_byte_capacity() {
    let plugin = plugin_key("immediate-class-capacity");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        background_queue_byte_capacity: 512,
        completion_reservation_byte_capacity: 4_096,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("unused"),
        command,
        Vec::new(),
        Vec::new(),
        None,
    ));
    let missing = PluginHandlerRef {
        plugin_key: plugin,
        kind: PluginHandlerKind::Command,
        handler_id: "missing".to_string(),
    };
    let mut request = invocation("oversized-immediate", missing, 1_000);
    request.payload = BoundaryJson(serde_json::json!({ "blob": "x".repeat(512) }));

    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            request,
            1,
        ),
        PluginAdmissionResult::RejectedBudget {
            queue_bytes: Some(queue_bytes),
            reason,
            ..
        } if queue_bytes > 512 && reason == "plugin invocation exceeds class byte capacity"
    ));
}

#[test]
fn request_overhead_above_per_completion_capacity_is_permanently_rejected() {
    let plugin = plugin_key("request-overhead");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        completion_reservation_byte_capacity: 512,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("unused"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    let mut request = invocation("large-request-overhead", command, 1_000);
    request.payload = BoundaryJson(serde_json::json!({ "blob": "x".repeat(512) }));

    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            request,
            1,
        ),
        PluginAdmissionResult::RejectedBudget {
            queue_bytes: Some(queue_bytes),
            reason,
            ..
        } if queue_bytes > 512
            && reason == "completion reservation exceeds per-completion byte capacity"
    ));
}

#[test]
fn explicit_completion_reservation_enforces_capacity_and_drain_lifetime() {
    let plugin = plugin_key("explicit-capacity");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = probed(PluginWorkerEngineConfig {
        per_plugin_queue_capacity: 8,
        per_plugin_executor_concurrency: 2,
        completion_queue_byte_capacity: 1_500
            + PluginWorkerEngine::completion_reservation_metadata_bytes(),
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));

    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("too-large", command.clone(), 1_000),
            1_501,
        ),
        PluginAdmissionResult::RejectedBudget { reason, .. }
            if reason == "completion reservation exceeds engine completion pool byte capacity"
    ));
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("first", command.clone(), 1_000),
            900,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("second", command.clone(), 1_000),
            900,
        ),
        PluginAdmissionResult::Backpressured { .. }
    ));

    runtime.release();
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().undrained_completions == 1
    });
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("before-drain", command.clone(), 1_000),
            900,
        ),
        PluginAdmissionResult::Backpressured { .. }
    ));
    let _ = wait_for_completion(&engine, "first");
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("after-drain", command, 1_000),
            900,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn repeated_reload_keeps_retired_completion_count_charged_until_drain() {
    let plugin = plugin_key("reload-count-reservation");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        completion_queue_capacity: 1,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("first"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("retired-count", command.clone(), 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().undrained_completions == 1
    });
    let charged = engine.debug_snapshot();
    assert_eq!(charged.reserved_completion_count, 1);
    assert!(charged.reserved_completion_bytes < 4_096);

    for generation in 2..=3 {
        engine.reload_plugin(
            PluginReloadSpec {
                request_id: request_id(&format!("reload-count-{generation}")),
                plugin_key: plugin.clone(),
                load: load_spec(&plugin, vec![descriptor(&plugin, "run", command.clone())]),
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            },
            registration(
                &plugin,
                FakeRuntime::success("next"),
                command.clone(),
                vec![descriptor(&plugin, "run", command.clone())],
                Vec::new(),
                None,
            ),
        );
        assert!(matches!(
            engine_admit_with_reservation(
                &engine,
                PluginInvocationClass::Background,
                invocation(
                    &format!("blocked-count-{generation}"),
                    command.clone(),
                    1_000,
                ),
                512,
            ),
            PluginAdmissionResult::Backpressured { reason, .. }
                if reason == "plugin completion reservation pool is at capacity"
        ));
    }

    let _ = wait_for_completion(&engine, "retired-count");
    let drained = engine.debug_snapshot();
    assert_eq!(drained.reserved_completion_count, 0);
    assert_eq!(drained.reserved_completion_bytes, 0);
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("released-count", command, 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn repeated_reload_keeps_retired_completion_bytes_charged_until_drain() {
    let plugin = plugin_key("reload-byte-reservation");
    let command = handler(&plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        completion_queue_capacity: 8,
        completion_queue_byte_capacity: 1_500
            + PluginWorkerEngine::completion_reservation_metadata_bytes(),
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("first"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("retired-bytes", command.clone(), 1_000),
            900,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().undrained_completions == 1
    });
    let charged = engine.debug_snapshot();
    assert!(charged.reserved_completion_count < 8);
    assert_eq!(
        charged.reserved_completion_bytes,
        900 + PluginWorkerEngine::completion_reservation_metadata_bytes()
    );

    for generation in 2..=3 {
        engine.reload_plugin(
            PluginReloadSpec {
                request_id: request_id(&format!("reload-bytes-{generation}")),
                plugin_key: plugin.clone(),
                load: load_spec(&plugin, vec![descriptor(&plugin, "run", command.clone())]),
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            },
            registration(
                &plugin,
                FakeRuntime::success("next"),
                command.clone(),
                vec![descriptor(&plugin, "run", command.clone())],
                Vec::new(),
                None,
            ),
        );
        assert!(matches!(
            engine_admit_with_reservation(
                &engine,
                PluginInvocationClass::Background,
                invocation(
                    &format!("blocked-bytes-{generation}"),
                    command.clone(),
                    1_000,
                ),
                900,
            ),
            PluginAdmissionResult::Backpressured { .. }
        ));
    }

    let _ = wait_for_completion(&engine, "retired-bytes");
    let drained = engine.debug_snapshot();
    assert_eq!(drained.reserved_completion_count, 0);
    assert_eq!(drained.reserved_completion_bytes, 0);
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("released-bytes", command, 1_000),
            900,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn active_plugins_share_one_engine_wide_completion_reservation() {
    let first_plugin = plugin_key("shared-reservation-first");
    let first_command = handler(&first_plugin, "run");
    let second_plugin = plugin_key("shared-reservation-second");
    let second_command = handler(&second_plugin, "run");
    let engine = probed(PluginWorkerEngineConfig {
        completion_queue_capacity: 1,
        completion_queue_byte_capacity: 4_096,
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &first_plugin,
        FakeRuntime::success("first"),
        first_command.clone(),
        vec![descriptor(&first_plugin, "run", first_command.clone())],
        Vec::new(),
        None,
    ));
    engine.load_plugin(registration(
        &second_plugin,
        FakeRuntime::success("second"),
        second_command.clone(),
        vec![descriptor(&second_plugin, "run", second_command.clone())],
        Vec::new(),
        None,
    ));

    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("shared-first", first_command, 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("shared-blocked", second_command.clone(), 1_000),
            512,
        ),
        PluginAdmissionResult::Backpressured { .. }
    ));

    let _ = wait_for_completion(&engine, "shared-first");
    let drained = engine.debug_snapshot();
    assert_eq!(drained.reserved_completion_count, 0);
    assert_eq!(drained.reserved_completion_bytes, 0);
    assert!(matches!(
        engine_admit_with_reservation(
            &engine,
            PluginInvocationClass::Background,
            invocation("shared-released", second_command, 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn immediate_failure_uses_the_engine_wide_completion_reservation() {
    let plugin = plugin_key("immediate-reservation");
    let command = handler(&plugin, "run");
    let required = network_capability();
    let engine = probed(PluginWorkerEngineConfig {
        completion_queue_capacity: 1,
        completion_queue_byte_capacity: 1_024
            + PluginWorkerEngine::completion_reservation_metadata_bytes(),
        ..PluginWorkerEngineConfig::default()
    });
    engine.load_plugin(registration(
        &plugin,
        FakeRuntime::success("unused"),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        Some(required.clone()),
    ));
    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            invocation("immediate-first", command.clone(), 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));

    engine.reload_plugin(
        PluginReloadSpec {
            request_id: request_id("reload-immediate"),
            plugin_key: plugin.clone(),
            load: load_spec(&plugin, vec![descriptor(&plugin, "run", command.clone())]),
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        },
        registration(
            &plugin,
            FakeRuntime::success("unused"),
            command.clone(),
            vec![descriptor(&plugin, "run", command.clone())],
            Vec::new(),
            Some(required),
        ),
    );
    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            invocation("immediate-blocked", command.clone(), 1_000),
            512,
        ),
        PluginAdmissionResult::Backpressured { .. }
    ));

    let completion = wait_for_completion(&engine, "immediate-first");
    assert!(matches!(
        completion.result,
        PluginInvocationResult::Failed(failure)
            if failure.kind == PluginInvocationFailureKind::HandlerFailed
                && failure.reason.contains("capability")
    ));
    let drained = engine.debug_snapshot();
    assert_eq!(drained.reserved_completion_count, 0);
    assert_eq!(drained.reserved_completion_bytes, 0);
    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            invocation("immediate-released", command, 1_000),
            512,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn debug_snapshot_reports_live_class_fields() {
    let plugin = plugin_key("snap");
    let command = handler(&plugin, "run");
    let runtime = GatedRuntime::default();
    let engine = probed(PluginWorkerEngineConfig::default());
    engine.load_plugin(registration(
        &plugin,
        runtime.clone(),
        command.clone(),
        vec![descriptor(&plugin, "run", command.clone())],
        Vec::new(),
        None,
    ));
    assert!(matches!(
        admit(
            &engine,
            PluginInvocationClass::Background,
            invocation("snap-bg", command, 2_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_until(Duration::from_millis(250), || {
        engine.debug_snapshot().background_in_flight_jobs == 1
    });
    let snapshot = engine.debug_snapshot();
    assert_eq!(snapshot.configured_reserved_request_response_executors, 1);
    assert_eq!(snapshot.configured_background_queue_capacity, 256);
    assert_eq!(snapshot.reserved_completion_count, 1);
    assert_eq!(snapshot.plugins[0].background_in_flight_jobs, 1);
    assert_eq!(snapshot.plugins[0].reserved_request_response_executors, 1);
    runtime.release();
}

//! Plugin timer scheduler acceptance tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use botster_core::{
    BotsterEngine, BoundaryJson, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime,
    PackageManifest, PluginCancellationToken, PluginCleanupScope, PluginDescriptorKind,
    PluginDescriptorRef, PluginHandlerKind, PluginHandlerRef, PluginHandlerRegistration,
    PluginInvocationContext, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginInvocationSuccess, PluginKey, PluginLoadSpec,
    PluginOwnedDescriptor, PluginResourceKind, PluginResourceRef, PluginRuntime, PluginTimerEvent,
    PluginTimerId, PluginTimerMode, PluginTimerSchedule, PluginUnloadSpec,
    PluginWorkerRegistration, RequestId,
};
use botster_core_test_support::fake::{
    FakePluginBehavior, FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
};

#[derive(Clone, Default)]
struct OccupiedRuntime {
    started: Arc<AtomicUsize>,
    invocations: Arc<AtomicUsize>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl OccupiedRuntime {
    fn release(&self) {
        let (released, condition) = &*self.gate;
        *released.lock().expect("occupied runtime release lock") = true;
        condition.notify_all();
    }

    fn invocation_count(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }

    fn started(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }
}

impl PluginRuntime for OccupiedRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        self.started.fetch_add(1, Ordering::SeqCst);
        let (released, condition) = &*self.gate;
        let mut guard = released.lock().expect("occupied runtime wait lock");
        while !*guard && !cancellation.is_cancelled() {
            guard = match condition.wait_timeout(guard, Duration::from_millis(10)) {
                Ok((inner, _)) => inner,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: Some(BoundaryJson(serde_json::json!({ "value": "occupied" }))),
        })
    }
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn plugin_key(value: &str) -> PluginKey {
    PluginKey(value.to_string())
}

fn timer_id(value: &str) -> PluginTimerId {
    PluginTimerId(value.to_string())
}

fn timer_handler(plugin_key: &PluginKey, handler_id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Timer,
        handler_id: handler_id.to_string(),
    }
}

fn command_handler(plugin_key: &PluginKey, handler_id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: handler_id.to_string(),
    }
}

fn manifest(plugin_key: &PluginKey) -> PackageManifest {
    PackageManifest {
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
    }
}

fn registration(
    runtime: impl PluginRuntime,
    plugin_key: &PluginKey,
    handler: &PluginHandlerRef,
) -> PluginWorkerRegistration {
    PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: plugin_key.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![PluginOwnedDescriptor {
                descriptor: PluginDescriptorRef {
                    plugin_key: plugin_key.clone(),
                    kind: PluginDescriptorKind::Timer,
                    descriptor_id: handler.handler_id.clone(),
                },
                handler: Some(handler.clone()),
                body: BoundaryJson(serde_json::json!({ "title": "Timer" })),
            }],
            metadata: None,
        },
        manifest: manifest(plugin_key),
        runtime: Arc::new(runtime),
        handlers: vec![PluginHandlerRegistration {
            handler: handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    }
}

fn timer_schedule(
    request_id_value: &str,
    timer_id_value: &str,
    handler: PluginHandlerRef,
    due_at_ms: u64,
    mode: PluginTimerMode,
    payload_value: &str,
) -> PluginTimerSchedule {
    PluginTimerSchedule {
        request_id: request_id(request_id_value),
        timer_id: timer_id(timer_id_value),
        handler,
        due_at_ms,
        mode,
        timeout_ms: 25,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("plugin-timer-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "value": payload_value })),
    }
}

fn plugin_invocation(
    request: &str,
    handler: PluginHandlerRef,
    timeout_ms: u64,
) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: request_id(request),
        handler,
        timeout_ms,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("plugin-timer-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "value": "occupy" })),
    }
}

fn engine() -> BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> {
    BotsterEngine::with_plugin_config(
        FakeSessionRuntime::new(),
        botster_core::PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 1,
            per_plugin_executor_concurrency: 2,
            ..botster_core::PluginWorkerEngineConfig::default()
        },
    )
}

fn wait_for_snapshot(
    engine: &BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime>,
    predicate: impl Fn(&botster_core::PluginWorkerDebugSnapshot) -> bool,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if predicate(&engine.plugin_workers().debug_snapshot()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!(
        "plugin worker snapshot never satisfied predicate: {:?}",
        engine.plugin_workers().debug_snapshot()
    );
}

#[test]
fn one_shot_timer_fires_through_plugin_worker_timer_handler() {
    let engine = engine();
    let plugin = plugin_key("timer-plugin");
    let handler = timer_handler(&plugin, "on_timer");
    let runtime = FakePluginRuntime::success("ok");
    engine.load_plugin(registration(runtime.clone(), &plugin, &handler));

    engine.schedule_plugin_timer(timer_schedule(
        "schedule-one-shot",
        "timer-one",
        handler,
        10,
        PluginTimerMode::OneShot,
        "first",
    ));
    let outcome = engine.drain_plugin_timers_due(10);

    assert_eq!(runtime.invocations().len(), 1);
    let invocation = &runtime.invocations()[0];
    assert_eq!(invocation.handler.kind, PluginHandlerKind::Timer);
    assert_eq!(invocation.payload.0["value"], "first");
    assert!(matches!(
        outcome.events.as_slice(),
        [PluginTimerEvent::Fired {
            result: PluginInvocationResult::Completed(_),
            ..
        }]
    ));
}

#[test]
fn timer_schedule_path_does_not_block_on_slow_plugin_handler() {
    let engine = engine();
    let plugin = plugin_key("slow-plugin");
    let handler = timer_handler(&plugin, "slow_timer");
    let runtime = FakePluginRuntime::delayed(Duration::from_millis(100));
    engine.load_plugin(registration(runtime, &plugin, &handler));

    let started = std::time::Instant::now();
    engine.schedule_plugin_timer(timer_schedule(
        "schedule-slow",
        "timer-slow",
        handler,
        10,
        PluginTimerMode::OneShot,
        "slow",
    ));

    assert!(started.elapsed() < Duration::from_millis(50));
}

#[test]
fn timer_cancellation_prevents_pending_delivery() {
    let engine = engine();
    let plugin = plugin_key("cancel-plugin");
    let handler = timer_handler(&plugin, "timer");
    let runtime = FakePluginRuntime::success("ok");
    engine.load_plugin(registration(runtime.clone(), &plugin, &handler));
    let id = timer_id("timer-cancelled");

    engine.schedule_plugin_timer(timer_schedule(
        "schedule-cancel",
        &id.0,
        handler,
        10,
        PluginTimerMode::OneShot,
        "cancelled",
    ));
    let cancellation = engine.cancel_plugin_timer(request_id("cancel"), &plugin, &id);
    let outcome = engine.drain_plugin_timers_due(10);

    assert!(cancellation.cancelled);
    assert_eq!(
        cancellation.removed_resource,
        Some(PluginResourceRef {
            plugin_key: plugin,
            kind: PluginResourceKind::Timer,
            resource_id: "timer-cancelled".to_string(),
        })
    );
    assert!(runtime.invocations().is_empty());
    assert!(outcome.events.is_empty());
}

#[test]
fn timer_schedule_rejects_non_timer_handler_without_panicking() {
    let engine = engine();
    let plugin = plugin_key("wrong-handler-plugin");
    let handler = command_handler(&plugin, "not_timer");

    let outcome = engine.schedule_plugin_timer(timer_schedule(
        "schedule-wrong-kind",
        "wrong-kind",
        handler,
        10,
        PluginTimerMode::OneShot,
        "wrong",
    ));

    assert!(matches!(
        outcome.events.as_slice(),
        [PluginTimerEvent::Rejected {
            timer_id,
            plugin_key,
            ..
        }] if timer_id.0 == "wrong-kind" && plugin_key.0 == "wrong-handler-plugin"
    ));
    assert!(engine.drain_plugin_timers_due(10).events.is_empty());
}

#[test]
fn debounce_replaces_prior_pending_timer_for_same_plugin_key() {
    let engine = engine();
    let plugin = plugin_key("debounce-plugin");
    let other_plugin = plugin_key("other-debounce-plugin");
    let handler = timer_handler(&plugin, "timer");
    let other_handler = timer_handler(&other_plugin, "timer");
    let runtime = FakePluginRuntime::success("ok");
    let other_runtime = FakePluginRuntime::success("other");
    engine.load_plugin(registration(runtime.clone(), &plugin, &handler));
    engine.load_plugin(registration(
        other_runtime.clone(),
        &other_plugin,
        &other_handler,
    ));

    engine.schedule_plugin_timer(timer_schedule(
        "debounce-a",
        "debounce-old",
        handler.clone(),
        10,
        PluginTimerMode::Debounce {
            key: "refresh".to_string(),
        },
        "old",
    ));
    engine.schedule_plugin_timer(timer_schedule(
        "debounce-b",
        "debounce-new",
        handler,
        10,
        PluginTimerMode::Debounce {
            key: "refresh".to_string(),
        },
        "new",
    ));
    engine.schedule_plugin_timer(timer_schedule(
        "debounce-other",
        "debounce-other",
        other_handler,
        10,
        PluginTimerMode::Debounce {
            key: "refresh".to_string(),
        },
        "other",
    ));
    engine.drain_plugin_timers_due(10);

    assert_eq!(runtime.invocations().len(), 1);
    assert_eq!(runtime.invocations()[0].payload.0["value"], "new");
    assert_eq!(other_runtime.invocations().len(), 1);
    assert_eq!(other_runtime.invocations()[0].payload.0["value"], "other");
}

#[test]
fn interval_timer_coalesces_under_worker_pressure() {
    let engine = engine();
    let plugin = plugin_key("interval-plugin");
    let handler = timer_handler(&plugin, "timer");
    let runtime = FakePluginRuntime::new(FakePluginBehavior::WaitForCancellation);
    engine.load_plugin(registration(runtime.clone(), &plugin, &handler));

    engine.schedule_plugin_timer(timer_schedule(
        "interval",
        "interval-timer",
        handler,
        10,
        PluginTimerMode::Interval { interval_ms: 10 },
        "tick",
    ));
    let first = engine.drain_plugin_timers_due(10);
    let second = engine.drain_plugin_timers_due(50);

    assert!(runtime.invocations().len() <= 2);
    assert!(matches!(
        first.events.as_slice(),
        [PluginTimerEvent::Fired { result: PluginInvocationResult::Failed(failure), .. }]
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
    assert!(second.events.iter().any(|event| matches!(
        event,
        PluginTimerEvent::Coalesced {
            timer_id,
            skipped_ticks,
            ..
        } if timer_id.0 == "interval-timer" && *skipped_ticks > 0
    )));
}

#[test]
fn interval_timer_retries_after_timeout() {
    let engine = engine();
    let plugin = plugin_key("timeout-recovery-plugin");
    let handler = timer_handler(&plugin, "timer");
    let runtime = FakePluginRuntime::new(FakePluginBehavior::WaitForCancellation);
    engine.load_plugin(registration(runtime.clone(), &plugin, &handler));

    engine.schedule_plugin_timer(timer_schedule(
        "interval-timeout",
        "interval-timeout-timer",
        handler,
        10,
        PluginTimerMode::Interval { interval_ms: 10 },
        "tick",
    ));
    let first = engine.drain_plugin_timers_due(10);
    for _ in 0..50 {
        if runtime.cancellations_observed() >= 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let second = engine.drain_plugin_timers_due(20);
    for _ in 0..50 {
        if runtime.cancellations_observed() >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(runtime.cancellations_observed(), 2);
    assert_eq!(runtime.invocations().len(), 2);
    assert!(matches!(
        first.events.as_slice(),
        [PluginTimerEvent::Fired { result: PluginInvocationResult::Failed(failure), .. }]
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
    assert!(matches!(
        second.events.as_slice(),
        [PluginTimerEvent::Fired { result: PluginInvocationResult::Failed(failure), .. }]
            if failure.kind == PluginInvocationFailureKind::TimedOut
    ));
}

#[test]
fn interval_timer_retries_after_backpressure() {
    let engine = engine();
    let plugin = plugin_key("backpressure-recovery-plugin");
    let handler = timer_handler(&plugin, "timer");
    let occupy = OccupiedRuntime::default();
    engine.load_plugin(registration(occupy.clone(), &plugin, &handler));

    let mut occupied = Vec::new();
    for index in 0..2 {
        let occupied_engine = engine.clone();
        let occupied_handler = handler.clone();
        occupied.push(std::thread::spawn(move || {
            occupied_engine.invoke_plugin(plugin_invocation(
                &format!("occupy-worker-{index}"),
                occupied_handler,
                5_000,
            ))
        }));
        let expected = index + 1;
        wait_for_snapshot(&engine, |debug| {
            debug.in_flight_jobs == expected && debug.queued_jobs == 0
        });
        assert_eq!(occupy.started(), expected);
    }

    let queued_engine = engine.clone();
    let queued_handler = handler.clone();
    let queued = std::thread::spawn(move || {
        queued_engine.invoke_plugin(plugin_invocation(
            "occupy-worker-queued",
            queued_handler,
            5_000,
        ))
    });
    wait_for_snapshot(&engine, |debug| {
        debug.in_flight_jobs == 2 && debug.queued_jobs == 1
    });

    engine.schedule_plugin_timer(timer_schedule(
        "interval-backpressure",
        "interval-backpressure-timer",
        handler.clone(),
        10,
        PluginTimerMode::Interval { interval_ms: 10 },
        "tick",
    ));
    let pressured = engine.drain_plugin_timers_due(10);
    occupy.release();
    for handle in occupied {
        handle.join().expect("join occupied worker");
    }
    queued.join().expect("join queued worker");
    let recovered = engine.drain_plugin_timers_due(20);

    assert!(pressured.events.iter().any(|event| matches!(
        event,
        PluginTimerEvent::Backpressured { timer_id, .. }
            if timer_id.0 == "interval-backpressure-timer"
    )));
    assert_eq!(occupy.invocation_count(), 4);
    assert!(matches!(
        recovered.events.as_slice(),
        [PluginTimerEvent::Fired { .. }]
    ));
}

#[test]
fn plugin_unload_cleans_owned_timers_only() {
    let engine = engine();
    let plugin_a = plugin_key("plugin-a");
    let plugin_b = plugin_key("plugin-b");
    let handler_a = timer_handler(&plugin_a, "timer");
    let handler_b = timer_handler(&plugin_b, "timer");
    let runtime_a = FakePluginRuntime::success("a");
    let runtime_b = FakePluginRuntime::success("b");
    engine.load_plugin(registration(runtime_a.clone(), &plugin_a, &handler_a));
    engine.load_plugin(registration(runtime_b.clone(), &plugin_b, &handler_b));

    engine.schedule_plugin_timer(timer_schedule(
        "schedule-a",
        "timer-a",
        handler_a,
        10,
        PluginTimerMode::OneShot,
        "a",
    ));
    engine.schedule_plugin_timer(timer_schedule(
        "schedule-b",
        "timer-b",
        handler_b,
        10,
        PluginTimerMode::OneShot,
        "b",
    ));
    let cleanup = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-a"),
        plugin_key: plugin_a.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    engine.drain_plugin_timers_due(10);

    assert!(cleanup.removed_resources.contains(&PluginResourceRef {
        plugin_key: plugin_a,
        kind: PluginResourceKind::Timer,
        resource_id: "timer-a".to_string(),
    }));
    assert!(runtime_a.invocations().is_empty());
    assert_eq!(runtime_b.invocations().len(), 1);
}

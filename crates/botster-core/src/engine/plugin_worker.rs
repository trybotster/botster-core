//! Core plugin worker execution engine.
//!
//! The engine owns reusable worker mechanics: handler lookup, per-plugin
//! capacity accounting, capability checks, deadline attribution, and scoped
//! reload/unload cleanup. Concrete Lua, WASM, or host runtimes implement
//! [`PluginRuntime`] outside core.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginCleanupResult, PluginCleanupScope,
    PluginDescriptorRef, PluginHandlerRef, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec, PluginReloadSpec,
    PluginResourceRef, PluginUnloadSpec, PluginWorkerEvent, QueueSource,
};
use crate::capability::Capability;
use crate::manifest::PackageManifest;
use crate::runtime::{PluginCancellationToken, PluginRuntime};

static NEXT_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Engine-wide worker execution configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWorkerEngineConfig {
    /// Maximum in-flight invocations per plugin worker.
    pub per_plugin_capacity: usize,
}

impl Default for PluginWorkerEngineConfig {
    fn default() -> Self {
        Self {
            per_plugin_capacity: QueueSource::PluginWorker.default_capacity(),
        }
    }
}

/// Handler metadata registered for one plugin worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHandlerRegistration {
    /// Stable handler address.
    pub handler: PluginHandlerRef,
    /// Capability this handler requires, checked against the package manifest.
    pub required_capability: Option<Capability>,
}

/// Plugin worker metadata registered with the engine.
#[derive(Clone)]
pub struct PluginWorkerRegistration {
    /// Load metadata and descriptors owned by this plugin.
    pub load: PluginLoadSpec,
    /// Package metadata declaring capabilities granted to this plugin.
    pub manifest: PackageManifest,
    /// Executable runtime supplied by the host.
    pub runtime: Arc<dyn PluginRuntime>,
    /// Stable handlers that may be invoked through this worker.
    pub handlers: Vec<PluginHandlerRegistration>,
    /// Runtime resources owned by this worker and removed during cleanup.
    pub resources: Vec<PluginResourceRef>,
}

/// Result plus typed worker events observed while invoking a plugin handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInvocationOutcome {
    /// Caller-facing invocation result.
    pub result: PluginInvocationResult,
    /// Typed worker events produced by the invoke path.
    pub events: Vec<PluginWorkerEvent>,
}

impl PluginInvocationOutcome {
    fn new(result: PluginInvocationResult) -> Self {
        Self {
            result,
            events: Vec::new(),
        }
    }

    fn with_event(result: PluginInvocationResult, event: PluginWorkerEvent) -> Self {
        Self {
            result,
            events: vec![event],
        }
    }
}

/// Reusable plugin worker execution engine.
#[derive(Clone, Default)]
pub struct PluginWorkerEngine {
    config: PluginWorkerEngineConfig,
    workers: Arc<Mutex<HashMap<PluginKey, WorkerState>>>,
}

impl PluginWorkerEngine {
    /// Create a new engine with default queue capacity.
    pub fn new() -> Self {
        Self::with_config(PluginWorkerEngineConfig::default())
    }

    /// Create a new engine with explicit configuration.
    pub fn with_config(config: PluginWorkerEngineConfig) -> Self {
        Self {
            config,
            workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        let plugin_key = registration.load.plugin_key.clone();
        let worker = WorkerState::new(registration, self.config.per_plugin_capacity);

        let previous = self
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .insert(plugin_key.clone(), worker);

        if let Some(previous) = previous {
            previous.cancel_all_in_flight();
            previous.runtime.stop(&plugin_key);
        }
    }

    /// Invoke a stable plugin handler through its owning runtime.
    pub fn invoke(&self, request: PluginInvocationRequest) -> PluginInvocationOutcome {
        let worker = match self.worker_for(&request.handler.plugin_key) {
            Some(worker) => worker,
            None => return worker_stopped(request, "plugin worker is not loaded"),
        };

        let handler = match worker.handlers.get(&request.handler).cloned() {
            Some(handler) => handler,
            None => return handler_failed(request, "plugin handler is not registered"),
        };

        if let Some(required_capability) = &handler.required_capability {
            if !worker.manifest.capabilities.contains(required_capability) {
                return handler_failed(
                    request,
                    "plugin handler requires a capability missing from package metadata",
                );
            }
        }

        if worker.depth() >= self.config.per_plugin_capacity {
            let summary = self.backpressure_for(&request.handler.plugin_key);
            let failure = PluginInvocationFailure {
                request_id: request.request_id,
                handler: request.handler,
                kind: PluginInvocationFailureKind::Backpressured,
                timeout_ms: None,
                reason: "plugin worker queue is at capacity".to_string(),
            };
            return PluginInvocationOutcome::with_event(
                PluginInvocationResult::Failed(failure),
                PluginWorkerEvent::Backpressure(summary),
            );
        }

        let timeout_ms = request.timeout_ms;
        let timeout_request_id = request.request_id.clone();
        let timeout_handler = request.handler.clone();
        let invocation_key = request.request_id.clone();
        let (sender, receiver) = mpsc::channel();
        let cancellation = PluginCancellationToken::new();
        worker.track_invocation(invocation_key.clone(), cancellation.clone());

        if let Err(error) = worker.dispatch(WorkerJob {
            request,
            cancellation: cancellation.clone(),
            result_sender: sender,
        }) {
            worker.finish_invocation(&invocation_key);
            let failure = PluginInvocationFailure {
                request_id: timeout_request_id,
                handler: timeout_handler,
                kind: if error.worker_stopped {
                    PluginInvocationFailureKind::WorkerStopped
                } else {
                    PluginInvocationFailureKind::Backpressured
                },
                timeout_ms: None,
                reason: if error.worker_stopped {
                    "plugin worker stopped before accepting invocation"
                } else {
                    "plugin worker queue is at capacity"
                }
                .to_string(),
            };
            let result = PluginInvocationResult::Failed(failure);
            return if error.worker_stopped {
                PluginInvocationOutcome::new(result)
            } else {
                PluginInvocationOutcome::with_event(
                    result,
                    PluginWorkerEvent::Backpressure(self.backpressure_for(&error.plugin_key)),
                )
            };
        }

        match receiver.recv_timeout(Duration::from_millis(timeout_ms)) {
            Ok(result) => PluginInvocationOutcome::new(result),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cancellation.cancel();
                let failure = PluginInvocationFailure {
                    request_id: timeout_request_id,
                    handler: timeout_handler,
                    kind: PluginInvocationFailureKind::TimedOut,
                    timeout_ms: Some(timeout_ms),
                    reason: "plugin handler exceeded timeout".to_string(),
                };
                PluginInvocationOutcome::with_event(
                    PluginInvocationResult::Failed(failure.clone()),
                    PluginWorkerEvent::InvocationTimedOut(failure),
                )
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => PluginInvocationOutcome::new(
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: timeout_request_id,
                    handler: timeout_handler,
                    kind: PluginInvocationFailureKind::WorkerStopped,
                    timeout_ms: None,
                    reason: "plugin runtime stopped before completing invocation".to_string(),
                }),
            ),
        }
    }

    /// Reload one plugin, replacing only that plugin's worker-owned state.
    pub fn reload_plugin(
        &self,
        spec: PluginReloadSpec,
        mut registration: PluginWorkerRegistration,
    ) -> PluginCleanupResult {
        registration.load = spec.load.clone();
        let cleanup = self.cleanup_plugin(spec.request_id, &spec.plugin_key, spec.cleanup, true);
        self.load_plugin(registration);
        cleanup
    }

    /// Unload one plugin worker and remove only its owned descriptors/resources.
    pub fn unload_plugin(&self, spec: PluginUnloadSpec) -> PluginCleanupResult {
        self.cleanup_plugin(spec.request_id, &spec.plugin_key, spec.cleanup, true)
    }

    /// Record an additional runtime resource owned by one plugin.
    pub fn record_resource(&self, resource: PluginResourceRef) {
        if let Some(worker) = self.worker_for(&resource.plugin_key) {
            worker
                .resources
                .lock()
                .expect("plugin worker resources mutex poisoned")
                .push(resource);
        }
    }

    /// Return descriptors currently owned by one plugin.
    pub fn descriptors_for(&self, plugin_key: &PluginKey) -> Vec<PluginDescriptorRef> {
        self.worker_for(plugin_key)
            .map(|worker| worker.descriptors.clone())
            .unwrap_or_default()
    }

    /// Return a backpressure summary for one plugin worker.
    pub fn backpressure_for(&self, plugin_key: &PluginKey) -> BackpressureSummary {
        let depth = self
            .worker_for(plugin_key)
            .map(|worker| worker.depth())
            .unwrap_or_default();

        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: self.config.per_plugin_capacity,
            depth,
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(plugin_key.clone()),
            },
        }
    }

    fn cleanup_plugin(
        &self,
        request_id: crate::session::RequestId,
        plugin_key: &PluginKey,
        scope: PluginCleanupScope,
        stop_runtime: bool,
    ) -> PluginCleanupResult {
        let worker = self
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .remove(plugin_key);

        let Some(worker) = worker else {
            return PluginCleanupResult {
                request_id,
                plugin_key: plugin_key.clone(),
                removed_descriptors: Vec::new(),
                removed_resources: Vec::new(),
            };
        };

        if stop_runtime {
            worker.cancel_all_in_flight();
            worker.runtime.stop(plugin_key);
        }

        let removed_descriptors = match scope {
            PluginCleanupScope::Descriptors | PluginCleanupScope::DescriptorsAndResources => {
                worker.descriptors
            }
            PluginCleanupScope::Resources => Vec::new(),
        };
        let removed_resources = match scope {
            PluginCleanupScope::Resources | PluginCleanupScope::DescriptorsAndResources => worker
                .resources
                .lock()
                .expect("plugin worker resources mutex poisoned")
                .clone(),
            PluginCleanupScope::Descriptors => Vec::new(),
        };

        PluginCleanupResult {
            request_id,
            plugin_key: plugin_key.clone(),
            removed_descriptors,
            removed_resources,
        }
    }

    fn worker_for(&self, plugin_key: &PluginKey) -> Option<WorkerState> {
        self.workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .get(plugin_key)
            .cloned()
    }
}

#[derive(Clone)]
struct WorkerState {
    manifest: PackageManifest,
    runtime: Arc<dyn PluginRuntime>,
    handlers: HashMap<PluginHandlerRef, PluginHandlerRegistration>,
    descriptors: Vec<PluginDescriptorRef>,
    resources: Arc<Mutex<Vec<PluginResourceRef>>>,
    slots: Arc<Vec<WorkerSlot>>,
    in_flight: Arc<Mutex<HashMap<crate::session::RequestId, PluginCancellationToken>>>,
}

impl WorkerState {
    fn new(registration: PluginWorkerRegistration, capacity: usize) -> Self {
        let descriptors = registration
            .load
            .descriptors
            .iter()
            .map(|descriptor| descriptor.descriptor.clone())
            .collect();
        let handlers = registration
            .handlers
            .into_iter()
            .map(|handler| (handler.handler.clone(), handler))
            .collect();
        let runtime = registration.runtime;
        let in_flight = Arc::new(Mutex::new(HashMap::new()));
        let generation = NEXT_WORKER_GENERATION.fetch_add(1, Ordering::SeqCst);
        let mut slots = Vec::with_capacity(capacity);

        for slot_index in 0..capacity {
            let (sender, receiver) = mpsc::channel::<WorkerJob>();
            let idle = Arc::new(AtomicBool::new(true));
            let slot_idle = idle.clone();
            let slot_runtime = runtime.clone();
            let slot_in_flight = in_flight.clone();
            std::thread::Builder::new()
                .name(format!("botster-plugin-worker-{generation}-{slot_index}"))
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        let request_id = job.request.request_id.clone();
                        let result = slot_runtime.invoke(job.request, job.cancellation);
                        slot_in_flight
                            .lock()
                            .expect("plugin worker in-flight mutex poisoned")
                            .remove(&request_id);
                        slot_idle.store(true, Ordering::SeqCst);
                        let _ = job.result_sender.send(result);
                    }
                })
                .expect("spawn plugin worker thread");
            slots.push(WorkerSlot { sender, idle });
        }

        Self {
            manifest: registration.manifest,
            runtime,
            handlers,
            descriptors,
            resources: Arc::new(Mutex::new(registration.resources)),
            slots: Arc::new(slots),
            in_flight,
        }
    }

    fn dispatch(&self, job: WorkerJob) -> Result<(), WorkerDispatchError> {
        let plugin_key = job.request.handler.plugin_key.clone();

        for slot in self.slots.iter() {
            if slot
                .idle
                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                if slot.sender.send(job).is_ok() {
                    return Ok(());
                }
                slot.idle.store(true, Ordering::SeqCst);
                return Err(WorkerDispatchError {
                    worker_stopped: true,
                    plugin_key,
                });
            }
        }

        Err(WorkerDispatchError {
            worker_stopped: false,
            plugin_key,
        })
    }

    fn track_invocation(
        &self,
        request_id: crate::session::RequestId,
        cancellation: PluginCancellationToken,
    ) {
        self.in_flight
            .lock()
            .expect("plugin worker in-flight mutex poisoned")
            .insert(request_id, cancellation);
    }

    fn finish_invocation(&self, request_id: &crate::session::RequestId) {
        self.in_flight
            .lock()
            .expect("plugin worker in-flight mutex poisoned")
            .remove(request_id);
    }

    fn cancel_all_in_flight(&self) {
        let tokens = self
            .in_flight
            .lock()
            .expect("plugin worker in-flight mutex poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for token in tokens {
            token.cancel();
        }
    }

    fn depth(&self) -> usize {
        self.in_flight
            .lock()
            .expect("plugin worker in-flight mutex poisoned")
            .len()
    }
}

struct WorkerSlot {
    sender: mpsc::Sender<WorkerJob>,
    idle: Arc<AtomicBool>,
}

struct WorkerJob {
    request: PluginInvocationRequest,
    cancellation: PluginCancellationToken,
    result_sender: mpsc::Sender<PluginInvocationResult>,
}

struct WorkerDispatchError {
    worker_stopped: bool,
    plugin_key: PluginKey,
}

fn worker_stopped(request: PluginInvocationRequest, reason: &str) -> PluginInvocationOutcome {
    PluginInvocationOutcome::new(PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id,
        handler: request.handler,
        kind: PluginInvocationFailureKind::WorkerStopped,
        timeout_ms: None,
        reason: reason.to_string(),
    }))
}

fn handler_failed(request: PluginInvocationRequest, reason: &str) -> PluginInvocationOutcome {
    PluginInvocationOutcome::new(PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id,
        handler: request.handler,
        kind: PluginInvocationFailureKind::HandlerFailed,
        timeout_ms: None,
        reason: reason.to_string(),
    }))
}

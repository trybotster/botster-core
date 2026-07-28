//! Core plugin worker execution engine.
//!
//! The engine owns reusable worker mechanics: handler lookup, per-plugin
//! capacity accounting, capability checks, deadline attribution, and scoped
//! reload/unload cleanup. Concrete Lua, WASM, or host runtimes implement
//! [`PluginRuntime`] outside core.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
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
    /// Maximum number of waiting invocations per plugin worker.
    pub per_plugin_queue_capacity: usize,
    /// Maximum number of concurrently executing invocations per plugin worker.
    pub per_plugin_executor_concurrency: usize,
}

impl Default for PluginWorkerEngineConfig {
    fn default() -> Self {
        Self {
            per_plugin_queue_capacity: QueueSource::PluginWorker.default_capacity(),
            per_plugin_executor_concurrency: 2,
        }
    }
}

/// Observable state for one loaded plugin executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWorkerPluginDebugSnapshot {
    /// Stable plugin identity.
    pub plugin_key: PluginKey,
    /// Executor workers that have not yet retired.
    pub live_executor_workers: usize,
    /// Invocations waiting for an executor worker.
    pub queued_jobs: usize,
    /// Invocations currently executing in the host runtime.
    pub in_flight_jobs: usize,
}

/// Aggregate observable state for the plugin worker engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWorkerDebugSnapshot {
    /// Configured waiting-job capacity for every plugin queue.
    pub configured_queue_capacity: usize,
    /// Configured executor width for every loaded plugin.
    pub configured_executor_concurrency: usize,
    /// Loaded plugin executors.
    pub live_plugin_executors: usize,
    /// Executor workers that have not yet retired.
    pub live_executor_workers: usize,
    /// Invocations waiting across all plugin queues.
    pub queued_jobs: usize,
    /// Invocations currently executing across all plugin runtimes.
    pub in_flight_jobs: usize,
    /// Per-plugin rows sorted by plugin key.
    pub plugins: Vec<PluginWorkerPluginDebugSnapshot>,
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
#[derive(Clone)]
pub struct PluginWorkerEngine {
    inner: Arc<PluginWorkerEngineInner>,
}

struct PluginWorkerEngineInner {
    config: PluginWorkerEngineConfig,
    workers: Mutex<HashMap<PluginKey, WorkerState>>,
}

impl Drop for PluginWorkerEngineInner {
    fn drop(&mut self) {
        let workers = self
            .workers
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain()
            .map(|(_, worker)| worker)
            .collect::<Vec<_>>();
        for worker in workers {
            worker.shutdown();
        }
    }
}

impl Default for PluginWorkerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginWorkerEngine {
    /// Create a new engine with default queue and executor settings.
    pub fn new() -> Self {
        Self::with_config(PluginWorkerEngineConfig::default())
    }

    /// Create a new engine with explicit configuration.
    pub fn with_config(config: PluginWorkerEngineConfig) -> Self {
        assert!(
            config.per_plugin_queue_capacity > 0,
            "plugin worker queue capacity must be greater than zero"
        );
        assert!(
            config.per_plugin_executor_concurrency > 0,
            "plugin worker executor concurrency must be greater than zero"
        );
        Self {
            inner: Arc::new(PluginWorkerEngineInner {
                config,
                workers: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        let plugin_key = registration.load.plugin_key.clone();
        let worker = WorkerState::new(
            registration,
            self.inner.config.per_plugin_queue_capacity,
            self.inner.config.per_plugin_executor_concurrency,
        );

        let previous = self
            .inner
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .insert(plugin_key, worker);

        if let Some(previous) = previous {
            previous.shutdown();
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
            .map(|worker| worker.queued_jobs())
            .unwrap_or_default();

        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: self.inner.config.per_plugin_queue_capacity,
            depth,
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(plugin_key.clone()),
            },
        }
    }

    /// Return aggregate and per-plugin executor/queue counters.
    #[must_use]
    pub fn debug_snapshot(&self) -> PluginWorkerDebugSnapshot {
        let workers = self
            .inner
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned");
        let mut plugins = workers
            .values()
            .map(WorkerState::debug_snapshot)
            .collect::<Vec<_>>();
        plugins.sort_by(|left, right| left.plugin_key.0.cmp(&right.plugin_key.0));

        PluginWorkerDebugSnapshot {
            configured_queue_capacity: self.inner.config.per_plugin_queue_capacity,
            configured_executor_concurrency: self.inner.config.per_plugin_executor_concurrency,
            live_plugin_executors: plugins.len(),
            live_executor_workers: plugins
                .iter()
                .map(|plugin| plugin.live_executor_workers)
                .sum(),
            queued_jobs: plugins.iter().map(|plugin| plugin.queued_jobs).sum(),
            in_flight_jobs: plugins.iter().map(|plugin| plugin.in_flight_jobs).sum(),
            plugins,
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
            .inner
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
            worker.shutdown();
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
        self.inner
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .get(plugin_key)
            .cloned()
    }
}

#[derive(Clone)]
struct WorkerState {
    plugin_key: PluginKey,
    manifest: PackageManifest,
    runtime: Arc<dyn PluginRuntime>,
    handlers: HashMap<PluginHandlerRef, PluginHandlerRegistration>,
    descriptors: Vec<PluginDescriptorRef>,
    resources: Arc<Mutex<Vec<PluginResourceRef>>>,
    executor: Arc<WorkerExecutor>,
}

impl WorkerState {
    fn new(
        registration: PluginWorkerRegistration,
        queue_capacity: usize,
        executor_concurrency: usize,
    ) -> Self {
        let plugin_key = registration.load.plugin_key.clone();
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
        let cancellations = Arc::new(Mutex::new(HashMap::new()));
        let metrics = Arc::new(WorkerMetrics {
            live_workers: AtomicUsize::new(executor_concurrency),
            queued_jobs: AtomicUsize::new(0),
            in_flight_jobs: AtomicUsize::new(0),
        });
        let (sender, receiver) = mpsc::sync_channel::<WorkerJob>(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let generation = NEXT_WORKER_GENERATION.fetch_add(1, Ordering::SeqCst);
        let mut join_handles = Vec::with_capacity(executor_concurrency);

        for worker_index in 0..executor_concurrency {
            let worker_receiver = receiver.clone();
            let worker_runtime = runtime.clone();
            let worker_cancellations = cancellations.clone();
            let worker_metrics = metrics.clone();
            let join_handle = std::thread::Builder::new()
                .name(format!("botster-plugin-worker-{generation}-{worker_index}"))
                .spawn(move || {
                    let _liveness = WorkerLivenessGuard {
                        metrics: worker_metrics.clone(),
                    };
                    loop {
                        let job = worker_receiver
                            .lock()
                            .expect("plugin worker receiver mutex poisoned")
                            .recv();
                        let Ok(job) = job else {
                            break;
                        };
                        let request_id = job.request.request_id.clone();
                        worker_metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
                        if job.cancellation.is_cancelled() {
                            worker_cancellations
                                .lock()
                                .expect("plugin worker cancellations mutex poisoned")
                                .remove(&request_id);
                            continue;
                        }
                        worker_metrics.in_flight_jobs.fetch_add(1, Ordering::SeqCst);
                        let in_flight = InFlightGuard {
                            metrics: worker_metrics.clone(),
                            cancellations: worker_cancellations.clone(),
                            request_id,
                        };
                        let result = worker_runtime.invoke(job.request, job.cancellation);
                        let _ = job.result_sender.send(result);
                        drop(in_flight);
                    }
                })
                .expect("spawn plugin worker thread");
            join_handles.push(join_handle);
        }

        Self {
            plugin_key,
            manifest: registration.manifest,
            runtime,
            handlers,
            descriptors,
            resources: Arc::new(Mutex::new(registration.resources)),
            executor: Arc::new(WorkerExecutor {
                sender: Mutex::new(Some(sender)),
                join_handles: Mutex::new(Some(join_handles)),
                stopping: AtomicBool::new(false),
                cancellations,
                metrics,
            }),
        }
    }

    fn dispatch(&self, job: WorkerJob) -> Result<(), WorkerDispatchError> {
        let plugin_key = job.request.handler.plugin_key.clone();
        let sender = self
            .executor
            .sender
            .lock()
            .expect("plugin worker sender mutex poisoned");
        if self.executor.stopping.load(Ordering::SeqCst) {
            return Err(WorkerDispatchError {
                worker_stopped: true,
                plugin_key,
            });
        }

        let Some(sender) = sender.as_ref() else {
            return Err(WorkerDispatchError {
                worker_stopped: true,
                plugin_key,
            });
        };

        self.executor
            .metrics
            .queued_jobs
            .fetch_add(1, Ordering::SeqCst);
        match sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.executor
                    .metrics
                    .queued_jobs
                    .fetch_sub(1, Ordering::SeqCst);
                Err(WorkerDispatchError {
                    worker_stopped: matches!(error, mpsc::TrySendError::Disconnected(_)),
                    plugin_key,
                })
            }
        }
    }

    fn track_invocation(
        &self,
        request_id: crate::session::RequestId,
        cancellation: PluginCancellationToken,
    ) {
        self.executor
            .cancellations
            .lock()
            .expect("plugin worker cancellations mutex poisoned")
            .insert(request_id, cancellation);
    }

    fn finish_invocation(&self, request_id: &crate::session::RequestId) {
        self.executor
            .cancellations
            .lock()
            .expect("plugin worker cancellations mutex poisoned")
            .remove(request_id);
    }

    fn shutdown(&self) {
        if self.executor.stopping.swap(true, Ordering::SeqCst) {
            return;
        }

        self.executor
            .sender
            .lock()
            .expect("plugin worker sender mutex poisoned")
            .take();
        let tokens = self
            .executor
            .cancellations
            .lock()
            .expect("plugin worker cancellations mutex poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for token in tokens {
            token.cancel();
        }
        self.runtime.stop(&self.plugin_key);

        let join_handles = self
            .executor
            .join_handles
            .lock()
            .expect("plugin worker join handles mutex poisoned")
            .take()
            .unwrap_or_default();
        for join_handle in join_handles {
            let _ = join_handle.join();
        }
    }

    fn queued_jobs(&self) -> usize {
        self.executor.metrics.queued_jobs.load(Ordering::SeqCst)
    }

    fn debug_snapshot(&self) -> PluginWorkerPluginDebugSnapshot {
        PluginWorkerPluginDebugSnapshot {
            plugin_key: self.plugin_key.clone(),
            live_executor_workers: self.executor.metrics.live_workers.load(Ordering::SeqCst),
            queued_jobs: self.executor.metrics.queued_jobs.load(Ordering::SeqCst),
            in_flight_jobs: self.executor.metrics.in_flight_jobs.load(Ordering::SeqCst),
        }
    }
}

struct WorkerExecutor {
    sender: Mutex<Option<mpsc::SyncSender<WorkerJob>>>,
    join_handles: Mutex<Option<Vec<JoinHandle<()>>>>,
    stopping: AtomicBool,
    cancellations: Arc<Mutex<HashMap<crate::session::RequestId, PluginCancellationToken>>>,
    metrics: Arc<WorkerMetrics>,
}

struct WorkerMetrics {
    live_workers: AtomicUsize,
    queued_jobs: AtomicUsize,
    in_flight_jobs: AtomicUsize,
}

struct WorkerLivenessGuard {
    metrics: Arc<WorkerMetrics>,
}

impl Drop for WorkerLivenessGuard {
    fn drop(&mut self) {
        self.metrics.live_workers.fetch_sub(1, Ordering::SeqCst);
    }
}

struct InFlightGuard {
    metrics: Arc<WorkerMetrics>,
    cancellations: Arc<Mutex<HashMap<crate::session::RequestId, PluginCancellationToken>>>,
    request_id: crate::session::RequestId,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.metrics.in_flight_jobs.fetch_sub(1, Ordering::SeqCst);
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.request_id);
    }
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

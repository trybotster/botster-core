//! Core plugin worker execution engine.
//!
//! The engine owns reusable worker mechanics: handler lookup, per-plugin
//! capacity accounting, capability checks, deadline attribution, and scoped
//! reload/unload cleanup. Concrete Lua, WASM, or host runtimes implement
//! [`PluginRuntime`] outside core.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginCleanupResult, PluginCleanupScope,
    PluginDescriptorRef, PluginHandlerRef, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec, PluginReloadSpec,
    PluginResourceRef, PluginUnloadSpec, QueueSource,
};
use crate::capability::Capability;
use crate::manifest::PackageManifest;
use crate::runtime::PluginRuntime;

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
        let worker = WorkerState::new(registration);

        self.workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .insert(plugin_key, worker);
    }

    /// Invoke a stable plugin handler through its owning runtime.
    pub fn invoke(&self, request: PluginInvocationRequest) -> PluginInvocationResult {
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

        if worker.in_flight.load(Ordering::SeqCst) >= self.config.per_plugin_capacity {
            return PluginInvocationResult::Failed(PluginInvocationFailure {
                request_id: request.request_id,
                handler: request.handler,
                kind: PluginInvocationFailureKind::Backpressured,
                timeout_ms: None,
                reason: "plugin worker queue is at capacity".to_string(),
            });
        }

        worker.in_flight.fetch_add(1, Ordering::SeqCst);

        let timeout_ms = request.timeout_ms;
        let timeout_request_id = request.request_id.clone();
        let timeout_handler = request.handler.clone();
        let runtime = worker.runtime.clone();
        let in_flight = worker.in_flight.clone();
        let (sender, receiver) = mpsc::channel();

        std::thread::spawn(move || {
            let result = runtime.invoke(request);
            in_flight.fetch_sub(1, Ordering::SeqCst);
            let _ = sender.send(result);
        });

        match receiver.recv_timeout(Duration::from_millis(timeout_ms)) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: timeout_request_id,
                    handler: timeout_handler,
                    kind: PluginInvocationFailureKind::TimedOut,
                    timeout_ms: Some(timeout_ms),
                    reason: "plugin handler exceeded timeout".to_string(),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: timeout_request_id,
                    handler: timeout_handler,
                    kind: PluginInvocationFailureKind::WorkerStopped,
                    timeout_ms: None,
                    reason: "plugin runtime stopped before completing invocation".to_string(),
                })
            }
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
            .map(|worker| worker.in_flight.load(Ordering::SeqCst))
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
    in_flight: Arc<AtomicUsize>,
}

impl WorkerState {
    fn new(registration: PluginWorkerRegistration) -> Self {
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

        Self {
            manifest: registration.manifest,
            runtime: registration.runtime,
            handlers,
            descriptors,
            resources: Arc::new(Mutex::new(registration.resources)),
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }
}

fn worker_stopped(request: PluginInvocationRequest, reason: &str) -> PluginInvocationResult {
    PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id,
        handler: request.handler,
        kind: PluginInvocationFailureKind::WorkerStopped,
        timeout_ms: None,
        reason: reason.to_string(),
    })
}

fn handler_failed(request: PluginInvocationRequest, reason: &str) -> PluginInvocationResult {
    PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id,
        handler: request.handler,
        kind: PluginInvocationFailureKind::HandlerFailed,
        timeout_ms: None,
        reason: reason.to_string(),
    })
}

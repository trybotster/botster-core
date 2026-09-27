//! Core plugin worker execution engine.
//!
//! The engine owns reusable worker mechanics: handler lookup, per-plugin
//! class-aware admission, capability checks, deadline attribution, reserved
//! request-response executors, and scoped reload/unload cleanup. Concrete Lua,
//! WASM, or host runtimes implement [`PluginRuntime`] outside core.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginAdmissionResult, PluginCleanupResult,
    PluginCleanupScope, PluginCompletion, PluginCompletionDrain, PluginCompletionItem,
    PluginDescriptorRef, PluginHandlerRef, PluginInvocationClass, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult, PluginKey,
    PluginLoadSpec, PluginReloadSpec, PluginResourceRef, PluginUnloadSpec, PluginWorkerEvent,
    QueueSource,
};
use crate::capability::Capability;
use crate::manifest::PackageManifest;
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;

#[path = "plugin_completion_store.rs"]
mod completion_store;
use completion_store::{CompletionReservation, CompletionStore, StoreAdmissionError};

#[path = "plugin_worker_resources.rs"]
mod worker_resources;

#[path = "plugin_delivery_pool.rs"]
mod delivery_pool;
pub use delivery_pool::{
    CallId, DeliveryPool, DeliveryRefusal, PluginDeliveryQuota, PoolOverdraw, UnitReturnedNotifier,
};
pub use worker_resources::{
    PluginWorkerResource, PluginWorkerResourceCountMismatch, PluginWorkerResources,
};
use worker_resources::{WorkerJoinRecord, WorkerMetadataGuard, WorkerResourceConstruction};

static NEXT_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

fn allocate_worker_generation(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |next| {
            next.checked_add(1)
        })
        .ok()
}

const DEFAULT_QUEUE_BYTE_CAPACITY: usize = 1024 * 1024;
const OVERSIZE_COMPLETION_REASON: &str = "completion exceeded reserved byte budget";
const ADMISSION_LOCK_BUSY: &str = "admission lock busy";

/// Host callback invoked after Core publishes an async plugin completion.
pub type PluginCompletionNotifier = Arc<dyn Fn() + Send + Sync + 'static>;

/// Engine-wide worker execution configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWorkerEngineConfig {
    /// Maximum number of waiting RequestResponse invocations per plugin worker.
    pub per_plugin_queue_capacity: usize,
    /// Maximum number of concurrently executing invocations per plugin worker.
    pub per_plugin_executor_concurrency: usize,
    /// Executor slots reserved for RequestResponse work. Must be at least 1
    /// and strictly less than [`Self::per_plugin_executor_concurrency`].
    pub reserved_request_response_executors: usize,
    /// Maximum encoded RequestResponse waiting-queue bytes per plugin.
    pub request_response_queue_byte_capacity: usize,
    /// Maximum number of waiting Background invocations per plugin worker.
    pub background_queue_capacity: usize,
    /// Maximum encoded Background waiting-queue bytes per plugin.
    pub background_queue_byte_capacity: usize,
    /// Maximum reserved async completions across the engine.
    pub completion_queue_capacity: usize,
    /// Maximum encoded payload allowance for one async completion.
    pub completion_reservation_byte_capacity: usize,
    /// Maximum reserved payload and logical completion-store metadata bytes.
    pub completion_queue_byte_capacity: usize,
}

impl Default for PluginWorkerEngineConfig {
    fn default() -> Self {
        Self {
            per_plugin_queue_capacity: QueueSource::PluginWorker.default_capacity(),
            per_plugin_executor_concurrency: 2,
            reserved_request_response_executors: 1,
            request_response_queue_byte_capacity: DEFAULT_QUEUE_BYTE_CAPACITY,
            background_queue_capacity: QueueSource::PluginWorker.default_capacity(),
            background_queue_byte_capacity: DEFAULT_QUEUE_BYTE_CAPACITY,
            completion_queue_capacity: QueueSource::PluginWorker.default_capacity(),
            completion_reservation_byte_capacity: DEFAULT_QUEUE_BYTE_CAPACITY,
            completion_queue_byte_capacity: DEFAULT_QUEUE_BYTE_CAPACITY,
        }
    }
}

impl PluginWorkerEngineConfig {
    fn class_queue_capacity(&self, class: PluginInvocationClass) -> usize {
        if is_background(class) {
            self.background_queue_capacity
        } else {
            self.per_plugin_queue_capacity
        }
    }

    fn class_queue_byte_capacity(&self, class: PluginInvocationClass) -> usize {
        if is_background(class) {
            self.background_queue_byte_capacity
        } else {
            self.request_response_queue_byte_capacity
        }
    }

    fn background_executor_limit(&self) -> usize {
        self.per_plugin_executor_concurrency
            .saturating_sub(self.reserved_request_response_executors)
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
    /// Waiting RequestResponse jobs.
    pub request_response_queued_jobs: usize,
    /// Encoded waiting RequestResponse bytes.
    pub request_response_queued_bytes: usize,
    /// RequestResponse jobs occupying an executor.
    pub request_response_in_flight_jobs: usize,
    /// Waiting Background jobs.
    pub background_queued_jobs: usize,
    /// Encoded waiting Background bytes.
    pub background_queued_bytes: usize,
    /// Background jobs occupying an executor.
    pub background_in_flight_jobs: usize,
    /// Reserved async completion slots, including undrained items.
    pub reserved_completion_count: usize,
    /// Reserved async completion bytes, including undrained items.
    pub reserved_completion_bytes: usize,
    /// Published completions the host has not drained.
    pub undrained_completions: usize,
    /// Configured reserved RequestResponse executor slots.
    pub reserved_request_response_executors: usize,
    /// Whether the RequestResponse waiting queue is at its count or byte bound.
    pub request_response_saturated: bool,
    /// Whether the Background waiting queue is at its count or byte bound.
    pub background_saturated: bool,
    /// Times RequestResponse admission returned backpressure.
    pub request_response_pressure_events: usize,
    /// Times Background admission returned backpressure.
    pub background_pressure_events: usize,
    /// Times admission was refused because the completion pool was full.
    pub completion_pressure_events: usize,
}

/// Aggregate observable state for the plugin worker engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWorkerDebugSnapshot {
    /// Configured waiting-job capacity for every RequestResponse queue.
    pub configured_queue_capacity: usize,
    /// Configured executor width for every loaded plugin.
    pub configured_executor_concurrency: usize,
    /// Configured RequestResponse executor reservation.
    pub configured_reserved_request_response_executors: usize,
    /// Configured RequestResponse waiting-queue byte capacity.
    pub configured_request_response_queue_byte_capacity: usize,
    /// Configured Background waiting-job capacity.
    pub configured_background_queue_capacity: usize,
    /// Configured Background waiting-queue byte capacity.
    pub configured_background_queue_byte_capacity: usize,
    /// Configured engine-wide completion reservation count.
    pub configured_completion_queue_capacity: usize,
    /// Configured engine-wide completion reservation byte capacity.
    pub configured_completion_queue_byte_capacity: usize,
    /// Loaded or retiring plugin executors.
    pub live_plugin_executors: usize,
    /// Executor workers that have not yet retired, including removed generations.
    pub live_executor_workers: usize,
    /// Invocations waiting across active and retiring plugin queues.
    pub queued_jobs: usize,
    /// Invocations currently executing across active and retiring plugin runtimes.
    pub in_flight_jobs: usize,
    /// Waiting RequestResponse jobs across live and retiring workers.
    pub request_response_queued_jobs: usize,
    /// Encoded waiting RequestResponse bytes across live and retiring workers.
    pub request_response_queued_bytes: usize,
    /// RequestResponse jobs occupying an executor.
    pub request_response_in_flight_jobs: usize,
    /// Waiting Background jobs across live and retiring workers.
    pub background_queued_jobs: usize,
    /// Encoded waiting Background bytes across live and retiring workers.
    pub background_queued_bytes: usize,
    /// Background jobs occupying an executor.
    pub background_in_flight_jobs: usize,
    /// Reserved async completion slots across active and retired generations.
    pub reserved_completion_count: usize,
    /// Reserved async completion bytes across active and retired generations.
    pub reserved_completion_bytes: usize,
    /// Published completions the host has not drained.
    pub undrained_completions: usize,
    /// Whether any RequestResponse queue is at its count or byte bound.
    pub request_response_saturated: bool,
    /// Whether any Background queue is at its count or byte bound.
    pub background_saturated: bool,
    /// Whether the engine-wide completion pool is at its count or byte bound.
    pub completions_saturated: bool,
    /// Times RequestResponse admission returned backpressure.
    pub request_response_pressure_events: usize,
    /// Times Background admission returned backpressure.
    pub background_pressure_events: usize,
    /// Times admission was refused because a completion pool was full.
    pub completion_pressure_events: usize,
    /// Currently registered per-plugin rows sorted by plugin key.
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
    shared: Arc<EngineShared>,
    waiter: Mutex<Option<JoinHandle<()>>>,
}

struct EngineShared {
    config: PluginWorkerEngineConfig,
    workers: Mutex<HashMap<PluginKey, WorkerState>>,
    completions: Mutex<CompletionStore>,
    completion_notifier: Mutex<Option<PluginCompletionNotifier>>,
    metrics: Arc<PluginWorkerEngineMetrics>,
    deadlines: Mutex<DeadlineBook>,
    deadline_cvar: Condvar,
    stopping: AtomicBool,
    /// Live delivery pools, by id, for returning drained units.
    pools: Mutex<HashMap<u64, std::sync::Weak<delivery_pool::PoolInner>>>,
    next_pool: AtomicU64,
    #[cfg(test)]
    publication_pause: Mutex<Option<Arc<PublicationPause>>>,
    #[cfg(test)]
    deadline_permits: Mutex<Option<Arc<DeadlinePermits>>>,
}

/// Test seam: when installed, the deadline waiter takes the permit of a
/// request before it fires that request's due deadline, so a test can order a
/// runtime's subscription before the deadline's delivery without changing
/// the deadline value.
#[cfg(test)]
#[derive(Default)]
struct DeadlinePermits {
    granted: Mutex<std::collections::HashSet<RequestId>>,
    changed: Condvar,
}

#[cfg(test)]
impl DeadlinePermits {
    fn grant(&self, request_id: RequestId) {
        self.granted
            .lock()
            .expect("deadline permits")
            .insert(request_id);
        self.changed.notify_all();
    }

    fn take(&self, request_id: &RequestId) {
        let mut granted = self.granted.lock().expect("deadline permits");
        while !granted.remove(request_id) {
            granted = self.changed.wait(granted).expect("deadline permits");
        }
    }
}

#[cfg(test)]
struct PublicationPause {
    generation: u64,
    sealed: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
}

/// Completion lock order:
///
/// - Admission and retirement lock worker admission before the completion store.
/// - Completion publication and drain lock only the store, never admission.
/// - The store owns every count/byte reservation and generation routing row.
/// - Host completion callbacks run after every state guard has been dropped.
#[derive(Default)]
struct CompletionReservationPool {
    reserved_count: usize,
    reserved_bytes: usize,
}

impl CompletionReservationPool {
    fn is_at_capacity(&self, reservation_bytes: usize, config: &PluginWorkerEngineConfig) -> bool {
        self.reserved_count
            .checked_add(1)
            .is_none_or(|count| count > config.completion_queue_capacity)
            || self
                .reserved_bytes
                .checked_add(reservation_bytes)
                .is_none_or(|bytes| bytes > config.completion_queue_byte_capacity)
    }

    fn reserve(&mut self, reservation_bytes: usize) {
        self.reserved_count += 1;
        self.reserved_bytes += reservation_bytes;
    }

    fn release(&mut self, reservation_bytes: usize) {
        self.release_many(1, reservation_bytes);
    }

    /// Whether `count` more entries of `bytes` in total still fit.
    fn fits_many(&self, count: usize, bytes: usize, config: &PluginWorkerEngineConfig) -> bool {
        self.reserved_count
            .checked_add(count)
            .is_some_and(|total| total <= config.completion_queue_capacity)
            && self
                .reserved_bytes
                .checked_add(bytes)
                .is_some_and(|total| total <= config.completion_queue_byte_capacity)
    }

    fn reserve_many(&mut self, count: usize, bytes: usize) {
        self.reserved_count += count;
        self.reserved_bytes += bytes;
    }

    fn release_many(&mut self, count: usize, bytes: usize) {
        self.reserved_count = self
            .reserved_count
            .checked_sub(count)
            .expect("completion reservation count released exactly once");
        self.reserved_bytes = self
            .reserved_bytes
            .checked_sub(bytes)
            .expect("completion reservation bytes released exactly once");
    }
}

#[derive(Default)]
struct DeadlineBook {
    entries: Vec<DeadlineEntry>,
}

#[derive(Clone)]
struct DeadlineEntry {
    at: Instant,
    plugin_key: PluginKey,
    generation: u64,
    request_id: RequestId,
}

impl Drop for PluginWorkerEngineInner {
    fn drop(&mut self) {
        {
            let _book = self
                .shared
                .deadlines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.shared.stopping.store(true, Ordering::SeqCst);
            self.shared.deadline_cvar.notify_all();
        }
        if let Some(handle) = self
            .waiter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = handle.join();
        }
        let workers = self
            .shared
            .workers
            .lock()
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
    /// Fixed conservative logical metadata allowance charged for each async
    /// completion in addition to its encoded payload allowance. This includes
    /// its slot, generation routing row, ready-front entry, FIFO links, and
    /// publication handles; shared rows are conservatively charged per slot.
    /// Allocator metadata, B-tree node padding and spare capacity are excluded.
    #[must_use]
    pub const fn completion_reservation_metadata_bytes() -> usize {
        completion_store::metadata_bytes()
    }

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
        assert!(
            config.reserved_request_response_executors > 0,
            "reserved request-response executors must be greater than zero"
        );
        assert!(
            config.reserved_request_response_executors < config.per_plugin_executor_concurrency,
            "reserved request-response executors must be less than executor concurrency"
        );
        assert!(
            config.request_response_queue_byte_capacity > 0,
            "request-response queue byte capacity must be greater than zero"
        );
        assert!(
            config.background_queue_capacity > 0,
            "background queue capacity must be greater than zero"
        );
        assert!(
            config.background_queue_byte_capacity > 0,
            "background queue byte capacity must be greater than zero"
        );
        assert!(
            config.completion_queue_capacity > 0,
            "completion queue capacity must be greater than zero"
        );
        assert!(
            config.completion_reservation_byte_capacity > 0,
            "completion reservation byte capacity must be greater than zero"
        );
        assert!(
            config.completion_queue_byte_capacity > 0,
            "completion queue byte capacity must be greater than zero"
        );

        let shared = Arc::new(EngineShared {
            config,
            workers: Mutex::new(HashMap::new()),
            completions: Mutex::new(CompletionStore::default()),
            completion_notifier: Mutex::new(None),
            metrics: Arc::new(PluginWorkerEngineMetrics::default()),
            deadlines: Mutex::new(DeadlineBook::default()),
            deadline_cvar: Condvar::new(),
            stopping: AtomicBool::new(false),
            pools: Mutex::new(HashMap::new()),
            next_pool: AtomicU64::new(1),
            #[cfg(test)]
            publication_pause: Mutex::new(None),
            #[cfg(test)]
            deadline_permits: Mutex::new(None),
        });
        let waiter_shared = shared.clone();
        let waiter = std::thread::Builder::new()
            .name("botster-plugin-deadline-waiter".to_string())
            .spawn(move || run_deadline_waiter(waiter_shared))
            .expect("spawn plugin deadline waiter");
        Self {
            inner: Arc::new(PluginWorkerEngineInner {
                shared,
                waiter: Mutex::new(Some(waiter)),
            }),
        }
    }

    /// Install the callback invoked after Core publishes a completion.
    ///
    /// Hosts should install the callback before they admit async plugin work.
    /// Installation also signals completions that Core already published.
    /// The callback must return promptly and must not panic.
    pub fn install_completion_notifier(&self, notifier: PluginCompletionNotifier) {
        *self
            .inner
            .shared
            .completion_notifier
            .lock()
            .expect("plugin completion notifier mutex poisoned") = Some(notifier.clone());
        if self
            .inner
            .shared
            .metrics
            .undrained_completions
            .load(Ordering::SeqCst)
            > 0
        {
            notifier();
        }
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        self.load_plugin_inner(registration, None);
    }

    /// Load or replace a plugin with one host-funded resource per worker.
    ///
    /// The host must fund every resource before calling this method. The count
    /// must equal `per_plugin_executor_concurrency`. A mismatch starts no worker,
    /// changes no registration, and drops all supplied resources normally.
    ///
    /// Each started worker keeps its resource until its exact join returns
    /// `Ok` or `Err`. Losing an unjoined record retains that resource until
    /// process exit, including construction, registration, and shutdown unwinds.
    /// A returned spawn failure releases only its never-started resource and
    /// unused resources. Existing callers can continue to use `load_plugin`.
    /// The existing panic on worker spawn failure remains unchanged. Resources
    /// for previously started, unjoined workers stay retained during that unwind.
    /// Batch metadata stays guarded until the input buffer, join buffer, and
    /// executor allocation are destroyed. Executor clones can outlive unload.
    pub fn load_plugin_with_worker_resources(
        &self,
        registration: PluginWorkerRegistration,
        resources: PluginWorkerResources,
    ) -> Result<(), PluginWorkerResourceCountMismatch> {
        let expected = self.inner.shared.config.per_plugin_executor_concurrency;
        if resources.len() != expected {
            return Err(PluginWorkerResourceCountMismatch {
                expected,
                actual: resources.len(),
            });
        }
        self.load_plugin_inner(registration, Some(resources));
        Ok(())
    }

    fn load_plugin_inner(
        &self,
        registration: PluginWorkerRegistration,
        resources: Option<PluginWorkerResources>,
    ) {
        let plugin_key = registration.load.plugin_key.clone();
        let worker = WorkerState::new(registration, self.inner.shared.clone(), resources);

        self.install_worker(plugin_key, worker);
    }

    fn install_worker(&self, plugin_key: PluginKey, worker: WorkerState) {
        let previous = self
            .inner
            .shared
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .insert(plugin_key, worker);

        if let Some(previous) = previous {
            previous.shutdown();
        }
    }

    /// Invoke a stable plugin handler through its owning runtime.
    ///
    /// This is the blocking RequestResponse compatibility path. It does not
    /// reserve a completion-mailbox slot and does not use the engine deadline
    /// waiter; the caller `recv_timeout` remains the timeout owner.
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

        let queue_bytes = match plugin_invocation_queue_bytes(&request) {
            Ok(bytes) => bytes,
            Err(_) => {
                worker.finish_invocation(&invocation_key);
                return self.invoke_backpressured(
                    request,
                    "plugin invocation request could not be encoded",
                );
            }
        };
        if queue_bytes
            > self
                .inner
                .shared
                .config
                .request_response_queue_byte_capacity
        {
            worker.finish_invocation(&invocation_key);
            return self.invoke_backpressured(request, "plugin worker queue is at capacity");
        }

        let mut admission = worker
            .admission
            .lock()
            .expect("plugin worker admission mutex poisoned");
        if admission.stopping || worker.executor.stopping.load(Ordering::SeqCst) {
            drop(admission);
            worker.finish_invocation(&invocation_key);
            return worker_stopped(request, "plugin worker stopped before accepting invocation");
        }
        if admission.rr_queue.len() >= self.inner.shared.config.per_plugin_queue_capacity
            || admission.rr_queued_bytes + queue_bytes
                > self
                    .inner
                    .shared
                    .config
                    .request_response_queue_byte_capacity
        {
            drop(admission);
            worker.finish_invocation(&invocation_key);
            return self.invoke_backpressured(request, "plugin worker queue is at capacity");
        }

        let job = WorkerJob {
            request,
            cancellation: cancellation.clone(),
            queue_bytes,
            completion: JobCompletion::Blocking {
                result_sender: sender,
            },
            pooled: false,
        };
        admission.push_queued(PluginInvocationClass::RequestResponse, job, &worker);
        worker.work_cvar.notify_one();
        drop(admission);

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

    /// Admit one invocation without waiting for execution or completion.
    ///
    /// `completion_reservation_bytes` sets the host's positive completion
    /// allowance. Core raises the payload allowance to fit request and failure
    /// overhead, then charges fixed logical store metadata in addition. The
    /// per-completion cap applies to payload; the engine pool includes metadata.
    /// `Queued.reservation_bytes` reports the total retained reservation.
    ///
    /// Never blocks on job completion, `recv`, sleep, or a contended mutex.
    /// A busy registry or admission lock is [`PluginAdmissionResult::Backpressured`].
    pub fn try_admit(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
        completion_reservation_bytes: usize,
    ) -> PluginAdmissionResult {
        if completion_reservation_bytes == 0 {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: None,
                reason: "completion reservation must be positive".to_string(),
            };
        }
        if self.inner.shared.stopping.load(Ordering::SeqCst) {
            return PluginAdmissionResult::WorkerStopped {
                request_id: request.request_id,
                class,
                reason: "plugin worker engine is stopping".to_string(),
            };
        }

        let worker = match self.try_worker_for(&request.handler.plugin_key) {
            Err(()) => {
                return self.admission_backpressured(class, request, ADMISSION_LOCK_BUSY, None);
            }
            Ok(None) => {
                return PluginAdmissionResult::WorkerStopped {
                    request_id: request.request_id,
                    class,
                    reason: "plugin worker is not loaded".to_string(),
                };
            }
            Ok(Some(worker)) => worker,
        };
        let Some(generation) = worker.generation else {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: None,
                reason: "plugin worker generation identities exhausted".to_string(),
            };
        };

        let handler = worker.handlers.get(&request.handler).cloned();
        if let Some(handler) = &handler {
            if let Some(required_capability) = &handler.required_capability {
                if !worker.manifest.capabilities.contains(required_capability) {
                    return self.admit_immediate_failure(
                        &worker,
                        class,
                        request,
                        completion_reservation_bytes,
                        "plugin handler requires a capability missing from package metadata",
                    );
                }
            }
        } else {
            return self.admit_immediate_failure(
                &worker,
                class,
                request,
                completion_reservation_bytes,
                "plugin handler is not registered",
            );
        }

        let queue_bytes = match plugin_invocation_queue_bytes(&request) {
            Ok(bytes) => bytes,
            Err(_) => {
                return PluginAdmissionResult::RejectedBudget {
                    request_id: request.request_id,
                    class,
                    queue_bytes: None,
                    reason: "plugin invocation request could not be encoded".to_string(),
                };
            }
        };
        let class_byte_capacity = self.inner.shared.config.class_queue_byte_capacity(class);
        if queue_bytes > class_byte_capacity {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin invocation exceeds class byte capacity".to_string(),
            };
        }

        let fallbacks = match build_completion_fallbacks(class, &request) {
            Ok(fallbacks) => fallbacks,
            Err(_) => {
                return PluginAdmissionResult::RejectedBudget {
                    request_id: request.request_id,
                    class,
                    queue_bytes: Some(queue_bytes),
                    reason: "plugin completion fallbacks could not be encoded".to_string(),
                };
            }
        };
        let payload_bytes = effective_completion_reservation_bytes(
            queue_bytes,
            &fallbacks,
            completion_reservation_bytes,
        );
        if payload_bytes
            > self
                .inner
                .shared
                .config
                .completion_reservation_byte_capacity
        {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "completion reservation exceeds per-completion byte capacity".to_string(),
            };
        }
        let Some(reservation_bytes) = payload_bytes.checked_add(completion_store::metadata_bytes())
        else {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin completion reservation size overflowed".to_string(),
            };
        };
        if reservation_bytes > self.inner.shared.config.completion_queue_byte_capacity {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "completion reservation exceeds engine completion pool byte capacity"
                    .to_string(),
            };
        }

        let plugin_key = request.handler.plugin_key.clone();
        let mut admission = match worker.admission.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };
        if admission.stopping || worker.executor.stopping.load(Ordering::SeqCst) {
            return PluginAdmissionResult::WorkerStopped {
                request_id: request.request_id,
                class,
                reason: "plugin worker stopped before accepting invocation".to_string(),
            };
        }

        let (class_capacity, class_byte_capacity) =
            admission.ordinary_room(class, &self.inner.shared.config);
        let (queued_count, queued_bytes) = admission.ordinary_occupancy(class);
        if queued_count >= class_capacity || queued_bytes + queue_bytes > class_byte_capacity {
            self.record_class_pressure(class, &worker);
            return self.admission_backpressured(
                class,
                request,
                "plugin worker class queue is at capacity",
                Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
            );
        }
        let mut completions = match self.inner.shared.completions.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };
        let mut deadlines = match self.inner.shared.deadlines.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };
        let mut cancellations = match worker.executor.cancellations.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };

        let timeout_ms = request.timeout_ms;
        let Some(deadline) = Instant::now().checked_add(Duration::from_millis(timeout_ms)) else {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin completion deadline is out of range".to_string(),
            };
        };
        let reservation = match completions.reserve_funded(
            generation,
            payload_bytes,
            reservation_bytes,
            &worker.metrics,
            &self.inner.shared,
            admission.ordinary_funding(),
            None,
        ) {
            Ok(reservation) => reservation,
            Err(error) => {
                return self.completion_admission_refused(
                    error,
                    class,
                    request,
                    queue_bytes,
                    &worker,
                )
            }
        };

        let request_id = request.request_id.clone();
        let already_expired = timeout_ms == 0;
        let cancellation = PluginCancellationToken::new();

        let async_state = Arc::new(AsyncJobState {
            class,
            reservation,
            terminal: JobTerminal::new(),
            fallbacks,
            shared: self.inner.shared.clone(),
        });
        let job = WorkerJob {
            request,
            cancellation: cancellation.clone(),
            queue_bytes,
            completion: JobCompletion::Async(async_state.clone()),
            pooled: false,
        };

        let published = if already_expired {
            // Safe under the admission locks: this token was created above
            // and never reached a runtime, so no target can be subscribed.
            cancellation.cancel();
            let published = publish_prepared_into(
                &mut completions,
                &async_state,
                async_state.fallbacks.timed_out.clone(),
            );
            assert!(
                published,
                "a fresh already-expired terminal publishes exactly once"
            );
            published
        } else {
            cancellations.insert(request_id.clone(), cancellation.clone());
            admission.push_queued(class, job, &worker);
            deadlines.entries.push(DeadlineEntry {
                at: deadline,
                plugin_key,
                generation,
                request_id: request_id.clone(),
            });
            worker.work_cvar.notify_one();
            self.inner.shared.deadline_cvar.notify_one();
            false
        };

        drop(cancellations);
        drop(deadlines);
        drop(completions);
        drop(admission);
        if published {
            notify_completion(&self.inner.shared);
        }

        PluginAdmissionResult::Queued {
            request_id,
            class,
            queue_bytes,
            reservation_bytes,
        }
    }

    /// Reserve the host-call delivery pool and the ordinary completion share
    /// of `plugin_key`'s current generation, all or nothing (plan section
    /// 5.1). Every quota value is Hub policy. On refusal nothing is reserved.
    pub fn try_reserve_delivery(
        &self,
        plugin_key: &PluginKey,
        quota: PluginDeliveryQuota,
    ) -> Result<DeliveryPool, DeliveryRefusal> {
        let shared = &self.inner.shared;
        let Some(worker) = self.worker_for(plugin_key) else {
            return Err(DeliveryRefusal::WorkerStopped(
                "the plugin is not loaded".to_string(),
            ));
        };
        let Some(generation) = worker.generation else {
            return Err(DeliveryRefusal::RejectedBudget(
                "plugin worker generation identities exhausted".to_string(),
            ));
        };
        let config = &shared.config;
        if quota.call_result_slots > config.background_queue_capacity
            || quota.call_result_request_bytes > config.background_queue_byte_capacity
        {
            return Err(DeliveryRefusal::RejectedBudget(
                "the pool exceeds the plugin's Background queue capacity".to_string(),
            ));
        }
        let metadata = completion_store::metadata_bytes();
        let pool_bytes = quota
            .call_result_completion_bytes
            .checked_add(metadata)
            .and_then(|per_slot| per_slot.checked_mul(quota.call_result_slots));
        let share_bytes = metadata
            .checked_mul(quota.ordinary_completion_entries)
            .and_then(|bytes| bytes.checked_add(quota.ordinary_completion_bytes));
        let (Some(pool_bytes), Some(share_bytes)) = (pool_bytes, share_bytes) else {
            return Err(DeliveryRefusal::RejectedBudget(
                "the quota's byte sizes overflow".to_string(),
            ));
        };

        let mut admission = worker
            .admission
            .lock()
            .expect("plugin worker admission mutex poisoned");
        if admission.stopping {
            return Err(DeliveryRefusal::WorkerStopped(
                "the plugin worker is stopping".to_string(),
            ));
        }
        if admission.delivery.is_some() {
            return Err(DeliveryRefusal::RejectedBudget(
                "this generation already reserved its delivery".to_string(),
            ));
        }
        let (ordinary_count, ordinary_bytes) =
            admission.ordinary_occupancy(PluginInvocationClass::Background);
        if ordinary_count + quota.call_result_slots > config.background_queue_capacity
            || ordinary_bytes + quota.call_result_request_bytes
                > config.background_queue_byte_capacity
        {
            return Err(DeliveryRefusal::Backpressured(
                "queued work leaves no room for the pool".to_string(),
            ));
        }
        let (pool_fund, share_fund) = {
            let mut completions = shared
                .completions
                .lock()
                .expect("plugin completion store mutex poisoned");
            let pool_fund = completions
                .create_fund(quota.call_result_slots, pool_bytes, config)
                .map_err(|_| {
                    DeliveryRefusal::Backpressured(
                        "the completion pool cannot fund the delivery pool".to_string(),
                    )
                })?;
            match completions.create_fund(quota.ordinary_completion_entries, share_bytes, config) {
                Ok(share_fund) => (pool_fund, share_fund),
                Err(_) => {
                    // All or nothing: return the pool's funding too.
                    completions.close_fund(pool_fund);
                    return Err(DeliveryRefusal::Backpressured(
                        "the completion pool cannot fund the ordinary share".to_string(),
                    ));
                }
            }
        };
        let id = shared.next_pool.fetch_add(1, Ordering::SeqCst);
        let pool = Arc::new(delivery_pool::PoolInner::new(
            id,
            plugin_key.clone(),
            generation,
            &quota,
            pool_fund,
            Arc::downgrade(shared),
        ));
        shared
            .pools
            .lock()
            .expect("plugin delivery pool registry mutex poisoned")
            .insert(id, Arc::downgrade(&pool));
        admission.delivery = Some(delivery_pool::GenerationDelivery {
            pool: pool.clone(),
            pool_slots: quota.call_result_slots,
            pool_request_bytes: quota.call_result_request_bytes,
            share_fund,
        });
        Ok(DeliveryPool { inner: pool })
    }

    /// Drain previously published async completions without waiting.
    ///
    /// Returns at most `max_items` completions whose encoded sizes sum to at
    /// most `max_bytes`. A completion that does not fit the remaining budget is
    /// left in the mailbox.
    pub fn drain_completions(&self, max_items: usize, max_bytes: usize) -> PluginCompletionDrain {
        let mut drain = PluginCompletionDrain::default();
        let mut returned_units = Vec::new();
        if max_items != 0 && max_bytes != 0 {
            let mut completions = self
                .inner
                .shared
                .completions
                .lock()
                .expect("plugin completion store mutex poisoned");
            while drain.item_count < max_items {
                let Some((item, unit)) = completions
                    .take_fitting(max_bytes - drain.byte_count, &self.inner.shared.metrics)
                else {
                    break;
                };
                drain.item_count += 1;
                drain.byte_count += item.encoded_len;
                drain.completions.push(item);
                returned_units.extend(unit);
            }
        }
        // Units return outside every engine lock: their notifiers run host code.
        delivery_pool::return_units(&self.inner.shared, returned_units);
        self.finish_completion_drain(drain)
    }

    fn finish_completion_drain(&self, mut drain: PluginCompletionDrain) -> PluginCompletionDrain {
        drain.has_remaining = self
            .inner
            .shared
            .metrics
            .undrained_completions
            .load(Ordering::SeqCst)
            > 0;
        drain
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

    /// Return waiting-queue backpressure for one plugin worker.
    ///
    /// `depth` excludes currently executing invocations, which are reported by
    /// [`Self::debug_snapshot`].
    pub fn backpressure_for(&self, plugin_key: &PluginKey) -> BackpressureSummary {
        let depth = self
            .worker_for(plugin_key)
            .map(|worker| worker.queued_jobs())
            .unwrap_or_default();

        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: self.inner.shared.config.per_plugin_queue_capacity,
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
    ///
    /// Aggregate counters include retiring generations that have left the
    /// active plugin registry but whose executor workers have not joined yet.
    /// Per-plugin rows represent only currently registered generations.
    #[must_use]
    pub fn debug_snapshot(&self) -> PluginWorkerDebugSnapshot {
        let workers = self
            .inner
            .shared
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned");
        let mut plugins = workers
            .values()
            .map(|worker| worker.debug_snapshot(&self.inner.shared.config))
            .collect::<Vec<_>>();
        plugins.sort_by(|left, right| left.plugin_key.0.cmp(&right.plugin_key.0));
        let request_response_saturated = plugins
            .iter()
            .any(|plugin| plugin.request_response_saturated);
        let background_saturated = plugins.iter().any(|plugin| plugin.background_saturated);
        let reserved_completion_count = self
            .inner
            .shared
            .metrics
            .reserved_completion_count
            .load(Ordering::SeqCst);
        let reserved_completion_bytes = self
            .inner
            .shared
            .metrics
            .reserved_completion_bytes
            .load(Ordering::SeqCst);
        let completions_saturated = reserved_completion_count
            >= self.inner.shared.config.completion_queue_capacity
            || reserved_completion_bytes >= self.inner.shared.config.completion_queue_byte_capacity;

        PluginWorkerDebugSnapshot {
            configured_queue_capacity: self.inner.shared.config.per_plugin_queue_capacity,
            configured_executor_concurrency: self
                .inner
                .shared
                .config
                .per_plugin_executor_concurrency,
            configured_reserved_request_response_executors: self
                .inner
                .shared
                .config
                .reserved_request_response_executors,
            configured_request_response_queue_byte_capacity: self
                .inner
                .shared
                .config
                .request_response_queue_byte_capacity,
            configured_background_queue_capacity: self
                .inner
                .shared
                .config
                .background_queue_capacity,
            configured_background_queue_byte_capacity: self
                .inner
                .shared
                .config
                .background_queue_byte_capacity,
            configured_completion_queue_capacity: self
                .inner
                .shared
                .config
                .completion_queue_capacity,
            configured_completion_queue_byte_capacity: self
                .inner
                .shared
                .config
                .completion_queue_byte_capacity,
            live_plugin_executors: self
                .inner
                .shared
                .metrics
                .live_plugin_executors
                .load(Ordering::SeqCst),
            live_executor_workers: self
                .inner
                .shared
                .metrics
                .live_executor_workers
                .load(Ordering::SeqCst),
            queued_jobs: self.inner.shared.metrics.queued_jobs.load(Ordering::SeqCst),
            in_flight_jobs: self
                .inner
                .shared
                .metrics
                .in_flight_jobs
                .load(Ordering::SeqCst),
            request_response_queued_jobs: self
                .inner
                .shared
                .metrics
                .request_response_queued_jobs
                .load(Ordering::SeqCst),
            request_response_queued_bytes: self
                .inner
                .shared
                .metrics
                .request_response_queued_bytes
                .load(Ordering::SeqCst),
            request_response_in_flight_jobs: self
                .inner
                .shared
                .metrics
                .request_response_in_flight_jobs
                .load(Ordering::SeqCst),
            background_queued_jobs: self
                .inner
                .shared
                .metrics
                .background_queued_jobs
                .load(Ordering::SeqCst),
            background_queued_bytes: self
                .inner
                .shared
                .metrics
                .background_queued_bytes
                .load(Ordering::SeqCst),
            background_in_flight_jobs: self
                .inner
                .shared
                .metrics
                .background_in_flight_jobs
                .load(Ordering::SeqCst),
            reserved_completion_count,
            reserved_completion_bytes,
            undrained_completions: self
                .inner
                .shared
                .metrics
                .undrained_completions
                .load(Ordering::SeqCst),
            request_response_saturated,
            background_saturated,
            completions_saturated,
            request_response_pressure_events: self
                .inner
                .shared
                .metrics
                .request_response_pressure_events
                .load(Ordering::SeqCst),
            background_pressure_events: self
                .inner
                .shared
                .metrics
                .background_pressure_events
                .load(Ordering::SeqCst),
            completion_pressure_events: self
                .inner
                .shared
                .metrics
                .completion_pressure_events
                .load(Ordering::SeqCst),
            plugins,
        }
    }

    fn cleanup_plugin(
        &self,
        request_id: RequestId,
        plugin_key: &PluginKey,
        scope: PluginCleanupScope,
        stop_runtime: bool,
    ) -> PluginCleanupResult {
        let worker = self
            .inner
            .shared
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
            .shared
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .get(plugin_key)
            .cloned()
    }

    fn try_worker_for(&self, plugin_key: &PluginKey) -> Result<Option<WorkerState>, ()> {
        let workers = self.inner.shared.workers.try_lock().map_err(|_| ())?;
        Ok(workers.get(plugin_key).cloned())
    }

    fn invoke_backpressured(
        &self,
        request: PluginInvocationRequest,
        reason: &str,
    ) -> PluginInvocationOutcome {
        let plugin_key = request.handler.plugin_key.clone();
        let failure = PluginInvocationFailure {
            request_id: request.request_id,
            handler: request.handler,
            kind: PluginInvocationFailureKind::Backpressured,
            timeout_ms: None,
            reason: reason.to_string(),
        };
        PluginInvocationOutcome::with_event(
            PluginInvocationResult::Failed(failure),
            PluginWorkerEvent::Backpressure(self.backpressure_for(&plugin_key)),
        )
    }

    fn admission_backpressured(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
        reason: &str,
        backpressure: Option<BackpressureSummary>,
    ) -> PluginAdmissionResult {
        PluginAdmissionResult::Backpressured {
            request_id: request.request_id,
            class,
            reason: reason.to_string(),
            backpressure,
        }
    }

    fn backpressure_snapshot(&self, plugin_key: &PluginKey, depth: usize) -> BackpressureSummary {
        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: self.inner.shared.config.per_plugin_queue_capacity,
            depth,
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(plugin_key.clone()),
            },
        }
    }

    fn record_class_pressure(&self, class: PluginInvocationClass, worker: &WorkerState) {
        if is_background(class) {
            worker
                .metrics
                .background_pressure_events
                .fetch_add(1, Ordering::SeqCst);
            self.inner
                .shared
                .metrics
                .background_pressure_events
                .fetch_add(1, Ordering::SeqCst);
        } else {
            worker
                .metrics
                .request_response_pressure_events
                .fetch_add(1, Ordering::SeqCst);
            self.inner
                .shared
                .metrics
                .request_response_pressure_events
                .fetch_add(1, Ordering::SeqCst);
        }
    }

    fn admit_immediate_failure(
        &self,
        worker: &WorkerState,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
        completion_reservation_bytes: usize,
        reason: &str,
    ) -> PluginAdmissionResult {
        let Some(generation) = worker.generation else {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: None,
                reason: "plugin worker generation identities exhausted".to_string(),
            };
        };
        let queue_bytes = match plugin_invocation_queue_bytes(&request) {
            Ok(bytes) => bytes,
            Err(_) => {
                return PluginAdmissionResult::RejectedBudget {
                    request_id: request.request_id,
                    class,
                    queue_bytes: None,
                    reason: "plugin invocation request could not be encoded".to_string(),
                };
            }
        };
        if queue_bytes > self.inner.shared.config.class_queue_byte_capacity(class) {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin invocation exceeds class byte capacity".to_string(),
            };
        }
        let fallbacks = match build_completion_fallbacks(class, &request) {
            Ok(fallbacks) => fallbacks,
            Err(_) => {
                return PluginAdmissionResult::RejectedBudget {
                    request_id: request.request_id,
                    class,
                    queue_bytes: Some(queue_bytes),
                    reason: "plugin completion fallbacks could not be encoded".to_string(),
                };
            }
        };
        let payload_bytes = effective_completion_reservation_bytes(
            queue_bytes,
            &fallbacks,
            completion_reservation_bytes,
        );
        if payload_bytes
            > self
                .inner
                .shared
                .config
                .completion_reservation_byte_capacity
        {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "completion reservation exceeds per-completion byte capacity".to_string(),
            };
        }
        let Some(reservation_bytes) = payload_bytes.checked_add(completion_store::metadata_bytes())
        else {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin completion reservation size overflowed".to_string(),
            };
        };
        if reservation_bytes > self.inner.shared.config.completion_queue_byte_capacity {
            return PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "completion reservation exceeds engine completion pool byte capacity"
                    .to_string(),
            };
        }
        let plugin_key = request.handler.plugin_key.clone();
        let admission = match worker.admission.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };
        if admission.stopping {
            return PluginAdmissionResult::WorkerStopped {
                request_id: request.request_id,
                class,
                reason: "plugin worker stopped before accepting invocation".to_string(),
            };
        }
        let mut completions = match self.inner.shared.completions.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_snapshot(&plugin_key, worker.queued_jobs())),
                );
            }
        };
        let reservation = match completions.reserve_funded(
            generation,
            payload_bytes,
            reservation_bytes,
            &worker.metrics,
            &self.inner.shared,
            admission.ordinary_funding(),
            None,
        ) {
            Ok(reservation) => reservation,
            Err(error) => {
                return self.completion_admission_refused(
                    error,
                    class,
                    request,
                    queue_bytes,
                    worker,
                )
            }
        };

        let request_id = request.request_id.clone();
        let async_state = Arc::new(AsyncJobState {
            class,
            reservation,
            terminal: JobTerminal::new(),
            fallbacks,
            shared: self.inner.shared.clone(),
        });
        let prepared = prepared_completion(class, handler_failed_result(&request, reason))
            .ok()
            .filter(|prepared| prepared.encoded.len() <= payload_bytes)
            .unwrap_or_else(|| async_state.fallbacks.oversize.clone());
        let published = publish_prepared_into(&mut completions, &async_state, prepared);
        assert!(
            published,
            "a fresh immediate-failure terminal publishes exactly once"
        );
        drop(completions);
        drop(admission);
        notify_completion(&self.inner.shared);
        PluginAdmissionResult::Queued {
            request_id,
            class,
            queue_bytes,
            reservation_bytes,
        }
    }

    fn completion_admission_refused(
        &self,
        error: StoreAdmissionError,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
        queue_bytes: usize,
        worker: &WorkerState,
    ) -> PluginAdmissionResult {
        match error {
            StoreAdmissionError::Capacity => {
                worker
                    .metrics
                    .completion_pressure_events
                    .fetch_add(1, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .completion_pressure_events
                    .fetch_add(1, Ordering::SeqCst);
                self.admission_backpressured(
                    class,
                    request,
                    "plugin completion reservation pool is at capacity",
                    Some(self.backpressure_snapshot(&worker.plugin_key, worker.queued_jobs())),
                )
            }
            StoreAdmissionError::IdentityExhausted => PluginAdmissionResult::RejectedBudget {
                request_id: request.request_id,
                class,
                queue_bytes: Some(queue_bytes),
                reason: "plugin completion reservation identities exhausted".to_string(),
            },
            StoreAdmissionError::Retired => PluginAdmissionResult::WorkerStopped {
                request_id: request.request_id,
                class,
                reason: "plugin completion generation is retired".to_string(),
            },
        }
    }
}

#[derive(Clone)]
struct WorkerState {
    plugin_key: PluginKey,
    generation: Option<u64>,
    manifest: PackageManifest,
    runtime: Arc<dyn PluginRuntime>,
    handlers: HashMap<PluginHandlerRef, PluginHandlerRegistration>,
    descriptors: Vec<PluginDescriptorRef>,
    resources: Arc<Mutex<Vec<PluginResourceRef>>>,
    admission: Arc<Mutex<WorkerAdmission>>,
    work_cvar: Arc<Condvar>,
    executor: WorkerExecutorHandle,
    metrics: Arc<WorkerMetrics>,
    shared: Arc<EngineShared>,
}

impl WorkerState {
    fn new(
        registration: PluginWorkerRegistration,
        shared: Arc<EngineShared>,
        resources: Option<PluginWorkerResources>,
    ) -> Self {
        let generation = allocate_worker_generation(&NEXT_WORKER_GENERATION);
        Self::with_generation(registration, shared, generation, resources)
    }

    fn with_generation(
        registration: PluginWorkerRegistration,
        shared: Arc<EngineShared>,
        generation: Option<u64>,
        resources: Option<PluginWorkerResources>,
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
        let metrics = Arc::new(WorkerMetrics::default());
        let admission = Arc::new(Mutex::new(WorkerAdmission::default()));
        let work_cvar = Arc::new(Condvar::new());
        let executor_concurrency = shared.config.per_plugin_executor_concurrency;
        let mut construction = WorkerResourceConstruction::new(resources, executor_concurrency);
        shared
            .metrics
            .live_plugin_executors
            .fetch_add(1, Ordering::SeqCst);
        shared
            .metrics
            .live_executor_workers
            .fetch_add(executor_concurrency, Ordering::SeqCst);
        metrics
            .live_workers
            .store(executor_concurrency, Ordering::SeqCst);

        let stopping = Arc::new(AtomicBool::new(false));
        for worker_index in 0..executor_concurrency {
            let worker_runtime = runtime.clone();
            let worker_cancellations = cancellations.clone();
            let worker_metrics = metrics.clone();
            let worker_engine_metrics = shared.metrics.clone();
            let worker_admission = admission.clone();
            let worker_cvar = work_cvar.clone();
            let worker_stopping = stopping.clone();
            let worker_config = shared.config.clone();
            let thread_name = match generation {
                Some(generation) => format!("botster-plugin-worker-{generation}-{worker_index}"),
                None => format!("botster-plugin-worker-exhausted-{worker_index}"),
            };
            let resource = construction.next_resource();
            let join_handle = WorkerJoinRecord::spawn(resource, || {
                std::thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || {
                        let _liveness = WorkerLivenessGuard {
                            metrics: worker_metrics.clone(),
                            engine_metrics: worker_engine_metrics.clone(),
                        };
                        loop {
                            let job = {
                                let mut admission = worker_admission
                                    .lock()
                                    .expect("plugin worker admission mutex poisoned");
                                loop {
                                    if let Some(job) = admission.take_dispatchable(
                                        &worker_config,
                                        &worker_metrics,
                                        &worker_engine_metrics,
                                    ) {
                                        break Some(job);
                                    }
                                    if worker_stopping.load(Ordering::SeqCst) || admission.stopping
                                    {
                                        break None;
                                    }
                                    admission = worker_cvar
                                        .wait(admission)
                                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                                }
                            };
                            let Some(job) = job else {
                                break;
                            };
                            let request_id = job.request.request_id.clone();
                            if job.cancellation.is_cancelled() {
                                finish_skipped_job(
                                    job,
                                    &worker_metrics,
                                    &worker_engine_metrics,
                                    &worker_cancellations,
                                    &worker_admission,
                                );
                                worker_cvar.notify_one();
                                continue;
                            }
                            let in_flight = InFlightGuard {
                                metrics: worker_metrics.clone(),
                                engine_metrics: worker_engine_metrics.clone(),
                                cancellations: worker_cancellations.clone(),
                                request_id: request_id.clone(),
                                class: job_class(&job),
                                async_state: async_state_of(&job),
                                admission: worker_admission.clone(),
                                work_cvar: worker_cvar.clone(),
                            };
                            let result = worker_runtime.invoke(job.request, job.cancellation);
                            complete_job(job.completion, result);
                            drop(in_flight);
                        }
                    })
            })
            .expect("spawn plugin worker thread");
            construction.push(join_handle);
        }

        let executor = WorkerExecutorHandle::new(construction, stopping, cancellations);

        Self {
            plugin_key,
            generation,
            manifest: registration.manifest,
            runtime,
            handlers,
            descriptors,
            resources: Arc::new(Mutex::new(registration.resources)),
            admission,
            work_cvar,
            executor,
            metrics,
            shared,
        }
    }

    fn track_invocation(&self, request_id: RequestId, cancellation: PluginCancellationToken) {
        self.executor
            .cancellations
            .lock()
            .expect("plugin worker cancellations mutex poisoned")
            .insert(request_id, cancellation);
    }

    fn finish_invocation(&self, request_id: &RequestId) {
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
        if let Some(generation) = self.generation {
            remove_deadlines_for_generation(&self.shared, &self.plugin_key, generation);
        }

        let (queued, open_async) = {
            let mut admission = self
                .admission
                .lock()
                .expect("plugin worker admission mutex poisoned");
            admission.stopping = true;
            if let Some(generation) = self.generation {
                let mut completions = self
                    .shared
                    .completions
                    .lock()
                    .expect("plugin completion store mutex poisoned");
                completions.retire(generation);
                if let Some(delivery) = &admission.delivery {
                    completions.close_fund(delivery.pool.fund);
                    completions.close_fund(delivery.share_fund);
                }
            }
            if let Some(delivery) = admission.delivery.take() {
                delivery.pool.close();
                self.shared
                    .pools
                    .lock()
                    .expect("plugin delivery pool registry mutex poisoned")
                    .remove(&delivery.pool.id);
            }
            let queued = admission.drain_queued();
            let open_async = admission
                .jobs
                .values()
                .filter_map(|tracked| match &tracked.completion {
                    JobCompletion::Async(state) => Some(state.clone()),
                    JobCompletion::Blocking { .. } => None,
                })
                .collect::<Vec<_>>();
            admission.jobs.clear();
            (queued, open_async)
        };
        for job in queued {
            cancel_queued_job(job, &self.metrics, &self.shared.metrics);
        }
        for state in open_async {
            seal_and_publish(
                &state,
                state.fallbacks.worker_stopped.result.clone(),
                Some(state.fallbacks.worker_stopped.clone()),
            );
        }

        self.work_cvar.notify_all();
        self.runtime.stop(&self.plugin_key);

        let join_handles = self
            .executor
            .join_handles
            .lock()
            .expect("plugin worker join handles mutex poisoned")
            .join_handles
            .take()
            .unwrap_or_default();
        for join_handle in join_handles {
            let _ = join_handle.join();
        }
        self.shared
            .metrics
            .live_plugin_executors
            .fetch_sub(1, Ordering::SeqCst);
    }

    fn queued_jobs(&self) -> usize {
        self.metrics.queued_jobs.load(Ordering::SeqCst)
    }

    fn debug_snapshot(&self, config: &PluginWorkerEngineConfig) -> PluginWorkerPluginDebugSnapshot {
        let request_response_queued_jobs = self
            .metrics
            .request_response_queued_jobs
            .load(Ordering::SeqCst);
        let request_response_queued_bytes = self
            .metrics
            .request_response_queued_bytes
            .load(Ordering::SeqCst);
        let background_queued_jobs = self.metrics.background_queued_jobs.load(Ordering::SeqCst);
        let background_queued_bytes = self.metrics.background_queued_bytes.load(Ordering::SeqCst);
        let reserved_completion_count = self
            .metrics
            .reserved_completion_count
            .load(Ordering::SeqCst);
        let reserved_completion_bytes = self
            .metrics
            .reserved_completion_bytes
            .load(Ordering::SeqCst);
        PluginWorkerPluginDebugSnapshot {
            plugin_key: self.plugin_key.clone(),
            live_executor_workers: self.metrics.live_workers.load(Ordering::SeqCst),
            queued_jobs: self.metrics.queued_jobs.load(Ordering::SeqCst),
            in_flight_jobs: self.metrics.in_flight_jobs.load(Ordering::SeqCst),
            request_response_queued_jobs,
            request_response_queued_bytes,
            request_response_in_flight_jobs: self
                .metrics
                .request_response_in_flight_jobs
                .load(Ordering::SeqCst),
            background_queued_jobs,
            background_queued_bytes,
            background_in_flight_jobs: self
                .metrics
                .background_in_flight_jobs
                .load(Ordering::SeqCst),
            reserved_completion_count,
            reserved_completion_bytes,
            undrained_completions: self.metrics.undrained_completions.load(Ordering::SeqCst),
            reserved_request_response_executors: config.reserved_request_response_executors,
            request_response_saturated: request_response_queued_jobs
                >= config.per_plugin_queue_capacity
                || request_response_queued_bytes >= config.request_response_queue_byte_capacity,
            background_saturated: background_queued_jobs >= config.background_queue_capacity
                || background_queued_bytes >= config.background_queue_byte_capacity,
            request_response_pressure_events: self
                .metrics
                .request_response_pressure_events
                .load(Ordering::SeqCst),
            background_pressure_events: self
                .metrics
                .background_pressure_events
                .load(Ordering::SeqCst),
            completion_pressure_events: self
                .metrics
                .completion_pressure_events
                .load(Ordering::SeqCst),
        }
    }
}

#[derive(Default)]
struct WorkerAdmission {
    stopping: bool,
    rr_queue: VecDeque<WorkerJob>,
    rr_queued_bytes: usize,
    bg_queue: VecDeque<WorkerJob>,
    bg_queued_bytes: usize,
    executor_in_flight_rr: usize,
    executor_in_flight_bg: usize,
    jobs: HashMap<RequestId, TrackedJob>,
    /// This generation's delivery reservation, once made.
    delivery: Option<delivery_pool::GenerationDelivery>,
    /// Pool result jobs waiting in `bg_queue`, and their bytes.
    pooled_queued: usize,
    pooled_queued_bytes: usize,
}

impl WorkerAdmission {
    /// Queue room left for ordinary (non-pool) work of `class`: the delivery
    /// pool holds its slots and request bytes out of the Background queue.
    fn ordinary_room(
        &self,
        class: PluginInvocationClass,
        config: &PluginWorkerEngineConfig,
    ) -> (usize, usize) {
        let (count, bytes) = (
            config.class_queue_capacity(class),
            config.class_queue_byte_capacity(class),
        );
        match (&self.delivery, is_background(class)) {
            (Some(delivery), true) => (
                count.saturating_sub(delivery.pool_slots),
                bytes.saturating_sub(delivery.pool_request_bytes),
            ),
            _ => (count, bytes),
        }
    }

    /// Queued ordinary (non-pool) work of `class`.
    fn ordinary_occupancy(&self, class: PluginInvocationClass) -> (usize, usize) {
        let (count, bytes) = self.queue_occupancy(class);
        if is_background(class) {
            (count - self.pooled_queued, bytes - self.pooled_queued_bytes)
        } else {
            (count, bytes)
        }
    }

    /// Where an ordinary completion is charged: the plugin's completion
    /// share when it reserved one, else the engine-wide pool.
    fn ordinary_funding(&self) -> completion_store::Funding {
        self.delivery
            .as_ref()
            .map_or(completion_store::Funding::Global, |delivery| {
                completion_store::Funding::Fund(delivery.share_fund)
            })
    }

    /// Account a pool job leaving the Background queue.
    fn pooled_left(&mut self, job: &WorkerJob) {
        if job.pooled {
            self.pooled_queued -= 1;
            self.pooled_queued_bytes -= job.queue_bytes;
        }
    }

    fn queue_occupancy(&self, class: PluginInvocationClass) -> (usize, usize) {
        match class {
            PluginInvocationClass::Background => (self.bg_queue.len(), self.bg_queued_bytes),
            PluginInvocationClass::RequestResponse => (self.rr_queue.len(), self.rr_queued_bytes),
        }
    }

    fn push_queued(&mut self, class: PluginInvocationClass, job: WorkerJob, worker: &WorkerState) {
        let request_id = job.request.request_id.clone();
        let queue_bytes = job.queue_bytes;
        self.jobs.insert(
            request_id,
            TrackedJob {
                phase: JobPhase::Queued,
                completion: job.completion.clone(),
                cancellation: job.cancellation.clone(),
            },
        );
        match class {
            PluginInvocationClass::Background => {
                if job.pooled {
                    self.pooled_queued += 1;
                    self.pooled_queued_bytes += queue_bytes;
                }
                self.bg_queue.push_back(job);
                self.bg_queued_bytes += queue_bytes;
                worker
                    .metrics
                    .background_queued_jobs
                    .fetch_add(1, Ordering::SeqCst);
                worker
                    .metrics
                    .background_queued_bytes
                    .fetch_add(queue_bytes, Ordering::SeqCst);
                worker
                    .shared
                    .metrics
                    .background_queued_jobs
                    .fetch_add(1, Ordering::SeqCst);
                worker
                    .shared
                    .metrics
                    .background_queued_bytes
                    .fetch_add(queue_bytes, Ordering::SeqCst);
            }
            PluginInvocationClass::RequestResponse => {
                self.rr_queue.push_back(job);
                self.rr_queued_bytes += queue_bytes;
                worker
                    .metrics
                    .request_response_queued_jobs
                    .fetch_add(1, Ordering::SeqCst);
                worker
                    .metrics
                    .request_response_queued_bytes
                    .fetch_add(queue_bytes, Ordering::SeqCst);
                worker
                    .shared
                    .metrics
                    .request_response_queued_jobs
                    .fetch_add(1, Ordering::SeqCst);
                worker
                    .shared
                    .metrics
                    .request_response_queued_bytes
                    .fetch_add(queue_bytes, Ordering::SeqCst);
            }
        }
        worker.metrics.queued_jobs.fetch_add(1, Ordering::SeqCst);
        worker
            .shared
            .metrics
            .queued_jobs
            .fetch_add(1, Ordering::SeqCst);
    }

    fn take_dispatchable(
        &mut self,
        config: &PluginWorkerEngineConfig,
        metrics: &WorkerMetrics,
        engine_metrics: &PluginWorkerEngineMetrics,
    ) -> Option<WorkerJob> {
        let in_flight_total = self.executor_in_flight_rr + self.executor_in_flight_bg;
        if in_flight_total >= config.per_plugin_executor_concurrency {
            return None;
        }
        if !self.rr_queue.is_empty() {
            return self.pop_class(
                PluginInvocationClass::RequestResponse,
                metrics,
                engine_metrics,
            );
        }
        if !self.bg_queue.is_empty()
            && self.executor_in_flight_bg < config.background_executor_limit()
        {
            return self.pop_class(PluginInvocationClass::Background, metrics, engine_metrics);
        }
        None
    }

    fn pop_class(
        &mut self,
        class: PluginInvocationClass,
        metrics: &WorkerMetrics,
        engine_metrics: &PluginWorkerEngineMetrics,
    ) -> Option<WorkerJob> {
        let job = match class {
            PluginInvocationClass::Background => self.bg_queue.pop_front()?,
            PluginInvocationClass::RequestResponse => self.rr_queue.pop_front()?,
        };
        let queue_bytes = job.queue_bytes;
        self.pooled_left(&job);
        match class {
            PluginInvocationClass::Background => {
                self.bg_queued_bytes = self.bg_queued_bytes.saturating_sub(queue_bytes);
                self.executor_in_flight_bg += 1;
                metrics
                    .background_queued_jobs
                    .fetch_sub(1, Ordering::SeqCst);
                metrics
                    .background_queued_bytes
                    .fetch_sub(queue_bytes, Ordering::SeqCst);
                metrics
                    .background_in_flight_jobs
                    .fetch_add(1, Ordering::SeqCst);
                engine_metrics
                    .background_queued_jobs
                    .fetch_sub(1, Ordering::SeqCst);
                engine_metrics
                    .background_queued_bytes
                    .fetch_sub(queue_bytes, Ordering::SeqCst);
                engine_metrics
                    .background_in_flight_jobs
                    .fetch_add(1, Ordering::SeqCst);
            }
            PluginInvocationClass::RequestResponse => {
                self.rr_queued_bytes = self.rr_queued_bytes.saturating_sub(queue_bytes);
                self.executor_in_flight_rr += 1;
                metrics
                    .request_response_queued_jobs
                    .fetch_sub(1, Ordering::SeqCst);
                metrics
                    .request_response_queued_bytes
                    .fetch_sub(queue_bytes, Ordering::SeqCst);
                metrics
                    .request_response_in_flight_jobs
                    .fetch_add(1, Ordering::SeqCst);
                engine_metrics
                    .request_response_queued_jobs
                    .fetch_sub(1, Ordering::SeqCst);
                engine_metrics
                    .request_response_queued_bytes
                    .fetch_sub(queue_bytes, Ordering::SeqCst);
                engine_metrics
                    .request_response_in_flight_jobs
                    .fetch_add(1, Ordering::SeqCst);
            }
        }
        metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
        metrics.in_flight_jobs.fetch_add(1, Ordering::SeqCst);
        engine_metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
        engine_metrics.in_flight_jobs.fetch_add(1, Ordering::SeqCst);
        if let Some(tracked) = self.jobs.get_mut(&job.request.request_id) {
            tracked.phase = JobPhase::InFlight;
        }
        Some(job)
    }

    fn drain_queued(&mut self) -> Vec<WorkerJob> {
        let mut jobs = Vec::new();
        jobs.extend(self.rr_queue.drain(..));
        jobs.extend(self.bg_queue.drain(..));
        self.rr_queued_bytes = 0;
        self.bg_queued_bytes = 0;
        self.pooled_queued = 0;
        self.pooled_queued_bytes = 0;
        jobs
    }

    fn remove_queued(&mut self, request_id: &RequestId) -> Option<WorkerJob> {
        if let Some(index) = self
            .rr_queue
            .iter()
            .position(|job| job.request.request_id == *request_id)
        {
            return self.rr_queue.remove(index);
        }
        if let Some(index) = self
            .bg_queue
            .iter()
            .position(|job| job.request.request_id == *request_id)
        {
            let job = self.bg_queue.remove(index)?;
            self.pooled_left(&job);
            return Some(job);
        }
        None
    }
}

struct WorkerExecutor {
    join_handles: Mutex<WorkerExecutorResources>,
    stopping: Arc<AtomicBool>,
    cancellations: Arc<Mutex<HashMap<RequestId, PluginCancellationToken>>>,
}

struct WorkerExecutorResources {
    join_handles: Option<Vec<WorkerJoinRecord>>,
    metadata: WorkerMetadataGuard,
}

struct WorkerExecutorHandle {
    inner: Option<Arc<WorkerExecutor>>,
}

impl WorkerExecutorHandle {
    fn new(
        mut construction: WorkerResourceConstruction,
        stopping: Arc<AtomicBool>,
        cancellations: Arc<Mutex<HashMap<RequestId, PluginCancellationToken>>>,
    ) -> Self {
        construction.destroy_input();
        // Construction still guards metadata while the Arc allocation starts.
        let mut handle = Self {
            inner: Some(Arc::new(WorkerExecutor {
                join_handles: Mutex::new(WorkerExecutorResources {
                    join_handles: None,
                    metadata: WorkerMetadataGuard::default(),
                }),
                stopping,
                cancellations,
            })),
        };
        let executor = Arc::get_mut(handle.inner.as_mut().expect("worker executor handle"))
            .expect("new worker executor has one owner");
        let resources = executor
            .join_handles
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        resources.join_handles = construction.take_join_handles();
        resources.metadata = construction.take_metadata();
        handle
    }
}

impl Clone for WorkerExecutorHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Some(Arc::clone(
                self.inner.as_ref().expect("worker executor handle"),
            )),
        }
    }
}

impl std::ops::Deref for WorkerExecutorHandle {
    type Target = WorkerExecutor;

    fn deref(&self) -> &Self::Target {
        self.inner.as_deref().expect("worker executor handle")
    }
}

impl Drop for WorkerExecutorHandle {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        // No raw Arc or Weak escapes. The allocation ends before extraction returns.
        if let Some(executor) = Arc::into_inner(inner) {
            let WorkerExecutor {
                join_handles,
                stopping,
                cancellations,
            } = executor;
            let WorkerExecutorResources {
                join_handles,
                mut metadata,
            } = join_handles
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(join_handles);
            drop(stopping);
            drop(cancellations);
            metadata.release();
        }
    }
}

#[derive(Default)]
struct PluginWorkerEngineMetrics {
    live_plugin_executors: AtomicUsize,
    live_executor_workers: AtomicUsize,
    queued_jobs: AtomicUsize,
    in_flight_jobs: AtomicUsize,
    request_response_queued_jobs: AtomicUsize,
    request_response_queued_bytes: AtomicUsize,
    request_response_in_flight_jobs: AtomicUsize,
    background_queued_jobs: AtomicUsize,
    background_queued_bytes: AtomicUsize,
    background_in_flight_jobs: AtomicUsize,
    reserved_completion_count: AtomicUsize,
    reserved_completion_bytes: AtomicUsize,
    undrained_completions: AtomicUsize,
    request_response_pressure_events: AtomicUsize,
    background_pressure_events: AtomicUsize,
    completion_pressure_events: AtomicUsize,
}

#[derive(Default)]
struct WorkerMetrics {
    live_workers: AtomicUsize,
    queued_jobs: AtomicUsize,
    in_flight_jobs: AtomicUsize,
    request_response_queued_jobs: AtomicUsize,
    request_response_queued_bytes: AtomicUsize,
    request_response_in_flight_jobs: AtomicUsize,
    background_queued_jobs: AtomicUsize,
    background_queued_bytes: AtomicUsize,
    background_in_flight_jobs: AtomicUsize,
    reserved_completion_count: AtomicUsize,
    reserved_completion_bytes: AtomicUsize,
    undrained_completions: AtomicUsize,
    request_response_pressure_events: AtomicUsize,
    background_pressure_events: AtomicUsize,
    completion_pressure_events: AtomicUsize,
}

struct WorkerLivenessGuard {
    metrics: Arc<WorkerMetrics>,
    engine_metrics: Arc<PluginWorkerEngineMetrics>,
}

impl Drop for WorkerLivenessGuard {
    fn drop(&mut self) {
        self.metrics.live_workers.fetch_sub(1, Ordering::SeqCst);
        self.engine_metrics
            .live_executor_workers
            .fetch_sub(1, Ordering::SeqCst);
    }
}

struct InFlightGuard {
    metrics: Arc<WorkerMetrics>,
    engine_metrics: Arc<PluginWorkerEngineMetrics>,
    cancellations: Arc<Mutex<HashMap<RequestId, PluginCancellationToken>>>,
    request_id: RequestId,
    class: PluginInvocationClass,
    async_state: Option<Arc<AsyncJobState>>,
    admission: Arc<Mutex<WorkerAdmission>>,
    work_cvar: Arc<Condvar>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        decrement_executor_in_flight(
            self.class,
            &self.metrics,
            &self.engine_metrics,
            &self.admission,
        );
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.request_id);
        if let Some(state) = &self.async_state {
            if std::thread::panicking() {
                seal_and_publish(
                    state,
                    state.fallbacks.worker_stopped.result.clone(),
                    Some(state.fallbacks.worker_stopped.clone()),
                );
            }
            if let Ok(mut admission) = self.admission.lock() {
                admission.jobs.remove(&self.request_id);
            }
        }
        self.work_cvar.notify_one();
    }
}

struct WorkerJob {
    request: PluginInvocationRequest,
    cancellation: PluginCancellationToken,
    queue_bytes: usize,
    completion: JobCompletion,
    /// A delivery-pool result job: it uses the pool's reserved queue room.
    pooled: bool,
}

#[derive(Clone)]
enum JobCompletion {
    Blocking {
        result_sender: mpsc::Sender<PluginInvocationResult>,
    },
    Async(Arc<AsyncJobState>),
}

struct AsyncJobState {
    class: PluginInvocationClass,
    reservation: CompletionReservation,
    terminal: JobTerminal,
    fallbacks: CompletionFallbacks,
    shared: Arc<EngineShared>,
}

struct JobTerminal {
    sealed: AtomicBool,
}

impl JobTerminal {
    fn new() -> Self {
        Self {
            sealed: AtomicBool::new(false),
        }
    }

    fn try_seal(&self) -> bool {
        self.sealed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

struct CompletionFallbacks {
    timed_out: PreparedCompletion,
    worker_stopped: PreparedCompletion,
    oversize: PreparedCompletion,
    timed_out_bytes: usize,
    worker_stopped_bytes: usize,
    oversize_bytes: usize,
}

#[derive(Clone)]
struct PreparedCompletion {
    completion: PluginCompletion,
    encoded: Vec<u8>,
    result: PluginInvocationResult,
}

#[derive(Clone)]
struct TrackedJob {
    phase: JobPhase,
    completion: JobCompletion,
    cancellation: PluginCancellationToken,
}

#[derive(Clone, Copy)]
enum JobPhase {
    Queued,
    InFlight,
}

/// Admit one delivery-pool result (see [`DeliveryPool::admit_result`]).
///
/// The unit already holds the Background queue room and a funded completion
/// entry, so nothing here checks capacity. Lock order matches `try_admit`:
/// worker admission, then the completion store, the deadline book, and the
/// cancellation map. The pool's own lock is never held with them.
fn admit_pool_result(
    shared: &Arc<EngineShared>,
    pool: &Arc<delivery_pool::PoolInner>,
    call: CallId,
    request: PluginInvocationRequest,
) -> PluginAdmissionResult {
    let class = PluginInvocationClass::Background;
    let rejected = |request: PluginInvocationRequest, queue_bytes, reason: &str| {
        PluginAdmissionResult::RejectedBudget {
            request_id: request.request_id,
            class,
            queue_bytes,
            reason: reason.to_string(),
        }
    };
    let stopped =
        |request: PluginInvocationRequest, reason: &str| PluginAdmissionResult::WorkerStopped {
            request_id: request.request_id,
            class,
            reason: reason.to_string(),
        };
    let declared_bytes = match pool.begin_admit(call) {
        Ok(bytes) => bytes,
        Err(delivery_pool::AdmitRefusal::Closed) => {
            return stopped(request, "the delivery pool's generation retired")
        }
        Err(delivery_pool::AdmitRefusal::UnknownCall) => {
            return rejected(request, None, "no accepted host call has this call id")
        }
        Err(delivery_pool::AdmitRefusal::AlreadyAdmitted) => {
            return rejected(
                request,
                None,
                "this host call's result was already admitted",
            )
        }
    };
    let finish = |admitted: bool, result: PluginAdmissionResult| {
        pool.end_admit(call, admitted);
        result
    };

    let worker = shared
        .workers
        .lock()
        .expect("plugin worker engine mutex poisoned")
        .get(&pool.plugin_key)
        .cloned();
    let Some(worker) = worker.filter(|worker| worker.generation == Some(pool.generation)) else {
        return finish(
            false,
            stopped(request, "the delivery pool's generation retired"),
        );
    };
    let registered = worker
        .handlers
        .get(&request.handler)
        .is_some_and(|handler| {
            handler
                .required_capability
                .as_ref()
                .is_none_or(|capability| worker.manifest.capabilities.contains(capability))
        });
    if !registered {
        return finish(
            false,
            rejected(
                request,
                None,
                "the result handler is not registered with its capability",
            ),
        );
    }
    let Ok(queue_bytes) = plugin_invocation_queue_bytes(&request) else {
        return finish(
            false,
            rejected(request, None, "the result request could not be encoded"),
        );
    };
    if queue_bytes > declared_bytes {
        return finish(
            false,
            rejected(
                request,
                Some(queue_bytes),
                "the result request exceeds its declared size",
            ),
        );
    }
    let Ok(fallbacks) = build_completion_fallbacks(class, &request) else {
        return finish(
            false,
            rejected(
                request,
                Some(queue_bytes),
                "the completion fallbacks could not be encoded",
            ),
        );
    };
    // The result request is bounded by its own declaration above. The
    // completion entry only has to hold the job's fallback markers, so the
    // request's size plays no part here.
    let payload_bytes = pool.completion_bytes;
    if effective_completion_reservation_bytes(0, &fallbacks, payload_bytes) > payload_bytes {
        return finish(
            false,
            rejected(
                request,
                Some(queue_bytes),
                "the result's completion fallbacks exceed the pool's completion allowance",
            ),
        );
    }
    let charged_bytes = payload_bytes + completion_store::metadata_bytes();
    let timeout_ms = request.timeout_ms;
    let Some(deadline) = Instant::now().checked_add(Duration::from_millis(timeout_ms)) else {
        return finish(
            false,
            rejected(
                request,
                Some(queue_bytes),
                "the result deadline is out of range",
            ),
        );
    };

    let mut admission = worker
        .admission
        .lock()
        .expect("plugin worker admission mutex poisoned");
    if admission.stopping {
        return finish(false, stopped(request, "the plugin worker stopped"));
    }
    let mut completions = shared
        .completions
        .lock()
        .expect("plugin completion store mutex poisoned");
    let mut deadlines = shared
        .deadlines
        .lock()
        .expect("plugin deadline book mutex poisoned");
    let mut cancellations = worker
        .executor
        .cancellations
        .lock()
        .expect("plugin worker cancellations mutex poisoned");
    let reservation = match completions.reserve_funded(
        pool.generation,
        payload_bytes,
        charged_bytes,
        &worker.metrics,
        shared,
        completion_store::Funding::Fund(pool.fund),
        Some(completion_store::UnitTag {
            pool: pool.id,
            call: call.0,
        }),
    ) {
        Ok(reservation) => reservation,
        Err(StoreAdmissionError::Capacity) => {
            debug_assert!(false, "a unit's completion entry is always funded");
            return finish(
                false,
                rejected(
                    request,
                    Some(queue_bytes),
                    "the pool's completion fund is exhausted",
                ),
            );
        }
        Err(_) => {
            return finish(
                false,
                stopped(request, "the delivery pool's generation retired"),
            )
        }
    };

    let request_id = request.request_id.clone();
    let cancellation = PluginCancellationToken::new();
    let async_state = Arc::new(AsyncJobState {
        class,
        reservation,
        terminal: JobTerminal::new(),
        fallbacks,
        shared: shared.clone(),
    });
    let job = WorkerJob {
        request,
        cancellation: cancellation.clone(),
        queue_bytes,
        completion: JobCompletion::Async(async_state.clone()),
        pooled: true,
    };
    let published = if timeout_ms == 0 {
        // A fresh token that never reached a runtime: no target can run.
        cancellation.cancel();
        publish_prepared_into(
            &mut completions,
            &async_state,
            async_state.fallbacks.timed_out.clone(),
        )
    } else {
        cancellations.insert(request_id.clone(), cancellation);
        admission.push_queued(class, job, &worker);
        deadlines.entries.push(DeadlineEntry {
            at: deadline,
            plugin_key: pool.plugin_key.clone(),
            generation: pool.generation,
            request_id: request_id.clone(),
        });
        worker.work_cvar.notify_one();
        shared.deadline_cvar.notify_one();
        false
    };
    drop(cancellations);
    drop(deadlines);
    drop(completions);
    drop(admission);
    if published {
        notify_completion(shared);
    }
    finish(
        true,
        PluginAdmissionResult::Queued {
            request_id,
            class,
            queue_bytes,
            reservation_bytes: charged_bytes,
        },
    )
}

fn is_background(class: PluginInvocationClass) -> bool {
    matches!(class, PluginInvocationClass::Background)
}

fn plugin_invocation_queue_bytes(request: &PluginInvocationRequest) -> Result<usize, ()> {
    serde_json::to_vec(request)
        .map(|bytes| bytes.len())
        .map_err(|_| ())
}

fn effective_completion_reservation_bytes(
    queue_bytes: usize,
    fallbacks: &CompletionFallbacks,
    completion_reservation_bytes: usize,
) -> usize {
    queue_bytes
        .max(fallbacks.timed_out_bytes)
        .max(fallbacks.worker_stopped_bytes)
        .max(fallbacks.oversize_bytes)
        .max(completion_reservation_bytes)
}

fn encode_completion(completion: &PluginCompletion) -> Result<Vec<u8>, ()> {
    serde_json::to_vec(completion).map_err(|_| ())
}

fn build_completion_fallbacks(
    class: PluginInvocationClass,
    request: &PluginInvocationRequest,
) -> Result<CompletionFallbacks, ()> {
    let timed_out = prepared_completion(
        class,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: request.request_id.clone(),
            handler: request.handler.clone(),
            kind: PluginInvocationFailureKind::TimedOut,
            timeout_ms: Some(request.timeout_ms),
            reason: "plugin handler exceeded timeout".to_string(),
        }),
    )?;
    let worker_stopped = prepared_completion(
        class,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: request.request_id.clone(),
            handler: request.handler.clone(),
            kind: PluginInvocationFailureKind::WorkerStopped,
            timeout_ms: None,
            reason: "plugin worker stopped before completing invocation".to_string(),
        }),
    )?;
    let oversize = prepared_completion(
        class,
        PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: request.request_id.clone(),
            handler: request.handler.clone(),
            kind: PluginInvocationFailureKind::CompletionTooLarge,
            timeout_ms: None,
            reason: OVERSIZE_COMPLETION_REASON.to_string(),
        }),
    )?;
    Ok(CompletionFallbacks {
        timed_out_bytes: timed_out.encoded.len(),
        worker_stopped_bytes: worker_stopped.encoded.len(),
        oversize_bytes: oversize.encoded.len(),
        timed_out,
        worker_stopped,
        oversize,
    })
}

fn prepared_completion(
    class: PluginInvocationClass,
    result: PluginInvocationResult,
) -> Result<PreparedCompletion, ()> {
    let completion = PluginCompletion {
        class,
        result: result.clone(),
    };
    let encoded = encode_completion(&completion)?;
    Ok(PreparedCompletion {
        completion,
        encoded,
        result,
    })
}

#[must_use]
fn publish_prepared_into(
    completions: &mut CompletionStore,
    state: &AsyncJobState,
    prepared: PreparedCompletion,
) -> bool {
    if !state.terminal.try_seal() {
        return false;
    }
    completions.publish(
        state.reservation,
        prepared.completion,
        prepared.encoded.len(),
        &state.shared.metrics,
    );
    true
}

fn notify_completion(shared: &EngineShared) {
    let notifier = shared
        .completion_notifier
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some(notifier) = notifier {
        notifier();
    }
}

fn remove_deadline_entry(shared: &EngineShared, generation: u64, request_id: &RequestId) {
    let Ok(mut book) = shared.deadlines.lock() else {
        return;
    };
    book.entries
        .retain(|entry| !(entry.generation == generation && entry.request_id == *request_id));
}

fn remove_deadlines_for_generation(shared: &EngineShared, plugin_key: &PluginKey, generation: u64) {
    if let Ok(mut book) = shared.deadlines.lock() {
        book.entries
            .retain(|entry| !(entry.plugin_key == *plugin_key && entry.generation == generation));
        shared.deadline_cvar.notify_all();
    }
}

fn seal_and_publish(
    state: &AsyncJobState,
    result: PluginInvocationResult,
    prepared: Option<PreparedCompletion>,
) {
    if !state.terminal.try_seal() {
        return;
    }
    #[cfg(test)]
    {
        let pause = state
            .shared
            .publication_pause
            .lock()
            .expect("publication pause")
            .as_ref()
            .filter(|pause| pause.generation == state.reservation.generation)
            .cloned();
        if let Some(pause) = pause {
            pause.sealed.send(()).expect("test observes terminal seal");
            pause
                .resume
                .lock()
                .expect("publication resume")
                .recv()
                .expect("test releases publisher");
        }
    }
    let (completion, encoded) = match prepared {
        Some(prepared) => (prepared.completion, prepared.encoded),
        None => {
            let completion = PluginCompletion {
                class: state.class,
                result,
            };
            match encode_completion(&completion) {
                Ok(encoded) if encoded.len() <= state.reservation.payload_bytes => {
                    (completion, encoded)
                }
                _ => (
                    state.fallbacks.oversize.completion.clone(),
                    state.fallbacks.oversize.encoded.clone(),
                ),
            }
        }
    };
    // Plugin code and host callbacks never run while Core holds this lock.
    // A poisoned lock therefore identifies an internal programming error.
    {
        let mut completions = state
            .shared
            .completions
            .lock()
            .expect("plugin completion store mutex poisoned");
        completions.publish(
            state.reservation,
            completion,
            encoded.len(),
            &state.shared.metrics,
        );
    }
    notify_completion(&state.shared);
}

fn complete_job(completion: JobCompletion, result: PluginInvocationResult) {
    match completion {
        JobCompletion::Blocking { result_sender } => {
            let _ = result_sender.send(result);
        }
        JobCompletion::Async(state) => {
            let request_id = match &result {
                PluginInvocationResult::Completed(success) => success.request_id.clone(),
                PluginInvocationResult::Failed(failure) => failure.request_id.clone(),
            };
            seal_and_publish(&state, result, None);
            remove_deadline_entry(&state.shared, state.reservation.generation, &request_id);
        }
    }
}

fn job_class(job: &WorkerJob) -> PluginInvocationClass {
    match &job.completion {
        JobCompletion::Async(state) => state.class,
        JobCompletion::Blocking { .. } => PluginInvocationClass::RequestResponse,
    }
}

fn async_state_of(job: &WorkerJob) -> Option<Arc<AsyncJobState>> {
    match &job.completion {
        JobCompletion::Async(state) => Some(state.clone()),
        JobCompletion::Blocking { .. } => None,
    }
}

fn finish_skipped_job(
    job: WorkerJob,
    metrics: &WorkerMetrics,
    engine_metrics: &PluginWorkerEngineMetrics,
    cancellations: &Mutex<HashMap<RequestId, PluginCancellationToken>>,
    admission: &Mutex<WorkerAdmission>,
) {
    decrement_executor_in_flight(job_class(&job), metrics, engine_metrics, admission);
    cancellations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&job.request.request_id);
    if let JobCompletion::Blocking { result_sender } = job.completion {
        let _ = result_sender.send(PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: job.request.request_id,
            handler: job.request.handler,
            kind: PluginInvocationFailureKind::WorkerStopped,
            timeout_ms: None,
            reason: "plugin worker stopped before completing invocation".to_string(),
        }));
    }
}

fn decrement_executor_in_flight(
    class: PluginInvocationClass,
    metrics: &WorkerMetrics,
    engine_metrics: &PluginWorkerEngineMetrics,
    admission: &Mutex<WorkerAdmission>,
) {
    match class {
        PluginInvocationClass::Background => {
            metrics
                .background_in_flight_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .background_in_flight_jobs
                .fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut admission) = admission.lock() {
                admission.executor_in_flight_bg = admission.executor_in_flight_bg.saturating_sub(1);
            }
        }
        PluginInvocationClass::RequestResponse => {
            metrics
                .request_response_in_flight_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .request_response_in_flight_jobs
                .fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut admission) = admission.lock() {
                admission.executor_in_flight_rr = admission.executor_in_flight_rr.saturating_sub(1);
            }
        }
    }
    metrics.in_flight_jobs.fetch_sub(1, Ordering::SeqCst);
    engine_metrics.in_flight_jobs.fetch_sub(1, Ordering::SeqCst);
}

fn cancel_queued_job(
    job: WorkerJob,
    metrics: &WorkerMetrics,
    engine_metrics: &PluginWorkerEngineMetrics,
) {
    let class = job_class(&job);
    let queue_bytes = job.queue_bytes;
    match class {
        PluginInvocationClass::Background => {
            metrics
                .background_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            metrics
                .background_queued_bytes
                .fetch_sub(queue_bytes, Ordering::SeqCst);
            engine_metrics
                .background_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .background_queued_bytes
                .fetch_sub(queue_bytes, Ordering::SeqCst);
        }
        PluginInvocationClass::RequestResponse => {
            metrics
                .request_response_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            metrics
                .request_response_queued_bytes
                .fetch_sub(queue_bytes, Ordering::SeqCst);
            engine_metrics
                .request_response_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .request_response_queued_bytes
                .fetch_sub(queue_bytes, Ordering::SeqCst);
        }
    }
    metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
    engine_metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
    match job.completion {
        JobCompletion::Blocking { result_sender } => {
            let _ = result_sender.send(PluginInvocationResult::Failed(PluginInvocationFailure {
                request_id: job.request.request_id,
                handler: job.request.handler,
                kind: PluginInvocationFailureKind::WorkerStopped,
                timeout_ms: None,
                reason: "plugin worker stopped before completing invocation".to_string(),
            }));
        }
        JobCompletion::Async(state) => {
            seal_and_publish(
                &state,
                state.fallbacks.worker_stopped.result.clone(),
                Some(state.fallbacks.worker_stopped.clone()),
            );
        }
    }
}

fn run_deadline_waiter(shared: Arc<EngineShared>) {
    loop {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let mut book = shared
            .deadlines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let now = Instant::now();
        let next = book
            .entries
            .iter()
            .map(|entry| entry.at)
            .min()
            .filter(|at| *at > now)
            .map(|at| at.saturating_duration_since(now));
        book = match next {
            Some(timeout) => match shared.deadline_cvar.wait_timeout(book, timeout) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            },
            None if book.entries.iter().any(|entry| entry.at <= now) => book,
            None => shared
                .deadline_cvar
                .wait(book)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        };
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let fired_at = Instant::now();
        let mut expired = Vec::new();
        book.entries.retain(|entry| {
            if entry.at <= fired_at {
                expired.push(entry.clone());
                false
            } else {
                true
            }
        });
        drop(book);
        for entry in expired {
            fire_deadline(&shared, entry);
        }
    }
}

fn fire_deadline(shared: &EngineShared, entry: DeadlineEntry) {
    #[cfg(test)]
    {
        let permits = shared
            .deadline_permits
            .lock()
            .expect("deadline permits")
            .clone();
        if let Some(permits) = permits {
            permits.take(&entry.request_id);
        }
    }
    let worker = {
        let workers = match shared.workers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match workers.get(&entry.plugin_key) {
            Some(worker) => worker.clone(),
            None => return,
        }
    };
    if worker.generation != Some(entry.generation) {
        return;
    }
    let mut admission = match worker.admission.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(tracked) = admission.jobs.remove(&entry.request_id) else {
        return;
    };
    // Cancel only after the admission lock is released: cancellation notifies
    // runtime targets, which must never run under engine locks.
    let JobCompletion::Async(state) = tracked.completion else {
        drop(admission);
        tracked.cancellation.cancel();
        worker.finish_invocation(&entry.request_id);
        return;
    };
    if matches!(tracked.phase, JobPhase::Queued) {
        if let Some(job) = admission.remove_queued(&entry.request_id) {
            cancel_queue_metrics_only(&job, &worker.metrics, &shared.metrics);
        }
    }
    drop(admission);
    worker.finish_invocation(&entry.request_id);
    // Seal the timeout before cancelling: a runtime that reacts to the cancel
    // at once must not win the first-commit race with its own result.
    seal_and_publish(
        &state,
        state.fallbacks.timed_out.result.clone(),
        Some(state.fallbacks.timed_out.clone()),
    );
    tracked.cancellation.cancel();
}

fn cancel_queue_metrics_only(
    job: &WorkerJob,
    metrics: &WorkerMetrics,
    engine_metrics: &PluginWorkerEngineMetrics,
) {
    let class = job_class(job);
    match class {
        PluginInvocationClass::Background => {
            metrics
                .background_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            metrics
                .background_queued_bytes
                .fetch_sub(job.queue_bytes, Ordering::SeqCst);
            engine_metrics
                .background_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .background_queued_bytes
                .fetch_sub(job.queue_bytes, Ordering::SeqCst);
        }
        PluginInvocationClass::RequestResponse => {
            metrics
                .request_response_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            metrics
                .request_response_queued_bytes
                .fetch_sub(job.queue_bytes, Ordering::SeqCst);
            engine_metrics
                .request_response_queued_jobs
                .fetch_sub(1, Ordering::SeqCst);
            engine_metrics
                .request_response_queued_bytes
                .fetch_sub(job.queue_bytes, Ordering::SeqCst);
        }
    }
    metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
    engine_metrics.queued_jobs.fetch_sub(1, Ordering::SeqCst);
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
    PluginInvocationOutcome::new(handler_failed_result(&request, reason))
}

fn handler_failed_result(
    request: &PluginInvocationRequest,
    reason: &str,
) -> PluginInvocationResult {
    PluginInvocationResult::Failed(PluginInvocationFailure {
        request_id: request.request_id.clone(),
        handler: request.handler.clone(),
        kind: PluginInvocationFailureKind::HandlerFailed,
        timeout_ms: None,
        reason: reason.to_string(),
    })
}

#[cfg(test)]
impl PluginWorkerEngine {
    fn try_admit_while_holding_admission_lock(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let worker = self
            .worker_for(&request.handler.plugin_key)
            .expect("plugin must be loaded for lock-contention proof");
        let _guard = worker
            .admission
            .lock()
            .expect("plugin worker admission mutex poisoned");
        self.try_admit(class, request, 1)
    }

    fn try_admit_while_holding_registry_lock(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let _guard = self
            .inner
            .shared
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned");
        self.try_admit(class, request, 1)
    }

    fn try_admit_while_holding_deadline_lock(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let _guard = self
            .inner
            .shared
            .deadlines
            .lock()
            .expect("plugin deadline book mutex poisoned");
        self.try_admit(class, request, 1)
    }

    fn try_admit_while_holding_completion_reservation_lock(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let _guard = self
            .inner
            .shared
            .completions
            .lock()
            .expect("plugin completion reservation mutex poisoned");
        self.try_admit(class, request, 1)
    }

    fn try_admit_while_holding_cancellation_lock(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let worker = self
            .worker_for(&request.handler.plugin_key)
            .expect("plugin must be loaded for lock-contention proof");
        let _guard = worker
            .executor
            .cancellations
            .lock()
            .expect("plugin worker cancellations mutex poisoned");
        self.try_admit(class, request, 1)
    }

    fn tracked_job_count(&self, plugin_key: &PluginKey) -> usize {
        self.worker_for(plugin_key)
            .and_then(|worker| {
                worker
                    .admission
                    .lock()
                    .ok()
                    .map(|admission| admission.jobs.len())
            })
            .unwrap_or_default()
    }

    fn tracked_cancellation_count(&self, plugin_key: &PluginKey) -> usize {
        self.worker_for(plugin_key)
            .and_then(|worker| {
                worker
                    .executor
                    .cancellations
                    .lock()
                    .ok()
                    .map(|cancellations| cancellations.len())
            })
            .unwrap_or_default()
    }

    /// Gate every later deadline delivery on a permit (see `DeadlinePermits`).
    fn gate_deadlines(&self) -> Arc<DeadlinePermits> {
        let permits = Arc::new(DeadlinePermits::default());
        *self
            .inner
            .shared
            .deadline_permits
            .lock()
            .expect("deadline permits") = Some(permits.clone());
        permits
    }

    fn tracked_deadline_count(&self) -> usize {
        self.inner
            .shared
            .deadlines
            .lock()
            .map(|book| book.entries.len())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::{PluginHandlerKind, PluginInvocationContext};
    use crate::manifest::PackageManifest;
    use crate::package::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};

    mod worker_resource_tests {
        include!("plugin_worker_resources_test.rs");
    }

    mod cancel_target_tests {
        include!("plugin_worker_cancel_test.rs");
    }

    mod delivery_pool_tests {
        include!("plugin_worker_delivery_test.rs");
    }

    #[derive(Clone)]
    struct DelayRuntime {
        delay: Duration,
        stopped: Arc<AtomicBool>,
    }

    impl DelayRuntime {
        fn new(delay: Duration) -> Self {
            Self {
                delay,
                stopped: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl PluginRuntime for DelayRuntime {
        fn invoke(
            &self,
            request: PluginInvocationRequest,
            cancellation: PluginCancellationToken,
        ) -> PluginInvocationResult {
            let started = Instant::now();
            while started.elapsed() < self.delay {
                if cancellation.is_cancelled() || self.stopped.load(Ordering::SeqCst) {
                    return PluginInvocationResult::Failed(PluginInvocationFailure {
                        request_id: request.request_id,
                        handler: request.handler,
                        kind: PluginInvocationFailureKind::Cancelled,
                        timeout_ms: None,
                        reason: "delay runtime observed cancellation".to_string(),
                    });
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            PluginInvocationResult::Failed(PluginInvocationFailure {
                request_id: request.request_id,
                handler: request.handler,
                kind: PluginInvocationFailureKind::HandlerFailed,
                timeout_ms: None,
                reason: "delay runtime should lose the first-commit race".to_string(),
            })
        }

        fn stop(&self, _plugin_key: &PluginKey) {
            self.stopped.store(true, Ordering::SeqCst);
        }
    }

    fn manifest() -> PackageManifest {
        PackageManifest {
            name: "test".into(),
            version: "0.1.0".into(),
            kind: ExtensionKind::Plugin,
            botster: ">=0.1.0".into(),
            source: None,
            capabilities: Vec::new(),
            entrypoints: vec![ExtensionEntrypoint {
                runtime: ExtensionRuntime::Lua,
                path: "plugin.lua".into(),
                bootstrap: false,
            }],
            dependencies: Vec::new(),
            features: Vec::new(),
            host_profile: None,
            configuration: None,
            runnable_entrypoints: Vec::new(),
        }
    }

    fn handler(plugin: &PluginKey) -> PluginHandlerRef {
        PluginHandlerRef {
            plugin_key: plugin.clone(),
            kind: PluginHandlerKind::Command,
            handler_id: "run".into(),
        }
    }

    fn request(id: &str, handler: PluginHandlerRef, timeout_ms: u64) -> PluginInvocationRequest {
        PluginInvocationRequest {
            request_id: RequestId(id.into()),
            handler,
            timeout_ms,
            context: PluginInvocationContext {
                client_id: None,
                session_id: None,
                subscription_id: None,
                surface_id: None,
                origin: None,
                metadata: None,
            },
            payload: serde_json::from_value(serde_json::json!({})).expect("empty payload"),
        }
    }

    fn try_admit_retrying_lock_busy(
        engine: &PluginWorkerEngine,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let started = Instant::now();
        loop {
            match engine.try_admit(class, request.clone(), 1) {
                PluginAdmissionResult::Backpressured { reason, .. }
                    if reason == ADMISSION_LOCK_BUSY
                        && started.elapsed() < Duration::from_millis(100) =>
                {
                    std::thread::yield_now();
                }
                other => return other,
            }
        }
    }

    fn load(engine: &PluginWorkerEngine, plugin: &PluginKey, delay: Duration) {
        engine.load_plugin(registration(plugin, Arc::new(DelayRuntime::new(delay))));
    }

    fn registration(
        plugin: &PluginKey,
        runtime: Arc<dyn PluginRuntime>,
    ) -> PluginWorkerRegistration {
        PluginWorkerRegistration {
            load: PluginLoadSpec {
                plugin_key: plugin.clone(),
                package: "test".into(),
                entrypoint: "plugin.lua".into(),
                descriptors: Vec::new(),
                metadata: None,
            },
            manifest: manifest(),
            runtime,
            handlers: vec![PluginHandlerRegistration {
                handler: handler(plugin),
                required_capability: None,
            }],
            resources: Vec::new(),
        }
    }

    fn publish_immediate_failure(
        engine: &PluginWorkerEngine,
        plugin: &PluginKey,
        request_id: &str,
    ) -> usize {
        let missing = PluginHandlerRef {
            plugin_key: plugin.clone(),
            kind: PluginHandlerKind::Command,
            handler_id: "missing".into(),
        };
        assert!(matches!(
            try_admit_retrying_lock_busy(
                engine,
                PluginInvocationClass::Background,
                request(request_id, missing, 1_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let worker = engine.worker_for(plugin).expect("worker");
        let store = engine
            .inner
            .shared
            .completions
            .lock()
            .expect("completion store");
        store
            .last_published_len(worker.generation.expect("generation"))
            .expect("published completion")
    }

    fn completion_request_id(item: &PluginCompletionItem) -> &RequestId {
        match &item.completion.result {
            PluginInvocationResult::Completed(success) => &success.request_id,
            PluginInvocationResult::Failed(failure) => &failure.request_id,
        }
    }

    fn completion_store_counts(engine: &PluginWorkerEngine) -> (usize, usize, usize, usize) {
        engine
            .inner
            .shared
            .completions
            .lock()
            .expect("completion store")
            .counts()
    }

    #[test]
    fn completion_store_admits_metadata_before_all_three_publication_paths() {
        let payload_bytes = 4096;
        let charged_bytes = payload_bytes + completion_store::metadata_bytes();
        for (name, timeout) in [("run", 1000), ("run", 0), ("missing", 1000)] {
            let plugin = PluginKey(format!("metadata-{name}-{timeout}"));
            let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
                completion_queue_byte_capacity: charged_bytes - 1,
                ..PluginWorkerEngineConfig::default()
            });
            load(&engine, &plugin, Duration::from_secs(1));
            let mut reference = handler(&plugin);
            reference.handler_id = name.into();
            let rejected = engine.try_admit(
                PluginInvocationClass::Background,
                request("refused", reference, timeout),
                payload_bytes,
            );
            assert!(
                matches!(rejected, PluginAdmissionResult::RejectedBudget { reason, .. }
                if reason == "completion reservation exceeds engine completion pool byte capacity")
            );
            assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
            assert_eq!(engine.debug_snapshot().reserved_completion_count, 0);
        }
        let plugin = PluginKey("metadata-exact".into());
        let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
            completion_queue_byte_capacity: charged_bytes,
            ..PluginWorkerEngineConfig::default()
        });
        load(&engine, &plugin, Duration::from_secs(1));
        let admitted = engine.try_admit(
            PluginInvocationClass::Background,
            request("exact", handler(&plugin), 0),
            payload_bytes,
        );
        assert!(
            matches!(admitted, PluginAdmissionResult::Queued { reservation_bytes, .. } if reservation_bytes == charged_bytes)
        );
        assert_eq!(completion_store_counts(&engine), (1, 1, 1, charged_bytes));
        assert_eq!(
            engine.debug_snapshot().reserved_completion_bytes,
            charged_bytes
        );
        assert!(matches!(
            engine.try_admit(
                PluginInvocationClass::Background,
                request("full", handler(&plugin), 0),
                payload_bytes
            ),
            PluginAdmissionResult::Backpressured { .. }
        ));
        assert_eq!(completion_store_counts(&engine), (1, 1, 1, charged_bytes));
        assert_eq!(engine.drain_completions(1, payload_bytes).item_count, 1);
        assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
        assert_eq!(engine.debug_snapshot().reserved_completion_bytes, 0);
        assert_eq!(engine.drain_completions(1, payload_bytes).item_count, 0);
    }

    #[test]
    fn completion_store_identity_exhaustion_never_creates_rows_or_credits() {
        let generations = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate_worker_generation(&generations), Some(u64::MAX - 1));
        assert_eq!(allocate_worker_generation(&generations), None);
        assert_eq!(allocate_worker_generation(&generations), None);
        let engine = PluginWorkerEngine::new();
        for number in 0..2 {
            let plugin = PluginKey(format!("exhausted-{number}"));
            let worker = WorkerState::with_generation(
                registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
                engine.inner.shared.clone(),
                None,
                None,
            );
            engine
                .inner
                .shared
                .workers
                .lock()
                .expect("workers")
                .insert(plugin.clone(), worker);
            for (name, timeout) in [("run", 1000), ("run", 0), ("missing", 1000)] {
                let mut reference = handler(&plugin);
                reference.handler_id = name.into();
                assert!(
                    matches!(engine.try_admit(PluginInvocationClass::Background, request("no-generation", reference, timeout), 1),
                    PluginAdmissionResult::RejectedBudget { reason, .. } if reason == "plugin worker generation identities exhausted")
                );
                assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
                assert_eq!(engine.tracked_deadline_count(), 0);
            }
            engine.unload_plugin(PluginUnloadSpec {
                request_id: RequestId("unload".into()),
                plugin_key: plugin,
                cleanup: PluginCleanupScope::DescriptorsAndResources,
            });
        }
        let plugin = PluginKey("slot-exhaustion".into());
        load(&engine, &plugin, Duration::ZERO);
        engine
            .inner
            .shared
            .completions
            .lock()
            .expect("store")
            .exhaust_slot_identities();
        for name in ["run", "missing"] {
            let mut reference = handler(&plugin);
            reference.handler_id = name.into();
            assert!(
                matches!(engine.try_admit(PluginInvocationClass::Background, request("no-slot", reference, 0), 1),
                PluginAdmissionResult::RejectedBudget { reason, .. } if reason == "plugin completion reservation identities exhausted")
            );
            assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
        }
    }

    #[test]
    fn completion_store_drop_releases_undrained_credits_once() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("drop-credits".into());
        load(&engine, &plugin, Duration::ZERO);
        let metrics = engine.inner.shared.metrics.clone();
        let worker_metrics = engine.worker_for(&plugin).expect("worker").metrics;
        publish_immediate_failure(&engine, &plugin, "not-drained");
        assert_eq!(metrics.reserved_completion_count.load(Ordering::SeqCst), 1);
        drop(engine);
        assert_eq!(metrics.reserved_completion_count.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.reserved_completion_bytes.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.undrained_completions.load(Ordering::SeqCst), 0);
        assert_eq!(
            worker_metrics
                .reserved_completion_count
                .load(Ordering::SeqCst),
            0
        );
        assert_eq!(
            worker_metrics
                .reserved_completion_bytes
                .load(Ordering::SeqCst),
            0
        );
        assert_eq!(
            worker_metrics.undrained_completions.load(Ordering::SeqCst),
            0
        );
    }

    struct HeldRuntime {
        entered: mpsc::SyncSender<()>,
        released: Mutex<bool>,
        wake: Condvar,
    }

    impl PluginRuntime for HeldRuntime {
        fn invoke(
            &self,
            request: PluginInvocationRequest,
            _cancellation: PluginCancellationToken,
        ) -> PluginInvocationResult {
            self.entered.send(()).expect("test observes invocation");
            let mut released = self.released.lock().expect("runtime gate");
            while !*released {
                released = self.wake.wait(released).expect("runtime gate");
            }
            handler_failed_result(&request, "runtime released")
        }

        fn stop(&self, _plugin_key: &PluginKey) {
            *self.released.lock().expect("runtime gate") = true;
            self.wake.notify_all();
        }
    }

    fn deadline_publication_crosses_reload(publish_before_retire: bool) {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("paused-deadline".into());
        let (entered, entered_rx) = mpsc::sync_channel(1);
        let runtime = Arc::new(HeldRuntime {
            entered,
            released: Mutex::new(false),
            wake: Condvar::new(),
        });
        engine.load_plugin(registration(&plugin, runtime));
        let old = engine.worker_for(&plugin).expect("old worker");
        let generation = old.generation.expect("generation");
        let (sealed, sealed_rx) = mpsc::sync_channel(1);
        let (resume, resume_rx) = mpsc::channel();
        *engine.inner.shared.publication_pause.lock().expect("pause") =
            Some(Arc::new(PublicationPause {
                generation,
                sealed,
                resume: Mutex::new(resume_rx),
            }));
        let (notified, notified_rx) = mpsc::channel();
        let shared = Arc::downgrade(&engine.inner.shared);
        engine.install_completion_notifier(Arc::new(move || {
            let shared = shared.upgrade().expect("engine alive");
            let _store = shared
                .completions
                .try_lock()
                .expect("notification after store unlock");
            notified.send(()).expect("notification observed");
        }));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("same", handler(&plugin), 60_000)
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("handler entered");
        {
            let mut deadlines = engine.inner.shared.deadlines.lock().expect("deadline book");
            deadlines
                .entries
                .iter_mut()
                .find(|entry| entry.generation == generation)
                .expect("admitted deadline")
                .at = Instant::now();
        }
        engine.inner.shared.deadline_cvar.notify_all();
        sealed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("real deadline waiter sealed before publication");
        assert_eq!(
            old.metrics.reserved_completion_count.load(Ordering::SeqCst),
            1
        );
        assert_eq!(old.metrics.undrained_completions.load(Ordering::SeqCst), 0);
        if publish_before_retire {
            resume.send(()).expect("publish before retire");
            notified_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("deadline published");
        }
        load(&engine, &plugin, Duration::ZERO);
        assert_ne!(
            engine.worker_for(&plugin).expect("replacement").generation,
            Some(generation)
        );
        assert_eq!(
            old.metrics.reserved_completion_count.load(Ordering::SeqCst),
            1,
            "reload cannot release an outstanding reservation"
        );
        publish_immediate_failure(&engine, &plugin, "same");
        notified_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("replacement published");
        assert_eq!(completion_store_counts(&engine).2, 2);
        let first = engine.drain_completions(1, usize::MAX);
        assert_eq!(first.item_count, 1);
        let expected = if publish_before_retire {
            PluginInvocationFailureKind::TimedOut
        } else {
            PluginInvocationFailureKind::HandlerFailed
        };
        assert!(
            matches!(&first.completions[0].completion.result, PluginInvocationResult::Failed(failure) if failure.kind == expected)
        );
        if !publish_before_retire {
            assert_eq!(
                old.metrics.reserved_completion_count.load(Ordering::SeqCst),
                1
            );
            assert_eq!(
                completion_store_counts(&engine).1,
                1,
                "empty retired route stays for unpublished reservation"
            );
            resume.send(()).expect("publish into retired generation");
            notified_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("late deadline published");
        }
        let second = engine.drain_completions(1, usize::MAX);
        assert_eq!(second.item_count, 1);
        let expected = if publish_before_retire {
            PluginInvocationFailureKind::HandlerFailed
        } else {
            PluginInvocationFailureKind::TimedOut
        };
        assert!(
            matches!(&second.completions[0].completion.result, PluginInvocationResult::Failed(failure) if failure.kind == expected)
        );
        assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
        assert_eq!(
            old.metrics.reserved_completion_count.load(Ordering::SeqCst),
            0
        );
        assert_eq!(
            old.metrics.reserved_completion_bytes.load(Ordering::SeqCst),
            0
        );
        assert_eq!(old.metrics.undrained_completions.load(Ordering::SeqCst), 0);
        assert_eq!(engine.debug_snapshot().reserved_completion_count, 0);
        assert!(!second.has_remaining);
        assert_eq!(engine.drain_completions(1, usize::MAX).item_count, 0);
    }

    #[test]
    fn completion_store_keeps_retired_route_for_sealed_unpublished_deadline() {
        deadline_publication_crosses_reload(false);
    }

    #[test]
    fn completion_store_retires_published_deadline_before_replacement_front() {
        deadline_publication_crosses_reload(true);
    }

    #[test]
    fn blocked_worker_does_not_starve_a_later_fitting_worker() {
        let engine = PluginWorkerEngine::new();
        let first = PluginKey("a-large".into());
        let second = PluginKey("b-small".into());
        load(&engine, &first, Duration::from_millis(1));
        load(&engine, &second, Duration::from_millis(1));
        let large_request = "large".repeat(64);
        let large_len = publish_immediate_failure(&engine, &first, &large_request);
        let small_len = publish_immediate_failure(&engine, &second, "small");
        assert!(large_len > small_len);

        let fitting = engine.drain_completions(8, small_len);

        assert_eq!(fitting.item_count, 1);
        assert_eq!(completion_request_id(&fitting.completions[0]).0, "small");
        assert!(fitting.has_remaining);
        let restored = engine.drain_completions(8, large_len);
        assert_eq!(restored.item_count, 1);
        assert_eq!(
            completion_request_id(&restored.completions[0]).0,
            large_request
        );
        assert!(!restored.has_remaining);
    }

    #[test]
    fn blocked_leftover_does_not_starve_a_fitting_worker() {
        let engine = PluginWorkerEngine::new();
        let removed = PluginKey("removed-large".into());
        let active = PluginKey("active-small".into());
        load(&engine, &removed, Duration::from_millis(1));
        load(&engine, &active, Duration::from_millis(1));
        let large_request = "leftover".repeat(64);
        let large_len = publish_immediate_failure(&engine, &removed, &large_request);
        let small_len = publish_immediate_failure(&engine, &active, "active");
        engine.unload_plugin(PluginUnloadSpec {
            request_id: RequestId("unload".into()),
            plugin_key: removed,
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        });

        let fitting = engine.drain_completions(8, small_len);

        assert_eq!(fitting.item_count, 1);
        assert_eq!(completion_request_id(&fitting.completions[0]).0, "active");
        assert!(fitting.has_remaining);
        let restored = engine.drain_completions(8, large_len);
        assert_eq!(restored.item_count, 1);
        assert_eq!(
            completion_request_id(&restored.completions[0]).0,
            large_request
        );
        assert!(!restored.has_remaining);
    }

    #[test]
    fn blocked_head_preserves_fifo_within_one_worker() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("fifo".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let large_request = "first".repeat(64);
        let large_len = publish_immediate_failure(&engine, &plugin, &large_request);
        let small_len = publish_immediate_failure(&engine, &plugin, "second");
        assert!(large_len > small_len);

        let blocked = engine.drain_completions(8, small_len);

        assert_eq!(blocked.item_count, 0);
        assert!(blocked.completions.is_empty());
        assert!(blocked.has_remaining);
        let restored = engine.drain_completions(8, usize::MAX);
        assert_eq!(restored.item_count, 2);
        assert_eq!(
            restored
                .completions
                .iter()
                .map(completion_request_id)
                .map(|id| id.0.as_str())
                .collect::<Vec<_>>(),
            vec![large_request.as_str(), "second"]
        );
    }

    #[test]
    fn item_cap_stops_after_one_fitting_front() {
        let engine = PluginWorkerEngine::new();
        let first = PluginKey("z-first".into());
        let second = PluginKey("a-second".into());
        load(&engine, &first, Duration::from_millis(1));
        load(&engine, &second, Duration::from_millis(1));
        let first_len = publish_immediate_failure(&engine, &first, "first");
        let second_len = publish_immediate_failure(&engine, &second, "second");
        assert!(
            first_len < second_len,
            "numeric front size, not plugin name, selects first"
        );

        let limited = engine.drain_completions(1, usize::MAX);

        assert_eq!(limited.item_count, 1);
        assert_eq!(completion_request_id(&limited.completions[0]).0, "first");
        assert!(limited.has_remaining);
        let rest = engine.drain_completions(1, usize::MAX);
        assert_eq!(completion_request_id(&rest.completions[0]).0, "second");
        assert!(!rest.has_remaining);
    }

    #[test]
    fn nonfitting_head_remains_counted_and_queued() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("counted".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let encoded_len = publish_immediate_failure(&engine, &plugin, "pending");
        assert_eq!(engine.debug_snapshot().undrained_completions, 1);

        let blocked = engine.drain_completions(8, encoded_len - 1);

        assert_eq!(blocked.item_count, 0);
        assert!(blocked.has_remaining);
        assert_eq!(engine.debug_snapshot().undrained_completions, 1);
        let restored = engine.drain_completions(8, encoded_len);
        assert_eq!(restored.item_count, 1);
        assert_eq!(completion_request_id(&restored.completions[0]).0, "pending");
        assert!(!restored.has_remaining);
    }

    #[test]
    fn try_admit_returns_backpressured_when_admission_lock_is_held() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("lock".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let result = engine.try_admit_while_holding_admission_lock(
            PluginInvocationClass::Background,
            request("busy", handler(&plugin), 1_000),
        );
        assert!(matches!(
            result,
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
    }

    #[test]
    fn immediate_completion_notifies_after_store_and_admission_unlock() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("immediate-notify".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let admission = engine.worker_for(&plugin).expect("worker").admission;
        let shared = Arc::downgrade(&engine.inner.shared);
        let (sender, receiver) = mpsc::channel();
        engine.install_completion_notifier(Arc::new(move || {
            let _admission = admission.try_lock().expect("admission lock is released");
            let shared = shared.upgrade().expect("engine alive");
            let _store = shared
                .completions
                .try_lock()
                .expect("store lock is released");
            assert!(
                shared.metrics.undrained_completions.load(Ordering::SeqCst) > 0,
                "completion is published"
            );
            sender.send(()).expect("notification receiver");
        }));
        let missing = PluginHandlerRef {
            plugin_key: plugin,
            kind: PluginHandlerKind::Command,
            handler_id: "missing".into(),
        };

        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("immediate", missing, 1_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));

        receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("completion notification");
        assert!(receiver.try_recv().is_err(), "one notification");
        assert_eq!(engine.drain_completions(1, usize::MAX).item_count, 1);
    }

    #[test]
    fn executed_completion_notifies_after_store_unlock() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("executed-notify".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let shared = Arc::downgrade(&engine.inner.shared);
        let (sender, receiver) = mpsc::channel();
        engine.install_completion_notifier(Arc::new(move || {
            let shared = shared.upgrade().expect("engine alive");
            let _store = shared
                .completions
                .try_lock()
                .expect("store lock is released");
            assert!(
                shared.metrics.undrained_completions.load(Ordering::SeqCst) > 0,
                "completion is published"
            );
            sender.send(()).expect("notification receiver");
        }));

        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("executed", handler(&plugin), 1_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));

        receiver
            .recv_timeout(Duration::from_millis(250))
            .expect("completion notification");
        assert!(receiver.try_recv().is_err(), "one notification");
        assert_eq!(engine.drain_completions(1, usize::MAX).item_count, 1);
    }

    #[test]
    fn notifier_install_signals_an_existing_completion() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("late-notify".into());
        load(&engine, &plugin, Duration::from_millis(1));
        let missing = PluginHandlerRef {
            plugin_key: plugin,
            kind: PluginHandlerKind::Command,
            handler_id: "missing".into(),
        };
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("already-published", missing, 1_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let (sender, receiver) = mpsc::channel();

        engine.install_completion_notifier(Arc::new(move || {
            sender.send(()).expect("notification receiver");
        }));

        receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("reconciled completion notification");
        assert!(receiver.try_recv().is_err(), "one notification");
        assert_eq!(engine.drain_completions(1, usize::MAX).item_count, 1);
    }

    #[test]
    fn deadline_first_then_unload_keeps_only_timed_out() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("deadline-first".into());
        load(&engine, &plugin, Duration::from_millis(200));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("job", handler(&plugin), 10),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let started = Instant::now();
        let mut completions = Vec::new();
        while started.elapsed() < Duration::from_millis(250) {
            completions.extend(
                engine
                    .drain_completions(8, usize::MAX)
                    .completions
                    .into_iter()
                    .map(|item| item.completion),
            );
            if !completions.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(matches!(
            completions.as_slice(),
            [PluginCompletion {
                result: PluginInvocationResult::Failed(failure),
                ..
            }] if failure.kind == PluginInvocationFailureKind::TimedOut
        ));
        engine.unload_plugin(PluginUnloadSpec {
            request_id: RequestId("unload".into()),
            plugin_key: plugin,
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        });
        assert!(engine
            .drain_completions(8, usize::MAX)
            .completions
            .is_empty());
    }

    #[test]
    fn unload_first_then_deadline_keeps_only_worker_stopped() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("unload-first".into());
        load(&engine, &plugin, Duration::from_secs(2));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("job", handler(&plugin), 5_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        engine.unload_plugin(PluginUnloadSpec {
            request_id: RequestId("unload".into()),
            plugin_key: plugin,
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        });
        let drain = engine.drain_completions(8, usize::MAX);
        assert!(matches!(
            drain.completions.as_slice(),
            [PluginCompletionItem {
                completion: PluginCompletion {
                    result: PluginInvocationResult::Failed(failure),
                    ..
                },
                ..
            }] if failure.kind == PluginInvocationFailureKind::WorkerStopped
        ));
        std::thread::sleep(Duration::from_millis(20));
        assert!(engine
            .drain_completions(8, usize::MAX)
            .completions
            .is_empty());
    }

    #[test]
    fn late_handler_after_timeout_publishes_nothing_more() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("late".into());
        load(&engine, &plugin, Duration::from_millis(80));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("job", handler(&plugin), 5),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(200)
            && engine
                .drain_completions(1, usize::MAX)
                .completions
                .is_empty()
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(100));
        assert!(engine
            .drain_completions(8, usize::MAX)
            .completions
            .is_empty());
    }

    #[test]
    fn idle_drop_joins_deadline_waiter_immediately() {
        let started = Instant::now();
        drop(PluginWorkerEngine::new());
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn drop_with_future_deadline_does_not_wait_for_deadline() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("future-drop".into());
        load(&engine, &plugin, Duration::from_secs(30));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("job", handler(&plugin), 30_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let started = Instant::now();
        drop(engine);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn try_admit_returns_backpressured_when_an_internal_admission_lock_is_held() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("locks".into());
        load(&engine, &plugin, Duration::from_millis(1));
        assert!(matches!(
            engine.try_admit_while_holding_registry_lock(
                PluginInvocationClass::Background,
                request("reg", handler(&plugin), 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        assert!(matches!(
            engine.try_admit_while_holding_deadline_lock(
                PluginInvocationClass::Background,
                request("dead", handler(&plugin), 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        assert!(matches!(
            engine.try_admit_while_holding_completion_reservation_lock(
                PluginInvocationClass::Background,
                request("completion", handler(&plugin), 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        assert!(matches!(
            engine.try_admit_while_holding_cancellation_lock(
                PluginInvocationClass::Background,
                request("cancel", handler(&plugin), 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        assert!(matches!(
            engine.try_admit_while_holding_admission_lock(
                PluginInvocationClass::Background,
                request("zero", handler(&plugin), 0),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        let missing = PluginHandlerRef {
            plugin_key: plugin.clone(),
            kind: PluginHandlerKind::Command,
            handler_id: "missing".into(),
        };
        assert!(matches!(
            engine.try_admit_while_holding_admission_lock(
                PluginInvocationClass::Background,
                request("fail", missing, 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
        let missing = PluginHandlerRef {
            plugin_key: plugin,
            kind: PluginHandlerKind::Command,
            handler_id: "missing-completion-lock".into(),
        };
        assert!(matches!(
            engine.try_admit_while_holding_completion_reservation_lock(
                PluginInvocationClass::Background,
                request("fail-completion", missing, 1_000),
            ),
            PluginAdmissionResult::Backpressured { reason, .. } if reason == ADMISSION_LOCK_BUSY
        ));
    }

    #[test]
    fn stale_deadline_does_not_seal_reloaded_generation_with_reused_request_id() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("reload-deadline".into());
        load(&engine, &plugin, Duration::from_secs(5));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("same", handler(&plugin), 40),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        load(&engine, &plugin, Duration::from_secs(5));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("same", handler(&plugin), 5_000),
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
        std::thread::sleep(Duration::from_millis(80));
        assert!(engine
            .drain_completions(8, usize::MAX)
            .completions
            .is_empty());
        assert_eq!(engine.tracked_job_count(&plugin), 1);
    }

    #[test]
    fn queued_timeouts_and_fast_completions_do_not_retain_private_tracking() {
        let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
            per_plugin_queue_capacity: 32,
            per_plugin_executor_concurrency: 2,
            reserved_request_response_executors: 1,
            background_queue_capacity: 32,
            ..PluginWorkerEngineConfig::default()
        });
        let plugin = PluginKey("bounded".into());
        load(&engine, &plugin, Duration::from_secs(5));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("occupy", handler(&plugin), 5_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        for index in 0..8 {
            assert!(matches!(
                try_admit_retrying_lock_busy(
                    &engine,
                    PluginInvocationClass::Background,
                    request(&format!("expire-{index}"), handler(&plugin), 20),
                ),
                PluginAdmissionResult::Queued { .. }
            ));
        }
        let started = Instant::now();
        let mut timed_out = 0;
        while started.elapsed() < Duration::from_millis(400) && timed_out < 8 {
            timed_out += engine
                .drain_completions(8, usize::MAX)
                .completions
                .into_iter()
                .filter(|item| {
                    matches!(
                        item.completion.result,
                        PluginInvocationResult::Failed(ref failure)
                            if failure.kind == PluginInvocationFailureKind::TimedOut
                    )
                })
                .count();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(timed_out, 8);
        assert_eq!(engine.tracked_job_count(&plugin), 1);
        assert_eq!(engine.tracked_cancellation_count(&plugin), 1);
        assert_eq!(engine.tracked_deadline_count(), 1);

        let fast = PluginKey("fast".into());
        load(&engine, &fast, Duration::from_millis(1));
        assert!(matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("fast", handler(&fast), 30_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(250) && engine.tracked_job_count(&fast) > 0
        {
            let _ = engine.drain_completions(8, usize::MAX);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(engine.tracked_job_count(&fast), 0);
        assert_eq!(engine.tracked_cancellation_count(&fast), 0);
        assert_eq!(engine.tracked_deadline_count(), 1);
    }
}

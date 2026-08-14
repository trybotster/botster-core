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
    PluginCleanupScope, PluginCompletion, PluginCompletionDrain, PluginDescriptorRef,
    PluginHandlerRef, PluginInvocationClass, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec, PluginReloadSpec,
    PluginResourceRef, PluginUnloadSpec, PluginWorkerEvent, QueueSource,
};
use crate::capability::Capability;
use crate::manifest::PackageManifest;
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;

static NEXT_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

const DEFAULT_QUEUE_BYTE_CAPACITY: usize = 1024 * 1024;
const OVERSIZE_COMPLETION_REASON: &str = "completion exceeded reserved byte budget";
const ADMISSION_LOCK_BUSY: &str = "admission lock busy";

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
    /// Maximum reserved async completions per plugin worker.
    pub completion_queue_capacity: usize,
    /// Maximum reserved async completion bytes per plugin worker.
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
    /// Whether the completion reservation pool is at its count or byte bound.
    pub completions_saturated: bool,
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
    /// Configured completion reservation count.
    pub configured_completion_queue_capacity: usize,
    /// Configured completion reservation byte capacity.
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
    /// Reserved async completion slots across live and retiring workers.
    pub reserved_completion_count: usize,
    /// Reserved async completion bytes across live and retiring workers.
    pub reserved_completion_bytes: usize,
    /// Published completions the host has not drained.
    pub undrained_completions: usize,
    /// Whether any RequestResponse queue is at its count or byte bound.
    pub request_response_saturated: bool,
    /// Whether any Background queue is at its count or byte bound.
    pub background_saturated: bool,
    /// Whether any completion reservation pool is at its count or byte bound.
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
    leftover_completions: Mutex<VecDeque<MailboxItem>>,
    metrics: Arc<PluginWorkerEngineMetrics>,
    deadlines: Mutex<DeadlineBook>,
    deadline_cvar: Condvar,
    stopping: AtomicBool,
}

#[derive(Default)]
struct DeadlineBook {
    entries: Vec<DeadlineEntry>,
}

#[derive(Clone)]
struct DeadlineEntry {
    at: Instant,
    plugin_key: PluginKey,
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
            config.completion_queue_byte_capacity > 0,
            "completion queue byte capacity must be greater than zero"
        );

        let shared = Arc::new(EngineShared {
            config,
            workers: Mutex::new(HashMap::new()),
            leftover_completions: Mutex::new(VecDeque::new()),
            metrics: Arc::new(PluginWorkerEngineMetrics::default()),
            deadlines: Mutex::new(DeadlineBook::default()),
            deadline_cvar: Condvar::new(),
            stopping: AtomicBool::new(false),
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

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        let plugin_key = registration.load.plugin_key.clone();
        let worker = WorkerState::new(registration, self.inner.shared.clone());

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
    /// Never blocks on job completion, `recv`, sleep, or a contended mutex.
    /// A busy registry or admission lock is [`PluginAdmissionResult::Backpressured`].
    pub fn try_admit(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
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

        let handler = worker.handlers.get(&request.handler).cloned();
        if let Some(handler) = &handler {
            if let Some(required_capability) = &handler.required_capability {
                if !worker.manifest.capabilities.contains(required_capability) {
                    return self.admit_immediate_failure(
                        &worker,
                        class,
                        request,
                        "plugin handler requires a capability missing from package metadata",
                    );
                }
            }
        } else {
            return self.admit_immediate_failure(
                &worker,
                class,
                request,
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
        let reservation_bytes = queue_bytes
            .max(fallbacks.timed_out_bytes)
            .max(fallbacks.worker_stopped_bytes)
            .max(fallbacks.oversize_bytes);

        let plugin_key = request.handler.plugin_key.clone();
        let mut admission = match worker.admission.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_for(&plugin_key)),
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

        let class_capacity = self.inner.shared.config.class_queue_capacity(class);
        let (queued_count, queued_bytes) = admission.queue_occupancy(class);
        if queued_count >= class_capacity || queued_bytes + queue_bytes > class_byte_capacity {
            self.record_class_pressure(class, &worker);
            return self.admission_backpressured(
                class,
                request,
                "plugin worker class queue is at capacity",
                Some(self.backpressure_for(&plugin_key)),
            );
        }
        if admission.reserved_completion_count + 1
            > self.inner.shared.config.completion_queue_capacity
            || admission.reserved_completion_bytes + reservation_bytes
                > self.inner.shared.config.completion_queue_byte_capacity
        {
            worker
                .metrics
                .completion_pressure_events
                .fetch_add(1, Ordering::SeqCst);
            self.inner
                .shared
                .metrics
                .completion_pressure_events
                .fetch_add(1, Ordering::SeqCst);
            return self.admission_backpressured(
                class,
                request,
                "plugin completion reservation pool is at capacity",
                Some(self.backpressure_for(&plugin_key)),
            );
        }

        let mut deadlines = match self.inner.shared.deadlines.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_for(&plugin_key)),
                );
            }
        };

        let request_id = request.request_id.clone();
        let timeout_ms = request.timeout_ms;
        let already_expired = timeout_ms == 0;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let cancellation = PluginCancellationToken::new();
        worker.track_invocation(request_id.clone(), cancellation.clone());

        let async_state = Arc::new(AsyncJobState {
            class,
            reservation_bytes,
            terminal: JobTerminal::new(),
            fallbacks,
            worker: worker.clone(),
        });
        let job = WorkerJob {
            request,
            cancellation: cancellation.clone(),
            queue_bytes,
            completion: JobCompletion::Async(async_state.clone()),
        };

        admission.reserved_completion_count += 1;
        admission.reserved_completion_bytes += reservation_bytes;
        worker
            .metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        worker
            .metrics
            .reserved_completion_bytes
            .fetch_add(reservation_bytes, Ordering::SeqCst);
        self.inner
            .shared
            .metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .shared
            .metrics
            .reserved_completion_bytes
            .fetch_add(reservation_bytes, Ordering::SeqCst);

        if already_expired {
            admission.jobs.insert(
                request_id.clone(),
                TrackedJob {
                    phase: JobPhase::Queued,
                    completion: JobCompletion::Async(async_state.clone()),
                    cancellation: cancellation.clone(),
                },
            );
            drop(deadlines);
            drop(admission);
            cancellation.cancel();
            seal_and_publish(
                &async_state,
                async_state.fallbacks.timed_out.result.clone(),
                Some(async_state.fallbacks.timed_out.clone()),
            );
            remove_tracked_job(&worker, &request_id);
        } else {
            admission.push_queued(class, job, &worker);
            deadlines.entries.push(DeadlineEntry {
                at: deadline,
                plugin_key,
                request_id: request_id.clone(),
            });
            drop(deadlines);
            worker.work_cvar.notify_one();
            self.inner.shared.deadline_cvar.notify_one();
            drop(admission);
        }

        PluginAdmissionResult::Queued {
            request_id,
            class,
            queue_bytes,
            reservation_bytes,
        }
    }

    /// Drain previously published async completions without waiting.
    ///
    /// Returns at most `max_items` completions whose encoded sizes sum to at
    /// most `max_bytes`. A completion that does not fit the remaining budget is
    /// left in the mailbox.
    pub fn drain_completions(&self, max_items: usize, max_bytes: usize) -> PluginCompletionDrain {
        if max_items == 0 || max_bytes == 0 {
            return PluginCompletionDrain::default();
        }

        let mut drain = PluginCompletionDrain::default();
        {
            let mut leftover = self
                .inner
                .shared
                .leftover_completions
                .lock()
                .expect("plugin leftover completions mutex poisoned");
            while drain.item_count < max_items {
                let Some(front) = leftover.front() else {
                    break;
                };
                if drain.byte_count + front.encoded_len > max_bytes {
                    return drain;
                }
                let item = leftover.pop_front().expect("front existed before pop");
                self.inner
                    .shared
                    .metrics
                    .reserved_completion_count
                    .fetch_sub(1, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .reserved_completion_bytes
                    .fetch_sub(item.reservation_bytes, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .undrained_completions
                    .fetch_sub(1, Ordering::SeqCst);
                drain.item_count += 1;
                drain.byte_count += item.encoded_len;
                drain.completions.push(item.completion);
            }
        }
        if drain.item_count >= max_items {
            return drain;
        }

        let mut workers = self
            .inner
            .shared
            .workers
            .lock()
            .expect("plugin worker engine mutex poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        workers.sort_by(|left, right| left.plugin_key.0.cmp(&right.plugin_key.0));

        for worker in workers {
            let mut admission = worker
                .admission
                .lock()
                .expect("plugin worker admission mutex poisoned");
            while drain.item_count < max_items {
                let Some(front) = admission.mailbox.front() else {
                    break;
                };
                if drain.byte_count + front.encoded_len > max_bytes {
                    return drain;
                }
                let item = admission
                    .mailbox
                    .pop_front()
                    .expect("front existed before pop");
                admission.reserved_completion_count =
                    admission.reserved_completion_count.saturating_sub(1);
                admission.reserved_completion_bytes = admission
                    .reserved_completion_bytes
                    .saturating_sub(item.reservation_bytes);
                worker
                    .metrics
                    .reserved_completion_count
                    .fetch_sub(1, Ordering::SeqCst);
                worker
                    .metrics
                    .reserved_completion_bytes
                    .fetch_sub(item.reservation_bytes, Ordering::SeqCst);
                worker
                    .metrics
                    .undrained_completions
                    .fetch_sub(1, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .reserved_completion_count
                    .fetch_sub(1, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .reserved_completion_bytes
                    .fetch_sub(item.reservation_bytes, Ordering::SeqCst);
                self.inner
                    .shared
                    .metrics
                    .undrained_completions
                    .fetch_sub(1, Ordering::SeqCst);
                drain.item_count += 1;
                drain.byte_count += item.encoded_len;
                drain.completions.push(item.completion);
            }
            if drain.item_count >= max_items {
                break;
            }
        }
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
        let completions_saturated = plugins.iter().any(|plugin| plugin.completions_saturated);

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
            reserved_completion_count: self
                .inner
                .shared
                .metrics
                .reserved_completion_count
                .load(Ordering::SeqCst),
            reserved_completion_bytes: self
                .inner
                .shared
                .metrics
                .reserved_completion_bytes
                .load(Ordering::SeqCst),
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
        reason: &str,
    ) -> PluginAdmissionResult {
        let queue_bytes = plugin_invocation_queue_bytes(&request).unwrap_or(0);
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
        let reservation_bytes = queue_bytes
            .max(fallbacks.timed_out_bytes)
            .max(fallbacks.worker_stopped_bytes)
            .max(fallbacks.oversize_bytes);
        let plugin_key = request.handler.plugin_key.clone();
        let mut admission = match worker.admission.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return self.admission_backpressured(
                    class,
                    request,
                    ADMISSION_LOCK_BUSY,
                    Some(self.backpressure_for(&plugin_key)),
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
        if admission.reserved_completion_count + 1
            > self.inner.shared.config.completion_queue_capacity
            || admission.reserved_completion_bytes + reservation_bytes
                > self.inner.shared.config.completion_queue_byte_capacity
        {
            return self.admission_backpressured(
                class,
                request,
                "plugin completion reservation pool is at capacity",
                Some(self.backpressure_for(&plugin_key)),
            );
        }
        admission.reserved_completion_count += 1;
        admission.reserved_completion_bytes += reservation_bytes;
        worker
            .metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        worker
            .metrics
            .reserved_completion_bytes
            .fetch_add(reservation_bytes, Ordering::SeqCst);
        self.inner
            .shared
            .metrics
            .reserved_completion_count
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .shared
            .metrics
            .reserved_completion_bytes
            .fetch_add(reservation_bytes, Ordering::SeqCst);
        drop(admission);

        let request_id = request.request_id.clone();
        let failure = handler_failed_result(&request, reason);
        let async_state = Arc::new(AsyncJobState {
            class,
            reservation_bytes,
            terminal: JobTerminal::new(),
            fallbacks,
            worker: worker.clone(),
        });
        seal_and_publish(&async_state, failure, None);
        PluginAdmissionResult::Queued {
            request_id,
            class,
            queue_bytes,
            reservation_bytes,
        }
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
    admission: Arc<Mutex<WorkerAdmission>>,
    work_cvar: Arc<Condvar>,
    executor: Arc<WorkerExecutor>,
    metrics: Arc<WorkerMetrics>,
    shared: Arc<EngineShared>,
}

impl WorkerState {
    fn new(registration: PluginWorkerRegistration, shared: Arc<EngineShared>) -> Self {
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
        let generation = NEXT_WORKER_GENERATION.fetch_add(1, Ordering::SeqCst);
        let executor_concurrency = shared.config.per_plugin_executor_concurrency;
        let mut join_handles = Vec::with_capacity(executor_concurrency);
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
            let join_handle = std::thread::Builder::new()
                .name(format!("botster-plugin-worker-{generation}-{worker_index}"))
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
                                if worker_stopping.load(Ordering::SeqCst) || admission.stopping {
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
            admission,
            work_cvar,
            executor: Arc::new(WorkerExecutor {
                join_handles: Mutex::new(Some(join_handles)),
                stopping,
                cancellations,
            }),
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

        let (queued, open_async) = {
            let mut admission = self
                .admission
                .lock()
                .expect("plugin worker admission mutex poisoned");
            admission.stopping = true;
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
            .take()
            .unwrap_or_default();
        for join_handle in join_handles {
            let _ = join_handle.join();
        }
        let leftovers = self
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .mailbox
            .drain(..)
            .collect::<Vec<_>>();
        if !leftovers.is_empty() {
            self.shared
                .leftover_completions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(leftovers);
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
            completions_saturated: reserved_completion_count >= config.completion_queue_capacity
                || reserved_completion_bytes >= config.completion_queue_byte_capacity,
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
    reserved_completion_count: usize,
    reserved_completion_bytes: usize,
    mailbox: VecDeque<MailboxItem>,
    jobs: HashMap<RequestId, TrackedJob>,
}

impl WorkerAdmission {
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
            return self.bg_queue.remove(index);
        }
        None
    }
}

struct WorkerExecutor {
    join_handles: Mutex<Option<Vec<JoinHandle<()>>>>,
    stopping: Arc<AtomicBool>,
    cancellations: Arc<Mutex<HashMap<RequestId, PluginCancellationToken>>>,
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
    reservation_bytes: usize,
    terminal: JobTerminal,
    fallbacks: CompletionFallbacks,
    worker: WorkerState,
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

struct MailboxItem {
    completion: PluginCompletion,
    encoded_len: usize,
    reservation_bytes: usize,
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

fn is_background(class: PluginInvocationClass) -> bool {
    matches!(class, PluginInvocationClass::Background)
}

fn plugin_invocation_queue_bytes(request: &PluginInvocationRequest) -> Result<usize, ()> {
    serde_json::to_vec(request)
        .map(|bytes| bytes.len())
        .map_err(|_| ())
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
            kind: PluginInvocationFailureKind::HandlerFailed,
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

fn seal_and_publish(
    state: &AsyncJobState,
    result: PluginInvocationResult,
    prepared: Option<PreparedCompletion>,
) {
    if !state.terminal.try_seal() {
        return;
    }
    let (completion, encoded) = match prepared {
        Some(prepared) => (prepared.completion, prepared.encoded),
        None => {
            let completion = PluginCompletion {
                class: state.class,
                result,
            };
            match encode_completion(&completion) {
                Ok(encoded) if encoded.len() <= state.reservation_bytes => (completion, encoded),
                _ => (
                    state.fallbacks.oversize.completion.clone(),
                    state.fallbacks.oversize.encoded.clone(),
                ),
            }
        }
    };
    if let Ok(mut admission) = state.worker.admission.lock() {
        admission.mailbox.push_back(MailboxItem {
            completion,
            encoded_len: encoded.len(),
            reservation_bytes: state.reservation_bytes,
        });
    }
    state
        .worker
        .metrics
        .undrained_completions
        .fetch_add(1, Ordering::SeqCst);
    state
        .worker
        .shared
        .metrics
        .undrained_completions
        .fetch_add(1, Ordering::SeqCst);
}

fn remove_tracked_job(worker: &WorkerState, request_id: &RequestId) {
    if let Ok(mut admission) = worker.admission.lock() {
        admission.jobs.remove(request_id);
    }
    worker.finish_invocation(request_id);
}

fn complete_job(completion: JobCompletion, result: PluginInvocationResult) {
    match completion {
        JobCompletion::Blocking { result_sender } => {
            let _ = result_sender.send(result);
        }
        JobCompletion::Async(state) => {
            seal_and_publish(&state, result, None);
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
    let mut admission = match worker.admission.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(tracked) = admission.jobs.get(&entry.request_id).cloned() else {
        return;
    };
    let JobCompletion::Async(state) = tracked.completion else {
        tracked.cancellation.cancel();
        return;
    };
    if matches!(tracked.phase, JobPhase::Queued) {
        if let Some(job) = admission.remove_queued(&entry.request_id) {
            cancel_queue_metrics_only(&job, &worker.metrics, &shared.metrics);
        }
    }
    tracked.cancellation.cancel();
    drop(admission);
    seal_and_publish(
        &state,
        state.fallbacks.timed_out.result.clone(),
        Some(state.fallbacks.timed_out.clone()),
    );
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
        self.try_admit(class, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::{PluginHandlerKind, PluginInvocationContext};
    use crate::manifest::PackageManifest;
    use crate::package::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};

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

    fn load(engine: &PluginWorkerEngine, plugin: &PluginKey, delay: Duration) {
        engine.load_plugin(PluginWorkerRegistration {
            load: PluginLoadSpec {
                plugin_key: plugin.clone(),
                package: "test".into(),
                entrypoint: "plugin.lua".into(),
                descriptors: Vec::new(),
                metadata: None,
            },
            manifest: manifest(),
            runtime: Arc::new(DelayRuntime::new(delay)),
            handlers: vec![PluginHandlerRegistration {
                handler: handler(plugin),
                required_capability: None,
            }],
            resources: Vec::new(),
        });
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
    fn deadline_first_then_unload_keeps_only_timed_out() {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("deadline-first".into());
        load(&engine, &plugin, Duration::from_millis(200));
        assert!(matches!(
            engine.try_admit(
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
                    .into_iter(),
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
            engine.try_admit(
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
            [PluginCompletion {
                result: PluginInvocationResult::Failed(failure),
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
            engine.try_admit(
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
            engine.try_admit(
                PluginInvocationClass::Background,
                request("job", handler(&plugin), 30_000),
            ),
            PluginAdmissionResult::Queued { .. }
        ));
        let started = Instant::now();
        drop(engine);
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}

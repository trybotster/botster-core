use super::*;
use std::cell::RefCell;
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn resource(drops: &Arc<AtomicUsize>) -> PluginWorkerResource {
    PluginWorkerResource::new(DropProbe(drops.clone()))
}

fn resources(drops: &Arc<AtomicUsize>, count: usize) -> PluginWorkerResources {
    let mut batch = PluginWorkerResources::with_capacity(count, PluginWorkerResource::new(()));
    for _ in 0..count {
        batch
            .try_push(resource(drops))
            .unwrap_or_else(|_| panic!("resource fixture capacity"));
    }
    batch
}

#[test]
fn resource_drops_after_thread_local_destructors() {
    thread_local! {
        static EXIT_PROBE: RefCell<Option<DropProbe>> = const { RefCell::new(None) };
    }
    struct CheckThreadExit(Arc<AtomicUsize>, Arc<AtomicUsize>);
    impl Drop for CheckThreadExit {
        fn drop(&mut self) {
            assert_eq!(self.0.load(Ordering::SeqCst), 1);
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    let exited = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let reservation = PluginWorkerResource::new(CheckThreadExit(exited.clone(), dropped.clone()));
    let record = WorkerJoinRecord::spawn(Some(reservation), || {
        std::thread::Builder::new().spawn(move || {
            EXIT_PROBE.with(|probe| *probe.borrow_mut() = Some(DropProbe(exited)));
        })
    })
    .expect("spawn the thread-local destructor worker");
    record.join().expect("join after thread-local destructors");
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[test]
fn returned_worker_panic_releases_resource() {
    let drops = Arc::new(AtomicUsize::new(0));
    let record = WorkerJoinRecord::spawn(Some(resource(&drops)), || {
        std::thread::Builder::new().spawn(|| panic!("worker panic"))
    })
    .expect("spawn the worker that panics");
    assert!(record.join().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn returned_spawn_failure_releases_only_unstarted_resources() {
    let started = Arc::new(AtomicUsize::new(0));
    let unstarted = Arc::new(AtomicUsize::new(0));
    let (finished, completion) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        finished
            .send(())
            .expect("signal completion of the started worker")
    });
    let record = WorkerJoinRecord::spawn(Some(resource(&started)), || Ok(handle))
        .expect("retain the started worker record");
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _previous = [record];
        let _unused = resources(&unstarted, 2);
        WorkerJoinRecord::spawn(Some(resource(&unstarted)), || {
            Err(io::Error::other("injected spawn failure"))
        })
        .unwrap_or_else(|_| panic!("spawn plugin worker thread"));
    }))
    .is_err());
    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("receive completion after spawn failure");
    assert_eq!(started.load(Ordering::SeqCst), 0);
    assert_eq!(unstarted.load(Ordering::SeqCst), 3);
}

#[test]
fn spawn_unwind_retains_resource_without_a_returned_failure() {
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = WorkerJoinRecord::spawn(Some(resource(&drops)), || panic!("spawn did not return"));
    }))
    .is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn join_unwind_retains_resource_after_handle_is_taken() {
    let drops = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel::<WorkerJoinRecord>();
    let (finished, completion) = mpsc::channel();
    let record = WorkerJoinRecord::spawn(Some(resource(&drops)), || {
        std::thread::Builder::new().spawn(move || {
            let record = receiver
                .recv()
                .expect("receive the worker record for the self-join check");
            let result = catch_unwind(AssertUnwindSafe(|| record.join()));
            finished
                .send(result.is_err())
                .expect("report whether self-join unwound");
        })
    })
    .expect("spawn the self-join worker");
    sender
        .send(record)
        .unwrap_or_else(|_| panic!("worker receiver closed"));
    assert!(completion
        .recv_timeout(Duration::from_secs(5))
        .expect("receive the self-join outcome"));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn count_mismatch_preserves_existing_registration() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("resource-count".into());
    let old_drops = Arc::new(AtomicUsize::new(0));
    let rejected_drops = Arc::new(AtomicUsize::new(0));
    let width = engine.inner.shared.config.per_plugin_executor_concurrency;
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            resources(&old_drops, width),
        )
        .expect("register the original worker resources");
    let generation = engine
        .worker_for(&plugin)
        .expect("find the original worker generation")
        .generation;
    for actual in [0, width - 1, width + 1] {
        let error = engine
            .load_plugin_with_worker_resources(
                registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
                resources(&rejected_drops, actual),
            )
            .expect_err("reject a mismatched resource count");
        assert_eq!(
            error,
            PluginWorkerResourceCountMismatch {
                expected: width,
                actual
            }
        );
        assert_eq!(
            engine
                .worker_for(&plugin)
                .expect("retain the original worker after count refusal")
                .generation,
            generation
        );
        assert_eq!(old_drops.load(Ordering::SeqCst), 0);
    }
    assert_eq!(rejected_drops.load(Ordering::SeqCst), width * 2);
    drop(engine);
    assert_eq!(old_drops.load(Ordering::SeqCst), width);
}

#[test]
fn replacement_unload_and_engine_drop_release_resources_once() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("resource-release".into());
    let drops = Arc::new(AtomicUsize::new(0));
    let width = engine.inner.shared.config.per_plugin_executor_concurrency;
    for replacement in 0..2 {
        engine
            .load_plugin_with_worker_resources(
                registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
                resources(&drops, width),
            )
            .expect("register replacement worker resources");
        assert_eq!(drops.load(Ordering::SeqCst), replacement * width);
    }
    engine.unload_plugin(PluginUnloadSpec {
        request_id: RequestId("unload".into()),
        plugin_key: plugin.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    assert_eq!(drops.load(Ordering::SeqCst), width * 2);
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            resources(&drops, width),
        )
        .expect("register resources before engine drop");
    drop(engine);
    assert_eq!(drops.load(Ordering::SeqCst), width * 3);
}

struct ExitRuntime {
    exited: mpsc::Sender<()>,
    panic_on_stop: bool,
}

impl Drop for ExitRuntime {
    fn drop(&mut self) {
        let _ = self.exited.send(());
    }
}

impl PluginRuntime for ExitRuntime {
    fn invoke(
        &self,
        _request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        unreachable!("idle worker")
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        assert!(!self.panic_on_stop, "injected stop failure");
    }
}

#[test]
fn registration_failure_retains_unjoined_resources() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("registration-failure".into());
    let drops = Arc::new(AtomicUsize::new(0));
    let (exited, completion) = mpsc::channel();
    let width = engine.inner.shared.config.per_plugin_executor_concurrency;
    let worker = WorkerState::new(
        registration(
            &plugin,
            Arc::new(ExitRuntime {
                exited,
                panic_on_stop: false,
            }),
        ),
        engine.inner.shared.clone(),
        Some(resources(&drops, width)),
    );
    let admission = worker.admission.clone();
    let wake = worker.work_signal.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _guard = engine
            .inner
            .shared
            .workers
            .lock()
            .expect("lock the worker registry before poisoning it");
        panic!("poison registration mutex");
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| engine.install_worker(plugin, worker))).is_err());
    engine.inner.shared.workers.clear_poison();
    // Stop detached test workers without recovering their join records.
    admission
        .lock()
        .expect("lock admission to stop the detached worker")
        .stopping = true;
    wake.notify_all();
    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("receive the detached worker completion");
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn cleanup_without_join_and_stop_unwind_retain_resources() {
    for panic_on_stop in [false, true] {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("cleanup-retention".into());
        let drops = Arc::new(AtomicUsize::new(0));
        let (exited, completion) = mpsc::channel();
        let width = engine.inner.shared.config.per_plugin_executor_concurrency;
        engine
            .load_plugin_with_worker_resources(
                registration(
                    &plugin,
                    Arc::new(ExitRuntime {
                        exited,
                        panic_on_stop,
                    }),
                ),
                resources(&drops, width),
            )
            .expect("register resources for cleanup retention");
        let worker = engine
            .worker_for(&plugin)
            .expect("find the worker before cleanup");
        let admission = worker.admission.clone();
        let wake = worker.work_signal.clone();
        drop(worker);
        let result = catch_unwind(AssertUnwindSafe(|| {
            engine.cleanup_plugin(
                RequestId("cleanup".into()),
                &plugin,
                PluginCleanupScope::DescriptorsAndResources,
                panic_on_stop,
            )
        }));
        assert_eq!(result.is_err(), panic_on_stop);
        admission
            .lock()
            .expect("lock admission after cleanup")
            .stopping = true;
        wake.notify_all();
        completion
            .recv_timeout(Duration::from_secs(5))
            .expect("receive completion after cleanup");
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
}

fn metadata_resources(
    workers: &Arc<AtomicUsize>,
    metadata: PluginWorkerResource,
    count: usize,
) -> PluginWorkerResources {
    let mut batch = PluginWorkerResources::with_capacity(count, metadata);
    for _ in 0..count {
        batch
            .try_push(resource(workers))
            .unwrap_or_else(|_| panic!("resource fixture capacity"));
    }
    batch
}

struct DropAfterResources {
    resources: Arc<AtomicUsize>,
    expected: usize,
    metadata: Arc<AtomicUsize>,
}

impl Drop for DropAfterResources {
    fn drop(&mut self) {
        assert_eq!(self.resources.load(Ordering::SeqCst), self.expected);
        self.metadata.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn typed_resource_and_batch_release_payloads_in_order() {
    let direct = Arc::new(AtomicUsize::new(0));
    drop(resource(&direct));
    assert_eq!(direct.load(Ordering::SeqCst), 1);

    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let batch = metadata_resources(
        &workers,
        PluginWorkerResource::new(DropAfterResources {
            resources: workers.clone(),
            expected: 2,
            metadata: metadata.clone(),
        }),
        2,
    );
    drop(batch);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn capacity_overflow_releases_metadata_after_valid_buffer_cleanup() {
    let metadata = Arc::new(AtomicUsize::new(0));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = PluginWorkerResources::with_capacity(usize::MAX, resource(&metadata));
    }))
    .is_err());
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn full_batch_returns_the_original_resource() {
    let accepted = Arc::new(AtomicUsize::new(0));
    let refused = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let mut batch = metadata_resources(&accepted, resource(&metadata), 1);
    let returned = batch
        .try_push(resource(&refused))
        .expect_err("full batch returns the original resource");
    assert_eq!(batch.len(), 1);
    assert_eq!(refused.load(Ordering::SeqCst), 0);
    assert_eq!(metadata.load(Ordering::SeqCst), 0);
    drop(returned);
    assert_eq!(refused.load(Ordering::SeqCst), 1);
    drop(batch);
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn metadata_waits_for_worker_clone_after_unload() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("metadata-surviving-clone".into());
    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let width = engine.inner.shared.config.per_plugin_executor_concurrency;
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            metadata_resources(&workers, resource(&metadata), width),
        )
        .expect("register worker and metadata resources");
    let survivor = engine
        .worker_for(&plugin)
        .expect("retain a worker clone across unload");
    engine.unload_plugin(PluginUnloadSpec {
        request_id: RequestId("unload".into()),
        plugin_key: plugin,
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    assert_eq!(workers.load(Ordering::SeqCst), width);
    assert!(survivor
        .executor
        .join_handles
        .lock()
        .expect("inspect the surviving executor join records")
        .join_handles
        .is_none());
    assert_eq!(metadata.load(Ordering::SeqCst), 0);
    drop(survivor);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_final_handles_release_metadata_after_owned_fields() {
    struct CheckOwnedFields {
        stopping: std::sync::Weak<AtomicBool>,
        cancellations: std::sync::Weak<Mutex<HashMap<RequestId, PluginCancellationToken>>>,
        metadata: Arc<AtomicUsize>,
        // The public resource contract must still accept a Send, non-Sync value.
        _send_only: std::cell::Cell<usize>,
    }
    impl Drop for CheckOwnedFields {
        fn drop(&mut self) {
            assert!(self.stopping.upgrade().is_none());
            assert!(self.cancellations.upgrade().is_none());
            self.metadata.fetch_add(1, Ordering::SeqCst);
        }
    }
    let metadata = Arc::new(AtomicUsize::new(0));
    let stopping = Arc::new(AtomicBool::new(false));
    let cancellations = Arc::new(Mutex::new(HashMap::new()));
    let batch = PluginWorkerResources::with_capacity(
        0,
        PluginWorkerResource::new(CheckOwnedFields {
            stopping: Arc::downgrade(&stopping),
            cancellations: Arc::downgrade(&cancellations),
            metadata: metadata.clone(),
            _send_only: std::cell::Cell::new(0),
        }),
    );
    let first = WorkerExecutorHandle::new(
        WorkerResourceConstruction::new(Some(batch), 0),
        stopping,
        cancellations,
    );
    let second = first.clone();
    let start = Arc::new(std::sync::Barrier::new(2));
    let other_start = start.clone();
    let first_thread = std::thread::spawn(move || {
        start.wait();
        drop(first);
    });
    let second_thread = std::thread::spawn(move || {
        other_start.wait();
        drop(second);
    });
    first_thread
        .join()
        .expect("join the first final-handle drop thread");
    second_thread
        .join()
        .expect("join the second final-handle drop thread");
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn replacement_keeps_old_metadata_with_its_surviving_clone() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("metadata-replacement".into());
    let workers = Arc::new(AtomicUsize::new(0));
    let old_metadata = Arc::new(AtomicUsize::new(0));
    let new_metadata = Arc::new(AtomicUsize::new(0));
    let width = engine.inner.shared.config.per_plugin_executor_concurrency;
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            metadata_resources(&workers, resource(&old_metadata), width),
        )
        .expect("register the old metadata generation");
    let old_worker = engine
        .worker_for(&plugin)
        .expect("retain the old worker across replacement");
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            metadata_resources(&workers, resource(&new_metadata), width),
        )
        .expect("register the new metadata generation");
    assert_eq!(workers.load(Ordering::SeqCst), width);
    assert_eq!(old_metadata.load(Ordering::SeqCst), 0);
    assert_eq!(new_metadata.load(Ordering::SeqCst), 0);
    drop(old_worker);
    assert_eq!(old_metadata.load(Ordering::SeqCst), 1);
    assert_eq!(new_metadata.load(Ordering::SeqCst), 0);
    drop(engine);
    assert_eq!(new_metadata.load(Ordering::SeqCst), 1);
    assert_eq!(workers.load(Ordering::SeqCst), width * 2);
}

#[test]
fn partial_spawn_failure_releases_metadata_after_unused_resources() {
    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let batch = metadata_resources(
        &workers,
        PluginWorkerResource::new(DropAfterResources {
            resources: workers.clone(),
            expected: 2,
            metadata: metadata.clone(),
        }),
        3,
    );
    let (finished, completion) = mpsc::channel();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut construction = WorkerResourceConstruction::new(Some(batch), 3);
        let first = WorkerJoinRecord::spawn(construction.next_resource(), || {
            std::thread::Builder::new().spawn(move || {
                finished
                    .send(())
                    .expect("signal completion of the partially constructed worker")
            })
        })
        .expect("spawn the first worker before injected failure");
        construction.push(first);
        WorkerJoinRecord::spawn(construction.next_resource(), || {
            Err(io::Error::other("injected spawn failure"))
        })
        .unwrap_or_else(|_| panic!("spawn plugin worker thread"));
    }))
    .is_err());
    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("receive completion after partial spawn failure");
    assert_eq!(workers.load(Ordering::SeqCst), 2);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn unused_resource_panic_retains_metadata_during_cleanup() {
    struct PanicOnDrop(Arc<AtomicUsize>);
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("unused resource destructor");
        }
    }
    for path in 0..3 {
        let unused = Arc::new(AtomicUsize::new(0));
        let metadata = Arc::new(AtomicUsize::new(0));
        let mut batch = PluginWorkerResources::with_capacity(1, resource(&metadata));
        batch
            .try_push(PluginWorkerResource::new(PanicOnDrop(unused.clone())))
            .unwrap_or_else(|_| panic!("resource fixture capacity"));
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if path == 0 {
                drop(batch);
            } else {
                let mut construction = WorkerResourceConstruction::new(Some(batch), 1);
                if path == 1 {
                    drop(construction);
                } else {
                    construction.destroy_input();
                }
            }
        }))
        .is_err());
        assert_eq!(unused.load(Ordering::SeqCst), 1);
        assert_eq!(metadata.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn failed_registration_and_cleanup_keep_metadata_until_final_handle() {
    for (registration_fails, panic_on_stop) in [(true, false), (false, false), (false, true)] {
        let engine = PluginWorkerEngine::new();
        let plugin = PluginKey("metadata-failure".into());
        let workers = Arc::new(AtomicUsize::new(0));
        let metadata = Arc::new(AtomicUsize::new(0));
        let width = engine.inner.shared.config.per_plugin_executor_concurrency;
        let (exited, completion) = mpsc::channel();
        let worker = WorkerState::new(
            registration(
                &plugin,
                Arc::new(ExitRuntime {
                    exited,
                    panic_on_stop,
                }),
            ),
            engine.inner.shared.clone(),
            Some(metadata_resources(&workers, resource(&metadata), width)),
        );
        let survivor = worker.clone();
        let admission = worker.admission.clone();
        let wake = worker.work_signal.clone();
        if registration_fails {
            assert!(catch_unwind(AssertUnwindSafe(|| {
                let _guard = engine
                    .inner
                    .shared
                    .workers
                    .lock()
                    .expect("lock the registry before injected registration failure");
                panic!("poison registration mutex");
            }))
            .is_err());
            assert!(
                catch_unwind(AssertUnwindSafe(|| engine.install_worker(plugin, worker))).is_err()
            );
            engine.inner.shared.workers.clear_poison();
        } else {
            engine.install_worker(plugin.clone(), worker);
            let result = catch_unwind(AssertUnwindSafe(|| {
                engine.cleanup_plugin(
                    RequestId("cleanup".into()),
                    &plugin,
                    PluginCleanupScope::DescriptorsAndResources,
                    panic_on_stop,
                )
            }));
            assert_eq!(result.is_err(), panic_on_stop);
        }
        assert_eq!(metadata.load(Ordering::SeqCst), 0);
        admission
            .lock()
            .expect("lock admission before final survivor drop")
            .stopping = true;
        wake.notify_all();
        drop(survivor);
        assert_eq!(metadata.load(Ordering::SeqCst), 1);
        completion
            .recv_timeout(Duration::from_secs(5))
            .expect("receive completion after final survivor drop");
        assert_eq!(workers.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn count_mismatch_drops_input_resources_before_metadata() {
    let engine = PluginWorkerEngine::new();
    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let error = engine
        .load_plugin_with_worker_resources(
            registration(
                &PluginKey("metadata-count".into()),
                Arc::new(DelayRuntime::new(Duration::ZERO)),
            ),
            metadata_resources(
                &workers,
                PluginWorkerResource::new(DropAfterResources {
                    resources: workers.clone(),
                    expected: 1,
                    metadata: metadata.clone(),
                }),
                1,
            ),
        )
        .expect_err("reject the resource count before metadata release");
    assert_eq!(error.actual, 1);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn executor_transfer_destroys_input_before_metadata_moves() {
    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let batch = metadata_resources(&workers, resource(&metadata), 1);
    // A leftover input exercises cleanup. Production consumes the validated count.
    let handle = WorkerExecutorHandle::new(
        WorkerResourceConstruction::new(Some(batch), 1),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(HashMap::new())),
    );
    assert_eq!(workers.load(Ordering::SeqCst), 1);
    assert_eq!(metadata.load(Ordering::SeqCst), 0);
    drop(handle);
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
}

#[test]
fn poisoned_executor_mutex_still_releases_metadata_on_final_drop() {
    let workers = Arc::new(AtomicUsize::new(0));
    let metadata = Arc::new(AtomicUsize::new(0));
    let batch = metadata_resources(&workers, resource(&metadata), 1);
    let mut construction = WorkerResourceConstruction::new(Some(batch), 1);
    let (finished, completion) = mpsc::channel();
    let record = WorkerJoinRecord::spawn(construction.next_resource(), || {
        std::thread::Builder::new().spawn(move || {
            finished
                .send(())
                .expect("signal completion of the executor worker")
        })
    })
    .expect("spawn the worker for executor poison cleanup");
    construction.push(record);
    let handle = WorkerExecutorHandle::new(
        construction,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(HashMap::new())),
    );
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _guard = handle
            .join_handles
            .lock()
            .expect("lock executor join records before poisoning");
        panic!("poison executor resource mutex");
    }))
    .is_err());
    drop(handle);
    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("receive completion after executor poison cleanup");
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
    assert_eq!(workers.load(Ordering::SeqCst), 0);
}

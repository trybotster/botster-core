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
    .unwrap();
    record.join().unwrap();
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[test]
fn returned_worker_panic_releases_resource() {
    let drops = Arc::new(AtomicUsize::new(0));
    let record = WorkerJoinRecord::spawn(Some(resource(&drops)), || {
        std::thread::Builder::new().spawn(|| panic!("worker panic"))
    })
    .unwrap();
    assert!(record.join().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn returned_spawn_failure_releases_only_unstarted_resources() {
    let started = Arc::new(AtomicUsize::new(0));
    let unstarted = Arc::new(AtomicUsize::new(0));
    let (finished, completion) = mpsc::channel();
    let handle = std::thread::spawn(move || finished.send(()).unwrap());
    let record = WorkerJoinRecord::spawn(Some(resource(&started)), || Ok(handle)).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _previous = vec![record];
        let _unused = resources(&unstarted, 2);
        WorkerJoinRecord::spawn(Some(resource(&unstarted)), || {
            Err(io::Error::other("injected spawn failure"))
        })
        .unwrap_or_else(|_| panic!("spawn plugin worker thread"));
    }))
    .is_err());
    completion.recv_timeout(Duration::from_secs(5)).unwrap();
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
            let record = receiver.recv().unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| record.join()));
            finished.send(result.is_err()).unwrap();
        })
    })
    .unwrap();
    sender
        .send(record)
        .unwrap_or_else(|_| panic!("worker receiver closed"));
    assert!(completion.recv_timeout(Duration::from_secs(5)).unwrap());
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
        .unwrap();
    let generation = engine.worker_for(&plugin).unwrap().generation;
    for actual in [0, width - 1, width + 1] {
        let error = engine
            .load_plugin_with_worker_resources(
                registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
                resources(&rejected_drops, actual),
            )
            .unwrap_err();
        assert_eq!(
            error,
            PluginWorkerResourceCountMismatch {
                expected: width,
                actual
            }
        );
        assert_eq!(engine.worker_for(&plugin).unwrap().generation, generation);
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
            .unwrap();
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
        .unwrap();
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
    let wake = worker.work_cvar.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _guard = engine.inner.shared.workers.lock().unwrap();
        panic!("poison registration mutex");
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| engine.install_worker(plugin, worker))).is_err());
    engine.inner.shared.workers.clear_poison();
    // Stop detached test workers without recovering their join records.
    admission.lock().unwrap().stopping = true;
    wake.notify_all();
    completion.recv_timeout(Duration::from_secs(5)).unwrap();
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
            .unwrap();
        let worker = engine.worker_for(&plugin).unwrap();
        let admission = worker.admission.clone();
        let wake = worker.work_cvar.clone();
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
        admission.lock().unwrap().stopping = true;
        wake.notify_all();
        completion.recv_timeout(Duration::from_secs(5)).unwrap();
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
        .err()
        .expect("batch is full");
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
        .unwrap();
    let survivor = engine.worker_for(&plugin).unwrap();
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
        .unwrap()
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
    first_thread.join().unwrap();
    second_thread.join().unwrap();
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
        .unwrap();
    let old_worker = engine.worker_for(&plugin).unwrap();
    engine
        .load_plugin_with_worker_resources(
            registration(&plugin, Arc::new(DelayRuntime::new(Duration::ZERO))),
            metadata_resources(&workers, resource(&new_metadata), width),
        )
        .unwrap();
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
            std::thread::Builder::new().spawn(move || finished.send(()).unwrap())
        })
        .unwrap();
        construction.push(first);
        WorkerJoinRecord::spawn(construction.next_resource(), || {
            Err(io::Error::other("injected spawn failure"))
        })
        .unwrap_or_else(|_| panic!("spawn plugin worker thread"));
    }))
    .is_err());
    completion.recv_timeout(Duration::from_secs(5)).unwrap();
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
        let wake = worker.work_cvar.clone();
        if registration_fails {
            assert!(catch_unwind(AssertUnwindSafe(|| {
                let _guard = engine.inner.shared.workers.lock().unwrap();
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
        admission.lock().unwrap().stopping = true;
        wake.notify_all();
        drop(survivor);
        assert_eq!(metadata.load(Ordering::SeqCst), 1);
        completion.recv_timeout(Duration::from_secs(5)).unwrap();
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
        .unwrap_err();
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
        std::thread::Builder::new().spawn(move || finished.send(()).unwrap())
    })
    .unwrap();
    construction.push(record);
    let handle = WorkerExecutorHandle::new(
        construction,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(HashMap::new())),
    );
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _guard = handle.join_handles.lock().unwrap();
        panic!("poison executor resource mutex");
    }))
    .is_err());
    drop(handle);
    completion.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(metadata.load(Ordering::SeqCst), 1);
    assert_eq!(workers.load(Ordering::SeqCst), 0);
}

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
    Box::new(DropProbe(drops.clone()))
}

fn resources(drops: &Arc<AtomicUsize>, count: usize) -> Vec<PluginWorkerResource> {
    (0..count).map(|_| resource(drops)).collect()
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
    let reservation = Box::new(CheckThreadExit(exited.clone(), dropped.clone()));
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

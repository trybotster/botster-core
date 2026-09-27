// Delivery pools (plan section 5.1): pre-funded host-call results that are
// never refused for capacity, units that return exactly once, all-or-nothing
// reservation, and the ordinary completion share.
//
// Admission right after a load can meet a starting executor's admission
// lock, so these tests use the module's existing lock-busy retry helper.

use super::*;
use std::sync::mpsc;

/// Bound for each event these tests wait for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(10);

/// Completes every invocation with its own payload.
struct EchoRuntime;

impl PluginRuntime for EchoRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        PluginInvocationResult::Completed(crate::actor::PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }
}

/// Holds every invocation on its executor until the test opens the gate,
/// and reports each invocation that starts.
#[derive(Default)]
struct GateRuntime {
    open: Mutex<bool>,
    changed: Condvar,
    started: Mutex<Option<mpsc::Sender<()>>>,
}

impl GateRuntime {
    fn open(&self) {
        *self.open.lock().expect("gate") = true;
        self.changed.notify_all();
    }
}

impl PluginRuntime for GateRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        if let Some(started) = &*self.started.lock().expect("started") {
            let _ = started.send(());
        }
        let mut open = self.open.lock().expect("gate");
        while !*open {
            open = self.changed.wait(open).expect("gate");
        }
        PluginInvocationResult::Completed(crate::actor::PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        self.open();
    }
}

fn quota(slots: usize, share_entries: usize) -> PluginDeliveryQuota {
    PluginDeliveryQuota {
        call_result_slots: slots,
        call_result_request_bytes: slots * 1024,
        call_result_completion_bytes: 4096,
        ordinary_completion_entries: share_entries,
        ordinary_completion_bytes: share_entries * 4096,
    }
}

fn notified(engine: &PluginWorkerEngine) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = tx.send(());
    }));
    rx
}

fn await_event(rx: &mpsc::Receiver<()>, what: &str) {
    // timer: deadline — the named event arrives; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .unwrap_or_else(|_| panic!("{what}"));
}

/// Drain until one completion arrives; returns the drained results.
fn drain_one(engine: &PluginWorkerEngine, completions: &mpsc::Receiver<()>) -> PluginCompletionDrain {
    loop {
        let drain = engine.drain_completions(usize::MAX, usize::MAX);
        if drain.item_count > 0 {
            return drain;
        }
        await_event(completions, "a completion is published");
    }
}

fn unit_returns(pool: &DeliveryPool) -> mpsc::Receiver<CallId> {
    let (tx, rx) = mpsc::channel();
    pool.install_unit_returned(Arc::new(move |call| {
        let _ = tx.send(call);
    }));
    rx
}

#[test]
fn a_reservation_is_all_or_nothing() {
    let metadata = completion_store::metadata_bytes();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        completion_queue_capacity: 3,
        completion_queue_byte_capacity: 3 * (4096 + metadata),
        ..PluginWorkerEngineConfig::default()
    });
    let plugin = PluginKey("all-or-nothing".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));

    // Two pool units fit, but the share's two entries do not also fit.
    assert!(matches!(
        engine.try_reserve_delivery(&plugin, quota(2, 2)),
        Err(DeliveryRefusal::Backpressured(_))
    ));
    assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0), "nothing stays reserved");

    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 1))
        .expect("the smaller quota fits");
    assert_eq!(pool.free(), (2, 2 * 1024));
    assert!(matches!(
        engine.try_reserve_delivery(&plugin, quota(1, 0)),
        Err(DeliveryRefusal::RejectedBudget(_))
    ));
}

#[test]
fn pool_results_neither_take_ordinary_room_nor_need_it() {
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_executor_concurrency: 2,
        reserved_request_response_executors: 1,
        background_queue_capacity: 4,
        ..PluginWorkerEngineConfig::default()
    });
    let plugin = PluginKey("ordinary-full".into());
    let gate = Arc::new(GateRuntime::default());
    let (started_tx, started) = mpsc::channel();
    *gate.started.lock().expect("started") = Some(started_tx);
    engine.load_plugin(registration(&plugin, gate.clone()));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 8))
        .expect("reserved");
    let admit = |id: &str| {
        try_admit_retrying_lock_busy(
            &engine,
            PluginInvocationClass::Background,
            request(id, handler(&plugin), 60_000),
        )
    };

    // One ordinary job holds the only Background executor.
    assert!(matches!(admit("busy"), PluginAdmissionResult::Queued { .. }));
    await_event(&started, "the busy job holds the Background executor");

    // A queued pool result does not use the ordinary room (4 - 2 slots).
    pool.accept_call(CallId(1), 512).expect("a unit");
    assert!(matches!(
        pool.admit_result(CallId(1), request("result-1", handler(&plugin), 60_000)),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(matches!(admit("queued-1"), PluginAdmissionResult::Queued { .. }));
    assert!(matches!(admit("queued-2"), PluginAdmissionResult::Queued { .. }));
    assert!(matches!(admit("refused"), PluginAdmissionResult::Backpressured { .. }));

    // And a pool result is admitted with the ordinary room exhausted.
    pool.accept_call(CallId(2), 512).expect("a unit");
    assert!(matches!(
        pool.admit_result(CallId(2), request("result-2", handler(&plugin), 60_000)),
        PluginAdmissionResult::Queued { .. }
    ));
    gate.open();
}

#[test]
fn a_unit_returns_exactly_once_when_its_completion_drains() {
    let engine = PluginWorkerEngine::new();
    let completions = notified(&engine);
    let plugin = PluginKey("unit-return".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);

    pool.accept_call(CallId(7), 512).expect("a unit");
    assert_eq!(pool.free(), (1, 2 * 1024 - 512));
    assert!(matches!(
        pool.admit_result(CallId(7), request("result", handler(&plugin), 60_000)),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(!pool.release_call(CallId(7)), "an admitted result owns the unit");
    assert!(returned.try_recv().is_err(), "the unit is held until the drain");

    let drain = drain_one(&engine, &completions);
    assert!(matches!(
        drain.completions[0].completion.result,
        PluginInvocationResult::Completed(_)
    ));
    // timer: deadline — the drain returns the unit; expiry fails the test
    assert_eq!(returned.recv_timeout(EVENT_DEADLINE).expect("returned"), CallId(7));
    assert_eq!(pool.free(), (2, 2 * 1024));
    assert!(returned.try_recv().is_err(), "the unit returns exactly once");
}

#[test]
fn release_call_returns_an_accepted_unit() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("release".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);

    pool.accept_call(CallId(1), 100).expect("a unit");
    assert!(pool.release_call(CallId(1)));
    assert_eq!(returned.try_recv().expect("notified"), CallId(1));
    assert!(!pool.release_call(CallId(1)), "released once");
    assert_eq!(pool.free(), (1, 1024));
}

#[test]
fn overdraws_are_refused() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("overdraw".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 0))
        .expect("reserved");

    assert_eq!(pool.accept_call(CallId(1), 4096), Err(PoolOverdraw::NoBytes));
    pool.accept_call(CallId(1), 1024).expect("first");
    assert_eq!(pool.accept_call(CallId(1), 1), Err(PoolOverdraw::DuplicateCall));
    pool.accept_call(CallId(2), 1024).expect("second");
    assert_eq!(pool.accept_call(CallId(3), 0), Err(PoolOverdraw::NoSlot));
}

#[test]
fn an_oversize_result_is_rejected_without_spending_its_unit() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("oversize".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");

    pool.accept_call(CallId(1), 1).expect("a tiny declared result");
    assert!(matches!(
        pool.admit_result(CallId(1), request("big", handler(&plugin), 60_000)),
        PluginAdmissionResult::RejectedBudget { .. }
    ));
    assert!(pool.release_call(CallId(1)), "the unit stayed accepted");
}

#[test]
fn unloading_retires_the_pool_and_returns_all_funding() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("retire".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 2))
        .expect("reserved");
    pool.accept_call(CallId(1), 100).expect("a unit");
    assert_ne!(completion_store_counts(&engine), (0, 0, 0, 0));

    engine.unload_plugin(PluginUnloadSpec {
        request_id: RequestId("unload".into()),
        plugin_key: plugin.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });

    assert_eq!(completion_store_counts(&engine), (0, 0, 0, 0));
    assert_eq!(pool.accept_call(CallId(2), 1), Err(PoolOverdraw::Closed));
    assert!(
        matches!(
            pool.admit_result(CallId(1), request("late", handler(&plugin), 60_000)),
            PluginAdmissionResult::WorkerStopped { .. }
        ),
        "a result after retirement is WorkerStopped (review D4)"
    );
    assert_eq!(pool.free(), (0, 0));
}

#[test]
fn the_ordinary_share_bounds_one_plugin_and_leaves_others_the_global_pool() {
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_executor_concurrency: 2,
        reserved_request_response_executors: 1,
        ..PluginWorkerEngineConfig::default()
    });
    let shared_plugin = PluginKey("with-share".into());
    let other = PluginKey("without-share".into());
    let gate = Arc::new(GateRuntime::default());
    engine.load_plugin(registration(&shared_plugin, gate.clone()));
    engine.load_plugin(registration(&other, gate.clone()));
    let _pool = engine
        .try_reserve_delivery(&shared_plugin, quota(1, 1))
        .expect("reserved");

    assert!(matches!(
        try_admit_retrying_lock_busy(
            &engine,
            PluginInvocationClass::Background,
            request("first", handler(&shared_plugin), 60_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    assert!(
        matches!(
            try_admit_retrying_lock_busy(
                &engine,
                PluginInvocationClass::Background,
                request("second", handler(&shared_plugin), 60_000),
            ),
            PluginAdmissionResult::Backpressured { .. }
        ),
        "the plugin's one-entry share is in use"
    );
    assert!(matches!(
        try_admit_retrying_lock_busy(
            &engine,
            PluginInvocationClass::Background,
            request("other", handler(&other), 60_000),
        ),
        PluginAdmissionResult::Queued { .. }
    ));
    gate.open();
}

/// Review D1: a result request larger than the completion allowance is
/// admitted when it fits its declaration; only the fallback markers must fit
/// the completion entry.
#[test]
fn a_result_request_is_bounded_by_its_declaration_not_the_completion_allowance() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("large-result".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(
            &plugin,
            PluginDeliveryQuota {
                call_result_slots: 1,
                call_result_request_bytes: 64 * 1024,
                call_result_completion_bytes: 4096,
                ordinary_completion_entries: 0,
                ordinary_completion_bytes: 0,
            },
        )
        .expect("reserved");
    let mut large = request("large", handler(&plugin), 60_000);
    large.payload =
        serde_json::from_value(serde_json::json!({ "blob": "x".repeat(8 * 1024) })).expect("payload");

    pool.accept_call(CallId(1), 16 * 1024).expect("a unit");
    assert!(matches!(
        pool.admit_result(CallId(1), large),
        PluginAdmissionResult::Queued { .. }
    ));
}

/// Review D2: a huge declaration cannot overflow the byte accounting.
#[test]
fn a_huge_declaration_is_refused_without_overflow() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("overflow".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(2, 0))
        .expect("reserved");

    pool.accept_call(CallId(1), 1).expect("one byte");
    assert_eq!(pool.accept_call(CallId(2), usize::MAX), Err(PoolOverdraw::NoBytes));
    assert_eq!(pool.free(), (1, 2 * 1024 - 1), "the accounting is unchanged");
}

/// Review D3: release_call checks and removes under one lock, so a unit whose
/// result admission began is never released early.
#[test]
fn release_never_takes_a_unit_whose_result_admission_began() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("release-race".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);

    pool.accept_call(CallId(1), 100).expect("a unit");
    pool.inner.begin_admit(CallId(1)).expect("admission begins");
    assert!(!pool.release_call(CallId(1)), "an admitting unit is not released");
    pool.inner.end_admit(CallId(1), true);
    assert!(!pool.release_call(CallId(1)), "an admitted unit is not released");
    assert!(returned.try_recv().is_err());
    assert_eq!(pool.free(), (0, 1024 - 100), "the unit is still held");
}

/// Review D5, immediate completion: a zero-timeout result publishes its
/// timeout inside the admission; a notifier that drains synchronously must
/// already find the unit Admitted, so the unit returns exactly once.
#[test]
fn an_immediately_published_result_returns_its_unit() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("immediate".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);
    let draining = engine.clone();
    let (drained_tx, drained) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = drained_tx.send(draining.drain_completions(usize::MAX, usize::MAX).item_count);
    }));

    pool.accept_call(CallId(1), 512).expect("a unit");
    assert!(matches!(
        pool.admit_result(CallId(1), request("zero", handler(&plugin), 0)),
        PluginAdmissionResult::Queued { .. }
    ));
    // timer: deadline — the synchronous drain reports; expiry fails the test
    assert_eq!(drained.recv_timeout(EVENT_DEADLINE).expect("drained"), 1);
    assert_eq!(returned.try_recv().expect("the unit returned"), CallId(1));
    assert!(returned.try_recv().is_err(), "exactly once");
    assert_eq!(pool.free(), (1, 1024));
    // The notifier holds an engine clone; replace it so the engine can drop.
    engine.install_completion_notifier(Arc::new(|| {}));
}

/// Review D5, fast normal completion: the admission is held after it
/// released every engine lock; meanwhile the executor finishes and the test
/// drains. The unit is already Admitted, so the drain returns it.
#[test]
fn a_result_drained_before_its_admission_returns_still_returns_its_unit() {
    let engine = PluginWorkerEngine::new();
    let completions = notified(&engine);
    let plugin = PluginKey("fast".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);
    let (reached_tx, reached) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *engine
        .inner
        .shared
        .pool_admit_pause
        .lock()
        .expect("pool admit pause") = Some((reached_tx, release_rx));

    pool.accept_call(CallId(1), 512).expect("a unit");
    let admitting = {
        let pool = pool.clone();
        let plugin = plugin.clone();
        std::thread::spawn(move || {
            pool.admit_result(CallId(1), request("fast", handler(&plugin), 60_000))
        })
    };
    await_event(&reached, "the admission is held after it released the engine locks");
    let drain = drain_one(&engine, &completions);
    assert_eq!(drain.item_count, 1);
    // timer: deadline — the drain returns the unit; expiry fails the test
    assert_eq!(returned.recv_timeout(EVENT_DEADLINE).expect("returned"), CallId(1));
    let _ = release_tx.send(());
    assert!(matches!(
        admitting.join().expect("admission"),
        PluginAdmissionResult::Queued { .. }
    ));
    assert_eq!(pool.free(), (1, 1024));
}

/// Review D3, forced: a result admission starts at the end of the release
/// step. The fixed release checked and removed in one step, so the call is
/// already gone; a release split into check-then-remove would let the
/// admission start in the gap and then remove an admitting unit.
#[test]
fn an_admission_racing_a_release_finds_the_call_gone() {
    let engine = PluginWorkerEngine::new();
    let plugin = PluginKey("release-forced".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    let pool = engine
        .try_reserve_delivery(&plugin, quota(1, 0))
        .expect("reserved");
    let returned = unit_returns(&pool);
    pool.accept_call(CallId(1), 100).expect("a unit");
    let racing: Arc<Mutex<Option<Result<usize, delivery_pool::AdmitRefusal>>>> = Arc::default();
    let hook_racing = racing.clone();
    let hook_pool = pool.clone();
    delivery_pool::RELEASE_STEP_END.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            *hook_racing.lock().expect("racing") = Some(hook_pool.inner.begin_admit(CallId(1)));
        }));
    });

    assert!(pool.release_call(CallId(1)));
    assert_eq!(
        racing.lock().expect("racing").take(),
        Some(Err(delivery_pool::AdmitRefusal::UnknownCall)),
        "the racing admission must find the call released"
    );
    assert_eq!(returned.try_recv().expect("returned"), CallId(1));
    assert!(returned.try_recv().is_err());
    assert_eq!(pool.free(), (1, 1024));
}

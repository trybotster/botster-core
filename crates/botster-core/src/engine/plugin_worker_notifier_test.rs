// Notifier conformance (readiness plan C2, section 2.4.1): a refused
// admission arms the engine's retry wake for a full class queue or
// completion store, as for a busy lock, and every release that can end
// that refusal fires the completion notifier while armed. A refused
// admission fires nothing of its own.
//
// The completion tests start from a quiet engine: every executor and the
// deadline waiter reported idle, and the setup admits only expired
// requests, which publish at admission and signal no thread. So the edge
// under test is the only release, and the armed flag shows it.

use super::*;
use std::collections::HashSet;
use std::sync::mpsc;

/// Bound for each event these tests wait for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(10);

/// Holds each invocation until the test opens its request id, and reports
/// each invocation that starts.
struct GatedRuntime {
    entered: Mutex<mpsc::Sender<String>>,
    open: Mutex<HashSet<String>>,
    changed: Condvar,
}

impl GatedRuntime {
    fn new() -> (Arc<Self>, mpsc::Receiver<String>) {
        let (entered, entered_rx) = mpsc::channel();
        (
            Arc::new(Self {
                entered: Mutex::new(entered),
                open: Mutex::new(HashSet::new()),
                changed: Condvar::new(),
            }),
            entered_rx,
        )
    }

    fn open(&self, request_id: &str) {
        self.open.lock().expect("gate").insert(request_id.to_string());
        self.changed.notify_all();
    }
}

impl PluginRuntime for GatedRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        let id = request.request_id.0.clone();
        let _ = self.entered.lock().expect("entered").send(id.clone());
        let mut open = self.open.lock().expect("gate");
        while !open.contains(&id) && !open.contains("*") {
            open = self.changed.wait(open).expect("gate");
        }
        PluginInvocationResult::Completed(crate::actor::PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        // Unload opens every gate, so a held executor can be joined.
        let mut open = self.open.lock().expect("gate");
        open.insert("*".to_string());
        drop(open);
        self.changed.notify_all();
    }
}

/// Completes every invocation at once.
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

fn notifications(engine: &PluginWorkerEngine) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = tx.send(());
    }));
    rx
}

fn armed(engine: &PluginWorkerEngine) -> bool {
    engine
        .inner
        .shared
        .admission_retry_armed
        .load(Ordering::SeqCst)
}

fn cause(result: &PluginAdmissionResult) -> PluginBackpressureCause {
    match result {
        PluginAdmissionResult::Backpressured { cause, .. } => *cause,
        other => panic!("expected Backpressured, got {other:?}"),
    }
}

/// Wait until `executors` executors and the deadline waiter reported their
/// idle waits. Nothing else signals them in these setups, so the engine is
/// quiet afterwards.
fn wait_quiet(events: &mpsc::Receiver<PluginQueueProbeEvent>, executors: usize) {
    let (mut idle_executors, mut waiter_idle) = (0, false);
    while idle_executors < executors || !waiter_idle {
        // timer: deadline — the engine's threads reach their idle waits; expiry fails the test
        match events
            // timer: deadline — the event must arrive; expiry fails the test
            .recv_timeout(EVENT_DEADLINE)
            .expect("the engine's threads reach their idle waits")
        {
            PluginQueueProbeEvent::ExecutorIdle => idle_executors += 1,
            PluginQueueProbeEvent::DeadlineWaiterIdle => waiter_idle = true,
            _ => {}
        }
    }
}

fn completion_capacity_one() -> PluginWorkerEngineConfig {
    PluginWorkerEngineConfig {
        completion_queue_capacity: 1,
        ..PluginWorkerEngineConfig::default()
    }
}

/// An already-expired request: it publishes its TimedOut completion at
/// admission, which holds one completion reservation until drained.
fn expired(id: &str, plugin: &PluginKey) -> PluginInvocationRequest {
    request(id, handler(plugin), 0)
}

fn try_admit(engine: &PluginWorkerEngine, id: &str, plugin: &PluginKey) -> PluginAdmissionResult {
    engine.try_admit(PluginInvocationClass::Background, expired(id, plugin), 1)
}

#[test]
fn a_refused_admission_arms_the_wake_and_a_drain_fires_it() {
    let (engine, events) = probed_engine(completion_capacity_one());
    let plugin = PluginKey("drain-edge".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    assert!(matches!(
        admit(&engine, PluginInvocationClass::Background, expired("held", &plugin)),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_quiet(&events, 2);
    let wakes = notifications(&engine);
    // The notifier reports the completion that already exists.
    while wakes.try_recv().is_ok() {}
    assert!(!armed(&engine));

    let refused = try_admit(&engine, "parked", &plugin);
    assert_eq!(cause(&refused), PluginBackpressureCause::CompletionReservation);
    assert!(armed(&engine), "the refusal armed the wake");
    assert!(wakes.try_recv().is_err(), "a refused admission fires nothing");

    let drained = engine.drain_completions(usize::MAX, usize::MAX);
    assert_eq!(drained.item_count, 1);
    assert!(
        wakes.try_recv().is_ok(),
        "the drain returned the reservation and fired the armed wake"
    );
    assert!(!armed(&engine));
    assert!(matches!(
        try_admit(&engine, "parked", &plugin),
        PluginAdmissionResult::Queued { .. }
    ));
}

#[test]
fn a_release_between_the_first_refusal_and_the_arm_is_not_lost() {
    let (engine, events) = probed_engine(completion_capacity_one());
    let plugin = PluginKey("refusal-window".into());
    engine.load_plugin(registration(&plugin, Arc::new(EchoRuntime)));
    assert!(matches!(
        admit(&engine, PluginInvocationClass::Background, expired("held", &plugin)),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_quiet(&events, 2);

    // The release lands after the first refusal and before the arm, so no
    // wake can report it: the retry itself must see it.
    let releaser = engine.clone();
    AFTER_FIRST_REFUSAL.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            assert_eq!(
                releaser.drain_completions(usize::MAX, usize::MAX).item_count,
                1
            );
        }));
    });
    assert!(matches!(
        try_admit(&engine, "parked", &plugin),
        PluginAdmissionResult::Queued { .. }
    ));
}

fn class_capacity_one() -> PluginWorkerEngineConfig {
    PluginWorkerEngineConfig {
        per_plugin_executor_concurrency: 2,
        reserved_request_response_executors: 1,
        background_queue_capacity: 1,
        ..PluginWorkerEngineConfig::default()
    }
}

/// Hold both executors with blocking invocations, which publish no
/// completion, and fill the one-slot Background queue with `queued`.
fn fill_class_queue(
    engine: &PluginWorkerEngine,
    plugin: &PluginKey,
    entered: &mpsc::Receiver<String>,
    queued: PluginInvocationRequest,
) -> Vec<std::thread::JoinHandle<PluginInvocationOutcome>> {
    let blockers: Vec<_> = ["hold-a", "hold-b"]
        .into_iter()
        .map(|id| {
            let engine = engine.clone();
            let handler = handler(plugin);
            std::thread::spawn(move || engine.invoke(request(id, handler, 60_000)))
        })
        .collect();
    for _ in 0..2 {
        // timer: deadline — each blocking invocation starts; expiry fails the test
        entered
            // timer: deadline — the event must arrive; expiry fails the test
            .recv_timeout(EVENT_DEADLINE)
            .expect("a blocking invocation holds an executor");
    }
    assert!(matches!(
        admit(engine, PluginInvocationClass::Background, queued),
        PluginAdmissionResult::Queued { .. }
    ));
    blockers
}

fn parked(engine: &PluginWorkerEngine, plugin: &PluginKey) -> PluginAdmissionResult {
    engine.try_admit(
        PluginInvocationClass::Background,
        request("parked", handler(plugin), 60_000),
        1,
    )
}

/// The pop of a queued job is the release that ends a class-cause refusal.
/// Every other release is held or consumed first: the deadline waiter is
/// held at its idle point, and the executor's own release wake is consumed
/// and re-armed while the job is still queued. Only the pop can then fire.
#[test]
fn a_worker_dequeue_wakes_an_admission_refused_for_its_class_queue() {
    let (engine, _events) = probed_engine(class_capacity_one());
    let plugin = PluginKey("class-edge".into());
    let (runtime, entered) = GatedRuntime::new();
    engine.load_plugin(registration(&plugin, runtime.clone()));
    let wakes = notifications(&engine);
    let blockers = fill_class_queue(
        &engine,
        &plugin,
        &entered,
        request("queued", handler(&plugin), 60_000),
    );

    let (waiter_held, waiter_resume) = install_idle_pause(&engine, IdleSite::DeadlineWaiter);
    engine.inner.shared.deadline_signal.notify();
    // timer: deadline — the deadline waiter reaches its hold; expiry fails the test
    waiter_held
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the deadline waiter holds before its wake");
    assert_eq!(cause(&parked(&engine, &plugin)), PluginBackpressureCause::ClassQueue);
    assert!(armed(&engine));

    // One executor frees. Its release wake fires; hold the executor before
    // it pops the queued job.
    let (dispatch_held, dispatch_resume) = install_idle_pause(&engine, IdleSite::WorkerDispatch);
    runtime.open("hold-a");
    // timer: deadline — the executor release fires the armed wake; expiry fails the test
    wakes
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the executor release fires the armed wake");
    // timer: deadline — the executor reaches its dispatch hold; expiry fails the test
    dispatch_held
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the executor holds before its pop");
    // The queue is still full: the retry is refused and re-arms.
    assert_eq!(cause(&parked(&engine, &plugin)), PluginBackpressureCause::ClassQueue);
    assert!(armed(&engine));
    assert!(wakes.try_recv().is_err(), "no release since the re-arm");

    // Only the pop can fire now.
    dispatch_resume.send(()).expect("release the dispatch");
    // timer: deadline — the pop fires the armed wake; expiry fails the test
    wakes
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the dequeue fires the armed wake");
    // timer: deadline — the queued job runs on the freed executor; expiry fails the test
    assert_eq!(
        // timer: deadline — the event must arrive; expiry fails the test
        entered.recv_timeout(EVENT_DEADLINE).expect("dequeued"),
        "queued"
    );
    waiter_resume.send(()).expect("resume the deadline waiter");
    assert!(matches!(
        engine.admit(
            PluginInvocationClass::Background,
            request("parked", handler(&plugin), 60_000),
            1,
        ),
        PluginAdmissionResult::Queued { .. }
    ));

    runtime.open("*");
    for blocker in blockers {
        blocker.join().expect("blocking caller");
    }
}

/// A queued job's deadline removes it from the class queue. The timed-out
/// completion's own notification does not consume the armed wake, so the
/// consumed flag proves that the deadline waiter fired it after the removal.
#[test]
fn a_deadline_unqueue_fires_the_armed_class_wake() {
    let (engine, events) = probed_engine(class_capacity_one());
    let plugin = PluginKey("deadline-edge".into());
    let (runtime, entered) = GatedRuntime::new();
    engine.load_plugin(registration(&plugin, runtime.clone()));
    let wakes = notifications(&engine);
    // The deadline delivery waits for its permit, so the queued job leaves
    // the queue only when the test grants it.
    let permits = engine.gate_deadlines();
    let blockers = fill_class_queue(
        &engine,
        &plugin,
        &entered,
        request("expiring", handler(&plugin), 1),
    );
    // The waiter is now inside the delivery of `expiring`, waiting for its
    // permit, so no earlier pass of it can fire the wake armed below.
    permits.wait_until_waiting(&RequestId("expiring".into()));
    assert_eq!(cause(&parked(&engine, &plugin)), PluginBackpressureCause::ClassQueue);
    assert!(armed(&engine));
    while events.try_recv().is_ok() {}
    while wakes.try_recv().is_ok() {}

    permits.grant(RequestId("expiring".into()));
    // The waiter is inside that delivery until it ends, so its next idle
    // report follows the removal and its armed fire.
    loop {
        // timer: deadline — the deadline waiter finishes the delivery; expiry fails the test
        if events
            // timer: deadline — the event must arrive; expiry fails the test
            .recv_timeout(EVENT_DEADLINE)
            .expect("the deadline waiter goes idle")
            == PluginQueueProbeEvent::DeadlineWaiterIdle
        {
            break;
        }
    }
    assert!(
        !armed(&engine),
        "the deadline waiter fired the armed wake after the removal"
    );
    assert!(wakes.try_recv().is_ok(), "the host was notified");
    assert!(matches!(
        engine.admit(
            PluginInvocationClass::Background,
            request("parked", handler(&plugin), 60_000),
            1,
        ),
        PluginAdmissionResult::Queued { .. }
    ));

    runtime.open("*");
    for blocker in blockers {
        blocker.join().expect("blocking caller");
    }
}

/// Unload `plugin` on a helper thread and hold its shutdown after the
/// generation retired and its funds closed, before its executors are told
/// to stop. Returns the release sender and the unload handle.
fn unload_held(
    engine: &PluginWorkerEngine,
    plugin: &PluginKey,
) -> (mpsc::Sender<()>, std::thread::JoinHandle<()>) {
    let (reached_tx, reached) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    *engine.inner.shared.shutdown_pause.lock().expect("pause") = Some((reached_tx, release_rx));
    let unloader = engine.clone();
    let plugin = plugin.clone();
    let handle = std::thread::spawn(move || {
        unloader.unload_plugin(PluginUnloadSpec {
            request_id: RequestId("unload".into()),
            plugin_key: plugin,
            cleanup: PluginCleanupScope::DescriptorsAndResources,
        });
    });
    // timer: deadline — the unload reaches its hold; expiry fails the test
    reached
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the unload holds before its executors stop");
    (release, handle)
}

#[test]
fn closing_an_unloaded_plugins_unused_funds_wakes_another_plugins_admission() {
    let (engine, events) = probed_engine(completion_capacity_one());
    let (holder, parker) = (PluginKey("funds".into()), PluginKey("parker".into()));
    engine.load_plugin(registration(&holder, Arc::new(EchoRuntime)));
    engine.load_plugin(registration(&parker, Arc::new(EchoRuntime)));
    // The holder's delivery pool funds the only completion entry, unused.
    let _pool = engine
        .try_reserve_delivery(
            &holder,
            PluginDeliveryQuota {
                call_result_slots: 1,
                call_result_request_bytes: 1024,
                call_result_completion_bytes: 4096,
                ordinary_completion_entries: 0,
                ordinary_completion_bytes: 0,
            },
        )
        .expect("the holder reserves the only entry");
    wait_quiet(&events, 4);
    let wakes = notifications(&engine);

    let refused = try_admit(&engine, "parked", &parker);
    assert_eq!(cause(&refused), PluginBackpressureCause::CompletionReservation);
    assert!(armed(&engine));
    assert!(wakes.try_recv().is_err(), "a refused admission fires nothing");

    // The holder unloads with zero completions ever drained. The unload
    // signals the deadline waiter, whose pass would fire any armed wake, so
    // it is held at its idle point, before that fire. The holder's executors
    // have not been told to stop, so no executor exit fires the wake either.
    let (waiter_held, waiter_resume) = install_idle_pause(&engine, IdleSite::DeadlineWaiter);
    let (release, unload) = unload_held(&engine, &holder);
    // timer: deadline — the deadline waiter reaches its hold; expiry fails the test
    waiter_held
        // timer: deadline — the event must arrive; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("the deadline waiter holds before its wake");
    assert!(
        wakes.try_recv().is_ok(),
        "closing the unused funds fired the armed wake"
    );
    waiter_resume.send(()).expect("resume the deadline waiter");
    release.send(()).expect("release the unload");
    unload.join().expect("unload");
    assert!(matches!(
        try_admit(&engine, "parked", &parker),
        PluginAdmissionResult::Queued { .. }
    ));
}

/// Retirement does not return a published completion's reservation: the
/// completion moves to the retired queue, and its reservation returns when
/// the host drains it, which fires the armed wake like any drain.
#[test]
fn a_retired_completion_returns_its_reservation_when_drained() {
    let (engine, events) = probed_engine(completion_capacity_one());
    let (holder, parker) = (PluginKey("undrained".into()), PluginKey("parker".into()));
    engine.load_plugin(registration(&holder, Arc::new(EchoRuntime)));
    engine.load_plugin(registration(&parker, Arc::new(EchoRuntime)));
    assert!(matches!(
        admit(&engine, PluginInvocationClass::Background, expired("held", &holder)),
        PluginAdmissionResult::Queued { .. }
    ));
    wait_quiet(&events, 4);
    let wakes = notifications(&engine);

    // The unload signals the deadline waiter. Its idle report after the
    // unload proves that pass ended, so it cannot fire after the arm below.
    while events.try_recv().is_ok() {}
    engine.unload_plugin(PluginUnloadSpec {
        request_id: RequestId("unload".into()),
        plugin_key: holder,
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    loop {
        // timer: deadline — the deadline waiter ends the unload's pass; expiry fails the test
        if events
            // timer: deadline — the event must arrive; expiry fails the test
            .recv_timeout(EVENT_DEADLINE)
            .expect("the deadline waiter goes idle")
            == PluginQueueProbeEvent::DeadlineWaiterIdle
        {
            break;
        }
    }
    while wakes.try_recv().is_ok() {}
    let refused = try_admit(&engine, "parked", &parker);
    assert_eq!(
        cause(&refused),
        PluginBackpressureCause::CompletionReservation,
        "the retired completion still holds its reservation"
    );
    assert!(armed(&engine));
    assert!(wakes.try_recv().is_err(), "a refused admission fires nothing");

    assert_eq!(engine.drain_completions(usize::MAX, usize::MAX).item_count, 1);
    assert!(
        wakes.try_recv().is_ok(),
        "draining the retired completion fires the armed wake"
    );
    assert!(matches!(
        try_admit(&engine, "parked", &parker),
        PluginAdmissionResult::Queued { .. }
    ));
}

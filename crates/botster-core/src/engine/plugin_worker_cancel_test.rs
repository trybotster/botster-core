// Engine call sites of `PluginCancellationToken::cancel` against Core cancel
// targets: no engine lock is held while targets run, and a panicking target
// cannot stop the shared deadline waiter.
//
// Each invocation arms a deadline far in the future. The runtime reports
// when its targets are subscribed, and only then does the test make the
// deadline due, so the deadline waiter always fires it after subscription.

use super::*;
use crate::runtime::CancelTarget;
use std::mem::ManuallyDrop;
use std::sync::mpsc;

/// Bound for each event these tests wait for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(10);
/// Invocation deadline that never fires on its own during a test.
const FAR_DEADLINE_MS: u64 = 600_000;
const DEADLINE_WAITER: &str = "botster-plugin-deadline-waiter";

struct Latch {
    cancelled: Mutex<bool>,
    cvar: Condvar,
}

impl CancelTarget for Latch {
    fn cancelled(&self) {
        *self.cancelled.lock().expect("latch") = true;
        self.cvar.notify_all();
    }
}

/// A runtime that subscribes the extra targets the test installed plus its
/// own latch, reports that, and waits for the cancellation as an event.
#[derive(Default)]
struct CancelWaitRuntime {
    extras: Mutex<Vec<Arc<dyn CancelTarget>>>,
    subscribed: Mutex<Option<mpsc::Sender<RequestId>>>,
}

impl PluginRuntime for CancelWaitRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        let extras = std::mem::take(&mut *self.extras.lock().expect("extras"));
        let _extras = extras
            .into_iter()
            .map(|target| cancellation.subscribe(target))
            .collect::<Vec<_>>();
        let latch = Arc::new(Latch {
            cancelled: Mutex::new(false),
            cvar: Condvar::new(),
        });
        let _latch = cancellation.subscribe(latch.clone());
        if let Some(subscribed) = &*self.subscribed.lock().expect("subscribed") {
            let _ = subscribed.send(request.request_id.clone());
        }
        let mut cancelled = latch.cancelled.lock().expect("latch");
        while !*cancelled {
            cancelled = latch.cvar.wait(cancelled).expect("latch");
        }
        PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: request.request_id,
            handler: request.handler,
            kind: PluginInvocationFailureKind::Cancelled,
            timeout_ms: None,
            reason: "cancel-wait runtime observed cancellation".to_string(),
        })
    }
}

/// The notifying thread's name and the tracked jobs it saw.
type ProbeReport = (Option<String>, usize);

/// Deliberately breaks the target contract by re-entering the engine: it
/// completes only if the cancelling engine thread holds no admission lock.
struct AdmissionProbe {
    engine: PluginWorkerEngine,
    plugin: PluginKey,
    done: Mutex<Option<mpsc::Sender<ProbeReport>>>,
}

impl CancelTarget for AdmissionProbe {
    fn cancelled(&self) {
        let thread = std::thread::current().name().map(str::to_owned);
        let jobs = self.engine.tracked_job_count(&self.plugin);
        if let Some(done) = self.done.lock().expect("done").take() {
            let _ = done.send((thread, jobs));
        }
    }
}

struct PanickingTarget;

impl CancelTarget for PanickingTarget {
    fn cancelled(&self) {
        panic!("a buggy cancel target");
    }
}

fn load_cancel_wait(
    engine: &PluginWorkerEngine,
    plugin: &PluginKey,
    subscribed: &mpsc::Sender<RequestId>,
) -> Arc<CancelWaitRuntime> {
    let runtime = Arc::new(CancelWaitRuntime::default());
    *runtime.subscribed.lock().expect("subscribed") = Some(subscribed.clone());
    engine.load_plugin(registration(plugin, runtime.clone()));
    runtime
}

fn admit_far(engine: &PluginWorkerEngine, plugin: &PluginKey, id: &str) {
    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            request(id, handler(plugin), FAR_DEADLINE_MS),
            1,
        ),
        PluginAdmissionResult::Queued { .. }
    ));
}

fn await_subscribed(subscribed: &mpsc::Receiver<RequestId>, id: &str) {
    // timer: deadline — the runtime reports its subscription; expiry means it never ran
    let reported = subscribed
        .recv_timeout(EVENT_DEADLINE)
        .expect("the runtime subscribes before the test fires its deadline");
    assert_eq!(reported.0, id);
}

#[test]
fn deadline_cancel_runs_targets_outside_the_admission_lock() {
    // Not dropped on the failure path: a deadlocked deadline waiter would make
    // the engine's drop join hang the harness.
    let engine = ManuallyDrop::new(PluginWorkerEngine::new());
    let plugin = PluginKey("cancel-outside-admission".into());
    let (subscribed_tx, subscribed_rx) = mpsc::channel();
    let runtime = load_cancel_wait(&engine, &plugin, &subscribed_tx);
    let (done_tx, done_rx) = mpsc::channel();
    runtime
        .extras
        .lock()
        .expect("extras")
        .push(Arc::new(AdmissionProbe {
            engine: PluginWorkerEngine::clone(&engine),
            plugin: plugin.clone(),
            done: Mutex::new(Some(done_tx)),
        }));

    admit_far(&engine, &plugin, "deadline");
    await_subscribed(&subscribed_rx, "deadline");
    engine.expire_deadline_now(&RequestId("deadline".into()));

    // timer: deadline — the probe reports from the deadline waiter; expiry means it deadlocked
    let (thread, jobs) = done_rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("a cancel target must run without the admission lock held");
    assert_eq!(thread.as_deref(), Some(DEADLINE_WAITER));
    assert_eq!(jobs, 0, "the fired deadline already retired its job");
    drop(ManuallyDrop::into_inner(engine));
}

#[test]
fn a_panicking_target_does_not_stop_later_deadlines() {
    let engine = ManuallyDrop::new(PluginWorkerEngine::new());
    let (notify_tx, notify_rx) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = notify_tx.send(());
    }));
    let panicking = PluginKey("panicking-target".into());
    let unrelated = PluginKey("unrelated-plugin".into());
    let (subscribed_tx, subscribed_rx) = mpsc::channel();
    load_cancel_wait(&engine, &panicking, &subscribed_tx)
        .extras
        .lock()
        .expect("extras")
        .push(Arc::new(PanickingTarget));
    load_cancel_wait(&engine, &unrelated, &subscribed_tx);

    admit_far(&engine, &panicking, "first");
    await_subscribed(&subscribed_rx, "first");
    admit_far(&engine, &unrelated, "second");
    await_subscribed(&subscribed_rx, "second");

    let mut timed_out = Vec::new();
    for id in ["first", "second"] {
        // The panicking target runs on the waiter when "first" fires; the
        // waiter must still be alive to fire "second" afterwards.
        engine.expire_deadline_now(&RequestId(id.into()));
        // timer: deadline — each fired deadline publishes a completion; expiry means the waiter died
        notify_rx
            .recv_timeout(EVENT_DEADLINE)
            .expect("the deadline waiter must survive a panicking target");
        for item in engine.drain_completions(usize::MAX, usize::MAX).completions {
            let PluginInvocationResult::Failed(failure) = item.completion.result else {
                panic!("a cancel-wait invocation cannot complete");
            };
            assert_eq!(failure.kind, PluginInvocationFailureKind::TimedOut);
            timed_out.push(failure.request_id.0);
        }
    }
    assert_eq!(timed_out, ["first", "second"]);
    drop(ManuallyDrop::into_inner(engine));
}

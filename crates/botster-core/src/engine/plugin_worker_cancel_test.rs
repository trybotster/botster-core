// Engine call sites of `PluginCancellationToken::cancel` against Core cancel
// targets: no engine lock is held while targets run, and a panicking target
// cannot stop the shared deadline waiter.

use super::*;
use crate::runtime::CancelTarget;
use std::mem::ManuallyDrop;
use std::sync::mpsc;

/// Bound for each event these tests wait for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(10);

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

/// A runtime that waits for its invocation's cancellation as an event, after
/// subscribing the extra targets that the test installed.
#[derive(Default)]
struct CancelWaitRuntime {
    extras: Mutex<Vec<Arc<dyn CancelTarget>>>,
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

/// Deliberately breaks the target contract by re-entering the engine: it
/// completes only if the cancelling engine thread holds no admission lock.
struct AdmissionProbe {
    engine: PluginWorkerEngine,
    plugin: PluginKey,
    done: Mutex<Option<mpsc::Sender<usize>>>,
}

impl CancelTarget for AdmissionProbe {
    fn cancelled(&self) {
        let jobs = self.engine.tracked_job_count(&self.plugin);
        if let Some(done) = self.done.lock().expect("done").take() {
            let _ = done.send(jobs);
        }
    }
}

struct PanickingTarget;

impl CancelTarget for PanickingTarget {
    fn cancelled(&self) {
        panic!("a buggy cancel target");
    }
}

fn load_cancel_wait(engine: &PluginWorkerEngine, plugin: &PluginKey) -> Arc<CancelWaitRuntime> {
    let runtime = Arc::new(CancelWaitRuntime::default());
    engine.load_plugin(registration(plugin, runtime.clone()));
    runtime
}

#[test]
fn deadline_cancel_runs_targets_outside_the_admission_lock() {
    // Not dropped on the failure path: a deadlocked deadline waiter would make
    // the engine's drop join hang the harness.
    let engine = ManuallyDrop::new(PluginWorkerEngine::new());
    let plugin = PluginKey("cancel-outside-admission".into());
    let runtime = load_cancel_wait(&engine, &plugin);
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

    assert!(matches!(
        engine.try_admit(
            PluginInvocationClass::Background,
            request("deadline", handler(&plugin), 20),
            1,
        ),
        PluginAdmissionResult::Queued { .. }
    ));

    // timer: deadline — the probe reports from the deadline waiter; expiry means it deadlocked
    let jobs = done_rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("a cancel target must run without the admission lock held");
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
    load_cancel_wait(&engine, &panicking)
        .extras
        .lock()
        .expect("extras")
        .push(Arc::new(PanickingTarget));
    load_cancel_wait(&engine, &unrelated);

    for (plugin, id, timeout_ms) in [(&panicking, "first", 10), (&unrelated, "second", 100)] {
        assert!(matches!(
            engine.try_admit(
                PluginInvocationClass::Background,
                request(id, handler(plugin), timeout_ms),
                1,
            ),
            PluginAdmissionResult::Queued { .. }
        ));
    }

    let mut timed_out = Vec::new();
    while timed_out.len() < 2 {
        // timer: deadline — both deadlines publish a completion; expiry means the waiter died
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
    timed_out.sort();
    assert_eq!(timed_out, ["first", "second"]);
    drop(ManuallyDrop::into_inner(engine));
}

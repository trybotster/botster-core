//! Cancel-wake contract for `PluginCancellationToken::subscribe`.

use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use super::{CancelTarget, PluginCancellationToken};

/// Bound for the re-entrant wake test; expiry fails the test instead of hanging it.
const WAKE_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Counting(AtomicUsize);

impl CancelTarget for Counting {
    fn cancelled(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Counting {
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

struct Panicking;

impl CancelTarget for Panicking {
    fn cancelled(&self) {
        panic!("a buggy cancel target");
    }
}

#[test]
fn subscribe_then_cancel_notifies_once() {
    let token = PluginCancellationToken::new();
    let target = Arc::new(Counting::default());
    let _subscription = token.subscribe(target.clone());
    assert_eq!(target.count(), 0);

    token.cancel();
    token.cancel();

    assert_eq!(target.count(), 1);
    assert!(token.is_cancelled());
}

#[test]
fn subscribe_after_cancel_notifies_at_once() {
    let token = PluginCancellationToken::new();
    token.cancel();
    let target = Arc::new(Counting::default());

    let _subscription = token.subscribe(target.clone());

    assert_eq!(target.count(), 1);
}

#[test]
fn dropping_the_subscription_before_cancel_removes_the_target() {
    let token = PluginCancellationToken::new();
    let dropped = Arc::new(Counting::default());
    let kept = Arc::new(Counting::default());

    let subscription = token.subscribe(dropped.clone());
    let _kept = token.subscribe(kept.clone());
    drop(subscription);
    token.cancel();

    assert_eq!(dropped.count(), 0);
    assert_eq!(kept.count(), 1);
}

#[test]
fn clones_share_one_cancellation() {
    let token = PluginCancellationToken::new();
    let clone = token.clone();
    let target = Arc::new(Counting::default());
    let _subscription = clone.subscribe(target.clone());

    token.cancel();

    assert!(clone.is_cancelled());
    assert_eq!(target.count(), 1);
}

/// A panicking target neither skips the other targets nor unwinds the
/// cancelling thread, before or after it registers.
#[test]
fn a_panicking_target_does_not_stop_the_others() {
    let token = PluginCancellationToken::new();
    let before = Arc::new(Counting::default());
    let after = Arc::new(Counting::default());
    let _before = token.subscribe(before.clone());
    let _panicking = token.subscribe(Arc::new(Panicking));
    let _after = token.subscribe(after.clone());

    token.cancel();

    assert_eq!(before.count(), 1);
    assert_eq!(after.count(), 1);
    let _late_panicking = token.subscribe(Arc::new(Panicking));
    let late = Arc::new(Counting::default());
    let _late = token.subscribe(late.clone());
    assert_eq!(late.count(), 1);
}

/// A target that re-enters the token completes only if targets run after the
/// token's lock is released.
struct Reentrant {
    token: PluginCancellationToken,
    done: Mutex<Option<mpsc::Sender<usize>>>,
}

impl CancelTarget for Reentrant {
    fn cancelled(&self) {
        let inner = Arc::new(Counting::default());
        let nested = self.token.subscribe(inner.clone());
        drop(nested);
        self.token.cancel();
        if let Some(done) = self.done.lock().expect("done sender").take() {
            let _ = done.send(inner.count());
        }
    }
}

#[test]
fn targets_run_outside_the_token_lock() {
    let token = PluginCancellationToken::new();
    let (done_tx, done_rx) = mpsc::channel();
    // Not dropped on the failure path: after a deadlock, dropping it would
    // block on the lock that the stuck canceller holds, and hang the harness.
    let subscription = ManuallyDrop::new(token.subscribe(Arc::new(Reentrant {
        token: token.clone(),
        done: Mutex::new(Some(done_tx)),
    })));

    let canceller = token.clone();
    thread::spawn(move || canceller.cancel());

    let nested_runs = done_rx
        // timer: deadline — the re-entrant target reports completion; expiry means a deadlock
        .recv_timeout(WAKE_DEADLINE)
        .expect("a re-entrant target must not deadlock on the token lock");
    assert_eq!(nested_runs, 1);
    drop(ManuallyDrop::into_inner(subscription));
}

/// Subscriptions racing one cancel: every target is notified exactly once,
/// whichever side wins the race.
#[test]
fn racing_subscriptions_are_each_notified_exactly_once() {
    const ROUNDS: usize = 200;
    const SUBSCRIBERS: usize = 8;

    for _ in 0..ROUNDS {
        let token = PluginCancellationToken::new();
        let target = Arc::new(Counting::default());
        let start = Arc::new(Barrier::new(SUBSCRIBERS + 1));
        let mut handles = Vec::new();
        for _ in 0..SUBSCRIBERS {
            let token = token.clone();
            let target = target.clone();
            let start = start.clone();
            handles.push(thread::spawn(move || {
                start.wait();
                token.subscribe(target)
            }));
        }
        start.wait();
        token.cancel();
        let subscriptions: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("subscriber thread"))
            .collect();

        assert_eq!(target.count(), SUBSCRIBERS);
        drop(subscriptions);
        assert_eq!(target.count(), SUBSCRIBERS);
    }
}

/// `on_cancel` registered after cancellation runs at once, on the caller.
#[test]
fn on_cancel_after_cancel_runs_at_once() {
    let token = PluginCancellationToken::new();
    token.cancel();
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    token.on_cancel(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

/// `on_cancel` runs exactly once, however often the token is cancelled.
#[test]
fn on_cancel_runs_exactly_once() {
    let token = PluginCancellationToken::new();
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    token.on_cancel(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(runs.load(Ordering::SeqCst), 0, "not before cancellation");
    token.cancel();
    token.cancel();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

/// A token dropped without cancellation drops its `on_cancel` callbacks
/// without running them.
#[test]
fn on_cancel_does_not_run_when_the_token_drops_uncancelled() {
    struct DropMark(Arc<AtomicUsize>);
    impl Drop for DropMark {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let runs = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let token = PluginCancellationToken::new();
    let counted = Arc::clone(&runs);
    let mark = DropMark(Arc::clone(&drops));
    token.on_cancel(move || {
        let _mark = &mark;
        counted.fetch_add(1, Ordering::SeqCst);
    });
    drop(token);
    assert_eq!(runs.load(Ordering::SeqCst), 0, "never ran");
    assert_eq!(drops.load(Ordering::SeqCst), 1, "dropped with the token");
}

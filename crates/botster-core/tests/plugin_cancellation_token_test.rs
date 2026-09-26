//! Cancel-wake contract for `PluginCancellationToken::subscribe`.

use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use std::time::Duration;

use botster_core::PluginCancellationToken;

/// Bound for the re-entrant wake test; expiry fails the test instead of hanging it.
const WAKE_DEADLINE: Duration = Duration::from_secs(10);

fn counting_wake(count: &Arc<AtomicUsize>) -> Box<dyn FnOnce() + Send> {
    let count = count.clone();
    Box::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
    })
}

#[test]
fn subscribe_then_cancel_runs_the_wake_once() {
    let token = PluginCancellationToken::new();
    let count = Arc::new(AtomicUsize::new(0));
    let _subscription = token.subscribe(counting_wake(&count));
    assert_eq!(count.load(Ordering::SeqCst), 0);

    token.cancel();
    token.cancel();

    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(token.is_cancelled());
}

#[test]
fn subscribe_after_cancel_runs_the_wake_at_once() {
    let token = PluginCancellationToken::new();
    token.cancel();
    let count = Arc::new(AtomicUsize::new(0));

    let _subscription = token.subscribe(counting_wake(&count));

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn dropping_the_subscription_before_cancel_removes_the_wake() {
    let token = PluginCancellationToken::new();
    let dropped = Arc::new(AtomicUsize::new(0));
    let kept = Arc::new(AtomicUsize::new(0));

    let subscription = token.subscribe(counting_wake(&dropped));
    let _kept = token.subscribe(counting_wake(&kept));
    drop(subscription);
    token.cancel();

    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    assert_eq!(kept.load(Ordering::SeqCst), 1);
}

#[test]
fn clones_share_one_cancellation() {
    let token = PluginCancellationToken::new();
    let clone = token.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let _subscription = clone.subscribe(counting_wake(&count));

    token.cancel();

    assert!(clone.is_cancelled());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// A wake that re-enters the token (subscribe, cancel, drop a subscription)
/// completes only if wakes run after the token's lock is released.
#[test]
fn wakes_run_outside_the_token_lock() {
    let token = PluginCancellationToken::new();
    let (done_tx, done_rx) = mpsc::channel();
    let reentrant = token.clone();
    // Not dropped on the failure path: after a deadlock, dropping it would
    // block on the lock that the stuck canceller holds, and hang the harness.
    let subscription = ManuallyDrop::new(token.subscribe(Box::new(move || {
        let inner = Arc::new(AtomicUsize::new(0));
        let nested = reentrant.subscribe(counting_wake(&inner));
        drop(nested);
        reentrant.cancel();
        let _ = done_tx.send(inner.load(Ordering::SeqCst));
    })));

    let canceller = token.clone();
    thread::spawn(move || canceller.cancel());

    let nested_runs = done_rx
        // timer: deadline — the re-entrant wake reports completion; expiry means a deadlock
        .recv_timeout(WAKE_DEADLINE)
        .expect("a re-entrant wake must not deadlock on the token lock");
    assert_eq!(nested_runs, 1);
    drop(ManuallyDrop::into_inner(subscription));
}

/// Subscriptions racing one cancel: every wake runs exactly once, whichever
/// side wins the race.
#[test]
fn racing_subscriptions_each_run_exactly_once() {
    const ROUNDS: usize = 200;
    const SUBSCRIBERS: usize = 8;

    for _ in 0..ROUNDS {
        let token = PluginCancellationToken::new();
        let count = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(SUBSCRIBERS + 1));
        let mut handles = Vec::new();
        for _ in 0..SUBSCRIBERS {
            let token = token.clone();
            let count = count.clone();
            let start = start.clone();
            handles.push(thread::spawn(move || {
                start.wait();
                token.subscribe(counting_wake(&count))
            }));
        }
        start.wait();
        token.cancel();
        let subscriptions: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("subscriber thread"))
            .collect();

        assert_eq!(count.load(Ordering::SeqCst), SUBSCRIBERS);
        drop(subscriptions);
        assert_eq!(count.load(Ordering::SeqCst), SUBSCRIBERS);
    }
}

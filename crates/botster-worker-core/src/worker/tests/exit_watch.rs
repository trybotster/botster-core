//! Steward ruling R-106: a payload exit that the worker cannot observe. The worker kills its own payload's group while it
//! holds the leader unreaped, sends no `Exited` and no other end message, and ends with a failure.

use super::*;

const ERRNO: i32 = 10;

fn failed() -> Input {
    Input::ExitWatchFailed { errno: ERRNO }
}

/// R-106 (b), (c): the group kill, then the failed end; no frame, no reap (the driver's end reaps the killed leader).
#[test]
fn a_failed_watch_kills_the_group_and_ends_with_a_failure_and_sends_nothing() {
    let mut w = World::running();
    assert_eq!(
        w.feed(failed()),
        [
            Action::SignalPayload(SIGKILL),
            Action::ExitFailure { errno: ERRNO }
        ]
    );
}

/// A pending `Stop` sent `SIGTERM`, not the kill: the failed watch still kills the group.
#[test]
fn a_failed_watch_after_a_graceful_request_kills_the_group() {
    let mut w = World::running();
    assert_eq!(signals(&w.feed(Input::EndPayload)), [SIGTERM]);
    assert_eq!(
        w.feed(failed()),
        [
            Action::SignalPayload(SIGKILL),
            Action::ExitFailure { errno: ERRNO }
        ]
    );
}

/// A group that was already killed is not killed again.
#[test]
fn a_failed_watch_after_the_group_kill_only_ends() {
    let mut w = World::running();
    assert_eq!(signals(&w.feed(Input::KillPayload)), [SIGKILL]);
    assert_eq!(w.feed(failed()), [Action::ExitFailure { errno: ERRNO }]);
}

/// A watch failure while the spawn is out applies after `Launched`, as an early exit does; nothing else starts.
#[test]
fn a_failed_watch_during_the_spawn_applies_after_launched() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    assert_eq!(w.feed(failed()), []);
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    let reports = w.reports(&actions);
    assert!(
        matches!(reports.as_slice(), [WorkerMsg::Launched { .. }]),
        "{reports:?}"
    );
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(actions.last(), Some(&Action::ExitFailure { errno: ERRNO }));
    assert_eq!(reaps(&actions), 0);
}

/// A watch reports once: after an observed exit, a failure changes nothing (and it cannot come).
#[test]
fn a_failed_watch_after_an_observed_exit_changes_nothing() {
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    assert_eq!(w.feed(failed()), []);
}

/// The worker ends at most once.
#[test]
fn a_second_failed_watch_changes_nothing() {
    let mut w = World::running();
    w.feed(failed());
    assert_eq!(w.feed(failed()), []);
}

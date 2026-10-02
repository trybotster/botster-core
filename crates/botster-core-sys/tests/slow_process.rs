//! The real `Process` edge (plan 2.3), proved with real processes. Slow tier: each test starts a child in its own process
//! group, and ends it on every exit path.
//!
//! Clause: Core AD-6 (identity by pid and start time; a reused pid is never signalled), Core LC-5 and SV-9 (the kill of a
//! group).
#![cfg(feature = "slow")]

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnSpec,
};
use botster_core_sys::process::{identity_state, start_time, Children};

fn spec(program: &str, args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: vec![("A".into(), "1".into())],
        cwd: None,
    }
}

/// Ends the child on every exit path, panics included.
struct Reaper<'a>(&'a mut Children, ProcessIdentity);

impl Drop for Reaper<'_> {
    fn drop(&mut self) {
        self.0.signal_group(self.1, GroupSignal::Kill);
        // Reap what ended, so no zombie is left. The group was killed, so the wait ends.
        let _ = self.0.wait_exit(self.1.pid);
    }
}

/// Core AD-6: a spawned child has an identity that matches while it lives, and the kill of its group ends it with the signal.
#[test]
fn a_spawned_child_matches_its_identity_and_dies_by_the_group_kill() {
    let mut children = Children::new();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let reaper = Reaper(&mut children, identity);
    assert_eq!(identity_state(identity), IdentityState::Matches);
    assert_eq!(start_time(identity.pid), Some(identity.start_time));
    reaper.0.signal_group(identity, GroupSignal::Kill);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit, (identity, ExitStatus::Signal(9)));
    std::mem::forget(reaper);
}

/// Core AD-6: a process that does not match its identity is never signalled.
#[test]
fn a_reused_identity_is_never_signalled() {
    let mut children = Children::new();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let stale = ProcessIdentity {
        pid: identity.pid,
        start_time: identity.start_time + 1,
    };
    assert_eq!(identity_state(stale), IdentityState::Reused);
    reaper.0.signal_group(stale, GroupSignal::Kill);
    assert_eq!(
        identity_state(identity),
        IdentityState::Matches,
        "the child still lives"
    );
}

/// Core LC-1, AD-7: a program that cannot be run is a typed spawn error with the errno, and the exit code of a child that ends
/// by itself is reported.
#[test]
fn a_missing_program_is_a_spawn_error_and_an_exit_code_is_reported() {
    let mut children = Children::new();
    let error = children
        .spawn(&spec("/botster-no-such-program", &[]))
        .expect_err("refused");
    assert_eq!(error.errno, 2, "ENOENT");
    let identity = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit.1, ExitStatus::Code(3));
    std::mem::forget(reaper);
}

/// Core A2-1: the child has exactly the environment that it was given.
#[test]
fn the_child_environment_is_exact() {
    let mut children = Children::new();
    let identity = children
        .spawn(&spec(
            "/bin/sh",
            &["-c", "test \"$A\" = 1 && test -z \"$HOME\""],
        ))
        .expect("spawn");
    let reaper = Reaper(&mut children, identity);
    let exit = reaper.0.wait_exit(identity.pid).expect("the child ends");
    assert_eq!(exit.1, ExitStatus::Code(0), "nothing is inherited");
    std::mem::forget(reaper);
}

/// Core LC-5, AD-6: `EndPayload` is `SIGUSR1` to the worker process. A worker with a handler runs it, and the signal is
/// repeatable. The shell writes a file when its trap is installed, and the test signals only after it sees the file.
#[test]
fn the_worker_control_signal_reaches_the_worker_handler() {
    let ready = std::env::temp_dir().join(format!("botster-core-sys-ready-{}", std::process::id()));
    let _ = std::fs::remove_file(&ready);
    let mut children = Children::new();
    let identity = children
        .spawn(&spec(
            "/bin/sh",
            &[
                "-c",
                "trap 'exit 7' USR1; : > \"$0\"; while :; do :; done",
                ready.to_str().expect("a temp path is UTF-8"),
            ],
        ))
        .expect("spawn");
    let guard = Reaper(&mut children, identity);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10); // timer: deadline — bounds the wait for the trap
    while !ready.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the shell never installed its trap"
        );
        std::thread::yield_now();
    }
    let _ = std::fs::remove_file(&ready);
    guard.0.signal_group(identity, GroupSignal::EndPayload);
    guard.0.signal_group(identity, GroupSignal::EndPayload);
    let exit = guard.0.wait_exit(identity.pid).expect("the worker ends");
    assert_eq!(exit.1, ExitStatus::Code(7), "the handler ran");
}

/// Core TM-6, LC-7: the host wakes at the moment a child ends. The reaper thread calls the notifier after it queued the
/// exit, and `poll_exit` then returns it at once, with no pump driven by a deadline.
#[test]
fn a_child_exit_calls_the_notifier_and_is_polled() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let notified = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&notified);
    let mut children = Children::with_notify(Arc::new(move || flag.store(true, Ordering::SeqCst)));
    let identity = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10); // timer: deadline — bounds the wait for the exit
    while !notified.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "the notifier never ran"
        );
        std::thread::yield_now();
    }
    let (id, status) = children.poll_exit().expect("the exit is queued");
    assert_eq!(id, identity);
    assert_eq!(status, ExitStatus::Code(3));
    assert!(children.poll_exit().is_none());
    assert!(
        children.wait_exit(identity.pid).is_none(),
        "an exit is taken once"
    );
}

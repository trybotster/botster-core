//! The real `Process` edge (plan 2.3), proved with real processes. Slow tier: each test starts a child in its own process
//! group, and ends it on every exit path.
//!
//! The tests never wait through the code that they test: the guard kills a group with its own call, and an exit is awaited
//! through the notifier with a deadline. So a defect in `Children` fails a test; it never hangs one.
//!
//! Clause: Core AD-6 (identity by pid and start time; a reused pid is never signalled), Core LC-5 and SV-9 (the kill of a
//! group), Core TM-6 (the reaper's notifier).
#![cfg(feature = "slow")]

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnSpec,
};
use botster_core_sys::process::{identity_state, start_time, Children};
use rustix::process::{kill_process_group, Pid, Signal};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

fn spec(program: &str, args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: vec![("A".into(), "1".into())],
        cwd: None,
    }
}

/// Children whose reaper threads tell the test of each exit.
fn children() -> (Children, Receiver<()>) {
    let (notified, woken) = std::sync::mpsc::channel();
    let children = Children::with_notify(Arc::new(move || {
        let _ = notified.send(());
    }));
    (children, woken)
}

/// The next exit, after the notifier said that one was queued.
fn next_exit(children: &mut Children, woken: &Receiver<()>) -> (ProcessIdentity, ExitStatus) {
    loop {
        if let Some(exit) = children.poll_exit() {
            return exit;
        }
        woken
            // timer: deadline — bounds the wait for an exit
            .recv_timeout(Duration::from_secs(10))
            .expect("a child ended");
    }
}

/// Kills the child's group on every exit path, panics included, with its own call (the reaper thread reaps the child). A
/// test disarms it once the child has ended, so it never signals a group whose pid may be reused.
struct Guard {
    pid: u32,
    armed: bool,
}

impl Guard {
    fn new(identity: ProcessIdentity) -> Guard {
        Guard {
            pid: identity.pid,
            armed: true,
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let pid = i32::try_from(self.pid).ok().and_then(Pid::from_raw);
        if let (true, Some(pid)) = (self.armed, pid) {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }
}

/// Core AD-6: a spawned child has an identity that matches while it lives, and the kill of its group ends it with the signal.
#[test]
fn a_spawned_child_matches_its_identity_and_dies_by_the_group_kill() {
    let (mut children, woken) = children();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let mut guard = Guard::new(identity);
    assert_eq!(identity_state(identity), IdentityState::Matches);
    assert_eq!(start_time(identity.pid), Some(identity.start_time));
    children.signal_group(identity, GroupSignal::Kill);
    assert_eq!(
        next_exit(&mut children, &woken),
        (identity, ExitStatus::Signal(9))
    );
    guard.armed = false;
}

/// Core AD-6: a process that does not match its identity is never signalled.
#[test]
fn a_reused_identity_is_never_signalled() {
    let (mut children, _woken) = children();
    let identity = children.spawn(&spec("/bin/sleep", &["30"])).expect("spawn");
    let _guard = Guard::new(identity);
    let stale = ProcessIdentity {
        pid: identity.pid,
        start_time: identity.start_time + 1,
    };
    assert_eq!(identity_state(stale), IdentityState::Reused);
    children.signal_group(stale, GroupSignal::Kill);
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
    let (mut children, woken) = children();
    let error = children
        .spawn(&spec("/botster-no-such-program", &[]))
        .expect_err("refused");
    assert_eq!(error.errno, 2, "ENOENT");
    let identity = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    let mut guard = Guard::new(identity);
    assert_eq!(
        next_exit(&mut children, &woken),
        (identity, ExitStatus::Code(3))
    );
    guard.armed = false;
}

/// Core A2-1: the child has exactly the environment that it was given.
#[test]
fn the_child_environment_is_exact() {
    let (mut children, woken) = children();
    let identity = children
        .spawn(&spec(
            "/bin/sh",
            &["-c", "test \"$A\" = 1 && test -z \"$HOME\""],
        ))
        .expect("spawn");
    let mut guard = Guard::new(identity);
    let exit = next_exit(&mut children, &woken);
    guard.armed = false;
    assert_eq!(exit.1, ExitStatus::Code(0), "nothing is inherited");
}

/// Core LC-5, AD-6: `EndPayload` is `SIGUSR1` to the worker process. A worker with a handler runs it, and the signal is
/// repeatable. The shell tells the test that its trap is installed through a FIFO (an external `/bin/echo`), and the test
/// signals only after it read that.
#[test]
fn the_worker_control_signal_reaches_the_worker_handler() {
    let tmp = tempfile::tempdir().unwrap();
    let ready = tmp.path().join("ready");
    let made = std::process::Command::new("/usr/bin/mkfifo")
        .arg(&ready)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    let (mut children, woken) = children();
    let identity = children
        .spawn(&spec(
            "/bin/sh",
            &[
                "-c",
                "trap 'exit 7' USR1; /bin/echo ready > \"$0\"; while :; do :; done",
                ready.to_str().expect("a temp path is UTF-8"),
            ],
        ))
        .expect("spawn");
    let mut guard = Guard::new(identity);
    let (told, heard) = std::sync::mpsc::channel();
    let fifo = ready.clone();
    std::thread::spawn(move || {
        let _ = told.send(std::fs::read_to_string(fifo));
    });
    let said = heard
        // timer: deadline — bounds the wait for the trap
        .recv_timeout(Duration::from_secs(10))
        .expect("the shell installed its trap");
    assert_eq!(said.unwrap().trim(), "ready");
    children.signal_group(identity, GroupSignal::EndPayload);
    children.signal_group(identity, GroupSignal::EndPayload);
    let exit = next_exit(&mut children, &woken);
    guard.armed = false;
    assert_eq!(exit, (identity, ExitStatus::Code(7)), "the handler ran");
}

/// Core TM-6, LC-7: the host wakes at the moment a child ends. The reaper thread calls the notifier after it queued the
/// exit, and `poll_exit` then returns it at once, with no pump driven by a deadline.
#[test]
fn a_child_exit_calls_the_notifier_and_is_polled() {
    let (mut children, woken) = children();
    let identity = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    woken
        // timer: deadline — bounds the wait for the notifier's event
        .recv_timeout(Duration::from_secs(10))
        .expect("the notifier ran");
    let (id, status) = children.poll_exit().expect("the exit is queued");
    assert_eq!(id, identity);
    assert_eq!(status, ExitStatus::Code(3));
    assert!(children.poll_exit().is_none());
    assert!(
        children.wait_exit(identity.pid).is_none(),
        "an exit is taken once"
    );
}

/// `wait_exit` takes the exit of the child that it names, also when another child's exit is queued before it; an exit is
/// taken once, and a pid that is not a live child is `None`.
#[test]
fn wait_exit_takes_the_exit_of_its_own_child() {
    let (mut children, woken) = children();
    let first = children
        .spawn(&spec("/bin/sh", &["-c", "exit 3"]))
        .expect("spawn");
    let mut first_guard = Guard::new(first);
    let second = children
        .spawn(&spec("/bin/sh", &["-c", "exit 4"]))
        .expect("spawn");
    let mut second_guard = Guard::new(second);
    // Both exits are queued before `wait_exit` runs, so it never blocks here.
    for _ in 0..2 {
        woken
            // timer: deadline — bounds the wait for each exit
            .recv_timeout(Duration::from_secs(10))
            .expect("a child ended");
    }
    first_guard.armed = false;
    second_guard.armed = false;
    assert_eq!(
        children.wait_exit(second.pid),
        Some((second, ExitStatus::Code(4)))
    );
    assert_eq!(
        children.wait_exit(first.pid),
        Some((first, ExitStatus::Code(3)))
    );
    assert_eq!(children.wait_exit(first.pid), None, "taken once");
    assert_eq!(children.wait_exit(u32::MAX - 1), None, "not a child");
}

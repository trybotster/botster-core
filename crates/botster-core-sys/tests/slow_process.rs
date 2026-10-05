//! The real `Process` edge (plan 2.3), proved with real processes. Slow tier: each test starts a child in its own process
//! group, and ends it on every exit path.
//!
//! The tests never wait through the code that they test: the guard kills a group with its own call, and an exit is awaited
//! through the notifier with a deadline. The anchor owns the group independently of `Children`.
//!
//! Clause: Core AD-6 (identity by pid and start time; a reused pid is never signalled), Core LC-5 and SV-9 (the kill of a
//! group), Core TM-6 (the reaper's notifier).
#![cfg(feature = "slow")]

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnSpec,
};
#[cfg(target_os = "macos")]
use botster_core_sys::process::start_time;
use botster_core_sys::process::{identity_state, Children};
use rustix::process::{test_kill_process, Pid};

#[path = "common/process_guard.rs"]
mod process_guard;
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

fn mkfifo(path: &std::path::Path) {
    let made = std::process::Command::new("/usr/bin/mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
}

/// A shell command that waits without using the CPU until a signal ends it: a `/bin/cat` blocked on a FIFO that nobody
/// writes, in the background, so that a trapped signal interrupts the shell's `wait` at once. The group guard ends it on
/// every exit path, a killed test included (plan R12), so it needs no timer and no parent check.
fn waiting_child(dir: &std::path::Path) -> String {
    let never = dir.join("never");
    mkfifo(&never);
    format!("/bin/cat '{}' & wait $!", never.display())
}

/// The anchor owns group membership before the child body can run.
struct Guard {
    _group: process_guard::GroupGuard,
    _dir: tempfile::TempDir,
}

fn spawn(children: &mut Children, original: SpawnSpec) -> (ProcessIdentity, Guard) {
    let dir = tempfile::tempdir().unwrap();
    let group = process_guard::GroupGuard::new(dir.path());
    let mut wrapped = spec(
        "/bin/sh",
        &["-c", &format!("{}exec \"$@\"", group.prefix()), "guard"],
    );
    wrapped.args.push(original.program.into());
    wrapped.args.extend(original.args);
    wrapped.env = original.env;
    wrapped.cwd = original.cwd;
    let guard = Guard {
        _group: group,
        _dir: dir,
    };
    let identity = children.spawn(&wrapped).expect("spawn");
    (identity, guard)
}

fn alive(pid: u32) -> bool {
    i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .is_some_and(|pid| test_kill_process(pid).is_ok())
}

/// Core AD-6: a spawned child has an identity that matches while it lives, and the kill of its group ends it with the signal.
#[test]
fn a_spawned_child_matches_its_identity_and_dies_by_the_group_kill() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut children, woken) = children();
    let (identity, _guard) = spawn(
        &mut children,
        spec("/bin/sh", &["-c", &waiting_child(tmp.path())]),
    );
    assert_eq!(identity_state(identity), IdentityState::Matches);
    children.signal_group(identity, GroupSignal::Kill);
    assert_eq!(
        next_exit(&mut children, &woken),
        (identity, ExitStatus::Signal(9))
    );
}

/// The worker-control script of these tests: a trap for `SIGUSR1` that exits 7, a line on the FIFO `$0` once the trap is in
/// place, and a wait that only a signal ends.
fn trapped_child(dir: &std::path::Path) -> (String, std::path::PathBuf) {
    let ready = dir.join("ready");
    mkfifo(&ready);
    let script = format!(
        "trap 'exit 7' USR1; /bin/echo ready > \"$0\"; {}",
        waiting_child(dir)
    );
    (script, ready)
}

/// Waits, with a deadline, for the line that a trapped child writes once its trap is in place.
fn trap_ready(ready: std::path::PathBuf) {
    let (told, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = told.send(std::fs::read_to_string(ready));
    });
    let said = heard
        // timer: deadline — bounds the wait for the trap
        .recv_timeout(Duration::from_secs(10))
        .expect("the shell installed its trap");
    assert_eq!(said.unwrap().trim(), "ready");
}

/// Core AD-6: a process that does not match its identity is never signalled. The stale identity gets a `SIGKILL`; the real
/// identity then gets `EndPayload`, whose trap exits 7. A stale kill that got through would have ended the child first, with
/// signal 9, because a `SIGKILL` that was sent earlier is never overtaken.
#[test]
fn a_reused_identity_is_never_signalled() {
    let tmp = tempfile::tempdir().unwrap();
    let (script, ready) = trapped_child(tmp.path());
    let (mut children, woken) = children();
    let (identity, _guard) = spawn(
        &mut children,
        spec(
            "/bin/sh",
            &["-c", &script, ready.to_str().expect("a temp path is UTF-8")],
        ),
    );
    trap_ready(ready);
    let stale = ProcessIdentity {
        pid: identity.pid,
        start_time: identity.start_time + 1,
    };
    assert_eq!(identity_state(stale), IdentityState::Reused);
    children.signal_group(stale, GroupSignal::Kill);
    children.signal_group(identity, GroupSignal::EndPayload);
    assert_eq!(
        next_exit(&mut children, &woken),
        (identity, ExitStatus::Code(7)),
        "the trap ended the child: the stale kill never reached it"
    );
}

/// Core AD-6: the start time on macOS is the process start that the system reports, to the second: `start_time` divided by a
/// million is the epoch second of `ps -o lstart=`, an independent source (audit A15: the identity must tell two start times
/// apart).
#[cfg(target_os = "macos")]
#[test]
fn the_start_time_is_the_start_that_the_system_reports() {
    let me = std::process::id();
    let lstart = std::process::Command::new("/bin/ps")
        .args(["-o", "lstart=", "-p", &me.to_string()])
        .output()
        .expect("ps runs");
    let lstart = String::from_utf8(lstart.stdout).unwrap();
    let epoch = std::process::Command::new("/bin/date")
        .args(["-j", "-f", "%a %b %e %T %Y", lstart.trim(), "+%s"])
        .env("LC_ALL", "C")
        .output()
        .expect("date runs");
    let epoch: u64 = String::from_utf8(epoch.stdout)
        .unwrap()
        .trim()
        .parse()
        .expect("an epoch second");
    assert_eq!(
        start_time(me).expect("this process exists") / 1_000_000,
        epoch
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
    let (identity, _guard) = spawn(&mut children, spec("/bin/sh", &["-c", "exit 3"]));
    assert_eq!(
        next_exit(&mut children, &woken),
        (identity, ExitStatus::Code(3))
    );
}

/// Core A2-1: the child has exactly the environment that it was given.
#[test]
fn the_child_environment_is_exact() {
    let (mut children, woken) = children();
    let (_identity, _guard) = spawn(
        &mut children,
        spec("/bin/sh", &["-c", "test \"$A\" = 1 && test -z \"$HOME\""]),
    );
    let exit = next_exit(&mut children, &woken);
    assert_eq!(exit.1, ExitStatus::Code(0), "nothing is inherited");
}

/// Core LC-5, AD-6: `EndPayload` is `SIGUSR1` to the worker process. A worker with a handler runs it, and the signal is
/// repeatable. The shell tells the test that its trap is installed through a FIFO (an external `/bin/echo`), and the test
/// signals only after it read that.
#[test]
fn the_worker_control_signal_reaches_the_worker_handler() {
    let tmp = tempfile::tempdir().unwrap();
    let (script, ready) = trapped_child(tmp.path());
    let (mut children, woken) = children();
    let (identity, _guard) = spawn(
        &mut children,
        spec(
            "/bin/sh",
            &["-c", &script, ready.to_str().expect("a temp path is UTF-8")],
        ),
    );
    trap_ready(ready);
    children.signal_group(identity, GroupSignal::EndPayload);
    children.signal_group(identity, GroupSignal::EndPayload);
    let exit = next_exit(&mut children, &woken);
    assert_eq!(exit, (identity, ExitStatus::Code(7)), "the handler ran");
}

/// Core TM-6, LC-7: the host wakes at the moment a child ends. The reaper thread calls the notifier after it queued the
/// exit, and `poll_exit` then returns it at once, with no pump driven by a deadline.
#[test]
fn a_child_exit_calls_the_notifier_and_is_polled() {
    let (mut children, woken) = children();
    let (identity, _guard) = spawn(&mut children, spec("/bin/sh", &["-c", "exit 3"]));
    woken
        // timer: deadline — bounds the wait for the notifier's event
        .recv_timeout(Duration::from_secs(10))
        .expect("the notifier ran");
    let (id, status) = children.poll_exit().expect("the exit is queued");
    assert_eq!(id, identity);
    assert_eq!(status, ExitStatus::Code(3));
    assert!(children.poll_exit().is_none(), "an exit is taken once");
}

/// Plan R12 and testing rule 10: the anchor ends the group without production cleanup.
/// The production reaper still owns the child wait. The test never competes for that wait.
#[test]
fn no_child_is_left_when_the_cleanup_of_a_test_fails() {
    // The cleanup of `Children` is skipped: only the guard ends the child.
    let tmp = tempfile::tempdir().unwrap();
    let (mut children, _woken) = children();
    let (identity, _guard) = spawn(
        &mut children,
        spec("/bin/sh", &["-c", &waiting_child(tmp.path())]),
    );
    assert!(alive(identity.pid));
    drop(_guard);
    next_exit(&mut children, &_woken);
    assert!(
        !alive(identity.pid),
        "the anchor killed the child and the production reaper reaped it"
    );
}

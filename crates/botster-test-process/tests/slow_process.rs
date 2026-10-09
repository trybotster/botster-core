//! The real-process tests of the crate (slow tier, BUILD.md testing rule 2): the owned child, the blocked fixture child, and
//! the group guard with its anchor, on real processes, groups and sessions. Each proves its own cleanup path: a test whose
//! cleanup is broken fails here, never leaves a process behind.
//!
//! The helpers are entries of this test binary that run only when the test starts them (an environment variable); they are
//! no-ops otherwise.
#![cfg(feature = "slow")]

use botster_test_process::anchor::Report;
use botster_test_process::platform::{await_end, pid, start_time, Waited};
use botster_test_process::{
    eof, first_line, Blocker, Bounded, Deadline, Guard, OwnedChild, CLEANUP,
};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

const ANCHOR: &str = env!("CARGO_BIN_EXE_botster-test-anchor");
const HELPER: &str = "BOTSTER_TEST_PROCESS_HELPER";
const KILL: i32 = 9;
const TERM: i32 = 15;

fn guard(dir: &Path) -> Guard {
    Guard::with_binary(dir, PathBuf::from(ANCHOR)).unwrap()
}

/// This test binary, running the helper `name` with `dir`.
fn helper(name: &str, dir: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env(HELPER, dir);
    command
}

fn helper_dir() -> Option<PathBuf> {
    std::env::var_os(HELPER).map(PathBuf::from)
}

/// Production's reap of its own child: it observes the exit within the cleanup bound, then reaps by the exact pid. A child
/// that someone else reaped fails here (the wait would report no such child).
fn production_reap(child: &mut Child) -> ExitStatus {
    let waited = await_end(pid(child.id()).unwrap(), Deadline::cleanup()).unwrap();
    assert_ne!(waited, Waited::Deadline, "the child did not end");
    child.wait().expect("production still owns its child")
}

/// This process's stderr, for a child's stdout: a helper reports on stderr, because the test harness writes its own lines
/// to stdout.
fn stderr() -> Stdio {
    use std::os::fd::AsFd;
    Stdio::from(std::io::stderr().as_fd().try_clone_to_owned().unwrap())
}

/// The panic message of a dropped owner.
fn drop_failure(owner: impl Send + 'static) -> String {
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(owner)))
        .expect_err("the cleanup failed");
    failed.downcast_ref::<String>().expect("a report").clone()
}

#[test]
fn a_child_that_does_not_end_fails_its_status_and_its_drop_still_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut child = OwnedChild::spawn(&mut blocker.command()).unwrap();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        child.status_by(Deadline::after(Duration::ZERO))
    }))
    .expect_err("a blocked child does not end");
    let message = failed.downcast_ref::<String>().unwrap();
    assert_eq!(
        message,
        &format!("the test child {} did not end within 0ns", child.id())
    );
    // The drop kills and reaps it; a drop that could not would fail this test.
    drop(child);
}

#[test]
fn a_released_blocker_ends_its_child_with_code_0_and_its_shell_words_name_the_fifo() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    assert!(
        Blocker::new(dir.path(), "block").is_err(),
        "the FIFO exists"
    );
    let mut child = OwnedChild::spawn(
        Command::new("/bin/sh")
            .args(["-c", &blocker.shell()])
            .stdout(Stdio::piped()),
    )
    .unwrap();
    let (_rest, line) = {
        blocker.send(b"through the fifo\n").unwrap();
        first_line(child.take_stdout().unwrap())
    };
    assert_eq!(line, "through the fifo\n");
    blocker.release();
    assert!(blocker.send(b"x").is_err());
    assert_eq!(child.status().code(), Some(0));
    assert_eq!(child.status().code(), Some(0), "the status is kept");
}

/// The blocked child ends by itself when the test that holds its FIFO dies with no cleanup: the end of file of the pipe
/// that only the blocked child holds proves it.
#[test]
fn a_blocked_child_ends_when_the_test_that_holds_its_fifo_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let mut parent = OwnedChild::spawn_group(
        helper("helper_blocked_parent", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .unwrap();
    let (rest, line) = first_line(parent.take_stderr().unwrap());
    assert_eq!(line, "opened\n");
    parent.kill().unwrap();
    eof(rest.into_inner());
    assert_eq!(parent.status().signal(), Some(KILL));
}

#[test]
fn helper_blocked_parent() {
    let Some(dir) = helper_dir() else { return };
    let mut blocker = Blocker::new(&dir, "block").unwrap();
    // The child copies the FIFO to this process's stderr, the test's pipe: the line proves that it has opened the FIFO.
    let _child = OwnedChild::spawn(blocker.command().stdout(stderr())).unwrap();
    blocker.send(b"opened\n").unwrap();
    // The test kills this process while it waits here.
    let _ = Bounded::new(std::io::stdin()).to_eof(Deadline::cleanup());
}

/// A group owner ends every member of its group, a member that outlives the leader included, and keeps the group id until
/// then: the leader's status is readable before the drop, and the end of file proves that every member ended.
#[test]
fn a_group_owner_ends_a_member_that_outlives_the_leader() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut leader = OwnedChild::spawn_group(
        Command::new("/bin/sh")
            .args(["-c", &format!("{} & exit 3", blocker.shell())])
            .stdout(Stdio::piped()),
    )
    .unwrap();
    blocker.send(b"member\n").unwrap();
    let (rest, line) = first_line(leader.take_stdout().unwrap());
    assert_eq!(line, "member\n");
    assert!(leader.exited_within(Deadline::cleanup()).unwrap());
    assert_eq!(leader.status().code(), Some(3));
    drop(leader);
    eof(rest.into_inner());
}

/// The production-style start of a wrapped program: in a new group, as an unreaped child of the test (production's role).
fn start(guard: &Guard, dir: &Path, program: &Path, args: &[&str]) -> Child {
    let script = dir.join("wrapped.sh");
    guard.wrapper(&script, program, args).unwrap();
    Command::new(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap()
}

fn one_anchor(guard: &mut Guard) -> Report {
    let reports = guard.anchors(1, Deadline::cleanup()).unwrap();
    assert_eq!(reports.len(), 1);
    reports[0]
}

/// The guard's drop ends the wrapped program's group, and production then reaps its own child: the guard reaped nothing
/// (lead ruling 2026-10-08, condition 1e). The anchor reports the leader's and its own identity.
#[test]
fn the_guard_ends_the_group_and_production_still_reaps_its_own_child() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut guard = guard(dir.path());
    let script = format!("{} & /bin/echo $!; wait", blocker.shell());
    let mut production = start(&guard, dir.path(), Path::new("/bin/sh"), &["-c", &script]);
    let report = one_anchor(&mut guard);
    let leader = production.id();
    assert_eq!((report.leader.pid, report.group), (leader, leader));
    assert_eq!(
        start_time(pid(leader).unwrap()),
        Some(report.leader.start_time)
    );
    assert_eq!(
        start_time(pid(report.anchor.pid).unwrap()),
        Some(report.anchor.start_time)
    );
    let (mut rest, member) = first_line(production.stdout.take().unwrap());
    assert!(member.trim().parse::<u32>().is_ok(), "{member}");
    blocker.send(b"up\n").unwrap();
    assert_eq!(
        rest.line(Deadline::cleanup()).unwrap().as_deref(),
        Some("up\n")
    );
    drop(guard);
    assert_eq!(production_reap(&mut production).signal(), Some(KILL));
    eof(rest.into_inner());
}

/// With a grace, the anchor first sends `TERM`: the members end by it within the grace, and no `KILL` reaches them.
#[test]
fn a_grace_ends_the_members_by_term() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut guard = guard(dir.path()).grace(CLEANUP);
    let mut production = start(
        &guard,
        dir.path(),
        Path::new("/bin/cat"),
        &[blocker.path().to_str().unwrap()],
    );
    one_anchor(&mut guard);
    blocker.send(b"up\n").unwrap();
    let (rest, line) = first_line(production.stdout.take().unwrap());
    assert_eq!(line, "up\n");
    drop(guard);
    assert_eq!(production_reap(&mut production).signal(), Some(TERM));
    eof(rest.into_inner());
}

/// When the test dies without any cleanup, the anchor sees the end of its connection and ends the group. The member blocks
/// on a FIFO whose writer this outer test holds, so only the anchor can end it.
#[test]
fn the_anchor_ends_the_group_when_the_test_dies() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut parent = OwnedChild::spawn_group(
        helper("helper_guarded_parent", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .unwrap();
    blocker.send(b"up\n").unwrap();
    let (rest, line) = first_line(parent.take_stderr().unwrap());
    assert_eq!(line, "up\n");
    parent.kill().unwrap();
    eof(rest.into_inner());
    assert_eq!(parent.status().signal(), Some(KILL));
}

#[test]
fn helper_guarded_parent() {
    let Some(dir) = helper_dir() else { return };
    let mut guard = guard(&dir);
    let script = dir.join("wrapped.sh");
    guard
        .wrapper(
            &script,
            Path::new("/bin/cat"),
            &[dir.join("block").to_str().unwrap()],
        )
        .unwrap();
    // Production's child copies the FIFO to this process's stderr, the test's pipe, in its own group.
    let _production = OwnedChild::spawn(
        Command::new(&script)
            .stdin(Stdio::null())
            .stdout(stderr())
            .process_group(0),
    )
    .unwrap();
    one_anchor(&mut guard);
    // The test kills this process while it waits here: neither the guard's drop nor any other cleanup runs.
    let _ = Bounded::new(std::io::stdin()).to_eof(Deadline::cleanup());
    std::mem::forget(guard);
}

/// A cleanup that cannot finish fails the test through the guard: with no time for its rounds, the anchor reports the live
/// member, and the guard's drop fails with that report. The member still ends, by the anchor's last kill.
#[test]
fn a_cleanup_that_cannot_finish_fails_through_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut guard = guard(dir.path()).cleanup(Duration::ZERO);
    let mut production = start(
        &guard,
        dir.path(),
        Path::new("/bin/cat"),
        &[blocker.path().to_str().unwrap()],
    );
    let report = one_anchor(&mut guard);
    let message = drop_failure(guard);
    assert!(
        message.starts_with("the group guard's cleanup failed: members left after 0ns: ")
            && message.contains(&report.leader.pid.to_string()),
        "{message}"
    );
    assert_eq!(production_reap(&mut production).signal(), Some(KILL));
}

/// A leader that moved to another group is never signalled: the anchor refuses, and the guard's drop fails with the
/// refusal (ruling item 13). The leader stays live until its owner ends it.
#[test]
fn a_leader_that_moved_to_another_group_is_refused_and_not_signalled() {
    let dir = tempfile::tempdir().unwrap();
    // The wrapped program joins a group that another process leads, so that it can move to a new group of its own.
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    let holder = OwnedChild::spawn_group(&mut blocker.command()).unwrap();
    let mut guard = guard(dir.path());
    let script = dir.path().join("wrapped.sh");
    guard
        .wrapper(
            &script,
            &std::env::current_exe().unwrap(),
            &["--exact", "helper_moves_its_group", "--nocapture"],
        )
        .unwrap();
    let mut production = OwnedChild::spawn(
        Command::new(&script)
            .env(HELPER, dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .process_group(i32::try_from(holder.id()).unwrap()),
    )
    .unwrap();
    let report = one_anchor(&mut guard);
    assert_eq!(report.group, holder.id());
    let (_rest, line) = first_line(production.take_stderr().unwrap());
    assert_eq!(line, "moved\n");
    let message = drop_failure(guard);
    assert_eq!(
        message,
        format!(
            "the group guard's cleanup failed: refused: the leader {0} moved from group {1} to group {0}",
            production.id(),
            holder.id()
        )
    );
    assert!(
        !production
            .exited_within(Deadline::after(Duration::ZERO))
            .unwrap(),
        "the refused anchor signalled nothing"
    );
}

#[test]
fn helper_moves_its_group() {
    if helper_dir().is_none() {
        return;
    }
    rustix::process::setpgid(None, None).unwrap();
    eprintln!("moved");
    let _ = Bounded::new(std::io::stdin()).to_eof(Deadline::cleanup());
}

/// An anchor in a terminal session survives the hangup that the exit of the session's leader sends to its group, and its
/// group is still ended at the drop.
#[test]
fn an_anchor_survives_the_hangup_of_its_session_leader() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = guard(dir.path());
    let script = dir.path().join("wrapped.sh");
    guard
        .wrapper(&script, Path::new("/bin/sh"), &["-c", "exit 4"])
        .unwrap();
    let (pty, pts) = pty_process::blocking::open().unwrap();
    let mut production = pty_process::blocking::Command::new(&script)
        .spawn(pts)
        .unwrap();
    let report = one_anchor(&mut guard);
    assert_eq!(production_reap(&mut production).code(), Some(4));
    let anchor = pid(report.anchor.pid).unwrap();
    assert_eq!(
        start_time(anchor),
        Some(report.anchor.start_time),
        "the anchor survived its leader's exit"
    );
    drop(guard);
    // Ended: gone, or a zombie that its new parent (init, or a subreaper) has not reaped yet.
    assert_ne!(
        await_end(anchor, Deadline::cleanup()).unwrap(),
        Waited::Deadline,
        "the anchor ended with its group"
    );
    drop(pty);
}

//! The real-process tests of the crate (slow tier, BUILD.md testing rule 2): the owned child, the blocked fixture child, and
//! the group guard with its anchor, on real processes, groups and sessions. Each proves its own cleanup path: a test whose
//! cleanup is broken fails here, never leaves a process behind.
//!
//! The helpers are entries of this test binary that run only when the test starts them (an environment variable); they are
//! no-ops otherwise.
#![cfg(feature = "slow")]

use botster_test_process::anchor::start_anchor;
use botster_test_process::anchor::Report;
use botster_test_process::platform::{await_end, await_status, peek, pid, start_time, Waited};
use botster_test_process::{
    eof, first_line, quoted, run_to_completion, Blocker, Bounded, Deadline, Guard, OwnedChild,
    CLEANUP,
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

/// Production's reap of its own child: its status is available within the cleanup bound, then it is reaped by the exact pid
/// (a wait that cannot block). A child that someone else reaped fails here (the check would report no such child).
fn production_reap(child: &mut Child) -> ExitStatus {
    let status = await_status(pid(child.id()).unwrap(), Deadline::cleanup())
        .expect("production still owns its child");
    assert!(status.is_some(), "the child did not end");
    child
        .try_wait()
        .expect("production still owns its child")
        .expect("a status is available")
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

/// A short-lived tool runs to its exit: its status, its stdout and its stderr come back. Its stdin is null (`cat` reads the end
/// of file at once), and a stderr larger than a pipe's capacity, written before the stdout, does not block the run.
#[test]
fn a_tool_runs_to_its_exit_with_its_output_and_a_null_stdin() {
    let output = run_to_completion(
        Command::new("/bin/sh").args([
            "-c",
            "/bin/cat; printf err >&2; /usr/bin/head -c 1048576 /dev/zero >&2; printf out; exit 3",
        ]),
        Deadline::cleanup(),
    )
    .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr.len(), 3 + (1 << 20));
    assert!(output.stderr.starts_with(b"err"));
}

/// A tool that does not end by the deadline fails the run with `TimedOut`; the run kills and reaps it (a drop that could
/// not would fail this test).
#[test]
fn a_tool_that_does_not_end_by_the_deadline_fails_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    let error = run_to_completion(&mut blocker.command(), Deadline::after(Duration::ZERO))
        .expect_err("a blocked tool does not end");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(
        error
            .to_string()
            .contains(" did not end within 0ns: nothing ended the read within 0ns"),
        "{error}"
    );
}

/// B7 (#181): a background child that the tool leaves behind is ended before the run returns, on the success path and at
/// the deadline. The child is a `cat` of a blocked FIFO whose stdout is the writer of a pipe that the test watches: the
/// tool inherits that writer (its close-on-exec flag is cleared), and the test drops its own copy, so the end of file of
/// the pipe proves that the `cat` is gone. With the tool's leader owned alone (`OwnedChild::spawn`), the `cat` stays, and
/// the end of file never comes.
#[test]
fn a_child_that_the_tool_leaves_is_ended_when_the_run_returns() {
    use std::os::fd::AsRawFd;
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    // "done": the tool exits at once. "late": the tool's leader blocks on the FIFO too, past its deadline.
    for (name, rest, deadline) in [
        ("done", String::new(), Deadline::cleanup()),
        (
            "late",
            format!("exec {}", blocker.shell()),
            Deadline::after(Duration::ZERO),
        ),
    ] {
        let (watched, writer) = std::io::pipe().unwrap();
        rustix::io::fcntl_setfd(&writer, rustix::io::FdFlags::empty()).unwrap();
        let script = format!(
            "{} >&{} 2>/dev/null & {rest}",
            blocker.shell(),
            writer.as_raw_fd()
        );
        let run = run_to_completion(Command::new("/bin/sh").args(["-c", &script]), deadline);
        drop(writer);
        match name {
            "done" => assert_eq!(run.unwrap().status.code(), Some(0)),
            _ => assert_eq!(run.unwrap_err().kind(), std::io::ErrorKind::TimedOut),
        }
        eof(watched);
    }
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

/// The owner hands out the child's stdin: what the test writes there comes back through the child.
#[test]
fn a_child_reads_what_the_test_writes_to_its_stdin() {
    use std::io::Write;
    let mut child = OwnedChild::spawn(
        Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()),
    )
    .unwrap();
    let mut input = child.take_stdin().unwrap();
    input.write_all(b"echo\n").unwrap();
    drop(input);
    let (rest, line) = first_line(child.take_stdout().unwrap());
    assert_eq!(line, "echo\n");
    eof(rest.into_inner());
    assert_eq!(child.status().code(), Some(0));
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

/// G1 (#177): when the cleanup cannot make its reserve, its last kill (`signal_own_group`) ends every member of its group,
/// itself included. The member is a `cat` of the test's stdin, which the test holds open: it can end only by a signal. So
/// the end of file of the stderr pipe, which only the helper and its member hold, proves that both ended, and the helper's
/// `KILL` status proves that the kill ended it before any drop of the member's owner could run. A refused kill lets the
/// helper report, end its member through the owner's drop, and exit 0: the status check fails.
#[test]
fn an_unreserved_cleanup_ends_every_member_by_its_last_kill() {
    let dir = tempfile::tempdir().unwrap();
    let mut parent = OwnedChild::spawn_group(
        helper("helper_unreserved_cleanup", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .unwrap();
    let _held = parent.take_stdin().unwrap();
    let (rest, line) = first_line(parent.take_stderr().unwrap());
    assert_eq!(line, "member started\n");
    eof(rest.into_inner());
    assert_eq!(parent.status().signal(), Some(KILL));
}

#[test]
fn helper_unreserved_cleanup() {
    use std::io::Write;
    if helper_dir().is_none() {
        return;
    }
    // This process leads its own group (`spawn_group`). The member copies the test's stdin to the test's stderr pipe. If the
    // last kill does not end this process, the owner's drop ends the member when the helper returns.
    let _member = OwnedChild::spawn(Command::new("/bin/cat").stdout(stderr())).unwrap();
    writeln!(std::io::stderr(), "member started").unwrap();
    let group = rustix::process::getpgrp();
    // The kill below ends this process's group: it must be the helper's own, never the test runner's.
    assert_eq!(
        group,
        rustix::process::getpid(),
        "the helper leads its own group"
    );
    let ended =
        botster_test_process::rounds::end_group_reserved(group, Deadline::cleanup(), || {
            Err(std::io::Error::other("no reserve"))
        });
    // Reached only when the last kill did not end this process.
    writeln!(std::io::stderr(), "survived: {ended:?}").unwrap();
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
        start_time(pid(leader).unwrap()).unwrap(),
        Some(report.leader.start_time)
    );
    assert_eq!(
        start_time(pid(report.anchor.pid).unwrap()).unwrap(),
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

/// Other crates build their guard with `Guard::new`, which runs the anchor that `cargo xtask prebuild-worker` installs in
/// `target/candidate`; the slow tier runs after that step. The installed anchor holds a wrapped program's group and reports
/// it, as the anchor of this crate does.
#[test]
fn the_prebuilt_anchor_of_guard_new_holds_the_group_of_a_wrapped_program() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut guard = Guard::new(dir.path()).unwrap();
    let mut production = start(
        &guard,
        dir.path(),
        Path::new("/bin/sh"),
        &["-c", &blocker.shell()],
    );
    let report = one_anchor(&mut guard);
    let leader = production.id();
    assert_eq!((report.leader.pid, report.group), (leader, leader));
    blocker.send(b"up\n").unwrap();
    let (rest, line) = first_line(production.stdout.take().unwrap());
    assert_eq!(line, "up\n");
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
    // The anchor waits only while a member other than itself lives: the drop ends long before the grace would.
    let grace = Deadline::after(CLEANUP);
    drop(guard);
    assert!(!grace.expired(), "the anchor waited out its grace");
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
    assert_eq!(line, format!("moved {}\n", production.id()));
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
    eprintln!("moved {}", std::process::id());
    // It must outlive a TERM grace of CLEANUP (a_member_that_moves_after_term_is_refused_and_no_kill_is_sent), so its bound
    // is that grace and then the cleanup bound, as the guard's own outcome bound adds them.
    let _ = Bounded::new(std::io::stdin()).to_eof(Deadline::after(CLEANUP + CLEANUP));
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
        start_time(anchor).unwrap(),
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

/// #171 TP1: the status of an exited child is awaited within the deadline and read without a reap: a second check finds it
/// again, and the child's own reap gets it. Once the child is reaped, its pid names no child of this process: the check fails.
#[test]
fn the_status_of_an_exited_child_is_awaited_without_a_reap() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "exit 7"])
        .spawn()
        .unwrap();
    let id = pid(child.id()).unwrap();
    let status = await_status(id, Deadline::cleanup()).unwrap();
    assert_eq!(status.unwrap().code(), Some(7));
    assert_eq!(peek(id).unwrap().unwrap().code(), Some(7), "not reaped");
    assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(7));
    assert!(peek(id).is_err(), "reaped: no longer a child");
}

/// #171 TP4: an intermediate stage that does not end fails the start at the deadline, and on that path it is killed and
/// reaped. The end of file of a pipe that only the intermediate held proves that no orphan is left.
#[test]
fn a_stalled_intermediate_fails_the_start_and_leaves_no_orphan() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = Blocker::new(dir.path(), "block").unwrap();
    let (reader, writer) = std::io::pipe().unwrap();
    let error = start_anchor(
        blocker.command().stderr(writer),
        Deadline::after(Duration::ZERO),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "the intermediate stage did not end");
    eof(reader);
}

/// #171 TP4: the start needs an intermediate that ends with code 0 and the anchor's acknowledgement.
#[test]
fn the_start_needs_an_intermediate_that_ends_with_code_0_and_an_acknowledgement() {
    let start = |script: &str| {
        start_anchor(
            Command::new("/bin/sh").args(["-c", script]),
            Deadline::cleanup(),
        )
        .map_err(|error| error.to_string())
    };
    assert_eq!(
        start("exit 1"),
        Err("the intermediate stage failed: exit status: 1".into())
    );
    assert_eq!(
        start("exit 0"),
        Err("the anchor ended before it held the group".into())
    );
    assert_eq!(start("/bin/echo ready"), Ok(()));
}

/// #171 TP4: when the test dies before any anchor registered with its guard, no process of the wrapped program is left: the
/// wrapper fails to connect or to start its anchor, or the anchor sees its connection end and ends the group. The end of
/// file of a pipe that the wrapped program holds proves it, whichever of these happened.
#[test]
fn a_test_that_dies_before_its_anchor_registers_leaves_no_orphan() {
    let dir = tempfile::tempdir().unwrap();
    let _blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut parent = OwnedChild::spawn_group(
        helper("helper_unregistered_parent", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .unwrap();
    let (rest, line) = first_line(parent.take_stderr().unwrap());
    assert_eq!(line, "started\n");
    parent.kill().unwrap();
    eof(rest.into_inner());
    assert_eq!(parent.status().signal(), Some(KILL));
}

#[test]
fn helper_unregistered_parent() {
    let Some(dir) = helper_dir() else { return };
    let guard = guard(&dir);
    let script = dir.join("wrapped.sh");
    guard
        .wrapper(
            &script,
            Path::new("/bin/cat"),
            &[dir.join("block").to_str().unwrap()],
        )
        .unwrap();
    // Production's child holds this process's stderr, the test's pipe, in its own group. No anchor is awaited.
    let _production = OwnedChild::spawn(
        Command::new(&script)
            .stdin(Stdio::null())
            .stdout(stderr())
            .process_group(0),
    )
    .unwrap();
    eprintln!("started");
    // The test kills this process while it waits here: neither the guard's drop nor any other cleanup runs.
    let _ = Bounded::new(std::io::stdin()).to_eof(Deadline::cleanup());
    std::mem::forget(guard);
}

/// #171 TP3: production hands its wrapped program a pipe writer with no close-on-exec. Once production and the program have
/// closed it, the reader gets its end of file while the anchor still lives: no helper stage holds the writer.
#[test]
fn a_writer_that_production_hands_the_program_reaches_no_helper_stage() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = guard(dir.path());
    let script = dir.path().join("wrapped.sh");
    guard
        .wrapper(&script, Path::new("/bin/sh"), &["-c", "exit 4"])
        .unwrap();
    // `pipe` sets no close-on-exec: the writer is inherited by every child that production starts.
    let (reader, writer) = rustix::pipe::pipe().unwrap();
    let mut production = Command::new(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let report = one_anchor(&mut guard);
    drop(writer);
    assert_eq!(production_reap(&mut production).code(), Some(4));
    eof(std::fs::File::from(reader));
    assert_eq!(
        start_time(pid(report.anchor.pid).unwrap()).unwrap(),
        Some(report.anchor.start_time),
        "the anchor still lives"
    );
    drop(guard);
}

/// #171 TP5: a test that panics drops its guard while the panic unwinds. The guard still ends production's group, and
/// production still reaps its own child.
#[test]
fn a_guard_dropped_by_a_test_panic_ends_the_group_and_production_still_reaps_its_child() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let mut production = None;
    let mut output = None;
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = guard(dir.path());
        let child = production.insert(start(
            &guard,
            dir.path(),
            Path::new("/bin/cat"),
            &[blocker.path().to_str().unwrap()],
        ));
        one_anchor(&mut guard);
        blocker.send(b"up\n").unwrap();
        let (rest, line) = first_line(child.stdout.take().unwrap());
        assert_eq!(line, "up\n");
        output = Some(rest);
        panic!("the test fails");
    }))
    .expect_err("the test panicked");
    assert_eq!(panicked.downcast_ref::<&str>(), Some(&"the test fails"));
    assert_eq!(
        production_reap(production.as_mut().unwrap()).signal(),
        Some(KILL)
    );
    eof(output.unwrap().into_inner());
}

/// #171 TP6: a member that moves to its own group when `TERM` reaches it is not left live by a `KILL` of the old group. After
/// the grace the anchor verifies the members again, refuses, and sends no `KILL`: the leader, which ignores `TERM`, later
/// ends by itself with code 0. The leader keeps a member live through the grace, so this test waits the whole grace.
#[test]
fn a_member_that_moves_after_term_is_refused_and_no_kill_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let mut blocker = Blocker::new(dir.path(), "block").unwrap();
    let member = dir.path().join("member.sh");
    std::fs::write(
        &member,
        format!(
            "exe={}\ntrap 'exec \"$exe\" --exact helper_moves_its_group --nocapture' TERM\n/bin/echo member >&2\n{} & wait\n",
            quoted(&std::env::current_exe().unwrap()),
            blocker.shell()
        ),
    )
    .unwrap();
    // The member reads the leader's stdin, the test's pipe, so it lives until the test closes that pipe. A shell gives an
    // asynchronous command /dev/null as stdin before its own redirections (dash), so the pipe goes through descriptor 3.
    let leader = format!(
        "exec 3<&0; /bin/sh {} 0<&3 3<&- & trap '' TERM; /bin/echo leader >&2; wait",
        quoted(&member)
    );
    let mut guard = guard(dir.path()).grace(CLEANUP);
    let script = dir.path().join("wrapped.sh");
    guard
        .wrapper(&script, Path::new("/bin/sh"), &["-c", &leader])
        .unwrap();
    let mut production = OwnedChild::spawn(
        Command::new(&script)
            .env(HELPER, dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .process_group(0),
    )
    .unwrap();
    let report = one_anchor(&mut guard);
    let (mut rest, first) = first_line(production.take_stderr().unwrap());
    let second = rest.line(Deadline::cleanup()).unwrap().unwrap();
    let mut ready = [first, second];
    ready.sort();
    assert_eq!(ready, ["leader\n", "member\n"]);
    let message = drop_failure(guard);
    let moved = rest.line(Deadline::cleanup()).unwrap().unwrap();
    let mover: u32 = moved
        .strip_prefix("moved ")
        .and_then(|rest| rest.trim().parse().ok())
        .unwrap_or_else(|| panic!("{moved}"));
    assert_eq!(
        message,
        format!(
            "the group guard's cleanup failed: refused: the member {mover} moved from group {0} to group {mover}",
            report.group
        )
    );
    // No KILL reached the old group: the leader ends by itself once the member ends at the end of its stdin.
    drop(production.take_stdin());
    assert_eq!(production.status().code(), Some(0));
    // The member's `cat` can outlive the TERM (it can come while the member's shell forks it, before `cat` runs), and no
    // KILL reached the old group: the release of its FIFO ends it.
    blocker.release();
    eof(rest.into_inner());
}

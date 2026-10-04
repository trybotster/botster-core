//! A real-process test of the group guard (testing rule 10). It starts real processes, so it lives in the slow tier.
//!
//! Clause: Core A5-4 (the real-process tier owns and ends its process groups).
#![cfg(feature = "slow")]

use botster_core_testkit::process_group::OwnedGroup;
use rustix::process::{test_kill_process, Pid};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

fn alive(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    test_kill_process(pid).is_ok()
}

/// The child blocks on the pipe owned by the test. The child exits when that pipe closes.
fn blocked_command(descendant: bool, wait: bool) -> Command {
    let script = if descendant {
        if wait {
            "exec 3<&0; /bin/cat <&3 & echo $!; wait"
        } else {
            "exec 3<&0; /bin/cat <&3 & echo $!"
        }
    } else {
        "exec /bin/cat"
    };
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    command
}

/// The guard ends the leader and a descendant of its group when it drops. The shell and its background `cat` both hold the write
/// end of the stdout pipe, so the read side reaches its end only when every process of the group is gone. The test waits on that
/// event, with no polling.
#[test]
fn dropping_the_guard_ends_the_whole_group() {
    let mut group = OwnedGroup::spawn(blocked_command(true, true)).unwrap();
    let mut stdout = group.take_stdout().unwrap();
    let mut line = String::new();
    let mut byte = [0u8; 1];
    while stdout.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        line.push(byte[0] as char);
    }
    let descendant: u32 = line.trim().parse().unwrap();
    let leader = group.pid();
    assert!(alive(leader) && alive(descendant));
    drop(group);
    // The end of the pipe: no process of the group holds it any more.
    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
    // The leader was reaped by the guard, so its pid is gone.
    assert!(!alive(leader));
}

/// A panic ends the group as well: the guard is dropped while the stack unwinds.
#[test]
fn a_panic_ends_the_group() {
    let group = OwnedGroup::spawn(blocked_command(false, false)).unwrap();
    let pid = group.pid();
    let result = std::panic::catch_unwind(move || {
        let _guard = group;
        panic!("a test failed");
    });
    assert!(result.is_err());
    assert!(!alive(pid));
}

/// Cleanup runs once. A repeat, and the drop after a cleanup, do nothing: the id is retired, so they cannot signal a group that
/// the OS gave to another process.
#[test]
fn cleanup_is_idempotent_and_retires_the_group_id() {
    let mut group = OwnedGroup::spawn(blocked_command(false, false)).unwrap();
    let pid = group.pid();
    assert!(group.is_active());
    assert!(!group.leader_exited().unwrap());
    group.kill();
    assert!(!group.is_active());
    assert!(!alive(pid), "the leader was reaped");
    group.kill();
    assert!(
        group.leader_exited().unwrap(),
        "after cleanup the leader is gone"
    );
    drop(group);
}

/// A leader that exits by itself stays unreaped until cleanup, so its group id is still held, and cleanup still ends the
/// descendants of the group. The test waits on the pipe that the descendant holds.
#[test]
fn a_leader_that_exited_still_has_its_group_ended() {
    let mut group = OwnedGroup::spawn(blocked_command(true, false)).unwrap();
    let mut stdout = group.take_stdout().unwrap();
    let mut line = String::new();
    let mut byte = [0u8; 1];
    while stdout.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        line.push(byte[0] as char);
    }
    let leader = group.pid();
    // The shell prints the pid and exits at once. Block on the event: the leader is a zombie that the guard has not reaped.
    group.wait_for_leader_exit().unwrap();
    assert!(group.leader_exited().unwrap());
    assert!(
        alive(leader),
        "an exited leader that nobody reaped keeps its pid"
    );
    group.kill();
    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).unwrap();
    assert!(!alive(leader));
}

/// The test's pipe supplies parent lifetime without a timer or CPU loop.
#[test]
fn closing_the_parent_pipe_ends_the_child() {
    let mut group = OwnedGroup::spawn(blocked_command(false, false)).unwrap();
    let stdin = group.take_stdin().unwrap();
    let mut stdout = group.take_stdout().unwrap();
    drop(stdin);
    group.wait_for_leader_exit().unwrap();
    assert!(group.leader_exited().unwrap());
    let mut bytes = Vec::new();
    stdout.read_to_end(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    let pid = group.pid();
    drop(group);
    assert!(!alive(pid));
}

/// The outer test owns the group. This guard also kills and reaps its direct child if the fixture panics.
struct FixtureChild(Child);

impl Drop for FixtureChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// This fixture exits without Rust cleanup only when the outer test starts it.
#[test]
fn parent_death_fixture() {
    if std::env::var_os("BOTSTER_P6_PARENT_DEATH_FIXTURE").is_none() {
        return;
    }
    // The child inherits the group owned by the outer test's OwnedGroup.
    let child = Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut child = FixtureChild(child);
    let _parent_pipe = child.0.stdin.take().unwrap();
    println!("fixture child {}", child.0.id());
    std::io::stdout().flush().unwrap();
    // This skips both guards' destructors. The OS must close the pipe when the parent exits.
    std::process::exit(0);
}

/// The child exits even when its parent skips cleanup. The outer guard still owns the whole group on panic.
#[test]
fn parent_exit_without_cleanup_ends_the_child() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "parent_death_fixture", "--nocapture"])
        .env("BOTSTER_P6_PARENT_DEATH_FIXTURE", "1")
        .stdout(Stdio::piped());
    let mut group = OwnedGroup::spawn(command).unwrap();
    let mut stdout = group.take_stdout().unwrap();
    group.wait_for_leader_exit().unwrap();
    assert!(group.leader_exited().unwrap());
    let mut bytes = Vec::new();
    // EOF requires the child to close its inherited stdout. The outer guard has not killed the group yet.
    stdout.read_to_end(&mut bytes).unwrap();
    let output = String::from_utf8(bytes).unwrap();
    assert!(output.contains("fixture child "));
    let pid = group.pid();
    drop(group);
    assert!(!alive(pid));
}

/// The standard streams of the leader are the caller's: `take_stdin` gives the writing end of its input and `take_stderr`
/// the reading end of its errors. The shell echoes its input line to its errors.
#[test]
fn the_leader_streams_are_taken_by_the_caller() {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "read line; echo \"$line\" >&2"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped());
    let mut group = OwnedGroup::spawn(command).unwrap();
    let mut stdin = group.take_stdin().expect("the input is piped");
    stdin.write_all(b"hello\n").unwrap();
    drop(stdin);
    let mut stderr = group.take_stderr().expect("the errors are piped");
    let mut said = String::new();
    // The end of the pipe comes when the shell exits.
    stderr.read_to_string(&mut said).unwrap();
    assert_eq!(said, "hello\n");
    assert!(group.take_stdin().is_none(), "taken once");
}

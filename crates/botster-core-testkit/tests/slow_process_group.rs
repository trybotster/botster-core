//! A real-process test of the group guard (testing rule 10). It starts real processes, so it lives in the slow tier.
//!
//! Clause: Core A5-4 (the real-process tier owns and ends its process groups).
#![cfg(feature = "slow")]

use botster_core_testkit::process_group::OwnedGroup;
use rustix::process::{test_kill_process, Pid};
use std::io::Read;
use std::process::{Command, Stdio};

fn alive(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    test_kill_process(pid).is_ok()
}

/// The guard ends the leader and a descendant of its group when it drops. The shell and its background `sleep` both hold the write
/// end of the stdout pipe, so the read side reaches its end only when every process of the group is gone. The test waits on that
/// event, with no polling.
#[test]
fn dropping_the_guard_ends_the_whole_group() {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "/bin/sleep 600 & echo $!; wait"])
        .stdout(Stdio::piped());
    let mut group = OwnedGroup::spawn(command).unwrap();
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
    let mut command = Command::new("/bin/sleep");
    command.arg("600");
    let group = OwnedGroup::spawn(command).unwrap();
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
    let mut command = Command::new("/bin/sleep");
    command.arg("600");
    let mut group = OwnedGroup::spawn(command).unwrap();
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
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "/bin/sleep 600 & echo $!"])
        .stdout(Stdio::piped());
    let mut group = OwnedGroup::spawn(command).unwrap();
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

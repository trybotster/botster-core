//! Positive controls with real processes (BUILD.md Testing rule 2: the slow tier; rule 10: nothing is left behind).
//!
//! They prove what the pure tests of `census` cannot: that real `ps` rows of real processes are attributed as the
//! rules say, on Linux and on Darwin.

use botster_test_support::census::{leftovers, ps_snapshot, update_owned};
use botster_test_support::process::Driver;
use botster_test_support::tempdir::TempRoot;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

fn ps_kill(pid: u32) {
    let _ = Command::new("kill")
        .args(["-s", "KILL", &pid.to_string()])
        .status();
}

/// Ends a pid on drop, so a failed assertion leaves nothing behind.
struct Reap(u32);
impl Drop for Reap {
    fn drop(&mut self) {
        ps_kill(self.0);
    }
}

// Test rule 10, xtask census: a descendant that calls `setsid` leaves the group of the run, and is still detected
// once its parent is gone and it is re-parented to pid 1. The tracker saw it while its parent was owned.
#[test]
fn a_child_that_calls_setsid_and_is_reparented_is_still_a_leftover() {
    let mut root = Command::new("sh")
        .args([
            "-c",
            // The root shell starts the escapee, which prints its pid after `setsid`, and then waits for its stdin to close.
            r#"perl -MPOSIX -e 'POSIX::setsid(); $|=1; print "$$\n"; exec "sleep", "300"' & read _; wait"#,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let root_pid = root.id();
    // The event: the escapee prints its pid, so it has called `setsid`.
    let mut line = String::new();
    BufReader::new(root.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let escapee: u32 = line.trim().parse().unwrap();
    let _reap = Reap(escapee);

    let mut owned = HashMap::new();
    update_owned(&mut owned, &ps_snapshot().unwrap(), root_pid, &[]);
    let row = owned
        .get(&escapee)
        .expect("seen while its parent was owned");
    assert_eq!(
        row.pgid, escapee,
        "setsid made it a group leader outside the run's groups"
    );

    // The root ends and the escapee is re-parented.
    drop(root.stdin.take());
    ps_kill(root_pid);
    root.wait().unwrap();
    update_owned(&mut owned, &ps_snapshot().unwrap(), root_pid, &[]);
    let left: Vec<u32> = leftovers(&owned).iter().map(|p| p.pid).collect();
    assert!(left.contains(&escapee), "{left:?}");
    assert_ne!(owned[&escapee].ppid, root_pid);
}

// Negative control of the same rule: an unrelated process that the tracker never saw as a descendant is not owned.
#[test]
fn a_process_that_was_never_a_descendant_is_not_owned() {
    let mut stranger = Command::new("sleep").arg("300").spawn().unwrap();
    let _reap = Reap(stranger.id());
    let mut owned = HashMap::new();
    // The run's root is some other pid.
    update_owned(&mut owned, &ps_snapshot().unwrap(), u32::MAX - 1, &[]);
    assert!(!owned.contains_key(&stranger.id()));
    stranger.kill().unwrap();
    stranger.wait().unwrap();
}

// Test rule 10: a child that exited and that nobody reaped is a leftover. Its parent here is this test process, the root.
#[test]
fn an_unreaped_child_is_a_leftover_zombie() {
    let mut child = Command::new("true").stdout(Stdio::piped()).spawn().unwrap();
    let pid = child.id();
    // Event: the pipe closes when the child exits. Its process-table row stays until `wait`.
    let mut sink = Vec::new();
    std::io::Read::read_to_end(child.stdout.as_mut().unwrap(), &mut sink).unwrap();
    let mut owned = HashMap::new();
    update_owned(&mut owned, &ps_snapshot().unwrap(), std::process::id(), &[]);
    let row = owned.get(&pid).expect("attributed by its parent");
    assert_eq!(row.ppid, std::process::id());
    assert!(leftovers(&owned).iter().any(|p| p.pid == pid));
    child.wait().unwrap();
    update_owned(&mut owned, &ps_snapshot().unwrap(), std::process::id(), &[]);
    assert!(!owned.contains_key(&pid), "reaped, so gone");
}

// The shutdown order: a child that ignores TERM gets KILL after the grace period, is reaped, and its output is complete.
#[test]
fn shutdown_kills_a_child_that_ignores_term_and_drains_its_pipes() {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        r#"trap '' TERM; echo oops >&2; echo ready; while :; do sleep 1; done"#,
    ]);
    let driver = Driver::spawn(command).unwrap();
    let pid = driver.pid();
    // The trap is set once the child says so; a TERM before that would end the shell.
    assert!(driver.wait_for_stdout(b"ready\n", Duration::from_secs(10)));
    let result = driver
        .shutdown(Duration::from_millis(300), Duration::from_secs(5))
        .unwrap();
    assert!(
        result.killed,
        "TERM was ignored, so KILL came after the grace period"
    );
    assert!(!result.pipes_held_open);
    assert!(!result.status.success());
    // Output that the child wrote before it died may be cut by the TERM race, but it is whole lines.
    assert!(String::from_utf8_lossy(&result.stdout)
        .lines()
        .all(|l| l == "ready"));
    // Reaped: no row of the pid remains.
    assert!(ps_snapshot().unwrap().iter().all(|p| p.pid != pid));
}

// A child that exits on TERM is not killed.
#[test]
fn shutdown_does_not_kill_a_child_that_exits_on_term() {
    let mut command = Command::new("sh");
    command.args(["-c", "echo hello; exec sleep 300"]);
    let driver = Driver::spawn(command).unwrap();
    assert!(driver.wait_for_stdout(b"hello\n", Duration::from_secs(10)));
    let result = driver
        .shutdown(Duration::from_secs(5), Duration::from_secs(5))
        .unwrap();
    assert!(!result.killed);
    assert_eq!(result.stdout, b"hello\n");
}

// Real sockets: the root is short enough for a Unix socket address.
#[test]
fn a_socket_binds_in_a_temp_root() {
    let root = TempRoot::new().unwrap();
    let path = root.socket_path("s.sock").unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    assert!(path.exists());
}

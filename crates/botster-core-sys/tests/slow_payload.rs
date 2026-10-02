//! The real payload edge (plan 2.3, `Program`), proved with real processes. Slow tier: every payload is a `Payload`, whose
//! drop kills its group and reaps it on every exit path, panics included.
//!
//! Clause: Core LC-4, Core LC-5, Core LC-6, Core EV-4, Core A2-1.
#![cfg(feature = "slow")]

use botster_core_edges::edges::ExitStatus;
use botster_core_sys::payload::{Payload, PayloadCommand, SpawnFailure};
use std::collections::BTreeMap;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

fn env() -> BTreeMap<String, String> {
    BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])
}

fn spawn(argv: &[&str], cwd: &str) -> Result<Payload, SpawnFailure> {
    let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    Payload::spawn(&PayloadCommand {
        argv: &argv,
        env: &env(),
        cwd,
        rows: 24,
        cols: 80,
    })
}

/// Reads until the output ends, waiting on the descriptor's readiness (never a sleep).
fn read_all(p: &Payload) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match p.read(&mut buf) {
            Ok(0) => return out,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let mut fds = [rustix::event::PollFd::from_borrowed_fd(
                    p.master(),
                    rustix::event::PollFlags::IN,
                )];
                // timer: deadline — the limit of a wait for output that a real process writes; not a contract value.
                let limit = rustix::event::Timespec {
                    tv_sec: 10,
                    tv_nsec: 0,
                };
                assert!(
                    rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0,
                    "no output"
                );
            }
            Err(e) => panic!("{e}"),
        }
    }
}

fn exit_of(p: &Payload) -> ExitStatus {
    let (tx, rx) = mpsc::channel();
    p.watch_exit(move |status| tx.send(status).unwrap())
        .unwrap();
    // timer: deadline — the limit of a wait for a real process's exit; not a contract value.
    rx.recv_timeout(Duration::from_secs(10)).expect("an exit")
}

/// LC-4: a missing directory and a missing program are typed failures.
#[test]
fn a_payload_that_cannot_start_is_a_typed_failure() {
    assert_eq!(
        spawn(&["/bin/sh"], "/no-such-dir-botster").unwrap_err(),
        SpawnFailure::CwdMissing
    );
    assert_eq!(
        spawn(&["/no-such-program-botster"], "/").unwrap_err(),
        SpawnFailure::Exec { errno: 2 }
    );
    assert_eq!(
        spawn(&[], "/").unwrap_err(),
        SpawnFailure::Exec { errno: 2 }
    );
}

/// A2-1, EV-4: the payload sees exactly the requested environment and a terminal; its output arrives on the master; its
/// exit code is reported while the leader is still unreaped.
#[test]
fn the_payload_runs_on_the_pty_with_the_exact_environment() {
    let p = spawn(
        &["/bin/sh", "-c", "test -t 0 && echo tty; env | sort; exit 3"],
        "/",
    )
    .unwrap();
    let out = String::from_utf8_lossy(&read_all(&p)).replace('\r', "");
    assert_eq!(exit_of(&p), ExitStatus::Code(3));
    assert!(out.starts_with("tty\n"), "{out}");
    assert!(out.contains("PATH=/usr/bin:/bin\n"), "{out}");
    assert!(!out.contains("HOME="), "nothing is inherited: {out}");
    // Unreaped: the group can still be signalled, and the reap completes at once.
    p.signal_group(9);
    p.reap();
}

/// LC-5, LC-6, EV-4: a signal to the group ends the payload and its child in the group; the exit carries the signal.
#[test]
fn a_group_signal_ends_the_leader_and_its_group() {
    let p = spawn(&["/bin/sh", "-c", "sleep 30 & echo up; wait"], "/").unwrap();
    let mut buf = [0u8; 64];
    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("up") {
        match p.read(&mut buf) {
            Ok(n) => seen.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let mut fds = [rustix::event::PollFd::from_borrowed_fd(
                    p.master(),
                    rustix::event::PollFlags::IN,
                )];
                // timer: deadline — the limit of a wait for output that a real process writes; not a contract value.
                let limit = rustix::event::Timespec {
                    tv_sec: 10,
                    tv_nsec: 0,
                };
                assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
            }
            Err(e) => panic!("{e}"),
        }
    }
    p.signal_group(15);
    assert_eq!(exit_of(&p), ExitStatus::Signal(15));
    // The `sleep` was in the group: the output ends because no process holds the PTY any more.
    read_all(&p);
    p.reap();
}

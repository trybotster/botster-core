//! The real payload edge (plan 2.3, `Program`), proved with real processes. Slow tier: every payload is a `Payload`, whose
//! test guard ends the group on every exit path. Production alone reaps the payload.
//!
//! Clause: Core LC-4, Core LC-5, Core LC-6, Core EV-4, Core A2-1.
#![cfg(feature = "slow")]

#[path = "common/payload_guard.rs"]
mod payload_guard;

#[path = "common/process_guard.rs"]
mod process_guard;

use botster_core_edges::edges::ExitStatus;
use botster_core_sys::payload::{Payload, PayloadCommand, SpawnFailure};
use payload_guard::PayloadGuard;
use std::collections::BTreeMap;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

fn env() -> BTreeMap<String, String> {
    BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])
}

struct GuardedPayload {
    payload: Option<Payload>,
    guard: Option<PayloadGuard>,
    _root: tempfile::TempDir,
}

impl std::fmt::Debug for GuardedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.payload.fmt(f)
    }
}

impl std::ops::Deref for GuardedPayload {
    type Target = Payload;
    fn deref(&self) -> &Payload {
        self.payload.as_ref().unwrap()
    }
}

impl GuardedPayload {
    fn reap(mut self) {
        self.payload.take().unwrap().reap();
    }
}

impl Drop for GuardedPayload {
    fn drop(&mut self) {
        // The test ends the group before production can block in its reaper.
        drop(self.guard.take());
    }
}

fn spawn(argv: &[&str], cwd: &str) -> Result<GuardedPayload, SpawnFailure> {
    let root = tempfile::Builder::new()
        .prefix("pg")
        .tempdir_in("/tmp")
        .unwrap();
    let guard = PayloadGuard::new(root.path());
    let mut argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    if let [shell, option, script] = argv.as_mut_slice() {
        if shell == "/bin/sh" && option == "-c" {
            *script = format!("{}{script}", guard.prefix());
        }
    }
    let payload = Payload::spawn(&PayloadCommand {
        argv: &argv,
        env: &env(),
        cwd,
        rows: 24,
        cols: 80,
    })?;
    Ok(GuardedPayload {
        payload: Some(payload),
        guard: Some(guard),
        _root: root,
    })
}

/// Reads until the output ends, waiting on the descriptor's readiness (never a sleep).
fn read_all(p: &Payload) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match p.read(&mut buf) {
            Ok(0) => return out,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                assert!(
                    out.len() <= 65_536,
                    "the bounded test program wrote too much output"
                );
            }
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
    assert!(rustix::fs::fcntl_getfl(p.master())
        .unwrap()
        .contains(rustix::fs::OFlags::NONBLOCK));
    assert!(p.pid() > 1);
    let description = format!("{p:?}");
    assert!(description.contains("Payload"));
    assert!(description.contains(&p.pid().to_string()));
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
            Ok(0) => panic!("the payload output ended before readiness"),
            Ok(n) => {
                seen.extend_from_slice(&buf[..n]);
                assert!(
                    seen.len() <= 65_536,
                    "the bounded test program wrote too much output"
                );
            }
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
    p.signal_group(9);
    p.reap();
}

/// A2-1 and plan 2.3: the PTY exposes pending program output and takes the program's input bytes.
#[test]
fn the_pty_counts_output_and_delivers_input_to_the_program() {
    let p = spawn(
        &[
            "/bin/sh",
            "-c",
            "stty -echo; printf ready; read answer; printf '%s' \"$answer\"",
        ],
        "/",
    )
    .unwrap();
    let mut fds = [rustix::event::PollFd::from_borrowed_fd(
        p.master(),
        rustix::event::PollFlags::IN,
    )];
    // timer: deadline — wait for the program's output before checking the pending byte count.
    let limit = rustix::event::Timespec {
        tv_sec: 10,
        tv_nsec: 0,
    };
    assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
    assert!(p.pending_output().unwrap() > 1);
    assert_eq!(p.write(b"input\n").unwrap(), 6);
    let bytes = read_all(&p);
    assert_eq!(bytes, b"readyinput");
    assert_eq!(exit_of(&p), ExitStatus::Code(0));
    p.signal_group(9);
    p.reap();
}

/// LC-5 and the payload ownership rule: dropping an unreaped payload retires its leader.
#[test]
fn dropping_the_payload_reaps_its_leader() {
    use std::os::unix::process::CommandExt;
    let root = tempfile::tempdir().unwrap();
    let group = process_guard::GroupGuard::new(root.path());
    let child = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("{}exec \"$@\"", group.prefix()), "observer"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "payload_reap_observer",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("BOTSTER_REAP_OBSERVER", "1")
        .process_group(0)
        .spawn()
        .unwrap();
    assert!(Observer(Some(child)).wait().unwrap().success());
}

/// The observer owns no other direct child. A reused payload PID cannot name another child of this observer.
#[test]
fn payload_reap_observer() {
    if std::env::var_os("BOTSTER_REAP_OBSERVER").is_none() {
        return;
    }
    let p = spawn(&["/bin/sh", "-c", "printf ready; exit 0"], "/").unwrap();
    let pid = rustix::process::Pid::from_raw(p.pid() as i32).unwrap();
    assert_eq!(read_all(&p), b"ready");
    assert_eq!(exit_of(&p), ExitStatus::Code(0));
    drop(p);
    // The isolated observer has no remaining child. This query does not reap and cannot wait for another child owner.
    assert!(matches!(
        rustix::process::waitid(
            rustix::process::WaitId::Pid(pid),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOWAIT
                | rustix::process::WaitIdOptions::NOHANG
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}

/// The test owns its observer independently of every mutated payload function.
struct Observer(Option<std::process::Child>);

impl Observer {
    fn wait(mut self) -> io::Result<std::process::ExitStatus> {
        let result = self.0.as_mut().unwrap().wait()?;
        // The observer was reaped. Retire its handle before Drop can signal it.
        self.0 = None;
        Ok(result)
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

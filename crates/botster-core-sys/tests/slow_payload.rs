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

/// The limit of a test's cleanup: the guard's own cleanup and the production drop or reap. Not a contract value.
const CLEANUP: Duration = Duration::from_secs(10);

struct GuardedPayload {
    payload: Option<Payload>,
    guard: Option<PayloadGuard>,
    /// The leader's pid, for the process table of a cleanup that does not finish.
    pid: u32,
    /// Where the drop reports whether its cleanup finished, for a test that drops the payload while it panics.
    cleaned: Option<mpsc::Sender<bool>>,
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
        let payload = self.payload.take().unwrap();
        finish_within(self.pid, move || payload.reap());
    }

    /// The report of this payload's cleanup: `true` when the guard and the production drop finished within the limit.
    fn cleanup_report(&mut self) -> mpsc::Receiver<bool> {
        let (report, reported) = mpsc::channel();
        self.cleaned = Some(report);
        reported
    }
}

impl Drop for GuardedPayload {
    fn drop(&mut self) {
        // The test ends the group before production can block in its reaper; both steps run within the limit.
        let guard = self.guard.take();
        let payload = self.payload.take();
        let finished = finish_within(self.pid, move || {
            drop(guard);
            drop(payload);
        });
        if let Some(report) = self.cleaned.take() {
            let _ = report.send(finished);
        }
    }
}

/// Runs the end of a payload (the guard's cleanup, then its production drop or reap) with the cleanup limit, so a leader
/// that never becomes waitable fails the test with the process table instead of holding the job until its deadline
/// (BUILD.md testing rule 5). Returns whether it finished. A failure panics, except while the test already panics (a second
/// panic would abort before the report); such a test reads the result through `cleanup_report`.
fn finish_within(payload_pid: u32, end: impl FnOnce() + Send + 'static) -> bool {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        end();
        let _ = done.send(());
    });
    // timer: deadline — the independent guard and production reaper must finish.
    if finished.recv_timeout(CLEANUP).is_ok() {
        return true;
    }
    cleanup_state(payload_pid);
    if !std::thread::panicking() {
        panic!("the payload's cleanup did not finish");
    }
    false
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
        pid: payload.pid(),
        payload: Some(payload),
        guard: Some(guard),
        cleaned: None,
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
    rx.recv_timeout(Duration::from_secs(10))
        .expect("an exit")
        .expect("the watch waited for the leader")
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

/// The program writes its marker only after its PTY output is queued.
fn payload_waiting_for_input() -> (GuardedPayload, tempfile::TempDir) {
    use std::io::Read;
    use std::os::fd::AsFd;

    let root = tempfile::Builder::new()
        .prefix("queued")
        .tempdir_in("/tmp")
        .unwrap();
    let ready = root.path().join("ready");
    assert!(std::process::Command::new("/usr/bin/mkfifo")
        .arg(&ready)
        .status()
        .unwrap()
        .success());
    let mut reader = std::fs::File::from(
        rustix::fs::open(
            &ready,
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    let script = format!(
        "stty -echo; printf ready; /bin/echo queued > '{}'; read answer; printf '%s' \"$answer\"",
        ready.display()
    );
    let p = spawn(&["/bin/sh", "-c", &script], "/").unwrap();
    let mut fds = [rustix::event::PollFd::from_borrowed_fd(
        reader.as_fd(),
        rustix::event::PollFlags::IN,
    )];
    // timer: deadline — bounds the wait for the program's FIFO marker.
    let limit = rustix::event::Timespec {
        tv_sec: 10,
        tv_nsec: 0,
    };
    assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
    assert!(fds[0].revents().contains(rustix::event::PollFlags::IN));
    let mut marker = [0; 64];
    let count = reader.read(&mut marker).unwrap();
    assert_eq!(&marker[..count], b"queued\n");
    (p, root)
}

/// Reports ownership and group state without signaling or reaping any process.
fn cleanup_state(payload_pid: u32) {
    let test_pid = std::process::id();
    eprintln!("test pid={test_pid}; payload pid recorded before cleanup={payload_pid}");
    eprintln!("the test guard owns control; its thread owns the anchor socket");
    eprintln!("production owns the payload Child and its reap");
    match std::process::Command::new("/bin/ps")
        .args(["-axo", "pid,ppid,pgid,state,command"])
        .output()
    {
        Ok(output) if output.status.success() => {
            eprintln!("PID PPID PGID STATE COMMAND");
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let fields: Vec<_> = line.split_whitespace().collect();
                if fields.len() < 4 {
                    continue;
                }
                let ids: Vec<_> = fields[..3]
                    .iter()
                    .map(|value| value.parse::<u32>().ok())
                    .collect();
                if ids[0] == Some(test_pid)
                    || ids[0] == Some(payload_pid)
                    || ids[1] == Some(test_pid)
                    || ids[1] == Some(payload_pid)
                    || ids[2] == Some(payload_pid)
                {
                    eprintln!("{line}");
                }
            }
        }
        result => eprintln!("process-state query failed: {result:?}"),
    }
}

/// A2-1 and plan 2.3: the PTY counts queued program output and takes program input. The count is taken once the master
/// is readable: on Linux, output that the terminal has not moved to its read buffer yet is not counted (A31), and the
/// master is readable only after the move.
#[test]
fn the_pty_counts_output_and_delivers_input_to_the_program() {
    let (p, _root) = payload_waiting_for_input();
    let mut fds = [rustix::event::PollFd::from_borrowed_fd(
        p.master(),
        rustix::event::PollFlags::IN,
    )];
    // timer: deadline — the program wrote its output before its marker; bounds the wait for the PTY to show it.
    let limit = rustix::event::Timespec {
        tv_sec: 10,
        tv_nsec: 0,
    };
    assert_eq!(rustix::event::poll(&mut fds, Some(&limit)).unwrap(), 1);
    let pending = p.pending_output().unwrap();
    assert_eq!(
        p.pending_output().unwrap(),
        pending,
        "the query consumes nothing"
    );
    // The count is what reads take now: read until no byte is left.
    let mut consumed = Vec::new();
    let mut buf = [0u8; 64];
    loop {
        match p.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => consumed.extend_from_slice(&buf[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("{error}"),
        }
    }
    assert_eq!(consumed, b"ready", "the program's output before its marker");
    assert_eq!(pending, consumed.len(), "the count is what the reads took");
    assert_eq!(
        p.pending_output().unwrap(),
        0,
        "a consumed queue counts nothing"
    );
    assert_eq!(p.write(b"input\n").unwrap(), 6);
    assert_eq!(read_all(&p), b"input");
    assert_eq!(exit_of(&p), ExitStatus::Code(0));
    p.signal_group(9);
    p.reap();
}

/// The independent guard ends a payload that waits for input when the test panics.
#[test]
fn a_panic_ends_the_payload_while_it_waits_for_input() {
    let (mut p, root) = payload_waiting_for_input();
    let cleaned = p.cleanup_report();
    // Every wait of the cleanup is bounded, so the unwind itself ends; its result is reported separately.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _payload = p;
        let _root = root;
        panic!("test cleanup while the payload waits for input");
    }));
    assert!(outcome.is_err());
    assert_eq!(
        cleaned.try_recv(),
        Ok(true),
        "the guard and the production drop finished during the unwind"
    );
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

/// The limit of the observer's run: its own bounded waits, one exit and one cleanup, each within `CLEANUP`.
const OBSERVER_RUN: Duration = Duration::from_secs(2 * CLEANUP.as_secs());

/// Whether the child `pid` ended within `limit`, observed without reaping it (`WNOWAIT`): the reap stays with its owner.
fn ended_within(pid: u32, limit: Duration) -> bool {
    use rustix::process::{waitid, Pid, WaitId, WaitIdOptions};
    let pid = Pid::from_raw(pid as i32).unwrap();
    let (done, ended) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            match waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
            ) {
                Err(rustix::io::Errno::INTR) | Ok(None) => continue,
                _ => break,
            }
        }
        let _ = done.send(());
    });
    // timer: deadline — bounds a wait on a test's own child; the caller names the limit.
    ended.recv_timeout(limit).is_ok()
}

impl Observer {
    fn wait(mut self) -> io::Result<std::process::ExitStatus> {
        let child = self.0.as_mut().unwrap();
        assert!(ended_within(child.id(), OBSERVER_RUN), "the observer ended");
        let result = child.wait()?;
        // The observer was reaped. Retire its handle before Drop can signal it.
        self.0 = None;
        Ok(result)
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            if ended_within(child.id(), CLEANUP) {
                let _ = child.wait();
            } else if std::thread::panicking() {
                eprintln!("the observer did not end after SIGKILL");
            } else {
                panic!("the observer did not end after SIGKILL");
            }
        }
    }
}

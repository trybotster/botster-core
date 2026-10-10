//! The test owns payload cleanup through a member of the payload's session.
//! The member ends its own group on socket EOF, with the rounds of `process_guard`, and reports the outcome to its guard.
//! Production alone reaps the payload.
//!
//! Two phases: an owner sends the cleanup request ([`PayloadGuard::release`], or a [`Release`] that it drops first)
//! before production's cleanup, and drops the guard after it. On macOS the member's rounds can wait in a tty drain until
//! production closes the PTY master, so the guard reads the member's report only after that.

use super::process_guard::cleanup::platform::{await_end, live_members};
use super::process_guard::cleanup::{end_group, end_members, Failure, CLEANUP};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub struct PayloadGuard {
    control: UnixStream,
    thread: Option<std::thread::JoinHandle<Outcome>>,
    socket: PathBuf,
    /// The member's cleanup limit.
    cleanup: std::time::Duration,
}

/// What the member's side of the guard came to.
struct Outcome {
    /// The payload's group, as the member registered it; `None` when no member registered.
    group: Option<rustix::process::Pid>,
    /// The member's report line; `None` when its stream ended (an end of file or a reset) without one.
    report: std::io::Result<Option<String>>,
}

impl Outcome {
    fn unregistered() -> Self {
        Self {
            group: None,
            report: Ok(None),
        }
    }
}

/// The cleanup request of a guard. Dropping it sends the request; an owner whose production must drop first holds one
/// ahead of production in its field order.
pub struct Release {
    control: UnixStream,
    socket: PathBuf,
}

impl Release {
    fn send(&self) {
        let _ = self.control.shutdown(std::net::Shutdown::Write);
        // Release either accept if the payload never reached registration.
        for _ in 0..2 {
            if let Ok(stream) = UnixStream::connect(&self.socket) {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}

impl Drop for Release {
    fn drop(&mut self) {
        self.send();
    }
}

fn helper(name: &str) -> String {
    let module = module_path!().split_once("::").unwrap().1;
    format!("{module}::{name}")
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// The member's registration: its stream, a reader over the same stream, and its group.
struct Member {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    group: rustix::process::Pid,
}

fn failed(error: std::io::Error) -> Outcome {
    Outcome {
        group: None,
        report: Err(error),
    }
}

/// Registers the member and the ready helper, waits for the cleanup request, and reads the member's report.
///
/// A connection that ends before its tag is the release of a waiting accept (the request came before registration);
/// it ends the registration. Every other failure is kept: an accept or read error, an unknown tag, or a group that is not
/// a valid id (no readiness is sent then). A ready helper with no member registered no payload: the guard sends readiness
/// only when both are registered, so the ready helper exits 1 and the payload's shell exits before its command. A group
/// signal that comes before the member's registration (a stop at once after the start) ends the member that way.
/// A member with no ready helper (the payload ended before it) gets the cleanup request at once and reports as usual.
fn serve(listener: UnixListener, mut receiver: UnixStream) -> Outcome {
    let mut member: Option<Member> = None;
    let mut ready = None;
    while member.is_none() || ready.is_none() {
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) => return failed(error),
        };
        let mut tag = [0];
        match stream.read_exact(&mut tag) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return failed(error),
        }
        match tag[0] {
            1 if member.is_none() => {
                let mut reader = match stream.try_clone() {
                    Ok(clone) => BufReader::new(clone),
                    Err(error) => return failed(error),
                };
                let mut line = String::new();
                if let Err(error) = reader.read_line(&mut line) {
                    return failed(error);
                }
                let Some(group) = line
                    .trim()
                    .parse()
                    .ok()
                    .and_then(rustix::process::Pid::from_raw)
                else {
                    return failed(std::io::Error::other(format!(
                        "the member registered an invalid group {line:?}"
                    )));
                };
                member = Some(Member {
                    stream,
                    reader,
                    group,
                });
            }
            2 if ready.is_none() => ready = Some(stream),
            other => {
                return failed(std::io::Error::other(format!(
                    "an unexpected registration tag {other}"
                )))
            }
        }
    }
    // With no member, no readiness is sent: the ready helper reads the end of its stream and the payload never starts.
    let Some(mut member) = member else {
        return match ready {
            None => Outcome::unregistered(),
            Some(_) => failed(std::io::Error::other(
                "the payload became ready without its member",
            )),
        };
    };
    if let Some(mut ready) = ready {
        let _ = member.stream.write_all(&[1]);
        let _ = ready.write_all(&[1]);
        let mut rest = Vec::new();
        let _ = receiver.read_to_end(&mut rest);
    }
    let _ = member.stream.shutdown(std::net::Shutdown::Write);
    // The cleanup request is sent. The member reports when its rounds end.
    let mut line = String::new();
    let report = match member.reader.read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(line.trim_end().to_string())),
        // A member that ended with the readiness byte unread (production's group kill came first) resets the connection:
        // Linux then reports ECONNRESET in place of the end of file, after any data that the member sent. So a reset is an
        // end of file.
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
            Ok((!line.is_empty()).then(|| line.trim_end().to_string()))
        }
        Err(error) => Err(error),
    };
    Outcome {
        group: Some(member.group),
        report,
    }
}

impl PayloadGuard {
    pub fn new(root: &Path) -> Self {
        Self::with_cleanup(root, CLEANUP)
    }

    /// A guard whose member ends the group within `cleanup` (the failure test gives it no time).
    pub fn with_cleanup(root: &Path, cleanup: std::time::Duration) -> Self {
        let socket = root.join("payload-guard.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let (control, receiver) = UnixStream::pair().unwrap();
        let thread = std::thread::spawn(move || serve(listener, receiver));
        Self {
            control,
            thread: Some(thread),
            socket,
            cleanup,
        }
    }

    /// The cleanup request, as a value that sends it when it is dropped.
    pub fn release_handle(&self) -> Release {
        Release {
            control: self.control.try_clone().unwrap(),
            socket: self.socket.clone(),
        }
    }

    /// Sends the cleanup request to the member, and returns at once. The owner calls it before production's cleanup.
    pub fn release(&mut self) {
        self.release_handle().send();
    }

    /// The shell starts the member in its session and waits for registration.
    pub fn prefix(&self) -> String {
        let exe = quoted(&std::env::current_exe().unwrap());
        let socket = quoted(&self.socket);
        let cleanup = self.cleanup.as_millis();
        format!(
            "BOTSTER_PAYLOAD_GUARD={socket} BOTSTER_PAYLOAD_GROUP_PID=$$ BOTSTER_PAYLOAD_GUARD_CLEANUP_MS={cleanup} {exe} --exact {} --nocapture </dev/null >/dev/null 2>/dev/null &\nBOTSTER_PAYLOAD_GUARD={socket} {exe} --exact {} --nocapture </dev/null >/dev/null 2>/dev/null || exit 1\n",
            helper("payload_anchor"),
            helper("payload_ready"),
        )
    }
}

impl Drop for PayloadGuard {
    /// Reads the member's report, and fails the test unless the group is proved empty (or reports the failure when the
    /// test already panics): an `ok` report; or no report and a group whose last live members end within the cleanup
    /// limit, which means that production's group kill ended the member with its group. A member that never registered
    /// owned nothing.
    fn drop(&mut self) {
        self.release();
        let Some(thread) = self.thread.take() else {
            return;
        };
        let failure = match thread.join() {
            Err(_) => Some("the guard's thread panicked".to_string()),
            Ok(Outcome {
                report: Err(error), ..
            }) => Some(format!("the member's report could not be read: {error}")),
            Ok(Outcome {
                report: Ok(Some(report)),
                ..
            }) => match report.strip_prefix("fail: ") {
                _ if report == "ok" => None,
                Some(why) => Some(why.to_string()),
                None => Some(format!("a malformed report: {report:?}")),
            },
            Ok(Outcome {
                group: None,
                report: Ok(None),
            }) => None,
            Ok(Outcome {
                group: Some(group),
                report: Ok(None),
            }) => await_group_end(group, CLEANUP)
                .err()
                .map(|why| format!("the member ended without a report: {why}")),
        };
        if let Some(why) = failure {
            let report = format!("the payload guard's cleanup failed: {why}");
            if std::thread::panicking() {
                eprintln!("{report}");
            } else {
                panic!("{report}");
            }
        }
    }
}

/// Waits until `group` has no live member, within `cleanup`, without a signal: the caller holds no reservation of the
/// group, so it must not signal it. It is for a group that production has killed: the rounds of `end_members` with no
/// kill, so a member that is still ending is awaited, never read as left.
///
/// # Errors
/// The members still live at the deadline, or why they could not be listed or awaited.
fn await_group_end(
    group: rustix::process::Pid,
    cleanup: std::time::Duration,
) -> Result<(), String> {
    // timer: deadline — bounds the wait for a killed group's last members.
    let deadline = std::time::Instant::now() + cleanup;
    end_members(
        || Ok(()),
        || live_members(group),
        |member| await_end(member.pid, deadline),
        || std::time::Instant::now() >= deadline,
    )
    .map_err(|failure| match failure {
        Failure::Left(members) => {
            let left: Vec<String> = members.iter().map(ToString::to_string).collect();
            format!("members left after {cleanup:?}: {}", left.join(", "))
        }
        Failure::Error(error) => format!("the members could not be listed or awaited: {error}"),
    })
}

/// This member cannot leave a stale group id: it signals its current group.
#[test]
fn payload_anchor() {
    let Some(socket) = std::env::var_os("BOTSTER_PAYLOAD_GUARD") else {
        return;
    };
    // A shell can give its background command a separate process group.
    // Join the leader's group before the shell can start its body.
    let group = rustix::process::Pid::from_raw(
        std::env::var("BOTSTER_PAYLOAD_GROUP_PID")
            .unwrap()
            .parse()
            .unwrap(),
    )
    .unwrap();
    rustix::process::setpgid(None, Some(group)).unwrap();
    // A graceful group signal or leader exit must not remove test ownership.
    for signal in [signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM] {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        signal_hook::flag::register(signal, flag).unwrap();
    }
    let cleanup = std::env::var("BOTSTER_PAYLOAD_GUARD_CLEANUP_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(CLEANUP, std::time::Duration::from_millis);
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return;
    };
    let _ = stream.write_all(&[1]);
    let _ = writeln!(stream, "{}", group.as_raw_nonzero());
    let mut ready = [0];
    if stream.read_exact(&mut ready).is_ok() && ready == [1] {
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
    }
    // Every member of its group, then it ends (the rounds of `process_guard`), and reports the outcome to its guard.
    match end_group(rustix::process::getpgrp(), cleanup) {
        Ok(()) => {
            let _ = writeln!(stream, "ok");
        }
        Err(report) => {
            let _ = writeln!(stream, "fail: {}", report.replace('\n', " "));
            std::process::exit(1);
        }
    }
}

#[test]
fn payload_ready() {
    let Some(socket) = std::env::var_os("BOTSTER_PAYLOAD_GUARD") else {
        return;
    };
    let ready = (|| -> std::io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        stream.write_all(&[2])?;
        let mut ready = [0];
        stream.read_exact(&mut ready)?;
        if ready != [1] {
            return Err(std::io::Error::other("the payload has no guard"));
        }
        Ok(())
    })();
    if ready.is_err() {
        std::process::exit(1);
    }
}

/// A payload cleanup that cannot finish fails the test through the real payload guard: with no time for its rounds, the
/// member reports the live member, and the guard's drop fails with that report. The member still ends, by the last kill.
#[test]
fn a_payload_cleanup_that_cannot_finish_fails_through_the_guard() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let never = super::process_guard::never_fifo(dir.path());
    let mut guard = PayloadGuard::with_cleanup(dir.path(), std::time::Duration::ZERO);
    // The payload blocks without CPU on a FIFO that nothing opens for writing. Its stdout stays the pipe and it writes
    // nothing, so it holds the pipe until it ends.
    let mut payload = super::process_guard::cleanup::Owned(
        std::process::Command::new("/bin/sh")
            .args([
                "-c",
                &format!(
                    "{}/bin/echo up; exec /bin/cat {}",
                    guard.prefix(),
                    quoted(&never)
                ),
            ])
            .stdout(std::process::Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let (pipe, line) = super::process_guard::first_line(payload.0.stdout.take().unwrap());
    assert_eq!(line, "up\n");
    guard.release();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
        .expect_err("the guard reports the cleanup failure");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("members left"), "{report}");
    super::process_guard::eof(pipe);
    payload.status();
}

/// A registration that cannot be trusted fails the guard, and the registrant is never told that the payload is ready: a
/// member that registers an invalid group.
#[test]
fn a_registration_that_cannot_be_trusted_fails_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = PayloadGuard::new(dir.path());
    let mut registrant = UnixStream::connect(&guard.socket).unwrap();
    // timer: deadline — bounds the read of the registrant's end (set while the stream is open; macOS refuses it once
    // the peer has closed).
    registrant.set_read_timeout(Some(CLEANUP)).unwrap();
    registrant.write_all(b"\x01not-a-pid\n").unwrap();
    guard.release();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
        .expect_err("the guard reports the failed registration");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("invalid group"), "{report}");
    let mut readiness = [0];
    assert_eq!(
        registrant.read(&mut readiness).unwrap(),
        0,
        "no readiness was sent"
    );
}

/// A member that ends with the readiness byte unread, as when production's group kill ends it first, is a member that
/// ended without a report: the guard awaits the end of its group, and does not fail on the reset of the connection
/// (Linux reports ECONNRESET to the guard's read).
#[test]
fn a_member_that_ends_with_the_readiness_unread_ended_without_a_report() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    // A group with no live member: the group of a child that this test started and reaped.
    let mut leader = super::process_guard::cleanup::Owned(
        std::process::Command::new("/usr/bin/true")
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let group = leader.0.id();
    leader.status();
    let mut guard = PayloadGuard::new(dir.path());
    let mut member = UnixStream::connect(&guard.socket).unwrap();
    member
        .write_all(format!("\x01{group}\n").as_bytes())
        .unwrap();
    let mut ready = UnixStream::connect(&guard.socket).unwrap();
    // timer: deadline — bounds the read of the readiness (set while the stream is open; macOS refuses it once the peer
    // has closed).
    ready.set_read_timeout(Some(CLEANUP)).unwrap();
    ready.write_all(&[2]).unwrap();
    // The guard writes the readiness to the member first, so it waits unread in the member's stream once the ready helper
    // has it.
    let mut readiness = [0];
    ready.read_exact(&mut readiness).unwrap();
    assert_eq!(readiness, [1]);
    drop(member);
    guard.release();
    drop(guard);
}

/// A ready helper whose member never registered, as when production's group signal ends the member before its
/// registration, is no payload: the guard never tells it that the payload is ready, and its drop passes.
#[test]
fn a_ready_helper_without_its_member_is_never_ready_and_owns_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = PayloadGuard::new(dir.path());
    let mut ready = UnixStream::connect(&guard.socket).unwrap();
    // timer: deadline — bounds the read of the readiness (set while the stream is open; macOS refuses it once the peer
    // has closed).
    ready.set_read_timeout(Some(CLEANUP)).unwrap();
    ready.write_all(&[2]).unwrap();
    guard.release();
    drop(guard);
    let mut readiness = [0];
    assert_eq!(
        ready.read(&mut readiness).unwrap(),
        0,
        "no readiness was sent"
    );
}

//! The test owns payload cleanup through a member of the payload's session.
//! The member ends its own group on socket EOF, with the rounds of `process_guard`, and reports the outcome to its guard.
//! Production alone reaps the payload.
//!
//! Two phases: an owner sends the cleanup request ([`PayloadGuard::release`], or a [`Release`] that it drops first)
//! before production's cleanup, and drops the guard after it. On macOS the member's rounds can wait in a tty drain until
//! production closes the PTY master, so the guard reads the member's report only after that.

use super::process_guard::cleanup::{end_group, platform::live_members, CLEANUP};
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
    /// The member's report line; `None` when its stream ended without one.
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

/// Registers the member and the ready helper, waits for the cleanup request, and reads the member's report.
fn serve(listener: UnixListener, mut receiver: UnixStream) -> Outcome {
    let mut anchor = None;
    let mut ready = None;
    for _ in 0..2 {
        let Ok((mut stream, _)) = listener.accept() else {
            return Outcome::unregistered();
        };
        let mut tag = [0];
        if stream.read_exact(&mut tag).is_err() {
            return Outcome::unregistered();
        }
        match tag[0] {
            1 => anchor = Some(stream),
            2 => ready = Some(stream),
            _ => return Outcome::unregistered(),
        }
    }
    let (Some(mut anchor), Some(mut ready)) = (anchor, ready) else {
        return Outcome::unregistered();
    };
    let mut reader = match anchor.try_clone() {
        Ok(stream) => BufReader::new(stream),
        Err(error) => {
            return Outcome {
                group: None,
                report: Err(error),
            }
        }
    };
    let mut group = String::new();
    let group = match reader.read_line(&mut group) {
        Ok(_) => group
            .trim()
            .parse()
            .ok()
            .and_then(rustix::process::Pid::from_raw),
        Err(error) => {
            return Outcome {
                group: None,
                report: Err(error),
            }
        }
    };
    let _ = anchor.write_all(&[1]);
    let _ = ready.write_all(&[1]);
    let mut rest = Vec::new();
    let _ = receiver.read_to_end(&mut rest);
    let _ = anchor.shutdown(std::net::Shutdown::Write);
    // The cleanup request is sent. The member reports when its rounds end.
    let mut line = String::new();
    let report = match reader.read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(line.trim_end().to_string())),
        Err(error) => Err(error),
    };
    Outcome { group, report }
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
    /// test already panics): an `ok` report; or no report and no live member left in the group, which means that
    /// production's group kill ended the member with its group. A member that never registered owned nothing.
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
            }) => match live_members(group) {
                Ok(left) if left.is_empty() => None,
                Ok(left) => {
                    let left: Vec<String> = left.iter().map(ToString::to_string).collect();
                    Some(format!(
                        "the member ended without a report, and members are left: {}",
                        left.join(", ")
                    ))
                }
                Err(error) => Some(format!(
                    "the member ended without a report, and its group cannot be listed: {error}"
                )),
            },
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
    let never = dir.path().join("never");
    assert!(std::process::Command::new("/usr/bin/mkfifo")
        .arg(&never)
        .status()
        .unwrap()
        .success());
    let mut guard = PayloadGuard::with_cleanup(dir.path(), std::time::Duration::ZERO);
    // The payload blocks without CPU on a FIFO that nothing opens for writing, and holds the pipe until it ends.
    let mut payload = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "{}/bin/echo up; exec /bin/cat {} >/dev/null",
                guard.prefix(),
                quoted(&never)
            ),
        ])
        .stdout(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let (pipe, line) = super::process_guard::first_line(payload.stdout.take().unwrap());
    assert_eq!(line, "up\n");
    guard.release();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
        .expect_err("the guard reports the cleanup failure");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("members left"), "{report}");
    super::process_guard::eof(pipe);
    payload.wait().unwrap();
}

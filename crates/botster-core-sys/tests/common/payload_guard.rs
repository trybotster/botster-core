//! The test owns payload cleanup through a member of the payload's session.
//! The member ends its own group on socket EOF, with the rounds of `process_guard`, and reports the outcome to its guard.
//! Production alone reaps the payload.
//!
//! An owner calls [`PayloadGuard::release`] before production's cleanup, and drops the guard after it: on macOS the
//! member's rounds can wait in a tty drain until production closes the PTY master, so the guard reads the member's report
//! only after that.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub struct PayloadGuard {
    control: UnixStream,
    /// The member's report: `ok`, `fail: <why>`, or nothing when the member ended with its group before it reported.
    thread: Option<std::thread::JoinHandle<String>>,
    socket: PathBuf,
    released: bool,
}

fn helper(name: &str) -> String {
    let module = module_path!().split_once("::").unwrap().1;
    format!("{module}::{name}")
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

impl PayloadGuard {
    pub fn new(root: &Path) -> Self {
        let socket = root.join("payload-guard.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let (control, mut receiver) = UnixStream::pair().unwrap();
        let thread = std::thread::spawn(move || {
            let mut anchor = None;
            let mut ready = None;
            for _ in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return String::new();
                };
                let mut tag = [0];
                if stream.read_exact(&mut tag).is_err() {
                    return String::new();
                }
                match tag[0] {
                    1 => anchor = Some(stream),
                    2 => ready = Some(stream),
                    _ => return String::new(),
                }
            }
            let (Some(mut anchor), Some(mut ready)) = (anchor, ready) else {
                return String::new();
            };
            let _ = anchor.write_all(&[1]);
            let _ = ready.write_all(&[1]);
            let mut rest = Vec::new();
            let _ = receiver.read_to_end(&mut rest);
            let _ = anchor.shutdown(std::net::Shutdown::Write);
            // The cleanup request is sent. The member reports when its rounds end.
            let mut report = String::new();
            let _ = BufReader::new(anchor).read_line(&mut report);
            report
        });
        Self {
            control,
            thread: Some(thread),
            socket,
            released: false,
        }
    }

    /// Sends the cleanup request to the member, and returns at once. The owner calls it before production's cleanup.
    pub fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let _ = self.control.shutdown(std::net::Shutdown::Write);
        // Release either accept if the payload never reached registration.
        for _ in 0..2 {
            if let Ok(stream) = UnixStream::connect(&self.socket) {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    /// The shell starts the member in its session and waits for registration.
    pub fn prefix(&self) -> String {
        let exe = quoted(&std::env::current_exe().unwrap());
        let socket = quoted(&self.socket);
        format!(
            "BOTSTER_PAYLOAD_GUARD={socket} BOTSTER_PAYLOAD_GROUP_PID=$$ {exe} --exact {} --nocapture </dev/null >/dev/null 2>/dev/null &\nBOTSTER_PAYLOAD_GUARD={socket} {exe} --exact {} --nocapture </dev/null >/dev/null 2>/dev/null || exit 1\n",
            helper("payload_anchor"),
            helper("payload_ready"),
        )
    }
}

impl Drop for PayloadGuard {
    /// Reads the member's report, and fails the test when the member could not end the group (or reports it when the
    /// test already panics). No report means that the member ended with its group, by production's group kill.
    fn drop(&mut self) {
        self.release();
        let Some(thread) = self.thread.take() else {
            return;
        };
        let report = thread.join().unwrap_or_default();
        if let Some(why) = report.trim().strip_prefix("fail: ") {
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
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return;
    };
    let _ = stream.write_all(&[1]);
    let mut ready = [0];
    if stream.read_exact(&mut ready).is_ok() && ready == [1] {
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
    }
    // Every member of its group, then it ends (the rounds of `process_guard`), and reports the outcome to its guard.
    match super::process_guard::end_group(rustix::process::getpgrp(), super::process_guard::CLEANUP)
    {
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

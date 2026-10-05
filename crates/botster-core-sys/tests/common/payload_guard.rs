//! The test owns payload cleanup through a member of the payload's session.
//! The member kills its own group on socket EOF. Production alone reaps the payload.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub struct PayloadGuard {
    control: UnixStream,
    thread: Option<std::thread::JoinHandle<()>>,
    socket: PathBuf,
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
                    return;
                };
                let mut tag = [0];
                if stream.read_exact(&mut tag).is_err() {
                    return;
                }
                match tag[0] {
                    1 => anchor = Some(stream),
                    2 => ready = Some(stream),
                    _ => return,
                }
            }
            let (Some(mut anchor), Some(mut ready)) = (anchor, ready) else {
                return;
            };
            let _ = anchor.write_all(&[1]);
            let _ = ready.write_all(&[1]);
            let mut rest = Vec::new();
            let _ = receiver.read_to_end(&mut rest);
            let _ = anchor.shutdown(std::net::Shutdown::Write);
            // Return after the cleanup request. The production owner can then close the PTY master.
            // macOS can hold an exiting anchor in tty drain until that master closes.
            // The anchor still owns its current group through its final signal.
        });
        Self {
            control,
            thread: Some(thread),
            socket,
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
    fn drop(&mut self) {
        let _ = self.control.shutdown(std::net::Shutdown::Write);
        // Release either accept if the payload never reached registration.
        for _ in 0..2 {
            if let Ok(stream) = UnixStream::connect(&self.socket) {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
        if let Some(thread) = self.thread.take() {
            bounded("the payload guard's cleanup", move || {
                let _ = thread.join();
            });
        }
    }
}

/// Runs one wait of the guard's cleanup with the cleanup deadline. A wait that does not finish fails the test, or is
/// reported when the test already panics (a second panic would abort before the report).
fn bounded(what: &str, wait: impl FnOnce() + Send + 'static) {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        wait();
        let _ = done.send(());
    });
    // timer: deadline — bounds a guard's cleanup, so a stuck process fails the test instead of the job.
    if finished
        .recv_timeout(std::time::Duration::from_secs(10))
        .is_err()
    {
        if std::thread::panicking() {
            eprintln!("{what} did not finish");
        } else {
            panic!("{what} did not finish");
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
    let group = rustix::process::getpgrp();
    let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
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

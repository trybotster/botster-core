//! Event gates between a test and its fixture child processes.
//!
//! A fixture child that waits for the test, or tells the test it reached a
//! point, uses a named pipe instead of polling a file: the child blocks in
//! `/bin/cat "$GATE" >/dev/null` until the test releases it, or writes into a
//! pipe the test is reading. A test that waits for a process to end uses the OS exit
//! event. Every wait is bounded by one deadline whose expiry fails the test.
//!
//! In fixture scripts, open a pipe in an external command, for example
//! `/bin/cat "$GATE" >/dev/null` or `/bin/echo ready > "$READY"`, never in a
//! shell builtin such as `read _ < "$GATE"` or `printf ready > "$READY"`. A
//! builtin's redirection opens the pipe in the shell itself, and a signal the
//! shell handles interrupts that blocking open, so the gate passes without
//! the release. An external command's redirection runs in the forked child.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::bounded_wait::wait_for;

static NEXT_FIFO: AtomicU64 = AtomicU64::new(0);

/// A named pipe in the temp directory, removed on drop.
#[derive(Debug)]
pub struct Fifo {
    path: PathBuf,
}

impl Fifo {
    /// Create a new named pipe whose name includes `label`.
    ///
    /// # Panics
    ///
    /// Panics when `mkfifo` fails.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "botster-fifo-{label}-{}-{nanos}-{}",
            std::process::id(),
            NEXT_FIFO.fetch_add(1, Ordering::SeqCst)
        ));
        let made = Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("run mkfifo");
        assert!(made.success(), "mkfifo {}", path.display());
        Self { path }
    }

    /// The pipe's path, for the child's environment or script.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Release a child blocked in `/bin/cat fifo` by writing one line and
    /// closing the pipe.
    ///
    /// Opening the write end blocks until the child opens the read end, so
    /// the release happens exactly when the child is waiting for it. A
    /// signal can make the child's shell drop the pipe after opening it and
    /// open it again; the write then fails with a broken pipe, and the
    /// release opens the pipe again and blocks for the next reader.
    ///
    /// # Panics
    ///
    /// Panics when the child does not read the release within `bound`.
    pub fn release(&self, bound: Duration) {
        let path = self.path.clone();
        run_bounded("fifo release", bound, move || loop {
            let mut writer = fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .expect("open fifo for writing");
            match writer.write_all(b"go\n") {
                Ok(()) => return,
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
                Err(error) => panic!("write fifo release: {error}"),
            }
        });
    }

    /// Wait until the child writes into the pipe and closes it; return what
    /// it wrote.
    ///
    /// # Panics
    ///
    /// Panics when the child does not write and close within `bound`.
    #[must_use]
    pub fn read_signal(&self, bound: Duration) -> Vec<u8> {
        let path = self.path.clone();
        run_bounded("fifo signal", bound, move || {
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .expect("open fifo for reading")
                .read_to_end(&mut bytes)
                .expect("read fifo signal");
            bytes
        })
    }
}

impl Drop for Fifo {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Run a blocking step on its own thread and wait at most `bound` for it.
fn run_bounded<T: Send + 'static>(
    what: &str,
    bound: Duration,
    step: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(step());
    });
    wait_for(what, bound, |remaining| {
        // timer: deadline — wait_for's deadline bounds this receive
        match receiver.recv_timeout(remaining) {
            Ok(value) => Some(value),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            // The step's thread ended without a result: it panicked. Fail
            // now; waiting on a disconnected channel would spin to the bound.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("{what}: the step failed before producing a result")
            }
        }
    })
}

/// Wait until process `pid` has exited, for at most `bound`.
///
/// Works for any process the test may signal, including grandchildren it
/// does not own; it never reaps. Returns `true` when the process is gone.
///
/// # Panics
///
/// Panics when the exit watch cannot be registered.
#[cfg(unix)]
#[must_use]
pub fn wait_pid_exit(pid: u32, bound: Duration) -> bool {
    let pid = libc::pid_t::try_from(pid).expect("pid in range");
    platform::wait_exit(pid, bound).expect("wait for process exit")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::{Duration, Instant};

    pub(super) fn wait_exit(pid: libc::pid_t, bound: Duration) -> io::Result<bool> {
        // SAFETY: kqueue takes no arguments and returns a new descriptor.
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: kq was just returned by kqueue and is owned by nothing else.
        let kq = unsafe { OwnedFd::from_raw_fd(kq) };
        let change = libc::kevent {
            ident: pid as libc::uintptr_t,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: a zeroed kevent is a valid output record.
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let deadline = Instant::now() + bound;
        let mut registered = false;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let timespec = libc::timespec {
                tv_sec: libc::time_t::try_from(left.as_secs()).unwrap_or(libc::time_t::MAX),
                tv_nsec: libc::c_long::from(left.subsec_nanos()),
            };
            let (changes, count) = if registered {
                (std::ptr::null(), 0)
            } else {
                (&change as *const libc::kevent, 1)
            };
            // SAFETY: changes is null or one valid record, event is a valid
            // output record, and timespec is live for the call.
            // timer: deadline — the caller's bound on the process exit
            let result =
                unsafe { libc::kevent(kq.as_raw_fd(), changes, count, &mut event, 1, &timespec) };
            if result < 0 {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(libc::EINTR) => continue,
                    // The process is gone (or exiting) already.
                    Some(libc::ESRCH) => return Ok(true),
                    _ => return Err(error),
                }
            }
            registered = true;
            if result > 0 {
                if event.flags & libc::EV_ERROR != 0 {
                    return if event.data == libc::ESRCH as isize {
                        Ok(true)
                    } else {
                        Err(io::Error::from_raw_os_error(event.data as i32))
                    };
                }
                return Ok(true);
            }
            if left.is_zero() {
                return Ok(false);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::{Duration, Instant};

    pub(super) fn wait_exit(pid: libc::pid_t, bound: Duration) -> io::Result<bool> {
        // SAFETY: pidfd_open takes a pid and flags and returns a new descriptor.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(true),
                _ => Err(error),
            };
        }
        // SAFETY: fd was just returned by pidfd_open and is owned by nothing else.
        let pidfd = unsafe { OwnedFd::from_raw_fd(fd as libc::c_int) };
        let mut poll_fd = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let deadline = Instant::now() + bound;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let timeout_ms = libc::c_int::try_from(left.as_nanos().div_ceil(1_000_000))
                .unwrap_or(libc::c_int::MAX);
            // SAFETY: poll_fd points at one valid pollfd for the call.
            // timer: deadline — the caller's bound on the process exit
            let count = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
            if count < 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                return Err(error);
            }
            return Ok(count > 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_bounded, wait_pid_exit, Fifo};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    /// A step that panics ends the wait at once, not at its bound.
    #[test]
    fn a_step_that_panics_fails_the_wait_at_once() {
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                run_bounded("a panicking step", Duration::from_secs(3600), || {
                    panic!("the step failed")
                })
            });
            let _ = done_tx.send(result.is_err());
        });
        // timer: deadline — a failed step must end the wait long before its bound
        let failed = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the wait ended when its step failed");
        assert!(failed, "a failed step fails the wait");
    }

    #[test]
    fn a_child_blocked_on_the_gate_runs_once_released() {
        let gate = Fifo::new("gate-test");
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!("/bin/cat '{}' >/dev/null", gate.path().display()))
            .spawn()
            .expect("spawn gated child");
        gate.release(Duration::from_secs(5));
        assert!(child.wait().expect("child").success());
    }

    #[test]
    fn a_child_signal_is_read_when_it_is_written() {
        let signal = Fifo::new("signal-test");
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!("/bin/echo ready > '{}'", signal.path().display()))
            .spawn()
            .expect("spawn signalling child");
        assert_eq!(signal.read_signal(Duration::from_secs(5)), b"ready\n");
        assert!(child.wait().expect("child").success());
    }

    #[test]
    fn a_process_exit_is_observed_and_a_live_one_is_not() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        assert!(!wait_pid_exit(child.id(), Duration::from_millis(20)));
        drop(child.stdin.take());
        assert!(wait_pid_exit(child.id(), Duration::from_secs(5)));
        child.wait().expect("reap");
    }
}

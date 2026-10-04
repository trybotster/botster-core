//! The payload of a session on a real PTY (plan 2.3, the `Program` edge): spawn, output, the exit watch, the group signal and
//! the reap.
//!
//! - **Spawn.** `pty-process` opens the PTY with `rustix` and starts the payload as the leader of a new session whose
//!   controlling terminal is the PTY (`setsid`, `TIOCSCTTY`). So the payload's pid is its process group id and its session
//!   id, and a signal to the group reaches every process that the payload did not move out of it (LC-5, LC-6). The master
//!   stays with the worker; it is close-on-exec and nonblocking. The environment is exactly the requested one (A2-1).
//! - **Exit watch.** One thread blocks in `waitid(P_PID, WEXITED | WNOWAIT)`: it returns when the leader can be reaped, and it
//!   leaves the leader unreaped. The thread reports the status through a callback and ends. No polling and no `unsafe`
//!   (lead decision: a waiter thread replaces the kqueue `EVFILT_PROC` watch of the old `process_exit.rs`).
//! - **Unreaped leader.** The leader is reaped only by [`Payload::reap`]. Until then its pid, which is its group id, cannot be
//!   reused, so [`Payload::signal_group`] never reaches an unrelated group (lead ruling on P1 F7).
//!
//! Clause: Core LC-4, Core LC-5, Core LC-6, Core EV-4, Core A2-1.

use botster_core_edges::edges::ExitStatus;
use rustix::process::{kill_process_group, waitid, Pid, Signal, WaitId, WaitIdOptions};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;
use std::process::Child;

/// Why a payload did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFailure {
    /// The working directory does not exist, or is not a directory (LC-4: `CwdMissing`).
    CwdMissing,
    /// The PTY could not be opened, or the program could not be run (LC-4: `ExecFailed{errno}`).
    Exec { errno: i32 },
}

/// What to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadCommand<'a> {
    pub argv: &'a [String],
    pub env: &'a BTreeMap<String, String>,
    pub cwd: &'a str,
    pub rows: u16,
    pub cols: u16,
}

/// A running payload: its PTY master and its unreaped leader.
///
/// Dropping a payload that was not reaped kills its group and reaps the leader, so a worker that ends leaves no payload
/// behind (BUILD.md testing rule 10 holds for every owner, a test included).
pub struct Payload {
    pty: pty_process::blocking::Pty,
    /// `None` once the leader is reaped: from then on the group is never signalled.
    child: Option<Child>,
    pid: u32,
}

fn errno_of(error: &io::Error) -> i32 {
    // ENOEXEC when the OS gave no number: the program could not be run.
    error.raw_os_error().unwrap_or(8)
}

fn exec_failure(error: pty_process::Error) -> SpawnFailure {
    match error {
        pty_process::Error::Io(e) => SpawnFailure::Exec {
            errno: errno_of(&e),
        },
        pty_process::Error::Rustix(e) => SpawnFailure::Exec {
            errno: e.raw_os_error(),
        },
    }
}

impl std::fmt::Debug for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Payload")
            .field("pid", &self.pid)
            .field("reaped", &self.child.is_none())
            .finish_non_exhaustive()
    }
}

impl Payload {
    /// Starts `command` on a new PTY of its size.
    ///
    /// # Errors
    /// `CwdMissing` when the directory is not there; `Exec{errno}` when the PTY or the program fails.
    pub fn spawn(command: &PayloadCommand<'_>) -> Result<Payload, SpawnFailure> {
        if !Path::new(command.cwd).is_dir() {
            return Err(SpawnFailure::CwdMissing);
        }
        let Some((program, args)) = command.argv.split_first() else {
            // An empty argv names no program (ENOENT).
            return Err(SpawnFailure::Exec { errno: 2 });
        };
        let (pty, pts) = pty_process::blocking::open().map_err(exec_failure)?;
        pty.resize(pty_process::Size::new(command.rows, command.cols))
            .map_err(exec_failure)?;
        // Every fallible setup of the master comes before the child exists, so a failure leaves no process behind. The
        // master's flags do not reach the payload's side of the PTY.
        set_nonblocking(pty.as_fd()).map_err(|e| SpawnFailure::Exec {
            errno: errno_of(&e),
        })?;
        let child = pty_process::blocking::Command::new(program)
            .args(args)
            .env_clear()
            .envs(command.env)
            .current_dir(command.cwd)
            .spawn(pts)
            .map_err(exec_failure)?;
        let pid = child.id();
        // From here a `Payload` owns the child: its drop kills the group and reaps the leader.
        Ok(Payload {
            pty,
            child: Some(child),
            pid,
        })
    }

    /// The leader's pid, which is its process group id.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The PTY master, for the readiness loop.
    pub fn master(&self) -> BorrowedFd<'_> {
        self.pty.as_fd()
    }

    /// Reads the payload's output. `Ok(0)` means that the output ended. On Linux a read after the last writer closed the PTY
    /// gives `EIO`; it is the end of the output too, and is returned as `Ok(0)`.
    ///
    /// # Errors
    /// `WouldBlock` when no byte is there now, or another read error.
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        use std::io::Read;
        match (&self.pty).read(buf) {
            Err(e) if e.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) => Ok(0),
            other => other,
        }
    }

    /// The bytes that the PTY holds for reading now (`FIONREAD`): the bound of the drain after the leader's exit.
    ///
    /// # Errors
    /// The query failed.
    pub fn pending_output(&self) -> io::Result<usize> {
        let n = rustix::io::ioctl_fionread(self.pty.as_fd())?;
        Ok(usize::try_from(n).unwrap_or(usize::MAX))
    }

    /// Writes input to the payload.
    ///
    /// # Errors
    /// `WouldBlock` when the PTY takes no byte now, or another write error.
    pub fn write(&self, bytes: &[u8]) -> io::Result<usize> {
        use std::io::Write;
        (&self.pty).write(bytes)
    }

    /// Starts the exit watch: a thread that calls `on_exit` once with the leader's status, when the leader can be reaped. The
    /// leader stays unreaped.
    ///
    /// # Errors
    /// The thread could not be started.
    pub fn watch_exit(&self, on_exit: impl FnOnce(ExitStatus) + Send + 'static) -> io::Result<()> {
        let pid = Pid::from_raw(i32::try_from(self.pid).unwrap_or(i32::MAX))
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        std::thread::Builder::new()
            .name("payload-exit".into())
            .spawn(move || on_exit(wait_unreaped(pid)))
            .map(|_| ())
    }

    /// Sends `signal` to the payload's process group. The leader is unreaped while `self` exists, so the group id is the
    /// payload's. A number that the platform does not name is ignored.
    pub fn signal_group(&self, signal: i32) {
        if self.child.is_none() {
            return;
        }
        let (Some(pid), Some(signal)) = (
            Pid::from_raw(i32::try_from(self.pid).unwrap_or(i32::MAX)),
            Signal::from_named_raw(signal),
        ) else {
            return;
        };
        // ESRCH: no process of the group is left; nothing remains to signal.
        let _ = kill_process_group(pid, signal);
    }

    /// Reaps the leader, after its group kill. It consumes the payload: no signal can follow.
    pub fn reap(mut self) {
        if let Some(mut child) = self.child.take() {
            // The leader has exited (the watch reported it), so this returns at once.
            let _ = child.wait();
        }
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        if self.child.is_some() {
            self.signal_group(9);
            if let Some(mut child) = self.child.take() {
                let _ = child.wait();
            }
        }
    }
}

/// Blocks until `pid` can be reaped, and returns its status without reaping it.
fn wait_unreaped(pid: Pid) -> ExitStatus {
    loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
            Ok(Some(status)) => {
                if let Some(signal) = status.terminating_signal() {
                    return ExitStatus::Signal(signal);
                }
                if let Some(code) = status.exit_status() {
                    return ExitStatus::Code(code);
                }
            }
            Err(rustix::io::Errno::INTR) | Ok(None) => {}
            // ECHILD: the child is no longer ours to wait for. It cannot happen while the leader is unreaped.
            Err(_) => return ExitStatus::Code(-1),
        }
    }
}

fn set_nonblocking(fd: BorrowedFd<'_>) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LC-4: an OS error keeps its errno; an error without one uses ENOEXEC.
    #[test]
    fn launch_errors_keep_the_os_errno_and_use_the_fallback_only_without_one() {
        assert_eq!(errno_of(&io::Error::from_raw_os_error(13)), 13);
        assert_eq!(errno_of(&io::Error::from(io::ErrorKind::Other)), 8);
        assert_eq!(
            exec_failure(pty_process::Error::Io(io::Error::from_raw_os_error(28))),
            SpawnFailure::Exec { errno: 28 }
        );
        assert_eq!(
            exec_failure(pty_process::Error::Rustix(rustix::io::Errno::ACCESS)),
            SpawnFailure::Exec { errno: 13 }
        );
    }
}

//! Wait for a child process to exit as an OS event, without reaping it.
//!
//! macOS delivers the exit through kqueue `EVFILT_PROC`/`NOTE_EXIT`, Linux
//! through a readable pidfd. The caller still owns the unreaped child, so the
//! pid cannot be reused during the wait; it collects the status afterwards
//! with a non-blocking `try_wait`.

use std::io;
use std::time::Duration;

/// Block until process `pid` exits or `timeout` passes. `None` waits for the
/// exit alone. Returns `Ok(true)` when the process has exited.
pub(crate) fn wait_for_pid_exit(pid: u32, timeout: Option<Duration>) -> io::Result<bool> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid out of range"))?;
    platform::wait(pid, timeout)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::Duration;

    pub(super) fn wait(pid: libc::pid_t, timeout: Option<Duration>) -> io::Result<bool> {
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
        let mut event = change;
        let timespec = timeout.map(|timeout| libc::timespec {
            tv_sec: libc::time_t::try_from(timeout.as_secs()).unwrap_or(libc::time_t::MAX),
            tv_nsec: libc::c_long::from(timeout.subsec_nanos()),
        });
        let timespec_ptr = timespec.as_ref().map_or(std::ptr::null(), |timespec| {
            timespec as *const libc::timespec
        });
        loop {
            // SAFETY: change and event are valid kevent records, and
            // timespec_ptr is null or points at a live timespec.
            let count =
                unsafe { libc::kevent(kq.as_raw_fd(), &change, 1, &mut event, 1, timespec_ptr) };
            if count < 0 {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    // The process already exited and is waiting to be reaped.
                    Some(libc::ESRCH) => return Ok(true),
                    Some(libc::EINTR) => continue,
                    _ => return Err(error),
                }
            }
            if count > 0 && event.flags & libc::EV_ERROR != 0 {
                return match i32::try_from(event.data).ok() {
                    Some(libc::ESRCH) => Ok(true),
                    Some(code) => Err(io::Error::from_raw_os_error(code)),
                    None => Err(io::Error::other("kevent error")),
                };
            }
            return Ok(count > 0);
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::Duration;

    pub(super) fn wait(pid: libc::pid_t, timeout: Option<Duration>) -> io::Result<bool> {
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
        let fd = unsafe { OwnedFd::from_raw_fd(fd as libc::c_int) };
        let timeout_ms = timeout.map_or(-1, |timeout| {
            libc::c_int::try_from(timeout.as_millis()).unwrap_or(libc::c_int::MAX)
        });
        let mut poll_fd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            // SAFETY: poll_fd points at one valid pollfd for the call.
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
    use super::wait_for_pid_exit;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn an_exited_child_is_reported_without_being_reaped() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        assert!(wait_for_pid_exit(child.id(), None).expect("wait"));
        // The status is still there to collect.
        assert!(child.try_wait().expect("try_wait").is_some());
    }

    #[test]
    fn a_live_child_is_not_reported_before_the_deadline() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        // timer: deadline — the child must still be alive when this bound expires
        assert!(!wait_for_pid_exit(child.id(), Some(Duration::from_millis(20))).expect("wait"));
        drop(child.stdin.take());
        assert!(wait_for_pid_exit(child.id(), None).expect("wait"));
        child.wait().expect("reap");
    }
}

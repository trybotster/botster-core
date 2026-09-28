//! Wait for a child process to become reapable as an OS event, without
//! reaping it.
//!
//! macOS wakes through kqueue (`EVFILT_PROC`/`NOTE_EXIT` and `SIGCHLD`) and
//! checks reapability with `waitid(WNOWAIT)`; Linux waits for a readable
//! pidfd, which becomes readable only when the child can be reaped. Register
//! the watch while the caller still owns the unreaped child, so the pid
//! cannot be reused before the registration; the caller collects the status
//! afterwards without blocking.

use std::io;
use std::time::Duration;

/// Block until child `pid` has exited and can be reaped, or `timeout` passes.
/// `None` waits for the exit alone. Returns `Ok(true)` when the child can be
/// reaped (or is already gone), `Ok(false)` when the timeout passed first.
pub(crate) fn wait_for_pid_exit(pid: u32, timeout: Option<Duration>) -> io::Result<bool> {
    ExitWatch::register(pid)?.wait(timeout)
}

/// One registered exit watch for one unreaped child.
pub(crate) struct ExitWatch {
    inner: platform::Watch,
}

impl ExitWatch {
    /// Register for the exit of `pid`. Call while the child is unreaped.
    pub(crate) fn register(pid: u32) -> io::Result<Self> {
        let pid = libc::pid_t::try_from(pid)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid out of range"))?;
        Ok(Self {
            inner: platform::Watch::register(pid)?,
        })
    }

    /// Block until the child has exited and can be reaped, or `timeout`
    /// passes. `None` waits for the exit alone. Returns `Ok(true)` when the
    /// child can be reaped (or is already gone).
    pub(crate) fn wait(&self, timeout: Option<Duration>) -> io::Result<bool> {
        self.inner.wait(timeout)
    }

    /// The descriptor that becomes readable on a possible exit, for a
    /// caller's own `poll`. `None` means the child was already gone at
    /// registration: [`Self::poll_exited`] reports it at once.
    pub(crate) fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
        self.inner.raw_fd()
    }

    /// Without blocking: consume what made the descriptor readable, then
    /// report whether the child can be reaped. After a `false`, the
    /// descriptor is readable again only on a new event, so a poll loop
    /// that calls this on each readable turn cannot spin.
    pub(crate) fn poll_exited(&self) -> io::Result<bool> {
        self.inner.poll_exited()
    }

    /// Test seam: [`Self::poll_exited`], and how many pending events it
    /// consumed from the kqueue.
    #[cfg(all(test, any(target_os = "macos", target_os = "ios")))]
    fn poll_exited_consuming(&self) -> io::Result<(bool, usize)> {
        self.inner.poll_exited_consuming()
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::{Duration, Instant};

    /// A kqueue that wakes on the child's `NOTE_EXIT` and on `SIGCHLD`.
    ///
    /// `NOTE_EXIT` can come before the child is reapable, and registration
    /// fails with `ESRCH` while the child is still exiting. An exit may not
    /// finish soon: a session leader's exit waits for its tty output to
    /// drain. The kernel posts `SIGCHLD` when the child becomes reapable, so
    /// every wake rechecks reapability without reaping, and the wait reports
    /// the exit only once the child can be reaped.
    pub(super) struct Watch {
        pid: libc::pid_t,
        kq: OwnedFd,
    }

    impl Watch {
        pub(super) fn register(pid: libc::pid_t) -> io::Result<Self> {
            // SAFETY: kqueue takes no arguments and returns a new descriptor.
            let kq = unsafe { libc::kqueue() };
            if kq < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: kq was just returned by kqueue and is owned by nothing else.
            let kq = unsafe { OwnedFd::from_raw_fd(kq) };
            // kqueue records a signal delivery even when its disposition
            // discards it, and never consumes it, so this does not change
            // how the process handles SIGCHLD.
            add(
                &kq,
                libc::SIGCHLD as libc::uintptr_t,
                libc::EVFILT_SIGNAL,
                0,
            )?;
            match add(
                &kq,
                pid as libc::uintptr_t,
                libc::EVFILT_PROC,
                libc::NOTE_EXIT,
            ) {
                Ok(()) => {}
                // The child has exited or is still exiting; SIGCHLD or the
                // reapability check covers it.
                Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {}
                Err(error) => return Err(error),
            }
            // The watch starts readable. An exit (and its SIGCHLD) before
            // this registration left no event on this kqueue, so an owner
            // that polls the descriptor first would never be woken for it.
            // One triggered user event makes the first poll return, and the
            // check it leads to observes reapability as it is now.
            let user = libc::kevent {
                ident: 0,
                filter: libc::EVFILT_USER,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: libc::NOTE_TRIGGER,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            change(&kq, &user)?;
            Ok(Self { pid, kq })
        }

        pub(super) fn wait(&self, timeout: Option<Duration>) -> io::Result<bool> {
            let deadline = timeout.map(|timeout| Instant::now() + timeout);
            // SAFETY: a zeroed kevent is a valid output record.
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            loop {
                if reapable(self.pid)? {
                    return Ok(true);
                }
                // An interrupted or unrelated wake resumes with the time that
                // is left.
                let left =
                    deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
                if left.is_some_and(|left| left.is_zero()) {
                    return Ok(false);
                }
                let timespec = left.map(|left| libc::timespec {
                    tv_sec: libc::time_t::try_from(left.as_secs()).unwrap_or(libc::time_t::MAX),
                    tv_nsec: libc::c_long::from(left.subsec_nanos()),
                });
                let timespec_ptr = timespec.as_ref().map_or(std::ptr::null(), |timespec| {
                    timespec as *const libc::timespec
                });
                // SAFETY: event is a valid output record, and timespec_ptr is
                // null or points at a live timespec.
                let count = unsafe {
                    libc::kevent(
                        self.kq.as_raw_fd(),
                        std::ptr::null(),
                        0,
                        &mut event,
                        1,
                        timespec_ptr,
                    )
                };
                if count < 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EINTR) {
                        return Err(error);
                    }
                }
            }
        }

        pub(super) fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
            Some(self.kq.as_raw_fd())
        }

        pub(super) fn poll_exited(&self) -> io::Result<bool> {
            self.poll_exited_consuming().map(|(exited, _)| exited)
        }

        /// [`Self::poll_exited`], and how many pending events it consumed.
        pub(super) fn poll_exited_consuming(&self) -> io::Result<(bool, usize)> {
            // Consume every pending event, so the kqueue is readable again
            // only on a new one; then decide by reapability alone.
            let mut consumed = 0;
            let zero = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: a zeroed kevent is a valid output record.
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            loop {
                // SAFETY: event is a valid output record and zero is a live
                // timespec, so the call returns without waiting.
                let count = unsafe {
                    libc::kevent(
                        self.kq.as_raw_fd(),
                        std::ptr::null(),
                        0,
                        &mut event,
                        1,
                        &zero,
                    )
                };
                if count == 0 {
                    break;
                }
                if count > 0 {
                    consumed += 1;
                }
                if count < 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EINTR) {
                        return Err(error);
                    }
                }
            }
            Ok((reapable(self.pid)?, consumed))
        }
    }

    fn add(kq: &OwnedFd, ident: libc::uintptr_t, filter: i16, fflags: u32) -> io::Result<()> {
        change(
            kq,
            &libc::kevent {
                ident,
                filter,
                flags: libc::EV_ADD,
                fflags,
                data: 0,
                udata: std::ptr::null_mut(),
            },
        )
    }

    fn change(kq: &OwnedFd, change: &libc::kevent) -> io::Result<()> {
        loop {
            // SAFETY: change is one valid kevent record; no events are read.
            let result = unsafe {
                libc::kevent(
                    kq.as_raw_fd(),
                    change,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if result == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINTR) {
                return Err(error);
            }
        }
    }

    /// Whether the child is a zombie that can be reaped now. Does not reap.
    /// A child that is gone (already reaped) counts as exited.
    fn reapable(pid: libc::pid_t) -> io::Result<bool> {
        // SAFETY: a zeroed siginfo_t is a valid output record.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: info is a valid output record for waitid; WNOWAIT leaves
            // the child reapable by its owner.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == 0 {
                // macOS also reports a stopped child here despite WEXITED;
                // only an exit makes the child reapable.
                return Ok(info.si_pid == pid
                    && matches!(
                        info.si_code,
                        libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
                    ));
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EINTR) => {}
                Some(libc::ECHILD) => return Ok(true),
                _ => return Err(error),
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::{Duration, Instant};

    /// A pidfd, or `None` when the child was already gone (reaped) at
    /// registration.
    pub(super) struct Watch {
        pidfd: Option<OwnedFd>,
    }

    impl Watch {
        pub(super) fn register(pid: libc::pid_t) -> io::Result<Self> {
            // SAFETY: pidfd_open takes a pid and flags and returns a new descriptor.
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            if fd < 0 {
                let error = io::Error::last_os_error();
                return match error.raw_os_error() {
                    Some(libc::ESRCH) => Ok(Self { pidfd: None }),
                    _ => Err(error),
                };
            }
            // SAFETY: fd was just returned by pidfd_open and is owned by nothing else.
            Ok(Self {
                pidfd: Some(unsafe { OwnedFd::from_raw_fd(fd as libc::c_int) }),
            })
        }

        pub(super) fn wait(&self, timeout: Option<Duration>) -> io::Result<bool> {
            let Some(pidfd) = &self.pidfd else {
                return Ok(true);
            };
            let deadline = timeout.map(|timeout| Instant::now() + timeout);
            let mut poll_fd = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                // An interrupted wait resumes with the time that is left,
                // rounded up so a sub-millisecond rest cannot become 0.
                let timeout_ms = deadline.map_or(-1, |deadline| {
                    let left = deadline.saturating_duration_since(Instant::now());
                    libc::c_int::try_from(left.as_nanos().div_ceil(1_000_000))
                        .unwrap_or(libc::c_int::MAX)
                });
                // SAFETY: poll_fd points at one valid pollfd for the call.
                // timer: deadline — the caller's bound; None passes -1 and waits only for the exit
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

        pub(super) fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
            self.pidfd.as_ref().map(AsRawFd::as_raw_fd)
        }

        pub(super) fn poll_exited(&self) -> io::Result<bool> {
            // A pidfd is readable exactly when the child can be reaped; there
            // is no other event to consume.
            self.wait(Some(Duration::ZERO))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{wait_for_pid_exit, ExitWatch};
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

    /// Kills and reaps the child on every path, so a failed assertion never
    /// leaves a stopped child holding the harness's output pipes.
    struct Reaped(std::process::Child);

    impl Drop for Reaped {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn a_stopped_child_is_not_reported_as_exited() {
        let mut child = Reaped(
            Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn cat"),
        );
        let pid = child.0.id();
        let watch = ExitWatch::register(pid).expect("register");
        // SAFETY: kill only sends a signal to our own child.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };
        // timer: deadline — a stopped child must still be unexited when this bound expires
        assert!(!watch.wait(Some(Duration::from_millis(200))).expect("wait"));
        // SAFETY: kill only sends a signal to our own child.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGCONT) };
        drop(child.0.stdin.take());
        assert!(watch.wait(None).expect("wait"));
    }

    #[test]
    fn a_watch_registered_before_exit_reports_it_on_another_thread() {
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let watch = ExitWatch::register(child.id()).expect("register");
        let waiter = std::thread::spawn(move || watch.wait(None).expect("wait"));
        drop(child.stdin.take());
        assert!(waiter.join().expect("waiter"));
        child.wait().expect("reap");
    }

    /// Wait for `fd` to become readable.
    fn wait_readable(fd: std::os::fd::RawFd) {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll_fd points at one valid pollfd for the call.
        // timer: deadline — the watched event must arrive; expiry fails the test
        let count = unsafe { libc::poll(&mut poll_fd, 1, 10_000) };
        assert_eq!(count, 1, "the exit watch never became readable");
    }

    /// A caller's poll loop wakes on a stop (the kqueue's SIGCHLD). The check
    /// must consume that event: a descriptor left readable would make every
    /// later poll return at once, and the loop would spin.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn a_polled_check_consumes_a_stop_event_so_a_poll_loop_cannot_spin() {
        let mut child = Reaped(
            Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn cat"),
        );
        let pid = child.0.id();
        let watch = ExitWatch::register(pid).expect("register");
        let fd = watch.raw_fd().expect("a kqueue descriptor");
        // The watch starts readable; its first check finds a live child and
        // consumes the start event. Each check consumes what made the watch
        // readable, so a poll loop cannot spin on it. A sibling test's
        // SIGCHLD (process-wide) can only add to what a check consumes.
        wait_readable(fd);
        let (exited, consumed) = watch.poll_exited_consuming().expect("the first check");
        assert!(!exited);
        assert!(consumed >= 1, "the first check consumed the start event");
        // SAFETY: kill only sends a signal to our own child.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };
        wait_readable(fd);
        let (exited, consumed) = watch.poll_exited_consuming().expect("check");
        assert!(!exited, "a stop is not an exit");
        assert!(
            consumed >= 1,
            "the check consumed the event that made the watch readable"
        );

        // SAFETY: kill only sends a signal to our own child.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGCONT) };
        drop(child.0.stdin.take());
        loop {
            wait_readable(fd);
            if watch.poll_exited().expect("check") {
                break;
            }
        }
        assert!(child.0.try_wait().expect("try_wait").is_some());
    }

    #[test]
    fn a_polled_check_reports_an_exit_through_the_descriptor() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let watch = ExitWatch::register(child.id()).expect("register");
        if let Some(fd) = watch.raw_fd() {
            loop {
                wait_readable(fd);
                if watch.poll_exited().expect("check") {
                    break;
                }
            }
        } else {
            assert!(watch.poll_exited().expect("check"));
        }
        // Not reaped by the check.
        assert!(child.try_wait().expect("try_wait").is_some());
    }

    /// A child that is already reapable when the watch registers left no
    /// event on it. The watch must still report the exit to an owner that
    /// polls its descriptor before checking.
    #[test]
    fn a_child_reapable_before_registration_is_reported_through_the_descriptor() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        // Event-driven and non-reaping: the child is reapable afterwards.
        assert!(wait_for_pid_exit(child.id(), None).expect("wait"));
        let watch = ExitWatch::register(child.id()).expect("register");
        if let Some(fd) = watch.raw_fd() {
            wait_readable(fd);
        }
        assert!(watch.poll_exited().expect("check"));
        // Not reaped by the watch.
        assert!(child.try_wait().expect("try_wait").is_some());
    }
}

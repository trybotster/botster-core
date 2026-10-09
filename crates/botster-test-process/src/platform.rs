//! The platform adapters: the live members of a group, the wait for a process's exit, and the start time of a process (libproc
//! and kqueue on macOS, /proc and pidfd on Linux). They only observe: nothing here signals or reaps.

use rustix::process::Pid;

/// How a wait for a process's exit came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// Its exit event came.
    Exited,
    /// It was gone before the wait began.
    Gone,
    /// The deadline came first.
    Deadline,
}

/// A live member of a group, with its state for a report. Two are the same member when their pids are the same.
#[derive(Clone, Debug)]
pub struct Member {
    pub pid: Pid,
    state: String,
}

impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}

impl std::fmt::Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.pid.as_raw_nonzero(), self.state)
    }
}

/// Whether an error proves that the process is gone (ESRCH).
pub fn gone(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error())
}

/// A pid from a `u32`, as `std::process::Child::id` gives it.
///
/// # Errors
/// The value is not a valid pid.
pub fn pid(raw: u32) -> std::io::Result<Pid> {
    i32::try_from(raw)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| std::io::Error::other(format!("{raw} is not a pid")))
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
pub use linux::{await_end, live_members, start_time};
#[cfg(target_os = "macos")]
pub use macos::{await_end, live_members, start_time};

/// The state and the group of a `/proc/<pid>/stat` line: the first and third fields after the command name, which is in
/// parentheses and may hold spaces.
#[cfg(any(target_os = "linux", test))]
fn state_and_group(stat: &str) -> Option<(&str, i32)> {
    let mut fields = stat.rsplit_once(") ")?.1.split_whitespace();
    let state = fields.next()?;
    let pgrp = fields.nth(1)?.parse().ok()?;
    Some((state, pgrp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stat_line_gives_its_state_and_group_after_a_command_with_spaces_and_parentheses() {
        assert_eq!(state_and_group("41 (a (b) c) S 7 40 40 0"), Some(("S", 40)));
        assert_eq!(state_and_group("41 (cat) Z 7 x 40"), None);
        assert_eq!(state_and_group("41 cat S 7 40"), None);
    }

    #[test]
    fn a_pid_is_positive_and_fits_the_platform_type() {
        assert_eq!(pid(42).unwrap().as_raw_nonzero().get(), 42);
        assert!(pid(0).is_err());
        assert!(pid(u32::MAX).is_err());
    }

    #[test]
    fn this_process_has_a_start_time_and_is_a_live_member_of_its_group() {
        let me = rustix::process::getpid();
        assert!(start_time(me).is_some());
        assert_eq!(start_time(me), start_time(me), "a start time is stable");
        let group = rustix::process::getpgrp();
        assert!(live_members(group).unwrap().iter().any(|m| m.pid == me));
        let shown = live_members(group).unwrap()[0].to_string();
        assert!(shown.contains('('), "{shown}");
    }

    /// A process that started before this test process has a different start time. A pid of no process has none.
    #[test]
    fn processes_that_started_at_different_times_have_different_start_times() {
        let me = start_time(rustix::process::getpid()).unwrap();
        // Linux counts in ticks of 10 ms, and a parent can start this process within its own tick: init is the reference.
        #[cfg(target_os = "linux")]
        let earlier = pid(1).unwrap();
        // macOS counts in microseconds, and it refuses the information of init to a user: the parent is the reference.
        #[cfg(target_os = "macos")]
        let earlier = rustix::process::getppid().expect("a test process has a parent");
        assert_ne!(start_time(earlier).expect("an earlier process"), me);
        assert_eq!(start_time(pid(i32::MAX.unsigned_abs()).unwrap()), None);
    }

    /// No process has the largest pid (both systems' pid limits are lower): the wait reports it gone at once.
    #[test]
    fn a_wait_for_a_pid_with_no_process_reports_it_gone() {
        let none = pid(i32::MAX.unsigned_abs()).unwrap();
        assert_eq!(
            await_end(none, crate::Deadline::after(std::time::Duration::ZERO)).unwrap(),
            Waited::Gone
        );
    }

    #[test]
    fn members_are_the_same_when_their_pids_are_the_same() {
        let member = |raw, state: &str| Member {
            pid: pid(raw).unwrap(),
            state: state.into(),
        };
        assert_eq!(member(7, "state S"), member(7, "state R"));
        assert_ne!(member(7, "state S"), member(8, "state S"));
        assert_eq!(member(7, "state S").to_string(), "7 (state S)");
    }

    #[test]
    fn only_esrch_proves_a_process_gone() {
        assert!(gone(&std::io::Error::from_raw_os_error(
            rustix::io::Errno::SRCH.raw_os_error()
        )));
        assert!(!gone(&std::io::Error::from_raw_os_error(
            rustix::io::Errno::PERM.raw_os_error()
        )));
    }
}

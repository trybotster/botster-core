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

/// `group` as the target of a group signal. Group 1 is refused: `kill(-1, ...)` (rustix's `kill_process_group` of pid 1)
/// signals every process that the caller may signal, not a group. So a wrong group id of 1 would end every process of the
/// user, the test runner included: a mutant of `OwnedChild::id` that returns 1 ended the whole mutation run (#171 round 2,
/// E1). Every group signal of this crate takes its target from here.
///
/// # Errors
/// `group` is 1.
pub fn signal_target(group: Pid) -> std::io::Result<Pid> {
    if group.as_raw_nonzero().get() == 1 {
        return Err(std::io::Error::other(
            "group 1 is not signalled: kill(-1) signals every process",
        ));
    }
    Ok(group)
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

/// The status of the exited child `pid`, read without a reap and without a wait: `None` while it is not available (the child
/// runs, or its exit has not completed). Only for the caller's own unreaped child, whose pid cannot be reused.
///
/// # Errors
/// The check failed (for example, `pid` is not an unreaped child of this process), or the status names no exit.
pub fn peek(pid: Pid) -> std::io::Result<Option<std::process::ExitStatus>> {
    use rustix::process::{waitid, WaitId, WaitIdOptions};
    use std::os::unix::process::ExitStatusExt;
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG;
    let Some(status) = waitid(WaitId::Pid(pid), options)? else {
        return Ok(None);
    };
    match (status.exit_status(), status.terminating_signal()) {
        (Some(code), _) => Ok(Some(std::process::ExitStatus::from_raw((code & 0xff) << 8))),
        (None, Some(signal)) => Ok(Some(std::process::ExitStatus::from_raw(signal & 0x7f))),
        (None, None) => Err(std::io::Error::other(format!(
            "the child {} has a status that names no exit",
            pid.as_raw_nonzero()
        ))),
    }
}

/// The decision of `await_status`: the status as soon as `peek` finds it, else a block on the next event, until `expired`.
/// An exit event can come before the status is available (macOS), so one event is not enough: the loop checks again after
/// each event. `peek` comes before the deadline check, so a status that is already available is never lost to the deadline;
/// `next_event` blocks on a real event, at most until the deadline, so the loop does not spin.
///
/// # Errors
/// The first error of `peek` or `next_event`.
pub(crate) fn status_after_events(
    mut peek: impl FnMut() -> std::io::Result<Option<std::process::ExitStatus>>,
    mut next_event: impl FnMut() -> std::io::Result<()>,
    expired: impl Fn() -> bool,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    loop {
        if let Some(status) = peek()? {
            return Ok(Some(status));
        }
        if expired() {
            return Ok(None);
        }
        next_event()?;
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
pub use linux::{await_end, await_status, live_members, start_time};
#[cfg(target_os = "macos")]
pub use macos::{await_end, await_status, live_members, start_time};

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

    /// #171 round 2, E1: group 1 is never a signal target (`kill(-1, ...)` signals every process); another group is kept.
    /// The test sends no signal.
    #[test]
    fn group_1_is_refused_as_a_signal_target_and_another_group_is_kept() {
        let one = Pid::from_raw(1).unwrap();
        assert_eq!(
            signal_target(one).unwrap_err().to_string(),
            "group 1 is not signalled: kill(-1) signals every process"
        );
        let two = Pid::from_raw(2).unwrap();
        assert_eq!(signal_target(two).unwrap(), two);
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
        assert!(start_time(me).unwrap().is_some());
        assert_eq!(
            start_time(me).unwrap(),
            start_time(me).unwrap(),
            "a start time is stable"
        );
        let group = rustix::process::getpgrp();
        assert!(live_members(group).unwrap().iter().any(|m| m.pid == me));
        let shown = live_members(group).unwrap()[0].to_string();
        assert!(shown.contains('('), "{shown}");
    }

    /// A process that started before this test process has a different start time. A pid of no process has none.
    #[test]
    fn processes_that_started_at_different_times_have_different_start_times() {
        let me = start_time(rustix::process::getpid()).unwrap().unwrap();
        // Linux counts in ticks of 10 ms, and a parent can start this process within its own tick: init is the reference.
        #[cfg(target_os = "linux")]
        let earlier = pid(1).unwrap();
        // macOS counts in microseconds, and it refuses the information of init to a user: the parent is the reference.
        #[cfg(target_os = "macos")]
        let earlier = rustix::process::getppid().expect("a test process has a parent");
        assert_ne!(
            start_time(earlier).unwrap().expect("an earlier process"),
            me
        );
        assert_eq!(
            start_time(pid(i32::MAX.unsigned_abs()).unwrap()).unwrap(),
            None
        );
    }

    /// On macOS a start time is in microseconds since the Unix epoch: this process started in the last day, by the clock.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_start_time_on_macos_is_in_microseconds_since_the_epoch() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started = start_time(rustix::process::getpid()).unwrap().unwrap() / 1_000_000;
        assert!(
            (now - 86_400..=now).contains(&started),
            "started {started}, now {now}"
        );
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

    /// A fake child: `peek` gives the scripted answers in turn, and each event and each deadline check is counted.
    fn awaited(
        peeks: &[Option<i32>],
        expired_after: usize,
    ) -> (std::io::Result<Option<std::process::ExitStatus>>, usize) {
        use std::os::unix::process::ExitStatusExt;
        let mut peeks = peeks.iter();
        let events = std::cell::Cell::new(0);
        let result = status_after_events(
            || {
                Ok(peeks
                    .next()
                    .copied()
                    .flatten()
                    .map(|code| std::process::ExitStatus::from_raw(code << 8)))
            },
            || {
                events.set(events.get() + 1);
                Ok(())
            },
            || events.get() >= expired_after,
        );
        (result, events.get())
    }

    /// A status that is already available is returned at once, even when the deadline has passed.
    #[test]
    fn an_available_status_is_returned_with_no_event_and_after_the_deadline() {
        let (status, events) = awaited(&[Some(3)], 0);
        assert_eq!(status.unwrap().unwrap().code(), Some(3));
        assert_eq!(events, 0);
    }

    /// TP1 (#171): an exit event can come before the status is available (macOS sends `NOTE_EXIT` before the child is a
    /// zombie). The wait checks again after each event, and the status that comes after a later event is returned.
    #[test]
    fn an_exit_event_whose_status_is_not_yet_available_waits_for_the_next_event() {
        let (status, events) = awaited(&[None, None, Some(5)], 10);
        assert_eq!(status.unwrap().unwrap().code(), Some(5));
        assert_eq!(events, 2);
    }

    /// TP1 (#171): a status that never becomes available ends the wait at the deadline, with no status, not in a hang.
    #[test]
    fn a_status_that_never_becomes_available_ends_the_wait_at_the_deadline() {
        let (status, events) = awaited(&[], 3);
        assert_eq!(status.unwrap(), None);
        assert_eq!(events, 3);
    }

    #[test]
    fn a_failed_check_or_event_fails_the_wait() {
        let failed =
            status_after_events(|| Err(std::io::Error::other("check")), || Ok(()), || false);
        assert_eq!(failed.unwrap_err().to_string(), "check");
        let failed = status_after_events(
            || Ok(None),
            || Err(std::io::Error::other("event")),
            || false,
        );
        assert_eq!(failed.unwrap_err().to_string(), "event");
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

//! The platform adapters: the live members of a group, the wait for a process's exit, and the start time of a process (libproc
//! and kqueue on macOS, /proc and pidfd on Linux). They only observe: nothing here signals or reaps.

use crate::Deadline;
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

/// The start time of `pid` in the unit of the platform, or `None` when no such process exists. Only equality has a meaning.
/// It reads the same field as `botster_core_sys::process::start_time`, so the two agree.
pub fn start_time(pid: Pid) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        use libproc::bsd_info::BSDInfo;
        use libproc::proc_pid::pidinfo;
        let info = pidinfo::<BSDInfo>(pid.as_raw_nonzero().get(), 0).ok()?;
        Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_nonzero())).ok()?;
        // The command name is in parentheses and may hold spaces: the fields start after its closing one. Field 22
        // (`starttime`) is the 20th after the state, which is the first.
        let after = stat.rsplit_once(") ")?.1;
        after.split_whitespace().nth(19)?.parse().ok()
    }
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: it never reaps. A
/// pid that was reused meanwhile can make it wait for another process, within the deadline; a caller that must not be
/// misled holds the process unreaped (its own child), so that the pid cannot be reused.
///
/// # Errors
/// The wait could not be set up, or it failed.
#[cfg(target_os = "macos")]
pub fn await_end(pid: Pid, deadline: Deadline) -> std::io::Result<Waited> {
    let mut watcher = kqueue::Watcher::new()?;
    watcher.add_pid(
        pid.as_raw_nonzero().get(),
        kqueue::EventFilter::EVFILT_PROC,
        kqueue::FilterFlag::NOTE_EXIT,
    )?;
    match watcher.watch() {
        Err(error) if gone(&error) => return Ok(Waited::Gone),
        other => other?,
    }
    // timer: deadline — bounds the wait for an exit event.
    match watcher.poll(Some(deadline.remaining())) {
        None => Ok(Waited::Deadline),
        Some(kqueue::Event {
            data: kqueue::EventData::Error(error),
            ..
        }) => Err(error),
        Some(_) => Ok(Waited::Exited),
    }
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: it never reaps. A
/// pid that was reused meanwhile can make it wait for another process, within the deadline; a caller that must not be
/// misled holds the process unreaped (its own child), so that the pid cannot be reused.
///
/// # Errors
/// The wait could not be set up, or it failed.
#[cfg(target_os = "linux")]
pub fn await_end(pid: Pid, deadline: Deadline) -> std::io::Result<Waited> {
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Err(rustix::io::Errno::SRCH) => return Ok(Waited::Gone),
        other => other?,
    };
    loop {
        let mut fds = [rustix::event::PollFd::new(
            &pidfd,
            rustix::event::PollFlags::IN,
        )];
        // timer: deadline — bounds the wait for an exit event.
        match rustix::event::poll(&mut fds, Some(&deadline.timespec())) {
            Ok(0) => return Ok(Waited::Deadline),
            Ok(_) => return Ok(Waited::Exited),
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

/// The live members of `group`, with their states. A process is left out only when it is proved not live: a zombie (it
/// cannot fork, and its parent reaps it) or a process that is gone. macOS refuses the information of a zombie, so a refused
/// process is checked with the process filter of kqueue, which refuses a process that is exiting or gone with ESRCH; a live
/// process whose information is refused fails the listing.
///
/// # Errors
/// The process table could not be read.
#[cfg(target_os = "macos")]
pub fn live_members(group: Pid) -> std::io::Result<Vec<Member>> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    use libproc::processes::{pids_by_type, ProcFilter};
    let group_id = group.as_raw_nonzero().get().unsigned_abs();
    let mut members = Vec::new();
    // libproc reads `errno` when the kernel lists no process, and that `errno` can be left over from an earlier call. So a
    // failed listing is checked with a test signal to the group, which skips zombies: ESRCH proves no live member.
    let pids = match pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group_id }) {
        Ok(pids) => pids,
        Err(error) => match rustix::process::test_kill_process_group(group) {
            Err(rustix::io::Errno::SRCH) => return Ok(Vec::new()),
            _ => return Err(error),
        },
    };
    for raw in pids {
        let Some(process) = i32::try_from(raw).ok().and_then(Pid::from_raw) else {
            continue;
        };
        let info = match pidinfo::<BSDInfo>(process.as_raw_nonzero().get(), 0) {
            Ok(info) => info,
            Err(refused) => {
                if exiting_or_gone(process)? {
                    continue;
                }
                return Err(std::io::Error::other(format!("process {raw}: {refused}")));
            }
        };
        if info.pbi_pgid == group_id && info.pbi_status != libc::SZOMB {
            members.push(Member {
                pid: process,
                state: format!("status {}", info.pbi_status),
            });
        }
    }
    Ok(members)
}

/// Whether `pid` is exiting (a zombie included) or gone: the process filter of kqueue refuses such a process with ESRCH.
///
/// # Errors
/// The filter could not be set up for another reason.
#[cfg(target_os = "macos")]
fn exiting_or_gone(pid: Pid) -> std::io::Result<bool> {
    let mut watcher = kqueue::Watcher::new()?;
    watcher.add_pid(
        pid.as_raw_nonzero().get(),
        kqueue::EventFilter::EVFILT_PROC,
        kqueue::FilterFlag::NOTE_EXIT,
    )?;
    match watcher.watch() {
        Ok(()) => Ok(false),
        Err(error) if gone(&error) => Ok(true),
        Err(error) => Err(error),
    }
}

/// The live members of `group`, with their states. A zombie is not live: it cannot fork, and its parent reaps it. A process
/// whose information cannot be read is left out only when it is proved gone.
///
/// # Errors
/// The process table could not be read.
#[cfg(target_os = "linux")]
pub fn live_members(group: Pid) -> std::io::Result<Vec<Member>> {
    let mut members = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let Some(pid) = entry?
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => stat,
            // The process ended since the listing: its directory is gone.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound || gone(&error) => continue,
            Err(error) => return Err(error),
        };
        let Some((state, pgrp)) = state_and_group(&stat) else {
            return Err(std::io::Error::other(format!(
                "process {pid}: an unreadable stat"
            )));
        };
        if state != "Z" && pgrp == group.as_raw_nonzero().get() {
            if let Some(pid) = Pid::from_raw(pid) {
                members.push(Member {
                    pid,
                    state: format!("state {state}"),
                });
            }
        }
    }
    Ok(members)
}

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
        let group = rustix::process::getpgrp();
        assert!(live_members(group).unwrap().iter().any(|m| m.pid == me));
        let shown = live_members(group).unwrap()[0].to_string();
        assert!(shown.contains('('), "{shown}");
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

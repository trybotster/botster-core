//! The Linux adapters: /proc for the members and the start time, a pidfd for the exit.

use super::{gone, state_parent_and_group, Member, Waited};
use crate::Deadline;
use rustix::process::Pid;

/// The start time of `pid` in the unit of the platform, or `None` when no such process exists. Only equality has a meaning.
/// It reads the same field as `botster_core_sys::process::start_time`, so the two agree.
///
/// # Errors
/// The process's stat could not be read for another reason, or it has no start time: the identity is not verified.
pub fn start_time(pid: Pid) -> std::io::Result<Option<u64>> {
    let stat = match std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_nonzero())) {
        Ok(stat) => stat,
        Err(error) if ended_since_listing(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    starttime(&stat).map(Some).ok_or_else(|| {
        std::io::Error::other(format!(
            "process {}: a stat with no start time",
            pid.as_raw_nonzero()
        ))
    })
}

/// The start time of a `/proc/<pid>/stat` line. The command name is in parentheses and may hold spaces: the fields start
/// after its closing one. Field 22 (`starttime`) is the 20th after the state, which is the first.
fn starttime(stat: &str) -> Option<u64> {
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: it never reaps. A
/// pid that was reused meanwhile can make it wait for another process, within the deadline; a caller that must not be
/// misled holds the process unreaped (its own child), so that the pid cannot be reused.
///
/// # Errors
/// The wait could not be set up, or it failed.
pub fn await_end(pid: Pid, deadline: Deadline) -> std::io::Result<Waited> {
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Err(error) if gone_at_open(error) => return Ok(Waited::Gone),
        other => other?,
    };
    let mut fds = [rustix::event::PollFd::new(
        &pidfd,
        rustix::event::PollFlags::IN,
    )];
    match crate::deadline::retry_interrupted(
        // timer: deadline — bounds the wait for an exit event.
        || rustix::event::poll(&mut fds, Some(&deadline.timespec())),
        || deadline.expired(),
    )? {
        None | Some(0) => Ok(Waited::Deadline),
        Some(_) => Ok(Waited::Exited),
    }
}

/// The exit status of the caller's own unreaped child `pid` once it is available, at most until `deadline`; `None` when the
/// deadline came first. It never reaps. The event is a readable pidfd: the kernel reports it once the child's thread group
/// has exited, which is when the child is a zombie and its status is available.
///
/// # Errors
/// The check or the wait failed.
pub fn await_status(
    pid: Pid,
    deadline: Deadline,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    super::status_after_events(
        || super::peek(pid),
        || await_end(pid, deadline).map(drop),
        || deadline.expired(),
    )
}

/// Whether an error of the read of `/proc/<pid>/stat` proves that the process ended since the listing: its directory is gone
/// (ENOENT), or the process is (ESRCH).
fn ended_since_listing(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound || gone(error)
}

/// Whether a `pidfd_open` error proves the process gone. ESRCH: no process has the pid. EINVAL: the pid is still allocated,
/// but its process was released (kernel/pid.c refuses a pid with no thread-group task), which a member that its parent reaps
/// during the wait can be. The flags are empty and the pid is positive, so EINVAL has no other cause.
fn gone_at_open(error: rustix::io::Errno) -> bool {
    matches!(error, rustix::io::Errno::SRCH | rustix::io::Errno::INVAL)
}

/// The live members of `group`, with their states. A zombie is not live: it cannot fork, and its parent reaps it. A process
/// whose information cannot be read is left out only when it is proved gone.
///
/// # Errors
/// The process table could not be read.
pub fn live_members(group: Pid) -> std::io::Result<Vec<Member>> {
    live_where(|_, pgrp| pgrp == group.as_raw_nonzero().get())
}

/// The live children of `parent`, with their states, as [`live_members`] reads them: a zombie is not live.
///
/// # Errors
/// The process table could not be read.
pub fn live_children(parent: Pid) -> std::io::Result<Vec<Member>> {
    live_where(|ppid, _| ppid == parent.as_raw_nonzero().get())
}

/// The live processes whose parent and group `keep` takes.
fn live_where(keep: impl Fn(i32, i32) -> bool) -> std::io::Result<Vec<Member>> {
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
            Err(error) if ended_since_listing(&error) => continue,
            Err(error) => return Err(error),
        };
        let Some((state, ppid, pgrp)) = state_parent_and_group(&stat) else {
            return Err(std::io::Error::other(format!(
                "process {pid}: an unreadable stat"
            )));
        };
        if state != "Z" && keep(ppid, pgrp) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_esrch_and_einval_at_open_prove_a_process_gone() {
        assert!(gone_at_open(rustix::io::Errno::SRCH));
        assert!(gone_at_open(rustix::io::Errno::INVAL));
        assert!(!gone_at_open(rustix::io::Errno::PERM));
        assert!(!gone_at_open(rustix::io::Errno::MFILE));
    }

    #[test]
    fn a_stat_line_gives_its_start_time_after_a_command_with_spaces_and_parentheses() {
        // The state and the 18 fields after it: field 22 is the next one.
        let fields: Vec<String> = (4..=21).map(|n| n.to_string()).collect();
        let stat = format!("41 (a (b) c) S {}", fields.join(" "));
        assert_eq!(starttime(&format!("{stat} 77 x")), Some(77));
        assert_eq!(starttime(&stat), None, "no field 22");
        assert_eq!(starttime(&format!("{stat} x")), None, "not a number");
        assert_eq!(starttime("41 cat S 7"), None, "no command name");
    }

    #[test]
    fn only_a_missing_stat_or_esrch_proves_an_end_since_the_listing() {
        let os = |errno: rustix::io::Errno| std::io::Error::from_raw_os_error(errno.raw_os_error());
        assert!(ended_since_listing(&os(rustix::io::Errno::NOENT)));
        assert!(ended_since_listing(&os(rustix::io::Errno::SRCH)));
        assert!(!ended_since_listing(&os(rustix::io::Errno::ACCESS)));
    }
}

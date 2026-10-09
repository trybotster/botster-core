//! The Linux adapters: /proc for the members and the start time, a pidfd for the exit.

use super::{gone, state_and_group, Member, Waited};
use crate::Deadline;
use rustix::process::Pid;

/// The start time of `pid` in the unit of the platform, or `None` when no such process exists. Only equality has a meaning.
/// It reads the same field as `botster_core_sys::process::start_time`, so the two agree.
pub fn start_time(pid: Pid) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_nonzero())).ok()?;
    // The command name is in parentheses and may hold spaces: the fields start after its closing one. Field 22
    // (`starttime`) is the 20th after the state, which is the first.
    let after = stat.rsplit_once(") ")?.1;
    after.split_whitespace().nth(19)?.parse().ok()
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
    fn only_a_missing_stat_or_esrch_proves_an_end_since_the_listing() {
        let os = |errno: rustix::io::Errno| std::io::Error::from_raw_os_error(errno.raw_os_error());
        assert!(ended_since_listing(&os(rustix::io::Errno::NOENT)));
        assert!(ended_since_listing(&os(rustix::io::Errno::SRCH)));
        assert!(!ended_since_listing(&os(rustix::io::Errno::ACCESS)));
    }
}

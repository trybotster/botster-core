//! The platform adapters of the guards' cleanup: the live members of a group and the wait for a member's end (libproc and
//! kqueue on macOS, /proc and pidfd on Linux).

/// How a wait for a member's end came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Waited {
    /// Its exit event came.
    Exited,
    /// It was gone before the wait began.
    Gone,
    /// The deadline came first.
    Deadline,
}

/// A live member of the group, with its state for a report. Two are the same member when their pids are the same.
#[derive(Clone)]
pub(crate) struct Member {
    pub(crate) pid: rustix::process::Pid,
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
pub(crate) fn gone(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error())
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: a pid that was
/// reused meanwhile can make it wait for another process, within the deadline.
///
/// # Errors
/// The wait could not be set up, or it failed.
#[cfg(target_os = "macos")]
pub(crate) fn await_end(
    pid: rustix::process::Pid,
    deadline: std::time::Instant,
) -> std::io::Result<Waited> {
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
    // timer: deadline — bounds the wait for a killed member's exit event.
    match watcher.poll(Some(
        deadline.saturating_duration_since(std::time::Instant::now()),
    )) {
        None => Ok(Waited::Deadline),
        Some(kqueue::Event {
            data: kqueue::EventData::Error(error),
            ..
        }) => Err(error),
        Some(_) => Ok(Waited::Exited),
    }
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: a pid that was
/// reused meanwhile can make it wait for another process, within the deadline.
///
/// # Errors
/// The wait could not be set up, or it failed.
#[cfg(target_os = "linux")]
pub(crate) fn await_end(
    pid: rustix::process::Pid,
    deadline: std::time::Instant,
) -> std::io::Result<Waited> {
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Err(rustix::io::Errno::SRCH) => return Ok(Waited::Gone),
        other => other?,
    };
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        // timer: deadline — bounds the wait for a killed member's exit event.
        let limit = rustix::event::Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: left.subsec_nanos().into(),
        };
        let mut fds = [rustix::event::PollFd::new(
            &pidfd,
            rustix::event::PollFlags::IN,
        )];
        match rustix::event::poll(&mut fds, Some(&limit)) {
            Ok(0) => return Ok(Waited::Deadline),
            Ok(_) => return Ok(Waited::Exited),
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

/// The live members of `group`, with their states. A process is left out only when it is proved not live: a zombie (it
/// cannot fork, and its parent reaps it) or a process that is gone. macOS refuses the information of a zombie, so a
/// refused process is checked with the process filter of kqueue, which refuses a process that is exiting or gone with
/// ESRCH; a live process whose information is refused fails the listing.
///
/// # Errors
/// The process table could not be read.
#[cfg(target_os = "macos")]
pub(crate) fn live_members(group: rustix::process::Pid) -> std::io::Result<Vec<Member>> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    use libproc::processes::{pids_by_type, ProcFilter};
    let group_id = group.as_raw_nonzero().get() as u32;
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
    for pid in pids {
        let Some(process) = rustix::process::Pid::from_raw(pid as i32) else {
            continue;
        };
        let info = match pidinfo::<BSDInfo>(pid as i32, 0) {
            Ok(info) => info,
            Err(refused) => {
                if exiting_or_gone(process)? {
                    continue;
                }
                return Err(std::io::Error::other(format!("process {pid}: {refused}")));
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
fn exiting_or_gone(pid: rustix::process::Pid) -> std::io::Result<bool> {
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

/// The live members of `group`, with their states. A zombie is not live: it cannot fork, and its parent reaps it. A
/// process whose information cannot be read is left out only when it is proved gone.
///
/// # Errors
/// The process table could not be read.
#[cfg(target_os = "linux")]
pub(crate) fn live_members(group: rustix::process::Pid) -> std::io::Result<Vec<Member>> {
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
        // The fields after the command name: state, ppid, pgrp.
        let fields: Vec<&str> = stat
            .rsplit_once(") ")
            .map_or(Vec::new(), |(_, rest)| rest.split_whitespace().collect());
        let (Some(&state), Some(pgrp)) = (fields.first(), fields.get(2)) else {
            return Err(std::io::Error::other(format!(
                "process {pid}: an unreadable stat"
            )));
        };
        let pgrp: i32 = pgrp.parse().map_err(|_| {
            std::io::Error::other(format!("process {pid}: an unreadable group {pgrp}"))
        })?;
        if state != "Z" && pgrp == group.as_raw_nonzero().get() {
            if let Some(pid) = rustix::process::Pid::from_raw(pid) {
                members.push(Member {
                    pid,
                    state: format!("state {state}"),
                });
            }
        }
    }
    Ok(members)
}

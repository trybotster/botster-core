//! The macOS adapters: libproc for the members and the start time, kqueue for the exit. The Linux gate does not compile this
//! file; the macOS slow tier runs it.

use super::{gone, Member, Waited};
use crate::Deadline;
use rustix::process::Pid;

/// The start time of `pid` in microseconds since the Unix epoch, or `None` when the process has ended (gone, or exiting: macOS
/// refuses the information of a zombie). Callers compare start times only for equality. It reads the same field as
/// `botster_core_sys::process::start_time`, so the two agree.
///
/// # Errors
/// The information of a live process was refused, or the check of the refusal failed: the identity is not verified.
pub fn start_time(pid: Pid) -> std::io::Result<Option<u64>> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    match pidinfo::<BSDInfo>(pid.as_raw_nonzero().get(), 0) {
        Ok(info) => Ok(Some(
            info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec,
        )),
        Err(_) if exiting_or_gone(pid)? => Ok(None),
        Err(refused) => Err(std::io::Error::other(format!(
            "process {}: {refused}",
            pid.as_raw_nonzero()
        ))),
    }
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: it never reaps. A
/// pid that was reused meanwhile can make it wait for another process, within the deadline; a caller that must not be
/// misled holds the process unreaped (its own child), so that the pid cannot be reused.
///
/// # Errors
/// The wait could not be set up, or it failed.
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

/// The exit status of the caller's own unreaped child `pid` once it is available, at most until `deadline`; `None` when the
/// deadline came first. It never reaps. XNU's `proc_exit` posts `NOTE_EXIT`, then makes the child a zombie (its status is
/// available), then sends `SIGCHLD` to the parent (`bsd/kern/kern_exit.c`). So the exit event can come before the status,
/// and the event that follows it is `SIGCHLD`. kqueue records a `SIGCHLD` with no handler installed, the default action
/// included: `psignal_internal` posts the signal's event before it checks whether the signal is ignored
/// (`bsd/kern/kern_sig.c`). The filter is set before the first check, so a `SIGCHLD` after that check wakes the wait. A
/// `SIGCHLD` of another child only makes the loop check again.
///
/// # Errors
/// The filter could not be set up, or the check or the wait failed.
pub fn await_status(
    pid: Pid,
    deadline: Deadline,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let mut watcher = kqueue::Watcher::new()?;
    // The crate has no call for a signal filter. The ident of a kevent is a number that the filter interprets, and `add_fd`
    // passes it unchanged: here, the signal number.
    watcher.add_fd(
        libc::SIGCHLD,
        kqueue::EventFilter::EVFILT_SIGNAL,
        kqueue::FilterFlag::empty(),
    )?;
    watcher.watch()?;
    super::status_after_events(
        || super::peek(pid),
        // timer: deadline — bounds the wait for a SIGCHLD.
        || match watcher.poll(Some(deadline.remaining())) {
            Some(kqueue::Event {
                data: kqueue::EventData::Error(error),
                ..
            }) => Err(error),
            _ => Ok(()),
        },
        || deadline.expired(),
    )
}

/// The live members of `group`, with their states. A process is left out only when it is proved not live: a zombie (it
/// cannot fork, and its parent reaps it) or a process that is gone. macOS refuses the information of a zombie, so a refused
/// process is checked with the process filter of kqueue, which refuses a process that is exiting or gone with ESRCH; a live
/// process whose information is refused fails the listing.
///
/// # Errors
/// The process table could not be read.
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

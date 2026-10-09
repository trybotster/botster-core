//! The macOS adapters: libproc for the members and the start time, kqueue for the exit. The Linux gate does not compile this
//! file; the macOS slow tier runs it.

use super::{gone, Member, Waited};
use crate::Deadline;
use rustix::process::Pid;

/// The start time of `pid` in the unit of the platform, or `None` when no such process exists. Only equality has a meaning.
/// It reads the same field as `botster_core_sys::process::start_time`, so the two agree.
pub fn start_time(pid: Pid) -> Option<u64> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    let info = pidinfo::<BSDInfo>(pid.as_raw_nonzero().get(), 0).ok()?;
    Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
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

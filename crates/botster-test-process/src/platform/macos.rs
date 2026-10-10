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
    exit_after_polls(
        // timer: deadline — bounds the wait for an exit event.
        || polled(watcher.poll(Some(deadline.remaining()))),
        || deadline.expired(),
    )
}

/// The outcome of the polls of a wait for an exit event: an event ends it as `Exited`, a timeout as `Deadline`. An
/// interrupted poll polls again only before the deadline: a poll with no time left can still return EINTR (XNU arms the
/// timer of a wait after it marks the thread waiting, and an aborted wait returns before that), so repeated interruptions
/// end at the deadline (#171 TP9).
///
/// # Errors
/// A poll failed.
fn exit_after_polls(
    mut poll: impl FnMut() -> std::io::Result<Polled>,
    expired: impl Fn() -> bool,
) -> std::io::Result<Waited> {
    loop {
        match poll()? {
            Polled::Timeout => return Ok(Waited::Deadline),
            Polled::Event => return Ok(Waited::Exited),
            Polled::Interrupted if expired() => return Ok(Waited::Deadline),
            Polled::Interrupted => {}
        }
    }
}

/// The meaning of one poll of a watcher.
#[derive(Debug, PartialEq, Eq)]
enum Polled {
    /// The timeout passed with no event.
    Timeout,
    /// An event came.
    Event,
    /// A signal interrupted the wait before any event (EINTR): the caller polls again, with the time that remains.
    Interrupted,
}

/// What `event`, the result of `Watcher::poll`, means. The kqueue crate returns a failed `kevent` call as an error event
/// (`Event::from_error`), and `kevent` fails with EINTR when a signal is delivered before the timeout and before any event
/// (kevent(2)).
///
/// # Errors
/// The `kevent` call failed for another reason.
fn polled(event: Option<kqueue::Event>) -> std::io::Result<Polled> {
    match event {
        None => Ok(Polled::Timeout),
        Some(kqueue::Event {
            data: kqueue::EventData::Error(error),
            ..
        }) if error.kind() == std::io::ErrorKind::Interrupted => Ok(Polled::Interrupted),
        Some(kqueue::Event {
            data: kqueue::EventData::Error(error),
            ..
        }) => Err(error),
        Some(_) => Ok(Polled::Event),
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
        // An interrupted poll, as a timeout, only makes the loop check the status and the deadline again.
        // timer: deadline — bounds the wait for a SIGCHLD.
        || polled(watcher.poll(Some(deadline.remaining()))).map(drop),
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
    use libproc::processes::{pids_by_type, ProcFilter};
    let group_id = group.as_raw_nonzero().get().unsigned_abs();
    // libproc reads `errno` when the kernel lists no process, and that `errno` can be left over from an earlier call. So a
    // failed listing is checked with a test signal to the group, which skips zombies: ESRCH proves no live member.
    let pids = match pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group_id }) {
        Ok(pids) => pids,
        Err(error) => match rustix::process::test_kill_process_group(group) {
            Err(rustix::io::Errno::SRCH) => return Ok(Vec::new()),
            _ => return Err(error),
        },
    };
    live_of(pids, |info| info.pbi_pgid == group_id)
}

/// The live children of `parent`, with their states, as [`live_members`] reads them: a zombie is not live.
///
/// # Errors
/// The process table could not be read.
pub fn live_children(parent: Pid) -> std::io::Result<Vec<Member>> {
    use libproc::processes::{pids_by_type, ProcFilter};
    let ppid = parent.as_raw_nonzero().get().unsigned_abs();
    // libproc takes an empty listing for an error when `errno` is not 0, and that `errno` can be left over from an earlier
    // call. With `errno` cleared first, an empty listing is no child, and an error is the listing's own.
    errno::set_errno(errno::Errno(0));
    let pids = pids_by_type(ProcFilter::ByParentProcess { ppid })?;
    live_of(pids, |info| info.pbi_ppid == ppid)
}

/// The live processes of `pids` whose information `keep` takes. A process whose information is refused is left out only
/// when it is exiting or gone.
fn live_of(
    pids: Vec<u32>,
    keep: impl Fn(&libproc::bsd_info::BSDInfo) -> bool,
) -> std::io::Result<Vec<Member>> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    let mut members = Vec::new();
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
        if keep(&info) && info.pbi_status != libc::SZOMB {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(kind: std::io::ErrorKind) -> Option<kqueue::Event> {
        Some(kqueue::Event {
            ident: kqueue::Ident::Fd(-1),
            data: kqueue::EventData::Error(kind.into()),
        })
    }

    /// #171 TP9: an exit event or a timeout ends the polls; interruptions poll again until the deadline, and then end the
    /// wait as at a timeout, whatever the poll would return next; a failed poll fails the wait.
    #[test]
    fn interrupted_polls_end_at_the_deadline_and_an_event_or_a_timeout_ends_them_at_once() {
        let run = |results: Vec<std::io::Result<Polled>>, expired_after: usize| {
            let mut results = results.into_iter();
            let polls = std::cell::Cell::new(0);
            let checks = std::cell::Cell::new(0);
            let waited = exit_after_polls(
                || {
                    polls.set(polls.get() + 1);
                    results.next().expect("no poll after the outcome")
                },
                || {
                    checks.set(checks.get() + 1);
                    checks.get() > expired_after
                },
            );
            (waited.map_err(|e| e.kind()), polls.get())
        };
        let interrupted = || Ok(Polled::Interrupted);
        assert_eq!(
            run(vec![interrupted(), interrupted(), Ok(Polled::Event)], 5),
            (Ok(Waited::Exited), 3)
        );
        assert_eq!(
            run(vec![interrupted(), Ok(Polled::Timeout)], 5),
            (Ok(Waited::Deadline), 2)
        );
        assert_eq!(
            run(
                vec![
                    interrupted(),
                    interrupted(),
                    interrupted(),
                    Ok(Polled::Event)
                ],
                2
            ),
            (Ok(Waited::Deadline), 3)
        );
        assert_eq!(run(vec![Ok(Polled::Event)], 0), (Ok(Waited::Exited), 1));
        assert_eq!(
            run(vec![Err(std::io::ErrorKind::InvalidInput.into())], 5),
            (Err(std::io::ErrorKind::InvalidInput), 1)
        );
    }

    /// #171 TP8: a signal that interrupts the wait makes the caller poll again; any other failure of `kevent` fails the wait.
    #[test]
    fn an_interrupted_poll_is_polled_again_and_another_failure_fails_the_wait() {
        assert_eq!(polled(None).unwrap(), Polled::Timeout);
        let signal = Some(kqueue::Event {
            ident: kqueue::Ident::Fd(libc::SIGCHLD),
            data: kqueue::EventData::Signal(1),
        });
        assert_eq!(polled(signal).unwrap(), Polled::Event);
        assert_eq!(
            polled(failed(std::io::ErrorKind::Interrupted)).unwrap(),
            Polled::Interrupted
        );
        let error = polled(failed(std::io::ErrorKind::InvalidInput)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}

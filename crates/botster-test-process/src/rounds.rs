//! The cleanup policy of the guards: the rounds that end every member of a group, signalling the group only while a reserve
//! holds its id, and the outcome that an owner reports.
//!
//! **Why a reserve exists** (lead ruling 2026-10-08, amended anchor item 10). One kill of a group is not enough: on macOS a
//! child whose fork completes after the kill escapes it and stays in the group. So the kill is repeated, in rounds, until no
//! live member is left. A repeated kill must never reach another group that reused the id. The reserve is a child of the
//! killer that stays in the group, unreaped, while the killer has left the group and signals it from outside: while the
//! reserve is unreaped (live or a zombie), the group exists and no other group can take its id. (POSIX, "Process Group
//! Lifetime": a group lives until its last process leaves it or ends its lifetime, and a process lifetime ends only when its
//! status is waited for; the system does not reuse a process group ID during its lifetime.) The killer reaps only its
//! reserve, by that exact pid; it never waits for any pid, any group or a negative id, so production keeps the reaping of
//! its own children.

use crate::platform::{await_end, gone, live_members, Waited};
use crate::Deadline;
use rustix::process::Pid;

/// Why `end_members` stopped before the group was empty.
#[derive(Debug)]
pub enum Failure<M> {
    /// The members that were still live at the deadline, or a member that a wait called gone while a listing still called
    /// it live.
    Left(Vec<M>),
    /// A kill, a listing or a wait failed, so the rounds cannot go on.
    Error(std::io::Error),
}

impl<M: std::fmt::Display> Failure<M> {
    /// The report of the failure, naming the limit of the rounds.
    pub fn report(&self, limit: std::time::Duration) -> String {
        match self {
            Failure::Left(members) => {
                let left: Vec<String> = members.iter().map(ToString::to_string).collect();
                format!("members left after {limit:?}: {}", left.join(", "))
            }
            Failure::Error(error) => {
                format!("the members could not be signalled, listed or awaited: {error}")
            }
        }
    }
}

/// Whether a group kill was refused (EPERM): some member could not be signalled.
fn refused(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::PERM.raw_os_error())
}

/// The decision of the cleanup: each round lists the live members, kills the group and waits for the ends of the listed
/// members, so every awaited member was live at its listing and the kill came after it. A round that lists none is final,
/// because only a live member can fork a new one; a member that joins after a kill (a fork that completed after it) is in the
/// next round's list, and that round's kill reaches it.
///
/// No round repeats without a blocking wait or a change in the live set: a member that a wait calls gone twice while the
/// listings still call it live is reported as left. It stops at `expired` with the members left (after one last kill), and at
/// a failed kill, listing or wait with its error, so it never spins and never reports an end that did not happen.
///
/// # Errors
/// The members left, or the error that stopped the rounds.
pub fn end_members<M: PartialEq + Clone>(
    mut kill: impl FnMut() -> std::io::Result<()>,
    mut members: impl FnMut() -> std::io::Result<Vec<M>>,
    mut await_end: impl FnMut(&M) -> std::io::Result<Waited>,
    mut expired: impl FnMut() -> bool,
) -> Result<(), Failure<M>> {
    let mut gone_before = Vec::new();
    loop {
        let live = members().map_err(Failure::Error)?;
        if live.is_empty() {
            return Ok(());
        }
        if expired() {
            let _ = kill();
            return Err(Failure::Left(live));
        }
        // A kill that reaches no member is not a failure by itself; the next listing decides. ESRCH: they all ended since the
        // listing, and a BSD kernel skips the zombies left. EPERM: macOS refuses a group kill when a member cannot be
        // signalled, which a member that is already exiting is. A member that truly cannot be signalled stays listed live and
        // ends the rounds as left at the deadline, so the failure is still reported.
        match kill() {
            Err(error) if !gone(&error) && !refused(&error) => return Err(Failure::Error(error)),
            _ => {}
        }
        let mut gone_now = Vec::new();
        for member in &live {
            if await_end(member).map_err(Failure::Error)? == Waited::Gone {
                if gone_before.contains(member) {
                    return Err(Failure::Left(vec![member.clone()]));
                }
                gone_now.push(member.clone());
            }
        }
        gone_before = gone_now;
    }
}

/// Kills `group`, but only while `reserve` holds it: the reserve took the group at its fork, nothing can move a zombie to
/// another group, and while it is an unreaped child of this process (live or a zombie), the group exists and no other group
/// has the id. (macOS refuses `getpgid` for a zombie, so the check is the wait status, not the group.) Without the
/// reservation it sends nothing.
///
/// # Errors
/// The reservation is gone, or the kill failed (ESRCH when no member was left to signal).
pub fn reserved_kill(group: Pid, reserve: Pid) -> std::io::Result<()> {
    use rustix::process::{waitid, WaitId, WaitIdOptions};
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG;
    if let Err(error) = waitid(WaitId::Pid(reserve), options) {
        return Err(std::io::Error::other(format!(
            "the reserve {} of group {} is no longer an unreaped child: {error}",
            reserve.as_raw_nonzero(),
            group.as_raw_nonzero()
        )));
    }
    rustix::process::kill_process_group(group, rustix::process::Signal::KILL).map_err(Into::into)
}

/// Ends every member of `group`, the group of this process, within `deadline`: a reserve child holds the group, this process
/// leaves it, and the rounds of `end_members` kill it from outside. Then this process reaps its reserve, by its exact pid.
///
/// # Errors
/// What was left: the members still live at the deadline, or why they could not be signalled, listed or awaited.
pub fn end_group(group: Pid, deadline: Deadline) -> Result<(), String> {
    // The reserve inherits this process's group. Without it, the one kill left also ends this process, after the report.
    let reserve = std::process::Command::new("/usr/bin/true")
        .spawn()
        .and_then(|reserve| {
            rustix::process::setpgid(None, None)
                .map(|()| reserve)
                .map_err(Into::into)
        });
    let mut reserve = match reserve {
        Ok(reserve) => reserve,
        Err(error) => {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
            return Err(format!("the group guard cannot reserve the group: {error}"));
        }
    };
    let reserve_pid = match crate::platform::pid(reserve.id()) {
        Ok(pid) => pid,
        Err(error) => return Err(error.to_string()),
    };
    let ended = end_members(
        || reserved_kill(group, reserve_pid),
        || live_members(group),
        |member| await_end(member.pid, deadline),
        || deadline.expired(),
    );
    // The reserve has ended (by itself or by a kill); its reap, by its exact pid, gives the id back.
    let _ = reserve.wait();
    ended.map_err(|failure| failure.report(deadline.limit()))
}

/// Waits until `group` has no live member, within `deadline`, without a signal: the caller holds no reservation of the group,
/// so it must not signal it. It is for a group that production has killed: the rounds of `end_members` with no kill, so a
/// member that is still ending is awaited, never read as left.
///
/// # Errors
/// The members still live at the deadline, or why they could not be listed or awaited.
pub fn await_group_end(group: Pid, deadline: Deadline) -> Result<(), String> {
    end_members(
        || Ok(()),
        || live_members(group),
        |member| await_end(member.pid, deadline),
        || deadline.expired(),
    )
    .map_err(|failure| failure.report(deadline.limit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A group as the kernel keeps it, for the decision: a kill ends every member in the group at that moment, and a fork
    /// that was in progress completes just after the first kill, so its child joins the group then. A member ends only by a
    /// kill.
    struct ForkRace {
        members: Vec<i32>,
        ended: Vec<i32>,
        escaping: Option<i32>,
    }

    impl ForkRace {
        fn kill(&mut self) {
            self.ended.append(&mut self.members);
            self.members.extend(self.escaping.take());
        }
    }

    /// A member that joins the group after the first kill still ends: the next round lists it, and that round's kill ends
    /// it. Every awaited member has ended by a kill that came after its listing, so no wait runs out its deadline (each round
    /// lists, kills, then awaits).
    #[test]
    fn a_member_that_joins_after_the_first_kill_still_ends() {
        let (first, joiner) = (1, 2);
        let group = std::cell::RefCell::new(ForkRace {
            members: vec![first],
            ended: Vec::new(),
            escaping: Some(joiner),
        });
        let ended = end_members(
            || {
                group.borrow_mut().kill();
                Ok(())
            },
            || Ok(group.borrow().members.clone()),
            |pid: &i32| {
                assert!(
                    group.borrow().ended.contains(pid),
                    "member {pid} was awaited with no kill after its listing"
                );
                Ok(Waited::Exited)
            },
            || false,
        );
        assert!(ended.is_ok());
        let group = group.into_inner();
        assert!(group.members.is_empty());
        assert_eq!(group.ended, [first, joiner]);
    }

    /// At the deadline the rounds stop with the members that are left, never as a success, and the report names them.
    #[test]
    fn the_rounds_stop_at_the_deadline_with_the_members_left() {
        let member = 7;
        let mut rounds = 0;
        let mut kills = 0;
        let ended = end_members(
            || {
                kills += 1;
                Ok(())
            },
            || Ok(vec![member]),
            |_: &i32| Ok(Waited::Deadline),
            || {
                rounds += 1;
                rounds > 1
            },
        );
        assert_eq!(kills, 2, "one kill per round and one last kill");
        let Err(failure) = ended else {
            panic!("the deadline is a failure")
        };
        assert!(matches!(&failure, Failure::Left(left) if left == &[member]));
        let report = failure.report(std::time::Duration::from_secs(3));
        assert_eq!(report, "members left after 3s: 7");
    }

    /// A kill, a listing or a wait that fails stops the rounds with its error: they never spin and never report an end.
    #[test]
    fn a_failed_kill_listing_or_wait_stops_the_rounds_with_its_error() {
        let failed = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let kill: Result<(), Failure<i32>> = end_members(
            || Err(failed()),
            || Ok(vec![7]),
            |_| Ok(Waited::Exited),
            || false,
        );
        let Err(failure) = kill else {
            panic!("a failed kill is a failure")
        };
        assert!(failure
            .report(std::time::Duration::ZERO)
            .starts_with("the members could not be signalled, listed or awaited: "));
        let listing: Result<(), Failure<i32>> = end_members(
            || Ok(()),
            || Err(failed()),
            |_| Ok(Waited::Exited),
            || false,
        );
        assert!(matches!(listing, Err(Failure::Error(_))));
        let wait = end_members(|| Ok(()), || Ok(vec![7]), |_: &i32| Err(failed()), || false);
        assert!(matches!(wait, Err(Failure::Error(_))));
    }

    /// A kill that reaches no member is not a failure: ESRCH (the members ended after the listing) or EPERM (macOS refuses a
    /// group kill while a member is exiting). The next listing is empty, and the cleanup succeeded.
    #[test]
    fn a_kill_that_reaches_no_member_lets_the_next_listing_decide() {
        let member = 7;
        for errno in [rustix::io::Errno::SRCH, rustix::io::Errno::PERM] {
            let mut listings = std::collections::VecDeque::from([vec![member], vec![]]);
            let ended = end_members(
                || Err(std::io::Error::from_raw_os_error(errno.raw_os_error())),
                || Ok(listings.pop_front().expect("no listing after an empty one")),
                |_: &i32| Ok(Waited::Gone),
                || false,
            );
            assert!(ended.is_ok(), "{errno}");
            assert!(listings.is_empty());
        }
    }

    /// A member that a wait calls gone while the listings still call it live (a process held in its exit) does not make the
    /// rounds spin: the second time, the rounds stop and report it as left. One gone answer alone is not a failure.
    #[test]
    fn a_member_gone_to_its_wait_but_still_listed_is_left() {
        let held = 7;
        let mut rounds = 0;
        let ended = end_members(
            || Ok(()),
            || {
                rounds += 1;
                assert!(rounds <= 2, "a third round would spin");
                Ok(vec![held])
            },
            |_: &i32| Ok(Waited::Gone),
            || false,
        );
        assert!(matches!(ended, Err(Failure::Left(left)) if left == [held]));
        let mut listings = std::collections::VecDeque::from([vec![held], vec![held], vec![]]);
        let mut waits = std::collections::VecDeque::from([Waited::Gone, Waited::Exited]);
        let ended = end_members(
            || Ok(()),
            || Ok(listings.pop_front().expect("a listing")),
            |_: &i32| Ok(waits.pop_front().expect("a wait")),
            || false,
        );
        assert!(ended.is_ok());
    }

    /// An empty group needs no kill and no wait.
    #[test]
    fn an_empty_group_ends_at_once_with_no_kill() {
        let ended: Result<(), Failure<i32>> = end_members(
            || panic!("no kill of an empty group"),
            || Ok(Vec::new()),
            |_| panic!("no wait"),
            || panic!("no deadline check"),
        );
        assert!(ended.is_ok());
    }
}

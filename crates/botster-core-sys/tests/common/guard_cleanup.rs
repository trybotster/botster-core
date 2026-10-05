//! The cleanup policy of the test guards: the rounds that end every member of a group, signalling the group only while a
//! reserve holds its id, and the outcome that the guard reports. The platform adapters are in `guard_platform.rs`.

use std::io::Write;

#[path = "guard_platform.rs"]
pub(crate) mod platform;

use platform::{await_end, gone, live_members, Waited};

/// The limit of the anchor's cleanup: the guards' cleanup limit. Not a contract value.
pub(crate) const CLEANUP: std::time::Duration = std::time::Duration::from_secs(10);

/// Ends every member of `group`, the group of this process, within `cleanup`. One kill of the group is not enough: on
/// macOS a child whose fork completes after the kill escapes it and stays in the group. So the kill is repeated until no
/// live member is left.
///
/// Each kill is a signal to the group, never to a pid, so it reaches members only. The group id stays reserved through
/// every signal: a child of this process stays in the group, unreaped, while this process leaves the group and signals it
/// from outside, so no other group can take the id before the end. (POSIX, "Process Group Lifetime": a group lives until
/// its last process leaves it or ends its process lifetime, and a process lifetime ends only when its status is waited
/// for; the system does not reuse a process group ID during its lifetime.)
///
/// # Errors
/// What was left: the members still live at the deadline, or why they could not be signalled, listed or awaited.
pub(crate) fn end_group(
    group: rustix::process::Pid,
    cleanup: std::time::Duration,
) -> Result<(), String> {
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
            let report = format!("the group guard cannot reserve the group: {error}");
            let _ = writeln!(std::io::stderr(), "{report}");
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
            return Err(report);
        }
    };
    let reserve_pid = rustix::process::Pid::from_raw(reserve.id() as i32).expect("a child pid");
    let kill = || reserved_kill(group, reserve_pid);
    // timer: deadline — bounds the anchor's cleanup; a member that never ends cannot hold the anchor forever.
    let deadline = std::time::Instant::now() + cleanup;
    let ended = end_members(
        kill,
        || live_members(group),
        |member| await_end(member.pid, deadline),
        || std::time::Instant::now() >= deadline,
    );
    // The reserve has ended (by itself or by a kill); its reap gives the id back.
    let _ = reserve.wait();
    ended.map_err(|failure| match failure {
        Failure::Left(members) => {
            let left: Vec<String> = members.iter().map(ToString::to_string).collect();
            format!("members left after {cleanup:?}: {}", left.join(", "))
        }
        Failure::Error(error) => {
            format!("the members could not be signalled, listed or awaited: {error}")
        }
    })
}

/// Kills `group`, but only while `reserve` holds it: the reserve took the group at its fork, nothing can move a zombie to
/// another group, and while it is an unreaped child of this process (live or a zombie), the group exists and no other
/// group has the id. (macOS refuses `getpgid` for a zombie, so the check is the wait status, not the group.) Without the
/// reservation it sends nothing.
///
/// # Errors
/// The reservation is gone, or the kill failed (ESRCH when no member was left to signal).
pub(crate) fn reserved_kill(
    group: rustix::process::Pid,
    reserve: rustix::process::Pid,
) -> std::io::Result<()> {
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

/// Why `end_members` stopped before the group was empty.
#[derive(Debug)]
enum Failure<M> {
    /// The members that were still live at the deadline, or a member that a wait called gone while a listing still
    /// called it live.
    Left(Vec<M>),
    /// A kill, a listing or a wait failed, so the rounds cannot go on.
    Error(std::io::Error),
}

/// The decision of `end_group`: each round lists the live members, kills the group and waits for the ends of the listed
/// members, so every awaited member was live at its listing and the kill came after it. A round that lists none is final,
/// because only a live member can fork a new one; a member that joins after a kill (a fork that completed after it) is in
/// the next round's list, and that round's kill reaches it.
///
/// No round repeats without a blocking wait or a change in the live set: a member that a wait calls gone twice while the
/// listings still call it live is reported as left. It stops at `expired` with the members left (after one last kill), and
/// at a failed kill, listing or wait with its error, so it never spins and never reports an end that did not happen.
fn end_members<M: PartialEq + Clone>(
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
        // A kill that finds no member to signal (ESRCH: they all ended since the listing, and a BSD kernel skips the
        // zombies left) is not a failure; the next listing decides.
        match kill() {
            Err(error) if !gone(&error) => return Err(Failure::Error(error)),
            _ => {}
        }
        let mut gone = Vec::new();
        for member in &live {
            if await_end(member).map_err(Failure::Error)? == Waited::Gone {
                if gone_before.contains(member) {
                    return Err(Failure::Left(vec![member.clone()]));
                }
                gone.push(member.clone());
            }
        }
        gone_before = gone;
    }
}

/// A group as the kernel keeps it, for the decision: a kill ends every member in the group at that moment, and a fork that
/// was in progress completes just after the first kill, so its child joins the group then. A member ends only by a kill.
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

/// A member that joins the group after the first kill still ends: the next round lists it, and that round's kill ends it.
/// Every awaited member has ended by a kill that came after its listing, so no wait runs out its deadline (each round
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

/// At the deadline the rounds stop with the members that are left, never as a success.
#[test]
fn the_rounds_stop_at_the_deadline_with_the_members_left() {
    let member = 7;
    let mut rounds = 0;
    let ended = end_members(
        || Ok(()),
        || Ok(vec![member]),
        |_: &i32| Ok(Waited::Deadline),
        || {
            rounds += 1;
            rounds > 1
        },
    );
    assert!(matches!(ended, Err(Failure::Left(left)) if left == [member]));
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
    assert!(matches!(kill, Err(Failure::Error(_))));
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

/// A kill that finds no member to signal (ESRCH: they ended after the listing) is not a failure: the next listing is empty,
/// and the cleanup succeeded.
#[test]
fn a_kill_that_finds_no_member_lets_the_next_listing_decide() {
    let member = 7;
    let mut listings = std::collections::VecDeque::from([vec![member], vec![]]);
    let ended = end_members(
        || {
            Err(std::io::Error::from_raw_os_error(
                rustix::io::Errno::SRCH.raw_os_error(),
            ))
        },
        || Ok(listings.pop_front().expect("no listing after an empty one")),
        |_: &i32| Ok(Waited::Gone),
        || false,
    );
    assert!(ended.is_ok());
    assert!(listings.is_empty());
}

/// A member that a wait calls gone while the listings still call it live (a process held in its exit) does not make the
/// rounds spin: the second time, the rounds stop and report it as left.
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
}

/// The reservation through the real kernel: the kill goes out while the reserve is an unreaped child, live or a zombie, and
/// once the reserve is reaped (the id may be reused) nothing is sent, whatever group has the id by then.
#[test]
fn a_kill_goes_out_only_while_the_reserve_holds_the_group() {
    use std::os::unix::process::CommandExt;
    let mut reserve = std::process::Command::new("/usr/bin/true")
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(reserve.id() as i32).unwrap();
    // The reserve leads its own group, so the group id is its pid.
    let held = reserved_kill(pid, pid);
    assert!(
        held.is_ok() || held.as_ref().is_err_and(platform::gone),
        "a held group is signalled: {held:?}"
    );
    reserve.wait().unwrap();
    let released = reserved_kill(pid, pid).expect_err("a reaped reserve holds nothing");
    assert!(!platform::gone(&released), "no signal is sent: {released}");
    assert!(released.to_string().contains("no longer an unreaped child"));
}

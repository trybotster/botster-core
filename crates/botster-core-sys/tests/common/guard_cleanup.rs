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
            // `group` is our own group: the last kill ends this process too.
            let _ = botster_core_sys::signal::signal_own_group(rustix::process::Signal::KILL);
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

/// Whether a group kill was refused (EPERM): some member could not be signalled.
fn refused(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::PERM.raw_os_error())
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
    botster_core_sys::signal::signal_group(
        group.as_raw_nonzero().get().unsigned_abs(),
        rustix::process::Signal::KILL,
    )
    .map_err(Into::into)
}

/// Why `end_members` stopped before the group was empty.
#[derive(Debug)]
pub(crate) enum Failure<M> {
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
pub(crate) fn end_members<M: PartialEq + Clone>(
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
        // A kill that reaches no member is not a failure by itself; the next listing decides. ESRCH: they all ended since
        // the listing, and a BSD kernel skips the zombies left. EPERM: macOS refuses a group kill when a member cannot be
        // signalled, which a member that is already exiting is. A member that truly cannot be signalled stays listed live
        // and ends the rounds as left at the deadline, so the failure is still reported.
        match kill() {
            Err(error) if !gone(&error) && !refused(&error) => return Err(Failure::Error(error)),
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

/// Whether the child `pid` of this process ended within `limit`, observed without reaping it (`WNOWAIT`), so a timeout
/// leaves the child to its owner. Only an interrupted wait is repeated.
///
/// # Errors
/// The observation failed: the child cannot be waited for.
fn ended_within(pid: rustix::process::Pid, limit: std::time::Duration) -> std::io::Result<bool> {
    use rustix::process::{waitid, WaitId, WaitIdOptions};
    let (ended, end) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let observed = loop {
            match waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
            ) {
                Err(rustix::io::Errno::INTR) => continue,
                Ok(Some(_)) => break Ok(()),
                // A blocking wait returns a status; no status is not an exit.
                Ok(None) => break Err(std::io::Error::other("the wait returned no status")),
                Err(error) => break Err(std::io::Error::from(error)),
            }
        };
        let _ = ended.send(observed);
    });
    // timer: deadline — bounds the wait for a test child's end.
    match end.recv_timeout(limit) {
        Ok(observed) => observed.map(|()| true),
        Err(_) => Ok(false),
    }
}

/// A failure of a test child's ownership: it fails the test, or is reported when the test already panics.
fn ownership_failed(report: String) {
    if std::thread::panicking() {
        eprintln!("{report}");
    } else {
        panic!("{report}");
    }
}

/// A child of this test, owned on every path: `status` reaps it only after its exit was observed within the cleanup
/// limit, and its drop ends a child that still runs (a kill of this unreaped child's own pid) and reaps it after its exit,
/// within the same limit. Every ownership or cleanup error is reported, never taken as an end.
pub(crate) struct Owned(pub(crate) std::process::Child);

impl Owned {
    fn pid(&self) -> rustix::process::Pid {
        rustix::process::Pid::from_raw(self.0.id() as i32).expect("a child pid")
    }

    /// The child's exit status, once its exit was observed within the cleanup limit.
    pub(crate) fn status(&mut self) -> std::process::ExitStatus {
        self.status_within(CLEANUP)
    }

    /// The child's exit status, once its exit was observed within `limit`; a child that does not end fails the test.
    fn status_within(&mut self, limit: std::time::Duration) -> std::process::ExitStatus {
        match ended_within(self.pid(), limit) {
            Ok(true) => self.0.wait().expect("the reap of an exited child"),
            Ok(false) => panic!(
                "the test child {} did not end within {limit:?}",
                self.0.id()
            ),
            Err(error) => panic!("the test child {} cannot be observed: {error}", self.0.id()),
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        let id = self.0.id();
        match self.0.try_wait() {
            // Reaped by `status`, or ended and reaped now.
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => {
                return ownership_failed(format!("the test child {id} cannot be checked: {error}"))
            }
        }
        if let Err(error) = self.0.kill() {
            return ownership_failed(format!("the test child {id} cannot be killed: {error}"));
        }
        match ended_within(self.pid(), CLEANUP) {
            Ok(true) => {
                if let Err(error) = self.0.wait() {
                    ownership_failed(format!("the test child {id} cannot be reaped: {error}"));
                }
            }
            Ok(false) => ownership_failed(format!("the test child {id} did not end after SIGKILL")),
            Err(error) => {
                ownership_failed(format!("the test child {id} cannot be observed: {error}"))
            }
        }
    }
}

/// The reservation through the real kernel, observed by the members' exit statuses. While the reserve is an unreaped
/// child, the kill reaches the group: its member ends by SIGKILL. Once the reserve is reaped, the kill sends nothing: a
/// member that still holds the group ends normally when the test lets it, so no signal reached it. Every child is owned
/// on every path, and every wait is bounded.
#[test]
fn a_kill_goes_out_only_while_the_reserve_holds_the_group() {
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let never = dir.path().join("never");
    assert!(Command::new("/usr/bin/mkfifo")
        .arg(&never)
        .status()
        .unwrap()
        .success());
    // A member blocks without CPU in the open of a FIFO that nothing writes, until a writer opens it or a signal ends it.
    let member = |group: i32| {
        Owned(
            Command::new("/bin/cat")
                .arg(&never)
                .stdout(Stdio::null())
                .process_group(group)
                .spawn()
                .unwrap(),
        )
    };
    let kill_signal = rustix::process::Signal::KILL.as_raw();

    // Held: the reserve leads the group, and the kill ends its member.
    let mut reserve = member(0);
    let group = reserve.pid();
    let mut held = member(group.as_raw_nonzero().get());
    reserved_kill(group, group).unwrap();
    assert_eq!(
        held.status().signal(),
        Some(kill_signal),
        "the held group was signalled"
    );
    reserve.status();

    // Released: the reserve alone ends and is reaped while a member still holds the group, and the kill sends nothing.
    let mut reserve = member(0);
    let group = reserve.pid();
    let mut alive = member(group.as_raw_nonzero().get());
    reserve.0.kill().unwrap();
    reserve.status();
    let refused = reserved_kill(group, group).expect_err("a reaped reserve holds nothing");
    assert!(
        refused.to_string().contains("no longer an unreaped child"),
        "{refused}"
    );
    // A writer lets the member read the end of the FIFO: it ends normally, so the refused kill reached nothing. The open
    // meets the member's open; it runs on a helper thread with the cleanup limit, so a member that is gone fails the test
    // instead of blocking it.
    let fifo = never.clone();
    let (opened, writer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = opened.send(std::fs::OpenOptions::new().write(true).open(fifo));
    });
    let writer = writer
        // timer: deadline — bounds the meeting with the member at its FIFO.
        .recv_timeout(CLEANUP)
        .expect("the member still reads the FIFO");
    drop(writer.unwrap());
    assert_eq!(
        alive.status().code(),
        Some(0),
        "no signal reached the member"
    );
}

/// A child that does not end fails the test with a clear message within the limit, never a hang; its owner still ends it.
/// The stuck child blocks without CPU in the open of a FIFO that nothing writes.
#[test]
fn a_stuck_child_fails_its_wait_and_is_still_ended() {
    let dir = tempfile::tempdir().unwrap();
    let never = dir.path().join("never");
    assert!(std::process::Command::new("/usr/bin/mkfifo")
        .arg(&never)
        .status()
        .unwrap()
        .success());
    let mut stuck = Owned(
        std::process::Command::new("/bin/cat")
            .arg(&never)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stuck.status_within(std::time::Duration::ZERO)
    }))
    .expect_err("a stuck child fails its wait");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("did not end within"), "{report}");
    // The owner's drop kills and reaps it; a drop that could not would fail this test.
    drop(stuck);
}

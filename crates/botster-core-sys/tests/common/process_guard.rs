//! Test ownership of a process group, independent of the production reaper.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The anchor joins the worker's group before the worker body runs.
/// The anchor stays alive after the production reaper reaps the worker.
/// Closing the control stream makes the anchor kill its group, including itself.
pub struct GroupGuard {
    control: UnixStream,
    anchor: Child,
    registration: Option<std::thread::JoinHandle<()>>,
    socket: PathBuf,
}

fn helper(name: &str) -> String {
    let module = module_path!().split_once("::").unwrap().1;
    format!("{module}::{name}")
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

impl GroupGuard {
    /// Creates ownership before the production spawn can run.
    pub fn new(dir: &Path) -> Self {
        Self::with_cleanup(dir, CLEANUP)
    }

    /// A guard whose anchor ends the group within `cleanup` (the failure test gives it no time).
    pub fn with_cleanup(dir: &Path, cleanup: std::time::Duration) -> Self {
        let socket = dir.join("guard.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let (control, child_control) = UnixStream::pair().unwrap();
        let input: OwnedFd = child_control.try_clone().unwrap().into();
        let output: OwnedFd = child_control.into();
        let anchor = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &helper("anchor_process"), "--nocapture"])
            .env("BOTSTER_TEST_ANCHOR", "1")
            .env(
                "BOTSTER_TEST_ANCHOR_CLEANUP_MS",
                cleanup.as_millis().to_string(),
            )
            .stdin(Stdio::from(input))
            .stderr(Stdio::from(output))
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let mut registration_control = control.try_clone().unwrap();
        let registration = std::thread::spawn(move || {
            let Ok((mut worker, _)) = listener.accept() else {
                return;
            };
            let mut pid = String::new();
            if BufReader::new(&worker).read_line(&mut pid).is_err() || pid.is_empty() {
                return;
            }
            if registration_control.write_all(pid.as_bytes()).is_err() {
                return;
            }
            let mut ready = [0];
            if registration_control.read_exact(&mut ready).is_ok() && ready == [1] {
                let _ = worker.write_all(&ready);
            }
        });
        Self {
            control,
            anchor,
            registration: Some(registration),
            socket,
        }
    }

    /// The shell waits for registration before it starts any blocking command.
    pub fn prefix(&self) -> String {
        format!(
            "BOTSTER_TEST_GROUP_SOCKET={} BOTSTER_TEST_GROUP_PID=$$ {} --exact {} --nocapture >/dev/null 2>/dev/null || exit 1\n",
            quoted(&self.socket),
            quoted(&std::env::current_exe().unwrap()),
            helper("register_worker")
        )
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        // Shutdown also closes the writing side of the registration thread's clone.
        let _ = self.control.shutdown(std::net::Shutdown::Write);
        // Release accept if no worker reached registration.
        if let Ok(stream) = UnixStream::connect(&self.socket) {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(thread) = self.registration.take() {
            let _ = thread.join();
        }
        // The anchor ends with success only when no member of the group is left; otherwise it wrote why on its stderr,
        // which is this end of the control stream.
        let status = self.anchor.wait();
        if !matches!(&status, Ok(status) if status.success()) {
            let mut report = String::new();
            let _ = self.control.read_to_string(&mut report);
            let report = format!(
                "the group guard's cleanup failed ({status:?}): {}",
                report.trim_start_matches('\u{1}').trim()
            );
            if std::thread::panicking() {
                eprintln!("{report}");
            } else {
                panic!("{report}");
            }
        }
    }
}

/// A separate test process holds membership in the worker's group.
#[test]
fn anchor_process() {
    if std::env::var_os("BOTSTER_TEST_ANCHOR").is_none() {
        return;
    }
    let mut input = BufReader::new(std::io::stdin());
    let mut pid = String::new();
    if input.read_line(&mut pid).unwrap() == 0 {
        return;
    }
    let group = rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap();
    rustix::process::setpgid(None, Some(group)).unwrap();
    let _ = std::io::stderr().write_all(&[1]);
    // EOF is the test's Drop or death. No timer or parent-PID check is needed.
    let mut remaining = Vec::new();
    let _ = input.read_to_end(&mut remaining);
    let cleanup = std::env::var("BOTSTER_TEST_ANCHOR_CLEANUP_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(CLEANUP, std::time::Duration::from_millis);
    if let Err(report) = end_group(group, cleanup) {
        let _ = writeln!(std::io::stderr(), "{report}");
        std::process::exit(1);
    }
}

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
    // Every kill first checks the reservation: the reserve took this process's group at its fork, nothing can move a
    // zombie to another group, and while it is an unreaped child of this process (live or a zombie), the group exists and
    // no other group has the id. (macOS refuses `getpgid` for a zombie, so the check is the wait status, not the group.)
    let kill = || {
        use rustix::process::{waitid, WaitId, WaitIdOptions};
        let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG;
        if let Err(error) = waitid(WaitId::Pid(reserve_pid), options) {
            return Err(std::io::Error::other(format!(
                "the reserve {} of group {} is no longer an unreaped child: {error}",
                reserve_pid.as_raw_nonzero(),
                group.as_raw_nonzero()
            )));
        }
        rustix::process::kill_process_group(group, rustix::process::Signal::KILL)
            .map_err(Into::into)
    };
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

/// Why `end_members` stopped before the group was empty.
#[derive(Debug)]
enum Failure<M> {
    /// The members that were still live at the deadline, or a member that a wait called gone while a listing still
    /// called it live.
    Left(Vec<M>),
    /// A kill, a listing or a wait failed, so the rounds cannot go on.
    Error(std::io::Error),
}

/// How a wait for a member's end came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waited {
    /// Its exit event came.
    Exited,
    /// It was gone before the wait began.
    Gone,
    /// The deadline came first.
    Deadline,
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

/// A live member of the group, with its state for a report. Two are the same member when their pids are the same.
#[derive(Clone)]
struct Member {
    pid: rustix::process::Pid,
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
fn gone(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error())
}

/// Waits for the exit event of `pid` (it may stay a zombie), at most until `deadline`. It only observes: a pid that was
/// reused meanwhile can make it wait for another process, within the deadline.
///
/// # Errors
/// The wait could not be set up, or it failed.
#[cfg(target_os = "macos")]
fn await_end(pid: rustix::process::Pid, deadline: std::time::Instant) -> std::io::Result<Waited> {
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
fn await_end(pid: rustix::process::Pid, deadline: std::time::Instant) -> std::io::Result<Waited> {
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
fn live_members(group: rustix::process::Pid) -> std::io::Result<Vec<Member>> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;
    use libproc::processes::{pids_by_type, ProcFilter};
    let group_id = group.as_raw_nonzero().get() as u32;
    let mut members = Vec::new();
    for pid in pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group_id })? {
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
fn live_members(group: rustix::process::Pid) -> std::io::Result<Vec<Member>> {
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
        if state != "Z" && pgrp.parse::<i32>().ok() == Some(group.as_raw_nonzero().get()) {
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

/// At the deadline the rounds stop with the members that are left, after one last kill, never as a success.
#[test]
fn the_rounds_stop_at_the_deadline_with_the_members_left() {
    let (member, rounds) = (7, 2);
    let mut checks = 0;
    let mut kills = 0;
    let ended = end_members(
        || {
            kills += 1;
            Ok(())
        },
        || Ok(vec![member]),
        |_: &i32| Ok(Waited::Deadline),
        || {
            checks += 1;
            checks > rounds
        },
    );
    assert!(matches!(ended, Err(Failure::Left(left)) if left == [member]));
    assert_eq!(kills, rounds + 1, "each round's kill and the last kill");
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

/// A cleanup that cannot finish fails the test through the real guard: with no time for its rounds, the anchor reports
/// the live member, and the guard's drop fails with that report. The member still ends, by the anchor's last kill.
#[test]
fn a_cleanup_that_cannot_finish_fails_through_the_guard() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let never = dir.path().join("never");
    assert!(Command::new("/usr/bin/mkfifo")
        .arg(&never)
        .status()
        .unwrap()
        .success());
    let guard = GroupGuard::with_cleanup(dir.path(), std::time::Duration::ZERO);
    // The member blocks without CPU on a FIFO that nothing opens for writing, and holds the pipe until it ends.
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "{}/bin/echo up; exec /bin/cat {} >/dev/null",
                guard.prefix(),
                quoted(&never)
            ),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pipe = child.stdout.take().unwrap();
    let (pipe, line) = first_line(pipe);
    assert_eq!(line, "up\n");
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(guard)))
        .expect_err("the guard reports the cleanup failure");
    let report = failed.downcast_ref::<String>().expect("a report").clone();
    assert!(report.contains("members left"), "{report}");
    eof(pipe);
    child.wait().unwrap();
}

/// The first line of `reader`, read with the cleanup deadline.
fn first_line(reader: impl Read + Send + 'static) -> (BufReader<Box<dyn Read + Send>>, String) {
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(Box::new(reader) as Box<dyn Read + Send>);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| (reader, line));
        let _ = sent.send(result);
    });
    received
        // timer: deadline — bounds the wait for a member's first line.
        .recv_timeout(CLEANUP)
        .expect("the member wrote its first line")
        .unwrap()
}

#[test]
fn register_worker() {
    let Some(socket) = std::env::var_os("BOTSTER_TEST_GROUP_SOCKET") else {
        return;
    };
    let registered = (|| -> std::io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        writeln!(
            stream,
            "{}",
            std::env::var("BOTSTER_TEST_GROUP_PID").unwrap()
        )?;
        let mut ready = [0];
        stream.read_exact(&mut ready)?;
        if ready != [1] {
            return Err(std::io::Error::other("the group has no anchor"));
        }
        Ok(())
    })();
    if registered.is_err() {
        std::process::exit(1);
    }
}

struct Parent(Option<Child>);

impl Drop for Parent {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn eof(reader: impl Read + Send + 'static) {
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut rest = Vec::new();
        let result = reader.read_to_end(&mut rest);
        let _ = sent.send(result);
    });
    received
        // timer: deadline — bounds cleanup when a guard fails
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("all descendants closed the pipe")
        .unwrap();
}

/// The parent dies while the worker has no FIFO reader.
#[test]
fn parent_dies_before_fifo_reader() {
    for handoff in ["ready", "launch"] {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("fifo");
        assert!(Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        let mut parent = Parent(Some(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &helper("blocked_parent"), "--nocapture"])
                .env("BOTSTER_TEST_PARENT_DIR", dir.path())
                .env("BOTSTER_TEST_HANDOFF", handoff)
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        ));
        let child = parent.0.as_mut().unwrap();
        let mut reader = BufReader::new(child.stderr.take().unwrap());
        let mut pid = String::new();
        reader.read_line(&mut pid).unwrap();
        assert!(
            pid.trim().parse::<u32>().is_ok(),
            "the worker reached its FIFO: {pid}"
        );
        child.kill().unwrap();
        // Drop reaps only the parent. The parent cannot run its group guard.
        drop(parent);
        eof(reader);
    }
}

#[test]
fn blocked_parent() {
    use std::os::unix::process::CommandExt;
    let Some(dir) = std::env::var_os("BOTSTER_TEST_PARENT_DIR") else {
        return;
    };
    let dir = Path::new(&dir);
    let guard = GroupGuard::new(dir);
    let write = if std::env::var("BOTSTER_TEST_HANDOFF").unwrap() == "launch" {
        format!(
            "/bin/echo launch | /usr/bin/tee {} >/dev/null",
            quoted(&dir.join("fifo"))
        )
    } else {
        format!("/bin/echo ready > {}", quoted(&dir.join("fifo")))
    };
    let mut worker = Command::new("/bin/sh")
        .args(["-c", &format!("{}echo $$ >&2; {write}", guard.prefix())])
        .process_group(0)
        .spawn()
        .unwrap();
    // The outer test kills this parent while the child holds the stderr pipe.
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    drop(guard);
    worker.wait().unwrap();
}

/// Panic cleanup starts before any readiness indication exists.
#[test]
fn a_panic_before_ready_ends_the_child() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let guard = GroupGuard::new(dir.path());
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!("{}while :; do /bin/sleep 1; done", guard.prefix()),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pipe = child.stdout.take().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("the test failed before ready");
    }));
    assert!(result.is_err());
    eof(pipe);
    assert!(!child.wait().unwrap().success());
}

/// An anchor keeps the group after another owner reaps the leader.
#[test]
fn an_early_exit_keeps_the_group_owned_until_cleanup() {
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let guard = GroupGuard::new(dir.path());
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "{}(while :; do /bin/sleep 1; done) & echo $!; exit",
                guard.prefix()
            ),
        ])
        .stdout(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let group = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
    let mut pipe = BufReader::new(child.stdout.take().unwrap());
    let mut descendant = String::new();
    pipe.read_line(&mut descendant).unwrap();
    assert!(descendant.trim().parse::<u32>().is_ok());
    assert!(child.wait().unwrap().success());
    let anchor = rustix::process::Pid::from_raw(guard.anchor.id() as i32).unwrap();
    assert_eq!(rustix::process::getpgid(Some(anchor)).unwrap(), group);
    drop(guard);
    eof(pipe);
}

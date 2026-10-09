//! The anchor binary of the group guard: `botster-test-anchor wrap <socket> <grace ns> <cleanup ns> <program> [args...]`. The stages and
//! the protocol are in `botster_test_process::anchor`.

use botster_test_process::anchor::{
    identity, send, start_anchor, stayed, verdict, Identity, Line, Report, WRAP_FAILED,
};
use botster_test_process::platform::{await_end, live_members, pid, signal_target, start_time};
use botster_test_process::rounds::{end_group, end_members};
use botster_test_process::Deadline;
use rustix::process::{getpgid, getpgrp, kill_process_group, Signal};
use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

/// The signals that the anchor survives. A no-op handler keeps it alive (the workspace forbids `unsafe`, so no `SIG_IGN`):
/// `TERM` from production's group signal and from its own cleanup, `HUP` when the leader of its session exits, the
/// job-control and keyboard signals that a terminal sends to its foreground group, and the user signals. `KILL` and `STOP`
/// cannot be caught.
const SURVIVED: [i32; 10] = [
    signal_hook::consts::SIGTERM,
    signal_hook::consts::SIGHUP,
    signal_hook::consts::SIGINT,
    signal_hook::consts::SIGQUIT,
    signal_hook::consts::SIGTSTP,
    signal_hook::consts::SIGTTIN,
    signal_hook::consts::SIGTTOU,
    signal_hook::consts::SIGUSR1,
    signal_hook::consts::SIGUSR2,
    signal_hook::consts::SIGALRM,
];

fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn group_now() -> u32 {
    getpgrp().as_raw_nonzero().get().unsigned_abs()
}

fn number(value: Option<&OsString>) -> io::Result<u64> {
    value
        .and_then(|v| v.to_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| other("an argument is not a number"))
}

/// Wrap: everything before the exec. It returns only when it fails.
fn wrap(args: &[OsString]) -> io::Result<std::convert::Infallible> {
    let [socket, grace, cleanup, program, rest @ ..] = args else {
        return Err(other(
            "wrap takes the socket, the grace, the cleanup bound and the program",
        ));
    };
    let mut guard = UnixStream::connect(socket)?;
    let result = (|| -> io::Result<()> {
        let grace = number(Some(grace))?;
        let cleanup = number(Some(cleanup))?;
        let leader = identity(std::process::id())?;
        // The guard connection is the intermediate's stdin and stderr: the anchor inherits both. The pipe of stdout carries
        // the anchor's one acknowledgement back here.
        let input: OwnedFd = guard.try_clone()?.into();
        let output: OwnedFd = guard.try_clone()?.into();
        let mut intermediate = Command::new(std::env::current_exe()?);
        intermediate
            .args([
                "intermediate".to_string(),
                leader.pid.to_string(),
                leader.start_time.to_string(),
                group_now().to_string(),
                grace.to_string(),
                cleanup.to_string(),
            ])
            .stdin(Stdio::from(input))
            .stderr(Stdio::from(output));
        // The intermediate is the only child of this process: owned, and reaped by its exact pid within the cleanup bound
        // on every path. The real program inherits no child.
        start_anchor(&mut intermediate, Deadline::cleanup())
    })();
    if let Err(error) = result {
        let _ = send(
            &mut guard,
            &Line::Error {
                stage: "wrap".into(),
                reason: error.to_string(),
            },
        );
        return Err(error);
    }
    // The guard connection and the pipe are close-on-exec: the real program inherits neither.
    let error = Command::new(program).args(rest).exec();
    let _ = send(
        &mut guard,
        &Line::Error {
            stage: "wrap".into(),
            reason: format!("exec {}: {error}", program.to_string_lossy()),
        },
    );
    Err(error)
}

/// Intermediate: starts the anchor with the inherited descriptors 0 to 2 and exits without waiting for it.
fn intermediate(args: &[OsString]) -> io::Result<()> {
    close_inherited()?;
    Command::new(std::env::current_exe()?)
        .arg("anchor")
        .args(args)
        .spawn()?;
    Ok(())
}

/// The directory that lists this process's open descriptors.
#[cfg(target_os = "linux")]
const OPEN_DESCRIPTORS: &str = "/proc/self/fd";
#[cfg(target_os = "macos")]
const OPEN_DESCRIPTORS: &str = "/dev/fd";

/// Closes every descriptor above 2 that this helper stage inherited (#171 TP3; lead ruling 2026-10-09, (A)). Production's
/// descriptors that lack close-on-exec reach the wrapper for the real program, and through it the intermediate; the anchor
/// lives for the whole test, so a pipe or FIFO writer that it held would keep production's reader from its end of file. It is
/// the first act of the intermediate and anchor stages. It lists the open descriptors, closes each one above 2 but the
/// listing's own, and closes the listing last.
///
/// # Errors
/// The listing failed, or a close failed with an error other than EBADF.
#[allow(unsafe_code)]
fn close_inherited() -> io::Result<()> {
    use rustix::fs::{Mode, OFlags};
    use std::os::fd::AsRawFd;
    let listing = rustix::fs::open(
        OPEN_DESCRIPTORS,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let own = listing.as_raw_fd();
    let mut descriptors = Vec::new();
    let mut entries = rustix::fs::Dir::new(listing)?;
    for entry in entries.by_ref() {
        let entry = entry?;
        let Some(fd) = entry
            .file_name()
            .to_str()
            .ok()
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        if fd > 2 && fd != own {
            descriptors.push(fd);
        }
    }
    for fd in descriptors {
        // SAFETY: this stage owns no descriptor yet (this is its first act, and it starts no thread), so nothing in this
        // process aliases the closed descriptors; the listing's own descriptor is excluded, and it is closed last, by its
        // drop. A descriptor that the listing named but that is closed meanwhile gives EBADF, which is ignored.
        if unsafe { libc::close(fd) } == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EBADF) {
                return Err(error);
            }
        }
    }
    drop(entries);
    Ok(())
}

/// Whether the group may be signalled now: the anchor is still a member of it, and the leader, while it lives, has not moved
/// to another group (ruling item 13). An identity or a group that cannot be read refuses (`verdict`).
fn verify(report: &Report) -> Result<(), String> {
    let leader = pid(report.leader.pid).map_err(|e| e.to_string())?;
    verdict(report, group_now(), start_time(leader), || {
        getpgid(Some(leader)).map(|group| group.as_raw_nonzero().get().unsigned_abs())
    })
}

/// The group of the process `member` now.
fn group_of(member: u32) -> rustix::io::Result<u32> {
    let member = pid(member).map_err(|_| rustix::io::Errno::INVAL)?;
    getpgid(Some(member)).map(|group| group.as_raw_nonzero().get().unsigned_abs())
}

/// The members other than the anchor end within `grace` after a `TERM`. The anchor is a member, so the group id is held
/// while it signals. A member left at the end of the grace is ended by the rounds of `KILL`.
fn terminate(group: rustix::process::Pid, grace: Duration) -> io::Result<()> {
    if grace.is_zero() {
        return Ok(());
    }
    match kill_process_group(signal_target(group)?, Signal::TERM) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => return Err(error.into()),
    }
    let me = rustix::process::getpid();
    let deadline = Deadline::after(grace);
    let _ = end_members(
        || Ok(()),
        || live_members(group).map(|members| members.into_iter().filter(|m| m.pid != me).collect()),
        |member| await_end(member.pid, deadline),
        || deadline.expired(),
    );
    Ok(())
}

/// Anchor: guard connection on fds 0 and 2, acknowledgement on fd 1.
fn anchor(args: &[OsString]) -> io::Result<()> {
    close_inherited()?;
    let leader = Identity {
        pid: u32::try_from(number(args.first())?).map_err(other)?,
        start_time: number(args.get(1))?,
    };
    let report = Report {
        anchor: identity(std::process::id())?,
        group: u32::try_from(number(args.get(2))?).map_err(other)?,
        leader,
    };
    let grace = Duration::from_nanos(number(args.get(3))?);
    let cleanup = Duration::from_nanos(number(args.get(4))?);
    // The anchor holds no directory of the test or of production.
    std::env::set_current_dir("/")?;
    for signal in SURVIVED {
        signal_hook::flag::register(signal, Arc::new(AtomicBool::new(false)))?;
    }
    verify(&report).map_err(other)?;
    send(io::stderr(), &Line::Anchor(report))?;
    io::Write::write_all(&mut io::stdout(), b"ready\n")?;
    // Blocks until the guard connection ends. An error is an end as well: a connection that the guard never accepted is reset
    // when the guard's process dies. Neither a timer nor a loop of the CPU is involved.
    let _ = io::copy(&mut io::stdin().lock().by_ref(), &mut io::sink());
    if let Err(reason) = verify(&report) {
        let _ = send(io::stderr(), &Line::Refused(reason));
        std::process::exit(1);
    }
    let group = pid(report.group)?;
    // The members that `TERM` reaches: after the grace, each one must still be in the group, or gone (#171 TP6).
    let me = rustix::process::getpid();
    let members: Vec<u32> = live_members(group)?
        .into_iter()
        .filter(|member| member.pid != me)
        .map(|member| member.pid.as_raw_nonzero().get().unsigned_abs())
        .collect();
    terminate(group, grace)?;
    // The grace can move a member or the leader to another group: verify again before any `KILL`.
    if let Err(reason) = verify(&report).and_then(|()| stayed(report.group, &members, group_of)) {
        let _ = send(io::stderr(), &Line::Refused(reason));
        std::process::exit(1);
    }
    // The rounds are bounded by the guard's cleanup bound: the existing `CLEANUP` (lead ruling 2026-10-08, condition 1b).
    match end_group(group, Deadline::after(cleanup)) {
        Ok(()) => {
            let _ = send(io::stderr(), &Line::Ok);
            Ok(())
        }
        Err(report) => {
            let _ = send(io::stderr(), &Line::Fail(report));
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some((stage, rest)) = args.split_first() else {
        eprintln!(
            "usage: botster-test-anchor wrap <socket> <grace ns> <cleanup ns> <program> [args...]"
        );
        std::process::exit(2);
    };
    let (name, result) = match stage.to_str() {
        Some("wrap") => match wrap(rest) {
            Ok(never) => match never {},
            // wrap reported its failure to the guard when it reached one: its stderr may be production's.
            Err(error) => {
                eprintln!("botster-test-anchor wrap: {error}");
                std::process::exit(WRAP_FAILED);
            }
        },
        Some("intermediate") => ("intermediate", intermediate(rest)),
        Some("anchor") => ("anchor", anchor(rest)),
        _ => {
            eprintln!("botster-test-anchor: unknown stage {stage:?}");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        // The helper stages have the guard connection as stderr.
        let _ = send(
            io::stderr(),
            &Line::Error {
                stage: name.into(),
                reason: error.to_string(),
            },
        );
        std::process::exit(1);
    }
}

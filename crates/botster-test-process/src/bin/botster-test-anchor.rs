//! The anchor binary of the group guard: `botster-test-anchor wrap <socket> <grace ns> <cleanup ns> <program> [args...]`. The stages and
//! the protocol are in `botster_test_process::anchor`.

use botster_test_process::anchor::{identity, send, Identity, Line, Report, WRAP_FAILED};
use botster_test_process::platform::{await_end, live_members, pid, start_time, Waited};
use botster_test_process::rounds::{end_group, end_members};
use botster_test_process::{Bounded, Deadline};
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
        let mut intermediate = Command::new(std::env::current_exe()?)
            .args([
                "intermediate".to_string(),
                leader.pid.to_string(),
                leader.start_time.to_string(),
                group_now().to_string(),
                grace.to_string(),
                cleanup.to_string(),
            ])
            .stdin(Stdio::from(input))
            .stderr(Stdio::from(output))
            .stdout(Stdio::piped())
            .spawn()?;
        let acknowledgement = intermediate
            .stdout
            .take()
            .ok_or_else(|| other("no pipe from the anchor"))?;
        // The only child of this process, reaped by its exact pid once its exit was observed. The real program inherits no
        // child.
        let deadline = Deadline::cleanup();
        if await_end(pid(intermediate.id())?, deadline)? == Waited::Deadline {
            return Err(other("the intermediate stage did not end"));
        }
        // A wait for the exact pid of the exited child: it returns once the exit completes.
        let status = intermediate.wait()?;
        if !status.success() {
            return Err(other(format!("the intermediate stage failed: {status}")));
        }
        match Bounded::new(acknowledgement).line(deadline) {
            Ok(Some(line)) if line == "ready\n" => Ok(()),
            Ok(_) => Err(other("the anchor did not report its group")),
            Err(error) => Err(other(error)),
        }
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

/// Intermediate: starts the anchor with the inherited descriptors and exits without waiting for it.
fn intermediate(args: &[OsString]) -> io::Result<()> {
    Command::new(std::env::current_exe()?)
        .arg("anchor")
        .args(args)
        .spawn()?;
    Ok(())
}

/// Whether the group may be signalled now: the anchor is still a member of it, and the leader, while it lives, has not moved
/// to another group (ruling item 13).
fn verify(report: &Report) -> Result<(), String> {
    if group_now() != report.group {
        return Err("the anchor left the group".into());
    }
    let leader = pid(report.leader.pid).map_err(|e| e.to_string())?;
    if start_time(leader) != Some(report.leader.start_time) {
        // The leader has ended (its pid is gone or names another process): only the group id is left to signal.
        return Ok(());
    }
    match getpgid(Some(leader)) {
        Ok(group) if group.as_raw_nonzero().get().unsigned_abs() != report.group => Err(format!(
            "the leader {} moved from group {} to group {}",
            report.leader.pid,
            report.group,
            group.as_raw_nonzero()
        )),
        // ESRCH: the leader ended between the two reads.
        _ => Ok(()),
    }
}

/// The members other than the anchor end within `grace` after a `TERM`. The anchor is a member, so the group id is held
/// while it signals. A member left at the end of the grace is ended by the rounds of `KILL`.
fn terminate(group: rustix::process::Pid, grace: Duration) -> io::Result<()> {
    if grace.is_zero() {
        return Ok(());
    }
    match kill_process_group(group, Signal::TERM) {
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
    terminate(group, grace)?;
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

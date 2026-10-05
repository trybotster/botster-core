//! The launch wrapper of the real-process tier (lead ruling 2026-10-04, RealCoreHarness; protocol in
//! `botster_core_testkit::anchor`). Core runs it as the worker (`OpenConfig.worker_path`) and the worker runs it as the session
//! program. It has three stages, chosen by `argv[0]`:
//!
//! 1. **wrap** (any other `argv[0]`): reads the configuration beside `argv[0]`, connects to the guard, starts the intermediate
//!    stage, reaps it, waits until the anchor holds the group, and execs the real binary with the same arguments. Exec keeps
//!    the pid, the start time, the group, the session, the environment and the exit path that Core or the worker watches.
//! 2. **intermediate**: starts the anchor and exits at once, so the anchor is no child of the real binary (the double fork).
//! 3. **anchor**: holds the inherited group and the guard connection. It reports itself, then blocks until the guard
//!    connection ends: the guard's drop or the death of the test. Then it sends `TERM` to the group, waits the configured
//!    grace, verifies the group again and sends `KILL` to the group as its last act. That signal ends the anchor too. The anchor
//!    reaps nothing: Core and the worker keep the reaping of their own children.
//!
//! A group id cannot be reused while a member lives, and the anchor is a member until the final `KILL`: so the group that the
//! anchor signals is always the one that it reported. If the real binary still lives and has moved to another group, the
//! anchor refuses to signal and reports it (ruling item 13). Nothing is found by name or pattern; nothing waits on a timer
//! except the grace.

use botster_core_edges::edges::ProcessIdentity;
use botster_core_sys::process::start_time;
use botster_core_testkit::anchor::{
    Config, Line, Report, CONFIG_FILE, STAGE_ANCHOR, STAGE_INTERMEDIATE, WRAP_FAILED,
};
use rustix::process::{getpgid, kill_process_group, Pid, Signal};
use std::convert::Infallible;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

/// The signals that the anchor survives. A no-op handler keeps it alive (the workspace forbids `unsafe`, so no `SIG_IGN`):
/// `TERM` from Core's group signal (Core LC-5) and from its own cleanup, `HUP` when the payload, its session's leader, exits,
/// the job-control and keyboard signals that the payload's terminal sends to its foreground group, and the user signals.
/// `KILL` and `STOP` cannot be caught.
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

fn raw(pid: u32) -> io::Result<Pid> {
    i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| other(format!("{pid} is not a pid")))
}

fn group_of(pid: Option<Pid>) -> io::Result<u32> {
    Ok(getpgid(pid)?.as_raw_nonzero().get().unsigned_abs())
}

fn identity_of(pid: u32) -> io::Result<ProcessIdentity> {
    let start_time = start_time(pid).ok_or_else(|| other(format!("{pid} has no start time")))?;
    Ok(ProcessIdentity { pid, start_time })
}

fn send(guard: &mut impl Write, line: &Line) -> io::Result<()> {
    writeln!(guard, "{}", line.encode())?;
    guard.flush()
}

/// Wrap: everything before the exec. It returns only when it fails.
fn wrap(argv0: &OsStr, args: &[OsString]) -> io::Result<Infallible> {
    let config_path = Path::new(argv0)
        .parent()
        .ok_or_else(|| other("argv[0] names no directory"))?
        .join(CONFIG_FILE);
    let config = Config::decode(&std::fs::read_to_string(&config_path)?).map_err(other)?;
    let mut guard = UnixStream::connect(&config.socket)?;
    let result = (|| -> io::Result<()> {
        let leader = identity_of(std::process::id())?;
        let group = group_of(None)?;
        let grace = config.grace.as_nanos().to_string();
        // The guard connection is the intermediate's stdin and stderr: the anchor inherits both. The pipe of stdout carries
        // the anchor's single acknowledgement back here.
        let input: OwnedFd = guard.try_clone()?.into();
        let output: OwnedFd = guard.try_clone()?.into();
        let mut intermediate = Command::new(std::env::current_exe()?)
            .arg0(STAGE_INTERMEDIATE)
            .args([
                leader.pid.to_string(),
                leader.start_time.to_string(),
                group.to_string(),
                grace,
            ])
            .arg(&config.binary)
            .stdin(Stdio::from(input))
            .stderr(Stdio::from(output))
            .stdout(Stdio::piped())
            .spawn()?;
        let acknowledgement = intermediate
            .stdout
            .take()
            .ok_or_else(|| other("no pipe from the anchor"))?;
        // The only child of this process. The real binary inherits no child.
        if !intermediate.wait()?.success() {
            return Err(other("the intermediate stage failed"));
        }
        let mut ready = String::new();
        BufReader::new(acknowledgement).read_line(&mut ready)?;
        if ready != "ready\n" {
            return Err(other("the anchor did not report its group"));
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = send(
            &mut guard,
            &Line::Error {
                stage: "wrap".into(),
                error: error.to_string(),
            },
        );
        return Err(error);
    }
    // The guard connection is close-on-exec: the real binary does not inherit it.
    let error = Command::new(&config.binary).args(args).exec();
    let _ = send(
        &mut guard,
        &Line::Error {
            stage: "wrap".into(),
            error: format!("exec {}: {error}", config.binary.display()),
        },
    );
    Err(error)
}

/// Intermediate: starts the anchor with the inherited descriptors and exits without waiting for it.
fn intermediate(args: &[OsString]) -> io::Result<()> {
    Command::new(std::env::current_exe()?)
        .arg0(STAGE_ANCHOR)
        .args(args)
        .spawn()?;
    Ok(())
}

fn number(value: &OsString) -> io::Result<u64> {
    value
        .to_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| other("an argument is not a number"))
}

/// Whether the group may be signalled now: the anchor is still a member (it always is: only it or its parent could move it),
/// and the leader, while it lives, has not moved to another group.
fn verify(report: &Report) -> io::Result<Result<(), String>> {
    if group_of(None)? != report.group {
        return Ok(Err("the anchor left the group".into()));
    }
    if start_time(report.leader.pid) != Some(report.leader.start_time) {
        // The leader has ended (its pid is gone or names another process): only the group id is left to signal.
        return Ok(Ok(()));
    }
    match group_of(Some(raw(report.leader.pid)?)) {
        Ok(group) if group != report.group => Ok(Err(format!(
            "the real binary {} moved from group {} to group {group}",
            report.leader.pid, report.group
        ))),
        // ESRCH: the leader ended between the two reads.
        _ => Ok(Ok(())),
    }
}

/// Anchor: guard fd 0, acknowledgement fd 1, guard fd 2.
fn anchor(args: &[OsString]) -> io::Result<()> {
    let [leader, leader_start, group, grace, binary] = args else {
        return Err(other(
            "the anchor takes the leader, its start time, the group, the grace and the binary",
        ));
    };
    // The anchor holds no directory of the session or the worker.
    std::env::set_current_dir("/")?;
    for signal in SURVIVED {
        signal_hook::flag::register(signal, Arc::new(AtomicBool::new(false)))?;
    }
    let report = Report {
        anchor: identity_of(std::process::id())?,
        group: u32::try_from(number(group)?).map_err(other)?,
        leader: ProcessIdentity {
            pid: u32::try_from(number(leader)?).map_err(other)?,
            start_time: number(leader_start)?,
        },
        binary: PathBuf::from(binary),
    };
    let grace = Duration::from_nanos(number(grace)?);
    if let Err(reason) = verify(&report)? {
        return Err(other(reason));
    }
    let mut guard = io::stderr();
    send(&mut guard, &Line::Anchor(report.clone()))?;
    let mut acknowledgement = io::stdout();
    acknowledgement.write_all(b"ready\n")?;
    acknowledgement.flush()?;
    // Blocks until the guard connection ends. An error is an end as well: a connection that the guard never accepted is reset
    // when the guard's process dies. Neither a timer nor a loop of the CPU is involved.
    let _ = io::copy(&mut io::stdin(), &mut io::sink());
    if let Err(reason) = verify(&report)? {
        let _ = send(&mut guard, &Line::Refused { reason });
        return Ok(());
    }
    let group = raw(report.group)?;
    let _ = send(&mut guard, &Line::Term);
    kill_process_group(group, Signal::TERM)?;
    // The grace that the Core limits give a payload before its group is killed (Core LC-5).
    std::thread::sleep(grace);
    if let Err(reason) = verify(&report)? {
        let _ = send(&mut guard, &Line::Refused { reason });
        return Ok(());
    }
    let _ = send(&mut guard, &Line::Kill);
    kill_process_group(group, Signal::KILL)?;
    Err(other("the group KILL did not end the anchor"))
}

fn main() {
    let mut argv = std::env::args_os();
    let argv0 = argv.next().unwrap_or_default();
    let args: Vec<OsString> = argv.collect();
    let (stage, result) = if argv0 == STAGE_INTERMEDIATE {
        ("intermediate", intermediate(&args))
    } else if argv0 == STAGE_ANCHOR {
        ("anchor", anchor(&args))
    } else {
        // wrap reports its own failure to the guard when it reached one: its stderr may be the payload's terminal.
        match wrap(&argv0, &args) {
            Ok(never) => match never {},
            Err(_) => std::process::exit(WRAP_FAILED),
        }
    };
    if let Err(error) = result {
        // The helper stages have the guard connection as stderr.
        let _ = send(
            &mut io::stderr(),
            &Line::Error {
                stage: stage.into(),
                error: error.to_string(),
            },
        );
        std::process::exit(1);
    }
}

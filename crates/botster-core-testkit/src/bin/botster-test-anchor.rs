//! The prebuilt process anchor for the real test harness.
//! Safe process creation performs the two forks through two helper stages.

use botster_core_sys::process::start_time;
use rustix::process::{getpgid, getpid, kill_process_group, Pid, Signal};
use serde_json::json;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

fn number(value: &OsString) -> io::Result<u64> {
    value
        .to_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the argument is not a number"))
}

fn wrap(args: &[OsString]) -> io::Result<()> {
    let [socket, grace, separator, binary, rest @ ..] = args else {
        return Err(io::Error::other(
            "wrap needs a socket, grace, --, and a binary",
        ));
    };
    if separator != "--" {
        return Err(io::Error::other("wrap needs -- before the binary"));
    }
    let guard = UnixStream::connect(socket)?;
    let guard_input: OwnedFd = guard.try_clone()?.into();
    let guard_error: OwnedFd = guard.into();
    let leader = std::process::id();
    let start =
        start_time(leader).ok_or_else(|| io::Error::other("the leader has no start time"))?;
    let group = getpgid(None)?;
    let mut intermediate = Command::new(std::env::current_exe()?)
        .arg("intermediate")
        .args([
            leader.to_string(),
            start.to_string(),
            group.as_raw_nonzero().to_string(),
        ])
        .arg(grace)
        .stdin(Stdio::from(guard_input))
        .stderr(Stdio::from(guard_error))
        .stdout(Stdio::piped())
        .spawn()?;
    let output = intermediate
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no anchor acknowledgement pipe"))?;
    let status = intermediate.wait()?;
    if !status.success() {
        return Err(io::Error::other("the intermediate anchor stage failed"));
    }
    let mut ready = String::new();
    BufReader::new(output).read_line(&mut ready)?;
    if ready != "ready\n" {
        return Err(io::Error::other(
            "the anchor did not establish group ownership",
        ));
    }
    // The only child of this process was reaped. Exec preserves its identity and native exit path.
    Err(Command::new(binary).args(rest).exec())
}

fn intermediate(args: &[OsString]) -> io::Result<()> {
    let child = Command::new(std::env::current_exe()?)
        .arg("anchor")
        .arg("0")
        .args(args)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit())
        .spawn()?;
    // This stage exits immediately. The anchor becomes a child of init or the Linux subreaper.
    drop(child);
    Ok(())
}

fn pid(value: u64) -> io::Result<Pid> {
    i32::try_from(value)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| io::Error::other("the argument is not a pid"))
}

fn verify_group(leader: Pid, leader_start: u64, group: Pid, anchor_start: u64) -> io::Result<()> {
    if start_time(std::process::id()) != Some(anchor_start) || getpgid(None)? != group {
        return Err(io::Error::other(
            "the anchor no longer owns the recorded group",
        ));
    }
    if start_time(leader.as_raw_nonzero().get() as u32) == Some(leader_start)
        && getpgid(Some(leader))? != group
    {
        return Err(io::Error::other(
            "the real binary moved to another process group",
        ));
    }
    Ok(())
}

fn anchor(args: &[OsString]) -> io::Result<()> {
    let [fd, leader, leader_start, group, grace] = args else {
        return Err(io::Error::other(
            "anchor needs fd, leader, start time, group, and grace",
        ));
    };
    if number(fd)? != 0 {
        return Err(io::Error::other("the anchor guard fd must be stdin"));
    }
    let leader = pid(number(leader)?)?;
    let leader_start = number(leader_start)?;
    let group = pid(number(group)?)?;
    let grace = Duration::from_nanos(number(grace)?);
    let anchor_pid = std::process::id();
    let anchor_start =
        start_time(anchor_pid).ok_or_else(|| io::Error::other("the anchor has no start time"))?;
    verify_group(leader, leader_start, group, anchor_start)?;
    signal_hook::flag::register(
        signal_hook::consts::SIGTERM,
        Arc::new(AtomicBool::new(false)),
    )?;
    writeln!(
        io::stderr(),
        "{}",
        json!({"kind": "anchor", "pid": anchor_pid, "start_time": anchor_start,
        "group": group.as_raw_nonzero().get(), "leader": leader.as_raw_nonzero().get(), "leader_start_time": leader_start})
    )?;
    writeln!(io::stdout(), "ready")?;
    io::stdout().flush()?;
    // Guard EOF is Drop or test death. This read blocks without a timer or CPU loop.
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    verify_group(leader, leader_start, group, anchor_start)?;
    kill_process_group(group, Signal::TERM)?;
    // timer: cleanup grace — this value is the existing CoreLimits.stop_grace supplied by the harness.
    let (_sender, receiver) = std::sync::mpsc::channel::<()>();
    let _ = receiver.recv_timeout(grace);
    verify_group(leader, leader_start, group, anchor_start)?;
    // Final action: this signal also ends this anchor. The anchor reaps no process.
    kill_process_group(group, Signal::KILL)?;
    Err(io::Error::other(
        "the final group signal did not end the anchor",
    ))
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let result = match args.split_first() {
        Some((stage, args)) if stage == "wrap" => wrap(args),
        Some((stage, args)) if stage == "intermediate" => intermediate(args),
        Some((stage, args)) if stage == "anchor" => anchor(args),
        _ => Err(io::Error::other("unknown anchor stage")),
    };
    if let Err(error) = result {
        let _ = writeln!(
            io::stderr(),
            "{}",
            json!({"kind": "error", "error": error.to_string(), "pid": getpid().as_raw_nonzero().get()})
        );
        std::process::exit(1);
    }
}

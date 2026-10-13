//! A bounded run of a test tool (moved from Core's xtask `test_budget`, with its tests): the command runs in its own process
//! group under a deadline, a tracker records the processes that belong to it, and after it every process that it left
//! behind is reported and killed (#220 R1-1). The verdicts are [`exit_failures`] and [`run_failures`]; the I/O shells are
//! [`run_bounded`], [`bounded_report`], [`bounded_success`] and [`clean_up`]. Signals go through
//! `botster_core_sys::signal`, the one owner of the signal rule (lead 2026-10-12, option (i)).

use anyhow::{Context, Result};
use botster_test_support::census::{leftovers, ps_snapshot, update_owned, Proc};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The pids that the run wrapper recorded.
fn read_pids(pidfile: &Path) -> Vec<u32> {
    std::fs::read_to_string(pidfile)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

/// A cleanup target: `-N` is the process group N, and `N` is the process N.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    Process(u32),
    Group(u32),
}

fn target(arg: &str) -> Option<Target> {
    match arg.strip_prefix('-') {
        Some(group) => group.parse().ok().map(Target::Group),
        None => arg.parse().ok().map(Target::Process),
    }
}

/// Kills every target through `botster_core_sys::signal`, which refuses 0, 1 and our own group or pid (the pattern rule).
fn kill(args: &[String]) {
    use botster_core_sys::signal::{signal_group, signal_process, Signal};
    for target in args.iter().filter_map(|arg| target(arg)) {
        let _ = match target {
            Target::Group(group) => signal_group(group, Signal::KILL),
            Target::Process(pid) => signal_process(pid, Signal::KILL),
        };
    }
}

/// A bounded run ([`run_bounded`]): how it ended, and what it left.
pub struct Run {
    /// The exit status, or why it could not be read.
    pub status: Result<ExitStatus, String>,
    /// Whether the run passed its deadline, and its group was killed.
    pub timed_out: bool,
    /// The run's wall time.
    pub wall: Duration,
    /// The process group of the run: the command leads it.
    pub group: u32,
    /// The processes of the run that are alive after it, or why they could not be listed.
    pub owned: Result<HashMap<u32, Proc>, String>,
}

/// Runs `command` in its own process group, and kills that group when `deadline` passes. A tracker thread records which
/// processes belong to the run while it goes (see `update_owned`). With `capture`, the run's stdout goes through a pipe:
/// each line is printed as it comes and sent to `capture`, so the caller reads it after [`clean_up`] (a leftover that holds
/// the pipe keeps it open until then). Everything after the spawn returns a value, so the caller can always clean up. An
/// I/O shell: its verdicts are [`exit_failures`] and [`run_failures`].
pub fn run_bounded(
    mut command: Command,
    deadline: Duration,
    pidfile: &Path,
    capture: Option<mpsc::Sender<String>>,
    start: &'static str,
) -> Result<Run> {
    let started = Instant::now();
    command.process_group(0);
    if capture.is_some() {
        command.stdout(Stdio::piped());
    }
    let mut child = command.spawn().context(start)?;
    let group = child.id();
    if let (Some(lines), Some(stdout)) = (capture, child.stdout.take()) {
        // The reader ends with the pipe; nothing waits for it.
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                println!("{line}");
                if lines.send(line).is_err() {
                    break;
                }
            }
        });
    }

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let tracker = std::thread::spawn(move || {
        let mut owned = HashMap::new();
        while let Err(mpsc::RecvTimeoutError::Timeout) =
            stop_rx.recv_timeout(Duration::from_millis(10))
        {
            if let Ok(procs) = ps_snapshot() {
                update_owned(&mut owned, &procs, group, &[]);
            }
        }
        owned
    });

    let (tx, rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let _ = tx.send(child.wait());
    });
    let (status, timed_out) = match rx.recv_timeout(deadline) {
        Ok(status) => (status, false),
        Err(_) => {
            kill(&[format!("-{group}")]);
            (
                rx.recv().unwrap_or_else(|e| Err(std::io::Error::other(e))),
                true,
            )
        }
    };
    let wall = started.elapsed();
    let _ = waiter.join();

    let _ = stop_tx.send(());
    let owned = tracker
        .join()
        .map_err(|_| "the tracker panicked".to_string());
    let owned = owned.and_then(|mut owned| {
        ps_snapshot().map(|procs| {
            update_owned(&mut owned, &procs, group, &read_pids(pidfile));
            owned
        })
    });
    Ok(Run {
        status: status.map_err(|e| e.to_string()),
        timed_out,
        wall,
        group,
        owned,
    })
}

/// Runs `command` under the bounds of [`bounded_report`]: its own process group, `deadline`, the process
/// tracker (with the tests' pid file `pidfile`, which it empties first) and the leftover check. Its stdout is printed as it
/// comes. Returns whether it succeeded. The stage-2 baseline of Core's mutation step runs this way (#225 MS-R1-1).
///
/// # Errors
/// The run could not start, passed its deadline, or left processes behind.
pub fn bounded_success(
    mut command: Command,
    deadline: Duration,
    pidfile: &Path,
    start: &'static str,
) -> Result<bool> {
    let _ = std::fs::remove_file(pidfile);
    if let Some(dir) = pidfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    command.env("BOTSTER_TEST_PIDFILE", pidfile);
    bounded_report(command, deadline, pidfile, start).map(|(success, _)| success)
}

/// Runs `command` with [`run_bounded`], capturing its stdout, and judges it with [`run_failures`]. The I/O shell of that
/// decision: returns whether the run succeeded and what it printed.
pub fn bounded_report(
    command: Command,
    deadline: Duration,
    pidfile: &Path,
    start: &'static str,
) -> Result<(bool, String)> {
    let started = Instant::now();
    let (lines_tx, lines_rx) = mpsc::channel();
    let run = run_bounded(command, deadline, pidfile, Some(lines_tx), start)?;
    // First, before any step that can fail: report and kill the leftovers.
    let leftover = clean_up(&run);
    let mut lines = Vec::new();
    // The reader ends when every holder of the pipe has closed it; the leftovers are killed above.
    let output_open = loop {
        match lines_rx.recv_timeout(deadline.saturating_sub(started.elapsed())) {
            Ok(line) => lines.push(line),
            Err(mpsc::RecvTimeoutError::Disconnected) => break false,
            Err(mpsc::RecvTimeoutError::Timeout) => break true,
        }
    };
    let failures = run_failures(leftover, run.timed_out, output_open, deadline);
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
    let success = matches!(&run.status, Ok(status) if status.success());
    let mut text = lines.join("\n");
    text.push('\n');
    Ok((success, text))
}

/// The failures of a bounded run (#220 R1-1): the processes that it left behind (`clean_up`), a run past its deadline, and
/// output still open at the deadline (a holder of the pipe outlived the run). None of them is the run's own exit status.
pub fn run_failures(
    leftover: Option<String>,
    timed_out: bool,
    output_open: bool,
    deadline: Duration,
) -> Vec<String> {
    let secs = deadline.as_secs();
    leftover
        .into_iter()
        .chain(timed_out.then(|| format!("the run passed its {secs} s deadline; it was killed")))
        .chain(
            output_open.then(|| format!("the run's output stayed open past its {secs} s deadline")),
        )
        .collect()
}

/// Reports and kills what the run left behind. It runs first after the run, on every path, so a later error cannot skip it.
/// The report is printed before the kill. It kills only processes that `update_owned` proved to be descendants
/// of the run, and the process group that the run made.
pub fn clean_up(run: &Run) -> Option<String> {
    let (message, mut targets) = match &run.owned {
        Ok(owned) => {
            let left = leftovers(owned);
            if left.is_empty() {
                return None;
            }
            let mut message = String::from("processes left behind by the tests:");
            for p in &left {
                let kind = if p.is_zombie() {
                    " zombie, not reaped"
                } else {
                    ""
                };
                message.push_str(&format!(
                    "\n    pid {} (parent {}, group {}{kind}): {}",
                    p.pid, p.ppid, p.pgid, p.command
                ));
            }
            (
                message,
                left.iter().map(|p| p.pid.to_string()).collect::<Vec<_>>(),
            )
        }
        Err(why) => (
            format!("could not list the processes of the run ({why}); the run group is killed"),
            Vec::new(),
        ),
    };
    eprintln!("test-budget: LEFTOVER {message}");
    targets.push(format!("-{}", run.group));
    kill(&targets);
    Some(message)
}

/// Removes the report of an earlier run. A file that is absent is fine; any other error fails, so a stale report can never
/// stand for this run (the package reviewer, 2026-10-11).
///
/// # Errors
/// The file exists and cannot be removed.
pub fn remove_stale(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("remove the earlier report {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// The failures that the way nextest ended gives: a run past its deadline, a failing status, or no status.
pub fn exit_failures(
    timed_out: bool,
    status: &Result<ExitStatus, String>,
    tier: &str,
    deadline: Duration,
) -> Vec<String> {
    if timed_out {
        return vec![format!(
            "the {tier} tier passed its {} s deadline; the run was killed",
            deadline.as_secs()
        )];
    }
    match status {
        Ok(status) if status.success() => Vec::new(),
        Ok(status) => vec![format!("nextest failed ({status})")],
        Err(why) => vec![format!("could not wait for nextest: {why}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::junit::junit_path;

    /// A leftover pid is a process and `-N` is a group; any other text is no target.
    #[test]
    fn a_cleanup_argument_names_a_process_or_a_group() {
        assert_eq!(target("42"), Some(Target::Process(42)));
        assert_eq!(target("-42"), Some(Target::Group(42)));
        for arg in ["", "-", "x", "-x", "--42", "4 2"] {
            assert_eq!(target(arg), None, "{arg}");
        }
    }

    /// An earlier report is removed, an absent one is fine, and a report that cannot be removed fails the run, so it never
    /// stands for this one.
    #[test]
    fn an_earlier_report_is_removed_or_the_run_fails() {
        let dir = tempfile::tempdir().unwrap();
        let junit = junit_path(dir.path(), "slow");
        assert_eq!(junit, dir.path().join("nextest/slow/junit.xml"));
        assert!(remove_stale(&junit).is_ok());
        std::fs::create_dir_all(junit.parent().unwrap()).unwrap();
        std::fs::write(&junit, "<testsuites/>").unwrap();
        assert!(remove_stale(&junit).is_ok());
        assert!(!junit.exists());
        // A directory in the report's place cannot be removed as a file.
        std::fs::create_dir(&junit).unwrap();
        let error = remove_stale(&junit).unwrap_err();
        assert!(
            format!("{error:#}").contains("remove the earlier report"),
            "{error:#}"
        );
    }

    #[test]
    fn the_pid_file_is_read_line_by_line_and_a_missing_file_is_empty() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let file = root.path().join("pids");
        std::fs::write(&file, "12\n x\n 34 \n\n").unwrap();
        assert_eq!(read_pids(&file), [12, 34]);
        assert!(read_pids(&root.path().join("none")).is_empty());
    }

    /// #220 R1-1: a bounded run fails for each process that it left behind, for passing its deadline, and for output still
    /// open at the deadline, each with its own message; a run with none of them has no failure.
    #[test]
    fn the_failures_of_a_bounded_run_are_its_leftovers_its_deadline_and_its_open_output() {
        let d = Duration::from_secs(600);
        assert!(run_failures(None, false, false, d).is_empty());
        assert_eq!(
            run_failures(
                Some("processes left behind by the tests: pid 9".into()),
                false,
                false,
                d
            ),
            ["processes left behind by the tests: pid 9"]
        );
        assert_eq!(
            run_failures(None, true, false, d),
            ["the run passed its 600 s deadline; it was killed"]
        );
        assert_eq!(
            run_failures(None, false, true, d),
            ["the run's output stayed open past its 600 s deadline"]
        );
        assert_eq!(run_failures(Some("left".into()), true, true, d).len(), 3);
    }

    #[test]
    fn the_way_nextest_ended_is_judged() {
        use std::os::unix::process::ExitStatusExt;
        let d = Duration::from_secs(90);
        assert!(exit_failures(false, &Ok(ExitStatus::from_raw(0)), "default", d).is_empty());
        assert!(
            exit_failures(false, &Ok(ExitStatus::from_raw(256)), "default", d)[0]
                .contains("nextest failed")
        );
        assert!(exit_failures(false, &Err("gone".into()), "default", d)[0].contains("gone"));
        let timed = exit_failures(true, &Ok(ExitStatus::from_raw(0)), "slow", d);
        assert_eq!(timed.len(), 1);
        assert!(timed[0].contains("slow tier passed its 90 s deadline"));
    }
}

/// The bounds of a run (#220 R1-1), with real processes.
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;

    /// The deadline of a run that must end on its own (the slow tier's deadline in Core).
    const SLOW_DEADLINE: Duration = Duration::from_secs(600);

    // The helpers are closures: a function of this module is mutated by cargo-mutants (only `cfg(test)` alone is skipped),
    // and the default mutation run does not build the slow feature.

    #[test]
    fn a_bounded_run_returns_its_status_and_what_it_printed() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let sh = |script: &str| {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        };
        let (success, text) = bounded_report(
            sh("echo 'real conformance: passed 1'; echo second"),
            SLOW_DEADLINE,
            &pids,
            "start sh",
        )
        .unwrap();
        assert!(success);
        assert_eq!(text, "real conformance: passed 1\nsecond\n");
        let (success, _) = bounded_report(sh("exit 3"), SLOW_DEADLINE, &pids, "start sh").unwrap();
        assert!(!success);
    }

    /// The bounded form of the stage-2 baseline (#225 MS-R1-1): whether the run succeeded; a run past its deadline and a
    /// process left behind fail, as in `bounded_report`. The pid file is made in a directory that does not exist yet.
    #[test]
    fn a_bounded_success_is_the_runs_success_and_a_hung_run_or_a_leftover_fails() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("new").join("pids");
        let sh = |script: &str| {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        };
        assert!(bounded_success(sh("true"), SLOW_DEADLINE, &pids, "start sh").unwrap());
        assert!(!bounded_success(sh("exit 3"), SLOW_DEADLINE, &pids, "start sh").unwrap());
        let blocker = botster_test_process::Blocker::new(dir.path(), "block").unwrap();
        let hung = bounded_success(
            sh(&format!("exec {}", blocker.shell())),
            Duration::ZERO,
            &pids,
            "start sh",
        )
        .unwrap_err()
        .to_string();
        assert!(hung.contains("deadline"), "{hung}");
        let left_blocker = botster_test_process::Blocker::new(dir.path(), "left").unwrap();
        let left = bounded_success(
            sh(&format!("{} & true", left_blocker.shell())),
            SLOW_DEADLINE,
            &pids,
            "start sh",
        )
        .unwrap_err()
        .to_string();
        assert!(left.contains("processes left behind"), "{left}");
    }

    /// A run that does not end by its deadline is killed with its group, and fails. A zero deadline has passed at the
    /// start, so the test needs no timeout value of its own.
    #[test]
    fn a_run_past_its_deadline_is_killed_and_fails() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let sh = |script: &str| {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        };
        let blocker = botster_test_process::Blocker::new(dir.path(), "block").unwrap();
        let error = bounded_report(
            sh(&format!("exec {}", blocker.shell())),
            Duration::ZERO,
            &pids,
            "start sh",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("deadline"), "{error}");
    }

    /// A process that the run leaves behind fails the run, and is killed before the report is read (it holds the pipe).
    #[test]
    fn a_process_left_behind_fails_the_run_and_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let sh = |script: &str| {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        };
        let blocker = botster_test_process::Blocker::new(dir.path(), "block").unwrap();
        let error = bounded_report(
            sh(&format!(
                "{} & echo 'real conformance: passed 1'",
                blocker.shell()
            )),
            SLOW_DEADLINE,
            &pids,
            "start sh",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("processes left behind"), "{error}");
    }
}

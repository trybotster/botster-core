//! `cargo xtask test-budget`: run a test tier under cargo-nextest and enforce its budgets
//! (BUILD.md "Testing: fast and lean", plan section 8 step 6; adapted from the xtask of botster-contracts).
//!
//! The default tier fails when a test runs over 2 s, when the whole run takes over 60 s, or when a
//! process outlives the run. The slow tier has no per-test limit and one whole-run deadline.
//! The command is unix-only, like the rest of the CI.

use crate::caps;
use crate::fsutil::{metadata, Meta};
use anyhow::{bail, Context, Result};
use botster_test_support::census::{leftovers, ps_snapshot, update_owned, Proc};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A default-tier test that runs longer than this fails (BUILD.md Testing rule 4).
const TEST_LIMIT_SECS: f64 = 2.0;
/// The default tier as a whole must finish within this.
const TIER_LIMIT: Duration = Duration::from_secs(60);
/// The slow tier deadline when `--deadline` is not given.
pub(crate) const SLOW_DEADLINE: Duration = Duration::from_secs(600);
const USAGE: &str = "usage: cargo xtask test-budget [--slow [--deadline <10m|90s|1h>]] [--compare <test-times.json>]";

#[derive(Debug, PartialEq)]
struct Options {
    slow: bool,
    deadline: Option<Duration>,
    compare: Option<PathBuf>,
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        slow: false,
        deadline: None,
        compare: None,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--slow" => options.slow = true,
            "--deadline" => {
                let value = it.next().context("--deadline needs a value")?;
                options.deadline = Some(parse_duration(value)?);
            }
            "--compare" => {
                options.compare = Some(it.next().context("--compare needs a path")?.into());
            }
            other => bail!("unknown argument '{other}'\n{USAGE}"),
        }
    }
    if options.deadline.is_some() && !options.slow {
        bail!("--deadline applies to --slow only; the default tier has a fixed 60 s budget");
    }
    Ok(options)
}

/// `90s`, `10m` or `2h`.
fn parse_duration(text: &str) -> Result<Duration> {
    let re = Regex::new(r"^(\d+)([smh])$")?;
    let caps = re
        .captures(text)
        .with_context(|| format!("bad duration '{text}' (use 90s, 10m or 2h)"))?;
    let n: u64 = caps[1].parse()?;
    let unit = match &caps[2] {
        "s" => 1,
        "m" => 60,
        _ => 3600,
    };
    Ok(Duration::from_secs(n * unit))
}

/// The per-test times of a nextest JUnit file: `<binary id> <test name>` to seconds.
fn parse_junit(xml: &str) -> BTreeMap<String, f64> {
    let case = Regex::new(r"<testcase\s+([^>]*?)/?>").expect("regex");
    let attr = Regex::new(r#"(\w+)="([^"]*)""#).expect("regex");
    let mut times = BTreeMap::new();
    for caps in case.captures_iter(xml) {
        let attrs: BTreeMap<&str, &str> = attr
            .captures_iter(caps.get(1).map_or("", |m| m.as_str()))
            .map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()))
            .collect();
        let (Some(name), Some(time)) = (attrs.get("name"), attrs.get("time")) else {
            continue;
        };
        let Ok(secs) = time.parse::<f64>() else {
            continue;
        };
        let class = attrs.get("classname").copied().unwrap_or("");
        times.insert(unescape(&format!("{class} {name}")), secs);
    }
    times
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn sorted_slowest_first(times: &BTreeMap<String, f64>) -> Vec<(&String, f64)> {
    let mut rows: Vec<(&String, f64)> = times.iter().map(|(k, v)| (k, *v)).collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    rows
}

/// Tests whose time at least doubled against `old`: (name, old, new), slowest first.
/// A test with a zero baseline counts when its new time is above zero. A test that is not in `old` is new, not doubled.
fn doublings(old: &BTreeMap<String, f64>, new: &BTreeMap<String, f64>) -> Vec<(String, f64, f64)> {
    let mut found: Vec<(String, f64, f64)> = new
        .iter()
        .filter_map(|(name, now)| {
            let before = *old.get(name)?;
            let doubled = if before > 0.0 {
                *now >= 2.0 * before
            } else {
                *now > 0.0
            };
            doubled.then(|| (name.clone(), before, *now))
        })
        .collect();
    found.sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    found
}

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

/// The CI seed set of the conformance runner, and the one seed of the real-process tier (plan 4.2c: a real implementation
/// ignores the seed).
const DEFAULT_SEEDS: &str = "0-31";
const SLOW_SEEDS: &str = "0";
/// The pinned seed of the bolero property tests of the default tier (bolero has no API for it, only this variable), and
/// their case count. A run that finds a failure prints the seed that reproduces it.
const BOLERO_SEED: &str = "20261001";
const BOLERO_ITERATIONS: &str = "256";

/// The environment that a tier gives to the test processes.
pub fn tier_env(slow: bool) -> Vec<(&'static str, &'static str)> {
    if slow {
        vec![("BOTSTER_SEEDS", SLOW_SEEDS)]
    } else {
        vec![
            ("BOTSTER_SEEDS", DEFAULT_SEEDS),
            ("BOLERO_RANDOM_SEED", BOLERO_SEED),
            ("BOLERO_RANDOM_ITERATIONS", BOLERO_ITERATIONS),
        ]
    }
}

/// The nextest filter of the slow tier.
pub const SLOW_FILTER: &str = "binary(/^slow/) | test(/(^|::)slow_/)";

/// The packages and filter that a tier runs.
fn selection(options: &Options, meta: &Meta) -> Vec<String> {
    if !options.slow {
        return vec!["--workspace".into()];
    }
    let mut args = Vec::new();
    for package in &meta.slow_packages {
        args.push("-p".to_string());
        args.push(package.clone());
    }
    let features: Vec<String> = meta
        .slow_packages
        .iter()
        .map(|p| format!("{p}/slow"))
        .collect();
    args.push("--features".into());
    args.push(features.join(","));
    // A slow test is an integration-test target named `slow` or `slow_*`, or a unit test of any target in a module named
    // `slow_*` at any depth (nextest matches the whole module path): the real-disk storage of a library, or the real
    // driver of a binary, which only its own unit tests reach.
    args.push("-E".into());
    args.push(SLOW_FILTER.into());
    args
}

struct Run {
    /// The exit status, or why it could not be read.
    status: Result<ExitStatus, String>,
    timed_out: bool,
    wall: Duration,
    /// The process group that the xtask started.
    group: u32,
    /// The processes of the run that are alive after it, or why they could not be listed.
    owned: Result<HashMap<u32, Proc>, String>,
}

/// Runs nextest in its own process group, and kills that group when `deadline` passes.
/// A tracker thread records which processes belong to the run while it goes (see `update_owned`).
/// Everything after the spawn returns a value, so the caller can always clean up.
fn run_nextest(
    root: &Path,
    args: &[String],
    deadline: Duration,
    pidfile: &Path,
    slow: bool,
) -> Result<Run> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["nextest", "run"])
        .args(args)
        .env("BOTSTER_TEST_PIDFILE", pidfile)
        .envs(tier_env(slow));
    caps::apply(&mut command);
    run_bounded(
        command,
        deadline,
        pidfile,
        None,
        "start cargo nextest (install it with `cargo install cargo-nextest --locked`)",
    )
}

/// Runs `command` in its own process group, and kills that group when `deadline` passes. A tracker thread records which
/// processes belong to the run while it goes (see `update_owned`). With `capture`, the run's stdout goes through a pipe:
/// each line is printed as it comes and sent to `capture`, so the caller reads it after [`clean_up`] (a leftover that holds
/// the pipe keeps it open until then). Everything after the spawn returns a value, so the caller can always clean up. An
/// I/O shell: its verdicts are [`exit_failures`] and [`run_failures`].
fn run_bounded(
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

/// Runs the real tier's pending-real trials (`cargo test -p botster-core --features slow --test slow_conformance --
/// --ignored`) under the slow tier's bounds: its own process group, `deadline`, the process tracker and the leftover check
/// (#220 R1-1). A pending-real id never fails the run, and its outcome is in the report that the binary prints: returns
/// that stdout, and whether the binary succeeded. A run past `deadline`, a process that it left behind, or a report pipe
/// still open at `deadline` fails.
///
/// # Errors
/// The run could not start, passed its deadline, or left processes behind.
pub fn pending_real(root: &Path, deadline: Duration) -> Result<(bool, String)> {
    caps::require()?;
    let meta = metadata(root)?;
    let pidfile = meta
        .target_dir
        .join("nextest")
        .join("slow")
        .join("pending-real-pids");
    let _ = std::fs::remove_file(&pidfile);
    if let Some(dir) = pidfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            "test",
            "-p",
            "botster-core",
            "--features",
            "slow",
            "--test",
            "slow_conformance",
            "--locked",
            "--",
            "--ignored",
            "--format",
            "terse",
            "--test-threads",
            "4",
        ])
        .envs(tier_env(true));
    caps::apply(&mut command);
    bounded_report(
        command,
        deadline,
        &pidfile,
        "start the real-tier conformance binary",
    )
}

/// Runs `command` under the slow tier's bounds, as [`pending_real`] does: its own process group, `deadline`, the process
/// tracker (with the tests' pid file `pidfile`, which it empties first) and the leftover check. Its stdout is printed as it
/// comes. Returns whether it succeeded. The stage-2 baseline of the mutation step runs this way (#225 MS-R1-1).
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
fn bounded_report(
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
fn run_failures(
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
/// of the run, and the process group that the xtask created.
fn clean_up(run: &Run) -> Option<String> {
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

fn times_document(tier: &str, wall: Duration, times: &BTreeMap<String, f64>) -> serde_json::Value {
    serde_json::json!({
        "tier": tier,
        "wall_secs": (wall.as_secs_f64() * 1000.0).round() / 1000.0,
        "tests": times,
    })
}

fn read_times(path: &Path) -> Result<BTreeMap<String, f64>> {
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
    )?;
    serde_json::from_value(json["tests"].clone())
        .with_context(|| format!("{} has no \"tests\" map", path.display()))
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    caps::require()?;
    let options = parse_options(args)?;
    let meta = metadata(root)?;
    let tier = if options.slow { "slow" } else { "default" };

    if options.slow && meta.slow_packages.is_empty() {
        println!("test-budget: no package has a `slow` feature; the slow tier is empty");
        return Ok(());
    }

    let profile = if options.slow {
        "slow"
    } else if std::env::var_os("CI").is_some_and(|v| !v.is_empty()) {
        "ci"
    } else {
        "default"
    };
    let mut nextest_args: Vec<String> = vec![
        "--profile".into(),
        profile.into(),
        // The slow tier is empty until P6, and a default tier may hold only ignored trials.
        "--no-tests=pass".into(),
    ];
    nextest_args.extend(selection(&options, &meta));

    // Build first, so the wall time below is the run and not the compile.
    let mut build_command = Command::new("cargo");
    build_command
        .current_dir(root)
        .args(["nextest", "run", "--no-run"])
        .args(&nextest_args);
    let build = caps::apply(&mut build_command)
        .status()
        .context("start cargo nextest")?;
    if !build.success() {
        bail!("the test build failed");
    }

    let junit = meta
        .target_dir
        .join("nextest")
        .join(profile)
        .join("junit.xml");
    let _ = std::fs::remove_file(&junit);

    let deadline = if options.slow {
        options.deadline.unwrap_or(SLOW_DEADLINE)
    } else {
        TIER_LIMIT
    };
    let pidfile = junit.with_file_name("test-pids");
    let _ = std::fs::remove_file(&pidfile);
    if let Some(dir) = pidfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let run = run_nextest(root, &nextest_args, deadline, &pidfile, options.slow)?;
    // First, before any step that can fail: report and kill the leftovers.
    let leftover = clean_up(&run);
    let mut failures: Vec<String> = leftover.into_iter().collect();
    match analyse(&options, &meta, tier, &junit, deadline, &run) {
        Ok(more) => failures.extend(more),
        Err(error) => failures.push(format!("{error:#}")),
    }

    if failures.is_empty() {
        println!("test-budget: ok");
        return Ok(());
    }
    for failure in &failures {
        eprintln!("test-budget: FAIL {failure}");
    }
    bail!("{} budget failure(s)", failures.len())
}

/// The failures that the way nextest ended gives: a run past its deadline, a failing status, or no status.
fn exit_failures(
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

/// The budget failures of a default-tier run: a test over the per-test limit, and a run over the tier limit. The slow tier
/// has neither (its deadline is the whole-run kill).
fn budget_failures(
    slow: bool,
    timed_out: bool,
    wall: Duration,
    times: &BTreeMap<String, f64>,
) -> Vec<String> {
    let mut failures = Vec::new();
    if slow {
        return failures;
    }
    for (name, secs) in sorted_slowest_first(times) {
        if secs > TEST_LIMIT_SECS {
            failures.push(format!(
                "{name} took {secs:.3} s (limit {TEST_LIMIT_SECS} s): fix it or move it to the slow tier"
            ));
        }
    }
    if !timed_out && wall > TIER_LIMIT {
        failures.push(format!(
            "the default tier took {:.1} s (limit {} s)",
            wall.as_secs_f64(),
            TIER_LIMIT.as_secs()
        ));
    }
    failures
}

/// The budget checks, the report and the files. Returns the failures; an `Err` is a failure of the command itself.
fn analyse(
    options: &Options,
    meta: &Meta,
    tier: &str,
    junit: &Path,
    deadline: Duration,
    run: &Run,
) -> Result<Vec<String>> {
    let mut failures = exit_failures(run.timed_out, &run.status, tier, deadline);

    let times = match std::fs::read_to_string(junit) {
        Ok(xml) => parse_junit(&xml),
        Err(_) if run.timed_out => BTreeMap::new(),
        Err(e) => bail!("no JUnit file at {}: {e}", junit.display()),
    };
    println!(
        "\ntest-budget: {tier} tier, {} test(s), slowest first",
        times.len()
    );
    for (name, secs) in sorted_slowest_first(&times) {
        println!("  {secs:>8.3}s  {name}");
    }
    println!("test-budget: wall time {:.1} s", run.wall.as_secs_f64());

    failures.extend(budget_failures(
        options.slow,
        run.timed_out,
        run.wall,
        &times,
    ));

    let file = meta.target_dir.join(if options.slow {
        "test-times-slow.json"
    } else {
        "test-times.json"
    });
    std::fs::create_dir_all(&meta.target_dir)?;
    std::fs::write(
        &file,
        serde_json::to_string_pretty(&times_document(tier, run.wall, &times))? + "\n",
    )?;
    println!("test-budget: wrote {}", file.display());

    if let Some(path) = &options.compare {
        if path.is_file() {
            let old = read_times(path)?;
            let doubled = doublings(&old, &times);
            if doubled.is_empty() {
                println!(
                    "test-budget: no test at least doubled against {}",
                    path.display()
                );
            } else {
                println!(
                    "test-budget: {} test(s) at least doubled against {} (a review finding, not a failure)",
                    doubled.len(),
                    path.display()
                );
                for (name, before, now) in doubled {
                    println!("  {before:.3}s -> {now:.3}s  {name}");
                }
            }
        } else {
            println!(
                "test-budget: no baseline at {}; nothing to compare",
                path.display()
            );
        }
    }
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(rows: &[(&str, f64)]) -> BTreeMap<String, f64> {
        rows.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    /// A leftover pid is a process and `-N` is a group; any other text is no target.
    #[test]
    fn a_cleanup_argument_names_a_process_or_a_group() {
        assert_eq!(target("42"), Some(Target::Process(42)));
        assert_eq!(target("-42"), Some(Target::Group(42)));
        for arg in ["", "-", "x", "-x", "--42", "4 2"] {
            assert_eq!(target(arg), None, "{arg}");
        }
    }

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("10m").unwrap(), Duration::from_secs(600));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert!(parse_duration("10").is_err());
    }

    #[test]
    fn deadline_needs_slow() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse_options(&args(&["--deadline", "5m"])).is_err());
        let o = parse_options(&args(&["--slow", "--deadline", "5m"])).unwrap();
        assert_eq!(o.deadline, Some(Duration::from_secs(300)));
    }

    #[test]
    fn junit_times_are_read_per_test() {
        let xml = r#"<testsuite><testcase name="a::b" classname="crate::bin/x" timestamp="t" time="0.250"><error type="e"/></testcase>
            <testcase name="c&amp;d" classname="y" time="1.5"/></testsuite>"#;
        let times = parse_junit(xml);
        assert_eq!(times["crate::bin/x a::b"], 0.25);
        assert_eq!(times["y c&d"], 1.5);
    }

    #[test]
    fn every_doubling_is_reported_and_a_zero_baseline_counts() {
        let old = map(&[
            ("a", 0.5),
            ("b", 0.01),
            ("c", 1.0),
            ("z", 0.0),
            ("y", 0.0005),
            ("w", 0.0),
        ]);
        let new = map(&[
            ("a", 1.0),
            ("b", 0.02),
            ("c", 1.9),
            ("d", 5.0),
            ("z", 0.001),
            ("y", 0.001),
            ("w", 0.0),
        ]);
        let found = doublings(&old, &new);
        let names: Vec<&str> = found.iter().map(|d| d.0.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "y", "z"]);
    }

    #[test]
    fn the_tier_environment_sets_seeds_and_the_bolero_pin_for_the_default_tier_only() {
        let default = tier_env(false);
        assert!(default.contains(&("BOTSTER_SEEDS", "0-31")));
        assert!(default
            .iter()
            .any(|(k, v)| *k == "BOLERO_RANDOM_SEED" && !v.is_empty()));
        assert!(default
            .iter()
            .any(|(k, v)| *k == "BOLERO_RANDOM_ITERATIONS" && !v.is_empty()));
        assert_eq!(tier_env(true), [("BOTSTER_SEEDS", "0")]);
    }

    fn meta(slow: &[&str]) -> Meta {
        Meta {
            target_dir: PathBuf::from("/t"),
            contracts_root: PathBuf::from("/c"),
            members: Vec::new(),
            slow_packages: slow.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn opts(slow: bool) -> Options {
        Options {
            slow,
            deadline: None,
            compare: None,
        }
    }

    #[test]
    fn the_default_tier_runs_the_whole_workspace() {
        assert_eq!(selection(&opts(false), &meta(&["a"])), ["--workspace"]);
    }

    #[test]
    fn the_slow_tier_runs_the_slow_packages_with_their_feature_and_the_slow_binaries() {
        let args = selection(&opts(true), &meta(&["a", "b"]));
        assert_eq!(
            args,
            [
                "-p",
                "a",
                "-p",
                "b",
                "--features",
                "a/slow,b/slow",
                "-E",
                "binary(/^slow/) | test(/(^|::)slow_/)"
            ]
        );
    }

    #[test]
    fn slowest_first_orders_by_time_then_name() {
        let times = map(&[("b", 1.0), ("a", 1.0), ("c", 3.0)]);
        let order: Vec<&str> = sorted_slowest_first(&times)
            .into_iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert_eq!(order, ["c", "a", "b"]);
    }

    #[test]
    fn the_pid_file_is_read_line_by_line_and_a_missing_file_is_empty() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let file = root.path().join("pids");
        std::fs::write(&file, "12\n x\n 34 \n\n").unwrap();
        assert_eq!(read_pids(&file), [12, 34]);
        assert!(read_pids(&root.path().join("none")).is_empty());
    }

    #[test]
    fn the_times_document_names_the_tier_and_keeps_the_times() {
        let doc = times_document("default", Duration::from_millis(1234), &map(&[("a", 0.5)]));
        assert_eq!(doc["tier"], "default");
        assert_eq!(doc["wall_secs"], 1.234);
        assert_eq!(doc["tests"]["a"], 0.5);
    }

    #[test]
    fn times_are_read_back_from_a_times_document() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let file = root.path().join("t.json");
        let doc = times_document(
            "default",
            Duration::from_secs(1),
            &map(&[("a", 0.5), ("b", 2.0)]),
        );
        std::fs::write(&file, doc.to_string()).unwrap();
        assert_eq!(read_times(&file).unwrap(), map(&[("a", 0.5), ("b", 2.0)]));
        std::fs::write(&file, "{}").unwrap();
        assert!(read_times(&file).is_err());
    }

    #[test]
    fn a_test_over_two_seconds_fails_the_default_tier_and_exactly_two_does_not() {
        let wall = Duration::from_secs(5);
        assert!(budget_failures(false, false, wall, &map(&[("a", 2.0), ("b", 0.1)])).is_empty());
        let failures = budget_failures(false, false, wall, &map(&[("a", 2.001), ("b", 0.1)]));
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("a took 2.001"));
    }

    #[test]
    fn a_default_tier_over_sixty_seconds_fails_and_sixty_does_not() {
        let none = map(&[]);
        assert!(budget_failures(false, false, Duration::from_secs(60), &none).is_empty());
        assert_eq!(
            budget_failures(false, false, Duration::from_millis(60_001), &none).len(),
            1
        );
        // A killed run reports its deadline instead.
        assert!(budget_failures(false, true, Duration::from_secs(90), &none).is_empty());
    }

    #[test]
    fn the_slow_tier_has_no_per_test_or_tier_limit() {
        assert!(
            budget_failures(true, false, Duration::from_secs(900), &map(&[("a", 50.0)])).is_empty()
        );
    }

    /// #220 R1-1: a bounded run fails for each process that it left behind, for passing its deadline, and for output still
    /// open at the deadline, each with its own message; a run with none of them has no failure.
    #[test]
    fn the_failures_of_a_bounded_run_are_its_leftovers_its_deadline_and_its_open_output() {
        let d = SLOW_DEADLINE;
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

/// The bounds of the pending-real run (#220 R1-1), with real processes.
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;

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

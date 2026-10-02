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
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A default-tier test that runs longer than this fails (BUILD.md Testing rule 4).
const TEST_LIMIT_SECS: f64 = 2.0;
/// The default tier as a whole must finish within this.
const TIER_LIMIT: Duration = Duration::from_secs(60);
/// The slow tier deadline when `--deadline` is not given.
const SLOW_DEADLINE: Duration = Duration::from_secs(10 * 60);
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

fn kill(args: &[String]) {
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--"])
        .args(args)
        .status();
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
    // A slow test is an integration-test target named `slow` or `slow_*`.
    args.push("-E".into());
    args.push("binary(/^slow/)".into());
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
    let started = Instant::now();
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["nextest", "run"])
        .args(args)
        .env("BOTSTER_TEST_PIDFILE", pidfile)
        .envs(tier_env(slow))
        .process_group(0);
    let mut child = caps::apply(&mut command)
        .spawn()
        .context("start cargo nextest (install it with `cargo install cargo-nextest --locked`)")?;
    let group = child.id();

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

/// The budget checks, the report and the files. Returns the failures; an `Err` is a failure of the command itself.
fn analyse(
    options: &Options,
    meta: &Meta,
    tier: &str,
    junit: &Path,
    deadline: Duration,
    run: &Run,
) -> Result<Vec<String>> {
    let mut failures: Vec<String> = Vec::new();
    if run.timed_out {
        failures.push(format!(
            "the {tier} tier passed its {} s deadline; the run was killed",
            deadline.as_secs()
        ));
    } else {
        match &run.status {
            Ok(status) if status.success() => {}
            Ok(status) => failures.push(format!("nextest failed ({status})")),
            Err(why) => failures.push(format!("could not wait for nextest: {why}")),
        }
    }

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

    if !options.slow {
        for (name, secs) in sorted_slowest_first(&times) {
            if secs > TEST_LIMIT_SECS {
                failures.push(format!(
                    "{name} took {secs:.3} s (limit {TEST_LIMIT_SECS} s): fix it or move it to the slow tier"
                ));
            }
        }
        if !run.timed_out && run.wall > TIER_LIMIT {
            failures.push(format!(
                "the default tier took {:.1} s (limit {} s)",
                run.wall.as_secs_f64(),
                TIER_LIMIT.as_secs()
            ));
        }
    }

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
}

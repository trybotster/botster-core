//! `cargo xtask test-budget`: run a test tier under cargo-nextest and enforce its budgets
//! (BUILD.md "Testing: fast and lean", plan section 8 step 6; adapted from the xtask of botster-contracts).
//!
//! The default tier fails when a test runs over 2 s, when the whole run takes over 60 s, or when a
//! process outlives the run. The slow tier has no per-test limit and one whole-run deadline.
//! The command is unix-only, like the rest of the CI.

use crate::caps;
use crate::fsutil::{metadata, Meta};
use anyhow::{bail, Context, Result};
use botster_core_testkit::pending;
use botster_gate_kit::bounded::{
    bounded_report, bounded_success, clean_up, exit_failures, remove_stale, run_bounded, Run,
};
use botster_gate_kit::junit::{junit_path, parse_junit};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// A default-tier test that runs longer than this fails (BUILD.md Testing rule 4).
pub(crate) const TEST_LIMIT_SECS: f64 = 2.0;
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

/// The name of the build-root test that `botster_test_process::built_in_test!()` and `built_in_trial!()` make (bug catalog
/// B13; design `design-gs-built-in-coverage.md`). xtask holds its own copy: the shared test crate exports no item with the
/// name (review B306-I2). If the copies differ, every binary fails the presence check of the `slow` step.
pub const BUILT_IN_TEST: &str = "this_test_binary_was_built_in_this_worktree";

/// The nextest filter of the slow tier. The last term selects the build-root test of every test binary ([`BUILT_IN_TEST`];
/// bug catalog B13, lead 2026-10-12): the slow tier builds the libraries and binaries again with their `slow` features, and
/// each of those artifacts runs its own check.
pub const SLOW_FILTER: &str =
    "binary(/^slow/) | test(/(^|::)slow_/) | test(/^this_test_binary_was_built_in_this_worktree$/)";

/// The packages and filter that a tier runs.
fn selection(options: &Options, meta: &Meta) -> Vec<String> {
    tier_selection(options.slow, &meta.slow_packages)
}

/// The packages and filter of the slow tier (`slow`) or of the default tier, as nextest arguments: `slow_packages` are the
/// workspace members with a `slow` feature. `slow_coverage` lists the tests of both tiers with the same arguments.
pub fn tier_selection(slow: bool, slow_packages: &[String]) -> Vec<String> {
    if !slow {
        return vec!["--workspace".into()];
    }
    let mut args = Vec::new();
    for package in slow_packages {
        args.push("-p".to_string());
        args.push(package.clone());
    }
    let features: Vec<String> = slow_packages.iter().map(|p| format!("{p}/slow")).collect();
    args.push("--features".into());
    args.push(features.join(","));
    // A slow test is an integration-test target named `slow` or `slow_*`, or a unit test of any target in a module named
    // `slow_*` at any depth (nextest matches the whole module path): the real-disk storage of a library, or the real
    // driver of a binary, which only its own unit tests reach.
    args.push("-E".into());
    args.push(SLOW_FILTER.into());
    args
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

/// Runs the real tier's pending-real trials (`cargo test -p botster-core --features slow --test slow_conformance --
/// --ignored`) under the slow tier's bounds: its own process group, `deadline`, the process tracker and the leftover check
/// (#220 R1-1). A pending-real id never fails the run, and its outcome is in the report that the binary prints: returns
/// that stdout, and whether the binary succeeded. A run past `deadline`, a process that it left behind, or a report pipe
/// still open at `deadline` fails.
///
/// # Errors
/// The run could not start, passed its deadline, or left processes behind.
pub fn pending_real(root: &Path, deadline: Duration) -> Result<(bool, String)> {
    let mut command = crate::tools::cargo(root);
    command
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
    bounded_conformance(
        root,
        command,
        deadline,
        "pending-real-pids",
        "start the real-tier conformance binary",
    )
}

/// Runs the held-full run of the testkit conformance binary (`-p botster-core --test conformance`, the binary of the `lists`
/// step) under nextest with the default tier's profile ([`default_tier_profile`]; plan 23cn, lead 2026-10-12): the CI seed
/// set, the strict and the held-full variables, and no selection variable. Each `budget` held id of
/// `conformance/core-held.txt` runs at every seed, one trial a seed, and each trial is a nextest test with the default
/// tier's 2 s limit (`.config/nextest.toml`). The run has the slow tier's bounds otherwise: `deadline`, its own process
/// group, the process tracker and the leftover check. Returns whether nextest succeeded, and the per-trial times of its
/// JUnit file (`<binary id> <test name>` to seconds).
///
/// # Errors
/// The run could not start, passed its deadline, left processes behind, or wrote no JUnit file.
pub fn held_full(root: &Path, deadline: Duration) -> Result<(bool, BTreeMap<String, f64>)> {
    caps::require()?;
    let meta = metadata(root)?;
    let profile = default_tier_profile();
    let junit = junit_path(&meta.target_dir, profile);
    // The JUnit file of an earlier run (the test-budget step writes the same file) must never count for this one.
    remove_stale(&junit)?;
    let mut command = crate::tools::cargo(root);
    command
        .args([
            "nextest",
            "run",
            "-p",
            "botster-core",
            "--test",
            "conformance",
            "--locked",
            "--profile",
            profile,
            // Every trial runs, so the report names each one over the limit.
            "--no-fail-fast",
            "--no-tests=fail",
        ])
        .envs(tier_env(false))
        .env(pending::STRICT_ENV, "1")
        .env(pending::HELD_FULL_ENV, "1");
    for var in crate::ci::SELECTION_VARS {
        command.env_remove(var);
    }
    let pidfile = meta
        .target_dir
        .join("nextest")
        .join("slow")
        .join("held-full-pids");
    let success = bounded_success(
        command,
        deadline,
        &pidfile,
        "start cargo nextest for the held-full run",
    )?;
    let xml = std::fs::read_to_string(&junit).with_context(|| {
        format!(
            "the held-full run wrote no JUnit file at {}",
            junit.display()
        )
    })?;
    Ok((success, parse_junit(&xml)))
}

/// The nextest profile of the default tier: `ci` under CI (no retries), else `default`. Both have the 2 s per-test limit.
pub fn default_tier_profile() -> &'static str {
    profile_under(std::env::var_os("CI"))
}

/// The default tier's profile for the value of `CI`: `ci` when it is set and not empty, else `default`.
fn profile_under(ci: Option<std::ffi::OsString>) -> &'static str {
    if ci.is_some_and(|v| !v.is_empty()) {
        "ci"
    } else {
        "default"
    }
}

/// The seeds of the CI seed set: the held-full run must have a trial of each `budget` held id at each of them.
pub fn ci_seeds() -> Option<Vec<u64>> {
    seeds_in(DEFAULT_SEEDS)
}

/// The seeds of a range `<from>-<to>`, both ends included; `None` for any other text.
fn seeds_in(range: &str) -> Option<Vec<u64>> {
    let (from, to) = range.split_once('-')?;
    let (from, to): (u64, u64) = (from.parse().ok()?, to.parse().ok()?);
    (from <= to).then(|| (from..=to).collect())
}

/// Runs a conformance binary's `command` (a capped `tools::cargo` command) with [`bounded_report`], with the pid file `pids`
/// of the slow tier's directory, which it empties first.
fn bounded_conformance(
    root: &Path,
    command: Command,
    deadline: Duration,
    pids: &str,
    start: &'static str,
) -> Result<(bool, String)> {
    caps::require()?;
    let meta = metadata(root)?;
    let pidfile = meta.target_dir.join("nextest").join("slow").join(pids);
    let _ = std::fs::remove_file(&pidfile);
    if let Some(dir) = pidfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    bounded_report(command, deadline, &pidfile, start)
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
    } else {
        default_tier_profile()
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

    let junit = junit_path(&meta.target_dir, profile);
    // A report of an earlier run must never count for this one (the named proofs read it after the slow tier).
    remove_stale(&junit)?;

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

    /// The default tier's profile is `ci` under CI and `default` elsewhere; both have the 2 s per-test limit, which the
    /// held-full run uses too (plan 23cn).
    #[test]
    fn the_default_tier_profile_is_ci_under_ci_and_default_elsewhere() {
        let ci = std::env::var_os("CI").is_some_and(|v| !v.is_empty());
        assert_eq!(default_tier_profile(), if ci { "ci" } else { "default" });
        assert_eq!(profile_under(None), "default");
        assert_eq!(profile_under(Some("".into())), "default");
        assert_eq!(profile_under(Some("true".into())), "ci");
    }

    /// The held-full run must have a trial at each of the 32 seeds of the CI set; a range has both ends, and other text has
    /// no seeds.
    #[test]
    fn the_ci_seed_set_has_32_seeds() {
        assert_eq!(ci_seeds(), Some((0..32).collect()));
        assert_eq!(seeds_in("3-5"), Some(vec![3, 4, 5]));
        assert_eq!(seeds_in("4-4"), Some(vec![4]));
        for text in ["5-3", "0", "a-3", "0-b", ""] {
            assert_eq!(seeds_in(text), None, "{text}");
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
                "binary(/^slow/) | test(/(^|::)slow_/) | test(/^this_test_binary_was_built_in_this_worktree$/)"
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
}

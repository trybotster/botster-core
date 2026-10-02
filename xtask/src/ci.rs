//! `cargo xtask ci [--job <name>] [--keep-going] [--list]`: every step of the merge gate (plan section 8), in order.
//!
//! The gate is one run on the exact head: `botsterq run --exclusive --deadline <d> -- env CARGO_BUILD_JOBS=4
//! NEXTEST_TEST_THREADS=4 cargo xtask ci`. A step whose tool is missing fails; nothing is skipped silently, so a green run
//! is the full gate. The pending and deferred file checks (`lists`) run right after the taint check, because they are fast.

use crate::tools::{cargo, cargo_nightly, ensure_nightly, require_cargo_tool, run};
use crate::{caps, fsutil, lists, prebuild, public_api, taint, test_budget, timers};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const USAGE: &str = "usage: cargo xtask ci [--job <name>] [--keep-going] [--list]

  (no flag)     run every step in order; stop at the first failure
  --job <name>  run one step
  --keep-going  run every step even after a failure
  --list        print the step names";

type JobFn = fn(&Path) -> Result<()>;

const JOBS: &[(&str, &str, JobFn)] = &[
    ("fmt", "cargo fmt --check", fmt_job),
    (
        "clippy",
        "clippy -D warnings, with the machine-crate lints",
        clippy_job,
    ),
    (
        "taint",
        "banned old-world names; unmarked timers",
        taint_job,
    ),
    (
        "lists",
        "core-ledger-ids, core-pending and core-deferred; the four counts",
        lists_job,
    ),
    (
        "public-api",
        "the facade against api/botster-core.txt",
        public_api_job,
    ),
    (
        "prebuild-worker",
        "the binaries of the real-process tier, with a sha256 manifest",
        prebuild_job,
    ),
    (
        "test-budget",
        "the default tier: 60 s, 2 s a test, no leftover process",
        test_budget_job,
    ),
    ("slow", "the slow tier", slow_job),
    ("mutants", "cargo mutants --in-diff", mutants_job),
    (
        "fuzz",
        "bolero fuzzing of the changed decoders (nightly)",
        fuzz_job,
    ),
];

/// The bolero harnesses of the repo: `(crate, harness name)`. Every byte decoder of the repo has one (plan section 8,
/// step 9). A package that adds a decoder adds its harness here, and the fuzz step runs it when the diff changes its crate.
const HARNESSES: &[(&str, &str)] = &[("botster-core-link", "link_decoder")];

/// The seconds of one fuzz run per harness.
const FUZZ_SECONDS: &str = "60s";

fn fmt_job(root: &Path) -> Result<()> {
    let mut cmd = cargo(root);
    cmd.args(["fmt", "--all", "--check"]);
    run(cmd)
}

fn clippy_job(root: &Path) -> Result<()> {
    let mut cmd = cargo(root);
    cmd.args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]);
    run(cmd)
}

fn taint_job(root: &Path) -> Result<()> {
    taint::command(root, &[])?;
    timers::command(root, &[])
}

/// The lists check, then the report of the conformance harness (its four counts).
fn lists_job(root: &Path) -> Result<()> {
    lists::command(root, &[])?;
    lists::ledger_ids_command(root, &[])?;
    let mut report = cargo(root);
    report.args([
        "test",
        "-p",
        "botster-core",
        "--test",
        "conformance",
        "--locked",
        "--",
        "--format",
        "terse",
    ]);
    run(report)
}

fn public_api_job(root: &Path) -> Result<()> {
    public_api::command(root, &[])
}

fn prebuild_job(root: &Path) -> Result<()> {
    prebuild::command(root, &[])
}

fn test_budget_job(root: &Path) -> Result<()> {
    test_budget::command(root, &[])
}

fn slow_job(root: &Path) -> Result<()> {
    test_budget::command(
        root,
        &[
            "--slow".to_string(),
            "--deadline".to_string(),
            "10m".to_string(),
        ],
    )
}

/// The paths that the diff against the base changes.
fn changed_paths(root: &Path, base: &str) -> Result<Vec<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .output()
        .context("run git diff --name-only")?;
    if !out.status.success() {
        bail!(
            "git diff --name-only failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8(out.stdout)?
        .lines()
        .map(str::to_string)
        .collect())
}

/// The names of the crates under `crates/` that a list of changed paths touches.
fn changed_crates(paths: &[String]) -> Vec<String> {
    let mut names: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            p.strip_prefix("crates/")?
                .split('/')
                .next()
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The harnesses of the crates that the diff changes.
fn harnesses_to_run<'a>(changed: &[String]) -> Vec<&'a (&'a str, &'a str)> {
    HARNESSES
        .iter()
        .filter(|(krate, _)| changed.iter().any(|c| c == krate))
        .collect()
}

fn base(root: &Path) -> Result<String> {
    let base = fsutil::base_ref();
    if !fsutil::resolves(root, &base) {
        bail!("{base} does not resolve; the landing diff has no base (fetch it, or set BOTSTER_CI_BASE_REF)");
    }
    Ok(base)
}

/// What the mutation run reports (`mutants.out/outcomes.json`).
#[derive(Debug, PartialEq, Eq)]
struct MutantSummary {
    total: u64,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
}

fn parse_outcomes(json: &str) -> Result<MutantSummary> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let n = |key: &str| {
        v[key]
            .as_u64()
            .with_context(|| format!("outcomes.json has no `{key}`"))
    };
    Ok(MutantSummary {
        total: n("total_mutants")?,
        caught: n("caught")?,
        missed: n("missed")?,
        timeout: n("timeout")?,
        unviable: n("unviable")?,
    })
}

/// Mutation tests of the code that the diff changes (plan section 8, step 8). A missed mutant or a timeout is a review
/// finding, so it fails the step.
fn mutants_job(root: &Path) -> Result<()> {
    require_cargo_tool(
        root,
        &["mutants", "--version"],
        "cargo install cargo-mutants --version 27.1.0 --locked",
    )?;
    require_cargo_tool(
        root,
        &["nextest", "--version"],
        "cargo install cargo-nextest --locked",
    )?;
    let base = base(root)?;
    let diff = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", &format!("{base}...HEAD")])
        .output()
        .context("run git diff")?;
    if !diff.status.success() {
        bail!("git diff failed: {}", String::from_utf8_lossy(&diff.stderr));
    }
    let target = root.join("target");
    std::fs::create_dir_all(&target)?;
    let diff_path = target.join("landing.diff");
    std::fs::write(&diff_path, &diff.stdout)?;

    let mut cmd = cargo(root);
    cmd.args(["mutants", "--in-diff"])
        .arg(&diff_path)
        .args([
            "--jobs",
            "1",
            "--no-shuffle",
            "--test-tool",
            "nextest",
            "--timeout-multiplier",
            "5",
        ])
        .arg("--output")
        .arg(&target)
        .args(["--", "--no-tests=pass"])
        .envs(test_budget::tier_env(false));
    // Exit status: 0 all caught (or no mutant in the diff), 2 a mutant was missed, 3 a timeout, 4 the baseline failed.
    let status = cmd.status().context("start cargo mutants")?;
    let outcomes = target.join("mutants.out/outcomes.json");
    let summary = match std::fs::read_to_string(&outcomes) {
        Ok(text) => Some(parse_outcomes(&text)?),
        Err(_) => None,
    };
    match &summary {
        Some(s) => println!(
            "mutants: {} mutants: {} caught, {} missed, {} timeout, {} unviable",
            s.total, s.caught, s.missed, s.timeout, s.unviable
        ),
        None => println!(
            "mutants: no outcomes.json (no Rust change in the diff, or the run failed early)"
        ),
    }
    if !status.success() {
        bail!("cargo mutants failed ({status}); a missed mutant or a timeout is a review finding");
    }
    Ok(())
}

/// Bolero fuzzing of the harnesses of the changed crates, on the pinned nightly (plan section 8, step 9).
fn fuzz_job(root: &Path) -> Result<()> {
    require_cargo_tool(
        root,
        &["bolero", "--version"],
        "cargo install cargo-bolero --locked",
    )?;
    ensure_nightly()?;
    let changed = changed_crates(&changed_paths(root, &base(root)?)?);
    let to_run = harnesses_to_run(&changed);
    if to_run.is_empty() {
        println!("fuzz: the diff changes no crate with a decoder harness");
        return Ok(());
    }
    for (krate, harness) in to_run {
        println!("fuzz: {krate} {harness}, {FUZZ_SECONDS}");
        let mut cmd = cargo_nightly(root);
        cmd.args(["bolero", "test", "-p", krate, harness, "-T", FUZZ_SECONDS]);
        run(cmd)?;
    }
    Ok(())
}

struct Options {
    job: Option<String>,
    keep_going: bool,
    list: bool,
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        job: None,
        keep_going: false,
        list: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--job" => options.job = Some(it.next().context("--job needs a name")?.clone()),
            "--keep-going" => options.keep_going = true,
            "--list" => options.list = true,
            other => bail!("unknown argument '{other}'\n{USAGE}"),
        }
    }
    if let Some(job) = &options.job {
        if !JOBS.iter().any(|(name, ..)| name == job) {
            let names: Vec<&str> = JOBS.iter().map(|(name, ..)| *name).collect();
            bail!("unknown step '{job}' (steps: {})", names.join(", "));
        }
    }
    Ok(options)
}

enum Status {
    Pass,
    Fail(String),
    NotRun,
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let options = parse_options(args)?;
    if options.list {
        for (name, what, _) in JOBS {
            println!("{name:<16} {what}");
        }
        return Ok(());
    }
    // Plan section 8: the xtask refuses to start a step without the parallelism cap.
    caps::require()?;
    let mut rows: Vec<(&str, Status, Duration)> = Vec::new();
    let mut failed = false;
    for (name, _, job) in JOBS {
        if options.job.as_deref().is_some_and(|only| only != *name) {
            continue;
        }
        if failed && !options.keep_going {
            rows.push((name, Status::NotRun, Duration::ZERO));
            continue;
        }
        println!("\n=== ci: {name} ===");
        let started = Instant::now();
        let status = match job(root) {
            Ok(()) => Status::Pass,
            Err(error) => {
                eprintln!("error: {error:#}");
                failed = true;
                Status::Fail(format!("{error:#}"))
            }
        };
        rows.push((name, status, started.elapsed()));
    }
    println!("\n=== ci summary ===");
    let mut total = Duration::ZERO;
    for (name, status, took) in &rows {
        total += *took;
        let (label, note) = match status {
            Status::Pass => ("PASS", String::new()),
            Status::Fail(why) => ("FAIL", why.lines().next().unwrap_or("").to_string()),
            Status::NotRun => ("NOT RUN", "an earlier step failed".into()),
        };
        println!(
            "{name:<16} {label:<8} {:>7.1} s  {note}",
            took.as_secs_f64()
        );
    }
    println!("{:<16} {:<8} {:>7.1} s", "total", "", total.as_secs_f64());
    if failed {
        bail!("ci failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Plan section 8: fmt, clippy, taint, public-api, prebuild-worker, test-budget, slow, mutants, fuzz; `lists` follows taint.
    #[test]
    fn the_steps_run_in_the_order_of_plan_section_8() {
        let names: Vec<&str> = JOBS.iter().map(|(name, ..)| *name).collect();
        assert_eq!(
            names,
            [
                "fmt",
                "clippy",
                "taint",
                "lists",
                "public-api",
                "prebuild-worker",
                "test-budget",
                "slow",
                "mutants",
                "fuzz"
            ]
        );
    }

    #[test]
    fn a_step_name_must_exist() {
        assert!(parse_options(&strings(&["--job", "nope"])).is_err());
        assert_eq!(
            parse_options(&strings(&["--job", "fmt"]))
                .unwrap()
                .job
                .as_deref(),
            Some("fmt")
        );
        assert!(parse_options(&strings(&["--bogus"])).is_err());
    }

    #[test]
    fn crates_are_named_from_changed_paths() {
        let paths = strings(&[
            "crates/botster-core-link/src/a.rs",
            "crates/botster-core-link/tests/b.rs",
            "tests/conformance.rs",
            "xtask/src/ci.rs",
            "crates/botster-core/src/lib.rs",
        ]);
        assert_eq!(
            changed_crates(&paths),
            ["botster-core", "botster-core-link"]
        );
    }

    #[test]
    fn a_harness_runs_only_when_its_crate_changed() {
        assert_eq!(harnesses_to_run(&strings(&["botster-core-link"])).len(), 1);
        assert!(harnesses_to_run(&strings(&["botster-core-sys"])).is_empty());
    }

    #[test]
    fn every_harness_exists_in_its_crate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for (krate, harness) in HARNESSES {
            let tests = root.join("crates").join(krate).join("tests");
            let found = std::fs::read_dir(&tests)
                .unwrap()
                .filter_map(Result::ok)
                .any(|e| {
                    std::fs::read_to_string(e.path())
                        .is_ok_and(|text| text.contains(&format!("fn {harness}()")))
                });
            assert!(found, "no harness `{harness}` in {krate}");
        }
    }

    #[test]
    fn the_mutation_summary_is_read_from_outcomes_json() {
        let json =
            r#"{"total_mutants":10,"caught":7,"missed":1,"timeout":0,"unviable":2,"success":0}"#;
        assert_eq!(
            parse_outcomes(json).unwrap(),
            MutantSummary {
                total: 10,
                caught: 7,
                missed: 1,
                timeout: 0,
                unviable: 2
            }
        );
        assert!(parse_outcomes("{}").is_err());
    }
}

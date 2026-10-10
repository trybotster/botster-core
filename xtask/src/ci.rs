//! `cargo xtask ci [--job <name>] [--keep-going] [--list]`: every step of the merge gate (plan section 8), in order.
//!
//! The gate is one run on the exact head: `botsterq run --exclusive --deadline <d> -- env CARGO_BUILD_JOBS=4
//! NEXTEST_TEST_THREADS=4 cargo xtask ci`. A step whose tool is missing fails; nothing is skipped silently, so a green run
//! is the full gate. The pending and deferred file checks (`lists`) run right after the taint check, because they are fast.

use crate::tools::{cargo, cargo_nightly, ensure_nightly, require_cargo_tool, run};
use crate::{
    caps, fsutil, gate_decisions, high_tier, lists, mutants_cited, platform_code, prebuild,
    process_check, public_api, signals, taint, test_budget, timers, unsafe_exception,
};
use anyhow::{anyhow, bail, Context, Result};
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
        "banned old-world names; unmarked timers; the one unsafe_code exception; raw signal calls; real-process test code outside its owner; cited mutants tests; no excluded gate decision; platform-only code; the HIGH-path list",
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
const HARNESSES: &[(&str, &str)] = &[
    ("botster-core-link", "link_decoder"),
    ("botster-core-link", "msg_decoder"),
    ("botster-guardian-core", "guardian_command_decoder"),
    ("botster-guardian-core", "guardian_log_decoder"),
];

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
        "--all-features",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]);
    let status = cmd.status().context("start cargo clippy")?;
    crate::tools::succeeded("cargo clippy", status)
}

fn taint_job(root: &Path) -> Result<()> {
    taint::command(root, &[])?;
    timers::command(root, &[])?;
    unsafe_exception::command(root, &[])?;
    signals::command(root, &[])?;
    process_check::command(root, &[])?;
    mutants_cited::command(root, &[])?;
    gate_decisions::command(root, &[])?;
    high_tier::command(root, &[])?;
    // The mutation step derives the platform-only code on its own OS; this static step fails early, on both gate OSes,
    // when the derivation cannot place some code.
    let files = package_sources(root)?;
    for os in ["linux", "macos"] {
        let patterns = derived_exclusions(root, &files, os)?;
        println!(
            "platform-only code: {} files not compiled on {os}",
            patterns.len()
        );
    }
    Ok(())
}

/// The passed count of a conformance report: the number after `passed ` in its `conformance:` line.
fn passed_count(report: &str) -> Option<u64> {
    let line = report
        .lines()
        .find(|l| l.starts_with("conformance: passed "))?;
    line.strip_prefix("conformance: passed ")?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// Runs the conformance binary and returns its report text.
fn conformance_report(root: &Path, extra: &[&str]) -> Result<String> {
    let mut cmd = cargo(root);
    cmd.args([
        "test",
        "-p",
        "botster-core",
        "--test",
        "conformance",
        "--locked",
        "--",
        "--format",
        "terse",
    ])
    .args(extra)
    .envs(test_budget::tier_env(false));
    let out = cmd.output().context("run the conformance binary")?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    print!("{text}");
    if !out.status.success() {
        bail!(
            "the conformance binary failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(text)
}

/// The lists check, then the report of the conformance harness (its four counts). A run that asks for the ignored trials too
/// must not pass more ids: a pending or deferred id is never a pass (plan section 5).
fn lists_job(root: &Path) -> Result<()> {
    lists::command(root, &[])?;
    lists::ledger_ids_command(root, &[])?;
    let normal = conformance_report(root, &[])?;
    let with_ignored = conformance_report(root, &["--include-ignored"])?;
    let (a, b) = (passed_count(&normal), passed_count(&with_ignored));
    if a.is_none() || a != b {
        bail!("the conformance report counts {a:?} passed, and {b:?} passed when ignored trials are included");
    }
    Ok(())
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

/// The slow tier: the slow tests, with every real-tier conformance id, under nextest; then the report of the ids of
/// `conformance/core-real-pending.txt` (plan 23l). Nextest skips those ids (ignored trials); here they run once, and their
/// outcome is reported, never counted as passed and never a failure of the step.
fn slow_job(root: &Path) -> Result<()> {
    test_budget::command(
        root,
        &[
            "--slow".to_string(),
            "--deadline".to_string(),
            "10m".to_string(),
        ],
    )?;
    // The pending-real trials run under the slow tier's bounds: the same deadline, process group, tracker and leftover
    // check (#220 R1-1).
    let (success, report) = test_budget::pending_real(root, test_budget::SLOW_DEADLINE)?;
    real_report_ran(success, &report)
}

/// The verdict on the run of the real tier's pending-real ids: the binary succeeded and printed its report, with the count
/// of the pending-real ids. A pending-real id never fails the run (its outcome is in the report); a binary that failed or
/// printed no report did not run them.
fn real_report_ran(success: bool, report: &str) -> Result<()> {
    let has = |prefix: &str| report.lines().any(|line| line.starts_with(prefix));
    if !success {
        bail!("the real-tier conformance binary failed under --ignored");
    }
    if !has("real conformance: passed ") || !has("real conformance: pending-real ") {
        bail!("the real-tier conformance binary printed no pending-real report");
    }
    Ok(())
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

/// Platform coverage exceptions of the native encoder, not equivalent mutants: a gate off macOS excludes each one, and a
/// Mac gate tests it. A test catches each mutant on macOS, where the native branch that shows it is compiled; off macOS
/// that branch is not compiled, so no test there can show the mutant. `cargo mutants` adds each `--exclude-re` to the
/// configured exclusions.
/// - `EncoderState::every_key_state`, `&` to `|` or `^` of the mode bits: the mutant keeps only the all-modes-on state
///   of each kitty combination. The Mac branch is the legacy Alt prefix of pinned Ghostty src/input/key_encode.zig:642-650
///   (`builtin.os.tag == .macos`): it writes ESC and the unshifted key, not the text, so an Alt key with long text writes
///   the most with that prefix off. The regression is
///   `the_key_bound_of_an_alt_key_with_long_text_covers_the_states_with_modes_off`. When `every_key_state`, the key
///   encoding or the Ghostty pin changes, a focused Mac mutation run of `every_key_state` must show both mutants caught.
///   The proof at aeda1cac, pin 3f8eb681 (PR #167): ~/.local/state/jobq/logs/jobq-botster-core-aeda1cac-20261008222501-7480.log.
///
/// Code under a platform `cfg` is not here: [`derived_exclusions`] finds it from the attributes.
pub(crate) const OFF_MACOS_EXCLUSIONS: &[&str] = &[
    r"crates/botster-terminal-ghostty/src/encode\.rs:\d+:40: replace & with [|^] in EncoderState::every_key_state$",
];

/// The exclusions that a gate on `os` (`std::env::consts::OS`) adds to the configured ones.
fn platform_exclusions(os: &str) -> &'static [&'static str] {
    if os == "macos" {
        &[]
    } else {
        OFF_MACOS_EXCLUSIONS
    }
}

/// The tracked Rust files of the packages.
fn package_sources(root: &Path) -> Result<Vec<String>> {
    Ok(package_rust_files(fsutil::tracked_files(root)?))
}

/// The Rust files of the packages among `files`: the fixtures are left out (each one is its own workspace).
fn package_rust_files(files: Vec<String>) -> Vec<String> {
    files
        .into_iter()
        .filter(|file| file.ends_with(".rs") && !file.starts_with("xtask/fixtures/"))
        .collect()
}

/// The exclusions of the code that `os` does not compile (plan section 8, "Platform-only code"), derived from the `cfg`
/// attributes of `files` (paths relative to `root`). The run on the OS that compiles the code tests its mutants.
fn derived_exclusions(root: &Path, files: &[String], os: &str) -> Result<Vec<String>> {
    let mut sources = Vec::new();
    for file in files {
        let text =
            std::fs::read_to_string(root.join(file)).with_context(|| format!("read {file}"))?;
        sources.push((file.clone(), text));
    }
    platform_code::exclusions(&sources, os)
        .map_err(|errors| anyhow!("platform-only code on {os}:\n{}", errors.join("\n")))
}

/// The step's verdict from the exit code of `cargo mutants` (`None`: ended by a signal). Only 0, every mutant caught or
/// no mutant in the diff, passes; 2 a missed mutant, 3 a timeout, 4 a failed baseline and any other end fail the step.
fn mutation_verdict(code: Option<i32>) -> Result<()> {
    if code == Some(0) {
        return Ok(());
    }
    bail!(
        "cargo mutants failed (exit code {code:?}: 2 a missed mutant, 3 a timeout, 4 a failed baseline); a missed \
         mutant or a timeout is a review finding"
    )
}

/// The options of the mutation run, after the mutant selection.
const MUTANTS_OPTIONS: [&str; 7] = [
    "--jobs",
    "1",
    "--no-shuffle",
    "--test-tool",
    "nextest",
    "--timeout-multiplier",
    "5",
];

/// The nextest arguments of the mutation run. The `mutants` profile (`.config/nextest.toml`) never terminates a test for
/// its time, so a mutant that hangs and fails no test reaches the timeout of cargo-mutants and counts as TIMEOUT, not as
/// CAUGHT. `--max-fail 1:immediate` ends the run at the first failed test, with the tests still running: a mutant that a
/// test fails is CAUGHT even when it also hangs another test.
const MUTANTS_TEST_ARGS: [&str; 6] = [
    "--",
    "--profile",
    "mutants",
    "--max-fail",
    "1:immediate",
    "--no-tests=pass",
];

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
    let diff = landing_diff(root, &fsutil::base(root)?)?;
    let target = root.join("target");
    std::fs::create_dir_all(&target)?;
    let diff_path = target.join("landing.diff");
    std::fs::write(&diff_path, &diff)?;

    let exclusions: Vec<String> = platform_exclusions(std::env::consts::OS)
        .iter()
        .map(|re| (*re).to_string())
        .chain(derived_exclusions(
            root,
            &package_sources(root)?,
            std::env::consts::OS,
        )?)
        .collect();
    let mutants = |cmd: &mut Command| {
        cmd.args(["mutants", "--in-diff"])
            .arg(&diff_path)
            .args(MUTANTS_OPTIONS);
        for re in &exclusions {
            cmd.arg("--exclude-re").arg(re);
        }
    };
    // The mutants of the diff, listed without a build, with the run's own filters.
    let listed = diff_mutants(&String::from_utf8_lossy(&diff), || {
        let mut list = cargo(root);
        mutants(&mut list);
        list.args(["--list", "--json"]);
        list.output().context("start cargo mutants --list")
    })?;
    let Some(listed) = listed else {
        println!(
            "mutants: 0 mutants: the diff changes no Rust source, so cargo mutants does not run"
        );
        return Ok(());
    };
    // An outcomes file left by an earlier run must not stand for this one.
    let fresh = |dir: &Path| -> Result<std::path::PathBuf> {
        let out = dir.join("mutants.out");
        if out.exists() {
            std::fs::remove_dir_all(&out).with_context(|| format!("remove {}", out.display()))?;
        }
        Ok(out)
    };
    let outcomes_of = |out: &Path| -> Result<Option<Outcomes>> {
        match std::fs::read_to_string(out.join("outcomes.json")) {
            Ok(text) => Ok(Some(parse_stage_outcomes(&text)?)),
            Err(_) => Ok(None),
        }
    };
    // Stage 1 writes `target/mutants.out`, as before 23n; stage 2 writes `target/mutants-stage2/mutants.out`. The pool
    // keeps both of a failed gate (ci/remote/job.sh).
    let stage1_dir = target.clone();
    let stage2_dir = target.join("mutants-stage2");
    let lines = two_stage_decision(
        listed,
        || {
            let out = fresh(&stage1_dir)?;
            let mut cmd = cargo(root);
            mutants(&mut cmd);
            cmd.arg("--output").arg(&stage1_dir);
            cmd.args(MUTANTS_TEST_ARGS)
                .envs(test_budget::tier_env(false));
            let started = Instant::now();
            let status = cmd.status().context("start cargo mutants (stage 1)")?;
            let took = started.elapsed();
            Ok(Stage {
                code: status.code(),
                outcomes: outcomes_of(&out)?,
                took,
            })
        },
        |missed| {
            let out = fresh(&stage2_dir)?;
            let slow_packages = fsutil::metadata(root)?.slow_packages;
            let selection = ["--in-diff".to_string(), diff_path.display().to_string()];
            let mut cmd = cargo(root);
            cmd.args(stage2_args(
                &selection,
                &slow_packages,
                &exclusions,
                missed,
                &stage2_dir,
            ))
            .envs(test_budget::tier_env(true));
            let started = Instant::now();
            let status = cmd.status().context("start cargo mutants (stage 2)")?;
            let took = started.elapsed();
            Ok(Stage {
                code: status.code(),
                outcomes: outcomes_of(&out)?,
                took,
            })
        },
    )?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

/// The landing diff, `base...HEAD`, in plain unified form whatever the git configuration: no color (`color.diff`,
/// `color.ui`) and no external diff tool (`diff.external`). cargo-mutants reads it, and `changes_rust_source` decides from
/// its `+++` lines, so a colored header would hide a Rust file and skip the mutants.
///
/// # Errors
/// `git diff` did not start or failed.
fn landing_diff(root: &Path, base: &str) -> Result<Vec<u8>> {
    let diff = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "diff",
            "--no-color",
            "--no-ext-diff",
            &format!("{base}...HEAD"),
        ])
        .output()
        .context("run git diff")?;
    if !diff.status.success() {
        bail!("git diff failed: {}", String::from_utf8_lossy(&diff.stderr));
    }
    Ok(diff.stdout)
}

/// Whether the diff changes a Rust source file, by the rule of cargo-mutants 27.1.0 (`src/in_diff.rs`): a file whose new
/// path (`+++`) is not `/dev/null` and has the extension `rs`, `build.rs` too. A deleted file has no new path. Git quotes
/// a path with special characters, and it adds a tab after a path with a space; neither hides the extension here. An added
/// line that starts with `++ ` reads as a path: it can only make the step list the mutants, never skip them. The diff must
/// be plain (`landing_diff`): a colored `+++` header does not start with `+++ `, and it would hide its Rust file.
fn changes_rust_source(diff: &str) -> bool {
    diff.lines()
        .filter_map(|line| line.strip_prefix("+++ "))
        .map(|path| path.trim_end_matches('\t').trim_matches('"'))
        .any(|path| path != "/dev/null" && Path::new(path).extension() == Some("rs".as_ref()))
}

/// The number of mutants of the diff (`list` runs `cargo mutants --list --json`), or `None` when the diff changes no Rust
/// source (`changes_rust_source`). Then `--in-diff` has no mutant to list, so `list` does not run: the step does not
/// depend on how cargo-mutants logs an empty or a non-Rust diff.
///
/// # Errors
/// The listing did not start or failed (`parse_listing`).
fn diff_mutants(
    diff: &str,
    list: impl FnOnce() -> Result<std::process::Output>,
) -> Result<Option<usize>> {
    if !changes_rust_source(diff) {
        return Ok(None);
    }
    parse_listing(&list()?).map(Some)
}

/// What cargo-mutants 27.1.0 logs, with no output, when the filters (`--in-diff`, `exclude_re`) leave no mutant.
const NO_MUTANT_LOG: &str = "No mutants to filter";

/// The number of mutants that `cargo mutants --list --json` lists (a JSON array, one object per mutant). When the filters
/// leave no mutant, cargo-mutants prints nothing and logs `NO_MUTANT_LOG`: that is 0.
///
/// # Errors
/// The listing failed, or its output is not a JSON array.
fn parse_listing(listing: &std::process::Output) -> Result<usize> {
    let json = crate::tools::stdout_of(listing, "cargo mutants --list")?;
    if json.trim().is_empty() && String::from_utf8_lossy(&listing.stderr).contains(NO_MUTANT_LOG) {
        return Ok(0);
    }
    let v: serde_json::Value = serde_json::from_str(&json).context("parse the mutant listing")?;
    v.as_array()
        .map(Vec::len)
        .context("the mutant listing is not an array")
}

/// What one mutation stage's `outcomes.json` reports: the counts, and the names of the missed mutants (the names that
/// cargo-mutants lists and that `-F` matches: `<file>:<line>:<column>: <change>`).
#[derive(Debug, PartialEq)]
struct Outcomes {
    summary: MutantSummary,
    missed: Vec<String>,
}

/// The outcomes of a stage (`Outcomes`) from its `outcomes.json`.
///
/// # Errors
/// The file has no counts (`parse_outcomes`), no `outcomes` array, or a missed outcome without a mutant name.
fn parse_stage_outcomes(json: &str) -> Result<Outcomes> {
    let summary = parse_outcomes(json)?;
    let v: serde_json::Value = serde_json::from_str(json)?;
    let missed = v["outcomes"]
        .as_array()
        .context("outcomes.json has no `outcomes`")?
        .iter()
        .filter(|o| o["summary"] == "MissedMutant")
        .map(|o| {
            o["scenario"]["Mutant"]["name"]
                .as_str()
                .map(str::to_string)
                .context("a missed outcome in outcomes.json has no mutant name")
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Outcomes { summary, missed })
}

/// One mutation stage: its exit code (`None`: ended by a signal), its outcomes (`None`: no `outcomes.json`), and its time.
struct Stage {
    code: Option<i32>,
    outcomes: Option<Outcomes>,
    took: Duration,
}

/// The report line of a stage that had to test `expected` mutants, with its outcomes.
///
/// # Errors
/// The stage wrote no outcomes (it ended before them: a failed start, a failed build, a signal), or outcomes for another
/// number of mutants.
fn stage_outcomes(
    name: &str,
    expected: usize,
    stage: Stage,
) -> Result<(Option<i32>, Outcomes, String)> {
    let Some(outcomes) = stage.outcomes else {
        bail!(
            "mutants: {name}: cargo mutants wrote no outcomes.json for its {expected} mutants (exit code {:?}): the run \
             failed before its outcomes",
            stage.code
        );
    };
    let s = &outcomes.summary;
    let line = format!(
        "mutants: {name}: {} mutants: {} caught, {} missed, {} timeout, {} unviable, in {:.1} s",
        s.total,
        s.caught,
        s.missed,
        s.timeout,
        s.unviable,
        stage.took.as_secs_f64()
    );
    if usize::try_from(s.total).ok() != Some(expected) {
        bail!("{line}; it had {expected} mutants to test: the run did not report every one");
    }
    Ok((stage.code, outcomes, line))
}

/// The verdict of the mutation step in two stages (plan section 8, revisions 23n and 23r), from the number of mutants that
/// the diff lists and the two runs. With no mutant listed, no run starts. Stage 1 is the default run; it must report every
/// listed mutant. A timeout or a failed baseline in stage 1 fails the step (`mutation_verdict`). A mutant that stage 1
/// misses is not yet a failure: stage 2 (`stage2`, the slow tier with the `slow` features) tests exactly the missed
/// mutants, and the step fails on a mutant that it also misses, or on a timeout there. The lines give each stage's counts
/// and time, then the step's counts.
///
/// # Errors
/// A stage failed to start, wrote no outcomes or outcomes for another number of mutants, or its exit code fails.
fn two_stage_decision(
    listed: usize,
    stage1: impl FnOnce() -> Result<Stage>,
    stage2: impl FnOnce(&[String]) -> Result<Stage>,
) -> Result<Vec<String>> {
    if listed == 0 {
        return Ok(vec![
            "mutants: the diff has no mutant (cargo mutants --list), so no run starts".into(),
        ]);
    }
    let (code, first, line) = stage_outcomes("stage 1 (default)", listed, stage1()?)?;
    let mut lines = vec![line.clone()];
    let s = &first.summary;
    let missed_only = code == Some(2) && s.timeout == 0;
    if !missed_only {
        mutation_verdict(code).with_context(|| line.clone())?;
        lines.push("mutants: stage 2 (slow): stage 1 missed no mutant, so it does not run".into());
        lines.push(format!(
            "mutants: {} mutants: {} caught, 0 missed, 0 timeout, {} unviable",
            s.total, s.caught, s.unviable
        ));
        return Ok(lines);
    }
    anyhow::ensure!(
        u64::try_from(first.missed.len()).ok() == Some(s.missed) && s.missed > 0,
        "{line}; outcomes.json names {} missed mutants",
        first.missed.len()
    );
    let (code2, second, line2) =
        stage_outcomes("stage 2 (slow)", first.missed.len(), stage2(&first.missed)?)?;
    lines.push(line2.clone());
    mutation_verdict(code2).with_context(|| {
        format!("{line2}: a mutant that both stages miss, or a timeout in stage 2, fails the step")
    })?;
    let t = &second.summary;
    lines.push(format!(
        "mutants: {} mutants: {} caught ({} by stage 2), 0 missed, 0 timeout, {} unviable",
        s.total,
        s.caught + t.caught,
        t.caught,
        s.unviable + t.unviable
    ));
    Ok(lines)
}

/// The arguments of stage 2 (plan 23r) after `cargo`: the mutants of `selection` (the diff), in place (so serial), with
/// the slow features of `slow_packages`, their tests, and the slow tier's filter; exactly the stage-1 misses (`-F` with
/// each whole name); the same exclusions; and the slow tier's deadline as the timeout of each mutant.
fn stage2_args(
    selection: &[String],
    slow_packages: &[String],
    exclusions: &[String],
    missed: &[String],
    output: &Path,
) -> Vec<String> {
    let mut args = vec!["mutants".to_string()];
    args.extend(selection.iter().cloned());
    args.extend(
        [
            "--in-place",
            "--no-shuffle",
            "--test-tool",
            "nextest",
            "--timeout",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    args.push(test_budget::SLOW_DEADLINE.as_secs().to_string());
    args.push("--features".into());
    args.push(
        slow_packages
            .iter()
            .map(|p| format!("{p}/slow"))
            .collect::<Vec<_>>()
            .join(","),
    );
    for package in slow_packages {
        args.push("--test-package".into());
        args.push(package.clone());
    }
    for re in exclusions {
        args.push("--exclude-re".into());
        args.push(re.clone());
    }
    for name in missed {
        args.push("-F".into());
        args.push(format!("^{}$", regex::escape(name)));
    }
    args.push("--output".into());
    args.push(output.display().to_string());
    args.extend(MUTANTS_TEST_ARGS.iter().map(|s| s.to_string()));
    args.push("-E".into());
    args.push(test_budget::SLOW_FILTER.into());
    args
}

/// Bolero fuzzing of the harnesses of the changed crates, on the pinned nightly (plan section 8, step 9).
fn fuzz_job(root: &Path) -> Result<()> {
    require_cargo_tool(
        root,
        &["bolero", "--version"],
        "cargo install cargo-bolero --locked",
    )?;
    ensure_nightly()?;
    let changed = changed_crates(&changed_paths(root, &fsutil::base(root)?)?);
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

/// Runs the steps in order. A failure stops the run unless `keep_going`; the steps after it are `NotRun`. `only` limits the
/// run to one step. Returns one row per step that was considered, and whether any step failed.
fn run_steps<'a>(
    root: &Path,
    jobs: &'a [(&'a str, &'a str, JobFn)],
    only: Option<&str>,
    keep_going: bool,
    out: &mut dyn FnMut(&str),
) -> (Vec<(&'a str, Status, Duration)>, bool) {
    let mut rows = Vec::new();
    let mut failed = false;
    for (name, _, job) in jobs {
        if only.is_some_and(|only| only != *name) {
            continue;
        }
        if failed && !keep_going {
            rows.push((*name, Status::NotRun, Duration::ZERO));
            continue;
        }
        out(&format!("\n=== ci: {name} ==="));
        let started = Instant::now();
        let status = match job(root) {
            Ok(()) => Status::Pass,
            Err(error) => {
                out(&format!("error: {error:#}"));
                failed = true;
                Status::Fail(format!("{error:#}"))
            }
        };
        rows.push((*name, status, started.elapsed()));
    }
    (rows, failed)
}

/// The summary line of one step.
fn summary_line(name: &str, status: &Status, took: Duration) -> String {
    let (label, note) = match status {
        Status::Pass => ("PASS", String::new()),
        Status::Fail(why) => ("FAIL", why.lines().next().unwrap_or("").to_string()),
        Status::NotRun => ("NOT RUN", "an earlier step failed".into()),
    };
    format!(
        "{name:<16} {label:<8} {:>7.1} s  {note}",
        took.as_secs_f64()
    )
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
    let (rows, failed) = run_steps(
        root,
        JOBS,
        options.job.as_deref(),
        options.keep_going,
        &mut |line| println!("{line}"),
    );
    println!("\n=== ci summary ===");
    let mut total = Duration::ZERO;
    for (name, status, took) in &rows {
        total += *took;
        println!("{}", summary_line(name, status, *took));
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

    /// A gate off macOS adds the exclusions that hold off macOS only; a Mac gate adds none, so it tests those mutants.
    #[test]
    fn only_a_gate_off_macos_adds_the_off_macos_exclusions() {
        assert!(platform_exclusions("macos").is_empty());
        assert_eq!(platform_exclusions("linux"), OFF_MACOS_EXCLUSIONS);
        assert!(!OFF_MACOS_EXCLUSIONS.is_empty());
    }

    /// Plan section 8, "Platform-only code": the mutation step on each gate OS excludes the other OS's adapters of
    /// botster-test-process, derived from their `cfg` attributes. The test reads the files from the source tree, not from
    /// git: a cargo-mutants copy has no `.git`.
    #[test]
    fn each_gate_os_excludes_the_adapters_of_the_other_os() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let files: Vec<String> = ["platform.rs", "platform/linux.rs", "platform/macos.rs"]
            .iter()
            .map(|file| format!("crates/botster-test-process/src/{file}"))
            .collect();
        let linux = derived_exclusions(root, &files, "linux").unwrap();
        let macos = derived_exclusions(root, &files, "macos").unwrap();
        let adapter = |os: &str| format!(r"^crates/botster\-test\-process/src/platform/{os}\.rs:");
        assert!(
            linux.contains(&adapter("macos")) && !linux.contains(&adapter("linux")),
            "{linux:?}"
        );
        assert!(
            macos.contains(&adapter("linux")) && !macos.contains(&adapter("macos")),
            "{macos:?}"
        );
    }

    /// The package sources of a repository are its tracked Rust files outside the fixtures.
    #[test]
    fn the_package_sources_are_read_from_the_tracked_files() {
        let repo = crate::fsutil::test_repo(&[
            ("a/lib.rs", ""),
            ("a/Cargo.toml", ""),
            ("xtask/fixtures/x/src/lib.rs", ""),
        ]);
        assert_eq!(package_sources(repo.path()).unwrap(), ["a/lib.rs"]);
    }

    /// The fixtures are their own workspaces, so their files are not package sources; files that are not Rust are not either.
    #[test]
    fn the_package_sources_are_the_rust_files_outside_the_fixtures() {
        let files = [
            "a/lib.rs",
            "xtask/fixtures/x/src/lib.rs",
            "a/Cargo.toml",
            "xtask/src/ci.rs",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(package_rust_files(files), ["a/lib.rs", "xtask/src/ci.rs"]);
    }

    /// Plan section 8, step 8: only a run with every mutant caught passes the mutation step.
    #[test]
    fn only_a_mutation_run_with_every_mutant_caught_passes() {
        assert!(mutation_verdict(Some(0)).is_ok());
        for failed in [Some(1), Some(2), Some(3), Some(4), None] {
            assert!(mutation_verdict(failed).is_err(), "{failed:?}");
        }
    }

    #[test]
    fn the_pending_real_run_passes_only_with_a_success_and_its_report() {
        let report = "real conformance: passed 0, failed 0, pending 3 (+ 1 with no transcript), deferred 2, withdrawn 17\n\
                      real conformance: pending-real 2 (they run under --ignored, never counted as passed), of which passed 1\n";
        assert!(real_report_ran(true, report).is_ok());
        assert!(real_report_ran(false, report).is_err());
        assert!(real_report_ran(true, "").is_err());
        let no_pending_line = report.lines().next().unwrap();
        assert!(real_report_ran(true, no_pending_line).is_err());
        let no_count_line = report.lines().nth(1).unwrap();
        assert!(real_report_ran(true, no_count_line).is_err());
    }

    #[test]
    fn the_passed_count_is_read_from_the_report_line() {
        let report =
            "x\nconformance: passed 12, failed 0, pending 3 (+ 4 with no transcript), deferred 2\n";
        assert_eq!(passed_count(report), Some(12));
        assert_eq!(passed_count("conformance: passed 0, failed 0"), Some(0));
        assert_eq!(passed_count("no report"), None);
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
        assert_eq!(harnesses_to_run(&strings(&["botster-core-link"])).len(), 2);
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

    fn counts(total: u64, caught: u64, missed: u64, timeout: u64, unviable: u64) -> MutantSummary {
        MutantSummary {
            total,
            caught,
            missed,
            timeout,
            unviable,
        }
    }

    /// A stage with exit code `code` and outcomes `summary`, whose missed mutants are named `m0`, `m1`, ...
    fn stage(code: Option<i32>, summary: Option<MutantSummary>) -> Result<Stage> {
        Ok(Stage {
            code,
            outcomes: summary.map(|summary| Outcomes {
                missed: (0..summary.missed).map(|i| format!("m{i}")).collect(),
                summary,
            }),
            took: Duration::from_millis(2500),
        })
    }

    /// The lead's ruling after #175: a run that failed early must not pass. No mutant listed passes with its reason and
    /// starts no run; stage 1 must report every listed mutant; with exit code 0 the step passes and stage 2 does not run.
    #[test]
    fn a_mutation_step_passes_only_with_no_mutant_or_every_outcome_and_exit_code_0() {
        let no_stage2 =
            |_: &[String]| -> Result<Stage> { panic!("stage 1 missed nothing, so no stage 2") };
        assert_eq!(
            two_stage_decision(0, || panic!("no mutant listed, so no run"), no_stage2).unwrap(),
            ["mutants: the diff has no mutant (cargo mutants --list), so no run starts"]
        );
        assert_eq!(
            two_stage_decision(3, || stage(Some(0), Some(counts(3, 2, 0, 0, 1))), no_stage2).unwrap(),
            [
                "mutants: stage 1 (default): 3 mutants: 2 caught, 0 missed, 0 timeout, 1 unviable, in 2.5 s",
                "mutants: stage 2 (slow): stage 1 missed no mutant, so it does not run",
                "mutants: 3 mutants: 2 caught, 0 missed, 0 timeout, 1 unviable",
            ]
        );
        assert_eq!(
            two_stage_decision(3, || stage(Some(0), None), no_stage2)
                .unwrap_err()
                .to_string(),
            "mutants: stage 1 (default): cargo mutants wrote no outcomes.json for its 3 mutants (exit code Some(0)): the \
             run failed before its outcomes"
        );
        assert!(
            two_stage_decision(3, || stage(None, None), no_stage2).is_err(),
            "a signal"
        );
        let failed =
            two_stage_decision(3, || Err(anyhow::anyhow!("start cargo mutants")), no_stage2);
        assert_eq!(failed.unwrap_err().to_string(), "start cargo mutants");
        let partial =
            two_stage_decision(3, || stage(Some(0), Some(counts(2, 2, 0, 0, 0))), no_stage2)
                .unwrap_err()
                .to_string();
        assert!(
            partial.ends_with("it had 3 mutants to test: the run did not report every one"),
            "{partial}"
        );
    }

    /// Plan 23n and 23r: a timeout or a failed baseline in stage 1 fails the step without stage 2; a stage-1 miss goes to
    /// stage 2 with exactly the missed names; the step passes when stage 2 catches each one, and fails on a mutant that
    /// both stages miss or on a timeout in stage 2.
    #[test]
    fn a_stage_1_miss_passes_only_when_stage_2_catches_it() {
        let no_stage2 = |_: &[String]| -> Result<Stage> { panic!("stage 1 failed, so no stage 2") };
        for (code, summary) in [
            (Some(3), counts(4, 3, 0, 1, 0)),
            (Some(2), counts(4, 2, 1, 1, 0)),
            (Some(4), counts(4, 0, 0, 0, 0)),
            (Some(1), counts(4, 4, 0, 0, 0)),
        ] {
            let error =
                two_stage_decision(4, || stage(code, Some(summary)), no_stage2).unwrap_err();
            assert!(
                format!("{error:#}").contains(&format!("exit code {code:?}")),
                "{error:#}"
            );
        }
        let seen = std::cell::RefCell::new(Vec::new());
        let caught = two_stage_decision(
            5,
            || stage(Some(2), Some(counts(5, 2, 2, 0, 1))),
            |missed| {
                seen.borrow_mut().extend(missed.iter().cloned());
                stage(Some(0), Some(counts(2, 1, 0, 0, 1)))
            },
        )
        .unwrap();
        assert_eq!(*seen.borrow(), ["m0", "m1"]);
        assert_eq!(
            caught,
            [
                "mutants: stage 1 (default): 5 mutants: 2 caught, 2 missed, 0 timeout, 1 unviable, in 2.5 s",
                "mutants: stage 2 (slow): 2 mutants: 1 caught, 0 missed, 0 timeout, 1 unviable, in 2.5 s",
                "mutants: 5 mutants: 3 caught (1 by stage 2), 0 missed, 0 timeout, 2 unviable",
            ]
        );
        let first = || stage(Some(2), Some(counts(5, 3, 2, 0, 0)));
        for (code, summary) in [
            (Some(2), counts(2, 1, 1, 0, 0)),
            (Some(3), counts(2, 1, 0, 1, 0)),
        ] {
            let error = two_stage_decision(5, first, |_| stage(code, Some(summary))).unwrap_err();
            assert!(
                format!("{error:#}")
                    .contains("a mutant that both stages miss, or a timeout in stage 2"),
                "{error:#}"
            );
        }
        let short = two_stage_decision(5, first, |_| stage(Some(0), Some(counts(1, 1, 0, 0, 0))))
            .unwrap_err()
            .to_string();
        assert!(short.contains("it had 2 mutants to test"), "{short}");
        let none = two_stage_decision(5, first, |_| stage(Some(0), None))
            .unwrap_err()
            .to_string();
        assert!(
            none.starts_with("mutants: stage 2 (slow): cargo mutants wrote no outcomes.json"),
            "{none}"
        );
        // The names of the missed mutants must match the missed count.
        let unnamed = two_stage_decision(
            5,
            || {
                let mut s = stage(Some(2), Some(counts(5, 3, 2, 0, 0)))?;
                s.outcomes.as_mut().unwrap().missed.pop();
                Ok(s)
            },
            |_| panic!("the names do not match the count"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            unnamed.ends_with("outcomes.json names 1 missed mutants"),
            "{unnamed}"
        );
        let zero = two_stage_decision(
            5,
            || stage(Some(2), Some(counts(5, 5, 0, 0, 0))),
            |_| panic!("no missed mutant to test"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            zero.ends_with("outcomes.json names 0 missed mutants"),
            "{zero}"
        );
    }

    #[test]
    fn the_missed_mutants_are_read_by_name_from_outcomes_json() {
        let json = r#"{"total_mutants":3,"caught":1,"missed":2,"timeout":0,"unviable":0,"success":0,"outcomes":[
            {"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"a.rs:1:2: replace f -> bool with true"}},"summary":"MissedMutant"},
            {"scenario":{"Mutant":{"name":"a.rs:3:4: replace g with ()"}},"summary":"CaughtMutant"},
            {"scenario":{"Mutant":{"name":"b.rs:5:6: replace + with - in h"}},"summary":"MissedMutant"}]}"#;
        let outcomes = parse_stage_outcomes(json).unwrap();
        assert_eq!(outcomes.summary, counts(3, 1, 2, 0, 0));
        assert_eq!(
            outcomes.missed,
            [
                "a.rs:1:2: replace f -> bool with true",
                "b.rs:5:6: replace + with - in h"
            ]
        );
        let no_list = r#"{"total_mutants":0,"caught":0,"missed":0,"timeout":0,"unviable":0}"#;
        assert!(parse_stage_outcomes(no_list).is_err());
        let unnamed = r#"{"total_mutants":1,"caught":0,"missed":1,"timeout":0,"unviable":0,"outcomes":[
            {"scenario":{"Mutant":{}},"summary":"MissedMutant"}]}"#;
        assert!(parse_stage_outcomes(unnamed).is_err());
    }

    /// Plan 23r: stage 2 runs in place with the slow features, the slow packages' tests, the slow filter, the never
    /// terminating profile and the slow deadline as its timeout, and tests exactly each missed mutant, by its whole name.
    #[test]
    fn stage_2_tests_exactly_the_missed_mutants_with_the_slow_tier() {
        let args = stage2_args(
            &["--in-diff".into(), "d.diff".into()],
            &["p".into(), "q".into()],
            &["x\\.rs".into()],
            &["a.rs:1:2: replace f -> bool with true".into()],
            Path::new("out"),
        );
        let expected: Vec<String> = [
            "mutants",
            "--in-diff",
            "d.diff",
            "--in-place",
            "--no-shuffle",
            "--test-tool",
            "nextest",
            "--timeout",
            "600",
            "--features",
            "p/slow,q/slow",
            "--test-package",
            "p",
            "--test-package",
            "q",
            "--exclude-re",
            "x\\.rs",
            "-F",
            r"^a\.rs:1:2: replace f \-> bool with true$",
            "--output",
            "out",
            "--",
            "--profile",
            "mutants",
            "--max-fail",
            "1:immediate",
            "--no-tests=pass",
            "-E",
            "binary(/^slow/) | test(/(^|::)slow_/)",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(args, expected);
        assert_eq!(test_budget::SLOW_DEADLINE.as_secs(), 600);
        let pattern = regex::Regex::new(&args[18]).unwrap();
        assert!(pattern.is_match("a.rs:1:2: replace f -> bool with true"));
        assert!(!pattern.is_match("xa.rs:1:2: replace f -> bool with true"));
        assert!(!pattern.is_match("a.rs:1:2: replace f -> bool with truex"));
    }

    #[test]
    fn a_mutant_listing_is_counted_and_anything_else_fails() {
        use std::os::unix::process::ExitStatusExt;
        let listing = |code: i32, stdout: &str| std::process::Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into(),
            stderr: b"no manifest".to_vec(),
        };
        assert_eq!(parse_listing(&listing(0, "[]")).unwrap(), 0);
        assert_eq!(
            parse_listing(&listing(0, r#"[{"name": "a"}, {"name": "b"}]"#)).unwrap(),
            2
        );
        assert!(parse_listing(&listing(0, "{}")).is_err());
        assert!(parse_listing(&listing(0, "not json")).is_err());
        // An empty output is 0 only with cargo-mutants' log of an empty filter result.
        assert!(parse_listing(&listing(0, "")).is_err());
        let filtered_out = std::process::Output {
            stderr: b" INFO No mutants to filter\n".to_vec(),
            ..listing(0, "")
        };
        assert_eq!(parse_listing(&filtered_out).unwrap(), 0);
        let not_empty = std::process::Output {
            stderr: filtered_out.stderr.clone(),
            ..listing(0, "not json")
        };
        assert!(parse_listing(&not_empty).is_err());
        assert_eq!(
            parse_listing(&listing(1, "[]")).unwrap_err().to_string(),
            "cargo mutants --list failed: no manifest"
        );
    }

    /// The diff of a change to a list file only (the flip PR at 5d45efc1): no Rust source.
    const LIST_DIFF: &str = "diff --git a/conformance/core-pending.txt b/conformance/core-pending.txt\n\
        --- a/conformance/core-pending.txt\n+++ b/conformance/core-pending.txt\n@@ -1,2 +1 @@\n-conf::a\n conf::b\n";

    /// The diff of a change to a Rust file.
    const RUST_DIFF: &str = "diff --git a/xtask/src/ci.rs b/xtask/src/ci.rs\n\
        --- a/xtask/src/ci.rs\n+++ b/xtask/src/ci.rs\n@@ -1 +1 @@\n-a\n+b\n";

    /// cargo-mutants' rule: a new path that is not `/dev/null` with the extension `rs`; git's quoted and tab-ended paths too.
    #[test]
    fn only_a_new_rust_path_is_a_change_to_rust_source() {
        assert!(changes_rust_source(RUST_DIFF));
        assert!(changes_rust_source("+++ b/crates/x/build.rs\n"));
        assert!(changes_rust_source("+++ b/a b.rs\t\n"));
        assert!(changes_rust_source("+++ \"b/tab\\t\\303\\251.rs\"\n"));
        assert!(changes_rust_source(&format!("{LIST_DIFF}{RUST_DIFF}")));
        assert!(!changes_rust_source(LIST_DIFF));
        assert!(!changes_rust_source(""));
        assert!(!changes_rust_source("--- a/gone.rs\n+++ /dev/null\n"));
        assert!(!changes_rust_source("--- a/x.rs\n+++ b/x.rsx\n"));
        assert!(!changes_rust_source(" +++ b/x.rs\n"));
    }

    /// The landing diff stays plain under a configuration that colors diffs or names an external diff tool, so a Rust change
    /// still needs the listing (a colored `+++` header would hide it); a change to a list file only still skips it.
    #[test]
    fn the_landing_diff_is_plain_under_any_diff_configuration() {
        let repo = fsutil::test_repo(&[("a.txt", "a\n"), ("x/src/lib.rs", "fn a() {}\n")]);
        let root = repo.path();
        fsutil::test_git(root, &["commit", "-q", "-m", "base"]);
        let base = fsutil::test_git(root, &["rev-parse", "HEAD"])
            .trim()
            .to_string();
        for (key, value) in [
            ("color.diff", "always"),
            ("color.ui", "always"),
            ("diff.external", "/bin/false"),
        ] {
            fsutil::test_git(root, &["config", key, value]);
        }
        std::fs::write(root.join("a.txt"), "b\n").unwrap();
        fsutil::test_git(root, &["commit", "-q", "-am", "list"]);
        let diff = landing_diff(root, &base).unwrap();
        assert!(!diff.contains(&0x1b), "no color");
        assert!(!changes_rust_source(&String::from_utf8_lossy(&diff)));
        std::fs::write(root.join("x/src/lib.rs"), "fn b() {}\n").unwrap();
        fsutil::test_git(root, &["commit", "-q", "-am", "rust"]);
        let diff = String::from_utf8(landing_diff(root, &base).unwrap()).unwrap();
        assert!(!diff.contains('\x1b'), "no color");
        assert!(diff.contains("+++ b/x/src/lib.rs\n"), "{diff}");
        assert!(changes_rust_source(&diff));
        assert_eq!(
            diff_mutants(&diff, || {
                Ok(std::process::Output {
                    status: std::os::unix::process::ExitStatusExt::from_raw(0),
                    stdout: b"[]".to_vec(),
                    stderr: Vec::new(),
                })
            })
            .unwrap(),
            Some(0),
            "a Rust diff runs the listing"
        );
        assert!(landing_diff(root, "no-such-commit").is_err());
    }

    /// A diff with no Rust source lists no mutant, and the listing does not run, so its log text does not matter. A Rust
    /// diff keeps the listing's rule: an empty output is 0 only with cargo-mutants' log of an empty filter result.
    #[test]
    fn a_diff_without_rust_source_skips_the_listing_and_a_rust_diff_needs_it() {
        use std::os::unix::process::ExitStatusExt;
        let listing = |stdout: &str, stderr: &str| {
            let output = std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.into(),
                stderr: stderr.into(),
            };
            move || Ok(output)
        };
        let no_run = || -> Result<std::process::Output> { panic!("the listing runs") };
        assert_eq!(diff_mutants(LIST_DIFF, no_run).unwrap(), None);
        assert_eq!(diff_mutants("", no_run).unwrap(), None);
        assert_eq!(diff_mutants(RUST_DIFF, listing("[]", "")).unwrap(), Some(0));
        assert_eq!(
            diff_mutants(RUST_DIFF, listing(r#"[{"name": "a"}]"#, "")).unwrap(),
            Some(1)
        );
        assert_eq!(
            diff_mutants(RUST_DIFF, listing("", " INFO No mutants to filter\n")).unwrap(),
            Some(0)
        );
        assert!(diff_mutants(
            RUST_DIFF,
            listing("", " INFO Diff changes no Rust source files\n")
        )
        .is_err());
        assert!(diff_mutants(RUST_DIFF, listing("", "")).is_err());
        assert_eq!(
            diff_mutants(RUST_DIFF, || Err(anyhow!("start cargo mutants --list")))
                .unwrap_err()
                .to_string(),
            "start cargo mutants --list"
        );
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

    fn pass(_: &Path) -> Result<()> {
        Ok(())
    }

    fn fail(_: &Path) -> Result<()> {
        bail!("boom\nsecond line")
    }

    const FAKE: &[(&str, &str, JobFn)] = &[("a", "", pass), ("b", "", fail), ("c", "", pass)];

    fn run(only: Option<&str>, keep_going: bool) -> (Vec<(String, String)>, bool) {
        let mut lines = Vec::new();
        let (rows, failed) = run_steps(Path::new("."), FAKE, only, keep_going, &mut |l| {
            lines.push(l.to_string())
        });
        let kinds = rows
            .iter()
            .map(|(n, s, _)| {
                let kind = match s {
                    Status::Pass => "pass",
                    Status::Fail(_) => "fail",
                    Status::NotRun => "notrun",
                };
                (n.to_string(), kind.to_string())
            })
            .collect();
        (kinds, failed)
    }

    fn kinds(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn a_failure_stops_the_run_and_marks_the_rest_not_run() {
        assert_eq!(
            run(None, false),
            (
                kinds(&[("a", "pass"), ("b", "fail"), ("c", "notrun")]),
                true
            )
        );
    }

    #[test]
    fn keep_going_runs_every_step() {
        assert_eq!(
            run(None, true),
            (kinds(&[("a", "pass"), ("b", "fail"), ("c", "pass")]), true)
        );
    }

    #[test]
    fn one_step_can_be_run_alone() {
        assert_eq!(run(Some("a"), false), (kinds(&[("a", "pass")]), false));
        assert_eq!(run(Some("b"), false), (kinds(&[("b", "fail")]), true));
    }

    #[test]
    fn a_summary_line_names_the_step_the_result_and_the_first_line_of_the_reason() {
        let took = Duration::from_millis(1500);
        assert!(summary_line("fmt", &Status::Pass, took).starts_with("fmt"));
        assert!(summary_line("fmt", &Status::Pass, took).contains("PASS"));
        let failed = summary_line("fmt", &Status::Fail("boom\nsecond".into()), took);
        assert!(failed.contains("FAIL") && failed.contains("boom") && !failed.contains("second"));
        assert!(summary_line("fmt", &Status::NotRun, took).contains("NOT RUN"));
        assert!(summary_line("fmt", &Status::Pass, took).contains("1.5 s"));
    }

    /// The profile that the mutation step names, and the slow profile of the slow mutation runs, never terminate a test:
    /// no `slow-timeout` with a `terminate-after` applies to them, through `inherits`. The default profile terminates a test
    /// (2 s), which would count a mutant that hangs as CAUGHT.
    #[test]
    fn the_mutation_runs_use_profiles_that_never_terminate_a_test() {
        let config: toml::Table = include_str!("../../.config/nextest.toml").parse().unwrap();
        let profile = MUTANTS_TEST_ARGS
            .windows(2)
            .find(|pair| pair[0] == "--profile")
            .map(|pair| pair[1])
            .expect("the mutation step names a profile");
        for name in [profile, "slow"] {
            assert_eq!(terminate_after(&config, name), None, "profile {name}");
        }
        assert_eq!(terminate_after(&config, "default"), Some(1));
    }

    /// The `terminate-after` of `profile`'s `slow-timeout`, through `inherits` (a profile with none inherits `default`).
    fn terminate_after(config: &toml::Table, profile: &str) -> Option<i64> {
        let mut name = profile;
        loop {
            let table = config["profile"][name].as_table().expect("a profile");
            if let Some(timeout) = table.get("slow-timeout") {
                return timeout
                    .get("terminate-after")
                    .and_then(toml::Value::as_integer);
            }
            name = match table.get("inherits").and_then(toml::Value::as_str) {
                Some(parent) => parent,
                None if name != "default" => "default",
                None => return None,
            };
        }
    }
}

/// The red-on-revert proof of the mutants profile, on a real cargo-mutants run (slow tier: it starts cargo).
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;
    use botster_test_process::{Deadline, OwnedChild};

    /// The fixture `xtask/fixtures/mutants-hang`: three of its eight mutants park a test thread for ever, and two of those
    /// also fail another test. With the step's options and nextest arguments, those two are caught at their first failure,
    /// and the third, which fails no test, is a TIMEOUT: cargo-mutants exits with 3, which fails the step. Under the
    /// default profile nextest terminates the hang at 2 s and cargo-mutants counts all eight as caught (exit 0).
    #[test]
    fn a_mutant_that_hangs_fails_the_mutation_step_as_a_timeout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/mutants-hang");
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join(".config")).unwrap();
        std::fs::copy(fixture.join("Cargo.toml.in"), dir.path().join("Cargo.toml")).unwrap();
        std::fs::copy(fixture.join("src/lib.rs"), dir.path().join("src/lib.rs")).unwrap();
        for file in ["nextest.toml", "test-wrapper.sh"] {
            std::fs::copy(
                root.join(".config").join(file),
                dir.path().join(".config").join(file),
            )
            .unwrap();
        }
        let out = dir.path().join("out");
        let mut command = Command::new("cargo");
        command
            .current_dir(dir.path())
            .arg("mutants")
            .args(MUTANTS_OPTIONS)
            .arg("--output")
            .arg(&out)
            .args(MUTANTS_TEST_ARGS)
            .envs(test_budget::tier_env(false))
            .stdout(std::process::Stdio::null());
        let mut run = OwnedChild::spawn_group(&mut command).unwrap();
        let status = run.status_by(Deadline::after(test_budget::SLOW_DEADLINE));
        let outcomes = parse_outcomes(
            &std::fs::read_to_string(out.join("mutants.out/outcomes.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            (
                status.code(),
                outcomes.total,
                outcomes.timeout,
                outcomes.missed
            ),
            (Some(3), 8, 1, 0),
            "{status}"
        );
    }

    /// The red-on-revert proof of plan 23n, on real cargo-mutants runs of the fixture `xtask/fixtures/mutants-two-stage`:
    /// `triple` is compiled only with the `slow` feature, so stage 1 (the step's default run) misses each of its mutants.
    /// Stage 2 (`stage2_args`: in place, the `slow` feature, the slow filter) tests exactly those, the slow test catches
    /// each one, and the step passes. Without the slow test, both stages miss them, and the step fails.
    #[test]
    fn a_mutant_that_only_a_slow_test_catches_passes_and_one_that_both_stages_miss_fails() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/mutants-two-stage");
        let copy = |with_slow_test: bool| {
            let dir = tempfile::tempdir().unwrap();
            for sub in ["src", "tests", ".config"] {
                std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            }
            std::fs::copy(fixture.join("Cargo.toml.in"), dir.path().join("Cargo.toml")).unwrap();
            std::fs::copy(fixture.join("src/lib.rs"), dir.path().join("src/lib.rs")).unwrap();
            if with_slow_test {
                std::fs::copy(
                    fixture.join("tests/slow_triple.rs"),
                    dir.path().join("tests/slow_triple.rs"),
                )
                .unwrap();
            }
            for file in ["nextest.toml", "test-wrapper.sh"] {
                std::fs::copy(
                    root.join(".config").join(file),
                    dir.path().join(".config").join(file),
                )
                .unwrap();
            }
            dir
        };
        let finish = |mut command: Command, out: &Path| -> Result<Stage> {
            command.stdout(std::process::Stdio::null());
            let mut run = OwnedChild::spawn_group(&mut command).unwrap();
            let status = run.status_by(Deadline::after(test_budget::SLOW_DEADLINE));
            let outcomes = std::fs::read_to_string(out.join("mutants.out/outcomes.json"))
                .ok()
                .map(|text| parse_stage_outcomes(&text).unwrap());
            Ok(Stage {
                code: status.code(),
                outcomes,
                took: Duration::ZERO,
            })
        };
        let step = |dir: &Path| {
            two_stage_decision(
                8,
                || {
                    let out = dir.join("stage1");
                    let mut command = crate::tools::cargo(dir);
                    command
                        .arg("mutants")
                        .args(MUTANTS_OPTIONS)
                        .arg("--output")
                        .arg(&out)
                        .args(MUTANTS_TEST_ARGS)
                        .envs(test_budget::tier_env(false));
                    finish(command, &out)
                },
                |missed| {
                    assert_eq!(missed.len(), 4, "the four mutants of triple: {missed:?}");
                    assert!(missed.iter().all(|m| m.contains("triple")), "{missed:?}");
                    let out = dir.join("stage2");
                    let mut command = crate::tools::cargo(dir);
                    command
                        .args(stage2_args(&[], &["twostage".into()], &[], missed, &out))
                        .envs(test_budget::tier_env(true));
                    finish(command, &out)
                },
            )
        };
        let with = copy(true);
        let lines = step(with.path()).unwrap();
        assert_eq!(
            lines.last().map(String::as_str),
            Some("mutants: 8 mutants: 8 caught (4 by stage 2), 0 missed, 0 timeout, 0 unviable"),
            "{lines:?}"
        );
        let without = copy(false);
        let error = step(without.path()).unwrap_err();
        assert!(
            format!("{error:#}")
                .contains("a mutant that both stages miss, or a timeout in stage 2"),
            "{error:#}"
        );
    }

    /// The fixture `xtask/fixtures/mutants-platform`: `triple` is `cfg(windows)` code, which no gate OS compiles. Without
    /// the derived exclusions its mutants are MISSED and cargo-mutants exits with 2, which fails the step; with them, only
    /// the mutants of `double` are tested, each one is caught, and the run passes.
    #[test]
    fn the_mutants_of_code_that_the_gate_os_does_not_compile_are_excluded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/mutants-platform");
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join(".config")).unwrap();
        std::fs::copy(fixture.join("Cargo.toml.in"), dir.path().join("Cargo.toml")).unwrap();
        std::fs::copy(fixture.join("src/lib.rs"), dir.path().join("src/lib.rs")).unwrap();
        for file in ["nextest.toml", "test-wrapper.sh"] {
            std::fs::copy(
                root.join(".config").join(file),
                dir.path().join(".config").join(file),
            )
            .unwrap();
        }
        let derived = derived_exclusions(
            dir.path(),
            &["src/lib.rs".to_string()],
            std::env::consts::OS,
        )
        .unwrap();
        let run = |exclusions: &[String], name: &str| {
            let out = dir.path().join(name);
            let mut command = Command::new("cargo");
            command
                .current_dir(dir.path())
                .arg("mutants")
                .args(MUTANTS_OPTIONS)
                .arg("--output")
                .arg(&out);
            for re in exclusions {
                command.arg("--exclude-re").arg(re);
            }
            command
                .args(MUTANTS_TEST_ARGS)
                .envs(test_budget::tier_env(false))
                .stdout(std::process::Stdio::null());
            let mut run = OwnedChild::spawn_group(&mut command).unwrap();
            let status = run.status_by(Deadline::after(test_budget::SLOW_DEADLINE));
            let outcomes = parse_outcomes(
                &std::fs::read_to_string(out.join("mutants.out/outcomes.json")).unwrap(),
            )
            .unwrap();
            (status.code(), outcomes.missed, outcomes.caught)
        };
        let (code, missed, _) = run(&[], "without");
        assert_eq!((code, missed > 0), (Some(2), true));
        let (code, missed, caught) = run(&derived, "derived");
        assert_eq!((code, missed, caught > 0), (Some(0), 0, true));
    }
}

//! `cargo xtask ci [--job <name>] [--keep-going] [--list]`: every check of the merge gate.
//!
//! Every job of `.github/workflows/ci.yml` runs `cargo xtask ci --job <name>`, and `cargo xtask ci` runs all the jobs in order.
//! So the workflow and the local gate (`botsterq run --exclusive --deadline <d> -- cargo xtask ci`) cannot drift: the steps
//! live here, and `workflow_runs_only_xtask_jobs` checks the workflow file (docs/BUILD.md "How each stage runs").
//!
//! A job whose tool is absent reports SKIPPED with its reason. Nothing is skipped silently. On CI (`CI` is set) a missing
//! cargo tool is installed, and a missing toolchain is an error.

use crate::{
    caps, clause_lint, codegen, crate_symbols, ledger, manifest, real_only, replacement_map, taint,
    test_budget, timers,
};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: cargo xtask ci [--job <name>] [--keep-going] [--list]

  (no flag)     run every job in order; stop at the first failure
  --job <name>  run one job (what a workflow job does)
  --keep-going  run every job even after a failure
  --list        print the job names";

/// The jobs, in the order of the gate. The names are the job names of `.github/workflows/ci.yml`.
type JobFn = fn(&Ctx) -> Result<Outcome>;

const JOBS: &[(&str, &str, JobFn)] = &[
    ("manifest", "the sha256 of every frozen file", manifest_job),
    ("frozen", "frozen/ is add-only", frozen_job),
    ("taint", "banned names, unmarked timers", taint_job),
    (
        "ledger",
        "the test-id ledger, the clause citations and the crate symbols",
        ledger_job,
    ),
    (
        "rust",
        "fmt, clippy, the default tier with its budgets",
        rust_job,
    ),
    ("slow", "the slow tier", slow_job),
    (
        "codegen",
        "generated artifacts and the real-only list",
        codegen_job,
    ),
    ("ts", "the TypeScript packages", ts_job),
    ("fuzz", "the route frame decoder fuzzer (nightly)", fuzz_job),
    (
        "mutants",
        "cargo-mutants on the diff (landing)",
        mutants_job,
    ),
];

enum Outcome {
    Pass,
    Skipped(String),
}

struct Ctx {
    root: PathBuf,
    /// The base branch ref: `BOTSTER_CI_BASE_REF` (a pull request sets it), else `origin/main`.
    base_ref: String,
    /// `CI` is set: tools may be installed.
    ci: bool,
}

impl Ctx {
    fn new(root: &Path) -> Ctx {
        let base_ref = env("BOTSTER_CI_BASE_REF").unwrap_or_else(|| "origin/main".into());
        Ctx {
            root: root.to_path_buf(),
            base_ref,
            ci: env("CI").is_some(),
        }
    }

    fn git(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.root).args(args);
        cmd
    }

    /// Whether `reference` names a commit.
    fn resolves(&self, reference: &str) -> bool {
        self.git(&[
            "rev-parse",
            "--verify",
            "-q",
            &format!("{reference}^{{commit}}"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    }

    /// A pull request names its base branch; fetch it, so the diff has a base.
    fn fetch_base(&self) -> Result<()> {
        let Some(branch) = self.base_ref.strip_prefix("origin/") else {
            return Ok(());
        };
        if env("BOTSTER_CI_BASE_REF").is_none() {
            return Ok(());
        }
        run(self.git(&["fetch", "--no-tags", "origin", branch]))
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Run a command with inherited output; an error names the command and the status.
fn run(mut cmd: Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().with_context(|| format!("start {shown}"))?;
    if !status.success() {
        bail!("{shown} failed ({status})");
    }
    Ok(())
}

fn cargo(ctx: &Ctx, args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&ctx.root).args(args);
    caps::apply(&mut cmd);
    run(cmd)
}

/// Whether a tool answers a version probe.
fn present(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A cargo subcommand tool: present, or installed on CI, or `false` (the caller reports SKIPPED).
fn ensure_cargo_tool(ctx: &Ctx, probe: &[&str], install: &[&str]) -> Result<bool> {
    if present("cargo", probe) {
        return Ok(true);
    }
    if !ctx.ci {
        return Ok(false);
    }
    cargo(ctx, install)?;
    Ok(true)
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn manifest_job(ctx: &Ctx) -> Result<Outcome> {
    manifest::command(&ctx.root, &[])?;
    Ok(Outcome::Pass)
}

/// frozen/ is add-only (README rule 1): an existing file is never modified or deleted,
/// except frozen/current/MANIFEST.md, the one file a new manifest edits.
fn frozen_job(ctx: &Ctx) -> Result<Outcome> {
    ctx.fetch_base()?;
    let base = frozen_base(ctx)?;
    let output = ctx
        .git(&[
            "diff",
            "--no-renames",
            "--name-only",
            "--diff-filter=MDT",
            &base,
            "HEAD",
            "--",
            "frozen",
            ":(exclude)frozen/current/MANIFEST.md",
        ])
        .output()
        .context("run git diff")?;
    if !output.status.success() {
        bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let changed = String::from_utf8(output.stdout)?;
    if !changed.trim().is_empty() {
        bail!("frozen/ is add-only; these existing files changed since {base}:\n{changed}");
    }
    println!("frozen: no existing file changed since {base}");
    Ok(Outcome::Pass)
}

/// `BOTSTER_CI_BASE` (a push sets it to the commit before the push), else the merge base with the base branch, else the parent.
fn frozen_base(ctx: &Ctx) -> Result<String> {
    if let Some(base) = env("BOTSTER_CI_BASE").filter(|b| ctx.resolves(b)) {
        return Ok(base);
    }
    let head = rev(ctx, "HEAD")?;
    if ctx.resolves(&ctx.base_ref) {
        let output = ctx
            .git(&["merge-base", &ctx.base_ref, "HEAD"])
            .output()
            .context("run git merge-base")?;
        let base = String::from_utf8(output.stdout)?.trim().to_string();
        if output.status.success() && !base.is_empty() && base != head {
            return Ok(base);
        }
    }
    Ok("HEAD~1".into())
}

fn rev(ctx: &Ctx, reference: &str) -> Result<String> {
    let output = ctx
        .git(&["rev-parse", reference])
        .output()
        .context("run git rev-parse")?;
    if !output.status.success() {
        bail!("git rev-parse {reference} failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn taint_job(ctx: &Ctx) -> Result<Outcome> {
    taint::command(&ctx.root, &[])?;
    // No sleeps in test code or fakes without a marked deadline timer (docs/BUILD.md Testing rule 5).
    timers::command(&ctx.root, &[])?;
    Ok(Outcome::Pass)
}

fn ledger_job(ctx: &Ctx) -> Result<Outcome> {
    ctx.fetch_base()?;
    // The pending list may only shrink against the base branch, except for ids new to the ledger.
    if ctx.resolves(&ctx.base_ref) {
        ledger::command(&ctx.root, &args(&["--check", "--base", &ctx.base_ref]))?;
    } else {
        println!(
            "ledger: {} does not resolve; checking without a base",
            ctx.base_ref
        );
        ledger::command(&ctx.root, &args(&["--check"]))?;
    }
    clause_lint::command(&ctx.root, &[])?;
    // Every code span of the contract files maps to crate symbols that resolve (docs/BUILD.md "Amendments"). The pending base files
    // may only shrink against the base branch, so CI needs the base.
    if !ctx.resolves(&ctx.base_ref) {
        bail!(
            "crate-symbols: the base {} does not resolve; CI cannot check that PENDING only shrinks",
            ctx.base_ref
        );
    }
    crate_symbols::command(&ctx.root, &args(&["--check", "--base", &ctx.base_ref]))?;
    // Every ledger id, R-1.4 fault and Core error code has a replacement proof (docs/fake-replacement-map.md).
    replacement_map::command(&ctx.root, &args(&["--check"]))?;
    Ok(Outcome::Pass)
}

fn rust_job(ctx: &Ctx) -> Result<Outcome> {
    cargo(ctx, &["fmt", "--all", "--check"])?;
    cargo(
        ctx,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    fetch_main_times(ctx);
    let times = ctx.root.join("target/main-times/test-times.json");
    let mut options = Vec::new();
    if times.is_file() {
        options.push("--compare".to_string());
        options.push(times.display().to_string());
    }
    // The default tier with its budgets (docs/foundation-design.md section 9): 2 s a test, 60 s in all, no leftover process.
    test_budget::command(&ctx.root, &options)?;
    Ok(Outcome::Pass)
}

/// On a pull request, get the per-test times of the last successful main run, for the doubling report. Best effort.
fn fetch_main_times(ctx: &Ctx) {
    if env("GITHUB_EVENT_NAME").as_deref() != Some("pull_request") {
        return;
    }
    let listed = Command::new("gh")
        .current_dir(&ctx.root)
        .args([
            "run",
            "list",
            "--workflow",
            "ci.yml",
            "--branch",
            "main",
            "--status",
            "success",
            "--limit",
            "1",
            "--json",
            "databaseId",
            "--jq",
            ".[0].databaseId",
        ])
        .output();
    let id = listed
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if id.is_empty() {
        println!("rust: no successful main run yet; no test-time baseline");
        return;
    }
    let downloaded = Command::new("gh")
        .current_dir(&ctx.root)
        .args([
            "run",
            "download",
            &id,
            "--name",
            "test-times",
            "--dir",
            "target/main-times",
        ])
        .status();
    if !downloaded.is_ok_and(|s| s.success()) {
        println!("rust: no baseline artifact");
    }
}

fn slow_job(ctx: &Ctx) -> Result<Outcome> {
    // The xtask kills the run at its own deadline; the workflow job timeout is the backstop.
    test_budget::command(&ctx.root, &args(&["--slow", "--deadline", "10m"]))?;
    Ok(Outcome::Pass)
}

fn codegen_job(ctx: &Ctx) -> Result<Outcome> {
    codegen::command(&ctx.root, &args(&["--check"]))?;
    // The README list of ids that a fake does not prove is generated from the transcripts.
    real_only::command(&ctx.root, &args(&["--check"]))?;
    Ok(Outcome::Pass)
}

/// The TypeScript packages (docs/foundation-design.md section 8, step 2): the types are generated from the checked-in schemas,
/// so a schema change that is not followed by `npm run codegen` fails the diff.
fn ts_job(ctx: &Ctx) -> Result<Outcome> {
    if !present("npm", &["--version"]) {
        return Ok(Outcome::Skipped("npm is not installed".into()));
    }
    let npm = |args: &[&str]| -> Result<()> {
        let mut cmd = Command::new("npm");
        cmd.current_dir(ctx.root.join("ts")).args(args);
        caps::apply(&mut cmd);
        run(cmd)
    };
    npm(&["ci"])?;
    npm(&["run", "codegen", "-w", "@trybotster/hub-contract"])?;
    let mut diff = ctx.git(&["diff", "--exit-code", "--", "ts"]);
    diff.current_dir(&ctx.root);
    run(diff).context("codegen left a diff in ts/")?;
    npm(&["run", "build"])?;
    npm(&["run", "typecheck"])?;
    // The default tier: 2 s a test, 60 s in all (docs/BUILD.md, Testing rule 4).
    npm(&["test"])?;
    // The built package loads under plain Node, as Web's consumers load it.
    npm(&["run", "test:consumer", "-w", "@trybotster/hub-contract"])?;
    Ok(Outcome::Pass)
}

/// Landing: one libFuzzer run of 60 s per decoder harness. Only this job uses the nightly toolchain. The plugin frame decoder
/// and the manifest parser harnesses join the list when the plugin-contract feature adds them.
fn fuzz_job(ctx: &Ctx) -> Result<Outcome> {
    if !present("cargo", &["+nightly", "--version"]) {
        if ctx.ci {
            bail!("the nightly toolchain is missing on CI");
        }
        return Ok(Outcome::Skipped(
            "no nightly toolchain (rustup toolchain install nightly --profile minimal)".into(),
        ));
    }
    if !ensure_cargo_tool(
        ctx,
        &["bolero", "--version"],
        &["install", "cargo-bolero", "--locked"],
    )? {
        return Ok(Outcome::Skipped(
            "cargo-bolero is not installed (cargo install cargo-bolero --locked)".into(),
        ));
    }
    cargo(
        ctx,
        &[
            "+nightly",
            "bolero",
            "test",
            "-p",
            "botster-route-codec",
            "props::bolero_route_frame_decoder",
            "-T",
            "60s",
        ],
    )?;
    Ok(Outcome::Pass)
}

/// Mutation tests of the code that the change touches. A surviving mutant fails the job and is a review finding; an accepted
/// equivalent mutant goes in .cargo/mutants.toml with its reason (README "Building").
fn mutants_job(ctx: &Ctx) -> Result<Outcome> {
    if !ensure_cargo_tool(
        ctx,
        &["mutants", "--version"],
        &[
            "install",
            "cargo-mutants",
            "--version",
            "27.1.0",
            "--locked",
        ],
    )? {
        return Ok(Outcome::Skipped(
            "cargo-mutants is not installed (cargo install cargo-mutants --version 27.1.0 --locked)".into(),
        ));
    }
    ctx.fetch_base()?;
    if !ctx.resolves(&ctx.base_ref) {
        bail!(
            "{} does not resolve; the mutants diff has no base",
            ctx.base_ref
        );
    }
    // A negative control for the equivalent-mutant entries of .cargo/mutants.toml: the seeded coin of the fake is excluded,
    // and the negotiation check, which is not equivalent, is still in the list.
    let mut list = Command::new("cargo");
    list.current_dir(&ctx.root)
        .args(["mutants", "-p", "botster-fake-route", "--list"]);
    let listed = caps::apply(&mut list)
        .output()
        .context("run cargo mutants --list")?;
    if !listed.status.success() {
        bail!(
            "cargo mutants --list failed: {}",
            String::from_utf8_lossy(&listed.stderr)
        );
    }
    check_equivalent_entries(&String::from_utf8_lossy(&listed.stdout))?;

    let diff = ctx
        .git(&["diff", &format!("{}...HEAD", ctx.base_ref)])
        .output()
        .context("run git diff")?;
    if !diff.status.success() {
        bail!("git diff failed: {}", String::from_utf8_lossy(&diff.stderr));
    }
    let target = ctx.root.join("target");
    std::fs::create_dir_all(&target)?;
    let diff_path = target.join("mutants.diff");
    std::fs::write(&diff_path, &diff.stdout)?;
    let diff_arg = diff_path.display().to_string();

    // The exit status is 0 when every mutant is caught or no Rust code changed, and 2 when a mutant survived.
    // A mutant runs the tests of its own package, except in a fake: the transcripts that prove a fake are in the conformance
    // crates, so a fake runs the workspace tests. The three runs split the changed code by path, and all run.
    let packages = cargo(
        ctx,
        &[
            "mutants",
            "--in-diff",
            &diff_arg,
            "--no-shuffle",
            "-j",
            "1",
            "--exclude",
            "crates/botster-fake-*/**",
            "--exclude",
            "crates/botster-plugin-fakes/**",
            "--exclude",
            "crates/botster-conformance-probe/**",
        ],
    );
    // The probe's tests are slow-tier tests: without the feature no test runs against a mutant of the probe. Only the
    // probe package has the feature, so the probe gets its own run (cargo-mutants passes `--features` to every package).
    let probe = cargo(
        ctx,
        &[
            "mutants",
            "--in-diff",
            &diff_arg,
            "--no-shuffle",
            "-j",
            "1",
            "-o",
            "mutants-probe",
            "--features",
            "botster-conformance-probe/slow",
            "--file",
            "crates/botster-conformance-probe/**/*.rs",
        ],
    );
    let fakes = cargo(
        ctx,
        &[
            "mutants",
            "--in-diff",
            &diff_arg,
            "--no-shuffle",
            "--test-workspace=true",
            "-j",
            "1",
            "-o",
            "mutants-fakes",
            "--file",
            "crates/botster-fake-*/**/*.rs",
            "--file",
            "crates/botster-plugin-fakes/**/*.rs",
        ],
    );
    packages.and(probe).and(fakes)?;
    Ok(Outcome::Pass)
}

/// The coin mutant is excluded, and the negotiation mutant is listed.
fn check_equivalent_entries(listed: &str) -> Result<()> {
    if !listed.contains("replace == with != in deflate_negotiated") {
        bail!("the negotiation mutant of deflate_negotiated is not listed; the equivalent-mutant entries are too wide");
    }
    if listed.contains("replace == with != in FakeWorker::send_output") {
        bail!(
            "the excluded coin mutant is still listed; the equivalent-mutant entry does not match"
        );
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
            bail!("unknown job '{job}' (jobs: {})", names.join(", "));
        }
    }
    Ok(options)
}

enum Status {
    Pass,
    Fail(String),
    Skipped(String),
    NotRun,
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let options = parse_options(args)?;
    if options.list {
        for (name, what, _) in JOBS {
            println!("{name:<10} {what}");
        }
        return Ok(());
    }
    let ctx = Ctx::new(root);
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
        let status = match job(&ctx) {
            Ok(Outcome::Pass) => Status::Pass,
            Ok(Outcome::Skipped(reason)) => {
                println!("SKIPPED {name}: {reason}");
                Status::Skipped(reason)
            }
            Err(error) => {
                // Problems carry their lines: print them as the command alone would.
                crate::print_error(&error);
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
            Status::Skipped(why) => ("SKIPPED", why.clone()),
            Status::NotRun => ("NOT RUN", "an earlier job failed".into()),
        };
        println!(
            "{name:<10} {label:<8} {:>7.1} s  {note}",
            took.as_secs_f64()
        );
    }
    println!("{:<10} {:<8} {:>7.1} s", "total", "", total.as_secs_f64());
    let skipped = rows
        .iter()
        .filter(|(_, s, _)| matches!(s, Status::Skipped(_)))
        .count();
    if skipped > 0 {
        println!("{skipped} job(s) SKIPPED: this run is not the full gate");
    }
    if failed {
        bail!("ci failed");
    }
    Ok(())
}

/// Checks the text of a workflow: every job runs exactly one `run:` step, `cargo xtask ci --job <its name>`,
/// and the job names are the xtask job names.
#[cfg(test)]
fn check_workflow(text: &str, jobs: &[&str]) -> Result<(), String> {
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_jobs = false;
    for line in text.lines() {
        if line.starts_with("jobs:") {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        if !line.starts_with(' ') && !line.trim().is_empty() && !line.starts_with('#') {
            break;
        }
        if let Some(name) = line.strip_prefix("  ").and_then(|l| l.strip_suffix(':')) {
            if !name.starts_with(' ') && !name.starts_with('#') {
                seen.push((name.to_string(), Vec::new()));
                continue;
            }
        }
        let step = line.trim_start().trim_start_matches("- ");
        if let Some(command) = step.strip_prefix("run:") {
            let Some((_, runs)) = seen.last_mut() else {
                continue;
            };
            runs.push(command.trim().to_string());
        }
    }
    let mut problems = Vec::new();
    for (name, runs) in &seen {
        let expected = format!("cargo xtask ci --job {name}");
        if runs.len() != 1 || runs[0] != expected {
            problems.push(format!(
                "job '{name}' must have exactly one run step, `{expected}`; it has {runs:?}"
            ));
        }
    }
    let names: Vec<&str> = seen.iter().map(|(name, _)| name.as_str()).collect();
    for job in jobs {
        if !names.contains(job) {
            problems.push(format!("xtask job '{job}' has no job in the workflow"));
        }
    }
    for name in &names {
        if !jobs.contains(name) {
            problems.push(format!("workflow job '{name}' is not an xtask job"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil;

    fn job_names() -> Vec<&'static str> {
        JOBS.iter().map(|(name, ..)| *name).collect()
    }

    fn workflow() -> String {
        let root = fsutil::repo_root().unwrap();
        std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap()
    }

    /// BUILD.md "How each stage runs": the merge gate and the workflow run the same steps.
    #[test]
    fn workflow_runs_only_xtask_jobs() {
        check_workflow(&workflow(), &job_names()).unwrap();
    }

    #[test]
    fn a_hand_written_step_in_the_workflow_is_drift() {
        let drifted = workflow().replace("cargo xtask ci --job taint", "cargo xtask taint");
        let error = check_workflow(&drifted, &job_names()).unwrap_err();
        assert!(error.contains("job 'taint'"), "{error}");
    }

    #[test]
    fn a_second_run_step_in_a_job_is_drift() {
        let drifted = workflow().replace(
            "      - run: cargo xtask ci --job manifest\n",
            "      - run: cargo xtask ci --job manifest\n      - run: echo extra\n",
        );
        let error = check_workflow(&drifted, &job_names()).unwrap_err();
        assert!(error.contains("job 'manifest'"), "{error}");
    }

    #[test]
    fn a_job_missing_from_either_side_is_drift() {
        let error = check_workflow(&workflow(), &["manifest", "not-a-job"]).unwrap_err();
        assert!(error.contains("'not-a-job' has no job"), "{error}");
        assert!(
            error.contains("workflow job 'frozen' is not an xtask job"),
            "{error}"
        );
    }

    #[test]
    fn options() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse_options(&args(&["--job", "nope"])).is_err());
        assert!(parse_options(&args(&["--bogus"])).is_err());
        let o = parse_options(&args(&["--job", "rust", "--keep-going"])).unwrap();
        assert_eq!(o.job.as_deref(), Some("rust"));
        assert!(o.keep_going && !o.list);
    }

    #[test]
    fn equivalent_entries_are_exact() {
        let negotiation = "src/x.rs:1:1: replace == with != in deflate_negotiated\n";
        assert!(check_equivalent_entries(negotiation).is_ok());
        assert!(check_equivalent_entries("").is_err());
        let coin =
            format!("{negotiation}src/x.rs:2:1: replace == with != in FakeWorker::send_output\n");
        assert!(check_equivalent_entries(&coin).is_err());
    }
}

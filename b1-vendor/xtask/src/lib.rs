//! Repo tooling: `cargo xtask <command>`. See `xtask/README.md`.
//!
//! The binary (`main.rs`) only reads argv and prints. Every command runs through [`run`], so the tests call it in-process.

mod caps;
mod ci;
mod clause_lint;
mod codegen;
mod config;
mod crate_symbols;
mod extract;
mod fsutil;
mod ledger;
mod manifest;
mod real_only;
mod replacement_map;
mod status;
mod taint;
mod test_budget;
mod timers;

use anyhow::{bail, Result};
use std::path::Path;

pub use fsutil::repo_root;

pub const USAGE: &str = "usage: cargo xtask <command>

commands:
  ledger [--check [--base <git-ref>]]  write or check conformance/ledger.json
  manifest                             check the sha256 of every file in <contracts.toml dir>/MANIFEST.md
  taint                                scan tracked files for names in docs/taint-denylist.txt
  timers                               fail on unmarked sleeps in test code and on any timer in a fake
  test-budget [--slow] [options]       run a test tier under nextest and enforce its budgets
  clause-lint                          check clause citations in the fake crates against the ledger
  codegen [--check]                    write or check the generated artifacts
  real-only [--check]                  write or check the README list of ids that a fake does not prove
  replacement-map [--check]            write or check docs/fake-replacement-map.md from conformance/replacement-map.json
  crate-symbols [--check] [--base <ref>] | --draft <file>   check that every code span of the contract files maps to crate symbols that resolve
  ci [--job <name>] [--keep-going]     run every job of the merge gate (what .github/workflows/ci.yml runs, job by job)";

/// A failed check: the problem lines the command found, then the one-line summary that is the error text.
/// `print_error` prints each line to stderr, then `stdout` (a summary that the command printed between its problem lines and its error,
/// kept here so the order of the two streams stays the same), then `error: <summary>`. A caller in-process reads `lines` directly.
#[derive(Debug)]
pub struct Problems {
    pub lines: Vec<String>,
    pub stdout: String,
    pub summary: String,
}

impl Problems {
    pub fn fail(lines: Vec<String>, summary: String) -> anyhow::Error {
        Self::fail_after(lines, String::new(), summary)
    }

    /// Like `fail`, with a stdout line that goes after the problem lines and before the error line.
    pub fn fail_after(lines: Vec<String>, stdout: String, summary: String) -> anyhow::Error {
        anyhow::Error::new(Problems {
            lines,
            stdout,
            summary,
        })
    }
}

impl std::fmt::Display for Problems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.summary)
    }
}

impl std::error::Error for Problems {}

/// The stderr text of a failed run, exactly as the binary prints it.
pub fn render_error(error: &anyhow::Error) -> String {
    let mut out = String::new();
    if let Some(problems) = error.downcast_ref::<Problems>() {
        for line in &problems.lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&format!("error: {error:#}\n"));
    out
}

/// Prints a failed run as the binary does: problem lines and error line to stderr, the carried stdout between them.
pub fn print_error(error: &anyhow::Error) {
    if let Some(problems) = error.downcast_ref::<Problems>() {
        for line in &problems.lines {
            eprintln!("{line}");
        }
        print!("{}", problems.stdout);
    }
    eprintln!("error: {error:#}");
}

/// Runs one command (`args[0]`) against the repo at `root`.
pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let Some(command) = args.first() else {
        bail!("{USAGE}");
    };
    let rest = &args[1..];
    match command.as_str() {
        "ledger" => ledger::command(root, rest),
        "manifest" => manifest::command(root, rest),
        "taint" => taint::command(root, rest),
        "timers" => timers::command(root, rest),
        "test-budget" => test_budget::command(root, rest),
        "clause-lint" => clause_lint::command(root, rest),
        "codegen" => codegen::command(root, rest),
        "crate-symbols" => crate_symbols::command(root, rest),
        "ci" => ci::command(root, rest),
        "real-only" => real_only::command(root, rest),
        "replacement-map" => replacement_map::command(root, rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => bail!("unknown command '{other}'\n{USAGE}"),
    }
}

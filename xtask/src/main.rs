//! Repo tooling: `cargo xtask <command>`. The gate is `cargo xtask ci` (plan section 8).

mod base_merge;
mod caps;
mod ci;
mod fsutil;
mod lists;
mod prebuild;
mod public_api;
mod taint;
mod test_budget;
mod timers;
mod tools;

use anyhow::{bail, Result};

const USAGE: &str = "usage: cargo xtask <command>

commands:
  ci [--job <name>] [--keep-going]   every step of the merge gate, in order
  base-merge-check <reviewed> <new>  a base-only merge after CLEAN: no conflict, no shared path, the same own diff
  taint                              banned old-world names (contracts list plus Core's additions)
  timers                             unmarked sleeps in test code; timers in machine crates
  lists                              check core-ledger-ids, core-pending and core-deferred
  ledger-ids [--write]               check or write conformance/core-ledger-ids.txt from the pinned ledger
  public-api [--update]              check or write the facade snapshot api/botster-core.txt
  prebuild-worker                    build the real-process binaries into target/candidate with a sha256 manifest
  test-budget [--slow] [options]     run a test tier under nextest and enforce its budgets
  slow                               the slow tier (test-budget --slow)

Heavy commands need the parallelism cap: launch them as
  botsterq run --label \"core <package> ...\" --deadline <d> -- env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask <command>";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        bail!("{USAGE}");
    };
    let rest: Vec<String> = args.collect();
    let root = fsutil::repo_root()?;
    match command.as_str() {
        "ci" => ci::command(&root, &rest),
        "base-merge-check" => base_merge::command(&root, &rest),
        "taint" => taint::command(&root, &rest),
        "timers" => timers::command(&root, &rest),
        "lists" => lists::command(&root, &rest),
        "ledger-ids" => lists::ledger_ids_command(&root, &rest),
        "public-api" => public_api::command(&root, &rest),
        "prebuild-worker" => prebuild::command(&root, &rest),
        "test-budget" => test_budget::command(&root, &rest),
        "slow" => test_budget::command(&root, &["--slow".to_string()]),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => bail!("unknown command '{other}'\n{USAGE}"),
    }
}

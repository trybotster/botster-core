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

use anyhow::Result;
use std::path::Path;

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

/// A command: it runs with the repository root and the arguments after its name.
type CommandFn = fn(&Path, &[String]) -> Result<()>;

/// Every command, by name. `USAGE` lists the same names (the_usage_lists_every_command_and_only_those).
const COMMANDS: &[(&str, CommandFn)] = &[
    ("ci", ci::command),
    ("base-merge-check", base_merge::command),
    ("taint", taint::command),
    ("timers", timers::command),
    ("lists", lists::command),
    ("ledger-ids", lists::ledger_ids_command),
    ("public-api", public_api::command),
    ("prebuild-worker", prebuild::command),
    ("test-budget", test_budget::command),
    ("slow", |root, _| {
        test_budget::command(root, &["--slow".to_string()])
    }),
];

/// What the first argument asks for.
enum Chosen {
    Run(CommandFn),
    Help,
}

/// The command or the help that the first argument names.
///
/// # Errors
/// No argument (the usage), or an unknown name.
fn choose(name: Option<&str>) -> Result<Chosen, String> {
    match name {
        None => Err(USAGE.to_string()),
        Some("-h" | "--help" | "help") => Ok(Chosen::Help),
        Some(name) => COMMANDS
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, command)| Chosen::Run(*command))
            .ok_or_else(|| format!("unknown command '{name}'\n{USAGE}")),
    }
}

/// The I/O shell: it reads the arguments, and `choose` decides.
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let chosen = choose(args.next().as_deref()).map_err(anyhow::Error::msg)?;
    let rest: Vec<String> = args.collect();
    match chosen {
        Chosen::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Chosen::Run(command) => command(&fsutil::repo_root()?, &rest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command names of `USAGE`: the first word of each line of its command list.
    fn listed() -> Vec<&'static str> {
        USAGE
            .split("commands:\n")
            .nth(1)
            .unwrap()
            .lines()
            .take_while(|line| !line.is_empty())
            .map(|line| line.split_whitespace().next().unwrap())
            .collect()
    }

    #[test]
    fn the_usage_lists_every_command_and_only_those() {
        let names: Vec<&str> = COMMANDS.iter().map(|(name, _)| *name).collect();
        assert_eq!(listed(), names);
    }

    #[test]
    fn each_name_chooses_its_own_command() {
        for (name, command) in COMMANDS {
            match choose(Some(name)) {
                Ok(Chosen::Run(chosen)) => {
                    assert!(std::ptr::fn_addr_eq(chosen, *command), "{name}")
                }
                _ => panic!("{name} chooses no command"),
            }
        }
    }

    #[test]
    fn the_help_names_choose_the_help_and_others_fail_with_the_usage() {
        for name in ["-h", "--help", "help"] {
            assert!(matches!(choose(Some(name)), Ok(Chosen::Help)), "{name}");
        }
        assert!(matches!(choose(None), Err(text) if text == USAGE));
        assert!(
            matches!(choose(Some("tain")), Err(text) if text.starts_with("unknown command 'tain'\nusage:"))
        );
    }
}

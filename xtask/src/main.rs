//! Repo tooling: `cargo xtask <command>`. The gate is `cargo xtask ci` (plan section 8).

mod base_merge;
mod caps;
mod ci;
mod fsutil;
mod lists;
mod prebuild;
mod public_api;
mod real_proofs;
#[cfg(test)]
mod signal_bans;
mod signals;
mod taint;
mod test_budget;
mod timers;
mod tools;
mod unsafe_exception;

use anyhow::Result;
use std::path::Path;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  ci [--job <name>] [--keep-going]   every step of the merge gate, in order
  base-merge-check <reviewed> <new>  a base-only merge after CLEAN: no conflict, no shared path, the same own diff
  taint                              banned old-world names (contracts list plus Core's additions)
  timers                             unmarked sleeps in test code; timers in machine crates
  signals                            raw signal calls and kill programs outside botster_core_sys::signal
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

/// A command: it runs with the repository root and its arguments.
type CommandFn = fn(&Path, &[String]) -> Result<()>;

/// Every command, by name, with its fixed arguments (`None`: the arguments after its name). `USAGE` lists the same names.
const COMMANDS: &[(&str, CommandFn, Option<&[&str]>)] = &[
    ("ci", ci::command, None),
    ("base-merge-check", base_merge::command, None),
    ("taint", taint::command, None),
    ("timers", timers::command, None),
    ("signals", signals::command, None),
    ("lists", lists::command, None),
    ("ledger-ids", lists::ledger_ids_command, None),
    ("public-api", public_api::command, None),
    ("prebuild-worker", prebuild::command, None),
    ("test-budget", test_budget::command, None),
    ("slow", test_budget::command, Some(&["--slow"])),
];

/// What the arguments ask for.
#[derive(Debug)]
enum Chosen {
    /// A command, with the arguments that it runs with.
    Run(CommandFn, Vec<String>),
    Help,
}

/// The command (and its arguments) or the help that the arguments of the process (without the program) name.
///
/// # Errors
/// No argument (the usage), or an unknown name.
fn choose(args: &[String]) -> Result<Chosen, String> {
    let Some((name, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    if matches!(name.as_str(), "-h" | "--help" | "help") {
        return Ok(Chosen::Help);
    }
    let (_, command, fixed) = COMMANDS
        .iter()
        .find(|(known, _, _)| known == name)
        .ok_or_else(|| format!("unknown command '{name}'\n{USAGE}"))?;
    let args = fixed.map_or_else(
        || rest.to_vec(),
        |f| f.iter().map(|a| a.to_string()).collect(),
    );
    Ok(Chosen::Run(*command, args))
}

/// The I/O shell: it reads the arguments and runs what `choose` decides.
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match choose(&args).map_err(anyhow::Error::msg)? {
        Chosen::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Chosen::Run(command, args) => command(&fsutil::repo_root()?, &args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    /// Each command name, the function that it runs and the arguments it gets for `<name> x`: written here, not read from
    /// `COMMANDS`, so a wrong entry there fails.
    fn expected() -> Vec<(&'static str, CommandFn, Vec<String>)> {
        let given = strings(&["x"]);
        vec![
            ("ci", ci::command, given.clone()),
            ("base-merge-check", base_merge::command, given.clone()),
            ("taint", taint::command, given.clone()),
            ("timers", timers::command, given.clone()),
            ("signals", signals::command, given.clone()),
            ("lists", lists::command, given.clone()),
            ("ledger-ids", lists::ledger_ids_command, given.clone()),
            ("public-api", public_api::command, given.clone()),
            ("prebuild-worker", prebuild::command, given.clone()),
            ("test-budget", test_budget::command, given),
            ("slow", test_budget::command, strings(&["--slow"])),
        ]
    }

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
        let names: Vec<&str> = expected().iter().map(|(name, _, _)| *name).collect();
        assert_eq!(listed(), names);
    }

    #[test]
    fn each_name_runs_its_own_function_with_its_arguments() {
        for (name, function, args) in expected() {
            match choose(&strings(&[name, "x"])) {
                Ok(Chosen::Run(chosen, given)) => {
                    assert!(std::ptr::fn_addr_eq(chosen, function), "{name}");
                    assert_eq!(given, args, "{name}");
                }
                other => panic!("{name}: {other:?}"),
            }
        }
    }

    #[test]
    fn the_help_names_choose_the_help_and_others_fail_with_the_usage() {
        for name in ["-h", "--help", "help"] {
            assert!(
                matches!(choose(&strings(&[name])), Ok(Chosen::Help)),
                "{name}"
            );
        }
        assert!(matches!(choose(&[]), Err(text) if text == USAGE));
        assert!(
            matches!(choose(&strings(&["tain"])), Err(text) if text.starts_with("unknown command 'tain'\nusage:"))
        );
    }
}

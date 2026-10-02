//! The parallelism cap (BUILD.md "Resources"; plan section 8).
//!
//! `cargo xtask` is a cargo alias for `cargo run -p xtask`, so cargo compiles xtask before any xtask code runs. The cap is
//! therefore set by the launch command (`botsterq run … -- env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci`).
//! xtask enforces it in two ways: [`require`] refuses to start a step when a variable is missing or above [`LIMIT`], and
//! [`apply`] passes both variables to every child explicitly.

use anyhow::{bail, Result};
use std::process::Command;

/// The most parallel jobs of one cargo build, and the most parallel tests of one nextest run.
pub const LIMIT: u32 = 4;

/// The variables that carry the cap. `RUST_TEST_THREADS` caps a libtest harness (cargo-mutants may run `cargo test`).
const REQUIRED: [&str; 2] = ["CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS"];
const VARIABLES: [&str; 3] = [
    "CARGO_BUILD_JOBS",
    "NEXTEST_TEST_THREADS",
    "RUST_TEST_THREADS",
];

/// The problems of a cap given as `(name, value)` pairs: empty when both required variables are set to `1..=LIMIT`.
fn problems(value_of: impl Fn(&str) -> Option<String>) -> Vec<String> {
    REQUIRED
        .iter()
        .filter_map(|name| match value_of(name) {
            None => Some(format!("{name} is not set")),
            Some(text) => match text.parse::<u32>() {
                Ok(n) if (1..=LIMIT).contains(&n) => None,
                _ => Some(format!("{name}={text} (it must be 1 to {LIMIT})")),
            },
        })
        .collect()
}

/// Refuses to continue when the launch command did not cap the run.
pub fn require() -> Result<()> {
    let found = problems(|name| std::env::var(name).ok());
    if found.is_empty() {
        return Ok(());
    }
    bail!(
        "the parallelism cap is missing: {}. Launch with `botsterq run … -- env CARGO_BUILD_JOBS={LIMIT} NEXTEST_TEST_THREADS={LIMIT} cargo xtask …`",
        found.join("; ")
    )
}

/// Caps a child: the inherited value when it is lower, else [`LIMIT`].
pub fn apply(cmd: &mut Command) -> &mut Command {
    for name in VARIABLES {
        let inherited = std::env::var(name).ok().and_then(|v| v.parse::<u32>().ok());
        cmd.env(
            name,
            inherited.map_or(LIMIT, |v| v.clamp(1, LIMIT)).to_string(),
        );
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(cmd: &Command, name: &str) -> Option<String> {
        cmd.get_envs()
            .find(|(key, _)| *key == name)
            .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
    }

    #[test]
    fn the_limit_is_four() {
        assert_eq!(LIMIT, 4);
    }

    #[test]
    fn a_capped_child_gets_every_variable_at_or_below_the_limit() {
        let mut cmd = Command::new("cargo");
        apply(&mut cmd);
        for name in VARIABLES {
            let value = env_of(&cmd, name).unwrap_or_else(|| panic!("{name} is not set"));
            assert!(
                (1..=LIMIT).contains(&value.parse::<u32>().unwrap()),
                "{name}={value}"
            );
        }
    }

    #[test]
    fn a_launch_with_both_variables_at_the_limit_passes() {
        assert!(problems(|_| Some("4".into())).is_empty());
        assert!(problems(|_| Some("1".into())).is_empty());
    }

    #[test]
    fn a_missing_variable_is_named() {
        let found = problems(|name| (name == "CARGO_BUILD_JOBS").then(|| "4".to_string()));
        assert_eq!(found, ["NEXTEST_TEST_THREADS is not set"]);
    }

    #[test]
    fn a_value_above_the_limit_or_not_a_number_or_zero_fails() {
        for bad in ["5", "12", "0", "many", ""] {
            assert_eq!(problems(|_| Some(bad.into())).len(), 2, "{bad:?}");
        }
    }

    /// Every spawn of cargo goes through `apply`.
    #[test]
    fn every_cargo_spawn_is_capped() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let production = text.split("#[cfg(test)]").next().unwrap();
            let spawns = production.matches("Command::new(\"cargo\")").count();
            let capped = production.matches("caps::apply(").count();
            assert!(
                capped >= spawns,
                "{}: {spawns} cargo spawns, {capped} caps",
                path.display()
            );
        }
    }
}

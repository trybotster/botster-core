//! Parallelism caps for every build and test child that xtask starts (docs/BUILD.md "Resources").
//!
//! An uncapped gate once drove the machine load to 46, so no command forgets the cap: xtask sets it in the child's environment,
//! and it overrides a higher inherited value. A lower inherited value stays, so a caller can still go slower.

use std::process::Command;

/// The most parallel jobs of one cargo build, and the most parallel tests of one nextest run.
pub const LIMIT: u32 = 4;

/// The variables that carry the cap. Vitest reads its cap (`maxWorkers`) from `ts/vitest.shared.ts`.
const VARIABLES: [&str; 3] = [
    "CARGO_BUILD_JOBS",
    "NEXTEST_TEST_THREADS",
    "RUST_TEST_THREADS",
];

/// Cap a child: cargo build jobs, nextest test threads, and the threads of a libtest harness (cargo-mutants runs `cargo test`).
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
    fn a_capped_child_gets_the_limit_in_its_environment() {
        let mut cmd = Command::new("cargo");
        apply(&mut cmd);
        for name in VARIABLES {
            let value = env_of(&cmd, name).unwrap_or_else(|| panic!("{name} is not set"));
            assert!(value.parse::<u32>().unwrap() <= LIMIT, "{name}={value}");
        }
    }

    #[test]
    fn the_limit_is_four() {
        assert_eq!(LIMIT, 4);
    }

    /// The cap of the TypeScript tier is a setting of the shared vitest config.
    #[test]
    fn vitest_is_capped_in_the_shared_config() {
        let root = crate::fsutil::repo_root().unwrap();
        let text = std::fs::read_to_string(root.join("ts/vitest.shared.ts")).unwrap();
        assert!(text.contains(&format!("maxWorkers: {LIMIT}")));
        let slow =
            std::fs::read_to_string(root.join("ts/packages/hub-contract/vitest.slow.config.ts"))
                .unwrap();
        assert!(slow.contains(&format!("maxWorkers: {LIMIT}")));
    }

    /// Every spawn of cargo, nextest or npm goes through `apply`.
    #[test]
    fn every_cargo_and_npm_spawn_is_capped() {
        let root = crate::fsutil::repo_root().unwrap();
        for file in ["ci.rs", "test_budget.rs"] {
            let text = std::fs::read_to_string(root.join("xtask/src").join(file)).unwrap();
            let production = text.split("#[cfg(test)]").next().unwrap();
            let spawns = production.matches("Command::new(\"cargo\")").count()
                + production.matches("Command::new(\"npm\")").count();
            let capped = production.matches("caps::apply(").count();
            assert!(
                capped >= spawns,
                "{file}: {spawns} cargo/npm spawns, {capped} caps"
            );
        }
    }
}

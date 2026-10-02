//! The external tools of the gate and how xtask finds them.

use crate::caps;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// The nightly of the two steps that need one: the `cargo public-api` snapshot (rustdoc JSON) and the landing fuzzer
/// (libFuzzer). Everything else, including the property-test runs of the bolero harnesses, uses the stable of
/// `rust-toolchain.toml`. The gate only verifies that it is installed ([`ensure_nightly`]).
pub const NIGHTLY: &str = "nightly-2026-09-30";

/// Runs a command with inherited output; an error names the command and the status.
pub fn run(mut cmd: Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().with_context(|| format!("start {shown}"))?;
    if !status.success() {
        bail!("{shown} failed ({status})");
    }
    Ok(())
}

/// A capped `cargo` command in `root`.
pub fn cargo(root: &Path) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root);
    caps::apply(&mut cmd);
    cmd
}

/// A capped `cargo` command that runs on the pinned nightly.
pub fn cargo_nightly(root: &Path) -> Command {
    let mut cmd = cargo(root);
    cmd.env("RUSTUP_TOOLCHAIN", NIGHTLY);
    cmd
}

/// Verifies that the pinned nightly is installed. The gate never installs or updates a toolchain: the image or the lead
/// installs the nightly once, outside any gate (plan section 8, step 9).
pub fn ensure_nightly() -> Result<()> {
    let installed = Command::new("rustup")
        .args(["run", NIGHTLY, "rustc", "--version"])
        // Offline: a missing toolchain is an error, never an install.
        .env("RUSTUP_AUTO_INSTALL", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    verified(installed)
}

/// The result of the nightly probe.
fn verified(installed: bool) -> Result<()> {
    if installed {
        Ok(())
    } else {
        bail!("missing prerequisite: {NIGHTLY}")
    }
}

/// Fails with the install command when a cargo subcommand tool is missing.
pub fn require_cargo_tool(root: &Path, probe: &[&str], install: &str) -> Result<()> {
    let mut cmd = cargo(root);
    cmd.args(probe).stdout(Stdio::null()).stderr(Stdio::null());
    if cmd.status().is_ok_and(|s| s.success()) {
        return Ok(());
    }
    bail!(
        "`cargo {}` is missing; install it with `{install}`",
        probe.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_nightly_is_a_missing_prerequisite() {
        assert!(verified(true).is_ok());
        assert_eq!(
            verified(false).unwrap_err().to_string(),
            "missing prerequisite: nightly-2026-09-30"
        );
    }
}

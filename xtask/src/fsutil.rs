//! File and workspace helpers shared by the commands.

use crate::caps;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repo root: the parent of the xtask manifest directory.
pub fn repo_root() -> Result<PathBuf> {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").context("run xtask through `cargo xtask`")?;
    let root = Path::new(&manifest)
        .parent()
        .context("xtask has a parent directory")?
        .to_path_buf();
    if !root.join("rust-toolchain.toml").is_file() {
        bail!("no rust-toolchain.toml in {}", root.display());
    }
    Ok(root)
}

/// The tracked files of the repo, from `git ls-files`.
pub fn tracked_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .context("run git ls-files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

/// The text of a file at a git ref, or `None` when the file does not exist there.
pub fn git_show(root: &Path, reference: &str, path: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("show")
        .arg(format!("{reference}:{path}"))
        .output()
        .context("run git show")?;
    if output.status.success() {
        return Ok(Some(String::from_utf8(output.stdout)?));
    }
    let err = String::from_utf8_lossy(&output.stderr);
    if err.contains("does not exist") || err.contains("exists on disk, but not in") {
        return Ok(None);
    }
    bail!("git show {reference}:{path} failed: {err}")
}

/// Whether `reference` names a commit.
pub fn resolves(root: &Path, reference: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "rev-parse",
            "--verify",
            "-q",
            &format!("{reference}^{{commit}}"),
        ])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The base ref of the merge gate: `BOTSTER_CI_BASE_REF`, else `origin/v1`.
pub fn base_ref() -> String {
    base_ref_from(std::env::var("BOTSTER_CI_BASE_REF").ok())
}

/// The base ref from the value of the variable.
pub fn base_ref_from(value: Option<String>) -> String {
    value
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "origin/v1".into())
}

/// What `cargo metadata` says about the workspace and its dependencies.
pub struct Meta {
    pub target_dir: PathBuf,
    /// The checkout of botster-contracts that the workspace depends on (the pinned tag).
    pub contracts_root: PathBuf,
    /// The names of the workspace packages, with the names of their targets' kinds: `(package, has a bin target)`.
    pub members: Vec<(String, bool)>,
    /// Packages that declare a `slow` feature.
    pub slow_packages: Vec<String>,
}

pub fn metadata(root: &Path) -> Result<Meta> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--locked"]);
    let out = caps::apply(&mut command)
        .output()
        .context("run cargo metadata")?;
    if !out.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let packages = json["packages"]
        .as_array()
        .context("no packages in cargo metadata")?;
    let member_ids: Vec<&str> = json["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    let mut members = Vec::new();
    let mut slow_packages = Vec::new();
    let mut contracts_root = None;
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default().to_string();
        let id = package["id"].as_str().unwrap_or_default();
        if member_ids.contains(&id) {
            let has_bin = package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| {
                    t["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|k| k == "bin"))
                });
            members.push((name.clone(), has_bin));
            if package["features"].get("slow").is_some() {
                slow_packages.push(name.clone());
            }
        }
        if name == "botster-core-contract" {
            let manifest = Path::new(package["manifest_path"].as_str().unwrap_or_default());
            // <checkout>/crates/botster-core-contract/Cargo.toml
            contracts_root = manifest
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .map(Path::to_path_buf);
        }
    }
    members.sort();
    slow_packages.sort();
    Ok(Meta {
        target_dir: json["target_directory"]
            .as_str()
            .context("no target_directory")?
            .into(),
        contracts_root: contracts_root
            .context("botster-core-contract is not in the dependency graph")?,
        members,
        slow_packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_is_origin_v1_unless_the_variable_names_another() {
        assert_eq!(base_ref_from(None), "origin/v1");
        assert_eq!(base_ref_from(Some(String::new())), "origin/v1");
        assert_eq!(base_ref_from(Some("origin/x".into())), "origin/x");
    }
}

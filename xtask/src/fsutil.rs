//! File and workspace helpers shared by the commands.

use crate::caps;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

/// The files under `root`, relative to it, without git: a test may run in a copy of the tree with no `.git` (the
/// cargo-mutants copy). A `target` or `vendor` directory and a hidden directory are skipped.
#[cfg(test)]
pub fn walk_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !(name.starts_with('.') || name == "target" || name == "vendor") {
                    dirs.push(path);
                }
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    files.sort();
    files
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

/// The base ref of the merge gate: `BOTSTER_CI_BASE_REF`, else `origin/v1`.
pub fn base_ref() -> String {
    base_ref_from(std::env::var("BOTSTER_CI_BASE_REF").ok())
}

/// The base commit of the run, resolved once (plan section 5, section 8).
///
/// The gate records its base in `BOTSTER_CI_BASE_REF`. Outside the gate, `origin/v1` moves while a run is under way, so the
/// first call resolves the reference to a commit, prints it, and every later check of the run uses that commit. No check
/// resolves the moving branch reference on its own.
pub fn base(root: &Path) -> Result<String> {
    static BASE: BaseCommit = BaseCommit(OnceLock::new());
    BASE.get(root, &base_ref())
}

/// A commit that is resolved once.
struct BaseCommit(OnceLock<String>);

impl BaseCommit {
    fn get(&self, root: &Path, reference: &str) -> Result<String> {
        if let Some(commit) = self.0.get() {
            return Ok(commit.clone());
        }
        let commit = commit_of(root, reference)?;
        eprintln!("base: {reference} = {commit}");
        Ok(self.0.get_or_init(|| commit).clone())
    }
}

/// The commit that `reference` names.
fn commit_of(root: &Path, reference: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "-q"])
        .arg(format!("{reference}^{{commit}}"))
        .output()
        .context("run git rev-parse")?;
    if !output.status.success() {
        bail!("{reference} does not resolve; the base of this run has no commit (fetch it, or set BOTSTER_CI_BASE_REF)");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
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

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// The base is resolved once: a reference that moves later does not change the commit of the run.
    #[test]
    fn the_base_commit_is_resolved_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "one"]);
        git(root, &["tag", "base"]);
        let first = commit_of(root, "base").unwrap();
        assert_eq!(first.len(), 40);
        let cell = BaseCommit(OnceLock::new());
        assert_eq!(cell.get(root, "base").unwrap(), first);
        git(root, &["commit", "-q", "--allow-empty", "-m", "two"]);
        git(root, &["tag", "-f", "base"]);
        assert_ne!(
            commit_of(root, "base").unwrap(),
            first,
            "the reference moved"
        );
        assert_eq!(
            cell.get(root, "base").unwrap(),
            first,
            "the run keeps its base"
        );
        assert!(commit_of(root, "no-such-ref")
            .unwrap_err()
            .to_string()
            .contains("does not resolve"));
    }

    /// `base` resolves `BOTSTER_CI_BASE_REF` to the commit of the run. (The one test that calls it: the commit is kept for the
    /// whole process.)
    #[test]
    fn base_resolves_the_variable_to_a_commit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "one"]);
        git(root, &["tag", "gate-base"]);
        std::env::set_var("BOTSTER_CI_BASE_REF", "gate-base");
        let commit = commit_of(root, "gate-base").unwrap();
        assert_eq!(base(root).unwrap(), commit);
        assert_eq!(base(root).unwrap(), commit);
    }
}

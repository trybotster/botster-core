//! File helpers shared by the commands.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repo root: the directory that holds `contracts.toml`, found from the xtask manifest directory.
/// `BOTSTER_XTASK_ROOT` overrides it, for the tests of xtask itself.
pub fn repo_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("BOTSTER_XTASK_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").context("run xtask through `cargo xtask`")?;
    let root = Path::new(&manifest)
        .parent()
        .context("xtask has a parent directory")?
        .to_path_buf();
    if !root.join("contracts.toml").is_file() {
        bail!("no contracts.toml in {}", root.display());
    }
    Ok(root)
}

/// Every file under `dir` (recursive), as paths relative to `root` with `/` separators, sorted.
pub fn files_under(root: &Path, dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(root, dir, &mut out)?;
    }
    out.sort();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
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
    let text = String::from_utf8(output.stdout)?;
    Ok(text
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

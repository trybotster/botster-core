//! The files of a repository (moved from Core's xtask `fsutil`).

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// `git <args>` in `root`, which must succeed: git runs without the user's configuration, within the cleanup bound.
    fn git(root: &Path, args: &[&str]) {
        let output = botster_test_process::run_to_completion(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null"),
            botster_test_process::Deadline::cleanup(),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(root: &Path, file: &str) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }

    /// The tracked files are the staged ones, by their path from the root; an untracked file is not one, and a directory
    /// that is not a repository is an error.
    #[test]
    fn the_tracked_files_are_the_files_that_git_lists() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q"]);
        for file in ["a.rs", "b/c d.txt", "b/e/f.toml"] {
            write(repo.path(), file);
        }
        git(repo.path(), &["add", "-A"]);
        write(repo.path(), "untracked.rs");
        assert_eq!(
            tracked_files(repo.path()).unwrap(),
            ["a.rs", "b/c d.txt", "b/e/f.toml"]
        );
        let empty = tempfile::tempdir().unwrap();
        git(empty.path(), &["init", "-q"]);
        assert_eq!(tracked_files(empty.path()).unwrap(), Vec::<String>::new());
        let error = tracked_files(&repo.path().join("missing")).unwrap_err();
        assert!(
            error.to_string().starts_with("git ls-files failed: "),
            "{error}"
        );
    }

    /// The walk lists every file by its path from the root, sorted, and skips hidden, `target` and `vendor` directories.
    #[test]
    fn the_walk_lists_the_files_and_skips_hidden_target_and_vendor() {
        let root = tempfile::tempdir().unwrap();
        for file in [
            "z.rs",
            "a/b.rs",
            "a/.hidden-file",
            ".git/config",
            ".cargo/config.toml",
            "target/debug/x",
            "vendor/y/lib.rs",
            "a/target/t.rs",
            "a/vendor/v.rs",
            "targets/kept.rs",
        ] {
            write(root.path(), file);
        }
        assert_eq!(
            walk_files(root.path()),
            ["a/.hidden-file", "a/b.rs", "targets/kept.rs", "z.rs"]
        );
        assert_eq!(
            walk_files(&root.path().join("missing")),
            Vec::<String>::new()
        );
    }
}

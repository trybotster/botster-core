//! The files of a repository (moved from Core's xtask `fsutil`).

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// The output of `git ls-files -z <args>` in `root`.
///
/// # Errors
/// git fails, or its output is not UTF-8.
fn ls_files(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .args(args)
        .output()
        .context("run git ls-files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// The tracked files of the repo, from `git ls-files`.
pub fn tracked_files(root: &Path) -> Result<Vec<String>> {
    Ok(ls_files(root, &[])?
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

/// The mode of a submodule entry in the index (a gitlink): a commit of another repository, not a file.
const GITLINK: &str = "160000";

/// The paths of a `git ls-files -z --stage` listing (`<mode> <object> <stage>\t<path>`, NUL-separated), without the
/// submodule entries ([`GITLINK`]).
///
/// # Errors
/// An entry has no tab or no mode.
pub fn blob_paths(listing: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for entry in listing.split('\0').filter(|entry| !entry.is_empty()) {
        let (info, path) = entry
            .split_once('\t')
            .ok_or_else(|| format!("git ls-files --stage: no tab in {entry:?}"))?;
        let mode = info
            .split(' ')
            .next()
            .filter(|mode| !mode.is_empty())
            .ok_or_else(|| format!("git ls-files --stage: no mode in {entry:?}"))?;
        if mode != GITLINK {
            paths.push(path.to_string());
        }
    }
    Ok(paths)
}

/// Each file of `files` under `root`, with its bytes.
///
/// # Errors
/// A file cannot be read: a check never skips a tracked file in silence (Hub #212 HUBB-3; lead 2026-10-12).
pub fn read_files(root: &Path, files: Vec<String>) -> Result<Vec<(String, Vec<u8>)>> {
    files
        .into_iter()
        .map(|file| {
            let bytes = std::fs::read(root.join(&file))
                .with_context(|| format!("read the tracked file {file}"))?;
            Ok((file, bytes))
        })
        .collect()
}

/// The tracked files of the repo that `keep` selects, with their bytes. A submodule is no file ([`blob_paths`]).
///
/// # Errors
/// git fails, or a selected file cannot be read ([`read_files`]).
pub fn read_tracked(root: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>> {
    let paths = blob_paths(&ls_files(root, &["--stage"])?).map_err(anyhow::Error::msg)?;
    read_files(root, paths.into_iter().filter(|path| keep(path)).collect())
}

/// The files that are UTF-8 text, as text. A file that is not UTF-8 is not text, and a text check leaves it out.
pub fn texts(files: Vec<(String, Vec<u8>)>) -> Vec<(String, String)> {
    files
        .into_iter()
        .filter_map(|(file, bytes)| String::from_utf8(bytes).ok().map(|text| (file, text)))
        .collect()
}

/// Every file as text, with each invalid UTF-8 sequence replaced: a check of every byte of every file (the taint scan).
pub fn lossy(files: Vec<(String, Vec<u8>)>) -> Vec<(String, String)> {
    files
        .into_iter()
        .map(|(file, bytes)| (file, String::from_utf8_lossy(&bytes).into_owned()))
        .collect()
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

    /// The tracked files are read with their bytes; a submodule is no file; a tracked file that cannot be read fails the
    /// read (Hub #212 HUBB-3), and `keep` selects the files.
    #[test]
    fn every_selected_tracked_file_is_read_and_an_unreadable_one_fails() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q"]);
        for file in ["a.rs", "b/c.txt", "gone.rs"] {
            write(repo.path(), file);
        }
        git(repo.path(), &["add", "-A"]);
        // A submodule entry: a gitlink in the index, with no file at its path.
        git(
            repo.path(),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "160000,1111111111111111111111111111111111111111,sub",
            ],
        );
        let keep = |path: &str| path != "gone.rs";
        assert_eq!(
            read_tracked(repo.path(), keep).unwrap(),
            [
                ("a.rs".to_string(), b"x".to_vec()),
                ("b/c.txt".to_string(), b"x".to_vec())
            ]
        );
        std::fs::remove_file(repo.path().join("gone.rs")).unwrap();
        let error = read_tracked(repo.path(), |_| true).unwrap_err();
        assert!(
            format!("{error:#}").starts_with("read the tracked file gone.rs: "),
            "{error:#}"
        );
        assert!(read_tracked(repo.path(), keep).is_ok());
        assert!(read_tracked(&repo.path().join("missing"), keep).is_err());
    }

    #[test]
    fn a_stage_listing_gives_the_paths_without_the_submodules() {
        let listing = [
            "100644 aaaa 0\ta.rs",
            "160000 bbbb 0\tsub",
            "100755 cccc 0\tb/x y.sh",
            "",
        ]
        .join("\0");
        assert_eq!(
            blob_paths(&listing),
            Ok(vec!["a.rs".to_string(), "b/x y.sh".to_string()])
        );
        assert_eq!(blob_paths(""), Ok(Vec::new()));
        assert_eq!(
            blob_paths("100644 aaaa 0 a.rs\0"),
            Err("git ls-files --stage: no tab in \"100644 aaaa 0 a.rs\"".to_string())
        );
        assert_eq!(
            blob_paths("\ta.rs\0"),
            Err("git ls-files --stage: no mode in \"\\ta.rs\"".to_string())
        );
    }

    #[test]
    fn a_text_check_reads_utf8_files_and_the_taint_scan_reads_every_file() {
        let files = || {
            vec![
                ("a.rs".to_string(), b"fn a() {}".to_vec()),
                ("b.png".to_string(), vec![0x89, b'P', 0xff]),
            ]
        };
        assert_eq!(
            texts(files()),
            [("a.rs".to_string(), "fn a() {}".to_string())]
        );
        assert_eq!(
            lossy(files()),
            [
                ("a.rs".to_string(), "fn a() {}".to_string()),
                ("b.png".to_string(), "\u{fffd}P\u{fffd}".to_string())
            ]
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

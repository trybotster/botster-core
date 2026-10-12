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

pub use botster_gate_kit::files::tracked_files;
#[cfg(test)]
pub use botster_gate_kit::files::walk_files;

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

/// The merge base of `HEAD` and the base commit of the run, resolved once.
///
/// A rule that compares the tree with the base counts only the changes of this branch when it reads the base's files here:
/// a change that the base made after the branch point is not a change of this branch. When the head contains the base (the
/// final gate of a merged head), the merge base is the base itself.
pub fn merge_base(root: &Path) -> Result<String> {
    static MERGE_BASE: OnceLock<String> = OnceLock::new();
    if let Some(commit) = MERGE_BASE.get() {
        return Ok(commit.clone());
    }
    let commit = merge_base_of(root, "HEAD", &base(root)?)?;
    Ok(MERGE_BASE.get_or_init(|| commit).clone())
}

/// The merge base of two commits. With no merge base (a shallow snapshot, or unrelated histories) it is an error that names
/// both commits, so a step that needs it fails closed.
pub fn merge_base_of(root: &Path, head: &str, base: &str) -> Result<String> {
    let head = commit_of(root, head)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", &head, base])
        .output()
        .context("run git merge-base")?;
    if !output.status.success() {
        bail!(
            "{head} and {base} have no merge base (a shallow snapshot or unrelated histories), so the changes of this \
             branch are not known: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
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

/// The output of `git <args>` in `root` for a test, which must succeed: git runs without the user's configuration, and
/// within the cleanup bound.
#[cfg(test)]
pub(crate) fn test_git(root: &Path, args: &[&str]) -> String {
    let output = botster_test_process::run_to_completion(
        Command::new("git")
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
    String::from_utf8(output.stdout).unwrap()
}

/// A git repository in a temporary directory for the test of a command: each `(path, text)` of `files` is written and
/// staged, so `tracked_files` lists it.
#[cfg(test)]
pub(crate) fn test_repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    test_git(dir.path(), &["init", "-q"]);
    for (path, text) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    test_git(dir.path(), &["add", "-A"]);
    dir
}

/// The checkout of the contracts repository that `lock` (the text of `Cargo.lock`) pins for `botster-core-contract`,
/// among `checkouts`: the directories `<CARGO_HOME>/git/checkouts/<repository>-<hash>/<short rev>`. The checkout is the
/// one directory under `<repository>-*` whose name is a prefix of the pinned rev, at least 7 hexadecimal digits. A
/// default-tier test uses it in place of `cargo metadata`, which can wait on the shared Cargo home lock (Core lead,
/// 2026-10-12, #306 re-gate); the slow tier compares it with [`metadata`].
#[cfg(test)]
pub(crate) fn pinned_contracts_checkout(
    lock: &str,
    checkouts: &[PathBuf],
) -> Result<PathBuf, String> {
    let lock: toml::Table = lock
        .parse()
        .map_err(|e| format!("Cargo.lock is not TOML: {e}"))?;
    let source = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .find(|package| {
            package.get("name").and_then(toml::Value::as_str) == Some("botster-core-contract")
        })
        .ok_or("Cargo.lock has no botster-core-contract")?
        .get("source")
        .and_then(toml::Value::as_str)
        .ok_or("botster-core-contract has no source in Cargo.lock")?;
    let (url, rev) = source
        .strip_prefix("git+")
        .and_then(|rest| rest.rsplit_once('#'))
        .ok_or_else(|| format!("the source {source:?} is not a git source with a rev"))?;
    let repository = url
        .split('?')
        .next()
        .and_then(|path| path.trim_end_matches('/').rsplit('/').next())
        .map(|name| name.trim_end_matches(".git"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("the source {source:?} names no repository"))?;
    let found: Vec<&PathBuf> = checkouts
        .iter()
        .filter(|checkout| {
            let short = checkout
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let parent = checkout
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            parent.starts_with(&format!("{repository}-"))
                && short.len() >= 7
                && short.chars().all(|c| c.is_ascii_hexdigit())
                && rev.starts_with(short)
        })
        .collect();
    match found.as_slice() {
        [one] => Ok(one.to_path_buf()),
        [] => Err(format!(
            "no checkout of {repository} at {rev} among {} checkouts",
            checkouts.len()
        )),
        many => Err(format!(
            "{} checkouts of {repository} at {rev}: {}",
            many.len(),
            many.iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// [`pinned_contracts_checkout`] for the workspace at `root`: its `Cargo.lock`, and the checkouts of the Cargo home
/// (`CARGO_HOME`, or `~/.cargo`). Reads files only; starts no process.
#[cfg(test)]
pub(crate) fn contracts_checkout(root: &Path) -> Result<PathBuf> {
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).context("read Cargo.lock")?;
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))
        .context("no CARGO_HOME and no HOME")?;
    let mut checkouts = Vec::new();
    let git = home.join("git/checkouts");
    for repository in std::fs::read_dir(&git).with_context(|| format!("read {}", git.display()))? {
        let repository = repository?.path();
        if repository.is_dir() {
            for checkout in std::fs::read_dir(&repository)? {
                checkouts.push(checkout?.path());
            }
        }
    }
    pinned_contracts_checkout(&lock, &checkouts).map_err(anyhow::Error::msg)
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
        test_git(root, args);
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

    /// `base` resolves `BOTSTER_CI_BASE_REF` to the commit of the run. The variable and the resolved commit belong to the
    /// whole process, so the test runs alone in a new process that has the variable (`botster_test_process::alone`); it no
    /// longer sets the variable in the runner's process.
    #[test]
    fn base_resolves_the_variable_to_a_commit() {
        const NAME: &str = "fsutil::tests::base_resolves_the_variable_to_a_commit";
        if !botster_test_process::alone::is_alone(NAME) {
            return botster_test_process::alone::run_alone(
                NAME,
                &[("BOTSTER_CI_BASE_REF", "gate-base")],
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "one"]);
        git(root, &["tag", "gate-base"]);
        let commit = commit_of(root, "gate-base").unwrap();
        assert_eq!(base(root).unwrap(), commit);
        assert_eq!(base(root).unwrap(), commit);
    }

    /// A repository whose `main` and `side` part at the commit `fork`: `main` then has `later`, `side` has `own`.
    fn forked() -> (tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "fork"]);
        let fork = commit_of(root, "HEAD").unwrap();
        git(root, &["branch", "side"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "later"]);
        let later = commit_of(root, "HEAD").unwrap();
        git(root, &["checkout", "-q", "side"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "own"]);
        (dir, fork, later)
    }

    #[test]
    fn the_merge_base_is_the_branch_point_or_the_base_that_the_head_contains() {
        let (dir, fork, later) = forked();
        let root = dir.path();
        assert_eq!(merge_base_of(root, "HEAD", &later).unwrap(), fork);
        git(root, &["merge", "-q", "--no-edit", "main"]);
        assert_eq!(
            merge_base_of(root, "HEAD", &later).unwrap(),
            later,
            "a merged head contains the base"
        );
    }

    #[test]
    fn unrelated_histories_have_no_merge_base_and_the_error_names_both_commits() {
        let (dir, _, later) = forked();
        let root = dir.path();
        git(root, &["checkout", "-q", "--orphan", "alone"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "alone"]);
        let alone = commit_of(root, "HEAD").unwrap();
        let error = merge_base_of(root, "HEAD", &later).unwrap_err().to_string();
        assert!(
            error.contains(&alone) && error.contains(&later) && error.contains("no merge base"),
            "{error}"
        );
        assert!(merge_base_of(root, "no-such-ref", &later).is_err());
    }

    /// `merge_base` is the merge base of `HEAD` and the base of the run (`BOTSTER_CI_BASE_REF`), resolved once. As in
    /// `base_resolves_the_variable_to_a_commit`, the test runs alone in a new process that has the variable.
    #[test]
    fn merge_base_is_of_the_head_and_the_base_of_the_run() {
        const NAME: &str = "fsutil::tests::merge_base_is_of_the_head_and_the_base_of_the_run";
        if !botster_test_process::alone::is_alone(NAME) {
            return botster_test_process::alone::run_alone(
                NAME,
                &[("BOTSTER_CI_BASE_REF", "main")],
            );
        }
        let (dir, fork, _) = forked();
        let root = dir.path();
        assert_eq!(merge_base(root).unwrap(), fork);
        git(root, &["merge", "-q", "--no-edit", "main"]);
        assert_eq!(
            merge_base(root).unwrap(),
            fork,
            "the run keeps its merge base"
        );
    }

    /// The pinned checkout from a lock and a list of checkouts, with no cargo call: the one directory under
    /// `<repository>-*` whose name is a prefix of the pinned rev (7 or more hexadecimal digits).
    #[test]
    fn the_pinned_contracts_checkout_comes_from_the_lock_and_the_checkouts() {
        let rev = "c69a3190b4d66e84401006e9d73788ff0e837f56";
        let lock = |source: &str| {
            format!(
                "version = 4\n\n[[package]]\nname = \"botster-core-contract\"\nversion = \"0.1.0\"\nsource = \"{source}\"\n"
            )
        };
        let pinned = lock(&format!(
            "git+https://github.com/trybotster/botster-contracts?tag=contracts-v0.1.43#{rev}"
        ));
        let home = Path::new("/h/git/checkouts");
        let paths =
            |names: &[&str]| -> Vec<PathBuf> { names.iter().map(|name| home.join(name)).collect() };
        let checkouts = paths(&[
            "botster-contracts-4d2a/0389165",
            "botster-contracts-4d2a/c69a319",
            "other-repo-1111/c69a319",
            "botster-contracts-4d2a/c69a31",
            "botster-contracts-4d2a/c69a31x",
        ]);
        assert_eq!(
            pinned_contracts_checkout(&pinned, &checkouts),
            Ok(home.join("botster-contracts-4d2a/c69a319"))
        );
        // A longer short rev is a prefix too.
        assert_eq!(
            pinned_contracts_checkout(&pinned, &paths(&["botster-contracts-4d2a/c69a3190b"])),
            Ok(home.join("botster-contracts-4d2a/c69a3190b"))
        );
        let error = |lock: &str, names: &[&str]| {
            pinned_contracts_checkout(lock, &paths(names)).unwrap_err()
        };
        assert!(error(
            &pinned,
            &["botster-contracts-4d2a/0389165", "other-repo-1111/c69a319"]
        )
        .starts_with("no checkout of botster-contracts at c69a3190"),);
        assert!(error(
            &pinned,
            &[
                "botster-contracts-4d2a/c69a319",
                "botster-contracts-9999/c69a319"
            ]
        )
        .starts_with("2 checkouts of botster-contracts at c69a3190"),);
        assert_eq!(
            error("version = 4\n", &[]),
            "Cargo.lock has no botster-core-contract"
        );
        assert!(
            error(&lock("registry+https://x/y"), &[]).contains("is not a git source with a rev")
        );
        assert!(
            error(&lock("git+https://x/y?tag=t"), &[]).contains("is not a git source with a rev")
        );
        assert!(error("[", &[]).starts_with("Cargo.lock is not TOML"));
    }
}

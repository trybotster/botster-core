//! The build-root check (bug catalog B13; design `design-gs-b13-stale-binary.md`, Core lead 2026-10-11).
//!
//! Two worktrees of this workspace with the same `Cargo.lock` give the same artifact names in one target directory. Cargo
//! can then find the other worktree's artifacts fresh, compile nothing, and run a test binary built from the other
//! worktree's sources. A test binary refuses to run in that case: each workspace crate exports its compile-time manifest
//! directory (`BUILT_IN`), and [`check`] compares the workspace root of each one with the workspace root of the running test
//! (the runtime `CARGO_MANIFEST_DIR`, which cargo and nextest set). There is no override (lead ruling).

use std::path::{Path, PathBuf};

/// The variable that cargo and nextest set for a test process: the manifest directory of the test's package.
pub const MANIFEST_DIR_ENV: &str = "CARGO_MANIFEST_DIR";

/// The workspace root of a package directory: `<root>/crates/<package>` or `<root>/<package>` (xtask).
fn workspace_root(manifest_dir: &Path) -> Option<&Path> {
    let parent = manifest_dir.parent()?;
    if parent.file_name().is_some_and(|name| name == "crates") {
        parent.parent()
    } else {
        Some(parent)
    }
}

/// The canonical form of a path, or the path as given when it cannot be resolved (a removed worktree): then it differs
/// from every existing root.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The canonical workspace root of the running test, from its runtime manifest directory. A test that cargo or nextest did
/// not start has none, and is refused (no override, lead ruling).
fn running_root(running: Option<&Path>) -> Result<PathBuf, String> {
    let running = running.ok_or_else(|| {
        format!("{MANIFEST_DIR_ENV} is not set: run this test through cargo or nextest, or set {MANIFEST_DIR_ENV} to the test package's directory (bug catalog B13)")
    })?;
    workspace_root(running).map(canonical).ok_or_else(|| {
        format!(
            "{MANIFEST_DIR_ENV} {} has no workspace root",
            running.display()
        )
    })
}

/// Checks that every crate of `built` (name, compile-time manifest directory) was built in the workspace of `running`
/// (the runtime manifest directory of the test). The error names each crate built in another workspace, both roots and the
/// `cargo clean` command.
pub fn check(running: Option<&Path>, built: &[(&str, &str)]) -> Result<(), String> {
    let here = running_root(running)?;
    let others: Vec<String> = built
        .iter()
        .filter_map(|&(name, dir)| {
            let root = workspace_root(Path::new(dir)).map(canonical);
            (root.as_deref() != Some(here.as_path())).then(|| format!("{name} was built in {dir}"))
        })
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    // The test binary is `<target>/<profile>/deps/<test>`.
    let target = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(|dir| dir.display().to_string()))
        .unwrap_or_else(|| "<the target directory>".to_string());
    Err(format!(
        "this test runs in {}, but {}: a shared target directory reused another worktree's build (bug catalog B13); run `cargo clean --target-dir {target}`, then build again",
        here.display(),
        others.join(", ")
    ))
}

/// [`check`] with the runtime `CARGO_MANIFEST_DIR` of this process.
pub fn check_here(built: &[(&str, &str)]) -> Result<(), String> {
    let running = std::env::var_os(MANIFEST_DIR_ENV).map(PathBuf::from);
    check(running.as_deref(), built)
}

/// The file of `target/candidate/` that names the workspace that `cargo xtask prebuild-worker` built the candidate
/// binaries from (one line, the canonical root).
pub const SOURCE_ROOT_FILE: &str = "source-root";

/// Checks that the candidate binaries of `candidate` were prebuilt from the workspace of `running` (the runtime manifest
/// directory of the test). A missing record is refused: the prebuild step did not run in this form.
pub fn check_candidate(candidate: &Path, running: Option<&Path>) -> Result<(), String> {
    let record = candidate.join(SOURCE_ROOT_FILE);
    let text = std::fs::read_to_string(&record).map_err(|error| {
        format!(
            "no {} ({error}): run `cargo xtask prebuild-worker` in this worktree",
            record.display()
        )
    })?;
    let built = text.trim_end_matches('\n');
    let here = running_root(running)?;
    if canonical(Path::new(built)) == here {
        return Ok(());
    }
    let target = candidate.parent().unwrap_or(candidate).display();
    Err(format!(
        "the candidate binaries of {} were prebuilt from {built}, and this test runs in {} (bug catalog B13): run `cargo clean --target-dir {target}`, then `cargo xtask prebuild-worker` in this worktree",
        candidate.display(),
        here.display()
    ))
}

/// [`check_candidate`] with the runtime `CARGO_MANIFEST_DIR` of this process.
pub fn check_candidate_here(candidate: &Path) -> Result<(), String> {
    let running = std::env::var_os(MANIFEST_DIR_ENV).map(PathBuf::from);
    check_candidate(candidate, running.as_deref())
}

/// The unit test of each workspace crate: its test binary was built in the worktree where it runs.
pub fn assert_built_here(name: &str, manifest_dir: &str) {
    if let Err(error) = check_here(&[(name, manifest_dir)]) {
        panic!("{error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_workspace_root_is_above_crates_or_above_the_package() {
        assert_eq!(
            workspace_root(Path::new("/w/crates/a")),
            Some(Path::new("/w"))
        );
        assert_eq!(workspace_root(Path::new("/w/xtask")), Some(Path::new("/w")));
        assert_eq!(workspace_root(Path::new("/")), None);
    }

    #[test]
    fn crates_built_in_the_running_workspace_pass() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(root.join("crates/core")).unwrap();
        std::fs::create_dir_all(root.join("xtask")).unwrap();
        let core = root.join("crates/core");
        let xtask = root.join("xtask");
        assert_eq!(
            check(
                Some(&core),
                &[
                    ("core", core.to_str().unwrap()),
                    ("xtask", xtask.to_str().unwrap())
                ]
            ),
            Ok(())
        );
    }

    #[test]
    fn a_crate_built_in_another_workspace_is_named_with_both_roots() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a/crates/core");
        let b = dir.path().join("b/crates/core");
        let b_kit = dir.path().join("b/crates/kit");
        for path in [&a, &b, &b_kit] {
            std::fs::create_dir_all(path).unwrap();
        }
        let error = check(
            Some(&b),
            &[
                ("core", a.to_str().unwrap()),
                ("kit", b_kit.to_str().unwrap()),
            ],
        )
        .unwrap_err();
        assert!(
            error.contains(&format!("core was built in {}", a.display())),
            "{error}"
        );
        assert!(!error.contains("kit was built"), "{error}");
        assert!(
            error.contains(&canonical(&dir.path().join("b")).display().to_string()),
            "{error}"
        );
        assert!(
            error.contains("B13") && error.contains("cargo clean --target-dir"),
            "{error}"
        );
    }

    #[test]
    fn a_removed_build_root_is_another_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("b/crates/core");
        std::fs::create_dir_all(&b).unwrap();
        let gone = dir.path().join("gone/crates/core");
        assert!(check(Some(&b), &[("core", gone.to_str().unwrap())]).is_err());
    }

    #[test]
    fn a_symlinked_root_is_the_same_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("crates/core")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let built = real.join("crates/core");
        assert_eq!(
            check(
                Some(&link.join("crates/core")),
                &[("core", built.to_str().unwrap())]
            ),
            Ok(())
        );
    }

    #[test]
    fn a_candidate_prebuilt_in_another_workspace_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let candidate = dir.path().join("target/candidate");
        for path in [&a.join("crates/core"), &b.join("crates/core"), &candidate] {
            std::fs::create_dir_all(path).unwrap();
        }
        let running = b.join("crates/core");
        let error = check_candidate(&candidate, Some(&running)).unwrap_err();
        assert!(
            error.contains("run `cargo xtask prebuild-worker` in this worktree"),
            "{error}"
        );
        std::fs::write(
            candidate.join(SOURCE_ROOT_FILE),
            format!("{}\n", canonical(&a).display()),
        )
        .unwrap();
        let error = check_candidate(&candidate, Some(&running)).unwrap_err();
        assert!(
            error.contains(&format!("prebuilt from {}", canonical(&a).display())),
            "{error}"
        );
        assert!(
            error.contains(&format!("this test runs in {}", canonical(&b).display())),
            "{error}"
        );
        assert!(
            error.contains(&format!(
                "cargo clean --target-dir {}",
                dir.path().join("target").display()
            )),
            "{error}"
        );
        std::fs::write(
            candidate.join(SOURCE_ROOT_FILE),
            format!("{}\n", canonical(&b).display()),
        )
        .unwrap();
        assert_eq!(check_candidate(&candidate, Some(&running)), Ok(()));
        assert!(check_candidate(&candidate, None).is_err());
    }

    /// The `_here` forms read this process's runtime `CARGO_MANIFEST_DIR` (this crate's directory): a crate built in
    /// another workspace, and a candidate directory with no record, are refused.
    #[test]
    #[should_panic(expected = "elsewhere was built in /elsewhere/crates/x")]
    fn the_unit_test_form_panics_for_a_crate_of_another_workspace() {
        assert_built_here("elsewhere", "/elsewhere/crates/x");
    }

    #[test]
    fn the_here_forms_use_the_runtime_manifest_directory() {
        assert_eq!(check_here(&[("self", env!("CARGO_MANIFEST_DIR"))]), Ok(()));
        assert!(check_here(&[("other", "/elsewhere/crates/x")]).is_err());
        let dir = tempfile::tempdir().unwrap();
        let error = check_candidate_here(dir.path()).unwrap_err();
        assert!(
            error.contains("run `cargo xtask prebuild-worker`"),
            "{error}"
        );
    }

    #[test]
    fn a_test_without_the_variable_is_refused() {
        let error = check(None, &[("core", "/w/crates/core")]).unwrap_err();
        assert!(
            error.contains("run this test through cargo or nextest"),
            "{error}"
        );
    }
}

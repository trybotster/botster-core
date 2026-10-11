//! The record of a passed slow tier, which the stage-2 baseline of the mutation step reuses (GS lever L2).
//!
//! The slow step (`cargo xtask test-budget --slow`) and the stage-2 baseline run the same tests: the slow selection
//! (`test_budget::slow_selection`) over the unmutated tree, each bounded by the slow deadline with the leftover check.
//! When the slow tier passes, it writes a record: the commit, whether the tree is clean, the selection, and each test that
//! the selection runs, with its test binary (`cargo nextest list`). The binary path holds cargo's metadata hash of the
//! build (profile, features, dependencies). `cargo xtask ci` removes the record before its first step, so only the slow
//! step of the same run can leave one. Before stage 2, the step makes the same record again; when the two are equal and
//! the tree is clean, the slow step of this run already passed exactly the tests of the baseline from the same builds, and
//! the baseline is not run again. On any difference, or with no record, the baseline runs, and the step prints why.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The record file under the target directory.
pub fn path(target_dir: &Path) -> PathBuf {
    target_dir
        .join("nextest")
        .join("slow")
        .join("passed-tests.txt")
}

/// The tests that a selection runs, from the JSON of `cargo nextest list --message-format json`: one line
/// `<binary id>\t<binary path>\t<test name>` for each test that the filter matches and that is not ignored, sorted.
///
/// # Errors
/// The text is not JSON, or it has no `rust-suites` map, or a suite has no binary path or test map.
pub fn parse_list(json: &str) -> Result<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse the nextest list")?;
    let suites = v["rust-suites"]
        .as_object()
        .context("the nextest list has no `rust-suites`")?;
    let mut tests = Vec::new();
    for (id, suite) in suites {
        let binary = suite["binary-path"]
            .as_str()
            .with_context(|| format!("the suite {id} has no binary path"))?;
        let cases = suite["testcases"]
            .as_object()
            .with_context(|| format!("the suite {id} has no testcases"))?;
        for (name, case) in cases {
            let runs = case["filter-match"]["status"] == "matches" && case["ignored"] == false;
            if runs {
                tests.push(format!("{id}\t{binary}\t{name}"));
            }
        }
    }
    tests.sort();
    Ok(tests)
}

/// The text of a record.
pub fn record_text(head: &str, clean: bool, selection: &[String], tests: &[String]) -> String {
    let mut text = format!(
        "head {head}\nclean {clean}\nselection {}\n",
        selection.join("\t")
    );
    for test in tests {
        text.push_str("test ");
        text.push_str(test);
        text.push('\n');
    }
    text
}

/// Whether the stage-2 baseline can reuse the slow step's pass: the number of tests that the slow step passed, or why
/// the baseline must run. `stored` is the slow step's record of this run (`None`: there is none), `current` the record
/// made before stage 2. Both must name a commit, a clean tree and a selection; then they must be equal, line for line.
pub fn reuse(stored: Option<&str>, current: &str) -> std::result::Result<usize, String> {
    let Some(stored) = stored else {
        return Err("the slow step of this run left no record of a pass".into());
    };
    for (name, text) in [
        ("the slow step's record", stored),
        ("the tree now", current),
    ] {
        let lines: Vec<&str> = text.lines().collect();
        let complete = lines.len() >= 3
            && lines[0].starts_with("head ")
            && lines[0].len() > "head ".len()
            && lines[2].starts_with("selection ");
        if !complete {
            return Err(format!("{name} is not a complete record"));
        }
        if lines[1] != "clean true" {
            return Err(format!("{name}: the tree differs from its commit"));
        }
    }
    if let Some((was, now)) = stored
        .lines()
        .map(Some)
        .chain(std::iter::repeat(None))
        .zip(current.lines().map(Some).chain(std::iter::repeat(None)))
        .take_while(|pair| *pair != (None, None))
        .find(|(was, now)| was != now)
    {
        return Err(format!(
            "the records differ: the slow step has {}, the tree now has {}",
            was.unwrap_or("no more lines"),
            now.unwrap_or("no more lines")
        ));
    }
    Ok(current.lines().filter(|l| l.starts_with("test ")).count())
}

/// The record of `selection` on the tree at `root` now: the commit, whether `git status --porcelain` is empty (untracked
/// files count), and `cargo nextest list` of the selection, with the environment `env` of the tier. The list builds the
/// selection first. An I/O shell: its decisions are `parse_list` and `record_text`, and `reuse` judges its result.
///
/// # Errors
/// git or the list did not start or failed, or the list is not readable.
pub fn current(root: &Path, selection: &[String], env: &[(&str, &str)]) -> Result<String> {
    let git = |args: &[&str]| -> Result<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .context("run git")?;
        crate::tools::stdout_of(&output, "git")
    };
    let head = git(&["rev-parse", "HEAD"])?;
    let status = git(&["status", "--porcelain", "--untracked-files=all"])?;
    let mut list = crate::tools::cargo(root);
    list.args(["nextest", "list", "--message-format", "json"])
        .args(selection)
        .envs(env.iter().copied());
    let output = list.output().context("start cargo nextest list")?;
    let tests = parse_list(&crate::tools::stdout_of(&output, "cargo nextest list")?)?;
    Ok(record_text(
        head.trim(),
        status.trim().is_empty(),
        selection,
        &tests,
    ))
}

/// Writes the record of `selection` (`current`) for a slow tier that passed.
///
/// # Errors
/// The record could not be made or written.
pub fn write(
    root: &Path,
    target_dir: &Path,
    selection: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let text = current(root, selection, env)?;
    let file = path(target_dir);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, text).with_context(|| format!("write {}", file.display()))
}

/// Removes the record, so that a later step reads only a record that this run wrote.
///
/// # Errors
/// The record exists and could not be removed.
pub fn clear(target_dir: &Path) -> Result<()> {
    match std::fs::remove_file(path(target_dir)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            bail!("remove the slow tier's record: {error}")
        }
        _ => Ok(()),
    }
}

/// The decision of `reuse` on the stored record (`None` when there is none) and the record of `selection` now.
///
/// # Errors
/// The record could not be read, or the record now could not be made.
pub fn check(
    root: &Path,
    target_dir: &Path,
    selection: &[String],
    env: &[(&str, &str)],
) -> Result<std::result::Result<usize, String>> {
    let stored = std::fs::read_to_string(path(target_dir)).ok();
    let now = current(root, selection, env)?;
    Ok(reuse(stored.as_deref(), &now))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"{"rust-build-meta":{},"test-count":3,"rust-suites":{
        "p::slow_a":{"binary-path":"/t/debug/deps/slow_a-1111","testcases":{
            "x":{"ignored":false,"filter-match":{"status":"matches"}},
            "y":{"ignored":true,"filter-match":{"status":"matches"}},
            "z":{"ignored":false,"filter-match":{"status":"mismatch","reason":"expression"}}}},
        "p":{"binary-path":"/t/debug/deps/p-2222","testcases":{
            "m::slow_tests::w":{"ignored":false,"filter-match":{"status":"matches"}}}}}}"#;

    #[test]
    fn the_listed_tests_are_the_matching_tests_that_are_not_ignored_with_their_binaries() {
        assert_eq!(
            parse_list(LIST).unwrap(),
            [
                "p\t/t/debug/deps/p-2222\tm::slow_tests::w",
                "p::slow_a\t/t/debug/deps/slow_a-1111\tx"
            ]
        );
        assert!(parse_list("{}").is_err());
        assert!(parse_list(r#"{"rust-suites":{"s":{"testcases":{}}}}"#).is_err());
        assert!(parse_list(r#"{"rust-suites":{"s":{"binary-path":"b"}}}"#).is_err());
        assert!(parse_list("not json").is_err());
    }

    fn record(head: &str, clean: bool, selection: &[&str], tests: &[&str]) -> String {
        let selection: Vec<String> = selection.iter().map(|s| s.to_string()).collect();
        let tests: Vec<String> = tests.iter().map(|s| s.to_string()).collect();
        record_text(head, clean, &selection, &tests)
    }

    #[test]
    fn a_record_names_the_commit_the_clean_tree_the_selection_and_each_test() {
        assert_eq!(
            record(
                "abc",
                true,
                &["--workspace", "-E", "f"],
                &["b\tp\tt1", "b\tp\tt2"]
            ),
            "head abc\nclean true\nselection --workspace\t-E\tf\ntest b\tp\tt1\ntest b\tp\tt2\n"
        );
    }

    /// The baseline is reused only for an equal, complete record of a clean tree; every other case runs it, with the reason.
    #[test]
    fn the_baseline_is_reused_only_for_the_same_clean_tree_selection_and_tests() {
        let base = record("abc", true, &["s"], &["b\tp\tt1", "b\tp\tt2"]);
        assert_eq!(reuse(Some(&base), &base), Ok(2));
        let no_tests = record("abc", true, &["s"], &[]);
        assert_eq!(reuse(Some(&no_tests), &no_tests), Ok(0));

        let refused = |stored: Option<&str>, now: &str, why: &str| {
            let error = reuse(stored, now).unwrap_err();
            assert!(error.contains(why), "{error}");
        };
        refused(None, &base, "left no record");
        refused(Some(""), &base, "not a complete record");
        refused(Some(&base), "", "not a complete record");
        refused(Some(&base), "xyzzy", "not a complete record");
        refused(
            Some(&record("", true, &["s"], &[])),
            &base,
            "not a complete record",
        );
        refused(
            Some(&base),
            &base.replace("selection ", "selected "),
            "not a complete record",
        );
        refused(
            Some(&record("abc", false, &["s"], &["b\tp\tt1", "b\tp\tt2"])),
            &base,
            "the slow step's record: the tree differs",
        );
        refused(
            Some(&base),
            &record("abc", false, &["s"], &["b\tp\tt1", "b\tp\tt2"]),
            "the tree now: the tree differs",
        );
        refused(
            Some(&base),
            &record("abd", true, &["s"], &["b\tp\tt1", "b\tp\tt2"]),
            "the slow step has head abc, the tree now has head abd",
        );
        refused(
            Some(&base),
            &record("abc", true, &["r"], &["b\tp\tt1", "b\tp\tt2"]),
            "the slow step has selection s, the tree now has selection r",
        );
        refused(
            Some(&base),
            &record("abc", true, &["s"], &["b\tq\tt1", "b\tp\tt2"]),
            "the slow step has test b\tp\tt1, the tree now has test b\tq\tt1",
        );
        refused(
            Some(&base),
            &record("abc", true, &["s"], &["b\tp\tt1"]),
            "the slow step has test b\tp\tt2, the tree now has no more lines",
        );
        refused(
            Some(&record("abc", true, &["s"], &["b\tp\tt1"])),
            &base,
            "the slow step has no more lines, the tree now has test b\tp\tt2",
        );
    }

    #[test]
    fn the_record_lives_under_the_slow_profile_of_the_target_directory() {
        assert_eq!(
            path(Path::new("/t")),
            Path::new("/t/nextest/slow/passed-tests.txt")
        );
    }

    #[test]
    fn clearing_removes_the_record_and_a_missing_one_is_no_error() {
        let dir = tempfile::tempdir().unwrap();
        clear(dir.path()).unwrap();
        let file = path(dir.path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "head x\n").unwrap();
        clear(dir.path()).unwrap();
        assert!(!file.exists());
        std::fs::create_dir_all(&file).unwrap();
        assert!(clear(dir.path()).is_err());
    }
}

#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;
    use botster_test_process::{run_to_completion, Deadline};

    /// The shells on a real repository and a real `cargo nextest list`: the record of a passed tier is reused on the same
    /// tree; a changed file, a cleared record or another selection runs the baseline.
    #[test]
    fn slow_a_record_is_reused_only_for_the_same_tree_and_selection() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/mutants-hang");
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let target = repo.join("target");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::copy(fixture.join("Cargo.toml.in"), repo.join("Cargo.toml")).unwrap();
        std::fs::copy(fixture.join("src/lib.rs"), repo.join("src/lib.rs")).unwrap();
        std::fs::write(repo.join(".gitignore"), "target/\nCargo.lock\n").unwrap();
        let git = |args: &[&str]| {
            let mut command = Command::new("git");
            command
                .arg("-C")
                .arg(&repo)
                .args(["-c", "user.name=gs", "-c", "user.email=gs@localhost"])
                .args(args);
            let output = run_to_completion(
                &mut command,
                Deadline::after(crate::test_budget::SLOW_DEADLINE),
            )
            .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "fixture"]);
        let selection: Vec<String> = ["--workspace", "-E", "test(/ready/)"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let env = crate::test_budget::tier_env(true);

        assert_eq!(
            check(&repo, &target, &selection, &env).unwrap(),
            Err("the slow step of this run left no record of a pass".to_string())
        );
        write(&repo, &target, &selection, &env).unwrap();
        assert_eq!(check(&repo, &target, &selection, &env).unwrap(), Ok(2));
        let other: Vec<String> = ["--workspace", "-E", "test(/second/)"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(check(&repo, &target, &other, &env)
            .unwrap()
            .unwrap_err()
            .contains("the records differ"));
        std::fs::write(repo.join("src/extra.rs"), "").unwrap();
        assert!(check(&repo, &target, &selection, &env)
            .unwrap()
            .unwrap_err()
            .contains("the tree now: the tree differs"));
        std::fs::remove_file(repo.join("src/extra.rs")).unwrap();
        assert_eq!(check(&repo, &target, &selection, &env).unwrap(), Ok(2));
        clear(&target).unwrap();
        assert!(check(&repo, &target, &selection, &env).unwrap().is_err());
    }
}

use super::*;
use std::process::Command;

/// A fact set that passes every condition: the reviewed diff is unchanged, and the base changed another path.
fn passing() -> Facts {
    Facts {
        reviewed: "r".into(),
        new: "n".into(),
        base: "b".into(),
        reviewed_base: "rb".into(),
        new_base: "nb".into(),
        new_descends: true,
        base_moved_forward: true,
        merge_tree: (Some(0), "tree\n".into()),
        base_paths: "docs/a.md\n".into(),
        reviewed_paths: "xtask/src/main.rs\n".into(),
        reviewed_diff: b"diff --git a/xtask/src/main.rs\n+x\n".to_vec(),
        new_diff: b"diff --git a/xtask/src/main.rs\n+x\n".to_vec(),
    }
}

#[test]
fn a_clean_merge_has_no_conflict_and_a_conflict_names_its_paths() {
    assert_eq!(conflicts(Some(0), "4b825dc\n"), Ok(Vec::new()));
    assert_eq!(
        conflicts(
            Some(1),
            "4b825dc\nCargo.lock\nxtask/src/ci.rs\n\nAuto-merging\n"
        ),
        Ok(vec![
            "Cargo.lock".to_string(),
            "xtask/src/ci.rs".to_string()
        ])
    );
    assert!(conflicts(Some(128), "fatal").is_err());
    assert!(conflicts(None, "").is_err());
}

#[test]
fn only_a_path_of_both_diffs_overlaps() {
    assert_eq!(overlap("a\nb\n", "c\nd\n"), Vec::<String>::new());
    assert_eq!(overlap("a\nb\n", "b\nc\n"), ["b"]);
    assert_eq!(overlap("", "b\n"), Vec::<String>::new());
}

#[test]
fn the_first_difference_of_two_diffs_names_its_line() {
    assert_eq!(first_difference(b"a\nb\n", b"a\nb\n"), None);
    assert_eq!(first_difference(b"", b""), None);
    assert_eq!(
        first_difference(b"a\nb\nc", b"a\nB\nc"),
        Some((2, "b".to_string(), "B".to_string()))
    );
    assert_eq!(
        first_difference(b"a", b"a\nb"),
        Some((2, "(end of the diff)".to_string(), "b".to_string()))
    );
}

#[test]
fn a_base_only_merge_that_keeps_the_diff_passes_with_its_report() {
    let report = judge(&passing()).unwrap();
    assert!(!report.contains("FAIL"), "{report}");
    assert!(
        report.contains("byte-identical: 34 bytes, sha256 "),
        "{report}"
    );
    assert!(report.ends_with(
        "result: PASS (no reviewer delta round is needed; the gate still runs on the new head)\n"
    ));
}

/// Each condition alone fails the check, and the report names the cause.
#[test]
fn each_failed_condition_fails_the_check_and_names_its_cause() {
    let mut cases: Vec<(Facts, &str)> = Vec::new();
    let mut f = passing();
    f.new_descends = false;
    cases.push((f, "FAIL the new head descends from the reviewed head r"));
    let mut f = passing();
    f.base_moved_forward = false;
    cases.push((
        f,
        "FAIL the new merge base descends from the reviewed merge base rb",
    ));
    let mut f = passing();
    f.merge_tree = (Some(1), "tree\nxtask/src/main.rs\n".into());
    cases.push((f, "FAIL (1) the merge-tree of the reviewed head and the new merge base has no conflict: xtask/src/main.rs"));
    let mut f = passing();
    f.base_paths = "docs/a.md\nxtask/src/main.rs\n".into();
    cases.push((f, "FAIL (2) none of the 2 paths that changed on the base is in the pull request's own diff (1 paths): xtask/src/main.rs"));
    let mut f = passing();
    f.new_diff = b"diff --git a/xtask/src/main.rs\n+y\n".to_vec();
    cases.push((
        f,
        "FAIL (3) the pull request's own diff differs at line 2:\n  reviewed: +x\n  new:      +y",
    ));
    for (facts, cause) in cases {
        // The failure is the error, so the command exits non-zero, and the error carries the whole report.
        let report = judge(&facts).unwrap_err();
        assert!(report.contains(cause), "{report}");
        assert_eq!(
            report.lines().filter(|l| l.starts_with("FAIL ")).count(),
            1,
            "{report}"
        );
        assert!(
            report.starts_with("base-merge-check: reviewed head r"),
            "{report}"
        );
        assert!(report.ends_with("result: FAIL (the merge needs delta rounds from both reviewers)"));
    }
}

/// The report names the size and the sha256 of the diff (here the empty diff, whose sha256 is the published value for an
/// empty input, FIPS 180-4 test vectors).
#[test]
fn the_report_names_the_size_and_the_sha256_of_the_diff() {
    let mut f = passing();
    f.reviewed_diff = Vec::new();
    f.new_diff = Vec::new();
    let report = judge(&f).unwrap();
    assert!(
        report.contains(
            "byte-identical: 0 bytes, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"
        ),
        "{report}"
    );
}

#[test]
fn a_failed_merge_tree_fails_the_judgement() {
    let mut f = passing();
    f.merge_tree = (Some(128), "fatal: bad object".into());
    let error = judge(&f).unwrap_err();
    assert!(
        error.starts_with("git merge-tree failed (exit code Some(128))"),
        "{error}"
    );
}

#[test]
fn only_exit_code_0_is_a_git_success() {
    assert_eq!(succeeded(&["diff", "a", "b"], Some(0)), Ok(()));
    assert_eq!(
        succeeded(&["diff", "a", "b"], Some(128)),
        Err("git diff a b failed (exit code Some(128))".to_string())
    );
    assert!(succeeded(&["diff"], Some(1)).is_err());
    assert!(succeeded(&["diff"], None).is_err());
}

#[test]
fn only_exit_code_0_proves_an_ancestor_and_1_disproves_it() {
    assert_eq!(ancestry(Some(0)), Ok(true));
    assert_eq!(ancestry(Some(1)), Ok(false));
    assert!(ancestry(Some(128)).is_err());
    assert!(ancestry(None).is_err());
}

/// A real repository for `check`: git runs without the user's configuration.
struct Repo(tempfile::TempDir);

impl Repo {
    /// The output of `git <args>` in the repository, which must succeed.
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.0.path())
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
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.path().join(path), text).unwrap();
    }

    /// Commits the staged changes; returns the commit.
    fn commit(&self, message: &str) -> String {
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Writes and stages `path`.
    fn stage(&self, path: &str, text: &str) {
        self.write(path, text);
        self.git(&["add", path]);
    }

    /// Points the gitlink `sub` at `commit`.
    fn gitlink(&self, commit: &str) {
        self.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},sub"),
        ]);
    }
}

/// A base `v1`; a pull request `pr` that changes `a.txt` (the reviewed head); `v1` then changes `v.txt`; the new head merges
/// `v1` into `pr`. The base also holds a gitlink `sub` and a textconv attribute for `f.txt`.
fn base_only_merge() -> (Repo, String, String) {
    let repo = Repo(tempfile::tempdir().unwrap());
    repo.git(&["init", "-q", "-b", "v1"]);
    repo.stage("a.txt", "a\n");
    repo.stage("v.txt", "v\n");
    repo.stage("f.txt", "f\n");
    repo.stage(".gitattributes", "f.txt diff=hide\n");
    repo.gitlink(&"1".repeat(40));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "pr"]);
    repo.stage("a.txt", "a2\n");
    let reviewed = repo.commit("the reviewed change");
    repo.git(&["checkout", "-q", "v1"]);
    repo.stage("v.txt", "v2\n");
    repo.commit("v1 moves");
    repo.git(&["checkout", "-q", "pr"]);
    repo.git(&["merge", "-q", "--no-edit", "v1"]);
    let new = repo.git(&["rev-parse", "HEAD"]);
    (repo, reviewed, new)
}

#[test]
fn a_real_base_only_merge_passes() {
    let (repo, reviewed, new) = base_only_merge();
    let report = check(repo.0.path(), &reviewed, &new, "v1").unwrap();
    assert!(report.ends_with("result: PASS (no reviewer delta round is needed; the gate still runs on the new head)\n"), "{report}");
    assert!(report.contains("(2) none of the 1 paths that changed on the base is in the pull request's own diff (1 paths)"), "{report}");
}

/// #170 review (HIGH): a repository setting that filters `git diff` must not hide an unreviewed change after the merge.
/// The case first shows that its setting hides the change from a plain `git diff`; `check` must still fail on condition 3.
fn a_change_hidden_by(key: &str, value: &str, change: fn(&Repo)) {
    let (repo, reviewed, merged) = base_only_merge();
    change(&repo);
    let new = repo.commit("an unreviewed change");
    repo.git(&["config", key, value]);
    let shown = repo.git(&["diff", &merged, &new]);
    assert!(
        !shown.contains("+f2") && !shown.contains("2222"),
        "{key} hides nothing: {shown}"
    );
    let report = check(repo.0.path(), &reviewed, &new, "v1")
        .unwrap_err()
        .to_string();
    assert!(
        report.contains("FAIL (3) the pull request's own diff differs"),
        "{key}: {report}"
    );
}

#[test]
fn an_unreviewed_change_fails_under_an_external_diff() {
    a_change_hidden_by("diff.external", "/usr/bin/true", |repo| {
        repo.stage("f.txt", "f2\n")
    });
}

#[test]
fn an_unreviewed_change_fails_under_a_textconv_driver() {
    a_change_hidden_by("diff.hide.textconv", "/usr/bin/true", |repo| {
        repo.stage("f.txt", "f2\n")
    });
}

#[test]
fn an_unreviewed_gitlink_change_fails_when_submodules_are_ignored() {
    a_change_hidden_by("diff.ignoreSubmodules", "all", |repo| {
        repo.gitlink(&"2".repeat(40))
    });
}

#[test]
fn a_git_failure_fails_the_check() {
    let (repo, reviewed, _) = base_only_merge();
    let error = check(repo.0.path(), &reviewed, "no-such-commit", "v1")
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("git rev-parse --verify no-such-commit^{commit} failed"),
        "{error}"
    );
}

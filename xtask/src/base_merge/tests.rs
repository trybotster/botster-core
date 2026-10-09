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
        set_texts: Vec::new(),
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
        report.contains("outside the line-set files is byte-identical: 34 bytes, sha256 "),
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
    cases.push((f, "FAIL (2) none of the 2 paths that changed on the base is in the pull request's own diff (1 paths; a line-set file has (4)): xtask/src/main.rs"));
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
            "outside the line-set files is byte-identical: 0 bytes, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"
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
        let output = botster_test_process::run_to_completion(
            Command::new("git")
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
                .env("GIT_CONFIG_GLOBAL", "/dev/null"),
            botster_test_process::Deadline::cleanup(),
        )
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
    assert!(report.contains("(2) none of the 1 paths that changed on the base is in the pull request's own diff (1 paths; a line-set file has (4))"), "{report}");
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

const PENDING: &str = "conformance/core-pending.txt";
const OLD: &str = "# comment\nconf::a\nconf::b\nconf::c\nconf::d\n";

/// Disjoint removals pass, and the merged file is the same in either order of the two sides.
#[test]
fn disjoint_removals_of_id_lines_merge_in_either_order() {
    let (pr, base, merged) = (
        "# comment\nconf::a\nconf::c\nconf::d\n",
        "# comment\nconf::a\nconf::b\nconf::c\n",
        "# comment\nconf::a\nconf::c\n",
    );
    assert_eq!(set_merge(OLD, pr, base, merged), Ok((1, 1)));
    assert_eq!(set_merge(OLD, base, pr, merged), Ok((1, 1)));
    assert_eq!(
        set_merge(OLD, OLD, OLD, OLD),
        Ok((0, 0)),
        "no change on either side"
    );
    assert_eq!(
        set_merge(OLD, pr, OLD, pr),
        Ok((1, 0)),
        "a base that leaves the file alone"
    );
}

#[test]
fn only_a_pure_disjoint_removal_with_the_right_merge_passes() {
    let fail =
        |reviewed: &str, base: &str, new: &str| set_merge(OLD, reviewed, base, new).unwrap_err();
    let added = "# comment\nconf::a\nconf::b\nconf::c\nconf::d\nconf::e\n";
    assert!(fail(added, OLD, added).contains(
        "the pull request is not a pure removal of id lines: it adds or edits the line \"conf::e\""
    ));
    let edited = "# comment\nconf::a\nconf::b2\nconf::c\nconf::d\n";
    assert!(fail(OLD, edited, edited).contains("the base is not a pure removal"));
    let comment = "# a comment\nconf::a\nconf::b\nconf::c\nconf::d\n";
    assert!(fail(comment, OLD, comment).contains("it removes or edits the line \"# comment\""));
    let no_comment = "conf::a\nconf::b\nconf::c\nconf::d\n";
    assert!(
        fail(no_comment, OLD, no_comment).contains("\"# comment\""),
        "a comment line is not an id line"
    );
    let moved = "# comment\nconf::b\nconf::a\nconf::c\nconf::d\n";
    assert!(
        fail(moved, OLD, moved).contains("not a pure removal"),
        "a reorder is an edit"
    );
    let without_b = "# comment\nconf::a\nconf::c\nconf::d\n";
    assert_eq!(
        fail(without_b, without_b, without_b),
        "both sides remove conf::b"
    );
    let without_d = "# comment\nconf::a\nconf::b\nconf::c\n";
    for wrong in [
        without_b,
        without_d,
        "# comment\nconf::c\nconf::a\n",
        "# comment\nconf::a\nconf::c",
    ] {
        assert_eq!(
            fail(without_b, without_d, wrong),
            "the new head's file is not the old file without both removed sets",
            "{wrong:?}"
        );
    }
}

#[test]
fn an_old_file_that_repeats_an_id_line_fails() {
    let twice = "conf::a\nconf::b\nconf::a\n";
    assert_eq!(
        set_merge(twice, "conf::b\nconf::a\n", twice, "conf::b\nconf::a\n"),
        Err("the old file lists conf::a twice".into())
    );
    // A repeated non-id line is not ambiguous for a removal of id lines.
    assert_eq!(
        set_merge("\n\nconf::a\n", "\n\n", "\n\nconf::a\n", "\n\n"),
        Ok((1, 0))
    );
}

#[test]
fn an_id_line_is_conf_and_a_lowercase_name() {
    for line in ["conf::a", "conf::ev_4_exit_has_signal", "conf::a2_8"] {
        assert!(is_id_line(line), "{line}");
    }
    for line in [
        "",
        "conf::",
        "conf::A",
        "conf::a b",
        " conf::a",
        "conf::a ",
        "conf::a-b",
        "# conf::a",
        "core::a",
    ] {
        assert!(!is_id_line(line), "{line:?}");
    }
}

#[test]
fn the_line_of_a_set_file_names_its_outcome() {
    let texts = |old: Option<&str>, new: Option<&str>| SetTexts {
        path: PENDING.into(),
        old: old.map(blob),
        reviewed: old.map(blob),
        base: old.map(blob),
        new: new.map(blob),
    };
    assert_eq!(
        set_line(&texts(None, None)),
        (
            true,
            format!("(4) the line-set file {PENDING}: it exists at none of the four commits")
        )
    );
    assert_eq!(
        set_line(&texts(Some(OLD), None)),
        (
            false,
            format!("(4) the line-set file {PENDING}: it is missing at one of the four commits")
        )
    );
    assert!(!set_line(&texts(None, Some(OLD))).0);
    let (ok, text) = set_line(&texts(Some(OLD), Some(OLD)));
    assert!(ok, "{text}");
    let mut f = passing();
    f.set_texts = vec![texts(Some(OLD), Some("conf::x\n"))];
    let report = judge(&f).unwrap_err();
    assert!(
        report.contains(&format!(
            "FAIL (4) the line-set file {PENDING}: the pull request leaves it alone, but the new head does not hold the base's file"
        )),
        "{report}"
    );
}

fn blob(text: &str) -> Blob {
    Blob {
        mode: REGULAR.into(),
        text: text.into(),
    }
}

/// #190 review F60: an entry that is not a regular `100644` file fails at any of the four commits, also when its text
/// passes.
#[test]
fn a_set_file_that_is_not_a_regular_file_at_any_commit_fails() {
    let without_b = "# comment\nconf::a\nconf::c\nconf::d\n";
    let without_d = "# comment\nconf::a\nconf::b\nconf::c\n";
    let merged = "# comment\nconf::a\nconf::c\n";
    let good = [blob(OLD), blob(without_b), blob(without_d), blob(merged)];
    assert!(judge_set(&good[0], &good[1], &good[2], &good[3]).is_ok());
    let names = [
        "the reviewed merge base",
        "the reviewed head",
        "the new merge base",
        "the new head",
    ];
    for mode in ["100755 blob", "120000 blob", "160000 commit", "040000 tree"] {
        for at in 0..4 {
            let mut entries = good.clone();
            entries[at].mode = mode.into();
            assert_eq!(
                judge_set(&entries[0], &entries[1], &entries[2], &entries[3]),
                Err(format!(
                    "at {} it is `{mode}`, not a regular file `100644 blob`",
                    names[at]
                )),
                "{mode} at {at}"
            );
        }
    }
}

/// #190 review R1: a side that leaves the file alone keeps the byte rule, so the other side may make any change, and only
/// a change on both sides needs the set rule.
#[test]
fn a_one_sided_change_needs_only_the_other_sides_file() {
    let comment = "# a new comment\nconf::a\nconf::b\nconf::c\nconf::d\n";
    let readded = "# comment\nconf::a\nconf::b\nconf::c\nconf::d\nconf::e\n";
    let (old, edited, added) = (blob(OLD), blob(comment), blob(readded));
    assert_eq!(
        judge_set(&old, &old, &edited, &edited),
        Ok("the pull request leaves it alone; the new head holds the base's file".into())
    );
    assert_eq!(
        judge_set(&old, &added, &old, &added),
        Ok("the base leaves it alone; the new head holds the reviewed file".into())
    );
    assert!(judge_set(&old, &old, &edited, &old).is_err());
    assert!(judge_set(&old, &added, &old, &old).is_err());
    assert_eq!(
        judge_set(&old, &added, &edited, &added),
        Err("the pull request is not a pure removal of id lines: it adds or edits the line \"conf::e\"".into()),
        "a change on both sides needs the set rule"
    );
}

/// Two flips: the pull request and the base each remove another id from the pending file. Only the line-set rule lets the
/// merge pass: with an empty `SET_FILES` the shared path fails condition 2 (red on revert).
fn two_flips(pr_removes: &str, base_removes: &str) -> (Repo, String, String) {
    let repo = Repo(tempfile::tempdir().unwrap());
    let without = |id: &str| OLD.replace(&format!("{id}\n"), "");
    repo.git(&["init", "-q", "-b", "v1"]);
    std::fs::create_dir(repo.0.path().join("conformance")).unwrap();
    repo.stage(PENDING, OLD);
    repo.stage("a.txt", "a\n");
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "pr"]);
    repo.stage(PENDING, &without(pr_removes));
    repo.stage("a.txt", "a2\n");
    let reviewed = repo.commit("the reviewed flip");
    repo.git(&["checkout", "-q", "v1"]);
    repo.stage(PENDING, &without(base_removes));
    repo.commit("another flip on v1");
    repo.git(&["checkout", "-q", "pr"]);
    repo.git(&["merge", "-q", "--no-edit", "v1"]);
    let new = repo.git(&["rev-parse", "HEAD"]);
    (repo, reviewed, new)
}

#[test]
fn a_real_merge_of_two_disjoint_flips_passes() {
    let (repo, reviewed, new) = two_flips("conf::b", "conf::d");
    let report = check(repo.0.path(), &reviewed, &new, "v1").unwrap();
    assert!(report.contains("PASS (2) none of the 1 paths that changed on the base is in the pull request's own diff (2 paths; a line-set file has (4))\n"), "{report}");
    assert!(report.contains(&format!("PASS (4) the line-set file {PENDING}: the pull request removes 1 id lines and the base removes 1 others")), "{report}");
    assert!(report.ends_with("result: PASS (no reviewer delta round is needed; the gate still runs on the new head)\n"), "{report}");
}

/// The set file is left out of condition 3 only by its own rule: an unreviewed change to it after the merge fails (4), and
/// an unreviewed change to another path still fails (3), also with `GIT_LITERAL_PATHSPECS` set.
#[test]
fn an_unreviewed_change_after_the_merge_still_fails() {
    let (repo, reviewed, _) = two_flips("conf::b", "conf::d");
    repo.stage(PENDING, "# comment\nconf::a\n");
    let new = repo.commit("an unreviewed removal");
    let report = check(repo.0.path(), &reviewed, &new, "v1")
        .unwrap_err()
        .to_string();
    assert!(report.contains("FAIL (4)"), "{report}");
    let (repo, reviewed, _) = two_flips("conf::b", "conf::d");
    repo.stage("a.txt", "a3\n");
    let new = repo.commit("an unreviewed change");
    // The gate's nextest runs each test in its own process, so the variable reaches only this test's git (as in fsutil).
    std::env::set_var("GIT_LITERAL_PATHSPECS", "1");
    let report = check(repo.0.path(), &reviewed, &new, "v1")
        .unwrap_err()
        .to_string();
    std::env::remove_var("GIT_LITERAL_PATHSPECS");
    assert!(
        report.contains("FAIL (3) the pull request's own diff differs"),
        "{report}"
    );
}

#[test]
fn a_real_merge_of_two_flips_of_one_id_fails() {
    let (repo, reviewed, new) = two_flips("conf::b", "conf::b");
    let report = check(repo.0.path(), &reviewed, &new, "v1")
        .unwrap_err()
        .to_string();
    assert!(
        report.contains(&format!(
            "FAIL (4) the line-set file {PENDING}: both sides remove conf::b"
        )),
        "{report}"
    );
}

/// #190 review F60: an unreviewed mode change of the pending file after a passing merge fails condition 4.
#[test]
fn a_real_mode_change_of_the_set_file_after_the_merge_fails() {
    let (repo, reviewed, _) = two_flips("conf::b", "conf::d");
    repo.git(&["update-index", "--chmod=+x", PENDING]);
    let new = repo.commit("an unreviewed mode change");
    let report = check(repo.0.path(), &reviewed, &new, "v1")
        .unwrap_err()
        .to_string();
    assert!(
        report.contains(&format!(
            "FAIL (4) the line-set file {PENDING}: at the new head it is `100755 blob`"
        )),
        "{report}"
    );
}

/// #190 review R1: a pull request that leaves the pending file alone passes when v1 edits a comment in it (as #188 did).
#[test]
fn a_real_merge_where_only_the_base_edits_the_set_file_passes() {
    let repo = Repo(tempfile::tempdir().unwrap());
    repo.git(&["init", "-q", "-b", "v1"]);
    std::fs::create_dir(repo.0.path().join("conformance")).unwrap();
    repo.stage(PENDING, OLD);
    repo.stage("a.txt", "a\n");
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "pr"]);
    repo.stage("a.txt", "a2\n");
    let reviewed = repo.commit("the reviewed change");
    repo.git(&["checkout", "-q", "v1"]);
    repo.stage(PENDING, &OLD.replace("# comment", "# another comment"));
    repo.commit("v1 edits a comment");
    repo.git(&["checkout", "-q", "pr"]);
    repo.git(&["merge", "-q", "--no-edit", "v1"]);
    let new = repo.git(&["rev-parse", "HEAD"]);
    let report = check(repo.0.path(), &reviewed, &new, "v1").unwrap();
    assert!(
        report.contains(&format!(
            "PASS (4) the line-set file {PENDING}: the pull request leaves it alone"
        )),
        "{report}"
    );
}

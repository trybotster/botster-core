use super::*;

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
    let (report, pass) = judge(&passing()).unwrap();
    assert!(pass, "{report}");
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
        let (report, pass) = judge(&facts).unwrap();
        assert!(!pass, "{report}");
        assert!(report.contains(cause), "{report}");
        assert_eq!(
            report.lines().filter(|l| l.starts_with("FAIL ")).count(),
            1,
            "{report}"
        );
        assert!(
            report.ends_with("result: FAIL (the merge needs delta rounds from both reviewers)\n")
        );
    }
}

/// The report names the size and the sha256 of the diff (here the empty diff, whose sha256 is the published value for an
/// empty input, FIPS 180-4 test vectors).
#[test]
fn the_report_names_the_size_and_the_sha256_of_the_diff() {
    let mut f = passing();
    f.reviewed_diff = Vec::new();
    f.new_diff = Vec::new();
    let (report, pass) = judge(&f).unwrap();
    assert!(pass);
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
    assert!(judge(&f).is_err());
}

#[test]
fn only_exit_code_0_proves_an_ancestor_and_1_disproves_it() {
    assert_eq!(ancestry(Some(0)), Ok(true));
    assert_eq!(ancestry(Some(1)), Ok(false));
    assert!(ancestry(Some(128)).is_err());
    assert!(ancestry(None).is_err());
}

use super::*;

/// A mutant of `function` in `xtask/src/ci.rs`; a name that starts with `replace <function> ` replaces the whole body.
fn mutant(function: &str, name: &str) -> Mutant {
    Mutant {
        file: "xtask/src/ci.rs".into(),
        function: function.into(),
        name: format!("xtask/src/ci.rs:10:5: {name}"),
        whole_body: name.starts_with(&format!("replace {function} ")),
    }
}

/// The mutants of a shell `mutants_job` that forwards to the decisions `mutation_verdict` and `parse_outcomes`, of a
/// printer, and of a decision `mutation_decision`.
fn mutants() -> Vec<Mutant> {
    vec![
        mutant(
            "mutants_job",
            "replace mutants_job -> Result<()> with Ok(())",
        ),
        mutant("mutants_job", "delete ! in mutants_job"),
        mutant(
            "mutation_verdict",
            "replace mutation_verdict -> Result<()> with Ok(())",
        ),
        mutant("mutation_verdict", "replace == with != in mutation_verdict"),
        mutant(
            "parse_outcomes",
            "replace parse_outcomes -> Result<u64> with Ok(0)",
        ),
        mutant("printer", "replace printer with ()"),
        mutant(
            "mutation_decision",
            "replace mutation_decision -> Result<()> with Ok(())",
        ),
    ]
}

fn exclusion(pattern: &str, reason: &str) -> Exclusion {
    Exclusion {
        pattern: pattern.into(),
        reason: reason.into(),
    }
}

const SHELL: &str =
    r"xtask/src/ci\.rs:\d+:\d+: replace mutants_job -> Result<\(\)> with Ok\(\(\)\)$";

/// The model entry of #167: a shell whose reason cites its decisions with their proofs (plan section 8: `decision
/// (proof, ..)`).
#[test]
fn a_shell_entry_that_names_the_tested_decision_it_calls_passes() {
    let entry = exclusion(
        SHELL,
        "process glue; its decisions mutation_verdict (verdicts) and parse_outcomes (outcomes) stay tested",
    );
    assert!(check(&mutants(), &[entry], &[]).unwrap().is_empty());
}

/// The red-on-revert proof: an exclusion of a decision mutant fails, whatever its reason says.
#[test]
fn an_exclusion_of_a_decision_function_fails() {
    let decision = r"xtask/src/ci\.rs:\d+:\d+: replace == with != in mutation_verdict$";
    let found = check(
        &mutants(),
        &[exclusion(decision, "equivalent; parse_outcomes (outcomes)")],
        &[],
    )
    .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with(&format!(
            "`{decision}` excludes mutation_verdict in xtask/src/ci.rs"
        )),
        "{found:?}"
    );
    assert!(found[0].contains("it is a decision mutant"), "{found:?}");
}

/// Plan section 8 (r23d, 23i): a whole-body exclusion fails unless its reason cites, as `decision (proof, ..)`, the name
/// of an xtask function that has mutants, none of which an exclusion covers.
#[test]
fn a_shell_entry_fails_unless_its_cited_decision_has_mutants_and_none_is_excluded() {
    let reasons = [
        "process glue",                                         // cites no decision
        "process glue; mutants_job (verdicts)", // cites itself: this entry covers it
        "process glue; other (verdicts)",       // no xtask function has the name
        "process glue; mutation_verdict (verdicts)", // has mutants, but one is excluded below
        "process glue; mutation_verdict decides, see verdicts", // plan r23d: a decision in free text is no citation
        "process glue; mutation_verdict (see verdicts)", // prose in parentheses is no citation
    ];
    for reason in reasons {
        let mut entries = vec![exclusion(SHELL, reason)];
        if reason.contains("mutation_verdict (verdicts)") {
            entries.push(exclusion(
                r"replace mutation_verdict -> Result<\(\)> with Ok\(\(\)\)$",
                "parse_outcomes (outcomes)",
            ));
        }
        let found = check(&mutants(), &entries, &[]).unwrap();
        let shell: Vec<&String> = found
            .iter()
            .filter(|v| v.starts_with(&format!("`{SHELL}`")))
            .collect();
        assert_eq!(shell.len(), 1, "{reason}: {found:?}");
        assert!(
            shell[0].contains(
                "its reason cites, as `decision (proof, ..)`, no xtask function with mutants of which no exclusion \
                 covers any"
            ),
            "{reason}: {found:?}"
        );
    }
}

/// #181 B10 (P6 round 8), plan section 8 (23i): the check reads no call relation, so both reviewer fixtures give the
/// result that a shell which calls its decision gives, and review decides that the shell forwards to it:
/// - the shell calls `runner.decide()`, a method with the name of the tested free function `decide`;
/// - a free `shell` that calls nothing beside a method `Runner::shell` that calls `decide`.
///
/// The functions are matched by name, so an exclusion of any function named `decide` (here the method) makes the
/// citation fail: a shared name only makes the check reject more.
#[test]
fn a_cited_decision_is_matched_by_name_and_a_shared_name_fails_closed() {
    let fixture = [
        mutant("decide", "replace decide -> Result<()> with Ok(())"),
        mutant("decide", "replace == with != in decide"),
        mutant(
            "Runner::decide",
            "replace Runner::decide -> Result<()> with Ok(())",
        ),
        mutant("shell", "replace shell -> Result<()> with Ok(())"),
        mutant(
            "Runner::shell",
            "replace Runner::shell -> Result<()> with Ok(())",
        ),
    ];
    let shell = exclusion(
        r"xtask/src/ci\.rs:\d+:\d+: replace shell -> Result<\(\)> with Ok\(\(\)\)$",
        "glue; decide (verdicts)",
    );
    assert!(check(&fixture, std::slice::from_ref(&shell), &[])
        .unwrap()
        .is_empty());
    let method = exclusion(
        r"replace Runner::decide -> Result<\(\)> with Ok\(\(\)\)$",
        "glue; shell (verdicts)",
    );
    let found = check(&fixture, &[shell, method], &[]).unwrap();
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().all(|v| v.contains("its reason cites")),
        "{found:?}"
    );
    assert!(
        found[0].contains("excludes Runner::decide") && found[1].contains("excludes shell"),
        "{found:?}"
    );
}

/// Plan section 8 (23g, 23i): a decision mutant is never excluded, whatever its reason cites. A whole-body exclusion
/// with a strict citation passes, also of a function that only forwards to a decision (`mutation_decision`): review of
/// every change to `.cargo/mutants.toml` (HIGH) decides that the function is an I/O shell.
#[test]
fn a_decision_mutant_is_never_excluded_and_a_cited_whole_body_exclusion_passes() {
    let found = check(
        &mutants(),
        &[exclusion(
            r"delete ! in mutants_job$",
            "glue; mutation_verdict (verdicts)",
        )],
        &[],
    )
    .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("`delete ! in mutants_job$` excludes mutants_job in xtask/src/ci.rs"),
        "{found:?}"
    );
    assert!(found[0].contains("it is a decision mutant"), "{found:?}");
    let forwarding = exclusion(
        r"replace mutation_decision -> Result<\(\)> with Ok\(\(\)\)$",
        "glue; mutation_verdict (verdicts)",
    );
    assert!(check(&mutants(), &[forwarding], &[]).unwrap().is_empty());
}

#[test]
fn a_glob_or_a_reasonless_regex_that_covers_the_xtask_fails_and_the_test_globs_pass() {
    let tests = vec!["**/tests/**".to_string(), "tests/**".to_string()];
    assert!(check(&mutants(), &[], &tests).unwrap().is_empty());
    for glob in ["xtask/**", "ci.rs", "xtask/src/*.rs", "**/ci.rs"] {
        let found = check(&mutants(), &[], &[glob.to_string()]).unwrap();
        assert_eq!(found.len(), 7, "{glob}: {found:?}");
    }
    let off_macos = exclusion(r"xtask/src/ci\.rs", "");
    assert_eq!(check(&mutants(), &[off_macos], &[]).unwrap().len(), 7);
}

#[test]
fn an_exclusion_that_is_not_a_regex_fails() {
    assert!(check(&mutants(), &[exclusion("(", "")], &[]).is_err());
}

#[test]
fn each_entry_takes_the_comment_block_above_it_or_above_its_group() {
    let toml = "exclude_globs = []\n\nexclude_re = [\n    # first reason\n    # goes on\n    'a',\n    'b',\n    # second\n    'c',\n]\n# after\n";
    let read: Vec<(String, String)> = exclusions_of(toml)
        .unwrap()
        .into_iter()
        .map(|e| (e.pattern, e.reason))
        .collect();
    assert_eq!(
        read,
        [
            ("a".to_string(), "first reason goes on".to_string()),
            ("b".to_string(), "first reason goes on".to_string()),
            ("c".to_string(), "second".to_string()),
        ]
    );
    assert!(exclusions_of("exclude_re = [\n    \"d\",\n]\n").is_err());
}

#[test]
fn the_mutant_list_is_read_from_the_json_of_cargo_mutants() {
    let json = r#"[{"file":"xtask/src/main.rs","function":{"function_name":"main","return_type":""},
        "name":"xtask/src/main.rs:37:5: replace main with ()","package":"xtask","genre":"FnValue"},
        {"file":"xtask/src/a.rs","name":"xtask/src/a.rs:1:1: replace * with + ","genre":"BinaryOperator"}]"#;
    assert_eq!(
        parse_mutants(json).unwrap(),
        [
            Mutant {
                file: "xtask/src/main.rs".into(),
                function: "main".into(),
                name: "xtask/src/main.rs:37:5: replace main with ()".into(),
                whole_body: true,
            },
            Mutant {
                file: "xtask/src/a.rs".into(),
                function: String::new(),
                name: "xtask/src/a.rs:1:1: replace * with + ".into(),
                whole_body: false,
            },
        ]
    );
    assert!(parse_mutants("{}").is_err());
    assert!(parse_mutants(r#"[{"file":"a"}]"#).is_err());
    assert!(parse_mutants(r#"[{"file":"a","name":"b"}]"#).is_err());
    assert_eq!(mutant("x", "y").short(), "x");
    assert_eq!(
        mutant("<impl Visit<'ast> for Scan<'_>>::visit_item", "y").short(),
        "visit_item"
    );
}

/// The inputs come from the mutants file: the reasoned `exclude_re` entries with the gate's off-macOS exclusions, and the
/// `exclude_globs`. The check reads no source (plan 23i), so a file that does not parse changes nothing.
#[test]
fn the_inputs_are_the_reasoned_entries_the_globs_and_the_xtask_sources() {
    let toml = "exclude_globs = [\"**/tests/**\"]\n\nexclude_re = [\n    # a reason\n    'x',\n]\n";
    let repo =
        crate::fsutil::test_repo(&[(MUTANTS_FILE, toml), ("xtask/src/a.rs", "not Rust {\n")]);
    let read = inputs(repo.path()).unwrap();
    assert_eq!(read.globs, ["**/tests/**"]);
    assert_eq!(
        read.exclusions.len(),
        1 + crate::ci::OFF_MACOS_EXCLUSIONS.len()
    );
    assert_eq!(
        (
            read.exclusions[0].pattern.as_str(),
            read.exclusions[0].reason.as_str()
        ),
        ("x", "a reason")
    );
    let unreasoned = crate::fsutil::test_repo(&[(MUTANTS_FILE, "exclude_re = [\n    \"x\",\n]\n")]);
    assert!(inputs(unreasoned.path()).is_err());
}

use super::*;

const CI: &str = r#"
fn mutation_verdict(code: Option<i32>) -> Result<()> { if code == Some(0) { return Ok(()); } bail!("x") }
fn parse_outcomes(json: &str) -> Result<u64> { Ok(json.len() as u64) }
fn untested_decision(code: i32) -> bool { code == 0 }
fn mutants_job(root: &Path) -> Result<()> {
    let status = run(root)?;
    let _ = untested_decision(1);
    println!("{}", parse_outcomes("x")?);
    mutation_verdict(status.code())
}
fn printer() { println!("{}", other()); }
#[cfg(test)]
mod tests {
    #[test]
    fn verdicts() { assert!(super::mutation_verdict(Some(0)).is_ok()); }
    #[test]
    fn outcomes() { assert_eq!(super::parse_outcomes("ab").unwrap(), 2); }
}
"#;

fn calls() -> Calls {
    Calls::of(&[("xtask/src/ci.rs".to_string(), CI.to_string())]).unwrap()
}

fn mutant(function: &str, name: &str) -> Mutant {
    Mutant {
        file: "xtask/src/ci.rs".into(),
        function: function.into(),
        name: format!("xtask/src/ci.rs:10:5: {name}"),
    }
}

fn mutants() -> Vec<Mutant> {
    vec![
        mutant(
            "mutants_job",
            "replace mutants_job -> Result<()> with Ok(())",
        ),
        mutant(
            "mutation_verdict",
            "replace mutation_verdict -> Result<()> with Ok(())",
        ),
        mutant("mutation_verdict", "replace == with != in mutation_verdict"),
        mutant("printer", "replace printer with ()"),
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

/// The model entry of #167: a shell whose reason names the tested decision functions that it calls.
#[test]
fn a_shell_entry_that_names_the_tested_decision_it_calls_passes() {
    let entry = exclusion(
        SHELL,
        "process glue; its decisions mutation_verdict and parse_outcomes stay tested",
    );
    assert!(check(&mutants(), &[entry], &[], &calls())
        .unwrap()
        .is_empty());
}

/// The red-on-revert proof: an exclusion of the step's verdict fails, whatever its reason says.
#[test]
fn an_exclusion_of_a_decision_function_fails() {
    let decision = r"xtask/src/ci\.rs:\d+:\d+: replace == with != in mutation_verdict$";
    let found = check(
        &mutants(),
        &[exclusion(decision, "equivalent, see parse_outcomes")],
        &[],
        &calls(),
    )
    .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with(&format!(
            "`{decision}` excludes mutation_verdict in xtask/src/ci.rs"
        )),
        "{found:?}"
    );
}

#[test]
fn a_shell_entry_fails_unless_its_named_decision_is_called_tested_and_not_excluded() {
    let reasons = [
        "process glue",                                         // names no decision
        "process glue; mutants_job is glue",                    // names itself
        "process glue; untested_decision decides",              // called, but no test calls it
        "process glue; other decides", // not called by the shell (nor a function here)
        "process glue; mutation_verdict decides, see verdicts", // called and tested, but excluded below
    ];
    for reason in reasons {
        let mut entries = vec![exclusion(SHELL, reason)];
        if reason.contains("mutation_verdict") {
            entries.push(exclusion(
                r"replace == with != in mutation_verdict$",
                "parse_outcomes",
            ));
        }
        let found = check(&mutants(), &entries, &[], &calls()).unwrap();
        assert!(
            found.iter().any(|v| v.starts_with(&format!("`{SHELL}`"))),
            "{reason}: {found:?}"
        );
    }
}

#[test]
fn a_function_that_calls_no_tested_decision_cannot_be_excluded() {
    let found = check(
        &mutants(),
        &[exclusion("replace printer with", "prints; other")],
        &[],
        &calls(),
    )
    .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_glob_or_a_reasonless_regex_that_covers_the_xtask_fails_and_the_test_globs_pass() {
    let tests = vec!["**/tests/**".to_string(), "tests/**".to_string()];
    assert!(check(&mutants(), &[], &tests, &calls()).unwrap().is_empty());
    for glob in ["xtask/**", "ci.rs", "xtask/src/*.rs", "**/ci.rs"] {
        let found = check(&mutants(), &[], &[glob.to_string()], &calls()).unwrap();
        assert_eq!(found.len(), 3, "{glob}: {found:?}");
    }
    let off_macos = exclusion(r"xtask/src/ci\.rs", "");
    assert_eq!(
        check(&mutants(), &[off_macos], &[], &calls())
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn an_exclusion_that_is_not_a_regex_fails() {
    assert!(check(&mutants(), &[exclusion("(", "")], &[], &calls()).is_err());
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
fn the_calls_come_from_the_syntax_with_the_calls_of_tests() {
    let calls = calls();
    let job = calls.calls("xtask/src/ci.rs", "mutants_job").unwrap();
    for callee in [
        "run",
        "untested_decision",
        "parse_outcomes",
        "mutation_verdict",
        "code",
    ] {
        assert!(job.contains(callee), "{callee}: {job:?}");
    }
    assert!(calls.tested.contains("mutation_verdict") && calls.tested.contains("parse_outcomes"));
    assert!(!calls.tested.contains("untested_decision"));
}

#[test]
fn the_mutant_list_is_read_from_the_json_of_cargo_mutants() {
    let json = r#"[{"file":"xtask/src/main.rs","function":{"function_name":"main","return_type":""},
        "name":"xtask/src/main.rs:37:5: replace main with ()","package":"xtask"},
        {"file":"xtask/src/a.rs","name":"xtask/src/a.rs:1:1: replace * with + "}]"#;
    assert_eq!(
        parse_mutants(json).unwrap(),
        [
            Mutant {
                file: "xtask/src/main.rs".into(),
                function: "main".into(),
                name: "xtask/src/main.rs:37:5: replace main with ()".into()
            },
            Mutant {
                file: "xtask/src/a.rs".into(),
                function: String::new(),
                name: "xtask/src/a.rs:1:1: replace * with + ".into()
            },
        ]
    );
    assert!(parse_mutants("{}").is_err());
    assert!(parse_mutants(r#"[{"file":"a"}]"#).is_err());
    assert_eq!(mutant("x", "y").short(), "x");
    assert_eq!(
        mutant("<impl Visit<'ast> for Scan<'_>>::visit_item", "y").short(),
        "visit_item"
    );
}

const INDEX: &str = r#"
fn shell() { decide(); lib_call(); }
fn decide() {}
impl S { fn method() { from_impl(); } }
#[test]
fn top_level() { decide(); lib_call(); }
#[cfg(unix)]
fn unix_only() { by_cfg_unix(); }
#[inline]
fn inlined() { by_inline(); }
#[cfg(test)]
mod t { fn helper() { by_test_module(); } }
"#;

fn index() -> Calls {
    Calls::of(&[("xtask/src/a.rs".to_string(), INDEX.to_string())]).unwrap()
}

/// Test code is a `#[test]` function or the code of a `#[cfg(test)]` module, at any level; another attribute does not make
/// test code. The calls of an impl's methods are indexed under the method.
#[test]
fn test_code_is_a_test_function_or_a_test_module_and_methods_are_indexed() {
    let calls = index();
    for name in ["decide", "lib_call", "by_test_module"] {
        assert!(calls.tested.contains(name), "{name}");
    }
    for name in ["by_cfg_unix", "by_inline", "from_impl"] {
        assert!(!calls.tested.contains(name), "{name}");
    }
    assert!(calls
        .calls("xtask/src/a.rs", "method")
        .unwrap()
        .contains("from_impl"));
}

/// A named decision must be a function of the xtask: a tested library call that the shell makes is not one.
#[test]
fn a_named_decision_must_be_a_function_of_the_xtask() {
    let shell = Mutant {
        file: "xtask/src/a.rs".into(),
        function: "shell".into(),
        name: "xtask/src/a.rs:2:1: replace shell with ()".into(),
    };
    let entry = |reason: &str| exclusion(r"replace shell with \(\)$", reason);
    let named = |reason| {
        check(
            std::slice::from_ref(&shell),
            &[entry(reason)],
            &[],
            &index(),
        )
        .unwrap()
    };
    assert!(named("glue; decide decides").is_empty());
    assert_eq!(named("glue; lib_call decides").len(), 1);
}

/// The inputs come from the repository: the reasoned `exclude_re` entries with the gate's off-macOS exclusions, the
/// `exclude_globs`, and the calls of the tracked Rust sources under `xtask/src/` only.
#[test]
fn the_inputs_are_the_reasoned_entries_the_globs_and_the_xtask_sources() {
    let toml = "exclude_globs = [\"**/tests/**\"]\n\nexclude_re = [\n    # a reason\n    'x',\n]\n";
    let repo = crate::fsutil::test_repo(&[
        (MUTANTS_FILE, toml),
        ("xtask/src/a.rs", "fn a() { b(); }\n"),
        ("xtask/src/notes.md", "not Rust {\n"),
        ("crates/c/src/lib.rs", "fn c() { d(); }\n"),
    ]);
    let inputs = inputs(repo.path()).unwrap();
    assert_eq!(inputs.globs, ["**/tests/**"]);
    assert_eq!(
        inputs.exclusions.len(),
        1 + crate::ci::OFF_MACOS_EXCLUSIONS.len()
    );
    assert_eq!(
        (
            inputs.exclusions[0].pattern.as_str(),
            inputs.exclusions[0].reason.as_str()
        ),
        ("x", "a reason")
    );
    assert!(inputs
        .calls
        .calls("xtask/src/a.rs", "a")
        .unwrap()
        .contains("b"));
    assert!(inputs.calls.calls("crates/c/src/lib.rs", "c").is_none());
}

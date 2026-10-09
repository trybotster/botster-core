use super::*;

const CI: &str = r#"
fn mutation_verdict(code: Option<i32>) -> Result<()> { if code == Some(0) { return Ok(()); } bail!("x") }
fn parse_outcomes(json: &str) -> Result<u64> { Ok(json.len() as u64) }
fn untested_decision(code: i32) -> bool { code == 0 }
fn mutants_job(root: &Path) -> Result<()> {
    let listing = cargo(root).output()?;
    let status = run(root)?;
    let _ = untested_decision(1);
    println!("{}", parse_outcomes("x")?);
    mutation_verdict(status.code())
}
fn printer() { println!("{}", other()); }
fn mutation_decision(listed: usize, run: impl FnOnce() -> Option<i32>) -> Result<()> {
    if listed == 0 { return Ok(()); }
    mutation_verdict(run())
}
fn forwarded(code: Option<i32>) -> Result<()> { mutation_verdict(code) }
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

/// A mutant of `function` in the CI fixture; a name that starts with `replace <function> ` replaces the whole body.
fn mutant(function: &str, name: &str) -> Mutant {
    let short = function.rsplit("::").next().unwrap();
    Mutant {
        file: "xtask/src/ci.rs".into(),
        function: function.into(),
        name: format!("xtask/src/ci.rs:10:5: {name}"),
        whole_body: name.starts_with(&format!("replace {short} ")),
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
        mutant(
            "mutation_decision",
            "replace mutation_decision -> Result<()> with Ok(())",
        ),
        mutant("forwarded", "replace forwarded -> Result<()> with Ok(())"),
        mutant("mutants_job", "delete ! in mutants_job"),
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
        assert_eq!(found.len(), 7, "{glob}: {found:?}");
    }
    let off_macos = exclusion(r"xtask/src/ci\.rs", "");
    assert_eq!(
        check(&mutants(), &[off_macos], &[], &calls())
            .unwrap()
            .len(),
        7
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

const INDEX: &str = r#"
fn shell() { decide(); lib_call(); std::fs::write(p, b); }
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
        whole_body: true,
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

/// #181 B5: a gate decision is never excluded, whatever its reason names: the whole body of `mutation_decision` (a decision
/// that calls the tested decision `mutation_verdict`), of a decision that only forwards to another, and a decision mutant
/// inside an I/O shell.
#[test]
fn a_decision_that_calls_a_tested_decision_or_a_decision_mutant_of_a_shell_is_never_excluded() {
    let cases = [
        (
            r"replace mutation_decision -> Result<\(\)> with Ok\(\(\)\)$",
            "mutation_decision",
            "the function does no process, file or signal I/O itself",
        ),
        (
            r"replace forwarded -> Result<\(\)> with Ok\(\(\)\)$",
            "forwarded",
            "the function does no process, file or signal I/O itself",
        ),
        (
            r"delete ! in mutants_job$",
            "mutants_job",
            "it is a decision mutant",
        ),
    ];
    for (pattern, function, problem) in cases {
        let found = check(
            &mutants(),
            &[exclusion(
                pattern,
                "glue; mutation_verdict decides, see verdicts",
            )],
            &[],
            &calls(),
        )
        .unwrap();
        assert_eq!(found.len(), 1, "{pattern}: {found:?}");
        assert!(
            found[0].starts_with(&format!(
                "`{pattern}` excludes {function} in xtask/src/ci.rs"
            )),
            "{found:?}"
        );
        assert!(found[0].contains(problem), "{found:?}");
    }
}

/// A function does I/O itself when it starts a process, calls a file or signal function, or runs an xtask command by its
/// module path, a call through `env::` or `fs::`, or a probe of a path; a field named `status`, a call of a closure, a
/// call through another module and an unqualified `command` are not I/O.
#[test]
fn a_function_does_io_when_it_starts_a_process_touches_a_file_or_signals() {
    let mut text = String::from(
        "fn by_status(c: &mut Command) { c.status(); }\n\
         fn by_output(c: &mut Command) { c.output(); }\n\
         fn by_spawn(c: &mut Command) { c.spawn(); }\n\
         fn by_command(root: &Path) { taint::command(root, &[]); }\n\
         fn local_command(root: &Path) { command(root); }\n\
         fn by_field(o: Output) -> bool { o.status.success() }\n\
         fn by_closure(run: impl FnOnce()) { run(); }\n\
         fn by_env() { std::env::args(); }\n\
         fn by_fs_module() { fs::metadata(p); }\n\
         fn by_other_module() { json::metadata(p); }\n\
         fn by_is_file(p: &Path) { p.is_file(); }\n\
         fn by_is_dir(p: &Path) { p.is_dir(); }\n\
         fn by_exists(p: &Path) { p.exists(); }\n",
    );
    for name in IO_CALLS {
        text.push_str(&format!("fn by_{name}() {{ x::{name}(a); }}\n"));
    }
    let calls = Calls::of(&[("xtask/src/a.rs".to_string(), text)]).unwrap();
    let io: BTreeSet<&str> = calls.io.iter().map(|(_, f)| f.as_str()).collect();
    let mut expected: BTreeSet<String> = [
        "by_status",
        "by_output",
        "by_spawn",
        "by_command",
        "by_env",
        "by_fs_module",
        "by_is_file",
        "by_is_dir",
        "by_exists",
    ]
    .map(String::from)
    .into();
    expected.extend(IO_CALLS.map(|name| format!("by_{name}")));
    assert_eq!(io, expected.iter().map(String::as_str).collect());
    assert!(calls.io.iter().all(|(file, _)| file == "xtask/src/a.rs"));
}

/// A function also does I/O when a path call names an xtask function that does I/O: `m::f` names the `f` of
/// `xtask/src/m.rs` (or `m/mod.rs`), `Self::f` the `f` of its own file, and a plain `f` the `f` of its own file first, else
/// each `f` of the xtask. A call of a parameter or another local binding names no function.
#[test]
fn io_reaches_a_function_through_the_xtask_functions_it_calls() {
    let files = [
        (
            "xtask/src/tools.rs",
            "pub fn run(mut c: Command) { c.status(); }\npub fn pure() {}\n",
        ),
        (
            "xtask/src/fsutil/mod.rs",
            "pub fn base() { git().output(); }\n",
        ),
        (
            "xtask/src/a.rs",
            "fn by_module() { tools::run(c); }\n\
             fn by_mod_rs() { crate::fsutil::base(); }\n\
             fn by_import() { run(c); }\n\
             fn by_chain() { by_import(); }\n\
             fn by_self() { Self::local_io(); }\n\
             fn local_io() { std::fs::write(p, b); }\n\
             fn by_parameter(run: impl FnOnce()) { run(); }\n\
             fn by_pattern(chosen: Chosen) { if let Chosen::Run(run) = chosen { run(); } }\n\
             fn by_pure_module() { tools::pure(); }\n\
             fn by_unknown_module() { serde_json::run(x); }\n\
             fn by_crate() { crate::pure(); }\n\
             fn by_crate_io() { crate::run(c); }\n\
             fn by_super_io() { super::local_io(); }\n",
        ),
        (
            "xtask/src/b.rs",
            "fn run() {}\nfn by_own_file() { run(); }\nfn by_late_chain() { by_late_link(); }\n\
             fn by_late_link() { tools::run(c); }\n",
        ),
    ]
    .map(|(file, text)| (file.to_string(), text.to_string()));
    let calls = Calls::of(&files).unwrap();
    let io: BTreeSet<(&str, &str)> = calls
        .io
        .iter()
        .map(|(file, function)| (file.as_str(), function.as_str()))
        .collect();
    let expected: BTreeSet<(&str, &str)> = [
        ("xtask/src/tools.rs", "run"),
        ("xtask/src/fsutil/mod.rs", "base"),
        ("xtask/src/a.rs", "by_module"),
        ("xtask/src/a.rs", "by_mod_rs"),
        ("xtask/src/a.rs", "by_import"),
        ("xtask/src/a.rs", "by_chain"),
        ("xtask/src/a.rs", "by_crate_io"),
        ("xtask/src/a.rs", "by_super_io"),
        ("xtask/src/b.rs", "by_late_chain"),
        ("xtask/src/b.rs", "by_late_link"),
        ("xtask/src/a.rs", "by_self"),
        ("xtask/src/a.rs", "local_io"),
    ]
    .into();
    assert_eq!(io, expected);
}

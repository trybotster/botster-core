use super::*;

const CI: &str = r#"
fn mutation_verdict(code: Option<i32>) -> Result<()> { if code == Some(0) { return Ok(()); } bail!("x") }
fn parse_outcomes(json: &str) -> Result<u64> { Ok(json.len() as u64) }
fn untested_decision(code: i32) -> bool { code == 0 }
fn mutants_job(root: &Path) -> Result<()> {
    let listing = std::process::Command::new("git").output()?;
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
fn forwarded(code: Option<i32>) -> Result<()> { let _ = std::process::Command::new("unused"); mutation_verdict(code) }
fn forwarded_write(write: fn() -> Option<i32>) -> Result<()> { mutation_verdict(write()) }
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
        mutant(
            "forwarded_write",
            "replace forwarded_write -> Result<()> with Ok(())",
        ),
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

/// The model entry of #167: a shell whose reason cites the tested decision functions that it calls, with their proofs
/// (plan section 8: `decision (proof, ..)`).
#[test]
fn a_shell_entry_that_names_the_tested_decision_it_calls_passes() {
    let entry = exclusion(
        SHELL,
        "process glue; its decisions mutation_verdict (verdicts) and parse_outcomes (outcomes) stay tested",
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
        "process glue",                                         // cites no decision
        "process glue; mutants_job (verdicts)",                 // cites itself
        "process glue; untested_decision (verdicts)",           // called, but no test calls it
        "process glue; other (verdicts)", // not called by the shell (nor a function here)
        "process glue; mutation_verdict (verdicts)", // called and tested, but excluded below
        "process glue; mutation_verdict decides, see verdicts", // plan r23d: a decision in free text is no citation
        "process glue; mutation_verdict (see verdicts)", // prose in parentheses is no citation
    ];
    for reason in reasons {
        let mut entries = vec![exclusion(SHELL, reason)];
        if reason.contains("mutation_verdict (verdicts)") {
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
        assert_eq!(found.len(), 8, "{glob}: {found:?}");
    }
    let off_macos = exclusion(r"xtask/src/ci\.rs", "");
    assert_eq!(
        check(&mutants(), &[off_macos], &[], &calls())
            .unwrap()
            .len(),
        8
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
    assert!(named("glue; decide (top_level)").is_empty());
    assert_eq!(named("glue; lib_call (top_level)").len(), 1);
    assert_eq!(named("glue; decide decides, see top_level").len(), 1);
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
/// that calls the tested decision `mutation_verdict`), of a decision that only forwards to another (round 3: behind a
/// `Command::new` builder that starts nothing), and a decision mutant inside an I/O shell.
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
            r"replace forwarded_write -> Result<\(\)> with Ok\(\(\)\)$",
            "forwarded_write",
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
            &[exclusion(pattern, "glue; mutation_verdict (verdicts)")],
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

/// #181 B5 rounds 2 and 3, plan section 8: a function does I/O only through a listed operation, resolved through the
/// `use` declarations in its scope (a block's included): a free function of `std::fs` or `std::env`, a signal,
/// `run_to_completion`, or a `status`, `output` or `spawn` call on a process command. A command is a method chain that
/// begins at `Command::new(..)` or at a function declared to return `Command`, a parameter typed `Command` (also by
/// reference), or a `let` bound to such a chain. `Command::new` alone is a builder and starts nothing, and so is a type's
/// function of `std::fs` (`OpenOptions::new`). A name alone is not I/O: a parameter or a local named `write` or
/// `read_to_string`, a method `status` on another type, a module `fs` of another crate, another function of the signal
/// module.
#[test]
fn a_function_does_io_when_a_call_resolves_to_an_io_function() {
    let text = "\
use std::process::Command;
use std::fs;
use botster_core_sys::signal::{signal_group, Signal};
use other::fs as other_fs;
fn by_fs_module() { fs::metadata(p); }
fn by_env() { std::env::args(); }
fn by_signal() { signal_group(g, Signal::KILL); }
fn by_other_signals() { botster_core_sys::signal::signal_process(p, s); }
fn by_own_group() { botster_core_sys::signal::signal_own_group(s); }
fn by_block_use() { use std::fs::write as put; put(p, b); }
fn by_run() { botster_test_process::run_to_completion(&mut c, d); }
fn by_chain() { Command::new(\"git\").arg(\"x\").status(); }
fn by_full_path_chain() { std::process::Command::new(\"git\").output(); }
fn by_typed_parameter(c: &mut Command) { c.spawn(); }
fn by_owned_parameter(mut c: Command) { c.status(); }
fn by_let() { let mut c = Command::new(\"git\"); c.arg(\"x\"); c.output(); }
fn by_command_new() { Command::new(\"git\"); }
fn by_full_path() { let _ = std::process::Command::new(\"unused\"); }
fn by_builder() { std::fs::OpenOptions::new(); }
fn by_type_function() { fs::File::open(p); }
fn by_method(c: &mut Other, p: &Path) { c.status(); c.output(); c.spawn(); p.exists(); }
fn by_let_of_another_type() { let c = Other::new(); c.status(); }
fn by_field(s: &mut S) { s.command.status(); }
fn by_parameter(write: fn() -> Option<i32>) { write(); }
fn by_local() { let read_to_string = f; read_to_string(); }
fn by_other_fs() { other_fs::write(p); }
fn by_longer_path() { serde_json::value::to_value(x); }
fn by_module_alone() { std::fs(); }
fn by_signal_target() { botster_core_sys::signal::target(1, 2); }
fn by_plain_name() { write(p); }
fn by_new_of_another_type() { std::process::Stdio::new(); }
fn by_longer_function() { std::process::Command::new::extra(); }
";
    let calls = Calls::of(&[("xtask/src/a.rs".to_string(), text.to_string())]).unwrap();
    let io: BTreeSet<&str> = calls.io.iter().map(|(_, f)| f.as_str()).collect();
    let expected: BTreeSet<&str> = [
        "by_fs_module",
        "by_env",
        "by_signal",
        "by_other_signals",
        "by_own_group",
        "by_block_use",
        "by_run",
        "by_chain",
        "by_full_path_chain",
        "by_typed_parameter",
        "by_owned_parameter",
        "by_let",
    ]
    .into();
    assert_eq!(io, expected);
}

/// #181 B5 round 3: a process start reaches through an xtask function declared to return `Command` (as
/// `tools::cargo`), in a chain or through a `let`; a function that returns another type is not a command.
#[test]
fn a_start_on_a_command_of_an_xtask_function_is_io() {
    let files = [
        (
            "xtask/src/tools.rs",
            "use std::process::Command;\npub fn cargo(root: &Path) -> Command { Command::new(\"cargo\") }\n\
             pub fn other() -> Other { Other }\n",
        ),
        (
            "xtask/src/a.rs",
            "use crate::tools::cargo;\n\
             fn by_chain(root: &Path) { cargo(root).args([\"x\"]).status(); }\n\
             fn by_let(root: &Path) { let mut cmd = cargo(root); cmd.arg(\"x\"); cmd.output(); }\n\
             fn by_module_path() { crate::tools::cargo(r).spawn(); }\n\
             fn by_builder_only(root: &Path) { let _ = cargo(root); }\n\
             fn by_other_type() { crate::tools::other().status(); }\n",
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
        ("xtask/src/a.rs", "by_chain"),
        ("xtask/src/a.rs", "by_let"),
        ("xtask/src/a.rs", "by_module_path"),
    ]
    .into();
    assert_eq!(io, expected);
}

/// A function also does I/O when a resolved path call names an xtask function that does I/O. Paths resolve by module:
/// `xtask/src/main.rs` is the crate root, `crate::m::f` names the `f` of `xtask/src/m.rs` (or `m/mod.rs`), `super` and
/// `self` are relative to the module of the file, and `m::f` names the `f` of the child `m` first, else of the root
/// module `m`. `Self::f` names the `f` of its own file, and a plain `f` (also through a `use` rename) the `f` of its own
/// file first, else each `f` of the xtask. A call of a parameter or another local binding names no function, and a path
/// of another crate, of the wrong module, or with a `super` above the crate root names none.
#[test]
fn io_reaches_a_function_through_the_xtask_functions_it_calls() {
    let files = [
        (
            "xtask/src/tools.rs",
            "pub fn run(c: &str) { std::process::Command::new(c).status(); }\npub fn pure() {}\n",
        ),
        (
            "xtask/src/fsutil/mod.rs",
            "pub fn base() { std::process::Command::new(\"git\").output(); }\n",
        ),
        (
            "xtask/src/a.rs",
            "use crate::tools::run as go;\n\
             fn by_module() { tools::run(c); }\n\
             fn by_mod_rs() { crate::fsutil::base(); }\n\
             fn by_import() { run(c); }\n\
             fn by_rename() { go(c); }\n\
             fn by_chain() { by_import(); }\n\
             fn by_self() { Self::local_io(); }\n\
             fn local_io() { std::fs::write(p, b); }\n\
             fn by_parameter(run: impl FnOnce()) { run(); }\n\
             fn by_pattern(chosen: Chosen) { if let Chosen::Run(run) = chosen { run(); } }\n\
             fn by_pure_module() { tools::pure(); }\n\
             fn by_unknown_module() { serde_json::run(x); }\n\
             fn by_crate() { crate::pure(); }\n\
             fn by_crate_io() { crate::root_io(); }\n\
             fn by_crate_wrong_module() { crate::local_io(); }\n\
             fn by_super_io() { super::root_io(); }\n\
             fn by_super_wrong_module() { super::local_io(); }\n\
             fn by_super_above_root() { super::super::root_io(); }\n",
        ),
        (
            "xtask/src/main.rs",
            "fn root_io() { std::env::args(); }\nfn pure() {}\n",
        ),
        (
            "xtask/src/fsutil/inner.rs",
            "fn by_super() { super::base(); }\nfn by_self_module() { self::helper(); }\n\
             fn helper() { std::fs::read(p); }\nfn by_child() { deep::leaf(); }\n\
             fn by_root_module() { tools::run(c); }\n",
        ),
        (
            "xtask/src/fsutil/inner/deep.rs",
            "pub fn leaf() { std::env::var(k); }\n",
        ),
        ("xtask/src/deep.rs", "pub fn leaf() {}\n"),
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
        ("xtask/src/a.rs", "by_rename"),
        ("xtask/src/a.rs", "by_chain"),
        ("xtask/src/a.rs", "by_crate_io"),
        ("xtask/src/a.rs", "by_super_io"),
        ("xtask/src/main.rs", "root_io"),
        ("xtask/src/fsutil/inner.rs", "by_super"),
        ("xtask/src/fsutil/inner.rs", "by_self_module"),
        ("xtask/src/fsutil/inner.rs", "helper"),
        ("xtask/src/fsutil/inner.rs", "by_child"),
        ("xtask/src/fsutil/inner.rs", "by_root_module"),
        ("xtask/src/fsutil/inner/deep.rs", "leaf"),
        ("xtask/src/b.rs", "by_late_chain"),
        ("xtask/src/b.rs", "by_late_link"),
        ("xtask/src/a.rs", "by_self"),
        ("xtask/src/a.rs", "local_io"),
    ]
    .into();
    assert_eq!(io, expected);
}

/// Plan section 8: gate-decisions reads the module of a function from its file path, so a `#[path]` module (declared or
/// inline, at any depth) is not a form that it resolves, and the check fails and names the form and the file.
#[test]
fn a_path_module_in_the_xtask_fails_the_index() {
    for (text, at, name) in [
        ("#[path = \"other.rs\"]\nmod m;\n", "1:1", "m"),
        (
            "mod outer {\n    #[path = \"x\"]\n    mod inner {}\n}\n",
            "2:5",
            "inner",
        ),
    ] {
        let error = Calls::of(&[("xtask/src/a.rs".to_string(), text.to_string())])
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!(
                "xtask/src/a.rs:{at}: `#[path]` on the module `{name}` is not a form that gate-decisions resolves \
                 (plan section 8): it reads the module of a function from its file path"
            ),
            "{text}"
        );
    }
}

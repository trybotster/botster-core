use super::*;

const CI: &str = r#"
use anyhow::bail;
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
        assert_eq!(found.len(), 6, "{glob}: {found:?}");
    }
    let off_macos = exclusion(r"xtask/src/ci\.rs", "");
    assert_eq!(
        check(&mutants(), &[off_macos], &[], &calls())
            .unwrap()
            .len(),
        6
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

/// #181 B5, plan section 8 (23g): a decision mutant is never excluded, whatever its reason cites. The check does not
/// decide whether a function does I/O, so a whole-body exclusion with a strict citation passes, also of a function that
/// only forwards to a tested decision (`mutation_decision`): review of every change to `.cargo/mutants.toml` (HIGH)
/// decides that the function is an I/O shell.
#[test]
fn a_decision_mutant_is_never_excluded_and_a_cited_whole_body_exclusion_passes() {
    let found = check(
        &mutants(),
        &[exclusion(
            r"delete ! in mutants_job$",
            "glue; mutation_verdict (verdicts)",
        )],
        &[],
        &calls(),
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
    assert!(check(&mutants(), &[forwarding], &[], &calls())
        .unwrap()
        .is_empty());
}

/// #181 R6-1, plan section 8 (23f): through the whole check (`inputs`), a declaration of a reserved name fails and the
/// check names the form, the file and the position (each reviewer fixture: `mod anyhow` with `pub use syn::parse_quote
/// as bail;`, and an internal glob of `pub use syn::parse_quote as println;`). The same file without them gives inputs.
#[test]
fn a_declaration_of_a_reserved_name_fails_the_inputs() {
    let head = "use anyhow::bail;\nfn decide() -> bool { true }\n";
    let toml =
        "exclude_re = [\n    # glue; decide (verdicts)\n    'replace shell with \\(\\)$',\n]\n";
    let cases: [(&str, &[&str]); 3] = [
        ("fn shell() { decide(); }", &[]),
        (
            "mod anyhow { pub use syn::parse_quote as bail; }\nfn shell() { let _: syn::Expr = anyhow::bail!(decide()); }",
            &[
                "xtask/src/ci.rs:3:5: the module `anyhow` declares a reserved name of gate-decisions",
                "xtask/src/ci.rs:3:14: the `use` introduces the reserved name `bail` of gate-decisions as `syn::parse_quote`",
            ],
        ),
        (
            "mod macros { pub use syn::parse_quote as println; }\nuse self::macros::*;\nfn shell() { let _: syn::Expr = println!(decide()); }",
            &["xtask/src/ci.rs:3:14: the `use` introduces the reserved name `println` of gate-decisions as `syn::parse_quote`"],
        ),
    ];
    for (shell, texts) in cases {
        let source = format!("{head}{shell}\n");
        let repo =
            crate::fsutil::test_repo(&[(MUTANTS_FILE, toml), ("xtask/src/ci.rs", source.as_str())]);
        match inputs(repo.path()) {
            Ok(_) => assert!(texts.is_empty(), "{shell}"),
            Err(error) => {
                let error = error.to_string();
                assert!(!texts.is_empty(), "{shell}: {error}");
                for text in texts {
                    assert!(error.contains(text), "{shell}: {error}\nwant: {text}");
                }
                assert_eq!(error.lines().count(), texts.len(), "{error}");
            }
        }
    }
}

/// #181 B5 rounds 5 and 6, plan section 8 (23f, 23g): a macro is an `ARGUMENT_MACROS` macro by its path as written, the
/// listed path or its last segment alone (`bail!` is `anyhow::bail!`). The check reads its arguments as code of the
/// caller, also in a test, and reads no token of any other macro (an unlisted one, another path to a listed name, an
/// opaque one, a `macro_rules!` body), so a call there does not count.
#[test]
fn a_macro_is_read_only_when_its_path_is_listed() {
    let text = "use anyhow::{anyhow, bail};\n\
                fn printed() { println!(\"{}\", x()); }\n\
                fn bailed() { bail!(\"{:?}\", x()); }\n\
                fn qualified() { anyhow::ensure!(true, \"{:?}\", x()); }\n\
                fn made() { let e = anyhow::anyhow!(\"{:?}\", x()); }\n\
                fn custom() { custom!(x()); }\n\
                fn other_path() { other::println!(\"{}\", x()); }\n\
                fn opaque() { matches!(x(), Ok(())); }\n\
                fn quoted() { let _: syn::Expr = syn::parse_quote!(x()); }\n\
                fn defined() { macro_rules! m { () => { x() } } }\n\
                #[test]\n\
                fn asserted() { assert!(decide()); custom!(hidden()); }\n";
    let calls = Calls::of(&[("xtask/src/a.rs".to_string(), text.to_string())]).unwrap();
    let callers: BTreeSet<&str> = calls
        .by_function
        .iter()
        .filter(|(_, called)| called.contains("x"))
        .map(|((_, function), _)| function.as_str())
        .collect();
    assert_eq!(callers, ["bailed", "made", "printed", "qualified"].into());
    assert!(calls.tested.contains("decide"));
    assert!(!calls.tested.contains("hidden"));
}

/// #181 R6, plan section 8 (23f, 23g): a declaration that introduces a reserved name (the name or the crate of an
/// `ARGUMENT_MACROS` macro) fails, in a block or a module, whatever its kind, a `macro_rules!` definition included; a
/// `use` that introduces one passes only with its listed path, its crate or a `std` or `core` path. Other names (the
/// names that only the removed I/O classification read: `Command`, `std`, `syn`, `json`), a function named like a macro,
/// `extern crate` without a rename and a function inside a function pass.
#[test]
fn a_declaration_of_a_reserved_name_fails() {
    let text = "mod m {\n\
                const vec: u8 = 0; enum println {} mod anyhow {} static format: u8 = 0; struct bail; trait ensure {}\n\
                type write = u8; union assert { x: u8 } extern crate libc as panic; macro_rules! assert_eq { () => {} }\n\
                struct Command; mod std {} mod syn {} const json: u8 = 0; enum parse_quote {}\n\
                }\n\
                fn outer() { struct todo; }\n\
                use crate::m::bail; use other::format; use std::env as print; use syn::parse_quote as println;\n\
                use std::fs::write; use anyhow::{anyhow, ensure}; use anyhow; use std::format;\n\
                use crate::pure::Command; use std::process::Command; use syn::parse_quote;\n\
                extern crate syn; fn format() {} fn vec() {} struct Local; fn outer2() { fn inner() {} }\n";
    let error = Calls::of(&[("xtask/src/a.rs".to_string(), text.to_string())])
        .unwrap_err()
        .to_string();
    let declared = [
        ("constant", "vec"),
        ("enum", "println"),
        ("module", "anyhow"),
        ("static", "format"),
        ("struct", "bail"),
        ("trait", "ensure"),
        ("type alias", "write"),
        ("union", "assert"),
        ("extern crate", "panic"),
        ("macro", "assert_eq"),
        ("struct", "todo"),
    ];
    for (kind, name) in declared {
        assert!(
            error.contains(&format!(
                "the {kind} `{name}` declares a reserved name of gate-decisions"
            )),
            "{kind} {name}: {error}"
        );
    }
    let used = [
        ("bail", "crate::m::bail"),
        ("format", "other::format"),
        ("print", "std::env"),
        ("println", "syn::parse_quote"),
    ];
    for (name, target) in used {
        assert!(
            error.contains(&format!(
                "the `use` introduces the reserved name `{name}` of gate-decisions as `{target}`"
            )),
            "{name}: {error}"
        );
    }
    assert_eq!(
        error.lines().count(),
        declared.len() + used.len(),
        "{error}"
    );
}

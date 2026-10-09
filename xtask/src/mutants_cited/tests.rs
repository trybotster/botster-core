use super::*;

/// The slow filter before #167: integration-test targets named `slow*` only.
const OLD_FILTER: &str = "binary(/^slow/)";

struct Repo {
    files: BTreeMap<String, String>,
    packages: Vec<Package>,
}

impl Repo {
    fn new() -> Repo {
        Repo {
            files: BTreeMap::new(),
            packages: Vec::new(),
        }
    }

    fn file(mut self, path: &str, text: &str) -> Repo {
        self.files.insert(path.to_string(), text.to_string());
        self
    }

    /// A package `crates/<name>` with a lib (`src/lib.rs`) and one test target per `tests/<target>.rs` file.
    fn package(mut self, name: &str, slow: bool, tests: &[&str]) -> Repo {
        let mut targets = vec![Target {
            kind: "lib".into(),
            name: name.replace('-', "_"),
            root: format!("crates/{name}/src/lib.rs"),
            required_features: Vec::new(),
        }];
        for test in tests {
            targets.push(Target {
                kind: "test".into(),
                name: (*test).into(),
                root: format!("crates/{name}/tests/{test}.rs"),
                required_features: Vec::new(),
            });
        }
        self.packages.push(Package {
            name: name.into(),
            slow,
            targets,
        });
        self
    }

    fn check(&self, toml: &str, filter: &str) -> Vec<String> {
        let read = |path: &str| self.files.get(path).cloned();
        let paths: Vec<String> = self.files.keys().cloned().collect();
        check(
            toml,
            &self.packages,
            &read,
            &paths,
            &Filter::parse(filter).unwrap(),
        )
        .unwrap()
    }
}

/// A library with real-disk tests in a `slow_tests` module behind the `slow` feature, as botster-core-sys storage.rs.
fn storage() -> Repo {
    Repo::new()
        .package("sys", true, &[])
        .file("crates/sys/src/lib.rs", "pub mod storage;\n")
        .file(
            "crates/sys/src/storage.rs",
            "pub fn write_row() {}\n#[cfg(all(test, feature = \"slow\"))]\nmod slow_tests {\n    #[test]\n    fn a_written_row_reads_back() {}\n}\n",
        )
}

const CITES_STORAGE: &str =
    "exclude_re = [\n    # write_row: a_written_row_reads_back proves it.\n    'x',\n]\n";

/// The red-on-revert proof of the selection fix (#167, #164): under the old filter no tier runs the cited `slow_tests`
/// test, so the check fails; under the gate's filter the slow tier runs it.
#[test]
fn a_cited_test_that_no_tier_runs_fails_and_the_slow_filter_runs_slow_test_modules() {
    let repo = storage();
    assert_eq!(
        repo.check(CITES_STORAGE, OLD_FILTER),
        [".cargo/mutants.toml:2: cites the test `a_written_row_reads_back`, which no gate tier runs (storage::slow_tests::a_written_row_reads_back in sys)"]
    );
    assert!(repo
        .check(CITES_STORAGE, crate::test_budget::SLOW_FILTER)
        .is_empty());
}

/// The red-on-revert proof of a rename: the reason keeps the old name, which names nothing now.
#[test]
fn a_cited_test_that_was_renamed_fails() {
    let renamed = storage().file(
        "crates/sys/src/storage.rs",
        "#[cfg(all(test, feature = \"slow\"))]\nmod slow_tests {\n    #[test]\n    fn a_written_row_is_read_back() {}\n}\n",
    );
    assert_eq!(
        renamed.check(CITES_STORAGE, crate::test_budget::SLOW_FILTER),
        [".cargo/mutants.toml:2: cites `a_written_row_reads_back`, which names no test, no test target, no identifier of the code and no vendored word"]
    );
}

#[test]
fn a_default_tier_test_an_item_and_a_vendored_word_pass() {
    let repo = Repo::new()
        .package("a", false, &[])
        .file("crates/a/src/lib.rs", "pub fn parse_the_line() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn a_line_parses() {}\n}\n")
        .file(".gitmodules", "[submodule \"vendor/z\"]\n\tpath = vendor/z\n")
        .file("vendor/z/terminal.zig", "const default_query_max_bytes = 4096;\n");
    let toml =
        "# parse_the_line: a_line_parses; the default is terminal.zig `default_query_max_bytes`.\n";
    assert!(repo.check(toml, OLD_FILTER).is_empty());
    let unknown = "# no_such_thing_anywhere is cited.\n";
    assert_eq!(
        repo.check(unknown, OLD_FILTER),
        [".cargo/mutants.toml:1: cites `no_such_thing_anywhere`, which names no test, no test target, no identifier of the code and no vendored word"]
    );
}

#[test]
fn an_ignored_test_runs_in_no_tier() {
    let repo = Repo::new().package("a", false, &[]).file(
        "crates/a/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[test]\n    #[ignore]\n    fn a_long_check_runs() {}\n}\n",
    );
    assert_eq!(
        repo.check("# a_long_check_runs\n", OLD_FILTER),
        [".cargo/mutants.toml:1: cites the test `a_long_check_runs`, which no gate tier runs (tests::a_long_check_runs in a)"]
    );
}

#[test]
fn a_slow_target_runs_only_in_a_package_with_a_slow_feature() {
    let target = "#![cfg(feature = \"slow\")]\n#[test]\nfn the_child_ends_in_time() {}\n";
    let slow = Repo::new()
        .package("a", true, &["slow_process"])
        .file("crates/a/src/lib.rs", "")
        .file("crates/a/tests/slow_process.rs", target);
    assert!(slow
        .check("# the_child_ends_in_time, slow_process\n", OLD_FILTER)
        .is_empty());
    let no_feature = Repo::new()
        .package("a", false, &["slow_process"])
        .file("crates/a/src/lib.rs", "")
        .file("crates/a/tests/slow_process.rs", target);
    assert_eq!(
        no_feature.check("# the_child_ends_in_time\n# slow_process_target: slow_process\n", OLD_FILTER),
        [
            ".cargo/mutants.toml:1: cites the test `the_child_ends_in_time`, which no gate tier runs (the_child_ends_in_time in slow_process)",
            ".cargo/mutants.toml:2: cites `slow_process_target`, which names no test, no test target, no identifier of the code and no vendored word",
        ]
    );
}

#[test]
fn a_cited_test_target_needs_a_test_that_some_tier_runs() {
    let repo = Repo::new()
        .package("a", false, &["slow_real_core"])
        .file("crates/a/src/lib.rs", "")
        .file(
            "crates/a/tests/slow_real_core.rs",
            "#![cfg(feature = \"slow\")]\n#[test]\nfn t() {}\n",
        );
    assert_eq!(
        repo.check("# slow_real_core\n", OLD_FILTER),
        [".cargo/mutants.toml:1: cites the test target `slow_real_core`, which no gate tier runs a test of"]
    );
}

#[test]
fn a_test_of_one_system_runs_in_a_tier_and_a_windows_test_does_not() {
    let repo = Repo::new()
        .package("a", false, &[])
        .file(
            "crates/a/src/lib.rs",
            "#[cfg(test)]\nmod tests {\n    #[cfg(target_os = \"macos\")]\n    #[test]\n    fn kqueue_sees_the_exit() {}\n    #[cfg(windows)]\n    #[test]\n    fn windows_has_no_group() {}\n}\n",
        );
    assert_eq!(
        repo.check("# kqueue_sees_the_exit, windows_has_no_group\n", OLD_FILTER),
        [".cargo/mutants.toml:1: cites the test `windows_has_no_group`, which no gate tier runs (tests::windows_has_no_group in a)"]
    );
}

#[test]
fn module_files_are_followed_and_named_as_nextest_names_them() {
    let repo = Repo::new()
        .package("a", true, &["slow_x"])
        .file(
            "crates/a/src/lib.rs",
            "mod outer;\nmod wrap {\n    #[path = \"w.rs\"]\n    mod w;\n}\n",
        )
        .file(
            "crates/a/src/outer.rs",
            "mod inner;\n#[path = \"side.rs\"]\nmod side;\n",
        )
        // A top-level `#[path]` of `outer.rs` is beside it; an inline one is under the inline module's directory. The
        // other two files are decoys.
        .file("crates/a/src/side.rs", "#[test]\nfn beside_test() {}\n")
        .file(
            "crates/a/src/outer/side.rs",
            "#[test]\nfn wrong_file_test() {}\n",
        )
        .file(
            "crates/a/src/wrap/w.rs",
            "#[test]\nfn inline_path_test() {}\n",
        )
        .file("crates/a/src/w.rs", "#[test]\nfn wrong_inline_test() {}\n")
        .file(
            "crates/a/src/outer/inner/mod.rs",
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn deep_test_is_found() {}\n}\n",
        )
        .file("crates/a/tests/slow_x.rs", "mod common;\n")
        .file(
            "crates/a/tests/common/mod.rs",
            "#[path = \"extra.rs\"]\nmod more;\n",
        )
        .file(
            "crates/a/tests/common/extra.rs",
            "#[test]\nfn shared_helper_test() {}\n",
        );
    let tests = tests(&repo.packages, &|path: &str| repo.files.get(path).cloned()).unwrap();
    let names: Vec<(&str, &str)> = tests
        .iter()
        .map(|t| (t.binary.as_str(), t.path.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("a", "outer::inner::tests::deep_test_is_found"),
            ("a", "outer::side::beside_test"),
            ("a", "wrap::w::inline_path_test"),
            ("slow_x", "common::more::shared_helper_test")
        ]
    );
    let missing = Repo::new()
        .package("a", false, &[])
        .file("crates/a/src/lib.rs", "mod gone;\n");
    let error = tests_of_repo(&missing).err().expect("no file").to_string();
    assert_eq!(error, "crates/a/src/lib.rs: `mod gone;` has no file");
}

fn tests_of_repo(repo: &Repo) -> Result<Vec<TestFn>> {
    tests(&repo.packages, &|path: &str| repo.files.get(path).cloned())
}

#[test]
fn the_filter_has_binary_and_test_terms_joined_by_or() {
    let test = |binary: &str, path: &str| TestFn {
        package: "a".into(),
        binary: binary.into(),
        path: path.into(),
        name: String::new(),
        cfgs: Vec::new(),
        ignores: Vec::new(),
    };
    let filter = Filter::parse("binary(/^slow/) or test(/::slow_tests::/)").unwrap();
    assert!(filter.selects(&test("slow_process", "t")));
    assert!(filter.selects(&test("sys", "storage::slow_tests::t")));
    assert!(!filter.selects(&test("sys", "storage::tests::t")));
    let gate = Filter::parse(crate::test_budget::SLOW_FILTER).unwrap();
    assert!(gate.selects(&test("worker", "slow_driver::t")));
    // A top-level `slow_tests` module has no `::` before its name: the filter of #164 alone misses it.
    assert!(gate.selects(&test("botster_core", "slow_tests::t")));
    assert!(!filter.selects(&test("botster_core", "slow_tests::t")));
    for unknown in [
        "kind(test)",
        "binary(/^slow/) & test(/x/)",
        "not binary(/a/)",
        "binary(/a/) |",
        "binary(/a/) test(/b/)",
    ] {
        let error = Filter::parse(unknown).err().expect(unknown).to_string();
        assert!(
            error.contains("has a term that the check does not know"),
            "{error}"
        );
    }
}

#[test]
fn an_unknown_predicate_fails_the_check() {
    let repo = Repo::new().package("a", false, &[]).file(
        "crates/a/src/lib.rs",
        "#[cfg(loom)]\n#[test]\nfn under_a_model_checker() {}\n",
    );
    let read = |path: &str| repo.files.get(path).cloned();
    let error = check(
        "# under_a_model_checker\n",
        &repo.packages,
        &read,
        &[],
        &Filter::parse(OLD_FILTER).unwrap(),
    )
    .unwrap_err()
    .to_string();
    assert_eq!(
        error,
        "under_a_model_checker (a): the check does not know the predicate `loom`"
    );
}

#[test]
fn only_comment_words_with_three_parts_are_cited() {
    let toml =
        "exclude_globs = [\"a_b_c\"]\n# one_two and one_two_three\n    # four_five_six_seven\n";
    let names: Vec<(String, usize)> = cited(toml).into_iter().collect();
    assert_eq!(
        names,
        [
            ("four_five_six_seven".to_string(), 3),
            ("one_two_three".to_string(), 2)
        ]
    );
}

/// A cited test target counts only its own tests: a running test of another binary does not make it run, and its own
/// running test does.
#[test]
fn a_cited_test_target_counts_only_its_own_tests() {
    let target = "#![cfg(feature = \"slow\")]\n#[test]\nfn t() {}\n";
    let unrun = Repo::new()
        .package("a", false, &["slow_real_core", "quick"])
        .file("crates/a/src/lib.rs", "")
        .file("crates/a/tests/slow_real_core.rs", target)
        .file("crates/a/tests/quick.rs", "#[test]\nfn q() {}\n");
    assert_eq!(unrun.check("# slow_real_core\n", OLD_FILTER).len(), 1);
    let run = Repo::new()
        .package("a", true, &["slow_real_core"])
        .file("crates/a/src/lib.rs", "")
        .file("crates/a/tests/slow_real_core.rs", target);
    assert!(run.check("# slow_real_core\n", OLD_FILTER).is_empty());
}

/// A word that only the mutants file holds is not a word of another file.
#[test]
fn a_word_of_the_mutants_file_itself_does_not_count() {
    let toml = "# see the_only_citation\n";
    let repo = Repo::new()
        .package("a", false, &[])
        .file("crates/a/src/lib.rs", "")
        .file(MUTANTS_FILE, toml);
    assert_eq!(repo.check(toml, OLD_FILTER).len(), 1);
}

/// The `cfg` predicates of a test, on a gate system.
#[test]
fn cfg_predicates_hold_as_on_the_gate_system() {
    let holds = |cfg: &str, system: &str| holds(&syn::parse_str(cfg).unwrap(), false, system);
    for (cfg, linux, macos) in [
        ("any(windows, unix)", true, true),
        ("any(windows, windows)", false, false),
        ("not(windows)", true, true),
        ("not(unix)", false, false),
        ("target_os = \"linux\"", true, false),
        ("target_os = \"macos\"", false, true),
        ("target_family = \"unix\"", true, true),
        ("target_family = \"windows\"", false, false),
    ] {
        assert_eq!(holds(cfg, "linux"), Ok(linux), "{cfg}");
        assert_eq!(holds(cfg, "macos"), Ok(macos), "{cfg}");
    }
    assert!(holds("not(unix, windows)", "linux").is_err());
}

/// The tracked files of a repository, from git.
#[test]
fn the_tracked_files_come_from_git() {
    let repo = crate::fsutil::test_repo(&[("a/b.rs", ""), ("c.toml", "")]);
    assert_eq!(
        tracked_with_submodules(repo.path()).unwrap(),
        ["a/b.rs", "c.toml"]
    );
    let not_a_repo = tempfile::tempdir().unwrap();
    assert!(tracked_with_submodules(not_a_repo.path()).is_err());
}

/// The packages of this workspace, from `cargo metadata`: botster-test-process with its slow feature and its slow test
/// target.
#[test]
fn the_packages_come_from_cargo_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let packages = packages(root).unwrap();
    let process = packages
        .iter()
        .find(|p| p.name == "botster-test-process")
        .unwrap();
    assert!(process.slow);
    assert!(process.targets.iter().any(|t| t.kind == "test"
        && t.name == "slow_process"
        && t.root == "crates/botster-test-process/tests/slow_process.rs"));
}

/// #181 B6: a deleted test's name that stays in a document, a comment, a string or a test of another name does not pass;
/// an item of the same name that is not a test does.
#[test]
fn a_deleted_test_that_a_document_or_a_string_still_mentions_fails() {
    let deleted = Repo::new()
        .package("a", false, &[])
        .file(
            "crates/a/src/lib.rs",
            "// a_written_row_reads_back\nconst NOTE: &str = \"a_written_row_reads_back\";\n\
             #[cfg(test)]\nmod tests {\n    #[test]\n    fn a_row_reads() {}\n}\n",
        )
        .file("crates/a/README.md", "a_written_row_reads_back proves it\n")
        .file("verdicts/v.md", "a_written_row_reads_back\n");
    assert_eq!(
        deleted.check(CITES_STORAGE, OLD_FILTER),
        [".cargo/mutants.toml:2: cites `a_written_row_reads_back`, which names no test, no test target, no identifier of the code and no vendored word"]
    );
    let item = deleted.file(
        "crates/a/src/lib.rs",
        "pub fn a_written_row_reads_back() {}\n",
    );
    assert!(item.check(CITES_STORAGE, OLD_FILTER).is_empty());
}

/// Each identifier of the code (an item, a field or a method of a dependency, a crate) is a name that a citation may name;
/// the name of a `#[test]` function is not, and neither is a word of a comment, a string, a file outside the vendored
/// paths, or the mutants file.
#[test]
fn the_defined_names_are_the_identifiers_of_the_code_and_the_vendored_words() {
    let files = BTreeMap::from([
        (
            "crates/a/src/lib.rs".to_string(),
            "fn f_fn() {}\n#[test]\nfn t_test() {}\n#[tokio::test]\nasync fn t_async() {}\n\
             impl S { fn f_method() {} }\ntrait T_trait { fn f_trait_fn(); }\nconst C_CONST: u8 = 0;\n\
             static S_STATIC: u8 = 0;\nstruct S_struct { f_field: u8 }\nenum E_enum { V_variant }\n\
             type T_type = u8;\nmod m_mod {}\nmacro_rules! m_macro { () => {} }\n\
             #[inline]\nfn f_inline() {}\n\
             fn uses(l: dep_crate::Limits) -> u8 { l.dep_field + l.dep_method() }\n\
             // c_comment\nconst Q: &str = \"s_string\";\n"
                .to_string(),
        ),
        ("vendor/z/a.c".to_string(), "int v_vendored(void);\n".to_string()),
        ("vendor/z/b.rs".to_string(), "not rust {\n".to_string()),
        (".gitmodules".to_string(), "\tpath = vendor/z\n".to_string()),
        ("docs/notes.md".to_string(), "d_doc\n".to_string()),
        (MUTANTS_FILE.to_string(), "# m_mutants\n".to_string()),
    ]);
    let read = |path: &str| files.get(path).cloned();
    let paths: Vec<String> = files.keys().cloned().collect();
    let names = defined_names(&read, &paths).unwrap();
    for name in [
        "f_fn",
        "f_method",
        "T_trait",
        "f_trait_fn",
        "C_CONST",
        "S_STATIC",
        "S_struct",
        "f_field",
        "E_enum",
        "V_variant",
        "T_type",
        "m_mod",
        "m_macro",
        "v_vendored",
        "not",
        "rust",
        "dep_crate",
        "dep_field",
        "dep_method",
        "f_inline",
    ] {
        assert!(names.contains(name), "{name}");
    }
    for name in [
        "t_test",
        "t_async",
        "d_doc",
        "m_mutants",
        "notes",
        "c_comment",
        "s_string",
    ] {
        assert!(!names.contains(name), "{name}");
    }
    let broken = BTreeMap::from([("crates/a/src/lib.rs".to_string(), "fn f( {\n".to_string())]);
    let read = |path: &str| broken.get(path).cloned();
    assert!(defined_names(&read, &["crates/a/src/lib.rs".to_string()])
        .unwrap_err()
        .to_string()
        .starts_with("crates/a/src/lib.rs:1: does not parse"));
}

/// #181 B6: `#[cfg_attr(<predicate>, ignore)]` ignores the test where the predicate holds, also nested; `cfg_attr(..,
/// test)` makes a test only where its predicate holds; an unknown predicate fails the check.
#[test]
fn a_conditionally_ignored_test_runs_only_where_its_condition_does_not_hold() {
    let repo = |attrs: &str| {
        Repo::new().package("a", false, &[]).file(
            "crates/a/src/lib.rs",
            &format!(
                "#[cfg(test)]\nmod tests {{\n    {attrs}\n    fn the_cited_check() {{}}\n}}\n"
            ),
        )
    };
    let unrun = ".cargo/mutants.toml:1: cites the test `the_cited_check`, which no gate tier runs (tests::the_cited_check in a)";
    for (attrs, runs) in [
        ("#[test]\n    #[cfg_attr(all(), ignore)]", false),
        ("#[test]\n    #[cfg_attr(unix, ignore = \"slow\")]", false),
        (
            "#[test]\n    #[cfg_attr(target_os = \"macos\", ignore)]",
            true,
        ),
        ("#[test]\n    #[cfg_attr(windows, ignore)]", true),
        (
            "#[test]\n    #[cfg_attr(unix, cfg_attr(test, ignore))]",
            false,
        ),
        (
            "#[test]\n    #[cfg_attr(unix, cfg_attr(windows, ignore))]",
            true,
        ),
        ("#[test]\n    #[cfg_attr(unix, derive(Debug))]", true),
        ("#[cfg_attr(unix, test)]", true),
        ("#[cfg_attr(windows, test)]", false),
    ] {
        let found = repo(attrs).check("# the_cited_check\n", OLD_FILTER);
        if runs {
            assert!(found.is_empty(), "{attrs}: {found:?}");
        } else {
            assert_eq!(found, [unrun], "{attrs}");
        }
    }
    let read = |attrs: &str| {
        let repo = repo(attrs);
        let read = |path: &str| repo.files.get(path).cloned();
        let paths: Vec<String> = repo.files.keys().cloned().collect();
        check(
            "# the_cited_check\n",
            &repo.packages,
            &read,
            &paths,
            &Filter::parse(OLD_FILTER).unwrap(),
        )
        .map_err(|error| error.to_string())
    };
    assert!(read("#[test]\n    #[cfg_attr(loom, ignore)]")
        .unwrap_err()
        .contains("the check does not know the predicate `loom`"));
    for malformed in ["#[test]\n    #[cfg_attr]", "#[test]\n    #[cfg_attr()]"] {
        assert!(read(malformed).is_err(), "{malformed}");
    }
}

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
        [".cargo/mutants.toml:2: cites `a_written_row_reads_back`, which names no test, no test target and no word of another file"]
    );
}

#[test]
fn a_default_tier_test_and_a_word_of_another_file_pass() {
    let repo = Repo::new()
        .package("a", false, &[])
        .file("crates/a/src/lib.rs", "pub fn parse_the_line() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn a_line_parses() {}\n}\n")
        .file("vendor/z/terminal.zig", "const default_query_max_bytes = 4096;\n");
    let toml =
        "# parse_the_line: a_line_parses; the default is terminal.zig `default_query_max_bytes`.\n";
    assert!(repo.check(toml, OLD_FILTER).is_empty());
    let unknown = "# no_such_thing_anywhere is cited.\n";
    assert_eq!(
        repo.check(unknown, OLD_FILTER),
        [".cargo/mutants.toml:1: cites `no_such_thing_anywhere`, which names no test, no test target and no word of another file"]
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
            ".cargo/mutants.toml:2: cites `slow_process_target`, which names no test, no test target and no word of another file",
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
        .file("crates/a/src/lib.rs", "mod outer;\n")
        .file("crates/a/src/outer.rs", "mod inner;\n")
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
        ignored: false,
    };
    let filter = Filter::parse("binary(/^slow/) or test(/::slow_tests::/)").unwrap();
    assert!(filter.selects(&test("slow_process", "t")));
    assert!(filter.selects(&test("sys", "storage::slow_tests::t")));
    assert!(!filter.selects(&test("sys", "storage::tests::t")));
    let gate = Filter::parse(crate::test_budget::SLOW_FILTER).unwrap();
    assert!(gate.selects(&test("worker", "slow_driver::t")));
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

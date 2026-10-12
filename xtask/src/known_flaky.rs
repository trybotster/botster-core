//! The known-flaky list `ci/known-flaky.toml` (flake guard PR 2; lead, 2026-10-11): each test that fails at random on v1
//! with a diagnosed cause and an open fix. The mutation stages do not run a listed test, so the test cannot catch a mutant
//! by its flake. The slow step and the stage-2 baseline still run it: the list hides no failure of an unmutated run.
//!
//! Each entry names the test (`binary`, the nextest binary id, and `test`, its exact name), the `diagnosis`, the recorded
//! failure `signature`, the `owner` of the fix, and the pull request that removes the entry (`removed_by`, `#<number>`).
//! The `lists` step refuses an entry with a missing, empty or unknown field, a duplicate, and a name with a character
//! outside `[A-Za-z0-9_:/-]`, which a filterset needs no escape for. The Stage 1 exit needs the list empty (plan 23ap, done
//! item 7).
//!
//! The `signature` is what a failure must match before a red gate may run once more (BUILD.md Testing rule 9, second
//! exception, contracts main b6c7ea9d): comma-separated clauses of the step, the limit and the symptom, for example
//! `seed any, step 8, step_timeout`. The seed is not part of it (plan 23bl(A)): a `seed` clause may only be `seed any`.
//!
//! A test can run in both stages (a library's unit test is in both builds), or in stage 2 only (a test that only the slow
//! features build, also in a binary that both builds have). So each stage lists its own tests (`cargo nextest list` with
//! the stage's features) and removes every listed test that its list has (`stage_filter`): a test of both stages is
//! removed from both. A stage never names a test that its list does not have, and nextest refuses a `binary_id` that the
//! stage's features do not build. The slow step fails on an entry that neither tier's list has (`absent`), so a stale or
//! misspelled entry fails every gate.

use std::collections::BTreeSet;

/// The list file, from the repository root.
pub const FILE: &str = "ci/known-flaky.toml";

/// The tests of a nextest list, by binary id and test name.
pub type Tests = BTreeSet<(String, String)>;

/// One known-flaky test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub binary: String,
    pub test: String,
    pub diagnosis: String,
    pub signature: String,
    pub owner: String,
    pub removed_by: String,
}

const FIELDS: [&str; 6] = [
    "binary",
    "test",
    "diagnosis",
    "signature",
    "owner",
    "removed_by",
];

/// Why `signature` is not a failure signature of the module documentation's form, or `None`.
fn signature_error(signature: &str) -> Option<String> {
    let clauses: Vec<&str> = signature.split(',').map(str::trim).collect();
    if clauses.len() < 2 || clauses.iter().any(|clause| clause.is_empty()) {
        return Some(format!(
            "`signature` {signature:?} is not comma-separated clauses of the step, the limit and the symptom"
        ));
    }
    clauses
        .iter()
        .find(|clause| clause.split_whitespace().next() == Some("seed") && **clause != "seed any")
        .map(|clause| format!("`signature` {signature:?} names a seed ({clause:?}); the seed is not part of it (seed any)"))
}

/// A name that a filterset `=` matcher takes as it is: not empty, only `[A-Za-z0-9_:/-]`.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | '-'))
}

/// The entries of the list file.
///
/// # Errors
/// The text is not TOML, it has a key other than `flaky`, or an entry breaks a rule of the module documentation. The error
/// names the entry by its position (from 1).
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("{FILE} is not TOML: {e}"))?;
    if let Some(key) = table.keys().find(|key| *key != "flaky") {
        return Err(format!("{FILE}: unknown key `{key}`"));
    }
    let Some(entries) = table.get("flaky") else {
        return Ok(Vec::new());
    };
    let entries = entries
        .as_array()
        .ok_or_else(|| format!("{FILE}: `flaky` is not an array of tables"))?;
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("{FILE}: entry {}", index + 1);
        let entry = entry
            .as_table()
            .ok_or_else(|| format!("{at} is not a table"))?;
        if let Some(key) = entry.keys().find(|key| !FIELDS.contains(&key.as_str())) {
            return Err(format!("{at}: unknown field `{key}`"));
        }
        let field = |name: &str| -> Result<String, String> {
            let value = entry
                .get(name)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{at}: no `{name}` string"))?;
            if value.trim().is_empty() {
                return Err(format!("{at}: `{name}` is empty"));
            }
            Ok(value.to_string())
        };
        let binary = field("binary")?;
        let test = field("test")?;
        for (name, value) in [("binary", &binary), ("test", &test)] {
            if !plain_name(value) {
                return Err(format!(
                    "{at}: `{name}` {value:?} has a character outside [A-Za-z0-9_:/-]"
                ));
            }
        }
        let removed_by = field("removed_by")?;
        let number = removed_by.strip_prefix('#').unwrap_or_default();
        if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "{at}: `removed_by` {removed_by:?} is not a pull request `#<number>`"
            ));
        }
        let signature = field("signature")?;
        if let Some(error) = signature_error(&signature) {
            return Err(format!("{at}: {error}"));
        }
        if !seen.insert((binary.clone(), test.clone())) {
            return Err(format!("{at}: {binary} {test} is listed twice"));
        }
        parsed.push(Entry {
            binary,
            test,
            diagnosis: field("diagnosis")?,
            signature,
            owner: field("owner")?,
            removed_by,
        });
    }
    Ok(parsed)
}

/// The entries whose test the stage's list `tests` has.
fn in_stage<'a>(entries: &'a [Entry], tests: &'a Tests) -> impl Iterator<Item = &'a Entry> {
    entries
        .iter()
        .filter(|entry| tests.contains(&(entry.binary.clone(), entry.test.clone())))
}

/// The filter of a mutation stage whose list is `tests`: `filter` (or every test) without each listed test that the stage
/// has. `None` (no filter) when `filter` is `None` and the stage has no listed test.
pub fn stage_filter(entries: &[Entry], tests: &Tests, filter: Option<&str>) -> Option<String> {
    let terms: Vec<String> = in_stage(entries, tests)
        .map(|entry| format!("(binary_id(={}) & test(={}))", entry.binary, entry.test))
        .collect();
    if terms.is_empty() {
        return filter.map(str::to_string);
    }
    let listed = terms.join(" | ");
    Some(match filter {
        None => format!("all() - ({listed})"),
        Some(filter) => format!("({filter}) - ({listed})"),
    })
}

/// The lines that a mutation stage whose list is `tests` prints for the listed tests that it does not run: each one with
/// its diagnosis, failure signature, owner and removing pull request.
pub fn report(entries: &[Entry], tests: &Tests) -> Vec<String> {
    in_stage(entries, tests)
        .map(|entry| {
            format!(
                "known flaky, not run: {} {} ({}; signature {}; owner {}; removed by {})",
                entry.binary,
                entry.test,
                entry.diagnosis,
                entry.signature,
                entry.owner,
                entry.removed_by
            )
        })
        .collect()
}

/// The violations: each entry whose test neither the default tier's list nor the slow tier's list has.
pub fn absent(entries: &[Entry], default: &Tests, slow: &Tests) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| {
            let key = (entry.binary.clone(), entry.test.clone());
            !default.contains(&key) && !slow.contains(&key)
        })
        .map(|entry| {
            format!(
                "{FILE}: {} {} is in neither the default tier's nor the slow tier's test list: correct or remove the \
                 entry",
                entry.binary, entry.test
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r##"
[[flaky]]
binary = "botster-core::slow_conformance"
test = "conf::ev_5_x"
diagnosis = "R-63"
signature = "seed any, step 8, step_timeout"
owner = "P3"
removed_by = "#243"
"##;

    fn entry(binary: &str, test: &str) -> Entry {
        Entry {
            binary: binary.into(),
            test: test.into(),
            diagnosis: "d".into(),
            signature: "s, t".into(),
            owner: "o".into(),
            removed_by: "#1".into(),
        }
    }

    fn tests(keys: &[(&str, &str)]) -> Tests {
        keys.iter()
            .map(|(binary, test)| (binary.to_string(), test.to_string()))
            .collect()
    }

    #[test]
    fn an_entry_is_read_with_each_field_and_an_empty_file_has_none() {
        assert_eq!(
            parse(ENTRY),
            Ok(vec![Entry {
                binary: "botster-core::slow_conformance".into(),
                test: "conf::ev_5_x".into(),
                diagnosis: "R-63".into(),
                signature: "seed any, step 8, step_timeout".into(),
                owner: "P3".into(),
                removed_by: "#243".into(),
            }])
        );
        assert_eq!(parse("# none\n"), Ok(vec![]));
    }

    /// Each field must be there and not empty; each rule of the module documentation refuses its entry. A `tier` field
    /// (the first form of this list) is an unknown field.
    #[test]
    fn an_entry_that_breaks_a_rule_is_refused() {
        for field in FIELDS {
            let without: String = ENTRY
                .lines()
                .filter(|line| !line.starts_with(&format!("{field} =")))
                .map(|line| format!("{line}\n"))
                .collect();
            assert_eq!(
                parse(&without),
                Err(format!("{FILE}: entry 1: no `{field}` string")),
                "{field}"
            );
            let line = ENTRY
                .lines()
                .find(|line| line.starts_with(&format!("{field} =")))
                .unwrap();
            let empty = ENTRY.replace(line, &format!("{field} = \" \""));
            assert_eq!(
                parse(&empty),
                Err(format!("{FILE}: entry 1: `{field}` is empty")),
                "{field}"
            );
        }
        let cases = [
            (
                ENTRY.replace("removed_by", "fixed_by"),
                "unknown field `fixed_by`",
            ),
            (format!("{ENTRY}tier = \"slow\"\n"), "unknown field `tier`"),
            // The failure signature (BUILD.md Testing rule 9, second exception): clauses, and no seed.
            (
                ENTRY.replace("seed any, step 8, step_timeout", "step_timeout"),
                "`signature` \"step_timeout\" is not comma-separated clauses",
            ),
            (
                ENTRY.replace("seed any, step 8, step_timeout", "step 8,, step_timeout"),
                "`signature` \"step 8,, step_timeout\" is not comma-separated clauses",
            ),
            (
                ENTRY.replace(
                    "seed any, step 8, step_timeout",
                    "seed 10, step 8, step_timeout",
                ),
                "`signature` \"seed 10, step 8, step_timeout\" names a seed (\"seed 10\")",
            ),
            (format!("{ENTRY}\n[other]\n"), "unknown key `other`"),
            (ENTRY.replace("\"#243\"", "\"243\""), "`removed_by` \"243\""),
            (ENTRY.replace("\"#243\"", "\"#\""), "`removed_by` \"#\""),
            (
                ENTRY.replace("\"#243\"", "\"#24a\""),
                "`removed_by` \"#24a\"",
            ),
            (
                ENTRY.replace("conf::ev_5_x", "conf::ev_5 x"),
                "`test` \"conf::ev_5 x\" has a character",
            ),
            (
                ENTRY.replace("conf::ev_5_x", "conf::ev_5_x)"),
                "`test` \"conf::ev_5_x)\" has a character",
            ),
            (
                ENTRY.replace("botster-core::slow_conformance", "a,b"),
                "`binary` \"a,b\" has a character",
            ),
            (
                format!("{ENTRY}{ENTRY}"),
                "entry 2: botster-core::slow_conformance conf::ev_5_x is listed twice",
            ),
            (
                "flaky = 1\n".to_string(),
                "`flaky` is not an array of tables",
            ),
            ("flaky = [1]\n".to_string(), "entry 1 is not a table"),
            ("[[flaky]\n".to_string(), "is not TOML"),
        ];
        for (text, error) in cases {
            match parse(&text) {
                Err(found) => assert!(found.contains(error), "{error}: {found}"),
                Ok(entries) => panic!("{error}: {entries:?}"),
            }
        }
    }

    /// GS-KF-R1-1: a stage removes each listed test that its own list has. A test of both stages (a library's unit test)
    /// is removed from both; a test that only the slow features build, in a binary of both stages, only from stage 2; a
    /// test that a stage does not have is not named in its filter. With no listed test in a stage, its filter is unchanged.
    #[test]
    fn each_stage_removes_every_listed_test_that_its_list_has() {
        let entries = [
            entry("lib", "slow_both"),
            entry("lib", "slow_tests::slow_only"),
            entry("lib::slow_x", "conf::one"),
        ];
        let stage1 = tests(&[("lib", "slow_both"), ("lib", "other")]);
        let stage2 = tests(&[
            ("lib", "slow_both"),
            ("lib", "slow_tests::slow_only"),
            ("lib::slow_x", "conf::one"),
        ]);
        assert_eq!(
            stage_filter(&entries, &stage1, None),
            Some("all() - ((binary_id(=lib) & test(=slow_both)))".to_string())
        );
        assert_eq!(
            stage_filter(&entries, &stage2, Some("f")),
            Some(
                "(f) - ((binary_id(=lib) & test(=slow_both)) | (binary_id(=lib) & test(=slow_tests::slow_only)) | \
                 (binary_id(=lib::slow_x) & test(=conf::one)))"
                    .to_string()
            )
        );
        let none = tests(&[("lib", "other")]);
        assert_eq!(stage_filter(&entries, &none, None), None);
        assert_eq!(
            stage_filter(&entries, &none, Some("f")),
            Some("f".to_string())
        );
        assert_eq!(stage_filter(&[], &stage2, Some("f")), Some("f".to_string()));
        assert_eq!(
            report(&entries, &stage1),
            ["known flaky, not run: lib slow_both (d; signature s, t; owner o; removed by #1)"]
        );
        assert_eq!(report(&entries, &stage2).len(), 3);
    }

    /// An entry that neither tier's list has is a violation; an entry that either list has is not.
    #[test]
    fn an_entry_in_neither_tier_is_a_violation() {
        let entries = [
            entry("lib", "a"),
            entry("lib", "b"),
            entry("lib", "missing"),
        ];
        let default = tests(&[("lib", "a")]);
        let slow = tests(&[("lib", "b")]);
        assert_eq!(
            absent(&entries, &default, &slow),
            [format!(
                "{FILE}: lib missing is in neither the default tier's nor the slow tier's test list: correct or remove \
                 the entry"
            )]
        );
        assert_eq!(absent(&entries[..2], &default, &slow), Vec::<String>::new());
    }

    /// The repository's list parses, and it is empty: #243 removed the cause of the ev_5 flake and its entry (the Stage 1
    /// exit needs the list empty).
    #[test]
    fn the_repository_list_parses_and_is_empty() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let entries = parse(&std::fs::read_to_string(root.join(FILE)).unwrap()).unwrap();
        let names: Vec<(&str, &str)> = entries
            .iter()
            .map(|e| (e.binary.as_str(), e.test.as_str()))
            .collect();
        assert_eq!(names, Vec::<(&str, &str)>::new());
    }
}

/// GS-KF-R1-1 on real nextest runs of the fixture `xtask/fixtures/slow-coverage` (slow tier: it starts cargo).
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;
    use botster_test_process::{run_to_completion, Deadline};
    use std::path::Path;
    use std::process::Command;

    /// Each stage lists its own tests and removes every listed test that its list has. `slow_in_both` is in both stages:
    /// both runs leave it out. `slow_tests::runs_only_under_slow` (the copy renames the fixture's `mod tests`, so the slow
    /// filter selects it, GS-KF-R2-2) is a test that only the slow feature builds, in the library binary that both stages
    /// have: only stage 2 has it, and stage 1's filter does not name it. `selected` is in a binary that only stage 2
    /// builds. Without its exclusion, each stage runs each listed test that it has; with it, none, and it still runs
    /// `slow_kept`. An entry that neither tier lists fails the slow step's check.
    #[test]
    fn each_stage_leaves_out_every_listed_test_that_it_builds() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/slow-coverage");
        let dir = tempfile::tempdir().unwrap();
        for sub in ["src", "tests", ".config"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        for (from, to) in [
            ("Cargo.toml.in", "Cargo.toml"),
            ("src/lib.rs", "src/lib.rs"),
            ("tests/slow_conformance.rs", "tests/slow_conformance.rs"),
        ] {
            std::fs::copy(fixture.join(from), dir.path().join(to)).unwrap();
        }
        for file in ["nextest.toml", "test-wrapper.sh"] {
            std::fs::copy(
                root.join(".config").join(file),
                dir.path().join(".config").join(file),
            )
            .unwrap();
        }
        let lib = dir.path().join("src/lib.rs");
        let source = std::fs::read_to_string(&lib).unwrap();
        assert_eq!(source.matches("mod tests").count(), 1);
        std::fs::write(&lib, source.replace("mod tests", "mod slow_tests")).unwrap();
        let target = dir.path().join("target");
        let packages = ["botster-core".to_string()];
        let entry = |binary: &str, test: &str| Entry {
            binary: binary.into(),
            test: test.into(),
            diagnosis: "d".into(),
            signature: "s, t".into(),
            owner: "o".into(),
            removed_by: "#1".into(),
        };
        let entries = [
            entry("botster-core", "slow_in_both"),
            entry("botster-core", "slow_tests::runs_only_under_slow"),
            entry("botster-core::slow_conformance", "selected"),
        ];
        let stage1_selection = crate::test_budget::tier_selection(false, &packages);
        let stage2_selection = crate::ci::stage2_packages(&packages);
        let stage1 = crate::slow_coverage::tests(dir.path(), &target, &stage1_selection).unwrap();
        let stage2 = crate::slow_coverage::tests(dir.path(), &target, &stage2_selection).unwrap();
        let stage1_filter = stage_filter(&entries, &stage1, None).unwrap();
        assert_eq!(
            stage1_filter,
            "all() - ((binary_id(=botster-core) & test(=slow_in_both)))"
        );
        let stage2_filter =
            stage_filter(&entries, &stage2, Some(crate::test_budget::SLOW_FILTER)).unwrap();
        for entry in &entries {
            assert!(
                stage2_filter.contains(&format!("test(={})", entry.test)),
                "{stage2_filter}"
            );
        }

        // The runs, with the mutants profile as the stages run: the listed tests do not run, `slow_kept` does.
        let run = |selection: &[String], filter: &str| {
            let mut command = Command::new("cargo");
            command
                .current_dir(dir.path())
                .args(["nextest", "run", "--target-dir"])
                .arg(&target)
                .args(selection)
                .args(["--profile", "mutants", "--no-tests=pass", "-E", filter]);
            let output = run_to_completion(
                &mut command,
                Deadline::after(crate::test_budget::SLOW_DEADLINE),
            )
            .unwrap();
            (
                output.status.success(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        };
        // Before the exclusion: stage 1 runs every test, so `slow_in_both`; stage 2's slow filter selects all three.
        for (selection, filter, listed) in [
            (&stage1_selection, "all()", &entries[..1]),
            (
                &stage2_selection,
                crate::test_budget::SLOW_FILTER,
                &entries[..],
            ),
        ] {
            let (passed, log) = run(selection, filter);
            assert!(passed, "{filter}: {log}");
            for entry in listed {
                assert!(
                    log.contains(&format!("{} {}", entry.binary, entry.test)),
                    "{filter}: {log}"
                );
            }
        }
        for (selection, filter) in [
            (&stage1_selection, &stage1_filter),
            (&stage2_selection, &stage2_filter),
        ] {
            let (passed, log) = run(selection, filter);
            assert!(passed, "{filter}: {log}");
            assert!(log.contains("botster-core slow_kept"), "{filter}: {log}");
            for entry in &entries {
                assert!(
                    !log.contains(&format!("{} {}", entry.binary, entry.test)),
                    "{filter}: {log}"
                );
            }
        }

        let found = crate::slow_coverage::check(
            dir.path(),
            &target,
            &packages,
            &BTreeSet::from(["conf::pending_id".to_string()]),
            &[entry("botster-core", "nosuch")],
            &[],
        )
        .unwrap();
        assert!(
            found.contains(&format!(
                "{FILE}: botster-core nosuch is in neither the default tier's nor the slow tier's test list: correct or \
                 remove the entry"
            )),
            "{found:?}"
        );
    }
}

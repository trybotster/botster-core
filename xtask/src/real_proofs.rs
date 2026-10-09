//! The real-only proofs of `conformance/real-proofs.toml` (plan revision 23, section 23a; lead ruling 2026-10-09).
//!
//! A Core id whose replacement-map proof is `slow:*` passes only on the real-process tier. The conformance runner runs it on
//! `TestkitHarness` today, so its passing run there proves nothing about the real behavior. Such an id may leave the pending
//! list only when the file names the real test that proves it, and that test exists and runs in the slow tier.
//!
//! One `[[proof]]` table per id: `id` (the Core id), `binary` (the test binary, the name of an integration test target) and
//! `test` (the test's nextest path inside that binary, `module::name` or `name`). The binary and the path are the key that
//! nextest and the slow filter use.
//!
//! Until `mutants_cited` (P6 PR B) is on v1, this module models the target and the tier itself: only an integration test
//! binary (`tests/<binary>.rs` or `tests/<binary>/main.rs`) is resolved. After PR B, `mutants_cited::tests` and `tiers_of`
//! replace the model, and a test of any target can be named.

use std::collections::{BTreeMap, BTreeSet};

/// The file, relative to the repository root.
pub const FILE: &str = "conformance/real-proofs.toml";

/// One entry of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub id: String,
    pub binary: String,
    pub test: String,
}

/// The entries of the file's `text`.
///
/// # Errors
/// The text is not TOML, or an entry lacks a string `id`, `binary` or `test`, or has another key.
pub fn parse(text: &str) -> Result<Vec<Proof>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("{FILE} is not TOML: {e}"))?;
    if let Some(key) = table.keys().find(|key| *key != "proof") {
        return Err(format!("{FILE}: unknown key `{key}`"));
    }
    let Some(entries) = table.get("proof") else {
        return Ok(Vec::new());
    };
    let entries = entries
        .as_array()
        .ok_or_else(|| format!("{FILE}: `proof` is not an array of tables"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry = entry
                .as_table()
                .ok_or_else(|| format!("{FILE}: proof {} is not a table", index + 1))?;
            if let Some(key) = entry
                .keys()
                .find(|key| !["id", "binary", "test"].contains(&key.as_str()))
            {
                return Err(format!(
                    "{FILE}: proof {} has the unknown key `{key}`",
                    index + 1
                ));
            }
            let field = |name: &str| {
                entry
                    .get(name)
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| format!("{FILE}: proof {} needs a string `{name}`", index + 1))
            };
            Ok(Proof {
                id: field("id")?,
                binary: field("binary")?,
                test: field("test")?,
            })
        })
        .collect()
}

/// Whether the test `test` of the binary `binary` runs in the slow tier: the nextest filter `test_budget::SLOW_FILTER`
/// selects a binary whose name starts with `slow` and a test with a `slow_` segment in its path.
fn in_slow_tier(binary: &str, test: &str) -> bool {
    binary.starts_with("slow") || test.split("::").any(|segment| segment.starts_with("slow_"))
}

/// The `tests/` directories (with the trailing `/`) of the crates that have the integration test binary `binary`.
fn crates_with_binary(binary: &str, sources: &BTreeMap<String, String>) -> Vec<String> {
    let roots = [
        format!("/tests/{binary}.rs"),
        format!("/tests/{binary}/main.rs"),
    ];
    sources
        .keys()
        .filter_map(|path| {
            roots
                .iter()
                .find_map(|root| path.strip_suffix(root.as_str()))
                .map(|krate| format!("{krate}/tests/"))
        })
        .collect()
}

/// The ids that the conformance runner runs: the ids of `ledger` that no list of `skipped` (pending, deferred, withdrawn)
/// holds.
pub fn running(ledger: &BTreeSet<String>, skipped: &[&BTreeSet<String>]) -> BTreeSet<String> {
    ledger
        .iter()
        .filter(|id| !skipped.iter().any(|list| list.contains(*id)))
        .cloned()
        .collect()
}

/// The inputs of [`verdict`].
pub struct Input<'a> {
    /// The ids that the conformance runner runs: the Core ids of the ledger that are not pending, deferred or withdrawn.
    pub running: &'a BTreeSet<String>,
    /// The Core ids of the ledger.
    pub ledger: &'a BTreeSet<String>,
    /// The replacement map's proof of each id (`slow:fsync`, `core-testkit`, ...).
    pub proof_of: &'a BTreeMap<String, String>,
    pub proofs: &'a [Proof],
    /// The text of each tracked Rust file, by its path from the repository root.
    pub sources: &'a BTreeMap<String, String>,
}

/// Every problem, one string each. Empty: the file is valid, and every running real-only id names its real test.
pub fn verdict(input: &Input<'_>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut named = BTreeSet::new();
    for proof in input.proofs {
        let id = &proof.id;
        if !named.insert(id.as_str()) {
            problems.push(format!("{FILE}: {id} is listed twice"));
        }
        if !input.ledger.contains(id) {
            problems.push(format!("{FILE}: {id} is not a Core id of the ledger"));
        } else if !input
            .proof_of
            .get(id)
            .is_some_and(|proof| proof.starts_with("slow:"))
        {
            problems.push(format!(
                "{FILE}: {id} has no `slow:*` proof in the replacement map; only a real-only id is listed"
            ));
        }
        let (binary, test) = (&proof.binary, &proof.test);
        let dirs = crates_with_binary(binary, input.sources);
        let name = test.rsplit("::").next().unwrap_or_default();
        let needle = format!("fn {name}(");
        let defined = input.sources.iter().any(|(path, text)| {
            dirs.iter().any(|dir| path.starts_with(dir.as_str())) && text.contains(&needle)
        });
        if dirs.is_empty() {
            problems.push(format!(
                "{FILE}: {id}: no integration test binary `{binary}` is tracked"
            ));
        } else if !defined {
            problems.push(format!(
                "{FILE}: {id}: the binary {binary} defines no test `{test}`"
            ));
        } else if !in_slow_tier(binary, test) {
            problems.push(format!(
                "{FILE}: {id}: the test {binary} {test} does not run in the slow tier"
            ));
        }
    }
    for id in input.running {
        let real_only = input
            .proof_of
            .get(id)
            .is_some_and(|proof| proof.starts_with("slow:"));
        if real_only && !named.contains(id.as_str()) {
            problems.push(format!(
                "{id} is not pending, and its replacement-map proof is `{}`: {FILE} must name its real slow-tier test",
                input.proof_of[id]
            ));
        }
    }
    problems
}

/// The replacement map's proof of each id, from the pinned `conformance/replacement-map.json`.
///
/// # Errors
/// The text is not the map's JSON shape.
pub fn proofs_of_map(json: &str) -> Result<BTreeMap<String, String>, String> {
    let map: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("replacement-map.json: {e}"))?;
    map["ids"]
        .as_array()
        .ok_or("replacement-map.json has no `ids` array")?
        .iter()
        .map(
            |entry| match (entry["id"].as_str(), entry["proof"].as_str()) {
                (Some(id), Some(proof)) => Ok((id.to_string(), proof.to_string())),
                _ => Err(format!(
                    "replacement-map.json: an entry lacks `id` or `proof`: {entry}"
                )),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    fn proof(id: &str, binary: &str, test: &str) -> Proof {
        Proof {
            id: id.into(),
            binary: binary.into(),
            test: test.into(),
        }
    }

    const TEST: &str = "the_real_lock_is_exclusive";

    fn sources() -> BTreeMap<String, String> {
        let test = format!("#[test]\nfn {TEST}() {{}}\n");
        map(&[
            ("crates/c/tests/slow_real.rs", "mod common;\n"),
            ("crates/c/tests/common/mod.rs", &test),
            ("crates/c/tests/fast.rs", &test),
            (
                "crates/c/tests/real/main.rs",
                &format!("mod slow_cases {{\n{test}}}\n"),
            ),
            ("crates/d/tests/slow_other.rs", ""),
            ("crates/d/src/lib.rs", &test),
        ])
    }

    fn verdict_of(running: &[&str], proofs: &[Proof]) -> Vec<String> {
        let ledger = ids(&["conf::a", "conf::b", "conf::c"]);
        let proof_of = map(&[
            ("conf::a", "slow:data-dir-lock"),
            ("conf::b", "core-testkit"),
            ("conf::c", "slow:fsync"),
        ]);
        verdict(&Input {
            running: &ids(running),
            ledger: &ledger,
            proof_of: &proof_of,
            proofs,
            sources: &sources(),
        })
    }

    /// The rule of the check: a running real-only id without its real test fails the step.
    #[test]
    fn a_running_real_only_id_without_its_real_test_is_a_problem() {
        assert_eq!(
            verdict_of(&["conf::a", "conf::b"], &[]),
            vec![format!(
                "conf::a is not pending, and its replacement-map proof is `slow:data-dir-lock`: {FILE} must name its real slow-tier test"
            )]
        );
    }

    #[test]
    fn a_running_real_only_id_with_its_slow_test_passes() {
        let in_module_file = [proof("conf::a", "slow_real", &format!("common::{TEST}"))];
        assert_eq!(
            verdict_of(&["conf::a", "conf::b"], &in_module_file),
            Vec::<String>::new()
        );
        let slow_module = [proof("conf::a", "real", &format!("slow_cases::{TEST}"))];
        assert_eq!(verdict_of(&["conf::a"], &slow_module), Vec::<String>::new());
    }

    /// An entry for a pending id is allowed: the real test can land before the id leaves the pending list.
    #[test]
    fn an_entry_for_a_pending_real_only_id_passes() {
        let named = [proof("conf::c", "slow_real", TEST)];
        assert_eq!(verdict_of(&[], &named), Vec::<String>::new());
    }

    #[test]
    fn a_named_test_that_is_missing_or_not_in_the_slow_tier_is_a_problem() {
        assert_eq!(
            verdict_of(
                &["conf::a"],
                &[proof("conf::a", "slow_real", "no_such_test")]
            ),
            vec![format!(
                "{FILE}: conf::a: the binary slow_real defines no test `no_such_test`"
            )]
        );
        // A name that only ends like the test is another test.
        let suffix = [proof("conf::a", "slow_real", "lock_is_exclusive")];
        assert_eq!(verdict_of(&["conf::a"], &suffix).len(), 1);
        // The test of another crate's file is not in the binary's crate.
        let other_crate = [proof("conf::a", "slow_other", TEST)];
        assert_eq!(
            verdict_of(&["conf::a"], &other_crate),
            vec![format!(
                "{FILE}: conf::a: the binary slow_other defines no test `{TEST}`"
            )]
        );
        for binary in ["slow_gone", "common", "c"] {
            assert_eq!(
                verdict_of(&["conf::a"], &[proof("conf::a", binary, TEST)]),
                vec![format!(
                    "{FILE}: conf::a: no integration test binary `{binary}` is tracked"
                )],
                "{binary}: a module file or a library is not an integration test binary"
            );
        }
        assert_eq!(
            verdict_of(&["conf::a"], &[proof("conf::a", "fast", TEST)]),
            vec![format!(
                "{FILE}: conf::a: the test fast {TEST} does not run in the slow tier"
            )]
        );
    }

    #[test]
    fn an_entry_for_an_unknown_id_a_testkit_id_or_a_second_time_is_a_problem() {
        assert_eq!(
            verdict_of(&[], &[proof("conf::x", "slow_real", TEST)]),
            vec![format!("{FILE}: conf::x is not a Core id of the ledger")]
        );
        assert_eq!(
            verdict_of(&[], &[proof("conf::b", "slow_real", TEST)]),
            vec![format!(
                "{FILE}: conf::b has no `slow:*` proof in the replacement map; only a real-only id is listed"
            )]
        );
        let twice = [
            proof("conf::a", "slow_real", TEST),
            proof("conf::a", "slow_real", TEST),
        ];
        assert_eq!(
            verdict_of(&["conf::a"], &twice),
            vec![format!("{FILE}: conf::a is listed twice")]
        );
    }

    #[test]
    fn the_running_ids_are_the_ledger_ids_that_no_skip_list_holds() {
        let ledger = ids(&["conf::a", "conf::b", "conf::c", "conf::d"]);
        let (pending, deferred) = (ids(&["conf::a"]), ids(&["conf::c", "conf::x"]));
        assert_eq!(
            running(&ledger, &[&pending, &deferred]),
            ids(&["conf::b", "conf::d"])
        );
        assert_eq!(running(&ledger, &[]), ledger);
    }

    /// `in_slow_tier` models this filter. A change of the filter must change the model too.
    #[test]
    fn the_model_follows_the_slow_filter() {
        assert_eq!(
            crate::test_budget::SLOW_FILTER,
            "binary(/^slow/) | test(/(^|::)slow_/)"
        );
    }

    #[test]
    fn the_slow_tier_is_a_slow_binary_or_a_slow_segment() {
        assert!(in_slow_tier("slow_real", "t"));
        assert!(in_slow_tier("slow", "t"));
        assert!(!in_slow_tier("real", "t"));
        assert!(in_slow_tier("real", "slow_tests::t"));
        assert!(in_slow_tier("real", "outer::slow_cases::t"));
        assert!(in_slow_tier("real", "slow_t"));
        assert!(!in_slow_tier("real", "tests::slowly"));
    }

    #[test]
    fn a_binary_is_a_tests_root_file_or_a_main_file() {
        let sources = sources();
        assert_eq!(
            crates_with_binary("slow_real", &sources),
            vec!["crates/c/tests/"]
        );
        assert_eq!(
            crates_with_binary("real", &sources),
            vec!["crates/c/tests/"]
        );
        assert_eq!(crates_with_binary("mod", &sources), Vec::<String>::new());
    }

    #[test]
    fn the_file_parses_its_entries_and_refuses_other_shapes() {
        assert_eq!(parse("# none yet\n").unwrap(), Vec::new());
        assert_eq!(
            parse("[[proof]]\nid = \"conf::a\"\nbinary = \"slow_b\"\ntest = \"m::t\"\n").unwrap(),
            vec![proof("conf::a", "slow_b", "m::t")]
        );
        for (text, error) in [
            ("x = 1\n", "unknown key `x`"),
            ("proof = 1\n", "`proof` is not an array of tables"),
            ("proof = [1]\n", "proof 1 is not a table"),
            (
                "[[proof]]\nid = \"a\"\nbinary = \"b\"\n",
                "proof 1 needs a string `test`",
            ),
            (
                "[[proof]]\nid = \"a\"\nfile = \"f\"\ntest = \"t\"\n",
                "proof 1 has the unknown key `file`",
            ),
            ("[[proof", "is not TOML"),
        ] {
            let got = parse(text).unwrap_err();
            assert!(got.contains(error), "{text:?}: {got}");
        }
    }

    #[test]
    fn the_map_gives_the_proof_of_each_id() {
        let json = r#"{"version":1,"ids":[{"id":"conf::a","proof":"slow:fsync","note":"n"},{"id":"conf::b","proof":"core-testkit"}]}"#;
        assert_eq!(
            proofs_of_map(json).unwrap(),
            map(&[("conf::a", "slow:fsync"), ("conf::b", "core-testkit")])
        );
        assert!(proofs_of_map("{}").unwrap_err().contains("no `ids` array"));
        assert!(proofs_of_map(r#"{"ids":[{"id":"conf::a"}]}"#)
            .unwrap_err()
            .contains("lacks `id` or `proof`"));
        assert!(proofs_of_map("[").is_err());
    }
}

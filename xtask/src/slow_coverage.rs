//! `cargo xtask slow-coverage`: every test that only the slow features build runs in the slow tier (lead, 2026-10-11: the
//! `mod tests` of `real/snapshot.rs` and `real/row_file.rs` exist only under the `slow` feature, and the slow filter does
//! not select them, so no tier ran them).
//!
//! The check lists the tests of both tiers with `cargo nextest list`, with the arguments of `test_budget::tier_selection`:
//! the default tier (the workspace, default features) and the slow tier (the slow packages, their `slow` features, the
//! slow filter). A slow-only test is a test of the slow list that the default list does not have (the same binary id and
//! test name). Each slow-only test must be one of these:
//! - a test that is not ignored, and that the slow filter selects;
//! - an ignored trial of the conformance binary `botster-core::slow_conformance`, named exactly `[<kind>] conf::<id>` with
//!   a kind of the real tier (`IGNORED_KINDS`, from `tests/suite/mod.rs`). The harness takes these kinds from the list
//!   files that the `lists` step checks. A `pending-real` trial must also have its `conf::<id>` in
//!   `conformance/core-real-pending.txt`: `test_budget::pending_real` runs those. A `real-only` trial must have its
//!   `conf::<id>` in `conformance/core-real-proofs.txt`: the gate counts it by its named proofs (plan 23s).
//!
//! Each line of `conformance/core-real-proofs.txt` names a test of the slow list that is not ignored and that the slow filter
//! selects, so that the slow tier runs it (lead, 2026-10-11; `real_proofs` then checks that it passed).
//!
//! Every other slow-only test is a violation, which names the binary and the test. The check fails closed: a list that it
//! cannot read, a list with no test, or no slow-only test at all (the slow features changed nothing) is an error.

use crate::real_proofs::FILE;
use anyhow::{bail, Context, Result};
use botster_core_testkit::pending::{parse_real_proofs, RealProof};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

/// The real tier's conformance binary: the only binary with ignored slow-only tests.
const PENDING_BINARY: &str = "botster-core::slow_conformance";

/// The kinds of the ignored trials of the real tier (`tests/suite/mod.rs`). Any other kind is a violation.
const IGNORED_KINDS: [&str; 7] = [
    "pending",
    "pending: no transcript",
    "pending-real",
    "withdrawn",
    "deferred",
    "testkit-proven",
    "real-only",
];

/// An ignored test that the real tier's conformance harness ignored by its lists: in the conformance binary, named
/// `[<kind>] conf::<id>` with a kind of `IGNORED_KINDS`; a `pending-real` trial has its `conf::<id>` in `pending`, and a
/// `real-only` trial in `proven` (`ignorable_real_only`).
fn listed_trial(
    binary: &str,
    name: &str,
    pending: &BTreeSet<String>,
    proven: &BTreeSet<String>,
) -> bool {
    let Some((kind, id)) = name
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    else {
        return false;
    };
    let id_form = id.strip_prefix("conf::").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    });
    binary == PENDING_BINARY
        && id_form
        && IGNORED_KINDS.contains(&kind)
        && (kind != "pending-real" || pending.contains(id))
        && (kind != "real-only" || proven.contains(id))
}

/// The real-only ids whose real-tier trial may be ignored: the ids of `core-real-proofs.txt` with no line that names the id's
/// own real-tier trial. The real tier runs that trial must-pass, so it must not be an ignored `[real-only]` trial (lead,
/// 2026-10-11, after R-55).
fn ignorable_real_only(proofs: &[RealProof]) -> BTreeSet<String> {
    proofs
        .iter()
        .filter(|proof| {
            !proofs
                .iter()
                .any(|own| own.id == proof.id && own.is_own_real_trial())
        })
        .map(|proof| proof.id.clone())
        .collect()
}

/// A test of a nextest list, by binary id and test name.
type Key = (String, String);

/// What a nextest list says of a test.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Listed {
    ignored: bool,
    /// The filter of the list selects the test (`filter-match` status `matches`).
    selected: bool,
}

/// A nextest list: every listed binary, also one with no test (review B306-I1), and the tests.
#[derive(Debug, PartialEq)]
struct List {
    binaries: BTreeSet<String>,
    tests: BTreeMap<Key, Listed>,
}

/// The binaries and tests of the JSON output of `cargo nextest list --message-format json`.
///
/// # Errors
/// The output is not such a list: a field is missing or has an unknown value, or a binary was not listed.
fn parse_list(json: &str) -> Result<List, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("the list is not JSON: {e}"))?;
    let suites = value["rust-suites"]
        .as_object()
        .ok_or("the list has no rust-suites")?;
    let binaries = suites.keys().cloned().collect();
    let mut tests = BTreeMap::new();
    for (binary, suite) in suites {
        if suite["status"] != "listed" {
            return Err(format!("{binary} was not listed: {}", suite["status"]));
        }
        let cases = suite["testcases"]
            .as_object()
            .ok_or_else(|| format!("{binary} has no testcases"))?;
        for (name, case) in cases {
            let ignored = case["ignored"]
                .as_bool()
                .ok_or_else(|| format!("{binary} {name} has no ignored flag"))?;
            let selected = match case["filter-match"]["status"].as_str() {
                Some("matches") => true,
                Some("mismatch") => false,
                other => return Err(format!("{binary} {name} has the filter status {other:?}")),
            };
            tests.insert((binary.clone(), name.clone()), Listed { ignored, selected });
        }
    }
    Ok(List { binaries, tests })
}

/// The slow-only tests of the slow list that no tier runs, each with the reason. `pending` holds the pending-real ids, and
/// `proven` the ids with a named real-process proof.
fn violations(
    default: &BTreeMap<Key, Listed>,
    slow: &BTreeMap<Key, Listed>,
    pending: &BTreeSet<String>,
    proven: &BTreeSet<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for ((binary, name), listed) in slow {
        if default.contains_key(&(binary.clone(), name.clone())) {
            continue;
        }
        if listed.ignored {
            if !listed_trial(binary, name, pending, proven) {
                found.push(format!(
                    "{binary} {name}: only the slow features build it, and it is ignored; only a listed trial of \
                     {PENDING_BINARY} may be ignored there"
                ));
            }
        } else if !listed.selected {
            found.push(format!(
                "{binary} {name}: only the slow features build it, and the slow filter does not select it, so no tier \
                 runs it"
            ));
        }
    }
    found
}

/// The proof lines whose test the slow tier does not run: not in the slow list, ignored, or not selected by the slow filter.
fn proof_violations(slow: &BTreeMap<Key, Listed>, proofs: &[RealProof]) -> Vec<String> {
    proofs
        .iter()
        .filter_map(|proof| {
            let line = format!("{FILE}: {} {} {}", proof.id, proof.binary, proof.test);
            match slow.get(&(proof.binary.clone(), proof.test.clone())) {
                None => Some(format!("{line}: the slow tier has no such test")),
                Some(listed) if listed.ignored => {
                    Some(format!("{line}: the test is ignored, so the slow tier does not run it"))
                }
                Some(listed) if !listed.selected => Some(format!(
                    "{line}: the slow filter does not select the test, so the slow tier does not run it"
                )),
                Some(_) => None,
            }
        })
        .collect()
}

/// The number of slow-only tests: the tests of the slow list that the default list does not have.
fn slow_only(default: &BTreeMap<Key, Listed>, slow: &BTreeMap<Key, Listed>) -> usize {
    slow.keys()
        .filter(|key| !default.contains_key(*key))
        .count()
}

/// The binaries of `list`, the list of the tier named `tier`, that do not run the build-root test (bug catalog B13; design
/// `design-gs-built-in-coverage.md`): each binary of the list must list `test_budget::BUILT_IN_TEST`, not
/// ignored and selected by the tier's filter. The list comes from cargo and nextest, so a binary that the tier builds
/// cannot be left out, and a `cfg` that removes the test on one OS fails that OS's list. The binaries come from the list's
/// suites, not from its tests: a binary with no test at all fails too (review B306-I1).
fn presence(tier: &str, list: &List) -> Vec<String> {
    use crate::test_budget::BUILT_IN_TEST as TEST_NAME;
    let mut found = Vec::new();
    for binary in &list.binaries {
        let why = match list.tests.get(&(binary.clone(), TEST_NAME.to_string())) {
            None => "does not list",
            Some(Listed { ignored: true, .. }) => "ignores",
            Some(Listed {
                selected: false, ..
            }) => "does not select",
            Some(_) => continue,
        };
        found.push(format!(
            "{tier} tier: {binary} {why} the build-root test `{TEST_NAME}`: add `botster_test_process::built_in_test!();` \
             (or `built_in_trial!()` with its own harness) (bug catalog B13)"
        ));
    }
    found
}

/// The tests that `cargo nextest list <selection>` lists in `root`, built in `target`.
fn list(root: &Path, target: &Path, selection: &[String]) -> Result<List> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            "nextest",
            "list",
            "--message-format",
            "json",
            "--target-dir",
        ])
        .arg(target)
        .args(selection)
        // The default profile: a run under nextest (the slow tier, a mutation run) sets the variable to its own profile,
        // which another workspace need not have. The profile does not change the list: no profile sets a default filter.
        .env_remove("NEXTEST_PROFILE");
    let output = crate::caps::apply(&mut command)
        .output()
        .context("start cargo nextest list")?;
    let json = crate::tools::stdout_of(&output, "cargo nextest list")?;
    let list = parse_list(&json).map_err(anyhow::Error::msg)?;
    if list.tests.is_empty() {
        bail!("cargo nextest list {} lists no test", selection.join(" "));
    }
    Ok(list)
}

/// The tests (binary id and name) that `cargo nextest list <selection>` lists in `root`, built in `target`: a mutation
/// stage removes the known-flaky tests of its own list (`known_flaky::stage_filter`).
///
/// # Errors
/// The list fails or lists no test.
pub fn tests(
    root: &Path,
    target: &Path,
    selection: &[String],
) -> Result<crate::known_flaky::Tests> {
    Ok(list(root, target, selection)?.tests.into_keys().collect())
}

/// The violations of the workspace at `root`, built in `target`, whose members with a `slow` feature are `slow_packages`;
/// `pending` holds the pending-real ids, and `proofs` the named real-process proofs. A known-flaky entry of `flaky` that
/// neither tier's list has is a violation too (`known_flaky::absent`).
///
/// # Errors
/// A list fails, or no test is slow-only.
pub fn check(
    root: &Path,
    target: &Path,
    slow_packages: &[String],
    pending: &BTreeSet<String>,
    flaky: &[crate::known_flaky::Entry],
    proofs: &[RealProof],
) -> Result<Vec<String>> {
    let default_list = list(
        root,
        target,
        &crate::test_budget::tier_selection(false, slow_packages),
    )?;
    let slow_list = list(
        root,
        target,
        &crate::test_budget::tier_selection(true, slow_packages),
    )?;
    let (default, slow) = (&default_list.tests, &slow_list.tests);
    let count = slow_only(default, slow);
    if count == 0 {
        bail!("no test is slow-only: the slow features build no test that the default features do not");
    }
    println!("slow-coverage: {count} slow-only tests checked");
    let mut found = violations(default, slow, pending, &ignorable_real_only(proofs));
    found.extend(proof_violations(slow, proofs));
    found.extend(presence("default", &default_list));
    found.extend(presence("slow", &slow_list));
    let keys = |tests: &BTreeMap<Key, Listed>| -> crate::known_flaky::Tests {
        tests.keys().cloned().collect()
    };
    found.extend(crate::known_flaky::absent(
        flaky,
        &keys(default),
        &keys(slow),
    ));
    Ok(found)
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let meta = crate::fsutil::metadata(root)?;
    let text = std::fs::read_to_string(root.join(crate::lists::REAL_PENDING_FILE))
        .with_context(|| format!("read {}", crate::lists::REAL_PENDING_FILE))?;
    let pending = crate::lists::parse_ids(&text).map_err(anyhow::Error::msg)?;
    let flaky = crate::known_flaky::parse(
        &std::fs::read_to_string(root.join(crate::known_flaky::FILE))
            .with_context(|| format!("read {}", crate::known_flaky::FILE))?,
    )
    .map_err(anyhow::Error::msg)?;
    let proofs = parse_real_proofs(
        &std::fs::read_to_string(root.join(FILE)).with_context(|| format!("read {FILE}"))?,
    )
    .map_err(|e| anyhow::anyhow!("{FILE}: {e}"))?;
    crate::tools::verdict(&check(
        root,
        &meta.target_dir,
        &meta.slow_packages,
        &pending,
        &flaky,
        &proofs,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(binary: &str, name: &str) -> Key {
        (binary.to_string(), name.to_string())
    }

    fn tests(rows: &[(&str, &str, bool, bool)]) -> BTreeMap<Key, Listed> {
        rows.iter()
            .map(|(binary, name, ignored, selected)| {
                (
                    key(binary, name),
                    Listed {
                        ignored: *ignored,
                        selected: *selected,
                    },
                )
            })
            .collect()
    }

    fn ids(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    /// The form of cargo-nextest 0.9.146.
    const LIST: &str = r#"{"test-count": 3, "rust-suites": {
        "a": {"binary-id": "a", "status": "listed", "testcases": {
            "plain": {"kind": "test", "ignored": false, "filter-match": {"status": "mismatch", "reason": "expression"}},
            "tests::ig": {"kind": "test", "ignored": true, "filter-match": {"status": "mismatch", "reason": "ignored"}}}},
        "a::slow_x": {"binary-id": "a::slow_x", "status": "listed", "testcases": {
            "s": {"kind": "test", "ignored": false, "filter-match": {"status": "matches"}}}}}}"#;

    #[test]
    fn a_list_gives_each_test_its_ignored_flag_and_whether_the_filter_selects_it() {
        assert_eq!(
            parse_list(LIST).map(|list| list.tests),
            Ok(tests(&[
                ("a", "plain", false, false),
                ("a", "tests::ig", true, false),
                ("a::slow_x", "s", false, true),
            ]))
        );
    }

    /// Red on revert of the slow filter's last term (lead, 2026-10-12): a slow-tier artifact whose build-root test the filter
    /// does not select, or that lists no such test, fails; so does an ignored one. A list where every binary runs it passes.
    #[test]
    fn every_binary_of_a_tier_must_run_the_build_root_test() {
        use crate::test_budget::BUILT_IN_TEST as TEST_NAME;
        let listed = |tests: BTreeMap<Key, Listed>| List {
            binaries: tests.keys().map(|(binary, _)| binary.clone()).collect(),
            tests,
        };
        let good = listed(tests(&[
            ("a", "plain", false, false),
            ("a", TEST_NAME, false, true),
            ("a::slow_x", "s", false, true),
            ("a::slow_x", TEST_NAME, false, true),
        ]));
        assert_eq!(presence("slow", &good), Vec::<String>::new());
        let nested = format!("tests::{TEST_NAME}");
        let bad = listed(tests(&[
            // The binary of the library, built again with the slow features: the old filter did not select its test.
            ("a", TEST_NAME, false, false),
            ("a::slow_x", "s", false, true),
            ("a::slow_y", TEST_NAME, true, true),
            ("a::slow_z", &nested, false, true),
        ]));
        let found = presence("slow", &bad);
        let starts: Vec<&str> = found
            .iter()
            .map(|line| line.split(" the build-root").next().unwrap())
            .collect();
        assert_eq!(
            starts,
            [
                "slow tier: a does not select",
                "slow tier: a::slow_x does not list",
                "slow tier: a::slow_y ignores",
                // A test of that name inside a module is not the root test.
                "slow tier: a::slow_z does not list",
            ]
        );
        assert!(
            found[0].contains("botster_test_process::built_in_test!();"),
            "{}",
            found[0]
        );
    }

    /// Review B306-I1: a listed binary with no test (its only test was the build-root test, and the macro line was removed)
    /// stays in the list and fails the presence check; the valid binary beside it passes.
    #[test]
    fn a_listed_binary_with_no_test_fails_the_presence_check() {
        use crate::test_budget::BUILT_IN_TEST as TEST_NAME;
        let json = format!(
            r#"{{"rust-suites": {{
                "anchor": {{"binary-id": "anchor", "status": "listed", "testcases": {{}}}},
                "b": {{"binary-id": "b", "status": "listed", "testcases": {{
                    "{TEST_NAME}": {{"kind": "test", "ignored": false, "filter-match": {{"status": "matches"}}}}}}}}}}}}"#
        );
        let list = parse_list(&json).unwrap();
        assert_eq!(list.binaries, ids(&["anchor", "b"]));
        let found = presence("default", &list);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("default tier: anchor does not list the build-root test"),
            "{}",
            found[0]
        );
    }

    #[test]
    fn a_list_of_an_unknown_form_is_an_error() {
        let cases = [
            ("{}", "the list has no rust-suites"),
            ("[", "the list is not JSON"),
            (
                r#"{"rust-suites": {"a": {"status": "skipped", "testcases": {}}}}"#,
                "a was not listed",
            ),
            (
                r#"{"rust-suites": {"a": {"status": "listed"}}}"#,
                "a has no testcases",
            ),
            (
                r#"{"rust-suites": {"a": {"status": "listed", "testcases": {"t": {"filter-match": {"status": "matches"}}}}}}"#,
                "a t has no ignored flag",
            ),
            (
                r#"{"rust-suites": {"a": {"status": "listed", "testcases": {"t": {"ignored": false, "filter-match": {"status": "maybe"}}}}}}"#,
                "a t has the filter status Some(\"maybe\")",
            ),
            (
                r#"{"rust-suites": {"a": {"status": "listed", "testcases": {"t": {"ignored": false}}}}}"#,
                "a t has the filter status None",
            ),
        ];
        for (json, error) in cases {
            match parse_list(json) {
                Err(text) => assert!(text.starts_with(error), "{json}: {text}"),
                Ok(list) => panic!("{json}: {list:?}"),
            }
        }
    }

    /// A slow-only test that the filter does not select fails; a selected one passes; a test that the default tier also
    /// lists is not slow-only, whatever the slow list says of it.
    #[test]
    fn a_slow_only_test_outside_the_slow_filter_is_a_violation() {
        let default = tests(&[("a", "shared", false, false)]);
        let slow = tests(&[
            ("a", "shared", false, false),
            ("a", "tests::unrun", false, false),
            ("a", "slow_tests::run", false, true),
            ("a::slow_x", "s", false, true),
        ]);
        assert_eq!(
            violations(&default, &slow, &ids(&[]), &ids(&[])),
            ["a tests::unrun: only the slow features build it, and the slow filter does not select it, so no tier runs it"]
        );
        assert_eq!(slow_only(&default, &slow), 3);
    }

    /// The violation line of an ignored slow-only test.
    fn ignored(binary: &str, name: &str) -> String {
        format!(
            "{binary} {name}: only the slow features build it, and it is ignored; only a listed trial of \
             botster-core::slow_conformance may be ignored there"
        )
    }

    /// The violations of a slow list with one ignored slow-only test, which the filter selects; `pending` is
    /// `conf::a1_1_x`.
    fn one_ignored(binary: &str, name: &str) -> Vec<String> {
        let slow = tests(&[(binary, name, true, true)]);
        let id = ids(&["conf::a1_1_x"]);
        violations(&BTreeMap::new(), &slow, &id, &id)
    }

    /// Each kind of the real tier passes in the conformance binary, and a pending-real trial passes only with its full
    /// `conf::<id>` in the pending-real file (the last segment alone is not the id).
    #[test]
    fn a_listed_trial_of_each_kind_passes_and_a_pending_real_id_must_be_in_the_file() {
        for kind in IGNORED_KINDS {
            let name = format!("[{kind}] conf::a1_1_x");
            assert_eq!(
                one_ignored(PENDING_BINARY, &name),
                Vec::<String>::new(),
                "{kind}"
            );
        }
        let slow = tests(&[(PENDING_BINARY, "[pending-real] conf::a1_1_x", true, false)]);
        for pending in [&[][..], &["a1_1_x"], &["conf::a1_1_y"]] {
            assert_eq!(
                violations(
                    &BTreeMap::new(),
                    &slow,
                    &ids(pending),
                    &ids(&["conf::a1_1_x"])
                ),
                [ignored(PENDING_BINARY, "[pending-real] conf::a1_1_x")],
                "{pending:?}"
            );
        }
        // A real-only trial passes only with its full id among the ids that have a named proof.
        let slow = tests(&[(PENDING_BINARY, "[real-only] conf::a1_1_x", true, false)]);
        for proven in [&[][..], &["a1_1_x"], &["conf::a1_1_y"]] {
            assert_eq!(
                violations(
                    &BTreeMap::new(),
                    &slow,
                    &ids(&["conf::a1_1_x"]),
                    &ids(proven)
                ),
                [ignored(PENDING_BINARY, "[real-only] conf::a1_1_x")],
                "{proven:?}"
            );
        }
    }

    /// An id whose proof line names its own real-tier trial is not ignorable: the real tier runs that trial. An id with only
    /// separate proofs is.
    #[test]
    fn an_id_proved_by_its_own_real_tier_trial_is_not_ignorable() {
        let line = |id: &str, binary: &str, test: &str| RealProof {
            id: id.into(),
            binary: binary.into(),
            test: test.into(),
        };
        let own = "botster-core::slow_conformance";
        let proofs = [
            line("conf::a1_1_x", "a::slow_x", "t"),
            line("conf::a1_1_y", "a::slow_x", "t"),
            line("conf::a1_1_y", own, "conf::a1_1_y"),
            line("conf::a1_1_z", own, "conf::a1_1_z"),
        ];
        assert_eq!(ignorable_real_only(&proofs), ids(&["conf::a1_1_x"]));
        // So the real tier's ignored real-only trial of an own-trial id is a violation, and its trial must run.
        let slow = tests(&[(PENDING_BINARY, "[real-only] conf::a1_1_y", true, false)]);
        assert_eq!(
            violations(
                &BTreeMap::new(),
                &slow,
                &BTreeSet::new(),
                &ignorable_real_only(&proofs)
            ),
            [ignored(PENDING_BINARY, "[real-only] conf::a1_1_y")]
        );
        let slow = tests(&[(own, "conf::a1_1_y", true, true)]);
        assert_eq!(
            proof_violations(&slow, &proofs[2..3]),
            ["conformance/core-real-proofs.txt: conf::a1_1_y botster-core::slow_conformance conf::a1_1_y: the test is ignored, so the slow tier does not run it"]
        );
        let slow = tests(&[(own, "conf::a1_1_y", false, true)]);
        assert_eq!(proof_violations(&slow, &proofs[2..3]), Vec::<String>::new());
    }

    /// A proof line passes only when it names a test of the slow list that is not ignored and that the filter selects.
    #[test]
    fn a_proof_names_a_test_that_the_slow_tier_runs() {
        let proof = |binary: &str, test: &str| RealProof {
            id: "conf::a1_1_x".into(),
            binary: binary.into(),
            test: test.into(),
        };
        let slow = tests(&[
            ("a::slow_x", "runs", false, true),
            ("a::slow_x", "ignored", true, true),
            ("a::slow_x", "unselected", false, false),
        ]);
        assert_eq!(
            proof_violations(&slow, &[proof("a::slow_x", "runs")]),
            Vec::<String>::new()
        );
        let line = |test: &str, why: &str| {
            format!("conformance/core-real-proofs.txt: conf::a1_1_x a::slow_x {test}: {why}")
        };
        assert_eq!(
            proof_violations(
                &slow,
                &[
                    proof("a::slow_x", "ignored"),
                    proof("a::slow_x", "unselected"),
                    proof("a::slow_x", "absent"),
                    proof("a::slow_y", "runs"),
                ]
            ),
            [
                line("ignored", "the test is ignored, so the slow tier does not run it"),
                line(
                    "unselected",
                    "the slow filter does not select the test, so the slow tier does not run it"
                ),
                line("absent", "the slow tier has no such test"),
                "conformance/core-real-proofs.txt: conf::a1_1_x a::slow_y runs: the slow tier has no such test"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn an_ignored_trial_of_another_binary_is_a_violation() {
        for binary in [
            "a::slow_x",
            "botster-core::conformance",
            "botster-core::slow_conformance_x",
        ] {
            assert_eq!(
                one_ignored(binary, "[pending] conf::a1_1_x"),
                [ignored(binary, "[pending] conf::a1_1_x")]
            );
        }
    }

    #[test]
    fn a_plain_ignored_test_is_a_violation() {
        for name in ["conf::a1_1_x", "tests::slow_ignored", "a1_1_x"] {
            assert_eq!(
                one_ignored(PENDING_BINARY, name),
                [ignored(PENDING_BINARY, name)]
            );
        }
    }

    #[test]
    fn an_ignored_trial_of_an_unknown_kind_is_a_violation() {
        for kind in ["real-only: x", "pending: strict", "Pending", "pending ", ""] {
            let name = format!("[{kind}] conf::a1_1_x");
            assert_eq!(
                one_ignored(PENDING_BINARY, &name),
                [ignored(PENDING_BINARY, &name)]
            );
        }
    }

    #[test]
    fn a_malformed_trial_name_is_a_violation() {
        for name in [
            "[pending] a1_1_x",
            "[pending] conf::",
            "[pending] conf::a1 x",
            "[pending] conf::a1_1_x ",
            "[pending] conf::A1_1_x",
            "[pending]conf::a1_1_x",
            "pending] conf::a1_1_x",
            "[pending] [pending] conf::a1_1_x",
        ] {
            assert_eq!(
                one_ignored(PENDING_BINARY, name),
                [ignored(PENDING_BINARY, name)]
            );
        }
    }
}

/// The red-on-revert proof on real `cargo nextest list` runs of the fixture `xtask/fixtures/slow-coverage` (slow tier: it
/// starts cargo).
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;

    /// The fixture's package is `botster-core`, with a `slow` feature and the test target `slow_conformance`. Its library
    /// has a `mod tests` that only the slow feature builds: the slow filter does not select it, and the check names it.
    /// Renamed `slow_tests`, the filter selects it. The fixture's `slow_conformance` lists the ignored trial
    /// `[pending-real] conf::pending_id` as the real harness names it; it passes only while `conf::pending_id` is pending-real.
    #[test]
    fn a_slow_only_test_outside_the_filter_or_an_ignored_one_not_pending_fails() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the repository");
        let fixture = root.join("xtask/fixtures/slow-coverage");
        let dir = tempfile::tempdir().unwrap();
        for sub in ["src", "tests"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        for (from, to) in [
            ("Cargo.toml.in", "Cargo.toml"),
            ("src/lib.rs", "src/lib.rs"),
            ("tests/slow_conformance.rs", "tests/slow_conformance.rs"),
        ] {
            std::fs::copy(fixture.join(from), dir.path().join(to)).unwrap();
        }
        // Its own target directory: a shared one can hold a build of an earlier copy whose files are newer than the copied
        // fixture, and cargo would reuse that build.
        let target = dir.path().join("target");
        let packages = ["botster-core".to_string()];
        let pending = ids(&["conf::pending_id"]);
        let unrun = "botster-core tests::runs_only_under_slow: only the slow features build it, and the slow filter \
                     does not select it, so no tier runs it";
        assert_eq!(
            check(dir.path(), &target, &packages, &pending, &[], &[]).unwrap(),
            [unrun]
        );

        let lib = dir.path().join("src/lib.rs");
        let source = std::fs::read_to_string(&lib).unwrap();
        assert_eq!(source.matches("mod tests").count(), 1);
        // A proof line that names the unselected test is a violation as well.
        let proof = |test: &str| RealProof {
            id: "conf::proven".into(),
            binary: "botster-core".into(),
            test: test.into(),
        };
        let unselected = check(
            dir.path(),
            &target,
            &packages,
            &pending,
            &[],
            &[proof("tests::runs_only_under_slow")],
        )
        .unwrap();
        assert_eq!(unselected.len(), 2, "{unselected:?}");
        assert!(unselected[1].ends_with(
            "the slow filter does not select the test, so the slow tier does not run it"
        ));
        std::fs::write(&lib, source.replace("mod tests", "mod slow_tests")).unwrap();
        assert_eq!(
            check(dir.path(), &target, &packages, &pending, &[], &[]).unwrap(),
            Vec::<String>::new()
        );
        // Renamed, the filter selects it, so a proof line may name it.
        assert_eq!(
            check(
                dir.path(),
                &target,
                &packages,
                &pending,
                &[],
                &[proof("slow_tests::runs_only_under_slow")]
            )
            .unwrap(),
            Vec::<String>::new()
        );

        assert_eq!(
            check(dir.path(), &target, &packages, &ids(&[]), &[], &[]).unwrap(),
            [
                "botster-core::slow_conformance [pending-real] conf::pending_id: only the slow features build it, and it \
                 is ignored; only a listed trial of botster-core::slow_conformance may be ignored there"
            ]
        );
    }

    fn ids(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|id| id.to_string()).collect()
    }
}

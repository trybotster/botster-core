//! `cargo xtask slow-coverage`: every test that only the slow features build runs in the slow tier (lead, 2026-10-11: the
//! `mod tests` of `real/snapshot.rs` and `real/row_file.rs` exist only under the `slow` feature, and the slow filter does
//! not select them, so no tier ran them).
//!
//! The check lists the tests of both tiers with `cargo nextest list`, with the arguments of `test_budget::tier_selection`:
//! the default tier (the workspace, default features) and the slow tier (the slow packages, their `slow` features, the
//! slow filter). A slow-only test is a test of the slow list that the default list does not have (the same binary id and
//! test name). Each slow-only test must be one of these:
//! - a test that is not ignored, and that the slow filter selects;
//! - an ignored test of the conformance binary `botster-core::slow_conformance` whose id (the last `::` segment of its
//!   name) is in `conformance/core-real-pending.txt`: the pending-real trials, which `test_budget::pending_real` runs.
//!
//! Every other slow-only test is a violation, which names the binary and the test. The check fails closed: a list that it
//! cannot read, a list with no test, or no slow-only test at all (the slow features changed nothing) is an error.

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

/// The binary whose ignored tests are the pending-real trials.
const PENDING_BINARY: &str = "botster-core::slow_conformance";

/// A test of a nextest list, by binary id and test name.
type Key = (String, String);

/// What a nextest list says of a test.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Listed {
    ignored: bool,
    /// The filter of the list selects the test (`filter-match` status `matches`).
    selected: bool,
}

/// The tests of the JSON output of `cargo nextest list --message-format json`.
///
/// # Errors
/// The output is not such a list: a field is missing or has an unknown value, or a binary was not listed.
fn parse_list(json: &str) -> Result<BTreeMap<Key, Listed>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("the list is not JSON: {e}"))?;
    let suites = value["rust-suites"]
        .as_object()
        .ok_or("the list has no rust-suites")?;
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
    Ok(tests)
}

/// The slow-only tests of the slow list that no tier runs, each with the reason. `pending` holds the pending-real ids.
fn violations(
    default: &BTreeMap<Key, Listed>,
    slow: &BTreeMap<Key, Listed>,
    pending: &BTreeSet<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for ((binary, name), listed) in slow {
        if default.contains_key(&(binary.clone(), name.clone())) {
            continue;
        }
        let id = name.rsplit("::").next().unwrap_or(name);
        if listed.ignored {
            if binary != PENDING_BINARY || !pending.contains(id) {
                found.push(format!(
                    "{binary} {name}: only the slow features build it, and it is ignored; only a pending-real id of \
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

/// The number of slow-only tests: the tests of the slow list that the default list does not have.
fn slow_only(default: &BTreeMap<Key, Listed>, slow: &BTreeMap<Key, Listed>) -> usize {
    slow.keys()
        .filter(|key| !default.contains_key(*key))
        .count()
}

/// The tests that `cargo nextest list <selection>` lists in `root`, built in `target`.
fn list(root: &Path, target: &Path, selection: &[String]) -> Result<BTreeMap<Key, Listed>> {
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
        .args(selection);
    let output = crate::caps::apply(&mut command)
        .output()
        .context("start cargo nextest list")?;
    let json = crate::tools::stdout_of(&output, "cargo nextest list")?;
    let tests = parse_list(&json).map_err(anyhow::Error::msg)?;
    if tests.is_empty() {
        bail!("cargo nextest list {} lists no test", selection.join(" "));
    }
    Ok(tests)
}

/// The violations of the workspace at `root`, built in `target`, whose members with a `slow` feature are `slow_packages`;
/// `pending` holds the pending-real ids.
///
/// # Errors
/// A list fails, or no test is slow-only.
pub fn check(
    root: &Path,
    target: &Path,
    slow_packages: &[String],
    pending: &BTreeSet<String>,
) -> Result<Vec<String>> {
    let default = list(
        root,
        target,
        &crate::test_budget::tier_selection(false, slow_packages),
    )?;
    let slow = list(
        root,
        target,
        &crate::test_budget::tier_selection(true, slow_packages),
    )?;
    let count = slow_only(&default, &slow);
    if count == 0 {
        bail!("no test is slow-only: the slow features build no test that the default features do not");
    }
    println!("slow-coverage: {count} slow-only tests checked");
    Ok(violations(&default, &slow, pending))
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let meta = crate::fsutil::metadata(root)?;
    let text = std::fs::read_to_string(root.join(crate::lists::REAL_PENDING_FILE))
        .with_context(|| format!("read {}", crate::lists::REAL_PENDING_FILE))?;
    let pending = crate::lists::parse_ids(&text).map_err(anyhow::Error::msg)?;
    crate::tools::verdict(&check(
        root,
        &meta.target_dir,
        &meta.slow_packages,
        &pending,
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
            parse_list(LIST),
            Ok(tests(&[
                ("a", "plain", false, false),
                ("a", "tests::ig", true, false),
                ("a::slow_x", "s", false, true),
            ]))
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
            violations(&default, &slow, &ids(&[])),
            ["a tests::unrun: only the slow features build it, and the slow filter does not select it, so no tier runs it"]
        );
        assert_eq!(slow_only(&default, &slow), 3);
    }

    /// An ignored slow-only test passes only in the conformance binary with a pending-real id: not in another binary, not
    /// with an id that is not pending, and not selected by the filter instead.
    #[test]
    fn an_ignored_slow_only_test_passes_only_as_a_pending_real_trial() {
        let default = tests(&[("a", "shared", true, false)]);
        let slow = tests(&[
            ("a", "shared", true, false),
            (PENDING_BINARY, "conf::pending_id", true, false),
            (PENDING_BINARY, "conf::other_id", true, true),
            ("a::slow_x", "conf::pending_id", true, true),
        ]);
        let ignored = |binary: &str, name: &str| {
            format!(
                "{binary} {name}: only the slow features build it, and it is ignored; only a pending-real id of \
                 botster-core::slow_conformance may be ignored there"
            )
        };
        assert_eq!(
            violations(&default, &slow, &ids(&["pending_id"])),
            [
                ignored("a::slow_x", "conf::pending_id"),
                ignored(PENDING_BINARY, "conf::other_id"),
            ]
        );
        assert_eq!(
            violations(&default, &slow, &ids(&["pending_id", "other_id"])),
            [ignored("a::slow_x", "conf::pending_id")]
        );
    }
}

/// The red-on-revert proof on real `cargo nextest list` runs of the fixture `xtask/fixtures/slow-coverage` (slow tier: it
/// starts cargo).
#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    use super::*;

    /// The fixture's package is `botster-core`, with a `slow` feature and the test target `slow_conformance`. Its library
    /// has a `mod tests` that only the slow feature builds: the slow filter does not select it, and the check names it.
    /// Renamed `slow_tests`, the filter selects it. The ignored test `conf::pending_id` of `slow_conformance` passes only
    /// while its id is pending-real.
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
        let pending = ids(&["pending_id"]);
        let unrun = "botster-core tests::runs_only_under_slow: only the slow features build it, and the slow filter \
                     does not select it, so no tier runs it";
        assert_eq!(
            check(dir.path(), &target, &packages, &pending).unwrap(),
            [unrun]
        );

        let lib = dir.path().join("src/lib.rs");
        let source = std::fs::read_to_string(&lib).unwrap();
        assert_eq!(source.matches("mod tests").count(), 1);
        std::fs::write(&lib, source.replace("mod tests", "mod slow_tests")).unwrap();
        assert_eq!(
            check(dir.path(), &target, &packages, &pending).unwrap(),
            Vec::<String>::new()
        );

        assert_eq!(
            check(dir.path(), &target, &packages, &ids(&[])).unwrap(),
            [
                "botster-core::slow_conformance conf::pending_id: only the slow features build it, and it is ignored; \
                 only a pending-real id of botster-core::slow_conformance may be ignored there"
            ]
        );
    }

    fn ids(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|id| id.to_string()).collect()
    }
}

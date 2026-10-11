//! The named real-process proofs of the real-only Core ids passed in the slow step (plan sections 1 (23s), 5 and 8; lead,
//! 2026-10-11). `conformance/core-real-proofs.txt` names, for each real-only id that left `core-pending.txt`, the slow-tier
//! tests that prove it; the suite counts the id by them and never runs its transcript. So the slow step must have run each
//! named test to PASS: after the slow tier, this check reads the tier's own JUnit file (`test_budget` deletes it before the
//! run, so it is never a stale one) and fails on a line whose test has no test case there, or a test case that did not pass.
//!
//! The JUnit file is nextest's, read as an XML tree: a well-formed document whose root is `<testsuites>`, with a
//! `<testcase>` per run test in a `<testsuite>` under the root; `classname` is the binary id and `name` the test name. A
//! test case passed when every element in it is `system-out`, `system-err`, `properties` or `property`. Any other element
//! (`failure`, `error`, `skipped`, `flakyFailure`, `rerunFailure`, ... or an element this check does not know) is not a
//! pass. A test that appears twice passes only when every test case passed. Text in a comment or in a CDATA section is
//! never a test case. A file that this check cannot read as such a file proves nothing: it is an error (Sol, #264
//! RP-SI-1).

use anyhow::{Context, Result};
use botster_core_testkit::pending::{parse_real_proofs, RealProof};
use std::collections::BTreeMap;
use std::path::Path;

/// The proof file.
pub const FILE: &str = "conformance/core-real-proofs.txt";

/// The elements in a test case that passed: its output and its properties.
const PASS_ELEMENTS: [&str; 4] = ["system-out", "system-err", "properties", "property"];

/// A test of a JUnit file, by binary id and test name.
type Key = (String, String);

/// The test cases of a nextest JUnit file: for each `(classname, name)`, the elements of each of its test cases that are
/// not a pass (empty: that test case passed).
///
/// # Errors
/// The text is not such a file: it is not well-formed XML, its root is not `<testsuites>`, a test case is not in a
/// `<testsuite>` under the root, or a test case has no `name` or `classname`.
fn results(xml: &str) -> Result<BTreeMap<Key, Vec<Vec<String>>>, String> {
    let document = roxmltree::Document::parse(xml)
        .map_err(|error| format!("the JUnit file is not well-formed XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "testsuites" {
        return Err(format!(
            "the root element of the JUnit file is <{}>, not <testsuites>",
            root.tag_name().name()
        ));
    }
    let mut cases: BTreeMap<Key, Vec<Vec<String>>> = BTreeMap::new();
    for case in root
        .descendants()
        .filter(|node| node.tag_name().name() == "testcase")
    {
        let suite = case.parent_element();
        if !suite.is_some_and(|suite| {
            suite.tag_name().name() == "testsuite" && suite.parent_element() == Some(root)
        }) {
            return Err(format!(
                "a test case is not in a <testsuite> under <testsuites> (line {})",
                document.text_pos_at(case.range().start).row
            ));
        }
        let (Some(name), Some(class)) = (case.attribute("name"), case.attribute("classname"))
        else {
            return Err(format!(
                "a test case has no name or classname (line {})",
                document.text_pos_at(case.range().start).row
            ));
        };
        let not_pass: Vec<String> = case
            .descendants()
            .filter(|node| node.is_element() && *node != case)
            .map(|node| node.tag_name().name().to_string())
            .filter(|element| !PASS_ELEMENTS.contains(&element.as_str()))
            .collect();
        cases
            .entry((class.to_string(), name.to_string()))
            .or_default()
            .push(not_pass);
    }
    Ok(cases)
}

/// The verdict on each proof line, in order: `None` when the slow step ran its test to PASS, else the reason.
///
/// # Errors
/// The JUnit text is not a nextest JUnit file (`results`).
fn verdicts(proofs: &[RealProof], junit: &str) -> Result<Vec<Option<String>>, String> {
    let cases = results(junit)?;
    Ok(proofs
        .iter()
        .map(|proof| {
            let line = format!("{} {} {}", proof.id, proof.binary, proof.test);
            match cases.get(&(proof.binary.clone(), proof.test.clone())) {
                None => Some(format!(
                    "{line}: the slow step did not run this test (no JUnit test case), so it proves nothing"
                )),
                Some(runs) => runs.iter().find(|children| !children.is_empty()).map(|children| {
                    format!(
                        "{line}: the slow step did not run this test to PASS (its JUnit test case has {})",
                        children.join(", ")
                    )
                }),
            }
        })
        .collect())
}

/// The check of the slow step: each line of the proof file has a passing test case in the slow tier's JUnit file.
///
/// # Errors
/// A file cannot be read or parsed, or a named proof did not pass.
pub fn check_slow_run(root: &Path, junit: &Path) -> Result<()> {
    let proofs = parse_real_proofs(
        &std::fs::read_to_string(root.join(FILE)).with_context(|| format!("read {FILE}"))?,
    )
    .map_err(|e| anyhow::anyhow!("{FILE}: {e}"))?;
    let text = std::fs::read_to_string(junit)
        .with_context(|| format!("read the slow tier's JUnit file {}", junit.display()))?;
    let verdicts =
        verdicts(&proofs, &text).map_err(|e| anyhow::anyhow!("{}: {e}", junit.display()))?;
    for (proof, verdict) in proofs.iter().zip(&verdicts) {
        if verdict.is_none() {
            println!(
                "real-proofs: {} passed by {} {} in the slow step",
                proof.id, proof.binary, proof.test
            );
        }
    }
    println!("real-proofs: {} named proofs checked", proofs.len());
    let problems: Vec<String> = verdicts.into_iter().flatten().collect();
    crate::tools::verdict(&problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The proof lines whose test the slow step did not run to PASS, each with the reason.
    fn unproven(proofs: &[RealProof], junit: &str) -> Result<Vec<String>, String> {
        Ok(verdicts(proofs, junit)?.into_iter().flatten().collect())
    }

    /// A JUnit document with these test cases in one test suite.
    fn suite(cases: &str) -> String {
        format!(r#"<testsuites><testsuite name="b">{cases}</testsuite></testsuites>"#)
    }

    /// The proof of test `t` of binary `b`.
    fn t_of_b() -> [RealProof; 1] {
        [RealProof {
            id: "conf::a".into(),
            binary: "b".into(),
            test: "t".into(),
        }]
    }

    fn proof(test: &str) -> RealProof {
        RealProof {
            id: "conf::lock".into(),
            binary: "botster-core::slow_lock".into(),
            test: test.into(),
        }
    }

    /// A hand-written report: a pass with its output in CDATA, a pass with no children, and a failure.
    const JUNIT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="1" errors="0">
    <testsuite name="botster-core::slow_lock" tests="3" disabled="0" errors="0" failures="1">
        <testcase name="locks::passes" classname="botster-core::slow_lock" timestamp="2026-10-11T00:00:00Z" time="0.5">
            <system-out><![CDATA[a log that says <failure> and </testcase>]]></system-out>
            <system-err></system-err>
        </testcase>
        <testcase name="locks::quiet" classname="botster-core::slow_lock" time="0.1"/>
        <testcase name="locks::fails" classname="botster-core::slow_lock" time="0.2">
            <failure type="test failure">assertion failed</failure>
            <system-out></system-out>
        </testcase>
    </testsuite>
</testsuites>
"#;

    #[test]
    fn a_proof_passes_only_with_a_passing_test_case_of_its_binary_and_name() {
        assert_eq!(
            unproven(&[proof("locks::passes"), proof("locks::quiet")], JUNIT),
            Ok(vec![])
        );
        let problems = unproven(
            &[
                proof("locks::fails"),
                proof("locks::absent"),
                RealProof {
                    binary: "botster-core::other".into(),
                    ..proof("locks::passes")
                },
            ],
            JUNIT,
        )
        .unwrap();
        assert_eq!(
            problems,
            [
                "conf::lock botster-core::slow_lock locks::fails: the slow step did not run this test to PASS (its JUnit test case has failure)",
                "conf::lock botster-core::slow_lock locks::absent: the slow step did not run this test (no JUnit test case), so it proves nothing",
                "conf::lock botster-core::other locks::passes: the slow step did not run this test (no JUnit test case), so it proves nothing",
            ]
        );
    }

    /// A report that cargo-nextest 0.9.146, the gate image's version, wrote (`xtask/fixtures/junit/nextest-0.9.146.xml`, whose
    /// comment says how it was made): only the test that passed on its first try passes. Its output, which prints a test
    /// case and a failure tag as escaped text, is not a test case.
    #[test]
    fn a_report_of_the_gate_s_nextest_passes_only_the_test_that_passed() {
        let junit = include_str!("../fixtures/junit/nextest-0.9.146.xml");
        let of = |test: &str| RealProof {
            id: "conf::a".into(),
            binary: "junit-probe".into(),
            test: test.into(),
        };
        let problems = unproven(
            &[
                of("tests::passes"),
                of("tests::flaky"),
                of("tests::fails"),
                of("tests::ignored"),
                RealProof {
                    binary: "y".into(),
                    ..of("x")
                },
            ],
            junit,
        )
        .unwrap();
        assert_eq!(
            problems,
            [
                "conf::a junit-probe tests::flaky: the slow step did not run this test to PASS (its JUnit test case has flakyFailure)",
                "conf::a junit-probe tests::fails: the slow step did not run this test to PASS (its JUnit test case has failure, rerunFailure)",
                "conf::a junit-probe tests::ignored: the slow step did not run this test (no JUnit test case), so it proves nothing",
                "conf::a y x: the slow step did not run this test (no JUnit test case), so it proves nothing",
            ]
        );
    }

    /// Every element that is not output or properties is not a pass: each JUnit result element, and an unknown one, also
    /// inside an output element.
    #[test]
    fn a_skipped_errored_flaky_or_unknown_result_is_not_a_pass() {
        for element in [
            "failure",
            "error",
            "skipped",
            "flakyFailure",
            "flakyError",
            "rerunFailure",
            "rerunError",
            "unknown",
        ] {
            for body in [
                format!(r#"<{element} message="x"/><system-out/>"#),
                format!(r#"<system-out><{element}/></system-out>"#),
            ] {
                let xml = suite(&format!(
                    r#"<testcase name="t" classname="b">{body}</testcase>"#
                ));
                let problems = unproven(&t_of_b(), &xml).unwrap();
                assert_eq!(problems.len(), 1, "{xml}");
                assert!(
                    problems[0].ends_with(&format!("has {element})")),
                    "{}",
                    problems[0]
                );
            }
        }
        // Properties are not a result.
        let xml = suite(
            r#"<testcase name="t" classname="b"><properties><property name="p" value="v"/></properties><system-err/></testcase>"#,
        );
        assert_eq!(unproven(&t_of_b(), &xml).unwrap(), Vec::<String>::new());
    }

    /// A test that appears twice passes only when both test cases passed, in either order.
    #[test]
    fn a_duplicate_test_case_cannot_hide_a_failure() {
        let pass = r#"<testcase name="t" classname="b"/>"#;
        let fail = r#"<testcase name="t" classname="b"><failure/></testcase>"#;
        for xml in [
            suite(&format!("{pass}{fail}")),
            suite(&format!("{fail}{pass}")),
        ] {
            assert_eq!(unproven(&t_of_b(), &xml).unwrap().len(), 1, "{xml}");
        }
        assert_eq!(
            unproven(&t_of_b(), &suite(&format!("{pass}{pass}"))).unwrap(),
            Vec::<String>::new()
        );
    }

    /// A file that is not a nextest JUnit file proves nothing: it is an error, not an empty result. That includes a document
    /// cut off after a passing test case (Sol, #264 RP-SI-1).
    #[test]
    fn a_malformed_junit_file_is_an_error() {
        let pass = r#"<testcase name="t" classname="b"/>"#;
        for (xml, error) in [
            (String::new(), "not well-formed"),
            (format!("<testsuites><testsuite>{pass}"), "not well-formed"),
            (
                format!("<testsuites><testsuite>{pass}</testsuites>"),
                "not well-formed",
            ),
            (format!("{pass}<testsuites/>"), "not well-formed"),
            (
                suite(r#"<testcase name="t" classname="b"><system-out>x</testcase>"#),
                "not well-formed",
            ),
            (
                suite(r#"<testcase name="t" classname="b"><system-out>x</system-out>"#),
                "not well-formed",
            ),
            (
                suite(
                    r#"<testcase name="t" classname="b"><system-out><![CDATA[x</system-out></testcase>"#,
                ),
                "not well-formed",
            ),
            (
                format!("<testsuite>{pass}</testsuite>"),
                "is <testsuite>, not <testsuites>",
            ),
            (pass.to_string(), "is <testcase>, not <testsuites>"),
            (
                format!("<testsuites>{pass}</testsuites>"),
                "not in a <testsuite> under <testsuites>",
            ),
            (
                format!("<testsuites><testsuite><x>{pass}</x></testsuite></testsuites>"),
                "not in a <testsuite> under <testsuites>",
            ),
            (
                format!("<testsuites><x><testsuite>{pass}</testsuite></x></testsuites>"),
                "not in a <testsuite> under <testsuites>",
            ),
            (suite(r#"<testcase name="t"/>"#), "no name or classname"),
            (
                suite(r#"<testcase classname="b"/>"#),
                "no name or classname",
            ),
        ] {
            let error_text = unproven(&t_of_b(), &xml).unwrap_err();
            assert!(error_text.contains(error), "{xml}: {error_text}");
        }
        // With no proof line, a malformed file is still an error.
        assert!(unproven(&[], "").is_err());
    }

    /// A test case in a comment or in a CDATA section is text, not a test case: the named proof has no test case (Sol,
    /// #264 RP-SI-1).
    #[test]
    fn a_test_case_in_a_comment_or_in_output_is_not_a_test_case() {
        for xml in [
            r#"<testsuites><!-- <testcase name="t" classname="b"/> --></testsuites>"#.to_string(),
            suite(r#"<!-- <testcase name="t" classname="b"/> -->"#),
            suite(
                r#"<testcase name="u" classname="b"><system-out><![CDATA[<testcase name="t" classname="b"/>]]></system-out></testcase>"#,
            ),
            suite(
                r#"<testcase name="u" classname="b"><system-out>&lt;testcase name="t" classname="b"/&gt;</system-out></testcase>"#,
            ),
        ] {
            let problems = unproven(&t_of_b(), &xml).unwrap();
            assert_eq!(problems.len(), 1, "{xml}");
            assert!(
                problems[0].contains("(no JUnit test case)"),
                "{}",
                problems[0]
            );
        }
        // An empty well-formed report has no test case either.
        assert_eq!(unproven(&t_of_b(), "<testsuites/>").unwrap().len(), 1);
        assert_eq!(
            unproven(&[], "<testsuites/>").unwrap(),
            Vec::<String>::new()
        );
    }

    /// Attribute values are read unescaped, as nextest escapes them.
    #[test]
    fn escaped_names_are_read_as_written() {
        let xml = suite(r#"<testcase name="t&lt;x&gt;" classname="b&amp;c"/>"#);
        let named = [RealProof {
            id: "conf::a".into(),
            binary: "b&c".into(),
            test: "t<x>".into(),
        }];
        assert_eq!(unproven(&named, &xml).unwrap(), Vec::<String>::new());
    }
}

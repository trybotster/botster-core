//! The named real-process proofs of the real-only Core ids passed in the slow step (plan sections 1 (23s), 5 and 8; lead,
//! 2026-10-11). `conformance/core-real-proofs.txt` names, for each real-only id that left `core-pending.txt`, the slow-tier
//! tests that prove it; the suite counts the id by them and never runs its transcript. So the slow step must have run each
//! named test to PASS: after the slow tier, this check reads the tier's own JUnit file (`test_budget` deletes it before the
//! run, so it is never a stale one) and fails on a line whose test has no test case there, or a test case that did not pass.
//!
//! The JUnit file is nextest's: a `<testcase>` per run test, with `classname` the binary id and `name` the test name. A
//! test case passed when it has no child element other than `system-out`, `system-err` and `properties`. Any other child
//! (`failure`, `error`, `skipped`, `flakyFailure`, `rerunFailure`, ... or an element this check does not know) is not a
//! pass. A test that appears twice passes only when every test case passed. A file that this check cannot read as such a
//! file proves nothing: it is an error.

use anyhow::{Context, Result};
use botster_core_testkit::pending::{parse_real_proofs, RealProof};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;

/// The proof file.
pub const FILE: &str = "conformance/core-real-proofs.txt";

/// The child elements of a test case that passed: its output and its properties.
const PASS_CHILDREN: [&str; 4] = ["system-out", "system-err", "properties", "property"];

/// A test of a JUnit file, by binary id and test name.
type Key = (String, String);

/// The text without its CDATA sections, so that no test output can read as a tag. An unclosed section is an error.
fn without_cdata(xml: &str) -> Result<String, String> {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<![CDATA[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "<![CDATA[".len()..];
        let end = after
            .find("]]>")
            .ok_or("the JUnit file has an unclosed CDATA section")?;
        rest = &after[end + "]]>".len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The test cases of a nextest JUnit file: for each `(classname, name)`, the children of each of its test cases that are not
/// a pass (empty: that test case passed).
///
/// # Errors
/// The text is not such a file: an unclosed CDATA section, a test case with no end, or with no `name` or `classname`.
fn results(xml: &str) -> Result<BTreeMap<Key, Vec<Vec<String>>>, String> {
    let xml = without_cdata(xml)?;
    let start = Regex::new(r"<testcase(\s[^>]*)?>").expect("regex");
    let attr = Regex::new(r#"(\w+)="([^"]*)""#).expect("regex");
    let child = Regex::new(r"<([A-Za-z][\w.:-]*)").expect("regex");
    let mut cases: BTreeMap<Key, Vec<Vec<String>>> = BTreeMap::new();
    let mut at = 0;
    while let Some(tag) = start.find_at(&xml, at) {
        let attrs: BTreeMap<&str, String> = attr
            .captures_iter(tag.as_str())
            .map(|c| {
                (
                    c.get(1).unwrap().as_str(),
                    unescape(c.get(2).unwrap().as_str()),
                )
            })
            .collect();
        let (Some(name), Some(class)) = (attrs.get("name"), attrs.get("classname")) else {
            return Err(format!(
                "a test case has no name or classname: {}",
                tag.as_str()
            ));
        };
        let (body, next) = if tag.as_str().ends_with("/>") {
            ("", tag.end())
        } else {
            let end = xml[tag.end()..]
                .find("</testcase>")
                .ok_or_else(|| format!("the test case {class} {name} has no end"))?;
            (
                &xml[tag.end()..tag.end() + end],
                tag.end() + end + "</testcase>".len(),
            )
        };
        let not_pass: Vec<String> = child
            .captures_iter(body)
            .map(|c| c.get(1).unwrap().as_str().to_string())
            .filter(|element| !PASS_CHILDREN.contains(&element.as_str()))
            .collect();
        cases
            .entry((class.clone(), name.clone()))
            .or_default()
            .push(not_pass);
        at = next;
    }
    Ok(cases)
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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

    fn proof(test: &str) -> RealProof {
        RealProof {
            id: "conf::lock".into(),
            binary: "botster-core::slow_lock".into(),
            test: test.into(),
        }
    }

    /// The form of cargo-nextest's JUnit file: a pass with its output in CDATA, a pass with no children, and a failure.
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

    /// Every child that is not output or properties is not a pass: each JUnit result element, and an unknown one.
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
            let xml = format!(
                r#"<testcase name="t" classname="b"><{element} message="x"/><system-out/></testcase>"#
            );
            let problems = unproven(
                &[RealProof {
                    id: "conf::a".into(),
                    binary: "b".into(),
                    test: "t".into(),
                }],
                &xml,
            )
            .unwrap();
            assert_eq!(problems.len(), 1, "{element}");
            assert!(
                problems[0].ends_with(&format!("has {element})")),
                "{}",
                problems[0]
            );
        }
    }

    /// A test that appears twice passes only when both test cases passed, in either order.
    #[test]
    fn a_duplicate_test_case_cannot_hide_a_failure() {
        let pass = r#"<testcase name="t" classname="b"/>"#;
        let fail = r#"<testcase name="t" classname="b"><failure/></testcase>"#;
        let named = [RealProof {
            id: "conf::a".into(),
            binary: "b".into(),
            test: "t".into(),
        }];
        for xml in [format!("{pass}{fail}"), format!("{fail}{pass}")] {
            assert_eq!(unproven(&named, &xml).unwrap().len(), 1, "{xml}");
        }
        assert_eq!(
            unproven(&named, &format!("{pass}{pass}")).unwrap(),
            Vec::<String>::new()
        );
    }

    /// A file that is not a nextest JUnit file proves nothing: it is an error, not an empty result.
    #[test]
    fn a_malformed_junit_file_is_an_error() {
        for (xml, error) in [
            (r#"<testcase name="t" classname="b">"#, "has no end"),
            (r#"<testcase name="t">"#, "no name or classname"),
            (r#"<testcase classname="b"/>"#, "no name or classname"),
            (
                r#"<testcase name="t" classname="b"><system-out><![CDATA[x</system-out></testcase>"#,
                "unclosed CDATA",
            ),
        ] {
            let error_text = unproven(&[proof("t")], xml).unwrap_err();
            assert!(error_text.contains(error), "{xml}: {error_text}");
        }
        // An empty file has no test case, so every named proof is unproven; with no proof line nothing is counted.
        assert_eq!(unproven(&[proof("t")], "").unwrap().len(), 1);
        assert_eq!(unproven(&[], "").unwrap(), Vec::<String>::new());
    }

    /// Attribute values are read unescaped, as nextest escapes them.
    #[test]
    fn escaped_names_are_read_as_written() {
        let xml = r#"<testcase name="t&lt;x&gt;" classname="b&amp;c"/>"#;
        let named = [RealProof {
            id: "conf::a".into(),
            binary: "b&c".into(),
            test: "t<x>".into(),
        }];
        assert_eq!(unproven(&named, xml).unwrap(), Vec::<String>::new());
    }
}

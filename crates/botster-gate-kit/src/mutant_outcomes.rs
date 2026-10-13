//! The verdict of a mutation stage from its `outcomes.json` and its logs (moved from Core's xtask `ci`, with its tests): the
//! counts, the missed and the tested mutants by full name, the log report of [`stage_report`] as part of the verdict, and
//! the step's verdict from the exit code. A gate that runs one stage composes [`count_difference`] (the selection),
//! [`stage_outcomes`], [`report_lines`] and [`mutation_verdict`]; Core's two-stage step composes them the same way.

use crate::mutant_logs::stage_report;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::time::Duration;

/// What the mutation run reports (`mutants.out/outcomes.json`): its counts.
#[derive(Debug, PartialEq, Eq)]
pub struct MutantSummary {
    pub total: u64,
    pub caught: u64,
    pub missed: u64,
    pub timeout: u64,
    pub unviable: u64,
}

/// The counts of `outcomes.json`.
///
/// # Errors
/// The text is not JSON, or a count is missing.
pub fn parse_outcomes(json: &str) -> Result<MutantSummary> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let n = |key: &str| {
        v[key]
            .as_u64()
            .with_context(|| format!("outcomes.json has no `{key}`"))
    };
    Ok(MutantSummary {
        total: n("total_mutants")?,
        caught: n("caught")?,
        missed: n("missed")?,
        timeout: n("timeout")?,
        unviable: n("unviable")?,
    })
}

/// The step's verdict from the exit code of `cargo mutants` (`None`: ended by a signal). Only 0, every mutant caught or
/// no mutant in the diff, passes; 2 a missed mutant, 3 a timeout, 4 a failed baseline and any other end fail the step.
///
/// # Errors
/// Any exit code but 0.
pub fn mutation_verdict(code: Option<i32>) -> Result<()> {
    if code == Some(0) {
        return Ok(());
    }
    bail!(
        "cargo mutants failed (exit code {code:?}: 2 a missed mutant, 3 a timeout, 4 a failed baseline); a missed \
         mutant or a timeout is a review finding"
    )
}

/// What one mutation stage's `outcomes.json` reports: the counts, the names of the missed mutants, the names of every
/// mutant that it tested (whatever the outcome), and the report of its logs ([`stage_report`]: the catching
/// test of each caught mutant and each FLAKY test, or why the logs fail the step). A name is a mutant's full identity in
/// cargo-mutants: `<file>:<line>:<column>: <change>`.
#[derive(Debug, PartialEq)]
pub struct Outcomes {
    pub summary: MutantSummary,
    pub missed: Vec<String>,
    pub tested: Vec<String>,
    pub report: std::result::Result<Vec<String>, String>,
}

/// The outcomes of a stage (`Outcomes`) from its `outcomes.json`, with the report of the logs that `read` gives by their
/// `log_path` (relative to the stage's `mutants.out`). A mutant that cargo-mutants reports as caught, but whose first
/// final failure is a known-flaky test (`flaky`, `StageReport::missed`), is MISSED: the counts and the
/// missed names include it (GS-KF-R1-1, lead settlement 2026-10-11).
///
/// # Errors
/// The file has no counts (`parse_outcomes`), no `outcomes` array, or a missed outcome without a mutant name.
pub fn parse_stage_outcomes(
    json: &str,
    flaky: &[String],
    read: impl Fn(&str) -> Result<String>,
) -> Result<Outcomes> {
    let mut summary = parse_outcomes(json)?;
    let report = stage_report(json, flaky, read);
    let mut flaky_first = report
        .as_ref()
        .map(|report| report.missed.clone())
        .unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(json)?;
    let mutants = v["outcomes"]
        .as_array()
        .context("outcomes.json has no `outcomes`")?
        .iter()
        .filter(|o| o["scenario"].get("Mutant").is_some())
        .map(|o| {
            o["scenario"]["Mutant"]["name"]
                .as_str()
                .map(|name| {
                    let caught_first_by_flaky = o["summary"] == "CaughtMutant"
                        && flaky_first
                            .iter()
                            .position(|missed| missed == name)
                            .map(|i| flaky_first.remove(i))
                            .is_some();
                    (
                        name.to_string(),
                        o["summary"] == "MissedMutant" || caught_first_by_flaky,
                    )
                })
                .context("a mutant outcome in outcomes.json has no mutant name")
        })
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(
        flaky_first.is_empty(),
        "outcomes.json has no caught mutant for each of {flaky_first:?}, which a known-flaky test caught first"
    );
    let moved = u64::try_from(report.as_ref().map_or(0, |report| report.missed.len()))?;
    anyhow::ensure!(
        moved <= summary.caught,
        "outcomes.json counts {} caught mutants, fewer than the {moved} that a known-flaky test caught first",
        summary.caught
    );
    summary.caught -= moved;
    summary.missed += moved;
    let missed = mutants
        .iter()
        .filter(|(_, missed)| *missed)
        .map(|(name, _)| name.clone())
        .collect();
    let tested = mutants.into_iter().map(|(name, _)| name).collect();
    Ok(Outcomes {
        summary,
        missed,
        tested,
        report: report.map(|report| report.lines),
    })
}

/// Gives each line of a stage's log report to `out`, after `mutants: <name>: `.
///
/// # Errors
/// The report is an error: the logs fail the step (a catch that names no test, a retried baseline, an unknown form).
pub fn report_lines(
    name: &str,
    report: &std::result::Result<Vec<String>, String>,
    out: &mut impl FnMut(String),
) -> Result<()> {
    match report {
        Ok(lines) => {
            for line in lines {
                out(format!("mutants: {name}: {line}"));
            }
            Ok(())
        }
        Err(error) => bail!("mutants: {name}: the logs fail the step: {error}"),
    }
}

/// The names that `tested` holds more often than `expected` (extra), and the names that it holds less often (absent),
/// each as often as the counts differ.
pub fn count_difference<'a>(
    expected: &'a [String],
    tested: &'a [String],
) -> (Vec<&'a String>, Vec<&'a String>) {
    let mut counts: BTreeMap<&String, (usize, usize)> = BTreeMap::new();
    for name in expected {
        counts.entry(name).or_default().0 += 1;
    }
    for name in tested {
        counts.entry(name).or_default().1 += 1;
    }
    let mut extra = Vec::new();
    let mut absent = Vec::new();
    for (name, (expected, tested)) in counts {
        extra.extend(std::iter::repeat_n(name, tested.saturating_sub(expected)));
        absent.extend(std::iter::repeat_n(name, expected.saturating_sub(tested)));
    }
    (extra, absent)
}

/// One mutation stage: its exit code (`None`: ended by a signal), its outcomes (`None`: no `outcomes.json`), and its time.
pub struct Stage {
    pub code: Option<i32>,
    pub outcomes: Option<Outcomes>,
    pub took: Duration,
}

/// The report line of a stage that had to test `expected` mutants, with its outcomes.
///
/// # Errors
/// The stage wrote no outcomes (it ended before them: a failed start, a failed build, a signal), or outcomes for another
/// number of mutants.
pub fn stage_outcomes(
    name: &str,
    expected: usize,
    stage: Stage,
) -> Result<(Option<i32>, Outcomes, String)> {
    let Some(outcomes) = stage.outcomes else {
        bail!(
            "mutants: {name}: cargo mutants wrote no outcomes.json for its {expected} mutants (exit code {:?}): the run \
             failed before its outcomes",
            stage.code
        );
    };
    let s = &outcomes.summary;
    // A mutant that a known-flaky test caught first is MISSED (`parse_stage_outcomes`), so cargo-mutants' success is a
    // missed mutant (exit code 2) when the counts have one.
    let code = if stage.code == Some(0) && s.missed > 0 {
        Some(2)
    } else {
        stage.code
    };
    let line = format!(
        "mutants: {name}: {} mutants: {} caught, {} missed, {} timeout, {} unviable, in {:.1} s",
        s.total,
        s.caught,
        s.missed,
        s.timeout,
        s.unviable,
        stage.took.as_secs_f64()
    );
    if usize::try_from(s.total).ok() != Some(expected) {
        bail!("{line}; it had {expected} mutants to test: the run did not report every one");
    }
    Ok((code, outcomes, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_logs(path: &str) -> Result<String> {
        bail!("no log {path} in this test")
    }

    fn counts(total: u64, caught: u64, missed: u64, timeout: u64, unviable: u64) -> MutantSummary {
        MutantSummary {
            total,
            caught,
            missed,
            timeout,
            unviable,
        }
    }

    fn outcomes(
        summary: MutantSummary,
        report: std::result::Result<Vec<String>, String>,
    ) -> Outcomes {
        Outcomes {
            missed: (0..summary.missed).map(|i| format!("m{i}")).collect(),
            tested: (0..summary.total).map(|i| format!("m{i}")).collect(),
            summary,
            report,
        }
    }

    /// GS-KF-R1-1 (lead settlement, 2026-10-11): only the first final failure of a mutant's run can be a catch. (a) A
    /// listed test fails first, and an unlisted test then fails by the cancellation: the mutant is MISSED, in the counts
    /// and the names. (b) An unlisted test fails first: it is the catch. (c) When every caught mutant moves, the counts
    /// still hold: 0 caught.
    #[test]
    fn a_mutant_that_a_listed_test_fails_first_is_missed() {
        let listed = ["w t::flaky".to_string()];
        let flaky_first =
            "        FAIL [   0.100s] w t::flaky\n     SIGTERM [   0.200s] w t::real\n";
        let real_first =
            "        FAIL [   0.100s] w t::real\n     SIGTERM [   0.200s] w t::flaky\n";
        let json = r#"{"total_mutants":2,"caught":2,"missed":0,"timeout":0,"unviable":0,"outcomes":[
            {"log_path":"m0.log","scenario":{"Mutant":{"name":"m0"}},"summary":"CaughtMutant"},
            {"log_path":"m1.log","scenario":{"Mutant":{"name":"m1"}},"summary":"CaughtMutant"}]}"#;
        let read = |m0: &'static str| {
            move |path: &str| -> Result<String> {
                Ok(if path == "m0.log" { m0 } else { real_first }.to_string())
            }
        };
        let a = parse_stage_outcomes(json, &listed, read(flaky_first)).unwrap();
        assert_eq!(a.summary, counts(2, 1, 1, 0, 0));
        assert_eq!(a.missed, ["m0"]);
        assert_eq!(a.tested, ["m0", "m1"]);
        assert_eq!(
            a.report.unwrap(),
            [
                "`m0` is MISSED: its first failure is the known-flaky test w t::flaky (ci/known-flaky.toml; log m0.log), \
                 which is no catch; the later failures [\"w t::real\"] come after the cancellation starts, so none is a \
                 catch"
                    .to_string(),
                "caught `m1` by w t::real".to_string(),
            ]
        );
        let b = parse_stage_outcomes(json, &listed, read(real_first)).unwrap();
        assert_eq!(
            (b.summary, b.missed),
            (counts(2, 2, 0, 0, 0), Vec::<String>::new())
        );
        let one = r#"{"total_mutants":1,"caught":1,"missed":0,"timeout":0,"unviable":0,"outcomes":[
            {"log_path":"m0.log","scenario":{"Mutant":{"name":"m0"}},"summary":"CaughtMutant"}]}"#;
        let c = parse_stage_outcomes(one, &listed, read(flaky_first)).unwrap();
        assert_eq!(
            (c.summary, c.missed),
            (counts(1, 0, 1, 0, 0), vec!["m0".to_string()])
        );
    }

    /// H212-I1 (the Hub's integration review): a run whose tool exits 0, but whose logs the report rejects, fails the step.
    /// Here the unmutated baseline retried a test: its outcomes still count, and its report fails at `report_lines`.
    #[test]
    fn a_rejected_log_report_fails_the_step_after_a_successful_exit() {
        let json = r#"{"total_mutants":1,"caught":1,"missed":0,"timeout":0,"unviable":0,"outcomes":[
            {"log_path":"baseline.log","scenario":"Baseline","summary":"Success"},
            {"log_path":"m0.log","scenario":{"Mutant":{"name":"m0"}},"summary":"CaughtMutant"}]}"#;
        let read = |path: &str| -> Result<String> {
            Ok(if path == "baseline.log" {
                "        TRY 1 FAIL [   0.100s] w t::a\n       FLAKY 2/2 [   0.100s] w t::a\n"
            } else {
                "        FAIL [   0.100s] w t::a\n"
            }
            .to_string())
        };
        let stage = Stage {
            code: Some(0),
            outcomes: Some(parse_stage_outcomes(json, &[], read).unwrap()),
            took: Duration::ZERO,
        };
        let (code, outcomes, _) = stage_outcomes("stage 1 (default)", 1, stage).unwrap();
        assert_eq!(code, Some(0));
        let mut lines = Vec::new();
        let error = report_lines("stage 1 (default)", &outcomes.report, &mut |line| {
            lines.push(line)
        })
        .unwrap_err();
        assert!(
            format!("{error:#}").starts_with(
                "mutants: stage 1 (default): the logs fail the step: the unmutated baseline"
            ),
            "{error:#}"
        );
        assert!(lines.is_empty());
    }

    #[test]
    fn each_report_line_is_named_and_a_rejected_report_fails() {
        let mut lines = Vec::new();
        report_lines(
            "s",
            &Ok(vec!["caught `m0` by w t::a".into(), "x".into()]),
            &mut |line| lines.push(line),
        )
        .unwrap();
        assert_eq!(
            lines,
            ["mutants: s: caught `m0` by w t::a", "mutants: s: x"]
        );
        let error = report_lines("s", &Err("a catch names no test".into()), &mut |line| {
            lines.push(line)
        })
        .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "mutants: s: the logs fail the step: a catch names no test"
        );
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn the_count_difference_names_each_extra_and_each_absent_name_as_often_as_it_differs() {
        let expected = ["a", "b", "b", "c"].map(String::from);
        let tested = ["a", "a", "b", "d", "d"].map(String::from);
        let (extra, absent) = count_difference(&expected, &tested);
        assert_eq!(extra, ["a", "d", "d"]);
        assert_eq!(absent, ["b", "c"]);
        let (extra, absent) = count_difference(&expected, &expected);
        assert!(extra.is_empty() && absent.is_empty());
    }

    /// A stage without outcomes, or with outcomes for another number of mutants, fails; exit code 0 with a missed mutant
    /// (a known-flaky first failure) is a missed mutant, code 2; any other code stays.
    #[test]
    fn a_stage_line_needs_every_outcome_and_a_miss_turns_success_into_code_2() {
        let stage = |code, summary: Option<MutantSummary>| Stage {
            code,
            outcomes: summary.map(|s| outcomes(s, Ok(Vec::new()))),
            took: Duration::from_millis(2500),
        };
        let error = stage_outcomes("s1", 3, stage(Some(4), None)).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "mutants: s1: cargo mutants wrote no outcomes.json for its 3 mutants (exit code Some(4)): the run failed before \
             its outcomes"
        );
        let (code, _, line) =
            stage_outcomes("s1", 3, stage(Some(0), Some(counts(3, 2, 0, 0, 1)))).unwrap();
        assert_eq!(code, Some(0));
        assert_eq!(
            line,
            "mutants: s1: 3 mutants: 2 caught, 0 missed, 0 timeout, 1 unviable, in 2.5 s"
        );
        assert_eq!(
            stage_outcomes("s1", 3, stage(Some(0), Some(counts(3, 2, 1, 0, 0))))
                .unwrap()
                .0,
            Some(2)
        );
        assert_eq!(
            stage_outcomes("s1", 3, stage(Some(3), Some(counts(3, 1, 1, 1, 0))))
                .unwrap()
                .0,
            Some(3)
        );
        assert_eq!(
            stage_outcomes("s1", 3, stage(None, Some(counts(3, 3, 0, 0, 0))))
                .unwrap()
                .0,
            None
        );
        let error =
            stage_outcomes("s1", 4, stage(Some(0), Some(counts(3, 3, 0, 0, 0)))).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "mutants: s1: 3 mutants: 3 caught, 0 missed, 0 timeout, 0 unviable, in 2.5 s; it had 4 mutants to test: the \
             run did not report every one"
        );
    }

    /// Plan section 8, step 8: only a run with every mutant caught passes the mutation step.
    #[test]
    fn only_a_mutation_run_with_every_mutant_caught_passes() {
        assert!(mutation_verdict(Some(0)).is_ok());
        for failed in [Some(1), Some(2), Some(3), Some(4), None] {
            assert!(mutation_verdict(failed).is_err(), "{failed:?}");
        }
    }

    #[test]
    fn the_missed_mutants_are_read_by_name_from_outcomes_json() {
        let json = r#"{"total_mutants":3,"caught":1,"missed":2,"timeout":0,"unviable":0,"success":0,"outcomes":[
            {"scenario":"Baseline","summary":"Success"},
            {"scenario":{"Mutant":{"name":"a.rs:1:2: replace f -> bool with true"}},"summary":"MissedMutant"},
            {"scenario":{"Mutant":{"name":"a.rs:3:4: replace g with ()"}},"summary":"CaughtMutant"},
            {"scenario":{"Mutant":{"name":"b.rs:5:6: replace + with - in h"}},"summary":"MissedMutant"}]}"#;
        let outcomes = parse_stage_outcomes(json, &[], no_logs).unwrap();
        assert_eq!(outcomes.summary, counts(3, 1, 2, 0, 0));
        assert_eq!(
            outcomes.missed,
            [
                "a.rs:1:2: replace f -> bool with true",
                "b.rs:5:6: replace + with - in h"
            ]
        );
        assert_eq!(
            outcomes.tested,
            [
                "a.rs:1:2: replace f -> bool with true",
                "a.rs:3:4: replace g with ()",
                "b.rs:5:6: replace + with - in h"
            ]
        );
        let no_list = r#"{"total_mutants":0,"caught":0,"missed":0,"timeout":0,"unviable":0}"#;
        assert!(parse_stage_outcomes(no_list, &[], no_logs).is_err());
        let unnamed = r#"{"total_mutants":1,"caught":0,"missed":1,"timeout":0,"unviable":0,"outcomes":[
            {"scenario":{"Mutant":{}},"summary":"MissedMutant"}]}"#;
        assert!(parse_stage_outcomes(unnamed, &[], no_logs).is_err());
        let unnamed_caught = r#"{"total_mutants":1,"caught":1,"missed":0,"timeout":0,"unviable":0,"outcomes":[
            {"scenario":{"Mutant":{}},"summary":"CaughtMutant"}]}"#;
        assert!(parse_stage_outcomes(unnamed_caught, &[], no_logs).is_err());
    }

    #[test]
    fn the_mutation_summary_is_read_from_outcomes_json() {
        let json =
            r#"{"total_mutants":10,"caught":7,"missed":1,"timeout":0,"unviable":2,"success":0}"#;
        assert_eq!(
            parse_outcomes(json).unwrap(),
            MutantSummary {
                total: 10,
                caught: 7,
                missed: 1,
                timeout: 0,
                unviable: 2
            }
        );
        assert!(parse_outcomes("{}").is_err());
    }
}

//! The test results in the logs of a mutation stage: which test caught each caught mutant, and which tests were FLAKY
//! (lead, 2026-10-11, after the #246 gate's flaky `ev_5` showed that a flake can count as a catch).
//!
//! BUILD.md Testing rule 9 (contracts `main` e1da7be) allows one retry in the `mutants` nextest profile only, and only
//! of the test that failed. A test that fails and then passes on its retry is FLAKY: nextest passes the run, so
//! cargo-mutants counts the mutant as MISSED, and a flake can no longer count as a catch. Each FLAKY test is named in
//! the gate output, because the flake itself still needs a structural fix. An unmutated baseline must never retry: its
//! log may hold no retry line.
//!
//! cargo-mutants keeps each scenario's output in `mutants.out/<log_path>` (`outcomes.json`), with nextest's status lines.
//! The parser reads those lines strictly: a status line of a form that it does not know is an error, so a nextest
//! upgrade that changes the form fails the step instead of hiding a catch (the slow test
//! `slow_a_flaky_catch_is_missed_and_a_real_catch_names_its_test` runs the real tools).

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::sync::OnceLock;

/// What one nextest run of a log reports: the tests whose final result is a failure, in the order in which they
/// reached it (the first is the catching test, with `--max-fail 1:immediate`), the tests whose final result is FLAKY,
/// and whether any test was retried.
#[derive(Debug, PartialEq, Default)]
pub struct TestRun {
    pub failed: Vec<String>,
    pub flaky: Vec<String>,
    pub retried: bool,
}

/// The state that a status line gives its test.
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Passed,
    Failed,
    Flaky,
}

/// The signal names that nextest prints without `SIG` on a retry line (`short_status_str`).
const SIGNALS: [&str; 18] = [
    "HUP", "INT", "QUIT", "ILL", "TRAP", "ABRT", "BUS", "FPE", "KILL", "USR1", "SEGV", "USR2",
    "PIPE", "ALRM", "TERM", "SYS", "XCPU", "XFSZ",
];

/// The state that a status (the text before the bracketed time) gives, `None` for a progress line (a start, a slow
/// notice, a termination in progress), and an error for a status that nextest 0.9 does not print in our profiles.
fn state_of(status: &str) -> Result<Option<State>> {
    let failure = |s: &str| {
        matches!(
            s,
            "FAIL" | "FAIL + LEAK" | "XFAIL" | "ABORT" | "LEAK-FAIL" | "TIMEOUT"
        ) || s
            .strip_prefix("SIG")
            .is_some_and(|name| SIGNALS.contains(&name))
            || s.strip_prefix("ABORT SIG ")
                .is_some_and(|n| n.parse::<u32>().is_ok())
    };
    let pass = |s: &str| {
        matches!(
            s,
            "PASS" | "LEAK" | "TIMEOUT-PASS" | "SLOW + LEAK" | "SLOW+TMPASS"
        )
    };
    if failure(status) {
        return Ok(Some(State::Failed));
    }
    if pass(status) {
        return Ok(Some(State::Passed));
    }
    if matches!(status, "START" | "SLOW" | "TERMINATING") {
        return Ok(None);
    }
    if let Some(rest) = status.strip_prefix("FLAKY ") {
        if attempts(rest).is_some() {
            return Ok(Some(State::Flaky));
        }
    }
    if let Some(rest) = status.strip_prefix("TRY ") {
        if let Some((attempt, what)) = rest.split_once(' ') {
            if attempt.parse::<u32>().is_ok() {
                let short_failure = matches!(
                    what,
                    "FAIL" | "FL+LK" | "XFAIL" | "ABORT" | "LKFAIL" | "TMT"
                ) || SIGNALS.contains(&what)
                    || what
                        .strip_prefix("SIG ")
                        .is_some_and(|n| n.parse::<u32>().is_ok());
                if short_failure {
                    return Ok(Some(State::Failed));
                }
                if matches!(what, "PASS" | "LEAK" | "TMPASS") {
                    return Ok(Some(State::Passed));
                }
                if matches!(what, "START" | "SLOW" | "TRMNTG") {
                    return Ok(None);
                }
            }
        }
    }
    bail!("an unknown nextest status `{status}`")
}

/// `<n>/<m>` of a FLAKY status.
fn attempts(text: &str) -> Option<(u32, u32)> {
    let (n, m) = text.split_once('/')?;
    Some((n.parse().ok()?, m.parse().ok()?))
}

/// A nextest status line: the status, right-aligned before a bracketed time (`[   0.183s]`, `[> 60.000s]` for a slow
/// notice, or blank for a start), an optional counter (`( 76/429)`, or `(───)` on a retry line in cargo-mutants' logs),
/// then the binary id and the test name. Mixed-case lines (`Summary`, `Starting`) and compiler output do not match.
fn status_line() -> &'static Regex {
    static LINE: OnceLock<Regex> = OnceLock::new();
    LINE.get_or_init(|| {
        Regex::new(
            r"^\s*([A-Z][A-Z0-9 +/\-]*?) \[\s*(?:>\s*)?(?:\d+(?:\.\d+)?s)?\s*\] (?:\([^)]*\) )?(\S+) (\S.*?)\s*$",
        )
        .expect("a valid regex")
    })
}

/// The tests of a log (`TestRun`). A test's final result is the state of its last status line that is not progress;
/// the order of the failed tests is the line at which each first reached its final state.
///
/// # Errors
/// A status line has a form that nextest 0.9 does not print in our profiles, or a failed test gives `FLKY-FL`
/// (`flaky-result = "fail"`, which no profile sets).
pub fn read_run(log: &str) -> Result<TestRun> {
    // For each test: its state, and the index of the line at which it reached that state.
    let mut tests: Vec<(String, State, usize)> = Vec::new();
    let mut retried = false;
    for (index, line) in log.lines().enumerate() {
        let Some(caps) = status_line().captures(line) else {
            continue;
        };
        let status = &caps[1];
        retried |= status.starts_with("TRY ") || status.starts_with("FLAKY ");
        let Some(state) =
            state_of(status).with_context(|| format!("the log line `{}`", line.trim()))?
        else {
            continue;
        };
        let test = format!("{} {}", &caps[2], &caps[3]);
        match tests.iter_mut().find(|(name, ..)| *name == test) {
            Some(entry) if entry.1 != state => {
                entry.1 = state;
                entry.2 = index;
            }
            Some(_) => {}
            None => tests.push((test, state, index)),
        }
    }
    let mut failed: Vec<(String, usize)> = tests
        .iter()
        .filter(|(_, state, _)| *state == State::Failed)
        .map(|(name, _, index)| (name.clone(), *index))
        .collect();
    failed.sort_by_key(|(_, index)| *index);
    Ok(TestRun {
        failed: failed.into_iter().map(|(name, _)| name).collect(),
        flaky: tests
            .iter()
            .filter(|(_, state, _)| *state == State::Flaky)
            .map(|(name, ..)| name.clone())
            .collect(),
        retried,
    })
}

/// The report of a stage from its `outcomes.json` (`json`) and the logs that `read` gives by their `log_path`: one line
/// for each caught mutant with its catching test, and one for each FLAKY test with its mutant.
///
/// # Errors
/// As text: `outcomes.json` has no outcomes, an outcome has no log path or summary, a log cannot be read or parsed
/// (`read_run`), an unmutated baseline retried a test, a caught mutant's log names no failed test, or a missed
/// mutant's log names a failed test.
pub fn stage_report(
    json: &str,
    read: impl Fn(&str) -> Result<String>,
) -> Result<Vec<String>, String> {
    report(json, read).map_err(|error| format!("{error:#}"))
}

fn report(json: &str, read: impl Fn(&str) -> Result<String>) -> Result<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse outcomes.json")?;
    let outcomes = v["outcomes"]
        .as_array()
        .context("outcomes.json has no `outcomes`")?;
    let mut lines = Vec::new();
    for outcome in outcomes {
        let path = outcome["log_path"]
            .as_str()
            .context("an outcome in outcomes.json has no `log_path`")?;
        let summary = outcome["summary"]
            .as_str()
            .context("an outcome in outcomes.json has no `summary`")?;
        let run = read_run(&read(path).with_context(|| format!("read the log {path}"))?)
            .with_context(|| format!("the log {path}"))?;
        let Some(mutant) = outcome["scenario"].get("Mutant") else {
            if run.retried {
                bail!(
                    "the unmutated baseline ({path}) retried a test (failed: {:?}, FLAKY: {:?}): an unmutated run never \
                     retries (BUILD.md Testing rule 9)",
                    run.failed,
                    run.flaky
                );
            }
            continue;
        };
        let name = mutant["name"]
            .as_str()
            .context("a mutant outcome in outcomes.json has no mutant name")?;
        for test in &run.flaky {
            lines.push(format!(
                "FLAKY {test} under `{name}`: it failed, then passed on its one retry, so it is no catch; the flake needs a \
                 structural fix (BUILD.md Testing rule 9)"
            ));
        }
        match summary {
            "CaughtMutant" => {
                let Some(test) = run.failed.first() else {
                    bail!("`{name}` is CAUGHT, but its log ({path}) names no failed test: a catch must name its test");
                };
                lines.push(format!("caught `{name}` by {test}"));
            }
            "MissedMutant" if !run.failed.is_empty() => {
                bail!(
                    "`{name}` is MISSED, but its log ({path}) names the failed tests {:?}",
                    run.failed
                );
            }
            _ => {}
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATCH: &str = "   Compiling twostage v0.1.0
     Starting 3 tests across 2 binaries
        PASS [   0.010s] (1/3) twostage tests::a
        FAIL [   0.183s] (2/3) twostage::slow_triple conf::b
  stdout ───
    test conf::b ... FAILED
   Cancelling due to test failure: 1 test still running
     SIGTERM [   0.200s] (3/3) twostage tests::c
     Summary [   0.300s] 3 tests run: 1 passed, 2 failed
        FAIL [   0.183s] (2/3) twostage::slow_triple conf::b
";

    /// The first test that reached a final failure is the catch; a test that the immediate end of the run terminated
    /// fails later; the summary's repeat changes nothing.
    #[test]
    fn the_first_final_failure_is_the_catching_test() {
        let run = read_run(CATCH).unwrap();
        assert_eq!(
            run.failed,
            ["twostage::slow_triple conf::b", "twostage tests::c"]
        );
        assert!(run.flaky.is_empty());
        assert!(!run.retried);
    }

    /// With one retry: a test that fails twice is a failure; one that fails and then passes is FLAKY, not a failure;
    /// both runs retried.
    #[test]
    fn a_retried_failure_fails_and_a_fail_then_pass_is_flaky() {
        let twice = "        TRY 1 FAIL [   0.100s] (1/2) w t::a
        TRY 2 START             (1/2) w t::a
        TRY 2 FAIL [   0.100s] (1/2) w t::a
        PASS [   0.050s] (2/2) w t::b
";
        let run = read_run(twice).unwrap();
        assert_eq!(run.failed, ["w t::a"]);
        assert!(run.flaky.is_empty());
        assert!(run.retried);
        let flaky = "        TRY 1 FAIL [   0.100s] (1/1) w t::a
        TRY 2 START             (1/1) w t::a
       FLAKY 2/2 [   0.100s] (1/1) w t::a
";
        let run = read_run(flaky).unwrap();
        assert!(run.failed.is_empty(), "{run:?}");
        assert_eq!(run.flaky, ["w t::a"]);
        assert!(run.retried);
    }

    /// The lines of a real cargo-mutants log (cargo-mutants 27.1.0, cargo-nextest 0.9.146, evidence
    /// `gates/botster-core-gs-flaky-log-a0fe57de-evidence.log`): a retry line has the counter `(───)` and a start has a
    /// blank time, and both name the same test as its final line.
    #[test]
    fn the_lines_of_a_real_log_name_one_test_whatever_their_counter() {
        let flaky = "       START [         ] (1/2) flaky tests::two_tripled_is_six
       START [         ] (1/2) flaky tests::half_fails_once_under_a_mutant
  TRY 1 FAIL [   0.002s] (\u{2500}\u{2500}\u{2500}) flaky tests::half_fails_once_under_a_mutant
    test tests::half_fails_once_under_a_mutant ... FAILED
        PASS [   0.002s] (1/2) flaky tests::two_tripled_is_six
 TRY 2 START [         ] (\u{2500}\u{2500}\u{2500}) flaky tests::half_fails_once_under_a_mutant
  TRY 2 PASS [   0.002s] (2/2) flaky tests::half_fails_once_under_a_mutant
     Summary [   0.006s] 2 tests run: 2 passed (1 flaky), 0 skipped
   FLAKY 2/2 [   0.002s] (2/2) flaky tests::half_fails_once_under_a_mutant
";
        let run = read_run(flaky).unwrap();
        assert!(run.failed.is_empty(), "{run:?}");
        assert_eq!(run.flaky, ["flaky tests::half_fails_once_under_a_mutant"]);
        let caught =
            "  TRY 1 FAIL [   0.003s] (\u{2500}\u{2500}\u{2500}) flaky tests::two_tripled_is_six
 TRY 2 START [         ] (\u{2500}\u{2500}\u{2500}) flaky tests::two_tripled_is_six
  TRY 2 FAIL [   0.002s] (2/2) flaky tests::two_tripled_is_six
     Summary [   0.006s] 2 tests run: 1 passed, 1 failed, 0 skipped
  TRY 2 FAIL [   0.002s] (2/2) flaky tests::two_tripled_is_six
";
        let run = read_run(caught).unwrap();
        assert_eq!(run.failed, ["flaky tests::two_tripled_is_six"]);
        assert!(run.flaky.is_empty() && run.retried);
    }

    /// Signals in both forms are failures; slow notices and terminations in progress change no state; an unknown status
    /// is an error, and so is `FLKY-FL`.
    #[test]
    fn every_known_status_has_a_state_and_an_unknown_one_is_an_error() {
        let signals = "     SIGSEGV [   0.100s] w t::a
        SLOW [> 60.000s] w t::b
 TRY 2 SEGV [   0.100s] w t::b
 TERMINATING [   0.100s] w t::c
";
        assert_eq!(read_run(signals).unwrap().failed, ["w t::a", "w t::b"]);
        for status in [
            "SKIPPED",
            "FLKY-FL 2/2",
            "TRY X FAIL",
            "TRY 2 NOPE",
            "SIGNOPE",
            "FLAKY 2",
        ] {
            let line = format!("{status:>12} [   0.100s] w t::a\n");
            assert!(read_run(&line).is_err(), "{status}");
        }
        assert_eq!(
            read_run("     Summary [   0.300s] 3 tests run\nerror[E0308]: x [0.1s] y z\n").unwrap(),
            TestRun::default()
        );
    }

    fn outcomes(entries: &[(&str, &str, Option<&str>)]) -> String {
        let items: Vec<String> = entries
            .iter()
            .map(|(path, summary, mutant)| {
                let scenario = match mutant {
                    Some(name) => format!("{{\"Mutant\":{{\"name\":\"{name}\"}}}}"),
                    None => "\"Baseline\"".to_string(),
                };
                format!(
                    "{{\"log_path\":\"{path}\",\"summary\":\"{summary}\",\"scenario\":{scenario}}}"
                )
            })
            .collect();
        format!("{{\"outcomes\":[{}]}}", items.join(","))
    }

    fn logs<'a>(map: &'a [(&'static str, &'static str)]) -> impl Fn(&str) -> Result<String> + 'a {
        move |path| {
            map.iter()
                .find(|(p, _)| *p == path)
                .map(|(_, text)| text.to_string())
                .context("no such log")
        }
    }

    /// The report names each catch's test and each FLAKY test with its mutant; a missed or unviable mutant with no failed
    /// test gives no line.
    #[test]
    fn the_report_names_each_catch_and_each_flaky_test() {
        let json = outcomes(&[
            ("log/baseline.log", "Success", None),
            ("log/a.log", "CaughtMutant", Some("a.rs:1:1: replace f")),
            ("log/b.log", "MissedMutant", Some("b.rs:2:2: replace g")),
            ("log/c.log", "Unviable", Some("c.rs:3:3: replace h")),
        ]);
        let flaky = "        TRY 1 FAIL [   0.100s] w t::x\n       FLAKY 2/2 [   0.100s] w t::x\n";
        let map = [
            ("log/baseline.log", "        PASS [   0.010s] w t::a\n"),
            ("log/a.log", CATCH),
            ("log/b.log", flaky),
            ("log/c.log", "error: could not compile\n"),
        ];
        let read = logs(&map);
        assert_eq!(
            stage_report(&json, read).unwrap(),
            [
                "caught `a.rs:1:1: replace f` by twostage::slow_triple conf::b".to_string(),
                "FLAKY w t::x under `b.rs:2:2: replace g`: it failed, then passed on its one retry, so it is no catch; the \
                 flake needs a structural fix (BUILD.md Testing rule 9)"
                    .to_string(),
            ]
        );
    }

    /// One case of an error report: the outcomes, the logs by path, and a part of the expected error.
    type Case = (String, Vec<(&'static str, &'static str)>, &'static str);

    /// A catch that names no test, a miss that names a failed test, a retry in the unmutated baseline, an unreadable log
    /// and an outcome without a log path are errors.
    #[test]
    fn a_catch_without_a_test_or_a_retried_baseline_is_an_error() {
        let pass = "        PASS [   0.010s] w t::a\n";
        let retried =
            "        TRY 1 FAIL [   0.100s] w t::a\n       FLAKY 2/2 [   0.100s] w t::a\n";
        let cases: [Case; 4] = [
            (
                outcomes(&[("l", "CaughtMutant", Some("m"))]),
                vec![("l", pass)],
                "names no failed test",
            ),
            (
                outcomes(&[("l", "MissedMutant", Some("m"))]),
                vec![("l", CATCH)],
                "is MISSED, but",
            ),
            (
                outcomes(&[("l", "Success", None)]),
                vec![("l", retried)],
                "never retries",
            ),
            (
                outcomes(&[("l", "Success", None)]),
                vec![],
                "read the log l",
            ),
        ];
        for (json, map, expected) in cases {
            let error = stage_report(&json, logs(&map)).unwrap_err();
            assert!(error.contains(expected), "{expected}: {error}");
        }
        assert!(stage_report("{\"outcomes\":[{\"summary\":\"Success\"}]}", logs(&[])).is_err());
        assert!(stage_report("{}", logs(&[])).is_err());
    }
}

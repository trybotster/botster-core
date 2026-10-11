//! A test that changes state of its whole process (a resource limit, a signal handler, an environment variable) runs alone,
//! in a process of its own, under any runner (lead, 2026-10-10). nextest gives each test its own process, but `cargo test`
//! runs every test of a binary as threads of one process, so the change reached the other tests: a lowered `RLIMIT_NOFILE`
//! made 8 to 15 storage tests fail with `EMFILE`.
//!
//! The form, in the test itself:
//!
//! ```ignore
//! #[test]
//! fn a_test_that_lowers_a_limit() {
//!     const NAME: &str = "storage::tests::a_test_that_lowers_a_limit";
//!     if !botster_test_process::alone::is_alone(NAME) {
//!         return botster_test_process::alone::run_alone(NAME, &[]);
//!     }
//!     // The body: it runs only in the new process.
//! }
//! ```
//!
//! The runner's process starts the test binary again with the test's exact name and [`ALONE`] set to that name, waits for
//! it within [`Deadline::cleanup`], and passes only when exactly that one test ran and passed. A name that matches no test
//! runs nothing, so it fails.

use crate::{run_to_completion, Deadline};
use std::process::Command;

/// The variable that marks the new process: its value is the name of the one test that runs there.
pub const ALONE: &str = "BOTSTER_TEST_ALONE";

/// Whether this process is the new process of [`run_alone`] for the test `name`.
#[must_use]
pub fn is_alone(name: &str) -> bool {
    std::env::var_os(ALONE).is_some_and(|value| value == name)
}

/// Runs the test `name` (its path in the test binary, for example `storage::tests::a_test`) alone, in a new process of the
/// current test binary, with `envs` set in that process only.
///
/// # Panics
/// The new process could not run by [`Deadline::cleanup`], or [`verdict`] fails: the test failed, or not exactly that
/// test ran. The panic shows the new process's output.
pub fn run_alone(name: &str, envs: &[(&str, &str)]) {
    let exe = std::env::current_exe().expect("the test binary");
    let mut command = Command::new(exe);
    command
        .args([name, "--exact", "--test-threads=1", "--nocapture"])
        .env(ALONE, name)
        .envs(envs.iter().copied());
    let output = run_to_completion(&mut command, Deadline::cleanup())
        .unwrap_or_else(|error| panic!("{name} alone: {error}"));
    let text = String::from_utf8_lossy(&output.stdout);
    if let Err(reason) = verdict(output.status.success(), &text) {
        panic!(
            "{name} alone: {reason}\n--- stdout\n{text}\n--- stderr\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// The verdict of the new process from its exit and libtest's summary line: it passes only when the process succeeded and
/// the summary says that one test passed and none failed.
///
/// # Errors
/// The process failed, or its summary is missing or counts another number of tests.
pub fn verdict(success: bool, stdout: &str) -> Result<(), String> {
    if !success {
        return Err("the test failed".into());
    }
    let summary = stdout
        .lines()
        .find(|line| line.starts_with("test result: "))
        .ok_or("libtest printed no summary")?;
    if summary.starts_with("test result: ok. 1 passed; 0 failed;") {
        Ok(())
    } else {
        Err(format!("not exactly one test ran and passed: {summary}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_success_with_one_passed_test_passes() {
        let one = "running 1 test\ntest a ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n";
        assert_eq!(verdict(true, one), Ok(()));
        assert_eq!(verdict(false, one), Err("the test failed".into()));
        let none = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s\n";
        assert!(verdict(true, none)
            .unwrap_err()
            .starts_with("not exactly one test ran and passed: test result: ok. 0 passed;"));
        let two = "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.01s\n";
        assert!(verdict(true, two).is_err());
        let failed = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n";
        assert!(verdict(true, failed).is_err());
        assert_eq!(
            verdict(true, "running 1 test\n"),
            Err("libtest printed no summary".into())
        );
    }

    /// The new process sees `envs` and its own name, and the runner's process does not: the variable never reaches the
    /// other tests of this binary.
    #[test]
    fn the_new_process_alone_sees_its_name_and_variables() {
        const NAME: &str = "alone::tests::the_new_process_alone_sees_its_name_and_variables";
        const VARIABLE: &str = "BOTSTER_TEST_ALONE_PROBE";
        if !is_alone(NAME) {
            assert_eq!(std::env::var_os(VARIABLE), None);
            return run_alone(NAME, &[(VARIABLE, "set")]);
        }
        assert_eq!(std::env::var(VARIABLE).as_deref(), Ok("set"));
        assert!(is_alone(NAME));
        assert!(!is_alone("another::test"));
    }

    /// A name that matches no test of the binary runs nothing in the new process, so the run fails.
    #[test]
    #[should_panic(expected = "not exactly one test ran and passed")]
    fn a_name_that_matches_no_test_fails() {
        run_alone("alone::tests::no_such_test", &[]);
    }
}

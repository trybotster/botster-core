//! `cargo xtask built-in`: only the shared entry makes the build-root test of a test binary (bug catalog B13; design
//! `design-gs-built-in-coverage.md`, Core lead 2026-10-12).
//!
//! The test `test_budget::BUILT_IN_TEST` comes only from `built_in_test!()` and `built_in_trial!()`. Those expand the
//! crate's compile-time name and manifest directory in the calling crate, so no caller passes an argument. This check is a
//! name rule, not a check of the call's form: the name may appear only in the files of `ALLOWED`, each time no more than
//! its count, so a test that is written by hand with that name, with any body, cannot exist. A comment or a string with the
//! name fails too. The count of the shared test crate's file is 2, one for each macro: so that crate cannot also export the
//! name as a constant, which a harness could give to a trial with an empty body (review B306-I2).
//!
//! That every test binary HAS the test is the presence check of the `slow` step (`slow_coverage::presence`), from the
//! `cargo nextest list` of each tier: a list that cargo and nextest make, not a search of the sources.

use crate::test_budget::BUILT_IN_TEST as TEST_NAME;
use anyhow::{Context, Result};
use std::path::Path;

/// The only files that may contain the name, each with the number of times that it may: the two macros that make the
/// test, and xtask's copy, the slow tier's filter, which selects it, and the unit test that pins the filter. No test crate
/// depends on xtask, so its copy gives no harness the name.
pub const ALLOWED: [(&str, usize); 2] = [
    ("crates/botster-test-process/src/built_in.rs", 2),
    ("xtask/src/test_budget.rs", 3),
];

/// The violations of `files` (path relative to the root, text): one line for each file that contains the name more times
/// than [`ALLOWED`] lets it (0 for a file that it does not name).
pub fn violations(files: &[(String, String)]) -> Vec<String> {
    files
        .iter()
        .filter_map(|(path, text)| {
            let count = text.matches(TEST_NAME).count();
            let allowed = ALLOWED
                .iter()
                .find(|(file, _)| file == path)
                .map_or(0, |&(_, allowed)| allowed);
            (count > allowed).then(|| {
                format!(
                    "{path}: names `{TEST_NAME}` {count} times ({allowed} allowed); only \
                     `botster_test_process::built_in_test!()` or `built_in_trial!()` makes that test (bug catalog B13)"
                )
            })
        })
        .collect()
}

/// `cargo xtask built-in`: [`violations`] over the tracked Rust files.
///
/// # Errors
/// A file cannot be read, or a file names the test.
pub fn command(root: &Path, _args: &[String]) -> Result<()> {
    let mut files = Vec::new();
    for path in crate::fsutil::tracked_files(root)? {
        if path.ends_with(".rs") {
            let text = std::fs::read_to_string(root.join(&path))
                .with_context(|| format!("read {path}"))?;
            files.push((path, text));
        }
    }
    crate::tools::verdict(&violations(&files))?;
    println!(
        "built-in: {} Rust files; the test name is only in {}",
        files.len(),
        ALLOWED.map(|(file, _)| file).join(" and ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, text: &str) -> (String, String) {
        (path.to_string(), text.to_string())
    }

    #[test]
    fn only_the_macros_and_the_slow_filter_may_name_the_test() {
        let allowed: Vec<_> = ALLOWED
            .iter()
            .map(|&(path, count)| file(path, &vec![TEST_NAME; count].join("\n")))
            .collect();
        assert_eq!(violations(&allowed), Vec::<String>::new());
        // A third copy in the shared test crate: a constant that a harness could give to a trial with an empty body
        // (review B306-I2).
        let exported = format!(
            "{}\npub const TEST_NAME: &str = \"{TEST_NAME}\";\n",
            allowed[0].1
        );
        let found = violations(&[file(ALLOWED[0].0, &exported)]);
        assert_eq!(found.len(), 1, "{exported}");
        assert!(
            found[0].starts_with("crates/botster-test-process/src/built_in.rs: names")
                && found[0].contains("3 times (2 allowed)"),
            "{}",
            found[0]
        );
        // The macro call does not contain the name.
        assert_eq!(
            violations(&[file(
                "crates/a/src/lib.rs",
                "botster_test_process::built_in_test!();"
            )]),
            Vec::<String>::new()
        );
        for text in [
            // A test written by hand, with any body: the rule does not read the body.
            format!("#[test]\nfn {TEST_NAME}() {{}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    botster_test_process::built_in::assert_built_here(\"a\", \"/x\");\n}}\n"),
            // A trial of that name, a comment and a string.
            format!("Trial::test(\"{TEST_NAME}\", || Ok(()))"),
            format!("// {TEST_NAME}"),
            format!("const N: &str = \"{TEST_NAME}\";"),
        ] {
            let found = violations(&[file("crates/a/tests/t.rs", &text)]);
            assert_eq!(found.len(), 1, "{text}");
            assert!(found[0].starts_with("crates/a/tests/t.rs: names"), "{}", found[0]);
        }
        // A path that only ends like an allowed one is not allowed.
        assert_eq!(
            violations(&[file(
                "x/crates/botster-test-process/src/built_in.rs",
                TEST_NAME
            )])
            .len(),
            1
        );
    }

    /// The command reads the tracked Rust files of the root and fails on a violation (the gate's first mutation run,
    /// a217da82: no test caught `command` -> `Ok(())`).
    #[test]
    fn the_command_fails_on_a_tracked_rust_file_that_names_the_test() {
        let allowed = vec![TEST_NAME; ALLOWED[0].1].join("\n");
        let by_hand = format!("#[test]\nfn {TEST_NAME}() {{}}\n");
        // A file that is not Rust is not read.
        let clean = crate::fsutil::test_repo(&[(ALLOWED[0].0, &allowed), ("notes.md", &by_hand)]);
        command(clean.path(), &[]).unwrap();
        let named = crate::fsutil::test_repo(&[
            (ALLOWED[0].0, &allowed),
            ("crates/a/tests/t.rs", &by_hand),
        ]);
        assert_eq!(
            command(named.path(), &[]).unwrap_err().to_string(),
            "1 violation(s)"
        );
    }

    #[test]
    fn the_slow_filter_selects_the_test_by_its_exact_name() {
        // The other allowed file holds the name only for the slow tier's filter (lead, 2026-10-12).
        assert!(crate::test_budget::SLOW_FILTER.contains(&format!("test(/^{TEST_NAME}$/)")));
    }
}

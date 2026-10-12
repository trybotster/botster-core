//! `cargo xtask built-in`: only the shared entry makes the build-root test of a test binary (bug catalog B13; design
//! `design-gs-built-in-coverage.md`, Core lead 2026-10-12).
//!
//! The test `botster_test_process::built_in::TEST_NAME` comes only from `built_in_test!()` and `built_in_trial!()`. Those
//! expand the crate's compile-time name and manifest directory in the calling crate, so no caller passes an argument. This
//! check is a name rule, not a check of the call's form: the name may appear only in the files of `ALLOWED`, so a test that
//! is written by hand with that name, with any body, cannot exist. A comment or a string with the name fails too.
//!
//! That every test binary HAS the test is the presence check of the `slow` step (`slow_coverage::presence`), from the
//! `cargo nextest list` of each tier: a list that cargo and nextest make, not a search of the sources.

use anyhow::{Context, Result};
use botster_core_testkit::built_in::TEST_NAME;
use std::path::Path;

/// The only files that may contain the name: the macros that make the test, and the slow tier's filter, which selects it.
pub const ALLOWED: [&str; 2] = [
    "crates/botster-test-process/src/built_in.rs",
    "xtask/src/test_budget.rs",
];

/// The violations of `files` (path relative to the root, text): one line for each other file that contains the name.
pub fn violations(files: &[(String, String)]) -> Vec<String> {
    files
        .iter()
        .filter(|(path, text)| text.contains(TEST_NAME) && !ALLOWED.contains(&path.as_str()))
        .map(|(path, _)| {
            format!(
                "{path}: names `{TEST_NAME}`; only `botster_test_process::built_in_test!()` or `built_in_trial!()` makes \
                 that test (bug catalog B13)"
            )
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
        ALLOWED.join(" and ")
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
        let allowed: Vec<_> = ALLOWED.iter().map(|path| file(path, TEST_NAME)).collect();
        assert_eq!(violations(&allowed), Vec::<String>::new());
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

    #[test]
    fn the_slow_filter_selects_the_test_by_its_exact_name() {
        // The other allowed file holds the name only for the slow tier's filter (lead, 2026-10-12).
        assert!(crate::test_budget::SLOW_FILTER.contains(&format!("test(/^{TEST_NAME}$/)")));
    }
}

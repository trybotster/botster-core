//! `cargo xtask mutants-stale`: every mutation exclusion still names a mutant of the tree (lead, 2026-10-11, after P3
//! found the READ_CHUNK entry of botster-worker pinned to a line that the constant had left).
//!
//! An exclusion is a regex over the full mutant name (`<file>:<line>:<column>: <change>`). When code moves, an entry that
//! pins a line, or names a function that the code left, matches nothing: it silently stops excluding, and only a mutation
//! run whose diff reaches that code shows the mutant, as MISSED. The check lists every mutant of the workspace as
//! cargo-mutants generates it (`cargo mutants --list --json --no-config`, which builds nothing and ignores `cfg`, so the
//! list is the same on every gate OS) and fails with each `exclude_re` entry of `.cargo/mutants.toml` and each regex of
//! `ci::OFF_MACOS_EXCLUSIONS` that matches no listed mutant. The derived platform exclusions come from the tree itself and
//! are not checked here.
//!
//! An `exclude_re` entry must also match exactly its count of mutants (the exact-count follow-up; lead, 2026-10-11): one,
//! or the `N` of a trailing `# N mutants` on its line (`gate_decisions::exclusions_of`). A new mutant in an excluded
//! function, or a pattern that grows, changes the count, so the step fails until a reviewer sees the matched mutants and
//! the entry states the new count. The violation lists the mutants that the entry matches. `OFF_MACOS_EXCLUSIONS` is code,
//! with no count note: it keeps the check for at least one mutant.

use crate::gate_decisions::{exclusions_of, parse_mutants};
use crate::mutants_cited::MUTANTS_FILE;
use crate::tools::{cargo, require_cargo_tool};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::Path;
use std::process::Command;

/// The file that an exclusion names: its text before the first `:`, with the regex escapes of `.`, `-` and `/` removed.
fn file_of(pattern: &str) -> String {
    pattern
        .split(':')
        .next()
        .unwrap_or_default()
        .replace("\\.", ".")
        .replace("\\-", "-")
        .replace("\\/", "/")
}

/// The violations: each exclusion of `exclusions` (a pattern and the count of mutants that it must match, if any) that
/// matches none of the mutant names `names` (as cargo-mutants matches an exclusion: a regex search in the full name), or
/// not exactly its count, with its file, and the commit `head` of the listing.
///
/// # Errors
/// An exclusion is not a valid regex.
pub fn stale(
    exclusions: &[(String, Option<usize>)],
    names: &[String],
    head: &str,
) -> Result<Vec<String>> {
    let mut violations = Vec::new();
    for (pattern, count) in exclusions {
        let regex = Regex::new(pattern).with_context(|| format!("the exclusion `{pattern}`"))?;
        let matched: Vec<&str> = names
            .iter()
            .filter(|name| regex.is_match(name))
            .map(String::as_str)
            .collect();
        if matched.is_empty() {
            violations.push(format!(
                "`{pattern}` in {} matches no mutant: 0 matches in cargo mutants --list --no-config at {head}; the \
                 code moved or changed, so correct the entry to name the mutant that its reason justifies, or remove it",
                file_of(pattern)
            ));
        } else if let Some(count) = count.filter(|count| *count != matched.len()) {
            violations.push(format!(
                "`{pattern}` in {} matches {} mutants in cargo mutants --list --no-config at {head}, and its line \
                 states {count}; check each matched mutant against the entry's reason, then correct the pattern or its \
                 `# N mutants` note (none for 1): {}",
                file_of(pattern),
                matched.len(),
                matched.join("; ")
            ));
        }
    }
    Ok(violations)
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    require_cargo_tool(
        root,
        &["mutants", "--version"],
        "cargo install cargo-mutants --version 27.1.0 --locked",
    )?;
    let output = cargo(root)
        .args(["mutants", "--list", "--json", "--no-config"])
        .output()
        .context("run cargo mutants --list")?;
    let names: Vec<String> =
        parse_mutants(&crate::tools::stdout_of(&output, "cargo mutants --list")?)?
            .into_iter()
            .map(|m| m.name)
            .collect();
    let head = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("run git rev-parse")?;
    let head = crate::tools::stdout_of(&head, "git rev-parse")?;
    let text = std::fs::read_to_string(root.join(MUTANTS_FILE))
        .with_context(|| format!("read {MUTANTS_FILE}"))?;
    let patterns: Vec<(String, Option<usize>)> = exclusions_of(&text)?
        .into_iter()
        .map(|e| (e.pattern, e.count))
        .chain(
            crate::ci::OFF_MACOS_EXCLUSIONS
                .iter()
                .map(|re| ((*re).to_string(), None)),
        )
        .collect();
    println!(
        "mutants-stale: {} exclusions checked against {} mutants of the workspace",
        patterns.len(),
        names.len()
    );
    crate::tools::verdict(&stale(&patterns, &names, head.trim())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Exclusions with no count: only the check for at least one mutant.
    fn uncounted(items: &[&str]) -> Vec<(String, Option<usize>)> {
        items.iter().map(|s| (s.to_string(), None)).collect()
    }

    /// An exclusion that matches no listed mutant is stale and named with its file and the commit; one that matches any
    /// mutant passes, whether it pins a line or not.
    #[test]
    fn an_exclusion_that_matches_no_mutant_is_stale() {
        let names = strings(&[
            "crates/w/src/main.rs:53:55: replace * with +",
            "crates/t/src/linux.rs:122:27: replace match guard ended(&e) with true in live_where",
        ]);
        let live = uncounted(&[
            r"crates/w/src/main\.rs:53:55: replace \* with \+$",
            r"crates/t/src/linux\.rs:\d+:\d+: replace match guard ended\(&e\) with (true|false) in live_where$",
            r"main\.rs",
        ]);
        assert_eq!(stale(&live, &names, "abc").unwrap(), Vec::<String>::new());
        let moved = uncounted(&[
            r"crates/w/src/main\.rs:47:55: replace \* with \+$",
            r"crates/t/src/linux\.rs:\d+:\d+: replace match guard ended\(&e\) with (true|false) in live_members$",
        ]);
        let violations = stale(&moved, &names, "abc").unwrap();
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations[0].starts_with(concat!(
                r"`crates/w/src/main\.rs:47:55: replace \* with \+$` in crates/w/src/main.rs matches no mutant: ",
                "0 matches in cargo mutants --list --no-config at abc"
            )),
            "{}",
            violations[0]
        );
        assert!(violations[1].contains("in crates/t/src/linux.rs matches no mutant"));
        assert_eq!(stale(&moved, &[], "abc").unwrap().len(), 2);
        assert!(stale(&uncounted(&["("]), &names, "abc").is_err());
    }

    /// A counted exclusion passes only with exactly its count of matched mutants, and the violation lists each one.
    #[test]
    fn a_counted_exclusion_must_match_exactly_its_count() {
        let names = strings(&[
            "a.rs:1:1: replace f -> bool with true",
            "a.rs:1:1: replace f -> bool with false",
            "a.rs:2:5: replace == with != in f",
        ]);
        let entry = |pattern: &str, count: usize| (pattern.to_string(), Some(count));
        let exact = [
            entry(r"replace f -> bool with (true|false)$", 2),
            entry(r"in f$", 1),
            entry(r"a\.rs", 3),
        ];
        assert_eq!(stale(&exact, &names, "abc").unwrap(), Vec::<String>::new());
        let violations = stale(
            &[entry(r"replace f -> bool with", 1), entry(r"in f$", 2)],
            &names,
            "abc",
        )
        .unwrap();
        assert_eq!(
            violations,
            [
                "`replace f -> bool with` in replace f -> bool with matches 2 mutants in cargo mutants --list \
                 --no-config at abc, and its line states 1; check each matched mutant against the entry's reason, then \
                 correct the pattern or its `# N mutants` note (none for 1): a.rs:1:1: replace f -> bool with true; \
                 a.rs:1:1: replace f -> bool with false",
                "`in f$` in in f$ matches 1 mutants in cargo mutants --list --no-config at abc, and its line states 2; \
                 check each matched mutant against the entry's reason, then correct the pattern or its `# N mutants` \
                 note (none for 1): a.rs:2:5: replace == with != in f",
            ]
        );
        // No match is the stale violation, whatever the count.
        let none = stale(&[entry("b\\.rs", 2)], &names, "abc").unwrap();
        assert_eq!(none.len(), 1);
        assert!(none[0].contains("matches no mutant"), "{}", none[0]);
    }

    #[test]
    fn the_file_of_an_exclusion_is_its_path_without_escapes() {
        assert_eq!(
            file_of(r"crates/botster\-x/src/a\.rs:\d+:\d+: f"),
            "crates/botster-x/src/a.rs"
        );
        assert_eq!(file_of("plain"), "plain");
    }
}

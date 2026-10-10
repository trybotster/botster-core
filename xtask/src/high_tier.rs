//! The check of the HIGH-path list `ci/high-tier-paths.txt` (botster-contracts docs/BUILD.md "Risk tiers", rule 5).
//!
//! An entry that matches no tracked file is a silent gap: the code that it named moved, and a PR that changes it is no longer
//! HIGH. So every entry must match at least one tracked file, and it must give its reason (`# <area>: ...`). An entry is a
//! path, or a directory followed by `/**` (every file under it). Other glob forms are refused, so that a match is exact.

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use std::path::Path;

/// The list, relative to the repository root.
pub const LIST: &str = "ci/high-tier-paths.txt";

/// Whether the entry `pattern` matches the tracked file `file`.
fn matches(pattern: &str, file: &str) -> bool {
    match pattern.strip_suffix("/**") {
        Some(dir) => file
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/')),
        None => pattern == file,
    }
}

/// The problems of the list `text` against the tracked `files`, one line each, with the 1-based line number. No problem: an
/// empty list. A comment line starts with `#`; an empty line is skipped.
pub fn verdict(text: &str, files: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let (pattern, reason) = match line.split_once('#') {
            Some((pattern, reason)) => (pattern.trim(), reason.trim()),
            None => (line, ""),
        };
        if reason.is_empty() {
            problems.push(format!("{LIST}:{number}: `{pattern}` gives no `# reason`"));
        }
        let glob = pattern.strip_suffix("/**").unwrap_or(pattern);
        if glob.contains(['*', '?', '[', ']']) {
            problems.push(format!(
                "{LIST}:{number}: `{pattern}` is not a path or `<dir>/**`"
            ));
        } else if !files.iter().any(|file| matches(pattern, file)) {
            problems.push(format!(
                "{LIST}:{number}: `{pattern}` matches no tracked file"
            ));
        }
    }
    problems
}

/// The outcome of the step: the pass line when the list has no problem, else every problem.
///
/// # Errors
/// The list has a problem.
fn outcome(problems: &[String]) -> Result<&'static str> {
    if !problems.is_empty() {
        bail!("the HIGH-path list has problems:\n{}", problems.join("\n"));
    }
    Ok("high-tier: every entry of ci/high-tier-paths.txt matches a tracked file")
}

/// `cargo xtask high-tier`: the list against `git ls-files`. The I/O shell of [`verdict`] and [`outcome`].
///
/// # Errors
/// The list cannot be read, git fails, or the list has a problem.
pub fn command(root: &Path, _args: &[String]) -> Result<()> {
    let text = std::fs::read_to_string(root.join(LIST)).with_context(|| format!("read {LIST}"))?;
    println!("{}", outcome(&verdict(&text, &tracked_files(root)?))?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn an_entry_that_matches_a_file_with_a_reason_passes() {
        let text = "# A comment.\n\ncrates/a/src/x.rs  # process control: spawn\ncrates/a/src/d/**  # storage: rows\n";
        assert_eq!(
            verdict(text, &files(&["crates/a/src/x.rs", "crates/a/src/d/y.rs"])),
            Vec::<String>::new()
        );
    }

    /// The gap that the check exists for: the named file moved, so the entry matches nothing.
    #[test]
    fn an_entry_that_matches_no_tracked_file_is_a_problem() {
        assert_eq!(
            verdict(
                "crates/a/src/x.rs  # pty\n",
                &files(&["crates/a/src/moved.rs"])
            ),
            vec![format!(
                "{LIST}:1: `crates/a/src/x.rs` matches no tracked file"
            )]
        );
    }

    /// `<dir>/**` matches the files under `dir` only: not `dir` itself as a file, and not a sibling with the same prefix.
    #[test]
    fn a_directory_entry_matches_only_files_under_that_directory() {
        assert!(matches("crates/a/d/**", "crates/a/d/y.rs"));
        assert!(matches("crates/a/d/**", "crates/a/d/e/z.rs"));
        assert!(!matches("crates/a/d/**", "crates/a/d"));
        assert!(!matches("crates/a/d/**", "crates/a/dx/y.rs"));
        assert!(matches("crates/a/x.rs", "crates/a/x.rs"));
        assert!(!matches("crates/a/x.rs", "crates/a/x.rs.bak"));
        assert_eq!(
            verdict("crates/a/d/**  # storage\n", &files(&["crates/a/dx/y.rs"])),
            vec![format!("{LIST}:1: `crates/a/d/**` matches no tracked file")]
        );
    }

    #[test]
    fn an_entry_without_a_reason_is_a_problem() {
        let tracked = files(&["crates/a/x.rs"]);
        let expected = vec![format!("{LIST}:2: `crates/a/x.rs` gives no `# reason`")];
        assert_eq!(verdict("# c\ncrates/a/x.rs\n", &tracked), expected);
        assert_eq!(verdict("# c\ncrates/a/x.rs  #  \n", &tracked), expected);
    }

    /// Any glob form other than a trailing `/**` is refused, even when it would match a file.
    #[test]
    fn a_glob_other_than_a_trailing_directory_wildcard_is_a_problem() {
        let tracked = files(&["crates/a/x.rs", "crates/a/d/y.rs"]);
        for pattern in [
            "crates/a/*.rs",
            "crates/a/x.r?",
            "crates/a/[x].rs",
            "crates/**/y.rs",
        ] {
            assert_eq!(
                verdict(&format!("{pattern}  # pty\n"), &tracked),
                vec![format!("{LIST}:1: `{pattern}` is not a path or `<dir>/**`")],
                "{pattern}"
            );
        }
    }

    /// Every problem of every line is reported, with its own line number.
    #[test]
    fn every_problem_is_reported_with_its_line() {
        let text = "a.rs\n# c\nb.rs  # pty\n";
        assert_eq!(
            verdict(text, &files(&["c.rs"])),
            vec![
                format!("{LIST}:1: `a.rs` gives no `# reason`"),
                format!("{LIST}:1: `a.rs` matches no tracked file"),
                format!("{LIST}:3: `b.rs` matches no tracked file"),
            ]
        );
    }

    /// A list with a problem fails the step with every problem; a list with none passes with its line.
    #[test]
    fn a_list_with_a_problem_fails_the_step_and_one_without_passes() {
        let problems = vec!["one".to_string(), "two".to_string()];
        assert_eq!(
            outcome(&problems).unwrap_err().to_string(),
            "the HIGH-path list has problems:\none\ntwo"
        );
        assert_eq!(
            outcome(&[]).unwrap(),
            format!("high-tier: every entry of {LIST} matches a tracked file")
        );
    }

    /// The repository's own list has no problem. The tree is walked, not read through git: the mutants copy has no `.git`.
    #[test]
    fn the_repository_list_has_no_problem() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let text = std::fs::read_to_string(root.join(LIST)).unwrap();
        // The walk skips dot directories, and `.cargo` holds a listed file (`.cargo/mutants.toml`).
        let mut files = crate::fsutil::walk_files(root);
        files.extend(
            crate::fsutil::walk_files(&root.join(".cargo"))
                .into_iter()
                .map(|file| format!(".cargo/{file}")),
        );
        assert_eq!(verdict(&text, &files), Vec::<String>::new());
    }
}

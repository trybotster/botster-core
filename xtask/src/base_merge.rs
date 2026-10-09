//! `cargo xtask base-merge-check <reviewed-head> <new-head>`: a commit after CLEAN that only merges `v1` needs no reviewer
//! delta round when this check passes (plan revision 22, section 8, "A base-only merge after CLEAN"; the orchestrator's
//! answer to Q7). It proves three conditions:
//!
//! 1. the merge-tree of the reviewed head and the new merge base has no conflict;
//! 2. no path that changed on `v1` since the reviewed merge base is in the pull request's own diff;
//! 3. the pull request's own diff against the new merge base is byte-identical to the reviewed one.
//!
//! A line-set file ([`SET_FILES`]) is left out of conditions 2 and 3 and has condition 4 instead (lead ruling 2026-10-09:
//! every pending flip edits `core-pending.txt`, so two flips failed condition 2 and cost a delta round each):
//!
//! 4. the pull request and the base each only remove whole id lines from the file at the reviewed merge base, the two sets
//!    of removed ids are disjoint, and the file at the new head is that old file without both sets.
//!
//! It also requires that the new head descends from the reviewed head, and that the new merge base descends from the
//! reviewed one (`v1` only moves forward). The base is `BOTSTER_CI_BASE_REF`, else `origin/v1`. The report goes in the pull
//! request. The decisions are pure functions over the outputs of git, with tests; `check` runs git with a configuration
//! that cannot filter a diff, and its tests run it on real repositories with hostile settings.

use crate::fsutil;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const USAGE: &str = "usage: cargo xtask base-merge-check <reviewed-head> <new-head>";

/// The line-set files: lists of whole id lines that a pull request only shrinks, so two removals of other ids commute.
/// Only these paths have condition 4; every other path keeps the byte rules of conditions 2 and 3.
pub const SET_FILES: &[&str] = &["conformance/core-pending.txt"];

/// The text of a line-set file at the four commits; `None` where the file does not exist.
pub struct SetTexts {
    pub path: String,
    /// At the reviewed merge base: the text that both sides change.
    pub old: Option<String>,
    /// At the reviewed head.
    pub reviewed: Option<String>,
    /// At the new merge base.
    pub base: Option<String>,
    /// At the new head.
    pub new: Option<String>,
}

/// What git says about the two heads, gathered by `check`.
pub struct Facts {
    pub reviewed: String,
    pub new: String,
    pub base: String,
    /// The merge base of the reviewed head and the base.
    pub reviewed_base: String,
    /// The merge base of the new head and the base.
    pub new_base: String,
    /// Whether the new head descends from the reviewed head.
    pub new_descends: bool,
    /// Whether the new merge base descends from the reviewed merge base.
    pub base_moved_forward: bool,
    /// The exit code and output of `git merge-tree --write-tree --name-only --no-messages <reviewed> <new-base>`.
    pub merge_tree: (Option<i32>, String),
    /// `git diff --name-only --no-renames <reviewed-base> <new-base>`: the paths that changed on the base.
    pub base_paths: String,
    /// `git diff --name-only --no-renames <reviewed-base> <reviewed>`: the paths of the reviewed diff.
    pub reviewed_paths: String,
    /// `git diff --binary --full-index --no-renames <reviewed-base> <reviewed>`, without the line-set files.
    pub reviewed_diff: Vec<u8>,
    /// `git diff --binary --full-index --no-renames <new-base> <new>`, without the line-set files.
    pub new_diff: Vec<u8>,
    /// Each line-set file of [`SET_FILES`].
    pub set_texts: Vec<SetTexts>,
}

/// Whether `line` (without its line end) is a whole id line: `conf::` and then lowercase letters, digits and `_`.
fn is_id_line(line: &str) -> bool {
    line.strip_prefix("conf::").is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// The id lines that `side` removes from `old` (lines with their line ends).
///
/// # Errors
/// `side` is not `old` with only some id lines removed: it adds or edits a line, or it removes another line.
fn removed<'a>(old: &[&'a str], side: &[&str]) -> Result<BTreeSet<&'a str>, String> {
    let mut gone = BTreeSet::new();
    let mut kept = side.iter().peekable();
    for line in old {
        if kept.peek() == Some(&line) {
            kept.next();
        } else if is_id_line(line.trim_end_matches('\n')) {
            gone.insert(*line);
        } else {
            return Err(format!(
                "it removes or edits the line {:?}",
                line.trim_end_matches('\n')
            ));
        }
    }
    match kept.next() {
        None => Ok(gone),
        Some(line) => Err(format!(
            "it adds or edits the line {:?}",
            line.trim_end_matches('\n')
        )),
    }
}

/// Condition 4 for one line-set file: `reviewed` and `base` each remove only id lines from `old`, the two sets are disjoint,
/// and `new` is `old` without both, in the order of `old`. Returns the number of lines each side removes.
///
/// # Errors
/// `old` repeats an id line (a removal is then ambiguous), a side changes another line, a line is removed on both sides, or
/// `new` is not the merge.
pub fn set_merge(
    old: &str,
    reviewed: &str,
    base: &str,
    new: &str,
) -> Result<(usize, usize), String> {
    let lines: Vec<&str> = old.split_inclusive('\n').collect();
    let mut seen = BTreeSet::new();
    if let Some(twice) = lines
        .iter()
        .map(|line| line.trim_end_matches('\n'))
        .find(|line| is_id_line(line) && !seen.insert(*line))
    {
        return Err(format!("the old file lists {twice} twice"));
    }
    let side = |name: &str, text: &str| {
        removed(&lines, &text.split_inclusive('\n').collect::<Vec<_>>())
            .map_err(|e| format!("the {name} is not a pure removal of id lines: {e}"))
    };
    let (by_pr, by_base) = (side("pull request", reviewed)?, side("base", base)?);
    if let Some(both) = by_pr.intersection(&by_base).next() {
        return Err(format!("both sides remove {}", both.trim_end_matches('\n')));
    }
    let merged: String = lines
        .iter()
        .filter(|line| !by_pr.contains(*line) && !by_base.contains(*line))
        .copied()
        .collect();
    if new != merged {
        return Err("the new head's file is not the old file without both removed sets".into());
    }
    Ok((by_pr.len(), by_base.len()))
}

/// The line of condition 4 for one line-set file, and whether it holds.
fn set_line(texts: &SetTexts) -> (bool, String) {
    let path = &texts.path;
    let outcome = match (&texts.old, &texts.reviewed, &texts.base, &texts.new) {
        (None, None, None, None) => Ok("it exists at none of the four commits".to_string()),
        (Some(old), Some(reviewed), Some(base), Some(new)) => set_merge(old, reviewed, base, new).map(|(pr, base)| {
            format!("the pull request removes {pr} id lines and the base removes {base} others; the new head holds the old file without both")
        }),
        _ => Err("it is missing at one of the four commits".to_string()),
    };
    match outcome {
        Ok(text) => (true, format!("(4) the line-set file {path}: {text}")),
        Err(text) => (false, format!("(4) the line-set file {path}: {text}")),
    }
}

/// The paths with conflicts from the exit code and output of `git merge-tree --write-tree --name-only --no-messages`: exit 0
/// is a clean merge, exit 1 a merge with conflicts whose paths follow the tree id, one per line.
///
/// # Errors
/// git failed (another exit code, or a signal).
pub fn conflicts(code: Option<i32>, output: &str) -> Result<Vec<String>, String> {
    match code {
        Some(0) => Ok(Vec::new()),
        Some(1) => Ok(output
            .lines()
            .skip(1)
            .take_while(|line| !line.is_empty())
            .map(str::to_string)
            .collect()),
        other => Err(format!(
            "git merge-tree failed (exit code {other:?}): {output}"
        )),
    }
}

/// The paths of `pr` (one per line) that also changed on the base (`base`, one per line).
pub fn overlap(base: &str, pr: &str) -> Vec<String> {
    let base: BTreeSet<&str> = base.lines().filter(|l| !l.is_empty()).collect();
    pr.lines()
        .filter(|path| base.contains(path))
        .map(str::to_string)
        .collect()
}

/// The first line (1-based) where the two diffs differ, with both lines, or `None` when they are byte-identical.
pub fn first_difference(reviewed: &[u8], new: &[u8]) -> Option<(usize, String, String)> {
    let a: Vec<&[u8]> = reviewed.split(|b| *b == b'\n').collect();
    let b: Vec<&[u8]> = new.split(|b| *b == b'\n').collect();
    let show = |part: Option<&&[u8]>| {
        part.map_or("(end of the diff)".to_string(), |p| {
            String::from_utf8_lossy(p).into_owned()
        })
    };
    (0..a.len().max(b.len()))
        .find(|&i| a.get(i) != b.get(i))
        .map(|i| (i + 1, show(a.get(i)), show(b.get(i))))
}

/// The sha256 of `bytes`, in hex.
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The verdict: the report for the pull request when every condition holds.
///
/// # Errors
/// A condition fails (the error is the whole report, so the step fails and still shows it), or git merge-tree failed.
pub fn judge(facts: &Facts) -> Result<String, String> {
    let mut report = format!(
        "base-merge-check: reviewed head {} (merge base {}), new head {} (merge base {}), base {}\n",
        facts.reviewed, facts.reviewed_base, facts.new, facts.new_base, facts.base
    );
    let mut pass = true;
    let mut line = |ok: bool, text: String| {
        pass &= ok;
        report.push_str(&format!("{} {text}\n", if ok { "PASS" } else { "FAIL" }));
    };
    line(
        facts.new_descends,
        format!(
            "the new head descends from the reviewed head {}",
            facts.reviewed
        ),
    );
    line(
        facts.base_moved_forward,
        format!(
            "the new merge base descends from the reviewed merge base {}",
            facts.reviewed_base
        ),
    );
    let conflicted = conflicts(facts.merge_tree.0, &facts.merge_tree.1)?;
    line(
        conflicted.is_empty(),
        format!(
            "(1) the merge-tree of the reviewed head and the new merge base has no conflict{}",
            listed(&conflicted)
        ),
    );
    let shared: Vec<String> = overlap(&facts.base_paths, &facts.reviewed_paths)
        .into_iter()
        .filter(|path| !SET_FILES.contains(&path.as_str()))
        .collect();
    line(
        shared.is_empty(),
        format!(
            "(2) none of the {} paths that changed on the base is in the pull request's own diff ({} paths; a line-set file has (4)){}",
            facts.base_paths.lines().count(),
            facts.reviewed_paths.lines().count(),
            listed(&shared)
        ),
    );
    let difference = first_difference(&facts.reviewed_diff, &facts.new_diff);
    line(
        difference.is_none(),
        match &difference {
            None => format!(
                "(3) the pull request's own diff outside the line-set files is byte-identical: {} bytes, sha256 {}",
                facts.reviewed_diff.len(),
                sha256(&facts.reviewed_diff)
            ),
            Some((at, reviewed, new)) => format!(
                "(3) the pull request's own diff differs at line {at}:\n  reviewed: {reviewed}\n  new:      {new}"
            ),
        },
    );
    for texts in &facts.set_texts {
        let (ok, text) = set_line(texts);
        line(ok, text);
    }
    if pass {
        report.push_str("result: PASS (no reviewer delta round is needed; the gate still runs on the new head)\n");
        Ok(report)
    } else {
        report.push_str("result: FAIL (the merge needs delta rounds from both reviewers)");
        Err(report)
    }
}

/// Whether a git command that must succeed did: only exit code 0.
///
/// # Errors
/// Another exit code, or a signal; the error names the command.
pub fn succeeded(args: &[&str], code: Option<i32>) -> Result<(), String> {
    if code == Some(0) {
        Ok(())
    } else {
        Err(format!(
            "git {} failed (exit code {code:?})",
            args.join(" ")
        ))
    }
}

/// `: a, b` for a non-empty list.
fn listed(paths: &[String]) -> String {
    if paths.is_empty() {
        String::new()
    } else {
        format!(": {}", paths.join(", "))
    }
}

/// The answer of `git merge-base --is-ancestor` from its exit code: 0 yes, 1 no.
///
/// # Errors
/// git failed (another exit code, or a signal).
pub fn ancestry(code: Option<i32>) -> Result<bool, String> {
    match code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        other => Err(format!(
            "git merge-base --is-ancestor failed (exit code {other:?})"
        )),
    }
}

/// The options of every diff: plumbing `diff-tree` output that no configuration can filter. Without them, a repository or
/// user setting hides a change and the check passes falsely: `diff.external` or `GIT_EXTERNAL_DIFF` replace the patch,
/// a `textconv` driver rewrites the content, `diff.ignoreSubmodules` drops a gitlink change, `diff.relative` drops the
/// paths outside a directory (#170 review, HIGH).
const CANONICAL: [&str; 6] = [
    "-r",
    "--no-renames",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "--no-relative",
];

/// The facts and the verdict of the check of `new` against `reviewed`, with `base` as the base. git runs without the user's
/// and the system's configuration, and without the environment that changes a diff; the diffs use [`CANONICAL`].
///
/// # Errors
/// git failed, or a condition fails (the error is the report).
pub fn check(root: &Path, reviewed: &str, new: &str, base: &str) -> Result<String> {
    // The exit code and output of `git -C root <args>`.
    let git = |args: &[&str]| -> Result<(Option<i32>, Vec<u8>)> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env_remove("GIT_EXTERNAL_DIFF")
            .env_remove("GIT_DIFF_OPTS")
            // These change how a pathspec is read: `GIT_LITERAL_PATHSPECS` reads the exclusion of a line-set file as a
            // literal path, so the diff of condition 3 is empty and the condition passes falsely.
            .env_remove("GIT_LITERAL_PATHSPECS")
            .env_remove("GIT_GLOB_PATHSPECS")
            .env_remove("GIT_NOGLOB_PATHSPECS")
            .env_remove("GIT_ICASE_PATHSPECS")
            .output()
            .with_context(|| format!("run git {}", args.join(" ")))?;
        Ok((output.status.code(), output.stdout))
    };
    // The output of a git command that must succeed.
    let ok = |args: &[&str]| -> Result<Vec<u8>> {
        let (code, out) = git(args)?;
        succeeded(args, code).map_err(anyhow::Error::msg)?;
        Ok(out)
    };
    let line =
        |args: &[&str]| -> Result<String> { Ok(String::from_utf8(ok(args)?)?.trim().to_string()) };
    let is_ancestor = |ancestor: &str, commit: &str| -> Result<bool> {
        ancestry(git(&["merge-base", "--is-ancestor", ancestor, commit])?.0)
            .map_err(anyhow::Error::msg)
    };
    // The diff of `from` to `to`: all the paths, or the whole binary patch with full blob ids without the line-set files.
    let excluded: Vec<String> = SET_FILES
        .iter()
        .map(|path| format!(":(exclude,literal){path}"))
        .collect();
    let diff = |from: &str, to: &str, names: bool| -> Result<Vec<u8>> {
        let (form, paths): (&[&str], Vec<&str>) = if names {
            (&["--name-only"], Vec::new())
        } else {
            let mut paths = vec!["--"];
            paths.extend(excluded.iter().map(String::as_str));
            (&["-p", "--binary", "--full-index"], paths)
        };
        ok(&[&["diff-tree"][..], &CANONICAL, form, &[from, to], &paths].concat())
    };
    // The text of `path` at `commit`, or `None` when the commit has no such file.
    let text_at = |commit: &str, path: &str| -> Result<Option<String>> {
        if ok(&["ls-tree", "--name-only", commit, "--", path])?.is_empty() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8(ok(&[
            "cat-file",
            "blob",
            &format!("{commit}:{path}"),
        ])?)?))
    };
    let commit = |name: &str| line(&["rev-parse", "--verify", &format!("{name}^{{commit}}")]);
    let (reviewed, new, base) = (commit(reviewed)?, commit(new)?, commit(base)?);
    let reviewed_base = line(&["merge-base", &reviewed, &base])?;
    let new_base = line(&["merge-base", &new, &base])?;
    let (code, out) = git(&[
        "merge-tree",
        "--write-tree",
        "--name-only",
        "--no-messages",
        &reviewed,
        &new_base,
    ])?;
    let set_texts = SET_FILES
        .iter()
        .map(|path| {
            Ok(SetTexts {
                path: path.to_string(),
                old: text_at(&reviewed_base, path)?,
                reviewed: text_at(&reviewed, path)?,
                base: text_at(&new_base, path)?,
                new: text_at(&new, path)?,
            })
        })
        .collect::<Result<_>>()?;
    let facts = Facts {
        set_texts,
        new_descends: is_ancestor(&reviewed, &new)?,
        base_moved_forward: is_ancestor(&reviewed_base, &new_base)?,
        merge_tree: (code, String::from_utf8(out)?),
        base_paths: String::from_utf8(diff(&reviewed_base, &new_base, true)?)?,
        reviewed_paths: String::from_utf8(diff(&reviewed_base, &reviewed, true)?)?,
        reviewed_diff: diff(&reviewed_base, &reviewed, false)?,
        new_diff: diff(&new_base, &new, false)?,
        reviewed,
        new,
        base,
        reviewed_base,
        new_base,
    };
    judge(&facts).map_err(anyhow::Error::msg)
}

/// The command: it resolves the base of the run and prints the report of [`check`].
pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let [reviewed, new] = args else {
        bail!("{USAGE}");
    };
    print!("{}", check(root, reviewed, new, &fsutil::base(root)?)?);
    Ok(())
}

#[cfg(test)]
mod tests;

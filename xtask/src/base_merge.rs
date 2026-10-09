//! `cargo xtask base-merge-check <reviewed-head> <new-head>`: a commit after CLEAN that only merges `v1` needs no reviewer
//! delta round when this check passes (plan revision 22, section 8, "A base-only merge after CLEAN"; the orchestrator's
//! answer to Q7). It proves three conditions:
//!
//! 1. the merge-tree of the reviewed head and the new merge base has no conflict;
//! 2. no path that changed on `v1` since the reviewed merge base is in the pull request's own diff;
//! 3. the pull request's own diff against the new merge base is byte-identical to the reviewed one.
//!
//! It also requires that the new head descends from the reviewed head, and that the new merge base descends from the
//! reviewed one (`v1` only moves forward). The base is `BOTSTER_CI_BASE_REF`, else `origin/v1`. The report goes in the pull
//! request. The decisions are pure functions over the outputs of git, with tests; `command` only runs git and prints.

use crate::fsutil;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const USAGE: &str = "usage: cargo xtask base-merge-check <reviewed-head> <new-head>";

/// What git says about the two heads, gathered by `command`.
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
    /// `git diff --binary --full-index --no-renames <reviewed-base> <reviewed>`.
    pub reviewed_diff: Vec<u8>,
    /// `git diff --binary --full-index --no-renames <new-base> <new>`.
    pub new_diff: Vec<u8>,
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

/// The report for the pull request, and whether every condition holds.
///
/// # Errors
/// git merge-tree failed.
pub fn judge(facts: &Facts) -> Result<(String, bool), String> {
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
    let shared = overlap(&facts.base_paths, &facts.reviewed_paths);
    line(
        shared.is_empty(),
        format!(
            "(2) none of the {} paths that changed on the base is in the pull request's own diff ({} paths){}",
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
                "(3) the pull request's own diff is byte-identical: {} bytes, sha256 {}",
                facts.reviewed_diff.len(),
                sha256(&facts.reviewed_diff)
            ),
            Some((at, reviewed, new)) => format!(
                "(3) the pull request's own diff differs at line {at}:\n  reviewed: {reviewed}\n  new:      {new}"
            ),
        },
    );
    report.push_str(if pass {
        "result: PASS (no reviewer delta round is needed; the gate still runs on the new head)\n"
    } else {
        "result: FAIL (the merge needs delta rounds from both reviewers)\n"
    });
    Ok((report, pass))
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

/// The I/O shell: it runs git, and the pure functions above decide.
pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let [reviewed, new] = args else {
        bail!("{USAGE}");
    };
    // The exit code and output of `git -C root <args>`.
    let git = |args: &[&str]| -> Result<(Option<i32>, Vec<u8>)> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .with_context(|| format!("run git {}", args.join(" ")))?;
        Ok((output.status.code(), output.stdout))
    };
    // The output of a git command that must succeed.
    let ok = |args: &[&str]| -> Result<Vec<u8>> {
        let (code, out) = git(args)?;
        if code != Some(0) {
            bail!("git {} failed (exit code {code:?})", args.join(" "));
        }
        Ok(out)
    };
    let line =
        |args: &[&str]| -> Result<String> { Ok(String::from_utf8(ok(args)?)?.trim().to_string()) };
    let is_ancestor = |ancestor: &str, commit: &str| -> Result<bool> {
        ancestry(git(&["merge-base", "--is-ancestor", ancestor, commit])?.0)
            .map_err(anyhow::Error::msg)
    };
    let reviewed = line(&["rev-parse", "--verify", &format!("{reviewed}^{{commit}}")])?;
    let new = line(&["rev-parse", "--verify", &format!("{new}^{{commit}}")])?;
    let base = fsutil::base(root)?;
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
    let names = ["diff", "--name-only", "--no-renames"];
    let bytes = ["diff", "--binary", "--full-index", "--no-renames"];
    let facts = Facts {
        new_descends: is_ancestor(&reviewed, &new)?,
        base_moved_forward: is_ancestor(&reviewed_base, &new_base)?,
        merge_tree: (code, String::from_utf8(out)?),
        base_paths: String::from_utf8(ok(&[&names[..], &[&reviewed_base, &new_base]].concat())?)?,
        reviewed_paths: String::from_utf8(ok(
            &[&names[..], &[&reviewed_base, &reviewed]].concat()
        )?)?,
        reviewed_diff: ok(&[&bytes[..], &[&reviewed_base, &reviewed]].concat())?,
        new_diff: ok(&[&bytes[..], &[&new_base, &new]].concat())?,
        reviewed,
        new,
        base,
        reviewed_base,
        new_base,
    };
    let (report, pass) = judge(&facts).map_err(anyhow::Error::msg)?;
    print!("{report}");
    if !pass {
        bail!("base-merge-check failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests;

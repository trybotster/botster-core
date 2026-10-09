//! `cargo xtask gate-decisions`: no mutation exclusion covers a gate decision (lead, 2026-10-08; plan r22 review PR1). A
//! decision is a tested pure function; an exclusion may cover only the I/O shell that calls it, and the entry names that
//! decision function (the `mutants_job` entry of #167 is the model).
//!
//! The check lists the mutants of the xtask as cargo-mutants generates them (`cargo mutants --list --json --no-config`,
//! which builds nothing) and applies every exclusion to them: each `exclude_re` entry of `.cargo/mutants.toml` with the
//! comment above it as its reason, each regex of `ci::OFF_MACOS_EXCLUSIONS`, and each `exclude_globs` entry.
//!
//! An exclusion that covers a mutant of xtask function F passes only when both of these hold (#181 B5):
//! - the mutant replaces the whole body of F (genre `FnValue`): an operator or a match-arm mutant is a decision mutant;
//! - its reason cites a proof (plan section 8, r23d), written `D (proof, ..)`, where D is the name of at least one xtask
//!   function that has a mutant, and no exclusion covers a mutant of any xtask function with that name. So every mutant of
//!   D runs in the mutation step, and the run proves that D is tested. mutants-cited checks that each proof is a test that
//!   a gate tier runs. A decision named only in free text does not count.
//!
//! The check reads no source: it does not decide whether F does I/O (plan section 8, 23g), whether F calls D, or whether a
//! test calls D (23i). These rest on the reason of the exclusion and on review: every change to `.cargo/mutants.toml` is
//! HIGH (`ci/high-tier-paths.txt`), so the package reviewer and the integration reviewer each read every new or changed
//! exclusion. The functions are matched by name only (the last segment of the cargo-mutants name, so `Type::d` is a `d`):
//! a name that several functions share only makes the check reject more.
//!
//! Every other exclusion of an xtask mutant fails, a glob or an `OFF_MACOS_EXCLUSIONS` regex included (neither has a
//! reason here).

use crate::mutants_cited::MUTANTS_FILE;
use crate::tools::{cargo, require_cargo_tool};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

/// A mutant as `cargo mutants --list --json` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    pub file: String,
    /// The function as cargo-mutants names it (`f`, `Type::f`, `<impl Trait for Type>::f`); empty outside a function.
    pub function: String,
    pub name: String,
    /// Whether the mutant replaces the whole body of its function (genre `FnValue`).
    pub whole_body: bool,
}

impl Mutant {
    /// The function's own name, the last segment.
    fn short(&self) -> &str {
        self.function.rsplit("::").next().unwrap_or_default()
    }
}

/// An exclusion and its reason (empty when it has none).
#[derive(Clone, Debug)]
pub struct Exclusion {
    pub pattern: String,
    pub reason: String,
}

/// The mutants of a `cargo mutants --list --json` document. A mutant outside a function (a constant) has no function.
///
/// # Errors
/// The document is not the list of mutants.
pub fn parse_mutants(json: &str) -> Result<Vec<Mutant>> {
    let list: serde_json::Value =
        serde_json::from_str(json).context("the mutant list is not JSON")?;
    let mut mutants = Vec::new();
    for item in list.as_array().context("the mutant list is not an array")? {
        let text = |value: &serde_json::Value, what: &str| -> Result<String> {
            value
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("a mutant has no {what}"))
        };
        mutants.push(Mutant {
            file: text(&item["file"], "file")?,
            function: item["function"]["function_name"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            name: text(&item["name"], "name")?,
            whole_body: text(&item["genre"], "genre")? == "FnValue",
        });
    }
    Ok(mutants)
}

/// The `exclude_re` entries of the mutants file, each with the comment block above it (or above its group of entries).
///
/// # Errors
/// An entry line is not a single-quoted string.
pub fn exclusions_of(toml: &str) -> Result<Vec<Exclusion>> {
    let mut entries = Vec::new();
    let mut inside = false;
    let mut reason: Vec<&str> = Vec::new();
    let mut last_was_comment = false;
    for line in toml.lines() {
        let line = line.trim();
        if !inside {
            inside = line.starts_with("exclude_re");
            continue;
        }
        if line == "]" {
            break;
        }
        if let Some(comment) = line.strip_prefix('#') {
            if !last_was_comment {
                reason.clear();
            }
            reason.push(comment.trim());
            last_was_comment = true;
        } else if !line.is_empty() {
            let Some(pattern) = line.strip_prefix('\'').and_then(|l| l.strip_suffix("',")) else {
                bail!("{MUTANTS_FILE}: an exclude_re entry that is not a single-quoted string: {line}");
            };
            entries.push(Exclusion {
                pattern: pattern.to_string(),
                reason: reason.join(" "),
            });
            last_was_comment = false;
        }
    }
    Ok(entries)
}

/// A glob of `exclude_globs` as a regex over a path: `**/` is any directories, `**` and `*` any text (`*` within one
/// component), `?` one character. A glob with no `/` matches the file name in any directory.
fn glob_regex(glob: &str) -> Regex {
    let mut pattern = String::from(if glob.contains('/') { "^" } else { "(^|/)" });
    let mut rest = glob;
    while let Some(c) = rest.chars().next() {
        let (piece, used) = if rest.starts_with("**/") {
            ("(.*/)?".to_string(), 3)
        } else if rest.starts_with("**") {
            (".*".to_string(), 2)
        } else if c == '*' {
            ("[^/]*".to_string(), 1)
        } else if c == '?' {
            ("[^/]".to_string(), 1)
        } else {
            (regex::escape(&c.to_string()), c.len_utf8())
        };
        pattern.push_str(&piece);
        rest = &rest[used..];
    }
    pattern.push('$');
    Regex::new(&pattern).expect("an escaped glob")
}

/// Whether an exclusion covers a mutant.
type Covers = Box<dyn Fn(&Mutant) -> bool>;

/// The violations: each exclusion that covers a decision mutant, or a mutant of a function whose reason cites no decision
/// that names an xtask function of `mutants` and that no exclusion covers. One violation per exclusion, function and kind
/// of mutant (whole body or not).
///
/// # Errors
/// An exclusion is not a valid regex.
pub fn check(
    mutants: &[Mutant],
    exclusions: &[Exclusion],
    globs: &[String],
) -> Result<Vec<String>> {
    let mut all: Vec<(Exclusion, Covers)> = Vec::new();
    for exclusion in exclusions {
        let regex = Regex::new(&exclusion.pattern)
            .with_context(|| format!("the exclusion `{}`", exclusion.pattern))?;
        all.push((
            exclusion.clone(),
            Box::new(move |m: &Mutant| regex.is_match(&m.name)),
        ));
    }
    for glob in globs {
        let regex = glob_regex(glob);
        all.push((
            Exclusion {
                pattern: glob.clone(),
                reason: String::new(),
            },
            Box::new(move |m: &Mutant| regex.is_match(&m.file)),
        ));
    }
    let covered: BTreeSet<&str> = mutants
        .iter()
        .filter(|m| all.iter().any(|(_, covers)| covers(m)))
        .map(Mutant::short)
        .collect();
    let mut seen = BTreeSet::new();
    let mut violations = Vec::new();
    for mutant in mutants {
        for (exclusion, covers) in &all {
            if !covers(mutant)
                || !seen.insert((
                    exclusion.pattern.clone(),
                    mutant.file.clone(),
                    mutant.function.clone(),
                    mutant.whole_body,
                ))
            {
                continue;
            }
            // Plan section 8 (r23d, 23i): only a proof citation `decision (proof, ..)` names a decision; a name in free
            // text does not. A cited decision has mutants and none of them is excluded, so the mutation run tests it;
            // F itself is covered, so it never counts as its own decision.
            let named = crate::mutants_cited::citations(&exclusion.reason)
                .iter()
                .map(|citation| citation.decision.as_str())
                .any(|d| !covered.contains(d) && mutants.iter().any(|m| m.short() == d));
            let problem = if !mutant.whole_body {
                "it is a decision mutant: an exclusion covers only the whole-body replacement of an I/O shell"
            } else if !named {
                "its reason cites, as `decision (proof, ..)`, no xtask function with mutants of which no exclusion \
                 covers any"
            } else {
                continue;
            };
            violations.push(format!(
                "`{}` excludes {} in {} (`{}`): {problem}",
                exclusion.pattern,
                if mutant.function.is_empty() {
                    "code outside a function"
                } else {
                    &mutant.function
                },
                mutant.file,
                mutant.name
            ));
        }
    }
    Ok(violations)
}

/// What the check reads from a repository: the `exclude_re` entries of the mutants file with their reasons and the
/// off-macOS exclusions of the gate, and the `exclude_globs`.
struct Inputs {
    exclusions: Vec<Exclusion>,
    globs: Vec<String>,
}

/// The inputs of the check in `root`.
///
/// # Errors
/// The mutants file cannot be read or parsed, or an `exclude_re` entry has no reason block.
fn inputs(root: &Path) -> Result<Inputs> {
    let text = std::fs::read_to_string(root.join(MUTANTS_FILE))
        .with_context(|| format!("read {MUTANTS_FILE}"))?;
    let config: toml::Table = text
        .parse()
        .with_context(|| format!("parse {MUTANTS_FILE}"))?;
    let globs: Vec<String> = config
        .get("exclude_globs")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut exclusions = exclusions_of(&text)?;
    let configured = config
        .get("exclude_re")
        .and_then(toml::Value::as_array)
        .map_or(0, Vec::len);
    if exclusions.len() != configured {
        bail!("{MUTANTS_FILE}: {} exclude_re entries read with their reasons, {configured} configured", exclusions.len());
    }
    exclusions.extend(crate::ci::OFF_MACOS_EXCLUSIONS.iter().map(|re| Exclusion {
        pattern: (*re).to_string(),
        reason: String::new(),
    }));
    Ok(Inputs { exclusions, globs })
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
        .args(["mutants", "--list", "--json", "--no-config", "-p", "xtask"])
        .output()
        .context("run cargo mutants --list")?;
    let mutants = parse_mutants(&crate::tools::stdout_of(&output, "cargo mutants --list")?)?;
    let inputs = inputs(root)?;
    println!(
        "gate-decisions: {} mutants of the xtask, {} exclude_re entries, {} exclude_globs",
        mutants.len(),
        inputs.exclusions.len(),
        inputs.globs.len()
    );
    crate::tools::verdict(&check(&mutants, &inputs.exclusions, &inputs.globs)?)
}

#[cfg(test)]
mod tests;

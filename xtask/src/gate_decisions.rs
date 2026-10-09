//! `cargo xtask gate-decisions`: no mutation exclusion covers a gate decision (lead, 2026-10-08). The gate is the xtask, so
//! every function of the xtask decides a step's pass or fail, except the reviewed glue in `GATE_GLUE` that only starts a
//! process, reads a file or prints (the rule of `.cargo/mutants.toml`: "Every decision of xtask is mutation-tested").
//!
//! The check lists the mutants of the xtask as cargo-mutants generates them (`cargo mutants --list --json --no-config`,
//! which builds nothing) and applies every exclusion to them: each `exclude_re` entry and each `exclude_globs` entry of
//! `.cargo/mutants.toml`, and each regex of `ci::OFF_MACOS_EXCLUSIONS`. An exclusion that matches a mutant of a function
//! outside `GATE_GLUE` fails the check, and so does a `GATE_GLUE` entry with no mutant (a renamed or deleted function).

use crate::mutants_cited::MUTANTS_FILE;
use crate::tools::{cargo, require_cargo_tool};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::Path;

/// The xtask functions that only start a process, read a file or print, by file and function name. Each has a reviewed
/// exclusion with its argument in `.cargo/mutants.toml`; its decisions are pure functions that stay mutation-tested.
const GATE_GLUE: [(&str, &str); 1] = [("xtask/src/ci.rs", "mutants_job")];

/// A mutant as `cargo mutants --list --json` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    pub file: String,
    pub function: String,
    pub name: String,
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
        });
    }
    Ok(mutants)
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

/// The violations: each exclusion that covers a mutant of a gate decision, once per function, and each `GATE_GLUE` entry
/// that names no function of `mutants`.
///
/// # Errors
/// An exclusion is not a valid regex.
pub fn check(
    mutants: &[Mutant],
    exclude_re: &[String],
    exclude_globs: &[String],
    glue: &[(&str, &str)],
) -> Result<Vec<String>> {
    let mut violations = Vec::new();
    let mut regexes = Vec::new();
    for entry in exclude_re {
        regexes.push((
            entry.clone(),
            Regex::new(entry).with_context(|| format!("the exclusion `{entry}`"))?,
        ));
    }
    let globs: Vec<(String, Regex)> = exclude_globs
        .iter()
        .map(|g| (g.clone(), glob_regex(g)))
        .collect();
    let mut reported = std::collections::BTreeSet::new();
    for mutant in mutants {
        if glue.contains(&(mutant.file.as_str(), mutant.function.as_str())) {
            continue;
        }
        let by_re = regexes
            .iter()
            .filter(|(_, re)| re.is_match(&mutant.name))
            .map(|(entry, _)| entry);
        let by_glob = globs
            .iter()
            .filter(|(_, re)| re.is_match(&mutant.file))
            .map(|(glob, _)| glob);
        for exclusion in by_re.chain(by_glob) {
            if reported.insert((
                exclusion.clone(),
                mutant.file.clone(),
                mutant.function.clone(),
            )) {
                violations.push(format!(
                    "`{exclusion}` excludes the gate decision {} in {} (`{}`): a gate decision is never excluded",
                    if mutant.function.is_empty() { "outside a function" } else { &mutant.function },
                    mutant.file,
                    mutant.name
                ));
            }
        }
    }
    for (file, function) in glue {
        if !mutants
            .iter()
            .any(|m| m.file == *file && m.function == *function)
        {
            violations.push(format!("GATE_GLUE names {function} in {file}, which has no mutant: remove or rename the entry"));
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
        .args(["mutants", "--list", "--json", "--no-config", "-p", "xtask"])
        .output()
        .context("run cargo mutants --list")?;
    if !output.status.success() {
        bail!(
            "cargo mutants --list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mutants = parse_mutants(&String::from_utf8(output.stdout)?)?;
    let config: toml::Table = std::fs::read_to_string(root.join(MUTANTS_FILE))
        .with_context(|| format!("read {MUTANTS_FILE}"))?
        .parse()
        .with_context(|| format!("parse {MUTANTS_FILE}"))?;
    let strings = |key: &str| -> Vec<String> {
        config
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let mut exclude_re = strings("exclude_re");
    exclude_re.extend(
        crate::ci::OFF_MACOS_EXCLUSIONS
            .iter()
            .map(|re| (*re).to_string()),
    );
    let violations = check(&mutants, &exclude_re, &strings("exclude_globs"), &GATE_GLUE)?;
    for violation in &violations {
        eprintln!("{violation}");
    }
    println!(
        "gate-decisions: {} mutants of the xtask, {} exclusions",
        mutants.len(),
        exclude_re.len()
    );
    if !violations.is_empty() {
        bail!("{} violation(s)", violations.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mutant(function: &str, name: &str) -> Mutant {
        Mutant {
            file: "xtask/src/ci.rs".into(),
            function: function.into(),
            name: format!("xtask/src/ci.rs:10:5: {name}"),
        }
    }

    fn fixture() -> Vec<Mutant> {
        vec![
            mutant(
                "mutants_job",
                "replace mutants_job -> Result<()> with Ok(())",
            ),
            mutant(
                "mutation_verdict",
                "replace mutation_verdict -> Result<()> with Ok(())",
            ),
            mutant("mutation_verdict", "replace == with != in mutation_verdict"),
        ]
    }

    const GLUE: [(&str, &str); 1] = [("xtask/src/ci.rs", "mutants_job")];

    /// The red-on-revert proof: an exclusion of the step's verdict fails the check; the reviewed glue entry passes.
    #[test]
    fn an_exclusion_of_a_gate_decision_fails_and_one_of_glue_passes() {
        let glue_entry =
            r"xtask/src/ci\.rs:\d+:\d+: replace mutants_job -> Result<\(\)> with Ok\(\(\)\)$"
                .to_string();
        assert!(check(&fixture(), &[glue_entry.clone()], &[], &GLUE)
            .unwrap()
            .is_empty());
        let verdict =
            r"xtask/src/ci\.rs:\d+:\d+: replace == with != in mutation_verdict$".to_string();
        assert_eq!(
            check(&fixture(), &[glue_entry, verdict.clone()], &[], &GLUE).unwrap(),
            [format!(
                "`{verdict}` excludes the gate decision mutation_verdict in xtask/src/ci.rs \
                 (`xtask/src/ci.rs:10:5: replace == with != in mutation_verdict`): a gate decision is never excluded"
            )]
        );
    }

    #[test]
    fn a_broad_exclusion_is_reported_once_per_decision() {
        let broad = vec![r"xtask/src/ci\.rs".to_string()];
        let found = check(&fixture(), &broad, &[], &GLUE).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("mutation_verdict"));
    }

    #[test]
    fn a_glob_that_covers_a_decision_fails_and_the_test_globs_pass() {
        let tests = vec!["**/tests/**".to_string(), "tests/**".to_string()];
        assert!(check(&fixture(), &[], &tests, &GLUE).unwrap().is_empty());
        for glob in ["xtask/**", "ci.rs", "xtask/src/*.rs", "**/ci.rs"] {
            let found = check(&fixture(), &[], &[glob.to_string()], &GLUE).unwrap();
            assert_eq!(found.len(), 1, "{glob}: {found:?}");
        }
    }

    #[test]
    fn a_glue_entry_with_no_function_fails() {
        let found = check(&fixture()[1..], &[], &[], &GLUE).unwrap();
        assert_eq!(
            found,
            ["GATE_GLUE names mutants_job in xtask/src/ci.rs, which has no mutant: remove or rename the entry"]
        );
    }

    #[test]
    fn an_exclusion_that_is_not_a_regex_fails() {
        assert!(check(&fixture(), &["(".to_string()], &[], &GLUE).is_err());
    }

    #[test]
    fn the_mutant_list_is_read_from_the_json_of_cargo_mutants() {
        let json = r#"[{"file":"xtask/src/main.rs","function":{"function_name":"main","return_type":""},
            "name":"xtask/src/main.rs:37:5: replace main with ()","package":"xtask"},
            {"file":"xtask/src/a.rs","name":"xtask/src/a.rs:1:1: replace * with + "}]"#;
        assert_eq!(
            parse_mutants(json).unwrap(),
            [
                Mutant {
                    file: "xtask/src/main.rs".into(),
                    function: "main".into(),
                    name: "xtask/src/main.rs:37:5: replace main with ()".into()
                },
                Mutant {
                    file: "xtask/src/a.rs".into(),
                    function: String::new(),
                    name: "xtask/src/a.rs:1:1: replace * with + ".into()
                },
            ]
        );
        assert!(parse_mutants("{}").is_err());
        assert!(parse_mutants(r#"[{"file":"a"}]"#).is_err());
    }
}

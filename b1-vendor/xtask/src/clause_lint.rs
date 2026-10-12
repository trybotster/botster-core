//! `cargo xtask clause-lint`: every clause citation in a fake crate names a clause that exists in the ledger.
//!
//! Citation syntax: a comment (`//`, `///` or `//!`) whose text starts with a contract prefix and a clause id.
//! More clauses may follow after commas, and a comma may start another prefix:
//!
//! ```text
//! // HC EN-8
//! // Core OR-3, OR-4
//! /// Clause: HC EN-1, HC A.1, Core 9B
//! ```
//!
//! Prefixes: `Core`, `Codec`, `HC`, `HP`, `LB`, `UI`, `CLI`. The first clause id has at least one digit. A later clause id in a list has a digit and a letter.
//! A comment that starts with a prefix and no clause id (for example `// HC frames`) is not a citation.
//! A clause is valid for a prefix when a contract file of that prefix holds a clause row or a heading with that id, and the ledger has a test id under it.
//! Scanned: every `.rs` file in `crates/botster-fake-*` and `crates/botster-plugin-fakes`.

use crate::config::Config;
use crate::fsutil::files_under;
use crate::ledger;
use anyhow::{bail, Result};
use std::collections::BTreeSet;
use std::path::Path;

const PREFIXES: [(&str, &str); 7] = [
    ("Core", "core"),
    ("Codec", "codec"),
    ("HC", "hc"),
    ("HP", "hp"),
    ("LB", "lb"),
    ("UI", "ui"),
    ("CLI", "cli"),
];

fn is_token(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn tokenize(body: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for c in body.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            word.push(c);
        } else {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            if c == ',' {
                tokens.push(",".to_string());
            } else if !c.is_whitespace() {
                tokens.push(c.to_string());
            }
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
        .into_iter()
        .map(|t| t.trim_end_matches('.').to_string())
        .collect()
}

fn contract_of(token: &str) -> Option<&'static str> {
    PREFIXES.iter().find(|(p, _)| *p == token).map(|(_, c)| *c)
}

/// The citations in one comment body: (ledger prefix, clause) pairs.
pub fn citations_in(body: &str) -> Vec<(String, String)> {
    let tokens = tokenize(body);
    let mut at = 0;
    if tokens.first().is_some_and(|t| t == "Clause") && tokens.get(1).is_some_and(|t| t == ":") {
        at = 2;
    }
    let Some(mut contract) = tokens.get(at).and_then(|t| contract_of(t)) else {
        return Vec::new();
    };
    let Some(first) = tokens
        .get(at + 1)
        .filter(|t| is_token(t) && t.chars().any(|c| c.is_ascii_digit()))
    else {
        return Vec::new();
    };
    let mut out = vec![(contract.to_string(), first.clone())];
    let mut i = at + 2;
    while tokens.get(i).is_some_and(|t| t == ",") {
        let Some(next) = tokens.get(i + 1) else { break };
        if let Some(c) = contract_of(next) {
            let Some(clause) = tokens
                .get(i + 2)
                .filter(|t| is_token(t) && t.chars().any(|c| c.is_ascii_digit()))
            else {
                break;
            };
            contract = c;
            out.push((contract.to_string(), clause.clone()));
            i += 3;
        } else if is_token(next)
            && next.chars().any(|c| c.is_ascii_digit())
            && next.chars().any(|c| c.is_ascii_alphabetic())
        {
            out.push((contract.to_string(), next.clone()));
            i += 2;
        } else {
            break;
        }
    }
    out
}

fn comment_body(line: &str) -> Option<&str> {
    let at = line.find("//")?;
    let rest = &line[at + 2..];
    Some(
        rest.strip_prefix('/')
            .or_else(|| rest.strip_prefix('!'))
            .unwrap_or(rest)
            .trim(),
    )
}

fn fake_crate_files(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root.join("crates"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("botster-fake-") || name == "botster-plugin-fakes" {
            out.extend(
                files_under(root, &entry.path())?
                    .into_iter()
                    .filter(|f| f.ends_with(".rs")),
            );
        }
    }
    out.sort();
    Ok(out)
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let config = Config::load(root)?;
    let (entries, occurrences) = ledger::build(root, &config)?;
    let mut valid: BTreeSet<(String, String)> = BTreeSet::new();
    for occ in &occurrences {
        if let Some(clause) = &occ.clause {
            valid.insert((occ.contract.clone(), clause.clone()));
        }
    }
    for entry in &entries {
        valid.insert((entry.contract.clone(), entry.clause.clone()));
    }
    let mut checked = 0;
    let mut problems = Vec::new();
    for file in fake_crate_files(root)? {
        let text = std::fs::read_to_string(root.join(&file))?;
        for (index, line) in text.lines().enumerate() {
            let Some(body) = comment_body(line) else {
                continue;
            };
            for (contract, clause) in citations_in(body) {
                checked += 1;
                if !valid.contains(&(contract.clone(), clause.clone())) {
                    problems.push(format!(
                        "{file}:{}: no clause '{clause}' in contract '{contract}'",
                        index + 1
                    ));
                }
            }
        }
    }
    println!("clause-lint: {checked} citation(s) checked");
    if problems.is_empty() {
        return Ok(());
    }
    let summary = format!("{} unknown citation(s)", problems.len());
    Err(crate::Problems::fail(problems, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cite(body: &str) -> Vec<(String, String)> {
        citations_in(body)
    }

    #[test]
    fn single_citation() {
        assert_eq!(cite("HC EN-8"), vec![("hc".into(), "EN-8".into())]);
        assert_eq!(
            cite("Core OR-3, OR-4 and more"),
            vec![
                ("core".into(), "OR-3".into()),
                ("core".into(), "OR-4".into())
            ]
        );
    }

    #[test]
    fn clause_leader_and_prefix_switch() {
        assert_eq!(
            cite("Clause: HC EN-1, HC A.1, Core 9B."),
            vec![
                ("hc".into(), "EN-1".into()),
                ("hc".into(), "A.1".into()),
                ("core".into(), "9B".into())
            ]
        );
    }

    #[test]
    fn prose_is_not_a_citation() {
        assert!(cite("HC frames are sent").is_empty());
        assert!(cite("HC section 12 says").is_empty());
        assert!(cite("Core API").is_empty());
        assert!(cite("the HC EN-8 rule").is_empty());
    }

    #[test]
    fn trailing_dot_is_dropped() {
        assert_eq!(cite("HP WF-1."), vec![("hp".into(), "WF-1".into())]);
    }
}

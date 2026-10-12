//! Extracts every `conf::<id>` from the current contract files, with the clause and the section that hold it.
//!
//! Attribution rules, applied line by line within a file:
//! - A heading starts a new section and clears the clause. When the heading text starts with a clause id (for example `## A2-5. Services`), that id is the clause.
//! - A table row whose first cell is a clause id sets the row clause (for example `| EN-8 | ...`, `| A2-1 (B2-1) | ...`).
//!   A row can span several lines. Its continuation lines do not start with `|`, so they keep the row clause.
//!   A line that starts with `|` and has two or more cells, and whose first cell is not a clause id (a header, a separator, a row keyed by something else), clears the row clause.
//!   A line that starts with `|` and has one cell is a continuation of the row above, and keeps the row clause.
//! - A bullet or a line that starts with a bold clause id sets the section clause (for example `- **LB-2 Arguments.**`) and clears the row clause.
//! - An id takes the row clause, or else the section clause. An id with neither gets no clause, and the ledger applies `contracts.toml` overrides.
//!   The extractor never lends the clause of an earlier row to a row that is keyed by something else.
//!
//! A clause id has letters, digits, `-` and `.`, and at least one digit. A `conf::` token that ends in `_` is a wildcard mention, not an id.

use crate::config::Config;
use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Occurrence {
    pub id: String,
    /// The ledger prefix of the contract that holds the file.
    pub contract: String,
    pub file: String,
    pub clause: Option<String>,
    pub section: String,
    pub line: usize,
}

struct Patterns {
    id: Regex,
    row: Regex,
    heading: Regex,
    heading_clause: Regex,
    bullet: Regex,
    bold: Regex,
}

fn patterns() -> Patterns {
    let named = r"[A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+";
    Patterns {
        id: Regex::new(r"conf::[A-Za-z0-9_]+").unwrap(),
        row: Regex::new(r"^\|\s*([A-Za-z0-9][A-Za-z0-9.\-]*)(?: \([^)]*\))?\s*\|").unwrap(),
        heading: Regex::new(r"^#{1,6}\s+(.*)$").unwrap(),
        heading_clause: Regex::new(&format!(r"^(?:\*\*)?({named}|[A-Z]\.\d+[A-Za-z]?)\b")).unwrap(),
        bullet: Regex::new(&format!(r"^[-*]\s+\*\*({named})\b")).unwrap(),
        bold: Regex::new(&format!(r"^\*\*({named})\b")).unwrap(),
    }
}

fn is_clause_id(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
}

/// Every occurrence in the current files, in owner order, then file order, then line order.
pub fn extract(root: &Path, config: &Config) -> Result<Vec<Occurrence>> {
    let pat = patterns();
    let mut out = Vec::new();
    for contract in &config.contracts {
        for file in &contract.files {
            let path = root.join(&config.dir).join(file);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            extract_file(&pat, &contract.prefix, file, &text, &mut out);
        }
    }
    Ok(out)
}

/// Every (contract prefix, clause id) that has a table row (`| <clause> |`) in the current files, whether or not the row lists a test id.
/// A clause that only withdraws an id lists none, yet it is an authority (conformance/withdrawn.txt).
pub fn clause_rows(
    root: &Path,
    config: &Config,
) -> Result<std::collections::BTreeSet<(String, String)>> {
    let pat = patterns();
    let mut out = std::collections::BTreeSet::new();
    for contract in &config.contracts {
        for file in &contract.files {
            let path = root.join(&config.dir).join(file);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            for line in text.lines() {
                if let Some(c) = pat.row.captures(line) {
                    if is_clause_id(&c[1]) {
                        out.insert((contract.prefix.to_ascii_lowercase(), c[1].to_string()));
                    }
                }
            }
        }
    }
    Ok(out)
}

fn extract_file(pat: &Patterns, prefix: &str, file: &str, text: &str, out: &mut Vec<Occurrence>) {
    let mut section = String::new();
    let mut marker: Option<String> = None;
    let mut row: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        if let Some(h) = pat.heading.captures(line) {
            let heading = h[1].trim().to_string();
            marker = pat
                .heading_clause
                .captures(&heading)
                .filter(|c| is_clause_id(&c[1]))
                .map(|c| c[1].to_string());
            row = None;
            section = heading;
        } else if line.starts_with('|') {
            let clause_row = pat.row.captures(line).filter(|c| is_clause_id(&c[1]));
            if let Some(c) = clause_row {
                row = Some(c[1].to_string());
            } else if line.matches('|').count() >= 3 {
                row = None;
            }
        } else if let Some(c) = pat
            .bullet
            .captures(line)
            .or_else(|| pat.bold.captures(line))
            .filter(|c| is_clause_id(&c[1]))
        {
            if row.is_none() {
                marker = Some(c[1].to_string());
            }
        }
        for m in pat.id.find_iter(line) {
            if m.as_str().ends_with('_') {
                continue;
            }
            out.push(Occurrence {
                id: m.as_str().to_string(),
                contract: prefix.to_string(),
                file: file.to_string(),
                clause: row.clone().or_else(|| marker.clone()),
                section: section.clone(),
                line: index + 1,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Vec<(String, Option<String>, String)> {
        let mut out = Vec::new();
        extract_file(&patterns(), "hc", "f.md", text, &mut out);
        out.into_iter()
            .map(|o| (o.id, o.clause, o.section))
            .collect()
    }

    fn clause_of(text: &str, id: &str) -> Option<String> {
        run(text).into_iter().find(|(i, _, _)| i == id).unwrap().1
    }

    #[test]
    fn a_row_clause_covers_the_continuation_lines_of_its_cell() {
        let text = "## 5. Section\n| EN-8 | Text\n- a list line\n<br>more | `conf::a` |\n- `conf::b`\n| `conf::c`, `conf::d` |\n";
        for id in ["conf::a", "conf::b", "conf::c", "conf::d"] {
            assert_eq!(clause_of(text, id).as_deref(), Some("EN-8"), "{id}");
        }
    }

    #[test]
    fn a_row_with_a_parenthesized_second_id_takes_the_first() {
        assert_eq!(
            clause_of("| A2-1 (B2-1) | text | `conf::a` |\n", "conf::a").as_deref(),
            Some("A2-1")
        );
    }

    #[test]
    fn a_row_keyed_by_something_else_does_not_borrow_the_earlier_clause() {
        let text = "| EN-8 | text | `conf::a` |\n| `Code` | caller | `conf::b` |\n";
        assert_eq!(clause_of(text, "conf::a").as_deref(), Some("EN-8"));
        assert_eq!(clause_of(text, "conf::b"), None);
    }

    #[test]
    fn a_heading_clause_covers_the_prose_tests_line() {
        let text = "## A2-5. Services\n\nProse.\n\nTests: `conf::a`, `conf::b`.\n\n## Other\n\nTests: `conf::c`.\n";
        assert_eq!(clause_of(text, "conf::a").as_deref(), Some("A2-5"));
        assert_eq!(clause_of(text, "conf::b").as_deref(), Some("A2-5"));
        assert_eq!(
            clause_of(text, "conf::c"),
            None,
            "a new heading clears the clause"
        );
    }

    #[test]
    fn a_bold_or_bullet_clause_marker_sets_the_clause() {
        let text = "- **LB-2 Arguments.** Test: `conf::a`.\n**TI-1 identity.** `conf::b`\n";
        assert_eq!(clause_of(text, "conf::a").as_deref(), Some("LB-2"));
        assert_eq!(clause_of(text, "conf::b").as_deref(), Some("TI-1"));
    }

    #[test]
    fn a_bold_clause_paragraph_inside_a_cell_keeps_the_row_clause() {
        let text =
            "| EN-7 | **Effect.** text\n\n**EN-7b. Ids.** more\n- item | `conf::a`, `conf::b` |\n";
        assert_eq!(clause_of(text, "conf::a").as_deref(), Some("EN-7"));
        assert_eq!(clause_of(text, "conf::b").as_deref(), Some("EN-7"));
    }

    #[test]
    fn the_frozen_en7_row_gives_en7_to_its_ids() {
        let text = include_str!("../../frozen/final6/hub-client-contract-v1-candidate-parity45.md");
        let rows = run(text);
        let (_, clause, _) = rows
            .iter()
            .find(|(i, _, _)| i == "conf::en_7_committed_survives_crash")
            .unwrap();
        assert_eq!(clause.as_deref(), Some("EN-7"));
    }

    #[test]
    fn a_hyphenated_word_without_a_digit_is_not_a_clause() {
        let text = "- **Package-root containment.** `conf::a`\n";
        assert_eq!(clause_of(text, "conf::a"), None);
    }

    #[test]
    fn a_wildcard_mention_is_not_an_id() {
        let ids: Vec<_> =
            run("| X-1 | `conf::cli_*` and `conf::a_9b_<kind>` and `conf::ok_id` |\n")
                .into_iter()
                .map(|t| t.0)
                .collect();
        assert_eq!(ids, vec!["conf::ok_id"]);
    }

    #[test]
    fn the_section_is_the_nearest_heading() {
        let rows = run("# Title\n## 3.1 Name\n| X-1 | `conf::a` |\n");
        assert_eq!(rows[0].2, "3.1 Name");
    }
}

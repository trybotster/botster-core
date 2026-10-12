//! `conformance/deferred.txt` and `conformance/withdrawn.txt`: the two ledger states beside "has a transcript" and "is pending".
//!
//! A ruling can put an id in one of two more states, each with its authority:
//!
//! - **deferred** (Core A6-2): the id waits for a release condition. One line per id:
//!   `conf::<id>  <reason>  (<Contract clause>)  until: <end condition>`.
//!   The same file may hold **not-applicable case** records, which are not ids and leave the id active:
//!   `not-applicable conf::<id>  <case>  (<Contract clause>)  because: <why>`.
//! - **withdrawn** (Core A9-2): the id is withdrawn and replaced. One line per id:
//!   `conf::<id>  withdrawn by <Contract clause>  replaced by conf::<id>|none`.
//!
//! The fields are separated by two or more spaces. `#` lines and blank lines are skipped. An authority is qualified by its contract
//! (`Core A6-2`): the ledger must have a row of that contract with that clause, or a current file of that contract must have a table
//! row for the clause that lists no test id (a clause that only withdraws, such as HP A3-2). Another contract's `A6-2` cannot authorize
//! a record.

use crate::ledger::Entry;
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::Path;

pub const DEFERRED_PATH: &str = "conformance/deferred.txt";
pub const WITHDRAWN_PATH: &str = "conformance/withdrawn.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deferred {
    pub id: String,
    pub reason: String,
    pub authority: String,
    pub until: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotApplicable {
    pub id: String,
    pub case: String,
    pub authority: String,
    pub because: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawn {
    pub id: String,
    pub authority: String,
    /// `None` is written `none`.
    pub replacement: Option<String>,
    pub line: String,
}

/// The fields of a line: the text between runs of two or more spaces.
fn fields(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            out.push(&line[start..i]);
            start = j;
            i = j;
        } else {
            i += 1;
        }
    }
    out.push(&line[start..]);
    out.into_iter().filter(|f| !f.is_empty()).collect()
}

/// The records of a file: the non-comment, non-blank lines, trimmed.
fn records(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// `(Core A6-2)` gives `Core A6-2`.
fn authority_of(field: &str) -> Option<String> {
    let inner = field.strip_prefix('(')?.strip_suffix(')')?.trim();
    (inner.split(' ').count() == 2).then(|| inner.to_string())
}

fn is_id(text: &str) -> bool {
    text.starts_with("conf::") && text.len() > "conf::".len() && !text.contains(' ')
}

pub fn parse_deferred(text: &str) -> (Vec<Deferred>, Vec<NotApplicable>, Vec<String>) {
    let (mut deferred, mut cases, mut problems) = (vec![], vec![], vec![]);
    for line in records(text) {
        let f = fields(line);
        if let Some(id) = f.first().and_then(|s| s.strip_prefix("not-applicable ")) {
            let tail = f.get(3..).map(|t| t.join("  ")).unwrap_or_default();
            let parsed = (f.len() >= 4)
                .then(|| (authority_of(f[2]), tail.strip_prefix("because:")))
                .and_then(|(a, b)| Some((a?, b?.trim().to_string())));
            match parsed {
                Some((authority, because)) if is_id(id) && !f[1].is_empty() && !because.is_empty() => {
                    cases.push(NotApplicable {
                        id: id.to_string(),
                        case: f[1].to_string(),
                        authority,
                        because,
                        line: line.to_string(),
                    })
                }
                _ => problems.push(format!(
                    "{DEFERRED_PATH}: not a not-applicable record `not-applicable conf::<id>  <case>  (<Contract clause>)  because: <why>`: {line}"
                )),
            }
            continue;
        }
        let tail = f.get(3..).map(|t| t.join("  ")).unwrap_or_default();
        let parsed = (f.len() >= 4 && is_id(f[0]))
            .then(|| (authority_of(f[2]), tail.strip_prefix("until:")))
            .and_then(|(a, u)| Some((a?, u?.trim().to_string())));
        match parsed {
            Some((authority, until)) if !until.is_empty() => deferred.push(Deferred {
                id: f[0].to_string(),
                reason: f[1].to_string(),
                authority,
                until,
                line: line.to_string(),
            }),
            _ => problems.push(format!(
                "{DEFERRED_PATH}: not a deferred record `conf::<id>  <reason>  (<Contract clause>)  until: <end condition>`: {line}"
            )),
        }
    }
    (deferred, cases, problems)
}

pub fn parse_withdrawn(text: &str) -> (Vec<Withdrawn>, Vec<String>) {
    let (mut out, mut problems) = (vec![], vec![]);
    for line in records(text) {
        let f = fields(line);
        let parsed = (f.len() == 3 && is_id(f[0]))
            .then(|| {
                let authority = f[1].strip_prefix("withdrawn by ")?.trim();
                let replacement = f[2].strip_prefix("replaced by ")?.trim();
                (authority.split(' ').count() == 2).then_some((authority, replacement))
            })
            .flatten();
        match parsed {
            Some((authority, replacement)) if replacement == "none" || is_id(replacement) => out.push(Withdrawn {
                id: f[0].to_string(),
                authority: authority.to_string(),
                replacement: (replacement != "none").then(|| replacement.to_string()),
                line: line.to_string(),
            }),
            _ => problems.push(format!(
                "{WITHDRAWN_PATH}: not a withdrawn record `conf::<id>  withdrawn by <Contract clause>  replaced by conf::<id>|none`: {line}"
            )),
        }
    }
    (out, problems)
}

/// Whether the authority is a clause of the current manifest: a ledger row of its contract with its clause, or a clause row of that
/// contract's current files that lists no test id (a clause that only withdraws, such as HP A3-2).
fn authority_exists(
    entries: &[Entry],
    clause_rows: &BTreeSet<(String, String)>,
    authority: &str,
) -> bool {
    let (contract, clause) = authority.split_once(' ').unwrap_or((authority, ""));
    entries
        .iter()
        .any(|e| e.contract.eq_ignore_ascii_case(contract) && e.clause == clause)
        || clause_rows.contains(&(contract.to_ascii_lowercase(), clause.to_string()))
}

/// The ids of the two files that the id-state rule needs.
pub struct Status {
    pub deferred: Vec<Deferred>,
    pub not_applicable: Vec<NotApplicable>,
    pub withdrawn: Vec<Withdrawn>,
}

impl Status {
    pub fn deferred_ids(&self) -> BTreeSet<String> {
        self.deferred.iter().map(|d| d.id.clone()).collect()
    }
    pub fn withdrawn_ids(&self) -> BTreeSet<String> {
        self.withdrawn.iter().map(|w| w.id.clone()).collect()
    }
}

/// Reads and parses the two files (a missing file is empty). Parse problems go to `problems`.
pub fn load(root: &Path, problems: &mut Vec<String>) -> Result<Status> {
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_default();
    let (deferred, not_applicable, mut p1) = parse_deferred(&read(DEFERRED_PATH));
    let (withdrawn, mut p2) = parse_withdrawn(&read(WITHDRAWN_PATH));
    problems.append(&mut p1);
    problems.append(&mut p2);
    Ok(Status {
        deferred,
        not_applicable,
        withdrawn,
    })
}

/// The rules of both files. `have` is the set of ids with a transcript, `pending` the set of pending ids.
///
/// Every line, whether the base has it or not, cites an authority that is a clause of the current manifest, qualified by its contract: so a
/// line that is added or changed since the base cites it too, whether the file of that clause is new or already existed. Edits and removals
/// of existing lines are allowed under those rules, and the reviewer checks their meaning. A base without a file counts as an empty file, with
/// no exemption: every line of the file is checked.
pub fn check(
    entries: &[Entry],
    clause_rows: &BTreeSet<(String, String)>,
    status: &Status,
    have: &BTreeSet<String>,
    pending: &BTreeSet<String>,
    problems: &mut Vec<String>,
) {
    let ledger: BTreeSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    let deferred = status.deferred_ids();
    let withdrawn = status.withdrawn_ids();
    // Each id of a file appears once, and is a ledger id.
    for (path, ids) in [
        (
            DEFERRED_PATH,
            status.deferred.iter().map(|d| &d.id).collect::<Vec<_>>(),
        ),
        (
            WITHDRAWN_PATH,
            status.withdrawn.iter().map(|w| &w.id).collect::<Vec<_>>(),
        ),
    ] {
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id) {
                problems.push(format!("{path}: '{id}' appears twice"));
            }
            if !ledger.contains(id.as_str()) {
                problems.push(format!("{path}: '{id}' is not in the ledger"));
            }
        }
    }
    // Exactly one state: a transcript, pending, deferred or withdrawn.
    for id in &ledger {
        let states: Vec<&str> = [
            (have.contains(*id), "a transcript"),
            (pending.contains(*id), "pending.txt"),
            (deferred.contains(*id), "deferred.txt"),
            (withdrawn.contains(*id), "withdrawn.txt"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        // A transcript with pending.txt keeps its own message in ledger.rs.
        if states.len() > 1 && states != ["a transcript", "pending.txt"] {
            problems.push(format!(
                "{id}: is in more than one state: {}",
                states.join(", ")
            ));
        }
    }
    // Authorities exist, qualified by contract.
    for (path, id, authority) in status
        .deferred
        .iter()
        .map(|d| (DEFERRED_PATH, &d.id, &d.authority))
        .chain(
            status
                .not_applicable
                .iter()
                .map(|c| (DEFERRED_PATH, &c.id, &c.authority)),
        )
        .chain(
            status
                .withdrawn
                .iter()
                .map(|w| (WITHDRAWN_PATH, &w.id, &w.authority)),
        )
    {
        if !authority_exists(entries, clause_rows, authority) {
            problems.push(format!(
                "{path}: '{id}' cites the authority '{authority}', which is no clause of that contract in the current manifest"
            ));
        }
    }
    // A not-applicable case leaves its id active: a transcript or pending, never deferred or withdrawn.
    for c in &status.not_applicable {
        if !ledger.contains(c.id.as_str()) {
            problems.push(format!("{DEFERRED_PATH}: '{}' is not in the ledger", c.id));
        } else if deferred.contains(&c.id) || withdrawn.contains(&c.id) {
            problems.push(format!(
                "{DEFERRED_PATH}: '{}' has a not-applicable case, so it stays active and is not deferred or withdrawn",
                c.id
            ));
        }
    }
    // A replacement is a ledger id other than the withdrawn one.
    for w in &status.withdrawn {
        if let Some(r) = &w.replacement {
            if !ledger.contains(r.as_str()) {
                problems.push(format!(
                    "{WITHDRAWN_PATH}: the replacement '{r}' of '{}' is not in the ledger",
                    w.id
                ));
            } else if *r == w.id {
                problems.push(format!(
                    "{WITHDRAWN_PATH}: '{}' cannot replace itself",
                    w.id
                ));
            } else if withdrawn.contains(r) {
                problems.push(format!(
                    "{WITHDRAWN_PATH}: the replacement '{r}' of '{}' is itself withdrawn",
                    w.id
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_split_on_runs_of_two_or_more_spaces_only() {
        assert_eq!(fields("a b  c   d e"), vec!["a b", "c", "d e"]);
        assert_eq!(fields("one"), vec!["one"]);
        assert_eq!(fields("  x  "), vec!["x"]);
    }

    #[test]
    fn an_authority_is_a_contract_and_a_clause_in_parentheses() {
        assert_eq!(authority_of("(Core A6-2)").as_deref(), Some("Core A6-2"));
        assert_eq!(authority_of("(A6-2)"), None);
        assert_eq!(authority_of("Core A6-2"), None);
        assert_eq!(authority_of("(Core A6-2 x)"), None);
    }

    #[test]
    fn a_deferred_record_has_an_id_a_reason_an_authority_and_an_end_condition() {
        let (d, c, p) = parse_deferred(
            "# c\n\nconf::a  r  (Core A6-2)  until: the first release  with two spaces\n\
             not-applicable conf::b  case  (Core A6-2)  because: no subject\n",
        );
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(
            (
                d[0].id.as_str(),
                d[0].reason.as_str(),
                d[0].authority.as_str()
            ),
            ("conf::a", "r", "Core A6-2")
        );
        assert_eq!(d[0].until, "the first release  with two spaces");
        assert_eq!(
            (c[0].id.as_str(), c[0].case.as_str(), c[0].because.as_str()),
            ("conf::b", "case", "no subject")
        );
        let (d, c, p) = parse_deferred("conf::a  r  (Core A6-2)  until:\nbad\nnot-applicable conf::b  case  (Core A6-2)\nconf::c  r  Core A6-2  until: x\n");
        assert!(d.is_empty() && c.is_empty());
        assert_eq!(p.len(), 4, "{p:?}");
    }

    #[test]
    fn a_withdrawn_record_names_its_authority_and_its_replacement_or_none() {
        let (w, p) = parse_withdrawn(
            "conf::a  withdrawn by Core A9-2  replaced by conf::b\nconf::c  withdrawn by HC X-1  replaced by none\n",
        );
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(
            (w[0].authority.as_str(), w[0].replacement.as_deref()),
            ("Core A9-2", Some("conf::b"))
        );
        assert_eq!(w[1].replacement, None);
        let (w, p) = parse_withdrawn(
            "conf::a  withdrawn by Core A9-2  replaced by b\nconf::a  withdrawn by Core  replaced by none\nconf::a  withdrawn by Core A9-2\nA  withdrawn by Core A9-2  replaced by none\n",
        );
        assert!(w.is_empty());
        assert_eq!(p.len(), 4, "{p:?}");
    }

    fn entry(contract: &str, clause: &str, file: &str) -> Entry {
        Entry {
            id: format!("conf::{clause}"),
            contract: contract.into(),
            clause: clause.into(),
            file: file.into(),
            section: String::new(),
            also_in: vec![],
        }
    }

    #[test]
    fn an_authority_needs_a_ledger_row_of_its_own_contract() {
        let rows = [entry("core", "A6-2", "a.md"), entry("hc", "A7-1", "b.md")];
        let none = BTreeSet::new();
        assert!(authority_exists(&rows, &none, "Core A6-2"));
        assert!(authority_exists(&rows, &none, "core A6-2"));
        assert!(!authority_exists(&rows, &none, "HC A6-2"));
        assert!(!authority_exists(&rows, &none, "Core A7-1"));
    }

    #[test]
    fn a_clause_row_without_a_test_id_is_an_authority_of_its_own_contract() {
        let rows = [entry("core", "A6-2", "a.md")];
        let clause_rows = BTreeSet::from([("hp".to_string(), "A3-2".to_string())]);
        assert!(authority_exists(&rows, &clause_rows, "HP A3-2"));
        assert!(!authority_exists(&rows, &clause_rows, "HC A3-2"));
        assert!(!authority_exists(&rows, &clause_rows, "HP A3-3"));
    }
}

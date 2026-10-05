//! The contracts' status files, `conformance/deferred.txt` and `conformance/withdrawn.txt` (plan section 5, from
//! `contracts-v0.1.6`). They are the source of the three states that a ruling gives an id besides "has a transcript" and
//! "is pending": *deferred*, *withdrawn*, and the *not-applicable case* of an active id.
//!
//! Format (xtask README of botster-contracts, "Deferred and withdrawn ids"): fields are separated by two or more spaces, and
//! `#` lines and blank lines are skipped.
//!
//! ```text
//! conf::<id>  <reason>  (<clause>)  until: <end condition>
//! not-applicable conf::<id>  <case>  (<clause>)  because: <why>
//! conf::<id>  withdrawn by <clause>  replaced by conf::<id>|none
//! ```

/// An id that waits for a release condition (Core A6-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deferred {
    pub id: String,
    /// The reason label of the line, for example `worker-protocol-2`.
    pub reason: String,
    /// The clause, for example `Core A6-2`.
    pub authority: String,
    pub until: String,
}

/// A case of an ACTIVE id that has no subject. The id runs; the report lists the case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotApplicable {
    pub id: String,
    pub case: String,
    pub authority: String,
    pub because: String,
}

/// An id that a ruling withdrew.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawn {
    pub id: String,
    pub authority: String,
    /// The replacing id, or `None` for `replaced by none`.
    pub replaced_by: Option<String>,
}

/// The lines of a status file: fields split on two or more spaces, with the line number for the error text.
fn lines(text: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    text.lines().enumerate().filter_map(|(i, line)| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        // Fields are separated by two or more spaces; a longer run leaves empty pieces, which are dropped.
        let fields: Vec<&str> = line
            .split("  ")
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .collect();
        Some((i + 1, fields))
    })
}

fn clause(field: &str) -> Option<&str> {
    field.strip_prefix('(')?.strip_suffix(')')
}

/// The id, the clause and the tail of a deferred or not-applicable line.
fn tail<'a>(
    line: usize,
    fields: &[&'a str],
    keyword: &str,
    at: usize,
) -> Result<(&'a str, &'a str), String> {
    let authority = fields
        .get(at)
        .and_then(|f| clause(f))
        .ok_or_else(|| format!("line {line}: the clause in parentheses is missing"))?;
    let last = fields
        .get(at + 1)
        .and_then(|f| f.strip_prefix(keyword))
        .ok_or_else(|| format!("line {line}: `{keyword}` is missing"))?;
    Ok((authority, last.trim()))
}

/// Parses `deferred.txt`: the deferred ids, and the not-applicable cases.
pub fn parse_deferred(text: &str) -> Result<(Vec<Deferred>, Vec<NotApplicable>), String> {
    let (mut deferred, mut cases) = (Vec::new(), Vec::new());
    for (line, fields) in lines(text) {
        if let Some(id) = fields[0].strip_prefix("not-applicable ") {
            let case = fields
                .get(1)
                .ok_or_else(|| format!("line {line}: the case is missing"))?;
            let (authority, because) = tail(line, &fields, "because:", 2)?;
            cases.push(NotApplicable {
                id: id.trim().to_string(),
                case: (*case).to_string(),
                authority: authority.to_string(),
                because: because.to_string(),
            });
        } else if fields[0].starts_with("conf::") {
            let reason = fields
                .get(1)
                .ok_or_else(|| format!("line {line}: the reason is missing"))?;
            let (authority, until) = tail(line, &fields, "until:", 2)?;
            deferred.push(Deferred {
                id: fields[0].to_string(),
                reason: (*reason).to_string(),
                authority: authority.to_string(),
                until: until.to_string(),
            });
        } else {
            return Err(format!("line {line}: not an id and not `not-applicable`"));
        }
    }
    Ok((deferred, cases))
}

/// Parses `withdrawn.txt`.
pub fn parse_withdrawn(text: &str) -> Result<Vec<Withdrawn>, String> {
    lines(text)
        .map(|(line, fields)| {
            let [id, by, replaced] = fields[..] else {
                return Err(format!("line {line}: expected three fields"));
            };
            let authority = by
                .strip_prefix("withdrawn by ")
                .ok_or_else(|| format!("line {line}: `withdrawn by <clause>` is missing"))?;
            let replacement = replaced
                .strip_prefix("replaced by ")
                .ok_or_else(|| format!("line {line}: `replaced by <id>|none` is missing"))?;
            Ok(Withdrawn {
                id: id.to_string(),
                authority: authority.trim().to_string(),
                replaced_by: (replacement.trim() != "none").then(|| replacement.trim().to_string()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFERRED: &str = "# a comment\n\
        conf::a_one  worker-protocol-2  (Core A6-2)  until: the first release whose T is 2\n\
        \n\
        not-applicable conf::b_two  n-minus-1-case  (Core A6-2)  because: it is in protocol 1\n";
    const WITHDRAWN: &str = "# a comment\n\
        conf::old_one  withdrawn by Core A9-2  replaced by conf::new_one\n\
        conf::old_two  withdrawn by Core A9-2  replaced by none\n";

    #[test]
    fn a_deferred_file_gives_ids_and_cases() {
        let (deferred, cases) = parse_deferred(DEFERRED).unwrap();
        assert_eq!(
            deferred,
            [Deferred {
                id: "conf::a_one".into(),
                reason: "worker-protocol-2".into(),
                authority: "Core A6-2".into(),
                until: "the first release whose T is 2".into(),
            }]
        );
        assert_eq!(
            cases,
            [NotApplicable {
                id: "conf::b_two".into(),
                case: "n-minus-1-case".into(),
                authority: "Core A6-2".into(),
                because: "it is in protocol 1".into(),
            }]
        );
    }

    #[test]
    fn a_withdrawn_file_gives_the_replacement_or_none() {
        let withdrawn = parse_withdrawn(WITHDRAWN).unwrap();
        assert_eq!(withdrawn.len(), 2);
        assert_eq!(withdrawn[0].id, "conf::old_one");
        assert_eq!(withdrawn[0].authority, "Core A9-2");
        assert_eq!(withdrawn[0].replaced_by.as_deref(), Some("conf::new_one"));
        assert_eq!(withdrawn[1].replaced_by, None);
    }

    /// A field that has one space inside is one field; two spaces split.
    #[test]
    fn two_spaces_split_fields_and_one_space_does_not() {
        let fields: Vec<_> = lines("a b  c d   e").next().unwrap().1;
        assert_eq!(fields, ["a b", "c d", "e"]);
    }

    #[test]
    fn a_malformed_line_names_its_number() {
        let error = parse_deferred("# x\nconf::a  why  (Core A6-2)  nope: x\n").unwrap_err();
        assert!(error.starts_with("line 2: `until:` is missing"), "{error}");
        let error = parse_deferred("conf::a  why  until: x\n").unwrap_err();
        assert!(error.contains("clause"), "{error}");
        assert!(parse_deferred("what\n").unwrap_err().starts_with("line 1"));
        assert!(parse_withdrawn("conf::a  by Core A9-2  replaced by none\n")
            .unwrap_err()
            .contains("withdrawn by"));
        assert!(parse_withdrawn("conf::a  withdrawn by X  gone\n")
            .unwrap_err()
            .contains("replaced by"));
        assert!(parse_withdrawn("conf::a  withdrawn by X\n").is_err());
        assert!(parse_deferred("not-applicable conf::a\n").is_err());
    }

    /// The files of the pinned contracts tag parse (`contracts-v0.1.15`: two deferred ids, one case, three withdrawn ids;
    /// the Hub id `wp_3` is withdrawn with no replacement).
    #[test]
    fn the_pinned_files_parse() {
        let (deferred, cases) =
            parse_deferred(include_str!("../../../conformance/contracts-deferred.txt")).unwrap();
        assert_eq!(deferred.len(), 2);
        assert_eq!(cases.len(), 1);
        let withdrawn =
            parse_withdrawn(include_str!("../../../conformance/contracts-withdrawn.txt")).unwrap();
        assert_eq!(withdrawn.len(), 3);
        assert_eq!(
            withdrawn.iter().filter(|w| w.replaced_by.is_none()).count(),
            1
        );
    }
}

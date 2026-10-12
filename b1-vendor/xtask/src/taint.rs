//! `cargo xtask taint`: whole-word match of the names in `docs/taint-denylist.txt` over every tracked file (design section 9).
//!
//! `frozen/` and `docs/` are not scanned: the frozen contracts name some old mechanisms only to forbid them, and `docs/` holds the process prose.
//! In `crate-symbols/`, the key of a row line is not scanned ([`scanned_text`]): `cargo xtask crate-symbols --check` proves that each
//! key is a verbatim code span of a file of `contracts.toml`, that is, frozen text (the steward's decision, state log 2026-10-09).

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use std::path::Path;

const DENYLIST: &str = "docs/taint-denylist.txt";
const SKIPPED_PREFIXES: [&str; 2] = ["frozen/", "docs/"];
const TABLES_PREFIX: &str = "crate-symbols/";

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The tokens of a denylist text: `#` comments and blank lines skipped.
pub fn parse_denylist(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The tokens that occur in `line` as whole words.
pub fn hits_in_line<'a>(line: &str, tokens: &'a [String]) -> Vec<&'a str> {
    let mut found = Vec::new();
    for token in tokens {
        let mut from = 0;
        while let Some(at) = line[from..].find(token.as_str()) {
            let start = from + at;
            let end = start + token.len();
            let before_ok = !line[..start].chars().next_back().is_some_and(is_word);
            let after_ok = !line[end..].chars().next().is_some_and(is_word);
            if before_ok && after_ok {
                found.push(token.as_str());
                break;
            }
            from = start + line[start..].chars().next().map_or(1, char::len_utf8);
        }
    }
    found
}

/// The part of `line` of `file` that the scan reads. For a row line of a crate-symbols table (`"<span>" = <value>` or
/// `<bare key> = <value>`), that is the value only. The key is a code span of a frozen file, which the crate-symbols check proves.
/// Every other line, the comments of a table included, is read in full.
pub fn scanned_text<'a>(file: &str, line: &'a str) -> &'a str {
    if !file.starts_with(TABLES_PREFIX) {
        return line;
    }
    let key_end = if let Some(rest) = line.strip_prefix('"') {
        let mut escaped = false;
        let mut close = None;
        for (i, c) in rest.char_indices() {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => {
                    close = Some(1 + i + 1);
                    break;
                }
                _ => {}
            }
        }
        close
    } else {
        let bare = line
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(line.len());
        (bare > 0).then_some(bare)
    };
    match key_end.and_then(|end| line[end..].strip_prefix(" = ")) {
        Some(value) => value,
        None => line,
    }
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let text =
        std::fs::read_to_string(root.join(DENYLIST)).with_context(|| format!("read {DENYLIST}"))?;
    let tokens = parse_denylist(&text);
    let mut lines = Vec::new();
    let mut scanned = 0;
    for file in tracked_files(root)? {
        if SKIPPED_PREFIXES.iter().any(|p| file.starts_with(p)) {
            continue;
        }
        let Ok(bytes) = std::fs::read(root.join(&file)) else {
            continue;
        };
        scanned += 1;
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            for token in hits_in_line(scanned_text(&file, line), &tokens) {
                lines.push(format!("{file}:{}: banned name '{token}'", index + 1));
            }
        }
    }
    let scan = format!("taint: {scanned} files scanned, {} names", tokens.len());
    if !lines.is_empty() {
        let summary = format!("{} hit(s)", lines.len());
        return Err(crate::Problems::fail_after(
            lines,
            format!("{scan}\n"),
            summary,
        ));
    }
    println!("{scan}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Vec<String> {
        parse_denylist("# comment\n\nold_name\nthing\n")
    }

    #[test]
    fn matches_whole_words_only() {
        let t = tokens();
        assert_eq!(hits_in_line("use old_name here", &t), vec!["old_name"]);
        assert_eq!(hits_in_line("(thing)", &t), vec!["thing"]);
        assert!(hits_in_line("something", &t).is_empty());
        assert!(hits_in_line("my_old_name2", &t).is_empty());
    }

    #[test]
    fn a_later_whole_word_after_a_partial_one_is_found() {
        assert_eq!(hits_in_line("something thing", &tokens()), vec!["thing"]);
    }

    #[test]
    fn a_table_row_key_is_not_scanned_but_its_value_and_the_comments_are() {
        let t = tokens();
        let table = "crate-symbols/x.toml";
        let hits = |line: &str| hits_in_line(scanned_text(table, line), &t);
        // The key is a frozen code span (the crate-symbols check proves it), so a banned word in it passes.
        assert!(hits(r#""old_name::new" = "n/a: a pre-contract name""#).is_empty());
        assert!(hits(r#""a \"q\" old_name" = ["core::A"]"#).is_empty());
        assert!(hits(r#"old_name = "n/a: a pre-contract name""#).is_empty());
        // The same word in a value, an inline table or a comment fails.
        assert_eq!(hits(r#""x" = "n/a: old_name""#), vec!["old_name"]);
        assert_eq!(
            hits(r#""x" = { symbols = [], uncovered = { old_name = "r" } }"#),
            vec!["old_name"]
        );
        assert_eq!(hits("# | LC-1 | `old_name::new` |"), vec!["old_name"]);
        assert_eq!(hits(r#"[rows."old_name"]"#), vec!["old_name"]);
        // A line that is not `key = value` is read in full, and outside the tables a key is scanned.
        assert_eq!(hits(r#""old_name""#), vec!["old_name"]);
        assert_eq!(
            hits_in_line(scanned_text("crates/a.toml", r#""old_name" = 1"#), &t),
            vec!["old_name"]
        );
    }

    #[test]
    fn denylist_skips_comments_and_blank_lines() {
        assert_eq!(tokens(), vec!["old_name", "thing"]);
    }
}

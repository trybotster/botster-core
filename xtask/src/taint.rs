//! `cargo xtask taint`: whole-word match of the old-world names over every tracked file (BUILD.md "Starting fresh" rule 5).
//!
//! The denylist is the one of botster-contracts at the pinned tag, plus `docs/core-taint-additions.txt` (plan section 8,
//! step 3). `docs/` is not scanned: it names the old mechanisms only to forbid them.

use crate::fsutil::{metadata, tracked_files};
use anyhow::{bail, Context, Result};
use std::path::Path;

const ADDITIONS: &str = "docs/core-taint-additions.txt";
const SKIPPED_PREFIXES: [&str; 1] = ["docs/"];

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

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let meta = metadata(root)?;
    let contracts = std::fs::read_to_string(meta.contracts_root.join("docs/taint-denylist.txt"))
        .context("read docs/taint-denylist.txt of the pinned botster-contracts")?;
    let additions = std::fs::read_to_string(root.join(ADDITIONS))
        .with_context(|| format!("read {ADDITIONS}"))?;
    let mut tokens = parse_denylist(&contracts);
    let from_contracts = tokens.len();
    tokens.extend(parse_denylist(&additions));
    let mut hits = 0;
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
            for token in hits_in_line(line, &tokens) {
                eprintln!("{file}:{}: banned name '{token}'", index + 1);
                hits += 1;
            }
        }
    }
    println!(
        "taint: {scanned} files scanned, {from_contracts} names from botster-contracts, {} from Core",
        tokens.len() - from_contracts
    );
    if hits > 0 {
        bail!("{hits} hit(s)");
    }
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
    fn denylist_skips_comments_and_blank_lines() {
        assert_eq!(tokens(), vec!["old_name", "thing"]);
    }

    #[test]
    fn the_core_additions_name_the_old_mechanisms_of_plan_section_8() {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(ADDITIONS),
        )
        .unwrap();
        let tokens = parse_denylist(&text);
        for name in [
            "botster-core-daemon",
            "botster-terminal-protocol",
            "TerminalMetadataProducer",
            "SessionRegistry",
            "SPH1",
        ] {
            assert!(tokens.iter().any(|t| t == name), "{name}");
        }
    }
}

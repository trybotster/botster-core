//! `cargo xtask manifest`: checks the sha256 of every file that `<dir>/MANIFEST.md` lists, where `dir` comes from `contracts.toml`.

use anyhow::{bail, Context, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

const MANIFEST: &str = "MANIFEST.md";

/// The (file, sha256) pairs of the manifest tables.
///
/// A row is a table row with a backticked 64-digit hex hash. The file is the first backticked `.md` name in the cell before the hash cell.
pub fn parse(text: &str) -> Result<Vec<(String, String)>> {
    let hash = Regex::new(r"`([0-9a-f]{64})`").unwrap();
    let name = Regex::new(r"`([^`\s]+\.md)`").unwrap();
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let Some(at) = cells.iter().position(|c| hash.is_match(c)) else {
            continue;
        };
        let sha = hash.captures(cells[at]).expect("matched")[1].to_string();
        let file = at
            .checked_sub(1)
            .and_then(|i| name.captures(cells[i]))
            .map(|c| c[1].to_string())
            .with_context(|| format!("{MANIFEST}:{}: no file name before the hash", index + 1))?;
        out.push((file, sha));
    }
    Ok(out)
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let rel = crate::config::Config::load(root)?.dir;
    let dir = root.join(&rel);
    let text = std::fs::read_to_string(dir.join(MANIFEST))
        .with_context(|| format!("read {rel}/{MANIFEST}"))?;
    let rows = parse(&text)?;
    if rows.is_empty() {
        bail!("{rel}/{MANIFEST} lists no files");
    }
    let mut problems = Vec::new();
    let mut listed = BTreeSet::new();
    for (file, want) in &rows {
        listed.insert(file.clone());
        match std::fs::read(dir.join(file)) {
            Err(_) => problems.push(format!(
                "{file}: listed in the manifest, but missing from {rel}"
            )),
            Ok(bytes) => {
                let got: String = Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                if &got != want {
                    problems.push(format!(
                        "{file}: sha256 {got} differs from the manifest {want}"
                    ));
                }
            }
        }
    }
    println!("manifest: {} files checked", rows.len());
    if problems.is_empty() {
        return Ok(());
    }
    let summary = format!("{} problem(s)", problems.len());
    Err(crate::Problems::fail(problems, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_and_four_column_rows() {
        let h = "a".repeat(64);
        let text = format!(
            "| Contract | File | sha | ACK |\n|---|---|---|---|\n| Core | `core.md` | `{h}` | ok |\n| `audits/x.md` | `{h}` |\nprose `y.md` `{h}`\n"
        );
        let rows = parse(&text).unwrap();
        assert_eq!(
            rows,
            vec![("core.md".into(), h.clone()), ("audits/x.md".into(), h)]
        );
    }

    #[test]
    fn a_row_with_a_hash_and_no_file_is_an_error() {
        let h = "b".repeat(64);
        assert!(parse(&format!("| text | `{h}` |\n")).is_err());
    }
}

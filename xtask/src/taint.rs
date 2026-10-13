//! `cargo xtask taint`: whole-word match of the old-world names over every tracked file (BUILD.md "Starting fresh" rule 5).
//!
//! The denylist is the one of botster-contracts at the pinned tag, plus `docs/core-taint-additions.txt` (plan section 8,
//! step 3). `docs/` is not scanned: it names the old mechanisms only to forbid them.

use crate::fsutil::metadata;
use anyhow::{bail, Context, Result};
use botster_gate_kit::files::{lossy, read_tracked};
use botster_gate_kit::taint::{parse_denylist, scan_files};
use std::path::Path;

const ADDITIONS: &str = "docs/core-taint-additions.txt";

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
    let files = lossy(read_tracked(root, |_| true)?);
    let (scanned, hits) = scan_files(&files, &tokens);
    let (hit_lines, summary, verdict) = report(scanned, from_contracts, tokens.len(), &hits);
    for line in hit_lines {
        eprintln!("{line}");
    }
    println!("{summary}");
    verdict.map_err(anyhow::Error::msg)
}

/// The report of a scan of `scanned` files with `names` names, the first `from_contracts` of them from botster-contracts
/// and the rest from Core: a line for each hit, the summary line, and the verdict, which fails on any hit.
fn report(
    scanned: usize,
    from_contracts: usize,
    names: usize,
    hits: &[(String, usize, String)],
) -> (Vec<String>, String, std::result::Result<(), String>) {
    let lines = hits
        .iter()
        .map(|(file, line, token)| format!("{file}:{line}: banned name '{token}'"))
        .collect();
    let summary = format!(
        "taint: {scanned} files scanned, {from_contracts} names from botster-contracts, {} from Core",
        names - from_contracts
    );
    let verdict = if hits.is_empty() {
        Ok(())
    } else {
        Err(format!("{} hit(s)", hits.len()))
    };
    (lines, summary, verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_hit_is_reported_and_any_hit_fails() {
        assert_eq!(
            report(3, 17, 31, &[]),
            (
                Vec::new(),
                "taint: 3 files scanned, 17 names from botster-contracts, 14 from Core".to_string(),
                Ok(())
            )
        );
        let hits = [
            ("a.rs".to_string(), 4, "old".to_string()),
            ("b.md".to_string(), 1, "older".to_string()),
        ];
        let (lines, _, verdict) = report(3, 17, 31, &hits);
        assert_eq!(
            lines,
            ["a.rs:4: banned name 'old'", "b.md:1: banned name 'older'"]
        );
        assert_eq!(verdict, Err("2 hit(s)".to_string()));
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
        // The names are built from halves: this file is scanned by the taint check too.
        let names = [
            ["botster-core", "-daemon"],
            ["botster-terminal", "-protocol"],
            ["TerminalMetadata", "Producer"],
            ["Session", "Registry"],
            ["SPH", "1"],
        ];
        for name in names.map(|halves| halves.concat()) {
            assert!(tokens.contains(&name), "{name}");
        }
    }
}

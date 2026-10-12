//! `cargo xtask taint`: whole-word match of the old-world names over every tracked file (BUILD.md "Starting fresh" rule 5).
//!
//! The denylist is the one of botster-contracts at the pinned tag, plus `docs/core-taint-additions.txt` (plan section 8,
//! step 3). `docs/` is not scanned: it names the old mechanisms only to forbid them.

use crate::fsutil::{metadata, tracked_files};
use anyhow::{bail, Context, Result};
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
    let mut files = Vec::new();
    for file in tracked_files(root)? {
        if let Ok(bytes) = std::fs::read(root.join(&file)) {
            files.push((file, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
    let (scanned, hits) = scan_files(&files, &tokens);
    for (file, line, token) in &hits {
        eprintln!("{file}:{line}: banned name '{token}'");
    }
    println!(
        "taint: {scanned} files scanned, {from_contracts} names from botster-contracts, {} from Core",
        tokens.len() - from_contracts
    );
    if !hits.is_empty() {
        bail!("{} hit(s)", hits.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

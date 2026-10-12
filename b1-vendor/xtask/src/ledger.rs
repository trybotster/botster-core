//! `cargo xtask ledger [--check [--base <ref>]]`: the list of every test id in the current contract files.

use crate::config::Config;
use crate::extract::{extract, Occurrence};
use crate::fsutil::{files_under, git_show};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const LEDGER_PATH: &str = "conformance/ledger.json";
pub const PENDING_PATH: &str = "conformance/pending.txt";
/// The replacement map is data about the ledger, not a transcript.
pub const REPLACEMENT_MAP_PATH: &str = "conformance/replacement-map.json";
const UI_VECTORS: &str = "crates/botster-ui-contract/vectors";
/// The vector files of the plugin contract: code-only checks such as `conf::ap_b_artifacts_match_code` (HP Appendix B).
const PLUGIN_VECTORS: &str = "crates/botster-plugin-contract/vectors";

/// One ledger row (design section 2).
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: String,
    /// The owning prefix: the first contract in owner order that names the id.
    pub contract: String,
    /// The clause row id, for example `EN-8` or `A2-44`.
    pub clause: String,
    pub file: String,
    /// The heading of the section that holds the id.
    pub section: String,
    /// The other files that name the id.
    pub also_in: Vec<String>,
}

#[derive(Serialize)]
struct Ledger<'a> {
    version: u32,
    ids: &'a [Entry],
}

/// Builds the ledger entries, sorted by id. `Err` lists every id that has no clause.
pub fn build(root: &Path, config: &Config) -> Result<(Vec<Entry>, Vec<Occurrence>)> {
    check_classified(root, config)?;
    let occurrences = extract(root, config)?;
    let mut owners: BTreeMap<&str, &Occurrence> = BTreeMap::new();
    let mut also: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for occ in &occurrences {
        match owners.get(occ.id.as_str()) {
            None => {
                owners.insert(&occ.id, occ);
            }
            Some(owner) if owner.file != occ.file => {
                also.entry(&occ.id).or_default().insert(&occ.file);
            }
            Some(_) => {}
        }
    }
    let mut used = vec![false; config.overrides.len()];
    let mut entries = Vec::new();
    let mut unattributed = Vec::new();
    for (id, owner) in &owners {
        let clause = match &owner.clause {
            Some(c) => Some(c.clone()),
            None => {
                let best = config
                    .overrides
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| id.starts_with(&o.id_prefix))
                    .max_by_key(|(_, o)| o.id_prefix.len());
                best.map(|(i, o)| {
                    used[i] = true;
                    o.clause.clone()
                })
            }
        };
        match clause {
            Some(clause) => entries.push(Entry {
                id: id.to_string(),
                contract: owner.contract.clone(),
                clause,
                file: owner.file.clone(),
                section: owner.section.clone(),
                also_in: also
                    .get(id)
                    .map(|s| s.iter().map(|f| f.to_string()).collect())
                    .unwrap_or_default(),
            }),
            None => unattributed.push(format!(
                "{id} ({}:{}, section '{}')",
                owner.file, owner.line, owner.section
            )),
        }
    }
    let mut problems = Vec::new();
    if !unattributed.is_empty() {
        problems.push(format!(
            "{} id(s) have no clause. Add a [[clause_override]] to contracts.toml:\n  {}",
            unattributed.len(),
            unattributed.join("\n  ")
        ));
    }
    for (o, used) in config.overrides.iter().zip(&used) {
        if !used {
            problems.push(format!(
                "clause_override '{}' matches no unattributed id",
                o.id_prefix
            ));
        }
    }
    if !problems.is_empty() {
        bail!("{}", problems.join("\n"));
    }
    Ok((entries, occurrences))
}

/// Every top-level contract file in the directory is either current or superseded, so a new file cannot be ignored by accident.
fn check_classified(root: &Path, config: &Config) -> Result<()> {
    let dir = root.join(&config.dir);
    let known: BTreeSet<&str> = config
        .contracts
        .iter()
        .flat_map(|c| c.files.iter())
        .chain(config.superseded.files.iter())
        .map(String::as_str)
        .collect();
    let mut problems = Vec::new();
    for name in &known {
        if !dir.join(name).is_file() {
            problems.push(format!(
                "contracts.toml names '{name}', which is not in {}",
                config.dir
            ));
        }
    }
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_file()
            && name.ends_with(".md")
            && name != "MANIFEST.md"
            && !known.contains(name.as_str())
        {
            problems.push(format!(
                "{}/{name} is neither current nor superseded in contracts.toml",
                config.dir
            ));
        }
    }
    if !problems.is_empty() {
        bail!("{}", problems.join("\n"));
    }
    Ok(())
}

/// The text of `conformance/ledger.json`.
pub fn render(entries: &[Entry]) -> Result<String> {
    let mut text = serde_json::to_string_pretty(&Ledger {
        version: 1,
        ids: entries,
    })?;
    text.push('\n');
    Ok(text)
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let config = Config::load(root)?;
    let mut check = false;
    let mut base = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--base" => base = Some(it.next().context("--base needs a git ref")?.clone()),
            other => bail!("unknown argument '{other}'"),
        }
    }
    if base.is_some() && !check {
        bail!("--base only applies to --check");
    }
    let (entries, _) = build(root, &config)?;
    let text = render(&entries)?;
    let path = root.join(LEDGER_PATH);
    if !check {
        std::fs::create_dir_all(path.parent().expect("ledger has a parent"))?;
        std::fs::write(&path, text)?;
        println!("wrote {LEDGER_PATH}: {} ids", entries.len());
        return Ok(());
    }
    let mut problems = Vec::new();
    match std::fs::read_to_string(&path) {
        Ok(on_disk) if on_disk == text => {}
        Ok(_) => problems.push(format!(
            "{LEDGER_PATH} is out of date. Run `cargo xtask ledger`."
        )),
        Err(_) => problems.push(format!(
            "{LEDGER_PATH} is missing. Run `cargo xtask ledger`."
        )),
    }
    check_transcripts(root, &config, &entries, base.as_deref(), &mut problems)?;
    report(problems, &format!("ledger ok: {} ids", entries.len()))
}

fn report(problems: Vec<String>, ok: &str) -> Result<()> {
    if problems.is_empty() {
        println!("{ok}");
        return Ok(());
    }
    let summary = format!("{} problem(s)", problems.len());
    let mut lines: Vec<String> = problems.iter().take(50).cloned().collect();
    if problems.len() > 50 {
        lines.push(format!("... and {} more", problems.len() - 50));
    }
    Err(crate::Problems::fail(lines, summary))
}

/// Reads a `pending.txt` text: one id per line, `#` lines and blank lines skipped.
fn parse_pending(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The transcript files: `conformance/**/*.json` except the ledger, the replacement map, engine vectors and fixtures, plus the UI and plugin vectors.
fn transcript_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for file in files_under(root, &root.join("conformance"))? {
        let skip = file == LEDGER_PATH
            || file == REPLACEMENT_MAP_PATH
            || !file.ends_with(".json")
            || file
                .split('/')
                .any(|part| part == "engine-vectors" || part == "fixtures");
        if !skip {
            files.push(file);
        }
    }
    for dir in [UI_VECTORS, PLUGIN_VECTORS] {
        for file in files_under(root, &root.join(dir))? {
            if file.ends_with(".json") {
                files.push(file);
            }
        }
    }
    Ok(files)
}

fn check_transcripts(
    root: &Path,
    config: &Config,
    entries: &[Entry],
    base: Option<&str>,
    problems: &mut Vec<String>,
) -> Result<()> {
    let ledger: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut have: BTreeSet<String> = BTreeSet::new();
    for file in transcript_files(root)? {
        let text =
            std::fs::read_to_string(root.join(&file)).with_context(|| format!("read {file}"))?;
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                problems.push(format!("{file}: not valid JSON: {e}"));
                continue;
            }
        };
        let Some(id) = value.get("id").and_then(|v| v.as_str()) else {
            problems.push(format!("{file}: no string field 'id'"));
            continue;
        };
        let stem = file
            .rsplit('/')
            .next()
            .unwrap_or(&file)
            .trim_end_matches(".json");
        if stem != id {
            problems.push(format!("{file}: the file name must equal the id '{id}'"));
        }
        match ledger.get(id) {
            None => problems.push(format!("{file}: id '{id}' is not in the ledger")),
            Some(entry) => {
                for (field, want) in [("contract", &entry.contract), ("clause", &entry.clause)] {
                    if let Some(got) = value.get(field).and_then(|v| v.as_str()) {
                        if got != want {
                            problems.push(format!(
                                "{file}: {field} '{got}' differs from the ledger '{want}'"
                            ));
                        }
                    }
                }
            }
        }
        if !have.insert(id.to_string()) {
            problems.push(format!("{file}: a second transcript for id '{id}'"));
        }
    }
    let pending_text = std::fs::read_to_string(root.join(PENDING_PATH)).unwrap_or_default();
    let pending = parse_pending(&pending_text);
    // Deferred and withdrawn ids (conformance/deferred.txt, withdrawn.txt) are the other two states (Core A6-2, A9-2).
    let status = crate::status::load(root, problems)?;
    let (deferred, withdrawn) = (status.deferred_ids(), status.withdrawn_ids());
    for id in ledger.keys() {
        if !have.contains(*id)
            && !pending.contains(*id)
            && !deferred.contains(*id)
            && !withdrawn.contains(*id)
        {
            problems.push(format!(
                "{id}: no transcript, and not in {PENDING_PATH}, {} or {}",
                crate::status::DEFERRED_PATH,
                crate::status::WITHDRAWN_PATH
            ));
        }
    }
    for id in &pending {
        if !ledger.contains_key(id.as_str()) {
            problems.push(format!("{PENDING_PATH}: '{id}' is not in the ledger"));
        } else if have.contains(id) {
            problems.push(format!(
                "{PENDING_PATH}: '{id}' has a transcript. Remove it from the file."
            ));
        }
    }
    let clause_rows = crate::extract::clause_rows(root, config)?;
    crate::status::check(entries, &clause_rows, &status, &have, &pending, problems);
    if let Some(reference) = base {
        // The file may only shrink, except for an id that the ledger itself gained since the
        // base: an accepted amendment adds its new ids as pending. An id the base ledger already
        // had may never come back. A base without the file (e.g. after Phase 0 deletes it) counts
        // as an empty file.
        let old_pending = git_show(root, reference, PENDING_PATH)?
            .map(|text| parse_pending(&text))
            .unwrap_or_default();
        let old_ledger = match git_show(root, reference, LEDGER_PATH)? {
            Some(text) => {
                ledger_ids(&text).with_context(|| format!("{LEDGER_PATH} at {reference}"))?
            }
            None => BTreeSet::new(),
        };
        for id in pending.difference(&old_pending) {
            if old_ledger.contains(id) {
                problems.push(format!(
                    "{PENDING_PATH}: '{id}' is new since {reference}. The file may only shrink."
                ));
            }
        }
    }
    Ok(())
}

/// The ids of a ledger file's text.
fn ledger_ids(text: &str) -> Result<BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let ids = value
        .get("ids")
        .and_then(|v| v.as_array())
        .context("no `ids` array")?;
    Ok(ids
        .iter()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(str::to_string))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_skips_comments_and_blank_lines() {
        let set = parse_pending("# c\n\nconf::a\n  conf::b  \n");
        assert_eq!(
            set.into_iter().collect::<Vec<_>>(),
            vec!["conf::a", "conf::b"]
        );
    }
}

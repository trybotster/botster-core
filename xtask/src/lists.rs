//! `cargo xtask lists`: the checks of `conformance/core-ledger-ids.txt`, `core-pending.txt` and `core-deferred.toml`
//! (plan section 5, rules 1 to 4).
//!
//! - The ledger file is the Core ids of the ledger of the pinned contracts tag.
//! - Every pending id is a ledger id, and no id is both pending and deferred.
//! - Initialization (the base ref has no pending file): every ledger id that has no passing proof is pending or deferred.
//!   P0 has no passing proof, so every ledger id is in one of the two files.
//! - After that the pending file only shrinks. A moved contracts pin may add the ids that the new ledger adds, and the
//!   ids whose transcript changed between the two tags (plan 23u, the pin-move exception). The strict run of the lists
//!   step fails each pending id that passes, so a re-pended id also fails at the new tag.
//! - The deferred file follows the four rules of plan section 5.

use crate::fsutil::{base, git_show};
use anyhow::{bail, Context, Result};
use botster_core_contract::prelude::Feature;
use botster_core_testkit::status;
use botster_worker_core::{WORKER_FEATURES_BY_PROTOCOL, WORKER_PROTOCOL};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const LEDGER_FILE: &str = "conformance/core-ledger-ids.txt";
const PENDING_FILE: &str = "conformance/core-pending.txt";
const DEFERRED_FILE: &str = "conformance/core-deferred.toml";
/// Verbatim copies of the contracts' status files at the pinned tag (`conformance/deferred.txt`, `conformance/withdrawn.txt`).
/// The harness reads the copies; `check` fails when a copy differs from the pinned source.
pub const CONTRACTS_DEFERRED_COPY: &str = "conformance/contracts-deferred.txt";
pub const CONTRACTS_WITHDRAWN_COPY: &str = "conformance/contracts-withdrawn.txt";

/// The deferred set that Core A6-2 enumerates, with the start condition of each id. A later accepted text replaces this data
/// in the commit that moves the contracts pin (plan section 5, rule 3). Source: A6-2 of
/// `frozen/current/core-contract-v1.17-amendment-6-candidate3.md`, manifest `manifest-final13`.
pub const A6_2_DEFERRED: [(&str, &str); 2] = [
    (
        "conf::ad_4_previous_worker_version_adopts",
        "worker_protocol >= 2",
    ),
    (
        "conf::ad_4_missing_worker_capability_is_unsupported",
        "new_worker_feature_over_previous",
    ),
];

/// The text that the manifest of the pinned contracts holds once A6 is accepted into it.
const A6_MANIFEST_MARKER: &str = "core-contract-v1.17-amendment-6";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deferred {
    pub id: String,
    pub authority: String,
    pub start_condition: String,
}

/// The ids of a list file: blank lines and `#` comments skipped. A duplicate is an error.
pub fn parse_ids(text: &str) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if !ids.insert(line.to_string()) {
            return Err(format!("{line} is listed twice"));
        }
    }
    Ok(ids)
}

pub fn parse_deferred(text: &str) -> Result<Vec<Deferred>> {
    let table: toml::Table = text.parse().context("core-deferred.toml is not TOML")?;
    let Some(entries) = table.get("deferred") else {
        return Ok(Vec::new());
    };
    let entries = entries
        .as_array()
        .context("`deferred` is not an array of tables")?;
    entries
        .iter()
        .map(|entry| {
            let field = |name: &str| -> Result<String> {
                Ok(entry
                    .get(name)
                    .and_then(toml::Value::as_str)
                    .with_context(|| format!("a deferred entry has no `{name}`"))?
                    .to_string())
            };
            Ok(Deferred {
                id: field("id")?,
                authority: field("authority")?,
                start_condition: field("start_condition")?,
            })
        })
        .collect()
}

/// The Core ids of a ledger document (`conformance/ledger.json` of botster-contracts).
pub fn core_ids_of_ledger(ledger_json: &str) -> Result<BTreeSet<String>> {
    ids_of_ledger(ledger_json, |row| row["contract"] == "core")
}

/// Every id of a ledger document, of every contract. The contracts' status files are shared by every contract.
pub fn all_ids_of_ledger(ledger_json: &str) -> Result<BTreeSet<String>> {
    ids_of_ledger(ledger_json, |_| true)
}

fn ids_of_ledger(
    ledger_json: &str,
    keep: impl Fn(&serde_json::Value) -> bool,
) -> Result<BTreeSet<String>> {
    let json: serde_json::Value = serde_json::from_str(ledger_json)?;
    Ok(json["ids"]
        .as_array()
        .context("the ledger has no `ids`")?
        .iter()
        .filter(|row| keep(row))
        .filter_map(|row| row["id"].as_str().map(str::to_string))
        .collect())
}

/// What the checks read.
pub struct Input<'a> {
    /// The Core ids of the ledger at the pinned tag.
    pub ledger: &'a BTreeSet<String>,
    /// Every id of the ledger at the pinned tag, of every contract. Every id of the shared status files is one of them.
    pub ledger_all: &'a BTreeSet<String>,
    /// The checked-in `core-ledger-ids.txt`.
    pub ledger_file: &'a BTreeSet<String>,
    pub pending: &'a BTreeSet<String>,
    pub deferred: &'a [Deferred],
    /// Whether the pinned manifest has an entry for A6.
    pub a6_in_manifest: bool,
    /// The worker protocol number `T` and the features of each protocol.
    pub worker_protocol: u8,
    pub features: &'a [(u8, &'a [Feature])],
    /// The ids that the contracts withdrew (`withdrawn.txt`), of every contract. A Core id is reported as withdrawn,
    /// never pending; an id of another contract is that contract's.
    pub withdrawn: &'a BTreeSet<String>,
    /// The whole-id deferrals of the contracts' `deferred.txt`, of every contract. `core-deferred.toml` must list exactly
    /// the Core ones.
    pub contract_deferred: &'a BTreeSet<String>,
    /// The ids of the `not-applicable` lines of `deferred.txt`, of every contract. A Core one is a case of an ACTIVE id.
    pub not_applicable: &'a [String],
    /// The base ref's files. `None`: the base has no pending file (initialization).
    pub base: Option<Base<'a>>,
    /// The contracts tag moved against the base.
    pub tag_moved: bool,
    /// The ids whose Core transcript differs between the base's pinned commit and this one (`changed_transcripts`).
    pub transcript_changed: &'a BTreeSet<String>,
}

pub struct Base<'a> {
    pub pending: &'a BTreeSet<String>,
    pub deferred: &'a BTreeSet<String>,
    /// The ids of the base's `core-ledger-ids.txt`, or none when the base has no such file.
    pub ledger_file: &'a BTreeSet<String>,
}

/// Every problem, one string each. Empty means the three files are valid.
pub fn check(input: &Input<'_>) -> Vec<String> {
    let mut problems = Vec::new();
    let deferred_ids: BTreeSet<String> = input.deferred.iter().map(|d| d.id.clone()).collect();

    if input.ledger_file != input.ledger {
        problems.push(format!(
            "{LEDGER_FILE} is not the Core ledger of the pinned tag ({} ids in the file, {} in the ledger); run `cargo xtask ledger-ids --write`",
            input.ledger_file.len(),
            input.ledger.len()
        ));
    }
    for id in input.pending.difference(input.ledger) {
        problems.push(format!(
            "{PENDING_FILE}: {id} is not a Core id of the ledger"
        ));
    }
    // The status files are shared by every contract. An id that is in no contract's ledger is a mistake (a mistyped id
    // must not pass as another contract's); an id of another contract is that contract's, and Core does not apply it.
    for id in input.withdrawn {
        if !input.ledger_all.contains(id) {
            problems.push(format!(
                "withdrawn.txt: {id} is not an id of the pinned ledger"
            ));
            continue;
        }
        if !input.ledger.contains(id) {
            continue;
        }
        if input.pending.contains(id) {
            problems.push(format!(
                "{PENDING_FILE}: {id} is withdrawn; a withdrawn id is never pending"
            ));
        }
        if deferred_ids.contains(id) {
            problems.push(format!("{id} is both withdrawn and deferred"));
        }
    }
    for id in input.contract_deferred.difference(input.ledger_all) {
        problems.push(format!(
            "deferred.txt: {id} is not an id of the pinned ledger"
        ));
    }
    let core_contract_deferred: BTreeSet<String> = input
        .contract_deferred
        .intersection(input.ledger)
        .cloned()
        .collect();
    // The deferred file is the whole-id Core deferrals of the contracts' `deferred.txt`, no more and no fewer.
    for id in deferred_ids.difference(&core_contract_deferred) {
        problems.push(format!(
            "{DEFERRED_FILE}: {id} is not deferred by the contracts' deferred.txt"
        ));
    }
    for id in core_contract_deferred.difference(&deferred_ids) {
        problems.push(format!(
            "{DEFERRED_FILE}: {id} is deferred by the contracts' deferred.txt and missing here"
        ));
    }
    // A `not-applicable` line names a case of an id that stays active: it is a ledger id, and neither deferred nor withdrawn.
    for id in input.not_applicable {
        if !input.ledger_all.contains(id) {
            problems.push(format!(
                "deferred.txt: the not-applicable id {id} is not an id of the pinned ledger"
            ));
        } else if !input.ledger.contains(id) {
            // A case of another contract's id.
        } else if input.withdrawn.contains(id) || input.contract_deferred.contains(id) {
            problems.push(format!(
                "deferred.txt: the not-applicable id {id} must stay active, and it is withdrawn or deferred"
            ));
        }
    }
    for id in &deferred_ids {
        if !input.ledger.contains(id) {
            problems.push(format!(
                "{DEFERRED_FILE}: {id} is not a Core id of the ledger"
            ));
        }
        if input.pending.contains(id) {
            problems.push(format!("{id} is both pending and deferred"));
        }
    }
    let mut seen = BTreeSet::new();
    for entry in input.deferred {
        if !seen.insert(&entry.id) {
            problems.push(format!("{DEFERRED_FILE}: {} is listed twice", entry.id));
        }
    }

    // Rule 1: only after acceptance.
    if !input.a6_in_manifest && !input.deferred.is_empty() {
        problems.push(format!(
            "{DEFERRED_FILE} must be empty: the pinned manifest has no entry for A6 (the ids stay pending)"
        ));
    }
    for entry in input.deferred {
        // Rule 3: exactly A6-2's set, with its start condition and an authority that names A6-2 and a manifest tag.
        match A6_2_DEFERRED.iter().find(|(id, _)| *id == entry.id) {
            None => problems.push(format!(
                "{DEFERRED_FILE}: {} is not in the deferred set of Core A6-2",
                entry.id
            )),
            Some((_, condition)) if *condition != entry.start_condition => problems.push(format!(
                "{DEFERRED_FILE}: {} has start_condition `{}`, A6-2 says `{condition}`",
                entry.id, entry.start_condition
            )),
            Some(_) => {}
        }
        if !entry.authority.contains("Core A6-2") || !entry.authority.contains("manifest-final") {
            problems.push(format!(
                "{DEFERRED_FILE}: {} needs an authority that names `Core A6-2` and the manifest tag",
                entry.id
            ));
        }
        // Rule 4: the start condition is false.
        if let Some(why) = condition_holds(
            &entry.start_condition,
            input.worker_protocol,
            input.features,
        ) {
            problems.push(format!(
                "{DEFERRED_FILE}: {} is no longer deferred: {why}; it must pass",
                entry.id
            ));
        }
    }

    match &input.base {
        None => {
            // Initialization: nothing has a passing proof yet, so every ledger id is pending or deferred.
            for id in input.ledger {
                if !input.pending.contains(id)
                    && !deferred_ids.contains(id)
                    && !input.withdrawn.contains(id)
                {
                    problems.push(format!(
                        "initialization: {id} has no passing proof and is neither pending, deferred nor withdrawn"
                    ));
                }
            }
        }
        Some(base) => {
            // The file only shrinks. A moved pin may add the ids that the new ledger adds, and the ids whose transcript
            // changed between the two tags (plan 23u; the strict run fails each one that passes).
            let new_in_ledger: BTreeSet<&String> =
                input.ledger.difference(base.ledger_file).collect();
            for id in input.pending.difference(base.pending) {
                let _repended = input.tag_moved && input.transcript_changed.contains(id);
                if !new_in_ledger.contains(id) {
                    problems.push(format!(
                        "{PENDING_FILE}: {id} is new; the file may only shrink (plan 23u: only the commit that moves \
                         the contracts pin may add an id, and only one that is new in the ledger or whose transcript \
                         changed between the two tags)"
                    ));
                }
            }
            // Rule 2: only the commit that moves the pin moves ids to the deferred file.
            if !input.tag_moved {
                for id in deferred_ids.difference(base.deferred) {
                    problems.push(format!(
                        "{DEFERRED_FILE}: {id} is new; only the commit that moves the contracts pin may defer an id"
                    ));
                }
            }
        }
    }
    problems
}

/// `Some(reason)` when the start condition holds now (so the id is no longer deferred), `None` when it is still false.
/// An unknown condition holds, so that it fails the gate.
fn condition_holds(condition: &str, t: u8, features: &[(u8, &[Feature])]) -> Option<String> {
    match condition {
        "worker_protocol >= 2" => (t >= 2).then(|| format!("the worker protocol is {t}")),
        "new_worker_feature_over_previous" => {
            let of = |p: u8| -> BTreeSet<Feature> {
                features
                    .iter()
                    .filter(|(n, _)| *n == p)
                    .flat_map(|(_, f)| f.iter().copied())
                    .collect()
            };
            if t < 2 {
                return None;
            }
            let added: Vec<Feature> = of(t).difference(&of(t - 1)).copied().collect();
            (!added.is_empty())
                .then(|| format!("protocol {t} adds {added:?} over protocol {}", t - 1))
        }
        other => Some(format!("the start condition `{other}` is unknown")),
    }
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let meta = crate::fsutil::metadata(root)?;
    let read = |path: &str| -> Result<String> {
        std::fs::read_to_string(root.join(path)).with_context(|| format!("read {path}"))
    };
    let ledger_json = pinned_ledger_json(&meta.contracts_root)?;
    let ledger = core_ids_of_ledger(&ledger_json)?;
    let ledger_all = all_ids_of_ledger(&ledger_json)?;
    let source = status_sources(&meta.contracts_root)?;
    let (contract_deferred, cases) =
        status::parse_deferred(&source.deferred).map_err(anyhow::Error::msg)?;
    let withdrawn: BTreeSet<String> = status::parse_withdrawn(&source.withdrawn)
        .map_err(anyhow::Error::msg)?
        .into_iter()
        .map(|w| w.id)
        .collect();
    // `deferred.txt` and `withdrawn.txt` are shared by every contract: `check` applies the Core ids.
    let contract_deferred: BTreeSet<String> = contract_deferred.into_iter().map(|d| d.id).collect();
    let not_applicable: Vec<String> = cases.into_iter().map(|c| c.id).collect();
    let mut problems: Vec<String> = copy_problems(&[
        (
            CONTRACTS_DEFERRED_COPY,
            &read(CONTRACTS_DEFERRED_COPY)?,
            &source.deferred,
        ),
        (
            CONTRACTS_WITHDRAWN_COPY,
            &read(CONTRACTS_WITHDRAWN_COPY)?,
            &source.withdrawn,
        ),
    ]);
    let ledger_file = parse_ids(&read(LEDGER_FILE)?).map_err(anyhow::Error::msg)?;
    let pending = parse_ids(&read(PENDING_FILE)?).map_err(anyhow::Error::msg)?;
    let deferred = parse_deferred(&read(DEFERRED_FILE)?)?;
    let manifest = std::fs::read_to_string(meta.contracts_root.join("frozen/current/MANIFEST.md"))
        .unwrap_or_default();

    let base_name = base(root)?;
    let base_pending_text = git_show(root, &base_name, PENDING_FILE)?;
    let base_deferred_text = git_show(root, &base_name, DEFERRED_FILE)?;
    let base_ledger_text = git_show(root, &base_name, LEDGER_FILE)?;
    let base_cargo = git_show(root, &base_name, "Cargo.toml")?;
    let cargo = read("Cargo.toml")?;
    let base_pending = base_pending_text
        .as_deref()
        .map(parse_ids)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let base_deferred: BTreeSet<String> = base_deferred_text
        .as_deref()
        .map(parse_deferred)
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.id)
        .collect();
    let base_ledger = base_ledger_text
        .as_deref()
        .map(parse_ids)
        .transpose()
        .map_err(anyhow::Error::msg)?
        .unwrap_or_default();
    let base = base_pending.as_ref().map(|pending| Base {
        pending,
        deferred: &base_deferred,
        ledger_file: &base_ledger,
    });
    let tag_moved = pin_moved(base_cargo.as_deref(), &cargo);
    let base_lock = git_show(root, &base_name, "Cargo.lock")?;
    let commit = contracts_commit(&read("Cargo.lock")?)
        .context("Cargo.lock has no source commit of botster-core-contract")?;
    let transcript_changed = changed_transcripts(
        &meta.contracts_root,
        base_lock.as_deref().and_then(contracts_commit).as_deref(),
        &commit,
    )?;

    problems.extend(check(&Input {
        ledger: &ledger,
        ledger_all: &ledger_all,
        ledger_file: &ledger_file,
        pending: &pending,
        deferred: &deferred,
        withdrawn: &withdrawn,
        contract_deferred: &contract_deferred,
        not_applicable: &not_applicable,
        a6_in_manifest: manifest.contains(A6_MANIFEST_MARKER),
        worker_protocol: WORKER_PROTOCOL,
        features: WORKER_FEATURES_BY_PROTOCOL,
        base,
        tag_moved,
        transcript_changed: &transcript_changed,
    }));
    report(&problems)?;
    let withdrawn = withdrawn.intersection(&ledger).count();
    println!(
        "lists: ok. ledger {} ids, pending {}, deferred {}, withdrawn {}, to run {}",
        ledger.len(),
        pending.len(),
        deferred.len(),
        withdrawn,
        ledger.len() - pending.len() - deferred.len() - withdrawn
    );
    for case in not_applicable.iter().filter(|id| ledger.contains(*id)) {
        println!("lists: not-applicable case of the active id {case}");
    }
    Ok(())
}

/// Prints the problems and fails when there is one.
fn report(problems: &[String]) -> Result<()> {
    for problem in problems {
        eprintln!("lists: {problem}");
    }
    if problems.is_empty() {
        Ok(())
    } else {
        bail!("{} problem(s) in the conformance lists", problems.len())
    }
}

/// Whether the contracts pin differs from the base's. A base without a `Cargo.toml` has no pin: the first commit that has
/// one moved it.
fn pin_moved(base_cargo: Option<&str>, cargo: &str) -> bool {
    base_cargo.map(contracts_tag) != Some(contracts_tag(cargo))
}

/// The text of `core-ledger-ids.txt` for a set of ids.
fn ledger_text(ledger: &BTreeSet<String>) -> String {
    ledger.iter().map(|id| format!("{id}\n")).collect()
}

/// The text of the contracts' status files at the pinned tag.
pub struct StatusSources {
    pub deferred: String,
    pub withdrawn: String,
}

pub fn status_sources(contracts_root: &Path) -> Result<StatusSources> {
    let read = |name: &str| {
        std::fs::read_to_string(contracts_root.join("conformance").join(name))
            .with_context(|| format!("read the pinned conformance/{name}"))
    };
    Ok(StatusSources {
        deferred: read("deferred.txt")?,
        withdrawn: read("withdrawn.txt")?,
    })
}

/// One problem for each checked-in copy that is not the pinned source: `(path, copy, source)`.
fn copy_problems(copies: &[(&str, &str, &str)]) -> Vec<String> {
    copies
        .iter()
        .filter(|(_, copy, source)| copy != source)
        .map(|(path, ..)| {
            format!("{path} is not the pinned file of botster-contracts; run `cargo xtask ledger-ids --write`")
        })
        .collect()
}

/// The Core ids of the ledger of the pinned contracts checkout.
pub fn ledger_of(contracts_root: &Path) -> Result<BTreeSet<String>> {
    core_ids_of_ledger(&pinned_ledger_json(contracts_root)?)
}

fn pinned_ledger_json(contracts_root: &Path) -> Result<String> {
    std::fs::read_to_string(contracts_root.join("conformance/ledger.json"))
        .context("read the pinned ledger")
}

/// `cargo xtask ledger-ids [--write]`: check or write `conformance/core-ledger-ids.txt` from the pinned ledger.
pub fn ledger_ids_command(root: &Path, args: &[String]) -> Result<()> {
    let write = match args {
        [] => false,
        [flag] if flag == "--write" => true,
        _ => bail!("usage: cargo xtask ledger-ids [--write]"),
    };
    let meta = crate::fsutil::metadata(root)?;
    let ledger = ledger_of(&meta.contracts_root)?;
    let source = status_sources(&meta.contracts_root)?;
    let files = [
        (LEDGER_FILE, ledger_text(&ledger)),
        (CONTRACTS_DEFERRED_COPY, source.deferred),
        (CONTRACTS_WITHDRAWN_COPY, source.withdrawn),
    ];
    if write {
        for (path, text) in &files {
            std::fs::write(root.join(path), text)?;
        }
        println!(
            "ledger-ids: wrote {} ids and the two status files",
            ledger.len()
        );
        return Ok(());
    }
    for (path, text) in &files {
        if std::fs::read_to_string(root.join(path)).ok().as_deref() != Some(text.as_str()) {
            bail!("{path} is not the pinned file; run `cargo xtask ledger-ids --write`");
        }
    }
    println!(
        "ledger-ids: {} ids and the status files match the pin",
        ledger.len()
    );
    Ok(())
}

/// The contracts dependency line of a root `Cargo.toml`: its `tag`, or its `rev`.
pub fn contracts_tag(cargo_toml: &str) -> String {
    let Ok(table) = cargo_toml.parse::<toml::Table>() else {
        return String::new();
    };
    let dep = &table["workspace"]["dependencies"]["botster-core-contract"];
    ["tag", "rev"]
        .iter()
        .find_map(|key| dep.get(key).and_then(toml::Value::as_str))
        .unwrap_or_default()
        .to_string()
}

/// The botster-contracts commit of a `Cargo.lock`: the `#<commit>` of the `botster-core-contract` package's source.
pub fn contracts_commit(cargo_lock: &str) -> Option<String> {
    let table = cargo_lock.parse::<toml::Table>().ok()?;
    table
        .get("package")?
        .as_array()?
        .iter()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("botster-core-contract"))?
        .get("source")?
        .as_str()?
        .rsplit_once('#')
        .map(|(_, commit)| commit.to_string())
}

/// The Core transcript directory of botster-contracts.
const TRANSCRIPT_DIR: &str = "conformance/core";

/// The ids whose Core transcript differs between two botster-contracts commits (plan 23u): `git diff` of the transcript
/// directory in the pinned checkout, which holds both commits. No base commit (initialization) changes nothing.
pub fn changed_transcripts(
    checkout: &Path,
    old: Option<&str>,
    new: &str,
) -> Result<BTreeSet<String>> {
    let Some(old) = old else {
        return Ok(BTreeSet::new());
    };
    // Explicit format flags: the user's git configuration cannot change the parsed names.
    let out = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args([
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
        ])
        .args([
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            old,
            new,
        ])
        .args(["--", TRANSCRIPT_DIR])
        .output()
        .context("run git diff of the contracts transcripts")?;
    anyhow::ensure!(
        out.status.success(),
        "git diff {old} {new} of the pinned botster-contracts checkout {} failed (the checkout must hold the base's \
         pinned commit): {}",
        checkout.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(transcript_ids(&String::from_utf8_lossy(&out.stdout)))
}

/// The ids of the `-z` names of transcript files: `conformance/core/<id>.json`.
fn transcript_ids(names: &str) -> BTreeSet<String> {
    names
        .split('\0')
        .filter_map(|name| {
            name.strip_prefix("conformance/core/")?
                .strip_suffix(".json")
        })
        .filter(|id| !id.contains('/'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn entry(id: &str, condition: &str) -> Deferred {
        Deferred {
            id: id.into(),
            authority: "Core A6-2, manifest-final13".into(),
            start_condition: condition.into(),
        }
    }

    const D1: &str = "conf::ad_4_previous_worker_version_adopts";
    const D2: &str = "conf::ad_4_missing_worker_capability_is_unsupported";
    /// An id of another contract (Hub) in the shared ledger.
    const HUB: &str = "conf::hub_only";
    const NO_FEATURES: &[(u8, &[Feature])] = &[(1, &[Feature::FocusReport])];

    struct World {
        ledger: BTreeSet<String>,
        /// The ledger of every contract: the Core ids and one id of another contract.
        ledger_all: BTreeSet<String>,
        pending: BTreeSet<String>,
        deferred: Vec<Deferred>,
        withdrawn: BTreeSet<String>,
        contract_deferred: BTreeSet<String>,
        not_applicable: Vec<String>,
        /// The ids whose transcript changed between the base's pin and this one.
        transcript_changed: BTreeSet<String>,
    }

    /// Ledger `a b c` plus the two deferred ids; `a b c` pending; both deferred ids deferred.
    fn world() -> World {
        World {
            ledger: set(&["a", "b", "c", D1, D2]),
            ledger_all: set(&["a", "b", "c", D1, D2, HUB]),
            pending: set(&["a", "b", "c"]),
            deferred: vec![
                entry(D1, "worker_protocol >= 2"),
                entry(D2, "new_worker_feature_over_previous"),
            ],
            withdrawn: BTreeSet::new(),
            contract_deferred: set(&[D1, D2]),
            not_applicable: Vec::new(),
            transcript_changed: BTreeSet::new(),
        }
    }

    fn run<'a>(w: &'a World, adjust: impl FnOnce(&mut Input<'a>)) -> Vec<String> {
        let mut input = Input {
            ledger: &w.ledger,
            ledger_all: &w.ledger_all,
            ledger_file: &w.ledger,
            pending: &w.pending,
            deferred: &w.deferred,
            withdrawn: &w.withdrawn,
            contract_deferred: &w.contract_deferred,
            not_applicable: &w.not_applicable,
            a6_in_manifest: true,
            worker_protocol: 1,
            features: NO_FEATURES,
            base: None,
            tag_moved: false,
            transcript_changed: &w.transcript_changed,
        };
        adjust(&mut input);
        check(&input)
    }

    #[test]
    fn the_initial_state_is_valid() {
        assert_eq!(run(&world(), |_| {}), Vec::<String>::new());
    }

    #[test]
    fn the_ledger_file_must_be_the_pinned_ledger() {
        let w = world();
        let short = set(&["a"]);
        let problems = run(&w, |i| i.ledger_file = &short);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("not the Core ledger"));
    }

    #[test]
    fn a_pending_id_outside_the_ledger_is_refused() {
        let mut w = world();
        w.pending.insert("zzz".into());
        let problems = run(&w, |_| {});
        assert!(problems
            .iter()
            .any(|p| p.contains("zzz") && p.contains("not a Core id")));
    }

    #[test]
    fn at_initialization_every_ledger_id_is_pending_or_deferred() {
        let mut w = world();
        w.pending.remove("b");
        let problems = run(&w, |_| {});
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("initialization: b"));
    }

    #[test]
    fn an_id_cannot_be_pending_and_deferred() {
        let mut w = world();
        w.pending.insert(D1.into());
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("both pending and deferred")));
    }

    #[test]
    fn rule_1_the_file_must_be_empty_before_acceptance() {
        let w = world();
        let problems = run(&w, |i| i.a6_in_manifest = false);
        assert!(problems.iter().any(|p| p.contains("must be empty")));
        let mut none = world();
        none.deferred.clear();
        none.contract_deferred.clear();
        none.pending.insert(D1.into());
        none.pending.insert(D2.into());
        assert_eq!(
            run(&none, |i| i.a6_in_manifest = false),
            Vec::<String>::new()
        );
    }

    #[test]
    fn rule_3_an_entry_outside_the_a6_2_set_or_with_another_condition_is_refused() {
        let mut w = world();
        w.deferred.push(entry("a", "worker_protocol >= 2"));
        w.pending.remove("a");
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("not in the deferred set of Core A6-2")));
        let mut other = world();
        other.deferred[0].start_condition = "worker_protocol >= 3".into();
        assert!(run(&other, |_| {}).iter().any(|p| p.contains("A6-2 says")));
    }

    #[test]
    fn rule_3_the_authority_names_a6_2_and_the_manifest_tag() {
        for authority in ["Core A6-2", "manifest-final13", "somebody said so"] {
            let mut w = world();
            w.deferred[0].authority = authority.into();
            assert!(
                run(&w, |_| {})
                    .iter()
                    .any(|p| p.contains("needs an authority")),
                "{authority}"
            );
        }
    }

    #[test]
    fn rule_4_protocol_two_ends_the_first_deferral_only() {
        let w = world();
        let problems = run(&w, |i| i.worker_protocol = 2);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains(D1));
    }

    #[test]
    fn rule_4_a_new_feature_ends_the_second_deferral_and_no_new_feature_keeps_it() {
        let w = world();
        const SAME: &[(u8, &[Feature])] =
            &[(1, &[Feature::FocusReport]), (2, &[Feature::FocusReport])];
        let problems = run(&w, |i| {
            i.worker_protocol = 2;
            i.features = SAME;
        });
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains(D1),
            "a release with no new feature keeps D2 deferred"
        );
        const ADDED: &[(u8, &[Feature])] = &[
            (1, &[Feature::FocusReport]),
            (2, &[Feature::FocusReport, Feature::SnapshotGraphics]),
        ];
        let problems = run(&w, |i| {
            i.worker_protocol = 2;
            i.features = ADDED;
        });
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains(D2)));
    }

    #[test]
    fn an_unknown_start_condition_fails_the_gate() {
        let mut w = world();
        w.deferred[0].start_condition = "someday".into();
        assert!(run(&w, |_| {}).iter().any(|p| p.contains("unknown")));
    }

    fn with_base(
        w: &World,
        base_pending: &BTreeSet<String>,
        base_deferred: &BTreeSet<String>,
        base_ledger: &BTreeSet<String>,
        tag_moved: bool,
    ) -> Vec<String> {
        run(w, |i| {
            i.base = Some(Base {
                pending: base_pending,
                deferred: base_deferred,
                ledger_file: base_ledger,
            });
            i.tag_moved = tag_moved;
        })
    }

    #[test]
    fn after_initialization_the_pending_file_may_shrink_but_not_grow() {
        let w = world();
        let base = set(&["a", "b", "c"]);
        let deferred = set(&[D1, D2]);
        assert!(with_base(&w, &base, &deferred, &w.ledger, false).is_empty());
        let mut shrunk = world();
        shrunk.pending.remove("c");
        assert!(with_base(&shrunk, &base, &deferred, &w.ledger, false).is_empty());
        let smaller_base = set(&["a", "b"]);
        let problems = with_base(&w, &smaller_base, &deferred, &w.ledger, false);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("c is new"));
    }

    #[test]
    fn a_moved_pin_may_add_the_ids_that_the_new_ledger_adds() {
        let w = world();
        let base = set(&["a", "b"]);
        let base_ledger = set(&["a", "b", D1, D2]);
        let deferred = set(&[D1, D2]);
        assert!(with_base(&w, &base, &deferred, &base_ledger, true).is_empty());
        // An id that was already in the old ledger may not come back.
        let old_ledger_with_c = set(&["a", "b", "c", D1, D2]);
        assert!(!with_base(&w, &base, &deferred, &old_ledger_with_c, true).is_empty());
    }

    /// Plan 23u, the pin-move exception: an id of the old ledger may become pending again only in the commit that moves the
    /// pin, and only when its transcript changed between the two tags.
    #[test]
    fn a_moved_pin_may_re_pend_an_id_whose_transcript_changed() {
        let base = set(&["a", "b"]);
        let deferred = set(&[D1, D2]);
        let mut w = world();
        w.transcript_changed = set(&["c"]);
        assert!(with_base(&w, &base, &deferred, &w.ledger, true).is_empty());
        // The same change without a pin move: refused.
        let problems = with_base(&w, &base, &deferred, &w.ledger, false);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("c is new"), "{problems:?}");
        assert!(problems[0].contains("plan 23u"), "{problems:?}");
        // A pin move that did not change the transcript of `c`: refused.
        let mut other = world();
        other.transcript_changed = set(&["a"]);
        let problems = with_base(&other, &base, &deferred, &other.ledger, true);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("c is new"), "{problems:?}");
    }

    #[test]
    fn the_contracts_commit_is_the_source_commit_of_botster_core_contract() {
        let lock = r#"
version = 4

[[package]]
name = "botster-conformance"
version = "0.1.0"
source = "git+https://github.com/trybotster/botster-contracts?tag=contracts-v0.1.25#1111111111111111111111111111111111111111"

[[package]]
name = "botster-core-contract"
version = "0.1.0"
source = "git+https://github.com/trybotster/botster-contracts?tag=contracts-v0.1.25#ff3405992fd3a76f324d3b683b1f1631c034f43c"
"#;
        assert_eq!(
            contracts_commit(lock).as_deref(),
            Some("ff3405992fd3a76f324d3b683b1f1631c034f43c")
        );
        assert_eq!(contracts_commit("version = 4\n"), None);
        assert_eq!(contracts_commit("not toml ["), None);
        let no_commit = "[[package]]\nname = \"botster-core-contract\"\nsource = \"registry+x\"\n";
        assert_eq!(contracts_commit(no_commit), None);
    }

    #[test]
    fn the_transcript_ids_are_the_json_files_directly_in_the_core_directory() {
        let names = "conformance/core/conf::a.json\0conformance/core/sub/conf::x.json\0conformance/core/conf::b.txt\0\
                     conformance/hub/conf::h.json\0conformance/core/conf::c.json\0";
        assert_eq!(transcript_ids(names), set(&["conf::a", "conf::c"]));
        assert!(transcript_ids("").is_empty());
    }

    /// `changed_transcripts` reads `git diff` of the transcript directory between two commits, also under a git
    /// configuration that would change the output (colors, renames, quoted paths, an external diff, no prefix).
    #[test]
    fn the_changed_transcripts_are_the_core_transcripts_that_differ_between_two_commits() {
        use crate::fsutil::{test_git, test_repo};
        let repo = test_repo(&[
            ("conformance/core/conf::a.json", "{\"a\": 1}\n"),
            ("conformance/core/conf::b.json", "{\"b\": 1}\n"),
            ("conformance/core/conf::d.json", "{\"d\": 1}\n"),
            ("conformance/hub/conf::h.json", "{}\n"),
        ]);
        let root = repo.path();
        let git = |args: &[&str]| test_git(root, args);
        git(&["commit", "-q", "-m", "old"]);
        let old = git(&["rev-parse", "HEAD"]).trim().to_string();
        for (key, value) in [
            ("color.ui", "always"),
            ("color.diff", "always"),
            ("diff.renames", "copies"),
            ("core.quotePath", "true"),
            ("diff.noprefix", "true"),
            ("diff.external", "false"),
            ("diff.relative", "true"),
        ] {
            git(&["config", key, value]);
        }
        let write = |path: &str, text: &str| std::fs::write(root.join(path), text).unwrap();
        write("conformance/core/conf::a.json", "{\"a\": 2}\n");
        write("conformance/core/conf::c.json", "{\"c\": 1}\n");
        write("conformance/hub/conf::h.json", "{\"h\": 2}\n");
        git(&[
            "mv",
            "conformance/core/conf::d.json",
            "conformance/core/conf::e.json",
        ]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "new"]);
        let new = git(&["rev-parse", "HEAD"]).trim().to_string();
        assert_eq!(
            changed_transcripts(root, Some(&old), &new).unwrap(),
            set(&["conf::a", "conf::c", "conf::d", "conf::e"])
        );
        assert!(changed_transcripts(root, Some(&new), &new)
            .unwrap()
            .is_empty());
        assert!(changed_transcripts(root, None, &new).unwrap().is_empty());
        let missing =
            changed_transcripts(root, Some("1111111111111111111111111111111111111111"), &new)
                .unwrap_err()
                .to_string();
        assert!(
            missing.contains("must hold the base's pinned commit"),
            "{missing}"
        );
    }

    #[test]
    fn rule_2_only_the_pin_move_may_defer_an_id() {
        let w = world();
        let base = set(&["a", "b", "c", D1, D2]);
        let none = BTreeSet::new();
        let problems = with_base(&w, &base, &none, &w.ledger, false);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems
            .iter()
            .all(|p| p.contains("moves the contracts pin")));
        assert!(with_base(&w, &base, &none, &w.ledger, true).is_empty());
    }

    #[test]
    fn list_files_reject_a_duplicate_and_skip_comments() {
        assert_eq!(parse_ids("# c\n\nx\ny\n").unwrap(), set(&["x", "y"]));
        assert!(parse_ids("x\nx\n").is_err());
    }

    #[test]
    fn the_deferred_file_parses_and_an_empty_file_has_no_entries() {
        let text = "[[deferred]]\nid = \"i\"\nauthority = \"a\"\nstart_condition = \"s\"\n";
        assert_eq!(
            parse_deferred(text).unwrap(),
            [Deferred {
                id: "i".into(),
                authority: "a".into(),
                start_condition: "s".into()
            }]
        );
        assert!(parse_deferred("# nothing\n").unwrap().is_empty());
        assert!(parse_deferred("[[deferred]]\nid = \"i\"\n").is_err());
    }

    #[test]
    fn only_core_ids_are_taken_from_the_ledger() {
        let json = r#"{"version":1,"ids":[{"id":"conf::a","contract":"core"},{"id":"conf::b","contract":"hp"}]}"#;
        assert_eq!(core_ids_of_ledger(json).unwrap(), set(&["conf::a"]));
        assert_eq!(
            all_ids_of_ledger(json).unwrap(),
            set(&["conf::a", "conf::b"])
        );
    }

    #[test]
    fn a_report_of_no_problem_passes_and_any_problem_fails() {
        assert!(report(&[]).is_ok());
        assert!(report(&["x".to_string()]).is_err());
    }

    #[test]
    fn the_pin_moved_unless_base_and_head_name_the_same_tag() {
        let a = "[workspace.dependencies]\nbotster-core-contract = { git = \"u\", tag = \"v1\" }\n";
        let b = "[workspace.dependencies]\nbotster-core-contract = { git = \"u\", tag = \"v2\" }\n";
        assert!(!pin_moved(Some(a), a));
        assert!(pin_moved(Some(a), b));
        assert!(pin_moved(None, a), "a base with no Cargo.toml has no pin");
    }

    #[test]
    fn the_ledger_file_is_one_id_per_line_in_order() {
        assert_eq!(ledger_text(&set(&["b", "a"])), "a\nb\n");
        assert_eq!(ledger_text(&set(&[])), "");
    }

    #[test]
    fn the_pinned_ledger_is_read_from_a_contracts_checkout() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        std::fs::create_dir_all(root.path().join("conformance")).unwrap();
        std::fs::write(
            root.path().join("conformance/ledger.json"),
            r#"{"ids":[{"id":"conf::a","contract":"core"},{"id":"conf::b","contract":"hc"}]}"#,
        )
        .unwrap();
        assert_eq!(ledger_of(root.path()).unwrap(), set(&["conf::a"]));
        assert!(ledger_of(&root.path().join("missing")).is_err());
    }

    #[test]
    fn the_contracts_pin_is_its_tag_or_rev() {
        let tag =
            "[workspace.dependencies]\nbotster-core-contract = { git = \"u\", tag = \"v1\" }\n";
        let rev =
            "[workspace.dependencies]\nbotster-core-contract = { git = \"u\", rev = \"abc\" }\n";
        assert_eq!(contracts_tag(tag), "v1");
        assert_eq!(contracts_tag(rev), "abc");
    }

    #[test]
    fn a_withdrawn_id_is_in_the_ledger_and_never_pending_or_deferred() {
        let mut w = world();
        w.ledger.insert("w".into());
        w.ledger_all.insert("w".into());
        w.withdrawn.insert("w".into());
        assert!(
            run(&w, |_| {}).is_empty(),
            "withdrawn needs no proof, even at initialization"
        );
        w.pending.insert("w".into());
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("w is withdrawn") && p.contains("never pending")));
        w.pending.remove("w");
        w.deferred.push(entry("w", "worker_protocol >= 2"));
        w.contract_deferred.insert("w".into());
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("w is both withdrawn and deferred")));
        let mut outside = world();
        outside.withdrawn.insert("zz".into());
        assert!(run(&outside, |_| {})
            .iter()
            .any(|p| p.contains("withdrawn.txt: zz is not an id of the pinned ledger")));
    }

    #[test]
    fn a_withdrawn_or_deferred_id_of_another_contract_is_accepted_and_not_applied() {
        // A Hub id in the shared files: no problem, and Core neither reports it withdrawn nor expects it deferred.
        let mut w = world();
        w.withdrawn.insert(HUB.into());
        w.contract_deferred.insert(HUB.into());
        w.not_applicable = vec![HUB.into()];
        assert_eq!(run(&w, |_| {}), Vec::<String>::new());
        // Core still applies its own withdrawn id next to it.
        w.withdrawn.insert("a".into());
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("a is withdrawn") && p.contains("never pending")));
    }

    #[test]
    fn a_deferred_id_outside_the_ledger_of_every_contract_is_refused() {
        let mut w = world();
        w.contract_deferred.insert("zz".into());
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("deferred.txt: zz is not an id of the pinned ledger")));
    }

    #[test]
    fn the_deferred_file_is_exactly_the_contracts_whole_id_deferrals() {
        let mut extra = world();
        extra.contract_deferred.remove(D2);
        let problems = run(&extra, |_| {});
        assert!(
            problems
                .iter()
                .any(|p| p.contains(D2) && p.contains("not deferred by the contracts")),
            "{problems:?}"
        );
        let mut missing = world();
        missing.deferred.pop();
        missing.pending.insert(D2.into());
        let problems = run(&missing, |_| {});
        assert!(
            problems
                .iter()
                .any(|p| p.contains(D2) && p.contains("missing here")),
            "{problems:?}"
        );
    }

    /// A `not-applicable` line is a case of an active id: the id is a ledger id, and it is not withdrawn or deferred.
    #[test]
    fn a_not_applicable_id_stays_active() {
        let mut w = world();
        w.not_applicable = vec!["a".into()];
        assert!(run(&w, |_| {}).is_empty());
        w.not_applicable = vec!["nope".into()];
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("nope") && p.contains("not an id of the pinned ledger")));
        w.not_applicable = vec![D1.into()];
        assert!(run(&w, |_| {})
            .iter()
            .any(|p| p.contains("must stay active")));
        let mut withdrawn = world();
        withdrawn.withdrawn.insert("a".into());
        withdrawn.pending.remove("a");
        withdrawn.not_applicable = vec!["a".into()];
        assert!(run(&withdrawn, |_| {})
            .iter()
            .any(|p| p.contains("must stay active")));
    }

    #[test]
    fn a_copy_that_is_not_the_pinned_file_is_a_problem() {
        let ok = copy_problems(&[("x", "same", "same"), ("y", "", "")]);
        assert!(ok.is_empty());
        let bad = copy_problems(&[("x", "same", "same"), ("y", "old", "new")]);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("y is not the pinned file"));
    }
}

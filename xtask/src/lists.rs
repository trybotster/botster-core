//! `cargo xtask lists`: the checks of `conformance/core-ledger-ids.txt`, `core-pending.txt` and `core-deferred.toml`
//! (plan section 5, rules 1 to 4).
//!
//! - The ledger file is the Core ids of the ledger of the pinned contracts tag.
//! - Every pending id is a ledger id, and no id is both pending and deferred.
//! - Initialization (the base ref has no pending file): every ledger id that has no passing proof is pending or deferred.
//!   P0 has no passing proof, so every ledger id is in one of the two files.
//! - After that the pending file only shrinks. A moved contracts pin may add the ids that the new ledger adds.
//! - The deferred file follows the four rules of plan section 5.

use crate::fsutil::{base_ref, git_show, resolves};
use anyhow::{bail, Context, Result};
use botster_core_contract::prelude::Feature;
use botster_worker_core::{WORKER_FEATURES_BY_PROTOCOL, WORKER_PROTOCOL};
use std::collections::BTreeSet;
use std::path::Path;

const LEDGER_FILE: &str = "conformance/core-ledger-ids.txt";
const PENDING_FILE: &str = "conformance/core-pending.txt";
const DEFERRED_FILE: &str = "conformance/core-deferred.toml";

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
    let json: serde_json::Value = serde_json::from_str(ledger_json)?;
    Ok(json["ids"]
        .as_array()
        .context("the ledger has no `ids`")?
        .iter()
        .filter(|row| row["contract"] == "core")
        .filter_map(|row| row["id"].as_str().map(str::to_string))
        .collect())
}

/// What the checks read.
pub struct Input<'a> {
    /// The Core ids of the ledger at the pinned tag.
    pub ledger: &'a BTreeSet<String>,
    /// The checked-in `core-ledger-ids.txt`.
    pub ledger_file: &'a BTreeSet<String>,
    pub pending: &'a BTreeSet<String>,
    pub deferred: &'a [Deferred],
    /// Whether the pinned manifest has an entry for A6.
    pub a6_in_manifest: bool,
    /// The worker protocol number `T` and the features of each protocol.
    pub worker_protocol: u8,
    pub features: &'a [(u8, &'a [Feature])],
    /// The base ref's files. `None`: the base has no pending file (initialization).
    pub base: Option<Base<'a>>,
    /// The contracts tag moved against the base.
    pub tag_moved: bool,
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
                if !input.pending.contains(id) && !deferred_ids.contains(id) {
                    problems.push(format!(
                        "initialization: {id} has no passing proof and is neither pending nor deferred"
                    ));
                }
            }
        }
        Some(base) => {
            // The file only shrinks. A moved pin may add the ids that the new ledger adds.
            let new_in_ledger: BTreeSet<&String> =
                input.ledger.difference(base.ledger_file).collect();
            for id in input.pending.difference(base.pending) {
                if !new_in_ledger.contains(id) {
                    problems.push(format!(
                        "{PENDING_FILE}: {id} is new; the file may only shrink"
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
    let ledger = ledger_of(&meta.contracts_root)?;
    let ledger_file = parse_ids(&read(LEDGER_FILE)?).map_err(anyhow::Error::msg)?;
    let pending = parse_ids(&read(PENDING_FILE)?).map_err(anyhow::Error::msg)?;
    let deferred = parse_deferred(&read(DEFERRED_FILE)?)?;
    let manifest = std::fs::read_to_string(meta.contracts_root.join("frozen/current/MANIFEST.md"))
        .unwrap_or_default();

    let base_name = base_ref();
    if !resolves(root, &base_name) {
        bail!("{base_name} does not resolve; the pending list has no base (fetch it, or set BOTSTER_CI_BASE_REF)");
    }
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
    // A base without Cargo.toml has no pin: the first commit that has one moved it.
    let tag_moved = base_cargo.as_deref().map(contracts_tag) != Some(contracts_tag(&cargo));

    let problems = check(&Input {
        ledger: &ledger,
        ledger_file: &ledger_file,
        pending: &pending,
        deferred: &deferred,
        a6_in_manifest: manifest.contains(A6_MANIFEST_MARKER),
        worker_protocol: WORKER_PROTOCOL,
        features: WORKER_FEATURES_BY_PROTOCOL,
        base,
        tag_moved,
    });
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("lists: {problem}");
        }
        bail!("{} problem(s) in the conformance lists", problems.len());
    }
    println!(
        "lists: ok. ledger {} ids, pending {}, deferred {}, to run {}",
        ledger.len(),
        pending.len(),
        deferred.len(),
        ledger.len() - pending.len() - deferred.len()
    );
    Ok(())
}

/// The Core ids of the ledger of the pinned contracts checkout.
pub fn ledger_of(contracts_root: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(contracts_root.join("conformance/ledger.json"))
        .context("read the pinned ledger")?;
    core_ids_of_ledger(&text)
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
    let text: String = ledger.iter().map(|id| format!("{id}\n")).collect();
    let path = root.join(LEDGER_FILE);
    if write {
        std::fs::write(&path, &text)?;
        println!("ledger-ids: wrote {} ids", ledger.len());
    } else if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        bail!("{LEDGER_FILE} is not the pinned ledger; run `cargo xtask ledger-ids --write`");
    } else {
        println!("ledger-ids: {} ids match the pinned ledger", ledger.len());
    }
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
    const NO_FEATURES: &[(u8, &[Feature])] = &[(1, &[Feature::FocusReport])];

    struct World {
        ledger: BTreeSet<String>,
        pending: BTreeSet<String>,
        deferred: Vec<Deferred>,
    }

    /// Ledger `a b c` plus the two deferred ids; `a b c` pending; both deferred ids deferred.
    fn world() -> World {
        World {
            ledger: set(&["a", "b", "c", D1, D2]),
            pending: set(&["a", "b", "c"]),
            deferred: vec![
                entry(D1, "worker_protocol >= 2"),
                entry(D2, "new_worker_feature_over_previous"),
            ],
        }
    }

    fn run<'a>(w: &'a World, adjust: impl FnOnce(&mut Input<'a>)) -> Vec<String> {
        let mut input = Input {
            ledger: &w.ledger,
            ledger_file: &w.ledger,
            pending: &w.pending,
            deferred: &w.deferred,
            a6_in_manifest: true,
            worker_protocol: 1,
            features: NO_FEATURES,
            base: None,
            tag_moved: false,
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
}

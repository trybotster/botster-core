//! `cargo xtask replacement-map [--check]`: the map from every conformance id, R-1.4 fault and Core error code to the proof that replaces its frozen fake.
//!
//! Input: `conformance/replacement-map.json`. Output: `docs/fake-replacement-map.md`, generated from the JSON and the transcripts.
//! Both forms fail when the JSON is incomplete or invalid. `--check` also fails when the markdown is stale.
//! Prior art: `~/botster-sessions/shared/fake-mode-steward-assessment.md`, BUILD.md "Testing architecture", Core Amendment 5 candidate 4 (A5-1 to A5-4), HC Amendment 5 candidate 2.

use crate::fsutil::files_under;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAP_PATH: &str = "conformance/replacement-map.json";
const DOC_PATH: &str = "docs/fake-replacement-map.md";
const LEDGER_PATH: &str = "conformance/ledger.json";
const CORE_ERROR_SCHEMA: &str = "schemas/core/core-error.schema.json";

/// The closed set of proof kinds. `+edge` takes an edge name; `slow:` and `deferred:` take a reason.
const KINDS: [(&str, &str); 15] = [
    ("core-testkit", "real Core in-process (botster-core-testkit: in-process worker, scripted program backend, real libghostty)"),
    ("core-testkit+edge", "as above, with an edge fault: storage, program-backend, process (including the withheld control link, Core A6-1), service-lane (Core A6-1), route-transport, wake or clock"),
    ("core-testkit+perturb", "as above, under seeded variation produced at its source: the testkit scheduler and edges (Core A5-2: OR-1..OR-3, batching, coalescing, partial pumps)"),
    ("hub-inproc", "the real Hub in-process over the in-memory transport, with the testkit under it"),
    ("hub-inproc+edge", "as above, with an edge fault: transport, storage, plugin-loader, clock, network, process or filesystem"),
    ("hub-inproc+perturb", "as above, under seeded variation: the Hub scheduler for producer variations and the permute-only transport decorator for delivery variations (HC A5-4: EN-3, WT-7, TS-3, TR-3, resync placement)"),
    ("player:client", "the client-subject runner plays the Hub from the transcript (client boundary checks; Web through the TS player)"),
    ("player:hp-frame", "FakePlugin or the plugin-side FakeHub as a frame-script player (HP section 19)"),
    ("real:lua-host", "the real Lua host as the subject, with the HP FakeHub frame-script player as its peer"),
    ("real:cli", "the real CLI as the subject, with the transcript player as the Hub"),
    ("realize", "the reference realize (already real code)"),
    ("static", "a check in the contract crates or xtask with no Core, Hub, player or fake: generated-artifact drift, a compile test, or a crate-layout or path check"),
    ("slow:<reason>", "a named real-process test in the slow tier"),
    ("deferred:<reason>", "a proof the contract defers until a named release condition (Core A6-2); the note gives the end condition"),
    ("withdrawn", "an id that a ruling withdrew and replaced (Core A9-2): it has no proof, and its note names the authority and the replacement (conformance/withdrawn.txt)"),
];

const CORE_EDGES: [&str; 7] = [
    "service-lane",
    "storage",
    "program-backend",
    "process",
    "route-transport",
    "wake",
    "clock",
];
const HUB_EDGES: [&str; 7] = [
    "transport",
    "storage",
    "plugin-loader",
    "clock",
    "network",
    "process",
    "filesystem",
];

/// How a Core error code is provoked.
const HOWS: [&str; 6] = [
    "call",
    "edge",
    "limit",
    "scripted-refusal",
    "unit",
    "real-only",
];

/// The R-1.4 faults (steward rulings R-1.4), by the `key` of their entry in `faults`.
const R14_FAULTS: [&str; 9] = [
    "drop_transport_mid_route",
    "plugin_reload_between_describe_and_dispatch",
    "drop_committed_response",
    "concurrent_duplicate_idempotency_keys",
    "resync_with_partial_frame",
    "unknown_verb_or_component",
    "watch_queue_loss",
    "resync_discards_notifications",
    "seeded_interleaving_points",
];

fn is_transcript(path: &str) -> bool {
    path.starts_with("conformance/")
        && path.ends_with(".json")
        && !path.ends_with("/ledger.json")
        && !path.ends_with("/replacement-map.json")
        && !path.contains("/engine-vectors/")
        && !path.contains("/fixtures/")
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// The display form of a proof: `core-testkit+edge:storage`.
fn display(entry: &Value) -> String {
    let proof = str_of(entry, "proof").unwrap_or("?");
    match (proof.ends_with("+edge"), str_of(entry, "edge")) {
        (true, Some(edge)) => format!("{proof}:{edge}"),
        _ => proof.to_string(),
    }
}

/// Checks one proof and its edge. Pushes a problem for each fault found.
fn check_proof(who: &str, entry: &Value, problems: &mut Vec<String>) {
    let Some(proof) = str_of(entry, "proof") else {
        problems.push(format!("{who}: no proof"));
        return;
    };
    let known = KINDS.iter().any(|(k, _)| !k.contains('<') && *k == proof);
    let slow = proof.strip_prefix("slow:").is_some_and(|r| !r.is_empty());
    let deferred = proof
        .strip_prefix("deferred:")
        .is_some_and(|r| !r.is_empty());
    if !known && !slow && !deferred {
        problems.push(format!("{who}: proof `{proof}` is not in the closed set"));
    }
    let edge = str_of(entry, "edge");
    if proof.ends_with("+edge") {
        let allowed: &[&str] = if proof.starts_with("core-testkit") {
            &CORE_EDGES
        } else {
            &HUB_EDGES
        };
        match edge {
            Some(e) if allowed.contains(&e) => {}
            Some(e) => problems.push(format!("{who}: edge `{e}` is not an edge of `{proof}`")),
            None => problems.push(format!("{who}: `{proof}` needs an `edge`")),
        }
    } else if edge.is_some() {
        problems.push(format!("{who}: an `edge` goes only with a `+edge` proof"));
    }
    let needs_note = slow || deferred || proof == "static" || proof == "withdrawn";
    if needs_note && str_of(entry, "note").is_none_or(str::is_empty) {
        problems.push(format!(
            "{who}: a `{proof}` entry needs a note that names its check or its reason"
        ));
    }
}

/// The map agrees with `conformance/deferred.txt` and `conformance/withdrawn.txt`: a deferred id has the proof `deferred:<its reason>`, a withdrawn
/// id has the proof `withdrawn`, and no other id has either. The map keeps an entry for a withdrawn id (it stays a ledger id, because the frozen
/// text still names it, and the map covers every ledger id), so the withdrawal is visible and cannot drift from the file.
pub fn check_status(map: &Value, status: &crate::status::Status) -> Vec<String> {
    let mut problems = Vec::new();
    let reasons: BTreeMap<&str, &str> = status
        .deferred
        .iter()
        .map(|d| (d.id.as_str(), d.reason.as_str()))
        .collect();
    let withdrawn = status.withdrawn_ids();
    let mut seen_deferred = BTreeSet::new();
    let mut seen_withdrawn = BTreeSet::new();
    for entry in map["ids"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let (Some(id), Some(proof)) = (str_of(entry, "id"), str_of(entry, "proof")) else {
            continue;
        };
        match (proof.strip_prefix("deferred:"), reasons.get(id)) {
            (Some(reason), Some(want)) if reason == *want => {
                seen_deferred.insert(id);
            }
            (Some(reason), Some(want)) => problems.push(format!(
                "{id}: the proof `deferred:{reason}` differs from the reason `{want}` in {}",
                crate::status::DEFERRED_PATH
            )),
            (Some(_), None) => problems.push(format!(
                "{id}: a `deferred:` proof for an id that is not in {}",
                crate::status::DEFERRED_PATH
            )),
            (None, Some(_)) => problems.push(format!(
                "{id}: it is in {} and needs a `deferred:<reason>` proof, not `{proof}`",
                crate::status::DEFERRED_PATH
            )),
            (None, None) => {}
        }
        match (proof == "withdrawn", withdrawn.contains(id)) {
            (true, true) => {
                seen_withdrawn.insert(id);
            }
            (true, false) => problems.push(format!(
                "{id}: the proof `withdrawn` for an id that is not in {}",
                crate::status::WITHDRAWN_PATH
            )),
            (false, true) => problems.push(format!(
                "{id}: it is in {} and needs the proof `withdrawn`, not `{proof}`",
                crate::status::WITHDRAWN_PATH
            )),
            (false, false) => {}
        }
    }
    for d in &status.deferred {
        if !seen_deferred.contains(d.id.as_str())
            && !map["ids"]
                .as_array()
                .is_some_and(|a| a.iter().any(|e| str_of(e, "id") == Some(&d.id)))
        {
            problems.push(format!(
                "{}: in {} but not in the map",
                d.id,
                crate::status::DEFERRED_PATH
            ));
        }
    }
    problems
}

/// Every problem in the map. Empty means the map is complete and valid.
pub fn validate(map: &Value, ledger_ids: &[String], error_codes: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in map["ids"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let Some(id) = str_of(entry, "id") else {
            problems.push("an id entry has no `id`".to_string());
            continue;
        };
        *seen.entry(id).or_default() += 1;
        check_proof(id, entry, &mut problems);
    }
    for (id, n) in &seen {
        if *n > 1 {
            problems.push(format!("{id}: appears {n} times"));
        }
    }
    let ledger: BTreeSet<&str> = ledger_ids.iter().map(String::as_str).collect();
    for id in &ledger {
        if !seen.contains_key(id) {
            problems.push(format!("{id}: in the ledger but not in the map"));
        }
    }
    for id in seen.keys() {
        if !ledger.contains(id) {
            problems.push(format!("{id}: in the map but not in the ledger"));
        }
    }
    let faults = map["faults"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    for key in R14_FAULTS {
        match faults.iter().find(|f| str_of(f, "key") == Some(key)) {
            None => problems.push(format!("fault `{key}`: not mapped")),
            Some(f) => {
                let proofs = f["proofs"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                if proofs.is_empty() {
                    problems.push(format!("fault `{key}`: no proof"));
                }
                for p in proofs {
                    check_proof(&format!("fault `{key}`"), p, &mut problems);
                }
            }
        }
    }
    let errors = map["core_errors"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for code in error_codes {
        if !errors.iter().any(|e| str_of(e, "code") == Some(code)) {
            problems.push(format!("core error `{code}`: not mapped"));
        }
    }
    for e in errors {
        let code = str_of(e, "code").unwrap_or("?");
        let who = format!("core error `{code}`");
        check_proof(&who, e, &mut problems);
        match str_of(e, "how") {
            Some(how) if HOWS.contains(&how) => {
                let proof = str_of(e, "proof").unwrap_or("");
                match how {
                    "edge" if !proof.ends_with("+edge") => {
                        problems.push(format!("{who}: how `edge` needs a +edge proof"))
                    }
                    "limit" if str_of(e, "limit").is_none() => {
                        problems.push(format!("{who}: how `limit` needs a `limit`"))
                    }
                    "scripted-refusal" if !proof.ends_with("+perturb") => problems.push(format!(
                        "{who}: how `scripted-refusal` needs a +perturb proof"
                    )),
                    "real-only" if !proof.starts_with("slow:") => {
                        problems.push(format!("{who}: how `real-only` needs a slow proof"))
                    }
                    "unit" if str_of(e, "note").is_none_or(str::is_empty) => {
                        problems.push(format!("{who}: how `unit` needs a reason in `note`"))
                    }
                    _ => {}
                }
            }
            other => problems.push(format!("{who}: how {other:?} is not one of {HOWS:?}")),
        }
    }
    problems
}

/// The code names of the error schema: the `ErrorCode` variants, and `StartFailed:<reason>` for each start-failure reason.
pub fn schema_error_codes(schema: &Value) -> Vec<String> {
    fn names(def: &Value) -> Vec<String> {
        let mut out = Vec::new();
        for variant in def["oneOf"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(list) = variant["enum"].as_array() {
                out.extend(list.iter().filter_map(Value::as_str).map(String::from));
            } else if let Some(props) = variant["properties"].as_object() {
                out.extend(props.keys().cloned());
            }
        }
        out
    }
    let defs = &schema["$defs"];
    let mut codes = names(&defs["ErrorCode"]);
    codes.extend(
        names(&defs["StartFailReason"])
            .into_iter()
            .map(|r| format!("StartFailed:{r}")),
    );
    codes
}

/// The subject of each transcript, by id, and whether the transcript is checked only structurally
/// (`fake: structural`, rulings R-5 and R-7). A structural check is not a fake behavior proof.
fn transcript_subjects(root: &Path) -> Result<BTreeMap<String, (String, bool)>> {
    let mut out = BTreeMap::new();
    for path in files_under(root, &root.join("conformance"))?
        .into_iter()
        .filter(|p| is_transcript(p))
    {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(root.join(&path))?)
            .with_context(|| format!("parse {path}"))?;
        if let (Some(id), Some(subject)) = (str_of(&v, "id"), str_of(&v, "subject")) {
            let structural = str_of(&v, "fake") == Some("structural");
            out.insert(id.to_string(), (subject.to_string(), structural));
        }
    }
    Ok(out)
}

fn short(id: &str) -> &str {
    id.strip_prefix("conf::").unwrap_or(id)
}

/// The base kind of a proof for the totals: every `slow:<reason>` counts under `slow`.
fn base(proof: &str) -> &str {
    if proof.starts_with("slow:") {
        "slow"
    } else if proof.starts_with("deferred:") {
        "deferred"
    } else {
        proof
    }
}

pub fn render(
    map: &Value,
    contracts: &BTreeMap<String, String>,
    subjects: &BTreeMap<String, (String, bool)>,
) -> String {
    let ids = map["ids"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut o = String::new();
    o.push_str("# Fake replacement map\n\n");
    o.push_str("<!-- generated by `cargo xtask replacement-map` from conformance/replacement-map.json; do not edit -->\n\n");
    o.push_str("This file maps every conformance id, every R-1.4 fault and every Core error code to the proof that replaces its frozen fake. BUILD.md \"Testing architecture\" says that nothing fake is deleted before this map is reviewed and the replacement is green.\n\n");
    o.push_str("Prior art: the steward's assessment (`~/botster-sessions/shared/fake-mode-steward-assessment.md`), BUILD.md \"Testing architecture\", and Core Amendment 5 candidate 4 (A5-1 to A5-4) and Hub→client Amendment 5 candidate 2. Both amendments are accepted (manifest final12). This map follows them. The service-lane edge, the retained `Cancelled` code and the `Internal` proof are open steward items.\n\n");
    o.push_str("Each id was classified by reading its clause. The `edge` and the `note` of each entry are in `conformance/replacement-map.json`. `cargo xtask replacement-map --check` runs in CI. It fails when an id, a fault or a Core error code is missing, when a proof is outside the closed set, or when this file is stale.\n\n");

    o.push_str("## Proof kinds (the closed set)\n\n| Kind | Meaning |\n|---|---|\n");
    for (k, m) in KINDS.iter() {
        o.push_str(&format!("| `{k}` | {m} |\n"));
    }
    o.push_str("\nEdges: Core edges are `storage`, `program-backend` (the scripted program), `process` (spawning or ending a worker, guardian or service, A5-1; and, from Core A6-1, keeping a worker alive while it withholds its control link until after the deadline, which gives `WorkerUnreachable`), `service-lane` (Core A6-1: a scripted service over in-memory lanes, with the real preamble and epoch checks), `route-transport`, `wake` (a spurious wake, A5-1) and `clock` (the host passes `now`, TM-1; the testkit adds no clock). Hub edges are `transport`, `storage`, `plugin-loader`, `clock`, `network` (the scripted I/O service, MCP server, DNS and git source), `process` and `filesystem`.\n\n");

    // Totals per proof kind.
    let mut per_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_edge: BTreeMap<String, usize> = BTreeMap::new();
    let mut matrix: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut slow: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
    let mut deferred: Vec<(&str, &str, &str)> = Vec::new();
    let mut withdrawn: Vec<(&str, &str)> = Vec::new();
    for e in ids {
        let id = str_of(e, "id").unwrap_or("?");
        let proof = str_of(e, "proof").unwrap_or("?");
        *per_kind.entry(display(e)).or_default() += 1;
        if let Some(edge) = str_of(e, "edge") {
            *per_edge.entry(edge.to_string()).or_default() += 1;
        }
        let contract = contracts.get(id).map(String::as_str).unwrap_or("?");
        *matrix
            .entry((contract.to_string(), base(proof).to_string()))
            .or_default() += 1;
        if proof == "withdrawn" {
            withdrawn.push((id, str_of(e, "note").unwrap_or("")));
        }
        if proof.starts_with("deferred:") {
            deferred.push((id, proof, str_of(e, "note").unwrap_or("")));
        }
        if proof.starts_with("slow:") {
            slow.entry(proof.to_string())
                .or_default()
                .push((id, str_of(e, "note").unwrap_or("")));
        }
    }
    o.push_str(&format!(
        "## Totals\n\n{} ids.\n\n### Per proof kind\n\n| Proof | Ids |\n|---|---|\n",
        ids.len()
    ));
    for (k, n) in &per_kind {
        o.push_str(&format!("| `{k}` | {n} |\n"));
    }
    o.push_str("\n### Per edge\n\n| Edge | Ids |\n|---|---|\n");
    for (k, n) in &per_edge {
        o.push_str(&format!("| `{k}` | {n} |\n"));
    }
    let kinds: BTreeSet<&String> = matrix.keys().map(|(_, k)| k).collect();
    let contract_names: BTreeSet<&String> = matrix.keys().map(|(c, _)| c).collect();
    o.push_str("\n### Per contract\n\n| Contract |");
    for k in &kinds {
        o.push_str(&format!(" `{k}` |"));
    }
    o.push_str(" Total |\n|---|");
    for _ in &kinds {
        o.push_str("---|");
    }
    o.push_str("---|\n");
    for c in &contract_names {
        o.push_str(&format!("| {c} |"));
        let mut total = 0;
        for k in &kinds {
            let n = matrix
                .get(&((*c).clone(), (*k).clone()))
                .copied()
                .unwrap_or(0);
            total += n;
            o.push_str(&format!(
                " {} |",
                if n == 0 { String::new() } else { n.to_string() }
            ));
        }
        o.push_str(&format!(" {total} |\n"));
    }

    // Faults.
    o.push_str("\n## R-1.4 faults\n\nEach fault is injected through an edge, the perturbation layer or the player. The Hub side and the client side are separate proofs.\n\n| Fault | Clauses | Side | Proof | How |\n|---|---|---|---|---|\n");
    for f in map["faults"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let proofs = f["proofs"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        for (n, p) in proofs.iter().enumerate() {
            let clauses = f["clauses"]
                .as_array()
                .map(|c| {
                    c.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            if n == 0 {
                o.push_str(&format!(
                    "| {} | {} | {} | `{}` | {} |\n",
                    str_of(f, "fault").unwrap_or(""),
                    clauses,
                    str_of(p, "role").unwrap_or(""),
                    display(p),
                    str_of(f, "note").unwrap_or("")
                ));
            } else {
                o.push_str(&format!(
                    "| | | {} | `{}` | |\n",
                    str_of(p, "role").unwrap_or(""),
                    display(p)
                ));
            }
        }
    }

    // Error codes.
    o.push_str("\n## Core error codes\n\nCore Amendment 5 candidate 4 (A5-3) says every code is reached at its own timing: a scripted synchronous refusal before admission, an edge for an asynchronous failure, or a `CoreLimits` value; a code that only the OS produces is a named real-process test. `call` is an ordinary call with a bad argument or state: it needs no injection. `unit` is a static check with a reason.\n\n| Code | How | Edge or limit | Proof | Note |\n|---|---|---|---|---|\n");
    for e in map["core_errors"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let via = str_of(e, "edge")
            .or_else(|| str_of(e, "limit"))
            .unwrap_or("");
        o.push_str(&format!(
            "| `{}` | {} | {} | `{}` | {} |\n",
            str_of(e, "code").unwrap_or(""),
            str_of(e, "how").unwrap_or(""),
            via,
            str_of(e, "proof").unwrap_or(""),
            str_of(e, "note").unwrap_or("")
        ));
    }

    // Slow tier.
    o.push_str(&format!("\n## Slow tier\n\n{} ids need a named real-process test. Each test cites its clause. The reason is in the proof kind.\n\n", slow.values().map(Vec::len).sum::<usize>()));
    for (reason, items) in &slow {
        o.push_str(&format!("### `{reason}` ({})\n\n", items.len()));
        for (id, note) in items {
            o.push_str(&format!("- `{}`: {}\n", short(id), note));
        }
        o.push('\n');
    }

    // Deferred.
    o.push_str(&format!("## Deferred\n\n{} ids have no subject yet (Core A6-2). Each note gives the end condition.\n\n", deferred.len()));
    for (id, proof, note) in &deferred {
        o.push_str(&format!("- `{}` (`{proof}`): {note}\n", short(id)));
    }
    o.push_str(&format!("\n## Withdrawn\n\n{} ids are withdrawn and replaced (Core A9-2); they stay ledger ids because the frozen text still names them.\n\n", withdrawn.len()));
    for (id, note) in &withdrawn {
        o.push_str(&format!("- `{}`: {note}\n", short(id)));
    }
    o.push('\n');

    // Frozen fakes.
    let fake_name = |s: &str| match s {
        "core" => Some("FakeCore"),
        "worker" => Some("fake route endpoint"),
        _ => None,
    };
    o.push_str("## Ids whose current proof is a frozen fake\n\nThe frozen fakes are FakeCore and the fake route endpoint. There is no FakeHub: a `hub` transcript runs against the real Hub in Stage 2 (HC A5-1), and in Phase 0 it gets a structural check only. An id has a current fake proof when its transcript has the subject `core` or `worker` and is not marked `fake: structural`. A structural transcript (R-5, R-7) is checked only for its structure; it is not a fake behavior proof, so the table leaves it out and counts it below. An id with no transcript yet has no current proof. The fakes stay unchanged until the replacement is green.\n\n| Id | Fake today | Replacement |\n|---|---|---|\n");
    let mut count = 0;
    let mut structural = 0;
    for e in ids {
        let id = str_of(e, "id").unwrap_or("?");
        if let Some((subject, true)) = subjects.get(id) {
            if fake_name(subject).is_some() {
                structural += 1;
            }
            continue;
        }
        if let Some(fake) = subjects.get(id).and_then(|(s, _)| fake_name(s)) {
            o.push_str(&format!(
                "| `{}` | {} | `{}` |\n",
                short(id),
                fake,
                display(e)
            ));
            count += 1;
        }
    }
    o.push_str(&format!("\n{count} ids. {structural} more ids have a structural transcript on a fake subject (not a fake behavior proof).\n\n"));

    let open_items: Vec<&Value> = map["core_errors"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter(|e| str_of(e, "note").is_some_and(|n| n.starts_with("OPEN STEWARD ITEM:")))
        .collect();
    o.push_str(&format!("\n## Open steward items\n\nThese {} Core error codes have a provisional proof, because the steward has not ruled.\n\n", open_items.len()));
    for e in &open_items {
        o.push_str(&format!(
            "- `{}` (`{}`): {}\n",
            str_of(e, "code").unwrap_or("?"),
            str_of(e, "proof").unwrap_or("?"),
            str_of(e, "note").unwrap_or("")
        ));
    }
    o.push_str("\n## Points for the steward and the reviewer\n\n");
    o.push_str("- **Adoption.** Adoption ids are mapped to a new Core handle over the surviving in-process worker and the same storage edge (LC-12, AD-1). Only an adoption after a real host crash (`slow:real-host-crash`) is slow.\n");
    o.push_str("- **The C ABI** (Core section 13) has no ledger id, so it is not in this map.\n");
    o.push_str("- **Withdrawn.** `conf::a2_26_withdrawn` is a withdrawn clause. It has a proof of `hub-inproc` for completeness and proves no behavior.\n");
    o.push_str("- **Known FakeCore defect (steward ruling R-73, item 4).** FakeCore's session control `pty_input` gives the `op` of each segment as a JSON number (`botster-fake-core/src/control.rs`), where `Op` is a decimal string (Codec A.1). Since R-73 no driver reads `segments`, so the defect changes no result. FakeCore is frozen and keeps it.\n");
    o
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => bail!("usage: cargo xtask replacement-map [--check]"),
    };
    let read = |p: &str| -> Result<Value> {
        serde_json::from_str(
            &std::fs::read_to_string(root.join(p)).with_context(|| format!("read {p}"))?,
        )
        .with_context(|| format!("parse {p}"))
    };
    let map = read(MAP_PATH)?;
    let ledger = read(LEDGER_PATH)?;
    let mut ledger_ids = Vec::new();
    let mut contracts = BTreeMap::new();
    for e in ledger["ids"].as_array().context("ledger has no ids")? {
        let id = str_of(e, "id").context("ledger id")?.to_string();
        contracts.insert(id.clone(), str_of(e, "contract").unwrap_or("?").to_string());
        ledger_ids.push(id);
    }
    let codes = schema_error_codes(&read(CORE_ERROR_SCHEMA)?);
    let mut problems = validate(&map, &ledger_ids, &codes);
    let mut status_problems = Vec::new();
    let status = crate::status::load(root, &mut status_problems)?;
    problems.extend(status_problems);
    problems.extend(check_status(&map, &status));
    if !problems.is_empty() {
        let lines = problems
            .iter()
            .take(50)
            .map(|p| format!("replacement-map: {p}"))
            .collect();
        return Err(crate::Problems::fail(
            lines,
            format!(
                "replacement-map: {} problem(s) in {MAP_PATH}",
                problems.len()
            ),
        ));
    }
    let doc = render(&map, &contracts, &transcript_subjects(root)?);
    let path = root.join(DOC_PATH);
    if check {
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        if have != doc {
            bail!("{DOC_PATH} is out of date. Run `cargo xtask replacement-map`.");
        }
        println!(
            "replacement-map: {} ids, {} faults, {} error codes mapped; {DOC_PATH} up to date",
            ledger_ids.len(),
            R14_FAULTS.len(),
            codes.len()
        );
    } else {
        std::fs::write(&path, doc)?;
        println!("replacement-map: wrote {DOC_PATH}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good() -> (Value, Vec<String>, Vec<String>) {
        let faults: Vec<Value> = R14_FAULTS
            .iter()
            .map(|k| json!({"key": k, "proofs": [{"proof": "hub-inproc"}]}))
            .collect();
        let map = json!({
            "ids": [{"id": "conf::a", "proof": "core-testkit"}, {"id": "conf::b", "proof": "hub-inproc+edge", "edge": "transport"}],
            "faults": faults,
            "core_errors": [{"code": "UnknownSession", "how": "call", "proof": "core-testkit"}],
        });
        (
            map,
            vec!["conf::a".into(), "conf::b".into()],
            vec!["UnknownSession".into()],
        )
    }

    #[test]
    fn a_complete_map_has_no_problem() {
        let (m, l, c) = good();
        assert_eq!(validate(&m, &l, &c), Vec::<String>::new());
    }

    #[test]
    fn a_missing_id_a_duplicate_and_an_extra_id_are_problems() {
        let (mut m, mut l, c) = good();
        l.push("conf::c".into());
        m["ids"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "conf::a", "proof": "core-testkit"}));
        m["ids"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "conf::zz", "proof": "core-testkit"}));
        let p = validate(&m, &l, &c).join("\n");
        assert!(
            p.contains("conf::c: in the ledger but not in the map"),
            "{p}"
        );
        assert!(p.contains("conf::a: appears 2 times"), "{p}");
        assert!(
            p.contains("conf::zz: in the map but not in the ledger"),
            "{p}"
        );
    }

    #[test]
    fn a_proof_outside_the_closed_set_or_without_its_edge_or_note_is_a_problem() {
        let (mut m, l, c) = good();
        let ids = m["ids"].as_array_mut().unwrap();
        ids[0] = json!({"id": "conf::a", "proof": "fake"});
        ids[1] = json!({"id": "conf::b", "proof": "core-testkit+edge"});
        ids.push(json!({"id": "conf::c", "proof": "slow:fsync"}));
        ids.push(json!({"id": "conf::d", "proof": "core-testkit+edge", "edge": "transport"}));
        let mut l = l;
        l.extend(["conf::c".into(), "conf::d".into()]);
        let p = validate(&m, &l, &c).join("\n");
        assert!(p.contains("`fake` is not in the closed set"), "{p}");
        assert!(p.contains("needs an `edge`"), "{p}");
        assert!(p.contains("needs a note"), "{p}");
        assert!(
            p.contains("`transport` is not an edge of `core-testkit+edge`"),
            "{p}"
        );
    }

    #[test]
    fn a_deferred_proof_needs_a_reason_and_a_note() {
        let (mut m, l, c) = good();
        let ids = m["ids"].as_array_mut().unwrap();
        ids[0] = json!({"id": "conf::a", "proof": "deferred:worker-protocol-2"});
        ids[1] = json!({"id": "conf::b", "proof": "deferred:", "note": "x"});
        let p = validate(&m, &l, &c).join("\n");
        assert!(p.contains("needs a note"), "{p}");
        assert!(p.contains("`deferred:` is not in the closed set"), "{p}");
    }

    fn status(deferred: &str, withdrawn: &str) -> crate::status::Status {
        let mut problems = Vec::new();
        let (d, c, p1) = crate::status::parse_deferred(deferred);
        let (w, p2) = crate::status::parse_withdrawn(withdrawn);
        problems.extend(p1);
        problems.extend(p2);
        assert!(problems.is_empty(), "{problems:?}");
        crate::status::Status {
            deferred: d,
            not_applicable: c,
            withdrawn: w,
        }
    }

    #[test]
    fn a_withdrawn_proof_needs_a_note() {
        let (mut m, l, c) = good();
        m["ids"][0] = json!({"id": "conf::a", "proof": "withdrawn"});
        let p = validate(&m, &l, &c).join("\n");
        assert!(p.contains("a `withdrawn` entry needs a note"), "{p}");
        m["ids"][0] = json!({"id": "conf::a", "proof": "withdrawn", "note": "Core A9-2: replaced"});
        assert!(validate(&m, &l, &c).is_empty());
    }

    #[test]
    fn the_map_agrees_with_the_deferred_and_withdrawn_files() {
        let (mut m, _, _) = good();
        let st = status(
            "conf::a  waits  (Core A6-2)  until: later\n",
            "conf::b  withdrawn by Core A9-2  replaced by none\n",
        );
        // The proofs of the two files: `deferred:<its reason>` and `withdrawn`.
        m["ids"][0] = json!({"id": "conf::a", "proof": "deferred:waits", "note": "n"});
        m["ids"][1] = json!({"id": "conf::b", "proof": "withdrawn", "note": "n"});
        assert!(check_status(&m, &st).is_empty());
        // A deferred id with another reason, or with an ordinary proof.
        m["ids"][0] = json!({"id": "conf::a", "proof": "deferred:other", "note": "n"});
        let p = check_status(&m, &st).join("\n");
        assert!(
            p.contains("the proof `deferred:other` differs from the reason `waits`"),
            "{p}"
        );
        m["ids"][0] = json!({"id": "conf::a", "proof": "core-testkit"});
        let p = check_status(&m, &st).join("\n");
        assert!(
            p.contains(
                "conf::a: it is in conformance/deferred.txt and needs a `deferred:<reason>` proof"
            ),
            "{p}"
        );
        // A withdrawn id with an ordinary proof.
        m["ids"][0] = json!({"id": "conf::a", "proof": "deferred:waits", "note": "n"});
        m["ids"][1] = json!({"id": "conf::b", "proof": "core-testkit"});
        let p = check_status(&m, &st).join("\n");
        assert!(
            p.contains(
                "conf::b: it is in conformance/withdrawn.txt and needs the proof `withdrawn`"
            ),
            "{p}"
        );
        // The proofs for ids that are in neither file.
        let empty = status("", "");
        m["ids"][0] = json!({"id": "conf::a", "proof": "deferred:waits", "note": "n"});
        m["ids"][1] = json!({"id": "conf::b", "proof": "withdrawn", "note": "n"});
        let p = check_status(&m, &empty).join("\n");
        assert!(
            p.contains("a `deferred:` proof for an id that is not in conformance/deferred.txt"),
            "{p}"
        );
        assert!(
            p.contains("the proof `withdrawn` for an id that is not in conformance/withdrawn.txt"),
            "{p}"
        );
    }

    #[test]
    fn an_unmapped_fault_or_error_code_is_a_problem() {
        let (mut m, l, mut c) = good();
        m["faults"].as_array_mut().unwrap().pop();
        c.push("RegistryFailed".into());
        let p = validate(&m, &l, &c).join("\n");
        assert!(
            p.contains("fault `seeded_interleaving_points`: not mapped"),
            "{p}"
        );
        assert!(p.contains("core error `RegistryFailed`: not mapped"), "{p}");
    }

    #[test]
    fn an_edge_how_needs_an_edge_proof_and_real_only_needs_a_slow_proof() {
        let (mut m, l, c) = good();
        m["core_errors"] = json!([
            {"code": "UnknownSession", "how": "edge", "proof": "core-testkit"},
            {"code": "X", "how": "real-only", "proof": "core-testkit"},
        ]);
        let p = validate(&m, &l, &c).join("\n");
        assert!(p.contains("how `edge` needs a +edge proof"), "{p}");
        assert!(p.contains("how `real-only` needs a slow proof"), "{p}");
    }

    #[test]
    fn the_schema_lists_codes_and_start_failure_reasons() {
        let schema = json!({"$defs": {
            "ErrorCode": {"oneOf": [{"enum": ["UnknownSession"]}, {"properties": {"StartFailed": {}}}]},
            "StartFailReason": {"oneOf": [{"enum": ["CwdMissing"]}, {"properties": {"ExecFailed": {}}}]},
        }});
        assert_eq!(
            schema_error_codes(&schema),
            [
                "UnknownSession",
                "StartFailed",
                "StartFailed:CwdMissing",
                "StartFailed:ExecFailed"
            ]
        );
    }
}

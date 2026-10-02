//! The Core conformance suite, one trial per Core id of the pinned ledger (plan section 5). `harness = false`.
//!
//! The runner's `conformance_tests!` macro cannot mark an id as not yet expected and cannot see a ledger id without a
//! transcript, so this harness drives the runner's public functions instead:
//!
//! - an id in `conformance/core-pending.txt` is an ignored trial of kind `pending`;
//! - an id in `conformance/core-deferred.toml` is an ignored trial of kind `deferred`;
//! - a ledger id without a transcript that is not deferred is an ignored trial of kind `pending: no transcript`;
//! - every other id runs `run_transcript` over the seed set and passes only on `Outcome::Passed`.
//!
//! A pending or deferred id is never counted as passed. `cargo xtask ci` validates the three files. The seed set and the
//! selection come from the runner's environment variables (`BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE`, `BOTSTER_SEED`).

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection, Transcript};
use botster_core_conformance::{driver_for, CoreHarness, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;
use libtest_mimic::{Arguments, Completion, Failed, Trial};
use std::collections::BTreeSet;

/// The Core ids of the ledger at the pinned contracts tag.
const LEDGER_IDS: &str = include_str!("../conformance/core-ledger-ids.txt");
const PENDING_IDS: &str = include_str!("../conformance/core-pending.txt");
const DEFERRED: &str = include_str!("../conformance/core-deferred.toml");

/// Builds the harness of one seed: `TestkitHarness` for the default tier. P6 adds the `slow` feature with `RealCoreHarness`
/// for the real-process tier (plan section 5). Until P1 provides the engine, `open` reports that no Core exists, so a trial
/// that is not pending fails; the ids stay in `conformance/core-pending.txt`.
fn harness_factory() -> Option<fn(u64) -> Box<dyn CoreHarness>> {
    Some(|seed| Box::new(TestkitHarness::new(seed)))
}

fn id_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The deferred entries: `(id, authority, start_condition)`.
fn deferred_entries(text: &str) -> Vec<(String, String, String)> {
    let table: toml::Table = text.parse().expect("core-deferred.toml is TOML");
    let field = |entry: &toml::Value, name: &str| {
        entry
            .get(name)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    table
        .get("deferred")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            (
                field(entry, "id"),
                field(entry, "authority"),
                field(entry, "start_condition"),
            )
        })
        .collect()
}

/// A trial that never runs, even under `--ignored` or `--include-ignored`: it reports itself ignored. It cannot be counted
/// as passed (plan section 5).
fn never_passes(id: &str, kind: &str, reason: String) -> Trial {
    Trial::ignorable_test(id.to_string(), move || Ok(Completion::ignored_with(reason)))
        .with_kind(kind)
        .with_ignored_flag(true)
}

fn run_id(transcript: &Transcript) -> Result<(), Failed> {
    let Some(make) = harness_factory() else {
        return Err(Failed::from(format!(
            "{}: no harness is available yet; the id belongs in conformance/core-pending.txt",
            transcript.id
        )));
    };
    let seeds = Selection::from_env().seeds(&SeedSet::from_env());
    let outcome = run_transcript(
        transcript,
        &|seed| driver_for(make(seed)),
        &seeds,
        &CoreSchemas,
        &Limits::default(),
    );
    if outcome.is_pass() {
        Ok(())
    } else {
        Err(Failed::from(describe(&transcript.id, &outcome)))
    }
}

fn main() {
    let args = Arguments::from_args();
    let ledger = id_list(LEDGER_IDS);
    let pending = id_list(PENDING_IDS);
    let deferred = deferred_entries(DEFERRED);
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let selection = Selection::from_env();

    let mut trials = Vec::new();
    let (mut pending_count, mut no_transcript_count) = (0usize, 0usize);
    for id in &ledger {
        let transcript = transcripts.iter().find(|t| &t.id == id);
        if let Some((_, authority, start)) = deferred.iter().find(|(d, _, _)| d == id) {
            let reason = format!("deferred by {authority}; starts when {start}");
            trials.push(never_passes(id, "deferred", reason));
        } else if pending.contains(id) || transcript.is_none() {
            let (kind, reason) = match transcript {
                Some(_) => {
                    pending_count += 1;
                    ("pending", "pending: no passing proof yet")
                }
                None => {
                    no_transcript_count += 1;
                    (
                        "pending: no transcript",
                        "pending: the ledger id has no transcript",
                    )
                }
            };
            trials.push(never_passes(id, kind, reason.to_string()));
        } else if let Some(transcript) = transcript {
            if selection.selects(transcript) {
                let transcript = transcript.clone();
                trials.push(Trial::test(id.clone(), move || run_id(&transcript)));
            } else {
                trials.push(never_passes(
                    id,
                    "not selected",
                    "not selected by BOTSTER_ONLY, BOTSTER_CLAUSE or BOTSTER_SEED".to_string(),
                ));
            }
        }
    }

    let report = !args.list && !args.exact;
    let conclusion = libtest_mimic::run(&args, trials);
    if report {
        // A pending, deferred or unselected id is never a pass (plan section 5): the four counts of the report.
        println!(
            "conformance: passed {}, failed {}, pending {} (+ {} with no transcript), deferred {}",
            conclusion.num_passed,
            conclusion.num_failed,
            pending_count,
            no_transcript_count,
            deferred.len()
        );
        for (id, authority, start) in &deferred {
            println!("conformance: deferred {id} ({authority}; starts when {start})");
        }
    }
    conclusion.exit();
}

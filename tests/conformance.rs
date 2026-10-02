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
use libtest_mimic::{Arguments, Failed, Trial};
use std::collections::BTreeSet;

/// The Core ids of the ledger at the pinned contracts tag.
const LEDGER_IDS: &str = include_str!("../conformance/core-ledger-ids.txt");
const PENDING_IDS: &str = include_str!("../conformance/core-pending.txt");
const DEFERRED: &str = include_str!("../conformance/core-deferred.toml");

/// Builds the harness of one seed. P6 provides `TestkitHarness` for the default tier, and adds the `slow` feature with
/// `RealCoreHarness` for the real-process tier (plan section 5). Until then no id has a proof, so every id is pending or deferred and this is never reached.
fn harness_factory() -> Option<fn(u64) -> Box<dyn CoreHarness>> {
    None
}

fn id_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn deferred_ids(text: &str) -> BTreeSet<String> {
    let table: toml::Table = text.parse().expect("core-deferred.toml is TOML");
    table
        .get("deferred")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id")?.as_str().map(str::to_string))
        .collect()
}

fn run_id(transcript: &Transcript) -> Result<(), Failed> {
    let Some(make) = harness_factory() else {
        return Err(Failed::from(format!(
            "{}: no harness is available yet; the id belongs in conformance/core-pending.txt",
            transcript.id
        )));
    };
    let selection = Selection::from_env();
    if !selection.selects(transcript) {
        return Ok(());
    }
    let seeds = selection.seeds(&SeedSet::from_env());
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
    let deferred = deferred_ids(DEFERRED);
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");

    let mut trials = Vec::new();
    let (mut pending_count, mut no_transcript_count) = (0usize, 0usize);
    for id in &ledger {
        let transcript = transcripts.iter().find(|t| &t.id == id);
        if deferred.contains(id) {
            trials.push(
                Trial::test(id.clone(), || Ok(()))
                    .with_kind("deferred")
                    .with_ignored_flag(true),
            );
        } else if pending.contains(id) || transcript.is_none() {
            let kind = match transcript {
                Some(_) => {
                    pending_count += 1;
                    "pending"
                }
                None => {
                    no_transcript_count += 1;
                    "pending: no transcript"
                }
            };
            trials.push(
                Trial::test(id.clone(), || Ok(()))
                    .with_kind(kind)
                    .with_ignored_flag(true),
            );
        } else if let Some(transcript) = transcript {
            let transcript = transcript.clone();
            trials.push(Trial::test(id.clone(), move || run_id(&transcript)));
        }
    }

    let report = !args.list && !args.exact;
    let conclusion = libtest_mimic::run(&args, trials);
    if report {
        // A pending or deferred id is never a pass (plan section 5): the four counts of the report.
        println!(
            "conformance: passed {}, failed {}, pending {} (+ {} with no transcript), deferred {}",
            conclusion.num_passed,
            conclusion.num_failed,
            pending_count,
            no_transcript_count,
            deferred.len()
        );
    }
    conclusion.exit();
}

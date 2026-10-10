//! The Core conformance suite, one trial per Core id of the pinned ledger (plan section 5). `harness = false`.
//!
//! The runner's `conformance_tests!` macro cannot mark an id as not yet expected and cannot see a ledger id without a
//! transcript, so this harness drives the runner's public functions instead:
//!
//! - an id in `conformance/core-pending.txt` is an ignored trial of kind `pending`;
//! - an id in `conformance/core-deferred.toml` is an ignored trial of kind `deferred`;
//! - an id in the contracts' `withdrawn.txt` is an ignored trial of kind `withdrawn`: never pending, never a pass;
//! - a `not-applicable` line of the contracts' `deferred.txt` names a CASE of an active id: the id runs, and the report lists the
//!   case;
//! - a ledger id without a transcript that is not deferred is an ignored trial of kind `pending: no transcript`;
//! - every other id runs `run_transcript` over the seed set and passes only on `Outcome::Passed`.
//!
//! A pending or deferred id is never counted as passed. `cargo xtask ci` validates the three files. The seed set and the
//! selection come from the runner's environment variables (`BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE`, `BOTSTER_SEED`).
//!
//! With `BOTSTER_PENDING_STRICT=1` (set by the `lists` step of `cargo xtask ci`; `botster_core_testkit::pending`), a selected
//! pending id with a transcript runs: it must fail, a held id (`core-held.txt`) must pass, and a real-only id
//! (`core-real-only.txt`) does not run. The expected result reports itself ignored, so a pending id is still never a pass.
//! The nextest tiers do not set the variable: there a pending trial stays ignored and does not run.

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection, Transcript};
use botster_core_conformance::{driver_for, CoreHarness, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::pending::{self, Class};
use botster_core_testkit::status::{parse_deferred as parse_status_deferred, parse_withdrawn};
use botster_core_testkit::TestkitHarness;
use libtest_mimic::{Arguments, Completion, Failed, Trial};
use std::collections::BTreeSet;

/// The Core ids of the ledger at the pinned contracts tag.
const LEDGER_IDS: &str = include_str!("../conformance/core-ledger-ids.txt");
const PENDING_IDS: &str = include_str!("../conformance/core-pending.txt");
const DEFERRED: &str = include_str!("../conformance/core-deferred.toml");
/// The contracts' status files at the pinned tag (`cargo xtask lists` checks that they are the pinned files).
const CONTRACTS_DEFERRED: &str = include_str!("../conformance/contracts-deferred.txt");
const CONTRACTS_WITHDRAWN: &str = include_str!("../conformance/contracts-withdrawn.txt");
/// The exemptions of the strict pending run (`cargo xtask lists` checks both files).
const REAL_ONLY: &str = include_str!("../conformance/core-real-only.txt");
const HELD: &str = include_str!("../conformance/core-held.txt");

// Core TH-1, checked when this suite compiles: the facade's handle is `Send` and not `Sync` (no nightly feature).
static_assertions::assert_impl_all!(botster_core::Core: Send);
static_assertions::assert_not_impl_any!(botster_core::Core: Sync);

/// The answer of `type_check: send_not_sync` (Core TH-1): true, because the two assertions above compile.
const CORE_IS_SEND_NOT_SYNC: bool = true;

/// Builds the harness of one seed: `TestkitHarness` for the default tier. P6 adds the `slow` feature with `RealCoreHarness`
/// for the real-process tier (plan section 5). Until P1 provides the engine, `open` reports that no Core exists, so a trial
/// that is not pending fails; the ids stay in `conformance/core-pending.txt`.
fn harness_factory() -> Option<fn(u64) -> Box<dyn CoreHarness>> {
    Some(|seed| Box::new(TestkitHarness::new(seed).with_core_type(CORE_IS_SEND_NOT_SYNC)))
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

/// Runs one transcript over the seed set: `Ok(())` on a pass, else the description of the outcome.
fn outcome_of(transcript: &Transcript) -> Result<(), String> {
    outcome_at(transcript, |seeds| seeds)
}

/// Runs one transcript over the seeds that `narrow` keeps of the seed set.
fn outcome_at(
    transcript: &Transcript,
    narrow: impl FnOnce(SeedSet) -> SeedSet,
) -> Result<(), String> {
    let Some(make) = harness_factory() else {
        return Err(format!(
            "{}: no harness is available yet; the id belongs in conformance/core-pending.txt",
            transcript.id
        ));
    };
    let seeds = narrow(Selection::from_env().seeds(&SeedSet::from_env()));
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
        Err(describe(&transcript.id, &outcome))
    }
}

fn run_id(transcript: &Transcript) -> Result<(), Failed> {
    outcome_of(transcript).map_err(Failed::from)
}

/// The trial of a pending id in the strict run. A real-only id does not run. Another one runs, and `pending::verdict`
/// decides: a pending id that passes fails its trial, and so does a held id that fails. A `budget` held id runs at the
/// first seed only (`pending::seeds_of`). An expected result reports itself ignored, so that the strict run counts no pass.
fn strict_trial(transcript: &Transcript, class: Class) -> Trial {
    let id = transcript.id.clone();
    if let Class::RealOnly(_) = class {
        let note = pending::verdict(&id, &class, false).unwrap_or_default();
        return never_passes(&id, "pending: real-only", note);
    }
    let transcript = transcript.clone();
    Trial::ignorable_test(id.clone(), move || {
        let passed = outcome_at(&transcript, |set| SeedSet {
            seeds: pending::seeds_of(&class, &set.seeds),
        })
        .is_ok();
        pending::verdict(&id, &class, passed)
            .map(Completion::ignored_with)
            .map_err(Failed::from)
    })
    .with_kind("pending: strict")
}

fn main() {
    let args = Arguments::from_args();
    let ledger = id_list(LEDGER_IDS);
    let pending = id_list(PENDING_IDS);
    let deferred = deferred_entries(DEFERRED);
    let withdrawn = parse_withdrawn(CONTRACTS_WITHDRAWN).expect("withdrawn.txt parses");
    let (_, cases) = parse_status_deferred(CONTRACTS_DEFERRED).expect("deferred.txt parses");
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let selection = Selection::from_env();
    let strict = std::env::var(pending::STRICT_ENV).is_ok_and(|v| v == "1");
    let real_only = pending::parse_real_only(REAL_ONLY).expect("core-real-only.txt parses");
    let held = pending::parse_held(HELD).expect("core-held.txt parses");

    let mut trials = Vec::new();
    let (mut pending_count, mut no_transcript_count, mut withdrawn_count) =
        (0usize, 0usize, 0usize);
    // The strict run's own count: the pending ids that ran, and the real-only ones that it does not run.
    let (mut strict_ran, mut strict_real_only) = (0usize, 0usize);
    for id in &ledger {
        let transcript = transcripts.iter().find(|t| &t.id == id);
        if let Some(w) = withdrawn.iter().find(|w| &w.id == id) {
            let replacement = w.replaced_by.as_deref().unwrap_or("nothing");
            let reason = format!("withdrawn by {}; replaced by {replacement}", w.authority);
            withdrawn_count += 1;
            trials.push(never_passes(id, "withdrawn", reason));
        } else if let Some((_, authority, start)) = deferred.iter().find(|(d, _, _)| d == id) {
            let reason = format!("deferred by {authority}; starts when {start}");
            trials.push(never_passes(id, "deferred", reason));
        } else if pending.contains(id) || transcript.is_none() {
            let (kind, reason) = match transcript {
                Some(transcript) if strict && selection.selects(transcript) => {
                    pending_count += 1;
                    let class = pending::class_of(id, &real_only, &held);
                    match class {
                        Class::RealOnly(_) => strict_real_only += 1,
                        _ => strict_ran += 1,
                    }
                    trials.push(strict_trial(transcript, class));
                    continue;
                }
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
        // A pending, deferred, withdrawn or unselected id is never a pass (plan section 5): the five counts of the report.
        println!(
            "conformance: passed {}, failed {}, pending {} (+ {} with no transcript), deferred {}, withdrawn {}",
            conclusion.num_passed,
            conclusion.num_failed,
            pending_count,
            no_transcript_count,
            deferred.len(),
            withdrawn_count
        );
        // A not-applicable case belongs to an ACTIVE id: the id itself is counted under its run result.
        for case in &cases {
            println!(
                "conformance: not-applicable case `{}` of {} ({}): {}",
                case.case, case.id, case.authority, case.because
            );
        }
        for (id, authority, start) in &deferred {
            println!("conformance: deferred {id} ({authority}; starts when {start})");
        }
        if strict {
            // `cargo xtask ci` reads this line: without it, the harness ignored the variable; with fewer ids run than are
            // pending, a selection narrowed the run.
            println!(
                "conformance strict: ran {strict_ran} of {pending_count} pending ids, {strict_real_only} real-only not run, {} held",
                held.len()
            );
        }
    }
    conclusion.exit();
}

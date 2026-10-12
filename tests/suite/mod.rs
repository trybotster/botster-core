//! The runner of the Core conformance suite (plan section 5), shared by `conformance.rs` (`TestkitHarness`, the default tier)
//! and `slow_conformance.rs` (`RealCoreHarness`, the real-process tier): the same trials and the same report on either
//! harness.
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
//! - on the real tier only, an id in `conformance/core-real-pending.txt` (plan 23l) is an ignored trial of kind
//!   `pending-real`. Under `--ignored` (the run of `cargo xtask ci --job slow` after nextest) it runs and reports itself
//!   ignored with its outcome, never as passed and never as failed; the report names each one that passed, for removal;
//! - on the real tier only, an id of `TESTKIT_PROVEN` (Core A20-1: its proof observes the worker's own allocator) is an
//!   ignored trial of kind `testkit-proven`. It is not run and it is not a failure; the report lists it with its reason;
//! - a non-pending id of `conformance/core-real-only.txt` counts by its named real-process proofs
//!   (`conformance/core-real-proofs.txt`): the gate's slow step runs each named test must-pass, and the minimum count
//!   includes the id by those tests (plan 23s). It has no `TestkitHarness` form (plan 4.2b), so on the testkit tier it is an
//!   ignored trial of kind `real-only` whose reason names its proofs. On the real tier:
//!   - with separate named tests, it is the same ignored trial, and its transcript never runs: the real tier is exempt from
//!     the duplicate trial once the proof passes in the gate (plan section 5);
//!   - when a proof line names its own real-tier trial (`botster-core::slow_conformance conf::<id>`), its transcript runs
//!     must-pass, and that trial is the proof (lead, 2026-10-11, after R-55).
//!
//!   A non-pending real-only id with no proof line is a failing trial on both tiers (fail closed);
//! - every other id runs `run_transcript` over the seed set and passes only on `Outcome::Passed`. On the real tier it then
//!   runs again on a plain `Core::open` (the pass-through test of plan 23l): it must pass there too, unless it needs a
//!   control, which only the wrapped composition serves (`unsupported_control` on the plain harness).
//!
//! A pending or deferred id is never counted as passed. `cargo xtask ci` validates the three files. The seed set and the
//! selection come from the runner's environment variables (`BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE`, `BOTSTER_SEED`).
//!
//! On the testkit tier, with `BOTSTER_PENDING_STRICT=1` (set by the `lists` step of `cargo xtask ci`;
//! `botster_core_testkit::pending`), a selected pending id with a transcript runs: it must fail, a held id (`core-held.txt`)
//! must pass, and a real-only id (`core-real-only.txt`) does not run. The expected result reports itself ignored, so a
//! pending id is still never a pass. The nextest tiers do not set the variable: there a pending trial stays ignored and does
//! not run.

use botster_conformance::report::describe;
pub use botster_conformance::Limits;
use botster_conformance::{load_dir, run_transcript, Outcome, SeedSet, Selection, Transcript};
use botster_core_conformance::{
    driver_for, CoreHarness, CoreSchemas, CORE_TRANSCRIPTS, TESTKIT_PROVEN,
};
use botster_core_testkit::built_in;
use botster_core_testkit::pending::{self, Class, NonPending};
use botster_core_testkit::status::{parse_deferred as parse_status_deferred, parse_withdrawn};
use libtest_mimic::{Arguments, Completion, Failed, Trial};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

/// The Core ids of the ledger at the pinned contracts tag.
const LEDGER_IDS: &str = include_str!("../../conformance/core-ledger-ids.txt");
const PENDING_IDS: &str = include_str!("../../conformance/core-pending.txt");
/// The ids that pass on the `TestkitHarness` and not yet on the real tier (plan 23l). `cargo xtask lists` checks its rules.
const REAL_PENDING_IDS: &str = include_str!("../../conformance/core-real-pending.txt");
const DEFERRED: &str = include_str!("../../conformance/core-deferred.toml");
/// The contracts' status files at the pinned tag (`cargo xtask lists` checks that they are the pinned files).
const CONTRACTS_DEFERRED: &str = include_str!("../../conformance/contracts-deferred.txt");
const CONTRACTS_WITHDRAWN: &str = include_str!("../../conformance/contracts-withdrawn.txt");
/// The minimum Core (plan section 1, revision 23q): the canonical list; its first column is the id.
const MINIMUM: &str = include_str!("../../conformance/minimum-core.txt");
/// The real-only Core ids with their `slow:*` proof class in the replacement map (`id<TAB>proof`; `cargo xtask ledger-ids`
/// checks it).
const REAL_ONLY: &str = include_str!("../../conformance/core-real-only.txt");
/// The named real-process proofs of the real-only ids (`id<TAB>binary<TAB>test`; `cargo xtask lists` and `slow-coverage`
/// check it, and the slow step checks that each named test passed).
const REAL_PROOFS: &str = include_str!("../../conformance/core-real-proofs.txt");
/// The held ids of the strict pending run (`cargo xtask lists` checks it).
const HELD: &str = include_str!("../../conformance/core-held.txt");
/// The workspace crates that these binaries link, with their compile-time manifest directories (bug catalog B13). The
/// coverage guarantee of this list is a separate design item (Core lead, #293 split).
const LINKED: [(&str, &str); 10] = [
    // The test binary itself: its files outside the package root are the ones that RC saw reused.
    ("this test binary", env!("CARGO_MANIFEST_DIR")),
    ("botster-core", botster_core::BUILT_IN),
    ("botster-core-edges", botster_core_edges::BUILT_IN),
    ("botster-core-host", botster_core_host::BUILT_IN),
    ("botster-core-link", botster_core_link::BUILT_IN),
    ("botster-core-sys", botster_core_sys::BUILT_IN),
    ("botster-core-testkit", botster_core_testkit::BUILT_IN),
    (
        "botster-terminal-ghostty",
        botster_terminal_ghostty::BUILT_IN,
    ),
    ("botster-test-process", botster_test_process::BUILT_IN),
    ("botster-worker-core", botster_worker_core::BUILT_IN),
];

// Core TH-1, checked when this suite compiles: the facade's handle is `Send` and not `Sync` (no nightly feature).
static_assertions::assert_impl_all!(botster_core::Core: Send);
static_assertions::assert_not_impl_any!(botster_core::Core: Sync);

/// The answer of `type_check: send_not_sync` (Core TH-1): true, because the two assertions above compile.
pub const CORE_IS_SEND_NOT_SYNC: bool = true;

/// Builds the harness of one seed.
pub type Factory = fn(u64) -> Box<dyn CoreHarness>;

fn id_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The ids of `minimum-core.txt`: the first tab-separated field of each line that is not empty or a comment.
fn minimum_ids(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split('\t').next())
        .map(|id| id.trim().to_string())
        .collect()
}

/// The ids that count by their named real-process proofs (plan 23s), with those proofs as `<binary> <test>`, comma-separated:
/// a real-only id that is not in `core-pending.txt` left it only with them (plan section 5), and the gate's slow step runs
/// each to PASS. A real-only id with no proof line is not here: its trial fails.
fn named_proofs(
    real_only: &[pending::RealOnly],
    proofs: &[pending::RealProof],
    pending: &BTreeSet<String>,
) -> Vec<(String, String)> {
    real_only
        .iter()
        .filter(|entry| !pending.contains(&entry.id))
        .filter_map(
            |entry| match pending::non_pending(&entry.id, real_only, proofs) {
                NonPending::ByNamedProof(_, named) | NonPending::ByOwnRealTrial(_, named) => {
                    Some((entry.id.clone(), proof_names(&named)))
                }
                _ => None,
            },
        )
        .collect()
}

/// The named proofs of an id, as the report prints them: `<binary> <test>`, comma-separated.
fn proof_names(named: &[pending::RealProof]) -> String {
    named
        .iter()
        .map(|p| format!("{} {}", p.binary, p.test))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The progress counts of the minimum Core (plan section 1, revision 23q). `passed` holds the ids whose trial ran and passed
/// on this tier. On the testkit tier: "testkit-passing / N". On the real tier: "real-passing / N - P", where P is the
/// number of A20-1 testkit-proven minimum ids, which the real tier never runs, and "real-accepted / N" = real-passing plus
/// the A20-1 minimum ids that are not in `core-pending.txt` (the testkit tier of the same gate proves that they pass). On
/// both tiers an id of `by_proof` (`named_proofs`) passes too (plan 23s).
fn minimum_report(
    minimum: &BTreeSet<String>,
    passed: &BTreeSet<String>,
    by_proof: &[(String, String)],
    pending: &BTreeSet<String>,
    real: bool,
) -> String {
    let total = minimum.len();
    let passing = minimum
        .iter()
        .filter(|id| passed.contains(*id) || by_proof.iter().any(|(p, _)| p == *id))
        .count();
    if !real {
        return format!("minimum: testkit-passing {passing} / {total}");
    }
    let proven: Vec<&String> = minimum
        .iter()
        .filter(|id| TESTKIT_PROVEN.contains(&id.as_str()))
        .collect();
    let accepted = passing + proven.iter().filter(|id| !pending.contains(**id)).count();
    format!(
        "minimum: real-passing {passing} / {}, real-accepted {accepted} / {total}",
        total - proven.len()
    )
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

fn outcome_of(make: Factory, limits: Limits, transcript: &Transcript) -> Outcome {
    outcome_at(make, limits, transcript, |seeds| seeds)
}

/// Runs one transcript over the seeds that `narrow` keeps of the seed set.
fn outcome_at(
    make: Factory,
    limits: Limits,
    transcript: &Transcript,
    narrow: impl FnOnce(SeedSet) -> SeedSet,
) -> Outcome {
    let seeds = narrow(Selection::from_env().seeds(&SeedSet::from_env()));
    run_transcript(
        transcript,
        &|seed| driver_for(make(seed)),
        &seeds,
        &CoreSchemas,
        &limits,
    )
}

fn run_id(make: Factory, limits: Limits, transcript: &Transcript) -> Result<(), String> {
    let outcome = outcome_of(make, limits, transcript);
    if outcome.is_pass() {
        Ok(())
    } else {
        Err(describe(&transcript.id, &outcome))
    }
}

/// The trial of a pending id in the strict run. A real-only id does not run. Another one runs, and `pending::verdict`
/// decides: a pending id that passes fails its trial, and so does a held id that fails. A `budget` held id runs at the
/// first seed only (`pending::seeds_of`). An expected result reports itself ignored, so that the strict run counts no pass.
fn strict_trial(make: Factory, limits: Limits, transcript: &Transcript, class: Class) -> Trial {
    let id = transcript.id.clone();
    if let Class::RealOnly(_) = class {
        let note = pending::verdict(&id, &class, false).unwrap_or_default();
        return never_passes(&id, "pending: real-only", note);
    }
    let transcript = transcript.clone();
    Trial::ignorable_test(id.clone(), move || {
        let passed = outcome_at(make, limits, &transcript, |set| SeedSet {
            seeds: pending::seeds_of(&class, &set.seeds),
        })
        .is_pass();
        pending::verdict(&id, &class, passed)
            .map(Completion::ignored_with)
            .map_err(Failed::from)
    })
    .with_kind("pending: strict")
}

/// The pass-through test (plan 23l): an id that passed on the wrapped composition passes on a plain `Core::open` too, unless
/// it needs a control.
fn pass_through(plain: Factory, limits: Limits, transcript: &Transcript) -> Result<(), String> {
    match outcome_of(plain, limits, transcript) {
        Outcome::Passed | Outcome::UnsupportedControl { .. } => Ok(()),
        other => Err(format!(
            "pass-through: passes on the wrapped composition and not on a plain Core::open: {}",
            describe(&transcript.id, &other)
        )),
    }
}

/// A trial of an id of `core-real-pending.txt`: an ignored trial, so that nextest skips it. Under `--ignored` it runs and
/// reports itself ignored with its outcome: it is never counted as passed, and its failure never fails the run. A pass is
/// recorded in `passed`, so that the report names the id for removal.
fn pending_real(
    make: Factory,
    limits: Limits,
    transcript: Transcript,
    passed: Arc<Mutex<Vec<String>>>,
) -> Trial {
    let id = transcript.id.clone();
    Trial::ignorable_test(id, move || {
        Ok(match run_id(make, limits, &transcript) {
            Ok(()) => {
                passed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(transcript.id.clone());
                Completion::ignored_with(
                    "pending-real: PASSED on the real tier; remove it from core-real-pending.txt",
                )
            }
            Err(why) => Completion::ignored_with(format!("pending-real: {why}")),
        })
    })
    .with_kind("pending-real")
    .with_ignored_flag(true)
}

/// Runs every Core id of the ledger on the harnesses that `make` builds, under the runner's execution `limits` (design 6.1),
/// and prints the report under `name`. `plain` is the real tier's factory of plain `Core::open` harnesses (plan 23l): with
/// it, the run also reads `core-real-pending.txt` and runs the pass-through test; without it, it is the testkit tier.
pub fn run(name: &str, make: Factory, plain: Option<Factory>, limits: Limits) {
    // A binary built in another worktree (a shared target directory, bug catalog B13) runs that worktree's sources and lists:
    // it refuses before it lists or runs a trial.
    if let Err(error) = built_in::check_here(&LINKED) {
        eprintln!("{name}: {error}");
        std::process::exit(101);
    }
    let args = Arguments::from_args();
    let ledger = id_list(LEDGER_IDS);
    let pending = id_list(PENDING_IDS);
    let real_pending = match plain {
        None => BTreeSet::new(),
        Some(_) => id_list(REAL_PENDING_IDS),
    };
    let real_passed = Arc::new(Mutex::new(Vec::new()));
    let mut real_pending_count = 0usize;
    let mut testkit_proven = Vec::new();
    // The ids whose trial ran and passed (on the real tier: on the wrapped composition and the plain one).
    let ran_passed = Arc::new(Mutex::new(BTreeSet::new()));
    // The ids that have a trial that must pass (not ignored).
    let mut must_pass = BTreeSet::new();
    let deferred = deferred_entries(DEFERRED);
    let withdrawn = parse_withdrawn(CONTRACTS_WITHDRAWN).expect("withdrawn.txt parses");
    let (_, cases) = parse_status_deferred(CONTRACTS_DEFERRED).expect("deferred.txt parses");
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let selection = Selection::from_env();
    // The strict pending run is a run of the testkit tier only.
    let strict = plain.is_none() && std::env::var(pending::STRICT_ENV).is_ok_and(|v| v == "1");
    let real_only = pending::parse_real_only(REAL_ONLY).expect("core-real-only.txt parses");
    let proofs = pending::parse_real_proofs(REAL_PROOFS).expect("core-real-proofs.txt parses");
    let held = pending::parse_held(HELD).expect("core-held.txt parses");
    // The strict run's own count: the pending ids that ran, and the real-only ones that it does not run.
    let (mut strict_ran, mut strict_real_only) = (0usize, 0usize);

    let mut trials = Vec::new();
    let (mut pending_count, mut no_transcript_count, mut withdrawn_count) =
        (0usize, 0usize, 0usize);
    for id in &ledger {
        let transcript = transcripts.iter().find(|t| &t.id == id);
        let treatment = if pending.contains(id) {
            NonPending::MustPass
        } else {
            pending::non_pending(id, &real_only, &proofs)
        };
        if let Some(w) = withdrawn.iter().find(|w| &w.id == id) {
            let replacement = w.replaced_by.as_deref().unwrap_or("nothing");
            let reason = format!("withdrawn by {}; replaced by {replacement}", w.authority);
            withdrawn_count += 1;
            trials.push(never_passes(id, "withdrawn", reason));
        } else if let Some((_, authority, start)) = deferred.iter().find(|(d, _, _)| d == id) {
            let reason = format!("deferred by {authority}; starts when {start}");
            trials.push(never_passes(id, "deferred", reason));
        } else if let (NonPending::ByOwnRealTrial(entry, named), None) = (&treatment, plain) {
            trials.push(never_passes(
                id,
                "real-only",
                format!(
                    "real-only ({}): passes by its own real-tier trial {} in the gate's slow step (plan 23s); no TestkitHarness form (plan 4.2b)",
                    entry.proof,
                    proof_names(named)
                ),
            ));
        } else if let NonPending::ByNamedProof(entry, named) = &treatment {
            trials.push(never_passes(
                id,
                "real-only",
                format!(
                    "real-only ({}): passes by its named real-process proof {} in the gate's slow step (plan 23s); its transcript does not run",
                    entry.proof,
                    proof_names(named)
                ),
            ));
        } else if let NonPending::NoNamedProof(entry) = &treatment {
            let why = format!(
                "{id} is real-only ({}) and not in core-pending.txt, but conformance/core-real-proofs.txt names no real-process proof of it",
                entry.proof
            );
            trials.push(
                Trial::test(id.clone(), move || Err(Failed::from(why))).with_kind("real-only"),
            );
        } else if pending.contains(id) || transcript.is_none() {
            let (kind, reason) = match transcript {
                Some(transcript) if strict && selection.selects(transcript) => {
                    pending_count += 1;
                    let class = pending::class_of(id, &real_only, &held);
                    match class {
                        Class::RealOnly(_) => strict_real_only += 1,
                        _ => strict_ran += 1,
                    }
                    trials.push(strict_trial(make, limits, transcript, class));
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
            if plain.is_some() && TESTKIT_PROVEN.contains(&id.as_str()) {
                testkit_proven.push(id.clone());
                trials.push(never_passes(
                    id,
                    "testkit-proven",
                    "testkit-proven (Core A20-1): its proof observes the worker's own allocator; not run on the real tier"
                        .to_string(),
                ));
            } else if selection.selects(transcript) && real_pending.contains(id) {
                real_pending_count += 1;
                trials.push(pending_real(
                    make,
                    limits,
                    transcript.clone(),
                    Arc::clone(&real_passed),
                ));
            } else if selection.selects(transcript) {
                must_pass.insert(id.clone());
                let transcript = transcript.clone();
                let ran_passed = Arc::clone(&ran_passed);
                trials.push(Trial::test(id.clone(), move || {
                    run_id(make, limits, &transcript)?;
                    match plain {
                        Some(plain) => pass_through(plain, limits, &transcript),
                        None => Ok(()),
                    }
                    .map_err(Failed::from)?;
                    ran_passed
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(transcript.id.clone());
                    Ok(())
                }));
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
            "{name}: passed {}, failed {}, pending {} (+ {} with no transcript), deferred {}, withdrawn {}",
            conclusion.num_passed,
            conclusion.num_failed,
            pending_count,
            no_transcript_count,
            deferred.len(),
            withdrawn_count
        );
        if strict {
            // `cargo xtask ci` reads this line: without it, the harness ignored the variable; with fewer ids run than are
            // pending, a selection narrowed the run.
            println!(
                "{name} strict: ran {strict_ran} of {pending_count} pending ids, {strict_real_only} real-only not run, {} held",
                held.len()
            );
        }
        // Under `--ignored` (the real tier's report run in `cargo xtask ci --job slow`) the trials that must pass do not run
        // here: they ran under nextest first, and the job reaches this run only when every one passed. Then those ids
        // count; otherwise the ids whose trial passed in this run count.
        let passed = if args.ignored {
            must_pass
        } else {
            ran_passed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        };
        let source = if args.ignored {
            " (under --ignored: the trials that passed in the nextest run of this job)"
        } else {
            ""
        };
        let minimum = minimum_ids(MINIMUM);
        let by_proof = named_proofs(&real_only, &proofs, &pending);
        println!(
            "{name}: {}{source}",
            minimum_report(&minimum, &passed, &by_proof, &pending, plain.is_some())
        );
        for (id, proof) in &by_proof {
            let scope = if minimum.contains(id) {
                "minimum: "
            } else {
                ""
            };
            println!(
                "{name}: {scope}{id} counts by its named real-process proof {proof} (plan 23s)"
            );
        }
        if plain.is_some() {
            let passed = real_passed.lock().unwrap_or_else(PoisonError::into_inner);
            println!(
                "{name}: pending-real {real_pending_count} (they run under --ignored, never counted as passed), of which passed {}",
                passed.len()
            );
            for id in passed.iter() {
                println!("{name}: pending-real {id} PASSED: remove it from conformance/core-real-pending.txt");
            }
            for id in &testkit_proven {
                println!("{name}: testkit-proven {id} (Core A20-1): not run on the real tier, not a failure");
            }
        }
        // A not-applicable case belongs to an ACTIVE id: the id itself is counted under its run result.
        for case in &cases {
            println!(
                "{name}: not-applicable case `{}` of {} ({}): {}",
                case.case, case.id, case.authority, case.because
            );
        }
        for (id, authority, start) in &deferred {
            println!("{name}: deferred {id} ({authority}; starts when {start})");
        }
    }
    conclusion.exit();
}

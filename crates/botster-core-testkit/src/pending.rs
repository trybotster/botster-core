//! The strict pending check (plan 23a and 23l; lead, 2026-10-09). A pending id must fail on the testkit: an id that passes
//! leaves `conformance/core-pending.txt`. Two kinds of pending id are exempt:
//!
//! - **real-only**: the pinned replacement map gives the id a `slow:*` proof. The testkit cannot prove it, so the strict
//!   run does not run it. `cargo xtask ledger-ids --write` derives `conformance/core-real-only.txt` from the map; it is never
//!   written by hand. A real-only id leaves `core-pending.txt` only with its named real-process proof (plan section 5):
//!   `conformance/core-real-proofs.txt`, written by hand, names the slow-tier tests that prove it, and the gate's slow step
//!   runs each named test must-pass (plan 23s; lead, 2026-10-11). With separate named tests, neither tier runs its
//!   transcript. When a line names the id's own real-tier trial (`RealProof::is_own_real_trial`), the real tier runs the
//!   transcript must-pass and only the testkit tier does not (lead, 2026-10-11, after R-55).
//! - **held**: `conformance/core-held.txt` lists an id that passes on the testkit and is held (below: an `unproven` id stays
//!   pending), with its reason class, owner and authority. A held id must pass: a held id that fails is an error, so the list cannot go stale. A `budget` id is over the
//!   time budget, so the strict run runs it at the first seed of the set only (lead, 2026-10-09). Its full-seed proof is the
//!   held-full run of the gate's slow step ([`HELD_FULL_ENV`]): there it runs at every seed of the set, one trial a seed,
//!   and must pass at each (lead, 2026-10-11). Its line names that proof ([`BUDGET_PROOF`]).
//!
//!   A `budget` id may also leave `core-pending.txt` (plan 23cn, 2026-10-12): it then counts as passing by the held-full run.
//!   The gate runs that run under nextest with the default tier's profile, so each trial (one id at one seed) has the default
//!   tier's 2 s limit; a failed or over-budget trial fails the slow step, so no head that lacks the proof can land. The
//!   testkit tier does not run its transcript ([`NonPending::ByHeldFull`]): 32 seeds in one test are over the budget. An
//!   `unproven` id stays pending.
//!
//! Formats (`core-real-only.txt` and `core-real-proofs.txt`: one tab between the fields, as the suite reads them;
//! `core-held.txt`: fields separated by two or more spaces, as in `status`):
//!
//! ```text
//! conf::<id><TAB>slow:<what>                                                      (core-real-only.txt)
//! conf::<id><TAB><nextest binary id><TAB><test name>                              (core-real-proofs.txt)
//! conf::<id>  budget|unproven  owner: <package>  (<authority>)  because: <why>   (core-held.txt)
//! ```

use crate::status::{clause, lines};
use std::collections::BTreeSet;

/// The environment variable that turns the conformance harness into the strict pending run. `cargo xtask ci` sets it in
/// its `lists` step.
pub const STRICT_ENV: &str = "BOTSTER_PENDING_STRICT";

/// The environment variable that turns the strict pending run into the held-full run: each `budget` held id runs at every
/// seed of the set, one trial a seed, and no other id runs. `cargo xtask ci` sets it, with [`STRICT_ENV`], in its `slow` step.
pub const HELD_FULL_ENV: &str = "BOTSTER_PENDING_HELD_FULL";

/// The kind of a held-full trial. libtest-mimic puts it in front of the trial's name (`[<kind>] <id>@seed_<n>`), so
/// nextest names the trial with it, and `cargo xtask ci` finds each trial in the JUnit file by that name (plan 23cn).
pub const HELD_FULL_KIND: &str = "pending: held-full";

/// The name of the held-full trial of `id` at `seed`, as nextest and its JUnit file give it.
pub fn held_full_trial(id: &str, seed: u64) -> String {
    format!("[{HELD_FULL_KIND}] {id}@seed_{seed}")
}

/// The text that a `budget` line of `core-held.txt` contains in its `because:` field: its full-seed proof is the held-full
/// run (lead, 2026-10-11: a machine check, so that the rule cannot drift).
pub const BUDGET_PROOF: &str = "Full-seed proof: the gate's slow step, held-full run";

/// A Core id whose proof in the pinned replacement map is `slow:*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealOnly {
    pub id: String,
    pub proof: String,
}

/// A named real-process proof of a real-only id (`core-real-proofs.txt`): a test of the slow tier, by its nextest binary id
/// and test name, as `cargo nextest list` prints them. The test's doc comment cites the id and its clause (lead, 2026-10-11;
/// the reviewers check it by reading).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RealProof {
    pub id: String,
    pub binary: String,
    pub test: String,
}

/// The nextest binary id of the real tier's conformance suite (`tests/slow_conformance.rs`), whose trial of an id is named
/// after the id.
pub const REAL_TIER_SUITE: &str = "botster-core::slow_conformance";

impl RealProof {
    /// The proof is the id's own trial on the real tier: `conf::<id><TAB>botster-core::slow_conformance<TAB>conf::<id>`
    /// (lead, 2026-10-11, after R-55 gave `conf::ad_6_reused_pid_is_never_killed` a real-tier form).
    pub fn is_own_real_trial(&self) -> bool {
        self.binary == REAL_TIER_SUITE && self.test == self.id
    }
}

/// The reason class of a held id. `real-only` is not a class of the held file: those ids come from the replacement map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldReason {
    /// The id passes, but over the default-tier time budget. The strict run runs it at the first seed only; the held-full
    /// run of the slow step runs it at every seed.
    Budget,
    /// The id passes on the testkit, but that pass does not prove the clause; the line names the missing proofs. It runs at
    /// every seed, and its line leaves the file in the PR that adds the last missing proof (lead, 2026-10-09).
    Unproven,
}

/// An id that passes on the testkit and is held: an `unproven` one stays pending; a `budget` one may leave
/// `core-pending.txt` and count by the held-full run (plan 23cn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub reason: HeldReason,
    pub owner: String,
    pub authority: String,
    pub because: String,
}

/// Parses `core-real-only.txt`.
pub fn parse_real_only(text: &str) -> Result<Vec<RealOnly>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|(i, line)| match line.split_once('\t') {
            Some((id, proof))
                if id.starts_with("conf::")
                    && proof.starts_with("slow:")
                    && !proof.contains('\t') =>
            {
                Ok(RealOnly {
                    id: id.to_string(),
                    proof: proof.to_string(),
                })
            }
            _ => Err(format!(
                "line {}: expected `conf::<id><TAB>slow:<what>`",
                i + 1
            )),
        })
        .collect()
}

/// Parses `core-real-proofs.txt`: three fields with one tab between them, a Core id, then a binary id and a test name with
/// no white space.
pub fn parse_real_proofs(text: &str) -> Result<Vec<RealProof>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|(i, line)| {
            let word = |f: &str| !f.is_empty() && !f.contains(char::is_whitespace);
            match line.split('\t').collect::<Vec<_>>()[..] {
                [id, binary, test]
                    if id.starts_with("conf::") && word(id) && word(binary) && word(test) =>
                {
                    Ok(RealProof {
                        id: id.to_string(),
                        binary: binary.to_string(),
                        test: test.to_string(),
                    })
                }
                _ => Err(format!(
                    "line {}: expected `conf::<id><TAB><nextest binary id><TAB><test name>`",
                    i + 1
                )),
            }
        })
        .collect()
}

/// The problems of `core-real-proofs.txt` against the real-only and pending ids. Empty means the file is valid: its lines are
/// in byte order, once each; each names a real-only id; a line of the real tier's suite names the id's own trial; and each
/// real-only id that is not pending has at least one line (it left `core-pending.txt` only with its named proof, plan
/// section 5).
pub fn real_proof_problems(
    proofs: &[RealProof],
    real_only: &[RealOnly],
    pending: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    // One tab sorts before every other byte of a field, so the order of the fields is the byte order of the lines.
    for pair in proofs.windows(2) {
        let fault = match pair[0].cmp(&pair[1]) {
            std::cmp::Ordering::Less => continue,
            std::cmp::Ordering::Equal => "is listed twice",
            std::cmp::Ordering::Greater => "is out of byte order",
        };
        problems.push(format!(
            "core-real-proofs.txt: {} {} {} {fault}",
            pair[1].id, pair[1].binary, pair[1].test
        ));
    }
    for proof in proofs {
        if !real_only.iter().any(|r| r.id == proof.id) {
            problems.push(format!(
                "core-real-proofs.txt: {} is not real-only in the replacement map; only a real-only id has a named proof",
                proof.id
            ));
        }
        if proof.binary == REAL_TIER_SUITE && !proof.is_own_real_trial() {
            problems.push(format!(
                "core-real-proofs.txt: {} names the real-tier trial {}; a line of {REAL_TIER_SUITE} names the id's own trial",
                proof.id, proof.test
            ));
        }
    }
    for entry in real_only {
        if !pending.contains(&entry.id) && !proofs.iter().any(|p| p.id == entry.id) {
            problems.push(format!(
                "{} is real-only ({}) and not in core-pending.txt, but core-real-proofs.txt names no real-process proof of it",
                entry.id, entry.proof
            ));
        }
    }
    problems
}

/// Parses `core-held.txt`.
pub fn parse_held(text: &str) -> Result<Vec<Held>, String> {
    lines(text)
        .map(|(line, fields)| {
            let [id, reason, owner, authority, because] = fields[..] else {
                return Err(format!("line {line}: expected five fields"));
            };
            if !id.starts_with("conf::") {
                return Err(format!("line {line}: `{id}` is not an id"));
            }
            let reason = match reason {
                "budget" => HeldReason::Budget,
                "unproven" => HeldReason::Unproven,
                other => {
                    return Err(format!(
                        "line {line}: the reason class `{other}` is not `budget` or `unproven` (a real-only id comes from the replacement map)"
                    ))
                }
            };
            let nonempty = |text: Option<&'_ str>, what: &str| -> Result<String, String> {
                match text.map(str::trim) {
                    Some(text) if !text.is_empty() => Ok(text.to_string()),
                    _ => Err(format!("line {line}: {what} is missing or empty")),
                }
            };
            let owner = nonempty(owner.strip_prefix("owner: "), "`owner: <package>`")?;
            let authority = nonempty(clause(authority), "the authority in parentheses")?;
            let because = nonempty(because.strip_prefix("because: "), "`because:`")?;
            if reason == HeldReason::Budget && !because.contains(BUDGET_PROOF) {
                return Err(format!(
                    "line {line}: a `budget` line names its full-seed proof: `because:` contains `{BUDGET_PROOF}`"
                ));
            }
            Ok(Held {
                id: id.to_string(),
                reason,
                owner,
                authority,
                because,
            })
        })
        .collect()
}

/// The ids of the Core transcripts at the pinned tag.
pub fn transcript_ids() -> Result<BTreeSet<String>, String> {
    botster_conformance::load_dir(&botster_core_conformance::CORE_TRANSCRIPTS)
        .map(|transcripts| transcripts.into_iter().map(|t| t.id).collect())
        .map_err(|e| format!("the Core transcripts: {e:?}"))
}

/// The problems of the held file against the pending file, the real-only ids and the transcript ids. Empty means the file
/// is valid. A held id must pass, so it needs a transcript.
pub fn held_problems(
    held: &[Held],
    pending: &BTreeSet<String>,
    real_only: &[RealOnly],
    transcripts: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in held {
        if !seen.insert(&entry.id) {
            problems.push(format!("core-held.txt: {} is listed twice", entry.id));
        }
        if !pending.contains(&entry.id) && entry.reason != HeldReason::Budget {
            problems.push(format!(
                "core-held.txt: {} is not pending; an `unproven` held id is a pending id (only a `budget` id may leave \
                 core-pending.txt: it counts by the held-full run, plan 23cn)",
                entry.id
            ));
        }
        if !transcripts.contains(&entry.id) {
            problems.push(format!(
                "core-held.txt: {} has no transcript; a held id must pass, so it needs one",
                entry.id
            ));
        }
        if real_only.iter().any(|r| r.id == entry.id) {
            problems.push(format!(
                "core-held.txt: {} is real-only in the replacement map; it needs no held entry",
                entry.id
            ));
        }
    }
    problems
}

/// What the strict run does with a pending id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// Not run: the testkit cannot prove it.
    RealOnly(RealOnly),
    /// Runs, and must pass.
    Held(Held),
    /// Runs, and must fail.
    Pending,
}

/// The class of a pending id. A real-only id is real-only even when the held file names it (`held_problems` reports that).
pub fn class_of(id: &str, real_only: &[RealOnly], held: &[Held]) -> Class {
    if let Some(entry) = real_only.iter().find(|r| r.id == id) {
        Class::RealOnly(entry.clone())
    } else if let Some(entry) = held.iter().find(|h| h.id == id) {
        Class::Held(entry.clone())
    } else {
        Class::Pending
    }
}

/// What a tier does with an id that is not in `core-pending.txt` (plan section 5, 4.2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonPending {
    /// Its transcript runs, and must pass.
    MustPass,
    /// A real-only id passes by its named real-process proofs, which the gate's slow step runs must-pass. Its transcript
    /// does not run on either tier: it has no `TestkitHarness` form (plan 4.2b), and the real tier is exempt from the
    /// duplicate trial once the proof passes in the gate (plan section 5). The minimum count counts it by its proofs (23s).
    ByNamedProof(RealOnly, Vec<RealProof>),
    /// A real-only id that one of its named proofs names by its own real-tier trial (`RealProof::is_own_real_trial`): the
    /// testkit tier does not run its transcript (no `TestkitHarness` form, plan 4.2b); the real tier runs it must-pass, and
    /// the gate's slow step checks that this trial passed. The minimum count counts it by its proofs (23s). (Lead,
    /// 2026-10-11, after R-55.)
    ByOwnRealTrial(RealOnly, Vec<RealProof>),
    /// A real-only id with no named proof: it must not have left `core-pending.txt`. Its trial fails (fail closed; the
    /// `lists` step reports it too).
    NoNamedProof(RealOnly),
    /// A `budget` held id that left `core-pending.txt` (plan 23cn): it passes by the held-full run of the gate's slow step,
    /// at every CI seed, one trial a seed, each within the default tier's 2 s limit. The testkit tier does not run its
    /// transcript (all seeds in one test are over the budget); the real tier runs it must-pass, as any other id. The minimum
    /// count counts it by that run.
    ByHeldFull(Held),
}

/// The treatment of `id`, which is not pending: a real-only id passes by its named proofs (its own real-tier trial among
/// them, or not), or fails without one; a `budget` held id passes by the held-full run; every other id must pass.
pub fn non_pending(
    id: &str,
    real_only: &[RealOnly],
    proofs: &[RealProof],
    held: &[Held],
) -> NonPending {
    let Some(entry) = real_only.iter().find(|r| r.id == id) else {
        return match budget(held).find(|h| h.id == id) {
            Some(entry) => NonPending::ByHeldFull(entry.clone()),
            None => NonPending::MustPass,
        };
    };
    let named: Vec<RealProof> = proofs.iter().filter(|p| p.id == id).cloned().collect();
    if named.is_empty() {
        NonPending::NoNamedProof(entry.clone())
    } else if named.iter().any(RealProof::is_own_real_trial) {
        NonPending::ByOwnRealTrial(entry.clone(), named)
    } else {
        NonPending::ByNamedProof(entry.clone(), named)
    }
}

/// The seeds at which the strict run runs a pending id of `class`: a `budget` held id at the first seed only, every other id
/// at every seed of the set.
pub fn seeds_of(class: &Class, seeds: &[u64]) -> Vec<u64> {
    match class {
        Class::Held(Held {
            reason: HeldReason::Budget,
            ..
        }) => seeds.iter().take(1).copied().collect(),
        _ => seeds.to_vec(),
    }
}

/// The `budget` held ids: the ids of the held-full run.
pub fn budget(held: &[Held]) -> impl Iterator<Item = &Held> {
    held.iter().filter(|h| h.reason == HeldReason::Budget)
}

/// The result of a `budget` held id's trial at one `seed` of the held-full run: [`verdict`], with the seed named. An id that
/// is not `pending` counts by this run (plan 23cn), so its note says so, and its failure fails the proof.
pub fn held_full_verdict(
    id: &str,
    seed: u64,
    class: &Class,
    pending: bool,
    passed: bool,
) -> Result<String, String> {
    match (class, pending, passed) {
        (Class::Held(entry), false, true) => Ok(format!(
            "held ({:?}, owner {}): passes; it counts by this run (plan 23cn)",
            entry.reason, entry.owner
        )),
        (Class::Held(_), false, false) => Err(format!(
            "{id} counts by the held-full run (plan 23cn) and fails: the gate fails; return it to \
             conformance/core-pending.txt, or fix it"
        )),
        _ => verdict(id, class, passed),
    }
    .map(|note| format!("seed {seed}: {note}"))
    .map_err(|error| format!("seed {seed}: {error}"))
}

/// The `budget` held ids that left `core-pending.txt`: they count by the held-full run (plan 23cn).
pub fn by_held_full<'a>(
    held: &'a [Held],
    pending: &'a BTreeSet<String>,
) -> impl Iterator<Item = &'a Held> {
    budget(held).filter(move |h| !pending.contains(&h.id))
}

/// The strict result of a pending id that ran on the testkit: `Ok(note)` when its result is the expected one, else the error.
pub fn verdict(id: &str, class: &Class, passed: bool) -> Result<String, String> {
    match (class, passed) {
        (Class::Pending, false) => Ok("pending: fails as expected".to_string()),
        (Class::Pending, true) => Err(format!(
            "{id} is pending and passes on the testkit: remove it from conformance/core-pending.txt"
        )),
        (Class::Held(entry), true) => Ok(format!(
            "held ({:?}, owner {}): passes and stays pending",
            entry.reason, entry.owner
        )),
        (Class::Held(_), false) => Err(format!(
            "{id} is held but failing: remove its line from conformance/core-held.txt (it stays pending), or fix it"
        )),
        (Class::RealOnly(entry), _) => Ok(format!("real-only ({}): not run", entry.proof)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELD: &str = "# a comment\n\
        conf::slow_query  budget  owner: p4b-queries-files (unstaffed)  (lead, 2026-10-09)  because: 2.005 s, over 2 s. Full-seed proof: the gate's slow step, held-full run\n";

    fn ids(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn real(id: &str) -> RealOnly {
        RealOnly {
            id: id.into(),
            proof: "slow:lock".into(),
        }
    }

    /// A real-only line is a Core id and its `slow:*` proof, with one tab between them (`cargo xtask ledger-ids` writes it).
    #[test]
    fn a_real_only_line_is_an_id_and_its_slow_proof_separated_by_a_tab() {
        assert_eq!(
            parse_real_only("# x\n\nconf::a_lock\tslow:dir\nconf::z_lock\tslow:lock\n").unwrap(),
            [
                RealOnly {
                    id: "conf::a_lock".into(),
                    proof: "slow:dir".into(),
                },
                real("conf::z_lock"),
            ]
        );
        for bad in [
            "conf::a\tcore-testkit\n",
            "a\tslow:x\n",
            "conf::a  slow:x\n",
            "conf::a\tslow:x\tmore\n",
        ] {
            let error = parse_real_only(&format!("conf::b\tslow:y\n{bad}")).unwrap_err();
            assert!(error.starts_with("line 2: "), "{bad:?}: {error}");
        }
    }

    #[test]
    fn a_held_line_gives_its_class_owner_authority_and_reason() {
        assert_eq!(
            parse_held(HELD).unwrap(),
            [Held {
                id: "conf::slow_query".into(),
                reason: HeldReason::Budget,
                owner: "p4b-queries-files (unstaffed)".into(),
                authority: "lead, 2026-10-09".into(),
                because: "2.005 s, over 2 s. Full-seed proof: the gate's slow step, held-full run"
                    .into(),
            }]
        );
        let bad = |line: &str| parse_held(line).unwrap_err();
        assert!(bad("conf::a  real-only  owner: p  (x)  because: y\n")
            .contains("not `budget` or `unproven`"));
        let unproven =
            parse_held("conf::a  unproven  owner: p6-testkit  (x)  because: y\n").unwrap();
        assert_eq!(unproven[0].reason, HeldReason::Unproven);
        assert!(bad("conf::a  budget  p  (x)  because: y\n").contains("owner"));
        assert!(bad("conf::a  budget  owner: p  x  because: y\n").contains("authority"));
        assert!(bad("conf::a  budget  owner: p  (x)  y\n").contains("because"));
        assert!(bad("a  budget  owner: p  (x)  because: y\n").contains("not an id"));
        assert!(bad("conf::a  budget  owner: p  (x)\n").contains("five fields"));
        assert!(bad("conf::a  budget  owner: p  ()  because: y\n").contains("authority"));
        assert!(bad("conf::a  budget  owner: p  ( )  because: y\n").contains("authority"));
        assert!(bad("conf::a  budget  owner:   (x)  because: y\n").contains("owner"));
        assert!(bad("conf::a  budget  owner: p  (x)  because: \n").contains("because"));
        // A `budget` line names its full-seed proof; an `unproven` line needs no such text.
        assert_eq!(
            bad("conf::a  budget  owner: p  (x)  because: y\n"),
            "line 1: a `budget` line names its full-seed proof: `because:` contains `Full-seed proof: the gate's slow step, held-full run`"
        );
        assert!(
            bad("conf::a  budget  owner: p  (x)  because: Full-seed proof: missing\n")
                .contains("names its full-seed proof")
        );
    }

    /// A held id is listed once, has a transcript, and is not real-only. An `unproven` id is pending; a `budget` id may leave
    /// `core-pending.txt` (plan 23cn).
    #[test]
    fn a_held_id_is_a_pending_id_once_with_a_transcript_and_not_real_only() {
        let held = parse_held(HELD).unwrap();
        let has = ids(&["conf::slow_query"]);
        assert!(held_problems(&held, &has, &[], &has).is_empty());
        assert!(
            held_problems(&held, &ids(&[]), &[], &has).is_empty(),
            "a `budget` id out of core-pending.txt counts by the held-full run"
        );
        let unproven =
            parse_held("conf::slow_query  unproven  owner: p  (x)  because: y\n").unwrap();
        let problems = held_problems(&unproven, &ids(&[]), &[], &has);
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].contains("not pending; an `unproven` held id is a pending id"),
            "{problems:?}"
        );
        let problems = held_problems(&held, &has, &[], &ids(&[]));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("no transcript"), "{problems:?}");
        let problems = held_problems(&held, &has, &[real("conf::slow_query")], &has);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("real-only"), "{problems:?}");
        let twice = [held.clone(), held].concat();
        let problems = held_problems(&twice, &has, &[], &has);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("twice"), "{problems:?}");
    }

    /// Both sides are strict: a pending id that passes is an error, and a held id that fails is an error.
    #[test]
    fn a_pending_id_must_fail_and_a_held_id_must_pass() {
        let held = parse_held(HELD).unwrap();
        let real_only = [real("conf::lock")];
        assert_eq!(class_of("conf::other", &real_only, &held), Class::Pending);
        let as_real = class_of("conf::lock", &real_only, &held);
        assert_eq!(as_real, Class::RealOnly(real_only[0].clone()));
        let as_held = class_of("conf::slow_query", &real_only, &held);
        assert_eq!(as_held, Class::Held(held[0].clone()));

        assert!(verdict("conf::other", &Class::Pending, false).is_ok());
        let error = verdict("conf::other", &Class::Pending, true).unwrap_err();
        assert!(
            error.contains("remove it from conformance/core-pending.txt"),
            "{error}"
        );
        assert!(verdict("conf::slow_query", &as_held, true).is_ok());
        let error = verdict("conf::slow_query", &as_held, false).unwrap_err();
        assert!(error.contains("held but failing"), "{error}");
        assert!(verdict("conf::lock", &as_real, true).is_ok());
        assert!(verdict("conf::lock", &as_real, false).is_ok());

        // A real-only id stays real-only when the held file also names it.
        let both = [real("conf::slow_query")];
        assert_eq!(
            class_of("conf::slow_query", &both, &held),
            Class::RealOnly(both[0].clone())
        );
    }

    fn proof(id: &str, test: &str) -> RealProof {
        RealProof {
            id: id.into(),
            binary: "botster-core::slow_lock".into(),
            test: test.into(),
        }
    }

    /// A proof line is a Core id, a nextest binary id and a test name, with one tab between them and no white space in a field.
    #[test]
    fn a_proof_line_is_an_id_a_binary_and_a_test_separated_by_tabs() {
        assert_eq!(
            parse_real_proofs(
                "# x\n\nconf::lock\tbotster-core::slow_lock\ta_lock\nconf::lock\tbotster-core::slow_lock\tb_lock\n"
            )
            .unwrap(),
            [proof("conf::lock", "a_lock"), proof("conf::lock", "b_lock")]
        );
        assert_eq!(parse_real_proofs("# only comments\n").unwrap(), []);
        for bad in [
            "conf::a\tbin\n",
            "conf::a\tbin\ttest\tmore\n",
            "a\tbin\ttest\n",
            "conf::a\t\ttest\n",
            "conf::a\tbin\t\n",
            "conf::a\tbin\ta test\n",
            "conf::a\tb in\ttest\n",
            "conf::a b\tbin\ttest\n",
            "conf::a  bin  test\n",
        ] {
            let error = parse_real_proofs(&format!("conf::b\tbin\ttest\n{bad}")).unwrap_err();
            assert!(error.starts_with("line 2: "), "{bad:?}: {error}");
        }
    }

    /// The proof file is in byte order, once each; it names only real-only ids; and a real-only id that left the pending
    /// list has a proof. A pending real-only id may already have one.
    #[test]
    fn a_real_only_id_leaves_the_pending_list_only_with_a_named_proof() {
        let real_only = [real("conf::lock"), real("conf::reuse")];
        let pending = ids(&["conf::reuse"]);
        let good = [proof("conf::lock", "a"), proof("conf::lock", "b")];
        assert!(real_proof_problems(&good, &real_only, &pending).is_empty());
        let both = [proof("conf::lock", "a"), proof("conf::reuse", "a")];
        assert!(real_proof_problems(&both, &real_only, &pending).is_empty());
        let one = |proofs: &[RealProof], pending: &BTreeSet<String>| {
            let problems = real_proof_problems(proofs, &real_only, pending);
            assert_eq!(problems.len(), 1, "{problems:?}");
            problems[0].clone()
        };
        assert!(one(&[], &pending)
            .contains("conf::lock is real-only (slow:lock) and not in core-pending.txt"));
        assert!(one(&good, &ids(&[])).contains("conf::reuse is real-only"));
        let twice = [proof("conf::lock", "a"), proof("conf::lock", "a")];
        assert!(one(&twice, &pending).contains("listed twice"));
        let unordered = [proof("conf::lock", "b"), proof("conf::lock", "a")];
        assert!(one(&unordered, &pending).contains("out of byte order"));
        let by_id = [proof("conf::reuse", "a"), proof("conf::lock", "b")];
        assert!(one(&by_id, &pending)
            .contains("conf::lock botster-core::slow_lock b is out of byte order"));
        let other = [proof("conf::lock", "a"), proof("conf::other", "a")];
        assert!(one(&other, &pending).contains("conf::other is not real-only"));
        // A line of the real tier's suite names the id's own trial: of a real-only id, and not of another id.
        let own = |id: &str, test: &str| RealProof {
            id: id.into(),
            binary: REAL_TIER_SUITE.into(),
            test: test.into(),
        };
        assert!(
            real_proof_problems(&[own("conf::lock", "conf::lock")], &real_only, &pending)
                .is_empty()
        );
        let own_other = [
            own("conf::lock", "conf::lock"),
            own("conf::other", "conf::other"),
        ];
        assert!(one(&own_other, &pending).contains("conf::other is not real-only"));
        let not_own = [own("conf::lock", "conf::reuse")];
        assert!(one(&not_own, &pending).contains(
            "conf::lock names the real-tier trial conf::reuse; a line of botster-core::slow_conformance names the id's own trial"
        ));
    }

    /// An id that left `core-pending.txt`: a real-only one passes by its named proofs, by its own real-tier trial when a line
    /// names it, or fails without one; any other id runs its transcript and must pass.
    #[test]
    fn a_real_only_id_that_left_the_pending_list_passes_by_its_named_proofs() {
        let real_only = [real("conf::lock"), real("conf::reuse")];
        let proofs = [
            proof("conf::lock", "a"),
            proof("conf::lock", "b"),
            proof("conf::other", "c"),
        ];
        assert_eq!(
            non_pending("conf::lock", &real_only, &proofs, &[]),
            NonPending::ByNamedProof(real_only[0].clone(), proofs[..2].to_vec())
        );
        assert_eq!(
            non_pending("conf::reuse", &real_only, &proofs, &[]),
            NonPending::NoNamedProof(real_only[1].clone())
        );
        assert_eq!(
            non_pending("conf::other", &real_only, &proofs, &[]),
            NonPending::MustPass
        );
        assert_eq!(
            non_pending("conf::other", &[], &[], &[]),
            NonPending::MustPass
        );
        // A `budget` held id passes by the held-full run; an `unproven` one must pass; a real-only id stays real-only.
        let held = parse_held(&format!(
            "{HELD}conf::other  unproven  owner: p  (x)  because: y\nconf::lock  budget  owner: p  (x)  because: {BUDGET_PROOF}\n"
        ))
        .unwrap();
        assert_eq!(
            non_pending("conf::slow_query", &[], &[], &held),
            NonPending::ByHeldFull(held[0].clone())
        );
        assert_eq!(
            non_pending("conf::other", &[], &[], &held),
            NonPending::MustPass
        );
        assert_eq!(
            non_pending("conf::lock", &real_only, &proofs, &held),
            NonPending::ByNamedProof(real_only[0].clone(), proofs[..2].to_vec())
        );
        // The own real-tier trial, alone or with a separate test.
        let own = RealProof {
            id: "conf::reuse".into(),
            binary: REAL_TIER_SUITE.into(),
            test: "conf::reuse".into(),
        };
        assert!(own.is_own_real_trial());
        for named in [
            vec![own.clone()],
            vec![proof("conf::reuse", "a"), own.clone()],
        ] {
            assert_eq!(
                non_pending("conf::reuse", &real_only, &named, &[]),
                NonPending::ByOwnRealTrial(real_only[1].clone(), named.clone())
            );
        }
        // The real tier's suite with another test, or another binary with the id's name, is not the own trial.
        for not_own in [
            RealProof {
                test: "conf::lock".into(),
                ..own.clone()
            },
            RealProof {
                binary: "botster-core::slow_lock".into(),
                ..own.clone()
            },
        ] {
            assert!(!not_own.is_own_real_trial(), "{not_own:?}");
            assert_eq!(
                non_pending(
                    "conf::reuse",
                    &real_only,
                    std::slice::from_ref(&not_own),
                    &[]
                ),
                NonPending::ByNamedProof(real_only[1].clone(), vec![not_own])
            );
        }
    }

    /// A `budget` held id runs at the first seed of the set only; an `unproven` or pending id runs at every seed.
    #[test]
    fn a_budget_id_runs_at_the_first_seed_and_a_pending_id_at_every_seed() {
        let held = parse_held(HELD).unwrap();
        let class = class_of("conf::slow_query", &[], &held);
        assert_eq!(seeds_of(&class, &[3, 4, 5]), [3]);
        assert_eq!(seeds_of(&class, &[]), Vec::<u64>::new());
        assert_eq!(seeds_of(&Class::Pending, &[3, 4, 5]), [3, 4, 5]);
        let unproven = parse_held("conf::b  unproven  owner: p  (x)  because: y\n").unwrap();
        assert_eq!(
            seeds_of(&Class::Held(unproven[0].clone()), &[3, 4, 5]),
            [3, 4, 5]
        );
        assert_eq!(
            seeds_of(&Class::RealOnly(real("conf::lock")), &[3, 4]),
            [3, 4]
        );
    }

    /// The held-full run takes the `budget` lines only.
    #[test]
    fn the_held_full_run_takes_the_budget_lines() {
        let held = parse_held(&format!(
            "conf::b  unproven  owner: p  (x)  because: y\n{HELD}"
        ))
        .unwrap();
        let ids: Vec<&str> = budget(&held).map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["conf::slow_query"]);
    }

    /// A trial of the held-full run names its seed: a held id that passes there is the expected result, and one that fails
    /// is an error at that seed.
    #[test]
    fn a_held_full_trial_names_its_seed() {
        let held = parse_held(HELD).unwrap();
        let class = class_of("conf::slow_query", &[], &held);
        assert_eq!(
            held_full_verdict("conf::slow_query", 7, &class, true, true),
            Ok("seed 7: held (Budget, owner p4b-queries-files (unstaffed)): passes and stays pending".to_string())
        );
        assert_eq!(
            held_full_verdict("conf::slow_query", 7, &class, true, false),
            Err("seed 7: conf::slow_query is held but failing: remove its line from conformance/core-held.txt (it stays pending), or fix it".to_string())
        );
        // Out of core-pending.txt, the id counts by this run (plan 23cn).
        assert_eq!(
            held_full_verdict("conf::slow_query", 7, &class, false, true),
            Ok("seed 7: held (Budget, owner p4b-queries-files (unstaffed)): passes; it counts by this run (plan 23cn)".to_string())
        );
        assert_eq!(
            held_full_verdict("conf::slow_query", 7, &class, false, false),
            Err("seed 7: conf::slow_query counts by the held-full run (plan 23cn) and fails: the gate fails; return it to conformance/core-pending.txt, or fix it".to_string())
        );
    }

    /// A held-full trial's name has its kind in front, as libtest-mimic lists it and nextest names it (plan 23cn).
    #[test]
    fn a_held_full_trial_is_named_with_its_kind_id_and_seed() {
        assert_eq!(
            held_full_trial("conf::a", 7),
            "[pending: held-full] conf::a@seed_7"
        );
    }

    /// The `budget` ids out of core-pending.txt are the ones that count by the held-full run; an `unproven` id never is.
    #[test]
    fn the_budget_ids_out_of_pending_count_by_the_held_full_run() {
        let held = parse_held(&format!(
            "{HELD}conf::fast  budget  owner: p  (x)  because: {BUDGET_PROOF}\nconf::open  unproven  owner: p  (x)  because: y\n"
        ))
        .unwrap();
        let by = |pending: &[&str]| {
            let pending = ids(pending);
            by_held_full(&held, &pending)
                .map(|h| h.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(by(&["conf::slow_query"]), ["conf::fast"]);
        assert_eq!(by(&[]), ["conf::slow_query", "conf::fast"]);
        assert_eq!(
            by(&["conf::slow_query", "conf::fast"]),
            Vec::<String>::new()
        );
    }

    /// The checked-in files parse, and agree: the held ids are pending and not real-only, and the proof file is valid.
    #[test]
    fn the_checked_in_files_parse_and_agree() {
        let real_only =
            parse_real_only(include_str!("../../../conformance/core-real-only.txt")).unwrap();
        let held = parse_held(include_str!("../../../conformance/core-held.txt")).unwrap();
        let proofs =
            parse_real_proofs(include_str!("../../../conformance/core-real-proofs.txt")).unwrap();
        let pending: BTreeSet<String> = include_str!("../../../conformance/core-pending.txt")
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("conf::"))
            .map(str::to_string)
            .collect();
        assert!(real_only
            .iter()
            .any(|r| r.id == "conf::lc_2_data_dir_is_exclusive"));
        assert!(held_problems(&held, &pending, &real_only, &transcript_ids().unwrap()).is_empty());
        assert_eq!(
            real_proof_problems(&proofs, &real_only, &pending),
            Vec::<String>::new()
        );
    }
}

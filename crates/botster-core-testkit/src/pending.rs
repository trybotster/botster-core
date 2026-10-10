//! The strict pending check (plan 23a and 23l; lead, 2026-10-09). A pending id must fail on the testkit: an id that passes
//! leaves `conformance/core-pending.txt`. Two kinds of pending id are exempt:
//!
//! - **real-only**: the pinned replacement map gives the id a `slow:*` proof. The testkit cannot prove it, so the strict
//!   run does not run it. `cargo xtask ledger-ids --write` derives `conformance/core-real-only.txt` from the map; it is never
//!   written by hand.
//! - **held**: `conformance/core-held.txt` lists a pending id that passes but stays pending, with its reason class, owner and
//!   authority. A held id must pass: a held id that fails is an error, so the list cannot go stale. A `budget` id is over the
//!   time budget, so the strict run runs it at the first seed of the set only (lead, 2026-10-09): its full-seed proof is
//!   missing, and the held file says so.
//!
//! Formats (`core-real-only.txt`: one tab between the fields, as the suite reads it; `core-held.txt`: fields separated by
//! two or more spaces, as in `status`):
//!
//! ```text
//! conf::<id><TAB>slow:<what>                                                      (core-real-only.txt)
//! conf::<id>  budget|unproven  owner: <package>  (<authority>)  because: <why>   (core-held.txt)
//! ```

use crate::status::{clause, lines};
use std::collections::BTreeSet;

/// The environment variable that turns the conformance harness into the strict pending run. `cargo xtask ci` sets it in
/// its `lists` step.
pub const STRICT_ENV: &str = "BOTSTER_PENDING_STRICT";

/// A Core id whose proof in the pinned replacement map is `slow:*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealOnly {
    pub id: String,
    pub proof: String,
}

/// The reason class of a held id. `real-only` is not a class of the held file: those ids come from the replacement map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldReason {
    /// The id passes, but over the default-tier time budget. It runs at the first seed only.
    Budget,
    /// The id passes on the testkit, but that pass does not prove the clause; the line names the missing proofs. It runs at
    /// every seed, and its line leaves the file in the PR that adds the last missing proof (lead, 2026-10-09).
    Unproven,
}

/// A pending id that passes on the testkit and stays pending.
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
        if !pending.contains(&entry.id) {
            problems.push(format!(
                "core-held.txt: {} is not pending; a held id is a pending id",
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
        conf::slow_query  budget  owner: p4b-queries-files (unstaffed)  (lead, 2026-10-09)  because: 2.005 s, over 2 s\n";

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
                because: "2.005 s, over 2 s".into(),
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
    }

    /// A held id is pending, once, has a transcript, and is not real-only.
    #[test]
    fn a_held_id_is_a_pending_id_once_with_a_transcript_and_not_real_only() {
        let held = parse_held(HELD).unwrap();
        let has = ids(&["conf::slow_query"]);
        assert!(held_problems(&held, &has, &[], &has).is_empty());
        let problems = held_problems(&held, &ids(&[]), &[], &has);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("not pending"), "{problems:?}");
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

    /// The checked-in files parse, and agree: the held ids are pending and not real-only.
    #[test]
    fn the_checked_in_files_parse_and_agree() {
        let real_only =
            parse_real_only(include_str!("../../../conformance/core-real-only.txt")).unwrap();
        let held = parse_held(include_str!("../../../conformance/core-held.txt")).unwrap();
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
    }
}

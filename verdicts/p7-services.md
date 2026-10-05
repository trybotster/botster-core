# P7 services and guardian reviews

## PR #142 — round 1

Head: `127994133869e3b6dbbcc8dedf7a22d320225ac5`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
PR: https://github.com/trybotster/botster-core/pull/142.

This review covers P7's guardian machine, private commands, lifecycle tests, and decoder registration.
The PR supplies the required Prior art note and copies no old source.
The machine uses injected time, process results, and control bytes.
The PR adds no mutation exclusion and removes no pending conformance id.
The host services, lane readiness, real edges, and real-process proofs remain outside this PR.
Cross-package acceptance also requires the integration reviewer's verdict.

### P7-142-R1-F1 — HIGH — Stop can skip SIGTERM and consume its grace before delivery

**Requirement:** Core SV-9 requires SIGTERM to the service, followed by SIGKILL after `stop_grace`.
The transcript `conf::sv_9_sigterm_then_group_kill` distinguishes a cooperative SIGTERM exit from forced group cleanup.

**Evidence:** In `crates/botster-guardian-core/src/guardian.rs`, `start_stop` arms `stop` before it emits `Enumerate`.
When that deadline expires, `on_timer` calls `kill_tree`, which sets `kill_requested` while the census remains pending.
The `Input::Descendants` arm then emits `KillTree` instead of `TermService`.
The service receives no SIGTERM in this path.
The existing test `a_late_census_cannot_omit_known_descendants_or_reap_the_leader_early` explicitly expects this incorrect sequence.

A shorter census delay also consumes the service's grace before SIGTERM delivery.
With a 300 ms grace and census completion at 250 ms, the machine permits SIGKILL at 300 ms.
The service receives only 50 ms after SIGTERM.
A delayed `TermSent` result has the same deadline problem.

**Required change:** Preserve the descendant census and unreaped leader protections.
Send SIGTERM to a live service before the Stop path can request SIGKILL.
Measure `stop_grace` from the observed SIGTERM delivery, using injected time.
Keep cleanup for an already exited leader and independent shutdown causes valid.
Replace the late-census test's incorrect expectation.
Add clause tests for census delay beyond the current deadline and delayed SIGTERM delivery.
Each test must prove SIGTERM ordering and a complete grace after delivery.

**Status:** OPEN.

### Verification limits

The implementer reports 17 lifecycle tests and one decoder test passing on this head.
This review used source and transcript inspection; the reviewer ran no tests or gates.
Mutation results and the full gate remain pending.
Any missed mutant or mutation timeout requires a finding and review on its exact head.

VERDICT: NOT CLEAN (1 open)

## PR #142 — round 2

Head: `5a68841a88a307ad9d66f60b1088333ad0312c27`.
Previous head: `127994133869e3b6dbbcc8dedf7a22d320225ac5`.
PR: https://github.com/trybotster/botster-core/pull/142.

This delta review covers the Stop correction, additional lifecycle tests, and two proposed mutation exclusions.

### P7-142-R1-F1 — CLOSED

`start_stop` no longer arms the deadline before enumeration.
The machine emits `TermService` after the census and arms the deadline when it receives `TermSent`.
The delayed-census and delayed-delivery tests check the complete grace and SIGTERM ordering.
The exit-during-census test preserves cleanup and the unreaped leader.

The failed-TERM case also receives a grace when the leader remains live.
I accept that behavior: the guardian attempts SIGTERM, waits, and then attempts group cleanup.
A failed TERM does not set `delivered_reason` or falsely claim `HostStop`.
The real edge must report failed delivery and an exiting leader accurately.
Real-process proof remains assigned to the later edge PR.

### Mutation exclusion: `GuardianConfig::fmt` — ACCEPTED

The exclusion covers one exact mutation in one function.
Returning an empty successful debug result preserves token omission.
No clause fixes the remaining debug text.
`authentication_controls_launch_and_hides_the_token` checks token omission.
This mutation is equivalent for the stated contract property.

### P7-142-R2-F2 — LOW — Log chunk exclusion lacks the required proving test

**Requirement:** The lead's tuning-constant ruling requires one test across the default, mutant, and minimum values.
The ruling also requires an enforced lower bound and one exclusion per function or constant, with named proving tests.

**Evidence:** The new `Guardian::begin` exclusion changes `(MAX - 128) / 4` to `(MAX / 128) / 4`.
The comment correctly identifies smaller positive chunks and an unspecified chunk count.
However, `log_reports_split_at_the_link_bound_and_empty_tails_finish` uses only the compiled chunk expression.
The test does not exercise the default, mutant, and minimum chunk values together.
The exclusion therefore lacks a required condition of the ruling.

**Required change:** Remove the exclusion and kill the mutant, or satisfy all four conditions of the tuning-constant ruling.
Name the proving test and the enforced lower bound in the exclusion comment.
Use one production code path for the chunk behavior.

**Status:** OPEN.

### P7-142-R2-F3 — Mutation evidence — OPEN

The `guardian.rs` mutation run tested the previous head and reported 25 missed, 95 caught, six unviable, and no timeout.
The artifacts are under:
`~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p7_services-e129505d-20261004194426-97024/target-mutants.out/`.

The delta adds tests for missed guards and removes several redundant conditions.
Source inspection supports those corrections, but no mutation result proves closure on this head.
The accepted debug exclusion closes its one equivalent mutant.
F2 covers the proposed log chunk exclusion.
The remaining missed mutations require evidence of closure.

**Required evidence:** Record per-file mutation results on the final reviewed head, including `guardian.rs` and `wire.rs`.
There must be zero missed mutations and zero timeouts after accepted exclusions.
Submit any additional equivalence argument or exclusion for review.

The lead's gate-evidence ruling permits the gate after every other finding closes.
This mutation finding alone will not block that permitted gate.
The reviewer runs no gate.

### Verification limits

The implementer reports 24 lifecycle tests, one decoder test, and focused clippy passing on this head.
This delta review used source and mutation-artifact inspection; the reviewer ran no tests or gates.
The implementer reports that the lead classified PR 1 as single-package and waived integration review for this PR.
Later PRs that change another package still require integration review.

VERDICT: NOT CLEAN (2 open)

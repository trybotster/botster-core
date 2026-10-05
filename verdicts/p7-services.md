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

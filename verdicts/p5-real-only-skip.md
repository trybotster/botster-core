# P5 real-only runner review — #241

## Round 1 — 2026-10-11

Head: `c121ec3105af513afc5a56e1df49f114e92d5bdb`.
Tree: `486fe978efce6bbbf3675b66be852c9d2a0f0863`.
Parent, PR base, and gate base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
The base is contained in the head.
PR: <https://github.com/trybotster/botster-core/pull/241>.
READY: `msg_plugin-w_1791698497_e4309e`.
The published head, base, and gate agree.

Authority: approved plan 23ai, `~/botster-sessions/pins/stage1-plan.c7cbc474.md`.
Verified SHA256: `ea4b778d22a7421c0da60a224e7624ffe50e30799dfc919087b6e9787df9bd6b`.
Relevant plan rules are sections 1, 4.2b, 5, and 8.
BUILD.md at contracts `56bd0a5` applies.
The lead's three explicit conditions arrive in `msg_plugin-w_1791698508_cbb2e8`.
Risk: HIGH under rule 1 for gate-decision code and rule 3 for shared testkit changes.
The published body states HIGH and identifies the runner's gate decision.

### Finding

**RO-F1 / integration SI-1 LOW — CLOSED: the PR body omitted the required Prior art note.**
`pair-common.md`, Old code, requires that note on every PR.
The reviewer sent this finding directly to P5 and Sol integration; Sol independently reported the same omission.
P5 added the note in `msg_plugin-w_1791698601_8ee5b6` without changing the head.
The reviewer read the published correction and checked the cited old skip helper and opt-in ignore annotations.
The note names old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and rejects its runtime host skip and opt-in tests.
It names the reused parser, ignored-trial helper, and minimum report.
It explains why the pure decision belongs beside the existing tested decisions in `pending.rs`.
No code was copied from old Core, so no Stolen-From trailer is required.

### Source and required tests

The reviewer read the complete two-file delta: 63 insertions and one deletion.
The files are `crates/botster-core-testkit/src/pending.rs` and `tests/suite/mod.rs`.
The reviewer read both affected files and their callers.
No conformance list, contracts pin, dependency, mutation exclusion, native configuration, or product source changes.
`git diff --check` passes.

The source satisfies the lead's three conditions:

1. The suite handles pending IDs and missing transcripts before it calls `non_pending`.
   It uses `parse_real_only`, which requires a `conf::` ID and a `slow:*` proof.
   `non_pending` returns `NoTestkitForm` only for a listed ID on the testkit tier.
   The resulting trial has kind `real-only` and names its proof.
   `never_passes` returns an ignored completion even under `--ignored` or `--include-ignored`.
2. On the real tier, `non_pending` returns `MustPass` for that ID.
   The suite retains its existing real trial and pass-through path.
   The new branch does not change the real pending or A20-1 testkit-proven rules.
3. The new test checks two listed IDs on the testkit tier and a listed ID on the real tier.
   It checks an unlisted ID on both tiers, with both populated and empty real-only lists.
   The existing strict test requires an error when a pending ID passes and when a held ID fails.
   Both required tests pass at the exact head in 0.002 seconds each.

No real-only ID has left the current pending list, so the new suite branch changes no existing trial today.
The named-proof minimum count remains unchanged.
Both `pending-real` report formats remain unchanged.
The new `real-only` trial kind cannot match either report format.
P6's later real-tier work must retain this split.

The reviewer read the old local-PTY skip helper at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
That helper selects by platform availability; this change selects by the reviewed replacement map and suite tier.
The current `parse_real_only`, strict pending decision, ignored-trial helper, and real pass-through path provide the reusable parts.

### Exact-head evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-only-skip-c121ec31-pool-20261010-225217-51859.log`.
The command is the plain Linux gate with `CARGO_BUILD_JOBS=4` and `NEXTEST_TEST_THREADS=4`.
All ten stages pass in 436.5 seconds.
Default: 1604 passed, 484 skipped, in 12.063 seconds.
Slow: 412 passed, 1992 skipped, in 110.328 seconds.
Testkit conformance reports 206 passes and zero failures.
The reviewer counted 125 distinct actual real-tier PASS records.
Strict pending runs 373 of 394 transcripts, with 21 real-only IDs and two held IDs.
Every conformance file equals the base bytes.
Ledger: 690; Core pending: 464, including 70 IDs without transcripts; real pending: 80; deferred: two; withdrawn: 18.
Minimum results remain testkit 55 / 69, real-passing 36 / 68, real-accepted 37 / 69.
Pending-real reports zero newly passing IDs.

The reviewer checked all five printed mutation outcomes.
Stage 1 catches four and finds one unviable, with zero missed or timeout, in 86.7 seconds.
The catches cover the tier guard, its negation, and the ID equality check.
The whole-function default replacement is unviable.
Stage 2 does not run because stage 1 misses no mutant.
The full mutation step takes 89.3 seconds.
The job exits zero after 507 seconds on msa1, with no queue wait; the gate exits zero after 508 seconds.

This change contains no behavior that the OS decides, so plan 23ai requires no focused Mac run for its own delta.
All findings close, including LOW.
No new source finding remains.
The reviewer changed only this verdict file and ran no builds, tests, gates, mutation jobs, measurements, or base-merge checks.
Sol integration must publish its separate final verdict after it reads this package verdict.

VERDICT: CLEAN

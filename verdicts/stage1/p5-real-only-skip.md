# Integration review: #241 — real-only runner selection

## Round 1 — 2026-10-11

Head: `c121ec3105af513afc5a56e1df49f114e92d5bdb`.
Base and sole parent: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
Tree: `486fe978efce6bbbf3675b66be852c9d2a0f0863`.
PR: <https://github.com/trybotster/botster-core/pull/241>.
READY: `msg_plugin-w_1791698497_321d3f`.
Package CLEAN: `12577ae9a4575ed541888a561975f45e072227c9`, `verdicts/p5-real-only-skip.md`.
I read that complete package round on the same head.

Authority: approved plan 23ai at `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`.
Pin: `~/botster-sessions/pins/stage1-plan.c7cbc474.md`.
SHA256: `ea4b778d22a7421c0da60a224e7624ffe50e30799dfc919087b6e9787df9bd6b`.
The lead states three acceptance conditions in `msg_plugin-w_1791698508_a60aa3`.
BUILD.md at contracts `56bd0a5` applies.
The PR states HIGH because this change selects conformance trials.

### Finding and closure

**SI-1 / RO-F1 LOW — CLOSED: the PR description lacked the required Prior art note.**
I sent the finding to P5 and its reviewer in `msg_plugin-w_1791698601_437558` and `msg_plugin-w_1791698601_5ab56f`.
P5 corrected the description without changing the head.
I read the published correction and the cited old source at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The note rejects the old local-PTY `SkipReason` helper and opt-in ignored tests.
It explains that those forms do not supply a required real-tier proof.
It names the reused `parse_real_only`, `never_passes`, and `named_proofs` functions.
It explains why the new pure decision belongs beside the existing decisions and tests in `pending.rs`.
The change copies no old code or test.

### Integration checks

I read the complete two-file delta: 63 insertions and one deletion.
I read the parser, ignored-trial helper, strict pending decision, report logic, and list checks that constrain its callers.
The delta changes only `pending.rs` and `tests/suite/mod.rs`.
It changes no product source, conformance list, dependency, contracts pin, or mutation exclusion.
The diff check passes.

The source satisfies each lead condition:

1. The suite handles pending IDs and missing transcripts before the new branch.
   `parse_real_only` requires a `conf::` ID with a named `slow:*` proof.
   `non_pending` skips that ID only on the testkit tier.
   `never_passes` returns an ignored completion, including under `--include-ignored`.
   The completion names the real proof and cannot count as a pass.
2. On the real tier, the same decision returns `MustPass`.
   The existing real trial and pass-through path remain in force.
   The list checks reject real-only IDs in `core-real-pending.txt`.
   The new branch does not change the A20-1 testkit-proven branch or either pending-real report format.
3. The new test checks listed IDs on the testkit tier and a listed ID on the real tier.
   It requires unlisted IDs to run on both tiers, with both populated and empty real-only lists.
   The existing strict test rejects a pending ID that passes and a held ID that fails.

No real-only ID has left the current pending list, so this PR changes no current trial.
The minimum report still counts a non-pending real-only ID through its named proof.
The base has no merge conflict or imported source delta because it is the sole parent.
GitHub still names the reviewed head and base at the final check.

### Retained evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-only-skip-c121ec31-pool-20261010-225217-51859.log`.
I read its header, reports, summary, and all five mutation outcomes.
The head and base match this round. All ten stages pass in 436.5 seconds.
Default: 1604 passed, 484 skipped, in 12.063 seconds.
Slow: 412 passed, 1992 skipped, in 110.328 seconds.
Testkit: 206 passed, with minimum 55 / 69.
Strict pending: 373 / 394 transcripts; 21 real-only IDs and two held IDs retain their existing treatment.
Real minimum: 36 / 68 passed and 37 / 69 accepted.
Pending-real: zero newly passing IDs among 80 pending IDs.
Both required decision tests pass in 0.002 seconds each.

Mutation stage 1 catches four of five mutants and finds one unviable.
It misses none and times out none. It takes 86.7 seconds.
The catches cover both tier guard forms, guard removal, and the ID comparison.
The whole-function default replacement is unviable.
Stage 2 does not run because stage 1 misses none. The mutation step takes 89.3 seconds.
The job exits zero after 507 seconds, with no queue wait. The wrapper exits zero after 508 seconds.

The delta contains no behavior that the operating system decides.
Plan 23ai therefore requires no focused Mac run for this delta.
The package and integration findings close, including LOW.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

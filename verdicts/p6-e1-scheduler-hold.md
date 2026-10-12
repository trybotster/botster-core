# P6 review: E1 scheduler holds — PR #300

## Round 1 — 2026-10-11

Exact head: `d5c3c038d0f9a01fb915ea7b16b2f34900569685`.
PR source base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
Gate base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Tree: `ba80b57d294250018659db3f1c0fe5f04d52b20d`.
Risk: HIGH. The PR changes a production trait, the pump, the shared testkit, and mutation exclusions.

I read the amended design before code review. I read the complete 16-file source delta and relevant driver, engine, and scheduler context.
I read the complete PR body after GitHub MCP truncates it.
Raw body SHA256: `cd60af2200b769b4f708c8e849458a4b70f0a6426fed6d716d14a990a0169f23`.

The provided Scheduler methods preserve the production default: held returns false; admitted discards its arguments.
The admission report occurs once after a successful engine admission. A refused admission returns before the report.
The testkit schedulers record admitted operations and fixed timing without drawing choices.
The real scheduler tap also forwards the admission to its inner scheduler. No Timed CoreApi layer remains in the effective delta.
The controls share one hold implementation, use typed fixed-timing refusal, and signal the host wake after release.
The driver filters held work before choice points. Its more and wake decisions use the remaining runnable work.
Deadline, Silent, and never_deferred work bypass the hold. The existing OperationDeferral pick remains in place.
The public stop/deadline tests, registry tests, and real release/wake test exercise useful behavior.
The new real test uses the existing harness ownership and bounded event-wait helper. It adds no sleep or production-child reap.
No product behavior defect was found in this source review. Two required test findings remain below.

### E1-P1 — MEDIUM — OPEN: the new tests assert implementation state

`crates/botster-core-host/src/tests/driver/holds.rs:150` obtains the engine and asserts three exact ops_of vectors.
The no-flow test makes no public progress or scheduler assertion after it installs the hold.
It therefore tests the internal work mapping instead of the no-flow behavior required by lead condition 1.
The retained mutation record cites this test for three ops_of catches. Those catches do not replace the required public behavior proof.
Replace these assertions with a public scheduler observation and/or public Core progress assertion for the no-flow case.
Keep the condition's default-draw behavior observable.

`crates/botster-core-edges/src/scheduler.rs:200` also compares the whole Production value after admitted.
Its fields hold scheduler state. Assert public choice behavior after admission instead of equality of that stored state.
The recorded policy's choice-call log is an observation of the public scheduler calls and can remain.

### E1-P2 — MEDIUM — OPEN: lead condition 3 has no seeded before/after proof

`crates/botster-core-host/src/tests/driver/holds.rs:159` compares two runs of the new implementation.
Holding delegates choices to Production, not the seeded scheduler. The log records only ChoicePoint and candidate count.
It does not record returned picks. One run has no hold; the other holds an operation that never exists.
The comparison cannot establish the required unchanged seeded sequence before and after this PR.
The focused conformance passes at seeds 0-31 establish transcript results. They do not compare scheduler choices with the earlier behavior.
Supply a comparison of the same seeded public picks, counts, and order against the pre-change behavior.
The test must identify its before/after reference. Update the PR body's condition-3 claim to match that proof.
Integration receipt `msg_plugin-w_1791773954_053299` independently confirms this gap.

### Retained evidence

Focused Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194155-49849.log`.
SHA256: `51a5d271e526823ef22b9438c926103df9d4a51eb3422a44eb7a8e994c085b31`.
I read the decoded command, selected test results, API diff, strict seed commands, and exits.
Clippy exits zero. Edges passes six tests; host passes 299, including all five new holds tests.
The five op_holds tests pass. Both selected real controls tests pass.
The API comparison exits zero for each API extraction. Its diff only adds the two provided methods and their re-exports/defaults.
The API diff exits one because those additions differ. The script restores the edges source before later tests.
The selected atomic-transitions transcript passes on each tier under BOTSTER_SEEDS=0-31. Both commands exit zero.
The job exits zero after 40 seconds. The wrapper exits zero after 41 seconds.

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194248-53397.log`.
SHA256: `22d124d3b70a6e565c276afddcda89938f0b64409fa2bfec6ae0c30cc486f66d`.
I read the exact-head/base header, lists records, tier/stage summaries, raw mutation outcomes, named catches, and exits.
All ten stages pass in 796.9 seconds. Default passes 1855 tests, with 434 skipped, in 22.358 seconds.
Slow passes 608 tests, with 2044 skipped, in 62.267 seconds.
Stage 1 has 64 unique mutants: 42 caught, 13 missed, nine unviable, and no timeout, in 205.9 seconds.
Stage 2 tests exactly those 13 misses and catches all 13, in 315.7 seconds. Its unmutated slow baseline passes.
All 55 raw catches match their named catches. All named catchers also pass normally.
The aggregate has 55 caught, zero missed or timed out, and nine unviable. Unviable outcomes supply no catch evidence.
The complete log has no FLAKY record or failed nextest result.
The job exits zero after 806 seconds: one queued and 805 running. The wrapper exits zero after 807 seconds.

The two new exclusions match equivalent provided-default bodies. Their exact patterns do not exclude the overrides.
The moved LINK_CAPACITY exclusion retains its existing reason and follows the changed line. No broader exclusion is added.
The source removes only conf::a5_2_reads_match_atomic_transitions from core-pending.txt.
I read that transcript at contracts-v0.1.41, commit `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`.
It asserts public get and event behavior around hold/release. The selected 32-seed commands run that exact identifier on both tiers.
The deferred-completion identifier remains pending. This review accepts no future R-92 transcript or pin change.

I read all three named Prior-art functions at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
They report buffered output or held session termination, rather than scheduler control of one operation's progress.
The note rejects each for that reason and uses the approved scheduler policy seam.

I sent both findings directly to P6 and integration. No ordinary NOT CLEAN finding goes to the lead.
The final-slot import, applicable base review, exact-head final gate, and final body remain due after these findings close.
I changed no product code and ran no execution job. This verdict accepts no later head, pin, or status removal.

VERDICT: NOT CLEAN

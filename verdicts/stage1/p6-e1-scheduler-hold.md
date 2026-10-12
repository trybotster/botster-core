# PR #300 integration review

## Round 1

Exact head: `d5c3c038d0f9a01fb915ea7b16b2f34900569685`. Source base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`. Gate comparison base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`. Lead assignment: `msg_plugin-w_1791773852_ef8d44`. Branch: `stage1/p6-e1-scheduler-hold`. Risk: HIGH.

Sol-B read the amended design, public Scheduler methods, driver and engine changes, both scheduler adapters, shared hold controls, required proofs, conformance change, complete body, and retained evidence.

No additional cross-package source defect was found. The production policy retains the provided defaults. Admission reporting draws no choice and occurs after successful engine.begin. A refused admission reports nothing. Both adapters record into the same hold object that their handle's controls use. The real adapter also forwards to its inner scheduler. The Timed CoreApi layer is absent.

The driver filters held steps before choosing work. It keeps deadlines and fixed-timing work outside the hold policy. Release signals the handle's wake. Both tiers share the control validation. A plain real harness retains Unsupported for these controls. No production test branch or worker-machine change is added.

### E1-I1 — MEDIUM — OPEN: the required seeded comparison is absent

Matches the package reviewer's preliminary E1-P2. The lead requires identical seeded choices before and after the change. with_no_hold_the_choice_sequence_is_unchanged instead uses Production, records only choice point and candidate count, and compares two runs of the new implementation. It records neither seeded selected results nor the old implementation's sequence.

Required closure: compare the same seeded workload before and after the change with no hold. Retain the selected results as well as the choice points and counts. Establish equality against the pre-change implementation. The default-method test alone proves no draws from those methods; it does not supply the required before/after comparison.

Sol-B independently confirmed this gap and sent it to P6 and 016a.

### E1-I2 — MEDIUM — OPEN: required proofs inspect private representation

Matches the package reviewer's preliminary E1-P1. a_session_with_no_flow_serves_no_op asserts engine.ops_of results directly. It does not exercise the scheduler call or public progress behavior for a no-flow session. The default-method test compares the complete Production object with a new instance.

Required change: replace these representation assertions with behavior proofs. Demonstrate the no-flow scheduler/progress behavior through the driver. Demonstrate unchanged choices through the scheduler edge. Preserve the lead's no-flow and no-draw conditions without inspecting private state.

The package reviewer sent both preliminary findings to P6 in `msg_plugin-w_1791773927_969f5e`. The completed same-head package verdict remains due. This review does not claim package CLEAN.

### Evidence and accounting

Full gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194248-53397.log`. The header names the reviewed head. The lists step names comparison base `bfef8865` and merge base `1b984f06`. All ten steps PASS. Default: 1,855 passed. Slow: 608 passed. Mutants: 64 total, 55 caught, 0 missed, 0 timeout, 9 unviable. Stage 2 catches all 13 stage-1 misses. The log contains no FLAKY result. The job exits 0 after 806 seconds; the wrapper exits 0 after 807 seconds.

Focused evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194155-49849.log`. Clippy passes. The host and edge tests pass. Both control tests through the harness and the real wake proof pass. The public edge API comparison adds the two provided methods without removing a line. Strict conformance passes a5_2_reads_match_atomic_transitions on both tiers at seeds 0-31.

That ID is the only own pending removal. The deferred-completion ID remains pending with the R-92 transcript correction recorded. No other ID or status removal is approved. The two new default-body mutation exclusions are equivalent to their current bodies. Overrides and the true held mutant remain tested.

The gate comparison base does not establish its import. The lead requires #291 to merge first. The final #300 delta must preserve #291's draining term at the settle site only, while both sites use the held-aware runnable decision. The current-base import, required check, applicable delta review, and exact-head final gate remain due.

Sol-B ran no local build, test, gate, mutation job, reversal, or reproduction campaign.

VERDICT: NOT CLEAN (2 open)

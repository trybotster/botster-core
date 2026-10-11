# Core #284 — route paste and R-82 write classification

## Round 1 — complete source review, 2026-10-11

Head: `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Risk tier: HIGH. Package CLEAN: `561d49824f7fd68df840f199b3a270c0171b94bf`.
Sol reads the complete package round, corrected live body, original eight-file delta, R-82 delta, all imported paths, and affected source context.
Authority: plan 23bp's approved PR A, RC's route-input design, pinned TS-5b/TS-5d, Core IN-2/IN-8/AM-2, R-75, and R-82.
The lead's HOLD on the original head ends with this R-82 READY delta.

### Finding history

RC-PASTE-1 — LOW, closed. Sol identified the body count before the R-82 hold.
The original body said 12 queue_overflow identifiers. Its 34-row table contains 14 such identifiers.
Package `98e8fdcafef995b2a66a7f17f5541252a9e51ba9` confirmed NOT CLEAN on the original head.
The corrected body says 14. Sol independently counts 34 rows and 14 queue_overflow rows.
No open finding remains on the exact head above.

### Package boundaries and interfaces

The route holds one bounded open paste. A continuation must match its op.
A sequencing refusal preserves the open paste and has zero write effect. The first oversized chunk discards the paste.
An abort before commit and a route close discard the open paste without a write or answer.
The commit enters the existing admission point as one InputPayload transaction.
The existing libghostty encoder supplies markers and checks require_bracketed at the transaction's start.
The route sends input_done only for want_result. Refusals report exact encoded PTY bytes when a prefix was written.
The read allowance counts held input. The existing refusal reservation bounds the result frame.
The client receipt revision still advances once for a complete refused input frame.
Paste and file slots remain separate. No file edge or route key, mouse, focus, or query-reply implementation enters this PR.
The tests observe actions, route frames, and host results. Expected paste markers come from an independent libghostty terminal.

R-82 classifies the failed edge instead of the event order.
PROGRAM_SIDE_GONE names EIO 5 for Linux and measured macOS, and EPIPE 32 for the testkit edge.
Those errors use the payload-end completion path. Other errors remain Failed, including after PayloadExited.
The shared completion path retains exact host and route counts. A certain zero becomes NotWritten(SessionEnded).
The route test covers EIO, EPIPE, and ENOSPC with the error before and after PayloadExited.
The host test covers a partial prefix and a certain zero.
The testkit's pty_fail_after now injects ENOSPC. Its pinned control requires Failed with exact counts.
The program-end error remains EPIPE. This keeps both error classes distinct without a production test branch.
The transcript is unchanged.

The imported route log reads the program edge or real probe. Its unattributed label does not invent an op attribution.
The real route validates its session instance and holds the host tap weakly.
The imported xtask tests retain the scan decisions and leave the repository scan in the gate.
The mutation configuration has no own change. No inherited exclusion covers the new paste or R-82 decisions.

### Prior art and import

Sol reads old Core's intake_terminal_command and all three permitted test scenarios at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Steal `fcdb88d00ae0d5ae44df3611ced0377772aecb31` has Stolen-From and For-Clause trailers.
The lead explicitly permits those scenarios. The current contract supplies their expected values.
The PR rejects old op ordering, fixed bounds, timeout, chunk index, and safety policy.
The original head is `e062de537d534b89336ef276b2ddd3e1694abc52`; R-82 fix is `21318131c6afbdd909f88ccfe3949a276349144d`.
Import `f56f8841cc41e5604fd72d1c85ed01ab6690640a` has that fix and the reviewed base as parents.
The lead permits this early import because lists needs the base's removals.
Sol reviews its overlapping list paths and all eleven imported paths. No automatic disjoint-path carry is claimed.
The static merge tree equals the import tree, `d64316653d2dff8afc7c37342d6c07046fbf49ae`.
Later own commits contain only the three additional pending moves and the ENOSPC correction.

### Conformance accounting

The historical comparison covers 34 testkit identifiers and four minimum identifiers on the real tier.
Sol reads every named result, expected value, and received value in both comparison logs.
Testkit PASS grows from zero to five, with no PASS regression.
The real minimum ev_8_no_route_shadow_answers_at_once changes from step_timeout to PASS.
The other three retain their raw-output or unsupported-control limits. No completion is claimed for them.
Four initial identifiers leave testkit pending. in_3_route_reports_only_exceptions_on_the_route enters real-pending because echoed output defeats its expect.
The query-retirement identifier remains pending with the lead-approved second budget hold.
Its real_wait uses the worker's real query clock. GS owns the later full-seed gate work.
This review accepts no full-seed proof for that hold and no future #286 import.

After R-82 and the base import, lane-full and route-op namespace pass on both tiers.
They leave testkit pending. Invalid-UTF8 passes on the testkit and enters real-pending.
Its Mac log contains four failures at the empty PTY-log read. The body labels a missing fence as a likely cause, not a proved cause.
Its Linux pending trial passes, but it stays real-pending because the Mac failure remains.
Against the reviewed base, there are seven own testkit removals and two own real-pending additions.
The four imported testkit removals remain separate. Sol independently checks these sets.
The active contracts pin remains .36. The partial-paste session-ended identifier stays pending until the .39 pin.
Its scratch .39 evidence is diagnostic only; it does not change this pin or this pending list.

### Execution evidence

Exact-head gate: `~/botster-sessions/gates/botster-core-stage1-rc-paste-8d56987e-pool-20261011-115009-66983.log`.
SHA-256: `3b45c7a8b68171dc35b2214f3cef7ae2e3a61d9beb2bdcef4f7424ad8833179f`.
Sol reads the exact head and base, tier summaries, relevant tests, proof records, pending report, all mutation identities, and final exits.
All ten steps PASS in 292.1 seconds. The job and wrapper exit zero after 300 seconds, with one second queued.
Default: 1771 PASS, 467 skipped, 17.568 seconds. Slow: 564 PASS, 2026 skipped, 58.300 seconds.
Minimum: testkit 62/69, real-passing 50/68, real-accepted 51/69. All four named real proofs pass.
The two R-82 tests and the pty_fail_after completion test pass.
Mutation: 58 total, 55 caught, three unviable, zero missed, zero timeout.
All 55 raw caught identities equal all 55 catcher identities, with no duplicate.
Both constant replacements of PROGRAM_SIDE_GONE.contains(&errno) are caught.
The three unviable mutants return Some(Default::default()) from the three paste decisions.
Stage 2 has no remaining mutant. Fuzz selects no changed decoder crate. The complete log contains no FLAKY token.

Mac errno record: `gates/botster-core-rc-r82-pty-errno-mac-e062de53-20261011-113602.log`.
It reports EIO after program exit in three cases and EIO while a live program has closed its terminal.
Mac reversal record: `gates/botster-core-rc-r82-reversal-mac-21318131-20261011-113750.log`.
With input.rs restored to the original head, both new tests fail; 190 of 192 tests pass.
The first failing route case is EIO before PayloadExited. The source test covers both orders; the reversal stops at its first failed assertion.
Scratch .39 record: `gates/botster-core-rc-r82-session-ended-v039-mac-b4626b84-20261011-114024.log` reports one PASS.
Additional pending record: `gates/botster-core-rc-paste-gate3-ids-mac-f56f8841-20261011-114337.log` retains the four invalid-UTF8 failures.

Historical gates remain distinct: d1821fe2 fails the query-retirement budget; e062de53 passes; f56f8841 fails lists; 375e8de1 fails the error-class test.
The lists failure names exactly the three later removals. The error-class failure reports Partial instead of Failed.
The exact-head ENOSPC correction closes that error-class failure. No unchanged-head retry or unrelated timeout increase is accepted.
Historical manual reversals and comparison runs are scratch evidence, not gates for the current head.
The historical .36 comparison records zero-based raw steps; the corrected body counts steps from one.

Both source CLEANs now cover this exact head and base.
RC must request FINAL-READY and wait for the lead's SLOT after #283.
The applicable final import review or carry, base check, exact-head final gate, and final body remain due under 23bu.
This source verdict accepts no future source, gate, pin, or pending removal.
Sol runs no build, test, gate, mutation, measurement, or executable base-check script. Never poll.

VERDICT: CLEAN (0 open) at 8d56987ec2d7983bdeebd2101cca751f1d2c92ee

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

## Evidence receipt — 2026-10-11

RC withdraws the shared-target scripted log for #285 under hazard B13. That log is outside #284's verdict.
For #284, RC supplies botster-core-rc-paste-invalid-utf8-clean-mac-8d56987e-20261011-120738.log.
Sol reads the complete log. Its header states a clean target and a build from this worktree only.
All three real runs fail at seed 0, step 13, with the same empty PTY log.
The retained real-pending entry remains supported. The source CLEAN verdict at 8d56987e does not change.
The final READY must identify build provenance for every cited local record under the lead's new B13 condition.

## Round 2 — final import and pending correction, 2026-10-11

PR: Core #284, HIGH.
Head: `006b61cc454e0dce4e0063852e6f0701fef081bc`.
Base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Tree: `5b13e06270096dfad3b73ea8cf87136c26da0261`.
Previous reviewed head: `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
Previous base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.

### Source and base composition

Sol reviews the merge `716f07b05cb1a331eb2fc6a18800417789b1be9c` and all three subsequent commits through this head.
The imported base includes #283 and #282. The base tree exactly equals #282's reviewed and gated tree, `20f7a73777f20e8a976b62c4b0f70792ea2fd1cb`.
Sol verifies that merge's parents and tree. This closes the #282 merge receipt without a new execution claim.
The previous own delta has ten files; the imported base delta has fourteen.
Their overlap contains only `program.rs` and `core-real-pending.txt`.
At the merge, both files preserve the previous own additions and deletions exactly against their respective bases.
The other eight own files preserve their previous blobs at the merge.
All other final tree entries preserve the imported base, except the stated own changes.
After the merge, only the two pending files change. All worker code remains unchanged.
The own `program.rs` delta retains ENOSPC and its errno test. The imported kernel queue preserves that classification.
The mutation exclusions remain byte-identical to the new base.

The canonical check for `dfc38b478b018d63f7bb0851397b2a00a0dd4561` fails for that head and its first merge.
Log: `gates/botster-core-rc-paste-base-merge-check-dfc38b47-20261011-141941.log` under `~/botster-sessions`.
SHA-256: `d6df59b8a8259201569820c0e626b09b5dae32aa8dfdc99b7266760aec60f5e3`.
Ancestry and conflict checks pass. The path overlap and pending-file conditions fail; both commands exit one.
Sol grants no automatic Q7 carry. This round supplies an explicit integration delta review.
RC must retain the check for the corrected head in the final body.

### Pending correction and evidence limits

Against the new base, the final testkit set removes the same seven own identifiers and adds none.
The real-pending set adds the same two own identifiers and removes none.
Sol independently verifies both set formulas against the previous review and the imported base.
`conf::dp_5_session_ended_is_a_reason_with_written_bytes_after_a_partial_paste` stays testkit-pending.
The temporary eighth removal and third real-pending addition at dfc38b47 are reversed.
Its new comment records the testkit seed-10 failure, real seed-0 failure, undiagnosed cause, and RC owner.

Failed gate: `gates/botster-core-stage1-rc-paste-dfc38b47-pool-20261011-141844-22243.log` under `~/botster-sessions`.
SHA-256: `ef4c20b219e1807342e8a2b17c5e0554f4bf76af1a9d71f52a1e7a06f3053287`.
The exact head and base match. At seed 10, step 15, the route reports `route_closed session_ended` with signal 15.
The transcript expects `input_refused session_ended` with `written_bytes` for operation 1.
The strict run reports 223 passes, one failure, and 466 ignored identifiers.
Formatting, clippy, and taint pass. Lists fails; all six later steps do not run.
The step total is 81.4 seconds. The job and wrapper exit one after 154 seconds on msa1, with zero seconds queued.
Sol accepts no mutation or real-tier result from this failed gate.

Sol reads both relevant transcripts at contracts commit `ddce507e2c08e740f899e97dcfec566f3d2275b8` (.39).
The fence exceptions name partial-paste steps 10 and 16, and invalid-UTF8 step 9.
The invalid-UTF8 read at raw step 13 follows `await_quiet`, with no PTY fence.
Its revised pending comment names the R-60 exception, R-56 work, and P3 owner. It claims no diagnosed cause.

The local check on `5ab38e0b560fb490aab62cc99d31589d8225dfce` retains 395 testkit passes and 192 worker passes.
Its strict check reports 223 passes and one failure while the partial-paste identifier remains pending.
Log SHA-256: `a0adabe8d700d1bbc487142e35e9f4980f46c382fa8b1f08d78d7a9cc1e3f106`.
The real partial-paste record retains one failure at seed 0, step 15.
Log SHA-256: `593bbf5cfb0f03f3f1e8505c34adabd59afde83e7a30ece53717985069dcd01b`.
These filtered records contain no complete build output or independent clean verification.
RC reports a clean at 14:13:56 and only this worktree's builds thereafter, in target `~/botster-sessions/targets/rc`.
That reported provenance satisfies B13's single-worktree condition. Sol does not claim raw verification of the clean.
RC's dfc38b47 strict PASS used fewer seeds and has no retained log. The failed gate replaces that acceptance claim.
RC reports 223 strict passes on this corrected head with `BOTSTER_SEEDS=0-31`. This remains an owner report.

### Imported finding and remaining acceptance condition

Sol reads the package review through `3ce01a075f4a1656e75e445aab546b0bd413e75e`, including the complete #284 correction section.
Package evidence commits: `b47ca04e3ebe6e5f3c49edc25632a2783e3a636a` and `a2a78ed0764e945fce97a70fb489f8d4581fe4dd`.
RC-PASTE-DELTA-1 identifies imported assertions through `queued()`, `unread()`, and `waits_for_next_step()`.
Those assertions originate in merged #283. This PR adds no flagged assertion against the new base.
The lead explicitly transfers the finding to F-283-1, owner PC, with reviewers 0179 and Sol.
The lead's ruling closes that finding for #284. The transfer does not claim a code fix.
Sol's previous #283 review missed this finding. Sol preserves the follow-up obligation.
Open package findings for #284: zero under that ruling.

RC-PASTE-FINAL-1 — MED, open acceptance condition: the corrected head lacks a completed final gate and final evidence body.
Closure requires the completed exact-head gate, its full applicable evidence, the corrected canonical check, and the matching package verdict.
The final body must preserve the failed gate, seed limits, pending correction, and B13 provenance.
The source delta adds no integration finding. This receipt permits no claim that final acceptance is complete.
Sol runs no build, test, gate, mutation, measurement, or executable base-check script. Never poll.

VERDICT: NOT CLEAN (1 open) at 006b61cc454e0dce4e0063852e6f0701fef081bc

## Round 3 — completed final evidence, 2026-10-11

Head: `006b61cc454e0dce4e0063852e6f0701fef081bc`.
Base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Tree: `5b13e06270096dfad3b73ea8cf87136c26da0261`.
RC-PASTE-FINAL-1 closes. Open integration findings: zero, including LOW.
The complete source delta review in Round 2 remains applicable to this unchanged head.

### Completed gate

Log: `~/botster-sessions/gates/botster-core-stage1-rc-paste-006b61cc-pool-20261011-142613-53641.log`.
SHA-256: `b46195cec277e3d329876cd8955d86d3fafe71b2f17cffe7c45c8e7be5e6d36e`.
Sol reads the exact header, applicable conformance and proof records, suite summaries, complete mutation section, and final exits.
The head and base match this verdict. The Linux job runs on MS-A1, kernel `6.12.111+deb13-amd64`, with four CPUs.
All ten steps PASS in 327.3 seconds. The job exits zero after 336 seconds, with zero seconds queued.
The wrapper exits zero after 337 seconds.
Default: 1779 PASS, 467 skipped, 12.809 seconds. Slow: 578 PASS, 2022 skipped, 43.778 seconds.
The strict and normal testkit summaries each report 223 PASS, zero failure, and 467 ignored identifiers.
The strict check runs 356 of 374 pending transcripts; 18 real-only identifiers do not run, and three are held.
Lists verifies 690 ledger identifiers, 444 pending identifiers, two deferred identifiers, and 18 withdrawn identifiers.
Minimum: testkit 62/69, real-passing 55/68, real-accepted 56/69. All four named real proofs pass.
All seven own testkit removals have normal PASS records in this exact gate.
Both R-82 tests, the ENOSPC write test, and the public failed-write completion test pass.
The partial-paste session-ended identifier remains pending. No all-seed PASS is claimed for that identifier.
Twenty-six identifiers remain real-pending. Seven ignored trials pass, including both own real-pending additions.
Those trial passes do not remove their pending status or erase the retained Mac failures and fence limits.

Mutation stage 1: 58 total, 55 caught, three unviable, zero missed, zero timeout, 91.1 seconds.
Sol independently compares all 55 raw caught identities with all 55 citation identities.
Both sets contain 55 unique identities and are equal. Every cited test has a normal PASS in this gate.
Both constant replacements of `PROGRAM_SIDE_GONE.contains(&errno)` are caught.
The three unviable replacements return `Some(Default::default())` from `decide`, `paste_begin`, and `paste_chunk`.
Stage 2 does not run because stage 1 misses no mutant. The complete log contains no FLAKY token.
Fuzz passes because the diff changes no crate with a decoder harness. Sol claims no fuzz execution or Mac execution from this gate.

### Canonical check, final body, and matching package verdict

Corrected check: `~/botster-sessions/gates/botster-core-rc-paste-base-merge-check-006b61cc-20261011-142236.log`.
SHA-256: `b13948afa3892199a907bfc8d04f252f2ccc208d44ed4cfd1a3d8fb084b19528`.
Sol reads all 38 lines. Ancestry and conflict checks pass; overlap, own-diff, and pending-file conditions fail. The command exits one.
The check confirms that both explicit delta reviews are required. Round 2 supplies the integration source review; no automatic carry applies.

GitHub MCP fails. Sol reads the published PR through the authorized GitHub CLI fallback.
The live head and base match this verdict.
Final body SHA-256: `12669601b1ccd0f1280a64b9ab7615c67ae4d9a4f856cf91199f4d62cece8832`.
Sol reads the complete body delta and verifies that it contains the corrected canonical output exactly.
The final section retains the dfc38b47 failed gate, local seed limit, pending correction, F-283-1 transfer, and reported B13 provenance.
The corrected local strict result remains an owner report. The final pool gate independently supplies exact-head execution evidence.
The final gate paragraph states the correct head, base, duration, and mutation totals.
The historical .36 table remains distinct from the final .39 result. No new pending removal is claimed.

Package CLEAN: `019f5418f9fa153b781c15bd3ed513bd3fc3ed86`, final-slot section in `verdicts/rc-real-controls.md`.
Sol reads that complete section. The exact head, base, source scope, canonical failure, final gate, body, and transfer agree.
All package findings close, including LOW. F-283-1 remains PC's separate work with reviewers 0179 and Sol.
The transfer does not claim a fix for the imported assertions. RC retains the separate undiagnosed pending failure.

Both reviews and the final evidence now cover this exact head and base.
The lead controls the merge. The merge must preserve this gated tree.
This verdict accepts no later head, pin, pending removal, or different merge tree.
Sol runs no build, test, gate, mutation, measurement, or executable base-check script. Never poll.

VERDICT: CLEAN (0 open) at 006b61cc454e0dce4e0063852e6f0701fef081bc

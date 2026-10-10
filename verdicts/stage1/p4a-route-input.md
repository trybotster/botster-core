# PR #227 — Route input and read allowance

## Round 1 — 2026-10-10

Reviewed head: `a65c8f9e347c752307c661f70871614c7ef72dee`.
PR and gate base: `fc1e8160abfd8248b155383ef1d766c6377f8a11`.
Tier: HIGH, shared testkit changes and new pending-real entries.
Scope: all 11 own-diff files, worker admission, the testkit binding, conformance accounting, and supplied evidence.
Authority: approved plan 23y, Core DP-5, IN-4, DP-8, OU-2b, and codec TS-5b.
Contracts pin: `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
Test authority: BUILD.md at `56bd0a5`.

### R1-1 / F99 — MEDIUM — OPEN — Consumed read allowance can remain exhausted

Locations: `worker/route.rs:533-544,584-607`; testkit `worker.rs:1420-1423,1522-1526`.

The binding subtracts each positive read from its remaining allowance.
The worker caches the last absolute allowance that it sent.
An empty bytes/text frame or an ignored extension consumes transport bytes without retaining input.
The desired allowance then equals the cached value, so the worker sends no replacement allowance.
Immediate refusals can follow the same path while the outgoing queue still has room.
The binding can exhaust its allowance and stop permanently while the input queue is empty.
Client reads of refusals do not necessarily change the computed allowance or restore it.

Account for consumed allowance, or send a replacement allowance after a read that consumes bytes.
Add a worker-and-binding proof that reads more than the initial allowance on this path.
Check continued progress, exact delivery, and bounded retention.
I independently confirmed the package reviewer's source finding.

### R1-2 / F100 — MEDIUM — OPEN — Delayed refusals can exceed the outgoing queue bound

Locations: `worker/input.rs:234-244,428-435,472-479`; `worker/route.rs:652-673`.

The decoder checks refusal room before it admits a valid route input.
Admission reserves no space for that input's later refusal.
A blocked host write can hold many one-byte route inputs at the admission point.
After `PayloadExited` and the host's write count, `try_start` refuses all queued route inputs in one loop.
Each `refuse` appends a frame without checking or reserving outgoing queue space.
Those frames can exceed `route_queue_bytes`, although input admission respected its input bound.
An active route write's delayed failure uses the same unchecked append path.

Keep delayed outcomes under the outgoing queue bound without losing an input's outcome.
Add a proof with queued valid input, a blocked PTY, payload exit, and a client that does not read.
Check the bound and delivery after the client resumes.
I independently confirmed the package reviewer's source finding.

### R1-3 / F101 — MEDIUM — OPEN — A delayed refusal can follow `route_closed`

Locations: `worker/route.rs:453-475,652-673`; `worker/input.rs:461-479`.

`close_healthy` marks the route closing and appends `route_closed`.
It does not cancel an outstanding route PTY write.
A later failed PTY count calls `refuse` while that route still exists.
`refuse` accepts a closing route and appends `input_refused` after `route_closed`.
OU-2b requires `route_closed` to be the last frame.

Preserve the last-frame rule for outstanding input outcomes during a healthy close.
Add a proof with a route write in flight, `Detach`, a failed PTY count, and a gated outgoing stream.
Check the decoded frame order and the single close report.
I independently confirmed the package reviewer's source finding.

### R1-4 / F102 — MEDIUM — OPEN — Unsupported input loses its supplied op

Location: `worker/route.rs:631-649`.

The decoded branch for kinds other than bytes/text passes `None` to `refuse`.
`PasteChunk` and `FileChunk` both carry an op, but their refusals omit it.
The new unsupported-kind test expects that omission.
Codec TS-5b requires a refusal to echo the input's op when supplied.

Retain each decoded input's supplied op when forming its refusal.
Correct the observable test expectations.
I independently checked the source against TS-5b after the package reviewer reported F102.

### R1-5 / F103 — LOW — OPEN — New proofs inspect private state and miss their claimed first-allowance boundary

Locations: `worker/route.rs:701-708`; `worker/tests/routes.rs:1369-1380,1407-1408,1560-1562`.

The new `route_held` test accessor exposes private queue accounting and decoder retention.
The refusal-pressure, closing-route, and baseline-refusal tests assert those private values.
BUILD Testing shape rule 3 forbids tests of helper internals.
Replace these assertions with machine actions, transport behavior, and decoded frames.
Remove the private-state accessor.
Keep the clause coverage; name retained coverage if a duplicate test is removed.

The binding's `a_route_is_read_only_within_its_allowance` also claims to prove no read before the first allowance.
Its `bind_route_of` helper already sends `usize::MAX`, before the test sends zero.
The test proves pause and resume after an allowance, but does not prove the stated first-allowance boundary.
Add that boundary through a bind that has received no allowance.
I found these test-shape issues independently. The package reviewer confirms F103.

### R1-6 — MEDIUM — OPEN — The worker raises the configured input queue bound

Location: `worker/route.rs:122-129`.

`input_bound` uses `max(route_input_queue_bytes, max_frame_bytes + 4)`.
CoreLimits validation requires only a nonzero `route_input_queue_bytes`.
The host copies that limit into AppliedRouteLimits unchanged (`admit.rs:1002`).
A valid small input queue with a larger frame limit therefore permits retained input above its configured bound.
For example, a one-byte queue does not stop after one queued byte when the frame limit is 1 MiB.
DP-5 requires the worker to stop reading when the configured route input queue is full.
The pinned contract and rulings do not grant this raised threshold.

Keep retained route input within the configured bound.
If completing a larger frame requires different contract behavior, request a contract amendment.
Add a proof with a valid input queue smaller than the frame bound.
I sent this source finding to P3 and the package reviewer and requested any binding authority for the floor.
P3 confirms that no contract text or ruling permits the floor and has asked the steward about the gap.
The package reviewer independently confirms that a valid small queue can admit a larger retained payload.

### Carried findings from #217

F86 / integration R1-1 closes: `on_route_ended` preserves an existing healthy close reason.
The proof checks one transport close and one report after the peer ends during the healthy close.
F87 / integration R1-4 closes: complete Unsupported and Invalid frames advance the client revision and report their route.
The proof checks the sequence of public observations and later valid input.
F90 / integration R1-5 closes its original ownership and adoption loss:
queued and active input retain the route and op; failures report written bytes; adoption removes only old host requests.
R1-3 and R1-4 above identify additional outcome defects.
F88 / integration R1-2 remains open through R1-1, R1-2, and R1-6 above.
Closing reads now discard client bytes before retention.

### Accounting and supplied evidence

Core pending changes from 477 to 465: twelve removals, no additions.
Pending-real changes from 87 to 99: the same twelve additions, no removals.
I found each removed id's testkit PASS in the exact-head gate.
The earlier `e9f1702c` gate records each id's real-tier failure with `UNSUPPORTED_CONTROL`.
The seven route-control failures name `attach_stream`, `client_close`, or `route_gate`.
The five program-control failures name `pty_output`, `pty_input`, or `pty_blocked`.
The new real-pending entries state those causes and owners.
The packaged worker still ignores route actions. This PR makes no real-route pass claim.
The source uses the existing codec and admission point; it adds no terminal implementation.

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p4a-route-input-a65c8f9e-pool-20261010-140656-95834.log`.

The header names the reviewed head and current base. The command is plain `cargo xtask ci`.
All ten steps pass. Default: 1,519 passed. Slow: 384 passed.
Testkit conformance: 205 passed, zero failed. Minimum: 54/69.
Real minimum: 29/68; accepted: 29/69. Pending-real: 99, zero passes.
Mutation stage 1: 94 mutants, 92 caught, two unviable, zero missed or timeouts.
Stage 2 does not run because stage 1 misses none. The gate exits zero after 493 seconds.
These results do not cover the source defects above.

I read `evidence/p3-pr3/mutant-proofs.log` and its script.
The supplied four operator changes and partial-write allowance reversal fail named public-action or frame assertions.
The restored source passes the three selected tests. I ran none of those jobs.

The supplied base check passes all four conditions for `8ac2e36f` to this head.
I independently compared both own binary diffs outside the pending file: 72,987 bytes, identical SHA256
`4ae9ff35cc121aa3a7a97f1190531bade2b54744e2053e5bf3a8c3148e577754`.
The automatic merge tree and reviewed head tree both equal `fad990c1d427a52485dbaec9e1032f8b9bc82ead`.
This is a base merge of the preceding implementation. It does not remove the findings.

Package NOT CLEAN `ed3aaf046db617a5f85c66bf3afbfcf6d4cbc5f3` is received and read:
`verdicts/p3-worker.md`, round 150. Findings F99–F103 agree with this review.
The package reviewer confirms R1-6 and will add it to the same round.
I changed no product code and ran no build, test, gate, mutation job, or reversal job.

VERDICT: NOT CLEAN (6 open: 5 MEDIUM, 1 LOW).

### Round 1 addendum — R-53 settles the effective input bound

After publication, the steward issued R-53 in contracts commit
`3a6dd2d3ee7a0e1f0e1830bccbae407ee6bb342f`, `docs/steward-rulings.md`.
I read the exact ruling from the contracts repository.
R-53 permits `max(route_input_queue_bytes, max_frame_bytes + 4)` so an admitted frame can complete.
It rejects a validation restriction that would fail at the existing defaults.
Its memory budget uses the effective allowance.

This authority settles R1-6's interpretation in favor of the existing source formula.
No input-bound redesign or amendment is required under R-53.
R1-6 remains open for the ruling's required citation and proof in the replacement PR.
The proof sends an exact-max-frame input at default limits and observes processing instead of a permanent stop.
P3 also proposes a legal small-queue proof. Review that proof at the replacement head.

I read the package round 150 addendum at `48fd6fdd174c39781ccf6e616b697dd6ae68d14f`.
The package reviewer independently confirms the original small-queue path and accepts the new R-53 authority.
This addendum changes no review round or reviewed head.

VERDICT: NOT CLEAN (6 open: 5 MEDIUM, 1 LOW; R1-6 now requires the R-53 citation and proof).

Package receipt: I read the final round 150 artifact at `3a7522d7879a48809a383cbd23320efbd5b2bae2`.
F104 records the same R-53 citation and proof requirement. Both reviewers agree on all six open findings.

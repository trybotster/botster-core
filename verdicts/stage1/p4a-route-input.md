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

## Round 2 — 2026-10-10

Reviewed head: `3e4c79a916673de015765bb951b165fc155db963`.
Prior reviewed head: `a65c8f9e347c752307c661f70871614c7ef72dee`.
PR and full gate base: `3677b37e502497d5e63a91dfad92e5671bd19196`.
Tier remains HIGH. Plan 23ab retains the applicable mutation, pending, and acceptance rules.
Scope: the complete five-file correction delta, affected input and route paths, base import, PR body, and supplied evidence.
The new PTY test file brings the complete own PR diff to twelve files.
The code still pins contracts-v0.1.26. The separate code pin move remains pending.

### Original finding closures

- R1-1 / F99 is CLOSED. Each RouteRead lowers the cached allowance by the bytes read.
  A new allowance follows when its computed value differs from the driver's remaining allowance.
  The whole-frame worker proof exceeds the first allowance and fails when this correction is removed.
  The combined binding proof delivers all 600 refused frames. The PR correctly states that seeded partial reads conceal this reversal there.
- R1-2 / F100 is CLOSED. Each queued or active route input reserves one refusal bound.
  Refusal admission and the PTY output budget subtract those reservations.
  The deferred-end proof checks every refusal in order and the encoded early-refusal total against the queue bound.
  Removal of the reservation fails that proof. R2-1 below concerns progress after reservations are released.
- R1-3 / F101 is CLOSED. `refuse` adds no frame to a closing route.
  The detach-then-write-failure proof requires `route_closed` last and no refusal. Removal of the check fails that proof.
- R1-4 / F102 is CLOSED. PasteChunk, FileChunk, and structured Input refusals retain the input's op.
  The public frame proof checks ops 2, 3, and 5 and keeps the route open. Removal of the op fails that proof.
- R1-5 / F103 is CLOSED. `route_held` is removed; its tests now check public actions, frames, and observations.
  The binding proof starts without an allowance. It checks no read before the first allowance, the first bounded read, exhaustion, and renewal.
- R1-6 / F104 is CLOSED. The route documentation and PR cite R-53.
  The default-limits proof sends one frame of exactly `max_frame_bytes`, with its prefix, and checks the complete PTY write.
  The legal-small-queue proof checks completion and the public remaining allowance. Removal of the permitted floor fails both proofs.

### R2-1 — MEDIUM — OPEN — Successful writes do not resume buffered route frames

Package finding: F105. I independently confirmed the source path.
Locations: `worker/route.rs:539`, `decode_route`; `worker/input.rs:462`, `finish_active`.

A permitted RouteRead can contain a finite batch of complete, valid one-byte inputs.
Their reserved refusal room can fill the outgoing bound, so `decode_route` stops with complete frames in `StreamReader`.
Successful PTY completions free reservations through `finish_active`, `send_budget`, and `read_allowances`.
None of those calls resumes `decode_route`.
The admitted inputs can finish while the complete buffered suffix never reaches the admission point.

Choose a small outgoing queue and a finite batch that fits the input allowance.
Let the client send no more bytes. Let the live payload accept every write and produce no output.
Successful route input sends no response, so no RouteWritten event resumes decoding.
The route has free space but retains the remaining complete frames indefinitely. This breaks DP-5 progress.
The deferred-end proof generates refusals and RouteWritten events, which conceal this successful-write path.

Resume buffered decoding when input completion frees reserved room, without requiring another transport read or outgoing write.
Add a public proof with a finite fully read batch, successful PTY counts, and a quiet live payload.
Require the whole batch to reach the PTY in order while retaining both queue bounds.
This is a source counterexample. I ran no reproduction job.

### R2-2 — MEDIUM — OPEN — Outgoing pressure delays the receipt revision

Package finding: F106. I independently confirmed the source path and IN-4/IN-10 text.
Locations: `worker/route.rs:546`, `decode_route`; `worker/input.rs:351`, `start_checks`.

A permitted RouteRead can give the worker many complete valid frames at once.
`decode_route` checks refusal room before `reader.next_frame` and `client_received`.
When the refusal queue fills, the already received complete suffix does not advance `input_rev{client}`.
A view-only route creates refusals but no client PTY transaction.
A host write can start with a guard equal to the last emitted prefix revision.
`start_checks` accepts that guard although the complete suffix reached the worker before the host write starts.

IN-4 requires advancement on receipt. IN-10 counts refused input and requires stale refusal for earlier received client input.
Those clauses permit the host's reported view to lag; they do not permit this worker guard result.
The immediate-refusal pressure proof counts decoded frames, which does not establish the required receipt boundary.

Preserve complete-frame receipt while buffering later application and refusal delivery within their bounds.
Add a public proof with a finite complete batch within its allowance and an unread refusal queue.
After the read, start a host write guarded by the partial reported client revision.
Require `NotWritten(Stale)` with exact zero bytes and preserve ordered exception delivery.
This is a source counterexample. I ran no reproduction job.

### Accounting, base import, and evidence

All Core pending id lines equal the prior head. Pending-real equals its prior bytes.
The same twelve ids move from Core pending to pending-real. The packaged worker still ignores route actions.
The PR claims no real-route completion.
The automatic `fd88f274` plus `3677b37e` tree and actual head tree both equal `0509f86c990277a60a49efcbfee73a326628e93d`.
The imported #218 code and exclusions match that base merge. There is no additional own exclusion change.
`git diff --check` passes.

Gate: `~/botster-sessions/gates/botster-core-stage1-p4a-route-input-3e4c79a9-pool-20261010-143403-84107.log`.
All ten jobs pass under plain `cargo xtask ci`.
Default: 1535 passing tests, 13.937 seconds. Slow: 384 passing tests, 124.742 seconds.
Both conformance reports show 205 passed and zero failed.
Strict coverage: 374/395, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 54/69, real 29/68, and real-accepted 29/69.
Pending-real remains 99, with zero newly passing ids.
Stage 1 has 112 mutants: 110 caught, two unviable, zero missed, and zero timeout. Stage 2 does not run.
The gate exits zero after 822 seconds, including 340 queued seconds. These results do not cover the new findings.

I read `evidence/p3-pr3/pr3-round150-reverts.log` and its complete script.
Five source reversals fail their named worker proofs. The restored source passes all six selected proofs.
The combined binding proof passes each reversal, as the PR records.
I read package verdict `44eb5f85b9990d62ae12cedb607c3b8d6a79fe3e`, `verdicts/p3-worker.md`, Round 151.
It is NOT CLEAN on this exact head. Both reviewers close F99-F104 and open F105/F106 as MEDIUM.
The retained reversal log contains result summaries and failed test names, not full assertion output.
I claim no independent confirmation of a precise assertion site in those reversal runs.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.
The tree comparison uses Git source objects only.

VERDICT: NOT CLEAN (2 open: R2-1 MEDIUM and R2-2 MEDIUM).

## Source and evidence inspection — 2026-10-10 — Package verdict pending

Reviewed head: `d1d8742a36de00753d32e302aeac99fb72bf1a0a`.
Prior reviewed head: `3e4c79a916673de015765bb951b165fc155db963`.
PR and full gate base: `89d57cc7d07957b88003722897cf43e540d19bcc`.
Scope: the complete three-file correction, affected input and route paths, both base merges, PR body, and supplied evidence.
The tier remains HIGH. This inspection completes no verdict round before the same-head package verdict arrives.

### Checked corrections

R2-1 / F105 closes its reported source path.
Route input completion now calls `settle_routes`, which applies waiting frames before it sends the budget and allowances.
Partial PTY counts, route queue writes, reads, output, refusals, bind, and close also call this shared point.
Calls inside an active pass request another pass instead of starting a recursive route pass.
Each pass consumes the unsettled flag. The unmodified loop stops after the waiting work and its generated changes settle.
The quiet-batch proof checks all 50 bytes at the PTY in order without another client read or outgoing frame.

R2-2 / F106 closes its reported source path.
`receive_route` counts every complete input frame before the application pass checks refusal room.
Unsupported and Invalid frames use this receipt path too. Ignored extensions advance no input revision.
`apply_route` preserves each route's received order and reserves refusal room before admitting input.
The guard proof reports revision 50 on the read, rejects an earlier reported revision as Stale with exact zero, and accepts revision 50.
It checks that later refusal delivery adds no second receipt report.

The waiting-frame byte counter was removed after the earlier gate found ineffective mutations in that counter.
The allowance remains zero while refusal room blocks application.
`settle_routes` applies waiting frames before computing a nonzero allowance, so no waiting frame is omitted from a renewed allowance.
The final source retains the R-53 floor, reservation, last-frame, op, and spent-allowance corrections.

### R3-1 — LOW — OPEN — Private queue assertions restore the R1-5 / F103 defect

Locations: `worker/route.rs:798-804`; `worker/tests/routes.rs:1404-1406,1631,1639-1640,1847,1867,1894`.

The new cfg(test) `route_pressure` exposes `queued - baseline` and `received.len()`.
Both pressure proofs and both new correction proofs assert these private values.
The pressure proofs also remove their previous public encoded-refusal bound checks.
BUILD Testing shape rule 3 forbids tests of helper internals.
The Round 2 closure of R1-5 / F103 no longer holds for this new delta.

Remove the accessor. Use actions, decoded frames, and transport progress to check the same conditions.
Keep the immediate and deferred refusal bounds, ordered delivery, and the F105/F106 scenario coverage.
I sent this finding to the implementer and package reviewer before completing this inspection.

### R3-2 — MEDIUM — OPEN — Delayed early input loses its NotReady decision

Package finding: F107. I independently confirmed the source path and read R-51 at contracts-v0.1.27.
Locations: `worker/route.rs:576-589,599-620,708-712`.

An early finite batch can fit the input allowance and arrive before the gated live completion.
`receive_route` counts every complete frame but stores no readiness decision with those valid frames.
`apply_route` emits NotReady refusals until refusal pressure stops application, leaving the received suffix waiting.
After the baseline completes and a refusal write frees room, `settle_routes` applies that suffix.
`on_client_frame` checks the current `baseline`, now zero, and sends the early bytes to the PTY.

DP-5 and TS-5 require NotReady for input that the worker receives before live.
R-51 permits either order only when the input remains in the transport until the worker receives it.
That exception does not apply here: the complete frames have already advanced the receipt revision before live.

Preserve the receipt-time NotReady decision while refusal delivery waits.
Add a public proof with an early complete batch, refusal pressure, later live completion, and queue draining.
Require NotReady for every early op and no PTY write from that batch.
This is a source counterexample. I ran no reproduction job.
The package reviewer reported F107 and independently confirms this MEDIUM finding.

### Source merges, accounting, and evidence

The automatic `1d96f4c0` plus `6c2bccda` tree equals the actual `374f1f2a` tree: `0862083e4cc0efd83714a77860997e1f3574ec70`.
The automatic `9a95a0ae` plus `89d57cc7` tree equals the actual reviewed tree: `eb09ce10f8b690e198a880be3bbdd731220a033c`.
All three correction files equal their pre-final-merge bytes.
The imports retain accepted #229 mutation selection and #231's contracts-v0.1.27 pin.
Core pending ID lines equal the prior head. Real-pending, real-only, held, deferred, and minimum files equal their prior bytes.
No additional ID leaves pending. No new real-route pass is claimed. `git diff --check` passes.
Remote v1, PR head, and PR base match the recorded commits.

Gate: `~/botster-sessions/gates/botster-core-stage1-p4a-route-input-d1d8742a-pool-20261010-154315-72557.log`.
Its header names the exact head and base. The command is plain `cargo xtask ci` through the pool.
All ten jobs pass. Default: 1540 passing tests, 14.747 seconds. Slow: 384 passing tests, 135.143 seconds.
Both conformance reports show 205 passed and zero failed.
Strict coverage remains 374/395, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 54/69, real 29/68, and real-accepted 29/69.
Real-pending remains 99, with zero newly passing IDs.
Stage 1 has 118 mutants: 116 caught, two unviable, zero missed, and zero timeout, in 237.8 seconds.
Stage 2 does not run because stage 1 misses none. The gate exits zero after 527 seconds, including two queued seconds.
The passing gate does not remove the test-shape finding.

I read `evidence/p3-pr3/pr3-round151-reverts.log` and its complete script. I did not run the script.
The F105 reversal fails the quiet-batch proof. The F106 reversal fails the guard proof and both pressure proofs.
The shared application-pass reversal fails 21 route proofs. The restored source passes all 143 selected proofs.
The log retains result summaries and failed test names, not full assertion output.
These receipts support the source correction but do not establish compliance of the private assertions with BUILD rule 3.

The same-head package verdict is pending. One LOW and one MEDIUM source finding remain open.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.
The tree comparisons use Git source objects only.

## Round 3 — 2026-10-10

Reviewed head: `d1d8742a36de00753d32e302aeac99fb72bf1a0a`.
Reviewed base: `89d57cc7d07957b88003722897cf43e540d19bcc`.
The complete source delta, two source merges, accounting, gate, and reversal receipts are recorded in the inspection above.

I read package Round 152 at verdict commit `fa9d05caa236b029a92cbd5bdbd5902875651527`, `verdicts/p3-worker.md`.
It reports NOT CLEAN on this exact head and base. Both reviewers agree on the closures and remaining findings.

R2-1 / F105 and R2-2 / F106 are CLOSED in their reported scopes.
R1-1 / F99, R1-2 / F100, R1-3 / F101, R1-4 / F102, and R1-6 / F104 retain their closures.
R1-5 / F103 is REOPENED for the new private queue assertions, recorded as R3-1 LOW above.
R3-2 / F107 is MEDIUM and OPEN: complete early input loses its receipt-time NotReady decision while application waits.
The passing gate does not close either finding.

This is the third NOT CLEAN round for PR #227.
The package reviewer will report BLOCKED for the required lead decision before a fourth round.
The integration reviewer sends this verdict and commit to the lead under the assigned messaging rule.
No fourth review round starts without that decision.

I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: NOT CLEAN (2 open: R3-1 LOW and R3-2 MEDIUM).

## Source and evidence inspection — 2026-10-10 — Package verdict pending

Reviewed head: `27e1dae68638421a9820ee893ccf6dbcaec1c67c`.
Prior reviewed head: `d1d8742a36de00753d32e302aeac99fb72bf1a0a`.
PR and gate base: `0a564513e50930ce25f2e4a175f45cffbef7119d`.
Plan 23ad authorizes the full fourth review. Plan 23ae retains that authorization.
I inspected all twelve own paths, the complete receipt correction, affected interfaces, six source merges, accounting, and supplied evidence.
This inspection completes no round before the same-head package verdict arrives.

### Checked corrections

R3-1 / F103 closes in source. The private `route_pressure` accessor and its assertions are removed.
The pressure proofs use actions and decoded frames to check encoded refusal bytes, ordered ops, and renewed allowance.
The healthy-close proof checks the encoded suffix before `route_closed` under the 1000-byte bound.
It also requires fewer than 50 refusals, so some received frames must remain waiting before close.
The quiet-batch and guard proofs retain their public checks without private queue assertions.

R3-2 / F107 closes in source. `Route::decide` decides each complete frame at receipt.
`Received` holds decided input, refusal, or bad-frame values. `apply_route` waits for resources without deciding route readiness again.
The early-batch proof receives 50 complete frames before Live, within the granted allowance, while the baseline remains unread.
It drains Live and all 50 NotReady refusals. It requires no PTY write from that batch.
Healthy close clears waiting frames and prevents later application.

F105 and F106 retain their closures. Completion still calls the shared, nonrecursive settling pass.
Receipt reports every complete input revision before refusal pressure stops application.
The quiet-batch proof checks all 50 bytes without another read or route write.
The guard proof rejects revision 49 with Stale and exact zero after receipt reports revision 50.
Later refusal delivery produces no second receipt revision.
F99, F100, F101, F102, and F104 retain their source corrections and public checks.

### R4-1 / F113 — MEDIUM — OPEN — Route admission permits input during payload stop

Locations: `worker/input.rs:253-260,329-330,351-362`; `worker.rs:486-494,854-879`.

`Pending::Route` calls `payload_takes_input`, which checks only Live and absence of exit.
Stop sets `termed` and the grace deadline. Kill sets `killed`. The payload can remain Live until exit arrives.
A writable route can receive bytes or text in that interval and start a PTY write.
The same route path can start previously queued input when its admission turn arrives during that interval.
Host admission rejects `termed`, `killed`, or an active grace deadline as NotWritten(Stopping).
DP-5 at contracts-v0.1.27 maps that outcome to `input_refused{session_ended}`.
The new route admission path does not apply this stop check.

Use a shared stop predicate for host and route admission. Preserve the existing behavior for model replies under EV-8.
Add public proofs for route bytes and text after Stop and Kill, before exit arrives.
Also check queued route input when its admission turn arrives after Stop.
Require SessionEnded refusal and no PTY write for each refused input.
Retain a failed source-reversal receipt for the correction.

The package reviewer reported F113. I independently confirmed its source path and the pinned DP-5 mapping.
The implementer accepts this defect and plans a correction. Neither reviewer has a contrary lead ruling.
This is a source counterexample. I ran no reproduction job.

### Source merges and accounting

Five automatic source merge trees equal their actual trees:

- `492e1a7e`: `53a884f761832f180a2b8118d6fd6ec0c635517d`.
- `f87b0538`: `6b1c25f583120595540d01979f367eb08296007c`.
- `171d70ed`: `bfdaf1e98c9fb87a635bef34ad0e62b14a9ffa87`.
- `3e9cd9f0`: `19383c3f4aa41b9b1b101760d59c8acf9f4ff3e3`.
- Final `27e1dae6`: `d6bfc014c62aa029fc653fa6aa7d8aba57e15302`.

Merge `2e475bf9` conflicts only in `core-real-pending.txt`.
Its automatic tree is `277f6b6299c73b9018b5628918d40eae37545b0c`; its actual tree is `04eddb44fa00896dcb48fcd3bcb363e21464d9ba`.
I read the complete resolution. It retains both introductions and all IDs from both parents.
The route group has 29 IDs; the program group has 38. No other path differs from the automatic tree.
The final merge retains the pre-merge route, input, and route-test bytes.
All six imports retain the reviewed #235, #232, #236, #222, #237, and #234 changes.

The same twelve IDs move from Core pending to real-pending. No further own ID leaves pending.
The real tier still lacks the route data plane. The packaged worker still ignores its route actions.
The PR claims no real-route completion. Pins, exclusions, held IDs, and other coverage lists have no own change.
`git diff --check` passes. The PR head and base match the recorded commits.
The updated PR body contains its route-input Prior art note. I read the two cited vault notes.
The note identifies the old receipt and rejection caps as lessons, not contract bounds.

### Supplied evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p4a-route-input-27e1dae6-pool-20261010-183554-22780.log`.
The header names the exact head and base. All ten jobs pass in 540.4 seconds.
Default: 1568 passing tests, 20.055 seconds. Slow: 396 passing tests, 159.119 seconds.
Both conformance reports show 205 passed and zero failed. Strict coverage is 374/395.
There are 21 real-only transcripts and two held transcripts. The reports list 70 IDs without transcripts, two deferred IDs, and 18 withdrawn IDs.
Minimum counts are testkit 54/69, real 31/68, and real-accepted 31/69.
Real-pending is 93, with zero newly passing IDs. The twelve-ID own transfer explains the change from the base's 81.
Stage 1 has 118 mutants: 115 caught, three unviable, zero missed, and zero timeout, in 235.2 seconds.
Stage 2 does not run because stage 1 misses none.
The job takes 651 seconds, including 94 queued seconds. The wrapper takes 652 seconds.
The passing gate does not cover the new stop finding.

I read `evidence/p3-pr3/pr3-round152-reverts.log` and its complete script. I did not run the script.
The F105 reversal fails the quiet-batch proof. The F106 reversal fails four named proofs.
The F107 reversal fails the early-batch proof. The reservation reversal fails both named pressure-related proofs.
Removal of the application pass fails 22 route proofs. The restored source passes all 144 selected proofs.
These receipts contain result summaries and failed test names, not full assertion output or a final-head header.
The retained correction source matches the reported reversals. I claim no final-head reversal run from those receipts.

I read `evidence/p3-pr3/pr3-5a41-mutants-reverts.log`.
The healthy-close proof fails both previously missed pressure mutants: it observes 1027 bytes and 1841 bytes against the 1000-byte bound.
Deleting `received.clear()` remains green: the closing check still prevents application. The source contains the required discard.
I require no private queue assertion to make that deletion fail.

One MEDIUM source finding remains open. The same-head package verdict is pending.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.
The tree comparisons use Git source objects only.

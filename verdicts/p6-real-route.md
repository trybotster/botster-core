# P6 real route review — PR #238

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/238

Exact head: `4c675cbc0fcd86cf4dffccc825f7a96a27ee0513`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
Risk tier: HIGH. This change affects the shared testkit crate and the real conformance tier.

### Review scope

The reviewer read the complete six-file delta, the route implementation, its tests, both harness modes, and the retained gate.
The reviewer also read the pinned contracts driver, Deadline, R-47, and BUILD.md rules for prior art and test shape.
The reviewer read the shared Bounded reader and its deadline handling.
The reviewer inspected the two reported merge conflicts through Git objects.
The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

### Open findings

#### RR-R1-1 — HIGH — A flush can retry without progress before the read checks its deadline

Location: `crates/botster-core-testkit/src/real/route.rs:62-84`, called by `write` and `read` at lines 156 and 162.

`after_write` returns true for `Interrupted`, but it removes no byte from `unsent`.
It also returns true for `Ok(0)`, which removes no byte.
`flush_into` then calls the same writer again while the same bytes remain.
The loop has no deadline check or progress bound.
Repeated results of either kind therefore keep this call in a busy loop.
`read` calls `flush` before it reads `deadline.remaining()` or reaches the bounded poll.
The poll deadline cannot bound this earlier loop.
This is a source counterexample; the reviewer did not execute a hanging case or claim the retained gate encountered one.

The current decision test asserts that an interrupted write continues.
The partial-write test covers successful progress and `WouldBlock`, but it does not cover this retry path.
The passing mutation run therefore does not close this finding.
The existing in-memory route has a similar retry shape; copying that shape does not bound real I/O.

Required change: Stop this flush when a write makes no progress, and preserve the pending bytes for a later call.
Positive partial writes can continue because each one removes bytes.
Add controlled-writer proofs for `Interrupted` and zero-byte writes through the complete flush operation.
The proofs must show termination and preservation of the pending bytes without an unbounded retry.
Keep the existing ordering and partial-write assertions.

#### RR-R1-2 — LOW — The close test asserts a private field

Location: `crates/botster-core-testkit/src/real/route.rs:395`.

`a_client_close_ends_the_stream_for_both_ends` asserts `route.unsent.is_empty()` after a late write.
This assertion observes a private field of the helper.
BUILD.md, Testing Shape rule 3, forbids tests of helper internals.
The user's review rule also requires public behavior assertions.

Required change: Remove this assertion or replace it with an observable behavior assertion.
Retain the existing peer EOF assertion, late write, and subsequent route EOF assertion.
Assertions on the outputs of the pure decision functions are not this finding.

#### RR-R1-3 — LOW — The prior-art note omits the required old Core comparison

Location: PR body, `Prior art`.
This matches integration finding SI-1.

The note names the current in-memory route, `UnixStream::pair`, rustix poll, and the shared Bounded reader.
It records reuse of the flag parser and shared reader.
It does not compare the old Core at the binding pin `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The pair rules and BUILD.md require that comparison and the reuse or rejection reason.
The reviewer confirmed that the old pin contains `crates/botster-core-test-support/src/route_observer.rs` and the old route protocol.

Required change: Add the old-code comparison at the pinned SHA.
State which relevant pieces were reused or rejected and why.
Record the reason for each custom piece beside the maintained library or existing helper that was considered.
Do not port old tests or check out an old tree.

#### RR-R1-4 — LOW — The body describes a historical send failure as current behavior

Location: PR body, `What stays blocked`.
This matches integration finding SI-2.

The body says `RealEdges::link_send_descriptor` always returns `DescriptorSendError::Failed`.
At this exact head, `crates/botster-core/src/real.rs:586-617` calls `fd_pass::send_with_fd` and handles success.
PR #234 added that path and is already in the base.
The worker driver still does not receive or bind route descriptors; its real route actions remain pending work.
The earlier EOF evidence remains useful, but it describes the earlier head.

Required change: Mark the earlier failure as historical evidence.
Describe the implemented Core send and the remaining worker receive and route transport work at this head.

### Reviewed behavior and limits

`attach_stream` creates one Unix stream pair.
Core receives the worker end through its public attach API.
The harness keeps only the client end and keeps no copy of the worker end.
An attach refusal drops the local client route and follows the attach API's error result.
Both plain and tapped harnesses use this same implementation.
This change adds no process spawn, group owner, waitpid call, or production reaper.
The existing process ownership remains unchanged.

The client stream stays nonblocking.
After the first empty read, a later read waits through one marked rustix poll using the step deadline's remaining time.
The pinned conformance driver's first idle route read causes one handoff pump.
The poll returns Empty on a timeout or interruption, then the driver applies its own step limit.
The source has no sleep and adds no read retry loop.
RR-R1-1 concerns the separate flush loop before this bounded read.

`client_close` shuts down both directions and drops the client end.
`route_gate` stops reads and uses the real kernel's backpressure, within R-47 A's accepted scope.
The harness does not set socket buffer sizes or implement `route_accept` or `route_fill` in this change.
Those controls and the worker's real route transport remain outside this PR.
The real route reports `Eof { ended: None }`; its documentation states that it has no cause-recording wrapper yet.

The final delta removes only three IDs from `core-real-pending.txt` and updates the route group's count from 29 to 26.
The gate passes all three IDs on the real tier:

- `conf::a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input`.
- `conf::ev_8_attach_requires_a_positive_query_deadline`.
- `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`.

No transcript, contract assertion, contract pin, or mutation exclusion changes in the final delta.
The final list conflict retains the new base IDs and removes these three IDs.
The earlier import conflict retains `LoseReason` and removes `no_route`.

### Retained evidence

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-4c675cbc-pool-20261010-211130-72458.log`.

The header records the full head and base above.
All ten jobs pass. Wrapper exit: 0 after 2763 seconds, with 43 seconds queued and 2720 seconds running.
Default tier: 1585 passed. Slow tier: 423 passed, including all twelve new route tests and the three removed pending IDs.
Conformance: 205 passed; strict tier ran 374 of 395 pending IDs, with 21 real-only IDs not run and 2 held.
Minimum counts: testkit-passing 54/69, real-passing 35/68, real-accepted 35/69.
Pending-real: 80; zero newly passing IDs remain on that list.

Stage 1 lists 34 mutants and misses all 34 because the route module compiles only with the slow feature.
Its duration is 120.2 seconds.
The unmutated slow baseline passes in 120.5 seconds.
Stage 2 tests those 34 mutants: 29 caught, 0 missed, 0 timeout, and 5 unviable, in 2281.7 seconds.
The log prints each completed stage-2 outcome.
The five unviable replacements require default values that the replaced result types do not provide.
There is no new exclusion in this PR.

The reviewer also read the retained earlier mutation evidence.
At `9df0d0fb`, the blocking-read mutant times out after 600 seconds; that gate is stopped with exit 130.
At `b036e1ed`, the gate fails with one stage-2 survivor: deletion of `!` in the flush condition.
Commit `b036e1ed` replaces the blocking read with the nonblocking stream and bounded poll.
Commit `02b8b9de` adds the controlled partial-write proof through `flush_into`.
The exact-head log catches both flush condition mutants.
These corrections close those earlier mutation defects, but they do not prove termination after repeated no-progress writes.

Open findings: 4. Integration independently confirms RR-R1-1 and the private-field finding.

VERDICT: NOT CLEAN

## Round 2 — Corrections and the #239 base merge

Exact head: `fd627c55cb888fde6225d9a8a964ce65183ee8a8`.
Base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
Pre-merge gate head: `89eeb2db28793674ef4c2739d17b790ffddf5151`.
Pre-merge gate base: `08ee8f2c6e343ff47966f12fcb80fe7cd35d0a83`.
Risk tier: HIGH.

### Finding closure

RR-R1-1 HIGH is closed.
`after_write` stops the flush on `Ok(0)`, `WouldBlock`, and `Interrupted`, and keeps the pending bytes.
Other errors clear the bytes and stop the flush.
Each positive write removes at least one byte from a finite buffer.
Each loop pass therefore makes progress or returns before another write.
The read can reach its deadline check and marked poll.

The complete flush proof covers an interruption, partial writes, a full stream, later ordered delivery, and a zero-byte write.
The controlled writer rejects a call beyond the allowed count.
The interruption and zero-byte cases each permit one call.
A regression therefore fails promptly instead of leaving the proof in an unbounded retry.
The reviewer read both correction deltas, at `96116a19251b3b86912fe6a88748fd5260c79fd6` and `eec41d21bd7752363ff264dd26ead4905ea0ae64`.
The corrected proof passes in the retained gate.
The gate catches the `after_write` whole-body replacements, its no-progress guards, and both flush-condition replacements without a timeout.

RR-R1-2 LOW is closed.
The private `route.unsent` assertion is removed.
The proof retains peer EOF, the late write, and subsequent route EOF.
The decision proofs observe their outputs and the controlled writer's I/O effects.

RR-R1-3 LOW is closed.
The published body compares old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It records the retained stream-pair and poll forms, the rejected blocking-timeout and interrupted-retry forms, and the scripted pressure adapter.
It explains reuse of rustix, the current flag parser, and the shared Bounded reader.
The reviewer verified the old worker_process.rs references through Git objects.
These are comparisons of concepts. The correction copies no old code or tests.

RR-R1-4 LOW is closed.
The body describes #234's implemented descriptor send and the remaining worker-side work.
It labels the earlier Failed and HandoffFailed evidence as historical.
The body also assigns the 80-ID report to gated head `89eeb2db` and the 77-ID list to final head `fd627c55`.

### Base merge review

The reviewer read the lead's pool-saving rule in `handoffs/core-lead.md`, at the ~22:50 entry.
The rule requires one pre-review gate, review of base merges, and one final gate after both CLEAN verdicts.
The reviewer applies that rule to this merge. The earlier gate is evidence for the corrected source, not a gate on the final head.

The reviewer read the supplied merge-check log:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-fd627c55-pool-20261010-225925-73587.log`.

The log records the exact final head and base above.
It exits 1 after 9 seconds because the merge requires delta reviews.
Both ancestry checks pass. Condition 1 reports no merge conflict.
Condition 2 fails on two shared paths: `conformance/core-real-pending.txt` and `crates/botster-core-testkit/src/real.rs`.
Condition 3 fails on changed diff index lines. Condition 4 passes because this PR leaves `core-pending.txt` unchanged.
This record does not grant automatic acceptance. The following delta review covers its shared paths.

The reviewer read the final six-path own delta, both shared-path changes, merge parents, and imported path summary through Git objects.
The final own delta has 539 insertions and 19 deletions.
The route file equals the gated source exactly: blob `b2629607c8b79460c4c8dac729daded557efe795`.
The attach_stream implementation still creates one pair and passes the worker end to the supplied CoreApi.
The merge adds #239's start controls and RefusalLayer to the tapped harness.
The reviewer read that layer's attach method: it returns a scripted refusal with the original transport, or forwards the transport to its inner CoreApi.
The harness keeps no duplicate endpoint. A refusal drops the returned transport and the local client route.
The plain harness remains plain. Both harnesses retain the same route implementation.

The merged real-pending list retains both disjoint sets of removals.
#239 removes `a5_3_only_sync_column_codes_scripted`, `a5_3_sync_refusal_has_no_op_event_slot_or_state`, and `ad_7_payload_launches_after_durable_identity`.
P6 removes only the three IDs named in Round 1.
The final list has 77 IDs; the new base has 80.
No transcript or contract assertion changes in this PR's own delta.
The final own delta changes no pin, mutation exclusion, native configuration, process owner, or production reaper.
The four corrections remain intact. The delta review finds no new package defect.

### Retained correction gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-89eeb2db-pool-20261010-221028-80389.log`.
The header records the pre-merge head and base above.
All ten jobs pass. The job exits 0 after 2842 seconds, with 171 seconds queued and 2671 seconds running.
The wrapper exits 0 after 2843 seconds.
Default tier: 1596 passed in 18.599 seconds.
Slow tier: 423 passed in 137.861 seconds, including all twelve route proofs and all three P6 list removals.
Conformance: 206 passed. Strict tier ran 373/394 pending IDs, with 21 real-only IDs not run and 2 held.
Minimum counts: testkit-passing 55/69, real-passing 35/68, real-accepted 36/69.
Pending-real: 80, with zero newly passing IDs on that gated head.
These counts belong to the gated head, before the #239 merge.

Stage 1: 30 mutants, all missed, zero timeout or unviable, in 130.1 seconds.
The unmutated slow baseline passes in 134.5 seconds.
Stage 2: 30 mutants, 25 caught, 0 missed, 0 timeout, and 5 unviable, in 2138.0 seconds.
The reviewer read all thirty stage-2 outcomes. The five unviable replacements require unavailable default values.
No new exclusion accounts for the smaller mutant count; the corrected match structure changes the generated mutants.

All four Round 1 findings are closed. Open findings: 0.
This is a CLEAN review of the exact final head. Its final gate remains the implementer's next step after both CLEAN verdicts.
The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN

## Round 3 — Route control declarations

Exact head: `5921340d4f90d94ab7864b740820c0d5fb88ad03`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Parent: `8474e449f19b848f0bfe6e6174316f027b815c4e`.
Risk tier: HIGH. The change affects the shared testkit and the runner's capability check.

### Source review

The reviewer read the complete one-file delta from the parent, the affected method and test, and the published body.
The reviewer also read the pinned runner's preflight check and route dispatch interfaces.
The delta has 8 additions and 2 deletions in `crates/botster-core-testkit/src/real.rs`.

The runner checks every `requires.controls` entry through the driver's `has_control` before setup or step 0.
The Core driver forwards that check to the harness.
Previously, the real harness omitted `client_close` and `route_gate`, although RealRoute implements both.
The correction adds the existing `route::CONTROLS` to the tapped harness's declared controls.
The plain harness still declares no controls, as its existing mode requires.
The route subject checks the attached route's controls and dispatches route controls to that route.
The correction adds no call to the harness's separate `control` method for these operations.
The declaration therefore permits preflight; the existing route implementation still performs each operation.

The reviewer read P3's retained evidence:
`~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/real-pending-minimum-ids-238-240-prc-macos.log`.
`conf::ou_5_peer_fault_closes_only_that_route` stops with UnsupportedControl(client_close).
`conf::ou_7_exit_after_tail_queued_then_close` stops with UnsupportedControl(route_gate).
This evidence comes from P3's local combined tree, not the exact head of this round.
The pinned preflight source confirms the cause: both checks occur before any step can use the route.

The existing public capability proof now checks each route control on both harness modes.
It expects true for the tapped harness and false for the plain harness.
The proof calls `CoreHarness::has_control`; it adds no assertion on a private field.
The existing route proofs still cover client close, gate release, and control dispatch.
The correction reuses the route's declared control list and the existing capability proof.
It adds no custom I/O mechanism.

The delta changes no route I/O, deadline, timer, flush loop, process owner, cleanup, or production reaper.
It changes no transcript, contract assertion, contract pin, mutation exclusion, or pending list.
Both named IDs remain pending-real. The reviewer does not claim this declaration makes their complete transcripts pass.
The four Round 1 corrections remain intact. There is no new package finding.

### Accepted base import and retained evidence

The parent merges the Round 2 head with accepted #241.
That import changes only `crates/botster-core-testkit/src/pending.rs` and `tests/suite/mod.rs`.
The reviewer read its merge parents, imported path summary, and supplied merge-check output:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-8474e449-pool-20261010-231238-48492.log`.
All ancestry and four merge conditions pass.
The own diff remains byte-identical outside line-set files: 28219 bytes, SHA-256 `2ab66880f1c71fdf665c191fe2e8f9fae6f1c9f45e8677839500c5d99845ab0b`.
The check exits 0. The job takes 7 seconds; the wrapper takes 8 seconds.

The reviewer also read the completed parent gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-8474e449-pool-20261010-231252-49132.log`.
All ten jobs pass. Job and wrapper exit 0 after 2499 seconds, with zero queued seconds.
Default: 1604 passed in 11.186 seconds. Slow: 427 passed in 106.861 seconds.
Conformance: 206 passed. Strict pending: 373/394, with 21 real-only IDs not run and 2 held.
Minimum counts: testkit 55/69, real-passing 36/68, real-accepted 37/69.
Pending-real: 77, with zero newly passing IDs.
Stage 1 misses all 30 slow-only mutants in 104.2 seconds.
The slow baseline passes in 103.6 seconds.
Stage 2: 30 total, 25 caught, 0 missed, 0 timeout, and 5 unviable, in 2097.5 seconds.
These results belong to the parent. They do not prove execution of the new capability test assertions.

The reviewer read the header of the implementer's new gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-5921340d-pool-20261010-235507-54316.log`.
It names the exact head and base of this round. That gate is still running; its completed result remains required.
The lead's ~02:50 entry orders this delta review and one new gate.
This verdict completes the source review. It does not claim the new gate has passed.

Open findings: 0. All prior findings remain closed.
The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN

## Round 4 — Completed gate and accepted #242 base merge

Exact head: `af5d51386c1d7dec6d6633f4d3a5dfa9503c2e10`.
Base: `910c0a9adcbec718e52abaea1c568065d2241dd0`.
Reviewed and gated parent: `5921340d4f90d94ab7864b740820c0d5fb88ad03`.
Risk tier: HIGH.

The reviewer read the completed gate for the Round 3 head:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-5921340d-pool-20261010-235507-54316.log`.
Its header names the full Round 3 head and base.
All ten jobs pass. Job and wrapper exit 0 after 2659 seconds, with 1 second queued and 2658 seconds running.
Default: 1604 passed in 11.942 seconds. Slow: 427 passed in 104.636 seconds.
The capability proof passes in both the slow step and the mutation baseline.
Conformance: 206 passed. Strict pending: 373/394, with 21 real-only IDs not run and 2 held.
Minimum counts: testkit 55/69, real-passing 36/68, real-accepted 37/69.
Pending-real: 77, with zero newly passing IDs.
Stage 1 misses all 34 slow-only mutants in 112.7 seconds.
The slow baseline passes in 109.7 seconds.
Stage 2: 34 total, 29 caught, 0 missed, 0 timeout, and 5 unviable, in 2237.5 seconds.
The reviewer read all 34 stage-2 outcomes.
Both whole-body capability replacements and both boolean-operator replacements are caught.
This completed gate supplies the evidence that remained pending in Round 3.

The exact head above merges that gated parent with accepted #242.
The reviewer read the merge parents, path summary, published body, and supplied merge-check output:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-af5d5138-pool-20261011-003944-36495.log`.
Both ancestry checks and all four conditions pass. The check reports no conflict and no shared own path.
The own diff remains byte-identical outside line-set files: 29504 bytes, SHA-256 `4a7fcc9e381585c2b9c5fb1b29ce43d73de6109b9d2b14065efc4253ad28c44e`.
The job and wrapper exit 0 after 137 seconds, with 1 second queued and 136 seconds running.
The base adds only `crates/botster-core-host/src/tests/driver/observations.rs`, an accepted test path outside this PR's own delta.
The own six-path delta remains unchanged. The real.rs blob is `0add8992333b792206f6694423650b05dc38aa0f` at both heads.
The route code, capability correction, contract assertions, pending list, deadlines, and ownership remain unchanged.
The passing check requires no additional source delta round.

The gate belongs to parent `5921340d`; the final merged head has the passing base check, not a separate full gate.
The published body applies the lead's pool-saving rule to that disjoint base import.
GitHub reports PR #238 as MERGED at the exact source head above.
All prior findings remain closed. Open findings: 0.
The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN

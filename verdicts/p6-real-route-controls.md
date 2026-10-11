# P6 package review — #287: real route controls and holds

## Round 1 — 2026-10-11

Exact head: `722b415a5e7900fcbc5ea48c8244fcafe01ba4c3`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `43006ac98b01d4b08c0866e45f09488e19d1194b`.
PR: https://github.com/trybotster/botster-core/pull/287.
Risk: HIGH. The own delta changes ten testkit files, with 1509 additions and 80 deletions.
The current GitHub base has advanced. This round reviews the source delta from the base above, which is also the gate reference.
It accepts no later import, pending-list change, or gate.

I read the complete PR body through the authorized CLI fallback after the GitHub MCP response truncated it.
I read the own source delta, new modules, changed tests, relevant source contexts, and retained rulings.
Rulings: `~/botster-sessions/shared/core-stage1/evidence/p6-route-controls/rulings-23bf-23bg.md`.
SHA256: `ff842c450bdfe972f9d488412676c7b9c6efacee412a156b29414fcb0a5173c7`.
I compare the controls with the pinned Contracts .36 rows and the later explicit 23bf/23bg rulings.
The own delta changes no production source, transcript, timeout, pending list, mutation exclusion, or Contracts pin.
It adds the rustix net feature. Cargo.lock remains unchanged.

### RC-R1-1 — HIGH — new loops can retry without progress

Locations: `crates/botster-core-testkit/src/edge_tap.rs:398` and `crates/botster-core-testkit/src/real/route.rs:1051`.
Tap::sort_unnamed retries Interrupted with no state change, deadline, or attempt bound.
The new fill test helper also loops without a bound. Its Ok(0) and Interrupted paths make no progress.
A sequence of these results can hold the test or control call indefinitely.
These paths violate the P6 brief's no-busy-loop rule. A passing outer gate does not bound the operations themselves.
Stop the operation on no progress, or use a finite, derived bound.
Retain the hello bytes, link order, accepted byte count, and backpressure assertions.
Add an edge or writer fixture that fails immediately if the operation retries without progress.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745074_c9bf63`.

### RC-R1-2 — MEDIUM — new socket reads have no deadline

Locations: `crates/botster-core-testkit/src/edge_tap/tests.rs:1510` and `crates/botster-core-testkit/src/real/route.rs:1028`.
Both tests read a blocking UnixStream directly. Neither read has a deadline.
Each test expects the preceding operation to close the peer.
If that close regresses or a copy remains open, the read can wait indefinitely instead of failing at a derived bound.
The existing client-close tests use Bounded::to_eof with TestDeadline::cleanup for this reason.
Use a shared bounded read or a nonblocking attempt for these new reads.
Retain the first test's EOF assertion and the second test's EOF-or-reset assertion.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745074_c9bf63`.

### RC-R1-3 — LOW — assertions inspect private diagnostic state

Locations: `crates/botster-core-testkit/src/real/route.rs:974` and `crates/botster-core-testkit/src/real/route.rs:999`.
The new tests assert the Debug form of private Fill, Requests, and Holders values.
These assertions require internal type names and field layout. They prove no control clause or public behavior.
BUILD.md Testing rule 3 forbids tests of helper internals. The user's review instruction requires public behavior assertions.
Remove these diagnostic-layout assertions. Retain the public control-result assertions in both tests.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745181_35b033`.

### RC-R1-4 — MEDIUM — required Mac proof is absent

The 23bf ruling approves read + unread as route_stream_written only with a test on each OS.
The own delta also introduces the macOS lsof holder count and socket identity path.
The Linux gate does not compile the darwin module or establish the Mac socket facts.
P6 confirms that no Mac evidence has completed for this branch in receipt `msg_plugin-w_1791745049_9db0a0`.
P6 reports that a focused Mac job is queued while the Mac node is below its disk floor.
Supply completed focused Mac evidence for the new socket identity, holder count, written count, and holds.
The evidence must name the exact source head and tree. A later head needs a reviewed source delta.
The future un-pend evidence does not replace this source acceptance condition.
I sent this condition directly to P6 in receipt `msg_plugin-w_1791745181_35b033`.

### Source assessment outside the findings

The route retains only the client end. Core takes the worker end; the harness creates no duplicate of it.
The route reads both buffer sizes at the ruled points and obtains F from the applied attach limits.
route_fill sends the existing alphabet pattern through the same probe Output request and acknowledgement as pty_output.
The request acknowledgement does not wait for the payload's output write to complete.
Requests retains the existing identity check, condition-variable waits, and shared cleanup deadlines.
Programs still shuts down its sockets and joins its reader threads at drop.
The new Mac command uses run_to_completion with Deadline::cleanup. That helper owns and reaps its command group.
The own delta adds no production-child reap, raw child wait, sleep, group signal, or independent process guard.

The accept drain reads once per route read/write call or route_accept control, with a 64 KiB chunk cap.
It keeps drained bytes in order. Its allowance decreases only by bytes read.
gate(false) clears the allowance; gate(true) retains it. The public tests cover both cases.
drop_transport closes without shutdown or SO_LINGER. client_close retains shutdown(Both).
The new tests permit the OS EOF/reset result without requiring one path.
The written count adds client reads and FIONREAD. Both close controls preserve their pre-close count.
The Linux public test covers ordinary reads, accept drains, backpressure, and both close controls.
The Mac counterpart remains an open acceptance condition above.

The hand-over hold returns the endpoint with Blocked and suppresses write interest only on the held link.
Release restores the last interest, respects an overlapping Launch hold, and signals the wake.
The public edge tests cover endpoint return, other-link progress, quiet edges, duplicate hold refusal, and release.
The start hold reads the hello to name the instance and keeps the accepted link from Core.
Release restores the link with its bytes in order; an armed-only release disarms the hold.
The harness refuses a hold after Created, a duplicate hold, and a release with no hold.
The body states the worker clock condition per transcript. I read the five named pinned transcript contexts.
No pending removal or proof of the unreached AD-1/AD-7 gaps is accepted here.

The holder count reads socket identity before Core takes the end.
Linux counts matching descriptors through /proc; Mac uses the bounded lsof command and matches its socket address field.
The body states the limit for processes that this user cannot read.
The tap counts the bytes passed to close_route_stream by socket identity.
The body states the sans-IO argument and the limit: requested bytes are an upper bound; zero is exact.
The existing slow:fd-handoff proof remains the real byte proof. This change does not replace it.

The own delta retains the earlier public pty_input assertions and changes the served-control assertion for route_fill.
It adds assertions for public bytes, close results, control errors, descriptor counts, and edge calls.
The private Debug assertions above are the exception.
The fixed-buffer source check records the three sources that it must read and rejects the three named setter forms.
I do not treat this text check as a general syntax-aware ban on all possible buffer-setting forms.
The existing syntax-aware process check and its red-on-revert tests remain outside this delta.

### Prior art

I read the named old sources through Git at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old conformance wrapper uses the public engine commands and supplies no matching route control or start hold.
The old output-backpressure test uses one paced in-process write slot. This PR instead uses actual socket backpressure.
The old read_available helper measures a pipe with unsafe FIONREAD. This PR uses rustix on the Unix socket.
The old plugin worker enumerates only its own inherited descriptors for close. It supplies no cross-process socket count.
The body rejects direct code reuse for each item and records the different boundary or unsafe-code constraint.
I find no missing relevant reuse item in the named note.

### Retained Linux evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-722b415a-pool-20261011-112154-51848.log`.
SHA256: `88a781fdc8ff046d69d28090726485e402281c71ed879665c79fe72519554f5c`.
I read the exact-head/base headers, stage records, summaries, changed test outcomes, and pending-real results.
All ten stages pass in 1756.9 seconds. The job exits zero after 1764 seconds, with zero queued and 1764 running.
The wrapper exits zero after 1765 seconds. Default passes 1753 tests, with 474 skipped, in 13.134 seconds.
Slow passes 570 tests, with 2036 skipped, in 43.047 seconds.
The new Linux socket identity, holder count, written count, control, and hold tests pass.
The gate reports real minimum 49 / 68 passing and 50 / 69 accepted.
It reports 12 passing pending-real ids out of 36. This head removes none.

A static parser compares the complete raw mutation identities with the named catch records.
Stage 1 has 165 unique identities: 65 caught, 96 missed, four unviable, and zero timeout, in 348.6 seconds.
Stage 2's 96 unique raw identities exactly match stage 1's misses.
The slow baseline passes in 54.2 seconds. Stage 2 catches 76, reports 20 unviable, and misses none, in 1212.6 seconds.
Each stage's raw caught identities exactly match its unique named catch records.
The final total is 141 caught, 24 unviable, zero missed, and zero timeout. Unviable results provide no catch evidence.
The fuzz step reports no changed crate with a decoder harness.
I do not accept the body-only historical probe and flake claims as new independent evidence.
The body reports the earlier capture flake as a separate structural-fix obligation; this round does not close it.

Four findings remain open. P6 owns the source fixes and required Mac evidence.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11: source corrections

Exact head: `cc4a1a40c4c4a3cc3120cb55154f25b5f3e72af0`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `6918cd32accaa8b7a75fbafb725980068140b39b`.
Reviewed parent: `722b415a5e7900fcbc5ea48c8244fcafe01ba4c3`.
Correction commits: `572f119567e934afe0425844908fa8a8621f340f` and this exact head.
I read the complete four-file correction delta and the quiet/accept source contexts.

RC-R1-1 closes in the source. sort_unnamed stops the current read on Interrupted or WouldBlock.
It retains the unnamed link for the next call. The new test supplies two Interrupted results before a hello.
The test checks one read per call, non-quiet edges while the hello is incomplete, and the final public link-held result.
The fill helper ends on Ok(0), WouldBlock, or Interrupted. Each continuing write takes at least one byte.
No receiver drains the socket during that helper call, so the finite kernel buffers bound the writes that make progress.

RC-R1-2 closes in the source. Both identified reads now use nonblocking sockets.
A retained peer returns WouldBlock and fails the assertion immediately.
The EOF and EOF-or-reset assertions remain unchanged. Neither read introduces a wait.

RC-R1-3 closes. The three unused Debug implementations and all identified layout assertions are removed.
The public control-result assertions remain unchanged.

RC-R1-4 remains open. P6 reports that a Mac job on the reviewed parent failed during a resolver outage.
That result supplies no Mac proof. P6 reports new Mac and Linux jobs on this head, without completed results.
I accept no result from an unfinished job. The Round 1 Linux gate remains historical parent-head evidence.
The PR body and final evidence for this head remain pending.

### RC-R2-1 — HIGH — the modified quiet operation can still retry without progress

Location: `crates/botster-core-testkit/src/edge_tap.rs:321`.
P6 identifies this path in receipt `msg_plugin-w_1791745365_9f8218` and asks whether to fix it in this PR.
I read the path. Tap::quiet retries Interrupted in its per-link loop with no state change, deadline, or attempt bound.
A sequence of Interrupted results can prevent quiet from returning.
The retry predates this PR, but this PR changes Tap::quiet and uses it with the new holds.
The same P6 wait rule applies to this modified operation. The sort_unnamed correction does not fix this second retry path.
Fix this path in this PR. End the call on Interrupted with a non-quiet result and retry on a later call, or use another finite approach.
An Interrupted read must not prove that the edges are quiet. Retain pending bytes and signal the wake for further progress.
Add a finite fake-edge check that fails if quiet retries repeated Interrupted results within one call.
I send this finding directly to P6 in receipt `msg_plugin-w_1791745450_3271b9`.

Two findings remain open: RC-R1-4 and RC-R2-1.
This source review accepts no future import, pending removal, unfinished job, or final body.
I changed no product code and ran no execution job.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-11: quiet retry correction

Exact head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
Parent: `cc4a1a40c4c4a3cc3120cb55154f25b5f3e72af0`.
I read the complete two-file correction delta. The head has the exact parent above.

RC-R2-1 closes in the source. Tap::quiet now makes one read attempt per link, with no inner retry loop.
WouldBlock proceeds to the next link without changing the quiet result.
Interrupted leaves the link and buffered bytes intact, makes the result non-quiet, and causes the wake to signal.
The next call can retry. Successful reads and terminal errors retain their earlier behavior.
The new finite fake-edge test supplies two Interrupted results, then one byte.
Each interrupted call returns non-quiet, consumes one attempt, and signals one wake.
The final public link_recv result gives the byte. The test does not wait for a timer to detect a retry defect.

RC-R1-1, RC-R1-2, and RC-R1-3 remain closed. No source finding remains.
RC-R1-4 remains open: the required Mac evidence has not arrived.
P6 reports that the prior-head jobs stopped and new Linux and Mac jobs run on this head.
This review accepts no completed result from those unfinished jobs.
The final body and exact-head evidence remain pending. The Round 1 Linux gate retains only its parent-head scope.

One finding remains open: RC-R1-4. This round accepts no later import or pending removal.
I changed no product code and ran no execution job.

VERDICT: NOT CLEAN

## Round 4 — 2026-10-11: source and required Mac evidence

Exact head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
This round retains the source review from Rounds 1–3. The implementation head and tree have not changed.
All source findings remain closed. This round closes RC-R1-4 with completed Mac evidence.

Mac log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-2645bfc1-pool-20261011-120525-8436.log`.
SHA256: `2d8b8666b146c621d2115cf7092d4898b3cd26a1d50fb71537af14f6bac05395`.
The retained header and command output identify the exact head above on Darwin 27.0.0.
Git identifies that head's tree above. The log does not print a tree identity.
The gate header sets BOTSTER_CI_BASE_REF to `926dfb62f3f7168990b2a48741946932d84b2fbb`.
That gate setting does not change this branch's source base.

I read the retained results and decoded the complete command from the log.
The focused command selects the real::route::, real::holders::, edge_tap::, and stream_holders:: library test groups with the slow feature.
The library summary reports 63 passed, zero failed, zero ignored, and 409 filtered out, in 0.19 seconds.
The source and filters include the required written-count, socket-holder, socket-identity, and hold tests.
The summary does not print individual library test names. I do not claim that the log prints those names.
The Darwin holder path and written-count tests therefore have completed evidence on this exact head.
RC-R1-4 closes. No review finding remains open.

The later transcript probes have mixed results. Their selected real tests report these outcomes across three runs:

| Selected id | Named real outcomes |
| --- | --- |
| ou_1, ou_11, ou_12, ou_3, ou_5 | Three passes each |
| a2_1_attach_state_rules | Three passes; the testkit tier reports Unsupported for the start hold |
| ou_9 | Two passes and one failure |
| ou_10 | One pass and two failures |
| dp_3 | Three INCONCLUSIVE step_timeout results |
| a2_1_no_deadline_code_exists | Three INCONCLUSIVE step_timeout results; the testkit tier reports Unsupported |
| dp_1, dp_2 | Named-proof guards fail; the retained output supplies no named transcript pass |

The ou_9 failure occurs at seed 7, step 21. The ou_10 failures occur at seeds 1 and 4, step 25.
The dp_3 timeout occurs at seed 0, step 17. The a2_1_no_deadline_code_exists timeout occurs at seed 0, step 23.
The script temporarily removes twelve ids from both pending lists in the job's copy and restores those files afterward.
The missing dp_1 and dp_2 proof entries cause aggregate commands to fail, including commands with passing selected tests.
The script uses set -u without set -e or pipefail. It does not preserve each command's failure in its final exit status.
The job exits zero after 263 seconds: one queued and 262 running. The wrapper also exits zero after 263 seconds.
That exit status does not prove a green full Mac gate or green aggregate transcript commands.
The focused library summary supplies the evidence that closes RC-R1-4.
The retained filtered ou_3 output contains no RouteStalled report. I do not claim to have read an unfiltered ou_3 log.

This CLEAN verdict covers the reviewed source and required Mac library evidence at the exact head above.
The completed final Linux gate and updated final PR body remain due. This round does not approve PR completion.
The Round 1 Linux gate retains its historical parent-head scope. This round accepts no later import or pending removal.
I sent the Mac evidence limits and body update request directly to P6 in receipt `msg_plugin-w_1791745963_04e973`.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: CLEAN

## Round 5 — 2026-10-11: quiet contract assertion after the missed mutant

Exact head: `27cb2cc48562334612d2d227aff8d8bb88a30d52`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `e02638de17d5bf1031e2a6807a07eb20ef906125`.
Parent: `2645bfc1c216cea54cd61577cae25e834e66133c`.
I read the complete one-file delta and the quiet, holds_anything, link_held_for_start, and test contexts.
The delta adds one assertion and one comment. It changes no product or harness implementation.

The interrupted-read test now calls quiet again after the tap has read one byte, before the driver's link_recv call.
The new assertion requires a non-quiet result while that byte waits for the driver on a link without a start hold.
The existing link_recv assertion still requires the exact byte z. The earlier bounded-attempt and wake assertions remain unchanged.
The new assertion checks the observable quiet contract. It does not inspect a private hold collection or counter.
It introduces no wait, sleep, loop, process, signal, or reap operation.

### Completed parent-head Linux evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-2645bfc1-pool-20261011-120525-8394.log`.
SHA256: `2922694baf31867c1e6086859c9f6882f94ed7febb58efd0c762c336fcb18f74`.
I read its exact-head/base headers, summaries, mutation outcomes, and relevant conformance results.
The gate runs at the exact parent above with BOTSTER_CI_BASE_REF set to `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Eight stages pass. Mutants fails. Fuzz does not run because an earlier stage failed.
The gate takes 1646.0 seconds. The job and wrapper exit one after 1654 seconds: one queued and 1653 running.
Default passes 1755 tests, with 474 skipped, in 11.016 seconds.
Slow passes 570 tests, with 2038 skipped, in 40.584 seconds.
The real minimum remains 49 / 68 passing and 50 / 69 accepted. Eleven of 36 pending-real ids pass.
This head removes no pending id. These trial outcomes do not authorize a later removal.

A static parser verifies all raw mutation identities and named catch records.
Stage 1 has 164 unique identities: 66 caught, 94 missed, four unviable, and zero timeout, in 318.3 seconds.
Stage 2's 94 unique identities exactly match stage 1's missed identities.
The slow baseline passes in 52.3 seconds. Stage 2 catches 73, misses one, and reports 20 unviable and zero timeout, in 1155.8 seconds.
Each stage's raw caught identities exactly match its unique named catch records.
The final total is 139 caught, one missed, 24 unviable, and zero timeout. Unviable results supply no catch evidence.
The single stage-2 miss is `crates/botster-core-testkit/src/edge_tap.rs:347:9: replace Tap<E>::link_held_for_start -> bool with true`.
That result is a test-coverage finding at the parent head. The failed gate supplies no complete gate approval.

At the new head, the added assertion covers the missing case in the source.
With that mutation, holds_anything ignores the buffered byte, and the next quiet call can return true after WouldBlock.
The new assertion requires false before link_recv consumes the byte. The source therefore targets the reported miss directly.
I do not claim a completed catch at this new head. P6 reports that the new full Linux gate is running.

The implementation remains byte-identical to Round 4. Its required Mac evidence retains that unchanged-implementation scope.
The extra assertion has no completed new-head Mac result in this review.
No source finding remains open. This CLEAN verdict covers the source delta and the carried required Mac implementation evidence.
The completed new-head Linux gate and updated final body remain due. This verdict does not approve PR completion.
It accepts no later import or pending removal. It does not convert the failed parent gate into a pass.
I changed no product code and ran no execution job.

VERDICT: CLEAN

## Round 6 — 2026-10-11: completed Linux gate and corrected body

Exact head: `27cb2cc48562334612d2d227aff8d8bb88a30d52`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `e02638de17d5bf1031e2a6807a07eb20ef906125`.
The source and tree remain unchanged from Round 5. That source review remains valid.

Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-27cb2cc4-pool-20261011-123345-13820.log`.
SHA256: `3d15d9aea86074d2e1f1fdde37c51e8aa5962169691a143a18efd02cad99715a`.
I read the exact-head/base headers, stage records, summaries, relevant test outcomes, mutation identities, and named catch records.
The pool uses four CPUs on MS-A1. BOTSTER_CI_BASE_REF is `926dfb62f3f7168990b2a48741946932d84b2fbb`.
That gate reference does not change the branch's source base above.
All ten stages pass in 1557.6 seconds. The job and wrapper exit zero after 1568 seconds: one queued and 1567 running.
Default passes 1755 tests, with 474 skipped, in 12.458 seconds.
Slow passes 570 tests, with 2038 skipped, in 41.723 seconds.
The interrupted-read, socket-identity, holder-count, and written-count tests pass.
Strict runs 363 of 381 pending ids, with 18 real-only not run and two held.
The real minimum remains 49 / 68 passing and 50 / 69 accepted. Eleven of 36 pending-real ids pass.
Fuzz reports no changed crate with a decoder harness. This head removes no pending id.

A static parser verifies every raw mutation identity and both stages' named catch records.
Stage 1 has 164 unique identities: 67 caught, 93 missed, four unviable, and zero timeout, in 298.0 seconds.
Stage 2's 93 unique identities exactly match stage 1's missed identities.
Its slow baseline passes in 46.0 seconds. Stage 2 catches 73, misses none, and reports 20 unviable and zero timeout, in 1091.6 seconds.
Each stage's raw caught identities exactly match its unique named catch records.
The final total is 140 caught, zero missed, zero timeout, and 24 unviable. Unviable results supply no catch evidence.
Stage 1 catches `edge_tap.rs:347:9: replace Tap<E>::link_held_for_start -> bool with true`.
Its named catcher is `edge_tap::tests::held_starts::an_interrupted_read_in_quiet_is_not_quiet_and_is_read_again_at_the_next_call`.
The new assertion therefore has completed catch evidence. The parent gate remains failed as recorded in Round 5.

Stage 2 reports one FLAKY result for `botster-core-sys::slow_process a_spawned_child_matches_its_identity_and_dies_by_the_group_kill`.
The test fails once and passes on its one mutation-profile retry. That result is not a catch.
The mutation deletes the route_stream_holders control arm in botster-core-testkit.
The holder-count test catches that mutation independently. The final catch result does not depend on the flaky test.
I read the sys, edges, and test-process manifests. Their dependency paths do not include botster-core-testkit.
The main gate log gives no first-attempt failure output. P6 later supplies a retained mutant log beside it.
I read its sys-test attempts and holder-test failure output. The sys test panics in process_guard.rs:113 during cleanup, with wait status 9.
The sys test passes its second attempt. The holder test fails both attempts at its public control-error assertion.
The supplemental output confirms the cleanup panic. This review does not prove the proposed registration-race diagnosis or a structural fix.
The body names P6 as the owner of a separate structural fix after #287, under BUILD.md Testing rule 9.
I read the complete shared flake record, its correction, and rule 9. I accept the owner assignment; the structural fix remains open.
P6 removes the proposed many-run campaign after integration and this reviewer's requests. This verdict authorizes no such campaign.
It accepts no future diagnosis method, retry, timeout change, or structural-fix implementation.

### Body and historical probe evidence

I read the complete retained body and its complete correction delta.
Retained body: `~/botster-sessions/shared/core-stage1/evidence/p6-route-controls/pr-287-body-27cb2cc4.md`.
SHA256: `47d3c7961a9986ce51931bffa9063d9a69cec87cd2fda8fb21128ccb8c96d4b4`.
The preferred GitHub tool truncates the body. The GitHub CLI supplies its complete text and current head/base identities.
At 20:07:41Z, GitHub names the exact head above and base `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Its body matches the retained body after trailing-newline normalization.
The body names the correct slow_tests paths, source fixes, failed parent gate, current green gate, and mixed Mac outcomes.
The body states that the probe scripts mask aggregate failures. It does not present their zero exits as green full gates.
The prior-art note and limits of host_bytes remain explicit.

I read the decoded command and named outcomes in the historical Linux three-run probe log on `722b415a`.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-722b415a-pool-20261011-115554-74918.log`.
SHA256: `2d0da52eff78d44496db9858cf4cb29080201393f92d0921f957038064ebace2`.
Its 40 retained named records agree with the body's Linux table. The seven listed OU candidates each have three named real passes.
The a2_1 attach test has three named real passes; its testkit test fails. Both reported timeout cases remain failed.
The dp_1 and dp_2 selections supply no named pass. The script masks failures and restores the scratch pending files.
It sets BOTSTER_ONLY per selection and does not set BOTSTER_SEEDS. This record supplies no explicit 32-seed proof.
The job exits zero after 186 seconds; the wrapper exits zero after 187 seconds. Those exits do not prove green aggregate commands.
I accept these results only as historical named probe outcomes. I approve no pending removal from this table.

I read the current plan pin's complete R-71 follow-up term and integration's complete current-authority explanation.
The corrected body labels the old .36 start-hold table as current transcript forms with open follow-up conditions.
P3 owns the later transcript PR. The a2_1 startup becomes a named value of at least 30000 ms; clock steps 7 and 35 exceed it.
Steps 8 and 36 retain StartupTimeout. The plan leaves step 27's real_wait 1200 unchanged.
The ad_7 first section, steps 3–13, is removed. Its remaining sections retain startup 500 for the worker's own exit.
Both ids remain pending here. This review closes no R-71 transcript condition.
The steward draft path supplied by integration is absent locally. The plan term and integration's explanation supply the current scope.

I read complete integration Round 3 at `31f348afd84dd0d2bc974ed3ebcb14fb414cd989` through Git.
Its source, gate, body, mutation comparison, and pending-work limits agree with this review. Integration closes RG-SI-1.

The reviewed source, required carried Mac implementation evidence, completed exact-head Linux gate, and corrected body are CLEAN.
The separate structural flake fix remains open. The current-v1 import, applicable check, delta review, and final gate remain future work.
This verdict accepts no later head, import, pending removal, contracts pin, or PR completion beyond this reviewed head and evidence.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: CLEAN

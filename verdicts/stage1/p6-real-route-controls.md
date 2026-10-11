# Core #287 integration review

## Review in progress — 2026-10-11

Current head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source comparison base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Current tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
The fetched PR body names current v1 `926dfb62f3f7168990b2a48741946932d84b2fbb` as its live base.
The body still cites its original gate on 722b415a. It does not yet contain the new evidence results.
This record is not a CLEAN verdict. The completed Linux gate and package closure remain due.

Sol reads all ten cumulative source deltas: Cargo.toml, lib.rs, stream_holders.rs, route_client.rs,
edge_tap.rs, edge_tap/tests.rs, real.rs, real/program.rs, real/route.rs, and real/holders.rs.
Sol reads the affected tap decoder, accept, receive, quiet, hold, and write-interest paths.
Sol reads the request acknowledgement path and the route count, drain, close, fill, and holder paths.
Sol reads the complete lead rulings 23bf/23bg and the relevant controls rows at .36.
The ruling record has SHA-256 ff842c450bdfe972f9d488412676c7b9c6efacee412a156b29414fcb0a5173c7.
No source finding is established in this static review.

Sol reads all three package rounds through `4261a2dab39ae959493afdfe77f76a571f4a0815`.
The package closes its four source findings. Its remaining MEDIUM finding is RC-R1-4, missing Mac proof.
Sol verifies the corrected source: Interrupted ends the current read attempt in sort_unnamed and quiet.
Quiet signals a wake after Interrupted. The tests retain the link and read its bytes at a later call.
The two test reads are nonblocking. The fill helper ends on zero progress or Interrupted.
The source removes the Debug implementations and assertions that the package identified.
The package has not yet published closure of RC-R1-4 for the new Mac record.

The route_accept drain reads at most one 64 KiB chunk per call and retains byte order.
The route_stream_written count adds read bytes to FIONREAD, including allowance drains.
Both close controls retain the count at close. The test checks partial reads and backpressure before both closes.
The tap retains no duplicate of the worker endpoint. Its host_bytes count is an edge-request upper bound; zero is exact.
The body states that limitation. The holder count and the separate fd-handoff proof cover different observations.
Linux counts readable process descriptors. Mac uses bounded lsof commands and the socket address read before hand-over.
The source does not add unsafe code. The fill uses the existing output request path and the applied frame bound.
The hold_start form holds the accepted link after its hello. The body lists the worker-clock condition per transcript.
The hand-over hold returns the endpoint with Blocked. Release restores the last write interest and signals the wake.
The overlap test keeps write interest disabled while the separate Launch hold remains.

Sol reads selected prior-art paths at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old backpressure test uses a manually completed write slot.
The old read_available uses unsafe ioctl on a pipe. The old plugin worker lists its own descriptors for closure.
The old conformance module wraps the public engine. Sol does not claim an exhaustive prior-art scan.

### Mac evidence and its limits

Log: ~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-2645bfc1-pool-20261011-120525-8436.log.
SHA-256: `2d8b8666b146c621d2115cf7092d4898b3cd26a1d50fb71537af14f6bac05395`.
The pool records the exact head on Darwin 27.0.0. The focused unit groups report 63 PASS, zero failures.
Those groups include the Mac holder-count and written-count tests. Pool evidence is outside the local shared-target hazard.
Sol reads the decoded command, every named result, and the complete failed-run detail groups.
The wrapper reports exit 0 after 263 seconds: one second queued, 262 seconds running.
The command retains failed transcript runs and ends with checkout and echo. Wrapper exit 0 does not prove all tests passed.

Real ou_1, ou_11, ou_12, ou_3, ou_5, and a2_1_attach_state_rules each report three PASS results.
Real ou_9 fails run 1 at seed 7, step 21: the screen text is empty. Runs 2 and 3 pass.
Real ou_10 fails runs 1 and 2 at seeds 1 and 4, step 25: output ab precedes input_refused. Run 3 passes.
Real dp_3 times out at step 17 in all three runs. Real a2_1_no_deadline times out at step 23 in all three runs.
Testkit a2_1 cases report unsupported hold_start. The testkit control belongs to separate work.
After the scratch list removals, dp_1 and dp_2 fail the missing named-proof guards on both tiers.
Those guards also make each other cargo command report failure, even when its selected transcript passes.
Sol sent these limits to both P6 agents. No pending removal is accepted from this record.

### Historical Linux gate

The original 722b415a log has SHA-256 `88a781fdc8ff046d69d28090726485e402281c71ed879665c79fe72519554f5c`.
Sol reads its step summaries, timing, tier totals, and mutation totals. Sol does not claim a complete catcher comparison.
All ten steps report PASS in 1756.9 seconds. Default: 1753 PASS; slow: 570 PASS.
Minimum: real-passing 49/68, real-accepted 50/69. There is no FLAKY token.
Stage 1: 165 mutants, 65 caught, 96 missed, four unviable, zero timeouts, 348.6 seconds.
Stage 2: final total 141 caught, including 76 in stage 2; 24 unviable, zero missed, zero timeouts.
The original gate does not cover the later source corrections. The new Linux gate remains running by owner report.
Sol runs no build, test, gate, mutant, reversal, or measurement. Never poll.

## Round 1 — source and required Mac evidence, 2026-10-11

Exact head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
Sol reads the complete package Round 4 at `b64bc6368dcea89176ec877ad7822ac7a43aae18`.
It closes RC-R1-4 using the 63 passing Mac library tests. No package finding remains.
The source and tree retain the complete static review recorded above. No integration source finding remains.
The focused library filters include the required holder and written-count tests.
The log gives the group summary, not each test name. Sol makes no claim that individual names appear in that log.
The exact-head Mac evidence satisfies the per-OS condition for this source verdict.

The mixed transcript results and masked aggregate command failures remain as recorded above.
No failed run supplies pending-removal evidence. This verdict accepts no pending removal.
The historical Linux gate applies only to 722b415a. It does not prove the later source corrections passed a full gate.
The completed new Linux gate and updated PR body remain due.
The current-v1 import, applicable check and delta review, final gate, and final body remain due under the slot rule.
This verdict does not approve PR completion, any later head, or a later import.
Sol runs no execution job. Never poll.

VERDICT: CLEAN (source and required Mac evidence only) at 2645bfc1c216cea54cd61577cae25e834e66133c

## Round 2 — failed Linux gate and proposed test correction, 2026-10-11

Gate head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Gate comparison base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Owner receipt: `msg_plugin-w_1791747225_d791ab`.
The completed gate fails. Round 1's limited source verdict does not approve this failed gate or PR completion.

### RG-SI-1 — MEDIUM, open: missed held-byte mutation

Both mutation stages miss `edge_tap.rs:347:9: replace Tap<E>::link_held_for_start -> bool with true`.
This mutation lets the quiet check treat unread link bytes as bytes held for start.
The existing test reads the bytes after the first quiet call that receives them.
It does not call quiet again while those bytes wait for the driver.
Add that observable quiet check and supply a completed corrected gate.
Keep the failed gate explicit in the body. Do not present it as green.

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-2645bfc1-pool-20261011-120525-8394.log`.
SHA-256: `2922694baf31867c1e6086859c9f6882f94ed7febb58efd0c762c336fcb18f74`.
Sol reads the header, tier and stage summaries, missed identity, final summary, and exits.
Eight steps PASS; mutants FAIL; fuzz is NOT RUN.
Default: 1755 PASS, 474 skipped, 11.016 seconds. Slow: 570 PASS, 2038 skipped, 40.584 seconds.
Minimum: testkit 61/69, real-passing 49/68, real-accepted 50/69.
Stage 1 has 164 mutants: 66 caught, 94 missed, four unviable, zero timeouts, in 318.3 seconds.
Stage 2 has 94 mutants: 73 caught, one missed, 20 unviable, zero timeouts, in 1155.8 seconds.
The unmutated baseline passes in 52.3 seconds.
The step total is 1646.0 seconds. The job and wrapper exit 1 after 1654 seconds.
The job queues for one second and runs for 1653 seconds. The complete log contains no FLAKY token.
Sol claims no complete independent comparison of the caught identities in this failed run.

Proposed correction: `27cb2cc48562334612d2d227aff8d8bb88a30d52`, tree `e02638de17d5bf1031e2a6807a07eb20ef906125`.
Sol reads its complete two-line delta and the affected test and quiet implementation.
The new assertion calls quiet again before the driver receives the byte.
It observes the edge's quiet result and does not add a private-state assertion, loop, sleep, or production change.
Sol finds no new static source defect in this delta.
The corrected gate is running by owner report. Its result, matching package review, and corrected body remain due.
No formal CLEAN verdict covers this proposed head yet.
The final import, applicable check and review, final gate, and final body remain due under the slot rule.
Sol runs no build, test, gate, mutant, reversal, or measurement. Never poll.

VERDICT: NOT CLEAN (1 open evidence finding) at 2645bfc1c216cea54cd61577cae25e834e66133c

## Round 3 — corrected gate and body, 2026-10-11

Exact head: `27cb2cc48562334612d2d227aff8d8bb88a30d52`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Gate and live PR comparison base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Tree: `e02638de17d5bf1031e2a6807a07eb20ef906125`.
The complete source review and unchanged-implementation Mac evidence retain their earlier scopes.
Sol reads the complete two-line correction again and the affected test context.
Package source CLEAN at this head remains recorded in `dafcb2fbd0db52070e43a479df1cb1098070b5f8`.
Sol reads its complete Round 5. The package also reports that its current raw/cited comparison agrees.

### Completed corrected Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-27cb2cc4-pool-20261011-123345-13820.log`.
SHA-256: `3d15d9aea86074d2e1f1fdde37c51e8aa5962169691a143a18efd02cad99715a`.
The log has 6040 lines. Sol reads the exact head/base header, tier summaries, stage outcomes, catch records, summary, and exits.
All ten steps PASS in 1557.6 seconds. The job and wrapper exit zero after 1568 seconds.
The job queues for one second and runs for 1567 seconds.
Default: 1755 PASS, 474 skipped, 12.458 seconds. Slow: 570 PASS, 2038 skipped, 41.723 seconds.
Minimum: testkit 61/69, real-passing 49/68, real-accepted 50/69.
Eleven of 36 pending-real trials pass. No pending file changes, and these results authorize no pending removal.

Stage 1 has 164 unique mutants: 67 caught, 93 missed, four unviable, zero timeouts, in 298.0 seconds.
The unmutated slow baseline passes in 46.0 seconds: 570 PASS, including ten slow, and 2728 skipped.
Stage 2 has 93 unique mutants: 73 caught, zero missed or timed out, 20 unviable, in 1091.6 seconds.
The stage-2 identities exactly equal the stage-1 missed identities.
Sol independently compares all 140 raw and named catches. The sets match, with no duplicate identities.
Every named catcher appears as selected and passing in the normal gate tiers.
Final result: 140 caught, zero missed, zero timeouts, 24 unviable. Unviable results are not catch evidence.
Fuzz selects no changed crate with a decoder harness. No mutation exclusion changes in this PR.

RG-SI-1 closes. Stage 1 catches Tap::link_held_for_start -> true with the corrected interrupted-read quiet test.
The live body records the failed 2645bfc1 gate, its missed mutant, and the new observable quiet assertion.
No production code changes in the correction. Zero integration findings remain on this source and supplied evidence.

### Flaky result and required follow-up

The log has one FLAKY result: botster-core-sys::slow_process a_spawned_child_matches_its_identity_and_dies_by_the_group_kill.
It fails then passes under deletion of the route_stream_holders control arm. That retry supplies no catch.
The named real route holder test independently catches that mutant.
BUILD.md Testing rule 9 requires a structural fix; its mutation exception permits only removal of a flaky catch.
Lead receipt `msg_plugin-w_1791749219_245fdb` assigns P6 one structural-fix PR after #287 and states that this result does not block #287.
Sol reads the complete corrected record at `shared/core-stage1/handoffs/flaky-a-spawned-child-matches-its-identity-and-dies-by-the-group-kill-27cb2cc4.md`.
Sol reads the sys test, its helpers, and sys/test-process dependency declarations. The test does not call these route controls.
The record's cargo-tree result remains an owner report. Sol runs no cargo command.
The raw gate contains no first-attempt panic or assertion values. The exact failure cause remains unknown.
The proposed many-run campaign is removed from the record after Sol's review. No campaign, retry change, or timeout change is accepted.
The structural fix remains due. This verdict does not close that separate defect or accept an unproved load diagnosis.

### Current body and final import limits

Sol reads the complete retained body, all later body corrections, and the matching live GitHub body.
GitHub confirms the exact head and comparison base above.
Corrected body SHA-256: `47d3c7961a9986ce51931bffa9063d9a69cec87cd2fda8fb21128ccb8c96d4b4`.
The body labels the old .36 hold_start table as current transcript data, not proof of current R-71 conformance.
It states the a2_1 named startup requirement, both clock steps, and the removal of ad_7 section 1.
P3 owns that transcript follow-up after #58 with 016c review. Both identifiers remain pending in this PR.
The body cites P6's separate flaky-test record and corrects the two slow test paths.
The historical mixed Mac transcript failures and aggregate exit limits remain explicit.
No new Mac assertion result is claimed; the required Mac implementation evidence carries because the implementation is unchanged.

The final SLOT, current-v1 import, applicable check and delta review, exact final gate, and final body remain due.
This source CLEAN accepts no later head, pending removal, pin, final import, or PR completion.
Sol runs no build, test, gate, mutation, reversal, or measurement. Never poll.

VERDICT: CLEAN (0 open; source and supplied evidence before final import) at 27cb2cc48562334612d2d227aff8d8bb88a30d52

## Round 4 — final-slot source delta, 2026-10-11

Exact head: `fdd942db5f3d0aea71c37c33ea993179f42b4f19`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Tree: `ab1ed6f110d7629f9f865baa1719ac1d5bfed624`.
Owner receipt: `msg_plugin-w_1791757547_80f1fb`.
The completed canonical check, final gate, and matching final package verdict remain due.
Sol reads no running gate output and runs no executable check.

### Import and source composition

The merge `90efe15c9747023be0b18252f64cd68a6d5414be` has parents 27cb2cc4 and the exact base above.
The old source base remains `f49f74302526a28b6aced1595f74c7d0a1d22836`.
The import changes 26 base paths against ten own paths. Four paths overlap: real.rs, real/program.rs, real/route.rs, and route_client.rs.
Sol independently checks all 338 nonoverlap tree entries; each equals the applicable own source or new base entry.
Nine own files retain their exact added and removed lines against the respective bases.
The remaining route.rs delta preserves the union of controls, including imported `input_blocked`.
Its changed diff alignment moves two closing lines between adjacent hunks; the resolved match retains both complete arms.
Sol reads the complete merge message, both conflicting file deltas, relevant resolved source, and all three post-merge commits.
Both imports and both test sets remain. No reviewed control or imported R-68 test is removed.
The imported real program uses the shared `PtyOutput` parser. The imported testkit `input_blocked` arm remains unchanged.

Commit `75f0fd0dfcff45979dccbe03aec62e4463b9a8c2` supplies the new Handle wake field to the existing test literal.
It uses the same socket-end form as the imported base test. It changes no production path.
Commit `134ad99c06b7aa0eff6e05a93be081a6c6b9a4aa` changes readiness and adds its raw-route test.
Commit fdd942db removes exactly five real-pending identifiers and changes no other file.
The five identifiers are OU-1 attach, OU-11 ownership, OU-12 output order, OU-3 progressing reader, and OU-5 peer fault.
All other pending bytes equal the imported base. This review accepts no pending removal before the required final PASS.
The base is the #285 merge. Its tree equals #285's accepted gated tree `886ac9ddad72fc5b92a0c66646aa964965a1b00c`.

### RG-SI-2 — MEDIUM, open: buffered readiness disables the host wake for the rest of a route step

Source: `crates/botster-core-testkit/src/real/route.rs`, readiness at line 504 and read at lines 383–405.
The new readiness condition returns `None` while `taken` holds bytes, including after `route_gate {on: false}`.
The pinned .39 driver calls `install_waits(None)` once before `routes.exec` and clears those waits only after that step returns.
In `crates/botster-core-conformance/src/driver.rs`, lines 2017–2030, `install_waits` stores no host wake when readiness returns `None`.
In the same driver, lines 451–460 and 289–291, that stored wake determines whether every read uses a passed deadline.
When the stored wake is absent, `route_pass` instead gives the end the live step deadline.

A route step can start with buffered bytes, an ungated route, and `handed == true` from an earlier empty read.
The first read consumes `taken`. A later read in the same step can find the stream empty.
RealRoute then calls `read_stream` with the remaining step time. That poll waits on the route alone before the host pump runs.
The driver's stored wake stays absent even though readiness now supplies the route descriptor.
A tail or route close that requires a host pump can therefore wait until the step deadline despite a readable host wake.
The hub route driver supports this sequence: a partial frame produces `Pump::Progress`, then `next_until_received` reads again within the same step.
Its source is `crates/botster-hub-conformance/src/route/driver.rs`, lines 317–351, 414–447, and 603–618 at the same pin.

The new test polls raw readiness and then calls raw read. It does not call `install_waits` or run this buffered-to-empty driver transition.
Its reported red-on-revert result cannot prove preservation of the host wake across that transition.
The pinned driver also reads the end before `wait_route`; the new test does not establish the body’s claimed original driver failure.
Sol establishes this source path by static reading. Sol claims no executed reproduction or gate failure for it.

Required change: preserve the host wake and nonblocking read behavior throughout a real route step after buffered bytes are consumed.
Add a driver-level regression for an ungated route with buffered bytes followed by a host-dependent tail or close in the same step.
The regression must observe the driver outcome and retain the existing deadline and ownership rules.
If the repair needs a contracts driver change, the lead must assign its scope and pin order.
Do not use a longer timeout or the raw readiness test as a substitute for that interaction proof.
Sol sends this path to both P6 agents before this verdict. The package's initial static message reports no new source finding.
The package has not supplied a completed response to this integration path in this round.

### RG-SI-3 — LOW, open: final body still says no pending-list change

Body SHA-256: `73ce303e3b0eba56db7b21d72f9a1f2455ab78e06f071980124586018f138dd1`.
GitHub MCP fails. The GitHub CLI fallback confirms the exact final head and base and supplies the complete body delta.
The updated body records the four final commits, current plan pin, readiness interaction, R-86 relation, and five pending removals.
Its Risk tier paragraph still says `No transcript, pending list or timeout changes.`
The final head removes five identifiers from `core-real-pending.txt`, as its later paragraph correctly states.
Required change: make that opening scope statement agree with the final pending-list delta.
The completed canonical-check output, final gate result, and exact final evidence body remain due separately.

The earlier source CLEAN remains limited to 27cb2cc4. It does not carry across this behavior change or these pending removals.
RG-SI-1 stays closed. The separate sys flake and its assigned structural follow-up remain open outside this PR.
Sol runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base-check job. Never poll.

VERDICT: NOT CLEAN (2 open: RG-SI-2 MEDIUM and RG-SI-3 LOW) at fdd942db5f3d0aea71c37c33ea993179f42b4f19 against base 5e953398925351c12e463a7b0a946b79d9841edc

## Round 5 — full withdrawal and corrected body, 2026-10-11

Exact head: `d3a60c99ab9165422e469de3323a5be12e55977c`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Tree: `9b0eae8f1b632146d2e488ba4a4a1d1c6aadd17a`.
Owner receipt: `msg_plugin-w_1791757839_ebb30d`.
Lead direction: `msg_plugin-w_1791757881_6c75cb` accepts the defect and directs review of this reverted head.

Sol reads the complete revert and compares its effective tree with the earlier import and compile-fix trees.
The revert removes the entire readiness change, incorrect comment, and raw-route test from 134ad99c.
The route.rs blob equals the pure import at 90efe15c exactly.
Against `75f0fd0dfcff45979dccbe03aec62e4463b9a8c2`, the only effective tree delta is the five real-pending removals.
The accepted #282 readiness function is restored: an open ungated route supplies its descriptor even when taken bytes wait.
The driver therefore retains the host wake and uses nonblocking reads throughout that route step.
The driver reads taken bytes before it waits. Its wait follows idle reads, with the host wake still available.
A gated route still returns taken bytes or Empty without waiting. A closed route returns EOF without waiting.
RG-SI-2 closes through complete withdrawal of the added behavior and its incorrect premise.
The requested driver regression applied to a repair that retained new behavior. Sol requires no additional regression for this complete withdrawal.
Package message `msg_plugin-w_1791758025_dc1b93` independently confirms the revert and withdraws its added regression request.
Its formal new-head package receipt remains due.

GitHub CLI verifies this exact head and base and supplies the complete corrected body delta.
Body SHA-256: `9d790cfdde4a39a3d0326983ab3478189974095e5a002547060be68f25a1f845`.
The Risk tier paragraph now states the five real-pending removals. The earlier evidence heading is explicitly limited to the reviewed head.
The R-86 paragraph no longer says readiness reads taken or derives ordering from the withdrawn guard.
It preserves the different byte directions and the requirement that only read supplies taken bytes to the transcript.
Its future interaction note requires another read before waiting if progress moves bytes into taken.
This review accepts no future R-86 implementation or contracts driver change.
RG-SI-3 closes. The body retains the withdrawn fix and states that its older prep proof supplies no final-head evidence.
P6 reports that the fdd942db gate was stopped. Sol reads no running output and accepts no result from that stopped gate.

### Pending-removal evidence and accounting question

Sol reads the decoded Linux evidence command and all 15 relevant real PASS records, plus the five testkit PASS records.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-722b415a-pool-20261011-115554-74918.log`.
SHA-256: `2d0da52eff78d44496db9858cf4cb29080201393f92d0921f957038064ebace2`.
The job names 722b415a. It reports three distinct real trials per proposed removal and one testkit trial per identifier.
The Mac record in Round 1 likewise supplies three real PASS results per identifier on 2645bfc1.
Both commands retain aggregate failures from the scratch real-only removals. Wrapper zero does not mean every selected trial passed.
The evidence uses scratch pending removals and library seed selection, not new-head final-gate evidence.
Sol independently verifies that all five transcript files are byte-identical between .36 and .39.

The pinned .39 exception file still names three proposed removals:

- OU-1 attach, raw step 9: no `program_ready`, R-60.
- OU-11 ownership, raw step 9: no `program_ready`, R-60.
- OU-3 progressing reader, raw step 19: no `await_title`, R-57.

OU-12 and OU-5 have no entry in that file.
The lead's earlier per-OS rule permits removal only after the repeated proof and this PR's final PASS.
The lead handoff separately names a proposed R-56-only guard. It does not resolve the scope of these R-60 and R-57 entries.
Sol asks the lead whether that earlier #287 ruling explicitly permits these three removals or requires retention until the fences land.
Sol accepts no real acceptance for these three identifiers while that disposition remains due.
The repeated PASS results alone supply no proof that the missing fences are fixed.

### Remaining final acceptance condition

RG-SI-FINAL-1 — MEDIUM, open: the pending-accounting disposition, completed canonical check, final gate, final body, and matching package verdict remain due.
The source defect and LOW body finding are closed on the exact reverted head. The final acceptance condition remains open.
The old source CLEAN does not automatically accept these new pending removals or final execution.
Sol runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base-check job. Never poll.

VERDICT: NOT CLEAN (1 open final acceptance condition: RG-SI-FINAL-1) at d3a60c99ab9165422e469de3323a5be12e55977c against base 5e953398925351c12e463a7b0a946b79d9841edc

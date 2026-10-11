# RC oracle encode — Core #288

## Round 1 — 2026-10-11

Head: `622db05f9e24e6a62196a13a43e66e2e060e3c5e`.
Base and merge base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Tree: `eb193caa11773747591fe4b3fb5acad05890a017`.
Risk: HIGH, shared testkit and conformance lists.
Sol reads the complete seven-path delta, affected helpers, complete live body, and package Round 1.
Package verdict: `8526643d787ba761723eeaa8cf8a9243ee00c4e1`, verdicts/rc-real-controls.md.
The package has two open findings. Sol agrees with both findings below.
The GitHub MCP read fails. The GitHub CLI supplies the full body and exact head/base.
Body SHA-256: `494fef659d67cc0aa12279549eba84764dcb7d87f6cc6f10fee735d632656631`.
Current plan authority remains reviewed 23cm. The R-84 corrections and R-85 govern before their pending publication.

### Source and interfaces

The delta changes no production host, worker, binding, mutation configuration, exclusion, or contracts pin.
The active contracts pin remains .36 at `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
Both tiers register oracle_encode, oracle_modes, and oracle_state through the shared helpers.
The real tier retains its tapped guard. The controls use the real Core machines and injected edges.

Encode parses the contract input types and calls the pinned explicit-mode encoders.
The base comes from a fresh binding terminal at runtime. The other_modes overlay merges by key.
Unknown fields, unknown other_modes keys, and invalid typed values return Bad.
No testkit table supplies expected encoded bytes. The worker cross-check proves dispatch, parsing, and merge wiring, not the encoder.
The explicit size wins over stored size. The R-84 follow-up permits that stored host input.
NotReported becomes an empty hex string. Unsupported retains its refusal reason and what value.
Focus with reporting off is empty. Paste with bracketed paste off contains its text alone.
A repeat argument is Bad. Missing modes returns oracle_encode_without_modes and remains an explicit unsupported form.
This review does not claim support for that missing-modes form.

The history controls use O_raw, the independent output replay. They do not read subject modes, title, or cwd caches.
The testkit reads ModelLog, finds the consumed output under R-67, and creates a new raw terminal.
The real tier checks known history and replays the probe's output for the correct worker identity.
Both tiers retain the size/profile-change refusal. The refactored oracle_screen uses the same raw paths.
Sol reads ModelLog::raw, its replay, raw_oracle, both session lookup paths, and the affected real test context.
The state helper drains its own replay events. The last Title and Cwd events determine the fields independently of Core.
An explicit empty value remains set. No event leaves the field absent. A dropped replay event makes the result Bad.
The explicit empty-OSC binding test is required by R-84(5); it is not the literal mode defect below.
The current Linux gate records a PASS for that binding test.

### Conformance accounting and evidence

Eleven identifiers leave core-pending.txt. All eleven have named default-tier PASS records in the exact-head gate.
Two also have named real-tier PASS records: ev_2_mandatory_and_keyed_never_dropped and in_9_legacy_release_is_not_reported.
Nine enter core-real-pending.txt. Their pending limits remain explicit. They are not counted as accepted real passes.
The .36 transcripts contain 243 oracle_encode calls. Sol's static parse finds modes present in every call.
The two complete Mac evidence logs record the earlier testkit and real results.
Their headers report the clean and single-worktree history. Sol does not independently prove that history from a clean log.
The cea1de64 log covers eight identifiers. The 7b19bfe2 log names all 38 selected identifiers and 11 real selections.
The later log has 11 named testkit passes, 27 named failures, and six real failures or inconclusive results.
The three other unfenced real selections pass there once. The later list correctly retains their unfenced-read limits.
The exact-head Linux gate supplies current positive proof for the removed default identifiers and the two real removals.
The legacy-keys raw expectation also contains 0d. Its eventual transcript review must apply R-85 as well as R-56.

Mac log SHA-256 values:
- cea1de64: `62173f1fdf0ab2cd28c2203acf80f88853ce352973bfcf94636d81fc93db79fc`.
- 7b19bfe2: `acbca18862df90d07e0a808825b4429cf4072c2080eed5d8c7f04b7899f0db6c`.

The body names the four manual reversals and the 19-call comparison.
Sol asks RC for retained raw records. Those claims remain owner reports in this round.
Sol accepts no independently verified manual reversal or 19-call result from the body alone.
The three ModeFlags::default test contexts were read during the plan review. Their scripted-fixture explanation matches those contexts.
The resize replay gap and missing-modes form remain open limits. This review accepts no later implementation of either form.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-622db05f-pool-20261011-132210-26921.log`.
SHA-256: `166a234017cf97a81d44c7e342718a93a998ec1e1e5c4d290e75bc9ea5985a1f`.
The header names the exact head and base above. All ten stages pass in 387.7 seconds.
The job and wrapper exit zero after 395 seconds, with zero queued seconds and 395 running seconds on MS-A1.
Default passes 1770 tests, with 463 skipped, in 11.049 seconds.
Slow passes 561 tests, with 2040 skipped, in 40.428 seconds.
Minimum counts remain testkit 61/69, real-passing 49/68, and real-accepted 50/69.
Strict runs 352 of 370 pending identifiers, with 18 real-only not run and two held.
Pending-real has 45 entries, of which ten pass under the ignored run. No further removal is accepted from that report.
Fuzz reports no changed crate with a decoder harness.

Sol independently parses the raw mutation identities and every named catch record.
Stage 1 has 32 unique identities: 18 caught, 12 missed, two unviable, zero timeout, in 73.8 seconds.
Stage 2's 12 unique identities equal the stage-1 missed identities exactly.
The unmutated slow baseline passes in 45.6 seconds; its tests pass in 40.302 seconds.
Stage 2 catches 11, with one unviable and zero miss or timeout, in 152.8 seconds.
All 29 raw caught identities match the named catch identities, without duplicates.
Every named catcher has a selected PASS record in the gate. No FLAKY token occurs.
Final: 29 caught, zero missed, zero timeout, three unviable. Unviable results supply no catch proof.
The earlier three real-control misses now have catches, including the named real oracle test.

Sol reads the earlier gate headers, failure output, mutation records, summaries, and exits.
The 1c4ce117 gate fails slow for an empty unfenced input log at seed 0, step 8. Mutants and fuzz are NOT RUN.
Its SHA-256 is `7edf13f611b84c1710ba4fa70bf137f866e34a0e86bd88c7eef8ae81ef8d5f67`.
The 2808452a gate records three stage-2 misses before RC stops it. Its exit is 130, not a completed green gate.
Its SHA-256 is `57dbe9f32739023745edf0d71800fb724fd0e4b8019e9e161ad60267c369f927`.
The current body preserves both failures and their later corrections.

### OE-SI-1 — MEDIUM, open: literal terminal-mode assertions

This agrees with package RC-OE-1.
At oracle_controls/tests.rs:501–502, the test asserts that fresh dec_1035 and dec_1036 are true.
At lines 771–774, the test asserts three modes true and cursor visibility false.
Those assertions specify terminal state in the testkit. BUILD's oracle rule and corrected R-84 give that state to libghostty.
Remove the literal mode assertions. Retain the independent binding comparison and the merge-sensitivity checks.
If the test needs a changed mode, obtain its expected state from an independent binding terminal.
Retain the explicit empty-OSC binding test required by R-84(5).
The later source delta and matching package closure must be reviewed.

### OE-SI-2 — LOW, open: incorrect real-pending count in the body

This agrees with package RC-OE-2.
The Lists bullet names five unfenced in_9 identifiers, then says Four in this bullet plus the keypad identifier.
The five unfenced identifiers plus in_9_application_cursor_and_keypad make six in_9 identifiers.
Correct or remove that parenthetical count. Retain the per-identifier causes and owners.
This is a body correction. It needs no execution when the source head remains unchanged.

The green gate closes neither finding. Both findings, including LOW, must close before CLEAN.
Contracts #60 remains the separate steward-owned docs PR with 016c review.
The final SLOT, current-v1 import, applicable check and delta review, exact final gate, and final body remain due.
This verdict accepts no later head, later pin, future pending removal, import, or PR completion.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (2 open: OE-SI-1, OE-SI-2) at 622db05f9e24e6a62196a13a43e66e2e060e3c5e

## Round 2 — correction and reversal receipt, 2026-10-11

Correction head: `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Tree: `3ea547a1dc11cd38ce7a9185d8015eea2ee04441`. Base remains `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Sol reads the complete one-file correction: one added line and seven removed lines in oracle_controls/tests.rs.
OE-SI-1 closes. The tests remove the literal terminal-mode assertions.
The modes test compares the output replay with an independent fresh terminal, then compares the control with the output replay.
The binding comparisons, merge-sensitivity checks, and required empty-OSC binding test remain.
The correction changes no production control, list, or mutation exclusion.
Sol reads the complete corrected live body and its complete delta from the retained first body.
Body SHA-256: `a7e7ade9a8ae07e38bba0df8f42014748cceedb3fc8e21979050e02225439033`.
OE-SI-2 closes. The incorrect parenthetical count is removed. The body retains all per-identifier causes and owners.
The body adds the correction section, current reversal line numbers and log, and a queued-gate status.
Sol reads package receipts `f1a874d4160cc3cc6664a81d58d9885f7ea430f2` and `e8cc06940b5bf107ac45c8253c3c587920f785a6`.
Both package finding conditions close. The package has not issued completed CLEAN at the new head.

Sol reads the complete four-reversal log and its complete script without executing the script.
Log: `~/botster-sessions/gates/botster-core-rc-oracle-reversals-mac-41c1f7e2-20261011-133400.log`.
Log SHA-256: `b7b7a80914319f21fee795c1202c7755e5203537e785199900195bafe34c8572`.
Script: the same path with .py instead of .log.
Script SHA-256: `91d1013d4897b5b261392822c95d327076448279442925765385faa275daf691`.
The script runs one exact test after each source reversal. It removes RUSTUP_TOOLCHAIN and uses four build and test threads.
It uses CARGO_TARGET_DIR ~/botster-sessions/targets/rc. It restores the file after each run.
The script filters output to failures and result summaries. The log therefore contains no complete build output.
RC's provenance receipt is `msg_plugin-w_1791751209_2d16ce`.
RC reports a clean of that target, with 3.0 GiB removed, before this head's 18 oracle-control tests and four reversals.
RC reports that only this worktree builds there after that clean. Contracts builds use targets/rc-contracts.
That owner provenance satisfies B13's single-worktree condition. Sol has no raw clean log and claims no independent clean verification.
Each reversal exits 101 with one failed test and a behavior assertion.
The getter-state reversal fails at tests.rs:804: absent title/cwd differ from the expected empty strings.
The all-off base reversal fails at line 505. The shallow overlay reversal fails at line 530.
The ignored-size reversal fails at line 609. The log ends with a clean restoration status.
Sol accepts these retained failures with the stated provenance and filtering limits.
The 19-call comparison remains RC's report. Sol requests no additional comparison run.

Zero source or body finding conditions remain open. Formal new-head CLEAN remains pending the completed exact-head gate and matching package verdict.
The earlier head's green gate does not prove this correction. Contracts #60 remains a separate docs review.
Final SLOT, current-v1 import, applicable check and delta review, final exact-head gate, and final body remain due.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.


## Round 3 — completed corrected source gate, 2026-10-11

Source head: `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Tree: `3ea547a1dc11cd38ce7a9185d8015eea2ee04441`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
The complete source correction and four reversal records remain as reviewed in Round 2.
Sol reads package CLEAN `044e6e8e213afb214b907280568381ec780513df`, which covers this exact head and base.
Both integration findings and both package findings are closed, including LOW.

Gate: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-41c1f7e2-pool-20261011-133549-59239.log`.
SHA-256: `6e1fc8f6d1fef8f60c9e0aa52a446556892e48b0476afef3b319ef48e808115c`.
Sol reads the complete header, raw mutation results, catcher lines, suite summaries, step summary, and final exits.
The Linux pool header names this exact source head and base and four build and test CPUs.
All ten steps PASS. Step total: 409.7 seconds. Job and wrapper exit zero after 418 seconds on msa1.
The job queues one second and runs 417 seconds.
Default: 1770 PASS, 463 skipped, 11.859 seconds. Slow: 561 PASS, 2040 skipped, 40.701 seconds.
The strict testkit run records 227 PASS and zero failures. Lists records 690 ledger ids, 440 pending, and 230 to run.
Real minimum: 49/68 passing and 50/69 accepted. Eleven of 45 pending-real trials pass in this job.
Those pending-real results cause no extra removal through this verdict.

Stage 1: 32 mutants, 18 caught, twelve missed, two unviable, no timeout, 79.1 seconds.
Stage-2 baseline: 561 PASS, 2730 skipped, 41.504 seconds; baseline duration 47.6 seconds.
Stage 2 retests the same twelve missed identities: eleven caught and one unviable, no miss or timeout, 163.5 seconds.
Combined: 32 mutants, 29 caught, three unviable, no miss or timeout.
Sol independently parses all 29 raw catches and all 29 citations. Each set has 29 unique identities, and the sets agree.
Each cited test has a normal PASS in this gate. No FLAKY record exists in the gate log.
The catches cover fresh modes, partial mode merging, event state, history reads, and real control dispatch.

The live body retains this exact source head and reviewed base.
Body SHA-256: `0ef5d475a624f050dc453f2d7e882279c07242722998a0ddbee84d4a45776f5c`.
Sol reads its delta from the corrected body: only the queued-gate paragraph becomes the completed exact-head result.
The body preserves earlier red and stopped gates, their causes, source corrections, list limits, and reversal limits.

This CLEAN covers the source head and reviewed base above. It does not accept final-slot completion.
The implementer must obtain SLOT, import current v1, provide the applicable canonical check and delta reviews, and run the final exact-head gate.
The final body must name that final head, base, and gate. Contracts #60 remains a separate docs review.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: CLEAN (0 open) at 41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56 against reviewed base 926dfb62f3f7168990b2a48741946932d84b2fbb

## Round 4 — final-slot import and pending moves, 2026-10-11

Exact head: `46c8c5481a3cdf11ec98949227fda32f4d0f0ee0`.
Base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Tree: `abb16eb98ca2c99349976ea9a72d43a617293501`.
Reviewed head: 41c1f7e2, base 926dfb62. Owner receipt: `msg_plugin-w_1791761559_28d91d`.
Sol reads the complete canonical-check log and final-slot body.
Log: `~/botster-sessions/gates/botster-core-rc-oracle-base-merge-check-46c8c548-20261011-163114.log`.
SHA-256: `0081f088126d83e2c989da18c76b60d6ecf6f3d3d8246493d24c0fc6ec2203c8`.
The check names this exact head/base, passes both ancestry conditions, and returns FAIL requiring both delta reviews.
It reports two pending-file conflicts, real.rs overlap, changed own diff, and line-set changes.
No automatic Q7 carry applies. Sol performs the required explicit delta review.

Sol independently compares all seven own paths with the 30 changed base paths.
The four unchanged own files retain their exact blobs: DESIGN.md, oracle_controls.rs, its tests, and real/oracle.rs.
All own added/deleted lines in real.rs equal their earlier lines. Imported hold_handoff and release_handoff arms remain separate.
The oracle guards, binding-derived modes, history source, and event-based state retain their prior source review.
All imported paths outside the own paths equal the new base. No product behavior is added by this import resolution.

Sol reads both complete pending-file import deltas and the subsequent two-file move commit.
For core-pending, core-real-pending, minimum-core, and core-ledger-ids, Sol independently verifies the three-way identifier formula.
The merge sets equal the new base minus the original own removals plus the original own additions.
The six prior base removals remain removed. The real-pending file retains both appended groups.
The canonical diagnostic names an ev_8 comment as removed or edited; that exact comment exists unchanged at all four comparison heads.
Sol does not infer a literal removal from that diagnostic. Its failed line-set condition still requires this manual review.

The subsequent commit moves exactly four identifiers from testkit pending to real pending, with five explanatory comment lines.
They are in_9_key_encoded_by_worker_modes_at_write, in_9_kitty_modifier_keys_need_flag_8, and the two mouse groups.
Sol reads all of their pinned .39 fence entries.
They have respectively one, two, nine, and thirteen R-56 entries, and one R-60 entry each.
The new comments preserve these gaps and P3 ownership. The move grants no real-tier acceptance.
The source update makes no transcript or timeout change.

The supplied local merge-head log retains 1161 default PASS, prebuild, and a clippy count of zero.
Its strict section has no completed summary. RC's 245 strict PASS claim at the final head remains an owner report.
The final exact-head gate must establish the four new testkit removal results.
Sol reads complete package delta receipt `668b82aeb5364db5afd21fac477dce04cc7a2fdc`, `verdicts/rc-real-controls.md`.
That review covers the same head, base, tree, conflict resolution, overlap, fence entries, and execution limits.
No source or body finding remains open in this delta. The completed final gate and matching final body remain due.

GitHub confirms this exact head/base. Body SHA-256: `71d823a67ec7bef4795eaaaae3335e73251b750f724e0867270a3a3ee47c3dc1`.
Its final-slot section retains the failed check and reports the gate as running.
Sol reads no running gate output and accepts no result from that unfinished gate.
Sol runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base-check job. Never poll.

VERDICT: NOT CLEAN (1 open final acceptance condition: completed exact-head gate, removal evidence, final body, and final package verdict) at 46c8c5481a3cdf11ec98949227fda32f4d0f0ee0 against base f6025aed4cdf7da836e835e0040fc128b7e8f32f

## Round 5 — completed RED final gate, 2026-10-11

Exact head, base, and tree remain those in Round 4.
RC receipt: `msg_plugin-w_1791762337_921adf`.
Package receipt: `msg_plugin-w_1791762366_3c7fda`.
Log: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-46c8c548-pool-20261011-163127-84246.log`.
SHA-256: `887ba42df9e718f5c3a278de38b9a236d59d11fd0a8dedd15855646cb0c874a4`.
Sol reads the completed exact-head/base headers, tier results, all mutation outcomes and citations, full failure, summary, and final exits.

Eight steps PASS. Mutants FAIL at the unmutated stage-2 slow baseline. Fuzz is NOT RUN because an earlier step failed.
The step total is 658.2 seconds. The Linux job exits one after 769 seconds: one queued, 768 running.
The wrapper exits one after 771 seconds.
Default: 1822 PASS, 445 skipped, 20.357 seconds. Slow: 599 PASS, 2034 skipped, 61.260 seconds.
Stage 1: 32 mutants, 18 caught, twelve missed, zero timeout, two unviable, 153.7 seconds.
Sol independently matches all 18 unique raw catches to all 18 unique citations; each catcher has a normal gate PASS.
The unmutated baseline runs 599 tests: 598 PASS, one FAIL, 2740 skipped, 61.240 seconds; its interval is 81.4 seconds.
Stage 2 tests no mutant and supplies no catch. The twelve stage-1 misses remain unresolved in this gate.
They are not a completed combined-stage survival verdict. No final green mutation result or fuzz result exists.
The log contains no FLAKY token. That wording does not erase its baseline failure.

The failed baseline test is `conf::st_6_capture_is_frozen_at_model_rev`, seed 0, step 17.
The expected expression is `{"ne":[1,1]}`. The received text is `the relation does not hold`.
The same identifier passes normally in both default and the earlier real slow run of this gate.
Sol retains the failed baseline as FAIL. The earlier normal PASS does not replace it.

All four identifiers moved from testkit pending have normal conformance PASS records in this completed gate.
They are worker-mode keys, kitty modifier keys, and both mouse groups named in Round 4.
These records supply their required testkit PASS evidence. They grant no real-tier acceptance or final merge acceptance from this RED gate.
Their real-pending fence gaps and P3 owner remain recorded.

### Static attribution and the proposed controlled proof

Sol reads the complete ST-6 transcript at contracts .39, ddce507e.
Its first pty_output writes 41 before CaptureSnapshot, with only await_quiet between them.
Its second pty_output writes 42 before ReadScreen, again with only await_quiet between them.
Step 17 requires the snapshot and later screen model_rev values to differ.
No model-read fence proves that the worker applies the second output before the later ReadScreen result.
The observed 1/1 values are consistent with that missing fence. The failed log does not contain the worker-read trace that proves this timing.

Sol independently compares the PR with plain base f6025aed.
The program controls, real program-control implementation, production host, worker core, and Core real facade have no own delta.
The changed real.rs dispatch adds only distinct oracle control names, with their tapped guards.
pty_output still dispatches through program::CONTROLS and program_control to the existing write_output request.
The ST-6 transcript calls no oracle control. The reviewed own oracle source does not alter its model feed or output fence.
This establishes static non-interference with the proposed missing-fence mechanism. It does not establish the required plain-v1 failure.
The package independently reaches the same limited conclusion in its receipt above.

RC stops after the RED result and reports no re-gate.
RC proposes one controlled reproduction on plain v1 f6025aed under plan 23bl(A).
No completed controlled proof or case-specific lead approval is supplied at this review.
A valid proof must retain the exact identifier, step, and symptom, plus the scratch timing delta and source/cache provenance.
The scratch delta must only widen an existing timing window; it must neither add missing behavior nor change the result under test.
Both reviewers must assess the completed evidence and non-interference before any exception is accepted.
The lead must record its case-specific decision, repair owner, and removing PR before it authorizes one same-head re-gate.
The re-gate must pass every step. A second RED result requires a source repair before another gate.
The previous 23al/23bl decisions for other failures supply no exception for this ST-6 baseline.

The final body must replace RUNNING with this completed RED result and retain its log, failing identifier, and stage limits.
Sol requests that update. No corrected final body is accepted through RC's message alone.
No new source defect in the oracle controls is established. The final acceptance condition remains open.
Sol requests no execution job and runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base check.
Never poll. Wait for the doorbell.

VERDICT: NOT CLEAN (1 open final acceptance condition: RED baseline attribution and disposition, complete final gate, final body, and final package verdict) at 46c8c5481a3cdf11ec98949227fda32f4d0f0ee0 against base f6025aed4cdf7da836e835e0040fc128b7e8f32f

## Round 6 — accepted controlled ST-6 attribution

Lead authorization: `msg_plugin-w_1791762533_7b87fe`.
RC completed-proof receipt: `msg_plugin-w_1791762548_4e2ebb`.
Lead case acceptance: `msg_plugin-w_1791762655_9e3738`.
This is acceptance of one controlled evidence record. The RED final gate remains RED.

Plain-v1 base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`; #288 is absent.
Diagnostic snapshot: `bf55aa2fb5615c58333f87990433e127224d891e`, captured with --allow-dirty, explicitly not a merge gate.
Sol independently verifies the snapshot's first parent is that base and its complete changed-path set contains only worker main.rs.
The temporary snapshot captures the scratch delta; it is not a delivery-branch or PR source commit.
The snapshot's full diff hash equals the separately retained scratch diff hash below.
Contracts: .39, `ddce507e2c08e740f899e97dcfec566f3d2275b8`.
Transcript blob: `81ece132501f26262d8fcac4396b09da9abec237`, independently checked against the pinned transcript.

Completed log: `~/botster-sessions/gates/botster-core-detached-f6025aed-f6025aed-pool-20261011-164814-5086.log`.
Log SHA-256: `a21e40b2b2db7710acd8cc0eb3572a726cd27f3720debcdde6db0f49e7428c09`.
Scratch diff: `~/botster-sessions/gates/rc-st6-repro-scratch-f6025aed.diff`.
Diff SHA-256: `5348e52c1741df1b50ffa31fc676681db95e964507a9ae1ded72936bce7f22ca`.
Sol reads the complete decoded command, source/pin headers, entire diff, build output, test failure, worker trace, and completed exits.
The pool uses MS-A1, four CPUs, and an exact captured checkout under its normal snapshot and target-stamp mechanism.
It builds the changed worker and relevant test binaries and reinstalls the .39 probe before the real-tier test.
The real-tier test uses BOTSTER_SEEDS=0 and exits 101, with one failed test after 1.05 seconds.
The job and wrapper exit zero after 17 seconds, with one queued and 16 running.
That zero follows the shell's retained failure and trace commands. It is not a test PASS or green gate.

The exact failure is ST-6 capture_is_frozen_at_model_rev, seed 0, step 17, expected `{"ne":[1,1]}`, received relation does not hold.
It matches the original RED baseline's identifier, seed, step, expression, and symptom byte for byte.
The scratch code retains the first PTY read whose bytes equal B, delays its model feed by one second, then supplies those same bytes.
While those bytes wait, it prevents another PTY read and shortens the poll interval to at most ten milliseconds.
Control requests still run. The diagnostic changes no checked predicate, model-revision value, transcript, or terminal byte.
For this transcript, it adds no missing output or model behavior. It expands the existing control-before-model-feed timing window.
Sol reads the plain worker's control-first loop, read_pty_chunk, and on_output paths to check that mechanism.

The trace shows an earlier one-byte read, then 42 held at 1791762509598 and released at 1791762510598 by worker pid 3553.
It retains actual release of the byte; the scratch proof does not suppress it permanently.
The trace has no ReadScreen-answer timestamp. The unchanged-revision failure and source order support an answer before that delayed feed.
That answer ordering is an inference from the result and source, not an independently timestamped trace event.
Sol accepts this exact-signature controlled proof under the lead's case-specific 23bl(A) decision.
Sol confirms non-interference: #288 changes no model feed, program-control path, transcript, or model-read fence; this transcript calls no oracle.
This receipt claims neither an uninstrumented-v1 failure nor a recovered trace of the original gate attempt.

Sol reads full matching package receipt `843604d707cfb4a6a9049e54da46fba2ff4a001d`, `verdicts/rc-real-controls.md`.
Its controlled-proof acceptance has the same exact source, hashes, signature, and limits.
The lead explicitly accepts that record and assigns the future Core .42 pin work to RC.
Sol reads the complete named repair diff at contracts `72eb31ba198c39cb0a8510978749262d7863e91a`.
It replaces both ST-6 await_quiet steps with Output Activity waits, at clocks 5000 and 5001.
That source repair is identified, but this receipt accepts no future Core pin, import, or repair execution.

The lead moves SLOT through #294, then gives #288 one import of v1 `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Sol reads that exact merge: it is P6's lists-merge-base PR and its Cargo.toml retains contracts .39.
The #294 import does not contain the future ST-6 fence repair. No .42 repair import is inferred from the slot sequence.
If the imported source leaves this cause unchanged, plan 23an(a) still requires this accepted attribution and explicit lead case decision.
The next gate must use that new head and base, with the applicable check and both required delta receipts.
This receipt supplies Sol's controlled-attribution acceptance for that sequence. It authorizes no same-head re-gate.
The new-head source/import review, completed final gate, final body, and final package verdict remain due.
Sol runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base check. Never poll.

CONTROLLED ATTRIBUTION: CLEAN within the exact case and limits above.
VERDICT: NOT CLEAN (1 open final acceptance condition: new-head import review, completed final gate, final body, and final package verdict) for #288; no final merge acceptance

## Round 7 — Q7 source carry after #294

Exact head: `09e9fbc8b26456be592ce99d9814fcea4695536e`.
Base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Tree: `e4e38e9184c645d9e78db8b66aa490ca001a7b35`.
The parents are reviewed 46c8c548 and the new base. RC receipt: `msg_plugin-w_1791762717_e1d2ee`.
Sol reads the complete canonical PASS record at `~/botster-sessions/gates/botster-core-rc-oracle-base-merge-check-09e9fbc8-20261011-165140.log`.
SHA-256: `068875fa93cfda316715a68cfdf34ac50548ec703787c341a756f3e12e32f6f3`.
It names this exact head/base and passes both ancestry conditions and all four Q7 conditions.
No new source delta round is required under that rule. The completed final gate still must use this head.

Sol independently confirms that the base changes only xtask/src/fsutil.rs and xtask/src/lists.rs.
Neither path overlaps any of the seven own paths. Both imported blobs equal the new base.
All seven own blobs, including both pending files, equal the reviewed head exactly.
The non-list own patch equals the earlier patch byte for byte: 42637 bytes.
Patch SHA-256: `0b0844ddb5606e1bf9f7823152d1757b9c5b7eef90dbd0fa127925b3538123b1`.
The reviewed oracle source, corrected tests, four pending moves, fence comments, and inherited exclusions retain their accepted source scopes.
This static import review runs no executable check and grants no approval of unrelated future gate changes.

The imported #294 source retains .39. It does not remove the ST-6 fence gap.
Round 6's controlled proof and non-interference therefore retain their unchanged-cause scope on this import.
The lead's explicit case acceptance supplies the applicable 23an(a) disposition. No repair import is falsely claimed.
The future R-80/.42 fix remains assigned as recorded in Round 6.

Sol reads the retained strict log through its completed summary and all four moved-ID ok records.
Log: `~/botster-sessions/gates/botster-core-rc-oracle-strict-09e9fbc8-20261011-165122.log`.
SHA-256: `d258fa892eb20004daf54787319ef882f510963b289bd643824e2f51a9f3074b`.
Its header names this head and BOTSTER_SEEDS=0–31/BOTSTER_PENDING_STRICT=1.
It retains a 245-PASS, zero-failure summary, plus all four moved identifiers passing.
The same file begins with a refused invocation because NEXTEST_TEST_THREADS is unset.
Sol preserves that refusal separately; it is not a PASS check.
The retained file does not show the full command/provenance of its subsequent successful invocation.
Sol asks RC to clarify that command without running again. This local record does not replace the required final gate.

Sol reads matching package import receipt `5c4048614f24c313c8e5edd06e17ca50cdb19d76`, `verdicts/rc-real-controls.md`.
Its Q7 scope, retained .39 pin, strict-log refusal, and final-evidence conditions agree.
Source CLEAN carries to this exact head under Q7. Zero source or body finding is introduced by this import.
The completed exact-head final gate, final body, and final package verdict remain due.
The earlier RED gate and accepted controlled proof remain explicit; neither is a green gate for this head.
Sol reads no running output and runs no execution job. Never poll.

SOURCE CARRY: CLEAN at 09e9fbc8b26456be592ce99d9814fcea4695536e against base 3aacc58d086e55a6b186165ccf9001e2618e2869.
VERDICT: NOT CLEAN (1 open final acceptance condition: completed exact-head gate, final body, and final package verdict) at that head/base; no final merge acceptance

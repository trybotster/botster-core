# Core #285 — scripted output and input blocking controls

## Round 1 — source and supplied evidence, 2026-10-11

Head: `b5ca83e2825dc9404a3c6634fd97d755cf8f5d37`.
Source ancestor: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Gate comparison base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Tree: `8aca56a61879ef60a93d04b7a4f41648979c7e5d`.
Risk tier: HIGH. The PR changes the shared testkit crate.
Package verdict: NOT CLEAN, four open, in `cdd198fbf2ae7386c8839e42810758a7a5c8fa50`.
Sol reads the complete #285 package sections, all seven own source deltas, affected context, and the complete current body.

### TC-SI-1 — MEDIUM, open: private-state assertions

This finding agrees with package RC-TK-1.
The new `pty_output_ignores_scripted_and_refuses_any_other_unknown_field` test asserts `unread(&harness)` as 0, 1, and 0.
The helper finds the worker row and calls `ProgramControl::output_unread`.
That accessor returns `self.lock().unread` from the private program record.
The accessor does not make the assertion an observation of public Core behavior.
The binding review rule forbids private-state assertions.

Replace these assertions with observations through the Core API or route frames.
Prove that the refused unknown field has zero output effect.
Prove that the accepted `scripted` request delivers its bytes.
Keep the public terminal-mode assertion, which checks that the script changes no mode.
Do not replace the assertions with another private counter or accessor.

### TC-SI-2 — MEDIUM, open: an unbounded read loop

This finding agrees with package RC-TK-2.
The new real `input_blocked_reports_a_stream_that_takes_no_more_client_bytes` test loops while `got.len() < VOLUME`.
On `WouldBlock`, it calls the control and retries without a progress bound.
On `Ok(0)`, it adds no bytes and repeats indefinitely.
A stopped flush or premature EOF can therefore prevent the test from terminating.

Fail on EOF before the expected volume arrives.
Bound attempts that make no read progress within this test.
The same test owns both socket ends and can drive their reads and flushes directly.
Do not add sleeps or a polling wait for external progress.
Keep the exact byte-order check, final readiness check, and closed-end error check.

### TC-SI-3 — LOW, open: evidence cause, provenance, and trial count

This finding agrees with package RC-TK-3 and adds a trial-count check.
The body says the cause of the withdrawn `114153` result is unknown.
The implementer reports a stale binary from another worktree in `msg_plugin-w_1791745887_85d3e0`, under catalog B13.
Record that reported cause and identify the clean #285 build used for the replacement Mac evidence.
Keep the withdrawn log explicitly invalid.

The body also says the real `in_8_no_recheck_after_start` timeout occurs in two of two runs.
The cited replacement log records one trial: seed 0, step 9, `step_timeout`.
Identify the second valid trial after the clean build, or correct the count.
Do not use the withdrawn log to supply a second valid trial.
This finding does not require removing the real-pending entry.

### TC-SI-4 — LOW, open: the historical gate description

This finding agrees with package RC-TK-4.
The body says all other steps passed in the red `b1f4fb3a` gate.
Its raw summary reports eight PASS steps, `mutants FAIL`, and `fuzz NOT RUN`.
Replace the claim with those actual results.
Keep the later green gate distinct from this red gate.

### Integration checks and limits

The shared `PtyOutput` parser names `scripted` as the one ignored field and retains `deny_unknown_fields` for other fields.
Both tiers use that parser. The ignored script does not set terminal modes.
The testkit `input_blocked` control flushes and runs ready work before it reads client stream room.
The real control flushes, checks unsent bytes, and performs a zero-wait socket poll.
Its `revents` check decides writability. A closed client end returns an error.
These controls report the stream boundary. They do not model worker queue admission or clause completion.
The testkit socket test checks blocking, release, and ordered byte delivery.
The real socket test checks those behaviors, subject to TC-SI-2.

The mutation configuration is unchanged. This PR adds no exclusion for either control.
The branch predates two xtask test fixes on the comparison base.
Those reverse differences appear in `mutants_cited/tests.rs` and `signals.rs`; they are not own source changes.
The final import must retain the base fixes and receive the applicable check and review.
The current Core pin stays at `.36`.

The pending change removes five identifiers from testkit pending.
The replacement Mac record reports each identifier PASS on both tiers.
It reports testkit PASS and a real step-9 timeout for `in_8_no_recheck_after_start`; that identifier remains real-pending.
The bytes-and-text minimum identifier remains pending and fails at step 16 on this branch alone.
The body assigns its barrier correction to #284 and its transcript correction to the later Contracts work.
Sol accepts no later pin, pending removal, or final import in this round.
Sol does not claim an exhaustive independent prior-art review or validation of the body's scratch-composition table in this NOT CLEAN round.
The body's 016c audit and uncited local checks remain supplied reports.

### Supplied execution records

Current gate: `~/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-b5ca83e2-pool-20261011-121951-73258.log`.
SHA-256: `672963d6fa917d063d5d64288512010634fae879c60164614a6777869997b7cd`.
The header names the exact head and comparison base. All ten steps PASS in 237.7 seconds.
The job reports 248 seconds, with one second queued and 247 seconds running. The wrapper exits zero after 248 seconds.
These timings measure different intervals.
Default: 1755 PASS, 468 skipped, 12.730 seconds. Slow: 565 PASS, 2027 skipped, 41.114 seconds.
Minimum counts: testkit 61/69, real-passing 49/68, real-accepted 50/69.
Stage 1 reports six mutants: two caught and four missed, with zero timeout or unviable results, in 18.6 seconds.
Stage 2 catches all four remaining mutants in 50.7 seconds. Its baseline passes in 46.4 seconds.
Final result: six caught, zero missed, zero timed out, zero unviable.
Sol independently compares all six raw catches with their cited catcher identities; they match without duplicates.
The real `input_blocked` mutation is caught by the new socket behavior test.
These selected mutations do not prove that its no-progress loop terminates.
Fuzz selects no changed decoder crate. The complete log contains no FLAKY token.

Earlier list gate: `~/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-5c34d956-pool-20261011-115635-78231.log`.
SHA-256: `40c74bb79910b6b45236f66aefea677efce5b21844342855ee1705683068ce57`.
Sol reads its summary and the named failure. Fmt, clippy, and taint PASS; lists FAIL; six later steps are NOT RUN.
The bytes-and-text identifier fails at step 16. The step total is 81.3 seconds; the wrapper reports 147 seconds.

Earlier mutation gate: `~/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-b1f4fb3a-pool-20261011-120615-12620.log`.
SHA-256: `46fc876b468b29da43eb348e9fb73d2bf75d4f91f2d690fa64ccf27111218cc4`.
Sol reads its stage results and final summary. Eight steps PASS; mutants FAIL; fuzz is NOT RUN.
Stage 1 has ten mutants, two caught and eight missed. Stage 2 catches six and misses two.
The baseline passes. Both stages report zero timeout and zero unviable results.
The current source removes the redundant poll-count checks that the body identifies as equivalent mutations.
The step total is 486.3 seconds; the wrapper reports 497 seconds.

Replacement Mac record: `~/botster-sessions/gates/botster-core-rc-testkit-in-ids-evidence-mac-b1f4fb3a-20261011-120503.log`.
SHA-256: `9ccf83da33d7df8f191a53ade096ca663f8115a1c83710419496d55ec892dfa5`.
Sol reads all 89 lines. The header names short head `b1f4fb3a` and Darwin 27.0.1.
Five removed identifiers PASS once on each tier.
The no-recheck identifier passes on testkit and is INCONCLUSIVE on real at seed 0, step 9.
The bytes-and-text identifier fails on testkit and times out on real at step 16.
After the `18dc32c4` reversal, all seven identifiers fail on both tiers because `scripted` is unknown.
The reported clean-build provenance still requires the body correction in TC-SI-3.
Sol accepts no result from the withdrawn `114153` log.

All four findings must close before CLEAN.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable probe. Never poll.

VERDICT: NOT CLEAN (4 open) at b5ca83e2825dc9404a3c6634fd97d755cf8f5d37

## Round 2 — correction and completed gate, 2026-10-11

Head: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Tree: `e51e0f514763051e806e2ea274b55dd0402284fc`.
Source ancestor remains `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Comparison base remains `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Sol reads the complete three-file correction, affected helpers, full current body, and complete package Round 2.
Package `2de19afa5930a0763684070734c6020c471ce5a4` gives CLEAN with zero package findings.
Sol independently finds two remaining body defects. No new product-source defect is established.

TC-SI-1 closes. The new test uses Core's ReadScreen result after each control and a bounded settle.
The refused request has no visible x. The accepted request has visible x. The public terminal-mode check remains.
The new test reads no private unread counter. The unchanged older test lies outside this delta.

TC-SI-2 closes. The real socket test fails on premature EOF and on a second consecutive WouldBlock read.
A successful read resets the progress check. Each such read adds at least one byte toward the finite volume.
The test drives both local ends and adds no sleep or external wait.
The exact byte-order, final readiness, and closed-end error checks remain.

TC-SI-4 closes. The historical b1f4 gate paragraph now lists eight PASS steps, mutants FAIL, and fuzz NOT RUN.
The later green gates remain distinct. The body also separates step totals from wrapper times.

### TC-SI-3 — LOW, still open: unsupported reproduction claim

The replacement's clean-build provenance is now stated. The trial-count issue closes.
Sol reads all three new Mac trials; each is INCONCLUSIVE at seed 0, step 9.
The body and pending comment cite three logged trials. The body separately names the earlier valid trial.
That supplemental log names b5ca83e2 with local test edits, not the committed correction head.
Its clean and exclusive-writer statements are owner provenance reports.

The body now claims that the cited 120659 log reproduced the B13 cause of the withdrawn scratch PASS.
Sol reads that complete log. It shows #285 failing at step 16 before and after the #284 build command.
It shows the same artifact path for both worktrees. It shows no changed result, dep-info analysis, or scratch PASS.
It therefore does not prove the claimed stale-binary result or the specific freshness cause.
Label the cause as the owner's reported diagnosis and state this log's actual evidence limit.
Do not claim that this log reproduces the withdrawn result. Keep the withdrawn log invalid.
No new execution is required to close this record defect.

### TC-SI-5 — LOW, open: incorrect prior-art absence claim

The body says the Contracts botster-fake-route has no input_blocked control.
Both contracts-v0.1.36 and contracts-v0.1.40 list that control in controls.rs:31 and dispatch it.
At .40, input.rs:42 implements it as unread input plus refusal to read more input.
The fake's can_read_input uses its route queue allowance, reserved exceptions, PTY block, and input queue allowance.
That behavior differs from Core's client stream-room and socket-readiness observation.
Correct the absence claim and explain why this fake behavior is not suitable for the new Core control.
Sol also reads the named old-Core scripted plugin worker and its test-support exports.
Those scripted plugin load and panic cases do not supply a comparable Core stream control.

### Corrected-head execution records

Gate: `~/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-6363d559-pool-20261011-123453-16073.log`.
SHA-256: `666c16da530ecd29c592ca7f9d0b6655f2cdd23915e5fc6e109ec3d9bf17d0aa`.
Sol reads the exact head/base header, tier and stage results, all catches, summary, and exits.
All ten steps PASS in 243.3 seconds. The job and wrapper exit zero after 252 seconds.
The job queues for one second and runs for 251 seconds.
Default: 1755 PASS, 468 skipped, 12.743 seconds. Slow: 565 PASS, 2027 skipped, 41.819 seconds.
Minimum: testkit 61/69, real-passing 49/68, real-accepted 50/69.
Stage 1 has six mutants: two caught, four missed, zero timeout or unviable, in 20.2 seconds.
The stage-2 baseline passes in 49.9 seconds. Stage 2 catches all four remaining mutants in 50.9 seconds.
Final result: six caught, zero missed, zero timeout, zero unviable.
Sol independently compares all six raw and cited catches; they match without duplicates.
Both input_blocked arm deletions have named behavior-test catches.
The mutation configuration remains unchanged. Fuzz selects no changed decoder crate. The full log has no FLAKY token.

Three-trial Mac log: `~/botster-sessions/gates/botster-core-rc-testkit-in8-norecheck-real-mac-b5ca83e2-20261011-123346.log`.
SHA-256: `dab71ff0b4731de7b63ec025aaf2e5ccf8868577acaad96510b3823a5ebcdc29`.
Shared-target log: `~/botster-sessions/gates/botster-core-rc-shared-target-repro-mac-20261011-120659.log`.
SHA-256: `0e3a06a20f8128615a2eba791451a1cde3ba4bc19092812fef8e742df9102f6c`.
The real-pending entry remains justified by its valid timeout. No later pending removal or pin is accepted.
The final import must retain the two xtask base fixes and receive the applicable review/check and final gate after SLOT.
Sol sends both remaining body findings to RC and the package reviewer. Sol runs no execution job. Never poll.

VERDICT: NOT CLEAN (2 open: TC-SI-3 and TC-SI-5) at 6363d55936ff193c2c1164943b80a3f1adc90ada

## Round 3 — body correction, 2026-10-11

Exact head remains `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Comparison base remains `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Tree remains `e51e0f514763051e806e2ea274b55dd0402284fc`.
Sol reads the complete body-only delta and complete package Round 3 in `71c1dc82a0461a65c1877807f3ed1deea6212ebe`.
The package gives CLEAN with zero open findings on this exact source.
GitHub confirms the unchanged head and comparison base.
Corrected body SHA-256: `852aa019dfb9ccb68cecb331eedbb8635734a2a5b7c1b5d01bbebd4ed5c4c2d3`.

TC-SI-3 closes. The body labels the B13 cause as the owner's reported diagnosis.
It states the cited log's FAIL, shared artifact path, and FAIL results and their limits.
It claims no reproduction of the withdrawn PASS or proof of the dep-info choice.
The invalid log, replacement provenance, and logged trial counts remain explicit.

TC-SI-5 closes. The body identifies FakeRoute's input_blocked at .36 and .40 and explains the rejected admission-state behavior.
The Core control continues to observe the client-side stream boundary.
TC-SI-1, TC-SI-2, and TC-SI-4 remain closed as recorded in Round 2. Zero integration findings remain.
No source, pin, pending identifier, exclusion, or test changes in this body correction.
The completed exact-head green gate remains applicable. No repeat execution is needed for this body correction.

The complete own source review, correction review, original Mac parser and reversal results, and corrected Linux gate retain their stated scopes.
The new three-trial Mac record is supplemental evidence with local test edits; it is not a committed-head gate.
Uncited local checks and the scratch-composition table remain owner reports, not independent execution proof in this verdict.
The five removed identifiers have both-tier replacement evidence. The no-recheck and bytes-and-text limits remain pending.
This source CLEAN accepts no later pending removal, pin, import, or PR completion.
The final import must retain the two xtask base fixes and receive the applicable check and review after the lead's SLOT.
The final exact-head gate and final body remain due under that rule.
Sol runs no build, test, gate, mutation, reversal, or measurement. Never poll.

VERDICT: CLEAN (0 open; source and supplied evidence before final import) at 6363d55936ff193c2c1164943b80a3f1adc90ada

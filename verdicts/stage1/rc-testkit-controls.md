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

## Supplemental B13 evidence receipt — 2026-10-11

Exact #285 head remains `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Sol reads both complete reproduction logs, the updated B13 catalog entry, and the complete updated pair-common rules.
These logs test the shared-target hazard. They are not acceptance gates for either PR.
Both headers state Cargo 1.92.0 and the shared target `~/botster-sessions/targets/rc`.

Same-pin log: `~/botster-sessions/gates/botster-core-rc-b13-repro-samepin-mac-20261011-124836.log`.
SHA-256: `a95f272f251856807d5ded619614244fe4aab3c5d484e9999ce53b9ca92dfcaa`.
The header identifies #285 at the exact head above and #284 at `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
Both worktrees use contracts .36. Each has a local, uncommitted pending-list edit.
After a clean, #285 compiles and fails at seed 0, step 16.
The next #284 command has no Compiling line. It runs the same `conformance-30f1f7d856298029` artifact and fails at step 16.
After another clean, #284 compiles and fails at seed 0, step 9.
That failure reports ``unknown field `scripted`, expected `session` or `bytes_hex` ``.
The changed failure demonstrates reuse of the wrong test result before the clean.
The log also records absolute dep-info paths into the #285 worktree for the test source and included conformance lists.
The log does not trace Cargo's internal freshness decision. That detailed causal account remains an owner inference.
The log demonstrates a wrong FAIL. It demonstrates no false PASS or prebuilt candidate-worker result.

Negative-control log: `~/botster-sessions/gates/botster-core-rc-b13-repro-4step-mac-20261011-124502.log`.
SHA-256: `598043bcc5ee6342a5c1e1eccc3ee31ecddb2ed0cf78cb31b2f172520b73a2a2`.
The scratch head is `b4626b848cabb52343242675c1131d832020e3b8`, with the stated #284/#285/v1 composition and contracts .39.
The header states that commit `17ff45db` removes the identifier from pending in that scratch composition.
After the #285 failure, the scratch command compiles and runs `conformance-c0cedb3ddc2850f2` without an intervening clean.
It passes once. After a clean, the scratch command compiles again and passes once with the same artifact name.
The different pin produces a different artifact name in this record. The log's expected-hazard label is not its actual result.
This control does not establish a general rule for every Cargo.lock change.

This receipt adds stronger B13 evidence. It preserves the earlier record's limits for the earlier 120659 log.
Neither new log reproduces the original withdrawn scratch PASS. That withdrawn evidence remains invalid.
The #285 source CLEAN in `7009eba6960ba4e260f1026c12015e40e612585b` remains unchanged with zero open integration findings.
The #284 source CLEAN remains unchanged. These local pending-list edits authorize no pending removal or import.
The final SLOT, applicable import review and check, exact final gate, and final body remain due.
The future GS structural refusal remains unimplemented and unaccepted in this review.
Sol runs no build, test, gate, mutation, reversal, or measurement. Never poll.

VERDICT: CLEAN (supplemental B13 evidence within the stated limits; source verdict unchanged) at 6363d55936ff193c2c1164943b80a3f1adc90ada

## Round 4 — final import and body correction, 2026-10-11

Head: `3cf460714016af6683ea6c60f1753dfc0055c660`.
Base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Tree: `886ac9ddad72fc5b92a0c66646aa964965a1b00c`.
Previous reviewed head: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Previous source ancestor: `f49f74302526a28b6aced1595f74c7d0a1d22836`.

### Explicit import review

The new base merges #284. Sol verifies its parents and that its tree equals #284's reviewed and gated tree.
That tree is `5b13e06270096dfad3b73ea8cf87136c26da0261`. This receipt completes #284's merge verification.
The imported base also retains #282, #283, and both earlier xtask test fixes.
Sol reviews merge `9d4c7d074044e4268489dd8755115e841d94e7f6` and the subsequent pending change.
The previous own delta has seven files. The imported base delta has 24 files.
Five paths overlap: both pending files, `pty_controls.rs`, its tests, and `real/route.rs`.
All five own code files preserve their previous added and deleted lines against their respective bases.
All 335 tree entries outside the overlaps match the previous own source or the new base, as applicable.
The subsequent commit changes only the two pending files.

Sol reads the affected control, parser, route, and test-helper context.
The shared `PtyOutput` parser retains the one ignored `scripted` field and refuses other unknown fields.
The real `input_blocked` control retains the stream writability check. It changes no route readiness descriptor or read deadline.
The imported R-68 readiness method continues to expose the open client stream and returns none for a gated or closed end.
The testkit stream control still flushes and runs ready work before it observes stream room.
The merged settle helper retains all pump events for #283's held-write proofs; the existing #285 tests remain intact.
The real socket test retains its EOF failure, no-progress bound, byte-order proof, and closed-end check.
No own production code, private-state assertion, terminal encoder, process owner, or mutation exclusion is added by this import.
F-283-1 remains the separate follow-up for the imported assertions in `program.rs`.

The pending-file conflict resolution preserves all base text and the exact own `in_8` comment and identifier group.
The own group precedes both imported #284 groups. No comment or identifier from those groups is lost.
Both merged identifier sets equal the new base minus previous own removals plus previous own additions.
The final delta removes seven testkit identifiers and adds two real-pending identifiers against the new base.
Those seven comprise the five previous both-tier identifiers, `in_8_no_recheck_after_start`, and the new bytes-and-text identifier.
The two real-pending additions are the previous no-recheck identifier and the new bytes-and-text identifier.

Canonical check: `~/botster-sessions/gates/botster-core-rc-testkit-base-merge-check-3cf46071-20261011-143954.log`.
SHA-256: `5146a7fd1e573e691bf1ecf8ecd5de8cab16a00b90d60e815f98b26b3aa69b86`.
Sol reads the complete check. Ancestry passes. All four conditions fail, including the pending-file conflict. The command exits one.
The body contains the complete output exactly. Both explicit delta reviews are required; Sol grants no automatic Q7 carry.

### New pending move and local evidence

Sol reads the bytes-and-text transcript and its exception at contracts commit `ddce507e2c08e740f899e97dcfec566f3d2275b8` (.39).
The exception names raw step 13, `no program_ready, R-60`.
The new comment retains that fence gap and P3 owner despite the single local real PASS.
The identifier leaves testkit pending and enters real pending. The partial-paste session-ended identifier remains testkit-pending.

Sol reads all three supplied local records in full. Each is a filtered record, without complete build output.
Local check: `gates/botster-core-rc-testkit-local-check-mac-9d4c7d07-20261011-143602.log` under `~/botster-sessions`.
SHA-256: `e6c4b7e8166513c2f629fe558f00a140c0b0ff3998d01a168040d10267f0f633`.
It reports 397 testkit passes and 192 worker passes. Its strict check uses seeds 0-31.
The strict check fails because bytes-and-text passes while pending: 229 passes, one pending-status failure, 460 ignored identifiers.
Real record: `gates/botster-core-rc-testkit-dp5-bytes-text-real-mac-9d4c7d07-20261011-143826.log` under `~/botster-sessions`.
SHA-256: `75d7967bd5c7117987f0d4434a3a44160e884f8629c6a9eb75d215c5ee6db50b`.
It names the merge head plus a local pending edit and the slow suite's default seeds. One test passes.
Strict correction: `gates/botster-core-rc-testkit-strict-mac-20261011-143935.log` under `~/botster-sessions`.
SHA-256: `7c1fff47cb116b2c24567b3d3db5024e1cb453f281e1c009012c67511e20a9d5`.
It names the merge head plus a local list edit: 230 passes, zero failures, 460 ignored identifiers. Lists passes.
The body identifies this uncommitted edit as the later committed list change and states seeds 0-31.
Sol does not label that local record a committed-head gate.

RC reports a target clean at 14:36, with 3.9 GiB removed, and only this worktree's builds thereafter in `~/botster-sessions/targets/rc`.
The record headers support that reported single-worktree sequence. Sol does not claim independent raw clean verification.
The prior B13 reproduction retains its previously reviewed limits. The withdrawn scratch evidence remains invalid.

### TC-SI-6 — LOW, closed: superseded all-seed claim

The initial final-slot body still claims partial-paste session-ended PASS at every seed with .39 and R-82.
It also states that the identifier waits only for the .39 pin, which is now in the imported base.
The #284 dfc38b47 gate disproves that all-seed claim at seed 10, step 15.
Sol requests a body correction in `msg_plugin-w_1791754945_d1855c`. Package RC-TK-DELTA-1 identifies the same condition.
The corrected body labels the historical scratch result as one run with the default seeds, not seeds 0-31.
It preserves the current seed-10 testkit failure and seed-0 real failure, pending status, undiagnosed cause, and RC owner.
It removes the future-pin fix claim. No new execution or source change is required for this correction.
Sol verifies the unchanged live head through the GitHub CLI fallback after GitHub MCP fails.
Corrected body SHA-256: `4200ab5e366eeec5e0b968eb8403f8d4334818800676bb611c8a0b8b603c3112`.
Only the Stop section changes after the complete body read.
Sol reads the complete package import section at `5480a940abc4fde52a9c2d656b0de559f5a56de4`.
Sol also reads the complete correction section at `5c202137371c17567dd3c5eec75fbb1ba25c3061`.
Package findings now total zero. The completed exact-head gate and final body gate result remain due.

### Remaining final acceptance condition

TC-SI-FINAL-1 — MED, open: completed exact-head final gate, final evidence body, and matching completed package verdict remain due.
The source delta adds no integration source finding. TC-SI-1 through TC-SI-6 remain closed.
This receipt accepts no running or future gate result. RC reports that the gate waits for its separate Contracts job.
Sol requests no extra local run. Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check script.
Never poll. Wait for the doorbell.

VERDICT: NOT CLEAN (1 open final acceptance condition) at 3cf460714016af6683ea6c60f1753dfc0055c660

## Round 5 — red final gate and retry attribution, 2026-10-11

Head and base remain `3cf460714016af6683ea6c60f1753dfc0055c660` and `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The source review and TC-SI-6 closure remain unchanged. TC-SI-FINAL-1 stays open.

Gate: `~/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-3cf46071-pool-20261011-144458-14865.log`.
SHA-256: `b0b4f23f65cfef7cc7267e109fb7aff551934dc54a3699a203c149a41ec69206`.
Sol reads the exact header, suite summaries, raw baseline failure, mutation-stage result, and final exits.
Eight steps PASS. Mutants FAIL. Fuzz does not run.
The step total is 263.0 seconds. The job queues for one second and runs for 284 seconds.
The job and wrapper exit one after 285 seconds on msa1.
Default: 1788 PASS, 460 skipped, 12.420 seconds. Slow: 584 PASS, 2019 skipped, 42.050 seconds.
Mutation stage 1 reports six mutants: two caught, four missed, zero timeout, zero unviable, 35.9 seconds.
The unmutated stage-2 baseline reports 583 PASS and one failure, with 2725 skipped tests, in 42.634 seconds.
Its baseline interval is 57.5 seconds. Stage 2 tests no mutant because that baseline fails.

The failing identifier is `conf::ev_5_route_closed_retires_route_keyed_events`, seed 0, raw step 28.
It expects `activity_before_close: true, closed: true`. It receives `activity_before_close: false, closed: true`.
The same real test has normal PASS in the earlier slow step. This is not a caught mutant or a green final gate.

Sol reads the complete .39 transcript. Raw step 24 sends one byte on r2; step 25 calls `await_quiet`; step 26 begins Detach.
Step 27 waits for the close report. Step 28 checks that the client activity preceded the close.
No step fences the worker's receipt of that byte before Detach.
Static source establishes an unfenced window already present on the imported base.
The PR changes no worker, host, `route_reports`, or transcript behavior on this path.
The real stream's write and flush remain unchanged. The transcript calls neither new control.
Sol confirms this non-interference. The source supports the proposed pre-existing cause but does not prove an executed plain-v1 reproduction.

RC reports conditional lead approval for one same-head re-gate in `msg_plugin-w_1791755842_f1ace6`.
The lead handoff records the approval, P5 ownership of the rule-9 record, and a stop if the re-gate is red.
Plan 23al, section 8, still requires trace evidence and the exact failure on plain v1.
Plan 23bl permits an accepted controlled reproduction of the same identifier, step, and symptom.
The named gate runs on #285. No current plain-v1 or accepted controlled reproduction has been supplied.
Sol requests the existing reproduction record or an explicit decision on this missing condition.
Sol does not attest to a reproduction that it has not read.

Sol also reads the older P3 README, complete scratch patch, and complete five-run raw trace in `evidence/p3-ev5-route-closed/`.
That record uses .36 on `850bb785`, with local diagnostic instrumentation, and reports leaked Activity before retirement.
Its symptom differs from the current .39 `activity_before_close: false` failure.
It supports the limits of `await_quiet` but does not satisfy this retry's exact-signature condition.
Raw trace SHA-256: `78842f3507919ac722fa77ed7eaa815f901d11c6fc735994d7c9b60a1622aff8`.
Sol accepts no same-head retry result until the applicable attribution conditions are resolved.
Sol runs no build, test, gate, mutation, reversal, measurement, or diagnostic. Never poll.

VERDICT: NOT CLEAN (1 open final acceptance condition) at 3cf460714016af6683ea6c60f1753dfc0055c660

### Lead correction received

Lead message: `msg_plugin-w_1791755967_93a1c6`.
The lead confirms that no exact-signature plain-v1 failure or accepted controlled reproduction exists.
The earlier conditional approval is withdrawn; the 23al and 23bl conditions remain binding.
The lead directs RC to make one pool evidence reproduction on plain v1 `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The scratch change must alter timing only and include a trace of the exact step-28 signature.
Both reviewers must accept that record before any re-gate of the same head.
If the controlled run does not reproduce the signature, no re-gate is allowed; the lead decides the next step.
Sol's non-interference line remains recorded. This receipt accepts no future reproduction or retry result.
Package receipt `a5f38a5e760604e694932bd603f295123def08ba` reports the same corrected condition.

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

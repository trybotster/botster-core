# P5 review — #305, Core owns stop_grace

## Round 1 — 2026-10-11

- PR: <https://github.com/trybotster/botster-core/pull/305>.
- Head: `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`.
- Tree: `2738e82df7c461787f0ac6df736485b2a279adc3`.
- Base and merge base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Parents: `1cd6a3696ef3832e8e7145eab90412d19565b1d6` and the base above.
- READY: `msg_plugin-w_1791782423_d33864`.
- Risk: HIGH, rule 3, shared crates and both tiers.
- Integration reviewer: Sol-C, session `sess-1791781343-0185-0b0f5f6ddc65cfcfbf7326db792a6d82`.

I read the complete 22-file diff, affected machine and edge contexts, the approved design note, and the corrected PR body.
The diff has 251 additions and 206 deletions. Its full-index SHA256 is `b8e729677962685c4d19d498e7ee8f687eccddf5ee047e83770158bf0b644e7d`.
The Git whitespace check reports no error. Every own patch retains its added and removed lines after the base import.
All conformance files equal the base files. The contracts pin and public facade remain unchanged.

### Source and contract

I read the R-97 follow-up at contracts commit `12ebec28f9e00c6b84d3af817ab1e5e4df96461d`.
The lead handoff explicitly approves this design after the steward corrects the broken-link kill route.
Core owns `stop_grace` on the injected clock. The worker carries out Core's kill order and keeps no grace timer.

At Stop, the existing host path sends `EndPayload` when it has no link.
At the due pump, the host sends `HostMsg::Kill` with a link or `GroupSignal::KillPayload` without one.
The system edge verifies the worker's pid and start time before sending `SIGUSR2` to that process alone.
Core never signals a bare payload group or kills the worker on this path.
The host test checks EndPayload at Stop, no kill at 99 milliseconds, one KillPayload at 100 milliseconds, and no repeat at 600 milliseconds.

The real driver registers `SIGUSR2` and translates it to `Input::KillPayload`.
The testkit uses a separate pending flag and translates it to the same input on the shared machine.
The worker removes its grace field, duration, timer handler, and launch-field read.
Its remaining deadlines are orphan expiry and candidate-hello expiry.
The linked Kill and signal KillPayload share the group-kill and reap path.
The worker holds the leader unreaped until the group kill and exit/drain conditions permit reaping.
Repeated KillPayload is inert. A kill during spawn takes priority over the graceful request when spawn succeeds.
An ended leader still permits the group kill before reaping. The worker retains its final model and deferred exit report.

The worker test advances its clock beyond twice the default grace and observes no timer action after EndPayload.
It then requires an immediate kill from KillPayload and checks repeats, exit reporting, and reaping.
Other tests cover a previously linked Stop, an ended leader, pending spawn, and testkit signal translation.
The real signal test sends SIGUSR1 and SIGUSR2 consecutively, then checks the signal-9 exit and worker survival through Remove.
It does not separately observe the payload between those signals. The worker-core test provides the separate no-timer proof.

`LaunchSpec.stop_grace_ms` is removed from the wire structure and its constructors.
The new decoder accepts an older launch containing that key. New encoding omits it.
The old structure has a serde default, which supports the reverse decoding direction by source inspection.
The compatibility test does not execute an older decoder or worker binary.
Core Amendment 6 defers the previous released worker case until protocol 2; this review fabricates no previous protocol.
The remaining similarly named fixture parameter configures the independent test-process guard, not the worker timer.

The mutation configuration changes only comments. The process-check allow list renames the existing real test.
Neither change adds a mutation exclusion, timer allowance, or process site.
The design documents describe the two distinct worker signals and the Core-owned deadline.

### R305-F1 — LOW — evidence wording, closed

Finding messages: `msg_plugin-w_1791782559_2fbf93` and `msg_plugin-w_1791782663_df9d95`.
I requested precise scope for the consecutive real signals, aggregate suite times, and comparison diagnostics.
Owner correction receipts: `msg_plugin-w_1791782589_1befae` and `msg_plugin-w_1791782699_ff9288`.
I read the complete corrected body on the unchanged head.
It no longer claims a separate real observation after SIGUSR1 or infers each transcript's waits from aggregate time.
It states that 342 failure identifiers match, 317 normalized diagnostics match, and 25 other failures have no failing step.
It records the original awk parser's 580/603 limit. R305-F1 closes before this package round is published.

I read Sol-C's complete initial record at `2a8700f3dab46ead3c7db3aa13dfd43e199e48d9`.
It finds no separate code defect and remains NOT CLEAN only for this finding and the missing package receipt.
This package verdict supplies the closure evidence. Sol-C must publish its own updated disposition.

### Local comparison and checks

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/p5-stop-grace/`.
I read the comparison script, complete comparison log, current checks script, and complete current checks log.
The comparison script uses one physical worktree, a logged target clean, seeds 0–31, temporary pending-list removal, and list/lock restoration.
Its header states that only this worktree writes the target during the comparison.
The log records a clean of 18859 files and 5.7 GiB, both exact source heads, and a clean restored branch.
The run compares base `e4f495040a80b524a7ac9accba95272d5c2594fa` with own commit `b07b34acf6839432996c36888dcbf9850cdb3e4d`.
The later own commit changes only the mutation comment and allow-list test name.
The comparison is earlier implementation evidence, not a full-corpus run on the current imported head.

Both raw summaries report 261 passes, 342 failures, and 88 ignored results.
I independently match all 342 failed identifiers and all 88 ignored identifiers.
I match all 317 normalized diagnostic records to each raw file and between the two files, regardless of output order.
The remaining 25 failures include real-only proof errors and NOT_APPLICABLE results without failing steps.
The original awk parser captures only 580 of 603 executed test lines. The complete failure sections supply the final comparison.
Suite times are 41.80 and 41.74 seconds. These aggregate times establish no per-transcript wait behavior.

Comparison script SHA256: `d1d2c979914e97c37fe37bd485bf3357ad9d7152c976c3c81b98957f6cccd477`.
Comparison log SHA256: `3df3f04ba34a66e33fe36dfca32f489a69dad37f11abb82479fe18dcd24b0807`.
Before raw SHA256: `69c835e3b44c1495cda65af7af1b97eed3fb1743718240b0b4b7b0cd05ab95d1`.
After raw SHA256: `d46a72765a76208e0b0911a8789744e63a2f27f1b9735ef193a9acc76347f436`.

The current checks log names this exact clean head, rustc 1.97.0, and macOS 27.0.1.
It records a clean of 18160 files and 5.3 GiB. Its script states the single-worktree writer condition.
It reports fmt success, clippy completion, unit-suite success, 259 strict seed 0–31 passes, taint PASS, and clean final status.
Named library counts include host 332, link 50, system 44, testkit 426, and worker-core 193.
The filtered pipelines retain no numeric exit for each command. The unchanged Zig build-script warning remains visible.
Checks script SHA256: `1e9ab37137137785ff1e07aca9d71d35ae9700703b404185e7b030666e327326`.
Checks log SHA256: `bd2953c37d3bc2d4522ac9ece938196f016f1943d2b9375f9dbcf1234a2be8a9`.

### Earlier RED and completed current gate

Earlier log: `~/botster-sessions/gates/botster-core-stage1-p5-stop-grace-kill-1cd6a369-pool-20261011-210445-97328.log`.
SHA256: `c4ed61821efb12b2542253a0972277d14689f72d7efcd1d2c942149400eea5f0`.
The stage-2 baseline fails at real `conf::in_9_key_repeat_is_checked_by_the_guards_once_at_start`, seed 0, step 8, step_timeout.
The job and wrapper exit 1 after 591 seconds. That gate remains RED and supplies no acceptance.
The lead assigns the existing fence repair to the .43 import and permits the next ordinary gate.
I independently compare the .41 and .43 transcripts. The latter replaces the bare pump with a pump_until fence for Activity Output.
The current head carries that pin. The plain-v1 reproduction remains an owner/lead attribution; I ran no reproduction.

Current log: `~/botster-sessions/gates/botster-core-stage1-p5-stop-grace-kill-655bb3ab-pool-20261011-220739-62619.log`.
SHA256: `808f1b7166d67db804c84aad7dca600a9bb450d9223baee27b23cc00ceb3d9b1`.
Its header names the exact head and base. All ten steps PASS in 690.1 seconds.
The job and wrapper exit 0 after 721 seconds on msa1. Queue time is one second; run time is 720 seconds.
Default: 1915 passes and 432 skips in 9.941 seconds. Slow: 616 passes and 2060 skips in 38.711 seconds.
The broken-link Stop passes on the testkit in 0.090 seconds and the real tier in 0.088 seconds.
The real signal test passes in 0.007 seconds. Both strict summaries report 259 passes and no failure.
Strict checks run 321 of 339 pending identifiers, with eighteen real-only exclusions and four held identifiers.
All 34 passing ignored real identifiers remain in the unchanged 43-entry real-pending list.
Held-full reports 96 ignored trials, zero failures, and 21.75 seconds for three budget identifiers at 32 seeds.
Minimum counts are testkit-passing 65/69, real-passing 59/68, and real-accepted 60/69.
Coverage checks 1080 slow-only tests. No identifier removal is claimed.

Stage 1 has 34 unique mutants: 28 caught, three unviable, and three missed, in 70.1 seconds.
The slow baseline passes all 616 tests. Stage 2 tests exactly the three misses and catches all three in 142.5 seconds.
Final totals are 31 caught, three unviable, zero missed, and zero timeout.
I independently match all mutant identities, all 31 raw catches, their citations, and normal PASS for every cited test.
The deleted SIGUSR2 arm is caught by the real signal test. The complete log has no FLAKY token.
Fuzz runs both core-link decoder harnesses for the configured 60 seconds each.
The link decoder completes 6818479 runs; the message decoder completes 3870014 runs. Each reports 61 seconds and no crash.

### Disposition

Zero package source, body, or evidence findings remain on this exact head.
This CLEAN covers the complete source, corrected body, and completed READY gate.
Sol-C's matching CLEAN and the lead's later train rules remain required.
A later train composition requires applicable delta reviews, the exact-train-head gate, and final member evidence receipts.
This verdict accepts no future train head or gate and does not merge the PR.
The separate #303 worker-clock change remains outside this review.
I changed no product code and ran no build, test, gate, mutation, reproduction, measurement, or executable merge check.

VERDICT: CLEAN

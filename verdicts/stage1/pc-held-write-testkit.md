# Core #283 — held writes on the testkit tier

## Round 1 — 2026-10-11

Verdict: NOT CLEAN (1 MEDIUM, 1 LOW) at `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
Own base and supplied gate base: `9f7085f703457b0669785697dbeb14ac687a8942`.
Risk: HIGH. Package NOT CLEAN: `f91046e143764ff17504b66a355350804f346b7f`, complete verdicts/pc-held-write-testkit.md.
Sol reads that complete record, all three changed files, full body, program and control paths, scheduling paths, and gate records.
The lead's R-74 split permits the testkit part now. The real part remains in PC's later .40 pin PR.
No pending identifier, pin, timeout, worker code, production hook, or mutation configuration changes in this head.

### HW-SI-1 MEDIUM — the chunk limit hides later queue work (package HW-TK-1)

With pty_chunk=1 and a blocked kernel_bytes=3 queue, a write of abcd queues one byte.
The next attempt returns WouldBlock because input_left is zero. The kernel queue still has room for two bytes.
The write path leaves step_refused false because its guard requires !controls.blocked.
ProgramControl::waits_for_next_step therefore returns false.
Workers::has_ready uses that marker. TestkitCore::pump uses has_ready to retain more=true and signal the wake.
The combined controls can thus report no future work while a later pump can fill the queue.
The existing worker partial-write and readiness paths do not supply the missing next-step marker.

Mark next-step work when the chunk limit alone prevents progress and the blocked queue still has room.
Preserve no-next-step behavior when the queue is full or the accept limit is spent.
Add a test through Core pumps. It must fill the queue without release or an unrelated wake.
Queued input must remain unread until release. Sol verifies this defect through source inspection, not execution.

### HW-SI-2 LOW — the held-interval test discards completion events (package HW-TK-2)

The new held-write test calls settle before its no-Completed assertion.
settle polls and discards all events after every pump, including the final idle pump.
The assertion therefore cannot detect a completion consumed during the held interval.
The later post-release count and byte assertions do not prove that earlier absence.

Retain or inspect every event from the held interval. Assert that none completes this write before release.
Keep the post-release completion and ordered-byte checks.

### Other reviewed behavior and evidence

The queued bytes remain outside the program's input log until release.
Release moves the queue once into input_log and delivered. advance gives delivered to the script before a later direct write.
The write limit takes the minimum of kernel room, chunk room, accept room, and failure room.
Each applicable limit decreases by the accepted count. A block without kernel_bytes resets additional room to zero.
The control parser keeps deny_unknown_fields and session/handle validation.
It rejects kernel_bytes with on:false before changing the program. The read-only pty_queued control returns only the queue length.
Release retains the existing host wake. No terminal encoder, mode model, or machine action is added.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-646ebe73-pool-20261011-111204-3662.log`.
SHA-256: `6ad44b5a5fa5efd7129a5ac9de1535bb96efe1bd8030eb7f7fbc313cf37b658f`.
Sol reads the exact-head and base headers, ten step results, both suite summaries, all five new test records, and all mutation records.
All ten steps PASS in 554.7 seconds. Default: 1745 PASS, 478 skipped, 18.687 seconds.
Slow: 558 PASS, 2032 skipped, 57.292 seconds. Four named real proofs PASS.
Minimum: testkit 60/69; real 49/68; real accepted 50/69.
All 28 mutants are caught in stage 1, in 151.2 seconds. Every raw caught identity equals its named catcher identity.
Stage 2 does not run because stage 1 misses none. Zero missed, timeout, or unviable outcomes remain.
The complete log contains no FLAKY token. Fuzz selects no changed decoder crate.
Job and wrapper exit 0 after 668 seconds; queue one second, run 667 seconds.
This green gate does not close either finding. It contains no test for the missing combined-control schedule.

The body reports six testkit transcript passes before the split. Sol claims no independent raw verification of that scratch run.
This head pins .36, where no transcript uses the new forms. It grants no .40 transcript or real-tier completion.
The six pending removals require PART B's own comparison, repeated real evidence, and gate after the Contracts tag.
The body rejects the old runtime pipe snapshot as a source for this injected control. No old code is reused.

PC reports correction `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`, with a guard repair and event-retaining tests.
That correction's source, package closure, and exact-head gate remain unread and unaccepted in this round.
PC must close both findings at one exact head. Both source CLEANs must precede the final SLOT, import, check, and gate.
Sol ran no execution job or executable base check. Wait for the doorbell. Never poll.

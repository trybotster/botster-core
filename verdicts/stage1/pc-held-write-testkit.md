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

## Round 2 — source correction, 2026-10-11

Verdict: source CLEAN (0 open) at `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
Parent: `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
Own-diff and PR base: `9f7085f703457b0669785697dbeb14ac687a8942`.
Package source CLEAN: `123f0fe495f8550a7ef2c4dfcf9bbf6bbc7f74e1`, Round 2.
Sol reads the complete package correction section, complete two-file source delta, corrected body, and affected helper call sites.
Sol also reads the program controls, write and readiness paths, worker readiness path, and host pump path.
The source correction changes no contracts pin, pending list, production worker, timeout, or gate rule.

HW-SI-1 closes; it equals package finding HW-TK-1.
The next-step guard now permits `!controls.blocked || controls.kernel_room > 0`.
The guard still requires an unspent accept limit, an active chunk cap, and a spent per-step allowance.
Thus a spent chunk cap retains future work while the blocked kernel queue has room.
A full queue or spent accept limit does not retain that marker.
`Workers::has_ready` reads the marker. The host pump keeps `more` true and signals its wake while that work remains.
The program-edge test covers the queue with room, the full queue, and the spent accept limit.
The new host-pump test uses chunk 1, kernel capacity 3, and a five-byte write.
It pumps to idle and requires three queued bytes with no matching completion in the retained events.
Release then requires Written 5 and the exact ordered five-byte input log.
The old guard stops after the first queued byte, so the new queue-count assertion covers the original defect.

HW-SI-2 closes; it equals package finding HW-TK-2.
`settle` now returns events from each poll, including the poll after the final idle pump.
The held-write test checks that complete returned interval for a matching `Completed`.
The new chunked-write test checks its returned interval as well.
Other call sites discard the returned vector and keep their previous event-drain behavior.
The helper keeps its pump bound and clock. It adds no sleep or event re-injection.
These held intervals contain one write and no payload output; the existing poll bound covers their events.

PC reports 394 testkit tests passing, 212 conformance tests passing, and two failures after reverting the guard.
Sol traces the reported reversal to the next-step and host queue-count assertions.
No separate raw reversal log accompanies the correction receipt; those execution counts remain PC's reports.
The earlier green gate covers only parent `646ebe73`, not this correction.
The new exact-head gate remains pending. This source verdict accepts no running gate result.
After both source CLEANs, PC needs the lead's SLOT, one current-v1 import, applicable base check, and final exact-head gate.
The live body must name the completed evidence. No later import or `.40` transcript result is accepted here.
Sol ran no build, test, gate, reversal, mutant, measurement, or executable base check. Never poll.

## Round 3 — fixed-head red gate attribution, 2026-10-11

Source CLEAN remains at `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`; zero source findings remain.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-bd4c6f16-pool-20261011-113530-95097.log`.
SHA-256: `4208ebc4b8ac166e5e7686c6f87daa02940f462b9f9b96f4acd27090116fbd49`.
Sol reads the exact-head header, complete failed-step output, ten summaries, and final exits.
Sol reads the complete package audit at `9609f88011fe6f6fc6c8c6669eee91dbf5fa896a`.
The gate base is `926dfb62f3f7168990b2a48741946932d84b2fbb`; the PR base remains `9f7085f703457b0669785697dbeb14ac687a8942`.
Fmt, clippy, and taint PASS. Lists FAIL on exactly four retained old-base entries.
They are dp_3_unknown_type_closes_route, ou_10_no_authorization_in_core, ou_11_transport_ownership_is_defined, and ou_1_attach_hands_transport_to_worker.
Sol independently verifies that the later base removes exactly those four entries from core-pending.txt.
The original head and fixed head have the same pending-file blob, `2658d84019768cc372f3c31993429e182ccd3b31`.
The complete source correction touches only the two reviewed testkit files. It changes no conformance file.
The failed comparison thus treats the old-base entries as additions against the advanced base.

Public-api, prebuild-worker, test-budget, slow, mutants, and fuzz are NOT RUN.
The CI steps total 75.6 seconds. The job and wrapper exit 1 after 160 seconds; queued 1, running 159.
This red gate supplies no mutation evidence for the correction. The earlier 28 catches cover only its parent.
Lead handoff 07:38 chooses FINAL-READY now and SLOT after #282. No separate same-head retry is required.
The approved current-v1 import must remove the diagnosed cause and receive its applicable review or proven carry.
A complete final exact-head green gate and updated body remain due. This record accepts no unseen import or gate.
Sol ran no execution job or executable base check. Never poll.

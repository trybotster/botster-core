# #307 — control queue

## First READY inspection — package verdict and bound disposition pending

Head: `614cc30dabd59a7d8d5a770ff0eb74991bb240cb`.
Parent: `c48446bc73f7b5e86b65692b7d4fc67d2b642f36`.
Tree: `e1888d118a17aa12d09942840ea46f0acd24452f`.
Source base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
GitHub/gate reference: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`, not imported.
READY receipt: `msg_plugin-w_1791780222_50be40`.
The complete own diff has nineteen files, 620 additions, and twelve deletions.
Full-index SHA256: `0169ed8e59c3908ccc53aecba116d378b7b68f9ee8b3d49ecf24085dc3f3301c`.
Body raw JSON SHA256: `c5de2202f5f349baf51cea0ac1b2f6386d302df0feeb583a547c0db378e85979`.

I read the complete own diff, affected driver/engine/failure paths, R-91, design note, body, supplied scripts/logs, and both completed gates.
The exact-head package verdict remains pending. This inspection completes no formal review round.

### R307-SI-1 — MEDIUM — OPEN — failed write side appears to have room

After `cancel` returns `Refused(Full)`, the worker handle retains `refused_full`.
A fatal link send calls `close_write` at driver.rs:570.
That method sets `write_closed`, clears the unsent `out` buffer, and feeds `LinkWriteFailed`.
`on_link_write_failed` at inbound.rs:779 sets only `worker.write_failed`; it keeps the refusal mark and the worker link.
The read side correctly remains open for R-78's drain.

`service_links` then calls `control_room` after the flush.
The room helper at driver.rs:229 sees an empty buffer and reports available room.
`waits_for_room` at engine.rs:321 checks only the matching link and the refusal mark.
Neither room path excludes the failed write side.
With event budget, Core posts `SessionWritable` although this control stream cannot take another message.
Without event budget, Core marks this dead write-side work as `more` for the next pump.

IN-6 and SV-3 require room for one control message after the refusal.
R-78 requires no further sends on the failed write side. Discarding bytes does not restore that side's room.
Exclude failed write sides from room eligibility.
Retire the refusal mark without treating discarded bytes as a successful drain.
Preserve the existing read-side drain and its request completions.
Add a driver behavior regression for Full, fatal send, retained read side, and no false `SessionWritable` or `more` from that mark.
This is source evidence. I performed no runtime reproduction.
Finding receipts: `msg_plugin-w_1791780473_1b0ff4` and `msg_plugin-w_1791780473_b35f29`.
Package receipt `msg_plugin-w_1791780612_3d2e6b` independently confirms this finding as CQ-PC-1 MEDIUM.

### Bound question — unresolved

R-91 accepts an internal bound using the largest non-input host message frame.
The code instead uses `NON_INPUT_FRAME_BYTES = 4096` as an estimate.
The new DESIGN.md and body explicitly state that valid AttachRoute or UpdateMetadata messages can exceed that estimate.
The production-bound test checks the same constant; it establishes no largest-frame upper bound.
I found no later ruling for this difference in the lead handoff through its #307 READY record.

The lead must determine whether that estimate satisfies the accepted R-91 form, or whether a maximum-sized term is required.
No new CoreLimits field, control, budget, or broader implementation is requested here.
This remains a contract-meaning question, separate from the failed-write source finding.
Package audit request: `msg_plugin-w_1791780389_9d1494`.
The package reviewer confirms the difference and will request the lead's steward disposition.

### Other source assessment

The driver supplies the smaller of its counted room and the injected edge room to the engine's cancel check.
The check follows UnknownOp, TooLate, NotCancellable, and AlreadyAdmitted, and precedes sent/not-sent handling.
The engine retains the cancellable op on Full and records the session mark.
The ordinary drain clears that mark and posts a keyed event within the pump budget.
The corrected TM-6 check excludes links with no refused cancel before it tests room or a spent budget.
The source removes the unused `Observation::Writable` and its inbound mapping.
R-91 permits that internal message change.

Both testkit tiers implement `control_queue` through `HostEdges::link_room`.
The control records the current worker instance and signals the host wake when it releases the hold.
No production machine branches on the control.
The driver tests cover counted unsent bytes, two refusals/one event, no event without refusal, injected room, and the spent budget.
The testkit regression covers registration, default-on parsing, Full, room restoration, and the promised keyed event.
These tests do not cover the failed-write finding.

### Conformance and completed gate

Exactly two ids leave core-pending: `in_6_cancel_result_kinds` and `in_6_refused_full_wakes_on_room`.
The testkit pending set changes from 410 to 408. Real-pending stays at 44; held stays at three.
The other pending changes are comments for R-96 EV-9 and R-93 fairness.
EV-9 remains pending because its transcript expects one Activity where Core posts separate Output and Client events.
No pin, other status-set membership, gate budget, or mutation exclusion changes.

Current gate: `~/botster-sessions/gates/botster-core-stage1-pc-control-queue-614cc30d-pool-20261011-213222-45073.log`.
SHA256: `dedecaad1a1e92ced19f86f88c26c3710f97a976caa5f912ec35223e1537dde8`.
All ten steps PASS in 626.5 seconds.
Job exits zero after 635 seconds on msa1, with zero queued seconds and 635 run seconds.
The wrapper exits zero after 636 seconds.
Default: 1895 PASS, 431 skipped, 16.297 seconds. Slow: 615 PASS, 2041 skipped, 43.779 seconds.
Both strict summaries report 260 PASS and zero failures.
Both removed ids have normal testkit and real PASS records.
All 31 reported passing ignored real ids remain in the unchanged real-pending list.
Held-full runs two budget ids at 32 seeds: 64 expected ignored results, zero failures, 19.28 seconds.

I independently match all 57 unique initial mutation identities: 32 caught, thirteen missed, twelve unviable.
Stage 2 tests exactly those thirteen initial misses and catches all thirteen.
All 45 raw catches equal the citations. Every cited test has normal PASS.
All thirteen stage-2 catchers also have baseline PASS. Baseline: 615 PASS, ten slow, 2790 skipped, 44.071 seconds.
No mutation remains missed or timed out. The completed log has no FLAKY token.
Stage 1 takes 112.0 seconds; baseline 53.1 seconds; stage-2 mutations 148.9 seconds.
Fuzz runs link_decoder and msg_decoder for sixty seconds each.
The logs end with 8,090,929 and 4,691,312 runs respectively, each after 61 seconds.
The green gate does not close the source finding or the bound question.

Earlier gate: `~/botster-sessions/gates/botster-core-stage1-pc-control-queue-c48446bc-pool-20261011-210022-77744.log`.
SHA256: `e8a84f224b6dc4e76dac6c7c31df56186f49d159caf0adb6ae58abc29875ca1f`.
Its 58 unique initial identities have 26 catches, twenty misses, and twelve unviable.
Stage 2 tests exactly those twenty misses: thirteen caught and seven missed.
All 39 raw catches match citations and normal PASS. The log has no FLAKY token.
Six surviving testkit mutations lacked testkit coverage. The seventh is the old redundant engine guard mutation.
The repair adds the testkit behavior test and reuses `waits_for_room` in that engine method.
Eight steps PASS, mutants FAIL, fuzz does not run. Total: 1058.7 seconds.
Job and wrapper exit 1 after 1142 seconds on msa1, with one queued second and 1141 run seconds.
This gate remains RED.

### Local evidence and limits

I read the complete current local-check script/log.
The script sets both parallelism caps and removes the toolchain override.
Log SHA256: `f466233bb2d17d601fb7a42fa38c115099c931a3c6b15ff1eb3578ec310b5e5d`.
It names this head and the 04:24:35 UTC clean marker. It retains no numeric clean exit or raw clean output.
Ten xtask commands exit zero. Fmt succeeds; filtered clippy output reports the existing Ghostty build warning.
It records 336 host, 48 link unit plus four link integration, and 192 worker-core passes.
Testkit results total 427 across its unit and integration binaries. Strict seeds 0–31: 260 PASS, zero failures.
Filtered Cargo pipelines do not retain numeric exits for every command.

The complete focused evidence script/log names eb37555a, not this repair head.
It records the 03:37:18 UTC clean of 39912 files and 10.2 GiB and prebuild completion.
Both in_6 ids PASS on the testkit with strict seeds 0–31.
I independently match sixteen real PASS cells: both ids at each seed 0–7.
It supplies no real seed 8–31 proof. The current gate separately supplies normal real PASS on this head.
Focused log SHA256: `7c553f11dc45a4efe8996bbeeff8b8af1e6c3759e3fd16a736a1ab3722257f7e`.

The original reversal record names 2c7e7016 and fails two behavior tests when either the room check or the wake is removed.
The TM-6 reversal record names eb37555a: removing the waiting-mark check fails three earlier tests; removing `more` fails the new spent-budget test.
Both records report restored zero changes, with filtered test summaries and no numeric Cargo exits.
The manual repair record names this exact head. Each of six missed testkit mutations fails the new test with exit 101.
It ends with restored clean status. No complete mutation script or independent cache record accompanies those short reversal logs.
The completed pool gate independently catches all six.
Historical pre-merge local results remain attached to their own heads; they supply no final integration proof.

### State

R307-SI-1 remains open. The bound question and complete exact-head package verdict remain pending.
A formal first review round is not complete.
The current source has not imported its gate reference or the later final-slot base.
Final import/checks, applicable delta reviews, exact-final-head gate, final reviews, and Q7 remain due.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

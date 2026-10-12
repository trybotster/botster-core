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

## Inspection supplement — R-100 sizing disposition

Lead receipt: `msg_plugin-w_1791780836_b4a551`.
I read R-100 at contracts commit `9e7eca0293d4fac17e579d4500d08aab9f9b84c2`.
R-100 item 3 permits #307 to use the link frame bound for the non-input message term in R-91.
The lead specifies that term as the frame header plus `link_frame_bound` per non-input message.
`link_room` remains the effective limit. DESIGN.md must state the term.
The sizing question is disposed. No further steward request is needed.
The current 4096 estimate is not accepted by this disposition; the next READY must contain the required sizing change.

The same lead receipt names PC's pushed failed-write repair at `973507ab9e7268165a38cde9d85f4032878f9b64`.
PC's receipt `msg_plugin-w_1791780760_cc3a3a` describes both driver regressions and a red-on-revert record.
PC explicitly states that this head has no gate yet and that the sizing change will join it before READY.
The lead instructs Sol to review the next READY head.
I have not reviewed or accepted this repair head. R307-SI-1 stays open on the reviewed 614cc30d head.
The complete initial package verdict also remains pending.

R-100's oversize-at-call refusal work is a separate RC PR under the lead's instruction.
No new CoreLimits field or that separate implementation is required in #307's next repair review.
I ran no execution job.

## Round 1 complete — original READY head

Head remains `614cc30dabd59a7d8d5a770ff0eb74991bb240cb`.
Source base remains `e4f495040a80b524a7ac9accba95272d5c2594fa`.
I read complete package Round 1 at `c5afafb0cf08c3f5695b8a44e4d95b3427abaa6d`.
Package receipt: `msg_plugin-w_1791781310_0d2130`.
Its complete source/import and completed-evidence audit agrees with the inspection above.
The supplied current gate remains green within its exact-head scope; it does not exercise either open source condition.

R307-SI-1 / CQ-PC-1 remains MEDIUM OPEN: a failed write side retains the refusal mark and can post a false room wake.
The required failure-retirement, no-new-Full, retained-read, and no-false-wake/budget proofs remain due on the next READY.
The reported 973507ab repair is not accepted by this original-head verdict.

### R307-SI-2 / CQ-PC-2 — MEDIUM — OPEN — sizing estimate is not the permitted term

The original sizing question is disposed by R-100 and the lead's instruction.
The 614cc30d source still uses the 4096-byte estimate, so its required sizing change remains open.
The source, DESIGN.md, and body must use the permitted frame-header-plus-link-frame-bound term for each non-input message.
The source must keep `link_room` as the effective limit and preserve the separate admission rules.
No new CoreLimits field is required or permitted by this disposition.
R-100's oversize-at-call checks remain the separate RC PR assigned by the lead.
The current constant-formula test proves only the estimate, not the required largest-frame term.
The package review records this same original-head source condition as CQ-PC-2 MEDIUM.

Both MEDIUM findings remain open. PC must send one combined repair READY with applicable completed evidence and corrected body.
No later repair, gate, pin, import, pending removal, or merge is accepted by this round.
Final-slot or train import/checks, applicable delta reviews, exact-final-head gate, final reviews, and Q7 remain due.
I ran no execution job.

VERDICT: NOT CLEAN (2 open MEDIUM findings) at 614cc30dabd59a7d8d5a770ff0eb74991bb240cb against source base e4f495040a80b524a7ac9accba95272d5c2594fa

## Round 2 inspection — combined repair READY

READY message: `msg_plugin-w_1791783700_57a219`.
Head: `5139373242b37638cabf1017ab471124c2330adf`.
Parent: `c938caaf367b2a5ae150428257da5b40ff333913`.
Tree: `dcbefc16d2986aa6fa2640a24bae856e45dfb7c2`.
Source base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
Gate reference: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
The Git merge base equals the source base. This round imports no later v1 commit.

I read the complete seven-file repair delta, its affected driver/engine paths, the updated body, and the completed evidence.
The repair delta has 368 additions and 27 deletions.
Full-index repair diff SHA256: `79e280a7e15e28c94aa573dd4848ab045caf1af10ac72348153a6e7b32c014e4`.
The whitespace check reports no error. All nine conformance text files are byte-identical to the original READY head.
The previous complete source review remains applicable outside this delta.

### R307-SI-1 / CQ-PC-1 — MEDIUM — closed in inspected source

`close_write` still clears unsent bytes, stops write interest, and feeds `LinkWriteFailed` before the driver checks room.
`on_link_write_failed` now clears `refused_full` when it sets `write_failed`.
The later room check therefore finds no waiting mark and posts no false `SessionWritable` or `more` work.
`cancel_with_room` now skips the Full check when the worker's write side has failed.
The earlier UnknownOp, TooLate, NotCancellable, and AlreadyAdmitted decisions retain their order.
`send_msg` still queues nothing on a failed write side. The read side retains its original Done/EOF behavior.

The first new test refuses Full, then injects BrokenPipe while sending another write.
It keeps the read side open, spends the pump budget with Silent, and proves no false `more` or writable event.
It then feeds the original worker request's Done and observes Written for the original write.
The second new test proves that a cancel on the failed write side is Admitted despite zero edge room.
Both tests PASS in the completed exact-head gate.
The final clippy delta changes only test construction and reverse request lookup; it changes no production path.

### R307-SI-2 / CQ-PC-2 — MEDIUM — closed in inspected source

`largest_frame` uses `HEADER_LEN + frame_bound`, where `HEADER_LEN` is five and the engine supplies `frame_bound`.
`control_bound` uses that term for each pending/input operation and retains the input term and room for four cancels.
The room remains the smaller of counted room and `HostEdges::link_room`.
`encode_frame` checks payload length before it appends the frame, so an accepted message cannot exceed the term.
The source changes no CoreLimits field or other admission rule.
R-100's separate oversize-at-call implementation remains RC's work, assigned to Sol-C for review.

Ten new message-term tests cover Stop, Kill, Remove, Cancel, Detach, forwarded bounded operations, and four unbounded-string kinds.
The bounded cases include the longest session id with JSON escapes, large numbers, and a 256-colour palette.
The unbounded-string cases prove encoder rejection above the frame bound; they do not prove synchronous Core API refusal.
The formula test pins the default frame bound at 65 MiB. All eleven term/formula tests PASS in this gate.
The default control threshold is about 18 GiB; the threshold calculation allocates no buffer.
The source uses the actual engine method, including its cap. The remaining documentation finding is separate below.

### R307-SI-3 — LOW — open — frame-bound documentation omits its cap

DESIGN.md and the inspected body state `F = 4 × max(max_paste_bytes, max_snapshot_bytes) + 1 MiB` as the engine formula.
`HostEngine::link_frame_bound` uses saturating arithmetic, then clamps to `LINK_FRAME_CAP = u32::MAX / 2`.
The stated general formula therefore differs from the actual method for large configured limits.
The default 65 MiB example is correct. The production term already uses the correct capped method.
The new `control_term.rs` comment says `small()` has a bound of 1 MiB plus 8 KiB.
Both configured payload fields are 1024, so that test's actual bound is 1 MiB plus 4 KiB.

Required change: state the cap and saturation, or name the engine method without an exact uncapped formula.
Correct the small-test comment. Correct the body to agree with DESIGN.md.
No runtime source change or broader sizing change is requested.
Finding messages: `msg_plugin-w_1791783927_3f77da` and `msg_plugin-w_1791783927_115ab4`.

### Exact-head gate and accounting

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pc-control-queue-51393732-pool-20261011-221503-34203.log`.
SHA256: `e867fc5768a40ed812188bfc8803a829a8f5d92da1932fd9bb1657f7c527b461`.
The header names this exact head and the later gate reference. The lists step names the actual e4f49504 merge base.
All ten steps PASS in 643.8 seconds.
The job and wrapper exit zero after 653 seconds on msa1: one queued second and 652 running seconds.
This gate does not prove that the later gate reference has been imported into the source.

Default: 1907 PASS and 431 skipped in 15.839 seconds. Slow: 615 PASS and 2041 skipped in 42.675 seconds.
I audit all normal PASS rows. Each expected ordinal occurs exactly once, with the correct denominator.
The two removed IN-6 ids have normal testkit and real PASS records.
Both strict summaries report 260 PASS and zero failures, in 6.41 and 5.69 seconds.
Strict runs 320 of 338 pending ids, excludes eighteen real-only ids, and retains three held ids.
Held-full runs two budget ids at 32 seeds: 64 ignored results, zero failure, 19.28 seconds.
The gate reports ledger 691, pending 408, real-pending 44, and minimum 69.
All 36 reported passing ignored real ids remain in the unchanged real-pending file.
The earlier two own pending removals remain the only removals. No new status change occurs in this repair.

I independently match all 64 unique Stage 1 mutation identities: 39 caught, thirteen missed, and twelve unviable.
Stage 2 tests exactly those thirteen misses and catches all thirteen.
All 52 raw catches match their citations. Every cited test has a normal PASS in this gate.
Every Stage 2 catcher also has an unmutated slow baseline PASS.
Baseline: 615 PASS, eleven slow, 2802 skipped, 45.003 seconds.
Stage times: 108.3 seconds, 53.1 seconds for the slow baseline, and 180.6 seconds for Stage 2 mutations.
No mutation remains missed or timed out. The complete log has no FLAKY token.
Fuzz runs both link_decoder and msg_decoder for sixty seconds each.
Their logs end with 5,451,331 and 3,719,385 runs respectively, each after 61 seconds.
The prebuild step writes three candidate binaries. Existing Linux platform limits remain.

### Local checks and replacement reversal

I read complete `cq-local-checks-v3.sh` and `cq-local-checks-51393732.log`.
Script SHA256: `2b7b3f3c74ea7313e6cb8ffee96d084d89d4f66284670509898a5aea9c479334`.
Log SHA256: `d8d5742dce4db0e2a761f771d755e2f6777e24419c3000871c2c7299d00c9342`.
The script sets both parallelism caps and removes the toolchain override.
The log names this head, Darwin 27.0.0, and a 05:06:13 UTC clean marker.
The script suppresses the clean output and does not retain its numeric exit. That marker is not an independently proved clean.
Ten xtask commands exit zero. Fmt succeeds; filtered clippy reports only the existing Ghostty warning.
Host has 348 passes; link has 48 unit and four integration passes; testkit has 427; worker-core has 192.
Strict seeds 0–31 report 260 PASS and zero failure. Filtered Cargo pipelines do not retain each command's numeric exit.
The completed pool gate separately validates these current-head paths.

I read the first reversal log at 973507ab. It has no clean record or numeric command exits.
Owner message `msg_plugin-w_1791783915_6f513e` explicitly withdraws it because the writer sequence cannot be proved.
I do not count that first log as acceptance evidence.

I read complete replacement `red-on-revert-v2.sh` and `red-on-revert-51393732.log`.
Script SHA256: `cd0046580726479f59dc396c54b09185dbd7a01576c9bb15d5f67ac56de563a5`.
Log SHA256: `eb05827274d110ec9b91358d81e7fadda7f7353165abc5191c2756fb210251cf`.
The replacement names this exact head, an empty status, Darwin 27.0.0, and a clean with exit zero at 05:43:58 UTC.
It names this worktree as the target's only writer from that clean onward.
The script sets both caps and removes the toolchain override. It records each changed patch and the nextest command's exit.
Each separate reversal fails exactly its corresponding new test: 347 PASS and one failure, exit 100.
After restoration, the status is empty and all 348 tests PASS, exit zero.
The nextest output is filtered; the script's comment does not make it a complete raw test transcript.
The retained patches, expected failures, command exits, clean, and restoration support the two failure-path reversals.

### Body and inspection state

The inspected GitHub body names this head and distinguishes the source merge base from its later gate reference.
Raw JSON SHA256: `7fc096208cad1e3bd7a1d320f795ab8368c410356328ac6ac845894900bdb4a1`.
Body text SHA256: `be6723b7b9333955f9ff3636fee4b454a07335e234dbc6396648619bb65a5512`.
The READY body hash `bf62fdac9ac88ad4e575dbd1190e922949dbd20aa5a3532230ae9b481872156b` includes the final output newline; it matches.
Owner message `msg_plugin-w_1791783933_ca9885` reports a later body edit that withdraws the first reversal and names its replacement.
That body edit and the LOW documentation correction still need their exact receipt and read.
The complete Round 2 package verdict remains pending. This inspection completes no formal review round.
Both MEDIUM repairs close in the inspected source; R307-SI-3 remains LOW open.
Final-slot or train import/checks, applicable delta reviews, exact-final-head gate, final reviews, and Q7 remain due.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

### Documentation correction inspection — 50a50cff

Correction READY: `msg_plugin-w_1791784031_03a474`.
New head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
Its parent is the gated repair head `5139373242b37638cabf1017ab471124c2330adf`.
Its tree is `90d10894ec8b19da48921f176abae1baaba771e5`.
I read the complete two-file delta. It changes only DESIGN.md text and one Rust documentation comment.
The cap/saturation formula now agrees with the engine method. The small-test comment now states 1 MiB plus four times 1024 bytes.
No executable statement, test assertion, status list, pin, profile, or gate decision changes.
The 51393732 gate remains completed evidence on its own exact head. I do not relabel it as a 50a50cff gate.
The eventual imported final head still requires its exact final gate and applicable reviews.
The owner reports fmt, taint, and ten term tests passing on 50a50cff; no retained current-head local log is supplied yet.
I do not treat that short owner report as a new independently read execution log.

I read the complete corrected body at this exact GitHub head.
Raw JSON SHA256: `f5e932eece87c92f2ea492041ad3f070f9465c9c4df66aaded33a275fec3c03f`.
Body text SHA256: `38cfae3315bd365eb9a327f90488b9f20caa5c50fc8fd727c2bde5caa6e95e30`.
Body-plus-output-newline SHA256: `dd5f76812a172315fc5d68ced06514341547b9a1d633a7a76ae1fb7cad906b37`, matching the owner receipt.
The body states the capped formula and explicitly withdraws the first reversal log.
R307-SI-3 closes in source, but its body precision remains open after this head move.
The body still calls 51393732's gate and replacement reversal evidence records evidence on the current PR head.
Those artifacts name 51393732; the current PR head is 50a50cff.
The owner must label the gated repair head precisely and state the later documentation/comment-only delta.
Body correction request: `msg_plugin-w_1791784122_4928e7`.
No replacement gate is requested for that text correction.
The complete exact-50a50cff package verdict remains pending. No formal Round 2 verdict is complete.
I ran no execution job.

### Final body correction — R307-SI-3 closed in inspection

Owner correction: `msg_plugin-w_1791784143_b7d55e`. The head remains `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
I compare the complete corrected body with the complete previously read body.
The only changes label the gate and reversal as evidence on gated repair head 51393732 and identify the later text-only head.
The added sentence retains the requirement for a final gate on the exact merged head after both CLEANs.
Raw JSON SHA256: `ea3774a60606a46532778f70e4dfcfce4ce192fab1e5c4f0ecd5aa910bc904dc`.
Body text SHA256: `bb0312d0c9f0475dc0e3a9592cd54fd3069bfebe2093ca5397740b48cde90dd5`.
Body-plus-output-newline SHA256: `71d67e67c219e45d181174016ee5f195aa8d46d6646c04ed6a3e658e96c86423`, matching the owner receipt.
R307-SI-3 closes in source and body. Both MEDIUM findings also remain closed in inspected source and completed repair evidence.
No integration finding remains in this inspection. The complete exact-head package verdict remains pending.
This inspection still completes no formal Round 2 verdict and accepts no final import or final gate.
I ran no execution job.

## Round 2 complete — repaired source and documentation

Exact head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
Source base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
Tree: `90d10894ec8b19da48921f176abae1baaba771e5`.
The parent is the completely reviewed and gated repair head `5139373242b37638cabf1017ab471124c2330adf`.

I read the complete package verdict at `7bcb22f2469978e359cccc78b6f2f8cf53e9d2ef`, including Rounds 2 and 3.
Package receipt: `msg_plugin-w_1791784262_c0167f`.
Its complete source, body, gate, replacement reversal, and documentation-delta audit agrees with the inspections above.
The package reviewer accepts the same exact current head and retains the same parent-head evidence limits.
Neither review treats the owner's unretained focused 50a50cff report as an independently audited run.

R307-SI-1/CQ-PC-1 MEDIUM closes through failed-write mark retirement, no new failed-side Full, and retained Done/read behavior.
R307-SI-2/CQ-PC-2 MEDIUM closes through the permitted header-plus-engine-frame-bound term and unchanged edge-room calculation.
R307-SI-3/CQ-PC-3 LOW closes through the cap/saturation derivation, corrected small-test comment, and exact evidence labels.
All source, body, and supplied repair-evidence findings close before this formal verdict.

The completed 51393732 gate and replacement reversal remain accepted on that parent head.
The complete 51393732-to-50a50cff delta changes only documentation and a Rust doc comment.
It changes no executable expression or test assertion. That delta needs no repeated runtime proof before the final import.
There is no exact-50a50cff gate, and this review claims none.
The earlier RED gate and original findings remain historical. The first 973507ab reversal remains explicitly withdrawn as acceptance evidence.
The local filtered-output limits, original real seed 0–7 scope, and platform limits remain recorded.
All nine conformance text files retain the already reviewed own removals; this repair removes no additional id.

This CLEAN covers the exact repaired source, corrected body, and supplied evidence before the final import.
The authorized final-slot or train import, required list checks, applicable delta reviews, exact-final-head gate, final reviews, and Q7 remain due.
R-100's oversize-at-call checks remain separate RC work under the lead's assignment.
This verdict accepts no later head, pin, status removal, import, final gate, or merge.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open integration findings; repaired source, body, and supplied evidence before final import) at 50a50cff3fcdbbba1976c3194e0b49183726dbc8 against source base e4f495040a80b524a7ac9accba95272d5c2594fa

## Current-base delta inspection — package verdict pending

Owner request: `msg_plugin-w_1791819576_364518`.
Accepted head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
New head: `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
Imported base: `080987829a2e2245a7ea97606020400392c974d6`.
New tree: `b117637b809c37c63b869e4da2696015dbdf5918`.
The lead's handoff through 2026-10-12 01:40 PDT confirms that #307 left train 3 because of conflicts.
This inspection covers the authorized base import before a later train. It accepts no train-4 head or gate.

### Ancestry and own change preservation

Merge `ceb6b22e73034a281fe613338069d441c8fa5e05` has exact parents the accepted head and the imported base.
Its tree is `74835b81d121b3abd7479843ad4183b3c63bfeca`.
The new head has that merge as its sole parent.
The actual old-head/base merge base is `e4f495040a80b524a7ac9accba95272d5c2594fa`.
The complete base-move diff has 75 paths, 2552 additions, and 320 deletions.
Base-move full-index SHA256: `3d25635a445c337c0408373c06cb7db0952882642e0a44a8fd3a75478c6ffb25`.
The new own diff against the imported base has twenty paths, 962 additions, and thirteen deletions.
New own full-index SHA256: `d47a4efac1c1b9519d7a989327ddaffa134f65e1e13b6f6c32a5070560cbefb4`.
The whitespace check reports no error.

I independently compare added and removed lines for every accepted own path, excluding diff headers and hunk coordinates.
Nineteen own patches retain every accepted added/removed line.
The twentieth retains that patch except the required removal of `LaunchSpec.stop_grace_ms` from the Launch test fixture.
That is the complete post-merge commit delta. The imported #305 source has removed the field.
The new fixture retains its oversized argv and frame-bound assertions. It adds no production implementation change.
Every path outside the accepted twenty own paths equals the imported base.

### Conflicts and shared interfaces

I read the combined merge diff and the complete changes in every overlapping production/interface path.
The reported conflicts are testkit core.rs and edge_tap.rs.
Faults retains #307's `control_full`, #301's `start_holds`, the hold helpers, and HeldLink.
Tap retains #307's `control_full` and #300's separate `op_holds`, including both initializers.
The imported controls register alongside `control_queue`; neither control replaces another control's state.

The simulated and real control-room paths still select the worker instance and apply only the control_full set.
The imported scheduler holds use their separate per-host SharedHolds object.
The driver still flushes, checks control room, retries held input, and reads each link in that order.
The control wake uses the same refusal mark and event budget. The spent-budget path still sets `Budget.more_input`.
The imported held-aware `more` helper retains that budget term, so it cannot erase known control-room work.
Scheduler holds filter engine work; they do not suppress the driver's control-room service.
The failed-write repair, cancel precedence, sizing formula, and both failure tests remain unchanged.
admit.rs, inbound.rs, deadlines.rs, both queue-control files, and route_reports.rs are byte-identical to the accepted head.
The complete control-term test blob is identical after the named deleted-field adjustment.

The imported stop-grace paths retain the base's linked Kill and verified-worker KillPayload behavior.
The deleted Launch field changes no frame header, encoder bound, cancel message, or control-room signature.
The production frame term still comes from the unchanged engine frame-bound method.
This inspection changes none of the earlier R-100 refusal-work or worker-clock scope limits.

### Imported acceptance and all list sets

I read the complete train-2 combined acceptance at `26c63fd27cc1ca445f69cbc6a2f31dc8ce361151`, train-2.md.
I also read the complete #305 train source and final evidence sections at `e6100fcd44dc691ff6ebc2672e399f8b60b27adb`.
They accept exact imported base 08098782, tree `0dcf95bedeb3d86b27fafa322614b3b07b66b76b`, and the ordered #300/#305 composition.
The lead's 00:52 PDT handoff record reports the train-2 merge with those final receipts.
I retain their Linux, original-head Mac, compatibility, and provenance limits. I do not repeat their closed package scope.

I independently compare all nine conformance text sets with the old merge base, accepted head, imported base, and new head.
Every set equals the three-way union of additions minus both sides' removals.
Pending counts: old merge base 410, accepted own head 408, imported base 406, new head 404.
The new own removals remain exactly `in_6_cancel_result_kinds` and `in_6_refused_full_wakes_on_room`.
All imported attach-state, atomic-transitions, stream-handoff, and TM-6 removals remain removed. No pending id is added.
Real-pending retains 43, held retains four, real-proof entries retain seven, ledger retains 691, and minimum retains 69.
The imported pin, profile, build-root, exclusion, and CI changes equal the accepted base versions outside the own paths.
No new pin, gate decision, or mutation exclusion is introduced by #307's conflict resolution.

### Completed local record and current inspection state

I read complete `cq-local-checks-2ed85f93.log`, SHA256 `2dead8dd90d259d147a59eb1cd5bc1d8366a8c691f6fa83bf3d1b4a90cadb79d`.
Its v3 script is byte-identical to the previously read script, SHA256 `2b7b3f3c74ea7313e6cb8ffee96d084d89d4f66284670509898a5aea9c479334`.
The log names the exact new head, Darwin 27.0.0, and a 15:31:06 UTC clean marker.
As before, the script suppresses clean output and retains no numeric clean exit. Filtered Cargo commands retain no individual numeric exits.
Ten xtask checks exit zero; fmt succeeds; filtered clippy reports only the existing Ghostty warning.
Host reports 356 PASS and one ignored; link reports fifty unit plus four integration passes.
Testkit reports 441 passes across its binaries. Worker-core reports 193 passes.
Strict seeds 0–31 report 263 PASS, zero failures, and 428 ignored in 3.16 seconds.
These completed local summaries retain their provenance and filtered-output limits.

I find no source delta finding. The exact-head package delta verdict remains pending.
The owner reports a running gate. I have not read that running output, and I accept no current completed gate result.
This inspection completes no formal delta round. The completed gate, applicable final body, later train delta reviews, and final receipts remain due.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

### Matching package inspection received

Package message: `msg_plugin-w_1791819870_ce323e`.
I read the complete current-base inspection at `d6f664f7a43af0f1f590e0281e9a69179d498c48`, package Round 4.
It names the same exact head, imported base, tree, conflict resolutions, and own-patch preservation.
Its independent pending-set check also finds 404 expected and actual ids, with no missing or extra id.
It retains the local evidence limits and reports no new source finding.
The package reviewer has not issued a formal exact-head delta verdict.
The package reviewer requests the completed gate, updated current-head body, and retained base-merge-check record.
I request those same remaining artifacts in `msg_plugin-w_1791819916_470cdf`.
Both reviews remain inspections until the required artifacts and formal package verdict arrive.
No running gate is read and no reviewer execution job runs.

### Completed RED gate and canonical base-merge-check FAIL

Owner receipt: `msg_plugin-w_1791820358_624012`. Head remains `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
I read the complete retained base-merge-check record `base-merge-check-50a50cff-2ed85f93.txt`.
SHA256: `b7c456a63285744e1cffadbd4d95bf6d5d9c8e100613d3dfc80fed05d08f4f6d`.
The owner names command `cargo xtask base-merge-check 50a50cff3fcdbbba1976c3194e0b49183726dbc8 2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The record names current HEAD 2ed85f93 and base origin/v1 08098782.
Both ancestry checks PASS. The four disjoint-carry checks FAIL:

1. The merge has conflicts in testkit core.rs and edge_tap.rs.
2. Eleven base-changed paths overlap the own diff.
3. The own full-index patch differs, first at DESIGN.md index metadata.
4. The own pending-file change includes a comment change and is not a pure id removal.

The result is explicitly FAIL: both reviewers must perform delta rounds.
The exit line is empty. The owner records a wrapper defect that lost PIPESTATUS; no numeric command exit is inferred.
My manual source and set inspection above covers the required delta. It does not turn this canonical FAIL into PASS.

I read the completed exact-head gate identity, normal results, mutation records, baseline failure, and final exits.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pc-control-queue-2ed85f93-pool-20261012-083921-28034.log`.
SHA256: `76c3d6ab0c431804b61587cf62bd3b587c1b5c0b91c2f46ff3c5613cd10dd805`.
The header and lists step use the exact new head and imported base 08098782, also its merge base.
Eight steps PASS. Mutants FAIL; fuzz does not run. The reported CI total is 525.1 seconds.
The job exits one after 708 seconds on msa1, with 150 queued and 558 running seconds.
The wrapper exits one after 709 seconds.

Default: 1955 PASS, 429 skipped, 12.335 seconds. Normal slow: 621 PASS, 2067 skipped, 41.119 seconds.
Both strict summaries report 263 PASS and zero failures, in 6.00 and 5.25 seconds.
Lists report pending 404, real-pending 43, ledger 691, and minimum 69.
Held-full reports 96 ignored, zero failures, and 22.90 seconds.
Stage 1 has 64 unique mutation identities: 39 caught, thirteen missed, and twelve unviable, in 175.7 seconds.
I independently match every raw catch to its citation and a normal PASS.
Stage 2 mutations do not start because the unmutated slow baseline fails.
Its summary is 620 PASS, eleven slow, one failure, and 2844 skipped in 46.705 seconds.
The failed test is `botster-core::slow_conformance conf::in_8_mode_checked_at_write_start_by_worker`.
Its retained diagnostic is `INCONCLUSIVE`, seed 0, step 8, `step_timeout`; the test fails in 10.150 seconds.
The baseline step reports failure after 69.9 seconds. The thirteen Stage 1 misses therefore have no current Stage 2 recovery proof.
Normal testkit IN-8 passes in 0.131 seconds. Normal real IN-8 passes in the slow step; those distinct outcomes do not close the baseline failure.

The owner attributes this failure to the known IN-8 real-tier transcript race and its separate contracts fix through #312.
The lead's earlier handoff records that race and its planned pin fix. This inspection does not invent a new Core source defect.
The current tested head's `ci/known-flaky.toml` contains only comments and no entries.
It therefore does not satisfy the ordinary known-list re-gate exception on its own.
The owner has requested the lead's explicit decision between a re-gate and later train-import evidence.
I assume no exception, replacement gate, or green acceptance. The current gate remains RED.

### Updated body and R307-SI-4 — LOW — open

I read the current body's complete delta from the previously read body, including the new base-import and RED-gate sections.
GitHub names exact head 2ed85f93 and base 08098782.
Raw JSON SHA256: `90a3b1b9af28e37067153ef12531b7f6d17e9b5755270486186539e8daa9e862`.
Body text SHA256: `1c0e08fd1bc1341eaba98177709e77abeeef354e7a50d3b014a7c9e429faace7`.
The body corrects start_holds to #301 and op_holds to #300.
It preserves the canonical FAIL, both source inspections, current gate RED, and historical evidence scopes.

The body says eleven other paths automatically merged, but it lists nine.
The canonical check names eleven shared paths total, including the two conflicted files.
Required change: state nine automatically merged paths, or eleven shared paths including those two conflicts.
Also label the normal IN-8 positive outcomes by their testkit and real tiers before the failed real baseline.
Request messages: `msg_plugin-w_1791820499_10986a` and `msg_plugin-w_1791820499_985ae5`.
The old three findings stay closed in source. This LOW body correction and the unresolved gate acceptance condition remain.
The formal package delta verdict and lead's gate disposition remain pending. This inspection completes no formal delta round.
I ran no execution job.

### Explicit lead completion path and partial body correction

Owner ruling receipt: `msg_plugin-w_1791820521_a1349f`.
I independently read the lead's live handoff through its 2026-10-12 02:05 PDT entry.
That entry explicitly selects ruling (b): no re-gate of 2ed85f93; FINAL-READY on that head; train 4 imports #312's .44 pin before #307.
The train-4 gate must supply the completed final evidence on its exact composed head.
This is the lead's explicit completion path. It is not an inferred known-list exception or a green result on 2ed85f93.
The failed Stage 2 baseline and current RED gate remain recorded.

I read the corrected body's complete delta on unchanged head 2ed85f93.
Raw JSON SHA256: `b902a7a73c9323d7bdc7a17a13e16da671c8da639f52773f1f33d6ec5ed8a98b`.
Body text SHA256: `b2f9f44bafa07cc27d8483121095ace9a51043c16f965f228e7020674bbfd506`.
The count now correctly says eleven shared paths, including two conflicts and nine automatically merged paths.
The IN-8 outcomes now distinguish normal testkit PASS, normal real slow PASS, and failed unmutated real baseline.
That portion of R307-SI-4 closes.

The same gate paragraph still names #312 in train 3 and an active #308 known-flaky entry.
The lead's latest handoff records #312 leaving train 3 and requires it before #307 in train 4.
The tested 2ed85f93 known-flaky file remains empty.
Required body correction: state the current #312/#307 order and remove the active-list claim, or identify it as historical.
Correction messages: `msg_plugin-w_1791820664_7e6c55` and `msg_plugin-w_1791820664_935ba7`.
R307-SI-4 remains LOW open until that final body correction is read.
The lead disposition is now known. The corrected body and complete formal package delta verdict remain pending.
No new source finding or execution job is introduced by this inspection.

### Final body correction — R307-SI-4 closed

Owner receipt: `msg_plugin-w_1791820675_1e0ee5`.
I verify unchanged GitHub head `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54` and base `080987829a2e2245a7ea97606020400392c974d6`.
I read the complete final body delta. It changes only the stale fix-order and known-list references.
Raw JSON SHA256: `5e71faebcb5c443a2e24f6612bae4fd2100e44afad8cdbf06864cf8f58f06067`.
Body text SHA256: `05305586c1683a2d42fbdaa1988e3eee625e7e9310bbeea1128d39f60f412587`.
The body now names #312 before #307 in train 4 and states that this head's known-flaky file is empty.
It identifies #308's entry as a proposal outside the tested head.
The corrected count, tier labels, RED-gate identity, and explicit no-re-gate ruling remain intact.
R307-SI-4 closes. No source or body finding remains in inspection.
The complete formal package delta verdict remains pending.
The current RED gate and canonical FAIL retain their meanings. No final train evidence is accepted by this inspection.
I ran no execution job.

## Round 3 complete — current-base member delta before the train

Exact head: `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
Imported base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `b117637b809c37c63b869e4da2696015dbdf5918`.
Previously accepted own head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.

I read the complete package Round 4 at `ad4750f9cc30669c2d431814e75c6a4cbb653d55`, including its final body closure.
Package receipt: `msg_plugin-w_1791820875_1727e6`.
That final section supersedes the package's premature 4fb39c62 CLEAN and closes CQ-PC-4/R307-SI-4 after the exact corrected-body read.
The complete package source, conflict, interface, all-list-set, canonical-check, RED-gate, body, and ruling audit agrees with this review.
Both reviews accept the same head, base, tree, and final body hash `05305586c1683a2d42fbdaa1988e3eee625e7e9310bbeea1128d39f60f412587`.

The source delta retains both conflict sides and every accepted own change, except the required deleted Launch test field.
The imported scheduler holds retain the driver's independent room service and budget term.
Failed-write mark retirement, no failed-side Full, cancel precedence, frame-bound sizing, and wake behavior remain intact.
All nine conformance text sets match their independent three-way formulas. No removed base id returns.
The own pending removals remain exactly the two IN-6 ids; the current pending count is 404.
All earlier MEDIUM and LOW findings remain closed. R307-SI-4 closes through accurate shared-path counts, tier labels, fix order, and list scope.
No source, body, or supplied-record finding remains under this exact member review.

The canonical base-merge-check remains FAIL. Its overlaps and conflict resolutions require these two completed manual delta reviews.
The current exact-head gate remains RED at the unmutated real Stage 2 baseline.
It supplies no current Stage 2 catch proof for the thirteen initial misses, and fuzz did not run.
The previously accepted parent gate is not a substitute for the final train gate.
Local filtered-output and numeric-clean-exit limits remain. Imported compatibility, platform, and provenance limits remain.

The lead's independently read 2026-10-12 02:05 PDT handoff ruling permits this member to reach FINAL-READY without re-gating 2ed85f93.
It requires #312's contracts-v0.1.44 structural fix before #307 in train 4.
The exact composed train gate must supply the final evidence after the ordered import and applicable reviews.
This is explicit lead authority. It does not declare the RED gate green or apply the empty known-list exception.
If that ordered structural fix is absent, this verdict supplies no alternative completion path.

This CLEAN covers the exact member source delta, corrected body, and supplied records before train construction.
The ordered pin import, applicable member and combined train reviews, exact-train-head gate, final evidence receipts, and Q7 remain due.
This verdict accepts no train head, final mutation outcome, new pending removal, later source, or merge.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open integration findings; exact member before the ordered final train, under the explicit lead ruling) at 2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54 against imported base 080987829a2e2245a7ea97606020400392c974d6

# Core #273: attach wake and queued cancel

## Round 1 — 2026-10-11

Head: `0eba8683cee771d456b67df0bd40dc2b54fa1f80`.
PR and supplied gate base: `0e1f2feeac9e0aeb46d618b25dcb32f991fcfde9`.
Source merge base: `d582853da895688710ab97f8f1a0858599e8ba18`.
Tree: `06d1774953743142575fd70ed6596576ed6e5209`.
Package CLEAN: `83aa3637df809f9167ab210ef06ee5d98f671a46`, `verdicts/p3-worker.md`, Round 179.

Sol read the complete six-file delta, the complete package round, and the corrected PR body. The committed package file equals the local file Sol read. Sol read the surrounding action, pump, cancellation, route handoff, and real control paths. The source follows the lead's plan-23aw ruling and the steward's cancellation precision.

### Package boundaries and accounting

The wake condition now includes queued actions. The attach call queues its handoff and signals before it returns. The pump performs the handoff. The forwarded cancellation queues its message and signals before it returns. The next pump sends that message. The PR body states the one-pump delivery cost. The existing admission path still supplies cancellation results. Unsent cancellation retains its ready completion step.

The action drain and pump wake-settle logic remain unchanged. Parked work does not become queued work. The negative test calls `set_silence_threshold`. It makes no forbidden negative assertion about a cancellation that leaves no new work. No production test branch, public API, worker link format, or terminal behavior changes.

Exactly `conf::tm_6_begin_and_attach_signal_wake` moves from core-pending to core-real-pending. Sol read its complete .36 transcript. The transcript requires `no_spurious_wakes`, which the real harness does not offer. The supplied gate records its testkit PASS. The move claims no real TM-6 transcript PASS and approves no future R-43(5) pin. The parked-work transcript stays pending for its separate testkit defect. Pending Refused(Full) accounting remains separate, as the body states.

The reported PR base advance changes only testkit `real.rs`. It does not overlap these six paths. The reviewed source head remains fixed. This review accepts no future import.

### Supplied evidence

Gate: `gates/botster-core-stage1-p3-attach-wake-0eba8683-pool-20261011-090728-90026.log`.
SHA256: `e7517e71b109e8a4f0d05db260e45d99bb21d922d50a0633baa7c2b449816858`.

The header names this exact head and the supplied base. All ten steps pass in 276.0 seconds. The default suite has 1716 passes and 480 skips. The slow suite has 550 passes and 2011 skips. Testkit minimum is 57/69. Real minimum is 46/68 passing and 47/69 accepted. The gate records the named testkit transcript and new driver tests as PASS.

Sol read all six raw mutant outcomes and all five catcher records. The five caught identities match exactly. The forwarded-cancellation test catches all five. The cancel body returning Default is unviable. There is no missed mutant, timeout, or stage 2. The diff changes no decoder harness. The job exits zero after 355 seconds. The wrapper exits zero after 356 seconds.

Sol read the complete retained `shared/core-stage1/evidence/p3-attach-wake/reverts-macos.log`. Its runtime proof head is historical `8f97cf9d50b84da503db0da6a4911c60d8726254`. The runtime blobs equal this READY head. Removing action counting fails the attach and cancellation tests. Restoring immediate cancellation delivery fails the cancellation test. Always signalling fails the parked test. The retained correction at `2712a57f1116108c481c34332fafeeaedfdf2057` repeats that negative proof after the permitted test-call change.

The historical testkit comparison shows the attach transcript changing from step-12 failure to PASS. The parked transcript fails step 10 before and after. These records retain their historical execution heads. They do not replace the unavailable real TM-6 transcript.

### Verdict and limits

Package finding F133 is closed by the body correction to plan 23bd. Sol adds no finding. The supplied exact-head gate counts at this unchanged source head. The lead's later base-import and final-gate rules still apply. No future head, pending removal, or pin is accepted.

Sol changed no product code. Sol ran no builds, tests, gates, mutants, measurements, or reversal jobs.

VERDICT: CLEAN (0 open)

## Round 2 — final import, gate, and merge, 2026-10-11

Head: `035baf2c1609526c2388d921a3d8742126e7d67a`.
Base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Tree: `900db43c0cd5e2618c34cd5e2b830fd1a30dc699`.
Package completion: `c4492a4cad52b3195d6dc575257c333af836dfba`, Round 180, read in full.
Owner receipt: `msg_plugin-w_1791737408_ee59c3`.
Package receipt: `msg_plugin-w_1791737617_deabdc`.

Sol read the complete retained base check and final GitHub body.
The head has exact parents 0eba8683 and the base above. Both ancestry checks and all four conditions pass.
The base leaves both pending files unchanged. The new head preserves the reviewed line-set changes.
Sol independently verifies all six own blobs and all 20 imported blobs. The paths are disjoint.
The complete own diff is byte-identical before and after import: 7639 bytes.
SHA256: `4160ec6f09538322e8b5dc4ee8667586ef5259de083a98145668581551403a70`.
The retained check separately hashes the own diff outside the line-set files: 6299 bytes, accf2c8140309d0d94bfa47e2e9851e239b9b2ebdf5450ae683adac1aef1f7b4.
The static Git merge tree equals the head tree above. Q7 carries the accepted source verdict to this exact head.

Gate: `~/botster-sessions/gates/botster-core-stage1-p3-attach-wake-035baf2c-pool-20261011-094156-56422.log`.
SHA256: `9140eb7d57589e600c1e31e0789239fc271fe256cabcc2e5fa4c6bf4a4c86531`.
Sol read the exact-head and base headers, stage summaries, changed tests, all six raw mutant outcomes, and all five catcher records.
All ten steps pass in 138.1 seconds. Default passes 1729 tests, with 480 skipped, in 11.394 seconds.
Slow passes 553 tests, with 2022 skipped, in 41.903 seconds.
All three new driver tests and the named testkit TM-6 transcript pass.
All five unique raw caught identities match the five catcher records. The forwarded-write cancel test catches each mutant.
One cancel-body mutant is unviable. Zero mutants are missed or time out. Stage 2 does not run.
No FLAKY record appears. Fuzz reports no changed crate with a decoder harness.
Job exit zero follows 469 seconds, with 318 queued and 151 running. Wrapper exit zero follows 470 seconds.

The body records the completed final gate and links the passing base check. It labels the earlier gate Earlier.
Package F133 and F134 remain closed. Sol finds no body or source finding.
The one-id move retains its testkit-only scope. This gate proves no real-tier TM-6 transcript PASS on .36.
The historical Mac reversals retain their original heads. Future transcript pins and the parked-work defect remain separate.

Merge: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
GitHub reports MERGED. Git objects show exact parents 42d7056e and 035baf2c and the accepted tree above.
Sol independently verifies that merge. Zero findings remain. No future import, gate, pin, or pending removal is accepted.
Sol ran no execution job.

VERDICT: CLEAN (0 open) at 035baf2c1609526c2388d921a3d8742126e7d67a

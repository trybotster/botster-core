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

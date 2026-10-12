# Core PR #311: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/p3-resize-latest`.
- READY head: `28bd24161430f48e69bc00c275b258e9d9372a99`.
- Imported v1: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Stacked #302 head: `78d7057fca48480e2e8e3174baa2e5c2ab7a70c3`.
- Reviewed delta: the 15-file diff from imported v1, with #302's accepted scope separated from the new resize, Latest policy, testkit, and list changes.

I read #302 integration verdict `a4fade3be3e1dd4b1283c2e97efe9ccf5f18a8c1`, file `verdicts/stage1/p3-route-semantic-input.md`, Round 2. It accepts the stacked head and closes its three findings. #302's final import and merge remain separate. I did not reopen its closed package scope.

### Integration checks

The worker adds one shared `ResizePty(Size)` action. The real driver and testkit driver use the same row, column, and pixel conversion. The system edge uses the existing PTY library. The worker updates the terminal model, advances `model_rev`, reports the size, and sends route modes. A same-size resize produces no new PTY action or size report.

The Latest policy selects the open route with the most recent input and a reported size. Receive order breaks ties. A closing route leaves the candidate set. With no candidate, the size stays. A host resize stays until a policy change. The code preserves cell pixels when a route reports only rows and columns.

Both testkit layers record `SizeChanged` through `Changed::observed`. This closes the oracle gap for a route resize without a host Resize call. The new testkit regression checks the public size and the history-control refusals. The capture-layer regression checks the affected session only.

The three-way v1 import preserves the pending, real-pending, and held sets. I independently compared each merged set with the import formula. There is no unexpected entry. Beyond #302, this PR removes twelve pending entries: nine complete removals and three moves to real-pending.

The nine complete removals cover five DP-5/DP-6 resize identifiers, ST-1 resize, SZ-1, SZ-2, and every-resize completion. The retained .43 comparison reports passes on both tiers with seeds 0–31. SZ-2's listed fence exception permits its host-state completion. The three other passing identifiers retain their real-pending entries because .43 lacks the required fences: `dp_3_route_codec_matches_client_appendix_a4_a5`, `dp_5_no_input_has_a_success_acknowledgement`, and `dp_5_op_is_optional_except_on_a_paste`.

`sz_3_coalesced_resize_is_superseded` stays pending. Its testkit run fails at step 6; its real run is inconclusive at step 6. `dp_6_tie_by_receive_order` stays pending because .43 has no transcript. This round grants no future identifier removal or P5 ReadFacts composition.

### Open package findings

The package reviewer owns these findings. This review retains their status without adding duplicate requests.

- **F145 — MEDIUM — failed spawn drops queued resize requests.** `on_resize` queues requests during spawn. The failed-spawn path drops those requests without `Done`. Required closure includes the failed-spawn behavior proof.
- **F146 — MEDIUM — real test has an unbounded FIFO write.** The SZ-1 test uses blocking `std::fs::write` on the go FIFO. Required closure includes a bounded send and cleanup proof.
- **F147 — HIGH — adoption retains old queued resize requests.** `Early.resizes` survives adoption. A later spawn result can send an old request's completion through the new link after the new host reuses that request number. Required closure includes retirement at the adoption fence and success/failure behavior tests.

Package receipts: `msg_plugin-w_1791785499_636433` and `msg_plugin-w_1791785730_3c4517`. No exact-head package CLEAN is available.

### R311-I1 — HIGH — PTY resize error boundary requires a ruling or repair

Both ResizePty driver arms discard resize errors. The worker reports `Observation::Size` and `Applied` without receiving a PTY result. SZ-1 requires a PTY resize. SZ-3 reports the applied size after the resize. I found no approved anchor that permits Applied after an ioctl error.

The package reviewer raised this boundary in `msg_plugin-w_1791785644_3a8ecd`. I sent the lead QUESTION `msg_plugin-w_1791785722_c15043`. Closure requires a binding anchor that permits this behavior, or a result path with failure handling and a boundary regression. The source comment comparing ResizePty with SignalPayload is not such an anchor.

### Evidence

READY gate:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-resize-latest-28bd2416-pool-20261011-230219-11145.log`

The header names the READY head and gate-time reference `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. That reference is distinct from imported v1. All ten steps pass. Default tests report 1951 passes; slow tests report 630 passes. Mutation totals are 67 mutants, 64 caught, three unviable, zero missed, and zero timeout. The real PTY-size test catches the deleted system resize body in stage 2.

The earlier `ea8385ba` gate remains RED. It reports eight final missed mutants. The READY repair adds tests and changes the policy report flag handling; it adds no mutation exclusion. The completed READY gate does not close the source findings above.

I read the retained .43 comparison script/log and the current local-check record in `shared/core-stage1/evidence/p3-resize/`. The comparison names historical head `424bab51`, a target clean, and the .43 contracts tree. The current local checks name the READY head and another target clean. These records retain their exact source and evidence scopes. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (4 open)

## Round 2: source repair received

- Head: `c31c96737578efa01d7714002f4ca01ce0d6c35a`.
- Reviewed delta: its one commit on `28bd24161430f48e69bc00c275b258e9d9372a99`, three files, 58 additions and 15 deletions.
- Owner receipt: `msg_plugin-w_1791785642_3b5d87`.

The failed-spawn path now replays each queued Resize through `on_resize`, which returns SessionEnded when the payload failed. A new test checks two queued request completions after LaunchFailed. The real test replaces the blocking FIFO write with Blocker and marker-FIFO helpers. Both size reads use deadlines.

These changes address the source causes of F145 and F146. Their package closure and completed replacement-head evidence remain due. F147 still requires the adoption fence. The new failure replay must obey that fence too. R311-I1 still requires the lead's answer or repair.

No replacement-head gate was supplied. I accept no future gate, train head, or merge. I ran no execution job.

VERDICT: NOT CLEAN (4 open)

## Round 3: adoption source repair

- Head: `fd84949c27b2613fa3dbdcc4db4729152c1dd2c2`.
- Reviewed delta: one commit on `c31c96737578efa01d7714002f4ca01ce0d6c35a`; two files, 48 additions.
- Package source-audit receipt: `msg_plugin-w_1791787863_c8009a`.

The adoption fence now clears `early.resizes` before the new link's hello and adoption report. The new test reuses the old request number after adoption. It checks spawn success and failure. Only the new host's completion appears, once; success resizes only to the new host's size.

I read the retained F147 reversal script and log. Removing the clear produces both the old and new sizes. Restoring the clear passes the new test. This supports the source repair. The filtered script retains neither numeric Cargo exits nor an independent target-clean proof. It is supporting evidence, not an exact-head gate.

The package reviewer confirms that the source causes of F145, F146, and F147 are addressed. Their final exact-head closure evidence remains due. F148/R311-I1 remains HIGH OPEN. The lead's inbox denial gives no exception for PTY errors. A source repair can proceed without an assumed answer.

No completed gate or final package verdict on this head was supplied. I ran no execution job.

### Round-limit notice

Package receipt `msg_plugin-w_1791788182_fbb730` identifies the three published NOT CLEAN rounds. BUILD.md's round-limit rule requires the lead to split the PR, settle the disputed point, or replan before round four. I checked that rule at line 119. I will not start integration round four by default. The lead's inbox denial supplies no exception. Source repair can continue, but the next integration round requires the lead's decision.

Owner receipt `msg_plugin-w_1791789432_bb0b8b` reports repair head `6ed3cd7b0fd80ebe0b2e5c14507c2da3f3beb46f` and a completed gate at `gates/botster-core-stage1-p3-resize-latest-6ed3cd7b-pool-20261012-001046-988.log`. The owner reports a PTY-result input, serialized resize actions, success-only model/report updates, error completion, and adoption retirement. This is an unreviewed report. I read no source or gate output for that head and accept no closure from it. Round four still requires the lead's decision.

Package receipt `msg_plugin-w_1791789648_b74233` reports two remaining points at that unreviewed repair head. F148 HIGH remains open for an ignored rollback error: a later cached-old Resize can claim no-op Applied while the PTY differs. New package F149 MEDIUM concerns a queued Policy marker that moves after a later host resize on ordinary input without a new size report or policy-size change. The package reviewer requires failed-rollback and delayed-acknowledgment ordering proofs. These are package reports, not a new integration round or an accepted source result. I did not inspect the reported implementation. The Round 3 verdict below retains its reviewed-head scope.

Package Round 191 is published at `3bf24cb6134c7234633e4e367bfa5691e17bd6f2`, file `verdicts/p3-worker.md`, on reported head `6ed3cd7b0fd80ebe0b2e5c14507c2da3f3beb46f`. I read its published disposition and evidence scope. It closes F145/F146/F147 for package scope through the current-head source and gate. It retains F148 HIGH and F149 MEDIUM, including partial-read reordering. This receipt does not constitute my source or gate review of that head. It does not authorize integration round four.

Owner receipt `msg_plugin-w_1791790911_e58d88` reports later head `e214bbe03bfe5347879f6723f91eef7e96436fc7`. The owner reports repairs for F148/F149, a green gate at `gates/botster-core-stage1-p3-resize-latest-e214bbe0-pool-20261012-003514-95084.log`, and supporting reversal/local logs. I record the receipt only. I read no source or gate output for this head and accept no closure from the report. Integration round four still requires the lead's decision.

Package Round 192 is CLEAN at `e214bbe03bfe5347879f6723f91eef7e96436fc7`. Its verdict is `51304c533d8460204ea4fcdc40c267b1e0a41052`, file `verdicts/p3-worker.md`. I read its published disposition. It closes F145 through F149 for package scope and reports the completed exact-head gate. Package receipt `msg_plugin-w_1791791622_0e8a06` explicitly grants no permission for integration round four. This record supplies no independent integration source or gate acceptance. The lead's decision remains required.

VERDICT: NOT CLEAN (4 open)

## Round 4: lead-authorized repair review

- Head: `e214bbe03bfe5347879f6723f91eef7e96436fc7`.
- Tree: `3782dff7e6759d8e85671b909ce2ebda70d20d0e`.
- Previous integration head: `fd84949c27b2613fa3dbdcc4db4729152c1dd2c2`.
- Reviewed delta: seven files, 692 additions and 62 deletions; the PTY-result interface, restoration, resize order, drivers, and behavior tests.
- Source base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Gate reference: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`, not imported.
- Package verdict: `51304c533d8460204ea4fcdc40c267b1e0a41052`, file `verdicts/p3-worker.md`, complete Round 192.

Lead receipt `msg_plugin-w_1791818404_8a1341` explicitly approves round four at this head. If this round is NOT CLEAN, the PR must split; round five is not authorized. This round preserves the earlier round-limit history.

### Boundary and finding closures

**R311-I1 and F148 — HIGH — closed.** `Action::ResizePty` now has an answer through `Input::PtyResized(Result<(), i32>)`. Both drivers report the syscall result. Both use the raw errno, or EIO when no PTY exists. The real driver queues the answer before a later payload read. The testkit offers the ready answer before payload output or exit input. Both retain the same production worker state machine.

The worker permits one outstanding PTY resize. It changes the model, revision, observed size, and route modes only after PTY success. A direct failure produces Internal and leaves those values unchanged. If the model refuses a size that the PTY accepted, the original completion waits for restoration. Its Internal detail distinguishes restoration success from failure.

Failed restoration prevents a cached-size no-op. A later request for that size asks the PTY and waits for success before Applied. The public behavior tests cover that request, its completion, and the later normal no-op. A result after reap completes SessionEnded. Adoption retires queued host requests and the outstanding request number, including during restoration. It therefore sends no old-host completion through the new link.

**F149 — MEDIUM — closed.** The worker computes the policy result at the input that changes it. A queued policy resize retains that size and arrival position. A partial read or ordinary input with no policy change leaves the queued resize in place. A later actual policy change replaces only the waiting policy entry and preserves host requests. The delayed-answer test checks both partial and complete ordinary input. It observes actions A, B, C and final modes C.

**F145, F146, and F147 remain closed.** The current package review and gate retain failed-spawn completion, bounded real-test FIFO operations, and adoption retirement. I checked those boundary proofs and the driver result mapping. The #302 route-key bound and semantic-input charge remain. The earlier oracle-history repairs remain in both testkit layers.

The list files do not change in this repair. The nine complete removals and three real-pending moves retain Round 1's fence and evidence limits. Host resize coalescing remains outside this PR. No production test branch, timeout increase, mutation exclusion, pin change, or additional pending removal appears.

### Evidence

I read the complete current body from `/private/tmp/review-p3-pr311-e214bbe0.json`. Its current repair descriptions agree with the source. Its historical gates retain their heads. It distinguishes the imported source base from the later gate reference.

I read the restoration/order reversal script and full log. With the earlier worker source and the new tests, three tests fail and Cargo exits 101. After restoration, 225 tests pass and Cargo exits zero. The package clean exit does not prove a full target clean. This reversal supplies supporting evidence only. The separate current local-check record names a full target clean, the exact head, and numeric exits.

Completed gate:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-resize-latest-e214bbe0-pool-20261012-003514-95084.log`

SHA256: `b8ab9ae44c96c4522049f094e88d4ab254e4c63a754a76641a24086a076bb5d1`.

The header names this exact head and the stated gate reference. All ten steps pass. Default tests report 1965 passes and 416 skips. Slow tests report 630 passes and 2051 skips. The new restoration and ordering tests, earlier failed-spawn and adoption tests, and real PTY-size test pass.

I independently matched 92 unique mutation outcomes: 83 caught and nine unviable, with no final miss or timeout. All 83 catches match their citations and have a normal PASS. No FLAKY token appears. Stage 2 catches the Payload::resize replacement through the real PTY-size test. The unviable outcomes comprise five unsupported Default replacements and four non-exhaustive match replacements. Fuzz selects no changed decoder harness. Job and wrapper exit zero after 368 seconds on msa1.

Zero integration findings remain. This CLEAN accepts the complete PR source and READY evidence at this exact head. The final import, applicable train reviews, final train gate, member receipts, and equal-tree merge verification remain due. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

## Round 5: post-CLEAN v1 import source delta

- Head: `2c5d09a0f3e29e443fd8b33d8406bb84cb37940d`.
- Tree: `7c33a94223c0e88baea2701ddbac02d6f0663926`.
- Imported base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Previous CLEAN head: `e214bbe03bfe5347879f6723f91eef7e96436fc7`.
- Three-way base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Reviewed delta: the import's overlaps, list and session conflict resolutions, one obsolete Launch argument, and supplied local/body records.
- Package verdict: Round 195 at `13b850a15a32b4974ee7d5b2b32b4c9d66e2f144`, with complete body/ruling receipt at `39428df35cf6c4c437bce941ad2c2aa04d44b4ca`, file `verdicts/p3-worker.md`.
- DELTA request: `msg_plugin-w_1791829600_714507`.

I read the complete package source round and its subsequent body/ruling receipt. Both accept source only at this exact head. Round 4 was CLEAN, so the lead's conditional split did not trigger. This is the required import delta, not a reopening of the earlier settled findings.

### Import and interfaces

Merge `e2e45f0bf7751361e187c7a743da06dd60d1413c` joins the previous CLEAN head and imported base. The pending conflict preserves both parents' removals: this package's in_1 ID and the base's two in_6 IDs remain absent. I independently checked all nine conformance set formulas and the real-proof formula. Each matches. Final pending is 387, real-pending 38, and real proofs seven. No accepted removal returns.

The session conflict preserves the SZ-1 PTY-size test and the base's LC-5 comment. The following commit removes the old 5000 argument from Session::launch. That matches #305's removal of worker stop_grace: Core owns the deadline and sends Kill/KillPayload at its due pump. Both real and testkit signal mappings remain present.

Sixteen of seventeen own change sets match the previous accepted package diff. The remaining difference is that Launch argument. Resize, no-op coherence, serialized resize work, result handling, restoration, and model update functions remain byte-identical. The route policy, oracle controls, capture layer, sys resize, and real result-mapping blobs remain unchanged. The overlapping testkit and worker code preserves these paths while importing the stop-grace changes. F145–F149 and R311-I1 retain their source closures. The stacked #302 source retains its accepted scope.

### Body precision and evidence limits

**R311-B1 — LOW — closed.** The initial body omitted this current import and still named e214 as current evidence. Its first correction also inferred no slowdown from older Linux timings. I requested current-head provenance and historical timing labels in `msg_plugin-w_1791829844_00cfac` and `msg_plugin-w_1791829961_2cda04`. Owner corrections are `msg_plugin-w_1791829888_34e756` and `msg_plugin-w_1791829977_e3264b`.

I read the complete corrected current evidence section. It names this head/base, both conflicts, the Launch repair, actual local failures, and the deferred train gate. It labels the e214 gate and Linux timings as historical and draws no current-import cost conclusion. The updated package receipt confirms the same correction.

The first local log records target removal on e2e45f0b and 1057 default passes. Clippy and worker slow fail to compile that head's obsolete Launch call. The current log names 2c5d09a0 and an unchanged-worktree cache claim. Fmt, CI taint, clippy, and prebuild exit zero; 141 worker slow tests pass. These records are local evidence only.

The local strict results remain failed: 578 pass and two time out at the two-second budget on this head. The isolated legacy-key trial also times out. The predecessor has three timeouts, including ou_3. Older Linux times do not diagnose these current Mac results. The package reviewer forwards the lead's budget disposition in `msg_plugin-w_1791829985_58e43e`: source may be reviewed now; the Mac records remain unclassified local evidence, and the exact msa1 train gate decides. This is no timeout exception, retry authority, or current-head execution acceptance.

Zero integration source or body findings remain. This CLEAN accepts the import source only. It supplies no FINAL-READY or final evidence receipt. Budget diagnosis, applicable train-head and combined reviews, the exact train gate, final member receipts, and equal-tree merge verification remain due. No future head, gate, pin, status removal, or merge is accepted. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)


## Round 6: train-6 source delta for #311

- Train head: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Train tree: `4b7ace5012cf0543ed64dea143e5db5932c500c2`.
- Train base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- Member order: #303 `df461c83`, #309 `cfcc438d`, #308 `c3bd7fcd`, #311 `2c5d09a0`.
- Lead request: `msg_plugin-w_1791832618_7d2987`.
- Lead correction: `msg_plugin-w_1791832646_cbd010`. I supply member deltas only. Sol supplies the one train receipt.

- Frozen member: `2c5d09a0f3e29e443fd8b33d8406bb84cb37940d`.
- Member source base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Reviewed delta: #311's fit on this train, including the train-5 test changes and #303's worker-clock interface.

All seventeen member file deltas retain exactly their additions and deletions after merge onto #303 at 3837b2c7.
The final worker-core production files, io_decisions, payload resize edge, and session test equal the frozen #311 member.
The capture layer, oracle controls and tests, and resume controls also equal that member.
The route tests retain every reviewed resize test. The sole extra line registers train 5's report_order module.
I read that module. It observes returned LinkSend and PtyWrite actions and checks that client reports precede their PTY writes.
It changes no resize queue, arrival-order decision, adoption retirement, or policy state.
The worker main differs only through train 5's built_in_test macro replacement. The macro preserves the build-location check.
All reviewed resize handling, acknowledgment, failed-size restoration, and Stop/Kill handling remain byte-identical there.

#303 supplies the workers' own clock and changes Workers::run to take no host time.
The train retains #311's PtyResize edge and its result while adapting the surrounding testkit run calls to that clock interface.
No #311 delta disappears during that composition. Sol owns the required combined #303/#311 review; this member check does not replace it.
The pending, real-pending, ledger, and real-proof ID sets equal the #311 member: 387, 38, 691, and seven respectively.
Only #303's existing a2_1_no_deadline_code_exists debt comment differs in pending. No additional ID leaves pending.
The train retains .44 and #308's structured EV-2 entry. It does not contain #316's .46 pin or its IN-8 entry.

Earlier findings remain closed. Earlier local Mac failures retain their classification and evidence limits.
No finding appears in #311's fit on this train. The lead reports successful pre-checks; I do not claim independent log verification.
The gate has not run. Combined review, exact-head train gate, Sol's train receipt, and merge verification remain due.
I supply no final receipt under the lead's corrected instruction. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open, including LOW; #311 source delta only)


### Matching train-6 package delta

I read complete package Round 199 at `8a47bd75d5beb4b93119a69eb4f46e3d049fc0e2`.
Message `msg_plugin-w_1791832907_70b792` supplies that receipt.
It matches the exact train head, base, tree, frozen member, preserved source edits, and zero findings in integration Round 6.
Both reviews retain the lead's pre-checks as reported evidence. Neither claims a completed train gate.
Sol owns the combined review, the dp_3 real-pending trigger check, and the one final train receipt.
No additional review round or final acceptance follows from this matching record.

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

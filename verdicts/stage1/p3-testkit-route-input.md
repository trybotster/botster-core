# Core #279 — route PTY input log

## Round 3 — 2026-10-11 — cumulative base imports

Verdict: source CLEAN (0 open) at `80c5b50cfd4e94efd65b06ea13c045bab91a476f`.
Base: `9f7085f703457b0669785697dbeb14ac687a8942`.
Parents: `e75d804056b080023f077d7780fa16becb53f895` and this base.
Package source CLEAN: `1a3ea8cbda7140a16acd3aef203025a4a7b5f6fe`, Round 186, read in full.
Sol also reads the complete earlier package Round 185 at `39704ff9dad7cad8ce55a0f67e6fb0f983afd567`.
The lead gives this head the final-gate slot after #277 merges. The owner reports that its final gate is running.

Sol reads both complete failed import reports, including the cumulative check:
`~/botster-sessions/shared/core-stage1/evidence/p3-testkit-route-input/base-merge-check-abcbf164-80c5b50c.log`.
Its SHA256 is `fabe4d46e71f1505cc762bf16eff5930bd59d065f5dda610e7771484e0c4b655`.
The cumulative check passes ancestry and the testkit pending condition.
It fails conditions 1–3 and the real pending condition. This round supplies the required explicit delta review.
The resolved conflict affects only the real pending file. The source overlap affects only real.rs.
Q7 automatic carry does not apply.

Sol independently compares every cumulative own and imported path against the accepted head and current base.
All six own blobs outside the source overlap and pending files remain byte-identical.
All 22 changed base paths outside own scope equal their current-base blobs.
The complete own added and removed lines in real.rs remain identical against the new base.
The complete imported added and removed lines in real.rs equal the base changes exactly.
Sol reads that complete import delta: the oracle module, capability term, three dispatch arms, and test entries;
and the worker_file_name capability, its documentation, and its test entry.
The self.tapped guard still covers the complete control expression. The route log and original-instance checks remain unchanged.
The weak tap remains weak. The import introduces no strong reference that would hold the data-directory claim.

The testkit pending file equals its original file minus four own rows and four separate base rows.
No row belongs to both removal sets. The final own count is 455 to 451, with no own addition.
The real pending file equals the accepted file minus the base A2-3 row, followed by the base's complete oracle block.
Sol checks that exact byte composition and verifies that no identifier repeats.
All four own additions and their comments remain intact. Real pending changes from base 32 to own 36.
The minimum file retains the accepted blob and its 69 identifiers.
The four own moves, eighteen relation identifiers, AM-2, and A2-1 keep their accepted scope.

Sol reads the complete current body at this exact head. Its corrected comment and plan-input claims remain intact.
The final body still needs the cumulative imports, complete failed check, both delta verdicts, and completed exact-head gate.
The gate at 6161c5c2 remains historical evidence. No running or future gate is accepted here.
The earlier ST-6 FLAKY contributes no catch. P5 owns its separate diagnosis; this review does not resolve its cause.
F135 through F139 and RI-SI-1 remain closed. No new source finding arises from these imports.
This verdict accepts no later import, pin, pending removal, or merge. Sol ran no execution job.
Wait for the doorbell. Never poll.

## Round 2 — 2026-10-11 — comment correction

Verdict: source CLEAN (0 open) at `abcbf16451e7faa0eca84bca8f3b7ee474ffd880`.
Parent: `6161c5c2122e43b4721497ad787814b108f22169`. Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Risk: HIGH. READY: `msg_plugin-w_1791740717_ac3280`.
Package CLEAN: `c3710e4ca08ab6e3c152b7d2ff89608b5b6cf7eb`, Round 184.
Sol read complete package Rounds 183 and 184, the complete source delta, and the complete live body.
Package Round 183 at `0dd3717f39488ed1c8527e0afc536cf64805ac0d` adopts RI-SI-1 as F138.
That round supersedes Round 182's package CLEAN at the parent. It preserves all other source and evidence conclusions.

RI-SI-1 / F138 LOW closes. Only two comments in tests/route_pty_log.rs change.
The first states that pinned OU-1 step 9 sends bytes 78 with op "1". It states that step 12 reads the log.
It distinguishes unknown log attribution from a frame without an operation identifier.
The second explains that a null-op segment would falsely match written_without_op. The non-null label prevents that match.
These comments match the pinned transcript and matcher facts verified in Round 1. No executable statement or assertion changes.
The body now makes the same distinction. F135, F136, and F137 remain closed.

Package F139 LOW closes. The body labels 23bs as the review input and names the lead's 23bt pin separately.
Sol verifies the unchanged GitHub head and both pin hashes.
23bs SHA-256: `1209ec1cb4b5c38070266fa31d143a9f9ad01986b0de0fa6a5b1fdf865a0a538`.
23bt SHA-256: `a81ae3e282ff913d27f5469abc8c17476c124cf74feaa3e2feb4948bcbcff1ca`.
Sol reads the complete difference between those pins. It changes the R-74 lint count, related note, and history row.
Those terms concern Contracts #52. They do not alter this route-log correction. Full plan review remains separate and queued.

The owner reports two focused Mac tests passing. No raw focused-test log accompanies this READY; Sol records no independent test result.
The complete source behavior, pending accounting, import review, and evidence limits from Round 1 remain unchanged.
The 6161c5c2 gate stays historical evidence at that head. It is not a gate at this new head.
The ST-6 FLAKY result supplies no mutation catch. Its cause remains unknown, and P5 owns its separate structural diagnosis.
This verdict accepts no later import, gate, flake repair, or pending removal.
Both source delta CLEANs now exist. P3 must import current v1 once, supply the applicable base check, and run one final exact-head gate.
Sol ran no execution job. Wait for the doorbell. Never poll.

## Round 1 — 2026-10-11

Head: `6161c5c2122e43b4721497ad787814b108f22169`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Risk: HIGH. Open findings: one LOW.
Package record: `cfa8506c3de87b067222981a3de637cda0701ea4`, `verdicts/p3-worker.md`, Round 182 CLEAN.
Sol read complete package Rounds 181 and 182, the nine-path source delta, full live body, retained evidence, and pinned matcher.

### RI-SI-1 — LOW — the boundary-test comments misstate the pinned frame

`crates/botster-core-testkit/tests/route_pty_log.rs` says the selected OU-1 transcript sends byte 78 without an operation.
Its relation-test comment also says that the route byte had no operation.
At Contracts .36, the transcript's step 9 sends `{"frame":"bytes","op":"1","bytes":{"$bytes_hex":"78"}}`.
Step 12 asserts the literal byte. Sol read that pinned frame and verified both step indexes.
The log lacks known operation attribution. The transcript's frame still carries operation `"1"`.
These are different facts. The incorrect comments misdescribe the pinned boundary proof.

Required correction: describe unknown log attribution and the false written_without_op match that a null segment would permit.
Correct both comments. No production or test behavior change is required.
Finding receipts: `msg_plugin-w_1791740546_f8cc34` and `msg_plugin-w_1791740546_ab13cc`.

### Package findings close in source

F135 MEDIUM closes. The real route stores its original InstanceId and reads the current stored row at its tap.
An absent row or changed instance returns an error. The testkit already makes this same instance check.
The real control test verifies the old route after Remove and after a new Create and Start with the same SessionId.
The retained Mac instance-filter reversal fails with the replacement instance's empty log. Restoration passes.
The final Linux gate passes this test and catches the instance-comparison mutant with it.

The intermediate strong tap retains the host edges and data-directory claim. The final route stores a weak tap reference.
A control upgrades it only for the read. A route whose host has gone returns an error.
The Programs destructor still closes and joins its threads even when an input-log handle remains.
Sol read the intermediate lc_12 step-10 DataDirInUse failure and the final-head Mac reversal/restoration receipt.
The final slow suite and unmutated stage-2 baseline both pass lc_12_drop_leaves_workers_running.

F136 LOW closes. The body now specifies R-73's transcript-frame relations and literal byte log for .40.
It retains the accepted label form only for the current .36 pin. It no longer requires a worker attribution field.
F137 LOW closes. The pending comment and body retain the historical Mac outcome and the lead's later readiness/wake_fd diagnosis.
The .39 adoption and later real-route follow-up remain separate. No additional real pending removal occurs here.
The compact historical dp_3 log prints three failures, but no step or INCONCLUSIVE classification.
Those additional historical details remain owner reports. Sol claims no raw-log verification of them.

### Interfaces, label, and accounting

Both route ends reuse their session's existing input log. They use the same route_pty_log helper and unattributed label.
The pinned codec serializes an operation as a decimal u64 string. `testkit:unattributed` cannot equal such an operation.
Sol read the complete pinned pty_matches function. Literal parts compare the concatenated bytes in order.
Both relation forms reject this non-null label. Extra bytes still fail the final length check.
The boundary tests use the driver with a local transcript change. They add no production test hook or helper-state assertion.
The retained null-label reversal fails the relation test and leaves the literal test passing. Restoration passes both.
The real route tests and retained control reversals cover lookup, dispatch, and advertised control limits.
Probe reports signal the existing wake handles. The new real test uses the existing event wait and cleanup deadline.
The extracted Hub::connected body retains the prior event wait, process identity check, and bound.
No production code, committed transcript, Contracts pin, timeout, or mutation exclusion changes.

Exactly four identifiers leave testkit pending and enter real pending: dp_3_unknown_type_closes_route,
ou_10_no_authorization_in_core, ou_11_transport_ownership_is_defined, and ou_1_attach_hands_transport_to_worker.
All four pass in the final default suite. No new real-tier transcript PASS is accepted.
Testkit pending changes from 459 to 455 identifiers. Real pending changes from 31 to 35.
All 69 minimum identifiers remain identical. Sol scans the pinned transcript blobs and finds all eighteen relation identifiers still pending.
am_2 and a2_1 also retain their pending scope. The real ou_10 echo failure remains unresolved despite two historical passes.

### Base-import delta review

The supplied base check passes ancestry and conditions 1–3. Its real-pending line-set condition fails because the PR adds lines.
Sol read the complete check and performs the required delta review.
The 8b0f53aa import has parents 5c7f1b17 and the reviewed base.
Its complete non-list own diff equals the pre-import diff: 29057 bytes.
SHA256: `249ed058ac2ba8d8a03de357012a7a5d10a5b7a8dcc1dfd1ff7c2e17d70cab59`.
All four imported non-list blobs equal the base. Both pending-file import deltas contain only #273's TM-6 move and comment.
The static merge tree equals the imported tree, `ccdf9be00a2bc468ba46beeb8af677b09fd5a7d5`.
The final weak-reference correction changes only real.rs and real/route.rs. Sol reads that resulting correction in this round.
The final tree is `190fea2826ceeea0be6d323caeb4b278e97f427d`. Its merge base equals the reviewed base.

### Gate and separate flake

Gate: `gates/botster-core-stage1-p3-testkit-route-input-6161c5c2-pool-20261011-102345-10948.log`.
SHA256: `5c663b2ef7c400f687419165365be1a8b008879f6a51c9131b9f65f9f6004387`.
Sol read the exact head/base headers, relevant PASS records, suite summaries, all mutation outcomes and catcher records, and final exits.
All ten steps pass in 435.1 seconds. Default: 1736 passes, 476 skips. Slow: 554 passes, 2025 skips.
The unmutated stage-2 baseline passes. Stage 1 has nine caught, 24 missed, and one unviable mutant.
Stage 2 has thirteen caught and eleven unviable mutants. All 22 raw caught identities match all 22 catcher identities.
Final result: 22 caught, twelve unviable, zero miss, zero timeout. Job/wrapper exits are zero after 442/443 seconds; queue wait is zero.

One ST-6 FLAKY record contributes no catch. The new real route test catches the same Inputs::input mutant independently.
Sol read the full flake note and pinned ST-6 transcript. The transcript has no route input-log call.
The failure text and failing step were not retained. The cause remains unknown; the permitted retry does not diagnose it.
The lead assigns P5 the separate structural diagnosis after the pin PR, with one isolated evidence job.
This review accepts no flake diagnosis, structural repair, suppression, or reproduction campaign.

RI-SI-1 is the only open finding. Package F135, F136, and F137 close in source.
The green gate does not close the inaccurate boundary-test comments. A corrected head requires its own exact-head review and gate.
Sol accepts no future head, merge, pin, or pending removal. Sol ran no execution job.

VERDICT: NOT CLEAN (1 open: RI-SI-1, LOW) at 6161c5c2122e43b4721497ad787814b108f22169

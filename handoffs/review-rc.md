# RC reviewer handoff

Date: 2026-10-11.
Branch: `stage1/review-rc`.
Verdict file: `verdicts/rc-real-controls.md`.
Implementer: `sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`.
Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Integration reviewer: `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`.

Core #246 is CLEAN at `b168c192ff58459b4671834bf01b87cc823c6fb7`.
RC-FH-1 is closed. No finding remains open.
The integration reviewer reports source CLEAN at the same head.
Integration verdict: `745964561c19243f5fe0e032895e9c4c61946498`, `verdicts/stage1/rc-fail-handoff.md`.
Message: `msg_plugin-w_1791710645_32356d`.
The final gate remains required on this head or the later final head before merge.
A later base merge also requires the plan's base-merge record.
The reviewer ran no test or gate.

Core #255 is CLEAN at `15633946e020a9d092ff2fcb8d0bf20560efd202`.
RC-PR-1 and RC-PR-2 are closed. No finding remains open.
The real control reads the current stored row. Both tiers have removal regressions through the control.
The hello test checks only its observable wake signal.
The integration reviewer reports CLEAN at the same head, with 0 open findings.
Integration verdict: `0efa400e`, `verdicts/stage1/rc-program-ready.md`, round 2.
Message: `msg_plugin-w_1791714268_35bab6`.
The reviewer checked the final exact-head gate receipt. Every step passed; all 7 mutants were caught, including 5 in stage 2.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-program-ready-15633946-pool-20261011-032441-61505.log`.
The gate names base `5ac94fabf41909a733267a62b692c1a4ee72bf62` and ends with exit 0.
The integration reviewer reports completion CLEAN in `14f1d685979d04da55dbb86d704217149053ef52`, round 3.
Message: `msg_plugin-w_1791714711_df4927`.
The package reviewer checked merge `c8907b74bd247002784843e1a12f6487b08c579f` in the local Git object store.
Its parents are the recorded base and reviewed head. Its tree has no difference from the reviewed and gated head.
The earlier integration NOT CLEAN verdict was `25a1ca56597ed6ee463e715ab2983b297de873c2`.
The retained Mac failure reproduces on plain v1 and remains explicit in the verdict.
Core #259 is CLEAN at `6b20f6f2116f34526cf6ebf09e263776fff35123`, with 0 open findings.
Base: `d86bfcc747718452efe57b2aa207492acde96573`. Risk: HIGH.
The pin moves from contracts-v0.1.29 to contracts-v0.1.32 under plan 23an, section 0 item (2).
Normal and strict testkit results match by ID. The initial exact-head gate passes all ten steps.
The fenced `ad_7_payload` passes on Mac and Linux. The other six Mac trials remain pending and are not passes.
No ID leaves a pending file. No Core logic or mutation exclusion changes.
Integration CLEAN remains required on this exact head and base.
Under plan section 8 Q8, the supplied green full gate is the merge gate when both CLEAN verdicts cover this unchanged head.
A later head needs its own exact-head gate.
The Mac real log's second header line has a stale v0.1.29 label. Its first header names v0.1.32 and the exact head.
The integration reviewer reports CLEAN at the same head and base, with 0 open findings.
Integration verdict: `aa76bc183ac652775b15aa2f17d4a4aba5dc2513`, `verdicts/stage1/rc-pin-v0132.md`.
Message: `msg_plugin-w_1791715910_b5cd6f`.
Both CLEAN verdicts cover the supplied green gate. Q8 requires no repeat for this unchanged head.
No merge receipt has arrived for #259.
Core #269 is NOT CLEAN at `2a0c4922e93f3bb29234a90fe79fc57db0f7b48d`, with 3 open findings.
The gate base is `56fae5826f766b408abca06f99376e0506982bf9`. Risk: HIGH.
RC-OR-1 is HIGH: the second capture polls unrelated events and holds them outside Core, changing EV-5 admission and retirement.
The steward's R-69 requires `events_pending` refusal, no held or re-injected events, and success cleanup.
Plan 23at separates the transcript fences and pin move from Core #269.
Core #269 repairs the control, proves the real Activity refusal and success cleanup, and restores the three real-pending entries.
The three transcript fences need a contracts PR reviewed by 016c.
P5's separate Core PR pins combined contracts-v0.1.38 after #269 lands on v1.
The placement check and five Mac passes per ID without refusal apply to that later pin PR.
RC-OR-2 is MEDIUM: proofs assert private capture records, marks, changed sets, and history counters.
Replace those assertions with public completions, control results, and boundary observations.
RC-OR-3 is LOW: the prior-art note must state the existing facilities considered and the reasons for the new custom components.
The supplied gate is green, but it does not close the findings. Its three real ST-6b trials pass.
The retained Mac proof runs precede the R-69 repair. The Mac crate failures remain explicit in the verdict.
The reviewer ran no test, build, or gate.
The integration reviewer reports the same three open findings on this exact head and base.
Local integration verdict: `05eb66ad5e0bfc5df7336723e6b3d551750bc1ed`, `verdicts/stage1/rc-oracle-resume.md`.
Message: `msg_plugin-w_1791730003_ebecd9`. The integration verdict is not yet published because its pushes failed on GitHub DNS.
Update: `msg_plugin-w_1791730159_99754d` reports that the integration verdict is now pushed.
The three Core findings and agreed control-only scope remain unchanged. The reviewer waits for a corrected READY.
The remaining RC controls have no submitted head or verdict in this session.
The reviewer waits for a doorbell and calls `receive_messages` once after it arrives.
The reviewer does not poll.

Update: Core #269 round 2 is NOT CLEAN at `eba028f6ec2b4f50dae9c1a3bd73d53b72e15077`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`. Risk: HIGH.
RC-OR-1 and RC-OR-3 are closed. RC-OR-2 remains open (MEDIUM).
Two assertions in `capture_layer.rs` check the private stored mark through `Resumed { mark: 4, ... }`.
Remove those mark assertions. Keep the public `equal` proofs of the output cut and session isolation.
The exact-head full gate is green: 120 mutants, 86 caught, 34 unviable, no miss or timeout.
The reviewer ran no test, build, or gate. The reviewer waits for a corrected READY and does not poll.

Update: Core #269 round 3 is source CLEAN at `c3713dba91e38785eeadeb36444a86f51963a622`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`. All three findings are closed.
The delta removes both stored-mark assertions. It changes one test file only.
The exact-head full gate receipt and integration CLEAN remain required before merge.
The reviewer ran no test, build, or gate. The reviewer waits for a doorbell and does not poll.


### Integration source CLEAN received for round 3

The integration reviewer reports source CLEAN at `c3713dba91e38785eeadeb36444a86f51963a622`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`. All three findings are closed.
Integration verdict: `46a4c33b1453f907c8f5ac5e967c52c563186829`, `verdicts/stage1/rc-oracle-resume.md`.
Message: `msg_plugin-w_1791731482_f3af9f`.
The integration reviewer read package verdict `48e76c5f` and both complete correction deltas.
Both reviewers require the exact-head gate and any applicable current-base import review before merge.
The earlier-head gate supports no pending removal. The reviewer ran no test, build, or gate.


### Base-only source CLEAN carryover

Source head: `1efe84734e6297e1b5c79f123e2161f923dc233d`.
New base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
The implementer's base-merge-check reports PASS in:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r69-base-merge-check-1efe8473-20261011-081147.log`.
It reports no conflict, no overlapping path, an identical own diff, and unchanged pending files.
The reviewer checked both parents in the local Git objects.
The reviewer independently compared all changed paths and the complete binary diff.
The base paths do not overlap the PR paths. The complete own diff is byte-identical before and after the merge.
Under plan section 8 Q7, the package source CLEAN carries to this exact head without a delta round.
All three findings remain closed. The exact-head full gate receipt remains required before merge.
The reviewer ran no test, build, or gate.


### Integration carryover received

The integration reviewer carries source CLEAN under Q7 to `1efe84734e6297e1b5c79f123e2161f923dc233d`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Integration receipt: `51838b763f1a884ed920381a6342dbc60a11c6be`.
Message: `msg_plugin-w_1791731744_a2bca1`.
The integration reviewer confirms the parents, ten disjoint base paths, identical own diffs, and unchanged transferred blobs.
Both source CLEAN verdicts cover this exact head and base. All findings remain closed.
The final exact-head gate remains required. This receipt accepts no future gate or pending removal.
The reviewer ran no test, build, or gate.


## Core #269 — base import delta review — source CLEAN

Source head: `c2dd1fbfe801f3b9e43256071b01081eb75a0592`.
Reviewed base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Risk: HIGH. Open findings: 0.
The base-merge-check fails Q7 because `lib.rs` and `real.rs` overlap. This verdict performs the required package delta review.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r69-base-merge-check-c2dd1fbf-20261011-082659.log`.
The reviewer read every changed line in both overlapping files.
Both files add only the imported `route_reports` registration, implementation, argument type, and control tests.
The imported added and deleted lines equal the base patch in each file.
The import leaves the resume dispatch, capture layer construction, and existing control checks unchanged.
The reviewer also read the imported edge, harness, route-control, and report implementation changes for interactions.
The report reader changes no returned link byte and holds no Core event.
It does not replace the resume control's event polling or capture cleanup.
The other eight reviewed PR files retain identical blobs.
Both pending files equal the new base. The mutation configuration is unchanged.
All three findings remain closed at this exact head.

The reviewer checked the earlier exact-head full gate at `1efe84734e6297e1b5c79f123e2161f923dc233d`:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-resume-1efe8473-pool-20261011-081221-93103.log`.
Its base is `010f2a49952f9b77581f304dbaa41006b115d40b`.
All ten steps pass. It ends with exit 0 and reports 120 mutants: 86 caught, 28 in stage 2, 34 unviable, no miss or timeout.
That gate does not cover the new source head.
The final exact-head gate and integration delta CLEAN remain required before merge.
The reviewer ran no test, build, or gate.


### Integration delta CLEAN received

The integration reviewer reports source CLEAN at `c2dd1fbfe801f3b9e43256071b01081eb75a0592`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Integration verdict: `a255205594efa0c6bc0473736de623b713eb926d`, `verdicts/stage1/rc-oracle-resume.md`.
Message: `msg_plugin-w_1791732967_8d1d20`.
The integration reviewer read package verdict `324ed659`, the failed base check, and both overlapping file deltas.
The integration reviewer confirms the imported changed lines, eight unchanged own blobs, and matching static merge tree.
Both source CLEAN verdicts cover this head and base. All three findings remain closed.
The final exact-head gate and final PR body remain pending. Historical proof limits remain explicit.
The reviewer ran no test, build, or gate.


Core #275 round 1 is NOT CLEAN at `ff913e68dc666eb69fe074bed03fd9ae237a1be7`.
Source merge base: `0e1f2feeac9e0aeb46d618b25dcb32f991fcfde9`. Gate base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Risk HIGH. Two MEDIUM findings are open: RC-OC-1 hand-written expected CPR replies; RC-OC-2 private Changed record assertions.
Derive expected query replies with an independent libghostty terminal. Prove change-record behavior through harness controls.
The supplied re-gate is green: 40 mutants, 33 caught, 7 unviable, no miss or timeout.
Its special pre-review approval and missing plain-v1 reproduction remain explicit in the verdict.
The reviewer ran no execution job. The reviewer waits for a corrected READY and does not poll.


Core #275 round 2 is CLEAN at `147853f8a8ea9c39ab58433d29304c3e75eb8ce5`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`. Risk HIGH. Open findings: 0.
RC-OC-1 and RC-OC-2 are closed. The package reviewer completed the required base import delta review.
Every imported mutation exclusion delta was checked. No exclusion covers the oracle controls.
The exact-head full gate passes all ten steps: 40 mutants, 33 caught, 7 unviable, no miss or timeout.
Integration CLEAN remains required. Under Q8, this gate counts for merge when both CLEANs cover this unchanged head.
The reviewer ran no test, build, or gate. The reviewer waits for a doorbell and does not poll.


### Integration CLEAN received for Core #275

The integration reviewer reports CLEAN at `147853f8a8ea9c39ab58433d29304c3e75eb8ce5`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Integration verdict: `aff5ce79398391b58ea88371350759d230bbb83b`, `verdicts/stage1/rc-oracle-controls.md`, round 1.
Message: `msg_plugin-w_1791740275_9f669f`.
The integration reviewer closes both package findings and completes the import delta review.
The integration reviewer confirms that the static merge tree equals the source head.
The exact-head gate has ten PASS results, 33 matched caught identities, seven unviable mutants, and no miss, timeout, or FLAKY.
Both testkit-to-real pending moves pass on the testkit.
Both CLEAN verdicts now cover the supplied green gate's exact head and base.
Q8 requires no repeat for this unchanged head. A later import requires the applicable review record and exact-head gate.
Historical Mac and retry limits remain explicit. The reviewer ran no test, build, or gate.


### Core #275 — Q7 source CLEAN carryover

Source head: `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
New base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
The reviewer read the complete base-merge-check PASS:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-bmc-ad1a3809-20261011-103813.log`.
The reviewer independently checked both parents and all three base paths.
Outside the pending files, no base path overlaps the PR, and the complete own diff is byte-identical.
The testkit pending file combines the two reviewed removals with the base's separate `lc_2_data_dir_is_exclusive` removal.
The reviewer checked that the merged file equals exactly those combined removals.
The real pending file retains the reviewed blob.
Under Q7's line-set rule, source CLEAN carries to this exact head and base. Both findings remain closed.
The final exact-head gate receipt remains required. The reviewer accepts no future gate result.
The reviewer ran no test, build, or gate.


### Integration Q7 carryover received for Core #275

The integration reviewer carries source CLEAN to `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Integration verdict: `8f449adb861cdcd25b9d0691d0b3a4944bb29484`.
Message: `msg_plugin-w_1791740409_269847`.
The integration reviewer verifies the imported paths, combined pending removals, unchanged real-pending blob, and static merge tree.
The complete own diff outside the list files matches: 52169 bytes, SHA256 `ae788e8b30d7a2cbe068bb2b8197db092b10e10237fa93d73cf2cac3508985e3`.
Both source CLEAN verdicts cover this exact head and base. Both findings remain closed.
The final exact-head gate remains pending. The reviewer ran no test, build, or gate.


### Core #275 — completion CLEAN and merge receipt

The final exact-head gate passes every step at `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Gate base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ad1a3809-pool-20261011-103813-74978.log`.
The reviewer read its header, mutation result, complete summary, and exit 0.
It reports 40 mutants: 33 caught, including 13 in stage 2; seven unviable; no miss or timeout.
The reviewer read the final PR body. It records both source CLEANs, the Q7 carryover, and this final gate.
The final gate is no longer pending. Both findings remain closed.

Integration completion CLEAN: `6288863c2378f47745a9ce2491d407f57e734f78`.
Message: `msg_plugin-w_1791741845_8c4fae`.
GitHub reports #275 merged at `2026-10-11T17:47:34Z`.
Merge commit: `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
The reviewer checked the local merge object. Its parents are the recorded gate base and reviewed source head.
Its tree is identical to the reviewed and gated source head.
The review and completion record is CLEAN with zero open findings.
The reviewer ran no test, build, or gate. The reviewer owns no build target for this PR.

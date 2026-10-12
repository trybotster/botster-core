# Core #328: dp3 real-pending removal

## Source, gate, and expanded-body inspection

- Reviewer: Sol-C.
- Branch: `stage1/p3-unpend-dp3-real`.
- Head: `6b1918c93b49f636bd9a4d0f5808ecf92909fe5c`.
- Tree: `cff0c4b99d6e157b2bb9aba0ab171abd553576e1`.
- Source and gate base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Owner request: `msg_plugin-w_1791837919_445273`, under the lead's train-9 assignment.
- Lead scope: shared core-lead.md lines 1697 and 1700; the 30-ID audit adds no source removal beyond dp3.
- Initial package: `8590b7220d467ed45b29452da4556a46d472ce32`, complete Round 202 in `verdicts/p3-worker.md`.

STANDARD follows the lead's plan-23a rule for this removal-only list change. The lead also requires this integration review.
The complete own diff changes one list file: three comment additions and five removals, including one ID line.
Only conf::dp_3_route_codec_matches_client_appendix_a4_a5 leaves real-pending, from 38 IDs to 37.
Every other conformance text set equals the base. The ID remains in the ledger and minimum set and is already absent from pending.
No Rust, test, pin, transcript, exception, deadline, terminal expectation, or production behavior changes.

I read the complete .44 and .46 dp3 transcript. Their content is identical.
The .43-to-.44 repair adds the final title sentinel and await_title after mode output, then program_ready before route input.
The transcript retains its route barrier and oracle-derived PTY byte floor before route_expect_pty.
The terminal oracle supplies key encoding. No handwritten terminal bytes replace it.
No route-end wait appears. Neither .44 nor .46 has a fence-exception entry for this ID.
The final comment accurately keeps both steps 11 and 9 on dp_5_no_input and step 15 on dp_5_op_is_optional.
Those two IDs retain their exception debt and remain real-pending.

### Exact-head evidence

The train-6 trigger log reports dp3 pending-real PASSED on the exact base.
Trigger SHA256: `4237d5aadea652758d6040dce166067584356a8addcbf088c667364851aaba5d`.
This targeted read supplies no new combined or final train-6 receipt.

Final member gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-unpend-dp3-real-6b1918c9-pool-20261012-133354-1834.log`.
Verified SHA256: `ce853de32931cb1b7b04c1ebb58a0c06c8427f4fe59d3f2d48d70fd0b815c689`.
The header names this head and source base. Both concurrency caps are four. All ten steps PASS.
Default passes 2078 cases; slow passes 678. I verify every normal passing ordinal and unique test identity in both tiers.
Dp3 has normal PASS in conformance at 0.190 seconds and slow_conformance at 0.139 seconds.
The suite treats an ID absent from real-pending as a must-pass trial. This gate does not report dp3 as passing ignored work.
The list step records real-pending 37, pending 387, ledger 691, and minimum 69.
Strict reports 281 passes. Held and separately proved IDs retain their rules.
No runtime FLAKY, RETRY, or INCONCLUSIVE result appears.
The diff changes no Rust source, so mutation selection reports zero and starts no run. Fuzz selects no changed decoder harness.
CI takes 146.0 seconds. Job exits zero after 256 seconds; wrapper exits zero after 257 seconds, with 99 seconds queued and 157 running.
The gate retains its .44 scope. It is not execution of a future imported .46/.49 member or train head.

I read the local log and its producer script. The log names 3e655eff with an uncommitted list edit.
It records 9063 removed files totaling 2.2 GiB, but no resulting tree, patch, or numeric clean exit.
The script selects the real dp3 trial at seeds 0–31. Its local PASS retains that dirty-base provenance.
F155's corrected body states those limits. The pool gate supplies the committed-head evidence.
The superseded 6a635072 gate keeps its separate head and scope. It is not relabeled as the final comment-repair run.
F155 and F156 are closed in the initial complete package record and corrected source/body.

### The 30-ID audit at .46

I compare the complete table with the train-7b gate's thirty passing ignored IDs. The identity sets match exactly.
All 23 exception rows exactly match the .46 exception list, including every step and reason.
I also check each named step in its .46 transcript. Those open entries retain the listed IDs; passing ignored trials remove no debt.
The other seven IDs have no .46 exception. I read their complete relevant transcript forms, recorded pending comments, and cited audit rules.

| ID group | Checked disposition |
| --- | --- |
| dp3 | Its .44 fences remain in .46. The committed member gate proves the real must-pass trial. Only this ID leaves here. |
| in_7 | Step 7 reads pty_queued after the fill. The .46 driver still waits only for Core edges. R-111's bounded kernel-state re-check is absent. |
| ev_5_keyed | Step 76 fences the final model title. Step 80 can match an earlier Activity before the final output report. Step 81's absence check lacks a preceding read-completion fence. |
| ev_5_poll | Step 9 already awaits parked_work under corrected R-87. It fences the parked-work precondition before poll_all at step 13 and strict Woken at step 14. |
| ev_6 | The historical step-60 failure used the earlier 62-step form. The .42 sentinel adds a fence before current state reads and pump_collect; unlisted Activity events are allowed there. |
| in_3 | Step 15 writes through r1. Step 18 strictly reads r2's refusal without ignoring echo output. The r1 barrier cannot order r2's echo. The .39 and .46 transcripts are identical. |
| a2_2_missing_op | Each route read waits for its expected frame or end. The recorded plain-Core step-18 failure remains undiagnosed. The .39 and .46 transcripts are identical. |

The report-order inference agrees with the actual worker source: feed_model calls after_step before on_output calls output_advanced.
Thus the model title can reach Core before its later output observation. Awaiting the title does not prove consumption of that later report.
The a2_2 history comment was added at 530023e6 after route readiness commit 944c885d. The older readiness change is no identified fix for that later failure.

### F158 LOW: stale EV-5 audit row; corrected source description

The initial expanded body copied the historical .39 late-Exited gap and requested a new link-held control.
I identify the existing .46 parked_work fence and Core implementations on both tiers. Package reviewer 0165 independently confirms the same finding identity.
R-87 replaced the withdrawn link_held form because parked work can be a held frame or a parked engine transition.
The first body correction still claimed a particular Exited report. I request both parked-work forms under the same finding.
The final body states the correct precondition, step numbers, historical scope, and batch disposition.

Lead ruling `msg_plugin-w_1791838090_9cc236` keeps this source head and defers ev_5_poll to the post-.49 batch.
Shared core-lead.md lines 1708–1709 also confirm the ev_6 candidate deferral. P3 owns that later batch and its reconfirming gate.
These two IDs are candidates, not unfenced IDs waiting for the claimed missing control. Neither leaves in #328.
The remaining four nonexception IDs retain their current-tag reasons. No earlier failure is treated as a proven current cause without source support.

I read the complete latest body at the unchanged head.
Exact UTF-8 SHA256: `1a347c38e9b39a2eee24392f1e42d76d786d5901621c2dc3f7788f8f5c4fdd2a`.
The 30-row result now matches the source scan and explicit lead disposition. My F158 wording concern is corrected.
The complete matching package review of this expanded body remains due before formal integration CLEAN.
Initial package Round 202 retains its earlier body scope and supplies no automatic acceptance of that expansion.
Later base import, train composition, exact merging gate, and equal-tree/remote merge verification remain due.
I ran no build, test, gate, reproduction, measurement, or pool job. No formal integration verdict is issued in this inspection.


## Round 1: complete source, evidence, and expanded-body CLEAN

- Head: `6b1918c93b49f636bd9a4d0f5808ecf92909fe5c`.
- Tree: `cff0c4b99d6e157b2bb9aba0ab171abd553576e1`.
- Source and gate base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Complete inspection: `063ce874`, above.
- Complete package source and gate verdict: `8590b7220d467ed45b29452da4556a46d472ce32`, Round 202.
- Complete matching package expanded-body receipt: `744bf1e9e410865d43750780c07772926c722db2`, within Round 202.

I read the complete matching package expansion. It matches the exact source head, tree, base, final body hash, and 30-ID audit.
No source or gate changes after the completed inspection. Both reviews confirm the sole dp3 removal and zero added IDs.
F155 and F156 retain their corrected evidence and comment scopes. Shared F158 LOW is CLOSED.
The corrected R-87 row states both parked-work forms. The EV-6 row separates the old failure from its later sentinel fence.

The final body SHA256 remains `1a347c38e9b39a2eee24392f1e42d76d786d5901621c2dc3f7788f8f5c4fdd2a`.
All thirty audit IDs match the train-7b report. All 23 exception rows match their .46 entries and transcript steps.
Four current-tag retention reasons remain valid. Two fenced candidates remain deferred by the explicit lead disposition.
Only dp3 leaves here. The post-.49 batch must supply its own reconfirming evidence and review.
This .46 body audit remains distinct from this member's .44 source and gate. No future pin or batch removal is accepted.

The exact-head gate hash remains `ce853de32931cb1b7b04c1ebb58a0c06c8427f4fe59d3f2d48d70fd0b815c689`.
All ten steps pass. Dp3 has normal default and real must-pass results. The minimum membership remains intact.
The dirty-base local run and superseded gate retain their original provenance and head limits.
This verdict closes the lead's additional integration review against the stated source base.

No source, body, or supplied-evidence finding remains, including LOW findings.
A later base import or train-9 composition still requires its applicable list delta and exact merging gate.
Final train receipt and equal-tree/remote merge verification remain due. No final future-train or completed merge acceptance is supplied here.
I ran no build, test, gate, mutation, reproduction, measurement, or pool job.

VERDICT: CLEAN (0 open, including LOW; exact member source, gate, and expanded body)


### Matching package acknowledgment

Package message `msg_plugin-w_1791838524_76872b` confirms that the reviewer read complete integration Round 1 and its inspection at `e4c8ca5ea3a6bc16ba3fbc19bc555a91414b86be`.
Both reviews match the exact head, base, tree, body hash, 30-ID table, and member gate.
F155, F156, and F158 are closed. The .44 member evidence remains separate from the .46 body audit.
No new review round or final train receipt follows. Later train, batch, and merge requirements remain due.


## Round 2: source fit on replacement train 9b

- Train head: `3e4b3ab8172f1a04467a7969833708606a0f8580`.
- Tree: `fe7700440d2c658dbdfe223ae7b54c17f5516654`.
- Base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
- Frozen member: `6b1918c93b49f636bd9a4d0f5808ecf92909fe5c`.
- Lead assignment: `msg_plugin-w_1791839691_b4a292`; replacement: `msg_plugin-w_1791839933_347904`.
- Applicable complete package Round 203: `1ec68532428da09f782d7aaff787175f0a87f9b4`, read in full.

The lead explicitly carries that package CLEAN from original train 9 to 9b.
Package message `msg_plugin-w_1791840135_e43866` supplies lead carry receipt `msg_plugin-w_1791839963_3908e2` and its independent proof.
The shared lead handoff records the same carry. I independently verify every conformance blob equals its original-train blob.
The #328 and #319 insertion trees also equal the original-train insertion trees. The frozen member and complete own patch stay unchanged.
Only #318's eight paths differ between the trains. No #318 list change is present in either.

The #328 real-pending blob equals its frozen member. Its one removal leaves 37 IDs and restores none.
All other conformance sets retain the accepted base. Dp3 remains in ledger and minimum and outside pending.
The base now has 385 pending IDs; the member does not undo its accepted removals.
The train's .46 dp3 transcript equals the member's reviewed .44 transcript. Its title, readiness, barrier, and oracle floor remain intact.
The corrected 30-row body audit and lead candidate deferrals retain their earlier scope. No additional candidate leaves.
All 27 train-9b set formulas pass. No source or body fit finding appears.

The package carry and this source delta accept no future head or new gate result.
The completed exact-head train gate, combined review, one final train receipt, and lead merge verification remain due.
I ran no execution job.

VERDICT: CLEAN (0 open, including LOW; #328 train-9b list delta only)

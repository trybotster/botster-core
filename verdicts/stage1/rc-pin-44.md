# Core PR #312: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/rc-pin-44`.
- Head: `54805661b93f7acc7c548d0a429d429247c54255`.
- Tree: `02831589acd6254eb42b6bf68b4e8a35cf10ef4f`.
- Source and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Reviewed delta: the complete five-file diff and the new probe-control interface at the pinned tag.
- Assignment: the lead handoff names 017e and Sol-C for the .44 pin. RC also supplies lead receipt `msg_plugin-w_1791783455_ceed46`.

### Integration results

I found no cross-package source defect in this delta.

The seven direct workspace dependencies use `contracts-v0.1.44`. The nine lock records resolve to `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`, which is the tag's commit. The imported base is an ancestor of the reviewed head.

The probe adds opt-in `Report::Printed` and `Request::Recorded` for print history. Core handles the new report variant in its real-program match. Core's probe environment sets only `BOTSTER_PROBE_CONTROL`; it does not set `BOTSTER_PROBE_PRINTS`. Thus this pin does not activate print-history reporting without its serving implementation. The P3 serving PR remains separate. No new oracle acceptance is claimed here.

The route-log tests find their read step by its key. The .44 transcript adds `program_ready` before `route_send`; its existing `await_quiet` remains. The new step shifts the read index from 12 to 13. The tests still replace only that read and cut later steps.

The list delta removes exactly nine real-pending identifiers and adds none. I checked each identifier against the exact-head gate and the pinned .44 fence exception list. Each has a normal real PASS record and no .44 exception. The removals therefore use exact-head results, independently of any earlier baseline claim.

The removed identifiers are `dp_3_data_frames_are_raw`, `dp_5_bytes_and_text_reach_the_pty_exactly`, `dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false`, `dp_5_input_done_only_for_want_result_true`, `dp_5_mode_precondition_zero_effect_reported_on_the_route`, `in_9_key_encoded_by_worker_modes_at_write`, `ou_1_attach_hands_transport_to_worker`, `ou_3_progressing_reader_lossless`, and `ou_9_attach_is_sync_baseline_in_pump`. Other pending identifiers retain their owners and recorded scope.

### Package findings and closure evidence

The package reviewer owns these findings. This round retains their history and adds no duplicate request.

- **RC-PIN44-1 — LOW — closed.** The body now distinguishes seven direct workspace dependencies from nine changed lock records.
- **RC-PIN44-2 — MEDIUM — closed.** The body now distinguishes the #309 observation from a plain-v1 baseline. The package reviewer checked the retained plain `be95ef13` train-1 log: 35 of 43 pending identifiers pass; eight of these nine appear there; `dp_3_data_frames_are_raw` does not. This review makes no causal before/after claim from #309.
- **RC-PIN44-3 — LOW — open.** The body says .44 adds `await_quiet` before the route-log read. The actual inserted fence is `program_ready` before `route_send`. Required closure: correct the body to identify that fence and its index shift.

Finding receipt: `msg_plugin-w_1791786842_1a9504`. Closure and further finding receipt: `msg_plugin-w_1791786942_5caa4a`. The exact-head published package verdict remains due.

### Gate evidence

Log:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-pin-44-54805661-pool-20261011-232531-16645.log`

SHA256: `2ee6636403272d6fd6805631945e20ba967a50390f9e9937ae6108803828935e`.

The header names the reviewed head and base. All ten steps pass. Default tests report 1920 passes; slow tests report 626 passes. The route-log tests pass. The mutation step catches its one mutant in stage 2, with no miss, timeout, or unviable outcome. Job and wrapper exit zero after 277 seconds. These results accept no future train head.

CLEAN requires closure of RC-PIN44-3 and the exact-head package verdict. Applicable train delta reviews, combined review, final gate, and receipts remain required. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (1 open)

## Round 2

- Head: `54805661b93f7acc7c548d0a429d429247c54255`, unchanged.
- Tree: `02831589acd6254eb42b6bf68b4e8a35cf10ef4f`.
- Reviewed delta: no source change. This round checks the published package verdict and final body correction.
- Package verdict: `779edadce8641e4cbcd8d2cb0588c06b1c0dc697`, file `verdicts/rc-real-controls.md`, section "Core #312 — contracts .44 source verdict".

I read the complete published package section. It covers the same head, tree, base, tag commit, and completed gate as Round 1. It reports zero open findings.

RC-PIN44-3 closes through correction receipt `msg_plugin-w_1791786972_17bb6e` and the package reviewer's complete corrected-body check. The body now identifies the inserted `program_ready` fence, the existing `await_quiet`, and the read-index shift. It corrects the commit-message claim without changing source. RC-PIN44-1 and RC-PIN44-2 remain closed.

The package verdict independently confirms all nine normal real PASS records and absent .44 exceptions. Its evidence scope is explicit: strict seed setting 0–31, ordinary slow seed zero, and no separate full-seed real claim. Print-history support remains the separate P3 serving PR.

Zero integration findings remain. This CLEAN accepts the exact PR head and its completed READY gate. Applicable train delta reviews, combined review, final train gate, and member receipts remain required. I ran no execution job.

VERDICT: CLEAN (0 open)

## Round 3: original train 3 composition

- Train head: `3435a6d2ce981ec5c06a888ba907667938f50137`.
- Base: `080987829a2e2245a7ea97606020400392c974d6`.
- Tree: `787fc74f540d554f8e28070456d3cc5feeaa19b2`.
- Reviewed delta: the #312 pin with #310, #308, and #309 imported in that order.
- Package verdict: `7b46286be27fa4ccb952d84fa9425305fb53eda0`, file `verdicts/rc-real-controls.md`, train 3 section.

I read the complete package delta section. The five #312 blobs retain their reviewed source. Its nine real-pending removals remain.

**RC-PIN44-TRAIN-1 — MEDIUM, open composition condition.** The train applies the .44 pin but retains #308's known-flaky entry for `in_8_mode_checked_at_write_start_by_worker`. The entry names `contracts-v0.1.44 pin` as its removal trigger. The mutation stages exclude the fixed test. Retire the entry or record an explicit lead disposition for retention.

The lead removes #308 and #309 from this train in `msg_plugin-w_1791819182_21365e`. The replacement train requires its own exact-head delta verdict. This record accepts no gate or merge. I ran no execution job.

VERDICT: NOT CLEAN (1 open)

## Round 4: replacement train 3c source delta

- Train head: `b78df725177d1a931e1eddb9bb67023e7e0ae645`.
- Base: `080987829a2e2245a7ea97606020400392c974d6`.
- Tree: `43cdf59980d93e0672423bedc0edf506405256be`.
- Frozen #312 head: `54805661b93f7acc7c548d0a429d429247c54255`.
- Reviewed delta: the .44 pin and conformance removals with #310's checkout resolver and test-tier changes.
- Package verdict: `8afd00632950c0e2607e5e2dbc22c76f29ed48f4`, file `verdicts/rc-real-controls.md`, train 3c section.

I read the complete package section for this exact head. Git records #310's merge first and #312's merge second. The train contains neither #308 nor #309.

All five #312 blobs equal the reviewed member head. The seven direct dependencies, nine lock records, probe report arm, route-log test, and nine real-pending removals retain their source closures. I checked all nine conformance line-set formulas against the member base, member head, train base, and train head. Each formula passes. The real-pending list contains 34 IDs. No further removal appears.

I read the imported checkout resolver and its callers. The default-tier check selects the checkout by the exact revision in Cargo.lock. It fails if no checkout matches or several checkouts match. The slow-tier check compares that checkout with Cargo metadata. The resolver therefore follows the .44 lock revision. The shared CI job copy equals the train base. The final gate must still exercise the slow comparison on this train head.

RC-PIN44-TRAIN-1 does not apply to this replacement composition. The known-flaky file has no entry after #308 leaves. Its earlier finding remains recorded for the abandoned train. This closure grants no acceptance to a future #308 composition. The removed #309 member retains its separate composition finding.

The pending list, mutation configuration, process-check allowlist, and known-flaky file equal the accepted train base. This delta adds no exclusion, private-state assertion, or production test branch. P3 print-history serving remains separate.

Zero #312 integration delta findings remain. This CLEAN accepts the source delta only. The other train reviews, final train gate, final member receipts, and equal-tree merge verification remain due. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

### Train 3c gate failure received

Owner receipt `msg_plugin-w_1791820382_511ac7` supplies the completed train-3c gate. Its log is `gates/botster-core-stage1-train-3c-b78df725-pool-20261012-083917-27480.log` under the shared sessions directory.

I read the exact-head header, failure, and final summary. The lists step fails because `conf::a5_2_deferred_completion_keeps_slot_until_polled` is pending and passes on the testkit. Later steps do not run. Job and wrapper exit one. Round 4 accepted source only; this gate supplies no final acceptance.

The owner reports replacement member `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`, importing v1 `08098782` and removing that identifier. The replacement has no pool gate. Its source delta and same-head package verdict remain separate requirements. I ran no execution job.

## Round 5: replacement member after the lists failure

- Head: `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
- Tree: `e7324ae1ff712377222770843ea9dbe4caf64183`.
- Imported base: `080987829a2e2245a7ea97606020400392c974d6`.
- Previous member head: `54805661b93f7acc7c548d0a429d429247c54255`.
- Reviewed delta: the train-2 base import and one pending identifier removal.
- Package verdict: `101a3ac854f4cd0c5007e7cfda684b9cf10d0cce`, file `verdicts/rc-real-controls.md`, complete repaired-member section.

I read the complete same-head package verdict. It reports zero open findings, including LOW findings. The lead removes and unfreezes #312 after the train-3c lists failure. Its repair order defers the full gate to the final composed train after source delta reviews.

Merge `717276302460f3f3bd10d74fbfd6080c51d75b21` has the previous member and imported base as its parents. The next commit removes only `conf::a5_2_deferred_completion_keeps_slot_until_polled` and its obsolete pending comment. All five earlier #312 blobs remain unchanged. The pin, nine real-pending removals, probe report arm, and route-log behavior retain their accepted source.

The pinned .44 transcript uses `pending_ops: 1` and scheduler hold/release controls. The imported base contains #300's accepted scheduler support. The new exact-head strict testkit run reports a normal PASS for this identifier. Its focused real-tier trial also passes. The .44 fence-exception list has no entry for it. The identifier therefore leaves pending and does not enter real-pending. No transcript, budget, or expected result changes.

I checked all nine conformance line-set formulas after the declared removal. Each passes. Core-pending contains 405 IDs; core-real-pending retains 34. No other identifier removal appears. The mutation configuration, process-check allowlist, and known-flaky file equal the imported base. No new exclusion or private-state assertion appears.

**RC-PIN44-4 — LOW — closed.** The repair body initially said no other pending identifier passes without the required held qualification. The package reviewer requested a correction. Its same-head verdict verifies the final wording: unheld pending identifiers fail; the four held identifiers pass under their held rules and retain their status. The earlier body findings remain closed. This correction changes no source.

Local evidence: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/rc-pin-44/strict-c0f8a09c-20261012-154821.log`.

I read the script, exact-head header, commands, exits, and results. Fmt, lists, ledger-ids, and prebuild exit zero. Strict testkit at seeds 0–31 reports 262 passes and zero failures. It checks 318 of 335 pending identifiers; seventeen are real-only, and four use held rules. The focused real a5_2 trial passes with command exit zero. It sets no seed override; the pinned local default is 0–7. No full-seed real claim follows.

The script calls quiet Cargo clean but retains neither its numeric exit nor removal output. Its time marker proves no additional cache or target-writer history. This limitation remains explicit. The failed train-3c gate supplies no acceptance. No pool gate covers this replacement head yet.

Zero integration delta findings remain. This CLEAN accepts source only at the stated head. FINAL-READY can follow the matching delta receipts under the lead's repair order. Applicable train reviews, final composed gate, final member receipts, and equal-tree merge verification remain due. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

## Round 6: train 4 source delta

- Train head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
- Base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
- Frozen #312 member: `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
- Reviewed delta: #312 with #314's seeded-trace test and #307's control-room interface and conformance changes.
- Package verdict: `a401c753c91855cc484db771539e11b395db1ca9`, file `verdicts/rc-real-controls.md`, complete train 4 section.
- Lead assignment: `msg_plugin-w_1791821464_0bdff1`.

I read the complete same-head package section. It reports zero #312 findings. Git records the ordered merges of #314, #312, then #307. #309 and #311 are absent.

Five #312 blobs equal the accepted member head: the manifest, lock, real-program probe handler, route-log test, and real-pending list. The pending list additionally imports #307's two IN-6 removals and debt comments. I checked all nine combined line-set formulas. Each matches. Core-pending contains 403 IDs, and core-real-pending contains 34. The a5_2 removal and all nine earlier #312 real-pending removals remain. Neither am_2 nor ev_9 leaves pending. #307's reviewers own its two removal proofs; the final gate must check the combined lists.

I read the imported control-room interface. SimEdges and EdgeTap return zero room for the selected worker instance. Release restores room and signals the existing wake. The shared host driver computes room and posts SessionWritable. The control injects an edge condition on both tiers; it adds no production test branch or terminal model. It does not change #312's probe environment or Printed report handling.

#307 removes Observation::Writable. No #312 code refers to that variant. The six-times frame bound from #309 is absent; this review accepts no future queue/frame-bound composition. The seeded-trace change affects a test only. Its determinism comparison preserves the scheduler interface used by the a5_2 transcript.

The imported #310 resolver still selects the .44 lock revision. Its slow comparison with Cargo metadata remains due in the final gate. The shared CI job copy, mutation configuration, known-flaky file, and process-check allowlist equal the train base. No new exclusion appears. The earlier known-flaky retirement condition remains inapplicable because #308 is absent.

The package section retains the four body closures and the earlier evidence limits. A reported strict train pass supplies no completed gate acceptance. P3 print-history serving remains separate.

Zero #312 integration delta findings remain. This CLEAN accepts source only at this train head. Sol owns the combined review. Other member reviews, the final train gate, final member receipts, and equal-tree merge verification remain due. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

### Train 4 RED and active producer path

Sol receipt `msg_plugin-w_1791822878_b6b935` supplies combined record `dc4cc6fa5d40edec6d48f786affa71ba3e2f5638`, file `verdicts/stage1/train-4.md`. I read its complete requested EV-2 assessment.

The completed gate is `gates/botster-core-stage1-train-4-03709a0c-pool-20261012-092150-15636.log` under the shared sessions directory. Its final summary is RED at slow. Mutants and fuzz do not run. The wrapper exits one. Sol records EV-2 seed 0, step 71, step_timeout. No final train acceptance follows.

I independently read the complete .43-to-.44 probe control.rs and main.rs diff. Output previously sent directly to the writer channel before Ack. The pinned implementation calls Wire::output, locks WireState, and applies Queue then Ack. Disabling PRINTS_ENV prevents a wait for Recorded but does not bypass that new active path.

This check identifies a changed candidate path, not a diagnosed defect. I cannot certify that the producer path is unchanged. Round 6's source CLEAN grants no final gate acceptance or retry authority. The combined record retains the earlier failure's historical scope. The lead must resolve the changed-path condition before using that assessment as retry authority. I ran no execution job.

### Lead's unchanged re-gate request: REFUSE

Lead receipt `msg_plugin-w_1791823366_af9778` supplies P6's diagnosis. It attributes the failure to Stop racing the PTY read and TitleChanged after await_quiet. It states that Wire can widen that interval without adding a new cause. The lead asks whether this diagnosis permits one unchanged re-gate. Steward Q1 and Q2 remain pending.

I answer REFUSE in `msg_plugin-w_1791823393_44f27a`. BUILD.md's merge-train Red rule 2 and Testing rule 9 require a known-flaky entry, a matching recorded signature, and an open structural fix with an owner. The exact train head's known-flaky list is empty. A diagnosis or earlier failure does not replace those conditions. The pending steward questions supply no explicit exception.

This refusal concerns retry authority under the current rule. It does not reject the diagnosis or establish a product defect. The RED gate stands. No reviewer job runs.

### Lead withdrawal and steward R-107

Lead receipt `msg_plugin-w_1791823475_4e222b` accepts the refusal and withdraws the unchanged re-gate request. The lead will not re-gate `03709a0c`.

I read R-107 in contracts commit `ed7f654f`. It identifies the missing PTY-tail drain before Exited as a Core defect under OU-7. A31, Core #163 part B item B2, supplies the structural fix; P3 owns it. The ruling permits a temporary known-flaky entry for EV-2 with step 71 and step_timeout. Any other signature is outside that permission. P3 must check the exit shape before adding EV-1 or EV-2-retired.

The lead assigns Sol and Sol-C to the new HIGH known-flaky PR. After that PR lands, the lead plans train 4b on the new v1 with #314, #312, and #307. That train requires a new exact-head source delta and fresh gate. A matching known-flaky failure can permit one re-gate only after the required entry and open owned fix exist. This record accepts neither the unsubmitted PR nor a future train. I wait for READY and run no execution job.

### R107-SI-1: acknowledgement boundary unresolved

Sol supplies `53dfc3662afc56799d3a8b047ac13ad645dd89a8` in receipt `msg_plugin-w_1791823691_d35836`. I read its complete question and independently check the pinned probe source.

R-107's case says the Output Ack follows bytes reaching the PTY slave. The source instead queues `(bytes, None)` and sends Ack. The separate writer later calls write_all and flush. Output has no completion sender or write wait. Only program print waits for its write.

An Ack alone therefore proves enqueueing, not the actual PTY write before Stop. OU-7 remains binding for bytes actually written before exit. The timeout's attribution solely to A31 needs a steward disposition for this boundary or retained actual-write evidence. I send the lead supporting QUESTION `msg_plugin-w_1791823724_34f4d9`.

This question does not diagnose another observed defect or reopen the old source verdict. It limits acceptance of the proposed known-flaky diagnosis. The old re-gate stays withdrawn. No future PR, train, or gate is accepted. I ran no execution job.

### R107-SI-1 disposition: both candidate causes retained

Lead receipt `msg_plugin-w_1791823877_ad0305` supplies contracts commit `fe209c65`. I read the complete R-107 follow-up there. It confirms that Output Ack means queued and corrects the earlier premise. The retained logs cannot distinguish unwritten probe output from a missing worker tail drain.

The default Ack keeps its meaning. The new read-only control `pty_output_written` will report bytes whose write_all and flush returned. EV-2 must await that count before Stop. P6 owns the contracts patch. Core must serve the control on both tiers. A31 remains the P3 worker fix for actual tail bytes.

The proposed EV-2 known-flaky entry must name both candidate causes and both owners. It is removed only after both fixes land and the required identifiers pass at their failing seeds. R107-SI-1 closes as a question by this explicit disposition; it supplies no implementation acceptance.

The follow-up also confirms that EV-1 step 10 and EV-2-retired step 121 have no intervening exit. Their fix is an R-57 state fence, not A31 alone. They receive separate known-flaky entries only if they fail before that patch lands, with their own recorded signatures. The sentinel for EV-2-retired must preserve the live-instance and loss-marker conditions.

The new HIGH PR and train 4b still require exact-head reviews. The withdrawn unchanged re-gate stays withdrawn. I ran no execution job.

## Round 7: train 4b base-only delta

- Train head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
- Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
- Previous train: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
- Frozen #312 member: `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
- Reviewed delta: landed #317's known-flaky entry and repository-list test, with no other change from the previous train.
- Package verdict: `6fe61678c003a1205e17a1cfd27f0c9e62ccf648`, file `verdicts/rc-real-controls.md`, complete train 4b section.
- Lead assignment: `msg_plugin-w_1791825491_dd61c1`.

I read the complete same-head package section. It reports zero findings. I checked the ordered merges of the same #314, #312, and #307 heads onto the new base. The complete train difference equals #317's accepted base difference byte-for-byte. Both changed blobs equal landed #317. My complete #317 acceptance is `ee3dd333685af0bca99a8e879db78cebe620690f`.

All six #312 files and all nine conformance files equal the previous train. The 403 pending IDs and 34 real-pending IDs retain their accepted sets. Earlier body closures and interface checks remain valid. #309 and #311 remain absent.

R107-SI-2 governs the imported entry. Its one real EV-2 test has the recorded step-71 timeout signature. The current .44 pin remains before the .45 removal trigger, so the entry is applicable. RC owns removal through #316. A31 remains separate. This entry differs from the earlier obsolete in_8 entry, whose removal trigger was already applied.

The entry affects mutation selection only. Normal slow tests and the unmutated Stage 2 baseline still run it. A failure remains RED. The mutation configuration, parser, filters, budgets, probe, and production Core paths do not change. The entry and one-entry assertion must leave together with the later pin.

Zero #312 integration delta findings remain. This CLEAN accepts source only at this exact train head. The old train gate remains RED and is not re-gated. A fresh train 4b gate, other member reviews, combined review, final member receipts, and equal-tree merge verification remain due. This record authorizes no specific retry or future pin. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

## Round 8: FINAL evidence on train 4b

- Exact train head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
- Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
- Frozen #312 member: `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
- Reviewed delta: no source change; completed train gate, updated member body, and same-head package FINAL.
- Package FINAL: `ff3fb3392fe3c1ebeba5eb507de0e4bc84355a4e`, file `verdicts/rc-real-controls.md`, complete final train 4b section.
- Lead request: `msg_plugin-w_1791827181_b23f12`.

I read the complete package FINAL and current published body. Both identify this exact train, tree, base, frozen member, ordered #314/#312/#307 composition, and gate. The body retains the old RED scopes and labels its earlier missing-gate statement as historical. The package reviewer corrects the extra-LF receipt hash without a source or content change. All earlier findings remain closed, including LOW findings.

Final gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4b-e37b91d6-pool-20261012-102808-61022.log`.

Independently verified SHA256: `97dee8f4d6269dee403c2ff9202709e83d03817bcafd05c04b10cff7725b5d47`.

The exact-head gate passes all ten steps. Default tests report 1957 passes and 427 skips. Slow tests report 633 passes and 2058 skips. The combined lists retain 403 pending and 34 real-pending IDs. All nine #312 real removals and the a5_2 removal have normal PASS records on both tiers. No further identifier removal follows from the passing real-pending trials. Default seeds are 0–31; slow uses seed zero. Earlier Mac trial scopes remain historical; this Linux gate makes no full-seed real-tier claim.

I independently matched 65 unique raw mutation outcomes: 53 caught and twelve unviable. All 53 catches match their citations and have normal PASS records. Stage 1 catches 39; the unmutated 633-test slow baseline passes; Stage 2 catches all fourteen misses. No final miss, timeout, retry, or FLAKY result appears. The twelve unviable outcomes are unsupported default or constructor replacements, not behavioral catches.

The real EV-2 test passes in ordinary slow tests and the unmutated baseline. Stage 2 omits it under the accepted entry. No mutation citation credits it. The .44 checkout comparison with Cargo metadata passes. Both link fuzz harnesses finish their configured runs: 6143074 link_decoder cases and 3614151 msg_decoder cases. Neither reports a crash.

CI takes 962.9 seconds. Job and wrapper exit zero after 1064 seconds on msa1. The known-flaky removal obligation remains: the later pin removes its entry and restores the empty-list test together. A31 and the future OU-7 identifier remain separate. This gate grants no retry authority.

Zero #312 integration findings remain. This FINAL accepts #312's source and completed evidence on this exact train head. Other member FINAL receipts, combined FINAL acceptance, and lead ancestry, equal-tree merge, and push verification remain due. No later head, future pin, #309/#311 composition, or A31 implementation is accepted. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

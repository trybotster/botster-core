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

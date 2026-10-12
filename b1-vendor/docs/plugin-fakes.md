# Plugin fakes and the HP runner (feature plugin-fakes)

Owner: the plugin-fakes pair. Date: 2026-10-01. Contract: HP J4e with A1 and A2; steward rulings R-1.3, R-4, R-6, R-11.

## Prior art (BUILD.md rule 0)

- **Vault.** `botster-v1-plugin-contracts`, "botster plugin frames validate framing before epoch state and type" (R-6), "botster plugin call class selects the lane independently of frame type" (R-4), "botster plugin frame schema owns immutable type and lane bytes", "botster conformance runner limits yield inconclusive outcomes".
- **botster-hub@9bc6cd7d4963ae5cccd642931619b9cac5c8daac.** The plugin supervisor of that tree (`PluginWorkerEngine`, `src/host_executor.rs`, `src/plugin_response.rs`) is an in-process thread host. It speaks no frames, lanes or epochs, so there is nothing to reuse. REJECTED.
- **botster-plugin-test-kit@9bc6cd7d.** A Lua spec runner against a real Hub daemon, with "one settled step at a time" driving. REJECTED as a mechanism: the contract wants language-neutral frame scripts, and the kit has no fake. The idea of settled steps is kept as the `settle` control.
- **frozen/final6/audits/plugin-parity-20260930.md.** It lists plugin behavior to keep. It names no frame behavior, so it fed no fake branch.
- **Reused.** `botster-plugin-contract` (codec, frame types, the call table, `PluginLimits`), `botster-conformance` (engine, matchers, order constructs, seeds), the choice-point pattern of `botster-fake-route`. Nothing is stolen from old trees, so no `Stolen-From` trailer.
- **Libraries.** `rand_chacha` (seeds), `proptest` and `test-strategy` (properties), `jsonschema` (frame checks). Hand-rolled with reason: the lane scheduler and receive queue (`link.rs`). No library models per-lane FIFO with seeded interleave and a held lane.

## Shape

- A transcript frame is `{"lane", "type", "payload"}`. Steps: `send_frame`, `send_invalid`, `send_frame_raw` (`payload_hex`, `payload` or `payload_text` with `{{var}}`), `send_wire`, `close_channel`, `advance_clock`, `control`, `run_suite`. `conn` selects a connection (`main`, `cand` during a reload, a plugin name); engine constructs take `on`.
- `plugin_hub`: the runner plays the plugin against a Hub endpoint (FakeHub in Phase 0). `plugin_host`: the runner plays the Hub against a host with a fixture plugin (FakePlugin in Phase 0). `fixtures/<name>.json` is the language-neutral fixture plugin: registration, entrypoint and handler steps. A real host implements each fixture by name. `fixtures/frames.json` holds payload templates (`{"$ref": ...}`); a test checks that its `hello.limits` equal the fake's test limits.
- `hub_conformance_tests!` and `host_conformance_tests!` give one `#[test]` per id for each subject.
- The Hub controls are `HUB_CONTROLS` and the host controls are `HOST_CONTROLS` in the runner crate. `settle` makes the subject run until it has no more work and puts a `{"settled": true}` item after its frames; it is the barrier of `expect_quiet_until`. `received`, `counters`, `status`, `completions`, `published` and `diagnostics` are observers. A semantic control that only a script can supply (a restarted session, a store) is listed in `setup.controls` of the transcript, and a real Hub adapter maps it. `setup.given` states the Hub state that a real Hub needs for a scripted semantic id.
- The `published` observer returns every collection of the registration (handlers, tools, prompts, resources, timers, event subscriptions, declared events, entity providers, held subscriptions); transcripts assert exact lists, so a registration that stays published is detected. `counters` includes `refused_frame_bytes_buffered`: the body bytes buffered for a frame whose `len` was refused (HP WF-1: none).
- Seeded orders (choice points, HP section 19): the lanes of a receiver are read independently (HP MD-1), so a transcript that needs an order across lanes uses `expect_set`; frame batching; concurrent admission (IV-4, IV-6); event order across emitters (EV-4); a `Result` and a `Cancel` that cross (IV-7); a late `Result` after `Cancel` (IV-2, IV-5).

## R-1.3 check: what FakeHub decides and what the script decides

Protocol state that a clause defines, kept in the fake: handshake and registration state, epochs and the instance, lanes and the receive order, host call contexts (from the `CALLS` table), one active invocation with a bounded queue, deadlines and `Cancel`, limits, `Fault` and the channel close, the reload, unload, crash, restart, quarantine and adoption sequences, the descriptor identity of a handler, the event queues. Decisions from the real Hub's semantic state, which come only from the script (`setup`): the answer to a host call (`calls`), whether a registration is valid (`reject_registration`), a verified proof (`proofs`), the Hello grants and features, the trusted-host lists, semantic controls (`controls`). The script must answer every semantic call with a result and, for a mutating call, an effect; a call without an entry is a script error (the fake panics). The size bound of CA-2 keeps the scripted effect. A test (`calls_and_scripts_follow_appendix_a`) checks that every call in a transcript has the arguments of Appendix A, and that every scripted result has the result shape of Appendix A or is a failure that the call lists. One placeholder remains: a duplicate handler id is refused with `invalid_request` (the code is not asserted).

## Not modeled yet

The bounded loss ledger and its aggregation (EV-12, EV-13) and the retry of a failed gap marker (EV-15). The `hp-suite` features add them with their transcripts.

## Ablations (design 7.2 rules 6 and 7)

Each line is one throwaway local break of one fake branch. After it, the listed transcripts fail at `<id>@<step index>[because]` (seeds 0 to 7). `lh_3_second_host_passes_same_suite` and `tr_3_shipped_host_passes_suite` fail on every host-side break too, because they run the whole host suite. IV-7 is a unit test: the order is the Hub's own, so no transcript may assert it.

- MD-3 a full held lane is not refused (HP MD-3): md_3_full_lane_is_typed@6[HP MD-3]
- MD-1 a lane is not FIFO on send (HP MD-1): ld_3_old_write_authority_fenced@9[['HP LD-3', 'HP MD-1']]
- MD-1 a lane is not FIFO on receive (HP MD-1): md_1_lane_order@8[HP MD-1]; md_3_full_lane_is_typed@9[HP MD-3]
- MD-4 a stale frame is handled (HP MD-4): cr_5_adopt_resumed_fences_old_epoch@11[HP CR-5]; conf_md_4_stale_epoch_dropped (unit test); vr_2_unknown_state_changing_refused@6[['HP VR-2', 'HP MD-4']]
- WF-2 a bad payload does not end the epoch (HP WF-2): vr_2_unknown_advisory_ignored@9[['HP VR-2', 'HP WF-2']]; wf_2_invalid_utf8_ends_epoch@4[HP WF-2]
- WF-2 a malformed request ends the epoch (HP WF-2): vr_4_unknown_authority_fails_closed@4[HP VR-4]; wf_2_duplicate_keys_refused@4[HP WF-2]; wf_3_integer_range@6[HP WF-3]; wf_4_null_vs_absent@4[HP WF-4]
- VR-2 an unknown state-changing frame is ignored (HP VR-2): vr_2_unknown_state_changing_refused@4[HP VR-2]
- VR-2 an unknown advisory frame is not counted (HP VR-2): vr_2_unknown_advisory_ignored@5[HP VR-2]
- WF-1 the frame bound is not applied (HP WF-1): wf_1_len_checked_before_alloc@7[['HP WF-1', 'HP MD-2']]
- HS-4 a missing required feature does not fail the load (HP HS-4): hs_4_needs_checked_at_load@2[['HP HS-4', 'HP A2-27']]
- HS-5 the startup deadline is not enforced (HP HS-5): hs_5_ready_deadline@4[HP HS-5]; lm_2_operations_have_deadlines@11[['HP LM-2', 'HP HS-5']]
- HS-7 a mutating call is allowed in bootstrap (HP HS-7): er_3_detail_code_stability@2[['HP ER-3', 'HP CA-3']]; hs_7_bootstrap_reads_only@4[['HP HS-7', 'HP CA-3']]; ld_3_candidate_bootstrap_is_side_effect_free@6[['HP LD-3', 'HP HS-7']]; vc_4_no_caller_outside_a_request@2[['HP VC-4', 'HP CA-3']]
- CA-1 an unknown call is answered (HP CA-1): conf_er_3_detail_code_stability (unit test); conf_ld_7_no_plugin_initiated_unload (unit test)
- CA-3 an orphan call is not answered cancelled (HP CA-3): ld_3_old_write_authority_fenced@13[['HP LD-3', 'HP A2-11']]; ld_9_committed_never_reported_none@14[['HP LD-9', 'HP A2-11']]
- LD-9 a released paused call is not fenced (HP LD-9): ld_9_paused_old_write_refused_after_activation@14[['HP LD-9', 'HP A2-11']]
- LD-9 a paused call is not held (HP LD-9): iv_4_control_traffic_flows_while_suspended@8[HP IV-4]; iv_4_suspension_admits_nothing@10[HP IV-4]; iv_6_no_reentry_while_suspended@9[HP IV-6]; ld_9_paused_old_write_refused_after_activation@7[HP LD-9]
- IV-4 two handlers are active at once (HP IV-4): er_2_retryable_needs_no_effect@7[['HP ER-2', 'HP ER-5']]; er_5_refusal_before_dispatch_is_none@12[HP ER-5]; iv_4_one_active_handler@6[HP IV-4]; iv_4_queue_full_is_typed@8[['HP IV-4', 'HP ER-5']]
- IV-4 the invoke queue is unbounded (HP IV-4): er_2_retryable_needs_no_effect@7[['HP ER-2', 'HP ER-5']]; er_5_refusal_before_dispatch_is_none@12[HP ER-5]; iv_4_queue_full_is_typed@8[['HP IV-4', 'HP ER-5']]; lm_3_hub_breach_is_refusal@7[HP LM-3]
- IV-3 the long deadline applies to every request (HP IV-3): er_5_cancel_after_dispatch_is_unknown@10[HP ER-5]; iv_2_late_result_dropped@6[HP IV-3]; iv_3_deadline_cancels@4[HP IV-3]; lm_5_limits_overridable_for_tests@6[HP LM-5]
- IV-3 a timeout is reported as no effect (HP IV-3): er_5_cancel_after_dispatch_is_unknown@10[HP ER-5]; iv_3_deadline_cancels@10[HP IV-3]
- IV-7 an expired deadline does not reject a Result (HP IV-7): iv_7_expired_deadline_rejects_a_result_before_the_wake (unit test)
- IV-8 a stale descriptor is dispatched (HP IV-8): er_3_detail_code_stability@8[['HP ER-3', 'HP IV-8']]; er_5_refusal_before_dispatch_is_none@12[HP ER-5]; iv_8_queued_action_not_retargeted@11[HP IV-8]; iv_8_stale_descriptor_refused_at_dispatch@4[HP IV-8]
- VC-1 a missing proof is the operator (HP VC-1): vc_1_caller_is_total_for_every_origin@4[HP VC-1]; vc_1_missing_is_system@4[HP VC-1]; vc_7_event_source_separate_from_caller@4[HP VC-7]
- VC-1 an unknown proof is the operator (HP VC-1): vc_1_missing_is_system@7[HP VC-1]; vc_3_old_token_refused_after_restart@8[HP VC-3]
- ER-5 a handler failure is reported as no effect (HP ER-5): er_2_retryable_is_not_safe_to_repeat@9[['HP ER-2', 'HP ER-5']]; er_5_earlier_calls_stay_committed@8[HP ER-5]; er_5_host_result_effect_is_per_call@10[HP ER-5]
- ER-5 retryable survives without an idempotency key (HP ER-5): er_2_retryable_is_not_safe_to_repeat@9[['HP ER-2', 'HP ER-5']]
- CR-2 an in-flight request is not failed on a crash (HP CR-2): cr_2_crash_fails_inflight@8[['HP CR-2', 'HP ER-5']]; cr_7_only_the_store_is_durable@12[HP CR-7]
- CR-3 a second crash does not quarantine (HP CR-3): cr_3_restart_once_then_quarantine@10[HP CR-3]; cr_8_failure_isolation@13[HP CR-3]
- LD-3 the handover does not wait for the drain (HP LD-3): ld_10_index_rebuild_aside_then_switch@11[HP LD-10]; ld_3_reload_procedure_order@11[HP LD-3]; ld_8_no_mutation_overlap_across_switch@9[HP LD-8]
- LD-4 the unload does not drain (HP LD-4): ld_4_shutdown_drains_then_kills@7[HP LD-4]
- TR-1 every host is trusted (HP TR-1): tr_1_unlisted_host_refused@2[HP TR-1]
- CR-5 a Hub restart keeps the old requests (HP CR-5): cr_5_adopt_resumed_fences_old_epoch@16[HP CR-5]
- IV-8 a duplicate handler id loads (HP IV-8): iv_8_duplicate_handler_fails_load@2[HP IV-8]
- HS-3 the host picks the lowest version (HP HS-3): hs_3_negotiates_highest@1[HP HS-3]
- IV-2 an oversize result is sent (HP IV-2): iv_2_oversize_result_is_response_too_large@4[HP IV-2]
- LM-7 the memory cap is not enforced (HP LM-7): lm_7_host_memory_exceeded_is_typed@4[HP LM-7]
- WF-5 the host does not enforce the deadline (HP WF-5): wf_5_relative_deadlines@7[['HP WF-5', 'HP LM-4']]
- CR-5 adoption loses the process memory (HP CR-5): cr_5_adoption_keeps_memory_reload_does_not@12[HP CR-5]
- ER-1 an unknown kind is passed through (HP ER-1): er_1_closed_kinds@6[HP ER-1]
- VR-4 a malformed Invoke is not refused (HP VR-4): vr_4_unknown_authority_fails_closed@4[HP VR-4]
- IV-5 a Cancel does not abandon the handler (HP IV-5): iv_5_cancel_idempotent@8[HP IV-5]
- LM-4 a runaway handler is not stopped (HP LM-4): lm_4_runaway_handler_is_failed_not_fatal@4[HP LM-4]
- HS-6 a resumed Hello runs the entrypoint again (HP HS-6): cr_5_adoption_keeps_memory_reload_does_not@14[HP CR-5]
- MD-1 a Result goes before its calls are answered (HP MD-1): er_1_closed_kinds@6[HP ER-1]; md_1_result_after_calls@5[HP MD-1]
- WF-1 the body is buffered before the length is checked (HP WF-1): wf_1_len_checked_before_alloc@8[HP WF-1]
- CA-2 the size bound replaces the scripted effect (HP CA-2): ca_2_response_too_large_keeps_the_scripted_effect (unit test)
- LD-3 the old generation stops serving while the candidate prepares (HP LD-3): ld_3_reload_procedure_order@6[HP LD-3]
- CR-2 a crash leaves the registration published (HP CR-2): cr_2_crash_closes_subscriptions@7[HP CR-2]
- HS-2 a rejected registration has no typed reason (HP HS-2): hs_2_rejected_registration_tears_down@2[HP HS-2]
- LD-6 an unload leaves the registration published (HP LD-6): ld_6_unload_removes_everything@9[HP LD-6]

## hp-calls-a: host call transcripts (section 9, 9.2 to 9.5, 9.9, 9.10)

Owner: the hp-calls-a pair. Date: 2026-10-01. 145 ids of contract `hp`: CA-1 to CA-5, CA-9, CA-12 to CA-22, CF-1, ST-1 to ST-12, PX-1 to PX-4, FS-1 to FS-3.

### Prior art (BUILD.md rule 0)

- **Vault.** `botster-v1-plugin-contracts` and "botster plugin call class selects the lane independently of frame type" (R-4.3: wrong lane is `invalid_request`, not a `Fault`).
- **botster-hub@9bc6cd7d4963ae5cccd642931619b9cac5c8daac.** `src/lua_runtime.rs`, `src/capabilities.rs` and `crates/botster-plugin-test-kit/src/spec.rs`, read for the shapes of `plugin_db`, `timer_once` and `session_types` only. They carry no frames, contexts or `effect`, so they gave no expected value. REJECTED as a source of behavior.
- **frozen/final6/audits/plugin-parity-20260930.md** (G-2, G-3, G-6, G-12, PP-9, PP-15): the reason for `process.run`, the worktree request, `fs:<root>` and the notification policy. It fed the `given` text of the spawn, process and fs ids, not an expected value.
- **Reused.** `botster-plugin-contract` (`CALLS`, the Appendix A schemas), the runner and the FakeHub of plugin-fakes. Nothing is stolen from old trees, so no `Stolen-From` trailer.
- **Hand-rolled with reason.** The 145 transcripts come from one generator in `~/botster-sessions/shared/foundation/scratch-hp-calls-a/` (`lib.py`, `gen1.py` to `gen6.py`). Each transcript is plain JSON in `conformance/hp/`. The generator is scratch, not part of the repo: it only avoids 145 hand-typed copies of the Hello, Ready, Activated and Invoke preamble.

### What a transcript here asserts (R-1.3)

- **Run against FakeHub (not `structural`).** The exchange has one scripted answer (`setup.calls`; `setup.given` states the Hub state that a real Hub needs to give the same answer), and the clause fixes the asserted fields: the call shape of Appendix A, the failure kind and `detail.code`, the `effect`, the context (CA-3), the lane (CA-9, R-4.3), the ordering with the `Result` (MD-1), and the answers that the fake itself gives (CA-1 `unknown_call`, CA-2 `response_too_large`, CA-3 `wrong_context` and `cancelled`, IV-4 one active handler for CA-4, the published timers for CA-12). A value that the clause leaves open is never pinned: a revision, an operation id, a session id, a worktree id, a cursor, a sequence number, a port and the text of a redaction are bound (`"$x"`), typed (`$type`) or related (`$ne`, `$between`, `$not`). Ids that a call returns are bound and reused in later arguments, so a real Hub can run the same steps.
- **`fake: structural`, ruling R-1.3 (65 ids).** The outcome needs real Hub state or a real process. Each has full steps for the real Hub: a stimulus or an observer for the named behavior, and an assertion that cites the clause. Durability and kill -9 (ST-9), index rebuild (ST-1, ST-5), watch coalescing (ST-6, FS-3: `commit_burst`), the projection (ST-7: `entity_snapshot`, `entity_diffs`), accounting (ST-8), the port allocator (CA-20), the context file (CA-14), the process group (PX-3: `process_group_alive`), the argv and the environment (CA-19, PX-1: `spawned_command`), the Hub log (CA-9, CA-19, PX-2: `hub_log`), the symlink race (FS-2: `fs_race`), the notice path (CA-22: `terminal_notification`, `session_notifications`), the client route (CA-18: `client_route`, `client_input`), grants (CA-19, PX-4, CA-21: `setup.grants` and a second plugin), the starting reservation of a worktree (CA-21: `hold_spawn`, `release_spawn`). The controls are named, with their argument and result shapes, in the doc comment of `HUB_CONTROLS`. FakeHub does not implement them.
- **Host side (`plugin_host`).** `ca_5_unknown_is_not_retryable_blindly` (no second `HostCall` after `effect: unknown`; runs against FakePlugin). `ca_16_unknown_kind_waits_for_terminal` is `fake: structural`, ruling R-1.4: its fixture `host_ops_gate` retains the operation and emits `workflow_done` only when a notification record has `terminal: true`. FakePlugin does not implement that conditional, so a real host runs it.

### Findings for the lead and the steward

1. **No grant check in FakeHub.** CP-3 (`conf::cp_3_every_call_checked`) owns the per-call grant check and it is not in this feature's list. A denied call (`capability_denied`) in these transcripts is a scripted answer, or a `structural` transcript whose `setup.grants` states the grants of the real Hub.
2. **Matcher predicates.** `$not`, `$ne`, `$starts_with`, `$contains`, `$between` and `$array` were added to the Rust engine with vectors (design 5.2; lead-approved). The TypeScript engine implements them with the TypeScript runner.
3. **No fake change, no new fake lines.** The wrong-lane rule (R-4.3) already lives in the decoder of `botster-plugin-contract`. `ca_9_log_limits` is its first transcript.
4. **Not covered by a transcript.** `ca_16_operation_record_is_discriminated` creates the kinds spawn, safe_write and process in the transcript. The kinds http, ws and plugin_call need an outside server or a second provider; their per-kind states are the ids of A2-4.

5. **Package admission fixtures.** `admit_manifest` takes a complete package file: the `manifest` and `accessory` templates of `fixtures/frames.json` with only the field under test changed. A test (`admitted_package_files_are_complete_and_valid`) checks that the four required manifest keys are present (R-6.1) and that every file that must be admitted validates against its schema.
6. **Accessory schema (fixed, lead ruling).** `accessory.schema.json` lacked the legacy `port_forward` marker that HP A2-40, CA-20 and LB-19 name. It is now an optional bool on `Accessory` in `botster-plugin-contract` (its own commit), with a test of the one-to-one mapping to `allocate_port`. The legacy transcript's file validates against the schema like every admitted file.
7. **Pagination.** CA-13 promises a bound, not full pages. The list transcript asserts `$array` with `max: page_items` and a `next_after`; it does not loop over pages (a transcript has a fixed number of steps).
8. **Process cleanup (PX-3).** Each cleanup transcript waits on `await_operation` for `running`, asserts `process_group_alive: true`, triggers, waits on `await_operation` for terminal, and then asserts `false`. A queued command cannot satisfy it.

9. **Redaction (CF-1, PX-2).** A redacted value must be present and must hold no occurrence of the secret; the redaction text is not fixed by the contract, so any text without the secret passes. The `$b64` matcher decodes a captured tail before `$contains` looks for the secret, and the public marker `OUTPUT-7731` must survive (explicit `stdout_tail_bytes` and `stderr_tail_bytes`). `the_px_2_redaction_matchers_judge_samples` runs the matchers of the structural id over a leak, a lost tail, a missing tail and a permitted redaction; CF-1 mutations (absent, leaked, embedded, permitted text `[redacted:bnRl]`) at each of its three assertions fail or pass as expected.
10. **Worktree identity.** A transcript waits for the Hub's own completion with `await_operation` (a scripted no-op on the fake) and reads the final record once through `operations.get`; nothing injects a notification that a real Hub would have to produce.

### Ablations

Each line is one throwaway local break. After it, the listed transcript fails at `<id>@<step>`, the assertion that cites the clause (seeds 0 to 31).

- The decoder accepts a bulk-class call on lane 2 (R-4.3, CA-9): ca_9_log_limits@8
- An unknown call has another detail code (CA-1): ca_1_unknown_call_refused@6; st_12_no_change_log_in_v1@6; fs_1_no_write_call_exists@6; cf_1_own_config_only@12
- The size bound is not applied (CA-2): ca_2_result_bound@6
- A call that names a finished request is answered by the script (CA-3): ca_3_orphan_call_answered_cancelled@8
- A bootstrap call is allowed after `Activated` (CA-3): ca_3_contexts@10
- `autonomous` is not a closed set (CA-3): ca_3_contexts@16
- Two handlers are active at once (IV-4, CA-4): ca_4_no_reentrancy@7; ca_16_completion_is_a_notification@8; px_1_is_an_async_operation@7
- Timers stay published after a crash (CR-2, CA-12): ca_12_timers_are_rearmed_after_restart@5

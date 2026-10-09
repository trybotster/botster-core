# The minimum Core (plan revision 23; approved 2026-10-09)

Status: APPROVED by the orchestrator and the user on 2026-10-09 (plan revision 23). The id list is `docs/stage1-clauses/minimum-core.txt`. The answers to the borderline questions are in section 8.
Date: 2026-10-09.

## Sources

- Id lists: `trybotster-botster-core-stage1-plan/docs/stage1-clauses/*.txt` (673 ids), owner rules in `owners.py`.
- Live ledger: `origin/v1` at `aaac0c0d` (`conformance/core-ledger-ids.txt`, 675 ids; `conformance/core-pending.txt`; `conformance/core-deferred.toml`).
- Contract: `botster-contracts/frozen/current/core-contract-v1.17.md`, Amendments 2 to 16, errata 1 to 4. Codec: `route-codec-v1-extract-rev30.md`.
- Caveat: Amendment 3 re-pins the codec to revision 36. That file is not in `frozen/current`. Codec citations below are from revision 30 and are unverified against revision 36.

## The goal and the test

One real Hub session runs end to end:

1. The host opens Core.
2. It spawns a session (a worker with a PTY running a shell or program).
3. A client attaches and sees output, including a baseline for a late attacher.
4. The client types input that reaches the program.
5. The session ends cleanly (program exit, and a host `Stop`).
6. The host restarts, re-adopts the running worker, and the session continues.

An id is IN only if one step fails, or is unsafe, without it. "Unsafe" means: Core signals the wrong process, loses or corrupts the registry, or lets one session hang Core or grow memory without bound. Error paths that a first real session will hit are IN at the minimum needed to report them. Everything else is DEFERRED.

Assumed minimum configuration (each one removes a family of ids):

- The client attaches over `RouteTransport::Stream` (the codec's `local-stream` binding, TB-L2). See section 5.
- The route is attached with `answers_queries: false`, as the section 15 example consumer does ("it attaches with `answers_queries: false`, so the shadow answers every query at once").
- The route is attached with `history: none`. The baseline is the screen only.
- The client sends `bytes` or `text` frames, not semantic `key` frames. See borderline item B1.
- The session runs at its spawn size. See borderline item B3.
- No services are spawned.

## 1. Summary

| Package | IN | DEFERRED | Total |
|---|---|---|---|
| p0-skeleton | 2 | 0 | 2 |
| p1-lifecycle | 31 | 95 | 126 |
| p2-terminal | 1 | 5 | 6 |
| p3-worker | 2 | 165 | 167 |
| p4a-routes | 18 | 134 | 152 |
| p4b-queries-files | 1 | 67 | 68 |
| p4c-webrtc-perf | 0 | 15 | 15 |
| p5-adoption | 14 | 41 | 55 |
| p6-testkit | 0 | 15 | 15 |
| p7-services | 0 | 67 | 67 |
| **Total (clause files)** | **69** (70 before revision 23k) | **604** | **673** |
| other: in the v1 ledger, no owner | 0 | 2 | 2 |

The "other" row: the v1 ledger has 675 ids. Two of them, `conf::a3_1_route_too_small_for_the_worst_case_minimum_frame_gets_no_feature` and `conf::a3_1_minimum_frame_includes_truncated_and_the_widest_count`, were withdrawn by Amendment 9 (A9-2). `owners.py` skips withdrawn ids, so they are in no package file. They are not in `core-pending.txt`.

## 2. The IN list, by step

### Step 1. The host opens Core (15)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::e1_1_worker_found_by_host_supplied_path` | E1-1 | Core finds the worker only by the host's path ("Core fixes no file name for it"). Without it no worker starts. |
| `conf::th_1_core_is_send_not_sync` | TH-1 | "`Core` is `Send` and not `Sync`." The Hub must move the handle to its loop thread. It is a compile test. |
| `conf::lc_1_open_needs_worker_path` | LC-1 | Bad config: "A missing `worker_path` is `MissingWorkerPath`." A first install will hit it. |
| `conf::lc_2_data_dir_is_exclusive` | LC-2 | Registry safety: "A second `open` on the same directory fails `DataDirInUse`." Also, "the lock is released when the handle is dropped or the process dies", which step 6 needs. |
| `conf::lm_1_zero_limit_is_invalid_config` | 9B | Bad config: "a zero, or a value above a stated ceiling, makes `open` fail with `InvalidConfig`." A zero `stop_grace` or `mandatory_events` would break steps 5 and 6. |
| `conf::or_1_begin_does_not_complete` | OR-1 | The host loop model: "Events appear only after a `pump`." |
| `conf::or_1_no_progress_outside_pump` | TM-2 | "Progress happens only in `pump`." The host loop depends on it. |
| `conf::tm_3_next_deadline_is_host_armed` | TM-3 | "The host arms its own timer" from `next_deadline()`. Without it the startup and `stop_grace` deadlines never fire. |
| `conf::tm_3_due_deadline_fires_in_pump` | TM-3 | "A deadline that is due at a `pump` is processed in that `pump`." The `stop_grace` kill of step 5 depends on it. |
| `conf::tm_6_begin_and_attach_signal_wake` | TM-6 | "`begin`, `attach` … signals the `WakeHandle` before it returns." Without it the host sleeps on runnable work. |
| `conf::a2_7_pump_report_fields` | A2-7 | `PumpReport{more, events_posted}` tells the host to pump again. |
| `conf::er_0_sync_errors_allocate_no_op` | ER-0 | The error model: "`begin` returns the error, allocates no `OpId` and posts no event." The host must report refusals. |
| `conf::er_0_async_errors_arrive_as_completions` | ER-0 | "Every failure that needs the worker or the registry is asynchronous: it arrives as `Completed{op, Err(code)}`." |
| `conf::am_3_exactly_one_completion` | AM-3 | "Exactly one completion per admitted operation, including when the session stops … or the worker link fails." A missing completion hangs the host. |
| `conf::ev_5_no_mandatory_event_lost_or_replaced` | EV-5 | `Completed` and `SessionState` are "Never dropped or replaced." The host's view of the session depends on them. |

### Step 2. The host spawns a session (12)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::lc_3_create_then_start` | LC-3 | The spawn path: `Create` makes the row, `Start` launches it. |
| `conf::lc_3_duplicate_id` | LC-3 | Registry safety: "A duplicate id is `IdInUse`." A Hub that re-creates an id after a restart must not overwrite a live row. |
| `conf::am_1_create_then_start_same_turn` | AM-1 | "`Create(A)` then `Start(A)` before a `pump`: the `Start` is admitted." The Hub does both in one turn. |
| `conf::am_1_double_start_refused` | AM-1 | Safety: "two `Start(A)` before a `pump`: the second is `WrongState`." Otherwise two payloads could run for one row. |
| `conf::tm_6_create_without_worker_input_wakes` | TM-6 | "`Create`, which needs no worker" must still wake the host, or step 2 stalls. |
| `conf::or_2_session_order` | OR-2 | "`Completed{Start}` after `SessionState{Running}`, or after `SessionState{Exited\|Lost}` when the start failed." The host's state machine depends on this order. |
| `conf::lc_4_start_failure_is_typed` | LC-4 | Worker fails to start: "`StartFailed{reason}` and the row is `Exited`/`Lost`, never `Starting`." |
| `conf::a2_1_missing_cwd_is_start_failed_cwd_missing` | A2-1 | A first real error: "A `cwd` that does not exist at launch completes `Start` with `StartFailed{CwdMissing}`." |
| `conf::a2_1_empty_argv_is_invalid_input` | A2-1 | Bad request: "an empty `argv` is `InvalidInput`." Refused before it reaches `exec`. |
| `conf::a2_1_size_range_zero_is_invalid_input` | A2-1 | Bad request: "a zero, or a value above 65,535, is `InvalidInput`." A zero-size PTY and model is unsafe input (unverified what libghostty does with it). |
| `conf::ad_7_payload_launches_after_durable_identity` | AD-7 | Step 6 safety: "only then does Core tell the worker to launch the payload." A payload never runs without an identity a later host can find. |
| `conf::lc_9_get_list` | LC-9 | `get` and `list` are the host's reads of the session record. LC-11 says the host "builds its view from `list()` at start". |

### Step 3. A client attaches and sees output (16)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::a2_1_attach_state_rules` | A2-1 | "`attach` … is admitted for a session in `Starting`, `Running` or `Exited`." The Hub attaches right after `Start`. |
| `conf::ou_1_attach_hands_transport_to_worker` | OU-1 | The attach call: "`transport` is the client's connected stream endpoint, handed to the worker." |
| `conf::ou_1_terminal_format_is_negotiated_against_the_target_worker` | OU-1 | The worker "picks the first it can emit and announces it as `attached.terminal_format`." The client needs it to decode the screen. |
| `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy` | DP-2 | Descriptor safety: "the host must not read, write or close it by number, and Core closes the host's copy once the worker has its own." |
| `conf::dp_2_failed_handoff_closes_route_handoff_failed` | DP-2 | Error path: "if the handoff fails the route closes `HandoffFailed` and Core closes the descriptor." |
| `conf::ou_9_attach_is_sync_baseline_in_pump` | OU-9 | "The worker takes the baseline when it has received the transport … and writes it as the route's first frame." |
| `conf::ou_9_baseline_then_live_no_gap` | OU-9 | The late-attacher guarantee: "Live `Output` follows with no gap and no duplicate after `R`." |
| `conf::dp_3_route_codec_matches_client_appendix_a4_a5` | DP-3 | "ONE route protocol, shared with the client contract." The client cannot read the route otherwise. |
| `conf::dp_3_screen_is_one_frame_within_max_screen_frame_bytes` | DP-3 | "the screen is ONE `screen` frame, at most `max_screen_frame_bytes`." This is the baseline's screen. |
| `conf::dp_3_data_frames_are_raw` | DP-3 | "Data frames (`output`, `bytes`, `text`, `paste_chunk`) carry raw bytes with no JSON, base64 or transcoding." |
| `conf::dp_3_frame_limit_checked_before_allocation` | DP-3 | Bounded memory: "The limit is checked before any allocation." A bad length prefix must not make the worker allocate gigabytes. |
| `conf::ou_12_output_bytes_are_unmodified_and_in_order` | OU-12 | "`Output{bytes}` carries the PTY output unmodified and in order." |
| `conf::ou_3_progressing_reader_lossless` | OU-3 | "A reader that makes progress never loses output." With (d), "The worker stops reading the PTY while a progressing reader is behind", which bounds memory. |
| `conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model` | ST-6b | "a client that loads a baseline … and then applies every `output` byte after R reaches exactly the state of the session's model." Without it the late attacher sees a wrong screen. |
| `conf::ou_5_peer_fault_closes_only_that_route` | OU-5 | A client disconnect: it "closes the route (`PeerClosed`) … The session and the other routes are unaffected." A first session will hit this. |
| `conf::ev_8_no_route_shadow_answers_at_once` | EV-8 | "A session with no eligible route answers every query from the shadow at once." Many programs send a query and wait for the answer. With `answers_queries: false`, this is the only answer path. See B2. |

### Step 4. The client types input (3)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::dp_5_bytes_and_text_reach_the_pty_exactly` | DP-5 | "a `bytes` or `text` frame reaches the PTY exactly as sent." |
| `conf::am_2_transaction_is_contiguous_across_short_writes` | AM-2 | "across short PTY writes, internal retries … no other input of any source reaches the PTY between its first and last byte." Short PTY writes happen on real input. |
| `conf::dp_5_full_input_queue_stops_reading_the_route_and_never_drops` | DP-5 | Bounded memory: "when the worker's input queue for the route is full … the worker stops reading the route and transport backpressure applies." |

### Step 5. The session ends cleanly (10)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::ev_4_exit_has_signal` | EV-4 | "`Exited` carries `code` and `signal`." The exit must be reported correctly. |
| `conf::a2_1_exit_cause_values` | A2-1 | `ExitCause` = `Normal`, `Signal`, `HostStop`, `Killed`, `Other`. The host shows why the session ended. See B8. |
| `conf::ou_7_exit_after_tail_queued_then_close` | OU-7 | "The worker first drains the PTY tail into the model and the routes." The client sees the last output before the close. |
| `conf::ou_7_exit_after_last_frame_then_close` | OR-4 | The event side of the same rule: "The only order promised between them and events is OU-7." |
| `conf::ou_2b_healthy_reasons_send_route_closed_last` | OU-2b | `SessionEnded` → `route_closed{session_ended{exit{code, signal}}}` "(after the final PTY tail)". The client learns the session ended. |
| `conf::lc_5_stop_ends_payload` | LC-5 | Host stop: "a graceful request, then after `CoreLimits.stop_grace` a kill of the payload's process group." |
| `conf::lc_5_stop_with_broken_control` | LC-5 | Worker dies, then host stops: "A session whose control link is broken still ends." See B9. |
| `conf::a2_3_lost_session_closes_routes_session_lost` | A2-3 | Worker dies: "`SessionLost` (the session's worker exited or the session is `Lost`)" closes the route with a typed reason. |
| `conf::lc_7_remove_order_and_completion` | LC-7 | Clean end: "`Stop` does not end the worker" (LC-5); only `Remove` ends it and deletes the row. Without `Remove`, every session leaks a worker. |
| `conf::lc_7_remove_running_is_wrong_state` | LC-7 | Registry safety: `Remove` "on a `Starting`/`Running`/`Stopping` session it is `WrongState`." A live payload never loses its row. |

### Step 6. The host restarts and re-adopts (14)

| Id | Clause | Why it is IN |
|---|---|---|
| `conf::lc_12_drop_leaves_workers_running` | LC-12 | "Dropping `Core` never ends a worker … the next host adopts them." |
| `conf::ad_5_no_double_adoption` | AD-5 | Safety: "A second host cannot adopt sessions that a live first host holds." An overlapping Hub restart would hit this. |
| `conf::lc_11_adoptall_posts_a_state_for_every_row` | LC-11 | The host rebuilds its view from "the `SessionState` events that `AdoptAll` posts for every row." |
| `conf::ad_1_running_adopts_running` | AD-1 | "`Running` rows whose worker authenticates (AD-6) are adopted as `Running`." This is step 6. |
| `conf::ad_1_created_rows_kept` | AD-1 | Registry safety: "`Created` rows are kept as `Created`." `AdoptAll` must not drop rows. |
| `conf::ad_3_adopted_equals_started` | AD-3 | "After adoption a session is indistinguishable from one started in this host: same reads, same input, same events." This is "the session continues". |
| `conf::ad_6_token_and_instance_checked` | AD-6 | Safety: "The adopt handshake proves the token and the `InstanceId`." |
| `conf::ad_6_reused_pid_is_never_killed` | AD-6 | Safety: "A process that does not match is never signalled: an unrelated process that reuses the pid is `WorkerGone`." |
| `conf::a10_1_wrong_token_is_never_signalled` | A10-1 | Safety, through the testkit: "the test asserts that Core never signals the impostor." |
| `conf::a10_1_wrong_instance_is_never_signalled` | A10-1 | The same, for a wrong `InstanceId`. |
| `conf::a10_2_corrupted_row_is_lost_registry_corrupt` | A10-2 | Registry safety: a damaged row "becomes `Lost(RegistryCorrupt)`", so one bad row does not break adoption of the others. A host killed mid-write can leave one. |
| ~~`conf::ad_2_lost_reasons`~~ | AD-2 | **Removed (revision 23k, steward 2026-10-09).** Core A10's Ids item 2 makes this id cover every Lost reason through the testkit, including the service reasons (GuardianLost, EpochExhausted). A partial pass does not exist, and the minimum assumes no services. `WorkerGone` after a host restart stays covered by `ad_1`, `ad_5`, `lc_11` and `a6_1`. |
| `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone` | A6-1 | No hang: a live worker that does not connect becomes `WorkerUnreachable` "(alive but no connection within the deadline)". Without a deadline, `AdoptAll` could wait forever on one worker. |
| `conf::ou_9_adopted_session_attaches_by_baseline` | OU-9 | "an adopted or long-running session attaches by the same baseline." The client re-attaches after the restart. See B4. |

## 3. The DEFERRED list, by clause family

Counts are deferred ids per family, computed from the package files. Each package's families add up to its DEFERRED count in section 1.

**Lifecycle and host surface (P1, 95)**
- Operation table, features and admission (27): A2-1 (9), A2-6 (3), AM-1 (1), AM-4 (2), ID-1 (2), LC-3 (1), LC-6 (1), LC-7 (2), LC-9 (2), LC-10 (1), LC-12 StopAll (3). Exhaustive checks; the one-session path calls only `Create`, `Start`, `attach`, `Stop`, `Remove` and `AdoptAll` in their normal states.
- ER-0 per-code ids (12): one test per error code. The minimum keeps the two ER-0 model ids.
- Event queue (18): EV-2 (3), EV-5 (11), EV-6 (2), EV-9 (2). They bound memory against a host that stops polling. A polling host gets at most `pump_events` new events per `pump` (9B). See B7.
- Time and wake (17): TM-1 (2), TM-3 (1), TM-4 (1), TM-5 (1), TM-6 (3), E3-1 (7), TH-2 (1), A2-7 silence (1). Clock injection, silence, deadline order under a budget, parking. The minimum needs one due deadline processed per `pump`, not ordering under pressure.
- Captures and cached state (16): ST-4 (1), ST-6 (6), A8-1 (8), A8-2 capture (1). The route baseline is formed by the worker; host captures are not on the path.
- TI-1 (3) and A15-1 host and limits ids (2): the host can set `TERM` itself (B10), and the minimum sends no host `Key`.

**Terminal binding (P2, 5)**
- ST-6b (4) and A8-2 resume invariant (1): full snapshot fidelity. See B6.

**Worker input and terminal model (P3, 165)**
- Host `WriteInput` and semantic input (77): IN-1 to IN-10 (67), A2-2 (9), A15-1 at the bound (1). Step 4 uses client `bytes`/`text` frames only. See B1.
- AM-2 other ids (4): multi-source order, fairness and paste markers. One client is one source.
- Host reads (29): ST-1 (4), ST-2 (1), ST-3 (3), ST-5 (1), ST-7 (20). Not on the session path.
- Host metadata events (44): EV-1 (1), EV-3 (2), EV-7 (5), A2-4 (7), A13-1 (5), A13-1b (6), A14-1 to A14-3 (10), E2-1 to E2-3 (8). The client still sees every sequence in raw `output` (OU-12).
- SZ-1 to SZ-3 (4): host resize. See B3.
- TP-1 (7): the raw-output tap, "default off".

**Routes (P4a, 134)**
- Stall, resync and progress (19): OU-2 (8), OU-3 (3), OU-4 (1), OU-6 (2), OU-7 stall cases (2), OU-9 resync and oversize (3). A client that stops reading holds only its own session (OU-3d); memory stays bounded and the host is off the route path (TH-3). See B5.
- Attach options and route features (21): OU-1 (5), A7-1 (11), A9-1 (2), A9-2 (1), A9-3 (2). Defaults are in range.
- DP-3 deflate, history and size-edge ids (9): compression is negotiated, and `history: none` needs no paging.
- Input extras, size, detach, focus, notices (69): DP-5 (18), DP-6 (5), DP-7 (2), DP-9 (1), DP-12 (10), A3-1 (30), A3-3 (2), A15-1 route id (1). None is on a bytes-only, fixed-size, one-client path.
- Exhaustive mappings and architecture checks (14): OU-2b (3), A2-3 (4), OU-8 (1), OU-10 (1), OU-11 (1), DP-1 (1), DP-2 offer (1), DP-4 (1), TH-3 (1). The path's own close reasons are IN.
- A8-2 baseline and resync uncarriable sequence (2): a program that never ends an escape string. It blocks only that route's attach.

**Queries and files (P4b, 67)**
- Client-answered queries (34): EV-8 (30), DP-5 query frame (1), E4-1 (3). Not used with `answers_queries: false`. See B2.
- File upload and cleanup (33): DP-5b (20), A2-9 (5), A6-3 (6), A12-1a (1), A12-1b (1). No upload in the minimum.

**WebRTC and performance (P4c, 15)**: see section 5. DP-10 and DP-11 are measurements, not function.

**Adoption beyond the running case (P5, 40)**
- Crash windows and non-running rows (11): AD-1 (4), AD-2 retry (1), AD-6 exited (1), AD-7 crash and uncertainty (2), ID-2 (2), LC-5 worker survives stop (1). Step 6 is a clean restart of a running session. See B12.
- Version skew (11): AD-4 (8), A6-2 (3). One build has one worker protocol, and two ids are already deferred by A6-2.
- DP-8 (5): routes that survive the restart, and epoch fencing. See B4.
- Survival of unused features (13): A11-1, A2-1 adopt ids (2), A3-1 (2), A4-1, DP-5, DP-12, EV-8, ST-5, ST-6b, SV-6, SV-8.

**Testkit (P6, 15)**: A5-1 to A5-4, OR-3. The real session does not run the testkit. See B13.

**Services and guardian (P7, 67)**: SV-1 to SV-10, A2-5, A4-1, A6-1, A16-1. The minimum spawns no service. The contract still makes services mandatory in 0.1 (section 10 title: "Services (mandatory in 0.1)").

## 4. Borderline calls

**B1. Semantic key input (IN-9, DP-4): DEFERRED.**
- For IN: 5.1A names the consumer as "the Hub→client contract (input from the TUI and Web)", and the codec's A.5 adopts the `key` shape. If the v1 clients send `key` frames for typing, step 4 fails without `in_9_legacy_keys_table`, `in_9_application_cursor_and_keypad`, `in_9_key_encoded_by_worker_modes_at_write` and `dp_4_worker_encodes_route_frames_with_write_time_modes`. Kitty-mode programs would add `in_9_kitty_each_flag`.
- For DEFERRED: DP-3 also gives the client `0x83 bytes` and `0x84 text`, written "as they are". A client that encodes its own keys types without IN-9.
- Not verified: which frames the Hub's v1 TUI and Web clients send. This is the call most likely to flip.

**B2. `ev_8_no_route_shadow_answers_at_once`: IN.**
- For IN: shells and agent CLIs send device-attribute and similar queries and may wait for a reply. With no answer path, step 2 or 3 can hang the program.
- For DEFERRED: a program that sends no query runs without it. If the Hub must attach with `answers_queries: true` (the default), the client-answer path of EV-8 and A7-1's `query_deadline` ids become IN instead, which is much larger.

**B3. Resize (SZ-1, DP-6): DEFERRED.**
- For IN: a client window rarely matches the spawn size. Full-screen programs render wrongly at the wrong size.
- For DEFERRED: the session runs, and output reaches the client, at the spawn size. The host can create the session with the client's size.

**B4. Routes survive restart (DP-8) versus re-attach (OU-9): DP-8 DEFERRED.**
- For IN: DP-8 says "the worker keeps its route transports when its host dies, so clients stay connected and bytes keep flowing." If the Hub's restart design keeps clients connected, step 6 needs `dp_8_routes_survive_host_restart_and_bytes_keep_flowing` and `dp_8_route_adopted_event_per_live_route`.
- For DEFERRED: "the session continues" holds if the client re-attaches by baseline (`ou_9_adopted_session_attaches_by_baseline`, IN). LC-2's lock already keeps two hosts off one registry, so epoch fencing is a second guard.

**B5. Stall and resync (OU-2, OU-3b): DEFERRED.**
- For IN: a client that stops reading holds the program's output (OU-3d) until it reads or closes. A sleeping laptop could freeze an agent's session.
- For DEFERRED: memory stays bounded, the host is not on the route path (TH-3), and only that session waits. The safety rule is about Core, and Core does not hang.

**B6. Snapshot fidelity beyond the basic invariant (ST-6b): DEFERRED.**
- For IN: `st_6b_resume_invariant_holds_at_every_byte_offset…` and `st_6b_baseline_restores_palette_cursor_shape_hyperlinks_modes_and_kitty_flags` decide whether an attach during a redraw shows a correct screen. Mode restore also decides how a byte-encoding client encodes arrow keys after attach.
- For DEFERRED: `st_6b_model_after_baseline_plus_output_equals_the_sessions_model` (IN) covers the common case. The every-offset corpus is the exhaustive form.

**B7. Event-queue bounds (EV-2, EV-5): DEFERRED.**
- For IN: "bounded resources" is in the safety rule, and EV-5 is how the contract bounds the queue.
- For DEFERRED: these bounds protect against a host that stops polling. A polling host holds at most `pump_events` new events per `pump` (9B).

**B8. Small validations (`a2_1_exit_cause_values`, `a2_1_size_range_zero_is_invalid_input`, `a2_1_empty_argv_is_invalid_input`, `lm_1_zero_limit_is_invalid_config`): IN.**
- For IN: each is a bad input a first session can send, and each is cheap.
- For DEFERRED: none blocks a correct session. `exit_cause` enumerates every value, including `Killed`, which is more than the minimum report.

**B9. Rare failure paths (`lc_5_stop_with_broken_control`, `a6_1_withheld_control_link…`, `a10_2_corrupted_row…`): IN.**
- For IN: each is a safety case in the rule (never signal the wrong process, never lose the registry, never hang Core).
- For DEFERRED: a first session rarely hits them.
- Gap: no id proves that a running session whose worker dies, with the host up, becomes `Lost`. The closest are `a2_3_lost_session_closes_routes_session_lost` and `in_7_lost_worker_ends_pending`.

**B10. TI-1 (terminal identity): DEFERRED.**
- For IN: without `xterm-ghostty` terminfo, programs fall back to a generic `TERM`, and some features misrender.
- For DEFERRED: TI-1 says Core "does not set `TERM` or `TERMINFO` in a session", so the host picks `TERM` either way.

**B11. `th_1_core_is_send_not_sync` and `th_2_wake_handle_cross_thread`: TH-1 IN, TH-2 DEFERRED.**
- TH-2 matters only if the Hub waits on the `WakeHandle` from another thread. If it polls `fd()` in its own loop, it does not need TH-2. Not verified.

**B12. `ad_7_crash_between_steps_leaves_no_unregistered_payload`: DEFERRED.**
- For IN: a host crash during `Start` is plausible during development.
- For DEFERRED: step 6 is a clean restart. AD-7's ordering (IN) already keeps a payload from running without a row.

**B13. The testkit (P6): DEFERRED.**
- For IN: A5-4 says the Hub's unit tests run against the testkit. Hub S1 may want it before a second Core feature.
- For DEFERRED: the real session does not use it.

## 5. WebRTC

**Finding: the minimum session can run over a non-WebRTC stream route under the current contract text. The contract does not make WebRTC optional.**

What the contract allows:
- DP-2: "`RouteTransport` is one of: `Stream`, a connected stream endpoint … or `WebRtc{offer, expected_fingerprint}`." The two are separate variants.
- DP-2: "any connected, reliable, ordered byte stream handed to the worker (a Unix stream, a future QUIC or TCP/TLS stream) is a `RouteTransport::Stream` and needs no Core contract change."
- OU-1: `attach` takes "the client's connected stream endpoint, handed to the worker (DP-2)". WebRTC uses a different call, `begin(AttachWebRtc{…})` (DP-2; A2-1 row `AttachWebRtc`).
- DP-10 defines topology T1 as "a connected Unix stream route", separate from T2 (WebRTC).
- Section 15: the example consumer "hands its own connected socket to the worker". Its acceptance script (spawn, attach, type, restart, re-attach, end) covers all six steps on a stream route.
- The codec (revision 30) defines the `local-stream` binding (TB-L2) and says "A binding is not tied to a client kind." TB-L4 states the stream progress point, as OU-3 requires.

What the contract does not allow:
- A2-6: "`route_transport:stream`, `route_transport:webrtc` | Core | no (mandatory in 0.1 …)". The optional Core features are only `silence` and `size_policy_other`.
- 9B: "Mandatory in 0.1 … routes with a snapshot baseline". DP-2's WebRTC sentence is not conditional: "the worker terminates the peer connection".
- So deferring WebRTC is a staging order, not a contract change. A Core without it is not a conforming 0.1 Core, and plan section 5 needs zero pending ids for Stage 1 acceptance.

Not verified here: whether the Hub's real browser client can reach the worker without WebRTC. A browser cannot open a Unix socket. DP-11 says Hub-terminated WebRTC "is not what the user's requirement selects." So a first session over `local-stream` is likely a local TUI client, not the browser. This is inference.

WebRTC ids (all DEFERRED):
- P4c (15): `dp_2_host_never_reads_the_datachannel`, `dp_2_webrtc_answer_has_all_candidates`, `dp_2_webrtc_fingerprint_mismatch_closes_bad_peer`, `dp_2_webrtc_only_the_terminal_channel_is_accepted`, `dp_3_datachannel_chunking_matches_tr3`, `dp_3_incomplete_datachannel_frame_is_discarded`, `ou_2b_connect_deadline_closes_not_connected`, `ou_3_webrtc_progress_is_acknowledged_payload_not_retransmission`, `ou_3_webrtc_send_queue_is_bounded_and_backpressures`, `ou_3_webrtc_stuck_peer_with_retransmissions_is_a_stall`, `dp_10_t2_latency_and_throughput_budgets`, and the T1 and topology ids `dp_10_t1_keystroke_to_pty_latency_budget`, `dp_10_t1_pty_to_client_latency_budget`, `dp_10_t1_throughput_and_byte_identity`, `dp_11_hop_counts_match_the_stated_topologies` (performance, owned by P4c, not WebRTC-specific).
- Other packages, WebRTC-specific: `a7_1_chunk_cap_above_sctp_max_is_not_refused_and_chunks_stay_within_it`, `a7_1_connect_deadline_out_of_range_is_invalid_input` (P4a).
- Mixed: `dp_2_offer_must_carry_all_candidates` (P4a) is about the WebRTC offer. `ou_1_unavailable_binding_auth_is_unsupported_and_never_attached_unauthenticated` (P4a) names WebRTC's `expected_fingerprint` but applies to "any binding".
- Contract text with no own id: `StartFailReason::WebRtcFailed` and `InvalidOffer` (A2-1), `max_connect_deadline`, `default_route_chunk_bytes` and `route_send_buffer_bytes` (9B).

## 6. Status of the IN ids on v1

- Passing on v1 (not in `core-pending.txt`): **0 of 70.**
- Pending on v1: **70 of 70.**
- At `origin/v1` `aaac0c0d`, `core-pending.txt` lists 671 of the 675 ledger ids. The four that are not listed are the two A6-2 deferrals in `core-deferred.toml` (`ad_4_previous_worker_version_adopts`, `ad_4_missing_worker_capability_is_unsupported`) and the two ids withdrawn by A9-2. None of the four is IN.
- So the pending list does not show code progress. Packages remove an id only "in the pull request that makes them pass on both harnesses" (plan section 5). An id can have working code on v1 and still be pending. Measuring real progress on the 70 needs a run of the suite, which this draft did not do.

## 7. Lead notes (Core lead, 2026-10-09)

- **Contracts dependency.** Two IN ids have no contracts transcript at v0.1.19: `conf::dp_3_frame_limit_checked_before_allocation` and `conf::ou_9_attach_is_sync_baseline_in_pump` (source: `core-pending-no-transcript-v0.1.19.txt`). The steward must supply them before these ids can pass.
- **Staffing.** P4a (routes) has 18 IN ids and no pair today. The minimum Core needs P4a staffed. P7, P4b, P4c and P6-testkit ids are all DEFERRED or nearly so (P4b 1).
- **Decisions needed from the Hub side** (they move ids IN or OUT):
  1. B1: does the v1 client type with `bytes`/`text` frames or with semantic `key` frames? `key` frames move 4 or more IN-9 ids IN.
  2. B2: will the Hub attach with `answers_queries: false`? If not, the EV-8 client-answer path is IN.
  3. B3/B4: is "the session keeps its spawn size" and "a client re-attaches by baseline after a host restart" acceptable for the first session? If not, resize (SZ, DP-6) and DP-8 move IN.
- **WebRTC.** The minimum session runs over a Stream route under the current text, but A2-6 makes `route_transport:webrtc` mandatory for 0.1. Deferring it is a staging order. The placement question is with the steward.
- **Progress measure.** From now on, Core reports "minimum-Core ids passing / 70" in its milestones.

## 8. Answers (orchestrator, 2026-10-09)

- **B1:** the v1 client sends `bytes` and `text` frames (the user decision of 2026-10-05: Web encodes keys with restty from the `modes` frames). Semantic `key` frames stay OUT.
- **B2:** the Hub attaches with `answers_queries: false`. The worker's libghostty shadow answers, so `conf::ev_8_no_route_shadow_answers_at_once` stays IN and the client-answer path of EV-8 stays OUT.
- **B3/B4:** a fixed spawn size and re-attach by baseline after a restart are acceptable for the first session. Resize is the first work after the minimum.
- **WebRTC:** the steward writes Core A17, which removes WebRTC from Core (A2-6 included). Core plans no WebRTC work.
- **Transcripts:** the steward supplies the two missing transcripts (section 7).

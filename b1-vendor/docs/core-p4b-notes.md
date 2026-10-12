# core-p4b notes: Core transcripts for Stage 1 package p4b (queries and files)

Branch `foundation/core-p4b`. The clause list is `docs/stage1-clauses/p4b-queries-files.txt` of `botster-core` (branch `stage1/plan`): 63 ids, all pending at the start. The work comes in two parts on this branch: part 1, the file ids (DP-5b, A2-9, A6-3, DP-5), and part 2, the query ids (EV-8). FakeCore is not changed; every transcript is `fake: structural` (R-12) and runs against the real Core testkit in Stage 1.

## Part 1: files

28 transcripts: 16 of DP-5b, 2 of DP-5, 5 of A2-9, 4 of A6-3 and 1 of A11-1 (`conf::a6_3_unauthenticated_result_is_not_trusted` and `conf::a11_1_frames_after_a_failed_handshake_change_nothing`, with the impostor's `script` of Core A11-1). Part 3 adds the four Core A12 ids. Two ids stay pending with a gap each (see below).

### Prior art (pair-common.md rule 0)

Pinned sources: the pin `~/botster-sessions/pins/foundation-core-p4b.7ddf78fa.md`; the clause list `docs/stage1-clauses/p4b-queries-files.txt` of `botster-core` at `origin/stage1/plan` `be14eb4c75681b7dda28875776d9d12e13cd5fd7`; the frozen text of the Core contract (`frozen/current/core-contract-v1.17.md` DP-5b, EV-8; Amendments 2 A2-9, 6 A6-3, 7 A7-1, 10 A10-1, 11 A11-1); the branch base `origin/main` `d31e444`.

Reused:
- The route step shapes for file input (`file_begin`, `file_chunk`, `file_commit`, `input_done`, `input_refused`) and the idea of an observer of the files on the host come from the codec transcripts `conformance/codec/conf::ts_5f_*.json` (for example `conf::ts_5f_commit_answered_with_path`, which uses `fs_list`). The Core `fs_list` is a new host-side control with its own result; the codec one is a control of the fake route worker.
- The PTY observation forms of `route_expect_pty` (`written_by_op`, `len_var`, `contains`) come from `crates/botster-hub-conformance/tests/driver.rs` and the transcripts of core-p4a-dp, for example `conf::dp_5_input_done_only_for_want_result_true` and `conf::dp_5_mode_precondition_zero_effect_reported_on_the_route`. The barrier of a negative route check (a `paste_commit` with no paste, refused) is the same idiom.
- The adoption pattern (`drop`, `open as h2`, `AdoptAll`) and the impostor control come from core-p5 (`conf::id_2_instance_survives_adoption`, `conf::ad_6_token_and_instance_checked`).
- The route stream controls (`route_gate`, `queue_overflow`, `pty_blocked`, `pty_fail_after`) and `oracle_encode` are existing controls.

Rejected: a worker-side directory listing control (the host owns the directory, the lead's ruling below); fixed names of 255 or 300 bytes (the observed `NAME_MAX` is used); a check of `written_bytes` against the size of the file (DP-5b does not fix it); `Stopping` as the cause of a post-creation refusal (A2-1 admits writes in `Stopping`).

### The lead's ruling on the host acts and observers

The lead's words (2026-10-02): "YES to (1)+(2), with conditions, and no amendment needed: these are HOST actions, not Core edges. A6-3 and OU-1 make the host the owner of file_directory, and the test plays the host. A5-1 governs Core's edges, not the host's own filesystem. Conditions: fs_directory creates a fresh directory under the test's own canonicalized temp root (BUILD.md rule 6) and reports the real NAME_MAX of that filesystem; fs_chmod acts only on paths inside such a directory; fs_list is read-only, counts at the boundary, adds no sync point (R-14.3); docs/core-testkit-controls.md cites DP-5b / A2-9 / A6-3 for each. A permission-based failure case must state its precondition (not running as root) and report Inconclusive when it fails, never pass. (3) YES: oracle_encode kind:paste and oracle_query_reply are R-7 oracles (real libghostty in Stage 1); those ids stay structural here. (4) YES, with tests."

New controls (`docs/core-testkit-controls.md`):

| Control | Kind | Used for |
|---|---|---|
| `fs_directory{name?}` | host act | A fresh directory under the test's canonicalized temp root, and the real `NAME_MAX` of its filesystem |
| `fs_chmod{path, mode}` | host act | A creation failure (DP-5b) and a file that cannot be deleted (A6-3). It acts only inside a directory that `fs_directory` made, and reports `enforced`; a case that depends on it assumes `enforced`, so it is inconclusive, never a pass, when the OS does not enforce it (the test runs as root) |
| `fs_list{path}` | read-only observer (R-14.3) | What the directory holds: names, content, mode, count, distinct names, files outside the directory |

Runner additions with tests: the `let` operations `concat`, `repeat`, `str_len` and `hex_len`. The worst-case encoded size of a path paste (DP-5b) is a sum of observed lengths, a name longer than the observed `NAME_MAX` is a repeated string, and the expected path paste is the oracle's encoding of the path and one space.

### How the clauses are observed

- **Expected bytes come from the oracle.** `oracle_encode` with kind `paste` (R-7) encodes the path and one space with the markers of the mode. The PTY log must equal it (`dp_5b_path_is_followed_by_one_space_in_one_paste`), and `written_bytes` must equal its length (`dp_5b_written_bytes_are_encoded_pty_bytes_with_markers`). With the mode off the log is the path and one space, byte for byte, even when the path needs quoting (`dp_5b_path_is_pasted_unmodified_with_no_quoting`).
- **File content and PTY input are observed apart.** `fs_list` shows the content; `pty_input` and `route_expect_pty` show the PTY. No test expects uploaded content in the PTY. `written_bytes` is compared with the PTY boundary, never with the size of the file: with `pty_fail_after` of 3 it is exactly 3, and the PTY log is those 3 bytes, the start of the path.
- **The bound of DP-5b is built from the clause's terms.** The worst case is the directory, the separator, the longest uniquified name (the observed `NAME_MAX`, because the name is longer by construction), the trailing space and the markers (from the oracle). One Core has `max_paste_bytes` at that value and accepts the long name (its file name is within the limit); another has it one lower, and the `file_begin` is refused with `too_large` before any other frame of the op is sent, and no file exists. The two directories are made with names of the same length.
- **A2-9 asserts no 255, no 300 and no suffix spelling.** The names are the observed `NAME_MAX` plus 50 (or plus 3000), so the long-name condition holds on any filesystem. The assertions: extension kept (the name ends with `.png`), extension dropped when it cannot fit (the name starts as it did, and is shorter than the original), the leading dot (the name still starts with the dot), uniqueness (distinct names for the same long name), the final bound (`name_bytes` is at most the observed `NAME_MAX`), and no refusal.
- **A6-3:** `delete_failed` names exactly the one path that could not be deleted (a second upload directory made unwritable); the other file is gone. `outcome_unknown` for an unreachable worker (`withhold_control_link`) carries no `paths`, and the transcript states no file outcome. Each of the three cases shows that the teardown went on: `s1` is created again after its Remove (the no-upload case in `a6_3_uploads_deleted_at_remove_report_deleted` too).
- **DP-5:** `dp_5_input_done_answers_only_want_result_paste_and_file_commit` waits until the writes of the tested inputs are complete at the PTY boundary, and only then checks that no `input_done` is retained for them.
- **The path paste that fails after the file exists** (`dp_5b_path_paste_failure_carries_path_and_written_bytes`): the PTY is blocked, the file exists and the paste is pending (both observed), the payload is ended (`Signal`, then `payload_alive` is false), and only then is the PTY released.

### Gaps (ids that stay pending)

| Id | Gap |
|---|---|
| `conf::dp_5b_post_creation_refusal_carries_the_retained_path` | DP-5b says that an admission refusal of the path paste can come after the file exists. A2-1 admits `WriteInput` in `Stopping`, and IN-7 fixes `SessionEnded` only for `Exited` and `Lost`. No permitted edge sets up a refusal at the admission boundary after creation. |
| `conf::dp_5b_file_io_never_blocks_the_pty` | The proof needs file I/O that is held outstanding while the PTY and a second route progress. No permitted boundary holds file I/O of the worker. |


## Part 2: queries (EV-8, DP-5)

35 transcripts: 31 of EV-8, 1 of DP-5 (`conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves`) and 3 of Core erratum 4 (E4-1). All are `fake: structural` (R-12). No id of this part stays pending.

### Observers and runner additions

- `oracle_query_reply{session, request_hex, prefix_hex?, profile?}` (R-7, lead's ruling YES, `docs/core-testkit-controls.md`): the reply of a fresh libghostty terminal to the same query, with the program output before the query (`prefix_hex`) and the host's color profile when there is one. Every expected shadow reply comes from it. Its `answerable` result is the oracle's. The transcript binds the whole result first, `assume`s the answerability it needs (so the case is inconclusive, never a pass, when the oracle disagrees), and only then reads `reply_hex`: an unanswerable result is never read for a reply. No terminal byte is written by hand as an expected value. The query bytes the program writes and the client's own reply bytes are stimuli.
- `await_pty` takes a bound number for `min_bytes` (a reply's length is computed from the oracle's value); the `let` operations of part 1 give the sums. A wait for PTY bytes is an event of the worker's write, never a timer.
- **The deadline is the worker's own** (EV-8(b)). A case that needs a deadline to pass uses `real_wait` (core-p5's shared step, on main): a real interval, event-driven, bounded by the step deadline, that never moves the host's clock (the testkit injects the host clock, and the worker's deadline runs on its own real clock; a test covers an injected clock with a real wait). The deadline is 300 ms and the wait is longer. After the wait the PTY log is read with no further wait, so a fallback later than the deadline cannot be hidden; the check that nothing is written early is a separate observation before the wait. `advance_clock` proves nothing about it: `conf::ev_8_core_has_no_other_query_timer` uses a far jump of the host clock only to show that Core owns no query deadline (`next_deadline` is null, and the pump posts nothing).

### R-18: the id of an affected query, and "never from stale state"

Steward ruling R-18 (via the lead): `open_queries` is not allowed (the query table is inside the worker). The two ids that need the id of a query whose frame was never delivered use determinism (A5-1):

- `conf::ev_8_client_reply_after_a_resync_is_rejected_query_expired`, `conf::ev_8_refused_fallback_admission_stays_on_the_fallback_path_only` and `conf::ev_8_resync_never_lets_a_client_answer_from_stale_state`: run `a` delivers the frame and the transcript reads the `query_id` from the route. Run `b` has the same script with its own data directory and the route gated, so the frame is unstarted when a resync is forced (`queue_overflow`); the id was minted at the parse point, before the cut, so it is the same. A reply with it on run `b` is refused, and its bytes never reach the PTY. The assertions are at the program edge (every byte written to the PTY): the answer is the saved shadow fallback (the oracle's reply), and the retired frame never appears after the new baseline. An id that is not reproducible would be a testkit determinism bug.

### How the sub-clauses are observed

| Sub-clause | Transcripts | Observation |
|---|---|---|
| (a) who is asked | `query_goes_to_latest_route_with_host_deadline` | Two routes: with no input the latest attached route gets the frame, and its `answer_within_ms` is within its own deadline; after input on the other route it is asked next. The other route gets no frame (a barrier check). |
| (b) deadline range | `attach_requires_a_positive_query_deadline` | Sync `InvalidInput{field: query_deadline}` for absent, zero and above the maximum (A7-1); the bounds attach. |
| (b)(c) answers | `client_answers_first...`, `client_silent...`, `client_decline...`, `no_route...`, `late_client_reply...`, `route_close...` | The PTY log equals the client's bytes or the oracle's shadow reply, and nothing else; the deadline cases use a real wait. |
| (d) the reply is held from the query point | `shadow_reply_is_computed_at_the_query_point`, `worker_never_writes_before_client_opportunity_ends` | The oracle is fed the program output before the query only; a later state would give another reply (assumed different). Nothing is written while the opportunity is open. |
| (e) one admission | `exactly_one_reply_under_every_interleaving` | The client's first reply wins; a decline and a second reply change nothing; exactly one reply reached the program. |
| (f) | `replies_never_split_a_paste_and_keep_query_order` | With the PTY taking 10 bytes at a time, a paste of 40 bytes is written whole, and then the two replies in query order. |
| (g) | `full_query_capacity_backpressures_parser_in_order` | `pending_queries = 1` while q1 stays open: the output up to the unadmitted query reaches the second route, no second `terminal_query` frame reaches the first (a barrier check), and no frame of the held tail reaches the second route (a barrier check on its output frames); after q1 is answered the second query is offered, and the tail arrives, in order, with nothing dropped. Whether the worker holds the tail at the source or inside is not asserted (A5-2). |
| (h) retirement and survival | `route_close...`, core-p5's `ev_8_open_query_survives_host_restart` | A route that closes falls back at once. |
| (i) resync | `resync_before_answer...`, `unstarted_query_frame_is_retired...`, `a_sent_or_started_query_is_not_affected...`, and the three R-18 ids (`client_reply_after_a_resync...`, `refused_fallback_admission_stays_on_the_fallback_path_only`, `resync_never_lets_a_client_answer_from_stale_state`) | The route's stream is gated so the frame is unstarted; the resync is forced with `queue_overflow`, and the gate is released after it. The frames from the resync to the end of the new baseline are read strictly in order (`resync`, `baseline_begin`, `screen`, `baseline_end`, `live`), so a `terminal_query` frame anywhere in that span fails the step, and a quiet check follows. An unstarted query falls back to the saved shadow reply (at once, or when room returns when the fallback is refused for room, with R-18 ids); a delivered frame keeps its opportunity across the resync. |
| (j) | `every_query_stays_in_output` | The route's output has the query bytes, unmodified and in order, answered or not. |
| limits | `encoded_reply_is_bounded_and_once`, `oversize_query_answered_by_shadow_with_marker` | An `Encoded` reply over `max_query_reply_bytes` is `too_large` and the query stays pending; a query over `max_query_bytes` is answered by the shadow at once, with no frame, and `EventsLost` has kind `Query`. |
| IN-10 | `reply_does_not_move_input_rev` | `terminal_state.input_rev` is equal before and after an applied reply. |

### Choices that a reader may question

- `ev_8_exactly_one_reply_under_every_interleaving` is a deterministic script (the client's reply, a decline and a second reply, then the deadline passes) whose result is one reply: the client's first. The "every interleaving" is the seeded perturbation of the real Core (`core-testkit+perturb`).
- These cases send `Encoded` replies (the client's own bytes) and `Decline`. The typed replies of the table in EV-8(e) are not sent here; nothing in the pin or the oracle excludes oracle-backed typed cases, and they are not in the named ids of this part.

### Prior art (part 2)

- The query frame and the `reply_terminal_query` shapes: `crates/botster-route-codec/src/frames.rs` (`TerminalQuery`) and `src/input.rs` (`QueryReply`), and core-p5's `conf::ev_8_open_query_survives_host_restart` (the same attach options and reply).
- The gated route, the forced resync and the barrier of a negative route check: `conf::ou_9_resync_drops_unstarted_frames`, `conf::dp_5_*` of core-p4a-dp and core-p4a-ou.
- The two-Core pattern (one Core per limit): part 1's bound cases and core-p5's adoption transcripts.
- The real wait: `real_wait` of core-p5 (`crates/botster-core-conformance/src/driver.rs`, on main since `d31e444`), used by core-p5's `conf::sv_8_adoption_cancels_grace`.

### Admission cost (Core erratum 4, E4-1)

After the reviewer rejected the first setups, the steward's erratum 4 (in the manifest as final22) fixed the cost C that an admission reserves: for a client reply, the size of its `0x81 input` frame as received (the type byte plus the JSON body, with the base64 of the `Encoded` bytes; the binding's delimiting not counted); for the shadow fallback, the length of the reply bytes the worker writes (read from the oracle, R-7). C is reserved at admission and released when the write completes. A test now knows C exactly:

- The fallback's C is `L`, the length of the oracle's reply. The session's input bound `input_retained_bytes` is the filler (a host write of 8 payload bytes: IN-5 counts payload bytes held by in-flight writes) plus `L - 1` (the fallback cannot be admitted) or plus `L` (it is admitted, although the blocked PTY wrote nothing).
- The client reply's C is the size `S` of its frame. The step `route_send` has a new key `"bind": "$S"` (shared route driver, with a test): it binds the size of the frame that it sent. A first Core (default limits) runs the same script and learns `S`. That the later Cores' frames have the same size (the query id has the same length) is an explicit precondition: the transcript `assume`s it, so the case is inconclusive, never a failure, for a worker whose opaque ids differ in length (A5-1 repeats a script with the same limits and seed, and these runs differ in their limits). The other Cores have room for exactly `S` (the reply is admitted) or `S - 1` (it is parked). The raw bytes of the reply are fewer than `S`, so a worker that reserves only the raw bytes would admit the `S - 1` case.
- The probe: a host write of one more byte is refused at `begin` with `LaneFull` when the lane has no free byte (IN-5), and admitted when it has. It shows whether the reply holds its C bytes, with the PTY blocked, so that a completed write is never the evidence.
- When the PTY is released the filler is written, and then the reply (the fallback's oracle bytes, or the client's `Encoded` bytes), once.

The ids: `conf::ev_8_decline_does_not_consume_the_query_id_before_fallback_admission`, `conf::ev_8_reply_after_decline_is_query_expired_then_already_replied`, `conf::ev_8_fallback_started_means_admitted_refused_fallback_stays_pending` (room for exactly `L`), `conf::ev_8_admission_reserves_full_bytes` (a client reply: room for `S - 1` and for `S`), `conf::ev_8_refused_fallback_admission_stays_on_the_fallback_path_only` and `conf::ev_8_client_reply_after_a_resync_is_rejected_query_expired` (R-18, with room for `L - 1`), and the three `conf::e4_1_*` ids.

## Part 3: Core Amendment 12 (worker report held, a point inside the cleanup)

Four ids: `conf::a12_1_held_report_released_proceeds_normally`, `conf::a6_3_lost_result_reports_outcome_unknown`, `conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown` and `conf::a12_1b_partial_cleanup_claims_no_paths`. New controls (`docs/core-testkit-controls.md`): `hold_worker_report`, `release_worker_report` and `held_reports` (process edge, A12-1a; the count is a read-only observer, R-14.3), and `hold_cleanup_after{session, deleted}` (scheduler edge, A12-1b).

- A12-1a: the worker is stopped and its payload has ended (`payload_alive` is false) before the next report is held; `Remove` starts, a read-only count shows a report is held, and `fs_list` shows no uploaded file left while it is held: the cleanup is complete and its result is the held report (this separates the case from A12-1b). Released, it is read and the cleanup is reported `deleted`, and the id is free. With the link broken (`break_control`) and the report unread, the result is lost: `outcome_unknown`, no paths, the id free. The edge never drops a report while the link is up.
- A12-1b: three files are uploaded; the scheduler edge stops the cleanup after one is deleted; the disk (`fs_list`) shows exactly two files left (the cleanup is partial); then the worker is ended. The report is `outcome_unknown` with no paths, the teardown completes, and the id is free; the remaining files are still on disk.

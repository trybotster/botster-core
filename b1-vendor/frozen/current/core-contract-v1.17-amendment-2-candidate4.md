# Core contract v1.17, Amendment 2 — candidate 4 (candidate 3, sha256 3ce5581e…, with only the codec pin moved to revision 30)

Pins: `core-contract-v1.17.md` (sha256 8ee7ca05300156312363a00bf3bafae99674d0f09fc349866f71bb2f3a6d3486) is unchanged. This amendment adds clauses and replaces the named sentences of v1.17. It re-pins the route codec to **revision 30** (`route-codec-v1-extract-rev30.md`, sha256 **17f2d2667aba7e3d79b857d85eaf4751612f60594dcedb6a7d36209668ef62f6**), which replaces revision 26 (bf59b2b6…) wherever v1.17 pins it. Other contracts are cited by contract name and clause id only.

Author: Core writer. Date: 2026-09-30. Status: DESIGN ONLY. Source: the completeness sweep `audits/completeness-sweep-B2-20260930.md` (B2-41 to B2-61) and `audits/completeness-sweep-A-20260930.md` (G-11). Every clause has a test id. Immutable once published.

**How this amendment relates to v1.17.** A clause below named "replaces" supersedes the quoted sentence of the v1.17 clause. A clause named "adds" extends the v1.17 clause. Nothing else in v1.17 changes. Any "today" or file reference is non-normative.

---

## A2-1. The operation table (adds section 9.4; closes B2-41, B2-49, B2-53, B2-54, B2-55, B2-56, B2-59)

**Rule.** Every `begin` operation has one row. A row gives the arguments, the `Ok` value, the sync errors, the async errors and the states in which the operation is admitted. **Every row also has the sync error `PendingLimit`** (EV-5a). An operation that is not admitted in the cached state (AM-1) is `WrongState` (sync). A `WrongState` that arrives as a completion means the state changed while the operation was in flight (ER-0). A session that is `Released` is `UnknownSession`.

**Value types used in the table.**
- `SessionRecord` = the record that `get(id)` returns (LC-9), read at completion.
- `SessionEnd` = `Exited(Exit)` or `Lost(reason)`.
- `Screen{text, rows, cols, history_unavailable, model_rev}`; `Cursor{row, col, visible, row_text, text_before_cursor, model_rev}` (ST-3); `Modes{flags: ModeFlags, model_rev}`; `Capture` (ST-6); `Facts` (ST-7); `Answer{route, answer}` (DP-2); `SpawnReport` (SV-4).
- `Size{rows, cols, cell_px?}`: **`rows` and `cols` are 1 to 65,535; a zero, or a value above 65,535, is `InvalidInput`** (B2-59). A size such as 80 by 24 is valid.
- `SpawnRequest{argv: Vec<String>, env: Map<String,String>, cwd: String, size: Size, labels: Map<String,String>, color_profile?: ColorProfile}` (B2-54). `argv` is non-empty (an empty `argv` is `InvalidInput`). `env` is **exact** (the session gets these variables and no inherited set, as SV-1). `cwd` is an absolute path. Strings are UTF-8 in the Rust facade; a C embedder passes byte strings (section 13). A `cwd` that does not exist at launch completes `Start` with `StartFailed{CwdMissing}`.
- `ColorProfile{palette?: [Rgb; 256], foreground: Rgb, background: Rgb, cursor?: Rgb}` with `Rgb{r, g, b: u8}` (EV-8 shadow answers use it).
- `StartFailReason` (non-exhaustive) = `ExecFailed{errno}`, `CwdMissing`, `StartupTimeout`, `WorkerFailed`, `GuardianFailed`, `WebRtcFailed`.
- Session `ExitCause` (non-exhaustive; a host maps an unknown value to `Other`) = `Normal` (the payload exited by itself with a code), `Signal` (a signal that Core did not send ended it), `HostStop` (it ended after `Stop`, `Signal` or `StopAll` within `stop_grace`), `Killed` (Core killed it after `stop_grace`), `Other`.

| Operation | Arguments | `Ok` | Sync errors | Async errors | Admitted in |
|---|---|---|---|---|---|
| `Create` | `session: SessionId` (host-chosen, ID-1), `request: SpawnRequest` | `SessionRecord` (state `Created`) | `IdTooLong`, `IdInUse`, `SessionLimit`, `InvalidInput` | `RegistryFailed` | no session with that id |
| `Start` | `id` | `SessionRecord` (state `Running`) | `UnknownSession`, `WrongState` | `StartFailed{StartFailReason}`, `RegistryFailed`, `WrongState` | `Created` |
| `Stop` | `id` | `SessionEnd` | `UnknownSession`, `WrongState` | `RegistryFailed` | `Starting`, `Running`, `Stopping`, `Exited`, `Lost` |
| `Signal` | `id`, `sig` | `()` after the signal was sent | `UnknownSession`, `WrongState`, `Unsupported` | `SessionEnded`, `WorkerLinkFailed` | `Running`, `Stopping` |
| `Remove` | `id` | `()` after LC-7 step 5 | `UnknownSession`, `WrongState` | `RegistryFailed` | `Created`, `Exited`, `Lost` |
| `StopAll` | none | `()` | none | none (a target that cannot be stopped is left as LC-12 says) | always |
| `UpdateMetadata` | `id`, `labels` | `()` after the durable write | `UnknownSession`, `InvalidInput` | `RegistryFailed` | `Created`, `Starting`, `Running`, `Stopping`, `Exited`, `Lost` |
| `SetSizePolicy` | `session`, `policy` | `()` | `UnknownSession`, `WrongState`, `Unsupported` (`policy` needs `size_policy_other`) | `WorkerLinkFailed`, `SessionEnded` | `Created`, `Starting`, `Running` |
| `SetColorProfile` | `session`, `ColorProfile` | `()` | `UnknownSession`, `WrongState`, `InvalidInput` | `WorkerLinkFailed` | `Created`, `Starting`, `Running`, `Stopping`, `Exited` |
| `SetOutputTap` | `session`, `enabled` | `()` | `UnknownSession`, `WrongState` | `WorkerLinkFailed` | `Running`, `Stopping` |
| `ReadScreen` | `session`, `history: bool` | `Screen` | `UnknownSession`, `WrongState` | `WorkerLinkFailed`, `CursorReadUnsupported` | `Starting`, `Running`, `Stopping`, `Exited` |
| `ReadCursor` | `session` | `Cursor` | as `ReadScreen` | as `ReadScreen` | as `ReadScreen` |
| `ReadModeFlags` | `session` | `Modes` | as `ReadScreen` | as `ReadScreen` | as `ReadScreen` |
| `CaptureSnapshot` | `session`, `owner` | `Capture` | `UnknownSession`, `WrongState`, `CaptureLimit` | `SnapshotTooLarge`, `WorkerLinkFailed`, `CursorReadUnsupported` | as `ReadScreen` |
| `ReadFacts` | `session`, `after?` | `Facts` | `UnknownSession`, `WrongState` | `WorkerLinkFailed`, `CursorReadUnsupported` | as `ReadScreen` |
| `Resize` | `session`, `Size` | `Applied{actual}` or `Superseded{by}` (SZ-3) | `UnknownSession`, `WrongState`, `InvalidInput` | `WorkerLinkFailed`, `SessionEnded` | `Created` (stores the spawn size; completes `Applied` in the next `pump`), `Starting`, `Running` |
| `WriteInput` | `session`, `payload`, `guard?` | `InputResult` | `UnknownSession`, `WrongState`, `InvalidInput`, `PayloadTooLarge`, `LaneFull`, `Unsupported` | `Internal` only (A2-2) | `Running`, `Stopping`, `Exited`, `Lost` |
| `Detach` | `route`, `reason` | `()` after `RouteClosed` is posted (DP-7) | `UnknownRoute` | none | any live route |
| `AttachWebRtc` | `client`, `session`, `offer`, `expected_fingerprint`, `options` | `Answer` | as `attach`, plus `InvalidInput` for an `options` field | `InvalidOffer{reason}`, `StartFailed{WebRtcFailed}` | as `attach` |
| `AdoptAll` | none | `()` (one `SessionState` per row, LC-11) | `WrongState` (a second `AdoptAll` on one handle) | `RegistryFailed` | once per handle |
| `Adopt` | `id` | `SessionRecord` (its state tells the outcome: `Running`, `Exited` or `Lost(reason)` again) | `UnknownSession`, `WrongState` | `RegistryFailed` | `Lost(WorkerUnreachable)`, `Lost(WorkerVersion)` |
| `SpawnService` | `service_id`, `argv`, `env`, `cwd`, `ServiceLimits` | `SpawnReport` | `IdInUse`, `ServiceLimit`, `InvalidConfig`, `InvalidInput` (`service_id` is not 32 bytes), `BoundUnavailable` | `StartFailed{ExecFailed{errno}}`, `StartFailed{GuardianFailed}`, `RegistryFailed` | no service |
| `StopService` | `id` | `ServiceExit` (the `Exited` payload, SV-5) | `UnknownService`, `WrongState` | `RegistryFailed` | `Started`, `Reconnected`, `Exited`, `Lost` |
| `RemoveService` | `id` | `()` after the id is free | `UnknownService`, `WrongState` | `RegistryFailed` | `Exited`, `Lost` |
| `EndEpoch` | `service` | `()` after every lane of the epoch is closed | `UnknownService`, `WrongState` | `WrongState` (the service ended meanwhile) | a service with a committed epoch |

**The sync `attach` call (OU-1)** is admitted for a session in `Starting`, `Running` or `Exited` (an `Exited` session serves its final model, ST-5). It is `WrongState` for `Created`, `Stopping` and `Lost`. `attach` also has the sync errors `UnknownSession`, `RouteLimit`, `InvalidInput`, `Unsupported`.

**`ReadScreen` (B2-55).** `text` is the visible screen. With `history: true` it is the scrollback followed by the visible screen. When history was asked for and is not available, `text` is the visible screen and `history_unavailable` is true; Core never returns an empty text as the screen (ST-2). `rows` and `cols` are the size of the visible screen at `model_rev`.

**The code `CursorReadUnsupported` keeps its name and means "no worker model for this session"** for every read in the table, not only the cursor read. Reads of a `Created` session or a `Lost` session are `WrongState` (sync) because Core already knows there is no model.

**No other code names a deadline.** `DeadlineExpired` is **removed** from 9.3 (B2-56). Every deadline has a named outcome: the startup deadline completes `Start` with `StartFailed{StartupTimeout}` (LC-4); a service that does not authenticate every lane of epoch 1 within `startup` ends with `ServiceState{Exited{cause: StartupTimeout}}` (A2-5); a WebRTC `connect_deadline` closes the route `NotConnected` (OU-1); a `query_deadline` hands the query to the shadow (EV-8).

**`cancel(op)` (B2-51).** `cancel` applies to `WriteInput` only (IN-6). For an op that is already completed it returns `TooLate`, as IN-6 says. For any other uncompleted op it returns `Refused(NotCancellable)`; nothing changes. `NotCancellable` joins `Full` in the `Refused` reasons of `CancelResult`, and unlike `Full` it has no wake.

Tests: `conf::a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value`, `conf::a2_1_sync_errors_allocate_no_op_for_every_row`, `conf::a2_1_not_admitted_state_is_wrong_state_sync_for_every_row`, `conf::a2_1_state_change_in_flight_is_wrong_state_async`, `conf::a2_1_size_range_zero_is_invalid_input`, `conf::a2_1_empty_argv_is_invalid_input`, `conf::a2_1_missing_cwd_is_start_failed_cwd_missing`, `conf::a2_1_env_is_exact`, `conf::a2_1_adoptall_twice_is_wrong_state`, `conf::a2_1_adopt_completes_ok_with_the_state_it_reached`, `conf::a2_1_attach_state_rules`, `conf::a2_1_read_screen_history_flag_and_text`, `conf::a2_1_read_of_created_or_lost_is_wrong_state`, `conf::a2_1_cancel_non_input_is_not_cancellable`, `conf::a2_1_no_deadline_code_exists`, `conf::a2_1_exit_cause_values`.

**New codes (additive, 9.3).** `UnknownService` (caller, sync: the id names no service); `InvalidOffer{reason: bad_offer | offer_without_candidates}` (caller, **async**: the worker parsed the offer and refused it). Widened: `IdInUse` also covers a `service_id`; `InvalidConfig` is sync for `open` and `SpawnService`; `WrongState` also covers services.

---

## A2-2. `WriteInput` has one result path with exact outcomes (adds to IN-2, IN-6 and IN-7; closes B2-42 and B2-50)

**`WriteInput` completes `Ok(InputResult)` for every admitted write.** It never completes `Err(SessionEnded)`, `Err(Cancelled)` or `Err(WorkerLinkFailed)`: those `CoreError` codes do not apply to `WriteInput`. **Its only async error is `Internal`.** Its sync errors are in the A2-1 row. **The outcome inside `InputResult` is exactly the one that IN-2, IN-6 and IN-7 already define, and this clause changes none of them:**
- A write that was **never sent** to the worker, and a write to an `Exited` or `Lost` session, completes `NotWritten(SessionEnded)` with exact zero bytes (IN-7). A certain zero stays a certain zero.
- A write that was **sent and not acknowledged** when the worker link failed completes `Unknown{max_payload_bytes}` (IN-2, IN-7). Core never turns it into a zero.
- A `cancel` that is admitted completes the op with what really happened (IN-6): `Cancelled` (exact counts), `Partial`, `Unknown` if the link failed, **or the real outcome of the write if the write finished first** (`Written`, `Failed`, `NotWritten(...)`).

`InputResult.detail` is a human-readable string with no compatibility promise (B2-58). `outcome` and the counts are the contract.

**Two reasons that IN-2 lists get a definition (B2-50).**
- `NotWritten(Refused)`: the worker refused the write for a reason that is **not** a mode precondition, a guard, a size or a session state. **Route input only.** In 0.1 the causes are the **sequencing errors** of the route codec (TS-5d for a paste, TS-5f for a file): a chunk, commit or abort with no open paste or file, or whose `op` is not the open one; a second `paste_begin` or `file_begin` while one is open (DP-5b). Each has zero effect and the route stays open; on the wire it is `input_refused{refused}`. A missing or zero `op` is not `Refused`: it is a protocol error that closes the route (`BadFrame{malformed_frame}`). A host `WriteInput` never gets `Refused`.
- `NotWritten(Unsupported{what})`: no encoding exists for the event (5.1A rule 5). `what` is non-exhaustive: `pixel_position`, `produced_text`, `named_key`, `wheel_direction`, `coordinate`, `release_event`. A receiver treats an unknown `what` as opaque.

Tests: `conf::a2_2_never_sent_write_is_not_written_zero`, `conf::a2_2_sent_unacknowledged_write_is_unknown_on_link_loss`, `conf::a2_2_write_to_ended_is_ok_not_written_session_ended`, `conf::a2_2_cancel_race_reports_the_real_outcome`, `conf::a2_2_cancel_is_ok_cancelled_never_err`, `conf::a2_2_write_input_never_completes_with_err_except_internal`, `conf::a2_2_refused_is_a_sequencing_error_route_input_only`, `conf::a2_2_missing_op_is_bad_frame_not_refused`, `conf::a2_2_unsupported_what_values`.

---

## A2-3. The route close reasons are one closed set (replaces the reason lists of OU-2 and OU-2b; closes B2-43)

`RouteCloseReason` (non-exhaustive; a host maps an unknown value to `PeerClosed`) has these values:

`Detached`, `Replaced`, `Revoked` (the three `Detach{reason}` values of DP-7); `SessionEnded{exit}`; `SessionRemoved`; `SessionLost` (the session's worker exited or the session is `Lost`); `PeerClosed`; `WriteFailed` (the worker's write to the transport failed); `BadFrame{code}` (`code` is one of the codec's `frame_too_large`, `malformed_frame`, `unknown_frame`, `invalid_input`); `BadPeer`; `NotConnected`; `SnapshotTooLarge`; `StallTimeout`; `HandoffFailed`.

OU-2's list of the events that close a route gains `WriteFailed` and `SessionLost`. The first reason wins (OU-2).

**The mapping to the route wire (the codec's revision 30 form).** `Detached`, `Replaced`, `Revoked`, `SessionEnded`, `SessionRemoved`, `SnapshotTooLarge`, `BadPeer` and `BadFrame` map as OU-2b says. The failed-route reasons map to the codec's route-ended reasons: `NotConnected` → `not_connected`, `HandoffFailed` → `handoff_failed`, `PeerClosed` → `transport_lost`, `WriteFailed` → `write_failed`, `StallTimeout` → `stalled`, `SessionLost` → `session_lost`. A reason that the codec revision does not know maps to `transport_lost`. The JSON form of the data-carrying reasons is the codec's, in revision 30, and Core adopts it verbatim.

Tests: `conf::a2_3_every_route_close_reason_maps_to_one_wire_reason`, `conf::a2_3_write_failed_closes_with_write_failed`, `conf::a2_3_lost_session_closes_routes_session_lost`, `conf::a2_3_unknown_reason_maps_to_transport_lost`, `conf::a2_3_close_first_reason_wins_with_new_reasons`.

---

## A2-4. Terminal event payloads (replaces the three one-line rows of 6.2; closes B2-44 and B2-57)

Every payload below carries `id` (the `SessionId`), `instance` (the `InstanceId`, EV-9) and `at` (unix seconds from the injected clock, TM-1). They are class D events (EV-2).

- `Bell{id, instance, at}`.
- `PromptMark{id, instance, mark, exit_code?, at}`. `mark` is `PromptStart` (OSC 133 `A`), `CommandStart` (`B`, the end of the prompt), `CommandExecuted` (`C`, the program starts), `CommandFinished` (`D`). `exit_code` (an `i32`) is present only for `CommandFinished` when the sequence carries one.
- `Notification{id, instance, source, title?, body, truncated, at}`. `source` is `Osc9` or `Osc777`. **The pinned emulator's classification decides which sequences are a notification**: OSC 9 with a text, and OSC 777 in its `notify` form (`title` and `body`). Every other OSC form stays in `Output` (OU-12) and is not a `Notification`. `title` and `body` are each bounded by the **existing `CoreLimits.clipboard_bytes`** (a memory bound, as EV-3; no new limit is added). An oversize text is cut at a UTF-8 character boundary and `truncated` is `true`. The cut is a memory bound, not a filter (P9): the full sequence is still in `Output`.

**`EventsLost.kinds`** is a set (non-exhaustive) of: `Bell`, `PromptMark`, `Notification`, `ClipboardWrite`, `Query` (a query offer that was dropped, EV-8), `Tap` (TP-1).

Tests: `conf::a2_4_bell_payload`, `conf::a2_4_prompt_mark_each_mark_and_exit_code`, `conf::a2_4_notification_osc9_and_osc777`, `conf::a2_4_notification_oversize_is_truncated_with_flag`, `conf::a2_4_other_osc_forms_stay_in_output`, `conf::a2_4_every_payload_carries_instance_and_at`, `conf::a2_4_events_lost_kinds_set`.

---

## A2-5. Services: calls, states, lane reasons, limits (replaces text of SV-1 to SV-5; closes B2-45 to B2-48 and B2-61)

**Calls (SV-3).**
- `service_send(id, lane, frame) -> Result<(), SendError>`. It is sync. `SendError` (non-exhaustive) = `Backpressured` (the lane's outbound queue has no room; **resume:** `ServiceWritable{id, lane}`); `UnknownService`; `NoSuchLane`; `NotConnected` (no committed epoch yet, or the epoch ended; **resume:** `ServiceState{Reconnected}`); `ServiceEnded` (the service is `Exited` or `Lost`); `Oversize` (the frame is over the lane's `max_frame_bytes`; permanent). A refused send takes no ownership, no slot and posts no event (AM-4).
- `service_recv(id, lane) -> Result<Option<Frame>, RecvError>`. It is sync. It returns `Ok(None)` when the inbound queue is empty. `RecvError` = `UnknownService`, `NoSuchLane`. Frames that were received stay readable after `Exited` until `RemoveService` (SV-9).
- `Backpressured` is a `SendError` value, **not** a `CoreError` code.

**`ServiceState` (SV-5)** = `Started`; `Reconnected{epoch}`; `Exited{code, signal, cause, descendants_may_remain}`; `Lost{reason, payload_may_remain}` (`reason` is `GuardianLost` or `EpochExhausted`, as AD-2); `Released`. Every value is a mandatory event (EV-5).

**`LaneEnded.reason` (SV-3, SV-5)** (non-exhaustive) = `BadLane`, `Oversize`, `Truncated`, `PeerClosed`, `EpochEnded` (the host called `EndEpoch`), `Superseded` (a higher epoch was staged, SV-6), `ServiceExited`, `ServiceRemoved`.

**`ServiceLimits` (SV-2, SV-4)** adds `peer_identity: Uid` (default: the host's uid) and gives each rlimit the form `Option<{value, required: bool}>`. **`startup` runs from `exec` until every lane of epoch 1 has authenticated** (SV-6 commit). If it passes first, the service is ended and `ServiceState{Exited{cause: StartupTimeout}}` is posted.

**Spawn failures** are in the A2-1 row: sync `IdInUse`, `ServiceLimit`, `InvalidConfig`, `InvalidInput`, `BoundUnavailable{bound}`; async `StartFailed{ExecFailed{errno}}`, `StartFailed{GuardianFailed}`.

Tests: `conf::a2_5_service_send_error_values`, `conf::a2_5_send_before_epoch_commit_is_not_connected`, `conf::a2_5_send_to_exited_is_service_ended`, `conf::a2_5_service_recv_empty_is_none`, `conf::a2_5_recv_after_exit_until_remove`, `conf::a2_5_service_state_values`, `conf::a2_5_lane_end_reason_values`, `conf::a2_5_peer_identity_and_required_bounds`, `conf::a2_5_startup_runs_to_epoch_one_commit`, `conf::a2_5_spawn_failure_codes`.

---

## A2-6. The feature names (replaces the `features()` paragraphs of 9.3 and 9B; closes B2-52)

`features()` is one set of names. This table lists every name, who reports it and whether it is optional.

| Name | Reported by | Optional? | Meaning |
|---|---|---|---|
| `route_transport:stream`, `route_transport:webrtc` | Core | no (mandatory in 0.1; listed for the C ABI, section 13) | The route transports of DP-2. |
| `service_preamble_versions` | Core | no | The accepted service preamble versions (a list; SV-2). |
| `silence` | Core | **yes** | `set_silence_threshold` and `Silent` (TM-4). |
| `size_policy_other` | Core | **yes** | The `Smallest` and `Largest` size policies (DP-6). |
| `snapshot_graphics` | each session's worker | per worker | Images in snapshots (ST-6b). |
| `focus_report` | each session's worker | per worker | `FocusChanged` (AD-3); an N−1 worker may lack it. |
| `payload_dies_with_guardian` | the platform | per platform | SV-8. |
| `bound:cpu_seconds`, `bound:open_files`, `bound:processes`, `bound:address_space_bytes` | the platform | per platform | The OS bounds of SV-4. |

The sentence of 9B that says `silence` is the only optional feature is **replaced** by: "The optional Core features are `silence` and `size_policy_other`; the per-worker and per-platform names are reports, not options." A call that needs a missing name is `Unsupported` (sync).

Tests: `conf::a2_6_features_lists_every_documented_name`, `conf::a2_6_missing_optional_feature_is_unsupported`, `conf::a2_6_per_worker_features_in_worker_features`.

---

## A2-7. Smaller signatures (closes B2-58, B2-60)

- `pump(now) -> PumpReport{more: bool, events_posted: u32}`.
- `set_silence_threshold(session, Option<Duration>) -> Result<(), CoreError>`. It is sync. Errors: `UnknownSession`, `Unsupported` (no `silence` feature). `None` turns the threshold off.

Tests: `conf::a2_7_pump_report_fields`, `conf::a2_7_set_silence_threshold_errors`.

---

## A2-8. The terminal identity (adds TI-1; closes G-11 on the Core side)

**TI-1.** `Core::terminal_identity() -> TerminalIdentity{term: String, terminfo_source: String}` is a sync read that needs no session. `term` is the value a host puts in `TERM` for a session: **`xterm-ghostty`**, the terminfo entry of the pinned emulator. `terminfo_source` is the terminfo source text of that entry, and Core ships it with the library. Core does not install it, and it does not set `TERM` or `TERMINFO` in a session (a host gives the exact `env`, A2-1). The host compiles and installs the entry in the install's terminfo directory (the Hub contract names that directory and `TERMINFO`). **Invariant:** the entry declares only capabilities that the emulator implements. A change of the pinned emulator that changes the entry changes `terminfo_source` and is a contract-tag change. A host may override `term` by its own parameter; Core then makes no promise about the entry.

Tests: `conf::a2_8_terminal_identity_names_the_entry`, `conf::a2_8_terminfo_source_compiles_with_tic`, `conf::a2_8_core_sets_no_term_in_the_session_env`.

---

## A2-9. File names in DP-5b (adds to DP-5b; matches codec TS-5f in revision 30)

The worker derives the file's name from `name` as DP-5b says (reduced to `A-Z a-z 0-9 . _ -`; an empty result becomes `file`; unique; never outside the directory). **The worker never refuses a file for the length of its name and adds no new limit.** It shortens the name to the **file-name limit of the filesystem that holds the directory** (the platform's `NAME_MAX`; 255 bytes on common systems) by these rules, in this order:
1. **Reserve first.** The worker reserves the room for its own uniqueness suffix before it cuts, so the final name, suffix included, is within the limit and is still unique.
2. **Keep the extension when it fits.** The extension is the text after the last `.` of the reduced name, when that `.` is not the first character and the text after it is not empty. The worker cuts the **stem** (the text before the extension) and keeps the extension, if at least one stem byte, the `.`, the whole extension and the suffix fit.
3. **Drop the extension when it cannot fit.** If the extension is so long that rule 2 cannot hold (for example `a.` plus 300 letters), the worker treats the whole reduced name as the stem: it cuts the name to fit with the suffix. The result has no preserved extension.
4. The reduced name is ASCII, so a cut never splits a character. An empty stem after the cut becomes `file`.

Tests: `conf::a2_9_long_stem_is_cut_and_extension_kept`, `conf::a2_9_extension_too_long_to_keep_is_dropped_with_the_name_cut`, `conf::a2_9_suffix_room_is_reserved_so_the_name_stays_unique_and_within_the_limit`, `conf::a2_9_long_name_is_never_refused`, `conf::a2_9_leading_dot_is_not_an_extension`.

---

## A2-10. Changes to v1.17 text, listed

1. Section 9.3: `DeadlineExpired` removed; `UnknownService` and `InvalidOffer` added; the widened meanings of `IdInUse`, `InvalidConfig`, `WrongState` and `CursorReadUnsupported` (A2-1).
2. 9B: the `silence`-only sentence replaced (A2-6). No limit is added.
3. IN-2, IN-6, IN-7: one result path and the `Refused` and `Unsupported` definitions per A2-2 (no outcome changes). IN-6: `Refused(NotCancellable)` added (A2-1).
4. OU-2, OU-2b: reason set per A2-3.
5. 6.2: the `Bell`, `PromptMark` and `Notification` rows per A2-4.
6. SV-2 to SV-5: per A2-5.
7. DP-5b: file-name shortening per A2-9.
8. Pin: codec revision 26 is replaced by revision 30 (the JSON form of the data-carrying route-ended reasons and the route-frame failure reasons).

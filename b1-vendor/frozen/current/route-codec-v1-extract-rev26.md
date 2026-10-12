# Botster route codec v1 — immutable extract of the Hub→client contract (revision 26)

Source: `hub-client-contract-v1-draft.md`, taken 2026-09-30. **The client contract owns this text. Core adopts it verbatim** (DP-3). This file never changes: a later revision gets a new path.

**Revision 26** (on revision 25; user decision UC-1): the P9 "one actor per side effect" rule now says that an attached client acts on notifications and the bell **on that client**, and that the Hub uses Core's notification events only to inform the other clients (client SF-5d). The Hub never writes to the terminal and never acts on OSC 52. **Core impact:** the worker reports the session's focus state (whether any route's latest focus input is true) to the Hub.

**Revision 25** (on revision 24; Sol Core): TS-4's parser-safe cut now cites Core ST-6b's resume invariant and allows both of Core's methods: carrying the pending parser state, or cutting at the preceding ground state and replaying the complete pending bytes as `output` after the baseline.

**Revision 24** (on revision 23; Sol Core's rev22 findings):
- **TS-10:** the focus-out condition is exact (Core DP-12). It applies when a route ends whose latest focus input was `focused: true` and no remaining route's latest focus input is `focused: true`.
- **TS-4:** the cut is parser-safe. A partial escape sequence either has its parser state carried in the snapshot or has the cut placed after it, as Core defines.
- **A.4 `0x0E`:** the inflated payload is at most the output frame bound minus the type byte.

**Revision 23** (on revision 22):
- **TS-4:** the state cut is exactly the state that Core ST-6b lists. Images need `snapshot_graphics`, and the cwd is included.
- **TS-5f (CG-1, legacy parity):** the worker pastes the file's path **followed by one space**, as one paste, so the user can type straight after it and agent CLIs detect the path as an attachment. `written_bytes` counts the space too.
- TB-0 also names the partial `mcp` binding for agents (already in revision 22; no route impact).

**Revision 22** (on revision 21; client parity audit, `audits/client-parity-20260930.md`):
- **TS-4 (CG-6):** the state cut covers the whole terminal state as Core's snapshot defines it: screen, size, every mode, the cursor and its style, the palette, the title, OSC 8 hyperlinks and images. Any exclusion is listed in the Core contract.
- **TS-10 (CG-17):** when the last route that reported focus ends and the program enabled focus reporting, the worker writes a focus-out.
- **A.4 (CG-7):** new frame `0x0E output_deflate`, raw DEFLATE of one output payload, only when the route negotiated the feature `output_deflate`. The inflated size is bounded by the output frame bound.

**Revision 21** (on revision 20; Sol Hub A's unit finding and Sol Core's path-bound finding):
- **`written_bytes` has one unit everywhere** (TS-5b): the bytes written to the PTY after encoding, including any bracketed-paste markers (Core IN-2). For a file, it counts only the path paste, never the uploaded content.
- **TS-5f:** before it creates the final file, the worker checks that the encoded path paste fits `max_paste_bytes`. If it does not, the commit is refused with `too_large` and no file is created.

**Revision 20** (on revision 19; Sol Core's TS-5f findings):
- A file refusal **before the file is complete** leaves no file and writes no path. A refusal **while the path is being pasted** (`failed`, `session_ended`) carries `path` (the file stays under the Hub's cleanup policy) and the path paste's `written_bytes`, exact or absent. `input_refused` gains `path?`.
- The worker pastes the absolute path's bytes unmodified. Neither Core nor the worker quotes or escapes it. The claim that the path needs no shell quoting is deleted.

**Revision 19** (on revision 18; revision 18 is superseded because its extract dropped TS-5f's conformance line):
- **TS-5c, live route:** the worker processes each input in receive order: it is written, refused, or (for input that the modes do not report) omitted with no frame. Silence proves neither a completed write nor the absence of a refusal in transit.
- **TS-5c and TS-9, failed route:** an `input_done` or `input_refused` that already arrived still decides that input's outcome. Only input without such a frame is unknown.
- **TS-7:** exception frames wait in the route's existing outgoing queue under Core's `route_queue_bytes`, and a resync never discards them. When that queue cannot hold one more, the worker stops reading the route's input. There is no separate count.
- TS-5f's conformance line is restored.

**Revision 18** (on revision 17):
- **File input, TS-5f** (a user gap; it replaces the monorepo `CONTENT_FILE` frame). `file_begin`, `file_chunk` (`0x85`), then `file_commit` or `file_abort`, bounded by `route_limits.max_file_bytes`. At the commit, the worker writes the file on the session's host and then writes its path as a paste at the commit's position in the receive order. Every commit is answered with `input_done { op, written_bytes, path }` or `input_refused` (new reason `write_failed`). The directory, naming, permissions and cleanup are Hub policy.
- **Sol Core's rev17 findings:**
  - TS-5c: silence while the route is live means "processed in order, written unless refused", not "applied";
  - TS-5b: `failed` and `session_ended` can follow a partial write of any input; `written_bytes` is exact when known and absent when unknown;
  - TS-7: exception frames need a bound (settled in revision 19).

**Revision 17** (the user rule "input is fire-and-forget; the worker reports only exceptions"):
- **No success acknowledgement** for any input (TS-5), except a paste that requests completion and a file. `op` is optional, except on a paste or a file.
- **`0x0A` is now `input_refused { op?, reason, written_bytes? }`**, sent only when an input is not applied as sent (TS-5b). The fields `outcome` and `accepted_bytes`, and the outcomes `superseded`, `applied`, `cancelled` and `unknown`, are deleted.
- **`0x0D input_done { op, written_bytes, path? }`** answers a `paste_commit { want_result: true }` and every `file_commit`.
- **A resize has no answer.** The effective size arrives in `modes` (TS-4d, TS-6).
- **Input that the modes do not report** (Core `NotReported`) sends no frame, as on a real terminal (A.5).
- **Backpressure:** the worker stops reading the route while its input queue is full. The reason `lane_full` is deleted from the route reasons.
- **The result mechanism is deleted:** no result slots and no results lost on a failed route (TS-9).
- **Aligned with Core** (delta check): the stream and WebRTC progress points restate Core OU-3a (TB-L4, TB-W4); a late answer while the fallback is pending is `query_expired` (Core EV-8(i)); `session_ended` is an `input_refused` reason.
- A query-reply refusal uses `input_refused` with `already_replied`, `query_expired`, `unknown_query`, `invalid_input` or `too_large`.

Revisions 15 and 16 wrote TS-5c as knowability rules and left client policy to clients.

## Terminal queries on the route (P9, extract)

  - **A query that needs a reply is answered exactly once, by the active client first, then by the shadow terminal** (user decision, 2026-09-30). This covers **every** reply-needing query, including model queries (device attributes, DSR and CPR, DECRQM, XTVERSION, color queries, DECRQSS, XTGETTCAP, title and size reports) and external ones (the clipboard read, pixel geometry, window state and position, the color scheme, ENQ). **The session worker arbitrates**, because it owns the routes (section 7.0):
      1. The worker parses the query, saves the **shadow answer at the parse point** (if `shadow_answerable`), and **never auto-answers**.
      2. **The asked route** is the session's most recently active route (under `Latest`, TS-6). If no route has sent input yet, it is the most recently attached route that can send input. A route attached with `answers_queries: false` is never asked. If such a route exists, the worker sends `terminal_query` on that route and waits at most the **host-supplied** `query_client_deadline_ms`. The Hub sets that value as policy (a documented default in the Hub configuration, with a range of 1 ms to the Hub's maximum), and **reports the value applied to each route in `route_limits.query_client_deadline_ms`**. **Core has no timer of its own:** the only deadline is the one the host supplies.
      3. The client answers **from its own real terminal state** (its theme colors, its real window and cell size, its clipboard, its emulator state), or it declines. A client `decline` is **not** a final answer: it triggers the fallback.
      4. **Fallback:** if there is no route to ask, or the client declines, or the deadline passes, the worker writes the saved shadow answer. With no shadow answer, it writes nothing, and the program's own timeout applies.
      5. **Exactly one reply:** the client answer and the fallback atomically consume the same `query_id` **at admission**. "The fallback started" means that the fallback was **admitted**. A fallback refused at admission (for example, the input lane is full) leaves the query pending. A client answer after an admitted fallback is refused with `already_replied`, and the reverse also holds.
    - **Renderer rules for clients (TS-QR):**
      - A client **disables every automatic query reply** of its terminal emulator for bytes that arrive in `output`.
      - It **never** sends reply bytes as `text` or `bytes`.
      - It answers only with `reply_terminal_query`, and only for a `terminal_query` it received **on that route**.
    - **Binding.** The **worker** binds each `query_id` to the route it chose: that route and the session run. A `reply_terminal_query` from any other route is refused with `input_refused { reason: invalid_input }`.
    - **Routing.** A query reply does **not** count as activity for `Latest` (TS-6). The worker never reroutes a query after a reply is admitted. If the completion of an admitted reply is unknown, **no other answer is sent**. If the chosen route ends **before** a reply is admitted, the worker goes straight to the shadow-terminal fallback (step 4).
    - **One actor per side effect.** The **worker** sends `output` bytes straight to the client (BP-1), so an attached client is the one actor for OSC 52 writes, and for notifications and the bell **on that client**. The Hub uses Core's notification events (Core EV-7) **only to inform the other clients** (the client contract's SF-5d: an event, the row's attention and Web push). It never writes to the terminal and never acts on OSC 52.
    - No reply is ever sent twice.
  - The only exception is the accepted unsafe-paste confirmation (TS-5e), which concerns input, not output.

## Byte path and route clauses

| # | Clause | Test |
|---|---|---|
| BP-1 | **The route ends at the session worker, not at the Hub, on every binding.** The Hub authorizes an attach (caller, permission, session), hands the route's endpoint to the session's worker in the binding's way (section 9: TB-L2, TB-W2), and gets out of the byte path. **No Hub process touches route bytes.** | `conf::bp_1_hub_stopped_bytes_still_flow` (`SIGSTOP` the Hub process: typing still echoes, and output still arrives on an attached route, on **every** binding) |
| BP-2 | **Unadulterated bytes.** `output` frames carry the PTY's bytes **exactly**, in order. Client input frames reach the PTY as the bytes they specify: raw `bytes` and `text` verbatim, and semantic keys and paste encoded only as P9 and Core 5.1A define. A baseline snapshot is sent **only** at attach and at a resync (TS-4, TS-4c), never as a substitute for output. | `conf::bp_2_output_bytes_identical_to_pty` (a random byte stream written to the PTY equals the concatenated `output` payloads) |
| BP-3 | **Framing** on the route is the thin frame of Appendix A.4, `[u8 type][body]`, delimited by the binding (TS-3). The data frames (`output`, and input `bytes` `0x83`, `text` `0x84` and `paste_chunk` `0x82`) carry raw bytes with no JSON. Control and lifecycle frames (`resize`, `modes`, `input_refused`, `input_done`, `terminal_query`, `route_closed`, and the baseline frames) travel **on the same ordered stream** beside the data. They are small and infrequent. | `conf::bp_3_data_frames_are_raw` |
| BP-5 | *(Non-normative.)* Route frames are produced and consumed by the session's worker. Control frames, authorization, attach, detach and revocation are the Hub's. The Core contract states the split (Core DP-4, EV-8). A client sees only the frames and results defined here. | `conf::bp_5_detach_via_hub_closes_route_at_worker`, `conf::bp_5_revocation_closes_worker_routes` |
| BP-7 | **Worker restart and adoption.** Routes do not survive a worker exit. On a Hub restart (the worker keeps running), **existing direct routes stay connected**, because the Hub is not on their path. The new Hub adopts the worker and its routes' metadata (Core adoption). | `conf::bp_7_routes_survive_hub_restart` |


**Layering (resolved with the Core pair).** The Hub defines the route **lifecycle** and the **input and result envelopes**. Core defines the **payload encodings** of the baseline (GHOSTSNP, Core ST-6), of output and of resync. The **worker** carries those payloads **verbatim** inside the route frames (BP-1). `terminal_format` (HS-3) names the Core payload encoding, for example `ghostsnp/2+raw`. A client never sees a Core version.
| TS-1 | `invoke(session, id, attach, { history: none | screen | full, replace_route?, answers_queries?, binding?, route_features? })` **reserves a route** and answers `{ route_id, transport, route_limits: { max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_chunk_bytes, max_paste_bytes, max_file_bytes, max_query_reply_bytes, query_client_deadline_ms, stall_deadline_ms, stall_close_after_ms } }`, taken from the worker options selected at attach. `binding` defaults to the binding of the requesting connection. **Before the route is reserved, the Hub refuses the attach with `unsupported { reason }`** in two cases:
- the caller cannot authenticate on the chosen binding (for example, `webrtc` needs a paired credential and the caller has none): `binding_auth_unavailable`. A binding always authenticates the route connection itself (TB-0 item 2), and **no client kind bypasses that**;
- no terminal format from the caller's `hello.terminal_formats` is supported by the **target session's worker**: `terminal_format`.

**Negotiation is per route.** The attach picks the first format in the caller's `hello.terminal_formats` that the target worker supports. `attached.terminal_format` echoes that choice, and `attached.features` is the **intersection** of the attach's `route_features` and the worker's features. **`transport` is `{ binding: <name>, … }`**, where `<name>` is an open, feature-detected string (TB-0) and the other fields are that binding's connect information (section 9). **The three frame bounds** (`max_frame_bytes`, `max_screen_frame_bytes`, `max_history_page_bytes`) bound a frame's size, which counts the type byte plus the body (TS-3). So `max_screen_frame_bytes` = Core's `max_snapshot_bytes` + 1, and `max_history_page_bytes` = the history page payload limit + 1. **`max_chunk_bytes` is not a frame bound:** it bounds one transport chunk, for a binding that chunks. Each route frame fits its own type's limit. **Every limit a route peer can hit is reported per route in `route_limits`**, and it is authoritative over any `welcome.limits` value of the same name (those are Hub-wide hints). The Hub mints `route_id`, unique within the `hub_id`. **A session may have many routes at once** (several panes or clients). `replace_route: <route_id>` ends that route with `replaced` in the same step. Without it, no other route is touched. | `conf::ts_1_multi_attach`, `conf::ts_1_explicit_replace` |
| TS-2 | **Connect.** The client completes its binding's connect step (section 9: TB-L2, TB-W2) within `limits.route_connect_seconds`. A route that is not connected in time ends with `not_connected` (TS-9). | `conf::ts_2_connect_once_in_time` |
| TS-3 | **Framing:** the route carries frames `[u8 type][body]` in both directions. **The binding delimits them** (section 9). A frame's **size** counts the type byte plus the body, and each frame type has its own bound (A.1 `frame_too_large`). The type values and bodies are normative in Appendix A.4. There is no route id, generation or epoch in any frame: the binding's connection **is** the route. | `conf::ts_3_framing` |
| TS-4 | **Baseline.** After connect: `attached`, `baseline_begin { rows, cols, modes }`, one `screen { payload }` frame (at most `route_limits.max_screen_frame_bytes`), zero or more `history { payload }`, and `baseline_end { history: not_requested | complete | unavailable { reason } | incomplete { reason } }`. The `complete` history may be empty. **The baseline is a state cut:** a terminal model that loads the baseline and then applies every later `output` reaches the same **terminal state** as the session's model at the same point (the worker's model): **exactly the state that Core ST-6b lists**: the screen and scrollback cells with their attributes, OSC 8 hyperlinks included; the palette and the dynamic colors; the cursor position, visibility, shape and blink; every `ModeFlags` field; the size and the alternate-screen state; the title and the cwd. Images are restored only when the worker advertises `snapshot_graphics`. Core states every exclusion, and nothing is lost silently (parity CG-6). **The cut is parser-safe, per Core ST-6b's resume invariant:** a model that loads the baseline and then applies every later `output` reaches the session's exact state at every cut point, including a cut inside an escape sequence (for example `ESC [` then `31mX`). Core either carries the pending parser state in the snapshot, or takes the cut at the preceding ground state and sends the complete pending sequence bytes as `output` after the baseline. Either way the prefix is restored, and a client never receives an unmatched sequence suffix. History is compared **only** when it is `complete`. If the screen cannot be produced, attach fails: the route ends with `attach_failed { reason }`. Missing or incomplete history leaves the route live. | `conf::ts_4_state_equivalence` (the model after baseline + output equals the worker's model), `conf::ts_4_history_states` |
| TS-4b | **`live`** follows `baseline_end`. **`live` is the input-ready point** (TS-5). | `conf::ts_4b_live_after_baseline`, `conf::ts_4b_early_input_not_ready` |
| TS-4c | **Resync within a route.** When the worker discards queued output for this route (a reader without progress, TS-7), the worker **first finishes every frame whose bytes or chunks have started**, discards only frames not yet started, and sends `resync` at the next frame boundary, followed by a new `baseline_begin` … `baseline_end`, `live`. The client replaces its screen. The route and its `route_id` stay the same. **If a started frame cannot be finished, the route ends** (TS-9 `write_failed`) and the client re-attaches. **Transport loss is not a resync:** it ends the route (TS-9). | `conf::ts_4c_resync_same_route` |
| TS-4d | **Modes and the effective size** travel on the route: `modes { modes, rows, cols }` in the baseline and at every change. `rows` and `cols` are the size actually applied to the PTY. | `conf::ts_4d_modes_effective_size` |
| TS-5 | **Input is fire-and-forget** (user rule, 2026-09-30: a terminal over ssh has no per-keystroke acknowledgement). The route's ordered, reliable delivery **is** the delivery guarantee, and **there is no success acknowledgement** for `bytes`, `text`, keys, mouse, focus or resize. The two exceptions are a paste that requests completion (TS-5b) and a file (TS-5f). `input { op?, kind: key | text | mouse | focus | resize | paste_begin | paste_chunk | paste_commit | paste_abort | file_begin | file_chunk | file_commit | file_abort, … }`. `op` is optional and chosen by the client, and it is used only to correlate a refusal (TS-5b). A paste's frames, and a file's frames, carry one `op`, which is required for them. **Order is the worker's receive order.** Input received before `live` is refused `not_ready`. Backpressure is the transport's: the worker stops reading the route while its input queue is full, so input is never dropped for lack of room. What a client does with unsent input, or with input handed to a transport that then failed, is client policy (TS-5c). | `conf::ts_5_no_success_ack`, `conf::ts_5_order_is_receive_order`, `conf::ts_5_backpressure_not_drop` |
| TS-5b | **The worker reports only exceptions:** `input_refused { op?, reason }`, sent when an input is **not** applied as sent. `op` echoes the input's `op` when it had one. `reason` is non-exhaustive (an unknown reason shows as "other"):
- `not_ready` (before `live`);
- `not_writable` (a view-only route);
- `too_large`;
- `mode_precondition_failed` (the paste asked for bracketed mode and it was off; the client, which classified the paste, shows its own unsafe-paste reason);
- `unsupported { what }` (an unknown kind or value, or invalid UTF-8 as `what: "utf8"`);
- `refused`;
- `session_ended`;
- `failed` (the write to the PTY failed);
- `write_failed` (a file could not be written on the host, TS-5f).

A refusal means the input had no effect, except `failed` and `session_ended`: those can follow a partial write of **any** input (bytes, text, a paste, or a multi-byte encoded key). They carry `written_bytes`, which is exact when the worker knows it and absent when it does not. **`written_bytes` has one unit everywhere:** the bytes written to the PTY after encoding, including any bracketed-paste markers (Core IN-2). For a file, it counts only the path paste, never the uploaded content. Input that the program's modes do not report (for example, mouse events with tracking off) is not an exception: as on a real terminal, it is simply not sent to the program, with no frame. **A paste may request completion:** `paste_commit { op, want_result: true }` is answered by `input_done { op, written_bytes }` or by `input_refused`. This supports the unsafe-paste confirmation flow and paste progress. Apart from this, only a file is acknowledged (TS-5f). | `conf::ts_5b_refusal_only_on_exception`, `conf::ts_5b_partial_write_reports_written_bytes`, `conf::ts_5b_paste_done_on_request`, `conf::ts_5b_unreported_mode_input_is_silent` |
| TS-5c | **What a client can always know about each input** (user ruling: what to do next is client policy):
- **Never sent on a route:** no effect.
- **Sent, and the route is still live:** the worker processes it in receive order. It is written, or refused with an `input_refused`, or (for input that the modes do not report, A.5) omitted with no frame, as on a real terminal. There is no success receipt: silence proves neither that the write has completed (the input may still be queued behind PTY backpressure) nor that no refusal is in transit.
- **Sent, and the route failed** (TS-9): an `input_done` or `input_refused` for it that already arrived still decides its outcome. Without such a frame, the outcome is unknown, as on ssh.

Each client chooses to drop, keep or resend. *(Non-normative: resending may duplicate input.)* A user-confirmed paste resend is a **new** `op` with `require_bracketed: false`. **There is no size result:** the effective size arrives in `modes { modes, rows, cols }` (TS-4d). | `conf::ts_5c_unsent_has_no_effect`, `conf::ts_5c_confirmed_paste_resend_is_new_op` |
| TS-5d | **Paste** is one operation: `paste_begin`, chunks, then `paste_commit` or `paste_abort`. An abort before the commit writes nothing, with no frame. An abort after the commit has no effect. A paste over `route_limits.max_paste_bytes` is refused with `input_refused { op, reason: too_large }` at the first chunk that exceeds it, with zero effect. A paste not committed when the route ends writes nothing. **Only Core's encoder** adds bracketed-paste markers, at write time, according to the mode at that time. Neither the client nor the Hub adds them. The paste's chunks reach the PTY contiguously: no other input is written between them. | `conf::ts_5d_abort_before_commit`, `conf::ts_5d_contiguous` |
| TS-5e | **Unsafe paste is Hub policy** (the definition, accepted by the user on 2026-09-30: bracketed paste is off **and** the payload contains a line break or ESC). **The seam (settled with the Core pair):**
- The **client** classifies the payload by this rule (the route goes straight to the worker, BP-1).
- When the rule needs bracketed mode, the client sends `paste_begin { require_bracketed: true }`, and the worker applies it as Core `Paste { require_bracketed: true }`.
- The worker checks the mode **atomically at the start of the write**. `ModePreconditionFailed` proves zero effect, and the worker reports it on the route as `input_refused { op, reason: mode_precondition_failed }`. The client derives its UI reason from the payload it classified.
- A confirmed resend is a new op with `require_bracketed: false`.
- Core adds the markers and writes contiguously. Core never classifies content.
- **Why the client classifies** (gate 3 #86): clients are authenticated operators, and a confirmed resend is a client choice anyway. So worker-side enforcement would add no protection against a nonconforming client. Core's `paste_require_bracketed_if` option is to be deleted (gate 2 #1). | `conf::ts_5e_precondition_zero_effect`, `conf::ts_5e_confirmed_resend_without_precondition_writes` |
| TS-5f | **File input** (user gap, 2026-09-30: a user pastes or drops a file or an image, and the program gets its path). It is transport-neutral, so every client can send it. One operation:
- `file_begin { op, name, mime?, total_bytes? }`, then `file_chunk` frames (`0x85`), then `file_commit` or `file_abort`. `op` is required and nonzero. One file is open per route at a time; a second `file_begin` before the first ends is refused with `refused`.
- **Bound:** a file over `route_limits.max_file_bytes` is refused with `too_large` at the first chunk that exceeds it, and nothing of it stays on the host.
- **Chunks are not terminal input:** other input keeps flowing while a file uploads.
- **Before it creates the final file**, the worker forms the absolute path and checks that the encoded path paste (markers included) fits `route_limits.max_paste_bytes`. If it does not, the commit is refused with `too_large` and no file is created.
- **At `file_commit`**, the worker writes the file on the session's host, where the session can read it. Then it writes the file's **path followed by one space** as one paste (TS-5d) **at the commit's position in the receive order** (the space lets the user type straight after the path, and agent CLIs detect the path as an attachment; legacy parity CG-1), so the path lands in order with the user's typing. Only Core's encoder adds bracketed-paste markers, when the mode is on.
- **The worker chooses the path.** The directory comes from Hub policy. The file name is unique, derived from `name` reduced to `A-Z a-z 0-9 . _ -` (an empty result becomes `file`), never overwrites a file and never leaves the directory. The worker pastes the absolute path's bytes **unmodified**: neither Core nor the worker quotes or escapes it, because neither knows the program's syntax. *(Non-normative: a Hub directory policy without spaces or shell metacharacters keeps the path usable at a shell prompt.)* `mime` is advisory.
- **The exception to fire-and-forget:** every `file_commit` is answered, with `input_done { op, written_bytes, path }` or with `input_refused { op, reason }`. The reasons include `too_large`, `write_failed`, `not_writable`, `not_ready`, `failed` and `session_ended`.
  - A refusal **before the file is complete** (`too_large`, `write_failed`, `not_writable`, `not_ready`, or `session_ended` during the upload) leaves no file and writes no path.
  - A refusal **while the path is being pasted** (`failed` or `session_ended`) carries `path` (the file stays, under the Hub's cleanup policy) and `written_bytes` of the path paste (exact when known, absent when unknown, TS-5b).
- **An abort, or a route end, before the commit** writes nothing and removes any partial data, with no frame.
- **Hub policy, not wire:** the directory, the naming, the permissions (readable only by the session's user by default) and the cleanup. Cleanup happens at session delete at the latest.
| `conf::ts_5f_path_written_in_receive_order`, `conf::ts_5f_commit_answered_with_path`, `conf::ts_5f_over_limit_leaves_nothing`, `conf::ts_5f_abort_or_route_end_leaves_nothing`, `conf::ts_5f_name_sanitized_and_unique`, `conf::ts_5f_other_input_flows_during_upload` |
| TS-6 | **Size policy `Latest`.** The Hub **selects** the policy (orchestrator ruling), and the **worker computes and applies** it, because it owns the routes (BP-5):
- **Activity:** a route's last activity is the worker's receive time of its most recent input of any kind (key, text, raw bytes, mouse, focus, paste, resize; matching Core DP-6).
- **Selection:** the effective size is the last reported size of the route with the most recent activity. A route that never reported a size is skipped.
- **Ties:** the worker's receive order is total, so the route whose input the worker received last wins.
- **Close:** when the active route closes, the next most recently active remaining route's last size applies. With no remaining route that has a size, the size stays as it is.
- **Resize input:** a `resize` counts as activity, so the resizing route becomes active. It gets no answer. The applied size (which may come from a more active route) arrives in `modes` (TS-4d).
- The worker resizes the PTY. There is no Hub hop for a resize. | `conf::ts_6_most_recent_activity_wins`, `conf::ts_6_close_falls_back`, `conf::ts_6_tie_by_receive_order` |
| TS-7 | **Backpressure** (matches Core OU-3). A reader that makes progress is lossless: output waits, and the session's output source waits too. A reader with no progress for `route_limits.stall_deadline_ms` (Hub policy, applied by the worker) may have its queued **output** discarded and gets a resync (TS-4c). **Only queued output may be discarded:** pending `input_refused` and `input_done` frames and `route_closed` are retained and still delivered on the continuing route. **Exception frames are bounded without loss:** they wait in the route's existing outgoing queue and count against its existing byte bound (Core `route_queue_bytes`), but a resync never discards them. When that queue cannot hold one more exception frame, the worker stops reading the route's input until it can, as it does for a full input queue. No separate count or ledger exists. So a client that sends input and never reads refusals stalls its own route, and the stall rules above then close it; the worker never drops an exception frame and never grows without bound. **A resync ends the answer chance only for an affected query**: a query whose required output prefix was discarded, which means its `terminal_query` frame had not started. The client would otherwise answer from stale state. **A query frame already sent (or started, and then finished under TS-4c) is not affected**, because ordered delivery has already carried its prefix. For an affected query:
  - The worker retires its unstarted `terminal_query` frame.
  - It admits the saved P9 fallback (the parse-point shadow answer).
  - A later client answer for that `query_id` is refused with `input_refused { reason: already_replied }`, or `input_refused { reason: query_expired }` if the fallback is still pending (Core EV-8(i)).
  - If admission of the fallback is refused (for example, the lane is full), the query stays pending on the fallback path only. **The stale client-answer path is never reopened.** If it still makes no progress after `route_limits.stall_close_after_ms`, the route ends with `stalled`. | `conf::ts_7_progressing_lossless`, `conf::ts_7_stall_resync_then_close`, `conf::ts_7_unread_refusals_stop_input_reads` |
| TS-8 | **A healthy close** drains. Every output frame queued for the route is sent. For `session_ended`, that includes **the final PTY tail through Core's final read** before the process exit. Then `route_closed { reason }` is the last frame, then EOF or the channel close. Healthy reasons: `detached`, `replaced`, `session_ended { exit { code, signal } }`, `revoked`, `attach_failed`. (A Hub shutdown does **not** end direct routes, BP-7.) | `conf::ts_8_drain_includes_final_tail`, `conf::ts_8_close_is_last` |
| TS-9 | **A failed route** (the transport died, or it never connected) may lose undelivered output. The worker reports the end to the Hub over its control link, and the Hub reports it with `event { stream_id: "routes", owner: "hub", name: "route.ended", subject: { route_id }, payload: { reason }, priority: "high", source, at }` (the section 8 shape, on the connection's implicit, always-open `routes` stream; `route_ended` below means this event) on the owning client's control connection, when that connection is alive. `reason` ∈ `not_connected`, `handoff_failed`, `transport_lost`, `write_failed`, `stalled` (TS-7), `session_lost`. A failed route may lack `route_closed`, and it may lose output. It may also lose undelivered `input_done` and `input_refused` frames. Input handed to it before the failure has an unknown outcome, unless a frame for that input already arrived (TS-5c). **EOF without `route_closed` means that the cause is unknown** until `route_ended` arrives or the session watch explains it. | `conf::ts_9_failed_route_reported`, `conf::ts_9_eof_without_close_is_unknown` |
| TS-10 | `invoke(session, id, detach, { route_id })` ends the route with `detached`. A client that closes its route connection ends only that route (TS-9 `transport_lost`). The session keeps running. **When a route ends** (by any close) **whose latest focus input was `focused: true`, and no remaining route's latest focus input is `focused: true`,** and the program enabled focus reporting, the worker writes a focus-out to the program, as a terminal losing its window would (Core DP-12; parity CG-17). | `conf::ts_10_detach`, `conf::ts_10_last_focused_route_end_writes_focus_out` |
| TS-11 | **Route identity.** A `route_id` names one attachment. A re-attach or a transport recovery makes a new route. **A Hub restart does not end direct routes** (BP-7). A worker exit ends them. The durable identity is `{hub_id, session_id}`. | `conf::ts_11_reattach_after_worker_exit` |
| TB-0 | **A binding defines ONLY these things:**
- (1) **frame delimiting**, for the control connection and for routes;
- (2) **peer authentication** to a caller;
- (3) how **progress** is measured for backpressure (TS-7);
- (4) the **close signal**;
- and the connect step that carries (1) to (4).

Everything else (the envelope, `describe`, watches, events, the terminal stream, input, queries, errors) is shared and binding-neutral.

**A binding is not tied to a client kind.** Any client may use any binding its platform supports. For example, a TUI on another machine attaches over `webrtc`, and a Web client could use a local stream where the platform allows it.

**The set is extensible:** a new binding is a new section 9 entry plus a feature name `binding:<name>` in `features[]`. `transport.binding` is an open string, and a client uses only bindings the Hub advertises. The v1 bindings are `local-stream` (9.1), `webrtc` (9.2) and the partial `mcp` binding for agents (9.4). | `conf::tb_0_transcripts_run_unchanged_on_every_binding` (the section 12 transcripts, byte for byte, on each binding), `conf::tb_0_any_client_kind_any_binding`, `conf::tb_0_unknown_binding_not_offered` |
| TR-1 | **Control.** One control connection to the Hub (the socket path). Frames are `[u32 big-endian length][UTF-8 JSON]`, where `length` counts the JSON bytes and is at most `welcome.limits.max_frame_bytes`. | `conf::tr_1_unix_framing` |
| TB-L2 | **Route connect: a token handshake on the Hub's own listener. There is no temporary route path.**
- `transport = { binding: "local-stream", socket_path, route_token }`. `socket_path` is the Hub's listener. `route_token` is **unguessable**, **single-use**, bound to the `route_id` and to the authenticated caller and peer uid, and valid only within `limits.route_connect_seconds`.
- The client opens a **new** connection to `socket_path`. Its first and only handshake frame uses the **control framing** (TR-1: `[u32 big-endian JSON byte length][UTF-8 JSON]`, bounded by `welcome.limits.max_frame_bytes`): `{ "frame": "route_connect", route_id, route_token }`. The listener tells a route connection from a control connection by this first frame (`hello` or `route_connect`), without reading any terminal frame. **After the handoff, the connection carries TS-3 route frames.**
- The Hub reads **exactly** that bounded handshake and verifies the token, the route, the peer uid and the connect window. **An invalid proof does not consume the token.** **Successful verification consumes the token atomically:** if two connections present the same valid token, exactly one gets the route, and the other is closed with `close { route_token_used }`.
- The Hub then hands the connected stream to the session's worker (BP-1; for example by descriptor passing, non-normative) **without reading any terminal frame**, and closes its own copy.
- If the handoff fails, the reservation ends with a typed reason (`route_ended { handoff_failed }`, TS-9).
- A route that is not connected within the window ends with `not_connected`.

**Delimiting:** `[u32 big-endian length][frame]`, where `length` is the frame's size (TS-3). | `conf::tb_l2_token_single_use_bound_to_route_and_uid`, `conf::tb_l2_concurrent_same_token_one_wins`, `conf::tb_l2_handshake_uses_control_framing`, `conf::tb_l2_invalid_proof_keeps_token`, `conf::tb_l2_no_terminal_frame_read_by_hub`, `conf::tb_l2_handoff_failure_typed`, `conf::tb_l2_no_temporary_paths` |
| TB-L3 | **Authentication:** the socket is mode 0600, and the Hub checks the peer uid **[today]**. A client on this binding does not pair. | `conf::tb_l3_peer_uid` |
| TB-L4 | **Progress** (Core OU-3a): bytes that the kernel accepted from the worker's write to the route socket. A readiness report that takes no bytes is not progress (TS-7). **Close:** end of stream. | `conf::tb_l4_progress_and_eof` |
| TR-2 | **WebRTC.** Signalling stays outside the envelope. The **Hub peer connection** carries **one** reliable, ordered `control` DataChannel for the envelope, watches and event streams. **Each route has its own peer connection, terminated by the session worker** (BP-1, BP-4), with one reliable, ordered DataChannel. | `conf::tr_2_control_on_hub_peer_routes_on_worker_peers` |
| TB-W2 | **Route connect.** `transport = { binding: "webrtc" }`. The client creates the route's **own** peer connection and sends `invoke(session, id, attach_offer, { route_id, sdp, client_fingerprint, signature })` on the control channel (BP-4). The answer is `{ sdp }`, the worker's answer. On that peer connection the client opens one reliable, ordered DataChannel labelled **`terminal`** (fixed: each route has its own peer connection). **ICE is not trickled:** both the offer and the answer carry the complete candidate set. | `conf::tb_w2_route_on_worker_peer` |
| BP-4 | **Route authentication, with no pairing secret in the worker.** After `attach` reserves the route (TS-1), `attach_offer` on the authenticated control channel carries the route peer connection's offer **and the client's signature**, made with its paired key (PR-3) over `botster-route-v1\n<route_id>\n<client_fingerprint>`. The Hub verifies it, passes the offer and the **expected client fingerprint** to the worker, and returns the worker's answer on the control channel. That control channel is authenticated, so the client learns the worker's fingerprint from a trusted source. The worker accepts only a DTLS peer with the expected fingerprint. Untrusted signalling learns nothing new. | `conf::bp_4_route_peer_fingerprint_bound`, `conf::bp_4_forged_route_offer_refused` |
| TR-3 | **WebRTC framing.** Each DataChannel message is `[u8 flags][chunk]`. Bit 0 of `flags` = `final`, and the other bits are 0. A frame is the concatenation of its chunks, up to and including the one marked `final`. Each chunk is at most `welcome.limits.max_chunk_bytes` on the control channel, and at most **`route_limits.max_chunk_bytes`** on a route (each chosen at or below the SCTP maximum message size). A control frame is at most `welcome.limits.max_frame_bytes`. A route frame is at most its type's `route_limits` bound (TS-1). A frame that is incomplete when its channel closes is discarded. Control frames are UTF-8 JSON, and route frames are TS-3 frames without the length prefix. **Reliable ordered channels do not lose single messages.** A transport failure closes the channel, and it ends the routes on it (TS-9). | `conf::tr_3_chunking`, `conf::tr_3_incomplete_frame_discarded` |
| TR-4 | **Head-of-line.** Large baselines are chunked (WT-1). Between two chunks of one baseline, the Hub may send, and the client may send, any other frame: responses, cancels, other watches' frames, events. A large baseline never holds the control stream. | `conf::tr_4_responses_between_baseline_chunks` |
| TB-W4 | **Progress** (Core OU-3a): new route payload that leaves the bounded SCTP send queue through transport acknowledgement (the cumulative acknowledgement advances). A retransmission, a heartbeat, a UDP write or a local enqueue is not progress (TS-7; Core's `route_send_buffer_bytes`). **Close:** the channel close, or the peer connection failing. | `conf::tb_w4_progress_and_close` |

### A.1 JSON rules (the control stream and JSON route bodies)
- The encoding is UTF-8 JSON (RFC 8259).
- **Integers** in JSON are within ±(2^53 − 1). A u64 identifier (request `id`, `op` in JSON) is sent as a **decimal string**.
- **Absent means "not provided".** `null` is accepted only where the field's schema permits it, and then it has that field's defined meaning. A required field is never `null`. **In an RFC 7396 merge patch, `null` deletes the member and absence leaves it unchanged,** as that RFC defines.
- **Duplicate object keys**, invalid UTF-8 or invalid JSON are `protocol_error { malformed_frame }`, and the connection closes.
- **Invalid framing** closes the connection with a typed `protocol_error`:
  - `frame_too_large`: the length exceeds the bound **of that frame's type**. A control frame is bound by `welcome.limits.max_frame_bytes`. On a route, `screen` is bound by `route_limits.max_screen_frame_bytes`, `history` by `route_limits.max_history_page_bytes`, and every other frame by `route_limits.max_frame_bytes`;
  - `malformed_frame`: the body is not what the frame type requires;
  - `unknown_frame`: an unknown frame type without the ignorable bit (A.4);
  - `handshake_order`;
  - `duplicate_request_id`;
  - `invalid_input`: a route frame that cannot be parsed. **Only an unparseable frame ends a route.** An unknown enum value inside a parseable input is refused with `input_refused { reason: unsupported { what } }`, and invalid UTF-8 in a `text` frame with `input_refused { reason: unsupported { what: "utf8" } }`. Both have zero effect, and the route stays open.

  On a route, the same codes end the route (TS-9 `write_failed` for the worker's own failure; `route_closed { protocol_error { code } }` for the client's).

### A.2 Reason sets

**Every reason set in this contract is non-exhaustive.** A receiver shows an unknown reason as "other". An unknown `Lost` reason maps to `lost` (`indeterminate`), and an unknown ended cause maps to `ended`.
- **`watch_closed.reason`:** `owner_unloaded`, `owner_failed`, `kind_removed`, `permission_revoked`, `item_too_large`.
- **`close.reason`:** HS-9.
- **`route_closed.reason`:** TS-8, plus `protocol_error { code }`.
- **`route_ended.reason`:** TS-9.
- **`snapshot_begin.reason`:** absent for the first baseline, and one of `overflow`, `source_reset` or `owner_reloaded` for a resync.

### A.2b More reason sets
- **History `unavailable`:** `evicted`, `restart`, `oversize`, `capture_failed`. **`incomplete`:** `truncated`, `capture_failed`.
- **`attach_failed`:** `session_not_running`, `screen_unavailable`, `not_attachable`.

### A.2c `lifecycle_class` of a session

| `state` | Meaning | `lifecycle_class` | Input | `restart` | `delete` |
|---|---|---|---|---|---|
| `starting` (including Core `Created`), `running`, `stopping` | live | `current` | yes (on a live route) | no | no (`wrong_state`) |
| `ended` | the process is known to have ended, including a worker that was lost with a **known** termination (`failure_reason: worker_lost`; Core AD-2 `WorkerGone` and `StartInterrupted`) | `ended` | no | if `restartable` | yes |
| `lost` | adoption is **unresolved**: the worker may still be alive (Core AD-2 `WorkerUnreachable`, `WorkerVersion`, `RegistryCorrupt`) | `indeterminate` | no | no | yes: an operator cleanup, carried out as Core `Remove` (Core LC-7 and AD-2 end a stray worker only after verifying its identity) |

### A.4 Route frames (in TS-3 framing)

Hub → client:

| Type | Frame | Body |
|---|---|---|
| `0x01` | `attached` | JSON `{ features: [..], terminal_format, limits }`, where `limits` restates the attach's `route_limits` as the worker applies them. `features` is the intersection of the client's `route_features` and the worker's features. `terminal_format` is the format negotiated at attach (TS-1). `limits` are the route's **own** limits, announced by the worker that serves it. They are authoritative for that route over the Hub-wide `welcome` values, because an adopted worker may be one version older (Core AD-4). |
| `0x02` | `baseline_begin` | JSON `{ rows, cols, modes: ModeFlags }` |
| `0x03` | `screen` | the Core screen payload (opaque bytes, `terminal_format`) |
| `0x04` | `history` | one Core history payload page (opaque bytes) |
| `0x05` | `baseline_end` | JSON `{ history: "not_requested" | "complete" | { "unavailable": reason } | { "incomplete": reason } }` |
| `0x06` | `live` | empty |
| `0x07` | `output` | the Core output payload (opaque bytes) |
| `0x0E` | `output_deflate` | a raw DEFLATE stream (RFC 1951) of one `output` payload. Sent only when the route negotiated the feature `output_deflate` (TS-1 `route_features`); the worker chooses per frame. The inflated payload is at most the `output` frame bound minus the one type byte (TS-3 counts the type byte); a larger or corrupt stream is `protocol_error { invalid_input }` (parity CG-7: legacy compressed large output, which matters over TURN) |
| `0x08` | `modes` | JSON `{ modes: ModeFlags, rows, cols }` |
| `0x09` | `resync` | JSON `{ reason: "stalled" | "overflow" }` |
| `0x0A` | `input_refused` | JSON `{ op?, reason, written_bytes?, path? }` (TS-5b; `path` only for a file whose path paste failed, TS-5f) |
| `0x0D` | `input_done` | JSON `{ op, written_bytes, path? }`, sent only for a `paste_commit` with `want_result: true` (TS-5b) and for every `file_commit`, with `path` (TS-5f) |
| `0x0C` | `terminal_query` | JSON `{ query_id, request_bytes_base64, kind?, shadow_answerable, answer_within_ms }`. `request_bytes_base64` is the exact query sequence, so a client can answer any query its emulator understands with `encoded`. `kind` is present **only** for Core's typed kinds, and it uses Core's `QueryKind` names at the frozen Core contract, in snake_case. The set is **non-exhaustive**, and a client declines an unknown kind: `{ "clipboard_read": { selection } }`, `"text_area_pixels"`, `"cell_pixels"`, `"screen_pixels"`, `"window_pixels"`, `"screen_chars"`, `"window_state"`, `{ "window_position": { area } }`, `"window_title"`, `"icon_label"`, `"color_scheme"`. Model queries carry no `kind`: the client reads `request_bytes_base64`. `shadow_answerable` says whether the Core fallback has an answer. A client answers each kind from its own real terminal state, or declines; the worker then falls back to the shadow terminal (P9). `query_id` is Core's opaque id. |
| `0x0B` | `route_closed` | JSON `{ reason, exit?: { code?, signal? } }` (TS-8) |

Client → Hub:

| Type | Frame | Body |
|---|---|---|
| `0x81` | `input` | JSON, one of the shapes in A.5 |
| `0x82` | `paste_chunk` | `[u64 big-endian op][bytes]`. In every binary input frame, `op` 0 means "no op"; a paste needs a nonzero `op`. |
| `0x83` | `bytes` | `[u64 big-endian op][bytes]`: raw bytes written as they are, for IME and passthrough |
| `0x84` | `text` | `[u64 big-endian op][UTF-8 bytes]`: text written as one transaction (invalid UTF-8 is refused with `unsupported { what: "utf8" }`) |
| `0x85` | `file_chunk` | `[u64 big-endian op][bytes]`: file content (TS-5f), never written to the PTY |

**Extension rules (gate 5):**
- **Frame types:** bit `0x40` of a type marks it **ignorable**. A receiver drops an unknown ignorable frame, and an unknown frame **without** that bit is `protocol_error { unknown_frame }`. So a new worker-to-client notice uses `0x40` to `0x7F`, and a new client-to-worker hint uses `0xC0` to `0xFF`. A new frame that must be understood is sent only when the peer advertised its feature. **The worker learns the client's route features from the attach:** `attach` carries `route_features: [..]` (the route features the client understands), and the Hub passes them to the worker with the route handoff. **The client learns the worker's features from `attached.features`.**
- **Input:** an unknown input `kind` in a `0x81` frame is answered `input_refused { op?, reason: unsupported { what } }`, and the route stays open.
- **Queries:** an unknown query `kind` is answered by the client with `decline` (then the P9 fallback applies). An unknown reply variant is refused with `input_refused { reason: unsupported { what } }`.
- **Enums and modes:** an unknown enum value or mode name is shown as "other", or ignored (EN-6).

### A.5 Input shapes (`0x81`), with `op` as a decimal string

**The semantic input model is Core contract section 5.1A, as frozen in Core contract v1.0, adopted verbatim.** This contract adds no key, mouse or mode vocabulary of its own. **Field aliases are forbidden**: the wire uses exactly Core's field and value names, and the Hub converts nothing. Any settlement of 5.1A before the Core freeze (for example, the produced text and alternate keys of a `Key`, raised by Sol with the Core writer) applies here unchanged. The shapes below are a summary; **5.1A governs.**
- `{ op, kind: "key", key: { "named": <NamedKey> } | { "char": "<scalar>" }, mods: [..], event: "press" | "repeat" | "release", text?, shifted_key?, base_layout_key? }`. The produced-text and alternate-key fields are exactly as Core 5.1A settled them. `NamedKey` and the meaning of each modifier are as in Core 5.1A (lowercase: `enter`, `arrow_up`, `f1`…`f35`, the keypad keys, the modifier keys; `mods` ⊆ `shift`, `alt`, `ctrl`, `meta`, `super`, `hyper`, `caps_lock`, `num_lock`).
- `{ op, kind: "mouse", action, button, row, col, x?, y?, mods, notches? }`: Core 5.1A. `row` and `col` are zero-based cells. `x` and `y` are zero-based pixels, used only with the `sgr_pixels` encoding. `action` is `wheel` exactly when `button` is a `wheel_*` value.
- `{ op, kind: "focus", focused }`.
- Text is **not** a JSON shape: it is the raw frame `0x84` (BP-3).
- `{ op, kind: "resize", rows, cols }`.
- `{ op, kind: "reply_terminal_query", query_id, reply }`. `reply` is a the JSON projection of a Core `ReplyTerminalQuery` variant (byte fields become `*_base64`):
  - `{ "clipboard": { selection, data_base64 } }` (the request's `selection` exactly);
  - `{ "pixels": { width, height } }`;
  - `{ "chars": { rows, cols } }`;
  - `{ "window_state": { iconified } }`;
  - `{ "position": { x, y } }` (u16 xterm wire values);
  - `{ "text": { text } }` (for `window_title` and `icon_label`);
  - `{ "color_scheme": "dark" | "light" }`;
  - `{ "encoded": { bytes_base64 } }`: the complete reply bytes chosen by the client, for **any** query, including model queries with no `kind`, at most `route_limits.max_query_reply_bytes`;
    - `"decline"`: it triggers the fallback (P9 step 3); it is never a final answer. The client never sends `shadow`; only the worker's fallback writes the shadow answer.

  **Typed variants are encoded by Core. `encoded` bytes are produced by the selected client.** In both cases, raw `output` is untouched, and no automatic reply ever travels as `text` or `bytes`. **A reply is fire-and-forget too:** only a refusal is sent, as `input_refused { op?, reason }` with `reason` ∈ `already_replied`, `query_expired`, `unknown_query`, `invalid_input`, `too_large` (non-exhaustive).
- `{ op, kind: "paste_begin", total_bytes?, require_bracketed? }`, then `paste_chunk` frames, then `{ op, kind: "paste_commit", want_result? }` or `{ op, kind: "paste_abort" }`.
- `{ op, kind: "file_begin", name, mime?, total_bytes? }`, then `file_chunk` frames, then `{ op, kind: "file_commit" }` or `{ op, kind: "file_abort" }` (TS-5f). Every other shape takes an optional `op`.

Semantic input is **always available**: Core makes IN-9 mandatory in 0.1, so no feature gate is needed. The worker encodes with the modes at the start of the write.

**Input not reported by the modes:** Core `NotWritten(NotReported)` (for example a mouse event with tracking off, or a key release in legacy mode) sends **no frame**, as on a real terminal. `NotWritten(Unsupported { what })` becomes `input_refused { op?, reason: unsupported { what } }`.

### A.6 `ModeFlags`
**It is Core 5.1A's `ModeFlags`, verbatim:** `alt_screen`, `cursor_visible`, `bracketed_paste`, `application_cursor_keys`, `application_keypad`, `focus_reporting` (all bool); `kitty_flags` (a u8 bitset: 1, 2, 4, 8, 16 as the kitty protocol defines them); `mouse_tracking` ∈ `none`, `x10`, `normal`, `button_event`, `any_event`; `mouse_encoding` ∈ `x10`, `utf8`, `sgr`, `urxvt`, `sgr_pixels`; and `other_modes` (unknown modes, ignored). On the wire it is a JSON object with these fields.

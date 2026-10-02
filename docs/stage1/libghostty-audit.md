# libghostty audit for Core Stage 1 (package P2)

Scope: every Core clause that needs terminal semantics, checked against libghostty.
Contract: botster-contracts tag `contracts-v0.1.1` (`366bca41da0a6de69cc1ea13b17c773cdfdb75b6`): manifest final14, Core erratum 2 ("E2") and steward ruling R-13.
Plan pin: `stage1-plan.555bc433` (sha256 `555bc4337fe72e9fad833fe330d43e8147a39569cb14291734f34847f56596d7`), sections 0, 6.1, 6.3, 7.1, 8, 9 (Q1) and 10 (R1).
Revision 3 of this audit. Revision 2 closed findings F1 to F8. Revision 3 closes F9 to F12 of the reviewer verdict `3b2acb3`: the per-commit mapping of the nine removed commits (F9), the offline build prerequisite (F10), the UP clipboard-write callback (F11) and the snapshot rule at every cut (F12). R-13 closes the SGR-pixels question (old Q5).

## Method and revisions

- **Pinned fork:** `trybotster/ghostty` at `eb72ec61304ea256be1d86ed8fa961c84e43ecbd` ("PIN").
- **Upstream main:** `ghostty-org/ghostty` at `83edd491e3024ae5e50393d62877b8897da1cccd` ("UP"), fetched 2026-10-01. The lead approved UP as the base of the new fork branch.
- Headers are under `include/ghostty/vt/`. Zig sources are under `src/`. All reads are `git show <sha>:<path>`, or reads in a checkout of UP.
- A row says **COVERED** when a libghostty symbol serves the clause. It says **GAP** when no symbol serves it without terminal parsing or encoding outside libghostty (BUILD.md rule 1). It says **NOTE** when libghostty serves the clause and the binding follows a stated rule. A NOTE never hides a known source defect: a known defect is a GAP.
- "UP" in a row means that upstream main has the symbol and the pin does not.
- The binding keeps no terminal semantics. It keeps bookkeeping only: counters, buffers, range checks of its own arguments, and copying.

## Summary

| ID | Gap | PIN | UP | Resolution |
|---|---|---|---|---|
| G1 | Query detection: exact request bytes, kind, and a stop at the query boundary (EV-8) | none | none | fork patch 1 |
| G2 | Notification source, OSC 9 or OSC 777 (A2-4, A3-1, EV-7) | none | none | fork patch 2 |
| G3 | Paste without sanitizing (IN-8) | `ghostty_paste_encode` strips and rewrites | same | fork patch 3 |
| G4 | OSC 1 and `TitleChanged` | OSC 1 ignored | same | **none: E2 ruling A** (OSC 1 gives no `TitleChanged`) |
| G5 | Key input: `hyper` and `meta` bits, `shifted_key`, `base_layout_key`, `f26` to `f35`, and the kitty associated-text rules (5.1A) | missing, rules differ | same | fork patch 4 |
| G6 | OSC 133 prompt marks with exit code (A2-4) | state only | **covered** | move to UP |
| G7 | Clipboard read request (EV-8) | ignored | **covered** (payload gap in G10) | move to UP |
| G8 | Mouse: coordinate semantics and the active tracking and format enums (5.1A, ST-4, EV-7) | defects, no getters | same | fork patch 6 |
| G9 | Typed query reply encoders (EV-8) | partial | same | fork patch 7 |
| G10 | Clipboard `selection` and terminator (EV-3, EV-8) | location only | same | fork patch 8 |
| G11 | `xterm-ghostty` terminfo source text (TI-1) | not exported | same | fork patch 5 |
| H1 | Image and placement state in snapshots (ST-6b) | not serialized | same | **configuration, no patch** (see ST-6b graphics row) |

Other findings:

- **No callback for a mode change** at PIN or UP. E2 ruling C accepts a read after each chunk. No patch.
- **The plan names `GHOSTTY_TERMINAL_OPT_MODE` (34) as a callback.** It is a setter.
- **The nine fork-only SEGV commits:** section "Pin move" has one row per commit. One root-cause lifetime fix is carried. The state-losing degradation patches are not carried.
- **Terminfo:** the entry is identical at PIN and UP. I compared `src/terminfo/ghostty.zig` and `src/terminfo/Source.zig`.
- **Zig is `0.16.0` at both** (`build.zig.zon` `minimum_zig_version`).
- **UP adds build dependencies:** the lib-vt build needs seven Zig packages, among them `translate_c` (from `codeberg.org`) and `uucode` (from `github.com`). The gate builds offline from a prefetched cache. See section "Build notes".

## Audit rows

### Input (IN-8, IN-9, IN-10, 5.1A, DP-12)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| IN-8 | Paste wrapped in `ESC[200~ … ESC[201~` when bracketed mode is on, bare when off. Core does no payload inspection. DP-5 says bytes reach the PTY exactly as sent. | `paste.h`: `ghostty_paste_encode(data, len, bracketed, buf, …)`. | **GAP G3.** `ghostty_paste_encode` replaces unsafe bytes (NUL, ESC, DEL and others) with spaces. When `bracketed` is false it also turns newlines into CR. UP has the same function. UP's `ghostty_terminal_paste` is a clipboard-driven path with the same stripping unless `allow_unsafe` is set. Patch 3 adds a wrap-only encode. The binding must not write the wrapper by hand (rule 1). The lead confirmed byte-exact. |
| IN-8 | Bracketed mode at the start of the write transaction. | `ghostty_terminal_get(…, GHOSTTY_TERMINAL_DATA_MODE)` with `GHOSTTY_MODE_BRACKETED_PASTE` (2004). | COVERED. One mode read at admission. |
| IN-9 `Key` | Encode a key with the authoritative modes. | `key/encoder.h`: `ghostty_key_encoder_new`, `_setopt_from_terminal`, `_encode`. `key/event.h`: `_set_action`, `_set_key`, `_set_mods`, `_set_consumed_mods`, `_set_utf8`, `_set_unshifted_codepoint`. | COVERED for legacy keys and application cursor and keypad keys. Kitty rules: see the rows below. |
| IN-9 `Key` | `shifted_key` and `base_layout_key` (kitty flag 4). | `key/event.h` has `unshifted_codepoint` and `utf8` only. In `src/input/key_encode.zig` the encoder derives the shifted key from `utf8` and the base key from the key code. | **GAP G5.** No setter for either field. 5.1A says Core never derives them. Patch 4 adds the setters and makes the encoder use them as given. |
| IN-9 `Key` | `mods`: `shift alt ctrl meta super hyper caps_lock num_lock`. | `key/event.h`: `GHOSTTY_MODS_SHIFT`, `CTRL`, `ALT`, `SUPER`, `CAPS_LOCK`, `NUM_LOCK` plus side bits. | **GAP G5.** No `HYPER` and no `META` bit (kitty bits 16 and 32). Patch 4. |
| IN-9 `Key` | `NamedKey`: `f1` to `f35`, keypad keys, modifier keys, `print_screen`, `pause`, `menu`. | `key/event.h` `GhosttyKey`: `GHOSTTY_KEY_F1` to `F25`, `NUMPAD_*`, `PRINT_SCREEN`, `PAUSE`, `CONTEXT_MENU`, `CAPS_LOCK`, `NUM_LOCK`, `SCROLL_LOCK`, left and right modifier keys. | **GAP G5** for `f26` to `f35`. Patch 4. |
| IN-9 `Key` | Kitty flags use the top of the stack masked to `0x1F`. Associated text (flag 16) is encoded only when flags 8 and 16 are both set. A `text` with any control codepoint (C0, DEL, C1) is omitted from the associated-text parameter only; the key still encodes, and the same text still serves legacy output. | Encoder option `GHOSTTY_KEY_ENCODER_OPT_KITTY_FLAGS`. `src/input/key_encode.zig` (PIN and UP, near line 294) tests `report_associated` without testing `report_all`. `isControl` is `cp < 0x20 or cp == 0x7F` and misses C1. `KittySequence.encodeFull` skips single ASCII controls and does not omit the whole text. `utf8` is one field for legacy text, plain-text output, alternate-key inference and associated text. | **GAP G5.** The binding must not clear `utf8` (that changes legacy output). Patch 4 changes the encoder: associated text only with flags 8 and 16; omit the whole associated-text parameter if any codepoint is C0, DEL or C1. Mask to `0x1F` stays binding-side input handling because `_setopt` takes the stack value. |
| IN-9 `Key` | Zero output is typed (`NotReported`, `Unsupported{what}`). | `ghostty_key_encoder_encode` returns `out_len == 0`. | COVERED. The binding maps `out_len == 0` to the typed result by the 5.1A rule. It adds no bytes. |
| IN-9 `Mouse` | Encode press, release, move and wheel for X10, UTF-8, SGR, URXVT and SGR-pixels, for tracking 9, 1000, 1002 and 1003. | `mouse/encoder.h`: `ghostty_mouse_encoder_new`, `_setopt_from_terminal`, `_setopt` with `GHOSTTY_MOUSE_ENCODER_OPT_SIZE`, `_encode`. Event positions are floating-point surface pixels. | COVERED for tracking and format selection (`_setopt_from_terminal` reads the active enums inside libghostty). Coordinates: see the next rows. |
| IN-9 `Mouse` | `row` and `col` are zero-based cells (Core adds the 1). | `src/input/mouse_encode.zig`: cell formats write `cell.x + 1`. The cell comes from `posToCell`, which converts a pixel position and **clamps** an out-of-range value to the grid. | **GAP G8.** The C API takes pixels only. To pass a cell, the binding would set a 1-pixel cell size and a position of `col`, `row`; that reuses libghostty's conversion but still clamps. Patch 6 adds a cell-coordinate entry point that does not clamp and reports an unrepresentable cell as no output. |
| IN-9 `Mouse` | A coordinate that the encoding cannot express is not reported (typed zero), never truncated. | `mouse_encode.zig`: X10 over 222 returns no output (good). For non-release events a position outside the viewport returns no output. A **release** bypasses the viewport check and `posToCell` clamps it. | **GAP G8.** A release outside the grid is encoded at the clamped cell. Patch 6 fixes this together with the previous row. |
| IN-9 `Mouse` | SGR-pixels: `x` and `y` are zero-based surface pixels. R-13: SGR-pixels reports zero-based pixels with no +1; the 5.1A `+1` is for cells only. | `mouse_encode.zig` (`.sgr_pixels`): writes `posToPixels` (rounded terminal-space pixels) with **no +1**, while cell formats add 1. | COVERED by R-13 for the offset. The C API still takes floating-point surface positions, which is the pixel input that this format needs. The cell coordinate path is G8 (rows above). The binding adds no offset. |
| IN-9 `Mouse` | Without `x` and `y` under SGR-pixels the result is typed zero. | none needed | NOTE. The binding refuses before the encoder runs. This is argument checking. |
| IN-9 `Focus`, DP-12 | `ESC [ I` and `ESC [ O` only when focus reporting (1004) is on. | `focus.h`: `ghostty_focus_encode(GHOSTTY_FOCUS_GAINED \| GHOSTTY_FOCUS_LOST, …)`. `DATA_MODE`, `GHOSTTY_MODE_FOCUS_EVENT`. | COVERED. |
| IN-10 terminal guard | The write is refused if `model_rev` differs from the guard. | none | NOTE. Core bookkeeping: the binding exposes its `model_rev` counter (see ST-1). The guard compare belongs to P3. No library semantics. |
| 5.1A `alt_screen` | read | `DATA_ACTIVE_SCREEN` | COVERED |
| 5.1A `cursor_visible` | read | `DATA_CURSOR_VISIBLE`, or `DATA_MODE` 25 | COVERED |
| 5.1A `bracketed_paste` (2004) | read | `DATA_MODE` | COVERED |
| 5.1A `application_cursor_keys` (1) | read | `DATA_MODE`, `GHOSTTY_MODE_DECCKM` | COVERED |
| 5.1A `application_keypad` | read | `DATA_MODE`, `GHOSTTY_MODE_KEYPAD_KEYS` (66) | COVERED |
| 5.1A `focus_reporting` (1004) | read | `DATA_MODE`, `GHOSTTY_MODE_FOCUS_EVENT` | COVERED |
| 5.1A `kitty_flags` | top of the stack | `DATA_KITTY_KEYBOARD_FLAGS` | COVERED |
| 5.1A `mouse_tracking` | the active tracking mode | PIN and UP: `DATA_MOUSE_TRACKING` is a boolean. `DATA_MODE` reads one bit per mode (9, 1000, 1002, 1003). `src/terminal/modes.zig` changes only the requested bit. `src/terminal/stream_terminal.zig` sets `terminal.flags.mouse_event` separately on each command. Enabling 1000 then 1003 and enabling 1003 then 1000 give equal bits and different active modes. | **GAP G8.** The bits cannot give the authoritative value. The binding must not rebuild it from bit precedence. Patch 6 adds `GHOSTTY_TERMINAL_DATA_MOUSE_EVENT`. |
| 5.1A `mouse_encoding` | the active format | Same as above for `terminal.flags.mouse_format`. | **GAP G8.** Patch 6 adds `GHOSTTY_TERMINAL_DATA_MOUSE_FORMAT`. |
| `other_modes` | E2 ruling B: the modes that libghostty tracks and that have no normative field. An unrecognized mode number changes no field. | `modes.h` lists the tracked modes (`GHOSTTY_MODE_*`). `DATA_MODE` reads each. | COVERED. The binding reads each tracked mode that has no normative field. No second parser. |

### State, snapshot and size (SZ-1, ST-1 to ST-7)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| SZ-1 `cell_px` and size | Apply the size and `cell_px`. Reads: `Size.cell_px` decides which pixel queries the shadow answers. | `ghostty_terminal_resize(terminal, cols, rows, cell_width_px, cell_height_px)`. `DATA_COLS`, `DATA_ROWS`, `DATA_WIDTH_PX`, `DATA_HEIGHT_PX`. `GHOSTTY_TERMINAL_OPT_SIZE` (6) supplies the size to XTWINOPS replies. | COVERED. The PTY resize and `Superseded` bookkeeping belong to P3. |
| ST-1 `model_rev` | A token that changes on output, resize, mode change, title change and cwd change. | none | NOTE. Core bookkeeping: one counter, advanced after every `vt_write` of a nonzero length, every `resize`, and every observed mode, title or cwd change. It holds no terminal semantics. It never says "unchanged" when something changed. |
| ST-2 screen text, `history_unavailable` | Plain text with scrollback. | `formatter.h`: `ghostty_formatter_terminal_new`, `_format_buf`, `_format_alloc`. `DATA_SCROLLBACK_ROWS`, `DATA_TOTAL_ROWS`. | COVERED. |
| ST-3 `row`, `col`, `visible` | Zero-based cell coordinates. | `DATA_CURSOR_Y`, `DATA_CURSOR_X`, `DATA_CURSOR_VISIBLE`, `DATA_CURSOR_PENDING_WRAP`. | COVERED. |
| ST-3 `row_text`, `text_before_cursor` | Row text with trailing empty cells and spaces removed. Cells `[0, col)` untrimmed. | `grid_ref.h`: `ghostty_grid_ref_cell`, `_graphemes`; or `formatter.h` on one row. | COVERED. The ST-3 trim rule is string handling on cell data. A wide character takes two cells. |
| ST-4 `terminal_state` | `size`, `modes`, `title`, `cwd`, `model_rev`. | `DATA_COLS`, `DATA_ROWS`, the mode reads above, `DATA_TITLE` (12), `DATA_PWD` (13). | COVERED. The worker keeps a copy for the sync read; the copy is Core bookkeeping. The `modes` field depends on G8. |
| ST-5 exit hold | Reads work after `Exited`. | The terminal object lives until `ghostty_terminal_free`. | COVERED. P3 frees it at `Remove`. |
| ST-6 `GHOSTSNP` | Capture the terminal as a snapshot. | `snapshot.h`: `ghostty_snapshot_encode`, `_encode_buf`, `_encode_alloc`. Magic `GHOSTSNP` plus a `u16` version. CRC32C per record. Decoder: `ghostty_snapshot_decoder_*`. | COVERED. The decoder refuses a bad version with `GHOSTTY_INVALID_VALUE`. |
| ST-6 pages | `read_page` pages of the snapshot bytes. | A snapshot is a byte stream. | NOTE. Paging is a Core split of the encoded bytes. |
| ST-6b (1) cells, attributes, OSC 8 hyperlinks | | `src/terminal/snapshot/screen.zig`, `page.zig`, `hyperlink.zig`, `style.zig` | COVERED. |
| ST-6b (2) palette and dynamic colors | | TERMINAL record: original palette, override mask, overrides. | COVERED. |
| ST-6b (3) cursor position, visibility, shape, blink | | TERMINAL record: cursor default style and blink policy. SCREEN record: cursor. | COVERED. |
| ST-6b (4) mode fields, kitty flags, mouse tracking and encoding | | TERMINAL record: mouse event and format, current, saved and default modes. SCREEN record: kitty keyboard stack. | COVERED. |
| ST-6b (5) size, alternate screen | | TERMINAL record: size, active screen key, screen count. | COVERED. |
| ST-6b (6) title and cwd | | TERMINAL record: PWD and title. | COVERED. |
| ST-6b graphics | Restore images "when the terminal model holds them" and `snapshot_graphics` is advertised. A model that cannot hold them reports the feature absent. | `src/terminal/snapshot/screen.zig` (line ~500) says kitty image payloads are outside the record. `src/terminal/snapshot/snapshot.zig` (test at ~1221) preserves only virtual-placeholder rows and expects zero images and zero placements after a restore. The same at UP. | **H1: not covered, resolved by configuration.** The binding sets `GHOSTTY_TERMINAL_OPT_KITTY_IMAGE_STORAGE_LIMIT` to zero. The doc says that zero disables the kitty graphics protocol and deletes stored images and placements. The model then never holds an image, `snapshot_graphics` is absent from `features()`, and the exclusion is stated, as ST-6b requires. The APC bytes stay in `Output` (OU-12). A test feeds a kitty image and checks that the snapshot has no image. If the orchestrator wants graphics, a fork patch to serialize the image and placement registries is a later item. |
| ST-6b resume invariant at every cut (F12) | Saved cursor, tab stops, margins, rendition, charsets, parser pending state. The invariant holds at every cut, also for production output outside any test corpus, also when continuation data is unavailable. | Snapshot has tab stops, scroll margins, saved cursor, charset state, previous codepoint and a CONTINUATION record. `GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES` (31). `ghostty_terminal_continuation_write`, `_buf`, `_alloc`. `DATA_VT_GROUND` (38). | **COVERED, with the cut rule in section "ST-6b cut rule" below.** A snapshot with no ground state and no continuation is refused and maps to `SnapshotTooLarge` (A8-2, accepted in manifest final16; its a8_* ids enter the ledger with the next contracts tag). The rule has two parts: libghostty refuses an incomplete snapshot, and the binding reports the refusal as a typed result. The worker does not wait for ground; it maps the result as rule 3 says. The corpus test `conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences` is a proof of the covered path only. The refusal path has its own tests (see the section). |
| ST-7 atomic facts | `modes`, `size`, `cursor` read at one point. | All are synchronous reads of one terminal. | COVERED. The worker thread owns the terminal, so the reads are one point. The input records and `client_tail` are Core bookkeeping (P3). |

### ST-6b cut rule (F12)

Facts at UP (`83edd491e`), from `git show`:

- `Tracker` (`src/terminal/stream_continuation.zig`) keeps the replay suffix of an unfinished sequence. It sets `broken` when the suffix is longer than `max_bytes` (lines 95 to 97, the checks at 166 to 172 and 182 to 186) or when the allocation fails. `broken` stays set until a feed ends at ground or contains a new replay start.
- `ghostty_snapshot_encode` (`src/terminal/c/snapshot.zig`, `continuationOwned`) reads the continuation **before it writes any byte**. When the parser is not at ground and the continuation is disabled or unavailable, it returns `GHOSTTY_INVALID_VALUE` and writes nothing. The C wrapper preflights the continuation before it builds any writer: `src/terminal/c/snapshot.zig` `encode` lines 565 to 567 and `encode_alloc` lines 645 to 646 (`continuationOwned`, then return on any result other than success). The core encoder says the same at `src/terminal/snapshot/snapshot.zig` line 50: "Continuation errors must not emit even the snapshot envelope." At ground the continuation is empty, and tracking is not needed.
- So libghostty never emits a snapshot that silently omits parser state. The case "unavailable" is a typed refusal, not a short snapshot.

The rule:

1. **Tracking is always on.** The binding sets `GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES` to a nonzero value (`CONTINUATION_LIMIT`, 1 MiB) when it creates the terminal, before the first `vt_write`. The binding does not claim that this limit is larger than every admitted sequence. It does not need the claim, because the refusal path below is complete.
2. **The binding never returns a partial snapshot.** `GhosttyTerminal::snapshot()` returns the bytes of `ghostty_snapshot_encode_alloc` on success. On `GHOSTTY_INVALID_VALUE` it reads `DATA_VT_GROUND`. If the parser is not at ground it returns the typed result `SnapshotUnavailable::ContinuationUnavailable`. It never retries with a different continuation and never edits snapshot bytes.
3. **The worker maps the typed result to `SnapshotTooLarge` (Core Amendment 8, A8-2, accepted in manifest final16 (`frozen/current/core-contract-v1.17-amendment-8-candidate5.md`); its a8_* ids enter the pinned ledger with the next contracts tag that carries final16).** A snapshot that cannot be formed within its bounds, which includes a pending sequence beyond what the format can carry with no ground state available, is `SnapshotTooLarge`:
   - `CaptureSnapshot` completes `SnapshotTooLarge` (async, one completion);
   - a route baseline or a resync closes the route at once with reason `SnapshotTooLarge`, with no partial baseline.
   There is no waiting, no deadline and no new limit. Every snapshot that is offered meets the resume invariant, because libghostty emitted it with its continuation (or at ground). The worker (P3) owns the mapping. The ground-state deferral of the previous revision is withdrawn.
4. **The format description names the limit.** ST-6b says exclusions are stated. The `snapshot_format` description (P1 and P3 own the text) names libghostty's continuation limit: `CONTINUATION_LIMIT` (1 MiB). **Core Amendment 8** (accepted, manifest final16, `frozen/current/core-contract-v1.17-amendment-8-candidate5.md`) covers both causes: "pending parser state that the model can no longer carry, for any reason (larger than the format's stated continuation limit, or the model failed to retain it), with no ground state available" is `SnapshotTooLarge`, a resource condition. The C API reports both as `GHOSTTY_INVALID_VALUE`, and the binding does not need to tell them apart. The `snapshot_format` text names the limit, as ST-6b requires.
5. **Owners.** The binding (P2) owns: the nonzero limit, `snapshot()`, the typed result, `vt_ground()` and the `CONTINUATION_LIMIT` constant. The worker (P3) owns the `SnapshotTooLarge` mapping for captures, baselines and resyncs.

Tests (P2 supplies the oracle, P3 supplies the worker tests):
- **Refusal:** set the limit to a small value, feed a sequence longer than the limit and stop inside it, and check that `snapshot()` returns `ContinuationUnavailable` with no bytes. Feed a bounded sequence cut in the middle, and check that `snapshot()` succeeds and that decoding it and applying the rest of the bytes gives the same state as the uncut model.
- **Ground:** at ground, `snapshot()` succeeds with an empty continuation, and the restore plus the remaining bytes equals the uncut model.
- **Owned id (plan revision 12, `stage1-plan.00f63f57`, contracts `contracts-v0.1.3`):** `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant` is P2's. It is the same property as the corpus test below: every snapshot that the binding offers meets the exact resume invariant. The contracts pin moves to `contracts-v0.1.3` in one commit when P2 moves it.
- **Every byte offset (the ST-6b corpus test):** at each offset the test accepts one of two outcomes. Either an offered snapshot meets the exact resume invariant, or `ContinuationUnavailable` where the pending state is beyond the stated limit. The corpus is small: a small screen and small history, so that every snapshot fits in `max_snapshot_bytes`. The only refusal the test allows is the pending-state one beyond the stated limit. The test injects no resource failure. A refusal at an offset within the limit fails the test. The plain oversize refusal is covered elsewhere (`ou_9_oversize_snapshot_closes_route_typed`).
- **Retention failure:** the binding cannot make the tracker's allocation fail in a test. The small limit sets the same `broken` flag, so the refusal test covers the same code path. The test does not claim to cover the allocator branch. The route-side id `a8_2_resync_inside_an_uncarriable_sequence_closes_route_snapshot_too_large` belongs to P4a.

### Events (EV-1, EV-3, EV-7, A2-4, A3-1)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| EV-1 | Title, cwd, bell, notification, prompt mark reach the host in observation order. | Callbacks run inside `vt_write` in stream order. | COVERED. The binding appends each observation to a bounded buffer inside the callback: no block, no lock, no I/O. |
| EV-7 title | OSC 0 and OSC 2 give `TitleChanged`. | `GHOSTTY_TERMINAL_OPT_TITLE_CHANGED` (5). Read `DATA_TITLE` (12). `stream_terminal.zig` `windowTitle` cuts a title at 1024 bytes. | COVERED. NOTE: the cut may fall inside a UTF-8 character. The binding checks the read value and keeps a complete prefix. |
| EV-7 OSC 1 | E2 ruling A: OSC 1 gives no `TitleChanged`; its bytes stay in `Output`. | `src/terminal/stream.zig` (~line 2490): `change_window_icon` is logged and ignored. | COVERED by the ruling. No patch. |
| EV-7 cwd | OSC 7 gives `CwdChanged`. | `GHOSTTY_TERMINAL_OPT_PWD_CHANGED` (25). Read `DATA_PWD` (13). The header specifies that the pwd is cleared by an empty value; the getter also returns an empty value when none was set. | COVERED. The binding compares with the last value and does not treat "empty" as a clear without a callback. |
| EV-7, A2-4 bell | BEL gives `Bell`. | `GHOSTTY_TERMINAL_OPT_BELL` (2). | COVERED. |
| EV-7, A2-4 prompt marks | OSC 133 A, B, C, D give `PromptMark{mark, exit_code?}`. | PIN: `stream_terminal.zig` calls `terminal.semanticPrompt(value)`, which updates state only. `DATA_CURSOR_AT_PROMPT` (39) is a boolean. UP: `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT` (42), `GhosttyTerminalSemanticPromptFn`, `GhosttyTerminalSemanticPrompt{kind, prompt_kind, has_exit_code, exit_code, …}`. Kinds: `PROMPT_START` (A), `INPUT_START` (B), `OUTPUT_START` (C), `COMMAND_END` (D). Commit `7bb45ba34`. | **G6: gap at PIN, covered at UP.** |
| EV-7, A2-4 notification | OSC 9 and OSC 777 `notify` give `Notification{source, title?, body}`. Other OSC forms stay in `Output`. | `GHOSTTY_TERMINAL_OPT_DESKTOP_NOTIFICATION` (29), `GhosttyTerminalDesktopNotification{size, title, body}`. Same at UP. | **G2.** No source field. OSC 9 always has an empty title, but an OSC 777 `notify` with an empty title looks the same. Patch 2 adds `source`. |
| A2-4 truncation | `title` and `body` bounded, cut at a character boundary. | Callback strings are borrowed. | NOTE. The binding copies and cuts at a character boundary. |
| EV-3, EV-7 clipboard write | `ClipboardWrite{selection, bytes, total_bytes}`. | `GHOSTTY_TERMINAL_OPT_CLIPBOARD_WRITE` (26), `GhosttyClipboardWrite{location, contents, contents_len, name}`. `src/terminal/stream_terminal.zig` (~line 508) maps `s` and `p` to distinct locations and every other selector to the standard clipboard. The original selector string is lost; `c` and `q` look the same. | **G10.** `selection` is part of the event. Patch 8 adds the original selection string to the write payload. **UP callback shape (F11):** `GhosttyTerminalClipboardWriteFn` returns `void` (`terminal.h` 816 to 870). The callback must call `write->reply(write, &reply)` before it returns; a return without a reply denies the write. The binding does not carry the PIN interface, where the callback returned a result. The binding's callback copies the borrowed `contents`, `location` and the patch 8 selection string into its bounded event buffer, and then calls `write->reply(write, &reply)` once, inside the callback, with `reply.result = GHOSTTY_CLIPBOARD_WRITE_RESULT_SUCCESS` and `reply.remember = false`. Core decides nothing here: EV-3 reports the write as an event. OSC 52 has no write acknowledgement, so libghostty discards the reply and writes no byte to the PTY. A test feeds `OSC 52 ; c ; <base64> ST` and checks that no byte reaches the PTY and that the event carries the selection `c`. The `write` object and its borrowed data are valid only until the callback returns, so the buffer copy happens first and nothing keeps the pointer. The oversize rule is the binding's byte count. |
| EV-3, EV-8 clipboard read | `ClipboardRead{selection}`, and a typed reply that carries the request's selection and terminator. | PIN: OSC 52 read ignored. UP: `GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ` (38), `GhosttyClipboardRead{location, mimes, mimes_len, list, can_remember, reply}`. | **G7 at PIN, covered at UP, with G10:** UP gives `location` but not the selector string or the terminator. Patch 8 adds both. The UP callback is synchronous and generates an empty-clipboard answer when it returns without a reply or with a denial. The binding must suppress that answer: it never reaches the PTY and it never counts as a shadow answer (EV-8: the shadow never answers a clipboard read). Test: feed `OSC 52 ; c ; ? ST` and check that no byte is written to the PTY by libghostty. |
| EV-7 OSC 8 hyperlink | In `Output` and in every snapshot. | Snapshot `hyperlink.zig`. | COVERED. |
| EV-7 every mode change | `ModesChanged` with the full `ModeFlags`. E2 ruling C: after each `vt_write`, compare the final `ModeFlags` with the last posted value; post one `ModesChanged` if different, none if equal. | No mode-change callback at PIN or UP. | COVERED by the ruling. No patch. Binding constraint for the worker (P3): input is admitted only between `vt_write` calls, never inside one; each admission reads the model's current modes. Tests: `e2_3_one_step_net_unchanged_posts_nothing`, `e2_3_one_step_net_changed_posts_final_flags`. |
| A3-1 | Classify a notification. | Same as the notification row. | G2. |
| OSC 9;4 progress | Not a Core event. | `GHOSTTY_TERMINAL_OPT_PROGRESS_REPORT` (30). | Not used. The sequence stays in `Output`. |

### Queries and replies (EV-8, OU-12)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| EV-8 detect a query | Exact request bytes (at most `max_query_bytes`), the kind, and an offer to the client before the shadow answers. | Per-query effects exist for device attributes (8), XTWINOPS size (6), color scheme (7), enquiry (3) and XTVERSION (4). Other queries (DSR, DECRQM, DECRQSS, XTGETTCAP, OSC color reports, kitty keyboard query) answer inside libghostty through `WRITE_PTY` (1) with no hook. `GHOSTTY_TERMINAL_OPT_UNKNOWN_SEQUENCE` (35): at PIN only APC (the tag has no OSC); at UP it reports APC and OSC, and never CSI (so CSI 11 t, 13 t, 15 t, 19 t and 20 t are not reported). | **GAP G1.** No hook gives request bytes or a kind. |
| EV-8c, EV-8g order and backpressure | The route carries the raw `Output` up to and including the query sequence, then the `terminal_query` frame, then later `Output`. When `pending_queries` is full, output after the next unadmitted query is held in order. | `vt_write` runs a whole chunk. `ghostty_terminal_vt_write_until_ground` stops at ground, not at a query. | **GAP G1.** A callback that only reports the query lets the rest of the chunk run, so a later byte can change state before the client answers. Patch 1 therefore has two parts: (a) an effect with `kind` and the raw request bytes; (b) a stop: `ghostty_terminal_vt_write_until_query(terminal, data, len, &consumed)` consumes up to and including the sequence that triggers a query effect, calls the effect once, and returns the consumed count. The worker then splits the chunk at that byte. `ghostty_terminal_vt_write` keeps its current behavior. |
| EV-8d held shadow reply | The reply computed at the query point, held unwritten. | `GHOSTTY_TERMINAL_OPT_WRITE_PTY` (1) fires inside the write at the query point. | COVERED with G1. The binding copies the bytes into a per-query held buffer (bounded by `max_query_reply_bytes`) and writes nothing to the PTY. |
| EV-8 shadow set | Which queries the shadow answers. | See `~/botster-sessions/shared/ghostty-query-inventory-20260930.md`. Host values: `OPT_SIZE`, `OPT_COLOR_SCHEME`, `OPT_DEVICE_ATTRIBUTES`, `OPT_XTVERSION`, `OPT_ENQUIRY`, `OPT_COLOR_FOREGROUND`, `_BACKGROUND`, `_CURSOR`, `_PALETTE`, `OPT_TERMINFO_NAME` (37). `OPT_TITLE_REPORT` (32) stays false. | COVERED. UP adds ANSI DECRQM (`9dc0d974e`) and `CSI 8 t` (`714fe9b90`). The shadow list is re-checked at the final pin (Q1, A2-8). |
| EV-8 typed `Clipboard` reply | `OSC 52 ; <selection> ; <base64> <terminator>` with the request's selection and terminator. | No encoder at PIN or UP. | **GAP G9.** Patch 7. Needs the selection and terminator from G10. |
| EV-8 typed `TextAreaPixels`, `CellPixels`, `WindowPixels` | `CSI 4 ; h ; w t` and `CSI 6 ; h ; w t`. | `size_report.h`: `ghostty_size_report_encode` with `GHOSTTY_SIZE_REPORT_CSI_14_T` and `CSI_16_T`. | COVERED. `WindowPixels` (`CSI 14 ; 2 t`) uses the CSI 4 form, the same as `CSI_14_T`. |
| EV-8 typed `ScreenPixels` (`CSI 5 ; h ; w t`) | | `ghostty_size_report_encode` has styles 2048, 14, 16 and 18 only. | **GAP G9.** Patch 7. |
| EV-8 typed `ScreenChars` (`CSI 9 ; rows ; cols t`) | | none | **GAP G9.** Patch 7. (`CSI_18_T` is the different `CSI 8 ; r ; c t` reply.) |
| EV-8 typed `WindowState` (`CSI 1 t`, `CSI 2 t`) | | none | **GAP G9.** Patch 7. |
| EV-8 typed `WindowPosition` (`CSI 3 ; x ; y t`) | | none | **GAP G9.** Patch 7. |
| EV-8 typed `WindowTitle` (`OSC l <text> ST`), `IconLabel` (`OSC L <text> ST`) | | none (`CSI 21 t` is disabled by default and answers from the stored title) | **GAP G9.** Patch 7. |
| EV-8 typed `ColorScheme` (`CSI ? 997 ; 1 n`, `; 2 n`) | | `color_scheme.h`: `ghostty_color_scheme_report_encode`. | COVERED. |
| EV-8 `Encoded` | The client's own bytes. | none needed | COVERED. The worker only bounds and admits them. |
| OU-12 | `Output` bytes unchanged. | `vt_write` takes a borrowed slice. | COVERED. The worker forwards the PTY bytes and passes the same slice to `vt_write`. |

### Terminal identity and build (A2-8, TI-1)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| TI-1, A2-8 `xterm-ghostty` | `terminal_identity() -> {term: "xterm-ghostty", terminfo_source}`. The entry declares only capabilities that the emulator implements. | `src/terminfo/ghostty.zig` names `xterm-ghostty`, `ghostty`, `Ghostty`. `src/terminfo/Source.zig` renders the text. The text comes from `ghostty +terminfo` (`src/build/GhosttyResources.zig`), which needs the full Ghostty executable and not `libghostty-vt`. The old binding crate has no terminfo code. | **G11.** Patch 5 exports the text from lib-vt (the lead confirmed). The entry is the same at PIN and UP, so `terminfo_source` and the contract tag do not change. P2 adds the `tic` compile test. `GHOSTTY_TERMINAL_OPT_TERMINFO_NAME` is set to `xterm-ghostty`. |

## Pin move

- **Step (a), upstream.** UP `83edd491e3024ae5e50393d62877b8897da1cccd` covers G6 (`7bb45ba34`, `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`) and part of G7 (`e03475c0c`, `GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ`).
- **Step (b), fork patches.** The branch is `botster/vt-core-stage1` of `trybotster/ghostty`, based on UP. One commit per patch, each citing its Core clause. Nothing goes to upstream.

| # | Patch | Clause | Gap |
|---|---|---|---|
| 0 | Own `startHyperlink` uri and id across the capacity-retry loop (root-cause lifetime fix with a Zig test) | ST-6b (1), EV-7 | F9 row 1 |
| 1 | Query effect (kind, raw request bytes) and `ghostty_terminal_vt_write_until_query` | EV-8 | G1 |
| 2 | `source` field on `GhosttyTerminalDesktopNotification` | A2-4, A3-1 | G2 |
| 3 | Wrap-only paste encode (bracketed markers, no sanitizing, no newline rewrite) | IN-8, DP-5 | G3 |
| 4 | Key event: `HYPER` and `META` bits, `shifted_key` and `base_layout_key` setters, `F26` to `F35`; encoder: associated text only with flags 8 and 16, whole-text omission for C0, DEL and C1 | IN-9, 5.1A | G5 |
| 5 | Export the `xterm-ghostty` terminfo source from lib-vt | TI-1, A2-8 | G11 |
| 6 | Mouse: cell-coordinate entry point with no clamp and no output for an unrepresentable cell, a release that obeys the same rule, `GHOSTTY_TERMINAL_DATA_MOUSE_EVENT` and `_MOUSE_FORMAT` getters, (SGR-pixels needs no offset: R-13) | IN-9, 5.1A, ST-4, EV-7 | G8 |
| 7 | Reply encoders: clipboard, `CSI 5 t` and `CSI 9 t` replies, window state, window position, title and icon label text | EV-8 | G9 |
| 8 | Original selection string and terminator on the clipboard write and read payloads | EV-3, EV-8 | G10 |

  Patches 6, 7 and 8 are additions to the lead's list of five. They answer reviewer findings F1, F3, F4 and F8 and need the lead's approval. The lead said no patch for OSC 1 or for mode changes; both stay out (E2 rulings A and C).

- **The nine fork-only SEGV commits at PIN (F9).** The new branch is based on UP and does not contain them. This section has one row per commit. The words below are used in one sense each:
  - **Root-cause fix:** the change removes the defect that causes a crash. It keeps all state.
  - **State-losing workaround:** the change avoids a crash path by dropping a hyperlink, a style or a log line. It loses state that ST-6b and EV-7 require, or it has no effect in the binding. A state-losing workaround is **not carried**, and it is **not promised back** after a crash.
  - **No effect in the configured binding:** the binding installs no log callback and builds the library with `-Doptimize=ReleaseFast`. A change that only affects logging, or only a safe-build assertion, has no effect there. Evidence: `src/terminal/c/sys.zig` `logFn` returns at `if (global.log == null) return;` **before** it formats; the old binding crate at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no `ghostty_sys_set` or log option (`git grep`); `assertIntegrity` runs only when `build_options.slow_runtime_safety` is set.

| # | Commit | Material change | UP logic, or why it has no effect | Class | Carried |
|---|---|---|---|---|---|
| 1 | `3025fa29e` | (a) `startHyperlink` copies `uri` and `id` once for the whole retry loop. (b) The loop is bounded to 12 attempts and a failed `increaseCapacity` (`OutOfSpace`, `OutOfMemory`) drops the hyperlink silently. (c) `assertIntegrity` runs only on the first and last attempt. | (a) UP `startHyperlinkOnce` (`Screen.zig` 2747 to 2771) copies `source` before `endHyperlink` (commit `cfce1cd56`), so the **first** attempt reads valid slices. `endHyperlink` then frees the old cursor hyperlink. When the slices alias it (the API contract of the test "hyperlink accepts its current values"), the **retry** attempts of the loop (2693 to 2742) read freed memory. `cfce1cd56` is already in PIN (`Screen.zig` 2639 to 2648 at PIN); it does not cover the retry lifetime, which is what (a) fixes. UP did not fix it. The production OSC 8 path passes parser-buffer slices and the reload and resize paths pass heap links that they own, so I found no production caller that aliases; I cannot show that this was the production crash. (b) UP returns `OutOfSpace` to the caller, and the callers log and forget the hyperlink (`Screen.zig` 1493, 1579, 2210). The 12-attempt bound adds a silent state loss that UP does not have. (c) the assertion compiles out in `ReleaseFast`. | (a) root-cause fix; (b) state-losing workaround; (c) no effect | **(a) only**, as one commit (patch 0, below) with a Zig test; the test must fail without the fix, and its result is recorded when it runs (not claimed here). (b) and (c) no. |
| 2 | `1041a51ae` | Compile correction of row 1 (Zig 0.16 requires `_ =` on the `*Node` result) and a `catch` form of the loop. | UP already writes `_ = try self.increaseCapacity(...)`. The correction repairs code that UP does not contain. | no effect | no |
| 3 | `80916b69c` | `startHyperlink` makes one insert attempt and drops the hyperlink on `StringsOutOfMemory`, `SetOutOfMemory` and `SetNeedsRehash` instead of growing the page. | UP grows the page (`increaseCapacity`) and retries. The change removes growth, so a hyperlink is lost when a page's string or hyperlink table is full. That breaks EV-7 and ST-6b (1) for that cell. The crash that it avoided was an access fault inside `increaseCapacity`. UP's `33cda4dc5` repairs the access fault that page growth caused in `Terminal.print` (see row 7). | state-losing workaround | no |
| 4 | `f4d6b79c4` | Removes `log.warn` on the degrade path. | The degrade path of row 3 is not carried, so the line does not exist. The log line itself was formatted only when a log callback is installed; the binding installs none. | no effect | no |
| 5 | `ea6550256` | Comments; implicit-id rollback on degrade; removes `log.warn` in `increaseCapacity` hyperlink re-add (replaced by an empty `catch`). | The rollback exists only on the degrade path of row 3. UP's `errdefer` already rolls the implicit id back on an error return (`Screen.zig` 2710). The `catch` change removes one log line (no effect, as row 4). | state-losing workaround, plus no-effect log edits | no |
| 6 | `2a465b03e` | `manualStyleUpdate` makes one `styles.add` attempt and leaves the default style on `OutOfMemory` or `NeedsRehash` instead of calling `increaseCapacity` or `splitForCapacity`. Log lines removed. | UP grows the style table, splits the page if growth is impossible, and logs `err` only on the impossible cases (`Screen.zig` 2499 onward). The change drops a cell's style under table pressure. That loses SGR state that ST-6b (1) requires. The log edits have no effect. | state-losing workaround | no |
| 7 | `6a47dd81e` | `cursorSetHyperlink` returns without growing when the hyperlink map is full; `Terminal.printCell` log removed. The observed site was `print` then `cursorSetHyperlink` then `increaseCapacity`. | UP `33cda4dc5` ("reload cell pointers when print grows a page") repairs that site at its cause: `Terminal.print` held `prev.cell` across `printCell`, whose `cursorSetHyperlink` grows and **replaces the page**, so `appendGrapheme` wrote through a freed pointer. UP records the page identity (node and serial) and reloads the cell, and reads the other three held pointers through the cursor. UP `9313d580c` repairs stale cursor style and hyperlink ids after a scroll clear changes the cursor page (and adds the `assertIntegrity` check for them). The commit does not apply cleanly to UP: it conflicts with `33cda4dc5` in `Terminal.zig`. | state-losing workaround (UP has the root-cause fixes) | no |
| 8 | `5f329cb9a` | Removes `log.warn` for unimplemented CSI actions in `stream.zig`. | No log callback is installed, so `logFn` returns before it formats. No effect. | no effect | no |
| 9 | `5e9ba17a2` | `logFn` returns for every level except `err`. | The binding never calls `ghostty_sys_set(GHOSTTY_SYS_OPT_LOG, ...)`, so `global.log` is null and `logFn` already returns at the first line. The commit text says "Zig formats before the C callback"; that is false at UP when no callback is installed (the null check comes first). No effect. | no effect | no |

  **Decision.** Carry one commit: **patch 0**, "own `startHyperlink` uri and id across the capacity-retry loop". It is the root-cause part of row 1 (a), with a Zig test that restarts the hyperlink from slices of the current one under forced page growth. The commit lands only if that test fails without the fix; the P2 pull request records both runs. The fork stack that the lead pushed (`273aed4b5`, `2c00969f5`) also carries parts (b) and (c) and the compile correction. The new stack replaces both commits with patch 0, so no state-losing code is carried. Nothing else of the nine is carried.

  **What this does not prove.** I cannot reproduce the production crashes (sessions named in the commit messages). The stress program `~/botster-sessions/p2-stress/stress.c` (300,000 iterations each of unique OSC 8 hyperlinks, unique truecolor style pairs, and both together, on a 226 by 70 terminal, built `ReleaseFast`) exits 0 on UP and on the pre-fix base `12967b68f`. It does not show that UP fixed the crash, because it did not crash on the base. The root-cause reading is source evidence: rows 1 (a) and 7. The original replay fixtures are not available to P2.

  **If a crash remains.** The binding keeps a slow-tier stress test of that shape (and the aliased-restart Zig test). If a crash appears on the final pin, the rule is: **no state-losing degradation patch comes back.** The crash needs a root-cause repair in the fork (a patch with its own test), or a lead decision that names the state that may be lost and updates ST-6b and EV-7 accordingly. The pin does not move while a crash is open.

## Build notes (F10)

- **Build command at UP** (from the old `build_data.rs`, flags kept): `zig build -Demit-lib-vt -Doptimize=ReleaseFast -Dsimd=false -Dcpu=baseline -Demit-xcframework=false --prefix <OUT_DIR>/...`. Zig is `0.16.0`.
- **Package dependencies.** The lib-vt build at UP needs seven Zig packages. Zig 0.16 keeps each one as `<global-cache>/p/<hash>.tar.gz` and unpacks it into `<project>/zig-pkg/<hash>` at build time:
  - `aro-0.0.0-JSD1Qk6lNgDdcDV4Vh7Sfy-34m2TluIVOdPzMmj_0BjX`
  - `N-V-__8AAB0eQwD-0MdOEBmz7intriBReIsIDNlukNVoNu6o`
  - `N-V-__8AADYiAAB_80AWnH1AxXC0tql9thT-R-DYO1gBqTLc`
  - `N-V-__8AAM94BAAFk_hn4UW0x_OBD2g0vOwexeAAyWNNo4eB`
  - `N-V-__8AAP5JWgCGP_AD0teWpa4krRvE9VPZzvviGdbmN4jI`
  - `translate_c-0.0.0-Q_BUWhVNBwDOEcIqub4VFPJPB6D9dgwzUMHTX5KWr8Xr` (URL `https://codeberg.org/vancluever/translate-c/archive/4e879eb8aba615de112eabd1231ea6e01920cead.tar.gz`; `pkg/translate-c/build.zig.zon`)
  - `uucode-0.2.0-ZZjBPuuFVgC8YZ8eld4fOKsZANLIhTFMzULQxhkLi1C7` (URL `https://github.com/jacobsandlund/uucode/archive/9d55524551411b493cca41ca06363625d90aff1e.tar.gz`; `build.zig.zon`)
  The package hash in each `build.zig.zon` is the content hash that Zig checks. The exact `translate_c` hash stays as recorded above. The final list is re-read from the final pin.
- **Two directories, two roles.** Zig writes compilation artifacts into its global cache (`h/`, `o/`, `z/`, `tmp/` next to `p/`), so a global cache is never read-only. The design separates them:
  - **Package store (immutable input):** `~/.cache/botster/zig-packages/p/<hash>.tar.gz`, one file per hash listed above. Nothing in the build or the gate writes it. Only the prefetch command writes it.
  - **Build cache (writable artifacts):** `<OUT_DIR>/zig-global`, used as `--global-cache-dir`, with `--cache-dir <OUT_DIR>/zig-local`. `build.rs` copies each required `<hash>.tar.gz` from the package store into `<OUT_DIR>/zig-global/p/` before it runs Zig. Zig then finds every package without a fetch.
- **Prefetch step** (a separate command, `cargo xtask prefetch-zig`, run once with network; never run by `build.rs` and never by the gate): it runs the lib-vt build command above in the Ghostty checkout with a scratch `--global-cache-dir`, and then copies the seven `p/<hash>.tar.gz` files into the package store. It checks each file's name against the recorded hash list.
- **Missing prerequisite:** `build.rs` checks the package store for each recorded hash before it runs Zig. A missing file stops the build with the package name, the hash, the store path and the prefetch command. `build.rs` runs no fetch.
- **How the gate denies network (not only the prerequisite check).** `build.rs` runs `zig build` under an OS network denial, in every build and not only in the gate, so there is no gate-only branch (plan 2.3c): on macOS, `sandbox-exec -p '(version 1)(allow default)(deny network*)'`; on Linux, `unshare --net` (or `unshare -rn` where needed). If the wrapper is not available, `build.rs` fails with a message and does not run Zig unwrapped. So a missing package cannot be fetched even by mistake; Zig fails with `unable to connect to server`.
- **Evidence (2026-10-01, Zig 0.16.0, macOS).** I tested the denial wrapper and the prefetched cache together, with one shared global cache directory (`~/.cache/botster/zig-global`) instead of the copy design above. I have not yet tested the copy step; the binding's offline build test covers it. A fresh `git archive` export of the fork stack (no `zig-pkg`), built under the `sandbox-exec` denial of the previous item:
  - with an **empty** global cache the build fails: `unable to connect to server: UnknownHostName` for `uucode` (`build.zig.zon:45`) and `translate_c` (`pkg/translate-c/build.zig.zon:15`), so a missing prerequisite does not build and does not silently succeed;
  - with the **prefetched** cache the build succeeds in about 45 seconds and installs `libghostty-vt.a`.
  A cold build **with** the fetch took 74 seconds on the same machine (`botsterq run --exclusive`, 4 jobs), under the plan's 2-minute limit, so the shared `libghostty-vt.a` cache of plan section 8 is not needed for now. The binding re-measures it.
- The earlier note of revision 2 (one fetch attempt failed with `HttpConnectionClosing`) is the reason for the prefetch: the network is not reliable.

## Prior art (BUILD.md rule 0)

- **Sources read:** the vault (`~/knowledge/notes/`, for the Zig cache gotcha and the OSC and callback gotchas); the pinned plan, section 7.1; the old crate `crates/botster-terminal-ghostty` at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`, read with `git show`; the query inventory of 2026-09-30; the Ghostty fork and upstream, read with `git show` and in a checkout of UP; the libghostty examples referenced from the headers.
- **Proposed reuse (steals, each its own commit with trailers, after the pin is settled):** `sys.rs`, `lib.rs`, `input.rs`, `build.rs`, `build_support.rs`, `build_data.rs`, and the `vendor/ghostty` entry of `.gitmodules`. Each stays only if it passes its clause tests with trivial edits. The native library becomes unconditional.
- **Rejected:** `src/client.rs` (a client projection) and the old `TerminalMetadataProducer` scanner (a parser outside libghostty, BUILD.md rule 1). `keys.rs` of the old protocol crate is read for coverage only.
- **Custom pieces and reasons:**
  - The 5.1A key table is written fresh: the contract names differ from the old W3C names.
  - The `model_rev` counter, the held-reply buffers, the title and notification buffers: no library holds Core's state.
  - The fork patches 1 to 8: each fills a gap above, and each is the only way to keep rule 1.
  - The stress test: it covers the nine commits that the fork branch does not carry.
- **Ecosystem:** `libghostty-vt` has no maintained Rust binding with these hooks. The `libghostty-rs` crates were not evaluated at the pinned revision; the lead may name one if it should be considered.

## Rulings used

- **E2 ruling A:** OSC 1 gives no `TitleChanged`. The bytes stay in `Output`.
- **E2 ruling B:** `other_modes` is the set of modes that libghostty tracks and that have no normative `ModeFlags` field. An unrecognized mode number changes no field. There is no second parser.
- **E2 ruling C:** after each `vt_write`, compare the final `ModeFlags` with the last posted `ModesChanged` value. Post one event with the final flags if they differ. Post nothing if they are equal. No mode-change patch.
- **R-13:** SGR-pixels reports zero-based pixels with no +1; the 5.1A `+1` is for cells only.
- **Lead decision:** paste is byte-exact (DP-5, IN-8).

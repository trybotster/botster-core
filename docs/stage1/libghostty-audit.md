# libghostty audit for Core Stage 1 (package P2)

Scope: every Core clause that needs terminal semantics, checked against libghostty.
Contract: manifest final13 (botster-contracts `2f2996ef0f016a1fefc6879e74deaef033383b66`).
Plan pin: `stage1-plan.a24efe7e`, sections 0, 6.1, 6.3, 7.1, 8, 9 (Q1) and 10 (R1).

## Method and revisions

- **Pinned fork:** `trybotster/ghostty` at `eb72ec61304ea256be1d86ed8fa961c84e43ecbd` ("PIN").
- **Upstream main:** `ghostty-org/ghostty` at `83edd491e3024ae5e50393d62877b8897da1cccd` ("UP"), fetched 2026-10-01. Read only with `git show`.
- Headers are under `include/ghostty/vt/`. Zig sources are under `src/terminal/`. All reads are `git show <sha>:<path>`.
- A row says **COVERED** when a libghostty symbol serves the clause. It says **GAP** when no symbol serves it without terminal parsing outside libghostty (BUILD.md rule 1). It says **NOTE** when libghostty serves the clause and the binding must follow a stated rule.
- "UP" in a row means that upstream main has the symbol and the pin does not.

## Summary

| ID | Gap | PIN | UP | Needs |
|---|---|---|---|---|
| G1 | Query detection: exact request bytes and kind (EV-8) | none | none | fork patch |
| G2 | Notification source, OSC 9 or OSC 777 (A2-4, A3-1, EV-7) | none | none | fork patch |
| G3 | Paste without sanitizing (IN-8) | `ghostty_paste_encode` strips and rewrites | same | fork patch, or a steward ruling |
| G4 | OSC 1 icon label, contract says OSC 0/1/2 give `TitleChanged` (EV-7) | ignored | ignored | steward ruling, else fork patch |
| G5 | Key encoder inputs: `hyper` and `meta` modifier bits, `shifted_key`, `base_layout_key`, `f26` to `f35` (5.1A) | missing | missing | fork patch |
| G6 | OSC 133 prompt marks with exit code (A2-4) | state only, no callback | **covered** (`GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`) | move pin |
| G7 | Clipboard read request (EV-8, EV-3) | ignored | **covered** (`GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ`) | move pin |

Other findings:

- **No callback for a mode change.** The plan lists `GHOSTTY_TERMINAL_OPT_MODE` (34) as a callback. It is a setter. The binding observes modes by reading `GHOSTTY_TERMINAL_DATA_MODE` after each write. See rows EV-7 and IN-9.
- **The pin carries nine fork-only commits** that fix Botster SEGV crashes (list in section "Pin move"). A pin that is plain upstream drops them.
- **The `xterm-ghostty` terminfo entry is identical** at PIN and UP (`git diff PIN UP -- src/terminfo` is empty).
- **Zig is `0.16.0` at both** (`build.zig.zon` `minimum_zig_version`).

## Audit rows

### Input (IN-8, IN-9, 5.1A, DP-12)

| Clause | Need | libghostty API (PIN unless stated) | Result |
|---|---|---|---|
| IN-8 | Paste wrapped in `ESC[200~ … ESC[201~` when bracketed mode is on, bare when off. Core does no payload inspection. | `paste.h`: `ghostty_paste_encode(data, len, bracketed, buf, …)`. Mode read: `ghostty_terminal_get(…, GHOSTTY_TERMINAL_DATA_MODE)` with `GHOSTTY_MODE_BRACKETED_PASTE` (2004). | **GAP G3.** `ghostty_paste_encode` replaces unsafe bytes (NUL, ESC, DEL and others) with spaces. When bracketed is false it also turns newlines into CR. IN-8 requires a payload that Core does not inspect. UP has the same function (`ghostty_terminal_paste` is a clipboard-driven path). The binding must not write the `ESC[200~` wrapper by hand (rule 1). |
| IN-8 | Mode read at the start of the write transaction. | `DATA_MODE`, mode 2004. | COVERED. The binding reads one mode value at admission. |
| IN-9 `Key` | Encode a 5.1A key with the authoritative modes. | `key/encoder.h`: `ghostty_key_encoder_new`, `_setopt_from_terminal` (sets cursor-key application, keypad application, kitty flags and the other options from the terminal), `_encode`. `key/event.h`: `_set_action`, `_set_key`, `_set_mods`, `_set_consumed_mods`, `_set_utf8`, `_set_unshifted_codepoint`. | COVERED for legacy keys, application cursor and keypad, and kitty flags 1, 2, 8 and 16. See G5 for the rest. |
| IN-9 `Key` | `shifted_key` and `base_layout_key` (kitty flag 4, alternate keys). | `key/event.h` has `unshifted_codepoint` and `utf8` only. | **GAP G5.** No setter for the shifted key or the base-layout key. |
| IN-9 `Key` | `mods` set: `shift alt ctrl meta super hyper caps_lock num_lock`. | `key.h`: `GHOSTTY_MODS_SHIFT`, `CTRL`, `ALT`, `SUPER`, `CAPS_LOCK`, `NUM_LOCK` plus side bits. | **GAP G5.** No `HYPER` and no `META` bit (kitty bits 16 and 32). |
| IN-9 `Key` | `NamedKey` set: `f1` to `f35`, keypad keys, modifier keys, `print_screen`, `pause`, `menu`. | `key.h` `GhosttyKey`: `GHOSTTY_KEY_F1` to `F25`, `GHOSTTY_KEY_NUMPAD_*`, `PRINT_SCREEN`, `PAUSE`, `CONTEXT_MENU`, `CAPS_LOCK`, `NUM_LOCK`, `SCROLL_LOCK`, left and right modifier keys. | **GAP G5** for `f26` to `f35` only. The contract says a client that sends `f1` to `f24` stays in the set, so the effect is small. |
| IN-9 `Key` | Kitty flag mask `0x1F`, flag 16 only with flag 8, no control codepoint in associated text. | Encoder option `GHOSTTY_KEY_ENCODER_OPT_KITTY_FLAGS`. | NOTE. The binding masks the flags to `0x1F` before `_setopt`. It drops a `text` with a control codepoint before `_set_utf8`. Both are input validation, not encoding. Whether libghostty also ignores flag 16 without flag 8 is checked by the test `conf::in_9_kitty_flag_16_needs_flag_8`. |
| IN-9 `Key` | Zero output is typed (`NotReported`, `Unsupported{what}`). | `ghostty_key_encoder_encode` returns `out_len == 0`. | NOTE. The binding maps `out_len == 0` to the typed result with the rule of 5.1A (legacy release, no text). It does not invent bytes. |
| IN-9 `Mouse` | Encode press, release, move and wheel for X10, UTF-8, SGR, URXVT and SGR-pixels, for modes 9, 1000, 1002 and 1003. | `mouse/encoder.h`: `ghostty_mouse_encoder_new`, `_setopt_from_terminal` (tracking and format from terminal state), `_setopt` with `GHOSTTY_MOUSE_ENCODER_OPT_SIZE` (needs cell and screen pixel size), `_encode`. `mouse/event.h` has the event. | COVERED. The binding supplies the size from the terminal's cell size. |
| IN-9 `Mouse` | A coordinate that the encoding cannot express is not reported (typed zero), never truncated. | `_encode` output length. | NOTE. Checked by test. If libghostty clamps, that is a GAP found by the test, and it goes to the lead. |
| IN-9 `Mouse` | Without `x` and `y` under SGR-pixels the result is typed zero. | none needed | NOTE. The binding refuses before it calls the encoder. |
| IN-9 `Focus`, DP-12 | Encode `ESC [ I` and `ESC [ O` only when focus reporting (1004) is on. | `focus.h`: `ghostty_focus_encode(GHOSTTY_FOCUS_GAINED \| GHOSTTY_FOCUS_LOST, …)`. Mode: `DATA_MODE`, `GHOSTTY_MODE_FOCUS_EVENT`. | COVERED. |
| 5.1A `ModeFlags.alt_screen` | read | `DATA_ACTIVE_SCREEN` | COVERED |
| 5.1A `cursor_visible` | read | `DATA_CURSOR_VISIBLE`, or `DATA_MODE` 25 | COVERED |
| 5.1A `bracketed_paste` (2004) | read | `DATA_MODE` | COVERED |
| 5.1A `application_cursor_keys` (1) | read | `DATA_MODE`, `GHOSTTY_MODE_DECCKM` | COVERED |
| 5.1A `application_keypad` (DECKPAM) | read | `DATA_MODE`, `GHOSTTY_MODE_KEYPAD_KEYS` (66) | COVERED |
| 5.1A `focus_reporting` (1004) | read | `DATA_MODE`, `GHOSTTY_MODE_FOCUS_EVENT` | COVERED |
| 5.1A `kitty_flags` | read the top of the stack | `DATA_KITTY_KEYBOARD_FLAGS` | COVERED |
| 5.1A `mouse_tracking` | read | `DATA_MOUSE_TRACKING`, or `DATA_MODE` 9, 1000, 1002, 1003 | COVERED |
| 5.1A `mouse_encoding` | read | `DATA_MODE` 1005, 1006, 1015, 1016 (`GHOSTTY_MODE_UTF8_MOUSE`, `SGR_MOUSE`, `URXVT_MOUSE`, `SGR_PIXELS_MOUSE`) | COVERED |
| `other_modes` | every mode that is not a normative field | `modes.h` lists the modes libghostty tracks (`GHOSTTY_MODE_*`). The binding reads each one with `DATA_MODE`. | NOTE. `other_modes` is the set of tracked modes outside the normative fields. A mode that libghostty does not recognize has no state and cannot be observed (`GHOSTTY_MODE_REPORT_NOT_RECOGNIZED`). The contract sentence "an unknown mode is carried in `other_modes`" holds only for tracked modes. This is a contract-meaning point for the lead to put to the steward. |

### State and snapshot (ST-1 to ST-3, ST-6, ST-6b)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| ST-1 `model_rev` inputs | A token that changes on output, resize, mode change, title change and cwd change. | No revision counter in the C API. | NOTE. The binding owns one counter, advanced after every `vt_write` of a nonzero length, `resize`, and every observed mode, title or cwd change. This is bookkeeping and holds no terminal semantics. It never says "unchanged" when something changed. |
| ST-2 screen text, `history_unavailable` | Plain text with scrollback. | `formatter.h`: `ghostty_formatter_terminal_new`, `_format_buf`, `_format_alloc`. `DATA_SCROLLBACK_ROWS`, `DATA_TOTAL_ROWS`. | COVERED. |
| ST-3 `row`, `col`, `visible` | Zero-based cell coordinates. | `DATA_CURSOR_Y`, `DATA_CURSOR_X`, `DATA_CURSOR_VISIBLE`, `DATA_CURSOR_PENDING_WRAP`. | COVERED. |
| ST-3 `row_text`, `text_before_cursor` | Row text with trailing empty cells and spaces removed. Text of cells `[0, col)` not trimmed. | `grid_ref.h`: `ghostty_grid_ref_cell`, `_graphemes` (read each cell); or `formatter.h` on one row. | COVERED with NOTE. The trim rule of ST-3 is applied to cell data that libghostty returns. It is string handling, not terminal parsing. A wide character takes two cells (the spacer cell has no grapheme). |
| ST-6 `GHOSTSNP` | Capture the terminal as a snapshot. | `snapshot.h`: `ghostty_snapshot_encode`, `_encode_buf`, `_encode_alloc`. Magic `GHOSTSNP` plus a `u16` version. Records are CRC32C protected. | COVERED. The decoder (`ghostty_snapshot_decoder_*`) refuses a bad version with `GHOSTTY_INVALID_VALUE`. |
| ST-6 pages | `read_page` pages of the snapshot bytes. | Snapshot is a byte stream. | NOTE. Paging is a Core split of the encoded bytes. |
| ST-6b restores (1) cells, attributes, OSC 8 hyperlinks | | `src/terminal/snapshot/` `screen.zig`, `page.zig`, `hyperlink.zig`, `style.zig` | COVERED. |
| ST-6b (2) palette and dynamic colors | | TERMINAL record: original palette, override mask, overrides | COVERED. |
| ST-6b (3) cursor position, visibility, shape and blink | | TERMINAL record: cursor default style and blink policy; SCREEN record: cursor | COVERED. |
| ST-6b (4) every mode field, kitty flags, mouse tracking and encoding | | TERMINAL record: mouse event and format, current, saved and default modes; SCREEN record: kitty keyboard stack | COVERED. |
| ST-6b (5) size, alternate screen | | TERMINAL record: size, active screen key, screen count | COVERED. |
| ST-6b (6) title and cwd | | TERMINAL record: PWD and title | COVERED. |
| ST-6b graphics | Only when `snapshot_graphics` is advertised. | `snapshot.zig` and `screen.zig` reference kitty graphics. `kitty_graphics.h`. | COVERED. The binding advertises `snapshot_graphics` only if its own test shows a placement survives a round trip. |
| ST-6b resume invariant at every byte offset | Saved cursor, tab stops, margins, rendition, charsets, parser pending state. | Snapshot has tab stops, scroll margins, saved cursor, charset state, previous codepoint, and a CONTINUATION record. `GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES` (31) enables tracking. `ghostty_terminal_continuation_write`, `_buf`, `_alloc` export the pending bytes. `GHOSTTY_TERMINAL_DATA_VT_GROUND` (38) and `ghostty_terminal_vt_write_until_ground` find a ground state. | COVERED. Rules for the binding: (1) set `CONTINUATION_MAX_BYTES` to a nonzero value before the first write; the doc says that lowering the limit or enabling tracking mid-sequence makes the continuation unavailable until the next ground state; (2) the limit must exceed the longest sequence that matters, or the snapshot cut falls to a ground state; (3) the corpus test `conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences` is the proof. The invariant is libghostty's (upstream tests "continuation every-byte cuts preserve future behavior"); P2 supplies the oracle. |

### Events (EV-1, EV-3, EV-7, A2-4, A3-1)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| EV-1 | Title, cwd, bell, notification, prompt mark reach the host in observation order. | Callbacks run inside `vt_write` in stream order. | COVERED. The binding appends each observation to a bounded buffer inside the callback (no block, no lock, no I/O). Order is the callback order. |
| EV-7 title | OSC 0 and OSC 2 give `TitleChanged`. | `GHOSTTY_TERMINAL_OPT_TITLE_CHANGED` (5); read `DATA_TITLE` (12). Title is truncated at 1024 bytes in `stream_terminal.zig` `windowTitle`. | COVERED. NOTE: libghostty cuts a title at 1024 bytes, and it may cut inside a UTF-8 character. The binding reads the title from `DATA_TITLE` and rejects a split character. A title is a state value read after the callback. |
| EV-7 title, OSC 1 | The contract says "OSC 0/1/2 → `TitleChanged`". | `stream_terminal.zig` `change_window_icon` is logged and ignored ("OSC 1 (change icon) received and ignored"). | **GAP G4.** OSC 1 sets an icon label, not the title. The query inventory already treats the icon label as an external kind (`IconLabel`). Ask the steward whether OSC 1 must give `TitleChanged`. If yes, a fork patch is needed. P2 recommends a ruling that OSC 1 is not a title. |
| EV-7 cwd | OSC 7 gives `CwdChanged`. | `GHOSTTY_TERMINAL_OPT_PWD_CHANGED` (25); read `DATA_PWD` (13). The doc says a zero-length value means the pwd was cleared or is not a valid OSC 7. | COVERED. |
| EV-7, A2-4 bell | BEL gives `Bell`. | `GHOSTTY_TERMINAL_OPT_BELL` (2). | COVERED. |
| EV-7, A2-4 prompt marks | OSC 133 A, B, C, D give `PromptMark{mark, exit_code?}`. | PIN: `stream_terminal.zig` calls `terminal.semanticPrompt(value)`, which only updates state. `DATA_CURSOR_AT_PROMPT` (39) is a boolean. There is no callback and no exit code. UP: `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT` (42), `GhosttyTerminalSemanticPromptFn`, `GhosttySemanticPrompt{kind, prompt_kind, has_exit_code, exit_code, …}`. Kinds: `PROMPT_START` (A), `INPUT_START` (B), `OUTPUT_START` (C), `COMMAND_END` (D). Added in UP commit `7bb45ba34`. | **GAP G6 at PIN, covered at UP.** Move the pin to a commit that has `7bb45ba34`. |
| EV-7, A2-4 notification | OSC 9 and OSC 777 `notify` give `Notification{source, title?, body}`. Other OSC forms stay in `Output`. | `GHOSTTY_TERMINAL_OPT_DESKTOP_NOTIFICATION` (29), `GhosttyTerminalDesktopNotification{size, title, body}`. Same at UP. | **GAP G2.** The struct has no source field. OSC 9 always has an empty title, but an OSC 777 `notify` with an empty title looks the same. The binding cannot tell the two apart without reading the raw bytes (rule 1). Patch: add `source` (OSC 9, OSC 777) to the struct. |
| A2-4 truncation | `title` and `body` bounded at `clipboard_bytes`, cut at a character boundary. | Callback strings are borrowed. | NOTE. Binding copies and cuts at a character boundary (string handling). |
| EV-3, EV-7 clipboard write | OSC 52 write gives `ClipboardWrite{selection, bytes, total_bytes}`. | `GHOSTTY_TERMINAL_OPT_CLIPBOARD_WRITE` (26), `GhosttyClipboardWrite` (destination, representations; protocol details normalized). The callback returns a `GhosttyClipboardWriteResult`. | COVERED. NOTE: the callback must return a result at once. The binding returns success and records the write; the host decides policy later. An oversize write is surfaced with `bytes: none` by the binding after it counts the bytes. |
| EV-3, EV-8 clipboard read | A read is a terminal query for EV-8 (`ClipboardRead`). | PIN: "OSC 52 clipboard read requests ('?') are always ignored and never forwarded". UP: `GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ` (38), `GhosttyTerminalClipboardReadFn(terminal, userdata, read)`. The reply is given before the callback returns, through `read->reply`. | **GAP G7 at PIN, covered at UP.** NOTE for P4b: the UP callback is synchronous. The binding can only report the request and answer `DENIED` or `UNSUPPORTED` in the callback. A later client reply as `Encoded{bytes}` is written by the worker as ordinary input. P4b owns that design; the lead should know. |
| EV-7 OSC 8 hyperlink | Carried in `Output` and in every snapshot. | Snapshot `hyperlink.zig`. | COVERED. |
| EV-7 every mode change | `ModesChanged` with the full `ModeFlags`, including `other_modes`. | No callback at PIN or UP (the `GHOSTTY_TERMINAL_OPT_RENDER_HOLD` callback at UP covers only mode 2026). | NOTE. The binding reads all modes after each `vt_write` and compares them with the previous `ModeFlags`. Two changes of one mode inside one write collapse into the final value. ModesChanged carries state, and EV-1 says superseded state values collapse. The test `conf::ev_7_every_mode_change_posts_modes_changed` feeds each sequence in a separate write. This is not a parser. If the steward requires every transition inside one chunk, a fork patch (a mode-change effect) is needed. |
| A3-1 | Classify a notification. | Same as the notification row. | Same as G2. |
| EV-7 progress (OSC 9;4) | Not a Core event. | `GHOSTTY_TERMINAL_OPT_PROGRESS_REPORT` (30). | Not used. The sequence stays in `Output`. |

### Queries and replies (EV-8, OU-12)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| EV-8 detect a query | The exact request bytes, the kind, at most `max_query_bytes`. | Effects exist per query: `GHOSTTY_TERMINAL_OPT_DEVICE_ATTRIBUTES` (8), `SIZE` (6), `COLOR_SCHEME` (7), `ENQUIRY` (3), `XTVERSION` (4). Other queries (DSR, DECRQM, DECRQSS, XTGETTCAP, OSC color reports, kitty keyboard query) answer inside libghostty through `WRITE_PTY` (1) with no hook. The unknown-sequence callback (`OPT_UNKNOWN_SEQUENCE`, 35) covers unrecognized OSC and APC only, not CSI (for example CSI 11 t, 13 t, 15 t, 19 t, 20 t). No callback gives request bytes or a kind. | **GAP G1.** Fork patch: one effect, called once per query, with `kind` (enum of the EV-8 shadow set plus `unknown_csi`) and the raw request bytes. A workaround that splits the stream at ground states (`vt_write_until_ground`) finds the bytes but not the kind, and costs one FFI call per byte. P2 does not recommend it. |
| EV-8 held shadow reply | The reply that the model computes at the query point, held unwritten. | `GHOSTTY_TERMINAL_OPT_WRITE_PTY` (1): `GhosttyTerminalWritePtyFn(terminal, userdata, data, len)`. The callback fires inside `vt_write` at the query point. | COVERED with G1. The binding copies the bytes into a per-query held buffer (bounded by `max_query_reply_bytes`). It writes nothing to the PTY. Without G1 the binding cannot match a reply to a request. |
| EV-8 shadow set | Which queries the shadow answers (DA, DSR 5 and 6, ENQ, DECRQM, kitty keyboard query, XTVERSION, OSC colors, DECRQSS, XTGETTCAP, XTWINOPS 14, 16 and 18 with a size, mode 2048). | See `~/botster-sessions/shared/ghostty-query-inventory-20260930.md` (pin, `stream_terminal.zig`). `GHOSTTY_TERMINAL_OPT_SIZE`, `COLOR_SCHEME`, `DEVICE_ATTRIBUTES`, `XTVERSION`, `ENQUIRY` supply the host values. `OPT_COLOR_FOREGROUND`, `BACKGROUND`, `CURSOR`, `PALETTE` supply the color profile. `GHOSTTY_TERMINAL_OPT_TERMINFO_NAME` (37) sets the XTGETTCAP `TN` answer. `GHOSTTY_TERMINAL_OPT_TITLE_REPORT` (32) stays false. | COVERED. UP adds ANSI DECRQM (`9dc0d974e`) and `CSI 8 t` (`714fe9b90`); the shadow list is re-checked at the new pin (A2-8 and Q1 ask for this). |
| EV-8 `Encoded` and typed replies | The worker writes client replies. | A typed reply table is in the contract. | NOTE. Typed reply bytes come from libghostty where an encoder exists (`ghostty_size_report_encode`, `ghostty_mode_report_encode`, `ghostty_focus_encode`). A reply without an encoder is a P4b design question. |
| OU-12 | `Output` bytes unchanged. | `vt_write` takes a borrowed slice. | COVERED. The worker forwards the PTY bytes and passes the same slice to `vt_write`. |

### Terminal identity and build (A2-8, TI-1, OU-12, DP-12)

| Clause | Need | libghostty API | Result |
|---|---|---|---|
| TI-1, A2-8 `xterm-ghostty` terminfo source | `terminal_identity() -> {term: "xterm-ghostty", terminfo_source}` and the invariant that the entry declares only implemented capabilities. | `src/terminfo/ghostty.zig` names `xterm-ghostty`, `ghostty`, `Ghostty`. `src/terminfo/Source.zig` renders the source text. No C symbol exports it at PIN. | **NOTE: no C symbol and no lib-vt build step exports the source text.** The text comes from `ghostty +terminfo` (`src/build/GhosttyResources.zig`), which needs the full Ghostty executable, not `libghostty-vt`. The old binding crate has no terminfo code. Options for P2, in order of preference: (1) a small fork patch that exports `Source.encode` of `ghostty.zig` from lib-vt (patch 5 below); (2) a checked-in `xterm-ghostty.terminfo` file plus a build-time check against the pin's entry. P2 proposes option 1 and asks the lead to confirm it. The entry is the same at PIN and UP, so `terminfo_source` and the contract tag do not change on a move to UP. The invariant holds by the emulator's own entry; P2 adds a `tic` compile test. `GHOSTTY_TERMINAL_OPT_TERMINFO_NAME` is set to `xterm-ghostty`. |
| DP-12 | Synthetic focus-out when the last focused route closes. | `ghostty_focus_encode`, `DATA_MODE` 1004. | COVERED (see the IN-9 `Focus` row). |
| OU-12 | Byte transparency. | See the OU-12 row above. | COVERED. |

## Pin move (Q1 order)

- **Step (a), upstream.** UP `83edd491e3024ae5e50393d62877b8897da1cccd` covers G6 (`7bb45ba34`, symbol `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`) and G7 (`e03475c0c`, symbol `GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ`). UP does not cover G1 to G5.
- **Step (b), fork patches.** G1 to G5 need a patch series on a new branch of `trybotster/ghostty` (branch name proposed: `botster/vt-core-stage1`). It starts from UP.
- **The nine fork-only SEGV commits.** They exist at PIN and not at UP: `3025fa29e`, `1041a51ae`, `80916b69c`, `f4d6b79c4`, `ea6550256`, `2a465b03e`, `6a47dd81e`, `5f329cb9a`, `5e9ba17a2`. The branch must carry them (rebased), or P2 must show that UP fixed the same crashes. P2 has not yet checked whether UP fixed them.
- **Proposed patches, one commit each, rebasable on UP:**
  1. `query` effect: kind and raw request bytes for every query that libghostty answers or ignores (G1).
  2. `source` field on `GhosttyTerminalDesktopNotification` (G2).
  3. Raw paste encode with no sanitizing: wrap only, in `paste.h` (G3). Not needed if the steward rules that sanitizing is allowed.
  4. `HYPER` and `META` modifier bits, `shifted_key` and `base_layout_key` setters on the key event, and `F26` to `F35` (G5).
  5. Export the `xterm-ghostty` terminfo source text from lib-vt (TI-1; see the TI-1 row).
  6. (Only if the steward requires it) OSC 1 as a title change (G4), or a mode-change effect (see EV-7).
  Plus the rebased SEGV commits first.
- **Terminfo.** No change at UP. A2-8: `terminfo_source` and the contract tag stay.
- **Zig.** `0.16.0`, unchanged.

## Questions for the lead (contract meaning, to the steward)

1. **IN-8 paste:** is sanitizing by libghostty (unsafe bytes to spaces, and bare newline to CR when not bracketed) allowed, or must the payload stay byte-exact? The contract says "Core does not inspect the payload for safety". P2 reads that as byte-exact, which needs patch 3.
2. **EV-7 OSC 1:** must OSC 1 (icon label) give `TitleChanged`?
3. **EV-7 `other_modes`:** "an unknown mode is carried in `other_modes`". libghostty keeps no state for a mode it does not recognize. Does `other_modes` mean the tracked modes outside the normative fields?
4. **EV-7 mode changes:** does `ModesChanged` need every transition inside one write, or does the final state per write satisfy EV-1 (state values collapse)?

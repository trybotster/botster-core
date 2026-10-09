# libghostty audit for Core Stage 1 (package P2)

## Public API follow-up for P6

The binding exposes the following existing C exports at fork `3f8eb6810bb673aa782b047de21783ac81fb1121`.
These APIs add no terminal parser, terminal encoder, or test branch.

| Clause | Public API | Existing C export | Verification |
|---|---|---|---|
| ST-6, ST-6b | `Terminal::from_snapshot` | `ghostty_snapshot_decoder_new_buf`, `_set`, `_decode`, `_free` | The existing every-cut resume tests use the public decoder. A rejected envelope version returns `SnapshotDecodeError::UnsupportedVersion`. |
| ST-6b graphics | `Terminal::from_snapshot` | Decoder option `KITTY_IMAGE_STORAGE_LIMIT` from patch 13 | The decoder sets the limit to zero before restore. Existing image-resume tests check both screens. |
| EV-7, ST-6b hyperlinks | `Terminal::hyperlink_uri` | `ghostty_grid_ref_hyperlink_uri` | Tests compare cell URIs before and after restore. |
| ST-6b item 1 | `Terminal::cell_attributes` | `ghostty_grid_ref_cell`, `ghostty_grid_ref_style`, `ghostty_cell_get` | Restore tests compare styled, wide, protected, and background cells. History reads use the library's screen coordinates. |
| ST-6b item 2 | `Terminal::colors` | `ghostty_terminal_get` color keys 18 through 25 | Restore tests compare current and default palettes and dynamic colors. A palette change leaves cursor and cell reads unchanged. |
| ST-6b item 3 | `Terminal::cursor_appearance` | `ghostty_render_state_new`, `_update`, `_get`, `_free` | Restore tests compare cursor shape and blink. The read consumes render dirty state but leaves snapshot bytes unchanged. |
| ST-6b item 4 | `Terminal::cell_hyperlink_uri` | `ghostty_grid_ref_hyperlink_uri` | Tests compare history URIs with their visible source and restored history. |
| ST-6b graphics | `Terminal::image_storage_limit`, `has_image` | Terminal data `KITTY_IMAGE_STORAGE_LIMIT`, `KITTY_GRAPHICS`; `ghostty_kitty_graphics_image` | Tests compare native storage at a nonzero limit with a restored terminal at zero. Each lookup uses an explicit image ID. |
| ST-6b, A8-2 pending state | `Terminal::continuation`, `set_continuation_max_bytes` | `ghostty_terminal_continuation_buf`, terminal option `CONTINUATION_MAX_BYTES` | Tests check retained input, ground state, disabled retention, and the configured limit. |
| ST-6b failure observation | `Terminal::vt_processing_error` | Terminal data key `VT_PROCESSING_ERROR` | Tests check the key against the pinned header. An injected C allocator failure sets the native flag and public read together. |

The C continuation API reports unavailable input without its length or parser kind.
It does not distinguish configured overflow from lost retention.
The testkit must use a separate oracle with a sufficient retention bound to measure pending input independently.
The binding does not infer parser state from the input bytes.

The fork does not export `Tracker.broken`. `VT_PROCESSING_ERROR` reads a separate semantic failure flag.
The lead accepted this classification for `oracle_resume_every_cut` under its no-injection condition:

- Unavailable retention within the independently measured continuation limit is a mismatch.
- Unavailable retention above that limit is expected and permits classification as `refused_beyond_limit`.
- An unknown pending size makes the cut inconclusive.
- A semantic failure is a mismatch with its own reason.

Prior art: the decoder reuses this crate's private decoder path from `tests_snapshot.rs`.
The binding copies the hyperlink URI and continuation bytes from libghostty.
No old botster-core code was reused.

Mutation follow-up: direct C reads check cell fields, cell backgrounds, and dynamic colors independently of restore equality.
The allocator fixture uses the existing `GhosttyAllocator` interface only in tests.
The pinned `src/lib/allocator.zig` passes log2 alignment to each callback.
The fixture follows that implementation, although the C header describes alignment in byte units.
The `Render::drop` exclusion covers one cleanup call that only frees a native handle.
Removing that call leaks memory but changes no public read.

The pinned fork exposes no complete image count.
The revised lead ruling requires the graphics control to read the limit on both actual terminal instances.
The model constructor and snapshot decoder set the storage limit to zero before any input.
Libghostty enforces that limit, so neither instance can store images while the limit remains zero.
Tests write explicit image IDs to both instances and check the native image lookup.
The control requires no stimulus record and parses no image bytes.

Scope: every Core clause that needs terminal semantics, checked against libghostty.
Contract: botster-contracts tag `contracts-v0.1.1` (`366bca41da0a6de69cc1ea13b17c773cdfdb75b6`): manifest final14, Core erratum 2 ("E2") and steward ruling R-13.
Plan pin: `stage1-plan.555bc433` (sha256 `555bc4337fe72e9fad833fe330d43e8147a39569cb14291734f34847f56596d7`), sections 0, 6.1, 6.3, 7.1, 8, 9 (Q1) and 10 (R1).
Revision 12 of this audit. Revision 12 moves the Ghostty pin to the upstream sync of 2026-10-09, which the fork policy requires before the pin move; patch 1's query options are renumbered to 47 and 48, and no audit row changes (section "Revision 12").
Revision 11 of this audit. Revision 11 moves the Ghostty pin to the upstream sync of 2026-10-04 and adds fork patch 14 for audit finding A6 (steward ruling R-32): an OSC 5522 write over libghostty's own transaction limit still reaches the clipboard callback (section "Revision 11").
Revision 10 of this audit. Revision 10 moves the Ghostty pin to the upstream sync of 2026-10-02 (section "Revision 10"); it changes no audit row, because the libghostty-vt sources are byte-identical between the old and the new fork head.
Revision 9 answers the binding review findings P24 to P34 with fork patches 9 to 13 and binding changes (section "Revision 9"); the status of each finding is in that section, and a finding is closed only when the reviewer and the required evidence say so. Core Amendment 13 (final, candidate 5: contracts `627d507`, manifest final30, `frozen/current/core-contract-v1.17-amendment-13-candidate5.md`) is the clipboard contract.
Revision 3 of this audit. Revision 2 closed findings F1 to F8. Revision 3 closes F9 to F12 of the reviewer verdict `3b2acb3`: the per-commit mapping of the nine removed commits (F9), the offline build prerequisite (F10), the UP clipboard-write callback (F11) and the snapshot rule at every cut (F12). R-13 closes the SGR-pixels question (old Q5).

## Method and revisions

- **Ghostty pin (revision 12):** `trybotster/ghostty` branch `botster/upstream-sync-20261009` at `39a68e822e505685d1e7fa9c6abeff21125b489c`, on upstream `ghostty-org/ghostty` `main` `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d`. The patch list with commit SHAs is in section "Revision 12".
- **Ghostty pin (revision 11, not pinned on `v1`):** `trybotster/ghostty` branch `botster/upstream-sync-20261004` at `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`, on upstream `ghostty-org/ghostty` `main` `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`. The patch list with commit SHAs is in section "Revision 11". The rows below were written against PIN and UP; section "Revision 11" lists what changed at this pin.
- **Ghostty pin (revision 10):** `trybotster/ghostty` branch `botster/upstream-sync-20261002` at `3f8eb6810bb673aa782b047de21783ac81fb1121`, on upstream `ghostty-org/ghostty` `main` `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.
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
| G10 | Clipboard `selection` and terminator (EV-3, EV-8): the program's selection string (`s0`, `cp`, ...) and the request terminator. The OSC 52 parser requires the selection to be empty or one character (`clipboard_operation.zig`: `data[1]` must be `;`), so it drops a selection such as `s0` or `cp` and no read or write callback runs | location only, one-character parse | same | fork patch 8 |
| G11 | `xterm-ghostty` terminfo source text (TI-1) | not exported | same | fork patch 5 |
| G12 | Legacy printable key with Shift and no text (5.1A legacy rule iii): the supplied `shifted_key`, else the ASCII uppercase of `a` to `z`, else no output (no layout guess) | `legacy()` writes nothing and never reads alternates | same | fork patch 4 |
| G13 | Clipboard write shape (Core Amendment 13, A13-1): location, exact selection, every representation, size admission in the callback | first representation only, reply always SUCCESS | same | **binding** (no patch): event and callback, section "Revision 9" |
| G14 | OSC 5522 acknowledgement is written after the host callback returns (A13-1b) | written after the callback | same | fork patch 11 |
| G15 | No getter for modifyOtherKeys state 2 and XTSHIFTESCAPE (E2-2) | none | none | fork patch 9 |
| G16 | Linux static archive defines `calloc` and `free` (P32) | defined | defined | fork patch 10 |
| G17 | Snapshot decode restores the library default image storage limit (ST-6b, P34) | default limit | same | fork patch 13 |
| G18 | Keypad equals key has no key table entry (IN-9) | none | none | fork patch 12 |
| G19 | An OSC 5522 write over libghostty's own transaction limit (64 MiB) gets no callback, and the model answers EFBIG itself (EV-3 as A13 replaces it, A13-1b; audit A6, R-32) | silent, EFBIG | same | fork patch 14 (section "Revision 11") |
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
| IN-9 `Key` legacy printable | Legacy (`kitty_flags = 0`): (i) text without ctrl, alt, meta, super is written as its UTF-8; (ii) ctrl and alt rules use the base character; (iii) Shift with no text writes `shifted_key` if given, else the ASCII uppercase of `a` to `z`, else typed zero (no layout guess). | `key_encode.zig` `legacy()`: (i) and (ii) exist. With empty `utf8` it returns after the alt prefix and never reads alternates. | **GAP G12** for (iii): Shift with no text writes nothing, also with a supplied shifted key. Patch 4 adds the rule inside `legacy()`, for events that supply alternates and without ctrl or alt. P3 validates that `shifted_key` needs Shift. P3 does not build the legacy bytes. |
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
| EV-3, EV-7 clipboard write | `ClipboardWrite{selection, bytes, total_bytes}`. | `GHOSTTY_TERMINAL_OPT_CLIPBOARD_WRITE` (26), `GhosttyClipboardWrite{location, contents, contents_len, name}`. `src/terminal/stream_terminal.zig` (~line 508) maps `s` and `p` to distinct locations and every other selector to the standard clipboard. The original selector string is lost; `c` and `q` look the same. The parser (`src/terminal/osc/parsers/clipboard_operation.zig`, PIN and UP) accepts only an empty or one-character selection: it needs `data[1] == ';'`, so a selection such as `s0` or `cp` makes the whole OSC invalid and no write callback runs. | **G10.** `selection` is part of the event. Patch 8 makes the parser read the whole selection (everything up to the first semicolon) and adds the original selection string to the write payload. **UP callback shape (F11):** `GhosttyTerminalClipboardWriteFn` returns `void` (`terminal.h` 816 to 870). The callback must call `write->reply(write, &reply)` before it returns; a return without a reply denies the write. The binding does not carry the PIN interface, where the callback returned a result. The binding's callback copies the borrowed `contents`, `location` and the patch 8 selection string into its bounded event buffer, and then calls `write->reply(write, &reply)` once, inside the callback, with `reply.result = GHOSTTY_CLIPBOARD_WRITE_RESULT_SUCCESS` and `reply.remember = false`. Core decides nothing here: EV-3 reports the write as an event. OSC 52 has no write acknowledgement, so libghostty discards the reply and writes no byte to the PTY. A test feeds `OSC 52 ; c ; <base64> ST` and checks that no byte reaches the PTY and that the event carries the selection `c`. The `write` object and its borrowed data are valid only until the callback returns, so the buffer copy happens first and nothing keeps the pointer. The oversize rule is the binding's byte count. |
| EV-3, EV-8 clipboard read | `ClipboardRead{selection}`, and a typed reply that carries the request's selection and terminator. | PIN: OSC 52 read ignored. UP: `GHOSTTY_TERMINAL_OPT_CLIPBOARD_READ` (38), `GhosttyClipboardRead{location, mimes, mimes_len, list, can_remember, reply}`. | **G7 at PIN, covered at UP, with G10:** UP gives `location` but not the selector string or the terminator, and its parser drops a selection of more than one character (`s0`, `cp`), so no read callback runs for it. Patch 8 reads the whole selection and adds both fields. The UP callback is synchronous and generates an empty-clipboard answer when it returns without a reply or with a denial. The binding must suppress that answer: it never reaches the PTY and it never counts as a shadow answer (EV-8: the shadow never answers a clipboard read). Test: feed `OSC 52 ; c ; ? ST` and check that no byte is written to the PTY by libghostty. |
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
| 4 | Key event: `HYPER` and `META` bits, `shifted_key` and `base_layout_key` setters, `F26` to `F35`; encoder: associated text only with flags 8 and 16, whole-text omission for C0, DEL and C1 | IN-9, 5.1A; legacy: Shift with no text produces the supplied shifted key, else the ASCII uppercase of a to z, else nothing (5.1A legacy rule iii, only for events that supply alternates, and only without ctrl or alt) | IN-9, 5.1A | G5, G12 |
| 5 | Export the `xterm-ghostty` terminfo source from lib-vt | TI-1, A2-8 | G11 |
| 6 | Mouse: cell-coordinate entry point with no clamp and no output for an unrepresentable cell, a release that obeys the same rule, `GHOSTTY_TERMINAL_DATA_MOUSE_EVENT` and `_MOUSE_FORMAT` getters, (SGR-pixels needs no offset: R-13) | IN-9, 5.1A, ST-4, EV-7 | G8 |
| 7 | Reply encoders: clipboard, `CSI 5 t` and `CSI 9 t` replies, window state, window position, title and icon label text | EV-8 | G9 |
| 8 | The OSC 52 parser reads the whole selection (everything up to the first semicolon), and the original selection string and terminator are on the clipboard write and read payloads | EV-3, EV-8 | G10 |
| 9 | Terminal data getters for modifyOtherKeys state 2 (45) and XTSHIFTESCAPE (46) | E2-2 | G15 |
| 10 | Link libc on Linux, so the static archive defines no allocator symbol | P32 | G16 |
| 11 | The OSC 5522 acknowledgement is written when the host replies | A13-1b | G14 |
| 12 | The keypad equals key: SS3 X in application keypad mode, `=` otherwise | IN-9 | G18 |
| 13 | Snapshot decoder option `GHOSTTY_SNAPSHOT_DECODER_OPT_KITTY_IMAGE_STORAGE_LIMIT` | ST-6b | G17 |

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
  - `N-V-__8AAAfDBACe1jGqjr9jIG3UAK8KbzJKQ7xrzYoau-_a` (revision 11; it was `N-V-__8AAM94BAAFk_hn4UW0x_OBD2g0vOwexeAAyWNNo4eB` until revision 10: upstream updated the `iterm2_themes` package)
  - `N-V-__8AAB0eQwD-0MdOEBmz7intriBReIsIDNlukNVoNu6o`
  - `N-V-__8AADYiAAB_80AWnH1AxXC0tql9thT-R-DYO1gBqTLc`
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
- **Final reuse (steals, each its own commit with `Stolen-From:` and `For-Clause:` trailers against `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`):** `build.rs` and `sys.rs` of the old crate, both for A2-8, with edits for the pinned fork (offline build under network denial, the new C declarations). `lib.rs` and `input.rs` of the old crate were **not** reused: the new `lib.rs` has no client projection, and the 5.1A key table is new.
- **Rejected:** `src/client.rs` (a client projection) and the old `TerminalMetadataProducer` scanner (a parser outside libghostty, BUILD.md rule 1). `keys.rs` of the old protocol crate is read for coverage only.
- **Custom pieces and reasons:**
  - The 5.1A key table is written fresh: the contract names differ from the old W3C names.
  - The event buffer, the held-reply buffers and the clipboard capture: no library holds Core's bounds (EV-2, EV-8).
  - The fork patches 1 to 11: each fills a gap in this audit, and each is the only way to keep rule 1 (no terminal semantics outside libghostty).
  - `Terminal::shadow_answerable_kinds` asks the library with a scratch terminal. No API lists the shadow's answers, and a table in Rust would be a second source of truth.
  - `snapshot_format` reads the library's own envelope from a snapshot, for the same reason.
  - The base64 decode of a typed clipboard reply uses the maintained `base64` crate (`STANDARD` engine: canonical padding, no stray bits). A hand-written decoder was removed in review finding P31.
  - The stress test: it covers the nine commits that the fork branch does not carry.
- **Ecosystem:** `libghostty-vt` has no maintained Rust binding with these hooks. The `libghostty-rs` crates were not evaluated at the pinned revision; the lead may name one if it should be considered.

## Revision 9: binding review and patches 9 to 13

Native patches 9 to 13 are on `trybotster/ghostty` branch `botster/vt-core-stage1-c`, on top of `85a8d8e`. They wait for the reviewer and the lead's pin record (Q1). Status words: **corrected** means the change and its tests are in the binding head; **pending** means the reviewer or the Linux gate has not yet confirmed it.

| Finding | Gap and result | Where | Status |
|---|---|---|---|
| P24 (H1) | The Kitty graphics protocol is off before any write: image storage limit zero, on both screens. Test: the limit reads zero on both screens, and a terminal that received a 1x1 image has the snapshot of one that did not. The worker keeps `snapshot_graphics` out of its feature list. | `lib.rs`, `tests_color.rs` | closed by the reviewer |
| P34 (G17, ST-6b) | The snapshot decoder rebuilds each screen with the library default image limit, so a restored model would accept image input that the session's model ignores. Patch 13 adds the decoder option `GHOSTTY_SNAPSHOT_DECODER_OPT_KITTY_IMAGE_STORAGE_LIMIT` (`uint64_t`), applied to every restored screen before the continuation is replayed. The snapshot format is unchanged and still holds no graphics state; the exclusion is stated in the doc of `snapshot_format`. Tests: Zig (the limit is the library default without the option and zero with it, on the alternate screen too); Rust (the every-offset resume corpus has image input, and the restored terminal's limit is zero before and after the suffix and on the alternate screen). **Image-resume proof (revision 10, in the binding, no fork change):** at every byte offset of a stimulus with a one-command image and a chunked image, a terminal restored with the option at zero takes the rest of the input and stores no image (the library's own storage lookup, `GHOSTTY_TERMINAL_DATA_KITTY_GRAPHICS` and `ghostty_kitty_graphics_image`), on the primary and the alternate screen. A control restore at the library default limit stores image 1 exactly when a default-limit model that took only the prefix has not stored it, stores image 2 when its input is all in the suffix, and stores both on the alternate screen; so the check fails without the option, including where the continuation replays part of the command. | patch 13, `tests_snapshot.rs` | corrected, pending review |
| P25 clipboard shape (G13, A13-1) | The event carries the native `location`, the exact OSC 52 `selection` (`None` when empty), the `terminator`, every `{mime, bytes}` representation in order (`Some(vec![])` clears; one empty entry is an empty value), `total_bytes` and `too_large`. The callback answers inside the native call by size alone: SUCCESS within `set_clipboard_limit`, IO_ERROR over it, never DENIED, UNSUPPORTED or BUSY. A write over the limit keeps no bytes. No I/O, no host call, no pointer kept. The worker maps `selection`: a non-empty string as written, else `location` (standard to `c`, primary to `p`, selection to `s`). | `events.rs`, `tests.rs` | corrected |
| P25 acknowledgement (G14, A13-1b) | At `85a8d8e` the OSC 5522 commit status is written after the host callback returns, and the model raises no callback between the two. Patch 11 writes it when the host replies (same bytes, same order). The binding captures the bytes written during the reply into `Drained::clipboard_acks`, **one entry per write, in order, apart from the event buffer**, so that an event dropped for a full buffer (class D) never takes its acknowledgement. Backpressure: undrained acknowledgements above `set_ack_backlog_limit` make `vt_write_until_query` return `Error::AckBacklog` until the caller drains; within one chunk they are bounded by the chunk, because each answers a write sequence in it. The binding writes them nowhere: the worker writes each as one contiguous AM-2 transaction. Tests: order and ids; full event buffer with no host still gives the acknowledgement; backlog stops the write; over-limit gives a different status; OSC 52 and OSC 1337 give none. | patch 11, `events.rs`, `tests.rs` | corrected, pending review |
| P26 (G15) | The terminal tracks `modify_other_keys_2` and `mouse_shift_capture`, the key encoder ignores state 2 under any kitty flag, and the C API had no getter. Patch 9 adds `GHOSTTY_TERMINAL_DATA_MODIFY_OTHER_KEYS_2` (45) and `GHOSTTY_TERMINAL_DATA_MOUSE_SHIFT_CAPTURE` (46). `other_modes` reports `xterm_modify_other_keys_2` and `xterm_mouse_shift_capture` (E2-2). Test: state 2 stays true with kitty flags on. | patch 9, `modes.rs` | closed by the reviewer |
| P27 (G8) | **Representation limits of an SGR pixel position.** The native API takes `f32` and converts to `i32`. The binding passes a position that an `f32` holds exactly and that is within `i32` (`f64::from(x as f32) == f64::from(x)` and `i32::try_from(x)`); it refuses every other position with `Unsupported{what: coordinate}` before encoding, for every action (releases bypass the viewport refusal). Every integer up to 2^24 is exact; above it only some are (2^24+2, 2^24+4, ..., and 2^31 - 128 is the last below `i32::MAX`). Tests: releases at 0, 1, 2^24-1, 2^24, 2^24+2, 2^24+4 and 2^31-128 report the value unchanged; 2^24+1, 2^24+3, 2^24+5, `i32::MAX`, 2^31, 2^31+128 and `u32::MAX` are refused for press, release and move, on each axis. Without a cell size, the pixel screen is unbounded (the position is reported as given, R-13). | `encode.rs`, `tests_encode.rs` | corrected, pending review |
| P28 (R-14.1) | The eight named modifier keys alone give no bytes unless kitty flag 8 is on. The binding maps that result to `Unsupported{what: named_key}`. Tests with flag 8 off and on, and flag 1 alone. | `encode.rs` | closed by the reviewer |
| P29 (ST-6b) | The resume test restores at every byte offset, feeds the restored terminal the rest, and compares snapshots. The corpus has pending UTF-8, CSI, OSC and DCS input, saved cursor, tab stops, margins, character sets and image input. | `tests_snapshot.rs` | closed by the reviewer (image input added for P34) |
| P30 | No generated expected terminal bytes in tests. | `tests_*.rs` | closed by the reviewer |
| P31 | The typed `Clipboard` reply is decoded with the `base64` crate (`STANDARD`). | `reply.rs` | closed by the reviewer |
| P32 (G16) | The vendored wuffs C code calls `calloc`; a module that does not link libc makes Zig compile its own `malloc.zig` into the archive, which defines `calloc` and `free` (weak) and leaves `malloc` and `realloc` to the host. Patch 10 links libc on Linux (`nm` on an `x86_64-linux-gnu` build: before, `calloc` and `free` defined; after, none defined). Test: `nm` on the archive must list no libc allocator symbol. | patch 10, `tests_archive.rs` | source fix reviewed CLEAN; **pending the Linux gate** |
| P33 | Unique gap identifiers (G13 to G18); the typed OSC 52 rule and the untyped read rule are stated below. | this file | corrected, pending review |
| IN-9 keypad (G18) | The key encoder had no entry for the keypad equals key, so it encoded to nothing in both keypad modes (xterm sends SS3 X in application keypad mode). Patch 12 adds it. The binding's result for a key that still gives no bytes stays `NotReported`, or `Unsupported` where a ruling names the case. Patch 12's Zig test keeps its literal expected bytes under lead ruling P35 (see "Revision 10"). | patch 12 | corrected, pending review |

**Clipboard reads.** Only the typed `ClipboardRead{selection}` label is OSC 52 only (selection `s0` when empty). Every other native clipboard read (OSC 5522, and any other protocol that the model reports) reaches the client as an untyped query with no shadow answer (frozen A13): the binding's query kind for them is `KittyClipboardRead` and its label is `None`.

The binding also exposes, from the lead's rulings: `set_color_profile`, `shadow_answerable_kinds`, `snapshot_format`, and a plain `vt_write` that counts the queries it meets in `Drained::unrouted_queries` and buffers none (PTY output goes only through `vt_write_until_query`, EV-8(d)).

## Revision 10: upstream sync and pin move

- **Sync record:** botster-contracts `docs/ghostty/upstream-sync-20261002.md` (`main` `8ee672a8442758da31665434ebd55059eaa9a876`), with its `docs/ghostty/evidence-20261002/`. The sync obeys the BUILD.md "Ghostty fork policy".
- **Old pin:** branch `botster/vt-core-stage1-c` at `ada251c5e99cdb753de8bf72d5d1f307d474518f`, on upstream `83edd491e3024ae5e50393d62877b8897da1cccd`.
- **New pin:** branch `botster/upstream-sync-20261002` at `3f8eb6810bb673aa782b047de21783ac81fb1121`, on upstream `main` `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.
- **What changed:** `git range-diff` shows all 22 commits as identical patches. `git diff ada251c5e 3f8eb6810` lists only `macos/Sources/Features/Terminal/TerminalController.swift`, which libghostty-vt does not build. The libghostty-vt sources, `build.zig.zon` and the terminfo are byte-identical, so no audit row changes.
- **Patch 12 test (lead ruling P35):** KEEP the literal expected bytes. BUILD.md rule 2 binds tests in Botster repositories, where libghostty is the oracle. Inside the fork, libghostty's own Zig tests define the oracle's behavior, and the upstream neighbor tests use literal bytes too. A test that derives the expected value from the `kpKeys` table checks the table against itself, so it cannot catch the missing table entry that patch 12 fixes.

| # | Commit at the new pin | Commit at the old pin | Subject | Decision |
|---|---|---|---|---|
| 0 | `ee1875dd3` | `ea5a1e297` | own startHyperlink uri and id across capacity retries | KEEP |
| 1 | `bcdcad95b` | `6495721bb` | report every query with its exact bytes and stop after it | KEEP |
| 1 | `79ae024c3` | `38599d320` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP |
| 1 | `303013782` | `b60d00542` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP |
| 1 | `99ceb84cc` | `56b54e923` | test that a C1 byte inside a string sequence is payload | KEEP |
| 1 | `50d8494c5` | `72902b1b7` | a C1 introducer ends an APC string in the query request tracking | KEEP |
| 2 | `d9531aca1` | `7afa387dd` | report the notification source (OSC 9 or OSC 777) | KEEP |
| 3 | `51c80ac71` | `970a1c9df` | paste marker frame without payload rewrite | KEEP |
| 4 | `fddc30c7c` | `50569efc8` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP |
| 4 | `1bef3ee4a` | `22035f7c2` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP |
| 4 | `ec9a95861` | `3f28780d2` | the legacy Alt test compares with the real base-character result | KEEP |
| 5 | `9f476b148` | `da42a8ac0` | export the xterm-ghostty terminfo name and source | KEEP |
| 6 | `f8aa86979` | `b59b1f47b` | mouse cells as given, and getters for the active mouse enums | KEEP |
| 7 | `72d54d39e` | `05540bd69` | encode the typed replies to terminal queries | KEEP |
| 7 | `49e68f944` | `96b4f3f4f` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP |
| 8 | `41024b252` | `d27593e55` | report the OSC 52 selection and terminator on clipboard requests | KEEP |
| 8 | `cfcd2c21c` | `85a8d8eb1` | the OSC 52 parser reads the whole selection | KEEP |
| 9 | `eafd0967b` | `468268e5c` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP |
| 10 | `f0d70e3f1` | `92d13482a` | link libc on Linux so the static archive defines no allocator symbol | KEEP |
| 11 | `1eb68104a` | `170d6faf8` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP |
| 12 | `d5bebc7e2` | `f71c7651f` | the keypad equals key has an application keypad sequence | KEEP (ruling P35) |
| 13 | `3f8eb6810` | `ada251c5e` | the snapshot decoder takes the host's Kitty image storage limit | KEEP |

The commits on the branch are in this order from the base: patches 2 and 3, then 0, 1, 4 to 13. Drop: none; no upstream change covers a patch.

- **Static link on macOS.** Zig installs `libghostty-vt.dylib` next to `libghostty-vt.a`. The Apple linker takes the `.dylib` from that one search directory, whatever the link kind says, so the binding was linked dynamically on macOS. Plain `cargo test` hid this because it sets the loader path; the Mac gate (nextest) failed to load every test. `build.rs` now copies the archive into `OUT_DIR/link` and links from there. Test: `tests_archive::the_library_is_linked_into_the_program_and_not_loaded_from_a_shared_library` (`dladdr` of `ghostty_terminal_new` names no `libghostty` image); it fails with the previous `build.rs`.
- **Mutation testing (review finding P39).** The first gate that reached `cargo mutants --in-diff` found 92 surviving mutants of this crate. Each one is now caught by a test whose expected value comes from libghostty by another path (a raw library call, a library getter, the pinned headers) or from the contract crate (`CoreLimits` defaults), or it is recorded in `.cargo/mutants.toml` with one function and its equivalence argument (Drop bodies that only free, which are excluded as cleanup that no test observes, not proved equivalent; disjoint-bit `|` and `^`; and a call that sets the library's own default). The constructor check `created` is tested on every combination of result and handle. Code that only repeated a library rule is removed: the zero-size refusal and the out-of-screen row (the library refuses both), and the impossible probe results of the size-then-fill calls. The review found one binding defect: `DEFAULT_CLIPBOARD_BYTES` was 16 MiB, and `CoreLimits.clipboard_bytes` is 1 MiB; it is now 1 MiB and tested against the contract.
- **Zig packages at the new pin: 7.** A lib-vt build with `GHOSTTY_BUILD_ARGS` and an empty scratch global cache (Zig 0.16.0, macOS, 2026-10-04) fetched exactly the 7 hashes of `ZIG_PACKAGES`, so the list is unchanged. The count of 9 came from the shell extraction: the `sed` range `/pub const ZIG_PACKAGES/` also matched `ZIG_PACKAGES_IN_ZON`, which repeats 2 of the 7 hashes. `ci/remote/fetch-public.sh` removes the duplicates with `sort -u`. `prefetch-zig.sh` printed "9 packages"; its pattern is now `/pub const ZIG_PACKAGES: /`, so it reads the 7 hashes once.

## Revision 11: upstream sync of 2026-10-04 and patch 14 (audit A6, R-32)

- **Sync record:** `docs/stage1/ghostty-upstream-sync-20261004.md` in this repository. The sync obeys the BUILD.md "Ghostty fork policy": it came before patch 14.
- **Old pin:** branch `botster/upstream-sync-20261002` at `3f8eb6810bb673aa782b047de21783ac81fb1121`, on upstream `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.
- **New pin:** branch `botster/upstream-sync-20261004` at `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`, on upstream `main` `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`.
- **Changes that the binding follows:**
  - Upstream #14483 (DECRQCRA and XTCHECKSUM) took terminal options 44 and 45. Patch 1's `GHOSTTY_TERMINAL_OPT_QUERY` and `_QUERY_MAX_BYTES` are now 46 and 47; `sys.rs` follows. With the old numbers the binding would set the checksum options.
  - DECRQCRA is not reported as a query. Before the sync it was an unimplemented `CSI y` and was ignored. The checksum report is off by default (`GHOSTTY_TERMINAL_OPT_XT_CHECKSUM_REPORT`), and the binding does not set it, so the model neither answers DECRQCRA nor takes XTCHECKSUM. Nothing changes for EV-8. If Botster ever enables the option, DECRQCRA needs a query kind first.
  - The `iterm2_themes` package hash changed (`build_data.rs` `ZIG_PACKAGES`, section "Build notes").
  - The snapshot sources, the terminfo and the key tables of the patches are unchanged; GHOSTSNP.md needs no change. Upstream #14508 adds a legacy `ctrl+alt+shift+backspace` table entry (IN-9 takes it through the encoder).
- **Patch 14 (G19).** Through the C API, data past `kitty_clipboard_write_max_bytes` no longer fails the transaction. The model frees the data it buffered, decodes the rest only to count it (a fixed stack buffer; the same base64 validation), and the commit still calls the clipboard callback. The new trailing fields of the sized `GhosttyClipboardWrite` are `too_large` (true) and `total_len`, with no contents. `total_len` is the **decoded size of the whole transaction**: every decoded byte, including the data of a MIME type that a later chunk of the same type replaced (the steward's reading of R-32's "full length"); an alias adds nothing. The model sends no acknowledgement of its own: the callback's reply is the only one (A13-1b). The Zig stream handler keeps the old behavior (EFBIG) for the Ghostty application. Zig tests: `WriteState` counting with a replaced region and an alias, validation and piece boundaries; through the C API, a write at the limit is delivered and only the reply answers it, and a write over it reaches the callback with its decoded size (11, with a replaced region and an alias, where the final contents would be 7), and only the reply's EIO is written.
- **Binding (G19).** `on_clipboard_write` reads `too_large` and `total_len` when `size` covers them. A write that the model did not keep is posted as `ClipboardWrite{contents: None, total_bytes: <decoded size>, too_large: true}` and answered IO_ERROR. The public type is unchanged, so the worker's use of it is unchanged. `set_clipboard_limit` also sets option 39 (`CLIPBOARD_WRITE_MAX_BYTES`) to the same value, as Core A14-3 (final33, contracts main `69327d5`) requires; `Terminal::new` and the snapshot restore both call it. So A14-2 runs in its two steps inside the callback: step 1 is the model's report (the decoded size, over the limit), step 2 is the binding's sum of the contents (each alias entry at its full length). `total_bytes` is the size of the step that decided. Test `the_clipboard_limit_decides_on_the_decoded_size_then_the_contents_size`: a write at the bound is carried; an alias over the bound is `too_large` with the contents size; a replaced region over the bound is `too_large` with the decoded size (at libghostty's default limit it would be carried, so the configured bound decides); a write over many chunks reports every chunk. Expected sizes come from the input; the acknowledgements are compared with each other, and nothing goes to the pty. A6, the native memory part included, is implemented; **verification pending** until the required Mac, Linux, binding and package-list evidence is recorded and accepted.
- **Ignored MIME types (R-33, contracts main `14c86ab`):** data of MIME types past the model's count limit (64) is never decoded, so it is not in the decoded size and does not count against the limit. A write whose kept types are within the bound is not `TooLarge` because of them; their bytes stay in `Output` (OU-12); invalid base64 in them is not counted. Fork commit `0bfddc16f` pins this with Zig tests (review finding F-A6-01, settled by R-33).
- **Invalid base64** still fails the transaction with EINVAL and no callback, as before (A6 calls this case arguable). R-32 does not cover it.

| # | Commit at the new pin | Commit at the old pin | Subject | Decision |
|---|---|---|---|---|
| 0 | `cba7ba134` | `ee1875dd3` | own startHyperlink uri and id across capacity retries | KEEP |
| 1 | `13f66d661` | `bcdcad95b` | report every query with its exact bytes and stop after it | REWORK (options renumbered to 46 and 47) |
| 1 | `0be524a08` | `79ae024c3` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP |
| 1 | `ed563afdd` | `303013782` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP |
| 1 | `50df7c7e1` | `99ceb84cc` | test that a C1 byte inside a string sequence is payload | KEEP |
| 1 | `8cea067c1` | `50d8494c5` | a C1 introducer ends an APC string in the query request tracking | KEEP |
| 2 | `09d186fd6` | `d9531aca1` | report the notification source (OSC 9 or OSC 777) | KEEP |
| 3 | `c2dc8118c` | `51c80ac71` | paste marker frame without payload rewrite | KEEP |
| 4 | `cdefd75c3` | `fddc30c7c` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP |
| 4 | `2dba9306f` | `1bef3ee4a` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP |
| 4 | `6d09770e4` | `ec9a95861` | the legacy Alt test compares with the real base-character result | KEEP |
| 5 | `7e96afe46` | `9f476b148` | export the xterm-ghostty terminfo name and source | KEEP |
| 6 | `91b06a537` | `f8aa86979` | mouse cells as given, and getters for the active mouse enums | KEEP |
| 7 | `786cd9e7b` | `72d54d39e` | encode the typed replies to terminal queries | KEEP |
| 7 | `1148713f6` | `49e68f944` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP |
| 8 | `db6c206a8` | `41024b252` | report the OSC 52 selection and terminator on clipboard requests | KEEP (context only) |
| 8 | `8da81d4cb` | `cfcd2c21c` | the OSC 52 parser reads the whole selection | KEEP |
| 9 | `fa481e17b` | `eafd0967b` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP |
| 10 | `5aa1e708b` | `f0d70e3f1` | link libc on Linux so the static archive defines no allocator symbol | KEEP |
| 11 | `c13162d1d` | `1eb68104a` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP |
| 12 | `bed0c871a` | `d5bebc7e2` | the keypad equals key has an application keypad sequence | KEEP (ruling P35) |
| 13 | `c370ef4d9` | `3f8eb6810` | the snapshot decoder takes the host's Kitty image storage limit | KEEP |
| 14 | `779907e0e` | none | an OSC 5522 write over the transaction limit reaches the callback with its size | NEW (R-32) |
| 14 | `0bfddc16f` | none | tests that ignored MIME types stay outside the over-limit size | NEW (R-33) |

The commits on the branch are in the same order as at revision 10, with patch 14 last. Drop: none; no upstream change covers a patch.

## Revision 12: upstream sync of 2026-10-09 (before the pin move)

- **Sync record:** `docs/stage1/ghostty-upstream-sync-20261009.md` in this repository. The fork policy requires a sync before every pin move; the 2026-10-04 stack was not pinned yet, so it is synced again before it is.
- **Old candidate:** branch `botster/upstream-sync-20261004` at `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`, on upstream `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94` (revision 11).
- **New pin:** branch `botster/upstream-sync-20261009` at `39a68e822e505685d1e7fa9c6abeff21125b489c`, on upstream `main` `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d`.
- **Changes that the binding follows:**
  - Upstream #14560 (OSC 7501, program status) took terminal option 46. Patch 1's `GHOSTTY_TERMINAL_OPT_QUERY` and `_QUERY_MAX_BYTES` are now 47 and 48; `sys.rs` follows. With the old numbers the binding would set the program status callback. Every other constant of `sys.rs` matches the new headers.
  - `OSC 7501 ; ?` is not reported as a query. The model answers it only while a program status callback is set, and the binding sets none, so the model neither answers nor reports it. Nothing changes for EV-8.
  - DECSTR (#14538) is now model behavior: the soft reset resets ten modes, the margins, the pen, the protection, the charsets, the saved cursor, `modify_other_keys_2` (patch 9's getter, E2-2), XTCHECKSUM and a changed palette. Core reports the model's state after it (E2-2); no clause names DECSTR.
  - The snapshot sources and the terminfo are unchanged; `ModePacked` is unchanged (`modes.zig` gains only a getter). GHOSTSNP.md needs no change. The binding's formatter use (plain text, extras off) is outside #14588's change.
  - The Zig package list is unchanged (an empty-cache build fetched exactly `ZIG_PACKAGES`).
  - **Test scope (lead decision, 2026-10-09):** `zig build test-lib-vt` runs on Linux with the shipped options (`-Demit-lib-vt -Dsimd=false -Dcpu=baseline`, Debug), from a Zig cache that holds only `ZIG_PACKAGES`, because the gate has no network. Upstream's default configuration (SIMD, the application packages) is covered by the Mac run only. `ZIG_PACKAGES` gets no test-only list. The Linux run is the proof that `ZIG_PACKAGES` is sufficient for the tests; the Mac empty-cache build is the proof of the shipped library's package list (finding F-A6-04). Results, and why the Mac and Linux step and skip counts differ: the sync record of 2026-10-09, section "Test results".
- **Patch 14 and A6** are unchanged in substance: the first commit changed only by a context line. The Revision 11 text above stays the description of G19, the binding and A14. A6: implemented; verification evidence in the sync record of 2026-10-09.

| # | Commit at the new pin | Commit at the old candidate | Subject | Decision |
|---|---|---|---|---|
| 0 | `9ace459ce` | `cba7ba134` | own startHyperlink uri and id across capacity retries | KEEP |
| 1 | `32a783ad9` | `13f66d661` | report every query with its exact bytes and stop after it | REWORK (options renumbered to 47 and 48) |
| 1 | `2d78928e8` | `0be524a08` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP |
| 1 | `85e7d8429` | `ed563afdd` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP |
| 1 | `9c40369f5` | `50df7c7e1` | test that a C1 byte inside a string sequence is payload | KEEP |
| 1 | `5e4328efe` | `8cea067c1` | a C1 introducer ends an APC string in the query request tracking | KEEP |
| 2 | `473f19472` | `09d186fd6` | report the notification source (OSC 9 or OSC 777) | KEEP |
| 3 | `17ae694b1` | `c2dc8118c` | paste marker frame without payload rewrite | KEEP |
| 4 | `a7f2a8910` | `cdefd75c3` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP |
| 4 | `e045bd0de` | `2dba9306f` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP |
| 4 | `3fbf36adc` | `6d09770e4` | the legacy Alt test compares with the real base-character result | KEEP |
| 5 | `30541bfc4` | `7e96afe46` | export the xterm-ghostty terminfo name and source | KEEP |
| 6 | `a10ad1bf3` | `91b06a537` | mouse cells as given, and getters for the active mouse enums | KEEP (context only) |
| 7 | `5dec71c70` | `786cd9e7b` | encode the typed replies to terminal queries | KEEP |
| 7 | `85086ba4f` | `1148713f6` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP |
| 8 | `64b68fe6e` | `db6c206a8` | report the OSC 52 selection and terminator on clipboard requests | KEEP |
| 8 | `96002bd35` | `8da81d4cb` | the OSC 52 parser reads the whole selection | KEEP |
| 9 | `014c0395c` | `fa481e17b` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP |
| 10 | `622e4fbf2` | `5aa1e708b` | link libc on Linux so the static archive defines no allocator symbol | KEEP |
| 11 | `b2e4a7c44` | `c13162d1d` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP |
| 12 | `e5c8b2827` | `bed0c871a` | the keypad equals key has an application keypad sequence | KEEP (ruling P35) |
| 13 | `2b0dd287e` | `c370ef4d9` | the snapshot decoder takes the host's Kitty image storage limit | KEEP |
| 14 | `5cb2efb68` | `779907e0e` | an OSC 5522 write over the transaction limit reaches the callback with its size | KEEP (context only) |
| 14 | `39a68e822` | `0bfddc16f` | tests that ignored MIME types stay outside the over-limit size | KEEP |

The commits are in the same order as at revision 11. Drop: none; no upstream change covers a patch.

## Rulings used

- **E2 ruling A:** OSC 1 gives no `TitleChanged`. The bytes stay in `Output`.
- **E2 ruling B:** `other_modes` is the set of modes that libghostty tracks and that have no normative `ModeFlags` field. An unrecognized mode number changes no field. There is no second parser.
- **E2 ruling C:** after each `vt_write`, compare the final `ModeFlags` with the last posted `ModesChanged` value. Post one event with the final flags if they differ. Post nothing if they are equal. No mode-change patch.
- **R-13:** SGR-pixels reports zero-based pixels with no +1; the 5.1A `+1` is for cells only.
- **R-28 (contracts `bcbd03c`, in `contracts-v0.1.13`):** a legacy key release is `NotWritten(NotReported)` (5.1A rule 1, "not reported"). `release_event` stays a listed `what` value that no v1 stimulus produces. The binding returns `EncodeError::NotReported` for it.
- **Lead decision:** paste is byte-exact (DP-5, IN-8).
- **Core A14 (final33, contracts main `69327d5`):** `clipboard_bytes` bounds the decoded size first, then the contents size; the model's decode limit equals it (option 39).
- **R-33 (contracts main `14c86ab`):** MIME types that the model ignores are not in the decoded size.
- **R-32 (contracts main `9a00db8`, in `contracts-v0.1.14`):** an OSC 5522 write over libghostty's own transaction limit still calls the clipboard callback, with no contents and the write's full length; the model sends no acknowledgement of its own (patch 14). The steward reads "full length" as the decoded size of the whole transaction (2026-10-04, through the lead).

# P2 libghostty review

Reviewed head: `fd1471eabca0adb344c6027b8995e1151aa8f8a8`.
Scope: the written audit only. This verdict does not approve the binding or a Ghostty pin change.

VERDICT: NOT CLEAN (8 open)

The review uses manifest final13, BUILD.md at `2f2996ef0f016a1fefc6879e74deaef033383b66`, and plan pin `stage1-plan.a24efe7e`.
The plan hash matches its recorded SHA-256.
Ghostty evidence uses `git show` at these commits:

- PIN: `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`.
- UP: `83edd491e3024ae5e50393d62877b8897da1cccd`.

The reviewer checked every audit row against its headers and relevant implementation.
The reviewer ran no tests. Source logic proves the findings below.

## F1 — HIGH — Mouse coverage omits known coordinate defects

Status: OPEN.
Audit evidence: lines 48–50 mark mouse encoding COVERED and defer possible clamping to a future test.

Core 5.1A supplies zero-based cells and pixels. The encoder must apply the xterm coordinate offset.
An unrepresentable coordinate must produce typed zero output, without truncation.
At PIN and UP, `src/input/mouse_encode.zig:161` writes `pixels.x` and `pixels.y` directly for SGR-pixels.
The encoder does not add the offset that it adds for cell formats.
At PIN, `posToCell` explicitly clamps to the viewport.
The release path bypasses the viewport refusal before this conversion.
Thus the audit cannot claim that `_encode` preserves supplied coordinates or always returns zero for unrepresentable coordinates.
The C API also accepts only floating-point surface positions, rather than independent cell and pixel coordinates.

Required change: record the coordinate differences as GAPs now.
Check each difference at UP and extend the Q1 patch proposal where UP does not cover it.
Describe how the binding preserves `row` and `col` without requiring `Size.cell_px` for non-pixel formats.
Do not defer an established source defect to a future test.

## F2 — HIGH — Graphics snapshot coverage is false

Status: OPEN.
Audit evidence: line 79 marks graphics snapshots COVERED because snapshot files reference kitty graphics.

At PIN, `src/terminal/snapshot/snapshot.zig:1197` tests preservation of virtual placeholders.
The test explicitly omits the image and placement registry.
After restoration, the test expects both registries to contain zero entries (`:1264` and `:1268`).
`kitty_graphics.h` exposes image data. It does not make the snapshot encoder serialize that data.
ST-6b requires restoration when the model holds images and reports `snapshot_graphics`.
The resume invariant also covers state that changes interpretation of later output.
Advertising the feature only after a future test does not establish current coverage.

Required change: mark image and placement restoration as a GAP.
Check UP and record its result.
Propose a patch, or state a concrete configuration in which the model cannot hold images and reports the feature absent.
Do not use a failed feature test as an undocumented exclusion of state that the model holds.

## F3 — HIGH — Typed query reply encoding is an unlisted GAP

Status: OPEN.
Audit evidence: line 108 leaves missing reply encoders as a P4b design question.
The summary and patch series omit this GAP.

EV-8 specifies typed replies for clipboard, pixel and character sizes, window state, position, title, icon label, and color scheme.
BUILD.md assigns all terminal encoding to libghostty.
At PIN and UP, `size_report.h` supports only mode 2048 and CSI 14, 16, and 18 reports.
It does not support the CSI 15 or 19 reply forms.
`ghostty_mode_report_encode` and `ghostty_focus_encode` encode different protocols. They do not serve the missing typed replies.
The audit also omits the available `ghostty_color_scheme_report_encode` symbol.

Required change: add one coverage entry for every EV-8 typed reply form.
Include clipboard selection and the request terminator.
Mark absent encoders as GAPs and check UP for each GAP.
Add required encoders to the Q1 patch proposal. P4b cannot write terminal reply bytes by hand.

## F4 — HIGH — Clipboard write coverage loses the selection

Status: OPEN.
Audit evidence: line 94 marks `ClipboardWrite{selection, bytes, total_bytes}` COVERED.

At PIN, `include/ghostty/vt/terminal.h:491` exposes `GhosttyClipboardWrite.location` and decoded representations.
It exposes no original selection string.
`src/terminal/stream_terminal.zig:508` maps `s` and `p` to distinct locations and maps every other selector to standard.
The callback therefore cannot distinguish `c` from `q`, for example.
`src/terminal/osc/parsers/clipboard_operation.zig` accepts a single selector and discards protocol information before this callback.
EV-3 requires the `selection` field. A normalized location alone does not establish coverage of that field.

Required change: record the missing selection data as a GAP, or obtain a steward ruling that permits this normalization.
Check UP's actual payload fields, not only the existence of a clipboard callback.
Include any required preservation of the selection in the patch proposal.

## F5 — HIGH — The key NOTE changes text and leaves flag behavior unchecked

Status: OPEN.
Audit evidence: lines 42 and 46 claim coverage for kitty flags 1, 2, 8, and 16.
The audit proposes dropping any `text` that contains a control codepoint before `_set_utf8`.

Core 5.1A omits control-bearing text from associated-text encoding. It does not reject or erase all produced text.
The same `utf8` field serves legacy produced text, kitty plain-text output, alternate-key inference, and associated text.
Unconditionally clearing this field changes more than the associated-text parameter.
At PIN and UP, `src/input/key_encode.zig:294` checks `report_associated` without also checking `report_all`.
At PIN, `KittySequence.encodeFull` skips individual ASCII controls, while `isControl` excludes C1 controls.
These rules do not establish the contract's whole-text omission rule for C0, DEL, and C1.
The audit cannot classify these differences as input validation and rely on future tests to discover them.

Required change: audit the actual flag-16 behavior and text paths.
Record differences as GAPs under G5, or describe a library API that preserves all required text uses independently.
Propose the required library change after checking UP.
Keep semantic encoding policy inside libghostty, as BUILD.md requires.

## F6 — MEDIUM — The audit lacks required coverage and a Prior art note

Status: OPEN.
The audit states that its scope includes every Core clause that needs terminal semantics.
It omits explicit coverage for SZ-1's cell pixel size, ST-4's terminal state, ST-5's retained model, ST-7's atomic terminal facts, and IN-10's terminal guard.
Some duties belong to other packages. The audit must still identify their required library inputs and ownership.
The document also lacks BUILD.md rule 0's Prior art note.

Required change: add the missing rows or explicit references to existing rows.
State which duties are Core bookkeeping and which require library semantics.
Add the Prior art note with sources, proposed reuse, rejected code, and reasons for each proposed custom piece.
Read old code only at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.

## F7 — LOW — Several header claims are inaccurate

Status: OPEN.

- Line 105 says the PIN unknown-sequence callback supports OSC and APC. `terminal.h:365` exposes only APC at PIN. UP adds OSC.
- Line 91 names `GhosttySemanticPrompt`. The UP payload type is `GhosttyTerminalSemanticPrompt`.
- Lines 44–45 cite `key.h` for modifier constants and key values. The definitions are in `key/event.h`.
- Line 89 says an empty pwd means a cleared or invalid OSC 7. The header specifies a clear and the getter also returns empty when unset.
- Line 88 assigns `change_window_icon` to `stream_terminal.zig`. `src/terminal/stream.zig:2490` contains the ignored-icon branch.

Required change: correct the paths, exact symbols, and PIN-versus-UP claims.
Do not add meanings that the headers do not state.

## F8 — HIGH — ModeFlags cannot read the authoritative mouse mode

Status: OPEN.
Audit evidence: lines 59–60 claim that mode bits cover `mouse_tracking` and `mouse_encoding`.

At PIN, `src/terminal/modes.zig:36` changes only the requested mode bit.
`src/terminal/stream_terminal.zig:810` separately sets `terminal.flags.mouse_event` for each tracking command.
The last tracking command sets that enum, even if other tracking bits remain set.
Enabling 1000 then 1003 produces the same mode bits as enabling 1003 then 1000.
The active tracking enums differ between those two histories.
The format commands likewise set `terminal.flags.mouse_format` separately from their mode bits.
`src/input/mouse_encode.zig:43` reads those enums for authoritative encoding.
The PIN and UP C headers expose only a mouse-tracking boolean and individual mode bits.
The mouse encoder exposes setters and encoding, but no getter for the selected enums.
The binding therefore cannot derive the required ModeFlags values from the proposed reads without duplicating mode semantics.

Required change: mark authoritative mouse-mode reads as a GAP at PIN and UP.
Add getters for the active tracking and format enums to the Q1 patch proposal, or identify an existing equivalent API.
Do not reconstruct these enums from bit precedence or from a Botster parser.

## Remaining review notes

G1, G2, G3, G4, G5, and G6 identify real PIN limitations.
UP supplies the prompt callback with an `int32_t` exit code.
UP supplies a synchronous clipboard-read callback, but that callback does not replace G1's exact request requirements.
The UP clipboard API returns an empty clipboard for a denial or for a callback that returns without a reply.
The binding must suppress that generated response. It must never treat it as an allowed shadow clipboard answer under EV-8.

The unknown-mode and mode-transition questions must remain unresolved until the lead supplies the steward's answer.
The audit must not state that unknown modes mean only tracked modes before that answer.
Reading only the final modes after a chunk can miss a set/reset pair. A test that splits sequences does not prove chunk-independent behavior.

The proposed G1 effect must also support EV-8g's parser backpressure at an unadmitted query.
The binding needs the query boundary to preserve Output/query/Output order under EV-8c.
A callback that reports a query but continues through the rest of the input does not alone establish these properties.
Record this requirement when the patch interface is designed.

The reviewer compared both `src/terminfo/ghostty.zig` and `src/terminfo/Source.zig` through `git show`.
Both files are byte-identical at PIN and UP.
This supports the audit's terminfo comparison. Any final patched pin still needs the same comparison.

Every finding, including F7, must close before CLEAN.

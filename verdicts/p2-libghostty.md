# P2 libghostty review

Reviewed head: `c1589932d6644927d4f4dbdfd32d8c474fda03d8`.
Previous audit head: `fd1471eabca0adb344c6027b8995e1151aa8f8a8`.
Scope: the written audit only. This verdict does not approve the binding or a Ghostty pin change.

VERDICT: NOT CLEAN (2 open: audit F12 reopened; fork patch P13).
Written audit: CLEAN at `e9f8727da6d23ee700312d4f67eb0a5aa0b502d7`; F1–F12 closed.

The review uses manifest final13, BUILD.md at `2f2996ef0f016a1fefc6879e74deaef033383b66`, and plan pin `stage1-plan.a24efe7e`.
The plan hash matches its recorded SHA-256.
Ghostty evidence uses `git show` at these commits:

- PIN: `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`.
- UP: `83edd491e3024ae5e50393d62877b8897da1cccd`.

The reviewer checked every audit row against its headers and relevant implementation.
The reviewer ran no tests. Source logic proves the findings below.

## F1 — HIGH — Mouse coverage omits known coordinate defects

Status: CLOSED in audit revision 2. Original finding follows for the record.
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

Status: CLOSED in audit revision 2. Original finding follows for the record.
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

Status: CLOSED in audit revision 2. Original finding follows for the record.
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

Status: CLOSED in audit revision 2. Original finding follows for the record.
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

Status: CLOSED in audit revision 2. Original finding follows for the record.
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

Status: CLOSED in audit revision 2. Original finding follows for the record.
The audit states that its scope includes every Core clause that needs terminal semantics.
It omits explicit coverage for SZ-1's cell pixel size, ST-4's terminal state, ST-5's retained model, ST-7's atomic terminal facts, and IN-10's terminal guard.
Some duties belong to other packages. The audit must still identify their required library inputs and ownership.
The document also lacks BUILD.md rule 0's Prior art note.

Required change: add the missing rows or explicit references to existing rows.
State which duties are Core bookkeeping and which require library semantics.
Add the Prior art note with sources, proposed reuse, rejected code, and reasons for each proposed custom piece.
Read old code only at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.

## F7 — LOW — Several header claims are inaccurate

Status: CLOSED in audit revision 2. Original finding follows for the record.

- Line 105 says the PIN unknown-sequence callback supports OSC and APC. `terminal.h:365` exposes only APC at PIN. UP adds OSC.
- Line 91 names `GhosttySemanticPrompt`. The UP payload type is `GhosttyTerminalSemanticPrompt`.
- Lines 44–45 cite `key.h` for modifier constants and key values. The definitions are in `key/event.h`.
- Line 89 says an empty pwd means a cleared or invalid OSC 7. The header specifies a clear and the getter also returns empty when unset.
- Line 88 assigns `change_window_icon` to `stream_terminal.zig`. `src/terminal/stream.zig:2490` contains the ignored-icon branch.

Required change: correct the paths, exact symbols, and PIN-versus-UP claims.
Do not add meanings that the headers do not state.

## F8 — HIGH — ModeFlags cannot read the authoritative mouse mode

Status: CLOSED in audit revision 2. Original finding follows for the record.
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


## Revision 2 review

The lead authorized final14 at `063d6f05a9bd2e02fe8d252e82aff72c34a25d01` and Core erratum 2.
The plan pin remains `stage1-plan.a24efe7e`.
The lead also supplied steward ruling R-13 at `bcdcf19c756de7c06b38481d38ca1db30f027f86`.
R-13 requires no offset for SGR-pixels. The original F1 pixel-offset concern is withdrawn under that ruling.

F1–F8 close for the written audit:

- F1: G8 records the cell API and clamping defects. Patch 6 supplies the cell path. R-13 resolves the pixel question.
- F2: H1 disables kitty image storage with a zero limit and omits `snapshot_graphics`.
- F3: G9 lists the missing typed encoders and the available color-scheme encoder.
- F4: G10 records the original-selection gap and proposes preservation in patch 8.
- F5: G5 preserves other text uses and changes associated-text rules inside libghostty.
- F6: the missing clause rows and the Prior art note are present.
- F7: the original paths, symbols, and PIN-versus-UP claims are corrected. F11 records a new UP callback error.
- F8: G8 supplies active mouse enum getters rather than mode-bit precedence.

E2 resolves the earlier OSC 1, unknown-mode, and mode-observation questions.
Patch 1 now includes the consumed-byte boundary needed for EV-8c and EV-8g.
These closures approve the audit descriptions only. They do not approve patches or binding code.
The reviewer ran no tests in this round.

## F9 — HIGH — The removed-fix mapping is incomplete and partly false

Status: CLOSED in audit revision 3. Original finding follows for the record.
Audit evidence: the Pin move section groups seven commits as page-pressure degradation and cites three upstream fixes.
The lead requires a complete per-commit mapping before the nine commits can be dropped.

`cfce1cd56` fixes replacement of an aliased current hyperlink in `startHyperlinkOnce`.
PIN already contains that fix: `Screen.zig:2639–2648` duplicates the hyperlink before `endHyperlink`.
It therefore does not establish that UP newly fixes the capacity-retry lifetime issue described by `3025fa29e`.
That fork commit owns URI and ID across the full retry loop, which is a different lifetime.
`1041a51ae` is a compile correction, rather than a separate degradation fix.
`f4d6b79c4` removes logging. `ea6550256` changes logging, implicit-ID rollback, and comments.
The grouped description does not account for those changes individually.
The audit itself states that its stress program cannot prove the crash fix.

Required change: provide one row per removed commit.
Map each material change to exact UP logic, or state why it has no effect in the configured binding.
Distinguish a root-cause fix from removal of a state-losing workaround.
Correct the claim about `cfce1cd56`.
Do not promise to restore a state-losing degradation patch after a crash; that contradicts the stated ST-6b and EV-7 rationale.
A remaining crash needs a root-cause repair or a lead decision before the pin moves.

## F10 — MEDIUM — The build proposal permits a network-dependent gate

Status: CLOSED in audit revision 3c. Original finding follows for the record.
Audit evidence: Build notes permit `build.rs` to fetch `translate_c` and defer the choice until binding implementation.
The lead requires a hash-pinned, prefetched dependency so gate builds work offline.

Required change: record the prefetch step and the cache location before the gate build.
Require the gate build to use the recorded package hash without network access.
A missing cache entry must report the missing prerequisite. It must not fetch during the gate.
Keep the existing exact `translate_c` hash in the record.

## F11 — LOW — The UP clipboard-write callback does not return a result

Status: CLOSED in audit revision 3. Original finding follows for the record.
Audit evidence: the clipboard-write row says that the callback returns a result and the binding returns success.
At UP, `terminal.h:816–870` requires `write->reply(write, &reply)` before the callback returns.
`GhosttyTerminalClipboardWriteFn` returns `void`. Returning without a reply denies the write.

Required change: describe the UP reply call and its callback-lifetime requirement.
Do not carry the PIN return-value interface into the proposed UP binding.

## F12 — HIGH — The resume rule covers the corpus but omits continuation failure

Status: REOPENED in revision 4. Revision 3c closed the original finding, which follows for the record.
Audit evidence: the ST-6b resume row sizes the continuation limit above the longest sequence in the test corpus.
ST-6b requires the invariant at every cut, including production output outside that corpus.
At UP, `stream_continuation.zig:95–97` marks tracking broken when the suffix exceeds the limit or retention fails.
The checks at lines 166–172 and 182–186 implement those failures.
The continuation APIs then return `GHOSTTY_INVALID_VALUE` for an unavailable continuation.
A corpus-size limit does not establish coverage of those paths.

Required change: describe the snapshot rule when continuation data is unavailable.
Use the ST-6b ground-state alternative, or prove a sufficient bound for every admitted sequence and address retention failure.
State which package owns the cut and pending-output handling.
Do not emit a snapshot that silently omits unfinished parser state.


## Review basis update

After the revision 2 verdict, the lead supplied plan revision 11:
`~/botster-sessions/pins/stage1-plan.555bc433.md`.
The reviewer verified SHA-256 `555bc4337fe72e9fad833fe330d43e8147a39569cb14291734f34847f56596d7`.
The contracts pin is `contracts-v0.1.1` at `366bca41da0a6de69cc1ea13b17c773cdfdb75b6` (final14 plus R-13).
This update does not change the four open findings. The next review uses these pins.


## Revision 3 review

Reviewed audit head: `f66bbcc740721db8dc017df5c4434caaa328c9a7`.
The review uses plan `555bc433` and contracts-v0.1.1.
The reviewer ran no tests.

F9 closes for the written audit. The nine rows now identify each material change.
The audit correctly states that PIN already contains `cfce1cd56` and that the retry-loop lifetime remains a separate defect.
It proposes a root-cause repair and does not restore state-losing workarounds after a crash.
This closure does not approve patch 0. Its code and test remain separate review work.

F11 closes. The audit now requires the void callback to copy borrowed data and call `write->reply` before returning.

F10 remains OPEN. The prefetch command, cache path, package hashes, and reported offline experiment improve the proposal.
However, the audit calls the entire global Zig cache read-only and identifies file existence checks as the offline control.
An existence check establishes a prerequisite. It does not itself prevent a later network request.
Required change: distinguish immutable package inputs from writable compiler artifacts.
State how the actual gate denies network access, or uses an equivalent enforced offline build mode.
Do not describe a build that can fetch as a build with no fetch step.

F12 remains OPEN. UP preflights continuation before emitting snapshot data (`c/snapshot.zig:565–567` and `:645–646`).
The proposed ground-state path preserves the resume invariant when it reaches ground.
Rule 4 adds `capture_ground_wait_bytes`, but assigns no permitted completion code when this byte bound expires.
It instead postpones the decision to a future QUESTION.
Section 9.3 permits `DeadlineExpired` for an actual operation deadline.
It permits `SnapshotTooLarge` when snapshot bytes exceed `max_snapshot_bytes`.
Neither sentence supplies a parser-byte-bound failure code.
Required change: cite and use an existing permitted rule, or remove the unsupported byte-bound policy.
If a contract decision is required, obtain it now rather than claiming COVERED and deferring the question.
Also replace the incorrect UP line-50 quote citation with the actual continuation preflight evidence above.


## Revision 3c review — CLEAN for the written audit

Reviewed head: `e9f8727da6d23ee700312d4f67eb0a5aa0b502d7`.
The reviewer checked the deltas at `765f43a1cb43eea9182649a7f237f3d5932bc577` and this head.
The review uses plan `555bc433` and contracts-v0.1.1.
The reviewer ran no tests.

F10 closes. The audit separates immutable package inputs from writable Zig artifacts.
The build proposal copies the pinned packages into the build cache and runs Zig with network access denied.
A missing package or unavailable denial wrapper fails the build.
The audit states which parts of the offline experiment were tested and which remain implementation work.
The binding review will check the actual prefetch, copy, and denial paths.

F12 closes for the written audit. The audit now states the refusal path and the ground-state alternative.
It identifies P2's library wrappers and P3's cut and pending-output duties.
It removes the unsupported byte-bound failure policy and corrects the source citation.
UP `src/terminal/snapshot/snapshot.zig:50` contains the quoted preflight comment.
UP `src/terminal/c/snapshot.zig:565–567` and `:645–646` preflight continuation before writing snapshot bytes.

The audit explicitly leaves the no-ground-state capture, baseline, and resync case open as a steward question.
It makes no completion, cancellation, bound, or failure-code claim for that case.
The lead has taken that question to the steward. Worker completion behavior remains unapproved until the answer arrives.
Recording that uncertainty closes the audit finding; it does not resolve the contract question.

All twelve review findings close for this written audit.
CLEAN does not approve a fork patch, binding code, a Ghostty pin change, or the worker's unresolved completion behavior.
The next review checks the fork patch series on exact commits.


## Initial fork patch review

The lead preserved fork head `c78b4beb4d8d5f2e09dcf60c7e42ea43af49e34c`.
The reviewer read notification commit `0be4a20a532cd7af209a93c8baf848ab6bc3fcae` and the paste commit at that head.
These reads use `git show` only. The reviewer ran no tests.
The replacement implementer will send the final patch commits separately.
The written audit remains CLEAN; the fork patch series is not approved.

## P13 — MEDIUM — The new paste test hand-writes expected terminal bytes

Status: OPEN.
Commit: `c78b4beb4d8d5f2e09dcf60c7e42ea43af49e34c`.
Evidence: new test `frame bracketed` in `src/terminal/c/paste.zig` uses literal escape sequences as expected prefix and suffix.
BUILD.md requires expected terminal bytes to come from the real libghostty oracle.
The user repeats that requirement for this review.

Required change: derive the expected frame from an existing libghostty encoder path.
For example, encode an empty payload with the existing paste encoder and compare its output with the returned frame.
Do not copy the literal assertions into the replacement stack.
Check other new tests for the same problem before requesting CLEAN on the patch series.


## Revision 4 delta review — F12 reopened

Reviewed audit head: `c1589932d6644927d4f4dbdfd32d8c474fda03d8`.
The lead supplied Core Amendment 8 candidate 1 at `16fcdad26318fe4d762fb4718b78f732eb123325` for this delta.
The candidate is under Amendment review. It is not part of the current contracts pin.
The reviewer ran no tests.

F12 reopens for two changes:

- The ST-6b table row still says the worker takes a ground-state cut after refusal.
  Rule 3 withdraws that deferral and instead applies A8-2's immediate `SnapshotTooLarge` result.
  Required change: make the table row agree with the revised rule.
- Rule 4 also maps a failed retention allocation to `SnapshotTooLarge`.
  UP `stream_continuation.zig:170–172` and `:186` mark tracking broken on allocation failure.
  This can occur below the advertised continuation limit.
  Candidate A8-2 enumerates excess snapshot size and pending state beyond what the format carries.
  It does not explicitly name allocation failure below that limit.
  Required change: obtain the steward's meaning through the lead, or mark this mapping open.
  The reviewer sent the QUESTION to the lead. The current C API does not expose the original cause of the broken tracker.

The CLEAN audit verdict remains valid only for the earlier `e9f8727` head.
P13 remains open on the preserved paste patch.

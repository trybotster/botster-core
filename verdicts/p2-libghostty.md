# P2 libghostty review

Current reviewed binding head: `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.
Reviewed Ghostty head: `3f8eb6810bb673aa782b047de21783ac81fb1121`.
The binding uses branch `botster/upstream-sync-20261002`, with upstream base `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.
The lead owns the plan pin record.

VERDICT: NOT CLEAN (2 open: P32, P37).
P25, P27, P33 and P34 close at this binding head. Lead ruling P35 closes P35 without a fork change.
P36 closes at the current binding head. P32 still requires a green Linux gate on this exact binding head.
F1–F12, P13–P24, P26 and P28–P31 remain closed. The earlier reviews below retain their stated scope and closure evidence.

## Earlier audit and fork reviews

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

Status: CLOSED in revision 4c against the steward text supplied by the lead. Original finding follows for the record.
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

Status: CLOSED at `970a1c9dfd62ab7c75ac8d1b652de2cb19cf6442`. The original finding follows for the record.
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


## Revision 4b delta review

Reviewed audit head: `c392adb3fa9aa0ae238cef2c669e44f3b136c3d9`.
The stale ground-state deferral sentence is corrected.
The audit now records retention allocation failure below the continuation limit as an open question.
The lead confirmed that candidate 1 does not explicitly cover this case and asked the steward to resolve it.
At the lead's instruction, F12 remains OPEN until that answer arrives.
P13 remains OPEN on the preserved paste patch. The reviewer ran no tests.


## Revision 4c delta review — written audit CLEAN

Reviewed audit head: `36e111f241029d43efb12be25e1179317b910d61`.
The lead instructed the reviewer to close the allocation-failure mapping against Core A8 candidate 4.
The reviewer read that candidate at `0d0bd7ea107d3383d07e4b7bf04c4bf882f18e53` with `git show`.
A8-2 now explicitly covers pending parser state the model failed to retain, including allocation failure below the limit.
The audit matches that text, withdraws ground-state deferral, and records the format limit and package duties.
The every-offset test rule rejects a within-limit refusal when no resource failure is injected.
F12 closes for the written audit. The reviewer ran no tests.

The candidate remains under Amendment review, as the audit states.
This audit verdict does not move the contracts pin or authorize implementation against an unaccepted amendment.
The lead owns the accepted pin update.

The written audit is CLEAN on this exact head. P13 remains OPEN on the preserved paste patch.
The fork series and binding code remain unapproved.


## Revision 4d delta review — written audit CLEAN

Reviewed audit head: `908b9b79fc625b22b885f23a8880e5e749c16d5a`.
The lead supplied Core A8 candidate 5 at `7bf8d0dffa2f81fa1570f3d804e4163141e6654e`.
The reviewer read the candidate and the audit delta.
The audit correctly constrains the every-offset corpus so that each snapshot fits `max_snapshot_bytes`.
With no injected resource failure, only pending state beyond the stated continuation limit may cause refusal in that test.
Ordinary snapshot size refusal has separate OU-9 coverage.
No new finding. F12 stays closed for the written audit; P13 stays open for the preserved paste patch.
The candidate remains pending acceptance. No pin moved. The reviewer ran no tests.


## Replacement fork review — patches 2–3 CLEAN

The reviewer checked both complete diffs and the relevant source with `git show` only.
The reviewer ran no tests.

- Patch 2, notification source: `7afa387ddc8b0ecd7b1e1f13892899165719304a`.
- Patch 3, paste frame: `970a1c9dfd62ab7c75ac8d1b652de2cb19cf6442`.

Patch 2 carries the OSC 9 or OSC 777 source through parsing, stream dispatch, and the C callback.
The C enum values match the Zig enum values. The callback supplies the extended structure size.
No open finding applies to this patch.

Patch 3 returns static markers from the existing paste encoder without changing payload bytes.
Its new tests compare the frame with output from the existing encoder for empty and safe payloads.
The unbracketed test checks empty markers and the existing encoder's empty output.
The replacement tests contain no hand-written expected terminal bytes. P13 closes.

VERDICT: CLEAN for patches 2–3 on these exact commits.
Patches 0, 1, and 4–8 remain outside this verdict.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Revision 5 delta review — accepted Core A8

Reviewed audit head: `36d82ecb48316087e347e42c16a2bd9176439506`.
The lead instructed the reviewer to check A8-2 against the frozen file in manifest final16.
The reviewer read `frozen/current/core-contract-v1.17-amendment-8-candidate5.md` at contracts commit `55171091b91be9a169182882993ac38199b0015a`.
Its SHA-256 is `c6918a9bf19d739422abc775441a9aa42811bbe044f1a18cd39f20215e2a8006`.
That hash matches manifest final16 and the previously reviewed candidate 5.

The delta changes only the ST-6b row and cut rules 3 and 4.
Those entries now cite the accepted amendment and its frozen file.
The rules still cover both continuation overflow and failed retention below the limit.
Captures complete with `SnapshotTooLarge`; baselines and resyncs close the route at once without partial snapshots.
The format description states the continuation limit. The worker does not wait for ground state.
The every-offset corpus keeps snapshots within `max_snapshot_bytes` and injects no resource failure.
It permits refusal only when pending state exceeds the stated continuation limit.

VERDICT: CLEAN for the written audit on this exact head. F12 stays closed.
The reviewer ran no tests. No dependency pin changed.
The existing CLEAN verdict for fork patches 2–3 stays unchanged.
The full fork series and binding code remain unapproved.


## Replacement fork review — patch 0 CLEAN

Reviewed fork commit: `ea5a1e2975aa6c999051cb6fb30685ececf8be51`.
Its parent is the previously reviewed paste commit `970a1c9dfd62ab7c75ac8d1b652de2cb19cf6442`.
The reviewer read the complete diff and the relevant source with `git show` only.
The reviewer ran no tests.

`startHyperlink` now copies the URI and explicit ID before it starts the capacity retry loop.
Each retry reads those owned copies after `startHyperlinkOnce` frees the previous cursor hyperlink.
The function frees both copies when it returns, including allocation and capacity errors.
The change preserves implicit ID creation and rollback.
It adds no retry bound, silent degradation, or change to capacity growth.
This is the root-cause lifetime repair specified by F9 row 1.

The new test restarts a hyperlink from slices of its current URI and explicit ID.
Each restart writes a cell and checks the retained values.
The test requires at least one string-capacity increase during those restarts.
The implementer reports SIGABRT without the copy and a pass with the copy.
The reviewer did not independently run those commands.
This evidence addresses the lifetime defect; it does not reproduce the production crash.

VERDICT: CLEAN for patch 0 on this exact commit. No new finding.
The existing CLEAN verdicts for patches 2–3 and the written audit remain unchanged.
Patches 1 and 4–8 remain outside this verdict.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Revision 5b delta review — P2 owns the A8-2 resume invariant

Reviewed audit head: `89afa037b198cb26173ff520245adb926b6ca21e`.
The complete delta adds one entry to the ST-6b test list.
The reviewer checked plan revision 12 and the regenerated P2 clause list.
The plan hash is `00f63f578ceac79d952b72ccb17a8d263f297a09331d7fb6dcc911db1afce4d2`.
The reviewer also read BUILD.md and ruling R-14 at `contracts-v0.1.3`.

Plan section 6.1 assigns `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant` to P2.
The added audit entry records that assignment and the property fixed by A8-2.
The adjacent corpus rule still checks exact restoration of each offered snapshot.
The planned dependency move uses `contracts-v0.1.3` in a separate commit, as plan section 0 requires.
This audit commit does not change the dependency pin.

VERDICT: CLEAN for the written audit on this exact head. No new finding.
The reviewer ran no tests. The fork verdicts remain unchanged.
The full fork series and binding code remain unapproved.


## Replacement fork review — patch 1 NOT CLEAN

Reviewed commit: `6495721bb0496b4de561eb5b377cacd50987de0e`.
The reviewer read every changed file and relevant parser, stream, and contract source.
All Ghostty reads used `git show`. The reviewer ran no tests.
The written audit and patches 0, 2, and 3 retain their scoped CLEAN verdicts.

## P14 — HIGH — Request tracking follows ground state instead of query boundaries

Status: CLOSED at `72902b1b738525d7fdf2c6ca828e2a40201f4f50`. The original finding and delta reviews follow for the record.
Evidence: `src/terminal/c/terminal.zig:1092–1159` clears request bytes only after ground state or a bulk text feed.
The parser can start a new sequence without passing through ground state.
`src/terminal/parse_table.zig` sends ESC from every state to escape state.
`src/terminal/stream.zig:1255` also executes ENQ inside CSI parameters without ending the CSI.

Examples of input stimuli:

- `ESC [ 3 ESC [ 5 n`: the second ESC abandons the first CSI.
  The callback reports the abandoned CSI prefix together with the operating-status query.
- `ESC [ 3 ENQ`: the callback reports the CSI prefix together with ENQ, instead of the ENQ request alone.
  The pending CSI must still remain valid after the ENQ query.
- A string query ended by ESC followed by a new CSI keeps the previous string in `query_raw`.
  On the next call, `query_raw_open` preserves that buffer because the parser remains in escape state.
  The next query therefore includes the previous query's bytes.

EV-8(c) requires the exact request bytes. The C API also promises each query's first through final byte.
These paths can report wrong bytes and can mark a short query as truncated because an abandoned prefix used its limit.

Required change: track actual query boundaries, including parser restarts and controls executed inside unfinished sequences.
Preserve pending parser state when an independent control query occurs.
Test restart, embedded ENQ, and string-to-CSI transitions across call boundaries.
Compare reported requests with the relevant input slices.

## P15 — HIGH — Two normative window queries are ignored

Status: CLOSED at `38599d3209beb7bdc8c8ffcde2f8af414a21f202`. The original finding follows for the record.
Evidence: `src/terminal/stream.zig:2424` accepts CSI 14 t only with one parameter.
The new arm at line 2452 accepts CSI 13 t only with one parameter.
The EV-8 normative table also names `CSI 14 ; 2 t` as `WindowPixels`.
It names `CSI 13 ; 2 t` as `WindowPosition{area: TextArea}`.
Both sequences have two parameters, so neither reaches the query callback or stops the write.

Required change: recognize both normative forms inside libghostty.
Expose enough semantic information to distinguish window pixels from text-area pixels and the two position areas.
Do not require the Rust binding to parse request bytes to recover those distinctions.
Add tests for both forms, exact request slices, and the stop before later output.
Reject extra parameters that are outside the supported forms.

## P16 — MEDIUM — A failed request allocation can produce bytes with a gap

Status: CLOSED at `38599d3209beb7bdc8c8ffcde2f8af414a21f202`. The original finding follows for the record.
Evidence: `src/terminal/c/terminal.zig:1050–1058` sets `query_raw_truncated` when append fails, but later bytes still call append.
If a later allocation succeeds, the buffer contains a prefix followed by later bytes, with the failed byte missing.
The callback still marks the request available.
The header says a truncated request contains the first bytes only. This buffer does not satisfy that promise.

Required change: stop retaining later bytes after the first failed append, or use explicit unavailable/error handling.
Never present a buffer with missing interior bytes as the request or its prefix.
Add a focused allocation-failure test that checks the callback's bytes and flags after a later allocation can succeed.

VERDICT: NOT CLEAN (3 open findings, P14–P16).
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 1 delta review — P15 and P16 closed; P14 remains open

Reviewed commit: `38599d3209beb7bdc8c8ffcde2f8af414a21f202`.
The reviewer read the complete delta and relevant stream and parser logic with `git show`.
The reviewer ran no tests.

P15 closes. CSI 14;2 t and CSI 13;2 t have distinct query kinds.
The parser accepts the normative parameter forms and rejects the tested extra-parameter forms.
The C enum values match the Zig enum values. Neither new kind has a library reply.

P16 closes. `QueryRaw.append` stops retaining bytes after its first failure or limit overflow.
Its retained bytes stay a contiguous prefix.
The allocation test makes a growth fail, restores allocation, and checks that later appends do not alter the prefix.

The ESC restart and shared string-ending ESC cases of P14 now have correct boundary resets.
The callback reports an embedded ENQ alone and preserves the unfinished parser state.
Two P14 matters remain:

- **C1 restart with equal states:** `src/terminal/c/terminal.zig:1220` resets a C1 request only when `after != old`.
  A CSI introducer, byte `0x9B`, starts a CSI even when the parser is already in `csi_entry`.
  Input `ESC [ <0x9B> 5 n` leaves the parser in `csi_entry` at that introducer.
  The condition keeps the abandoned `ESC [` prefix in the operating-status request.
  The parser's anywhere transition to `csi_entry` does not require a different old state.
  Required change: track this restart without using state inequality as the sole test.
  Add a test that compares the request with the input slice starting at the C1 introducer.
- **R-17 and executed C0 controls:** the lead relayed steward ruling R-17 during this review.
  The query contains parser-assembled bytes and excludes C0 controls executed inside it.
  The ENQ design therefore matches the ruling, and the ENQ interpretation question closes.
  The same rule also applies to other executed C0 controls, such as BEL inside CSI.
  The delta special-cases ENQ only; it still appends BEL to the outer request.
  Required change: exclude executed C0 controls consistently without changing their terminal effects.
  Keep every byte in raw Output, as OU-12 requires.
  Correct the C header's contiguous-span promise to match R-17.
  Add a test for an executed control other than ENQ inside a pending query.

## P17 — LOW — One new request assertion copies expected escape bytes

Status: CLOSED at `b60d005420f1eda6a932c509b257f8300994b34b`. The original finding follows for the record.
The new test `vt_write_until_query does not count an abandoned prefix against the limit` compares with literal `"\x1b[5n"`.
That literal duplicates bytes already present in the input.
BUILD.md forbids hand-written expected terminal bytes. P14 also required comparison with relevant input slices.
Required change: compare with the second query's slice of `input`, as the adjacent restart test already does.

VERDICT: NOT CLEAN (2 open findings, P14 and P17).
The written audit and patches 0, 2, and 3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 1 delta review — P17 closed; P14 remains open for string-to-C1 transitions

Reviewed commit: `b60d005420f1eda6a932c509b257f8300994b34b`.
The reviewer read the complete delta and relevant parser transitions with `git show`.
The reviewer ran no tests.

P17 closes. The assertion now compares the reported request with the second query's input slice.
The R-17 part of P14 closes. The delta excludes independent C0 controls from unfinished non-string requests while preserving their effects.
The header and Zig documentation now describe the recognized sequence and the C0 exclusion.
The tests check the two ENQ/CSI queries in order and the BEL effect with parser-assembled request bytes.
The equal-state C1 CSI restart now clears the abandoned prefix.

**P14 remains OPEN:** the replacement condition at `src/terminal/c/terminal.zig:1234` requires `!stringState(old)` for every C1 restart.
The parser's anywhere transitions can start a new CSI or DCS from a string state.
For example, input `ESC ] 0 ; title <0x9B> 5 n` changes `osc_string` to `csi_entry` at the C1 CSI introducer.
The condition does not clear the old request because the old state is a string state.
The operating-status callback then includes the preceding OSC bytes in its request.
The same issue applies across calls and when the old string itself reports a query before the new sequence.
The preceding revision handled changed-state C1 transitions; the new guard removes that coverage.

Required change: handle a C1 transition from a string state into a different sequence.
Keep the equal-state non-string restart fix.
Preserve any callback for the completed old string before starting the new request buffer.
Add string-to-C1 tests, including a call boundary, that compare each request with its relevant input bytes.

VERDICT: NOT CLEAN (1 open finding, P14). P15–P17 are closed.
The written audit and patches 0, 2, and 3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 1 evidence correction — P14 narrows to APC-to-C1 transitions

Reviewed commit: `56b54e92365fe94c512804e8ffe44ea93f1cb550`.
The reviewer read the new test and the complete relevant parse-table overrides.
The reviewer also checked the scalar dispatch and the APC bulk path.
All Ghostty reads used `git show`. The reviewer ran no tests.

**Correction:** the previous review missed the later OSC and DCS overrides in `parse_table.zig`.
OSC high bytes are payload. DCS passthrough high bytes are payload, and DCS ignore high bytes are ignored.
Those states override the initial anywhere C1 transitions.
The OSC and DCS examples in the previous P14 delta review are withdrawn.
The new OSC test correctly records the pinned parser's behavior and compares the later CSI request with its input.

**P14 remains OPEN for APC.** `sos_pm_apc_string` has no corresponding high-byte override.
`src/terminal/parse_table.zig:78` sends byte `0x9B` from that state to `csi_entry`.
The APC section at lines 111–121 does not replace that entry.
`src/terminal/stream.zig` also states that most C1 bytes exit APC in `consumeApcString`.
Its existing test `stream: apc bulk slice C1 ST` confirms that APC does honor a C1 transition.
The scalar path used by the query writer applies `Parser.next` to this byte.

Input `ESC _ Gpayload <0x9B> 5 n` therefore ends the APC and starts a CSI.
The condition at `src/terminal/c/terminal.zig:1234` still rejects the reset because `stringState(old)` includes `sos_pm_apc_string`.
The operating-status callback retains the APC prefix in its request.
This defect also applies when a call ends just after the C1 introducer.

Required change: reset the request on the actual APC-to-C1 transition.
Preserve OSC and DCS high bytes as their parser tables require.
Keep the equal-state CSI restart fix.
Add an APC-to-C1 query test across a call boundary and compare with the input slice starting at the C1 introducer.

VERDICT: NOT CLEAN (1 open finding, P14, limited to APC-to-C1 transitions).
P15–P17 remain closed. The written audit and patches 0, 2, and 3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 1 final delta review — P14 closed

Reviewed commit: `72902b1b738525d7fdf2c6ca828e2a40201f4f50`.
The reviewer read the complete delta with `git show`. The reviewer ran no tests.

The change handles C1 introducers in `sos_pm_apc_string` separately.
When the parser enters a different sequence, the request buffer starts at that introducer and drops the APC prefix.
The change preserves OSC and DCS high-byte behavior and equal-state CSI restarts.
The test checks the operating-status request against its input bytes across calls and in one call.
P14 closes. P15–P17 remain closed.

VERDICT: CLEAN for patch 1 through this exact head.
This verdict covers the initial patch and its reviewed deltas:

- `6495721bb0496b4de561eb5b377cacd50987de0e`.
- `38599d3209beb7bdc8c8ffcde2f8af414a21f202`.
- `b60d005420f1eda6a932c509b257f8300994b34b`.
- `56b54e92365fe94c512804e8ffe44ea93f1cb550`.
- `72902b1b738525d7fdf2c6ca828e2a40201f4f50`.

The written audit and patches 0, 2, and 3 retain their CLEAN verdicts.
Patches 4–8 remain outside this verdict.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Replacement fork review — patch 4 NOT CLEAN

Reviewed commit: `50569efc8806ba3e5411ca5e556a7168a1c82259`.
The reviewer read every changed file, relevant encoder logic, and Core 5.1A.
All Ghostty reads used `git show`. The reviewer ran no tests.

The new modifier bits preserve earlier C bit values. F26–F35 preserve earlier key enum values.
The C event uses supplied alternate keys; the Zig default still derives alternate keys.
The associated-text path requires flags 8 and 16 and omits text containing C0, DEL, or C1.
The change keeps `utf8` available to the other encoder paths.
Validation of `shifted_key` without Shift can remain in P3: it is input validation, not byte encoding.
The legacy Shift rule cannot remain there for the reason below.

## P18 — MEDIUM — New key tests hand-write expected terminal bytes

Status: CLOSED at `3f28780d2af7d8fea8717b2f9ccba0cfbafecc26`. The original finding and delta reviews follow for the record.
Evidence: the new tests in `src/input/key_encode.zig` use literal expected kitty sequences.
Examples include `kitty: hyper and meta modifiers`, `kitty: provided alternate keys are reported exactly and never derived`, and `kitty: function keys f26 to f35`.
BUILD.md forbids hand-written expected terminal bytes. The user repeats that rule for this review.
Existing upstream tests do not give new fork tests an exception.
The implementer's message explicitly identifies this exception, but no authorized rule permits it.

Required change: replace new literal byte expectations with comparisons against real native encoder paths or semantic property checks.
For example, the existing native sequence encoder can encode a structured expected key sequence as an oracle.
Keep checks for the supplied fields, modifier values, flag combinations, and whole-text omission.
The test that claims legacy text remains available currently uses plain `a`, not one of the control-containing texts.
Check preservation with the same text that triggers associated-text omission.
Do not copy old expected-byte tests into new tests.

## P19 — HIGH — Legacy Shift encoding is an unresolved library GAP

Status: CLOSED at fork `22035f7c2216b84caa7cf8e763affaca19d1ab8e` and audit `d2cce61b19d724ddc657f17fa8dee26dff409997`. The original finding follows for the record.
Core 5.1A rule 1 defines Shift with no text: use supplied `shifted_key`, else uppercase an ASCII letter, else typed zero `produced_text`.
The implementer proposes leaving this rule to P3 mapping.
BUILD.md assigns terminal input encoding to libghostty and forbids encoding outside it.

At this commit, `legacy` never reads the new supplied alternate fields.
A printable event with `.key = .key_a`, unshifted `a`, Shift, supplied shifted `A`, and empty `utf8` produces zero bytes.
It reaches the empty-text path, where `legacyAltPrefix` returns false without Alt.
The same event without the supplied alternate also produces zero instead of the required ASCII uppercase fallback.
The new API therefore does not supply the contractual legacy rule.
The audit's legacy key row claims COVERED and does not record this GAP. A missed GAP is a finding.

Required change: record this GAP in the written audit and its patch proposal.
Implement the legacy Shift rule in libghostty, with a C path that carries the caller's supplied fields without synthetic terminal semantics in Rust.
Preserve supplied text and the distinct kitty associated-text rules.
Return zero for the unsupported no-layout-guess case so the binding can map the typed `produced_text` result.
Add native tests for supplied Shift characters, ASCII fallback, and the unsupported non-ASCII case.
Use native encoder oracles or semantic properties rather than literal expected terminal bytes.

VERDICT: NOT CLEAN (2 open findings, P18–P19).
Patches 0–3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 4 delta review — P19 closed; P18 remains open

Reviewed fork delta: `22035f7c2216b84caa7cf8e763affaca19d1ab8e`.
Reviewed audit delta: `d2cce61b19d724ddc657f17fa8dee26dff409997`.
The reviewer read both complete deltas. All Ghostty reads used `git show`.
The reviewer ran no tests.

P19 closes. The audit adds G12 and a dedicated legacy printable-key row.
The native legacy encoder now handles Shift with no text for events with supplied alternates.
It uses the supplied shifted character, then ASCII uppercase fallback, then zero for the no-layout-guess case.
Ctrl and Alt retain the base-character path. The derived Zig event mode stays unchanged.
The generated text re-enters the native legacy encoder, so Rust does not encode this rule.
The tests cover supplied characters, ASCII fallback, unsupported characters, and unchanged Alt and derived behavior.

The new kitty tests now use the existing structured `KittySequence.encode` path as their oracle.
The preservation test also uses the actual C0-, DEL-, and C1-containing text.
Those parts of P18 close.

**P18 remains OPEN:** the new test `legacy: shift with no text keeps the alt and derive behavior` still uses a literal expected byte array.
It compares the output with `&[_]u8{ 0x1B, 'a' }`, which constructs the expected Alt encoding by hand.
Required change: compare with a real legacy encoder path for the unchanged base-character case.
For example, encode the corresponding derived event with the same modes and compare the two native results.
Keep the supplied shifted alternate on the actual event so the test checks that Alt retains the base-character rule.

VERDICT: NOT CLEAN (1 open finding, P18).
The written audit is CLEAN on the exact audit head above.
Patches 0–3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 4 final delta review — P18 closed

Reviewed commit: `3f28780d2af7d8fea8717b2f9ccba0cfbafecc26`.
The reviewer read the complete delta with `git show`. The reviewer ran no tests.

The Alt test now obtains its expected result from the real legacy encoder.
The reference event uses the base character with Alt and the same encoder options.
The actual event keeps Shift and the supplied shifted alternate.
The test requires a nonempty reference result and compares the native outputs.
It no longer constructs expected terminal bytes by hand. P18 closes. P19 remains closed.

VERDICT: CLEAN for patch 4 through this exact head.
This verdict covers these reviewed commits:

- `50569efc8806ba3e5411ca5e556a7168a1c82259`.
- `22035f7c2216b84caa7cf8e763affaca19d1ab8e`.
- `3f28780d2af7d8fea8717b2f9ccba0cfbafecc26`.

The written audit and patches 0–3 retain their CLEAN verdicts.
Patches 5–8 remain outside this verdict.
The full fork series, binding code, and Ghostty pin change remain unapproved.


## Patch 5 review — terminfo exports

Reviewed commit: `da42a8ac0d9f99ece0adf534b375991df5de489b`.
The reviewer read the complete delta and the native source encoder with `git show`.
The reviewer ran no tests.

The C API returns the first name and encoded source of the native Ghostty entry.
The source uses `terminfo.ghostty.encode` at compile time. The returned strings have static storage and a NUL after their stated length.
The header, C exports, and aggregate header agree.
The source test compares the result with the existing encoder at run time. It does not construct expected terminal bytes.
The name test checks the contract identity, `xterm-ghostty`.

The entry and encoder are unchanged at PIN, UP, and the reviewed commit. Their SHA-256 values are:

- `src/terminfo/ghostty.zig`: `75e9b0d0b4e5cff4b050a0abbd7f7ae5b87095a5c75c11aab1967ce83cda6b9d`.
- `src/terminfo/Source.zig`: `d928b56daed3d0863aad849842b49fffc510a408c4dc8d66e1e3139f39c9aa52`.

VERDICT: CLEAN for patch 5 at this exact head.
This verdict covers the native exports for TI-1 and A2-8. The binding must still verify installation with `tic`.
The written audit and patches 0–4 retain their CLEAN verdicts.
Patches 6–8, the binding code, and the Ghostty pin change remain outside this verdict.


## Patch 6 review — supplied mouse cells and active enums

Reviewed commit: `b59b1f47b9684319a3d20167c98c0f9976df4a00`.
The reviewer read the complete delta, the full encoder path, and the C wrapper with `git show`.
The review checked IN-9, section 5.1A, ST-4, and ruling R-13. The reviewer ran no tests.

The cell API uses an external struct with two `u32` fields. The header, exports, and schema declaration agree.
For cell formats, the native encoder uses the supplied cell without pixel conversion, viewport refusal, or clamping.
The encoder keeps the existing tracking rules and button encoding.
X10 rejects cells above 222. UTF-8 rejects cells above 2014 before it writes any bytes.
SGR and URXVT add one in `u64`, so a maximum `u32` cell does not overflow.
Motion deduplication uses the supplied cell. Columns beyond the stored coordinate range are not truncated for tracking.
SGR-pixels ignores the supplied cell and retains the native pixel path without an added offset, as R-13 requires.
The C wrapper retains its output-space recovery and restores motion state after a failed write.

The two terminal getters read `t.flags.mouse_event` and `t.flags.mouse_format`, which the native encoder also reads.
They do not derive the active enums from individual mode bits. Their header values and output types match the implementation.
The new tests use the real pixel encoder as the output oracle. They also check limits, cell tracking, and active mode histories.
The tests do not construct expected terminal bytes by hand.

VERDICT: CLEAN for patch 6 at this exact head.
The written audit and patches 0–5 retain their CLEAN verdicts.
Patches 7–8, the binding code, and the Ghostty pin change remain outside this verdict.


## Patch 7 review — typed query replies

Reviewed commit: `05540bd6906163ae3d7599071deb08b26829a16d`.
The reviewer read the complete delta, the enum implementation, and the relevant native encoders with `git show`.
The review checked EV-8 and BUILD.md at `contracts-v0.1.3`. The reviewer ran no tests.

The reply forms match the EV-8 table for valid unsigned position values.
The encoder checks UTF-8 text and refuses control codepoints. It checks clipboard selection characters.
The base64 chunks use a multiple of three input bytes, so only the final chunk can add padding.
The output-space path counts the complete reply with the same encoder.
Three findings remain open.

### P20 — MEDIUM — New tests construct expected terminal bytes

Status: OPEN.
Evidence: `src/terminal/c/query_reply.zig` tests contain literal expected CSI 9 and CSI 3 replies.
The title, icon, and clipboard tests also construct expected OSC prefixes and terminators by hand.
BUILD.md states: "Expected terminal bytes are never hand-written." The absence of an existing encoder does not create an exception.

Required change: remove all newly handwritten terminal-byte expectations from these tests.
Use existing native encoders or the native parser to check structured results.
Keep the base64 round-trip, text validation, and buffer-size checks.

### P21 — MEDIUM — Bad enum values bypass the documented refusal

Status: OPEN.
Evidence: `encode` validates the pointer and struct size, then calls `encodeReply` without validating `reply.kind`.
`encodeReply` uses an exhaustive switch. The clipboard branch also uses an exhaustive switch on `reply.terminator`.
Both enums come from `lib.Enum`, which constructs exhaustive enum types.
A C caller can supply an integer outside either enum. Those values cannot reach a documented `GHOSTTY_INVALID_VALUE` return.
An invalid clipboard terminator reaches its switch after the encoder writes the prefix and payload.
The header promises `GHOSTTY_INVALID_VALUE` for a bad kind or field.

Required change: validate the reply kind before the encoder uses it.
Validate the terminator for clipboard replies before the encoder writes bytes.
Return `GHOSTTY_INVALID_VALUE` for unknown values in both cases.
Add focused argument checks for values outside the enum sets.

### P22 — MEDIUM — Position fields use signed coordinates

Status: OPEN.
Evidence: the new header defines `x` and `y` as `int32_t`. The Zig reply uses `i32`.
The new position test requires a negative decimal coordinate in the reply.
EV-8 states: "Position{x, y} are the unsigned wire values (u16, 0 to 65,535)".
The client maps negative coordinates before it sends the typed reply. Core applies no conversion.
The new API and test therefore describe a different position model from the clause that this patch implements.

Required change: use the EV-8 unsigned wire-value model for this typed reply API.
Update the header and schema declaration together.
Replace the negative-coordinate expectation with checks for valid unsigned values, including the upper range.
Use a native oracle as P20 requires.

VERDICT: NOT CLEAN for patch 7 (P20–P22 remain open).
The written audit and patches 0–6 retain their CLEAN verdicts.
Patch 8, the binding code, and the Ghostty pin change remain outside this verdict.


## Patch 7 delta review — P20–P22 closed

Reviewed commit: `96b4f3f4f8024db6611afd8565f64ad33e30dd0e`.
The reviewer read the complete delta with `git show`. The reviewer ran no tests.

P20 closes. The new tests obtain structured results from the native parser and compare native encoders where available.
They no longer construct expected terminal bytes by hand. The two-character selection check compares two real encoder outputs.
P21 closes. The C entry point reads enums and the bool through an integer layout before it constructs typed values.
Compile-time checks verify the size, alignment, and every field offset. Unknown enum values and invalid bool values return `GHOSTTY_INVALID_VALUE`.
The checks occur before output.
P22 closes. The header and Zig struct now use unsigned 16-bit position values.
The position test checks the upper range through the native parser.

VERDICT: CLEAN for patch 7 through this exact head.
Patches 0–6 retain their CLEAN verdicts.

## Patch 8 review — clipboard selection coverage

Reviewed commit: `d27593e55d2e09e5af61ae358ff63d7620140f34`.
The reviewer read the complete delta and the parser-to-callback path with `git show`.
The reviewer also checked the parser at PIN and UP. The reviewer ran no tests.

The new fields preserve a single selection character and its terminator through the native action and the synchronous C callbacks.
The fields follow the existing sized structs. Other protocols retain empty selection and ST defaults.
The tests cover single characters, omitted selection, and both terminators. One finding remains open.

### P23 — HIGH — Clipboard selections with multiple characters are still dropped

Status: OPEN. This finding also identifies a missed GAP in the written audit at `d2cce61b19d724ddc657f17fa8dee26dff409997`.
Evidence: `src/terminal/osc/parsers/clipboard_operation.zig` requires `data[1] == ';'` for a nonempty selection.
For selection `s0`, `data[1]` is `0`, so the parser returns null and reports no command.
The same restriction exists at PIN and UP. Patch 8 only copies `data[0..1]` into the new selection field.
Thus an OSC 52 read with selection `s0` produces no query or clipboard-read callback.
An OSC 52 write with that selection produces no clipboard-write callback.
The new typed reply encoder already accepts selections with multiple characters, but the request parser cannot reach that path.

EV-8 defines `selection` as the program's selection string from the allowed character set. It also names `s0` as the default.
EV-3 requires clipboard writes to surface. EV-7 makes a missing clipboard observation a conformance failure.
The audit's G10 rows and patch proposal do not record this parser restriction.
The review missed this restriction earlier. This finding corrects that review omission.

Required change: parse the whole selection string up to its separator in libghostty.
Carry the whole selection string through the existing new fields without truncation or replacement.
Retain the omitted-selection representation and its contract mapping.
Add focused native checks for reads and writes with selections such as `s0` and `cp`, using both terminators.
Check that the query and clipboard callback each occur once for a read.
Check that the clipboard-write callback preserves the selection and decoded payload.
Update the written audit to record the restriction at PIN and UP and the patch that closes it.
Rust must not parse OSC 52 to bypass this GAP.

VERDICT: NOT CLEAN for patch 8 and the written audit (P23 remains open).
Patches 0–7 retain their CLEAN verdicts.
The binding code and the Ghostty pin change remain outside this verdict.


## Patch 8 delta and audit review — P23 closed

Reviewed fork commit: `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`.
Reviewed audit commit: `52b3d866da11cf7c1b9b89bb5d68089be6b2e834`.
The reviewer read both complete deltas. All Ghostty reads used `git show`. The reviewer ran no tests.

The native OSC 52 parser now finds the first selection separator and preserves the whole preceding string.
The parser excludes the final NUL from its separator search and retains the data slice's NUL sentinel.
An omitted selection remains empty. Its first-character destination mapping remains `c`.
A request without the separator remains invalid.
The existing action and callback fields carry the whole borrowed selection during the synchronous callback.
The read path still calls the query effect once before the clipboard-read effect.
The new tests cover `s0`, `cp`, and all allowed selection characters together, with both terminators.
They check one clipboard callback, the full selection, the terminator, and the decoded write payload.
The tests use literal request stimuli and structured callback results, rather than handwritten expected terminal bytes.

The audit updates the G10 summary, both clipboard rows, and the patch 8 proposal.
Those rows now record the parser restriction at PIN and UP and the native fix. P23 closes in both the fork and audit.
The terminfo entry and encoder hashes at this fork head still match the PIN and UP hashes recorded under patch 5.

VERDICT: CLEAN for patch 8 through this exact fork head and the written audit at the exact audit head above.
Patches 0–7 retain their CLEAN verdicts. All recorded findings are closed.
This verdict does not approve the Rust binding, the final pin change, or the full P2 package.


## Binding review — P24–P32 open

Reviewed Core head: `43432744e0db3f48494828ab5cca9d4cd7f94ea9`.
The reviewer read the complete production binding and its tests, including the delta from `977d986`.
The audit at rebased commit `bad4f47ea23e168d518f545e07d652796dc7d5e2` equals the previously reviewed audit at `52b3d866`.

Binding inputs:

- Contracts: `contracts-v0.1.9`, commit `7f72acf8427ad7bf414db42d2360d5dccc7b3e13`, manifest final22.
- Plan revision 19: `stage1-plan.62f664de.md`, SHA-256 `62f664de2476bc172336a9ddac7b111517a9fc49f34eb59b69a8d11a24c0bea8`.
- Ghostty: `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`.
- Clipboard: Core Amendment 13 candidate 5, frozen in manifest final30 at contracts commit `627d507`, supplied by the lead for this review.

The reviewer used `git show` for all Ghostty evidence. The reviewer ran no tests.
Source logic proves these findings. Every finding requires closure before CLEAN.

### Build scope — P32 remains open

`build.rs` and `build_data.rs` match the reviewed package list and Zig 0.16.0 pin.
The build copies package archives into caches under `OUT_DIR` and denies network access during the native build.
Missing prerequisites stop the build with a named prerequisite. The native library is unconditional.
The initial source review found no build defect. The later Linux evidence establishes P32 below.
This review does not approve the remote fetch configuration or the full package.

### P24 — HIGH — The model still holds graphics that its snapshot loses

**Evidence:** `lib.rs::Terminal::new` never sets `GHOSTTY_TERMINAL_OPT_KITTY_IMAGE_STORAGE_LIMIT` to zero.
`sys.rs::opt` does not declare that option. The build enables the default native features.
At the Ghostty pin, `build_options.zig::Features.kitty_graphics` defaults to true.
`kitty/graphics_storage.zig::ImageStorage.total_limit` defaults to 320,000,000 bytes. Its `enabled` method tests whether that limit is nonzero.
The audit's H1 resolution requires a zero limit before the model receives output because snapshots omit images and placements.
The current binding therefore breaks its reviewed ST-6b configuration premise.

**Required change:** set the native image storage limit to zero before any write.
Confirm that both screens retain that configuration. Keep `snapshot_graphics` absent in the worker feature list.
Add the audit's native-state test for a kitty image. Use native state and snapshot results as the oracle.

### P25 — HIGH — Clipboard events cannot satisfy Amendment 13

**Evidence:** `events.rs::on_clipboard_write` ignores `request.location` and copies only `contents[0].data`.
`TerminalEvent::ClipboardWrite` has only `selection: Option<String>` and `bytes: Vec<u8>`.
It loses MIME types and all later representations. It also merges a clear request with one empty value.
The callback always replies SUCCESS before the worker can apply its clipboard size bound.
The worker cannot produce the selection, atomic contents, total size, or TooLarge acknowledgement that A13-1 requires.

**Required change:** expose the native location, exact selection, and every `{mime, bytes}` representation in order.
Preserve the distinction between zero entries and one empty entry.
Provide synchronous worker size admission inside the native callback so it can choose SUCCESS or IO_ERROR before return.
Do not retain the native request pointer after the callback. Do not perform I/O or call host code inside the callback.
Return the native acknowledgement bytes to the worker as a value. The worker writes them as one contiguous AM-2 transaction (A13-1b).
Update the audit and tests against frozen A13 candidate 5. Remove the withdrawn erratum 5 reference.

### P26 — HIGH — The mode probe loses modifyOtherKeys state under kitty flags

**Evidence:** `encode.rs::modify_other_keys_state_2` compares three encodings with state 2 disabled.
At the Ghostty pin, `src/input/key_encode.zig::encode` selects `kitty()` whenever `kitty_flags.int() != 0`.
The three probe events enter that path. `kitty()` does not read `modify_other_keys_state_2`.
Only the legacy path reads that field. Thus both probe encodings match while kitty flags mask a tracked, active state 2.
`modes.rs::mode_flags` then reports `xterm_modify_other_keys_2 = false` although the native terminal retains true.
E2-2 and EV-7 require the tracked state, including state that has no current effect on key encoding.
The audit omits this missing state getter.

**Required change:** expose the actual native state through a getter that works with every kitty flag combination.
Record this GAP in the audit. Follow Q1 for any native patch and pin change.
Do not track the sequence in Rust. Add a regression that enables state 2, enables kitty flags, and checks the retained state.

### P27 — HIGH — SGR pixel coordinates lose precision or exceed the native integer range

**Evidence:** `encode.rs::encode_mouse` casts the contract's `u32` pixel coordinates to `f32` without a representation check.
The value 16,777,217 becomes 16,777,216 before the native encoder sees it.
At the pin, `src/input/mouse_encode.zig::posToPixels` rounds that value into an `i32`.
Coordinates above the `i32` range can also reach that conversion. Native release events bypass the viewport refusal.
The supplied cell does not protect this path: SGR-pixels explicitly ignores it.
5.1A requires an unrepresentable coordinate to produce typed zero, rather than a changed coordinate.
The audit's G8 coverage omits these representation limits.

**Required change:** preserve exact pixels through a native integer API, or refuse values that the pinned API cannot represent exactly.
Use `Unsupported{what: coordinate}` for a representation refusal. Do not encode protocol bytes in Rust.
Update the audit. Add native-oracle tests around the `f32` precision boundary and the native integer boundary, including releases.

### P28 — MEDIUM — A required unsupported named key returns NotReported

**Evidence:** `encode.rs::encode_key` maps a native empty result to NotReported except for the ProducedText case.
For a modifier key with kitty flag 8 off, the native encoder returns no bytes.
R-14.1 expressly requires `Unsupported{what: named_key}` for all eight named modifier keys in that case.
The current binding returns NotReported. The distinction reaches host and route input results through the shared oracle.

**Required change:** map this contract-defined empty result to NamedKey.
Review other empty named-key results against R-14.1 without inventing terminal bytes.
Add structured result tests for the fixed modifier case, with flag 8 off and on.

### P29 — MEDIUM — The snapshot test does not test restored continuation

**Evidence:** `tests_snapshot.rs::restore_and_encode` frees the restored terminal immediately after re-encoding it.
The every-offset test feeds the suffix into `split`, the original terminal, rather than the restored terminal.
It proves snapshot round trips and split writes. It does not prove ST-6b's resume invariant.
The corpus also omits explicit saved cursor, tab stop, margin, and charset state cases named in the audit.

**Required change:** retain the restored terminal and feed the suffix into it at every byte offset.
Compare its final native state or snapshot with the terminal that consumed the whole corpus input.
Include the required saved state and pending UTF-8, CSI, OSC and DCS cases. Keep the existing continuation refusal tests.

### P30 — MEDIUM — Tests still contain handwritten expected terminal bytes

**Evidence:** `tests_encode.rs` constructs control-code expectations with `(letter as u8) & 0x1f`.
It checks literal CSI-u framing and literal ESC prefixes. It also expects generated Shift results such as `b"Q"`.
`tests_reply.rs` expects clipboard framing with `b";p;"`.
These are handwritten terminal byte expectations. BUILD.md architecture rule 2 forbids them when libghostty is the oracle.
Literal request inputs and unchanged host payload bytes are permitted; they do not justify generated protocol expectations.

**Required change:** replace generated byte expectations with independent native encoder results or native parsed state.
Keep structured contract assertions and literal request stimuli.
Check every binding test for this rule, including tests added after this review.

### P31 — MEDIUM — The custom base64 decoder has no recorded reason

**Evidence:** `reply.rs::base64_decode` implements the alphabet, padding, decoded allocation and canonical-bit checks by hand.
The Prior art note lists other custom pieces but gives no ecosystem comparison or reason for this decoder.
Its reuse list also still describes proposed reuse, rather than the binding's final reuse and rejection decisions.
BUILD.md rule 0 requires a recorded reason for each custom infrastructure component and prefers maintained libraries.

**Required change:** use an established base64 component, or record a specific ecosystem comparison and reason for the custom decoder.
Update the Prior art note to describe the final binding, its reuse trailers, rejected code, and all custom pieces.
Preserve strict decoding behavior when changing the decoder.

### P32 — HIGH — The Linux archive mixes incompatible allocators

**Evidence supplied by the lead:** the infra engineer found this failure at Core head `977d986`.
The Zig archive defines `calloc` and `free`, while `malloc` and `realloc` remain glibc implementations.
The binding tests then abort with a heap error on Linux. The reviewed delta to `43432744` changes no build file.
The reviewer has not reproduced this failure and has run no tests.
This evidence supersedes the earlier source-only build result.

**Required change:** make the native archive use a consistent allocator on Linux.
Add a Linux check that the archive exports no libc allocator symbol.
Supply the corrected exact head and the green Linux gate that the lead requires for closure.

VERDICT: NOT CLEAN (9 open: P24–P32) for the binding and its audit at the exact Core head above.
The earlier native patch verdicts remain scoped to their reviewed changes. No new native patch or pin move is approved here.


## Frozen Amendment 13 update

The lead supplied frozen A13 candidate 5 at contracts commit `627d507`, manifest final30.
The reviewer read `frozen/current/core-contract-v1.17-amendment-13-candidate5.md` at that commit.
A13-1 and A13-1b keep the clipboard write requirements used in P25. P25 remains open without a change in scope.
The final read note includes every native clipboard read under EV-8.
OSC 52 reads have the typed label; other reads, including OSC 5522, are untyped queries.
The shadow must answer none of these reads. The binding's suppression of `KittyClipboardRead` agrees with that requirement.
This contract update closes no finding and approves no new binding head. The reviewer ran no tests.

VERDICT: NOT CLEAN (9 open: P24–P32) at Core head `43432744e0db3f48494828ab5cca9d4cd7f94ea9`.


## Binding delta and proposed native patches 9–11

Reviewed Core head: `ab8577a0e0f7017271be2daa27d63ee324e1ac9e`.
Reviewed native changes, read with `git show` only:

- Patch 9: `468268e5c9f9073d1d5856da9a088aacc52aab02`.
- Patch 10: `92d13482af18eaa39b3abb898759dd38874109e4`.
- Patch 11: `170d6faf82fb1a90f4776707421702f4ff4a66fc`.

The reviewer read the complete binding delta, audit revision 8, and all three native deltas.
The reviewer ran no tests. The contract and plan inputs remain those recorded above, including frozen A13 candidate 5.

### Native patch scope — CLEAN

Patch 9 reads the native modifyOtherKeys and mouse Shift capture state without an encoder probe.
Its output types match the C header. Its tests assert native state after native sequence handling.
Patch 10 links libc for the Linux native module. That correction removes the allocator-mixing premise of P32.
Patch 11 writes the OSC 5522 commit status inside the synchronous reply call.
The one-reply guard remains in place. The handler clears the transaction after the callback returns.
A callback that gives no reply still gets one EPERM response and transaction cleanup.
The new test checks that response delivery finishes before the synchronous reply returns.

VERDICT: CLEAN for the source logic of patches 9–11 through the exact native head above.
This scoped verdict approves no pin move and does not satisfy the Linux gate evidence required for P32.

### Closed binding findings

- P24 closes: the binding sets the native image limit to zero before writes. Native screen creation inherits that limit.
  The new test reads both screen limits and checks snapshot equality after image input. P34 records a separate restore defect below.
- P26 closes: the binding reads the actual native state through patch 9, including when kitty flags mask key encoding.
- P28 closes: the eight modifier keys with flag 8 off now produce the required NamedKey result after native zero output.
- P29 closes: the new every-offset test feeds the restored terminal. Its corpus includes saved cursor, tabs, margins and charsets.
- P30 closes: the identified generated protocol expectations are removed. Remaining supplied text and request stimuli do not construct protocol framing.
- P31 closes: the binding uses the base64 STANDARD engine. The Prior art note records final reuse and custom pieces.

### P25 remains HIGH — Acknowledgements are lost with discrete events

The event now preserves location, selection, all MIME representations, total size and the clear distinction.
The callback applies the configured size limit synchronously and captures the native acknowledgement from patch 11.
These parts of P25 are corrected.

**Remaining evidence:** `ClipboardWrite::ack` exists only inside `TerminalEvent::ClipboardWrite`.
`Shared::push` discards that whole event when the event count or byte bound is full.
A write of enough bells before an OSC 5522 commit fills the event buffer in one model step.
The callback then reports SUCCESS, but `push` discards the only copy of its acknowledgement.
Even without a preceding event, a clipboard value larger than the event buffer can cause the same loss.
A13-1b permits class D event loss but requires the acknowledgement to survive that loss.

**Required change:** separate acknowledgement delivery from the class D event buffer.
Preserve each acknowledgement as its own ordered input transaction, even when the event is dropped.
Bound the acknowledgement path with admission backpressure or a native model-step boundary; do not silently drop or truncate it.
Add a regression with a full event buffer and a clipboard write, including a SUCCESS response with no host.

### P27 remains HIGH — The precision check refuses valid exact pixels

The new check prevents rounded coordinates and native integer overflow. That part of P27 is corrected.

**Remaining evidence:** `encode_mouse` refuses every coordinate above 2^24.
An `f32` holds every integer up to 2^24, but it also holds some larger integers exactly.
For example, 16,777,218 is exact in `f32` and fits the native `i32` pixel result.
A native SGR-pixels release can report that coordinate. The binding returns Unsupported Coordinate before calling the encoder.
The comment and test also incorrectly call 2^24 the last exactly representable integer.

**Required change:** test the supplied value's exact representation and the native integer range, rather than imposing a blanket 2^24 cap.
Permit exactly representable values that the native API can encode.
Keep typed zero for values that lose precision or exceed the native integer range.
Add native-oracle coverage for 2^24+1 and 2^24+2, including releases, and correct the audit and comments.

### P32 remains open — The Linux gate evidence is pending

The source correction in native patch 10 is CLEAN.
`tests_archive.rs` checks the defined native symbols and rejects libc allocator definitions.
The implementer reports that the check detects the old Linux archive.
The lead requires a green Linux gate for closure. The implementer has not supplied that result.
No reviewer test ran. This finding remains open for that evidence only.

### P33 — LOW — Audit identifiers and the clipboard read note are inconsistent

**Evidence:** audit revision 8 assigns G12 to the clipboard event shape, although G12 already identifies legacy Shift key encoding.
Its closing note says `EV-8: OSC 52 only` without stating that only the typed clipboard label has that scope.
Frozen A13 also requires every other native clipboard read to reach the client as an untyped query, with no shadow answer.
The revision introduction claims that P24–P32 are closed, including P32 before its required Linux gate evidence exists.

**Required change:** use unique GAP identifiers and update their references.
State the typed OSC 52 rule and the untyped clipboard read rule explicitly.
Describe P25, P27 and P32 as corrected or pending only when the review and required evidence support that status.

### P34 — HIGH — Snapshot restore enables graphics that the source model disables

**Evidence:** the binding sets each source screen's image storage limit to zero.
Native `snapshot/terminal.zig::decode` reconstructs the terminal without a stored image limit.
Native `snapshot/snapshot.zig::Decoder.ready` also constructs `screen_options` without `kitty_image_storage_limit`.
`Screen.Options` therefore supplies the lib default of 10,000,000 bytes.
`snapshot/screen.zig::decode` applies that nonzero default to each restored screen.
The snapshot format excludes image state and does not record the disabled image policy.
The C decoder replays pending continuation through that restored terminal before it returns the terminal to the caller.
The original model rejects later image input while the restored model accepts it. H1's configuration does not survive restore.
The new resume corpus contains no image input and does not expose this ST-6b gap.

**Required change:** preserve the disabled native image policy through restore, including before pending continuation replay.
The lead chose policy preservation and authorized one minimal native patch on top of `170d6fa`.
The patch may restore the configured limit or carry the limit in the snapshot record; the implementer selects the smaller change.
Keep `snapshot_graphics` absent and state the image exclusion in the snapshot format description.
Record this GAP in the audit and follow Q1 for the resolution and any pin move.
Add every-offset resume coverage for image protocol input. Check native storage limits before and after restore as part of that proof.
The lead answered the reviewer’s QUESTION with this policy-preservation requirement.
P34 needs both a native image-resume test and the binding every-offset image-resume test.

VERDICT: NOT CLEAN (5 open: P25, P27, P32, P33, P34) at exact Core head `ab8577a0e0f7017271be2daa27d63ee324e1ac9e`.


## Resume review — upstream sync, patches 12–13, and binding delta

Exact binding head: `ce9b0fe8609cf9d958c595d52a3208fbea5bdde7`.
Exact fork head: `3f8eb6810bb673aa782b047de21783ac81fb1121`.

Review inputs:

- `pair-common.md` and `brief-p2-resume.md`, step 5.
- The P2 handoff on `origin/v1`, including the unpublished checkpoint at binding `52d1f730309a7361870871895f78e03f3e5df9ca`.
- The plan pin `stage1-plan.e4862c71.md` and `docs/stage1-clauses/p2-terminal.txt` on `origin/stage1/plan`.
- Contracts `contracts-v0.1.13`, commit `a8db5c9a0f43fc440564989fd38a55121fdda38b`, and BUILD.md.
- Contracts `origin/main:docs/ghostty/upstream-sync-20261002.md` and its evidence directory.
- Lead ruling P35 in the resume brief.

The reviewer read the binding changes since `52d1f73`, the unpublished corrections since `ab8577a`, and native patches 12–13.
The reviewer ran no tests, builds, or gates.
The implementer reports 94 binding tests passed, clippy passed, and fmt passed on the exact binding head.
Those reports do not supply the Linux gate required by P32.

### Fork identity and patch decisions

The reviewer ran this source comparison:

```text
git range-diff 83edd491e3024ae5e50393d62877b8897da1cccd..ada251c5e99cdb753de8bf72d5d1f307d474518f f523504ea5c9f41d150d1eb93cc7a748b90f9361..3f8eb6810bb673aa782b047de21783ac81fb1121
```

All 22 commit pairs have `=` identity. The old patch corrections remain separate and map to the sync record.
`git diff ada251c5e 3f8eb6810` changes only `macos/Sources/Features/Terminal/TerminalController.swift` (+11, -6).
The libghostty-vt sources, `build.zig.zon`, and terminfo are unchanged.
The earlier scoped approvals for patches 0–11 therefore carry to their corresponding commits in the new stack.

Patch 12 (`d5bebc7e244b8968534c2be32323ca421b3edca5`) uses the existing `kpKeys` helper for `numpad_equal`.
The helper selects the numeric or application keypad result through the existing encoder path.
Its test checks both modes. Lead ruling P35 permits the literal expectations inside the fork's oracle tests.
**P35 closes by that ruling. Patch 12 needs no change.**

Patch 13 (`3f8eb6810bb673aa782b047de21783ac81fb1121`) adds decoder option 3 with a `uint64_t` value.
The C decoder passes the configured limit to `Decoder.ready`.
The decoder applies that limit to every decoded screen before `fromDecoded` replays continuation.
An omitted option retains the library default. The snapshot format remains unchanged and excludes graphics state.
The binding declarations agree with the native header.

The source logic of patches 12–13 is accepted at the exact fork head.
This scoped acceptance does not make the binding CLEAN or close P32.

### Binding merge and build review

- `f711184` moves all six workspace contract dependencies and their lock entries to `contracts-v0.1.13`.
- Merge `654b5914f949629229e943783ade9827a85c3ee8` has parents `f711184` and `f96a9df`.
  The merge changes no binding crate file. Its `ci/remote` files equal those on the second parent.
  The fetch path stages the pinned submodule and Zig packages before the gate container runs without network.
- `2855690` moves the submodule, branch name, and audit record together.
  Audit revision 10 records upstream base `f523504ea`, all 22 new patch SHAs, and ruling P35.
- `91dfcf9` restricts the prefetch extraction to `pub const ZIG_PACKAGES: `.
  `build_data.rs` has seven packages. The old pattern also read two entries in `ZIG_PACKAGES_IN_ZON`, producing nine entries.
  The remote fetch script removes duplicates with `sort -u`, so it still selects the same seven packages.
  The implementer reports an empty-cache native build fetched those seven hashes at the new pin.

### Closed findings at this binding head

- **P25 closes.** `Drained.clipboard_acks` preserves each native acknowledgement outside the bounded event buffer.
  The callback captures the complete synchronous reply without truncation and queues replies in order.
  `vt_write_until_query` refuses another step when the undrained backlog exceeds the configured limit.
  One supplied chunk can exceed the limit; the caller must supply bounded chunks and drain between steps.
  The full-buffer regression checks that a dropped clipboard event still leaves its acknowledgement available.
  P36 below concerns the order test's expected bytes, not this source correction.
- **P27 closes.** `pixel_is_representable` checks exact `f32` representation and the native `i32` range.
  The binding permits exact values above 2^24 and refuses rounded values or integer overflow, including releases.
  Tests check each axis, 2^24+1, 2^24+2, larger exact values, and the native range boundary.
- **P33 closes.** The audit uses unique G13–G18 identifiers and distinguishes typed OSC 52 reads from untyped native clipboard reads.
  Its status table keeps P32 pending and does not claim that the pending review findings are closed.
- **P34 closes.** Resume step 4 places the native image-resume proof in the binding and requires no fork change.
  This instruction replaces the checkpoint's request for another Zig image test.
  The new test calls the real native decoder and reads native image storage through `ghostty_kitty_graphics_image`.
  At every byte offset, the zero-limit restore takes the suffix and stores neither direct nor chunked images.
  A default-limit control stores image 1 when the native prefix reference has not yet stored it.
  The control also stores image 2 when its complete input follows the cut.
  Both controls accept images on the alternate screen; the zero-limit restore rejects them there.
  Existing every-offset coverage checks the storage limit before and after the suffix and compares native snapshots.
  These checks distinguish policy preservation from a vacuous image stimulus, including pending continuation.
  `snapshot_graphics` remains absent, and `snapshot_format` states the image exclusion.

### P32 remains HIGH — exact-head Linux evidence is absent

The sync's recorded Linux job 8059 shows no defined allocator symbols at the new fork head.
Its Rust link program passes at optimization levels 0 and 3; the pre-patch archive aborts at level 0.
This supports the already accepted native source correction.
It is not a binding gate on `ce9b0fe8609cf9d958c595d52a3208fbea5bdde7`.
P32 closes only after a green Linux gate on the exact reviewed binding head.

### P36 — MEDIUM — The binding order test constructs an expected protocol field

**Evidence:** `crates/botster-terminal-ghostty/src/tests.rs:351` uses `format!("id={id}")` as an expected substring of each native acknowledgement.
The test `acknowledgements_come_in_the_order_of_their_writes_and_a_backlog_stops_the_write` constructs terminal protocol bytes in a Botster repository.
BUILD.md architecture rule 2 still applies there.
Lead ruling P35 permits literals inside the fork's own Zig tests; it does not permit this binding expectation.
The pause handoff identified both cases. P35 is closed under the ruling; this separate binding case remains open as P36.
The earlier P30 closure remains valid at its stated head.

**Required change:** compare each queued acknowledgement with the native acknowledgement from an isolated write using the same stimulus and id.
Use a fresh terminal for each isolated write.
Do not construct or parse expected protocol framing in this order assertion.
Keep the backlog and full-buffer regressions.

VERDICT: NOT CLEAN (2 open: P32, P36) at binding head `ce9b0fe8609cf9d958c595d52a3208fbea5bdde7`.


## P36 correction review — P32 evidence remains pending

Exact binding head: `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.
The reviewer read the complete delta from `ce9b0fe8609cf9d958c595d52a3208fbea5bdde7`.
The delta changes only the acknowledgement order test in `crates/botster-terminal-ghostty/src/tests.rs`.

**P36 closes.** The test removes the constructed expected `id=` field.
Each fresh terminal receives one isolated write with the same stimulus and id as the corresponding queued write.
The test gets exactly one native acknowledgement from each isolated write.
The three native results differ pairwise. The test compares the queued results with those native results in order.
The test therefore checks order without constructing expected terminal bytes.
The backlog assertion and full-buffer regression remain unchanged.

The implementer reports 94 binding tests passed, clippy passed, and fmt passed at this exact head.
The reviewer ran no tests, builds, or gates.
The fork pin and all other reviewed files remain unchanged, so the earlier source acceptance carries to this head.
All code and test findings are closed. **P32 remains open for evidence only.**
P32 requires a green Linux gate on `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.
A gate on a different head cannot close P32 for this head.

VERDICT: NOT CLEAN (1 open: P32) at binding head `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.


## Linux gate result — P32 remains open; P37 records the list failure

Exact binding head: `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.
The lead authorized the implementer to run the Linux gate before P32 closure.
The reviewer read this log:

```text
~/botster-sessions/gates/botster-core-stage1-p2-libghostty-bd94cb61-linux-20261004-004350-76462.log
```

The gate exits 1. Fmt, clippy, and taint pass. The conformance list check fails with three errors.
All later steps, including tests, are NOT RUN. This result does not close P32.
The earlier statement that all code and test findings were closed predates this gate result.

### P37 — MEDIUM — The contract pin move leaves invalid conformance lists

**Evidence:** the exact-head Linux gate reports these errors:

- `conformance/contracts-withdrawn.txt` differs from the withdrawn file of the pinned contracts tag.
- `conformance/core-ledger-ids.txt` contains 643 ids; the pinned Core ledger contains 654 ids.
- The withdrawn check rejects `conf::wp_3_served_code_is_trusted_not_claimed_otherwise` because it is not a Core ledger id.

**Required change:** regenerate the lists from the pinned contracts tag.
Correct the withdrawn check to preserve the pinned withdrawn record while checking the proper ledger scope.
Do not bypass the list check or remove a valid pinned withdrawal to make the gate pass.
Send the new exact head for delta review before another gate, under the lead's stated order.
The implementer has asked the lead for the withdrawn-check decision.

VERDICT: NOT CLEAN (2 open: P32, P37) at binding head `bd94cb61eefd026db41f3d3b14cf464b9d3e7bb8`.

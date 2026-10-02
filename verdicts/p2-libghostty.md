# P2 libghostty review

Reviewed head: `89afa037b198cb26173ff520245adb926b6ca21e`.
Previous audit head: `fd1471eabca0adb344c6027b8995e1151aa8f8a8`.
Scope: the written audit and fork patches 0, 2, and 3 only. This verdict does not approve the full fork series, binding, or pin change.

VERDICT: NOT CLEAN (patch 1: 2 open findings, P14 and P17).
Reviewed fork head: `38599d3209beb7bdc8c8ffcde2f8af414a21f202`.
Written audit: CLEAN at `89afa037b198cb26173ff520245adb926b6ca21e`; F1–F12 closed.

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

Status: OPEN.
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

Status: OPEN at `38599d3209beb7bdc8c8ffcde2f8af414a21f202`.
The new test `vt_write_until_query does not count an abandoned prefix against the limit` compares with literal `"\x1b[5n"`.
That literal duplicates bytes already present in the input.
BUILD.md forbids hand-written expected terminal bytes. P14 also required comparison with relevant input slices.
Required change: compare with the second query's slice of `input`, as the adjacent restart test already does.

VERDICT: NOT CLEAN (2 open findings, P14 and P17).
The written audit and patches 0, 2, and 3 retain their scoped CLEAN verdicts.
The full fork series, binding code, and Ghostty pin change remain unapproved.

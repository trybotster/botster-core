# Ghostty fork A6 review

## Round 1 — 2026-10-04

- Fork branch: `trybotster/ghostty` `botster/upstream-sync-20261004`.
- Exact fork head: `779907e0ec389c0a04de81dd4c092fda92b8325f`.
- Exact botster-core head: `59abb1f3d9159962dbb5035fca54ba875404fa23` on `stage1/p2-fork-a6`.
- Core comparison base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Upstream base: `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`.
- Synced stack head: `c370ef4d9910f9c8944c23207bdc3b425dbdaab2`.
- Binding rules: R-32 and R-7 at contracts `9a00db8b85bc165ff93ee620db3c54ba3ddeac32` (`contracts-v0.1.14`), BUILD.md, the pair brief, and lead ruling P35.

The reviewer inspected source, commit history, the sync record, and the supplied log. The reviewer ran no builds, tests, or gates.

### F-A6-01 — CONTRACT — The full length excludes data after the MIME count limit

**Status: OPEN.**

**Evidence:** At the exact fork head, `src/terminal/kitty/clipboard_write.zig:169-175` returns when a new MIME type exceeds `max_write_mimes` (64).
This return occurs before the new counting path at line 185.
The same return skips all later chunks for that omitted MIME type.

A valid transaction can cross the byte limit, contain 64 recorded MIME types, and then send data for a 65th MIME type.
The callback runs, but `over_limit_len` excludes the 65th type's bytes.
For example, set the byte limit to 1 and send one decoded byte for each of 65 distinct MIME types.
The code reports 64 bytes, although the transaction contains 65 decoded bytes.
This is a source-derived case; the reviewer did not execute it.

**Requirement:** R-32 requires the write's full byte length after the model stops buffering.
The record defines that length as every decoded byte of the whole transaction, including replaced regions.
The representation count limit must not truncate this new report.

**Required change:** Count valid data for omitted MIME regions in the over-limit path.
Preserve the existing contents behavior for ordinary writes and the Ghostty application.
Keep the counting memory bounded.
Add a regression test that crosses both the byte limit and the MIME count limit.
Include omitted data before and after the byte limit in the proof, or obtain a steward ruling that explicitly excludes such data.

### F-A6-02 — HIGH — Required test evidence is incomplete

**Status: OPEN; execution waits for the lead's HOLD to end.**

**Evidence:** The sync record at the exact Core head marks the patch tests, Linux tests, binding tests, and empty-cache package check as pending.
The supplied `mac-sync-c370ef4d9.log` names the synced stack and records successful Zig tests and a successful library build.
That log predates patch 14 and cannot prove the new callback behavior.
The recorded package hash change has no completed empty-cache verification.

**Required change:** Record successful `test-lib-vt` results and the library build with `GHOSTTY_BUILD_ARGS` for the final fork head.
Record successful binding tests for the final Core head.
Supply the required Mac and Linux evidence with exact heads, commands, results, and raw logs.
Verify the Zig package list with an empty cache.
Update the sync record and audit when the results are available.

The reviewer does not request a gate during the HOLD.
If only gate-dependent evidence remains after source findings close, the lead's gate-evidence closure rule applies.
A pending result does not qualify for CLEAN.

### Reviewed facts and scope limits

- The upstream ref log records a fetch to the stated base at `2026-10-04 20:35:45 -0700`.
  The rebase precedes patch 14.
- The reviewer independently compared all 22 old patches with the synced stack.
  The range comparison shows 20 identical patches and two changed patches.
  Patch 1 changes option numbers to 46 and 47 and retains the upstream checksum handler.
  Patch 8 changes only adjacent context.
- The record gives a KEEP or REWORK decision and a reason for each old patch.
  The inspected upstream changes do not replace an old patch.
  The record checks watched merges through fetched history without an upstream API call.
- The C wrapper enables counting for libghostty.
  The Ghostty application keeps its existing EFBIG behavior.
  Ordinary writes retain contents and use the existing reply path.
  Over-limit writes free the spool and use a fixed buffer for later accepted MIME regions.
  The callback carries the original location, no contents, and the counted length.
- The Zig callback test covers the limit and the over-limit path.
  It checks that only the callback reply writes the acknowledgement.
  It also checks a replaced MIME region and the next transaction.
  Literal terminal bytes in these fork tests comply with P35.
- The Core submodule names the exact fork head above.
  `.gitmodules` names the new branch and retains the `trybotster/ghostty` URL.
  The C declaration adds trailing fields to the sized request.
  The Rust public event type remains unchanged.
  The binding maps a model limit refusal to absent contents, the native length, and IO_ERROR.
- The binding test derives the error acknowledgement from another libghostty write.
  Its expected length comes from the input chunk lengths.
  The test also checks that no acknowledgement escapes through `pty_writes`.
- Audit revision 11 records the new pin, option numbers, callback fields, and package hash.
  The inspected snapshot sources remain unchanged, so no format change requires a GHOSTSNP.md edit.
  Final PR metadata, including the required Prior art note, was not supplied for this round.
- The record attributes retention of the default 64 MiB model limit to a lead decision pending Amendment 14.
  This round does not require an option 39 change or certify the separate native memory issue as closed.
- The action record reports only a fetch from ghostty-org and a push to trybotster/ghostty.
  The reviewer found no sign of an upstream push or contact in the inspected material.
  This statement describes the available evidence; it is not an independent audit of every external action.

VERDICT: NOT CLEAN (2 open)

## Round 2 — 2026-10-04

- Exact fork head: `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` on `botster/upstream-sync-20261004`.
- Exact botster-core head: `e676a2d577e07a350f8a85496fdd8e443491f28c` on `stage1/p2-fork-a6`.
- Delta bases: the exact fork and Core heads from round 1.
- Additional binding rules: R-33 (contracts main `14c86abedd59a1f528efe72603b031b304509f18`) and final A14 (contracts main `69327d52cedb05ee9b9c63912b58d6f62a36917f`, manifest final33).

The reviewer inspected both deltas and read R-33 and final A14.
The reviewer ran no builds, tests, or gates.

### F-A6-01 — CLOSED by R-33

R-33 explicitly excludes data for MIME types that the model ignores.
Those bytes are not decoded and must not be decoded only to count them.
The round 1 request to count those bytes is withdrawn.
The fork already follows the ruled behavior.

The new fork commit changes tests only.
The tests cover 64 retained types at the byte limit, followed by valid and invalid ignored types.
Another test checks that ignored types add nothing after the write crosses the byte limit.
The C callback test now includes an alias and still expects the decoded length of 11.
These expectations follow R-33 and A14.

### F-A6-02 — HIGH — OPEN

The required test evidence remains pending at both exact heads above.
The lead's HOLD still prevents the implementer from supplying the remaining results.
The closure requirements from round 1 apply to the new heads.
The reviewer accepts no completed test result for these heads from the supplied material.

### F-A6-03 — LOW — The sync record describes option 39 inconsistently

**Status: OPEN.**

**Evidence:** `docs/stage1/ghostty-upstream-sync-20261004.md:198` still describes option 39 as "for the test only".
Line 199 and `src/lib.rs::set_clipboard_limit` correctly show that production code now sets this option.
The record at line 157 and the audit at line 391 also state that A6 is closed while required evidence remains pending.

**Required change:** Describe option 39 as a production configuration under A14-3.
Mark the A6 correction as implemented with verification pending until the required evidence closes F-A6-02.
After that evidence is accepted, the record can state that A6 is closed.

### A14 source review

- `set_clipboard_limit` sets the callback bound and native option 39 to the same value.
  The native option accepts the supplied `size_t` without a value-dependent refusal.
  `Terminal::new` and `Terminal::from_snapshot` call this setter with the default binding bound.
- The callback uses the model's over-limit report for A14-2 step 1.
  Otherwise it sums final contents for step 2, including each alias entry.
  The public event type remains unchanged.
- The new binding test derives lengths from its inputs.
  It covers delivery at the bound, an alias refusal, a replaced region refusal, and counting over several chunks.
  It compares acknowledgements from libghostty and checks that no acknowledgement goes through `pty_writes`.
- A14-3 excludes temporary decode storage and allocator capacity from its retained payload guarantee.
  The existing crossing-chunk decode followed by spool release does not contradict that guarantee.
- The Core submodule names the exact fork head above.
  The upstream base, `.gitmodules`, package list, and snapshot format sources remain unchanged from round 1.
  The audit and sync record cite R-33 and final A14.
- The updated action record reports a fast-forward push only to trybotster/ghostty.
  The reviewer found no sign of an upstream push or contact in the inspected delta.

VERDICT: NOT CLEAN (2 open)

## Round 3 — 2026-10-04

- Exact fork head: `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` on `botster/upstream-sync-20261004`.
- Exact botster-core head: `65b2064de09a7a8c72f4edf42d6e1c2f467708ed` on `stage1/p2-fork-a6`.
- Core delta base: `e676a2d577e07a350f8a85496fdd8e443491f28c`.

The reviewer inspected the complete delta.
Only the sync record and audit changed.
The fork head and source code remain unchanged from round 2.
The reviewer ran no builds, tests, or gates.

**F-A6-03: CLOSED.** The record now states that production code sets option 39.
Both documents mark A6 as implemented with verification pending.

**F-A6-01 remains CLOSED under R-33.**

**F-A6-02: HIGH, OPEN.** The required test evidence remains pending.
No source or documentation finding remains open at these exact heads.
The lead's gate-evidence closure rule applies if completing the evidence requires a gate after the HOLD ends.
The evidence must still be accepted before CLEAN.

VERDICT: NOT CLEAN (1 open)

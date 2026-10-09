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


## Round 4 — 2026-10-09

- Exact fork head: `39a68e822e505685d1e7fa9c6abeff21125b489c` on `trybotster/ghostty` branch `botster/upstream-sync-20261009`.
- Fork tree: `bd8386949d687724e37cbd88f6516f37fc9f280f`.
- Exact Core head: `338f3ecb20bde1c47db4aa374b7fe987894dbee2`, PR #179, branch `stage1/p2-fork-a6-r2`.
- Core tree: `7d38fa503f48de41e213a8c0786b114da211f84b`.
- Previous reviewed fork head: `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`.
- Previous reviewed Core head: `65b2064de09a7a8c72f4edf42d6e1c2f467708ed`.
- New upstream base: `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d`.

The reviewer read the fork comparison, binding delta, sync records, audit revision 12, evidence script, raw final logs, and PR metadata.
The Core delta against the accepted v1 base separates the A6 changes from the inherited A3 binding code.
The reviewer ran no builds, tests, mutation jobs, or gates and changed no product code.

### USER RULE and fork policy

The user permits only a fetch from ghostty-org. The user forbids upstream pushes and every other upstream contact.
Ghostty pushes must go only to trybotster/ghostty, with the remote and branch named explicitly.

The action audit reports only `git fetch ghostty-org` against upstream.
It reports one successful explicit push of the new branch to trybotster/ghostty after verification of the origin URL.
The first push attempt failed because of email privacy. The author correction kept the tree unchanged and used no force-push.
The reviewer found no sign of an upstream push or other contact in the inspected material.
This statement covers the supplied records and local refs, not every external action.
The reviewer made no upstream contact.

The upstream reflog records the fetch at 2026-10-09 03:17:55 -0700.
The new branch reflog records its creation from the old candidate and its rebase onto that upstream head afterward.
Local remote refs retain the old 20261002 and 20261004 heads and name the new 20261009 head.
The sync therefore precedes the proposed pin move. The submodule and .gitmodules name the new fork branch and head.

### Patch decisions and source

The reviewer independently compared the 24 old and new commits with `git patch-id --stable` and `git range-diff`.
Twenty-one patches are identical. Patch 6 and patch 14 have context changes only.
Patch 1 moves QUERY and QUERY_MAX_BYTES from 46/47 to 47/48 and keeps upstream's program_status arm.
The binding uses the new option numbers. The decision table correctly gives KEEP 23, REWORK 1, and DROP 0.
The reviewed upstream changes replace no retained patch.

The reviewer read the upstream changes that affect the terminal, stream, formatter, mouse encoder, and C callback configuration.
OSC 7501 has no reply or effect without a program_status callback. The binding sets no such callback.
DECSTR changes the model state through the new native softReset function. The binding continues to read that model state.
The formatter change requires VT output with restored modes and margins; the binding uses plain text with those extras off.
The snapshot and terminfo source comparison is empty. This sync requires no GHOSTSNP format change.
The watched merge check uses fetched Git history and reports no merge for the listed watched PRs.

Patch 14 retains the R-32 callback, decoded-size counting, bounded count buffer, and one reply owner.
The R-33 tests retain the exclusion of ignored MIME types. F-A6-01 remains CLOSED under R-33.
The binding's A14 setter, callback shape, and two size checks remain unchanged from Round 3.
The constructor and snapshot restore both call the setter. Option 39 accepts the supplied limit.
F-A6-03 remains CLOSED. The current PR has the required Prior art note.

### F-A6-02 — CLOSED — Required final-source evidence is supplied

Both final native evidence logs name fork `39a68e822` and Core `2c636dbe411c2a2d4d5b8b8fcc867b976b019602`, with zero tracked changes.
The reviewer verified that the later delta to `338f3ecb` changes only records and logs, not source or evidence.sh.

The final Mac log is `docs/stage1/ghostty-upstream-sync-20261009/mac-run3-2c636dbe.log`.
It records a successful empty-cache library build with GHOSTTY_BUILD_ARGS and exactly the seven expected packages.
The fork's project-local zig-pkg directory is absent before that build.
Default test-lib-vt passes: 46/46 steps, 6751/6805 tests, 54 skipped.
Shipped-options test-lib-vt passes: 42/42 steps, 6749/6805 tests, 56 skipped.
The binding reports 134 tests passed. Each listed executed step exits zero.

The final Linux log is `docs/stage1/ghostty-upstream-sync-20261009/linux-run2-2c636dbe.log`.
Under the recorded lead scope, Linux runs test-lib-vt with shipped options in Debug from the seven-package store.
It passes: 41/41 steps, 6733/6805 tests, 72 skipped. The binding reports 134 tests passed.
The Linux empty-cache build and default-configuration tests are explicitly NOT RUN because the gate has no network.
The reviewer does not count those two steps as passing.
The earlier Linux step-1 result is explicitly disclaimed as an empty-cache proof.

The full Linux log at the exact Core head is:
`~/botster-sessions/shared/core-stage1/gate-logs/fork-a6-338f3ecb.log`.
It records 903 default tests and 241 slow tests passed.
Every listed CI stage passes. The in-diff run reports one mutant caught, zero missed, and zero timeouts.
Fuzz reports that the diff changes no crate with a decoder harness. The remote job exits zero.
The evidence closes the required native build, Zig tests, binding tests, and empty-cache library package check.
The remaining finding concerns a separate package-isolation claim in the record.

### F-A6-04 — LOW — Mac step 2b does not establish the claimed package isolation

Status: OPEN. The reviewer sent this finding directly to P5 and copied integration.

At `docs/stage1/ghostty-upstream-sync-20261009/evidence.sh:28`, the script checks project-local zig-pkg only before step 1.
Steps 1 and 2a can populate that directory before step 2b.
Step 2b uses fresh global and local caches, but it uses the same fork tree.
At line 73, its extra-package check reads only the new global cache.
Zig 0.16 can also use project-local packages, as the script and audit explain.
The Mac run therefore does not establish that step 2b had access to only ZIG_PACKAGES.
This limitation does not invalidate its shipped-options test result or the empty-cache library proof in step 1.

Required change: qualify the Mac step-2b package-sufficiency claim in the sync record and PR description.
Update the README or audit where those documents repeat the claim.
Keep the passing Mac test result. Attribute the seven-package shipped-test proof to the Linux run.
Alternatively, isolate the project-local package directory before Mac step 2b and supply the resulting evidence.
A documentation correction needs no new native evidence run.

No source finding remains open. Integration owns its separate review and platform-result explanation.
The lead retains the pin record and merge decision. This verdict closes no conformance id.

VERDICT: NOT CLEAN


## Round 5 — 2026-10-09

- Exact Core head: `c085c1b99cb306cbd89b6331ecee28f554494694`, PR #179, branch `stage1/p2-fork-a6-r2`.
- Core tree: `898524dc3b51679cb68cc7505eee7528b2a73250`.
- Exact fork head: `39a68e822e505685d1e7fa9c6abeff21125b489c`, branch `botster/upstream-sync-20261009` in `trybotster/ghostty`.
- Fork tree: `bd8386949d687724e37cbd88f6516f37fc9f280f`.
- Upstream base: `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d`.
- Previous reviewed Core head: `338f3ecb20bde1c47db4aa374b7fe987894dbee2`.
- Accepted v1 merge base: `d15579dadd7cf1ed65b768139d2a668f041bd932`.

The reviewer read the merge delta, evidence script, records, raw native logs, exact-head Linux gate, and current PR body.
The fork head and tree remain unchanged from Round 4.
The merge at `475bee89` has the previous reviewed head and accepted v1 head as its parents.
The A6 diff before and after that merge is byte-identical against each corresponding v1 base.
The later correction changes the evidence script, records, and logs. The final commit changes records and logs only.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### F-A6-04 — CLOSED — The evidence states the package scope

Before step 2b, evidence.sh reports the project-local zig-pkg directory and moves any existing directory outside the fork tree.
The latest Mac log reports 39 entries before that move. The Linux log reports that the directory was absent.
Each step 2b starts with fresh caches and the seven seeded packages.

The record, README, audit, and PR body distinguish the evidence scopes.
Linux step 2b proves that the seven packages suffice for shipped-options tests without network access.
Mac step 2b proves the shipped-options test result. Mac has network access, and its extra-package check covers the global cache only.
The accepted Mac step 1 from Round 4 proves the empty-cache shipped-library build.
The latest Mac step 1 fails during a Codeberg fetch with HttpConnectionClosing. The record does not count that result as passing.

### Native evidence and retained closure

The latest logs are:

- `docs/stage1/ghostty-upstream-sync-20261009/mac-run4-6e4a55aa.log`.
- `docs/stage1/ghostty-upstream-sync-20261009/linux-run3-6e4a55aa.log`.

Both logs name Core `6e4a55aa439f62818f21c01862fe1766b6790732` and the exact fork head, with zero tracked changes and Zig 0.16.0.
The delta from that Core head to the reviewed head changes only records and logs.

Mac default tests pass: 46/46 steps, 6751/6805 tests passed, 54 skipped.
Mac shipped-options tests pass: 42/42 steps, 6749/6805 tests passed, 56 skipped.
Linux shipped-options tests pass: 41/41 steps, 6733/6805 tests passed, 72 skipped.
Each platform reports 134 binding tests passed.
Linux step 1 and default tests remain explicitly NOT RUN because the gate has no network.
The reviewer does not count those steps as passing.

The accepted Mac empty-cache library proof remains `mac-run3-2c636dbe.log` at Core `2c636dbe411c2a2d4d5b8b8fcc867b976b019602`.
The fork remains unchanged. The native build data, submodule configuration, events, library, and sys source blobs match the reviewed head.
The later v1 clock-helper changes do not change native build flags. The latest binding tests cover the merged source.
F-A6-02 remains CLOSED.

The reviewer checked the platform explanation against native test guards and the full step trees.
Both vt modules skip one SIMD decoder test when simd=false, which gives two additional skipped tests.
Both modules also skip eight Mac-only input tests on Linux, which gives sixteen additional skipped tests.
The default configuration has four additional SIMD build steps.
Mac also has one additional WriteFile libc.txt step for the Apple SDK translate-c configuration.
Integration owns its separate review closure.

### Exact-head Linux gate

The supplied full gate is `~/botster-sessions/shared/core-stage1/gate-logs/fork-a6-c085c1b9.log`.
It names the exact reviewed Core head and accepted base `d15579da`.
The result is 920 default tests passed with 654 skipped, and 243 slow tests passed with 941 skipped.
The in-diff mutation run catches one mutant, with zero missed, zero timeouts, and zero unviable.
All listed CI stages pass. Fuzz reports no changed crate with a decoder harness.
The remote job and gate exit zero.

### USER RULE and final scope

No new fork commit, fork push, or upstream action appears in this delta.
The current PR retains the fetch-only upstream action and the explicit new-branch push to trybotster/ghostty.
The inspected local refs retain the old fork branch heads and the reviewed new head.
This check covers the supplied records and local refs, not every external action.
The reviewer made no upstream contact.

F-A6-01 and F-A6-03 remain CLOSED. F-A6-02 remains CLOSED. F-A6-04 is CLOSED.
No A6 package finding remains open. The lead owns the pin record and merge decision.
This verdict closes no conformance id and does not change the separate #176 F29 proof hold.

VERDICT: CLEAN

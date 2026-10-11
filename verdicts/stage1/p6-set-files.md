# #244 — base-merge check for the real pending list

## Source inspection before Round 1 — 2026-10-11

Inspected head: `8f24d0eed5dcc7815cef1923ef075fb155c037b1`.
Base: `475a442fa58cd2555c4121ce705ea3cf75470692`.
Merge parents: `51c5c80841df848a0369694a965f73743fefec06` and the base above.
Receipt: `msg_plugin-w_1791705114_f5bad4`.
Authority: approved plan 23aj at `6dbe1c43d55e31cba038cee1008098bc0ba41ef0` and BUILD.md at `56bd0a5`.
The lead's ~00:20 ruling requires this SET_FILES extension before 23u. The ~05:10 entry confirms this review request.
The HIGH classification is correct: this PR changes gate decisions and a shared conformance list.

### Interfaces and accounting

The complete own delta changes three files: 133 insertions and 48 deletions.
SET_FILES now names exactly `core-pending.txt` and `core-real-pending.txt`.
The check excludes both files from conditions 2 and 3. It reads both files at all four commits for condition 4.
The real pending list contains whole ID lines. Its additions still require review when both sides change the file.
The checker validates pure removals on both sides. It requires the final file to equal the old file without their union.
An ID removed by both sides leaves once. The return value and report identify both removal counts and their intersection.
The checker still rejects additions, edits, reordered lines, duplicate old IDs, incorrect final files, and non-regular file modes.
The one-sided rule still requires the exact other-side file. A missing file still fails unless all four commits lack it.
An unreviewed removal after the merge still fails condition 4. Another unreviewed path change still fails condition 3.
The conflict and ancestry requirements remain unchanged.

I inspect all nine files in `conformance/` and their data-line formats.
The six other text lists remain outside SET_FILES. Five contain records with fields; the ledger list must match one pinned tag.
The deferred TOML file also remains outside SET_FILES.
The exact two-path selection and the non-set list test preserve the review requirement for those other files.
I read the existing real-list validation in `lists.rs`. This PR changes none of its admission or pin rules.

The real pending list changes only comments. Both pending lists retain every data line from current v1.
The group comments lose their counts so a future ID removal can preserve comment bytes.
Historical header counts remain. This PR moves no conformance ID and changes no contract pin or assertion.
It changes no runtime package, process owner, deadline, cleanup, testkit control, dependency, or mutation exclusion.

### Merge resolution and prior art

The automatic merge reports one conflict, in the route-group comment of `core-real-pending.txt`.
I compare that automatic tree with the actual head. The only difference resolves that comment to the branch's count-free form.
The final file retains every v1 ID line, including #238's three removals. No other merge edit remains.
The exact-head diff check passes.

The PR compares OLD CORE at the binding `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
My static tree and symbol searches find no prior base-merge checker or conformance lists there.
The change reuses current #190's removal checker, regular-file check, one-sided byte rule, and Git command wrapper.
The custom decision code remains necessary to implement the lead's condition 4 for these two lists.
The added tests use the existing repository fixture and public check reports. They add no private queue assertion.

### Retained exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-set-files-8f24d0ee-pool-20261011-004331-48874.log`.
I read its exact-head header, relevant test results, conformance reports, summary, and all ten individual mutant outcomes.
The header names this inspected head and base. It runs plain `cargo xtask ci` with four build jobs and four nextest threads.
All ten stages pass in 400.2 seconds. Default: 1610 passed, 484 skipped, in 10.733 seconds.
Slow: 427 passed, 1993 skipped, in 94.214 seconds.
The added real-repository tests pass for disjoint removals, the same removed ID, and a non-set shared list.
The pure tests pass for overlapping removals, additions on either side, restored IDs, and incorrect final files.
The existing mode, one-sided byte, hostile Git setting, and unreviewed-edit tests pass.

Conformance passes 206 tests. Strict pending runs 373 / 394, with 21 real-only IDs not run and two held IDs.
Testkit minimum: 55 / 69. Pending-real: 77 IDs, zero newly passing IDs.
The gate reports real-passing 36 / 68 and real-accepted 37 / 69.
R-59 counts the current real minimum as 35 / 68 plus one unfenced AD-7 pass; these gate counts remain historical.
Stage 1 catches all ten mutants in 80.0 seconds, with zero missed, timeout, or unviable.
Eight mutants replace set_merge's complete return. Two replace judge_set's complete return.
Stage 2 does not run because stage 1 misses none. The mutation step takes 81.8 seconds.
Fuzz passes and reports no changed decoder harness.
The job and wrapper exit zero after 473 seconds on msa1, with two seconds queued and 471 seconds running.

This delta changes pure list decisions and the existing Git-based check. It adds no OS-dependent real-process behavior.
It requires no additional focused Mac execution for its own delta.
I find no integration source defect. The exact-head package verdict remains pending and is requested.
This inspection issues no formal CLEAN verdict before I read that artifact.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


### Package finding received before Round 1

Receipt: `msg_plugin-w_1791705258_39184e`.
The package reviewer reports SF-R1-1 LOW at the same inspected head.
I confirm `xtask/src/base_merge/tests.rs:580` asserts `SET_FILES` directly against the two internal paths.
That assertion checks the implementation constant. The user requires assertions on public behavior.
The repository tests already check the real pending list's accepted removals and a non-set list's rejected merge.
The correction must remove the constant assertion and its associated comment and PR-body claim.
The correction must retain those public behavior proofs.
My preceding inspection missed this test-policy issue. Its statement that no integration source defect exists does not close this finding.
The package reviewer is completing its evidence review. Its committed exact-head verdict remains pending.
This inspection still issues no formal verdict. SF-R1-1 remains open for the formal round and correction.

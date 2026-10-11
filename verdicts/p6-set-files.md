# P6: line-set merge rules (#244)

## Round 1 — 2026-10-11

Exact head: `8f24d0eed5dcc7815cef1923ef075fb155c037b1`.
Base: `475a442fa58cd2555c4121ce705ea3cf75470692`.
PR: https://github.com/trybotster/botster-core/pull/244.
Risk: HIGH. The change affects gate infrastructure and a shared conformance list.

### Finding

**SF-R1-1 — LOW — the new test asserts an internal constant.**

`xtask/src/base_merge/tests.rs:580` asserts `SET_FILES == [PENDING, REAL_PENDING]`.
This assertion checks the implementation's selection constant. It does not check the public result of the merge check.
The user's review rule requires assertions of public behavior.
Remove this assertion and its exact-constant claim in the test comment and PR body.
Keep the real repository assertions for disjoint removals, shared removals, and the non-set list failure.
Those assertions check the required public result and fail if the selection rule regresses.
I sent this finding directly to P6 and Sol integration.

### Source review

I read all three changed files, the full PR body, and the surrounding merge-check implementation and tests.
The selection rule adds only `conformance/core-real-pending.txt` to the existing pending list.
I read all nine conformance file headers and data formats. The table covers each file with a reason.
The five record lists retain the byte rules. The pinned ledger copy and TOML file also retain the byte rules.
The old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no corresponding check or conformance lists.
I checked its paths and searched its tracked content. The Prior-art note correctly reports no item to reuse.
The current check reuses the #190 rule. Its one-sided byte check and regular-file mode check remain intact.

When both sides change a selected list, each side must remove only whole ID lines.
The result must equal the original file without the union of those removals, in its original order.
The new shared-removal rule removes the ID once. It reports each side's count and the intersection count.
The check still rejects additions, edits, comment changes, reordering, duplicate IDs, and an incorrect merged file.
The tests retain the negative cases and add shared-removal, addition-versus-removal, and non-set list cases.
The changed shared-removal test now checks the lead's required PASS result.
Reverting the new list selection would make the real repository case fail condition 2.
Restoring the old shared-removal rejection would fail the repository and pure decision cases.
These are source deductions, not reviewer reversal runs.

The new repository cases reuse `Repo::git` and its shared `run_to_completion` helper with `Deadline::cleanup()`.
That helper owns its tool process group and its own child. The change adds no sleep, busy loop, or production reaper.
The unchanged production Git shell in `check` remains outside this delta.

The final merge parents are `51c5c80841df848a0369694a965f73743fefec06` and the base above.
The merge imports the three accepted #238 ID removals. The comment conflict keeps the group's line without a count.
Against the base, every ID line is identical and in the same order. All list edits are comments.
No transcript, assertion of a contract, contract pin, mutation exclusion, process deadline, or cleanup behavior changes.

### Retained evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p6-set-files-8f24d0ee-pool-20261011-004331-48874.log`.
I read its exact-head header, reports, relevant test results, summary, and all ten mutation outcomes.
All ten gate stages pass. Job and wrapper exit zero after 473 seconds, with two seconds queued and 471 running.
Default: 1610 passed, 484 skipped, in 10.733 seconds. Slow: 427 passed, 1993 skipped, in 94.214 seconds.
The changed merge tests pass. Stage 1 catches all ten mutants in 80.0 seconds, with zero missed, timeout, or unviable.
Stage 2 does not run because stage 1 misses no mutant. Fuzz reports no changed decoder harness.
Conformance passes 206 IDs. Strict pending runs 373/394, with 21 real-only IDs not run and two held IDs.
The testkit minimum is 55/69. Pending-real reports zero newly passing IDs among 77 IDs.
The gate's real minimum counts, 36/68 and 37/69, are historical. R-59 currently counts 35/68 plus one unfenced pass.
AD-7 cannot count until its required `held:true` form passes.

One LOW finding remains. I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11

Exact head: `76bab0085a7060c01e12b1bbf960cc7078ae50fb`.
Base: `475a442fa58cd2555c4121ce705ea3cf75470692`.
Parent: `8f24d0eed5dcc7815cef1923ef075fb155c037b1`.

I read the complete correction delta and the full updated PR body.
The delta changes only `xtask/src/base_merge/tests.rs`: one assertion is removed and its comment states the required behavior.
The test still creates real repositories for disjoint removals and one shared removal in the real pending list.
Both cases still require a successful check and the PASS condition-4 report.
The separate non-set list case still requires FAIL condition 2 and the failing final result.
The PR body removes the exact-constant claim and records the finding and correction.
SF-R1-1 is closed. No finding remains.

The correction preserves every public behavior assertion, merge decision, file selection, conformance ID, and list comment from Round 1.
It changes no process owner, deadline, cleanup, contract pin, transcript, or mutation exclusion.
The complete Round 1 source review and its retained evidence remain applicable within their stated limits.
That completed gate belongs to `8f24d0eed5dcc7815cef1923ef075fb155c037b1`, not this correction head.
The implementer states that the final gate will run after both CLEAN reviews, under the lead's pool rule.
This verdict completes the source review. It does not claim a completed gate at this head.
GitHub names the exact head and base above. The PR remains open.
I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: CLEAN

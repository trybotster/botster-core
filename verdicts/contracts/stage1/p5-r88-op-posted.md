# contracts #74 — stage1/p5-r88-op-posted (R-88 group (B), `op_posted`), reviewer 017f

HIGH (rule 3): a new control and a transcript change to `am_4_capacity_returns_on_poll`, which is live on both Core tiers.

## Round 1: CLEAN on 28e054875214f1433736afd2304ce59ecd692107

The head merges the PR commit `0e982850bd64834b30ede675aba27c45a90a51fa` (on main `5a83c853d4bcebe191e0ae7492eb1020b9c35281`) with main `3c68880b011fdb60f7bca8124eef350e2d4d4708`. Main is still `3c68880b` at this review.

- **Merge.** `git merge-tree --write-tree 0e982850 3c68880b` gives tree `696583dc20761a2314eed6dcb3dff9f125b1beb2`, which equals the head's tree. The PR's five files are the same against both bases.
- **Ruling text.** R-88 (B) on main: a read-only `op_posted {for_op}` → `{posted}`, true once Core posted the op's terminal event, polled or not; awaited after s10 for `$w2`; `$w1` and `$w3` only "if s24's claim needs their order".
- **The control row** (`docs/core-testkit-controls.md`) matches the ruling: the `for_op` argument (not `op`, as corrected), read-only, true before and after the poll, false before the post, named for the precondition. `vocabulary.rs` adds it to `CONTROL_ARGS` (`for_op`) and `CONTROLS`.
- **am_4.** Index 11 is `await_control {"op": "op_posted", "for_op": "$w2"}` → `posted: true`, `because: Core AM-4`. `requires.controls` is `["op_posted", "pty_input"]` (sorted). `await_control` gets the bindings and substitutes them (`driver.rs` ~1375-1386), as for `scheduler_hold`'s `for_op`.
- **Only `$w2` is enough.** I checked the claims after the fence:
  - s25 (`poll_events max 1` = `Completed{$w2}`): `$w1` begins at s15 and `$w3` at s20, after the fence proved that `$w2`'s completion is posted. So `$w2`'s completion is before theirs in the queue, whether or not `$w1` and `$w3` have completed. Their order needs no fence.
  - s26 and s27 (`LaneFull` for s1 and s3): with `input_ops_per_session: 1`, the lane is full while the op is in flight AND while its completion is unpolled. Both states give `LaneFull`, so no report-derived state decides the step.
  - s28-s32 are `pump_until` and `pump_collect`, which wait for their events.
- **`fake: structural`, `ruling: R-12`.** FakeCore does not answer `op_posted`. The README real-only list adds the id. `docs/fake-replacement-map.md` drops its row, and the counts move from 168/473 to 167/474. The gate's replacement-map step reports the file up to date.
- **Gate.** PASS, all ten steps, exit 0 after 546 s on linux, base `3c68880b`: `~/botster-sessions/gates/botster-contracts-stage1-p5-r88-op-posted-28e05487-pool-20261011-164527-82999.log`.
- **Core evidence** (Core scope; I read it, and do not review it): `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r88b-clean-evidence.log`. A logged `cargo clean` of `targets/p5`, one worktree, Core `077fbe71a3c0d93303158c43db831898f2aa7bd1`. The am_4 blob is `b113f41763478baa6e33a95e824e9b5c2a26dcbe`, which equals this head's. The am_4 testkit run (seeds 0-31) and real-tier run pass. Both self-tests `op_posted_reads_a_completion_before_its_poll` pass: false before the worker's report, true after a pump-only wait, the completion in the next poll, true after the poll, and false for an id that Core never minted.

No finding. CLEAN on `28e054875214f1433736afd2304ce59ecd692107`.

Note for the next merger (not a finding of this PR): #73 (the R-88 lint) names `op_posted {for_op}` as a fence of `Completed{op}`, and its self-test is red on today's am_4 s24. The second of #73 and #74 to merge reconciles that self-test and any exception line with the index 11 fence.

If main moves before the merge, I re-check the delta per Q7.

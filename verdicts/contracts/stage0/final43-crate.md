# contracts #96 — stage0/final43-crate (Core Amendment 24 candidate 2: `Exit.output_may_remain`, probe step `write_forever`, the A24-4 testkit-proven class), reviewer 017f

HIGH. Implementer: 016b. Package reviewer: 016c. Base: `steward/final43` `0bdb72e7`, with contracts main `216ea74e` merged in. The lead merges it into `steward/final43`.

## Round 1: NOT CLEAN on 0b7f68a0df506746006debc8f506c3264a4a1344 (1 finding, LOW)

### The gate

- `~/botster-sessions/gates/botster-contracts-stage0-final43-crate-0b7f68a0-pool-20261012-123641-25650.log`. My `shasum -a 256` of it is `a58344cf664eff2c2209c052ba5d204881bb5b702f3fabfe6bb7597aa73713af`, the value that 016c names.
- All ten steps PASS. Exit 0 after 528 s on gaming. Base main `216ea74e`. Ledger 2552 ids. Mutants: 9 of 9 caught, then 3 caught and 2 unviable of 5.

### Q7: the five merges

For each merge, `git merge-tree --write-tree` of its two parents gives the commit's own tree:

| Merge | Tree | Paths changed on both sides |
|---|---|---|
| `6c722373` | `b1bd546a` | none |
| `a6836763` | `7099e6d5` | `fence_lint.rs` (auto-merged, no conflict) |
| `cf7d0d22` | `87cb89c4` | none |
| `5dac4126` | `fd4b81db` | none |
| `59aa2ad8` | `0708d920` | `probe/tests/slow.rs` (auto-merged, no conflict) |

No merge carries a manual resolution.

### The integration points

- **Wire.** `Exit.output_may_remain` is absent when `false` (`serde(default, skip_serializing_if)`). The `event`, `op-result` and `session-record` schemas gain the optional boolean. `tests.rs` `exit_output_may_remain_is_absent_when_false` round-trips both values, parses an explicit `false`, and validates each `Exited` event against the schema. No existing `Exited` expectation changes.
- **FakeCore.** Only compile fixes: `output_may_remain: false` in each `Exit`, and `WriteForever` holds as `Hold` does. The new `route_end.rs` test closes the `routes.rs:169` MISSED mutant from the first gate (commit `6eb78de9`).
- **Ledger and maps.** `a24_1` is `core-testkit` with `fake: structural` (R-12) in the fake-replacement map, and the map names the FakeCore gap (no writer outlives the payload). `a24_2` is `core-testkit` and runs on FakeCore. The README counts move by 2 (2552 ids; `core-testkit` 465).
- **A20-1.** `TESTKIT_PROVEN_ROWS` reads both class rows from the frozen texts. The list equals the named members (`dp_3`, `a24_1`). The body records the red-on-revert run.
- **The clock of `a24_1`.** The transcript has no `advance_clock`. A24-1 measures the window "on its own clock (R-97)". R-97 item 1 gives the testkit's worker a process-edge clock with a real default, and its follow-up keeps only Core's `stop_grace` kill on the injected clock. So the 100 ms window passes in real time. This matches A24 c2 ("The testkit drives the worker's clock past `stop_grace` after the exit, as for R-97's worker deadlines"). The Core testkit PR that serves `write_forever` must keep this. I check it at the Core pin.
- **The Core testkit PR target.** The body states it from A24-4 and R-110: every worker read finds bytes; only a `fork_child` of `print` steps and then one `write_forever` is modelled; all other `fork_child` scripts stay `RealOnly`. Until it lands, Core keeps `a24_1` pending on its testkit.

### The reviewer checklist (R-115), for `a24_1` and `a24_2`

1. `route_accept` → `advance_clock`: no route. Not applicable.
2. `pty_output` on a gated route: no route. Not applicable.
3. Silence: no silence step. Not applicable.
4. Stall close and capture expiry: none. Not applicable.
5. Output → `oracle_resume`: none. Not applicable.
6. `route_frames_taken`: none. Not applicable.
7. `fork_child` title: **met.** Step 6 (`await_title a24-child`) has the data line in `core-fence-exceptions.txt` (line 266) with the reason `no title sentinel, R-57` and the note "fences only the child's prints". This is the text of R-115's correction. The leader's print T is fenced by its own title at step 8 (R-113: `a24-tail` occurs first at the end of that print). The `ReadScreen` pair at steps 11-15 comes after `Exited`. OU-7 drains the tail before `Exited`, and the two reads check A24-3 (no change after `Exited`), so they need no further fence.

### F1 (LOW): the probe doc says `write_forever` never spins, but control mode spins when the terminal is gone

- `crates/botster-conformance-probe/src/main.rs:7-8`: "If a `write_forever` write fails (the terminal is gone), the step holds as `hold` does: it never returns and never spins."
- Only direct mode does this (`main.rs:173-177`: `break` on a write error, then `park`).
- Control mode (`main.rs:179`) loops on `wire.print(bytes.clone())`. In `control.rs`, `write_chunks` sends `written` whether the write succeeded or not (`:170-176`). If the writer thread is gone, `done.recv()` returns an error at once (`:333-334`). So on a dead terminal each `wire.print` returns at once, and the loop spins. With reports on, it also sends one `Printed` report per turn. The comment at `main.rs:170` ("In control mode the one writer reports no failure") states the cause.
- The PR body scopes the claim correctly ("If a direct write fails"). The module doc does not.
- `a24_1` does not reach this path: `fork_child` removes the control variable (`main.rs:160`), so the child runs in direct mode (`main.rs:198`). A later transcript with a top-level `write_forever` under the control socket would.
- **Fix:** one of these:
  - make the doc say that only a direct write failure holds, and that control mode keeps writing until a signal ends the probe; or
  - make the control-mode writer report a failed write, and hold on it as direct mode does.

## Open items (not findings)

- 016c waits for 016b's retained output of the exception-line reversal. That is package scope.
- At the Core pin: the testkit `write_forever` edge, with the worker clock on its real default for the A24-1 window.

## Final: CLEAN on d4d1f213e8e04ab842bdf49841bf12bf0bda9388 (F1 closed)

- **F1: closed** by `7e486e5`, a doc-only change. `main.rs:7-9` now limits the hold on a failed write to direct mode. It says that in control mode the writer thread reports no failure, so the step writes until a signal ends it, also on a terminal that is gone. This is what the code does (`main.rs:173-179`; `control.rs:170-176`). No code or transcript changes.
- **The merge `d4d1f213`** (main `3d590705` into the branch): tree `05b605d6` is equal to `git merge-tree --write-tree` of its parents. Main's side brings only the three `st_6b` transcripts (#97) and `docs/steward-rulings.md`. No path is shared with this PR. `steward/final43` is still `0bdb72e7`.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-stage0-final43-crate-d4d1f213-pool-20261012-125150-86256.log`, sha256 `25316fdf2a71eff2298d7fd43e3f6d49ae1b0446d72aba2595c1793723419a0c`. All ten steps PASS. Mutants: 9 of 9 caught, then 3 caught and 2 unviable. Exit 0 after 1119 s on linux, base main `3d590705`, ledger 2552 ids.
- **Checklist (R-115):** the transcripts did not change since round 1, so the round-1 ticks stand. Item 7 is met, and items 1-6 are not applicable.

### Landing note (not a finding): the next main merge conflicts

Main is now `49ff52d2`. It contains #98 (`xtask/src/taint.rs`) and #100 (`botster-core-conformance` `driver.rs`, `lib.rs`, `tests/driver_steps.rs`). `git merge-tree --write-tree d4d1f213 origin/main` reports `CONFLICT (content)` in `crates/botster-core-conformance/src/lib.rs`, the `pub use driver::{…}` list that both PRs extend. `driver.rs` and `driver_steps.rs` merge automatically. So the landing head needs a merge commit that resolves the conflict by hand. It also needs my review of that resolution (both export sets kept, no other change) and a gate on that exact head. The rulings-only rule does not apply.

My finding F1 is closed. CLEAN on `d4d1f213e8e04ab842bdf49841bf12bf0bda9388`.

## The landing resolution: 8da7fff1ab5a31842aee24af1dd85cfd2fc638bf (receipt)

- **The parents** are the CLEAN head `d4d1f213` and main `49ff52d2` (#100).
- **The conflict.** `git merge-tree --write-tree` of the parents reports one conflict, in `crates/botster-core-conformance/src/lib.rs`, in the `pub use driver::{…}` list. The commit differs from that automatic merge only in this hunk.
- **The resolution keeps both sides.** Against main, `lib.rs` adds only `TESTKIT_PROVEN_ROWS`, and #100's `KERNEL_STATE_CONTROLS` and `KERNEL_STATE_RECHECK` stay. `driver.rs`, `driver_steps.rs` and `xtask` merged automatically.
- **This PR's own change is unchanged.** Outside `lib.rs`, the stable patch-id of the diff from main (`f95be4b7`) is equal to that of `d4d1f213` against `3d590705`. 016c checked the same, blob by blob.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-stage0-final43-crate-8da7fff1-pool-20261012-133721-19110.log`, sha256 `ac49f81abf4b964c3c73b895cfcadab61714e0bc06b2a86ca69229ac3bd7eed6`. All ten steps PASS. Mutants: 9 of 9 caught, then 3 caught and 2 unviable. Exit 0 after 475 s on msa1, base main `5ce2cff1`, ledger 2552 ids. The two earlier logs for this head (`131633` exit 141, no node; `133456` exit 75, disk) did not start, and they are not results.
- **The target.** `steward/final43` is `0bdb72e7`, an ancestor of `8da7fff1`, so it fast-forwards to `8da7fff1`. Main has since moved to `192d70bc` (#99 and #101). The final43 publish merge onto main gets its own gate and my Q7 receipt, as the steward planned. A test merge of `8da7fff1` with `5ce2cff1` was clean.

Receipt given for `8da7fff1ab5a31842aee24af1dd85cfd2fc638bf` as the landing head of `steward/final43`.

## Q7 receipt: the final43 publish merge 61f3c12836ec45dbe5eb6ec15b712f25678902d8 (branch steward/final43-publish)

- **The parents.** The first is `e02eb0c3`: main `192d70bc` with Sol's merge-review verdict `d848d178` merged in. That merge changes only `M verdicts/amendments.md`, a covered change under #101's rule. The second is `071a938e`: `steward/final43` = `8da7fff1` plus the steward's wording commit.
- **The wording commit** `071a938e` changes only `frozen/current/MANIFEST.md`. It records the crate PR #96 at `8da7fff`, both reviews, the conflict resolution and the tag `manifest-final43`, and it describes `a24_1` and `a24_2`. I checked these facts against this verdict.
- **The tree** `6356b668` is equal to `git merge-tree --write-tree` of the parents.
- **The auto-merged `conformance/core-fence-exceptions.txt`.** A three-way line check against the merge base `49ff52d2` gives exactly base + main's changes (#99: four lines removed, P5's `e3_1` lines moved) + final43's changes (the tenth reason and the `a24_1` step 6 line). Nothing is missing, and nothing is extra.
- **The patch-id.** The merge's diff from its first parent (30 paths) has the stable patch-id `d0a2edc3`. That is equal to the patch-id of final43's own diff (`071a938e` against the merge base `49ff52d2`).
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-steward-final43-publish-61f3c128-pool-20261012-135224-3645.log`, sha256 `42ab4a90149acac171b07ce53b511b9d1d26237cabfb96cc37468907877215f8`. All ten steps PASS. Mutants: 9 of 9 caught, then 3 caught and 2 unviable. Ledger 2552 ids. Exit 0 after 735 s on gaming, base main `192d70bc`.
- **Main** is still `192d70bc`, an ancestor of `61f3c128`, so main fast-forwards to it.

Q7 receipt given for `61f3c12836ec45dbe5eb6ec15b712f25678902d8`.

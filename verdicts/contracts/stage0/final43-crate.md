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

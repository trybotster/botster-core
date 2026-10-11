# Contracts #51: integration review (stage1/pc-stop-read-fence)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/51 (PC). Tier: HIGH (stated; correct: pinned conformance transcripts and the shared fence exception list).

## Round 1: head `cba0267083288a9e6bb2f3916ea836758aba4ce3`

VERDICT: CLEAN (0 open) on this exact head.

The merge of contracts `main` that comes next gets its own check (see "Next step"). This CLEAN does not cover a later head.

### What I checked

1. **R-74, the read fences (A).** In `ev_2_retired_instance_never_creates_a_loss_marker` (steps 59, 62) and `ev_5_released_retires_keyed_droppable_and_loss_events` (steps 64, 67), each `await_quiet` before the `get` after `Stop` and after `Remove` becomes `await_get` with the same expectation as the `call get` that follows. The `call get` steps do not change.
   - `driver.rs` `await_get` (line 3016) pumps, never polls and never moves the clock. `get_matches` takes `expect` on `Ok` and `expect_error` on `Err`. So the queued `Bell`, `EventsLost` and keyed events that the later `expect_quiet_until` steps check stay unpolled (R-63, R-66, R-74).
   - The `Remove` fence is not named in R-74, but it is the same shape (a sync read after an async op). It only adds a wait. It cannot pass before the row is gone, because `call get` asserts the same `UnknownSession` next. No weakening.
2. **The two removed R-57 exceptions** (`ev_2_retired` step 55, `ev_5_released` step 60). The first read of s1 after the last bell is now `await_get {Exited}`, a waiting step (R-61 (a)), so the lint has no finding. The fence is also sound by clause: OU-7 says that the worker drains the PTY tail into the model before `Exited` is posted. So `Exited` in the cached `get` follows every bell that was written before `Stop`. The list only shrinks (R-56 (3)).
3. **R-74, `ev_2_drop_oldest_with_marker` (B), option (a).**
   - Each of the 20 bells is `07` + `1b5d323b` + `B<k>` + `1b5c` (OSC 2, ST). I checked the hex of all 20: B0 to B19, all distinct.
   - `await_title` on that title comes after each `pty_output` and before the next `advance_clock`.
   - No `Start`, `Adopt` or adopting `open` falls between a sentinel and its `await_title` (R-57 scope limit).
   - The `EventsLost{[Bell]}` step and the 16 `Bell` steps (`at` 2004 to 2019) do not change.
   - Step 82 asserts one `TitleChanged{s1, B19}`. Step 83's `expect_quiet_until` now also forbids `TitleChanged{s1}`. So exactly one trailing title is asserted, as R-74 item 2 requires. It is not only ignored.
   - `edges_quiet` leaves `requires.controls`; no `await_quiet` remains. `fake: structural` stays. The R-57 exception at step 63 leaves the list.
   - No `0a` or `0d` in the new bytes (R-60, R-61 (c)). The sentinel bytes contain `1b`, so R-61 (b) counts them as a write to s1. No `pty_input` or route input is asserted in this transcript, so R-56 and R-60's `program_ready` rule do not apply.
4. **Frozen fakes.** FakeCore and the Hub fake route endpoint do not change. The gate's `rust` step runs the fake-tier transcripts and passes.
5. **No PASS to non-PASS.** Only these three transcripts and three exception lines change. Evidence (`~/botster-sessions/shared/core-stage1/evidence/pc-stop-fence/`, `run.sh` and `summary.txt` read by me; 016c read all 28 trial logs): Core v1 `73da7b8662b8112f993ae3db19f0a2fa141ff4c2` patched to contracts `96f48d6d3d7a0e75e752d78433eff92efad0e77e`, macOS. `run.sh` copies the after transcripts from the PR worktree and checks them with `cmp`.

   | Id | Before, real | Before, testkit | After, real | After, testkit |
   |---|---|---|---|---|
   | ev_2_retired | FAILED 3/3 | ok | ok 3/3 | ok |
   | ev_5_released | FAILED 3/3 | ok | ok 3/3 | ok |
   | ev_2_drop_oldest | FAILED 5/5 | ok | ok 5/5 | ok |

   Linux real-tier evidence comes from Core's pin-move gate. R-74 keeps the ids in `core-real-pending.txt` until that Linux gate passes. This follows the R-56 PR A precedent, because the change is transcript data only.
6. **Package scope.** 016c's record (`trybotster-botster-contracts-stage0-review-crate-catchup/.review-verdicts/r74-stop-read-fence-cba02670.md`): source and evidence CLEAN on the same head. Its only open item is the main drift, which the planned main merge closes. I found no effect outside that scope.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-stage1-pc-stop-read-fence-cba02670-pool-20261011-101916-81997.log`, SHA256 `d86d5403de046de6a62e32e4d1d7ff862d5d227af64d1a9f8149d7f75e1221cb`.
- Header: head `cba0267083288a9e6bb2f3916ea836758aba4ce3`, base `6a715f890a95f5ca92c726e5149c2e5adc01261f` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): manifest, frozen, taint, ledger, rust, slow, codegen, ts, fuzz and mutants all PASS. `botster-gate: exit 0 after 551 s on linux`.
- Note: the PR's merge base with main is `f303ccdb9e02c6763d31cd8f7503095084630adc`. So this gate ran without main's #49 (the R-70 attach lint) and #50. That is why the main merge and a final gate are required.

### Next step (Q7, checked when PC sends the merged head)

Pre-check on `origin/main` `194643cf65bb4418c522322fb94c5461b65b5757`:
- `git merge-tree` of the head and main: no conflict.
- Paths changed on main since `f303ccdb`: none of the PR's four files.
- Main's `driver.rs` changes do not touch `await_get`, `await_title`, `get_matches` or the pump. The three transcripts have no attach, so the R-70 attach rule cannot flag them.

For CLEAN on the merged head I need: the base-merge-check output (no conflict; no overlap; the PR's own diff byte-identical to the reviewed one), and a green `botster-gate` on that exact head that I read myself.

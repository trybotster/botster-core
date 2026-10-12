# contracts #100 — stage1/pc-kstate-recheck (R-111, KSTATE-1: `await_control` re-checks kernel-state controls), reviewer 017f

HIGH (rule 3: the shared driver). Implementer: PC (sess-…0178). Package reviewer: 016c. PR base: main `8aaee62d`. Gate base: main `216ea74e`. Four commits: `cb6e670` → `7874f71` → `e15f774` → `3da4b687`. Files: `botster-core-conformance` `src/driver.rs`, `src/lib.rs`, `tests/driver_steps.rs`.

## Round 1: CLEAN on 3da4b6877813e74fc368bdc58018ff353a141dbf

### The gate

- `~/botster-sessions/gates/botster-contracts-stage1-pc-kstate-recheck-3da4b687-pool-20261012-124330-59263.log`. My `shasum -a 256` of it is `6cc9d1d61b849401ca6693861a120d59adea10290f6d12372db9e4a605faa0f1`, the value in the body.
- All ten steps PASS. Exit 0 after 638 s on gaming. Mutants: 4 tested, 4 caught.
- The earlier gate on `7874f715` was red with a mutant TIMEOUT. The body names it and its cause.

### R-111, item by item

1. **The list.** `KERNEL_STATE_CONTROLS = ["pty_queued"]`, one named constant. `route_frames_taken` joins it in its own PR, which lands after this one. The body gives the owner's review of `pty_output_unread`: Core's testkit serves it from the in-memory program edge, and the real tier does not serve it. Its one use (`ou_7` s13) is a `control`, not an `await_control`. Met.
2. **The re-check.**
   - `KERNEL_STATE_RECHECK` is 5 ms, under the 10 ms limit.
   - `recheck_until_edge` waits on the wake handle in slices. After each slice that times out, it calls `control_matches` only. It never pumps, never polls events and never moves the clock.
   - It reads the deadline on every pass, so the step deadline bounds it (R-89).
   - A wake returns to the step loop, which pumps and checks, as before. With no step deadline (a fake), it is `wait_for_edge`, as before.
   - Every other control still goes through `wait_for_edge`.
   
   Met.
3. **Tests**, each red on revert (9 of 9 parts in `evidence/pc-kstate/red-on-revert-3da4b687.log`):
   - a listed control that changes with no edge ends the step in under 1 s with one pump;
   - an unlisted control is read exactly twice and ends at its deadline;
   - a re-check consumes no event (the `Created` event is still queued);
   - the deadline bounds the re-check (5 to 23 reads in 100 ms, one pump).
   
   Met. Each test runs in real time for at most about 200 ms.
4. **The Linux fill.** `ptyfill-linux.out` (msa1, kernel 6.12.111): the first write takes 11,776 of 40,960 bytes, and the rest waits, so `in_8` step 8 needs no change. `FIONREAD` reads 0 right after the write, and first reads non-zero after 104-120 µs. Then it reads 4,095, which is inside the fence's `[1, 40960]`. Met.

- **R-114's condition:** the body names `tm_6`, `in_2`, `in_6`, both `in_9` ids and `in_8`. Met.

### Notes (not findings)

- **The scope is wider than the table.** At this head, 34 `await_control pty_queued` steps in 28 ids take the new path, not only the six in the table. Examples are `a2_2`, `am_2`, `dp_5`, `e4_1` (3), `ev_8` (6), `in_2_partial`, `in_6_cancel`, `in_7` and `st_7` (7). The change can only end a wait earlier, when the control already matches, so no id gets a weaker check. The gate ran the whole corpus. The table's label ("the ids it covers") reads as the full set, but it is the audit's list of race sites.
- **Main moved.** Main is now `e5709067`. Since `8aaee62d` it changed `fence_lint.rs`, `fence_lint/written.rs` (#94), `BUILD.md`, rulings and transcripts. None of these is a path of this PR. This is not a rulings-only move, so the landing head needs a main merge, my Q7 receipt and a gate on that exact head.

CLEAN on `3da4b6877813e74fc368bdc58018ff353a141dbf`.

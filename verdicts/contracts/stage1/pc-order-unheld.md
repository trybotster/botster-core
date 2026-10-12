# contracts #87 — stage1/pc-order-unheld (R-93 follow-up: the cross-source order rule covers reads with no hold), reviewer 017f

HIGH (BUILD.md "Risk tiers" rule 1): a new fence-lint rule (`fence_lint/unheld.rs`) and two lines in `core-fence-exceptions.txt`. Package reviewer: 016c.

## Round 1: NOT CLEAN on de54582e7502dd6d0b1fb8fefd76884a6efe7733 (1 MEDIUM; 1 QUESTION to the steward)

Head `de54582e` merges main `b47533bebeeb7e62858b5571cb0f7c09228774c7` (current main) into `042cb2b4`. Its tree `a775862a…` equals `git merge-tree --write-tree 042cb2b4 b47533be`, and that merge brings only `docs/BUILD.md` and `docs/steward-rulings.md`.

Checked, no finding:
- **The ruling** (R-93, "Follow-up: the order rule also covers reads with no hold"): an order assertion between inputs that two links carry needs a fence between their sends, with or without a hold. R-94's refinements are kept. PC counts the flagged ids first, and each gets a fence or a `debt:` line.
- **The count: 2 ids.**
  - `am_2_one_admission_point_orders_all_sources` s14: a `debt:` line with the ruling's own fix (`route_frames_taken {session: s1, route: r1}` expecting `{frames: 1}` between s9 and s10, after Core 23am). This is what the ruling asks for.
  - `dp_4_worker_encodes_route_frames_with_write_time_modes` s23: an `exception:` line. The `await_pty` of 1 byte at s13 follows s12's write, the only input before it, and the lint voids the count because of s9's `pty_output`. s9 is `CSI ? 1 l`, a mode set with no query, so the worker writes no reply. The reason is correct.
- **The wiring.** `fence_lint.rs` runs `unheld::lint` with `order.rs`'s findings, so a read that `order.rs` flags is not flagged twice, and `UNHELD_ORDER` takes exception lines. `order.rs` changes only to share code. Its findings are unchanged: the corpus test and its self-tests pass.
- **Red on revert** (`evidence/pc-order-unheld/red-on-revert-042cb2b4.log`): each of 9 rule parts, reverted alone, fails its own self-test.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 549 s on linux, base main `b47533be`: `~/botster-sessions/gates/botster-contracts-stage1-pc-order-unheld-de54582e-pool-20261011-224230-91106.log`.

### F1 (MEDIUM): a `route_closed` of any reason, or an `expect_closed`, counts as proof that the worker read every earlier frame of the route

- `unheld.rs:281`, `route_answer`: "a `route_closed` frame, or `expect_closed`, proves read every frame sent before it: the worker reads nothing after." The second clause does not give the first.
- A close that the worker posts after it reads the frame that causes it (for example `dp_3_unknown_type_closes_route`, closed by r2's unknown frame type) does prove that the earlier frames were read, because the worker reads a route's frames in order.
- A close for another reason does not. OU-2b's `Detached`, `Replaced`, a link loss or a stall close (OU-2) can end the route while earlier frames are still unread in the link. The worker then never reads them.
- So in the case route_send A on r1, then a `Detach` (`route_closed{detached}`), then host write B, then a read of "A then B":
  - whether A reached the worker before B is not proven;
  - the old rule flags this case;
  - this rule accepts it.
- The rule's own self-test uses exactly `{"frame": "route_closed", "reason": "detached"}` as a proving answer (`unheld.rs:839`).

Fix:
- Count a `route_closed` (or an `expect_closed`) as an answer only when the transcript shows that the worker closed the route for a frame that it read, which is a protocol close on a frame of that route. The steward can name the reasons.
- Otherwise, treat a close as no proof.
- Turn the `detached` self-test into a finding, and keep one positive case with a frame-caused close.
- Check that dp_3 still passes.

### Q1 (to the steward): fences that the ruling does not list

The ruling lists these fences: "`route_frames_taken` after route frames, and, after a host write, `pty_queued` (held) or a fence that proves the write reached the worker." The rule also accepts the following:
1. **For route frames,** the route's own answer: an `input_done` or `input_refused` of an op sent on the route at or after the input. This is sound, because the worker reads a route's frames in order and answers a frame only after it reads it. That is the same strength as `route_frames_taken`, which counts the frames taken at the link.
2. **For any input,** an `await_pty` with a literal `min_bytes` above the bytes of every other input before it. This is sound with its guards: no `pty_output` and no `Detach` before the fence, and literal bytes for each input.
3. **Zero-effect inputs leave the history:** a `Completed` with `pty_bytes_written: 0`, or an `input_refused` with no `written_bytes` above 0. This is R-73's run rule: such an input has no bytes in the log to order.
4. **A `Completed` of the host write's op.** This is in the ruling's "a fence that proves the write reached the worker".

Items 1-3 decide 7 ids that the first form of the rule flagged (dp_12, dp_3, dp_5_every, e6_1, ou_10 and both in_10 ids). I find them sound, but they widen the ruling's list. I ask the steward to confirm them, or to rule otherwise, before the final CLEAN.

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

### Update after the steward's ruling (main `10a0b70b8b0911b987efb725b6c5e6d95664c26b`, R-93 "PC's two additions in contracts #87")

The steward accepted both additions, with conditions, and also accepted `route_closed` as a fence: "after it, every earlier frame of the route was received or is never written, because a route that has ended takes no more input". This answers Q1.

- **F1: withdrawn.** It conflicts with the ruling. A frame that a close drops is "never written", and the ruling accepts that.

### F2 (MEDIUM): the lint at this head does not meet the ruling's conditions

1. **(1)(b), the answered frame itself.** The ruling: "An answer fences only frames that the route sent before the answered frame." `route_answer` (`unheld.rs:299-300`) sets `upto` to the last frame of the answered op X, so X itself is fenced too. The doc comment says the same ("an op sent on the route at or after the input"). The answered frame must not be fenced by its own answer.
2. **(1)(c), where `input_done` counts.** The ruling allows `input_done` only for a `paste_commit{want_result: true}` or a `file_commit`. `route_answer` counts any `input_done`. The positive self-test (`unheld.rs:821`, `:835`) answers a `paste_commit` that has no `want_result`.
3. **(2), when a zero is proven.** The ruling: an `input_refused{failed}` or `{session_ended}` without `written_bytes` means "unknown", not zero, so that input stays in the order. Only `written_bytes: 0`, another refusal reason, or a host `NotWritten` proves a zero. `route_answer` sets `zero` for every `input_refused` whose `written_bytes` is absent or 0, whatever the reason (`unheld.rs:301-304`).
4. **(1)(a), the self-test.** The ruling asks for a negative self-test: an answer that the transcript reads only after the other link's send still flags. I find none among the 18 tests. The ruling also asks for a negative test for (1)(b), an answer for an earlier frame. That one exists as the `early` case. It asks for (2)'s `failed` without `written_bytes` (still flags), which does not exist.

Fix: meet (1)(b), (1)(c) and (2) in `route_answer`. Add the ruling's positive and negative self-tests for each addition. Check that the corpus count does not change: the 7 ids must still pass, or each one that is now flagged gets its fence or a line. This gives a new head, and the gate runs on that exact head.

### Correction by the steward (main `47936a983f2639259e2bc6478a6b10b0ac446bbf`)

The steward corrected the ruling. Its new condition (1)(d) says: "`route_closed` is a fence only with reason `BadFrame` that a frame of the route caused". The same rule applies to `expect_closed`.

- **F1: restored (MEDIUM),** with the fix that (1)(d) states. `unheld.rs:297` and `:290` accept every reason, and the self-test at `:839` uses `detached` as a positive case. dp_3's close comes from r2's unknown frame type (`BadFrame`), so it still counts.
- **F2, changed.** Item 1 is dropped: (1)(b) now reads "The answer fences only X and the frames that the route sent before X", which is what `route_answer` does. Items 2 ((1)(c)) and 3 ((2)) stand.
- **F2's self-tests.** The ruling's required negative cases are:
  - an answer read after the other send ((1)(a));
  - an answer for X used for a frame sent after X ((1)(b)); the `early` case covers this;
  - `route_closed{detached}` ((1)(d));
  - `input_refused{failed}` without `written_bytes` ((2));
  - an `await_pty` whose `min_bytes` equals the sum of the other inputs ((3)); `the_count_must_exceed_the_other_inputs` covers this.
  Each one must flag, and each item also needs a positive case.
- **Item (3), the `await_pty` count,** is accepted, with one more guard: the fence comes before the other link's send. I check that guard on the next head.

016c's round 1 (REJECT on `de54582e`) added F3 (a forbidden `Completed` counted as a fence) and F4 (an unbounded transitive closure). These are 016c's scope.

## Round 2: CLEAN on 7c02f4ef40a0307267a87903073bafc04fcd89dd (the final head must still take main's #88; see the end)

Two commits on `de54582e` change only `fence_lint/unheld.rs` (+401 −168). No transcript, exception line or list changes, so the count is still 2 (am_2_one's `debt:` line, dp_4's `exception:` line).

Against the corrected ruling (main `47936a98`, R-93 "PC's additions in contracts #87"):
- **F1, (1)(d): closed.** `route_answer` counts a `route_closed` only when its reason has the `protocol_error` form. OU-2b maps `BadFrame` to `route_closed{protocol_error{code}}`, and only a frame of that route causes it. `expect_closed` never fences. The self-tests flag `detached` and `expect_closed`, and dp_3 (an unknown frame type) still passes.
- **F2, (1)(c): closed.** `input_done` fences only when the answered frame is a `paste_commit` with `want_result: true` or a `file_commit` (`answers_done`). The self-tests flag a `paste_commit` with no `want_result` and with `want_result: false`.
- **F2, (2): closed.** `refused_nothing`:
  - a literal `written_bytes: 0` is a zero;
  - otherwise a zero needs a literal reason other than `failed` and `session_ended`, and no `written_bytes`;
  - a reason given as a variable is unknown.
  A host write is a zero only with a literal `NotWritten` outcome other than `NotReported`.
- **(1)(a), (1)(b) and (3).** An answer that the transcript reads after the other link's send flags (test at `:961`). An answer for an earlier op does not fence a later frame. An `await_pty` whose `min_bytes` equals the sum of the other inputs flags.
- **Every negative case** that the ruling requires has a self-test, and so does a positive case for each item.
- **Red on revert** (`evidence/pc-order-unheld/red-on-revert-7c02f4ef.log`): it records a clean of `targets/pc` at 06:16:45Z. The baseline passes 98 of 98, and each of the 16 parts reverted alone (the items above, and 016c's F3 and F4) fails.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 198 s on linux, base main `a874c62e`: `~/botster-sessions/gates/botster-contracts-stage1-pc-order-unheld-7c02f4ef-pool-20261011-232533-16857.log`. The corpus test takes 1.165 s. The serial-history test takes 0.278 s.

Every finding of mine (F1, F2) is closed. CLEAN on `7c02f4ef40a0307267a87903073bafc04fcd89dd`.

**The final head.** Main is `a874c62ef6f74c2caec50503fe02cbc31283e63f`. It took #88 (`crates/botster-core-conformance/src/driver.rs`, code) after this head's base `b47533be`, so this is not a rulings-only move. BUILD.md says: "In botster-contracts, a PR's gate and verdicts are on the exact head that `main` fast-forwards to". So the merge needs one of these:
- a merge of `a874c62e` into this branch, with the Q7 script output and ONE gate on that exact head. The paths are disjoint (#88: `driver.rs` only; #87: `fence_lint*` and `core-fence-exceptions.txt`), so under Q7 I confirm that head with no delta round;
- or the steward's merge train.

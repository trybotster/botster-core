# contracts #91 — p6/pty-output-written (R107-SI-1 and R107-SI-2: the `pty_output_written` control, the R-57 fences of `ev_1` and `ev_2_retired`, the sweep's two fences), reviewer 017f

HIGH: rule 1 (the held lint, `fence_lint/held.rs`) and rule 3 (`botster-probe-script`, `botster-conformance-probe`, `botster-core-conformance` tests). Implementer: P6. Package reviewer: 016c. The steward named me as the integration reviewer and merges.

## Round 1: CLEAN on 8b035f245976a493b771fb9d878d777afdb469c8 (the final head must take main 17cb4281)

The PR's parent is main `fe209c6563fa8943065ef1aaa3e09a7e3154a201`. Checked against R-107, R107-SI-1 and R107-SI-2 (`docs/steward-rulings.md` at main `17cb42818e0510e107e6e43253c1060ab403c9ec`):

- **The control (R107-SI-1 Q1).**
  - `botster-probe-script`: `Request::Written {}` and `Report::Written { bytes }`, with wire-form cases. The module doc and `Request::Output`'s doc comment say that the acknowledgement of an `Output` means "queued in the writer's order", as the ruling requires.
  - `botster-conformance-probe`: the queue is typed (`Chunk::Output`, `Chunk::Print`). `write_chunks` adds an `Output` chunk's length only after `write_all` and `flush` return `Ok`. A print is not counted, and a failed write counts nothing. The serve loop answers `Written` with the count and changes nothing, so the read holds no reference and adds no wait inside the probe (R-14.3). The `pty_output` acknowledgement still does not wait for the write (OU-3d).
  - With `PRINTS_ENV`, an `Output` that is deferred behind a print is not queued, so it is not counted until it is written. That agrees with the control's meaning.
  - `docs/core-testkit-controls.md`: the control row (after `pty_queued`) gives the real-tier and testkit forms, FakeCore `structural`, and R-14.3. It says that the control does not prove the worker's read, so an output-derived read still needs `await_title` (R-57). The probe section changes `pty_output` to "queued" and adds `pty_output_written`.
  - `vocabulary.rs` lists the control with `session` only. `held.rs` counts it as a probe read, with a self-test case.
- **`ev_1_replaced_keyed_event_moves_to_latest_position`.** `await_title {s1, "T3"}` comes directly after step 10 (OSC 0 `T3`), with no sentinel, as ruled. The only earlier title is `T1` (step 5), so the fence cannot pass early. No launch or adoption lies between the output and the fence.
- **`ev_2_retired_instance_never_creates_a_loss_marker`.**
  - Step 122 (121 before the sweep fence) appends the OSC 2 sentinel `p6-si-1`, ST-terminated, to s2's BEL. `await_title {s2, "p6-si-1"}` follows. OSC 2 with ST is a valid sentinel (R-57 follow-up, PC lint PR B).
  - The sentinel's `TitleChanged {id: s2}` is in the `ignore` list of step 125 (`expect_eventually EventsLost {s2, ["Bell"]}`). Step 126 forbids only s1's `Bell` and `EventsLost`, so it cannot see the new event. No other step follows.
  - **The condition.** s1 is stopped at step 58 and removed at step 61. s2 is never stopped. So the title belongs to s2's live instance. `TitleChanged` is keyed, not droppable, so the loss marker's kinds stay `["Bell"]`.
- **The exception lines.** `core-fence-exceptions.txt` loses exactly the two revoked lines (`ev_1` step 10, `ev_2_retired` step 121). No other line names `ev_1`, `ev_2_retired` or `ev_5_released`, so no line carries a step number that the inserted steps move.
- **The sweep fences** (the steward's answer: fences, no exception lines). I counted each session's `pty_output` bytes before the fence:
  - `ev_2_retired`: sixteen 1-byte BELs on s1 (steps 10-55), so 16; `await_control pty_output_written {s1}` `{bytes: 16}` at step 57, directly before the `Stop` at step 58.
  - `ev_5_released`: OSC 0 `TR` (8 bytes, step 9) and seventeen BELs (steps 12-60), so 25; the fence `{bytes: 25}` at step 62, directly before the `Stop` at step 63.
  - Both list `pty_output_written` in `requires.controls` and stay `"fake": "structural"`.
- **`ev_2_mandatory_and_keyed_never_dropped`** does not change (R107-SI-2, option A').
- **Gate PASS on the exact head**, all ten steps, exit 0 after 200 s on msa1, base main `17cb4281`; mutants 6 tested, 6 caught; the slow test `pty_output_written_counts_an_output_only_once_its_write_returned` 0.465 s; the two new unit tests 0.003 s: `~/botster-sessions/gates/botster-contracts-p6-pty-output-written-8b035f24-pool-20261012-102748-60004.log`.

### The main move (the merge rule, not a finding)

`git diff fe209c65 17cb4281` changes `docs/steward-rulings.md`, three Core amendment files (A23 c1 and c2, A24 c1, A25 c1) and `verdicts/amendments.md`. That is data, but it is not rulings-only, so main cannot fast-forward to `8b035f24` under BUILD.md's rulings-only rule. The head that main becomes (a merge of main into this branch, or a train head) needs its own gate. #91's paths are disjoint from that move. But #91 changes a list file (`core-fence-exceptions.txt`), so under the merge-train rule I give a delta round on the train head.

### Open items (not findings)

- **The sweep lint.** R107-SI-1 says that "a lint for this shape follows in the same PR". This PR carries the sweep fences and no lint, and the body puts the lint in its own contracts PR. The steward's backlog (B-4: "After #91 and the sweep/lint PR") and the lead's record ("sweep/lint after") show that order, so I accept it. Its integration review is a later PR.
- **Core.** The real tier does not forward an unknown probe control (`real/program.rs`), and the testkit tier needs the "total at once" form. So `ev_2_retired` and `ev_5_released` need P6's Core testkit PR before Core takes a pin with this patch (plan 23am: the control PRs merge before the pin PR).

No open finding. CLEAN on `8b035f245976a493b771fb9d878d777afdb469c8`.

016c's round 1 (REJECT on `8b035f24`): one MEDIUM, package scope. The slow test polled `written()` in a loop, against BUILD.md Testing rule 5. I do not re-review it. The steward approved the separate lint PR on main `f1ccacfd` ("R107-SI-1 sweep/lint split"), so my first open item is closed.

## Delta round: CLEAN on 9ab1ad6572bc6037d87fd25c7c27ed928aaec62b

- **`0c52ae32`** (016c's finding) changes only `crates/botster-conformance-probe/tests/slow.rs`. The program is now `print_after_input "m" -> "Z"`. After every `Output` is acknowledged, the test writes "m", reads all N bytes and then "Z", and reads `written()` once. "Z" goes through the same writer queue behind every `Output` chunk, so the one read sees the total. No transcript, no list file and no lint changes.
- **`9ab1ad65`**, the merge of main `f1ccacfd2794775176009c9dddbff67f43858840` (Q7):
  - the tree `81ad415372fbadbd9b8caed2cffc1767bfa2f96e` equals `git merge-tree --write-tree 0c52ae32 f1ccacfd`;
  - `git patch-id --stable` of #91's own diff is `698ca723…` against both its parent `fe209c65` and main `f1ccacfd`;
  - main's move since `fe209c65` changes only `docs/steward-rulings.md`, the A23 c1/c2, A24 c1 and A25 c1 amendment files and `verdicts/amendments.md`. None is a path of #91. No side changes a list file after the parent, and the merge leaves `core-fence-exceptions.txt` the same as at `0c52ae32`. So the id set is #91's set: the two revoked lines out, nothing added.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 182 s on msa1, base main `f1ccacfd`; mutants 6 tested, 6 caught; the slow test 0.381 s: `~/botster-sessions/gates/botster-contracts-p6-pty-output-written-9ab1ad65-pool-20261012-103604-92101.log`.
- **Main after the gate.** Main is now `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`. `git diff f1ccacfd 7e97064c` adds `amendments/core-contract-v1.17-amendment-24-candidate2.md` and changes `verdicts/amendments.md`. That is not rulings-only, so main cannot fast-forward to `9ab1ad65`. The steward decides the merge. If the head that main becomes is a new merge, I check its delta per Q7.

Every finding is closed. CLEAN on `9ab1ad6572bc6037d87fd25c7c27ed928aaec62b`.

## Final merge receipt: CLEAN on 6a8ba018edfa46c429ea9aaed5a6b12b7ca5d899 (the steward's merge into main)

The steward chose a final merge of main into the reviewed head. The merge commit is local to the steward's worktree until this receipt.
- The parents are main `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1` and the reviewed head `9ab1ad6572bc6037d87fd25c7c27ed928aaec62b`.
- The tree `cd2f5bdd46da3e1020272665099a4222049dedaf` equals `git merge-tree --write-tree 7e97064c 9ab1ad65`.
- `git diff --name-only 9ab1ad65 6a8ba018` gives exactly `amendments/core-contract-v1.17-amendment-24-candidate2.md` and `verdicts/amendments.md`. Neither is a path of #91.
- `git patch-id --stable` of `git diff 7e97064c 6a8ba018` is `698ca723…`, the same as #91's own diff against `fe209c65` and `f1ccacfd`.
- `core-fence-exceptions.txt` differs from main only by #91's two removed lines.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 219 s on linux, base main `7e97064c`; mutants 6 tested, 6 caught: `~/botster-sessions/gates/botster-contracts-detached-6a8ba018-6a8ba018-pool-20261012-104152-3370.log`.

CLEAN on `6a8ba018edfa46c429ea9aaed5a6b12b7ca5d899`. Main may fast-forward to it if main is still `7e97064c`.

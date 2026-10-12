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

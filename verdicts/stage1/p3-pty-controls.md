# Integration review: #195 (testkit pty_output and pty_blocked; branch stage1/p3-pty-controls)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 46642d85

Reviewed head: `46642d85de52c98ce8fe1cfbd0bf7afa27847c4d` (`e3537c57` the change, `3b43213a` the merge of v1 `1dd1657a`,
`46642d85` the tests). v1 is still `1dd1657a`. The stated tier is HIGH by rule 3 (the shared testkit), which is correct.

- **The merge.** Its tree is the tree of `git merge-tree --write-tree e3537c57 1dd1657a`.
- **Edge only.** Both controls act on the payload's `ProgramControl` (the program edge). The worker machine and the host do
  not change. `pty_output` injects plain output (`ProgramControl::write`: not atomic, so the scheduler chooses the read
  boundaries) and wakes the owning host. `pty_blocked` sets the block, and `on: false` wakes the host.
- **The new per-worker state, on every end path.** `ProcessCell::program` is set at `spawn_payload`, and it is cleared at
  `ReapPayload` and in `ended()` (the worker's own exit, and its end by a signal of the process edge). `program_edge` also
  refuses an ended worker before it reads the field. A dropped handle is refused by `session_row` (#191's PC-F1 fix).
- **Arguments.** `parse` denies unknown fields. An empty, odd-length or non-hex `bytes_hex` is `Bad`.
- **No flip.** `core-pending.txt` does not change. The PR states two limits: a blocked write cannot be observed through the
  harness yet (no input write), and the M1 worker reports no output, so the output test proves only that the worker reads
  every byte.
- **The gate log** (`…-p3-pty-controls-46642d85-pool-20261009-105935-10918.log`) names the head and base `1dd1657a`. The
  default tier runs 979 tests and the slow tier 243, all pass. Mutants: 10 caught, 0 missed, 5 unviable. Exit 0.

Observation (not counted): between the payload's exit and its reap, `pty_output` still injects bytes. A real PTY gets no
new writes after the program has exited; only bytes written before the exit remain to read. A transcript that injects
output after an exit step would then test something that cannot happen. When a transcript uses `pty_output` near an exit,
the control could refuse a payload that has exited.

VERDICT: CLEAN (0 open) at 46642d85de52c98ce8fe1cfbd0bf7afa27847c4d

### Correction after round 1 (same head 46642d85) — CLEAN WITHDRAWN (the P3 package reviewer's F63, missed here)

The P3 package reviewer's F63 MEDIUM is real, and it is in this reviewer's scope (shared testkit code). `program_edge`
(`worker.rs:234-242`) keeps the `ProcessCell` guard while it locks `owner` (the host's `Processes`) to read the wake.
`Processes::end` (`worker.rs:102-110`) runs under the `owner` lock (`lock(&self.processes).end(...)`, from `Action::Exit`
and from a kill by `signal_group`) and then locks the cell. `ProcessTable::holds_reports` also locks owner, then cell. So
the order is reversed. If one thread runs a program-edge control while another ends the worker, each waits for the other's
lock. `break_link` drops the cell guard before it locks `owner`. This reviewer checked the new field's end paths, but not
the lock order. Fix: F63's (drop the cell guard after the clone of `ProgramControl`, before `owner`; add a bounded test of
a control that runs at the same time as the process end).

VERDICT: NOT CLEAN at 46642d85de52c98ce8fe1cfbd0bf7afa27847c4d (1 open: F63 MEDIUM, the package reviewer's finding,
confirmed here)

## Round 2 — CLEAN on head ab430951 (F63 fix and the union with #196)

Reviewed head: `ab430951006a5aeee2f054b363679940356d9bbb`, four commits on `46642d85`: `497771a8` (F63), `0cf3d8d6` (timer
markers), the merge `491966a6` of v1 `1f157c29` (#196), and `ab430951`. The head contains v1 `1f157c29`, which is the
current v1. P3's gate log `gates/botster-core-stage1-p3-pty-controls-ab430951-pool-20261009-112122-4292.log` names this
head and base `1f157c29`. It is a full gate: `cargo xtask ci` (993 default, 243 slow) and the mutants job (18 mutants: 13
caught, 0 missed, 0 timeout, 5 unviable), exit 0. This reviewer read its header and summaries. This round is also the
union review of #195 and #196 that the round 1 condition and #196's verdict ask for.

- **F63 closed.** `program_edge` takes `run_processes`, clones the pair, and releases it. It then reads the cell in its own
  scope and releases the cell before it locks the owner. So no path takes the cell and then the owner, and the order
  run_processes -> owner -> cell holds. The new test runs `program_edge` while another thread holds the owner, then ends
  the process. Its two waits are bounded (`recv_timeout`) and carry the timer marker.
- **The merge.** `git merge-tree --write-tree 491966a6^1 491966a6^2` has conflicts in `controls.rs`, `worker.rs` and
  `worker/tests.rs` only. The merge commit differs from the conflicted tree only in those three files. The resolution
  keeps both sides:
  - `registered_controls` registers `pty_controls` and `start_controls`;
  - `ProcessCell` has both `payload_alive` and `program`;
  - `Workers` has `hold_start`, `release_start`, `payload_alive` and `program_edge` (the F63 form);
  - `WorkerEdges::ended` and `Action::ReapPayload` clear both fields under one cell guard, and `ended` still removes the
    held start (`StartKey`) and the held spawn;
  - `spawn_payload` sets `program` and then `payload_alive`; `poll_payload_exit` clears only `payload_alive`, so the PTY
    stays readable until the reap;
  - both sides' tests stay.
- **`ab430951`.** `pty_output` refuses (`Bad`) when the session's payload has exited (`Workers::payload_alive`, which takes
  run_processes -> cell). The text and the doc agree. `pty_blocked` keeps working until the reap, which matches the field
  doc of `program`.

Observation (not counted): `pty_output` checks `program_of` and `program_alive` in two steps, and `spawn_payload` sets
`program` and `payload_alive` under two guards. A payload exit between the checks lets one write reach a program that
has exited, as at round 1. A transcript sends `pty_output` after its start completes, so neither window changes a
result.

VERDICT: CLEAN (0 open) at ab430951006a5aeee2f054b363679940356d9bbb

### Note after round 2 (same head ab430951) — the P3 package reviewer keeps F63 open for its proof (package scope)

The round 2 text above says that the F63 test's two waits are bounded. That is wrong for one of them:
`worker/tests.rs:415-417` is a 50 ms `thread::sleep` that orders the two threads, and it has a `timer: deadline` label.
BUILD.md Testing rule 5 forbids it. With the old lock order, a schedule where the control runs after the end returns
`has ended` and passes. So the test can miss the bug. The package reviewer's F63 proof finding is correct. The source fix
(`program_edge`) stays correct, and no cross-package effect changes. By the lead's rule, this CLEAN is not withdrawn, and
the next head gets a delta check of the new test only.

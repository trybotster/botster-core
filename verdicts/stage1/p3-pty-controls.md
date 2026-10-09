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

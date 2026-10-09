# Integration review: #191 (testkit break_control, the process controls registered as Unsupported; branch stage1/p5-controls-process)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 504540a9

Reviewed head: `504540a93bebadb665562e2188ba06c3c2bb40ba` (the change `fb9250a8`, the merge `37e456f1` of v1 `3000ae14`, and
three fixes). v1 is still `3000ae14`. The stated tier is HIGH by rule 3 (the shared testkit, and it adds the workspace
`serde` to it), which is correct.

- **The merge.** Its tree is the tree of `git merge-tree --write-tree fb9250a8 3000ae14`.
- **Edge only.** The PR changes only `botster-core-testkit` (and one `Cargo.lock` line for `serde`). `break_control` sets a
  flag in the worker's process cell. The worker's edges deliver it at the worker's next turn as `Input::LinkClosed`, drop
  the unwritten bytes and close the in-memory link, as a broken socket does. The worker machine and the host do not
  change. The control wakes the owning host through the table's `HostWake`, which `open` sets after the spawner is built.
- **The row read.** `session_row` decodes the stored row with the host's own `Row::decode` and `row_key` (already public
  in `botster-core-host`), so the testkit has no second reading of the row format. `parse` denies unknown fields, so a
  misspelled argument is `Bad`.
- **The 11 controls registered as `Unsupported`.** An unregistered control already gives `ControlError::Unsupported`
  (`harness.rs:177`), so the registration changes no outcome. It records why each one waits. Each one stays a
  non-pass (4.2b).
- **The flip.** Against v1, `core-pending.txt` loses only `conf::lc_5_stop_with_broken_control`. Its replacement-map proof
  is `core-testkit+edge` (process), not `slow:*`, so plan 23a lets it leave pending.
- **The gate log** (`controls-process-504540a9.log`) names the head and base `3000ae14`. The conformance binary reports 24
  passed and 651 ignored (v1's 23 and `lc_5`). The default tier runs 953 tests and the slow tier 243, all pass. Mutants:
  19 caught, 0 missed, 0 timeout, 6 unviable. Exit 0.

Observation (not counted): 4.2b requires the `RealCoreHarness` to build `break_control` too. Under plan 23a, `lc_5` runs
there when that harness lands, and a failure is a finding for P5.

VERDICT: CLEAN (0 open) at 504540a93bebadb665562e2188ba06c3c2bb40ba

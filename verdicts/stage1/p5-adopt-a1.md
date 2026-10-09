# P5 adoption in memory — PR #201

## Round 1 — 2026-10-09

- Head: `d96c9128a9c0f8bafd07f6faf44ba8600e0d6273`.
- Base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960` (`v1`).
- Scope: the complete 18-file change, its interfaces, its tests, its pending removals, and its supplied gate evidence.
- Risk: HIGH, rule 3. The change crosses worker-core, host, and shared testkit interfaces.
- The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### R1-1 — HIGH — The replacement link keeps the old output byte count

`Worker::adopt` resets `queued_total` and `written_total`, but it keeps `output_sent_to` and `output_unsent`.
See `crates/botster-worker-core/src/worker.rs:904-916`.
The testkit driver also resets its written count when it handles `AdoptLink`.

A running worker can send and fully write many output reports before adoption.
In this state, `output_unsent` is false, and `output_sent_to` can exceed the entire new handshake.
The adoption report does not reset that count because no output report waits.
The next plain PTY output reaches `output_advanced`, which compares the new written count against the old `output_sent_to`.
The worker sets `output_unsent` and sends nothing.
Further plain output cannot increase the new written count because those reports also remain unsent.
Without another report, the new host stops receiving output activity and model revision updates.

This defect crosses the worker's report coalescing, the driver's link replacement, and the host's observed terminal state.
The source trace establishes the failure; the reviewer did not execute a reproduction.

Required change:

1. Reset all output-report state that belongs to the old link when adoption replaces the link.
2. Add a regression with a fully drained old link whose output byte count exceeds the new handshake length.
3. Require plain output after adoption to reach the new host without another operation or report to release it.
4. Cover a pending old output report as well as a fully drained old report.

Status: OPEN.

### R1-2 — MEDIUM — Adoption loses the supported snapshot formats

`Worker::adoption_report` sets `formats` to `Vec::new()` at `worker.rs:951`.
An ordinary launch reports `model::snapshot_formats()` at `worker.rs:561`.
`HostEngine::adopt_report` assigns `report.formats` directly to the session at `crates/botster-core-host/src/adopt.rs:223`.
The public `snapshot_formats` method returns that stored list.

Thus, the same worker advertises snapshot formats before a host drop and advertises none after adoption.
The worker still has its model and snapshot implementation.
This defect crosses the adoption message and the host's public capability query.

Required change:

1. Report the worker's supported snapshot formats during adoption.
2. Prove that a host drop and adoption preserve those formats for running and exited sessions.

Status: OPEN.

### R1-3 — MEDIUM — A pending candidate can replace the link during removal

`on_candidate` refuses a new candidate while removal, termination, or exit is pending.
However, `on_candidate_bytes` does not repeat this check before it calls `adopt` at `worker.rs:883-897`.
Neither `on_remove` nor `on_terminate` closes an already accepted candidate when teardown starts.

The order `Candidate -> Remove -> valid candidate hello` can therefore replace the link during removal.
The worker can send the removal result to the replacement host instead of the host that requested removal.
There is also a stuck-exit case:

1. Accept a candidate before removal starts.
2. Complete payload removal while the old link's `RemoveResult` remains unwritten.
3. Deliver the candidate's valid hello before the candidate deadline.
4. `adopt` changes `Closing` to `Ready` while `exit_pending` remains true.
5. Draining the new link no longer closes it or emits `Exit` through `finish_close`.

The added `a_removing_worker_takes_no_candidate` test presents a new candidate after removal starts.
It does not cover a candidate that the worker accepted earlier.
This defect crosses the worker's teardown state, the driver's fence, and the host's removal flow.

Required change:

1. Close pending candidates when teardown starts, or refuse their hello before adoption can replace the link.
2. Prove refusal during removal and while the removal result waits for its write.
3. Require the original removal to finish without `AdoptLink`.
4. Apply the same rule to termination.

Status: OPEN.

### A1-F1 and A1-F2 — MEDIUM — Control ownership stays with the spawning host

The package reviewer reported these two findings on this exact head.
The integration reviewer independently confirmed both source paths.

- **A1-F1:** `Workers::break_link` and `Workers::program_edge` use the wake from the original `Processes` table.
  `connect_worker` passes only a `LinkEnd`, and `AdoptLink` does not transfer the wake target.
  After host B adopts host A's worker, `break_control` and `pty_output` can wake A while B waits.
  Without spurious wakes, B's wait can time out with worker work ready.
- **A1-F2:** `ProcessTable::holds_reports` checks the cells that this host spawned.
  Adoption does not register the adopted link with B's table.
  Once the worker flushes a report and becomes idle, `edges_quiet(B)` can return true before B consumes that report.

Both defects share one invalid ownership assumption: the spawning host remains the control host.
The spawning host must keep process-exit ownership.
The current control host must own control wakes and unread-link observation.
Transfer those control responsibilities at successful adoption, and preserve them when a candidate fails.
Check repeated adoption, link retirement, and host retirement under the same rule.
This correction stays within the existing testkit scope of #201.

Required proofs:

1. After adoption, disable spurious wakes and require both `break_control` and `pty_output` to wake the adopting host.
2. Flush an adopted worker's report without pumping the adopting host.
3. Require `edges_quiet` to remain false until that host consumes the report.
4. Preserve the original host's process-exit ownership.

Status: A1-F1 OPEN; A1-F2 OPEN.

### Other checks and evidence

The head contains the fetched `v1` base. `git diff --check` reports no error.
The endpoint key includes both the data directory and the instance.
The testkit uses the real worker machine and in-memory link edges.
The new endpoint lock paths release their locks before they take the process-cell lock.
The new host action removes the endpoint before it deletes the row, after the worker's end is verified.
The startup limit reaches the in-process worker through `WorkerSpawn.startup`.
The real driver remains in #176 under the lead's split. This review claims no real adoption proof.
This change adds no dependency, mutation exclusion, contract pin, or real-process fixture.

The supplied full gate is:
`~/botster-sessions/gates/botster-core-stage1-p5-adopt-a1-d96c9128-pool-20261009-150552-98967.log`.
It names this exact head and base. It ran on Linux node `msa1`, allocation `6efd1d85`.
All ten gate checks passed. The default tier passed 1134 tests. The slow tier passed 249 tests.
The mutation run reports 101 mutants: 89 caught, 12 unviable, zero missed, and zero timeouts.
The gate exited zero after 287 seconds.

The pending change removes 12 ids and adds none.
All 12 have passing results in the full gate and permit a testkit proof in the pinned replacement map.
The supplied scratch probe also records all 12 as passing:
`~/botster-sessions/gates/botster-core-scratch-p5-a1-probe-2-ed69e091-pool-20261009-145759-82303.log`.
That probe is not a green gate: other pending cases fail, and the probe exits 101.
The minimum list confirms a change from 30 to 33 of 70 after acceptance. This verdict does not accept the PR.
The `ad_3` id remains pending while Amendment 18 and its pin change remain separate work.

The reviewer read the exact-head package verdict at `f99d313a4a8d3e1b870db56dbc3ccf98eb8665be`.
Its file is `verdicts/p5-adoption-a1.md`. It records NOT CLEAN with the same five open findings.
The reviewer sent the three integration findings and confirmed both package findings with P5 and the P5 reviewer.
Replacement reviews and exact-head gate evidence are required before a later CLEAN.

VERDICT: NOT CLEAN (5 open)

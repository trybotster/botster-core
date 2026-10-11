# PR #243 — real route transport

## Source inspection before Round 1 — 2026-10-11

Reviewed head: `5ca808f82967f25f476988439a74b5bb9893e7f6`.
Stack base: `e493ff5f6d8622f64feb1805fe1def887f69adad`, the reviewed #240 head.
Gate base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1`).
Risk: HIGH. The PR changes worker I/O, the mutation configuration, and the list of HIGH paths.
The lead permits this stack in the handoff ruling at about 02:20 PT.
The PR must merge #240 first, retarget to `v1`, and meet the final merge conditions.
The binding plan is 23aj at `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
The code still pins contracts-v0.1.27 at `b161db87ca82873b4f6b21d8433a69f086abb7cf`.

I read the complete own delta: ten paths, 878 insertions and 49 deletions. The diff check passes.
I also read the worker machine's route actions, input handling, allowances, output queue progress, and budget emission.
The worker machine's source remains unchanged relative to #240.
The package verdict for this exact head is requested. This entry records my source inspection before the formal round.

### PRC-1 MEDIUM / package F121 — The new real PTY budget path has no real-process proof

Evidence: `Driver::read_pty_chunk` now subtracts each successful read from `pty_budget` at `main.rs:560`.
The PR body and changed exclusion reason state that no real-process test covers this decrement.
The pure `pty_read_bytes` test checks one read size. It does not check the driver's remaining budget across reads.
The readiness test checks a supplied spent budget. It does not prove that real reads spend that budget.
The new output proof writes 3,000,000 bytes. That amount does not fill the default 32 MiB route queue.
The proof therefore does not establish that the driver stops when the machine supplies a spent budget.

BUILD.md's HIGH rule requires a named real-process proof before the code merges.
OU-3d and the `Action::PtyReadBudget` interface require a total read bound until the next budget.
The existing whole-body exclusion covers the driver's new budget path. Stating the gap does not supply its proof.
The implementer then identifies that the decrement has no effect in the current loop.
I check that explanation: every nonempty `PtyOutput` refreshes the budget through `on_output`, `route_output`, and `send_budget`.
The driver settles these actions before its next read. Removing the decrement is therefore valid in this loop.
An equivalent reversal of the decrement is not required. The real assignment, read limit, stop, and resume still require proof.

Required change: add a named real-process proof that exercises repeated reads against a finite budget.
The proof must observe that reads stop at a spent budget and resume after route progress supplies a new budget.
Use public behavior or injected I/O effects. Do not add a private queue accessor or a test branch in production.
Retain a selected reversal that ignores the budget and fails the proof.
Retain the required focused Mac evidence and the final Linux gate after the source correction.
Status: OPEN. I sent this finding to both P3 agents before the formal round.
The package reviewer independently reports F121 in `msg_plugin-w_1791703485_183d8d`.
The implementer accepts the missing proof in `msg_plugin-w_1791703514_9f2784` and proposes a small legal route queue.
An immediate check that the payload has not finished after the first client frame is insufficient.
That check can pass when the budget is ignored because the payload has not finished yet.
I request an event or bounded observation that rejects that schedule, plus the selected reversal.

### PRC-2 LOW / package F122 — A zero write can resume on a readable-only event

The package reviewer reports this finding in `msg_plugin-w_1791703530_80fb38`.
I independently check the public sequence in the source.
A bound scripted stream returns `Ok(0)` for a nonempty write. The turn reports `RouteWritten { result: Ok(0) }`.
`write_once` sets `wait_writable` but leaves `writable` true for that result.
An event with readable true and writable false then returns `RouteWritable` because `event` tests the retained writable flag.
The worker input contract requires waiting until the transport is writable again after a zero write.
This finding covers the accepted generic stream result. It does not claim an observed UnixStream failure.
Required change: clear the consumed write readiness for a nonempty zero write.
Add a public behavior assertion that a readable-only event does not resume that write.
Preserve the empty-write behavior and interrupted retry. Status: OPEN. HIGH requires LOW closure too.

### Interfaces checked

`DescriptorTable::take` transfers an OwnedFd to the route transport. The driver sets the stream nonblocking and registers both directions.
A registration failure removes the stream and queues `RouteEnded`. `RouteClose` deregisters the stream and closes it through Drop.
The route transport retains read and write readiness until WouldBlock. An interrupted call retries in the next turn.
Each route turn performs at most one pending write and one bounded read.
The writer reports the accepted byte count. The worker retains the unwritten suffix and starts the next write after progress.
A blocked write reports zero and waits for `RouteWritable`. The retained real output proof checks that wake path.
A route read uses the smaller of the shared driver chunk and the allowance. The read spends that allowance.
The machine also spends its allowance mirror before it computes a replacement allowance.
EOF or a terminal read error reports `RouteEnded` once. Other routes remain in the transport.
Route tokens use the top bit. The candidate decoder rejects these tokens.
The loop reads the control link before the candidates and routes. It queues their inputs in that order before settlement.
The PTY read-size decision applies the chunk, drain, and budget bounds. A zero budget suppresses PTY reads and the busy-loop decision.
A completed drain still reports `PtyDrained` without reading. PRC-1 concerns proof of the real budget path.

The added dependency is the existing route codec, for tests only. No new production dependency or unsafe block appears.
The status lists, transcript assertions, contracts pin, and other conformance accounting remain unchanged.
The PR records the pending route_fill obligation and does not claim an OU-3 conformance PASS.

### Prior art and retained proofs

I read the Prior art note and the pinned old worker's `flush` at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old implementation loops over its own queue and treats a zero write as WriteZero.
The PR rejects that mechanism because the v1 machine owns the queue and its interface waits after a zero write.
The PR reuses mio, standard Unix streams, the shared read bound, and the existing route codec.
It explains the custom per-turn and allowance decisions through plan 2.4 and DP-5.

I read the complete Mac, driver-revert, writable-revert, and seventeen hand-mutant logs named in the body.
The exact-head Mac log names Darwin 27.0.0 and the reviewed head.
It reports 76 worker unit tests, 17 slow_cli tests, 38 slow_session tests, and 115 core-sys tests passing.
The driver revert keeps the new proofs and restores #240's no-op route actions. Both attach/input/output and client-close proofs fail.
The writable revert suppresses `RouteWritable`. The large-output proof fails in each of three retained runs.
Those selected reversals have platform headers and source descriptions. They are not exact-head final gates.
All seventeen listed hand mutants fail. Their log does not provide an exact-head header.
I also checked the three new real proofs' Linux PASS records in both the prebuilt-worker and driver-observer fixtures.
These proofs establish route attach, input, output, client close, and recovery after a blocked route write.
They do not close PRC-1.

### Gate and conformance limits

Log: `~/botster-sessions/gates/botster-core-stage1-p4a-real-route-transport-5ca808f8-pool-20261011-000649-89485.log`.
The header names the reviewed head, gate base above, Linux, and the plain `cargo xtask ci` command.
I read the reports, summaries, and all 153 default-stage mutation outcomes plus the one slow-stage outcome.
The 153 identities are unique. The sole slow-stage identity equals the sole default-stage miss: deleting `Driver::bind_route`.
Stage 1 reports 135 caught, one missed, and seventeen unviable in 235.0 seconds.
The unmodified slow baseline passes in 114.2 seconds. Stage 2 catches its one mutant in 105.3 seconds.
The combined result is 136 caught, seventeen unviable, zero missed, and zero timeout.
This gate's diff against `v1` also includes #240. These mutation counts cover that combined diff.

All ten CI steps pass in 653.2 seconds. The mutation step takes 456.6 seconds.
Default tests: 1638 passed and 484 skipped in 11.061 seconds.
Slow tests: 420 passed and 2010 skipped in 117.155 seconds.
Testkit conformance: 206 passed; minimum 55 / 69.
Strict pending: 373 / 394 run, 21 real-only not run, and two held.
Real minimum: 36 / 68 passed and 37 / 69 accepted. Pending-real: eighty IDs, with zero passes.
Fuzz passes because the diff changes no crate with a decoder harness.
The job and wrapper exit zero after 870 seconds: 209 queued and 661 running.
The green gate does not establish the real budget behavior that its exclusion omits.

The local Mac comparison uses scratch head `e967a5f5`: PR C at `8256b967`, #240, and #238 at `8474e449`.
LC-12 and OU-1 pass there. Without PR C, both stop at the first route frame's deadline.
Three other minimum IDs fail and thirteen lack a control. The body records these results and removes no ID.
These scratch results do not establish acceptance on `v1` or on the reviewed head.

I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.
Next: read the exact-head package verdict and review the correction evidence after a doorbell.

VERDICT: NOT CLEAN (2 open)

## Round 1 — 2026-10-11: exact-head package verdict

Reviewed head: `5ca808f82967f25f476988439a74b5bb9893e7f6`.
Stack base: `e493ff5f6d8622f64feb1805fe1def887f69adad` (#240).
Gate base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1`).
Risk: HIGH. This round covers the own delta and source contexts recorded in the preceding inspection.
Source inspection commit: `19618643978e0379f642051f077f85417aa36cb0`.

I read the complete exact-head package Round 163 / #243 Round 1 artifact at `63504fd193452e49c3e59c7274ca7364b5df316b`.
Its head, stack base, ten-path scope, gate, and two findings match this review.
PRC-1 MEDIUM / F121 remains OPEN: the real PTY budget path needs a named real-process proof.
PRC-2 LOW / F122 remains OPEN: a nonempty zero write can resume on a readable-only event.
The source inspection preserves each finding's evidence and required correction. Both findings must close before CLEAN.
The package reviewer reports no observed budget-path failure and no observed UnixStream failure for F122.
I preserve those limits. The passing gate does not close either finding.

I read the lead's R-59 tally ruling in the current handoff, at about 03:42 PT.
The gate's real minimum counts above are its historical reports, before this ruling.
AD-7 remains in its current list but does not count toward the real minimum until its fenced form passes with `held:true`.
The lead reports the current `v1` real tally as 35 / 68 counted plus one that passes unfenced under R-59.
This PR adds no AD-7 fence and establishes no fenced AD-7 acceptance.
The later contracts and Core work remain separate from this review.

The lead also assigns the stale READ_CHUNK exclusion and the exclusion-match check to GS outside this PR.
That assignment does not waive the new real budget proof or the zero-write correction.
The stack remains reviewable under the lead's ruling. It cannot merge before #240 and the final merge conditions are met.
No conformance ID moves. No additional integration finding appears in the package artifact.
The next review must name the corrected head and retain both finding closures.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (2 open)

# PR #232 — PTY readiness after a FIFO marker

## Round 1 — 2026-10-10

Reviewed head: `af4ce07dc006d650c7a705a3b9162480c486c238`.
PR and full gate base: `504779bc7f56b684274763846d3fa052cc88b0e2`.
Tier: HIGH. The change affects real-process test helpers in two crates.
Scope: the complete two-file delta, helper callers, guard ownership, related FIFO sites, PR body, and supplied evidence.
The lead orders this PR before the next P5 gate.
The Git merge base equals the recorded gate base.

Both helpers retain their FIFO marker wait. They then poll the PTY master for IN.
Each helper requires a positive poll result and the IN flag. HUP alone cannot satisfy the assertion.
The added wait reads no PTY bytes. It leaves the existing output assertions to test the production path.
The helper uses its existing 10-second Timespec for each poll.
Each poll has a separate bound. The two waits do not share one absolute deadline and can total 20 seconds.
The change adds no timeout value, sleep, busy loop, or unbounded wait.

The scripts print the fixed five-byte ready output before they write the FIFO marker.
The FIFO and PTY carry separate data. Receipt of the FIFO marker does not prove PTY readiness.
I read Linux v6.6's [PTY write source](https://github.com/torvalds/linux/blob/v6.6/drivers/tty/pty.c),
[flip-buffer source](https://github.com/torvalds/linux/blob/v6.6/drivers/tty/tty_buffer.c),
and [N_TTY receive source](https://github.com/torvalds/linux/blob/v6.6/drivers/tty/n_tty.c).
The PTY write commits the flip buffer and queues work.
The receive path publishes the received batch before its IN wake.
I infer that the descriptor wait removes the observed zero-byte race for this fixed fixture.
Readiness does not define message boundaries for arbitrary output streams.
The existing read_all helper already uses the same descriptor poll. This change adds no wait component.

The payload helper owns its PayloadGuard before the added assertion can fail.
Its drop requests guard cleanup before production drops and reaps the payload.
The worker helper owns its guard before spawning the payload.
Its field order requests cleanup, drops the bounded driver, and then reads the guard report.
The borrowed descriptor changes no process owner. The change adds no reaper or production branch.

The four affected proofs retain their contract assertions:

- The payload output proof requires pending output, a query that retains bytes, six accepted input bytes, exact readyinput output, and exit code 0.
- The panic proof requires payload cleanup within twice the existing cleanup limit.
- The worker bound proof requires exact ready output at each positive read bound.
- The worker drain proof requires the existing readiness transitions and bounded drain results.

The added wait consumes none of the bytes used by these proofs.
The change adds no private-state assertion and removes no contract assertion.

I checked all mkfifo sites at this head and their scripts.
The changed two helpers contain the marker followed by immediate assertions about PTY output.
The other driver script writes PTY output after its first marker. Its later marker does not establish immediate PTY content for an assertion.
The session scripts mark signal setup or input completion.
The Core scripts mark launch or group facts. The shared process fixture uses a FIFO to block its child.
The guard scripts use FIFOs for child blocking, ownership, or pipe closure.
I found no additional instance of the changed assumption in this search.

### Base merges and supplied failure

The two changed test files equal their versions at `7867d7330638dd6e43b8bf1df8ed0a2c555e07b6`.
The automatic source merge of 7867d733 with v1 89d57cc7 matches ca8f06fb.
Both trees are `41b5cb68b695355a6a97614b89b60115dfaf952a`.
The automatic source merge of ca8f06fb with current v1 504779bc matches af4ce07d.
Both trees are `1d208cfaf6c9301761898da6528ea335fccdf70b`.
I used git merge-tree source comparisons. I ran no executable merge-check job.
The final delta changes only the two test files. It changes no contract pin, pending ID, conformance count, or mutation exclusion.
The remote PR and branch heads and v1 base match the recorded commits. git diff --check passes.

I read `~/botster-sessions/shared/core-stage1/gate-logs/ctl-a-full-7c93a2e7.log`.
The original payload output proof passes in the slow step, then fails in the unmutated stage-2 baseline.
The failure records `queued-output marker arrived; pending_output=0` and `assertion failed: pending > 1`.
The baseline records 394 passing tests and one failure. That failure stops stage 2.
This is supplied failure evidence. I did not reproduce it.

### Exact-head gate and package verdict

Log: `~/botster-sessions/gates/botster-core-stage1-p6-pty-order-af4ce07d-pool-20261010-160803-48880.log`.
Its header names the exact head and base. The command is plain cargo xtask ci through the Linux pool.
All ten jobs pass. Prebuild compiles the probe script for contracts-v0.1.27 and writes the three-binary candidate manifest.
Default: 1499 passing tests, 11.291 seconds. Slow: 384 passing tests, 91.201 seconds.
All four affected proofs pass.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Real-pending remains 87, with zero newly passing IDs.
The mutation step lists no mutant in this test-only delta. It runs no mutation stage.
The job exits zero after 190 seconds, including one second queued. The wrapper exits zero after 190 seconds.

I read package CLEAN at `20128d400d298584d86517aa9b025b93d9c54204`, `verdicts/p6-pty-order.md`, Round 1.
The package verdict covers this exact head and base. Both reviews agree on the delta, ownership, and supplied evidence.
I found no integration defect in this scope.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open).

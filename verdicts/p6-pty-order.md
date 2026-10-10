# P6 review: PTY readiness after a FIFO marker

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/232.
Exact head: `af4ce07dc006d650c7a705a3b9162480c486c238`.
Review and full gate base: `504779bc7f56b684274763846d3fa052cc88b0e2`.
Risk tier: HIGH. The PR changes real-process test helpers in two crates.

The reviewer read the complete two-file diff, affected helpers, callers, ownership code, and current PR body.
The Git merge base equals the recorded gate base.
No package finding remains.

After the FIFO marker, both helpers now poll the PTY master for `POLLIN`.
They require a positive result and the requested readiness flag.
They do not consume the PTY bytes or call the driver before the existing assertions.
They reuse the helper's existing 10-second `Timespec` for the added wait.
Each poll is bounded separately. This is not one absolute deadline shared by both polls.
The change adds no timeout value, sleep, busy loop, or unbounded wait.

The payload helper still owns its `PayloadGuard` before the assertion can fail.
Its drop requests guard cleanup before production drops and reaps the payload.
The worker helper still owns its guard before spawning the payload.
Its field order requests cleanup, drops the bounded driver, and then reads the guard report.
The new borrowed descriptor changes no process owner and adds no reaper.

The contract assertions remain unchanged.
The payload test still requires queued output, a query that retains those bytes, six accepted input bytes, and exact `readyinput` output.
It still requires exit code 0.
The panic test still requires cleanup to finish within twice the existing cleanup limit.
The worker tests still require exact `ready` output at each read bound and the existing readiness and drain results.
The helpers establish descriptor readiness. They do not replace these assertions with a test of private state.

The reviewer checked all `mkfifo` sites at the exact head and their fixture scripts.
The same PTY-output-before-marker assumption appears in the two changed helpers.
The other driver test writes no PTY bytes before its first marker.
Its later marker follows the large write, but the test does not use that marker to assert immediately queued PTY content.
The session scripts mark signal setup or input completion. They do not assert PTY output from those markers.
The Core scripts mark launch or group facts. The shared fixture uses a FIFO to block its child.
The guard tests use FIFOs for child blocking, ownership, or pipe closure.
The reviewer found no additional instance of the changed assumption in this search.

The Prior art note reuses the descriptor poll already used by `read_all` in the payload tests.
It adds no new process or wait component.
The reviewer also read Linux's [PTY write source](https://raw.githubusercontent.com/torvalds/linux/master/drivers/tty/pty.c)
and [N_TTY receive and poll source](https://raw.githubusercontent.com/torvalds/linux/master/drivers/tty/n_tty.c).
The write enters the flip buffer. The receive path publishes received data before its readiness wake.
The reviewer infers that waiting on this descriptor removes the observed zero-byte race for the fixed fixture.
This review does not claim that readiness frames arbitrary output streams.

The reviewer read P5's retained failure:
`~/botster-sessions/shared/core-stage1/gate-logs/ctl-a-full-7c93a2e7.log`.
The original payload test passes in the slow step, then fails in the unmutated stage-2 baseline.
The failed baseline records `queued-output marker arrived; pending_output=0` and `assertion failed: pending > 1`.
The baseline failure stops stage 2.

The two changed test files equal their versions at `7867d7330638dd6e43b8bf1df8ed0a2c555e07b6`.
The later heads merge v1 `89d57cc7d07957b88003722897cf43e540d19bcc`
and v1 `504779bc7f56b684274763846d3fa052cc88b0e2`.
The final diff against the latter base contains only these two test files.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-pty-order-af4ce07d-pool-20261010-160803-48880.log`.
Its header names the full head and base. All ten jobs pass. The gate exits 0 after 190 seconds.
Prebuild-worker compiles the probe script for the current contracts tag.
Default: 1499 passing tests. Slow: 384 passing tests.
The payload output test, panic cleanup test, worker read-bound test, and worker drain test all pass.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Pending-real remains 87, with zero newly passing ids.
The mutation step lists no mutant for this test-only diff. It runs no mutation stage.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN

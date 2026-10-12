# PB B1: R-106 exit watch

## Current verdict: round 2

Date: 2026-10-12. Tier: HIGH.
Reviewed head: `0e25e27da6606a27cf2ff94df5d139c80f27b4d5`.
Base: `3e655eff46d36f8c337339a1a17d401c8c662594`.

VERDICT: CLEAN

PB1-1 is closed. The design now names the pinned old code, reused v1 paths, rejected approaches, custom type, and dependency status.
The reviewer checked the old source references at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The design states that it copies no old code or tests and adds no dependency.
Design SHA256: `6990ade90135444d2e3bf38a359f61dd369072f20fbe00a35b6184fd5afb13d7`.

The delta from the previous head changes one test expression in `tests/common/driver_edges.rs`.
The test now requires the watch result to be successful after its own group kill.
This preserves the test's deadline and expected behavior. It changes no production path.
The reviewer checked the first gate's clippy error and confirmed that this delta repairs that error.

The first gate on `d8358ee920892da144c1f6289a735451bafda28f` failed because the test discarded a required Result.
Log: `~/botster-sessions/gates/botster-core-stage1-pb-b1-exit-watch-d8358ee9-pool-20261012-130202-26976.log`.
That red gate is not acceptance evidence.
The implementer supplied READY #327 at the reviewed head. The PR body contains the required prior-art note and states HIGH.
The reviewer checked the corrected Linux pool gate on `msa1`. All ten steps report PASS, and the gate exits 0.
The gate names base `3e655eff46d36f8c337339a1a17d401c8c662594` and the reviewed head.
The gate runs all nine new sys, machine, testkit, and zero-start-time tests successfully.
Mutation results: 21 total, 18 caught, zero missed, zero timeouts, and three unviable.
The slow mutation stage catches the one mutant missed by the default stage.
The fuzz step has no changed crate with a decoder harness. It is not SKIPPED.
Log: `~/botster-sessions/gates/botster-core-stage1-pb-b1-exit-watch-0e25e27d-pool-20261012-130618-58413.log`.
Log SHA256: `0de859daa3399bde2db85985186568a5df9694cb91947c0460f2667fb715d15f`.
The CLEAN verdict remains unchanged. The required integration review and the lead's merge train remain pending.
The reviewer built nothing locally and ran no tests.

## Historical verdict: round 1

Date: 2026-10-12. Round: 1. Tier: HIGH.
Reviewed head: `d8358ee920892da144c1f6289a735451bafda28f`.
Base: `3e655eff46d36f8c337339a1a17d401c8c662594`.

VERDICT: NOT CLEAN (1 open)

The source logic has no open finding. PB1-1 concerns the required prior-art note.
The first gate is running. The reviewer has not received READY or the gate log.
The reviewer built nothing locally and ran no tests.

## PB1-1 — LOW — Add the required prior-art note

Evidence: `shared/core-stage1/handoffs/pb-b1-design.md` has Design and Evidence sections, but no prior-art note.
The two commit messages also have no prior-art note.
BUILD.md rule 0 requires the note in the design or PR description and requires the reviewer to reject a feature without it.
All findings must close on a HIGH PR, including LOW findings.

Required change: add the note to the design or PR description.
Name the existing watch, signal, and worker-end paths that this change reuses.
Name the rejected approaches and their reasons, including the invented exit code and the old thread panic.
State why the new machine input and action are needed and whether the change adds a dependency.
No source change is required for this finding.

## Source review

- `wait_unreaped_with` retries EINTR and returns `ExitWatchFailed` for every other errno, including ECHILD.
- Only a successful waitid status constructs a payload `ExitStatus`.
- The watch thread sends the result and wake. It signals no process or group.
- The machine's `signal` acts only while the payload is Live, which means that the leader remains unreaped.
- `SignalPayload` uses the driver's owned Payload. `Payload::signal_group` uses that payload's recorded pid.
- The failure handler sends SIGKILL unless the machine already queued the group kill.
- The handler queues no end message or reap. It ends through `ExitFailure`.
- The Spawning path applies after Launched and returns before `try_start`.
- `perform(ExitFailure)` returns an error. `settle` propagates that error and runs no later action.
- The existing command-line path returns failure and writes the error to stderr.
- The steward's R-106 follow-up accepts stderr as the worker's own log, including when stderr goes to `/dev/null`.
- The testkit ends the worker with a non-zero status and drops its payload through the existing cleanup path.
- The testkit test keeps the clock fixed and makes the payload ignore SIGTERM. It checks Lost(WorkerGone), no Exited, Stop completion, and no live payload.
- The machine tests check the exact kill and exit actions, graceful-stop and killed cases, the Spawning case, and duplicate failure suppression.
- The sys tests inject EIO and ECHILD after EINTR. The zero-start-time test checks AD-6.

The lead approved the file split with P3 and the B1 design.
The host's worker-reaper `Code(-1)` sites remain outside B1 by the lead's ruling. PB-W1 tracks them for B3.
No transcript, timeout, budget, dependency pin, or mutation exclusion changes in this diff.

## Evidence read

Design: `~/botster-sessions/shared/core-stage1/handoffs/pb-b1-design.md`.
Design SHA256: `36d9fe0436b8658fdd738e1c6f4ba3f5fda0297c67a6b3944ede8d021671fe74`.

Reversal: `~/botster-sessions/gates/botster-core-pb-b1-reversal-d8358ee9-20261012-200131.log`.
Log SHA256: `6a3faf1095d678f21a8a1cd133d7b6132eed7caa46fac3bf4f40a68f5560fed0`.
The scratch reversal makes the handler return immediately. Five tests fail, including the testkit test.
The two tests for suppression after an exit or a duplicate failure still pass, as expected.
This is focused reversal evidence, not a full gate.

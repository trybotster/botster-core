# PB B1: R-106 exit watch

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

# PB-W1: worker process exit status

## Current verdict: round 2

Date: 2026-10-12. PR: #330. Tier: HIGH.
Head: `572924b29bf4fca3d6a8c76d45fc486c0c070b70`, unchanged.
Reviewed source and Linux gate base: `007efbde6a59af403d0937c38361c137a8961a48`.

VERDICT: CLEAN

PBW1-1 is closed. The reviewer read the focused Mac pool log and checked its citation in the complete PR body.
The log records the exact reviewed head, macOS 27.0.1, and arm64.
`slow_process`: 24 tests pass, zero skipped. Slow library tests: 25 pass, 43 outside the selected filter.
Both affected reaper tests pass: `a_reaper_returns_right_after_its_notify` and `a_reaper_without_a_child_returns_at_once`.
The job and wrapper exit 0. The commands use the slow feature and profile, with cargo and nextest parallelism capped at four.

Mac log: `~/botster-sessions/gates/botster-core-stage1-pb-w1-exit-status-572924b2-pool-20261012-163652-79654.log`.
Mac log SHA256: `93878c3f9836e48e69a7bd1301d7f13675b3e40d5fe3d8a12e1a08a14dbd7fe6`.
This is focused platform evidence, not a replacement for the Linux gate recorded below.
The Mac wrapper records the newer origin/v1 tip, but the executed source remains the unchanged reviewed head.

No source delta needs review in this round. No package finding remains open.
The required integration review, current-base import, #327 adaptation, and lead merge checks remain due.
The reviewer built nothing locally and ran no execution job.

## Historical verdict: round 1

Date: 2026-10-12. Round: 1. PR: #330. Tier: HIGH.
Head: `572924b29bf4fca3d6a8c76d45fc486c0c070b70`.
Base: `007efbde6a59af403d0937c38361c137a8961a48`.

VERDICT: NOT CLEAN (1 open)

The source logic has no open finding. PBW1-1 concerns required platform evidence.
The reviewer built nothing locally and ran no execution job.

## PBW1-1 — MEDIUM — Supply focused Mac pool evidence

Plan section 8, revision 23ai, requires a focused Mac run for portable code whose behavior the OS decides.
The rule explicitly covers process identity, signals, and reaping, even without an OS-specific cfg.
The run's log must be in the PR before the first READY.
The package brief and pair-common state that local Mac runs are not evidence.

This PR changes the reaper's stored result and the real-process tests for exit codes, signals, identity, and notification.
The PR body lists local Mac runs as not evidence. Its only pool evidence is the Linux gate.

Required change: supply a focused Mac pool run on this head for `slow_process` and the affected reaper `slow_tests`.
Put the log path and exact head in the PR body. Preserve the existing assertions, timeouts, and budgets.
No additional full gate or source change is requested by this finding.

## Source review

- The lead approved decision (A): remove the unused worker status from the process edge.
- `Process::poll_exit`, `HostEdges::poll_process_exit`, and `Input::ProcessExited` now carry only the process identity.
- The host handler previously ignored the status. Its classification and link-drain behavior remain unchanged.
- The driver keeps the same identity when its event budget is exhausted and feeds that identity in a later pump.
- sys `Children` stores the operating system's status when the wait succeeds and None when it fails.
- The conversion function and both invented `Code(-1)` values are gone.
- The reaper still queues the result, notifies waiting callers, and calls the host notifier in the same order.
- The real edge discards the optional status and returns the identity.
- The testkit, edge tap, and report holds preserve identity, order, hold, and release behavior.
- The slow sys tests still require exact exit codes and signals through the raw wait status.
- The deleted conversion test covered the deleted conversion. The slow process tests retain the behavior evidence.
- The two removed mutation exclusions name deleted code. The diff adds no mutation exclusion.
- The API snapshot records the changed RealEdges method. The contract facade's operations and outcomes do not change.
- The PR body includes prior art, the HIGH tier, gate evidence, and the known overlap with #327.

#327 adds `Action::ExitFailure` in the testkit. On integration, that arm must call the new one-argument `Processes::end`.
The package needs a delta review on the combined head because the process edge signature changes.
The payload's exit status and R-106 failure path remain owned by #327.

## Linux gate evidence

The reviewer checked the exact head and base in the log. All ten steps report PASS, and the gate exits 0 on `msa1`.
Mutation results: 20 total, 19 caught, zero missed, zero timeouts, and one unviable.
The slow mutation stage catches the reaper mutant missed by the default stage.
The fuzz step has no changed crate with a decoder harness. It is not SKIPPED.

Log: `~/botster-sessions/gates/botster-core-stage1-pb-w1-exit-status-572924b2-pool-20261012-135330-10095.log`.
Log SHA256: `38a8d37f78bbe087a653db9ffb2fecadb7ad01a5204cdef99f094f2816b5094c`.

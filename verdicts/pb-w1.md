# PB-W1: worker process exit status

## Train 12 package delta

Date: 2026-10-12. PR: #330. Tier: HIGH.
Train head: `7b37e787209e839feb2220b7216d1e417b3838b8`.
Tree: `6adc74c7fcc7573548997fcc83a9280a3260dc3c`.
Train base: `131024f3936898a3106af299e6751553c55bbc82`.
Member head: `99209ea598aec0c182f35a1645b2a938d2f7d8cc`.
Composition: base, then #330, then #331 `57cb242f5d3dda40e524090239222e4824021798`.

VERDICT: CLEAN

This is a package source delta verdict. The reviewer built nothing locally and ran no execution job.
The reviewer read the merge result of the ten overlapping host and testkit paths named by the lead.
The added and removed lines of #330 are identical in all 23 source and API files when compared against the train base.
The remaining member path, `.cargo/mutants.toml`, retains both PB-W1 exclusion removals and adds #331's separate
high-tier shell path move to gate-kit. No PB-W1 status fallback or exclusion returns.

The host still receives only a process identity. An exit held for pump budget keeps that identity and sets `more`.
The handler still drains a running or stopping worker's control link before it classifies the session.
The Remove teardown still retains its link for the result and bounds its completion with the existing grace.
The imported due-step and route-stall changes do not add a process status consumer or change those exit branches.
The drain fixture's new route mark retains the same queue-pressure and exit-order test.

The simulated process queue still ends and wakes once by identity. Both Exit and ExitFailure still call
`ended(true)`, which drops the payload and closes the worker's resources.
The imported facts-cost measurement reads LinkSend frames; it does not change the exit queue or cleanup actions.
The sys reaper still keeps only an operating-system status or None internally, with no invented status.
The real process edge still discards that status at the identity-only boundary.
No package finding remains open in this composition.

The lead reports a clean train compile. That report is not a reviewer execution result.
The lead ordered no member gate. The integration delta and exact-head train gate remain due.

## Current-base source delta: round 3

Date: 2026-10-12. PR: #330. Tier: HIGH.
Head: `99209ea598aec0c182f35a1645b2a938d2f7d8cc`.
Tree: `1ab9f5a0d3a6a0dfb040ea06b385078bea778495`.
Imported base: `644f632f194f62ad7fe14e66482ee01083706e30`.
Previously reviewed member: `572924b29bf4fca3d6a8c76d45fc486c0c070b70`.

VERDICT: CLEAN

This is a source delta verdict. The reviewer built nothing locally and ran no execution job.
The imported base is an ancestor of this head.
The merge is `befb21878c974c974b579e2e11b54d83b179eb34`, followed by the ExitFailure adaptation.

The added and removed source lines of #330 remain identical in 23 of its 24 changed files.
The remaining file, testkit `worker.rs`, adds the required adaptation to #327's ExitFailure arm.
The arm calls `Processes::end(self.id)` and then `self.ended(true)`.
`Processes::end` still queues the worker's identity once and wakes the host.
`ended(true)` still closes the link and endpoint and drops the payload through the existing cleanup path.
The adaptation removes only the worker status that the host never reads.
The real driver still returns an error for ExitFailure, so R-106's non-zero worker exit remains intact.

The imported #327 failure injection and tests remain present.
The host's identity-only exit classification and control-link drain remain unchanged.
The current-base sys code retains #327's zero-start-time test alongside #330's unchanged reaper edits.
The imported drain, route, and conformance changes do not add a worker status consumer.
No new mutation exclusion or invented exit status enters this delta.

The reviewer read the complete PR body at this head. It retains the HIGH tier, prior art, and old-head evidence citations.
The known #327 argument conflict is now resolved by this source delta.
The previous Linux and Mac logs remain evidence for the previous head; they are not a gate for this new head.
The lead ordered no separate member gate. The integration delta, delivery gate, and final merge checks remain due.
No package finding remains open.

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

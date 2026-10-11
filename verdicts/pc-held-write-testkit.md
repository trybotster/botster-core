# PC package review: R-74 held writes, testkit part, PR #283

VERDICT: NOT CLEAN (2 open).

- READY receipt: `msg_plugin-w_1791743029_b2452a`.
- Head: `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
- Parent, own-diff base, PR base, and gate header base: `9f7085f703457b0669785697dbeb14ac687a8942`.
- Tier: HIGH, under BUILD.md rule 3. The change touches the shared testkit crate.
- Contracts pin: `contracts-v0.1.36`, unchanged in all seven crates.
- Binding plan: 23bx, `~/botster-sessions/pins/stage1-plan.5224c5dc.md`.
- Plan SHA-256: `64977fbc8080379f6b684eb1c2dd0d5aa307b574f26a1193a911451c62c5038c`.

## Scope and contract basis

The reviewer reads the PC brief first and follows its reading order.
The reviewer reads the resume brief, pair-common, BUILD.md, applicable steward rulings, R-74, and the group-9 design note.
The previous #220 and P3/P6 harness reads remain applicable. This review changes no real-tier control.
The reviewer reads the complete three-file own diff, the current PR body, and the program and control paths.
The reviewer reads the pinned A5 candidate 4 text and the worker's PTY write and readiness paths.
The reviewer reads the plan changes from 23bu through 23bx and the lead's split approval at 04:56.

R-74 allows `kernel_bytes` only on Core's testkit program edge. FakeCore stays frozen.
The new testkit queue is accepted input that the program has not read. `pty_queued` reports its size without changing it.
Release delivers the queue in order. The real tier ignores the requested kernel capacity.
The lead approves PART A now, with PART B in PC's `.40` pin PR after #282 and the contracts tag.
PART B must add the real control and remove the six IDs only with its required evidence.
No pending ID, pin, transcript, timeout value, worker code, production hook, unsafe code, or mutation profile changes here.
This review grants no real-tier completion or group-completion credit.

## Findings

### HW-TK-1 — MEDIUM — the chunk cap can hide future queue work

Evidence: `crates/botster-core-testkit/src/program.rs:495-503`, especially the `!controls.blocked` guard at line 497.
R-74 permits writes while the program is blocked if `kernel_room` remains.
`is_writable` uses that rule, but the next-step marker still assumes every blocked program has no room.

Static case:

1. Set `pty_chunk` to 1 and `kernel_bytes` to 3 on a blocked program.
2. Write `abcd`. The edge accepts 1 byte into the queue.
3. The remaining write gets `WouldBlock` because `input_left` is 0. The kernel queue still has room for 2 bytes.
4. The guard leaves `step_refused` false because `blocked` is true.
5. `is_writable` is false until a new step restores the chunk cap.

`Workers::has_ready` at `worker.rs:481-487` includes `ProgramControl::waits_for_next_step`.
`TestkitCore::pump` at `worker.rs:1671-1677` uses that result to retain `more` and signal the wake.
The current marker can therefore report no future work while a later pump can accept more bytes into the queue.
This breaks the existing `pty_chunk` progress rule when the new form acts with it.
The worker's unchanged `on_pty_written` retries a partial write, then waits for readiness after the zero-progress result.
The defect is at the injected program edge, not in that worker logic.

Required change: mark next-step work when the chunk cap alone prevents progress and the blocked kernel queue still has room.
Retain no-next-step behavior when the kernel queue is full or the `pty_accept` limit is spent.
Add a behavior test for the combined controls. Show that later pumps fill the queue without a release or unrelated wake.
Keep the accepted queue unread until release. No test or reproduction runs in this reviewer worktree.

### HW-TK-2 — LOW — the held-write check discards the events it must inspect

Evidence: `crates/botster-core-testkit/src/pty_controls/tests.rs:534-541`.
The test calls `settle` before it checks that no `Completed` for the write arrived.
`settle` polls and discards events after every pump at lines 97-104.
The later assertion cannot inspect any matching completion that those polls already consumed.
The final post-release count and byte checks do not make this earlier absence assertion observe the held interval.
BUILD.md Testing rule 3 requires a test to prove its cited behavior. R-74's held-write fence requires the op to remain pending.

Required change: retain or check events from every pump in the held interval, through the final idle pump.
Assert that none completes this write before release. Keep the post-release completion and ordered-byte checks.
The change may use a focused helper or a local pump loop. It must not discard the events under the assertion.

Both findings were sent to PC in `msg_plugin-w_1791743210_dce502` before publication.

## Other source checks

The queue limit composes with `pty_accept`, `pty_fail_after`, and the per-step cap by taking the minimum available room.
The write decrements each applicable limit by its accepted count. It counts queued bytes as written without adding them to the input log.
`set_blocked(false)` drains the queue once into the input log and `delivered`.
`advance` moves `delivered` to the script's seen bytes before a later direct write, preserving byte order.
The script cannot match queued bytes before release. A plain block resets the additional room to zero.
These paths have no separate source finding outside HW-TK-1.

The control parser retains `deny_unknown_fields` and the session/handle checks through `program_of`.
It rejects `kernel_bytes` with `on: false` before it changes the edge. Negative and string capacities are parse errors.
`pty_queued` takes only the session and performs no pump, poll, wake, or state change.
The release keeps the existing host wake. The change adds no terminal parsing, mode tracking, or input encoding.
All terminal behavior remains in the real worker and libghostty.

The five added tests cover queue acceptance, unread bytes, release order, script matching, plain blocks, and bad control arguments.
The held-write test needs HW-TK-2's correction. The suite has no case for HW-TK-1's combined controls.
Exact queue sizes in these testkit tests come from the explicitly injected capacity. They are not real kernel-count assertions.

The Prior art note exists. The reviewer reads the pinned old `read_available` function at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its FIONREAD use takes a runtime pipe snapshot. It does not implement this program control, and the PR reuses none of it.
The new queue uses the standard Vec and existing injected edge to model only R-74's stated boundary.

## Supplied gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-646ebe73-pool-20261011-111204-3662.log`.
SHA-256: `6ad44b5a5fa5efd7129a5ac9de1535bb96efe1bd8030eb7f7fbc313cf37b658f`.
The header names the exact head and base above. All ten steps PASS in 554.7 seconds.
The Linux job and wrapper exit 0 after 668 seconds. The job queues for 1 second and runs for 667 seconds.
Default: 1745 PASS, 478 skipped, 18.687 seconds. Slow: 558 PASS, 2032 skipped, 57.292 seconds.
All five new tests pass. The default budget passes. The gate changes no pending-list state.
Minimum counts: testkit 60/69; real-passing 49/68; real-accepted 50/69.

Stage 1 catches all 28 mutants in 151.2 seconds. Stage 2 does not run because stage 1 misses none.
The reviewer matches all 28 raw identities with all 28 named catcher records.
There are zero missed, timeout, unviable, or FLAKY outcomes. Fuzz is PASS with no changed decoder crate.
A green gate does not close either finding. The tests do not cover the missing combined-control schedule.

The log reports five passing IDs that remain real-pending. This PR removes none of them.
Three of these IDs also appeared in #277's gate. The lead assigns pending-list removals to P5's `.39` pin PR.
No removal of those IDs is approved by this verdict.

The body reports six candidate testkit transcript passes before the split.
The retained `evidence/pc-held-write/summary-after.txt` lists those six testkit passes on a scratch candidate based on `b6fd4b98`.
That evidence is not an exact-head `.40` gate or an A5-4 table for this PR.
PART B still requires the final contracts tag, both-tier comparison, Linux gate, and repeated real-tier evidence.

The reviewer runs no build, test, gate, mutation, reproduction, or executable base-check job.

## Next review conditions

PC must close both findings and send a new exact head with the applicable gate evidence.
HIGH requires Sol's same-head integration CLEAN after the package findings close.
After both CLEANs, PC must request the lead's final-gate slot under plan 23bu.
With the slot, PC imports the then-current v1 once, posts the base check, and runs one exact-head final gate.
The gate base must equal the v1 tip at merge. This verdict approves no later import or source delta.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.

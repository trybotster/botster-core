# P6 testkit review — M0b

VERDICT: NOT CLEAN (1 open)

Reviewed head: `4f8ec55210e09afbd458ae956bf7ce855b55181f`.
Previous reviewed head: `b97b6055856e60c17af7cf0300ab89bc582a6d5e`.
Base: `118c972`.
Scope: M0b only, under plan pin `555bc433` and `contracts-v0.1.1`.
This review checks logic. No tests or gate ran in the reviewer worktree.

## F1 — MEDIUM — Apply `ignore_sigterm` at its script step

Status: CLOSED at `4f8ec55`. `IgnoreSigterm` is now an operation. Execution sets the flag only when the program reaches it.
Evidence: `program.rs:109` removes `IgnoreSigterm` from the operations. Line 130 sets the flag from the complete script during construction.
For `[PrintAfterInput(match="61"), IgnoreSigterm, Hold]`, the flag is true before the program receives `a`.
For `[Hold, IgnoreSigterm]`, the flag is true although the program never reaches that step.
The probe-script format specifies ordered steps. A waiting step holds later steps back (A5-1).
This behavior changes when a process edge can end the program with SIGTERM.

Required change: Keep `IgnoreSigterm` as an operation. Set the flag only when execution reaches that operation.
Add a regression case with a waiting step before `IgnoreSigterm`.

## F2 — MEDIUM — Dropping an endpoint must close its side

Status: CLOSED at `4f8ec55`. Endpoint drop closes its side. Close releases incoming descriptors outside the shared lock.
Evidence: `net.rs:167` sets `Shared.closed` only through an explicit `close` call. `End` has no `Drop` implementation.
Create a pair, then drop one endpoint without calling `close`. The surviving endpoint still reports writable.
Its reads return `WouldBlock` after queued bytes end. Its writes can succeed although no receiver exists.
An endpoint moved into a descriptor has the same defect when its owner drops the descriptor.
The in-memory stream must preserve the connection boundary and peer-close behavior of the real stream (A5-1, plan 2.3).

Required change: Close the owned side when an endpoint drops. Preserve queued outgoing bytes until the peer reads them.
Release incoming descriptors when their receiver closes. Drop descriptor objects outside the shared lock to avoid nested endpoint locks.
Add regression cases for a dropped link endpoint and a dropped route endpoint.

## F3 — MEDIUM — An injected write failure must produce write readiness

Status: CLOSED at `4f8ec55`. A pending write error now sets write readiness while the endpoint remains open.
Evidence: `net.rs:110` computes write readiness from peer closure or byte capacity. It ignores `write_error`.
Fill an endpoint's outgoing queue, then call `fail_next_write(ConnectionReset)`.
With write interest enabled, `is_ready()` remains false although `write()` would immediately return the injected error.
A binding that follows readiness cannot deliver that failure while the peer does not drain the queue.
This violates the edge's documented readiness behavior and plan 2.5 rule 8.

Required change: Include a pending immediate write failure in write readiness for an open endpoint.
Add a regression case with a full queue and write interest.

## F4 — LOW — `Scheduler::pick` can return an invalid index

Status: CLOSED at `4f8ec55`. Binary picks return 0 without a draw when fewer than two candidates exist.
Evidence: `scheduler.rs:125-126` use a two-way draw for `OperationDeferral` and `SpuriousWake`, irrespective of `candidates`.
With `candidates = 1`, either function can return 1. The `Scheduler` trait requires an index below the supplied candidate count.
This can make a generic driver select a nonexistent input.

Required change: Return 0 without a draw when only one candidate exists.
Validate or respect the supplied candidate count for the binary choice points.
Add a regression case through the `Scheduler` trait.

## F5 — LOW — The simulation reports idle work as a livelock at the limit

Status: CLOSED at `4f8ec55`. The simulation checks readiness after the limit without selecting or handling another input.
Evidence: `sim.rs:192-198` returns `Livelock` after exactly `limit` successful steps without checking whether work remains ready.
A simulation with one input returns `Err(Livelock { limit: 1 })` after handling its only input.
An empty simulation with `limit = 0` also returns a livelock.
The documented error requires work to remain ready after the limit.

Required change: Check readiness after the last allowed step without handling another input or drawing another scheduling choice.
Return `Ok(limit)` when the simulation is idle. Keep the error when work remains ready.
Add regression cases for exact-limit completion and an empty simulation with a zero limit.

## F6 — LOW — Record the required prior-art note and budget limits

Status: OPEN.
Evidence: The reviewed tree contains no P6 prior-art note. Its README contains only the crate description.
No pull request exists for `stage1/p6-testkit` at review time.
The implementer's message supplies prior-art decisions and budget numbers, but those facts are not in a durable review artifact.
BUILD.md rule 0 and the pair rules require a prior-art note in the feature design note or pull request.
The M0b brief requires the budget numbers in the pull request.

Required change: Record the prior-art decisions, library choices, and reasons for custom code in a design note or pull request.
Record the budget command, measured head, counts, and numbers in the pull request.
State that the measurement covers the failing `open` path. It does not establish the cost of passing Core transcripts.
Record the Stage 0 probe defect described below.

Delta review at `4f8ec55`: `DESIGN.md` records the prior-art decisions, custom-code reasons, and Stage 0 defect. Those requirements are satisfied.
The budget evidence remains incomplete. The note describes an uncommitted example instead of supplying its exact command or source.
It reports 132 transcripts and 32 seeds, but the pinned `botster-conformance/src/run.rs:40-43` returns after the first non-passing seed.
The harness reports no passing Core, so this procedure does not measure all 32 seeds.
The runner can also stop at a required control or feature before it calls `open`.

Remaining required change: Measure each seed separately, or correct the stated coverage to the actual executions.
For the M0b budget check, execute seeds 0 through 31 explicitly and count driver constructions and outcomes.
Record the exact command and example source so the measurement can be repeated.
Keep the limit that this measurement does not establish the cost of passing Core transcripts.
Copy the budget evidence into the pull request when the pull request exists.

## Contract question resolved by the lead

The pinned probe calls nested `run` for `PrintAfterInput`. That call ends the process with code 0 before later steps run.
The pinned probe also ends with code 3 for `IgnoreSigterm`.
The lead instructed P6 to preserve ordered script semantics and record the mismatch in its pull request.
Stage 0 owns the probe fix under a later authorized tag. P6 must not patch the probe or move its pin.
A real-harness run that hits the probe defect remains blocked by Stage 0. The probe defect is not a P6 finding.

## Scope checks

- The testkit adds no terminal model or terminal encoder.
- The scheduler and entropy use separate ChaCha8 streams.
- Variation occurs before machine handling or at an edge boundary. The code does not reorder finished Core results.
- Production machine crates gain no test branch.
- The facade uses the testkit only as a development dependency.
- `open` reports that no Core exists. The conformance pending list stays unchanged.
- `RefusalScript`, `RealCoreHarness`, and real-Core controls remain outside M0b.
- Route write size stays capacity-based. A5-2 names route read sizes as the source of this variation.

Every finding must close, including LOW findings. Any changed head requires review of its delta.

# P6 testkit review — Scope 2, step 2

VERDICT: CLEAN

Reviewed head: `9a55c2367ecfd193259d4e05c1da59f2b210eb12`.
Delta base: `3012073`.
Contracts pin: `contracts-v0.1.9`, commit `7f72acf8427ad7bf414db42d2360d5dccc7b3e13`.
No reported findings remain open. The reviewer ran no tests or gate.

- S2-R1 closes under R-19. The layer returns `AttachRefused { error, transport }` for every scripted attach refusal.
  Delegated results pass through unchanged. The test retains and uses an owned stream across refusal at occurrence 2.
  Calls 1 and 3 delegate. The test also checks the returned WebRTC fields.
- S4-R1 closes. The dependency graph resolves explicit package aliases and inherited workspace aliases.
  It reads target-specific dependency, build-dependency, and dev-dependency tables. The tests cover these cases and transitive aliases.
- S4-R2 closes. Cleanup takes and retires the group ID before signalling and reaping.
  The guard exposes pipes, not a mutable `Child`. Status checks use `NOWAIT`, so the leader stays unreaped until cleanup.
  The slow tests cover repeated cleanup, cleanup followed by drop, and leader exit before cleanup.
- The pin and lockfile agree with the fixed contract tag. Both ledger lists add the three E4 ids as pending.
  BUILD.md and the source status files are unchanged between the previous and new tags.

This CLEAN covers the reviewed refusal layer, edge building blocks, statement check, candidate reader, and group guard.
It does not approve completion of Scope 2. The real-process harness type, harness control dispatch, quiet fences,
identity-dependent controls, terminal oracle controls, and conformance proofs remain outstanding.
The gate must run on this exact head after CLEAN. A changed head requires delta review.

## Earlier review history

The entries below preserve earlier evidence. The CLEAN and closure statements above control their current status.

Latest delta reviewed: `3012073e6db33a953a69414575ba63807acc24b7`, against `c32c7a7`.
S3-R1 is closed. Output pieces preserve atomic boundaries and bound every read by the front piece and buffer.
Empty writes add no piece. The tests cover split writes, queued writes, later injection, and empty writes.
Open findings are S2-R1 descriptor preservation, S4-R1, and S4-R2.
The reviewer ran no tests or gate.

## S4-R1 — MEDIUM — The dependency statement can report a false absence

Evidence: `statements.rs:111-117` reads only top-level dependency tables and records their keys as package names.
Cargo permits `kit = { package = "botster-core-testkit", path = "../botster-core-testkit" }`.
With that dependency in `botster-core-ffi`, the graph records `kit` and reports no dependency on `botster-core-testkit`.
Cargo also permits dependencies under `[target.'cfg(unix)'.dependencies]`. The graph omits those tables entirely.
Renamed intermediate packages can also break the transitive check.
The A5-1 statement must detect these dependencies before it can prove the testkit is absent from FFI dependencies.

Required change: resolve Cargo dependency names to package names, including workspace-inherited aliases.
Include target-specific dependency tables. Prefer the resolved Cargo dependency graph if available.
Cover direct aliases, transitive aliases, and target-specific dependencies.

## S4-R2 — MEDIUM — The group guard can signal a group after releasing its identity

Evidence: `process_group.rs:35-50` exposes the mutable `Child` and retains the same `Pid` after `kill` reaps the leader.
Calling `kill`, retaining the guard, and then dropping it sends `SIGKILL` to the same numeric group ID twice.
Once the leader is reaped and the group ends, the OS can reuse that ID for another group.
The second signal can therefore reach a group that the test did not start.
The exposed `Child` also permits callers to reap the leader before the guard cleans up the group.
This breaks the guard's stated ownership guarantee and BUILD.md's test process ownership rule.

Required change: make cleanup idempotent and retire the group ID after cleanup.
Keep control of leader reaping in the guard so its identity remains reserved until group cleanup.
Expose the needed pipes or guarded status operations instead of unrestricted mutable `Child` access.
Cover explicit cleanup followed by drop, repeated cleanup, and leader exit before cleanup.

The candidate reader matches the current xtask manifest format and checks both binary hashes.
The real-process harness type and its integration remain outstanding. The current delta provides helper types only.

Latest reviewed head: `c32c7a7eae0b586998a778263e0e7e39248958be`.
The review covers the refusal fix `78d2452`, edge controls `38cbabf`, and scheduler overrides `c32c7a7`.
S2-R4 is closed: `checked_add` rejects an unrepresentable occurrence before insertion.
The boundary test checks rejection after a counted call and acceptance of the largest representable occurrence.
Open findings are S2-R1 descriptor preservation and S3-R1 below.
The reviewer ran no tests or gate.

## S3-R1 — HIGH — Atomic output markers can corrupt program output

Evidence: `program.rs`, `take_injected` and `Program::read`.
Each atomic marker stores an offset from the current output front.
A read updates only the first remaining marker. Markers behind it retain offsets for bytes already read.
When a split atomic write completes, the next marker still counts its earlier pieces.
The read length can then exceed the available output. `pop_front().unwrap_or_default()` supplies zero bytes for the deficit.
This changes the program's output, contrary to A5-1 and A5-2.

Concrete sequence with a holding program, `write_size(Some(3))`, and a four-byte read buffer:

1. Queue `write_once(b"abcdef")` and `write_once(b"XYZ")` before reading.
2. Reads return `abcd`, then `ef`, then `XYZ`.
3. The stale marker remains with offset 1 and length 3, although the output queue is empty.
4. Queue `write_once(b"pq")`.
5. Reads return `p`, then a three-byte result `q\0\0` instead of the remaining single byte.

An empty atomic write is another boundary case: its zero-length marker can return `Ok(0)` while later output remains queued.

Required change: keep every marker consistent with the output front, or store output pieces without stale absolute offsets.
Never return more bytes than the output holds. Preserve separate atomic writes when the buffer can hold them.
Cover multiple queued writes, a split first write, later injection after draining, and an empty write.

The other reviewed controls operate at the program, stream, or scheduler source.
The review found no reordering of finished Core results and no production test branch in this delta.
These controls remain building blocks. Harness dispatch, quiet fences, identity-dependent controls, and real Core proofs remain outstanding.

Delta reviewed: `04b0ca3bbd2536fdb279d26d35b92ccd14decb47`, against `4a001508`.
S2-R1 counting is closed: the precheck advice is removed, and the layer counts each attach once.
Its test checks delegation on calls 1 and 3, with a scripted refusal on call 2.
S2-R1 descriptor preservation remains open under R-19 until the fixed contract tag exists.
S2-R2 is closed: both missing rows cite their clauses, have row tests, and refuse before delegation.
S2-R3 is closed: absolute call numbers and conflict rejection preserve occurrences, including entries that converge after separate arming.
The reviewer ran no tests or gate for this delta.

## S2-R4 — LOW — An occurrence can overflow the absolute call number

Evidence: `RefusalScript::arm` computes `counted + occurrence` with unchecked `usize` addition.
Call `take("Start")` once, then arm `Start` with occurrence `usize::MAX` and `WrongState`.
The addition panics when overflow checks are enabled. Otherwise it wraps to 0, and the entry does not fire at its specified occurrence.
The harness accepts this occurrence from JSON on a 64-bit target.
The previous counter did not add occurrence to past calls.

Required change: use checked addition and return a script error when the call number cannot be represented.
Reject the entry without changing the script. Add a boundary test after at least one counted call.

The earlier findings below retain their original evidence. The delta status above controls their current status.

Reviewed head: `4a0015085fd91430cfc0204ebb16eeafc0dc3846`.
Base: `06f1f04`.
Authority: Scope 2 brief, plan pin `c43693ff`, and `contracts-v0.1.7` at `f14c895`.
The reviewer checked logic only. The reviewer ran no tests or gate.

## S2-R1 — HIGH — A scripted attach refusal drops the caller's endpoint

Evidence: `crates/botster-core-testkit/src/refusal.rs:605-618`.
`attach` owns `RouteTransport` and returns `Err` when the script matches.
For `RouteTransport::Stream`, that return drops `StreamEndpoint` and its owned stream.
The method comment states this behavior. Plan 4.2a requires the descriptor to stay with the caller.
DP-2 specifies ownership from a successful attach return. It does not define the refusal return shape.

The comment's proposed harness precheck also has a counting error if the call proceeds through the current layer.
For occurrence 2, the precheck decrements the entry to 1. The layer then refuses the first attach call.

Required change: preserve the caller's stream on refusal. Count each attach call exactly once.
Add proof with an owned stream endpoint, including a refusal at occurrence 2 and an unchanged successful delegation.
The lead confirmed the counting defect. Steward ruling R-19 (`botster-contracts` commit `8e1f51c`) confirms descriptor preservation.
Every synchronous attach refusal must return the caller's transport. Plan 4.2a stands.
The by-value refusal result is a contract-crate defect. Stage 0 will fix the trait.
The descriptor part stays open until P6 returns the transport on every refusal path against the fixed crate.
P6 must wait for the fixed tag and pin authority. Other findings can close during that wait.

## S2-R2 — MEDIUM — The table omits two synchronous calls

Evidence: `ROWS` has no `snapshot_formats` or `tap_read` row.
Their layer methods at lines 600 and 623 always delegate.
`arm("snapshot_formats", 1, "UnknownSession")` and the corresponding `tap_read` entry return `UnknownCall`.
Both methods take a session and return `Result<..., CoreError>` in the pinned `CoreApi`.
ST-6 and TP-1 define these synchronous reads. Section 9.3 defines `UnknownSession` as a synchronous error.
Plan 4.2a requires a row per call, including the other synchronous calls.

Required change: add the missing rows with their source clauses and valid synchronous codes.
Add a unit test per row and verify that each method refuses before delegation.
Keep calls without a refusal result outside the script table.

## S2-R3 — MEDIUM — Conflicting entries silently change their occurrence

Evidence: `RefusalScript::arm` accepts two entries with the same call and occurrence.
`take` at lines 454-458 fires only the first entry and keeps the second at remaining 1.
Arm `Start` occurrence 1 with `WrongState`, then `Start` occurrence 1 with `PendingLimit`.
The first call returns `WrongState`. The second call returns `PendingLimit`, although its entry named the first call.
The same defect occurs when entries armed at different times converge on one call.
Plan 4.2a and the public `arm` documentation specify the n-th call from arming.

Required change: reject conflicting entries when arming, or apply another explicit policy that preserves the specified occurrence.
Do not silently move a refusal to a later call. Cover both simultaneous and converging entries.

The remaining layer methods preserve real Core results. The reviewed delta adds no production test branch.
Both harnesses and the conformance proof remain part of later Scope 2 integration.

---

# P6 testkit review — Scope 2, step 1

VERDICT: CLEAN

Reviewed head: `96140c9a8baf4263ff8e896f08d47d3e9226c53c`.
Base: `ccb04eb`.
Authority: Scope 2 brief, plan pin `c43693ff`, and `contracts-v0.1.7` at `f14c895`.
No findings remain open for this step. The reviewer ran no tests or gate.

This verdict covers the contracts pin move, status reporting, the base commit, and nightly verification.
It does not approve the later refusal layer, edge controls, real-process harness, or completion of Scope 2.

The review checked these behaviors:

- The pin move is a separate commit. The commit lists all 41 added Core ids and both withdrawn ids.
- The copied status files match the pinned source. The list check rejects a changed copy.
- Withdrawn ids stay in the ledger and leave the pending list. Their trials remain ignored even when ignored trials are included.
- Whole-id deferrals must equal the contracts' Core deferrals. Existing A6-2 authority and start-condition checks remain active.
- A not-applicable case does not defer its active id. The report lists the case and its reason separately.
- The report prints passed, failed, pending, deferred, and withdrawn counts. Ignored categories do not count as passed.
- The pending-list check, mutation diff, and changed-decoder selection use the same cached base commit.
- The first base lookup resolves the reference and records the commit. Later checks reuse that commit.
- Nightly verification uses `rustup run` with automatic installation disabled. A missing nightly reports the required prerequisite error.
- The delta adds no test branch to a production machine and no second implementation of Core.

The gate must run on this exact head after CLEAN. A changed head requires delta review.

## Scope 2 step 1 — mutation delta

The gate on `15f26f3` reported four missed mutants. The reviewer read the gate's `missed.txt`.

- The harness assertion distinguishes `injects_clock = true` from `false`.
- The new base test compares the resolved commit with both returned values. It distinguishes an empty string and `xyzzy` from that commit.
- The former status parser mutant changes `at + 1` to `at - 1`. It detects each separator one space later.
  Trimming each field removes that added space, so the mutation preserves field values for this format.
  The reviewer accepts the equivalence explanation. The new implementation uses `split("  ")`, trimming, and removal of empty pieces.
  It preserves valid fields separated by two or more spaces and removes the index arithmetic.

All four mutation findings are closed. The delta adds no production test branch and changes no Core behavior.
The gate must confirm the mutation result on `96140c9` after CLEAN. The reviewer ran no tests or gate.

# Prior review — M0b

VERDICT: CLEAN

Reviewed head: `9846f9edfcdce277e136ff66c7e008e0f05af72f`.
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

Status: CLOSED at `82fcaf4`. The design note and repeatable example state the measured paths and limits correctly.
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

Delta review at `7797e33`: the committed budget example executes each seed separately. The command and source now permit a repeated measurement.
Two statements in `DESIGN.md` remain incorrect:

- The no-driver pass is the withdrawn `conf::er_deadline_expired` transcript. The runner returns before constructing a driver.
- Constructing a driver does not prove that a run reaches `open`. The runner checks required features and controls before setup.
  For example, `conf::am_2_fairness_bound_across_routes` requires controls that the harness does not provide.

Remaining required change: Identify the withdrawn pass correctly. Describe the numbers as an early-exit harness measurement.
The early exits include unsupported controls, absent features, and failed `open` calls. Do not claim that all constructed-driver runs reach `open`.

Delta review at `82fcaf4`: the design note identifies the withdrawn transcript and describes the measurement as an early-exit harness measurement.
All six findings are closed. Copying this evidence into the eventual pull request remains part of the implementer's handoff.

## Final review result

M0b is CLEAN on `9846f9edfcdce277e136ff66c7e008e0f05af72f`. No findings remain open.
This verdict does not approve later P6 milestones or establish passing Core conformance.
The implementer must run the gate on this exact head after CLEAN. The reviewer did not run the gate.

## Mutation delta at `e33ff13`

The gate on `82fcaf4` reported 17 missed mutants, 192 caught mutants, and 33 unviable mutants. It reported no mutation timeout.
Evidence: `~/botster-sessions/shared/core-stage1/logs/p6-m0b-gate-82fcaf4.log`.

The delta adds assertions for the reported behaviors. The later gate found one assertion gap, recorded below.
The sampler refactor extracts the same rejection loop into `sample_below`. The call still draws from the same ChaCha8 stream.
Scripted boundary draws test rejection of the biased tail. Fixed binary draws distinguish the two inverted predicates.
The remaining assertions cover descriptor output and closure, interest, program errors, size, readiness, node identifiers, and clock movement.
The later gate confirmed closure of 16 findings. One descriptor-index mutant remained missed, as recorded below.
No production machine, contract pin, conformance pending list, or M0b scope changes in this delta.

## Mutation delta at `9846f9e`

The implementer reported one missed mutant from the gate on `e33ff13`: `LinkEnd::send_descriptor` changed `1 - side` to `1 + side`.
The previous tests sent only from side 0. Both expressions produce 1 for that side, so those tests did not distinguish the mutation.
The reviewer corrects the earlier claim that all 17 mutations had distinguishing assertions.

The new test sends from side 1 and receives on side 0. The original expression indexes side 0; the mutation indexes nonexistent side 2.
The test also checks the error after the peer drops. The delta changes no library code.
The remaining mutation finding is closed by this assertion. The gate must confirm the result on `9846f9e` after CLEAN.

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

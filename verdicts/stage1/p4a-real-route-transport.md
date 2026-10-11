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

## Correction source inspection before Round 2 — 2026-10-11

Reviewed head: `981a1a7441497aa7e2b6839253a5b2621ed64f89`.
The sole parent is the Round 1 head, `5ca808f82967f25f476988439a74b5bb9893e7f6`.
The stack base remains `e493ff5f6d8622f64feb1805fe1def887f69adad` (#240).
I read the complete five-path correction: 106 insertions and sixteen deletions. The diff check passes.
I requested the exact-head package verdict. This inspection precedes the formal Round 2 verdict.

### PRC-1 MEDIUM / F121 — OPEN: the new proof uses invalid limits

The new proof is `a_full_real_route_queue_stops_the_pty_reads_until_the_client_reads` in both existing real-worker fixtures.
It waits for the done marker while the client reads nothing. The cleanup deadline must expire without that marker.
It then reads the route, receives the complete three-million-byte payload and marker, and requires the done marker.
This bounded observation replaces the insufficient immediate check. The existing guards own the worker and payload groups.
The driver removes its redundant decrement. The machine still refreshes the budget after each nonempty PTY read.
The changed exclusion reason names the new proof and keeps the progressing-reader conformance ID pending.

However, the proof sets only `route_queue_bytes` to 64 KiB and retains the other default CoreLimits values.
At the code's contracts pin, `max_snapshot_bytes` defaults to 16 MiB (`limits.rs:144`).
The cross-limit rule rejects `route_queue_bytes < max_snapshot_bytes` (`limits.rs:399`).
The test sends these limits directly in a worker Launch and does not call CoreLimits validation.
The new retained runs therefore prove the budget path under an invalid configuration.
They do not close the finding, whose required correction specifies a legal queue.

Required correction: use legal related limits and assert that `CoreLimits::validate` succeeds before launch.
Retain the normal-run and ignored-budget reversal evidence for that legal configuration.
I sent this point to both P3 agents in `msg_plugin-w_1791703937_d1b503` and `msg_plugin-w_1791703937_8e4901`.

### PRC-2 LOW / F122 — The behavior is corrected; the documentation remains inaccurate

`write_once` now clears write readiness for a nonempty zero write before it waits for writable.
The updated public test requires no `RouteWritable` after a readable-only event.
The same test then requires `RouteWritable` after a writable event. Empty writes and interrupted retries keep their earlier behavior.
The selected reversal removes the readiness clear. The new assertion fails with `Some(RouteWritable)` where it requires None.
The source correction and proof address the incorrect event decision.

The module header and PR description still say that only WouldBlock clears readiness.
The correction also clears write readiness for a nonempty zero write.
I requested this documentation correction in `msg_plugin-w_1791703964_5fd870`.
Status: OPEN for that documentation correction. The behavior is closed in source.

### Retained correction evidence and limits

I read all three new logs completely:

- `~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/f121-budget-ignored-revert-macos.log`.
- `~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/f122-revert-macos.log`.
- `~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/mac-slow-981a1a74.log`.

The budget reversal replaces the driver's PtyReadBudget assignment with a no-op.
Both real-worker fixtures fail because the done marker arrives while the client reads nothing, in 0.52 and 0.67 seconds.
The reversal header names the parent plus the working correction. It is selected evidence, not an exact-head final gate.
The zero-write reversal fails its public event assertion. Its log also names the parent plus the working correction.
The normal Mac log names the reviewed head and Darwin 27.0.0.
It passes 77 worker tests, seventeen slow_cli tests, 39 slow_session tests, and 115 core-sys tests.
The new budget proof passes in both fixtures. The invalid limits still prevent PRC-1 closure.

GitHub names the reviewed head and the unchanged stack base branch.
The body correctly states that no new gate ran under the lead's pool rule.
The parent pre-review gate remains evidence for its parent. A final gate remains required after both CLEANs.
No status list, pin, transcript assertion, or production test branch changes in this correction.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (2 open)

## Round 2 — 2026-10-11: invalid budget-proof configuration

Reviewed head: `981a1a7441497aa7e2b6839253a5b2621ed64f89`.
Stack base: `e493ff5f6d8622f64feb1805fe1def887f69adad` (#240).
Source inspection: `64d170f42fab6419c2c1151b9f864527cf3084a4`.
The head contains one correction commit after the Round 1 head. The preceding inspection records its complete scope.

I read the complete package Round 164 / #243 Round 2 at `43c7535c10143446b3041304d1d42af2d372354a`.
The package reviewer leaves F121 open and closes F122's behavior. The package reviewer confirms the invalid-limit finding independently.
The package artifact also requires the AttachRoute applied limits to derive from the same valid CoreLimits as Launch.
I check that fixture: `attach_route` currently derives these limits from a separate default CoreLimits value.
The corrected proof must represent the host's valid launch and corresponding route request.

PRC-1 MEDIUM / F121 remains OPEN for valid limits, validation before launch, matching applied route limits, and retained proof evidence.
PRC-2 LOW / F122 is closed in source for the event decision and public proof.
Integration retains its documentation closure condition because the module header and PR body still describe the old readiness rule.
This is the sole difference between the current package and integration closure status.
The package reviewer does not dispute the corrected event behavior. I do not claim an additional runtime defect.

The new bounded stop/resume observation is otherwise correct. Removing the redundant decrement is valid.
The complete Mac logs and selected reversals retain the limits stated in the source inspection.
They do not establish F121 under a valid configuration. The final Linux gate remains required after both CLEANs.
The PR moves no conformance ID. The R-59 tally limit and #240 stack requirement remain in force.

This completes integration Round 2. The next round is within the normal three-round limit.
If that round is NOT CLEAN, the lead must decide before Round 4 starts.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (2 open)

## Source closure inspection before Round 3 — 2026-10-11

Reviewed head: `53502d5517b5cf5863fca0ae768712d0e52ab804`.
The sole parent is `981a1a7441497aa7e2b6839253a5b2621ed64f89`.
The stack base remains `e493ff5f6d8622f64feb1805fe1def887f69adad` (#240).
I read the complete two-path delta: sixteen insertions and thirteen deletions. The diff check passes.
Both finding closures are present in source and retained evidence. The exact-head package verdict is requested.
This entry does not issue formal CLEAN before that package verdict is read.

**PRC-1 MEDIUM / F121 — CLOSED in source and focused evidence.**
The proof sets both `route_queue_bytes` and `max_snapshot_bytes` to 64 KiB.
It asserts `limits.validate()` before launch. The shared Launch helper clones these limits into Launch.
The helper derives AttachRoute applied limits from the same CoreLimits value.
RouteClient also derives its decode bounds from that value. The other callers retain their default configurations.
The guarded real-worker proof retains the bounded negative observation, resumed delivery, and complete payload check.
The driver still refreshes its budget before the next read. The redundant decrement remains removed.

I read `~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/f121-legal-limits-macos.log` completely.
Its header names the parent plus the working correction and states the valid limits and validation assertion.
Both normal fixtures pass in 10.68 and 10.78 seconds.
The selected reversal replaces the real PtyReadBudget assignment with a no-op.
Both fixtures then fail because the done marker arrives while the client reads nothing, in 0.46 and 0.64 seconds.
This is selected proof evidence, not a complete exact-head gate.
The earlier invalid-limit evidence remains historical and is no longer the closure evidence.

**PRC-2 LOW / F122 — CLOSED in source and documentation.**
The corrected zero-write behavior and public readable-only assertion remain intact.
The module header now names a nonempty zero write as another case that clears readiness.
I read the complete current GitHub body. Its behavior and Prior art descriptions also name this case.
GitHub names the reviewed head and the unchanged stack base branch.

I read `~/botster-sessions/shared/core-stage1/evidence/p3-route-transport/mac-slow-53502d55.log` completely.
The header names the exact reviewed head and Darwin 27.0.0 after prebuild of three binaries.
It passes 77 worker tests in 10.64 seconds, seventeen slow_cli tests in 0.19 seconds, and 39 slow_session tests in 10.56 seconds.
The new budget proof has a named PASS in both real-worker fixtures.
The separate-process core-sys run passes 115 tests in 0.813 seconds.
The preceding individual passes add no distinct coverage.

The body retains the final Linux gate requirement under the lead's pool rule. The parent gate proves its parent only.
The current delta changes documentation and test helpers. The status lists, pin, and transcript assertions remain unchanged.
The progressing-reader ID stays pending for its separate real route_fill proof.
I find no additional integration issue in this delta.

GitHub access recovers during this inspection. The previously local Round 2 commit `8ac8dacb4813b6b7b605e02f8f4902fa96f11579` is now pushed.
The earlier DNS failures remain recorded in the handoff as historical limits.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 3 — 2026-10-11: real route transport is CLEAN

Reviewed head: `53502d5517b5cf5863fca0ae768712d0e52ab804`.
Stack base: `e493ff5f6d8622f64feb1805fe1def887f69adad` (#240).
Pre-review gate base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1` at that gate).
Risk: HIGH. Binding plan: 23aj at `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
The code still pins contracts .27 at `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
Source closure record: `c5cddcd1ce16533aa625014e8f700a65b546b68c`.

I read the complete exact-head package Round 165 / #243 Round 3 at `b8c7112641fc90ce249c2518e274613a3197eb23`.
It reports CLEAN with F121 and F122 closed. Its head, stack base, scope, and evidence limits match this review.
I also read the complete prior clarification at `3dbb9657a637dcbff591b40a9d6475bd479a6392`.
The source inspection and formal rounds above preserve all findings and closure evidence.

**PRC-1 MEDIUM / F121 — CLOSED.**
The named real-worker proof uses validated legal limits for Launch, AttachRoute, and client decoding.
It observes the blocked payload while the client reads nothing, then complete delivery after the client reads.
Both normal fixtures pass. Both ignored-budget reversals fail when the done marker arrives before client progress.
The exact-head focused Mac run passes in both fixtures. The progressing-reader conformance ID remains pending.
The proof uses public stream and marker results. It adds no private queue accessor or production test branch.
The uniform payload proves its byte count and content. It does not distinguish reordered identical bytes.
The already-reviewed queue and write paths retain ordering. This closure concerns the real budget path.

**PRC-2 LOW / F122 — CLOSED.**
A nonempty zero write clears write readiness and waits for a writable event.
The public event proof rejects readable-only resumption, and the selected reversal fails that assertion.
The module header and PR body now describe this behavior correctly.
Empty writes and interrupted retries keep their prior behavior. No observed UnixStream failure is claimed.

No integration finding remains at this exact head. No conformance ID moves.
The package artifact adds no new source issue or change to the scope already reviewed.
The R-59 tally still excludes unfenced AD-7 from the real minimum.
The separate GS exclusion work remains assigned outside this PR.

The pre-review gate at `5ca808f8` proves its parent source and combined-stack diff, as recorded above.
It does not establish a green gate for the correction head.
The lead's pool rule requires one final Linux gate after both CLEAN reviews. That gate must cover the corrections before merge.
#240 must land first. The PR must then retarget to `v1` and merge the current base.
At the final reference check, `origin/v1` is `475a442fa58cd2555c4121ce705ea3cf75470692`, after #238 merges.
This verdict reviews the unchanged #240 stack base above. It does not review a future import of that new `v1` head.
The retained base-merge check must satisfy the disjoint-source conditions, or both reviewers must review the actual source delta.
This verdict does not waive those gate or stack conditions and does not complete the P4a group.

The local Round 2 commit `8ac8dacb4813b6b7b605e02f8f4902fa96f11579` is pushed after DNS recovery.
No verdict commit remains unpublished because of that earlier failure.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)


### Reported composition scope after Round 3

P3 reports a new lead ruling in msg_plugin-w_1791709722_519ba8: #240 and #243 will land through one merge of #243.
P3 reports that #240's final gate fails real EV-5 because a Detach does not complete before PR C's RouteWrite handling.
Sol has received that report but has not reviewed the new failed-gate artifact here.
The #243 source head remains 53502d55 and now targets v1. Its current diff therefore includes #240 and PR C.
The preceding CLEAN covers 53502d55 against stacked base e493ff5f. It does not automatically approve this wider diff against v1.

P3 will import v1 after RC's fenced pin lands and request composition delta reviews from both reviewers.
That review must identify the exact new head and base and check both previously CLEAN own diffs plus the base interaction.
One full gate must cover the resulting exact head before merge. The old pool instruction supplies no gate waiver.
No composition review of that future imported head has started. No new approval comes from this information record.
The preceding source findings remain closed within their reviewed scope. The new composition and final gate remain pending.

## Round 4: combined composition inspection, review active

- READY head: `bbc83015fde9e51127029667ccc3dcae766a7645`.
- Recorded base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Head tree: `cacc29701284724665bba83ee82c7aafbbc260b8`.
- Plan: 23ar, `a2897c1204dbfe6bbc44f476783686a68cabf57a`.
- READY receipt: `msg_plugin-w_1791719202_427b4b`.

Sol reads the complete updated PR body. This review covers the combined #240/#243 composition and later source changes.
The previous CLEAN applies to stacked source only. This inspection supplies no formal source CLEAN yet.

### Base composition

Merge bb73428bf4fc2dc6935135820abd470654062ed7 has parents 53502d55 and exact base b3cb268f.
Its tree e103c4f8a62ebfdd656c559f37492bba6fabdcc5 equals Sol's independent git merge-tree result.
Sol reads the complete retained base-merge-check log. Conditions 2 and 3 fail; both pending-file conditions pass.
The six shared paths are .cargo/mutants.toml, .config/process-check-allow.txt, Cargo.lock, testkit worker.rs, worker/tests.rs, and worker main.rs.
Sol reads their complete own deltas against b3cb268f.
Sol independently compares their ordered added/deleted line sequences with the previously reviewed own deltas against 7e8e07d6.
All six sequences match exactly, with 7, 4, 1, 55, 141, and 115 changed lines respectively.
This is a composition delta review, not mechanical carry-over through the failed check.

### Post-merge source inspection

Sol reads all eighteen changed paths after bb73428b: 477 added lines and 111 deleted lines.
The descriptor edge now returns the host's copy when SCM_RIGHTS duplicates the endpoint.
The real edge returns that copy. The testkit edge transfers its endpoint by value and returns no copy.
The tap forwards the copy without consuming it.
The host driver keeps sent copies per link and route. RouteReceived or RouteClosed removes the matching copy.
A lost link closes every remaining copy and queues HandoffLost for each route.
The driver feeds LinkClosed before those queued handoff-loss inputs. The existing state-first and first-reason rules remain applicable.
Sol reads on_handoff_lost and on_link_closed. Starting/Stopping retain the existing end-state route path; Running closes HandoffFailed.
The worker queues RouteReceived after BindRoute. No new timer or production test branch is added.

The real-link hold proof drops the receiver's copy, proves a client write succeeds before the acknowledgement, and then sends RouteReceived.
A completed capture on the same link fences acknowledgement handling. EOF and BrokenPipe then prove closure of the host's copy.
The DP-2 byte proof sends known client bytes before handoff and ends that direction. The worker receives every byte through EOF.
The client receives the worker's exact first line. Both proofs use public link, stream, and Core results.
The shared helper retains its stop over a broken link and worker process ownership.

Sol reads the complete hold-reversal and host-byte reversal logs.
Removing the hold fails the pre-acknowledgement write with BrokenPipe. A host read or write fails the corresponding byte assertion.
The DP-2 reversal header names staged tree cacc2970, equal to this head's tree.
The 30-run green log names that tree but contains summaries only. It supplies no independently read per-test PASS identity.
The final gate must supply the named proof results. Sol runs no repeat campaign or reproduction.

### Pin and pending data

Cargo.toml and Cargo.lock move all contracts sources from .32 to exact .36 commit 96f48d6d.
Sol reads the complete eight-transcript, BUILD.md, and steward-rulings deltas between those exact pins.
The other changed contracts paths are the fence lint and its exceptions, already checked for plan-target scope.
No frozen contract, manifest, ledger, or runtime contract crate changes between these pins.
The fences retain assertion values. R-65, its state qualification, and the A21 caveat remain explicit.
The prior .36 remote tag check remains valid.

Only the three in_10 IDs leave core-pending.txt. The post-merge source leaves core-real-pending.txt unchanged.
Sol reads the Mac receipt completely. It names an earlier 95ffb505 working tree with .35 and the three removals.
Each ID reports five passes. The relevant transcripts equal .36 because .35-to-.36 changes no transcript.
This receipt does not replace the final exact-head gate. No pending removal is accepted until its required PASS is checked there.
DP-2 stays pending until PC's separate named-proof mechanism and a passing named proof satisfy the gate.
The full real-tier before/after table and EV-5 result remain pending.

### Required release confirmation and remaining work

The body reports publish=false, zero repository tags, zero GitHub releases, and no crates.io botster-worker release.
R-65 also requires the Core lead to confirm in the PR that no released protocol-1 worker exists.
Sol finds conditional design approval in the lead handoff, but no explicit release confirmation in the supplied PR body.
Sol asks the lead for that confirmation. This is the release condition from R-65, not a new protocol requirement.

The exact-head package round, final full gate, full before/after table, and release confirmation remain pending.
Current v1 has advanced to 0c5ac80e by merging #261. This head still records b3cb268f as its base.
A later current-base import needs its applicable review and its own exact-head gate.
This inspection approves no merge, gate, pin implementation, or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

### Round 4 continuation: API snapshot and release condition

Sol reads the complete one-line delta at `27d7e821f69c89f97b1476fd56e5f71e04556798`.
The API snapshot now states the already inspected link_send_descriptor result, including the optional host copy.
No executable source changes in that commit.
Sol reads the bbc83015 gate's exact head/base header, API error, final summary, and exit record.
fmt, clippy, taint, and lists pass. public-api fails at this snapshot line.
All five later steps report NOT RUN. The wrapper exits one. This failed gate supplies no execution approval.

Sol reads the complete lead confirmation at https://github.com/trybotster/botster-core/pull/243#issuecomment-6108744687.
The lead confirms that no released Core worker speaks protocol 1.
The comment records no repository releases or tags, publish=false, and the worker's inherited publishing setting.
The R-65 protocol-1 release condition is CLOSED. The lead receipt is `msg_plugin-w_1791719661_514ce9`.

The lead assigns one new round head containing the API correction, current v1 0c5ac80e, and the EV-5 flaky-entry removal.
That head needs its applicable delta review, focused Mac receipt, complete before/after table, and full exact-head gate.
The package reviewer reports no runtime finding at 27d7e821, but has not committed the formal composition round.
Round 4 remains active. This continuation supplies no formal source CLEAN or merge approval.

### Round 4 continuation: final composition head supplied

READY msg_plugin-w_1791720047_272cd7 names `3cf27c1b0f6941100b096570bb3ca51ade0a7c2e`, base `0c5ac80e512f986edd2ec1455012552a41c7f855`.
Its tree is bb072608e02424557d9cc95d395d4b5ca6c8f12b. Sol reads the complete updated PR body.
Sol reads dff7e476's complete fixture delta and the surrounding Link decoder and report reader.
The route fixture reads Launched and RouteReceived for route 1 in either order. It requires exactly one of each.
A duplicate or other lifecycle report fails. The existing reader preserves every decoded frame and socket timeout.

Merge 38635c9e has parents dff7e476 and exact current base 0c5ac80e.
Sol's independent merge-tree result equals its tree 6624c7cd563f406ea81436e1d39ac2982029f006, with no conflict.
Sol reads the full base-check record: conditions 1, 2, and 4 pass; condition 3 fails on the inspected fixture change.
All twelve other imported paths retain the exact base blob. The remaining imported path is the inspected known-flaky file.
Commit 3cf27c1b removes EV-5's entry from that file. The file retains only its explanatory comments.
This removal changes mutation selection; it supplies no conformance pending removal.

Sol reads the complete exact-head Mac summary receipt and the complete earlier EV-5 receipt.
The exact-head receipt reports all selected groups passed, including 19 slow_facade_worker tests and 150 real must-pass trials.
It contains summaries rather than individual proof names. Final gate review still needs the named public proof results.
The bbc83015 receipt names EV-5 PASS in each of twenty runs. Sol starts no repeat campaign.
The current full gate, A5-4 table, and formal package composition verdict remain pending.
This continuation supplies no formal source CLEAN, final gate approval, or merge approval.

### Round 4 continuation: package artifact and replacement test

Sol reads the complete formal package Round 166 addition in e04cca4c6322a28b00e062c35d46167d616ecebb.
The fetched stage1/review-p3 branch contains that commit. Sol independently confirms its publication.
The package CLEAN covers 3cf27c1b, base 0c5ac80e, complete composition, and post-import corrections.
Sol also reads the complete driver and worker reversal records. They fail on missing copy retention and missing RouteReceived respectively.
Those selected reversals supplement the inspected public real-link and DP-2 proofs. They replace no full gate.

The 3cf27c1b gate then fails at test-budget. Sol reads its exact header, named failure, final summary, and exit record.
The repository-list test still requires the EV-5 entry that this head removes.
The gate stops before slow, mutants, and fuzz. This failed gate supplies no execution approval.
Lead receipt msg_plugin-w_1791720429_53ea49 assigns the replacement head for this same round.

Sol reads the complete correction at `7e577f68a167fb1f21e248fcd3300ee87118b9bf`, parent 3cf27c1b.
It changes only the repository-list test in xtask/src/known_flaky.rs: four added lines and nine deleted lines.
The test still reads and parses the actual repository file. It now requires the parsed list to be empty.
The parser, selection, verdict, and nonempty fixture tests do not change. No check of the empty list is removed.
This matches the inspected list removal and the plan's Stage 1 exit condition.
The package delta verdict, replacement exact-head gate, and A5-4 table remain pending.
No formal integration source CLEAN was issued for the superseded 3cf27c1b head.

The earlier #240 and stacked #243 Prior art checks remain applicable to their retained source.
The later changes reuse the descriptor edge, per-link driver, worker action queue, frame decoder, and real-link fixture.
No dependency or alternative runtime is added. The protocol condition remains closed by the lead's explicit PR confirmation.
This continuation accepts no gate, merge, new minimum result, future pending removal, or future base import.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

### Round 4 formal source verdict: replacement head

- Head: `7e577f68a167fb1f21e248fcd3300ee87118b9bf`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Parent: `3cf27c1b0f6941100b096570bb3ca51ade0a7c2e`.
- Tree: `c057c2d569a9fe42eb23563431a7652d7e79071e`.
- Package delta CLEAN: `9d9c765ba0eb63612d206f1fca51d0b7e4bda716`, verdicts/p3-worker.md.
- Package receipt: `msg_plugin-w_1791720707_e89f28`.

Sol reads the complete package Round 167 addition and verifies publication on fetched stage1/review-p3.
That verdict covers this exact head and base. Round 166 retains its complete composition review for unchanged source.
Sol's cumulative Round 4 inspection covers both base imports and every later own source delta recorded above.
The replacement changes only the actual repository-list test. Every other file equals the inspected parent.
The test still fails on a read error, parse error, or any parsed entry. It requires the empty list from the reviewed removal.
The parser, selection, verdict, and nonempty fixtures remain unchanged.

Zero integration source findings remain open. The R-65 protocol condition is closed by the lead's explicit PR confirmation.
The complete composition retains the inspected state-dependent failure paths, descriptor ownership, bounded route transport, and public behavior proofs.
The .36 pin and three in_10 removals retain their recorded execution limits. DP-2 stays pending.
The preceding Prior art checks remain applicable. The replacement reuses the current list parser and repository-file test.
No dependency, alternative runtime, timeout, retry, or test exclusion is added.

The Mac receipt still names parent 3cf27c1b. Its unchanged runtime coverage is retained within the stated summary-only limit.
No completed replacement full gate or A5-4 table has arrived.
Their exact-head results, named public proof outcomes, mutation identities, and failure classifications still need acceptance review before merge.
The body must record the replacement result before merge, as the package verdict requires.
This CLEAN covers source only. It accepts no final gate, merge, new minimum result, future status removal, or future base import.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 7e577f68a167fb1f21e248fcd3300ee87118b9bf

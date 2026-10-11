# Integration review: #233 program controls

## Source inspection before Round 1 — 2026-10-10

Inspected head: `e75ba5f71c6187697383fd9b7f298548c5ac8008`.
Base: `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
Plan: revision 23ae, `b895ad493b9919dd63a9e11a596ba5fd210f1ea5`.
Contracts: contracts-v0.1.27, `b161db87ca82873b4f6b21d8433a69f086abb7cf`.

The implementer supplied this head for early source inspection. The implementation is not READY.
The exact-head green gate and package verdict remain pending. This inspection completes no review round and grants no CLEAN verdict.

I read all five changed files, the P3 mapping, the PC brief, and both PC handoffs.
I read the contracts probe control loop and the shared process wait and read implementations.
The real harness serves the five program controls together with the merged P5 controls.
The plain harness serves no control. Both harnesses supply their own probe socket through probe_env.
The request and report types come from the pinned contracts crate. The harness does not write PTY bytes.
The observer wake exception and its no_spurious_wakes limit remain in the PR body and P3 mapping.
The temporary accept exception names the bounded, non-blocking loop and P6's replacement obligation.

The pending-list diff removes 16 IDs. It retains all six held-write IDs from the PC audit.
It also retains ev_5_full_queue_defers_transition_not_output. The handoffs' claim of 17 removals is incorrect for this head.
The three removed minimum IDs are ad_3_adopted_equals_started, tm_3_due_deadline_fires_in_pump, and tm_3_next_deadline_is_host_armed.
No green exact-head evidence establishes these removals yet.
I read all three minimum transcripts at the pinned contracts commit.
ad_3 retains its Activity(Output) fence before the terminal and screen checks, and its await_pty fence after adopted input.
The two tm_3 transcripts use the existing harness clock controls. The program control does not advance a clock.

### SI-1 HIGH — A request write has no deadline and holds the report lock

Evidence: real/program.rs:198–212 and 297–299. The accepted socket is blocking.
request() holds Hub.state while requests.write_all() writes the complete request line.
The acknowledgement deadline starts only after write_all returns.
A peer that stops reading can block this write without a deadline.
The report reader needs the same lock to update input and acknowledgements. Programs::drop needs that lock to start shutdown.
The contracts probe sends reports from the same event loop that reads requests.
With both socket directions under backpressure, the held lock also prevents the harness from draining reports.
The new connection and acknowledgement deadlines therefore do not bound the complete request.

Required change: bound request I/O with the shared cleanup deadline. Release Hub.state during blocking I/O.
Preserve request order and acknowledgement association. Keep shutdown possible during a failed or blocked write.
Supply a socket-boundary proof that a peer which does not read cannot hang the request or cleanup.
Status: OPEN. Source evidence only; I ran no reproduction job.

### SI-2 MEDIUM — Exit watchers survive harness cleanup

Evidence: real/program.rs:168–173, 237–253, and 408–422.
connected() drops each exit watcher's JoinHandle. Programs owns only its accept thread.
watch_exit() re-arms await_end after every cleanup deadline. It never checks Hub.state.closing.
The request tests watch std::process::id(), which remains live after Programs::drop.
Those watchers retain their Hub references and continue until the test process exits.
The Drop comment says the watcher ends at its next bound, but the loop does not implement that condition.
The shared await_end documentation also warns that a reused pid can select another process.

Required change: own every watcher. Stop every watcher when the harness closes, within the existing cleanup bound.
Join every watcher and release its Hub reference. Do not reap a production-owned worker.
Supply a cleanup proof with a watched process that remains live after the harness closes.
Status: OPEN. Source evidence only; I ran no reproduction job.

### SI-3 MEDIUM — Cached probe state bypasses process identity

Evidence: real/program.rs:65–70, 154–181, 198–201, and 231–233.
State keys probes, watched, and exited by pid. connected() checks start_time only when watched.insert(pid) succeeds.
The next call with that pid skips the identity check, even when its ProcessIdentity has a different start_time.
The loop also returns a cached probe before it checks exited.
Thus a cached probe can override a failed identity check. A reused pid can also select state from the previous worker.
The probe hello supplies only its parent's pid. The hello alone cannot establish the recorded worker's start_time.
This contradicts the stated R-46 process identity check and can apply a control to another recorded worker identity.

Required change: bind cached probes and exit watches to the complete recorded ProcessIdentity.
Reject mismatched identities before returning a probe. Do not reuse logs, acknowledgements, or exit state across worker identities.
Supply a boundary proof that an existing connection does not make a mismatched start_time valid.
Status: OPEN. Source evidence only; I ran no reproduction job.

I sent all three source findings to the implementer and package reviewer.
I changed no product code. I ran no build, test, gate, mutation job, merge-check job, or reversal job.

## Correction source inspection before Round 1 — 2026-10-10

Inspected head: `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
Correction commit: `375a1bb91ae72573f73e2ef4ee1a28aa825dcc1f`.
Merged base: `0a564513`, which includes reviewed #237 and #234.
The implementer reports a running gate and will send READY later.
No completed exact-head green gate or same-head package verdict is supplied.
This source inspection completes no round and grants no CLEAN verdict.

I read the complete correction delta and its callers, report reader, cleanup, and socket proofs.
Only real/program.rs changes in the correction commit after the previous inspected head.
The automatic Git merge tree equals the published head tree: `6bafaa011c2ead99f22f5cc94b4e4937b1d8a592`.
This is a comparison of source objects. I ran no executable merge-check job.
The own diff over the merged base still changes the five PC paths.
The pending-list removal remains sixteen IDs. The six held-write IDs and ev_5_full_queue remain pending.

### SI-1 HIGH — Partial source correction; proof and deadline remain open

request() now takes an Arc of the request socket and the acknowledgement target under the lock.
It releases the lock before writing and takes the lock again for the acknowledgement wait.
The accepted socket is nonblocking. send_line waits for OUT with the shared cleanup deadline after WouldBlock or Interrupted.
The write and acknowledgement wait use one deadline. The report reader keeps its existing Bounded line reader.
This removes the reported lock-held blocking write.
The serialized public control dispatch retains request and acknowledgement order.

Two points still prevent closure of SI-1.
First, send_line checks expiry only after WouldBlock or Interrupted.
Its positive-write path can start another write after the deadline has passed.
The shared Bounded reader checks expiry before each new read, including reads after positive progress.
Required change: check the same deadline before each request write. Keep the existing timeout failure.

Second, reports_are_taken_while_a_request_waits_for_room sends its Input report immediately after it spawns the request thread.
The report can finish before the request starts. The proof can therefore pass with the old lock-held write.
Its one-megabyte request proves no ordering between the write and report processing by itself.
Required change: observe request bytes at the real socket before sending Input.
Then leave the remaining request unread until the report arrives.
Keep the exact input-log and acknowledgement assertions. Use the existing cleanup bound for the fence.
Also retain a socket-boundary proof that a peer which never drains the request produces Bad at the cleanup bound.
No private accessor is required. Status: OPEN, pending the source and proof corrections.

### SI-2 MEDIUM — Closed in source

The correction removes watch_exit and all exit-watcher thread creation.
connected() checks the process identity when it waits and uses the existing cleanup bound.
Programs owns its accept thread. The accept thread owns and joins all report reader threads.
Drop starts shutdown under the state lock, ends the connection sockets, and joins the accept thread.
There is no detached watcher or watcher-held Hub reference left to stop.
No code reaps a production-owned worker.
The exited-before-connect proof uses an OwnedChild and confirms the child's exit before the request.
This source change closes the detached-watcher finding. Exact-head evidence remains pending.

### SI-3 MEDIUM — Closed in source

State.probes now uses the complete ProcessIdentity as its key. The pid-only watched and exited sets are gone.
The report reader records the parent's pid and observed start time together at registration.
Request, acknowledgement, and input lookups all use that same complete identity.
The new socket proof connects a probe, then requests input with the same pid and another start time.
It requires Bad and cannot select the earlier probe.
This source change closes the reported cross-identity cache lookup. Exact-head evidence remains pending.

I sent the remaining SI-1 points to both PC agents before READY.
I changed no product code and ran no build, test, gate, mutation, measurement, or reversal job.

## SI-1 follow-up source inspection before Round 1 — 2026-10-10

Inspected head: `686300e2b1e6cbaded29761dc8963483b4ac03bc`.
Previous inspected head: `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
Only real/program.rs changes: 33 insertions and four deletions.
I read the complete follow-up delta.

send_line now checks the shared deadline at the start of every loop pass, before each write.
The positive-progress path cannot start another write after an observed expiry.
The existing nonblocking socket, OUT wait, released state lock, and acknowledgement deadline remain.

FakeProbe::request_started waits for socket readiness with IN and the existing cleanup bound. It consumes no request bytes.
The report proof calls that fence before sending Input and leaves the request unread until the report arrives.
The report can no longer complete before the harness starts the request.
The proof retains the exact input bytes, complete request, acknowledgement, and successful control result.
The new never-draining-peer proof leaves a one-megabyte request unread and requires Bad from the write deadline path.
These assertions observe socket and control results. They expose no private product state.
The implementer reports that the lock-held-write reversal fails locally. Its retained evidence is still to be supplied with READY.

SI-1 HIGH is CLOSED in source. SI-2 and SI-3 retain their preceding source closures.
All three findings now close in source. This does not substitute for the final gate or same-head package verdict.
The implementer reports a running gate and will send READY later.
This inspection completes no round and grants no CLEAN verdict.
I changed no product code and ran no build, test, gate, mutation, measurement, or reversal job.

## Round 1 — 2026-10-11: real program controls and base imports

Exact head: `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Parents: `fce17f78b96009cb560fb3896a6f957840ec2c43` and the base.
Tree: `3f185d2babb3ba4f811ad3b13b5cc135a84c7bc3`.
READY: `msg_plugin-w_1791698170_384245`; latest base import: `msg_plugin-w_1791699106_e2ff02`.
Package CLEAN: `3ebf5746ef1876ce00e86e2bee5bd7a86a318621`, `verdicts/pc-program-controls.md`.
I read the complete current package round on this exact head.
It supersedes the package's first CLEAN at `deec5b5b` and its SI-4 NOT CLEAN supplement at `b5ca3ab9`.
Authority: plan 23ai at `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`, contracts .27, and BUILD.md at `56bd0a5`.
The PR correctly states HIGH for shared testkit changes and pending-ID removals.

### Finding closure

**SI-1 HIGH — CLOSED.**
The request writer clones its socket under the state lock and releases that lock before it writes.
It uses a nonblocking socket. Every write checks the same deadline that bounds the acknowledgement wait.
The reader can therefore record input while a request waits for socket room.
The proof waits for public socket readiness before it sends the report. The report cannot precede the request write.
A separate proof requires Bad when the peer never reads the request.
I read the selected lock-held reversal at `686300e2` in `botster-core-pc-si1-lock-held-reversal-686300e2-20261010-184710.log`.
That reversal fails the report-progress proof; the unmodified proof passes in 0.37 seconds.
These are selected records, not a complete final-head gate.

**SI-2 MEDIUM — CLOSED.**
Detached exit watchers are removed. The accept thread owns and joins every report reader.
Drop marks closing, shuts down every retained connection, wakes the accept, and joins the accept thread before removing the socket.
Connections enter the retained set before the reader starts, including connections that have not sent HELLO.
No reader needs a repeated private closing check: socket shutdown ends its bounded read.

**SI-3 MEDIUM — CLOSED.**
The probe map uses the complete worker ProcessIdentity: PID and start time.
A connected probe for one start time cannot satisfy a control for a later start time at the same PID.
The public control proof covers that already-connected state and requires Bad.

**SI-4 LOW — CLOSED: the description lacked required Prior art comparisons and custom-infrastructure reasons.**
I sent this finding to PC and its reviewer in `msg_plugin-w_1791698546_b33209` and `msg_plugin-w_1791698546_ff52a2`.
I read the final published correction and checked the cited old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The note rejects the worker's test-only routing channel, shell-cat payload tests, and the old bounded-wait helper.
It names the reused production probe protocol, process helpers, serde_json, rustix poll, and testkit control semantics.
It explains the synchronous listener, shared state, request writer, silent report reader, and temporary accept loop.
It correctly permits composition through exported HostEdges under plan 23l.
It compares per-call socket timeouts with the single deadline needed for the whole request and acknowledgement.
It removes the unsupported claim that no library can bound a socket write. The change copies no old code or tests.

### Package interfaces and conformance

The own delta changes five paths: 1012 insertions and 26 deletions.
The production worker owns the real PTY. The harness controls the production probe through its separate control socket.
The harness writes no expected terminal bytes and changes no terminal mode.
The probe acknowledges after applying a request; an output acknowledgement does not promise a completed terminal write.
The input result is the probe's cumulative read log. Both harnesses name their socket; only the tapped harness serves the five controls.
The injected clock changes only when the transcript advances it.

P3 permits the input observer to signal host wakes under its recorded rule 3 exception.
The body states that a transcript checking no_spurious_wakes during probe input cannot pass with this exception.
The current pin has no such transcript. The structural observer-wake change remains a contracts obligation.
The temporary accept exception names its nonblocking listener, bounded poll, and P6's bounded-accept replacement obligation.
The added rustix event feature enables that existing library API; the contracts pin and lockfile remain unchanged.

The mutation correction preserves partial report lines across cleanup deadlines and tests a probe that remains silent across a deadline.
Socket shutdown still ends those readers. The pure accept-error decision returns Wait, Next, or Stop, with tests for each result.
The default blocked state and all five control dispatches have proofs.
I read these corrections and the affected dispatch, cleanup, request, and reader paths.

Only sixteen IDs leave the real-pending list. The final group has 22 IDs: seventeen earlier IDs and five from #227.
The final real-pending list has 64 IDs. The Core pending list and other status lists equal the base.
I read the three minimum transcripts at the contracts pin: `ad_3_adopted_equals_started` and both `tm_3` IDs.
The AD-3 output and input fences remain intact. The TM-3 clock steps and exact deadline assertions remain intact.
I checked an actual slow_conformance PASS record for each of the sixteen removals in the completed Linux gate run.
The six IDs requiring steward approval remain pending. The four minimum route IDs and EV-5 full-queue ID remain pending.
`ev_2_one_marker_per_session` and `in_5_lane_bounds_refuse_at_begin` remain pending under lead ruling A, `msg_plugin-w_1791698173_6bb0a3`.
Their two Linux pending-real passes do not prove the missing Mac behavior or input-report fence.

### Source imports

I read the failed `7a4ab70b` to `fce17f78` base-check record and the actual conflict resolutions.
The resolutions retain P5's stand-in, registry, hold, release, and refusal controls beside the five program controls.
The resolved list retains #227's five new IDs and this PR's sixteen removals.
The program module equals the gated source: blob `2a83304cc10f4e2463bc41d90abeced1d62fb434`.
I do not treat the failed check as automatic acceptance.

I read `~/botster-sessions/gates/botster-core-pc-base-merge-check-fce17f78-ec317eae-20261010.log` completely.
Both ancestry checks and all four conditions pass. The #241 paths are disjoint from the five own paths.
My full-index own diffs match before and after that import: 50444 bytes, SHA256 `d96c1406b582875cac7ecd22ce9d105d18b64a74fa31ce72a4fe602f283cab0d`.
The automatic source merge tree equals the actual tree above and has no conflict.
The final diff check passes. GitHub names this exact head and base at the final check.
No imported accepted behavior is removed. The own delta changes no mutation exclusion or transcript assertion.

### Retained Linux evidence

Gate head: `7a4ab70bf08ff28344e9e47cad09d923ff3ae1d4`; base: `15791ea336c5f3d0fbd9483a1e190ec01743351e`.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-7a4ab70b-pool-20261010-201116-66449.log`.
The first allocation stops during migration. I use the complete second run on Linux.
All ten stages pass in 6140.8 seconds.
Default: 1536 passed, 497 skipped, in 14.647 seconds. Slow: 440 passed, 1955 skipped, in 279.068 seconds.
Testkit: 193 passed; minimum 50 / 69. Strict pending runs 386 / 407 transcripts, with 21 real-only IDs and two held IDs.
Real minimum: 38 / 68 passed and 38 / 69 accepted. Pending-real: 55 IDs, with the two passes retained under lead ruling A.
These counts belong to the old gate base, before the reviewed imports.

I read all 79 mutation outcomes in both stages. The identity multisets match, with no duplicate identity.
Stage 1 misses all 79 because this source requires the slow feature, in 214.6 seconds.
The unmutated slow baseline passes in 294.1 seconds.
Stage 2 catches 59 and finds 20 unviable, with zero missed or timeout, in 5201.4 seconds.
The mutation step takes 5713.3 seconds. The job and wrapper exit zero after 9740 seconds: 3586 queued and 6154 running.

### Focused Mac evidence and limits

I read `~/botster-sessions/gates/botster-core-pc-program-controls-mac-fce17f78-20261010-230102.log` completely.
It names `fce17f78` and macOS 27.0.1 arm64. All fifteen program tests and the both-harness probe-env proof pass.
The broader real:: run has 30 passes and one unchanged P5 zombie-identity test failure, with exit 101.
That test expects Matches where Mac reports Absent. I compare its body with the base; it is unchanged.
The lead assigns that separate failure to P5. This log clears the focused PC requirement, not the broader Mac suite.
The real conformance run passes 141 tests with zero failures in 345.14 seconds, with exit zero.
Its minimum is 39 / 68 passed and 40 / 69 accepted. Pending-real reports zero passes among 64 IDs.
The disjoint #241 import changes none of this PR's operating-system behavior.

All four findings close, including LOW. No integration finding remains at the exact reviewed head.
The lead's pool rule still requires the final gate on the CLEAN head before merge. The older gate does not prove that final head.
This verdict clears no additional pending ID and does not complete the PC group.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

## Round 2 — 2026-10-11: permanent socket-error proof

Reviewed head: `8cfdd6715c6a69c419d9819636e185bfff74bb24`.
Reviewed base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1`).
The sole parent is the Round 1 head, `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
Risk: HIGH; the delta belongs to the shared testkit PR.
The binding plan is 23aj at `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
The code still pins contracts-v0.1.27 at `b161db87ca82873b4f6b21d8433a69f086abb7cf`.

I read the complete current package delta record at `d26e3057d4909ec718de1e49ad2feb094a806e83`.
It reports NOT CLEAN with SI-5 open. SI-1 through SI-4 remain closed.
This round supersedes integration CLEAN `a7d8d900992deda9846573487dde2ab578d76df0` for the current PR.
The earlier source review and its finding closures remain in this file.

### SI-5 MEDIUM — The permanent socket-error guard lacked a reliable mutation proof

The final gate at `ec317eae` missed the guard-to-true mutant in `send_line` during stage 2.
The mutant retries permanent socket errors until the cleanup deadline instead of returning the socket error.
Plan section 8 treats a missed mutant as a review finding.

Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-ec317eae-pool-20261010-231846-63449.log`.
Its head and base equal the Round 1 head and this round's base.
I read the header, reports, summary, and all 79 stage 2 outcomes.
The identity multisets contain the same 79 unique identities in both stages.
Stage 1 misses all 79. Stage 2 reports 58 caught, 1 missed, 20 unviable, and zero timeout.
The sole miss replaces the WouldBlock or Interrupted guard with true at `real/program.rs:269`.
Stage 2 takes 2377.2 seconds. The mutation step takes 2660.6 seconds.
Eight earlier steps pass. Mutation fails. Fuzz does not run.
The job exits 1 after 3121 seconds. The wrapper exits 1 after 3122 seconds.
IN-5 is the sole pending-real pass in that run. It remains pending under the lead's ruling.

Required closure: retain the behavior proof, its selected reversal, focused Mac evidence, and a passing corrected final Linux gate.
**Status: OPEN. The source correction, selected reversal, and focused Mac evidence are present. The final gate is not green.**

### Source correction and focused Mac evidence

I read the complete delta and the surrounding writer and tests. The diff check passes.
The delta adds one 17-line test in `crates/botster-core-testkit/src/real/program.rs`.
The test is `a_request_write_to_a_closed_probe_fails_with_its_error_not_at_the_bound`.
It creates a UnixStream pair, sets the harness end nonblocking, and drops the peer.
It calls `send_line` with the existing cleanup deadline. It requires BrokenPipe or ConnectionReset and rejects TimedOut.
The assertion observes the socket result. It reads no private queue state and imposes no elapsed-time assertion.
Runtime code, transcript assertions, timeout values, status lists, the pin, and mutation exclusions remain unchanged.
I find no additional source defect in this delta.

I read both focused Mac logs completely:

- `~/botster-sessions/gates/botster-core-pc-send-line-test-mac-8cfdd671-20261011-001400.log`.
- `~/botster-sessions/gates/botster-core-pc-send-line-guard-reversal-8cfdd671-20261011-001419.log`.

Both headers name the reviewed head and macOS 27.0.1 arm64.
The unmodified run passes all 16 program tests, including the new proof, in 11.01 seconds; exit 0.
The reversal log contains the complete guard-to-true diff.
The new proof fails with TimedOut after 10.00 seconds; exit 101.
These logs satisfy the focused Mac requirement for this delta. They do not replace the final Linux gate.
The earlier reversal log without a head, platform header, or normal-run PASS is no longer the sole evidence.

### Corrected Linux gate and remaining condition

Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-8cfdd671-pool-20261011-001231-18048.log`.
I read its exact-head header, list reports, new-test PASS, AD-7 failure, suite summaries, and final summary.
The gate base equals the reviewed base. Seven steps pass. Slow fails. Mutation and fuzz do not run.
Default tests: 1604 passed and 484 skipped in 11.391 seconds.
Slow tests: 444 passed, one failed, and 1976 skipped in 114.458 seconds.
The new closed-peer proof passes on Linux in 0.003 seconds.
AD-7 fails at seed 0, step 6: expected `alive:false`; received `alive:true`.
This run supplies no corrected mutation result.
The job exits 1 after 247 seconds: 73 queued and 174 running. The wrapper exits 1 after 248 seconds.

The implementer reports that the lead assigned `payload_alive` to P5 and forbids another gate before that fix merges.
Messages: `msg_plugin-w_1791703021_930d16` and package confirmation `msg_plugin-w_1791703060_bf5baa`.
I do not treat that assignment as a waiver of SI-5 or the green final gate.
After the fix merges, the PR needs the current base, review of any source delta, and the required final gate.
This verdict clears no additional pending ID and does not complete the PC group.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open)


## Round 3: two v1 imports and conflict review

VERDICT: NOT CLEAN (1 open: SI-5)

- Head: 4d05d270d6c6cebcdae247050cd702b6a157c639.
- Base: 554d43e890b391cefc9da205c71db6856accb21d.
- Previous reviewed head: 8cfdd6715c6a69c419d9819636e185bfff74bb24.
- Package delta NOT CLEAN: 82fc2df8aa8c052c94f5819c5ec30b2b184aa337.
- Request: msg_plugin-w_1791708757_8533ef.

### Conflict and source checks

The first import, 3a71241d53c2613a978490d2df4cabea55b27487, merges v1 b7c97b42 into 8cfdd671.
It resolves conflicts in core-real-pending.txt and real.rs.
The second import merges v1 554d43e8 into that result without conflict.
Sol read the complete retained check: ~/botster-sessions/gates/botster-core-pc-base-merge-check-8cfdd671-4d05d270-20261011.log.
Its ancestry checks pass. The automatic merge, disjoint-path, own-diff, and real-pending-file conditions fail.
The failed check requires source delta reviews. It gives no automatic review waiver or gate evidence.

Sol read the combined conflict diff, complete own real.rs diff against the new base, and pending-file changes.
The real harness now declares both program::CONTROLS and route::CONTROLS for its tapped form.
The plain form still declares neither set. The two control-name sets are disjoint.
Program controls retain their harness dispatch. Route controls retain their route dispatch.
The tests retain the base's route-control assertions and the program-control assertions.
The removed assertion that pty_output is unsupported conflicts with this PR's supported program control.
The replacement still rejects pty_fail_after. The plain harness also retains its pty_output rejection proof.

The merged open path retains both the program wake registration and the base's shared Heard record.
The base's route attachment, payload identity, and post-hello child checks remain intact.
The union supplies no alternate worker, descriptor receipt, route, or program implementation.
The program module is byte-identical to 8cfdd671, including the corrected closed-peer proof.
Its SHA-256 is 502b0def622ce62ab80a29140c0e601eb61ea70924daa89387bcf1975122aaa6.
The process-check allowance and testkit manifest are also byte-identical to that reviewed head.
Sol's independent merge-tree for the second import equals the actual tree, 7f4d11bd6626d737407f38ab635a71d0ef788e16.
No additional source finding exists in these resolutions.

Sol independently compares pending IDs from the original merge base, old head, current base, and new head.
The old PR removes 16 IDs. The base removes three separate IDs. Neither side adds an ID.
The new pending file equals the original set minus both removal sets.
Against the current base it removes exactly the same 16 PR IDs and no other ID.
Every non-ID line equals the current base's line, including the count-free group comments from #244.
EV-2 and IN-5 remain pending. Their removal still requires PASS in the final gate on this new contracts pin.
The six steward-controlled IDs remain pending. The R-59 limit on the unfenced AD-7 minimum remains in force.

The current base includes the .29 contracts pin, #249's payload fix, #248's verification cache, and #251's exclusion check.
Sol already reviewed those changes. This delta changes no own deadline, mutation exclusion, transcript expectation, or runtime control logic.
The preceding focused Mac proof remains valid for the unchanged program module and closed-peer test.
The preceding Prior art and source findings retain their stated scope. SI-1 through SI-4 stay closed.
Sol read the complete package delta section. The package reviewer reaches the same source result and keeps SI-5 open.

### Remaining acceptance condition

SI-5 remains OPEN until the completed final Linux gate supplies the corrected mutation result on this exact head and base.
The old 8cfdd671 gate stopped before mutants. The focused closed-peer proof does not replace the full mutation step.
The final gate also supplies the required after-pin proof for any later EV-2 or IN-5 removal.
PC reports that the final gate is running. Sol has not read or polled that running log.
A running gate supplies no result and permits no pending-ID removal or merge.
Any removal commit requires the corresponding source delta review and exact-head gate under the binding rule.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


### Base-only import after Round 3

VERDICT: NOT CLEAN (1 open: SI-5) at 6dd3caef6ac68eb77bd872e54c9c5ff7ebe003fb.

The gate base is now 8dab35d14f7038d3cc0598e2cde8da207bd1b723.
Request: msg_plugin-w_1791709587_1e48ef.
The new head only merges that v1 tip into the reviewed 4d05d270 head.
The base adds #252's reviewed xtask/src/ci.rs change, outside this PR's five own paths.
Sol checked the source identities. The #252 v1 merge has the reviewed afc33981 tree.

Sol read the complete retained check: ~/botster-sessions/gates/botster-core-pc-base-merge-check-4d05d270-6dd3caef-20261011.log.
Both ancestry checks and all four numbered conditions pass.
Both pending files retain their required contents. EV-2 and IN-5 remain pending.
Sol's independent merge-tree equals the imported tree, 0ef621860a5d371196012ab2c731716e392b2ed2.
Sol independently compared both complete own diffs, including the real-pending removals. They are byte-identical.
The check's separate comparison outside the line-set files is 48483 bytes with SHA-256 74399e1ff9c3d0bdb8ae1bb3fc1606758efa09e3d7666243aae1790547aa38c3.
The unchanged source assessment carries through this verified base-only import. No new delta round is required.

PC reports that it stopped the 4d05d270 gate because its base became stale.
PC reports that the final gate now runs on 6dd3caef against 8dab35d1.
Sol has no completed result from that gate and has not read or polled its running log.
SI-5 stays open until that exact-head gate supplies the corrected mutation result.
Any EV-2 or IN-5 removal still needs its real PASS record, commit review, and required exact-head gate.
This import check supplies no full-gate result and grants no merge approval.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

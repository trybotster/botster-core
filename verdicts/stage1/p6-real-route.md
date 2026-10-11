# PR #238 — Real route client

## Round 1 — 2026-10-10

Inspected head: `4c675cbc0fcd86cf4dffccc825f7a96a27ee0513`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
READY receipt: `msg_plugin-w_1791694680_42b774`.
Tier: HIGH, shared testkit, BUILD.md rule 3 at `56bd0a5`.
Authority: approved plan 23ag and R-47 A at pinned contracts `b161db87`.
Scope: all six own paths, route and descriptor interfaces, eight source merges, three removed real IDs, and supplied gate evidence.
The own delta has 488 insertions and 19 deletions. The source diff check passes.
The package reviewer reports two source findings in `msg_plugin-w_1791694757_0e70ae`.
I read the complete package Round 1 NOT CLEAN artifact `fbb0f37e3321ecac0c25aaf8e81913fa926fb345`.
Its file is `verdicts/p6-real-route.md`. Receipt: `msg_plugin-w_1791694909_1e9834`.

### SI-1 LOW — OPEN: Prior art omits the binding old Core comparison

The published body names current TestkitRoute and maintained standard-library and rustix APIs.
It does not compare old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
`pair-common.md` requires that old-code comparison, with reuse or rejection and reasons, on every PR.
Old plugin_process/launch.rs uses UnixStream::pair and drops the child's endpoints after spawn.
Old worker_process.rs has a test-only socket session constructor for observing EOF.
I read those paths through pinned Git objects. They are candidate prior art, not accepted code to copy.
Required change: document the relevant old mechanisms and explain reuse or rejection for this real route.
A body-only correction needs no new source head.

### SI-2 LOW — OPEN: the body states the removed descriptor-send stub as current behavior

The body says RealEdges::link_send_descriptor always returns Failed and every other real route ends HandoffFailed.
At the reviewed head, `botster-core/src/real.rs:586-617` calls fd_pass::send_with_fd and returns accepted byte counts.
The accepted #234 import supplies that host-side send.
The body later acknowledges #234, but it retains the conflicting current-tense claim.
Required change: mark the older gate failures and typed stub as historical evidence.
State the remaining worker-side PR B boundary separately and accurately.
This finding requires no production change.

### SI-3 / RR-R1-2 LOW — OPEN: a new proof asserts the route's private queue

Location: `real/route.rs:395`, in `a_client_close_ends_the_stream_for_both_ends`.
The test reads `route.unsent.is_empty()` after a late write.
BUILD.md Testing Shape rule 3 forbids tests of helper internals.
The same proof already checks the worker endpoint's EOF and the route's later EOF.
Required change: remove the private queue assertion and retain those observable endpoint results.
I find this assertion independently. The package reviewer reports the same finding as RR-R1-2.

### SI-4 / RR-R1-1 HIGH — OPEN: the write flush can prevent a read from reaching its deadline

Locations: `real/route.rs:62-84` and `:161-166`.
RouteClient::read calls flush before it computes deadline.remaining() and before the marked poll.
`after_write` returns true on Interrupted without removing any unsent bytes.
It also returns true after Ok(0), which removes no bytes.
`flush_into` repeats while those bytes remain, without a deadline or a limit on attempts without progress.
Repeated Interrupted or zero-byte results can therefore prevent the read from reaching its caller's step deadline.
The nonblocking socket and bounded poll do not bound this earlier loop.
The RouteClient interface requires a subject that can block to give up at the deadline and return Empty.
BUILD.md also prohibits busy loops in tests.

Required change: stop the current flush when a write makes no progress, and preserve the pending bytes.
Positive partial writes may continue because each removes bytes from a finite offered buffer.
Prove the complete flush behavior through an injected Write that returns Interrupted or Ok(0).
Retain ordered later delivery and the existing partial-write proof.
Testing after_write alone does not prove that the enclosing loop returns.
The package reviewer finds RR-R1-1 first. I independently confirm its cross-package deadline effect.

### Other interface and source checks

Both real harnesses create one UnixStream pair and give the worker endpoint to CoreApi::attach.
The client route owns the other endpoint. The harness retains no duplicate of the worker endpoint.
A synchronous refusal drops the returned AttachRefused transport and the client route, so both owned endpoints close.
No production machine, public production API, dependency, contract pin, mutation exclusion, or native configuration changes.
The tap and plain compositions retain their accepted production paths.

The client endpoint becomes nonblocking before the route is returned and remains nonblocking.
After the first Empty read, later reads may wait on rustix poll under the supplied step deadline.
With no deadline or an expired deadline, the route does not wait.
The conformance driver's first idle route read permits one Core pump for the descriptor handoff.
The route's handed flag records that read opportunity; the harness does not observe or prove the actual handoff through this flag.
After poll, one nonblocking read maps bytes, transient Empty, or EOF through the existing RouteRead interface.
The real route reports no end cause. It derives no terminal value and parses no terminal bytes.
The route gate stops client reads. It uses the kernel's default buffers without adding an acceptance count.
Only client_close and route_gate are advertised. route_accept and route_fill remain Unsupported in this PR.
The pending stall and losslessness obligations remain pending under R-47.

The new socket proofs run in the slow tier. They use private socket pairs without child processes.
The peer reads use the shared Bounded adapter. Socket endpoints close through ownership on every exit path.
The marked poll waits for readability, EOF, or a socket error. It does not repeatedly probe readiness.
The existing real-process guards retain their ownership and cleanup behavior.
The public proofs cover ordered bytes, EOF, gate release, control availability, and the first Empty read before waiting.
The injected Write proof covers positive partial writes and WouldBlock. It lacks the no-progress loop cases in SI-4.

### Source merges and list accounting

I compare all eight source merges through Git objects. Six match their automatic trees without conflicts.
Their commits are `681b7303`, `457b1174`, `a220f2b7`, `9df0d0fb`, `c99309e0`, and `d3607488`.
Merge `846f5693` resolves only an import conflict in real.rs.
It retains this PR's removal of no_route and #222's LoseReason import.
Merge `4c675cbc` resolves only the real-pending list conflict.
It retains #227's own additions, removes this PR's three IDs, and changes the route-group count from 29 to 26.
Its tree is `d0f73601c82879f99df0b48981639582a0972be4`.
I read each complete resolution difference. Accepted #222, #234, #228, and #227 imports remain intact.
No executable merge-check job runs.

Only three IDs leave core-real-pending, which changes from 83 to 80:

- `conf::a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input`;
- `conf::ev_8_attach_requires_a_positive_query_deadline`;
- `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`.

I read all three complete pinned transcripts. Each has an exact-head real-tier PASS.
They prove attach refusals and event-capacity behavior. They do not claim worker route-frame delivery or stall controls.
They do not change the minimum count and do not overlap #239's three pending removals.
Core-pending remains 465, with 395 transcripts and 70 IDs without transcripts.
The other conformance files equal the current base.

### Supplied exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-4c675cbc-pool-20261010-211130-72458.log`.
The header names the exact head and current base. I read the result and conformance records without running a job.
All ten steps pass in 2710.2 seconds. The job and wrapper exit zero after 2763 seconds on msa1.
The job reports 43 queued seconds and 2720 running seconds.
Default: 1585 passed, 485 skipped, in 10.708 seconds.
Slow: 423 passed, 1974 skipped, in 94.230 seconds. All twelve new route proofs pass.
Both testkit reports show 205 passed and zero failed. Strict pending runs 374/395, with 21 real-only IDs and two held IDs.
Testkit minimum is 54/69. Real minimum is 35/68; real-accepted is 35/69.
Real-pending is 80, with zero passing IDs. All three removed IDs also pass in the slow mutation baseline.

Stage 1 reports 34 distinct misses, zero caught, zero unviable, and zero timeouts, in 120.2 seconds.
The changed real route compiles only with slow, so stage 2 must decide those misses.
The unmutated stage-2 baseline passes 423 tests in 108.853 seconds; the baseline job takes 120.5 seconds.
Stage 2 reports 34 mutants: 29 caught, five unviable, zero missed, and zero timeouts, in 2281.7 seconds.
I read all 68 individual outcomes. The complete stage-2 name multiset equals the complete stage-1 miss multiset.
Neither stage repeats a mutant identity. The final result is 29 caught and five unviable, with zero misses or timeouts.
The mutation CI step takes 2525.1 seconds. The gate has no failed, timed-out, or signal-ended test result.
The green gate does not cover SI-4's enclosing no-progress loop.
No new native-only source requires an additional Mac proof within this PR's own scope.

Four findings remain open at this reviewed source and initial body: one HIGH and three LOW.
I send them to P6 and its package reviewer.
I run no build, test, gate, mutation, measurement, reversal, or executable merge-check job.

VERDICT: NOT CLEAN (4 open)

## Correction source inspection — 2026-10-10 — Replacement evidence pending

Inspected correction: `96116a19251b3b86912fe6a88748fd5260c79fd6`.
Parent: `4c675cbc0fcd86cf4dffccc825f7a96a27ee0513`.
Receipt: `msg_plugin-w_1791694868_35ed9e`.
I read the complete one-file delta: 54 insertions and 15 deletions. The diff check passes.
I verify the published PR body and full correction head through GitHub.

SI-4 / RR-R1-1 closes its source path.
Ok(0), WouldBlock, and Interrupted now stop the current flush and preserve its bytes.
Other write errors clear the bytes and stop. Each positive write removes at least one byte from the finite offered buffer.
Thus each pass makes progress or returns. The read can reach its deadline check and marked poll.
The extended controlled-writer proof covers Interrupted, partial writes, WouldBlock, zero-byte writes, and later ordered delivery.
The fixture counts writer calls as observable injected I/O effects.
The zero-byte fixture can return Ok(0) repeatedly if the loop regresses. That regression could make the proof wait for the gate timeout.
Both reviewers advise P6 to reject an extra fixture call so the failure occurs promptly. Replacement mutation evidence remains required.

SI-3 / RR-R1-2 closes. The private queue assertion is removed.
The retained proof checks peer EOF, the late write, and subsequent route EOF.

SI-1 / RR-R1-3 closes through the published old Core comparison.
The body cites the pinned socket-pair constructor, timeout handshake reads, poll wait, interrupted snapshot loop, and in-memory pressure adapter.
It explains retained forms, rejected behavior, and reuse of rustix and the shared Bounded reader.
I read the cited old socket, timeout, interrupted-read, and poll source through Git objects.
I verify the old terminal adapter's WouldBlock pressure sites through source search.
These are conceptual forms; the correction copies no old code or tests.

SI-2 / RR-R1-4 closes. The body now names #234's implemented send_with_fd path and the remaining worker-side PR B obligation.
It marks the prior Failed and HandoffFailed records as historical evidence.

All four reported findings close in source or documentation at this correction.
The retained gate belongs to the earlier head and does not establish acceptance of this correction.
Replacement READY, exact-head gate, and package Round 2 artifact remain pending.
This source inspection completes no replacement round and grants no CLEAN verdict.
I run no reviewer execution job.

## Controlled-writer correction inspection — 2026-10-10

Inspected head: `eec41d21bd7752363ff264dd26ead4905ea0ae64`.
Parent: `96116a19251b3b86912fe6a88748fd5260c79fd6`.
Receipt: `msg_plugin-w_1791694955_ee4fa2`. Package agreement: `msg_plugin-w_1791694979_45fa5a`.
I read the complete one-file delta: 13 insertions and one deletion. Its diff check passes.
The controlled writer now asserts a maximum call count before it returns a scripted result.
The Interrupted and Ok(0) cases each permit one call. A second call fails immediately instead of repeating without progress.
The positive partial-write case permits two accepted writes followed by WouldBlock, then one final write after room becomes available.
The proof retains pending-byte and ordered-delivery assertions at the injected Write interface.
This closes the fixture concern recorded above. It changes no runtime source, list, pin, or exclusion.
P6 replaces the queued 96116a19 gate before that gate starts.
Replacement READY, the retained eec41d21 gate, and package Round 2 remain pending.
All four reported source and documentation corrections remain accepted. No replacement round is complete.
I run no reviewer execution job.

## Base merge source inspection — 2026-10-10

Inspected head: `89eeb2db28793674ef4c2739d17b790ffddf5151`.
Parents: `eec41d21bd7752363ff264dd26ead4905ea0ae64` and v1 `08ee8f2c6e343ff47966f12fcb80fe7cd35d0a83`.
Receipt: `msg_plugin-w_1791695434_aa3a60`.
The new base merges accepted #230 at `cc834158bfde2e87105bbd2c9620f51815d0c6aa` into the previous v1 base.
I compare the merge through Git objects. It has no conflict and matches automatic tree `9a8b41a43ddfa6c92c2542c3795fc70a33f5e797`.
The complete parent delta consists of the twelve accepted #230 paths. The route source equals inspected eec41d21 exactly.
The own delta against the new base remains six paths: 539 insertions and 19 deletions. Its diff check passes.
The merge preserves #230's allocator controls, exception check, test binaries, and conformance accounting.
All four reported #238 corrections remain accepted in source or documentation.
P6 stops the eec41d21 gate during the slow step and starts a full gate for the new merged head.
The stopped gate supplies no acceptance verdict for the replacement head.
Replacement READY, the retained 89eeb2db gate, and the exact-head package Round 2 artifact remain pending.
This inspection completes no replacement round and grants no CLEAN verdict. I run no reviewer execution job.

## Round 2 — 2026-10-11: corrections and the #239 import

Exact head: `fd627c55cb888fde6225d9a8a964ce65183ee8a8`.
Base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
Parents: `89eeb2db28793674ef4c2739d17b790ffddf5151` and the base.
Tree: `5779c52dab8cc0d3f51ea3e3b0dc0f0309abb41e`.
READY: `msg_plugin-w_1791698420_dc6fff`.
Package Round 2 CLEAN: `dce16a9ba158aa5b1cea2be00209345945a63d47`, `verdicts/p6-real-route.md`.
I read that complete package round on the same head.
Authority: plan 23ai at `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`, contracts .27, and BUILD.md at `56bd0a5`.
The published PR still states HIGH. GitHub names this exact head and base at the final check.

### Finding closure

**SI-4 / RR-R1-1 HIGH — CLOSED.**
The corrections at `96116a19251b3b86912fe6a88748fd5260c79fd6` and `eec41d21bd7752363ff264dd26ead4905ea0ae64` remain intact.
`after_write` keeps pending bytes and stops on `Ok(0)`, `WouldBlock`, or `Interrupted`.
A positive write removes bytes from a finite buffer. Another error clears that buffer and stops.
Each flush iteration therefore makes progress or returns. The read can reach its deadline check and bounded poll.
The controlled writer limits its call count. A no-progress regression fails instead of blocking the proof.
The full proof covers interruptions, zero-byte writes, partial writes, pressure, and later ordered delivery.

**SI-3 / RR-R1-2 LOW — CLOSED.**
The private `route.unsent` assertion is removed. Peer EOF, the late write, and route EOF remain.
The controlled writer and pure decisions expose their results through their interfaces.

**SI-1 / RR-R1-3 LOW — CLOSED.**
The published Prior art note compares old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It covers stream pairs, timeout reads, descriptor polls, interrupted retries, and scripted `WouldBlock` pressure.
It states the reused forms and the reasons for the rejected forms.
It names rustix, the shared Bounded reader, and the current flag parser.
I checked the cited old source through Git objects. The correction copies no old source or tests.

**SI-2 / RR-R1-4 LOW — CLOSED.**
The description states #234's implemented descriptor send and the remaining worker-side PR B work.
It labels the older Failed and HandoffFailed evidence as historical.
It assigns the 80-ID report to the gated head and the 77-ID list to this final head.

### Base merge and package boundary

The lead's pool rule requires one pre-review gate, source review of base imports, and a final gate after both CLEAN verdicts.
I read `~/botster-sessions/gates/botster-core-stage1-p6-real-route-fd627c55-pool-20261010-225925-73587.log` completely.
It exits 1 after nine seconds. Both ancestry checks and the conflict check pass.
The shared-path check fails on `real.rs` and `core-real-pending.txt`. The diff check fails on changed index lines.
The Core pending check passes. This failed check requires the following source review.

I compared the merge through Git objects. Its tree equals the automatic merge tree named above.
The own delta remains six paths, with 539 insertions and 19 deletions.
All 558 added and removed source lines equal the pre-import line multiset.
The route source equals the gated correction. The final diff check passes.

The merge preserves #239's start controls, registry control, and RefusalLayer.
The layer's attach method returns a refusal with the original transport or forwards that transport to its inner CoreApi.
The harness passes one socket endpoint to that CoreApi. It keeps no duplicate endpoint.
A refusal drops the returned transport and the local client endpoint.
Both the plain and tapped harnesses keep the same route implementation.

The two PRs remove disjoint ID sets. #239's three removals remain absent.
This PR removes only `a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input`, `ev_8_attach_requires_a_positive_query_deadline`, and `ev_5_repeated_attach_detach_without_polling_stays_bounded`.
The final real-pending list has 77 IDs; the base has 80.
The own delta changes no Core pending ID, transcript, pin, dependency, native setting, or mutation exclusion.
The merge preserves the accepted process ownership and route admission rules.

### Retained Linux evidence

Gate head: `89eeb2db28793674ef4c2739d17b790ffddf5151`.
Gate base: `08ee8f2c6e343ff47966f12fcb80fe7cd35d0a83`.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-89eeb2db-pool-20261010-221028-80389.log`.
I read its header, reports, summary, and all 30 outcomes in each mutation stage.
All ten stages pass in 2661.2 seconds.
Default: 1596 passed, 484 skipped, in 18.599 seconds.
Slow: 423 passed, 1984 skipped, in 137.861 seconds.
Testkit: 206 passed; minimum 55 / 69. Strict pending runs 373 / 394 transcripts, with 21 real-only IDs and two held IDs.
Real minimum: 35 / 68 passed and 36 / 69 accepted. Pending-real reports zero newly passing IDs among 80.
These counts belong to the gate head before #239. All twelve route tests and all three P6 removals pass there.

Stage 1 misses all 30 mutants because the route source compiles only with the slow feature. It takes 130.1 seconds.
The unmutated slow baseline passes in 134.5 seconds.
Stage 2 catches 25 mutants and finds five unviable. It misses none and times out none, in 2138.0 seconds.
The stage-2 identity multiset equals the 30 stage-1 misses, with no duplicate identity.
The five unviable replacements require unavailable default values.
The mutation step takes 2405.2 seconds. The job exits zero after 2842 seconds: 171 queued and 2671 running.
The wrapper exits zero after 2843 seconds.

### Focused Mac evidence

I read `~/botster-sessions/gates/botster-core-stage1-p6-real-route-fd627c55-pool-20261010-230450-11929.log` completely.
Its exact head and base match this round. It sets `TMPDIR=/tmp`.
Prebuild passes. All twelve route tests and each of the three removed IDs pass.
The job exits zero after 52 seconds. The wrapper exits zero after 53 seconds.

I also read the failure in `~/botster-sessions/gates/botster-core-stage1-p6-real-route-fd627c55-pool-20261010-230059-80924.log`.
Its route tests pass. With the default Mac temporary directory, the first ID fails at Core start because the socket path exceeds SUN_LEN.
The root comes from the base's unchanged `tempfile::tempdir()` call.
P6 records that harness-root issue as its follow-up. The short-root run proves this PR's route behavior on Mac.

All four findings close, including LOW. The exact final head has no open integration finding.
The final Linux gate remains the implementer's step after both CLEAN verdicts, under the lead's pool rule.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

## Source inspection before Round 3 — 2026-10-11

Inspected head: `5921340d4f90d94ab7864b740820c0d5fb88ad03`.
Parent: `8474e449f19b848f0bfe6e6174316f027b815c4e`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Receipt: `msg_plugin-w_1791701707_7188c6`.
The lead assigns this correction in the shared handoff's approximately 02:50 entry.
Authority: approved plan 23aj, pin `~/botster-sessions/pins/stage1-plan.6dbe1c43.md`.
I verify SHA256 `25f26484bc439817c213a1766fc6392f774b276b0bd4796eaea3bb890a5e82e9`.
The code still pins contracts .27. The later .28 code pin move remains separate.

### Disjoint base import

The parent imports #241 into the reviewed `fd627c55` source without a conflict.
I read `~/botster-sessions/gates/botster-core-stage1-p6-real-route-8474e449-pool-20261010-231238-48492.log` completely.
Both ancestry checks and all four conditions pass. The two base paths are disjoint from this PR's six own paths.
My full-index own diffs match: 28219 bytes, SHA256 `2ab66880f1c71fdf665c191fe2e8f9fae6f1c9f45e8677839500c5d99845ab0b`.
The automatic source merge tree matches the actual parent tree, `ad190ac37e24f7d2e9e742e785bc732b0fb1d473`.
The import preserves the accepted runner selection and all four Round 2 corrections.

### Route-control declaration correction

The complete correction changes only `real.rs`: eight insertions and two deletions. Its diff check passes.
`run_seed` checks `requires.controls` before setup. CoreDriver delegates that declaration to the harness.
The previous harness declaration omitted `client_close` and `route_gate`, though RealRoute implements both.
The omission made the runner return UnsupportedControl before the route could receive a step.
`has_control` now includes `route::CONTROLS` inside the tapped condition.
The plain harness still declares no control. The tapped harness declares only the two implemented route controls in this added branch.
The existing public capability test checks both results for every entry in that route list.
The other positive and negative capability checks remain. RealRoute's control dispatch remains unchanged.
I read the complete delta, the pre-setup check, declaration delegation, route control dispatch, and affected test.
This closes the reported declaration defect in source. I find no new integration source defect.
Both OU-5 and OU-7 IDs remain real-pending. The correction changes no conformance list, pin, dependency, or mutation exclusion.
The added selection changes no behavior that the operating system decides. It requires no additional focused Mac run for its own delta.

### Retained parent gate and limits

Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-8474e449-pool-20261010-231252-49132.log`.
I read its header, reports, summary, and all thirty outcomes in each mutation stage.
Its head and base match the parent and base above. All ten stages pass in 2489.2 seconds.
Default: 1604 passed, 484 skipped, in 11.186 seconds. Slow: 427 passed, 1989 skipped, in 106.861 seconds.
Strict pending runs 373 / 394 transcripts, with 21 real-only IDs and two held IDs.
Minimum: testkit 55 / 69, real-passing 36 / 68, real-accepted 37 / 69.
Pending-real reports zero newly passing IDs among 77 pending IDs.
Stage 1 misses all thirty mutants in 104.2 seconds because this source requires the slow feature.
The unmutated slow baseline passes in 103.6 seconds.
Stage 2 catches 25 and finds five unviable, with zero missed or timeout, in 2097.5 seconds.
The stage-2 identity multiset equals all thirty stage-1 misses, with no duplicate identity.
The mutation step takes 2307.5 seconds. The job and wrapper exit zero after 2499 seconds, with no queue wait.

That gate covers the parent. It does not cover the new capability declaration.
The implementer reports a running replacement gate. Replacement READY and the exact-head package artifact remain pending.
This source inspection completes no delta round and grants no CLEAN verdict at `5921340d`.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 3 — 2026-10-11: route-control declaration

Exact head: `5921340d4f90d94ab7864b740820c0d5fb88ad03`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Parent: `8474e449f19b848f0bfe6e6174316f027b815c4e`.
Delta request: `msg_plugin-w_1791701707_7188c6`.
Package Round 3 CLEAN: `cf060b989e8446cb6bf574d4b495e1c2f9cdba55`, `verdicts/p6-real-route.md`.
I read that complete exact-head round after `msg_plugin-w_1791701883_d48f1b`.
Authority: approved plan 23aj at `6dbe1c43d55e31cba038cee1008098bc0ba41ef0` and BUILD.md at `56bd0a5`.
The HIGH classification remains correct for shared testkit capability selection.

The complete source inspection above covers the one-file correction and its package interfaces.
The runner checks required controls before setup. CoreDriver forwards that check to the harness.
The tapped harness now declares the two controls that RealRoute implements: client_close and route_gate.
The plain harness still declares none. The actual operations still run through the route's existing dispatch.
The public capability proof checks both modes and retains its other positive and negative cases.
The correction reuses the existing route control list. It adds no custom I/O or private-state assertion.

I read P3's relevant retained records from the combined local tree, `e967a5f5`, on Darwin 27.0.0.
OU-5 stops with UnsupportedControl(client_close). OU-7 tail-queued stops with UnsupportedControl(route_gate).
These are combined-tree records, not exact-head acceptance proofs. The pinned preflight source establishes the reported cause.
The correction closes that declaration defect in source. Both IDs remain real-pending.
The four Round 1 findings remain closed. No new integration finding remains.

The preceding #241 import passes both ancestry checks and all four base-check conditions.
Its source merge tree and identical own diff remain verified in the preceding inspection.
The retained parent gate passes all ten stages and catches 25 slow-stage mutants, with five unviable and zero missed or timeout.
Its reports and complete thirty-outcome identity multisets remain recorded above.
Those results cover the parent. They do not establish execution of the new capability assertions.
The earlier focused Mac route evidence retains its exact head and limits from Round 2.
This new declaration changes no operating-system behavior, so its own delta requires no additional focused Mac run.

The final diff check passes. GitHub and origin/v1 still name this reviewed head and base at the final check.
The source changes no transcript, pending list, contract pin, deadline, cleanup, route I/O, or mutation exclusion.
I read only the header of the replacement gate, `~/botster-sessions/gates/botster-core-stage1-p6-real-route-5921340d-pool-20261010-235507-54316.log`.
That header names this exact head and base. The completed result remains pending.
The lead's approximately 02:50 ruling requires this delta review and one replacement gate.
This CLEAN verdict completes the source review. It does not claim the replacement gate passed or authorize merge before that result.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

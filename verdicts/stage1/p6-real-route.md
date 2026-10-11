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

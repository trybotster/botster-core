# P6 real route review — PR #238

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/238

Exact head: `4c675cbc0fcd86cf4dffccc825f7a96a27ee0513`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
Risk tier: HIGH. This change affects the shared testkit crate and the real conformance tier.

### Review scope

The reviewer read the complete six-file delta, the route implementation, its tests, both harness modes, and the retained gate.
The reviewer also read the pinned contracts driver, Deadline, R-47, and BUILD.md rules for prior art and test shape.
The reviewer read the shared Bounded reader and its deadline handling.
The reviewer inspected the two reported merge conflicts through Git objects.
The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

### Open findings

#### RR-R1-1 — HIGH — A flush can retry without progress before the read checks its deadline

Location: `crates/botster-core-testkit/src/real/route.rs:62-84`, called by `write` and `read` at lines 156 and 162.

`after_write` returns true for `Interrupted`, but it removes no byte from `unsent`.
It also returns true for `Ok(0)`, which removes no byte.
`flush_into` then calls the same writer again while the same bytes remain.
The loop has no deadline check or progress bound.
Repeated results of either kind therefore keep this call in a busy loop.
`read` calls `flush` before it reads `deadline.remaining()` or reaches the bounded poll.
The poll deadline cannot bound this earlier loop.
This is a source counterexample; the reviewer did not execute a hanging case or claim the retained gate encountered one.

The current decision test asserts that an interrupted write continues.
The partial-write test covers successful progress and `WouldBlock`, but it does not cover this retry path.
The passing mutation run therefore does not close this finding.
The existing in-memory route has a similar retry shape; copying that shape does not bound real I/O.

Required change: Stop this flush when a write makes no progress, and preserve the pending bytes for a later call.
Positive partial writes can continue because each one removes bytes.
Add controlled-writer proofs for `Interrupted` and zero-byte writes through the complete flush operation.
The proofs must show termination and preservation of the pending bytes without an unbounded retry.
Keep the existing ordering and partial-write assertions.

#### RR-R1-2 — LOW — The close test asserts a private field

Location: `crates/botster-core-testkit/src/real/route.rs:395`.

`a_client_close_ends_the_stream_for_both_ends` asserts `route.unsent.is_empty()` after a late write.
This assertion observes a private field of the helper.
BUILD.md, Testing Shape rule 3, forbids tests of helper internals.
The user's review rule also requires public behavior assertions.

Required change: Remove this assertion or replace it with an observable behavior assertion.
Retain the existing peer EOF assertion, late write, and subsequent route EOF assertion.
Assertions on the outputs of the pure decision functions are not this finding.

#### RR-R1-3 — LOW — The prior-art note omits the required old Core comparison

Location: PR body, `Prior art`.
This matches integration finding SI-1.

The note names the current in-memory route, `UnixStream::pair`, rustix poll, and the shared Bounded reader.
It records reuse of the flag parser and shared reader.
It does not compare the old Core at the binding pin `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The pair rules and BUILD.md require that comparison and the reuse or rejection reason.
The reviewer confirmed that the old pin contains `crates/botster-core-test-support/src/route_observer.rs` and the old route protocol.

Required change: Add the old-code comparison at the pinned SHA.
State which relevant pieces were reused or rejected and why.
Record the reason for each custom piece beside the maintained library or existing helper that was considered.
Do not port old tests or check out an old tree.

#### RR-R1-4 — LOW — The body describes a historical send failure as current behavior

Location: PR body, `What stays blocked`.
This matches integration finding SI-2.

The body says `RealEdges::link_send_descriptor` always returns `DescriptorSendError::Failed`.
At this exact head, `crates/botster-core/src/real.rs:586-617` calls `fd_pass::send_with_fd` and handles success.
PR #234 added that path and is already in the base.
The worker driver still does not receive or bind route descriptors; its real route actions remain pending work.
The earlier EOF evidence remains useful, but it describes the earlier head.

Required change: Mark the earlier failure as historical evidence.
Describe the implemented Core send and the remaining worker receive and route transport work at this head.

### Reviewed behavior and limits

`attach_stream` creates one Unix stream pair.
Core receives the worker end through its public attach API.
The harness keeps only the client end and keeps no copy of the worker end.
An attach refusal drops the local client route and follows the attach API's error result.
Both plain and tapped harnesses use this same implementation.
This change adds no process spawn, group owner, waitpid call, or production reaper.
The existing process ownership remains unchanged.

The client stream stays nonblocking.
After the first empty read, a later read waits through one marked rustix poll using the step deadline's remaining time.
The pinned conformance driver's first idle route read causes one handoff pump.
The poll returns Empty on a timeout or interruption, then the driver applies its own step limit.
The source has no sleep and adds no read retry loop.
RR-R1-1 concerns the separate flush loop before this bounded read.

`client_close` shuts down both directions and drops the client end.
`route_gate` stops reads and uses the real kernel's backpressure, within R-47 A's accepted scope.
The harness does not set socket buffer sizes or implement `route_accept` or `route_fill` in this change.
Those controls and the worker's real route transport remain outside this PR.
The real route reports `Eof { ended: None }`; its documentation states that it has no cause-recording wrapper yet.

The final delta removes only three IDs from `core-real-pending.txt` and updates the route group's count from 29 to 26.
The gate passes all three IDs on the real tier:

- `conf::a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input`.
- `conf::ev_8_attach_requires_a_positive_query_deadline`.
- `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`.

No transcript, contract assertion, contract pin, or mutation exclusion changes in the final delta.
The final list conflict retains the new base IDs and removes these three IDs.
The earlier import conflict retains `LoseReason` and removes `no_route`.

### Retained evidence

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-route-4c675cbc-pool-20261010-211130-72458.log`.

The header records the full head and base above.
All ten jobs pass. Wrapper exit: 0 after 2763 seconds, with 43 seconds queued and 2720 seconds running.
Default tier: 1585 passed. Slow tier: 423 passed, including all twelve new route tests and the three removed pending IDs.
Conformance: 205 passed; strict tier ran 374 of 395 pending IDs, with 21 real-only IDs not run and 2 held.
Minimum counts: testkit-passing 54/69, real-passing 35/68, real-accepted 35/69.
Pending-real: 80; zero newly passing IDs remain on that list.

Stage 1 lists 34 mutants and misses all 34 because the route module compiles only with the slow feature.
Its duration is 120.2 seconds.
The unmutated slow baseline passes in 120.5 seconds.
Stage 2 tests those 34 mutants: 29 caught, 0 missed, 0 timeout, and 5 unviable, in 2281.7 seconds.
The log prints each completed stage-2 outcome.
The five unviable replacements require default values that the replaced result types do not provide.
There is no new exclusion in this PR.

The reviewer also read the retained earlier mutation evidence.
At `9df0d0fb`, the blocking-read mutant times out after 600 seconds; that gate is stopped with exit 130.
At `b036e1ed`, the gate fails with one stage-2 survivor: deletion of `!` in the flush condition.
Commit `b036e1ed` replaces the blocking read with the nonblocking stream and bounded poll.
Commit `02b8b9de` adds the controlled partial-write proof through `flush_into`.
The exact-head log catches both flush condition mutants.
These corrections close those earlier mutation defects, but they do not prove termination after repeated no-progress writes.

Open findings: 4. Integration independently confirms RR-R1-1 and the private-field finding.

VERDICT: NOT CLEAN

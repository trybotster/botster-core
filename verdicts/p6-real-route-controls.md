# P6 package review — #287: real route controls and holds

## Round 1 — 2026-10-11

Exact head: `722b415a5e7900fcbc5ea48c8244fcafe01ba4c3`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `43006ac98b01d4b08c0866e45f09488e19d1194b`.
PR: https://github.com/trybotster/botster-core/pull/287.
Risk: HIGH. The own delta changes ten testkit files, with 1509 additions and 80 deletions.
The current GitHub base has advanced. This round reviews the source delta from the base above, which is also the gate reference.
It accepts no later import, pending-list change, or gate.

I read the complete PR body through the authorized CLI fallback after the GitHub MCP response truncated it.
I read the own source delta, new modules, changed tests, relevant source contexts, and retained rulings.
Rulings: `~/botster-sessions/shared/core-stage1/evidence/p6-route-controls/rulings-23bf-23bg.md`.
SHA256: `ff842c450bdfe972f9d488412676c7b9c6efacee412a156b29414fcb0a5173c7`.
I compare the controls with the pinned Contracts .36 rows and the later explicit 23bf/23bg rulings.
The own delta changes no production source, transcript, timeout, pending list, mutation exclusion, or Contracts pin.
It adds the rustix net feature. Cargo.lock remains unchanged.

### RC-R1-1 — HIGH — new loops can retry without progress

Locations: `crates/botster-core-testkit/src/edge_tap.rs:398` and `crates/botster-core-testkit/src/real/route.rs:1051`.
Tap::sort_unnamed retries Interrupted with no state change, deadline, or attempt bound.
The new fill test helper also loops without a bound. Its Ok(0) and Interrupted paths make no progress.
A sequence of these results can hold the test or control call indefinitely.
These paths violate the P6 brief's no-busy-loop rule. A passing outer gate does not bound the operations themselves.
Stop the operation on no progress, or use a finite, derived bound.
Retain the hello bytes, link order, accepted byte count, and backpressure assertions.
Add an edge or writer fixture that fails immediately if the operation retries without progress.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745074_c9bf63`.

### RC-R1-2 — MEDIUM — new socket reads have no deadline

Locations: `crates/botster-core-testkit/src/edge_tap/tests.rs:1510` and `crates/botster-core-testkit/src/real/route.rs:1028`.
Both tests read a blocking UnixStream directly. Neither read has a deadline.
Each test expects the preceding operation to close the peer.
If that close regresses or a copy remains open, the read can wait indefinitely instead of failing at a derived bound.
The existing client-close tests use Bounded::to_eof with TestDeadline::cleanup for this reason.
Use a shared bounded read or a nonblocking attempt for these new reads.
Retain the first test's EOF assertion and the second test's EOF-or-reset assertion.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745074_c9bf63`.

### RC-R1-3 — LOW — assertions inspect private diagnostic state

Locations: `crates/botster-core-testkit/src/real/route.rs:974` and `crates/botster-core-testkit/src/real/route.rs:999`.
The new tests assert the Debug form of private Fill, Requests, and Holders values.
These assertions require internal type names and field layout. They prove no control clause or public behavior.
BUILD.md Testing rule 3 forbids tests of helper internals. The user's review instruction requires public behavior assertions.
Remove these diagnostic-layout assertions. Retain the public control-result assertions in both tests.
I sent this finding directly to P6 in receipt `msg_plugin-w_1791745181_35b033`.

### RC-R1-4 — MEDIUM — required Mac proof is absent

The 23bf ruling approves read + unread as route_stream_written only with a test on each OS.
The own delta also introduces the macOS lsof holder count and socket identity path.
The Linux gate does not compile the darwin module or establish the Mac socket facts.
P6 confirms that no Mac evidence has completed for this branch in receipt `msg_plugin-w_1791745049_9db0a0`.
P6 reports that a focused Mac job is queued while the Mac node is below its disk floor.
Supply completed focused Mac evidence for the new socket identity, holder count, written count, and holds.
The evidence must name the exact source head and tree. A later head needs a reviewed source delta.
The future un-pend evidence does not replace this source acceptance condition.
I sent this condition directly to P6 in receipt `msg_plugin-w_1791745181_35b033`.

### Source assessment outside the findings

The route retains only the client end. Core takes the worker end; the harness creates no duplicate of it.
The route reads both buffer sizes at the ruled points and obtains F from the applied attach limits.
route_fill sends the existing alphabet pattern through the same probe Output request and acknowledgement as pty_output.
The request acknowledgement does not wait for the payload's output write to complete.
Requests retains the existing identity check, condition-variable waits, and shared cleanup deadlines.
Programs still shuts down its sockets and joins its reader threads at drop.
The new Mac command uses run_to_completion with Deadline::cleanup. That helper owns and reaps its command group.
The own delta adds no production-child reap, raw child wait, sleep, group signal, or independent process guard.

The accept drain reads once per route read/write call or route_accept control, with a 64 KiB chunk cap.
It keeps drained bytes in order. Its allowance decreases only by bytes read.
gate(false) clears the allowance; gate(true) retains it. The public tests cover both cases.
drop_transport closes without shutdown or SO_LINGER. client_close retains shutdown(Both).
The new tests permit the OS EOF/reset result without requiring one path.
The written count adds client reads and FIONREAD. Both close controls preserve their pre-close count.
The Linux public test covers ordinary reads, accept drains, backpressure, and both close controls.
The Mac counterpart remains an open acceptance condition above.

The hand-over hold returns the endpoint with Blocked and suppresses write interest only on the held link.
Release restores the last interest, respects an overlapping Launch hold, and signals the wake.
The public edge tests cover endpoint return, other-link progress, quiet edges, duplicate hold refusal, and release.
The start hold reads the hello to name the instance and keeps the accepted link from Core.
Release restores the link with its bytes in order; an armed-only release disarms the hold.
The harness refuses a hold after Created, a duplicate hold, and a release with no hold.
The body states the worker clock condition per transcript. I read the five named pinned transcript contexts.
No pending removal or proof of the unreached AD-1/AD-7 gaps is accepted here.

The holder count reads socket identity before Core takes the end.
Linux counts matching descriptors through /proc; Mac uses the bounded lsof command and matches its socket address field.
The body states the limit for processes that this user cannot read.
The tap counts the bytes passed to close_route_stream by socket identity.
The body states the sans-IO argument and the limit: requested bytes are an upper bound; zero is exact.
The existing slow:fd-handoff proof remains the real byte proof. This change does not replace it.

The own delta retains the earlier public pty_input assertions and changes the served-control assertion for route_fill.
It adds assertions for public bytes, close results, control errors, descriptor counts, and edge calls.
The private Debug assertions above are the exception.
The fixed-buffer source check records the three sources that it must read and rejects the three named setter forms.
I do not treat this text check as a general syntax-aware ban on all possible buffer-setting forms.
The existing syntax-aware process check and its red-on-revert tests remain outside this delta.

### Prior art

I read the named old sources through Git at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old conformance wrapper uses the public engine commands and supplies no matching route control or start hold.
The old output-backpressure test uses one paced in-process write slot. This PR instead uses actual socket backpressure.
The old read_available helper measures a pipe with unsafe FIONREAD. This PR uses rustix on the Unix socket.
The old plugin worker enumerates only its own inherited descriptors for close. It supplies no cross-process socket count.
The body rejects direct code reuse for each item and records the different boundary or unsafe-code constraint.
I find no missing relevant reuse item in the named note.

### Retained Linux evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-722b415a-pool-20261011-112154-51848.log`.
SHA256: `88a781fdc8ff046d69d28090726485e402281c71ed879665c79fe72519554f5c`.
I read the exact-head/base headers, stage records, summaries, changed test outcomes, and pending-real results.
All ten stages pass in 1756.9 seconds. The job exits zero after 1764 seconds, with zero queued and 1764 running.
The wrapper exits zero after 1765 seconds. Default passes 1753 tests, with 474 skipped, in 13.134 seconds.
Slow passes 570 tests, with 2036 skipped, in 43.047 seconds.
The new Linux socket identity, holder count, written count, control, and hold tests pass.
The gate reports real minimum 49 / 68 passing and 50 / 69 accepted.
It reports 12 passing pending-real ids out of 36. This head removes none.

A static parser compares the complete raw mutation identities with the named catch records.
Stage 1 has 165 unique identities: 65 caught, 96 missed, four unviable, and zero timeout, in 348.6 seconds.
Stage 2's 96 unique raw identities exactly match stage 1's misses.
The slow baseline passes in 54.2 seconds. Stage 2 catches 76, reports 20 unviable, and misses none, in 1212.6 seconds.
Each stage's raw caught identities exactly match its unique named catch records.
The final total is 141 caught, 24 unviable, zero missed, and zero timeout. Unviable results provide no catch evidence.
The fuzz step reports no changed crate with a decoder harness.
I do not accept the body-only historical probe and flake claims as new independent evidence.
The body reports the earlier capture flake as a separate structural-fix obligation; this round does not close it.

Four findings remain open. P6 owns the source fixes and required Mac evidence.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: NOT CLEAN

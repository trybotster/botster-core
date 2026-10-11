# Integration review — #240, DP-2 descriptor receipt (PR B)

## Round 1 — 2026-10-10

Reviewed head: `950527a6ff470b7f893cbb1a73b9d0efb918cf3d`.
Base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
Initial READY: `msg_plugin-w_1791697719_82e02c`, at `d901242ded24e77d17b0647b221f878b6be8976e`.
Base update: `msg_plugin-w_1791697799_c6f504`.
Package Round 159 / PR round 1: NOT CLEAN, `ecf32e79863d5c2ee204cd39ccc876605b55b97d`.
The package record amendment `408637c5639f952a2c062ba433fc9e14733ba603` adds the bound case to F117.
I read the complete package round and that amendment.
The GitHub head later moved to `d6ffb4384a536e1a492b8d5df1633df87c9c927f`.
This round accepts no replacement source at that later head.

### Scope and authority

The HIGH tier is correct. This PR changes descriptor receipt, the shared worker input, testkit edges, and workspace configuration.
The final own delta has fifteen paths, 1182 insertions, and 231 deletions.
Plan 23ae and the lead's DP-2 rulings bind this review.
The worker uses `Input::LinkBytes { bytes, descriptors }`. Guardian-core's input is unchanged.
The descriptors belong to the unique AttachRoute whose first byte occurs in that read.
The read can begin with an earlier frame's tail or another complete frame.
The worker binds the descriptor when that frame completes, in frame order.
A later fault must close each affected unbound descriptor and end each affected bound route with HandoffFailed.
Each transport closes once. Each route-end report occurs at most once.
PR C retains the real route transport, its read allowance, and the PTY read budget.
This PR removes no conformance ID and changes no contract pin or mutation exclusion.

### SI-1 / F117 MEDIUM — OPEN: a decoder error leaves descriptors open

Location: `crates/botster-worker-core/src/worker.rs`, `on_link_bytes`, decoder error and later return paths.
`link_read_begins` records received descriptors before decoding.
An oversized header calls `close_link` and returns before `link_read_ends`.
Neither `close_link` nor `finish_close` releases the handoff records.
The driver handles `LinkClose` with `drop_link`, which does not send `Input::LinkClosed` or clear the descriptor table.
An unbound descriptor therefore remains open until adoption or worker exit.

The same read can contain a complete AttachRoute followed by an oversized header.
The worker binds that descriptor before it finds the error.
The decoder error then leaves the route open without HandoffFailed, RouteClose, or a route-end report.
This violates the lead's required handling of an error found after a bind.
I sent the bound-case extension directly to P3 and the package reviewer.
Receipts: `msg_plugin-w_1791698157_458531` and `msg_plugin-w_1791698157_50aa9d`.
The package amendment includes this scope.

Required correction: audit every return after `link_read_begins`, including a frame that changes the link state.
Close each affected unbound descriptor once. End each affected bound route once with HandoffFailed.
Add Worker input/action proofs for both oversized-header cases.
The proofs must not require later LinkClosed input, adoption, or worker exit to produce closure.
Observe the bound transport close and its one required route-end report.

### SI-2 / F118 MEDIUM — OPEN: the close-on-exec proof races hello acceptance

Location: `crates/botster-worker/tests/common/session.rs`, `hello_and_launch`.
The helper sends the host HELLO and immediately sends Launch plus AttachRoute with a descriptor.
No worker event proves that HELLO passed before the descriptor send.
The accepted Linux read form can put both sends in one recvmsg.
`link_read_begins` then sees AwaitHello and rejects the descriptor before it decodes the preceding valid HELLO.
The proof depends on worker scheduling between sends. Its passing logs do not remove this race.

Required correction: wait for a worker event that proves hello acceptance before sending the descriptor.
Keep the descriptor receipt before payload spawn.
Use an event boundary within a deadline. Do not add a sleep or polling loop.
Retain the payload-alive marker after the client observes end of file.

### SI-3 / F119 MEDIUM — OPEN: a fault can make the worker panic

Location: `crates/botster-worker-core/src/worker/handoff.rs`, `check_reads`.
The function snapshots read keys, then indexes the map for each key.
`descriptor_fault` clears all other records through `close_unbound`.
The next iteration can index a removed key and panic.

Concrete input: two descriptor-bearing reads split one non-AttachRoute frame.
The second read completes that frame and starts no later frame.
Both records remain unbound until `link_read_ends(false)`.
The first fault removes both entries. The next map index panics.
The required result is a link protocol error with both descriptors closed once.

Required correction: stop after the fault, or iterate without indexing removed records.
Add a Worker input/action proof for this input.
Observe LinkClose and exactly one CloseDescriptor per received descriptor.
Do not inspect the private handoff map.

### Cross-package conclusions

I read the complete handoff module, worker decoder changes, route binding, driver ownership, adoption fence, and framing proofs.
The driver uses the accepted `fd_pass::recv_with_fds` and keeps each OwnedFd under a fresh DescriptorId.
BindRoute transfers ownership. CloseDescriptor and RouteClose drop their owned streams.
The adoption fence returns descriptor IDs from discarded old-link reads. The driver closes those entries.
The descriptor-table proof records injected stream drops. It does not inspect a private production queue.
The testkit models both measured kernel forms at its receive edge.
General worker runs use the Mac form without changing the seeded draw stream.
The split-frame proofs retain the read that contains the frame's first byte.
The ordinary two-route, wrong-count, no-start, and second-start paths have action proofs.
The second-start proof checks one transport close and one HandoffFailed report after the first bind.
The source findings above cover fault inputs that those passing proofs omit.

The process-check allowance only follows the existing guarded accept function's rename.
The condition-2 fixture uses GuardedSession, OwnedChild, Bounded, Blocker, and Deadline.
The loop-thread spawn invariant remains intact. The receiver sets close-on-exec before it returns to that loop.
The earlier accepted CTRUNC defence remains separate. It does not excuse an ordinary descriptor leak.
The Prior art note identifies the vault close-on-exec rule, the accepted rustix receiver, and Core's custom frame-ownership rule.
The package reviewer verified the old Core comparison at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
No old implementation or old test is copied.

### Merge and supplied evidence

I read both supplied base-merge records. The fifteen own paths do not overlap the later twelve and six base paths.
I independently compared the three ordinary Git diffs at 4c3fd5a3, d901242d, and 950527a6 against their respective bases.
All three diffs are byte-identical: 107295 bytes, SHA256 `27b8b4e25a0b3650fb7f322c7e3a0117a6b5e7f27dfc05b99e395415b1795f72`.
The supplied records report 108255 bytes with their own equal hashes. My comparison above uses ordinary Git diff output.
Source-only merge-tree comparisons match the recorded trees for 82a7784e, d901242d, and 950527a6.
The final tree is `594c7edf9454790c71a1906378c3d9d3d7ac25d4`.
The #227 conflict resolution keeps its refusal bound, retains the descriptor actions, and adds RouteReadAllowance to the PR C stubs.
The changed route-test calls use the new descriptor argument.
The final own diff preserves the accepted #227 admission code and imports the #230 and #239 base changes.

I read the headers, conformance reports, step summaries, and exit records of both supplied gates.
Pre-review gate: `botster-core-stage1-p4a-real-handoff-recv-4c3fd5a3-pool-20261010-220401-68609.log`.
It records head 4c3fd5a3 and base cf3bcaa9. All ten steps pass.
Default: 1598 passed, 485 skipped, 12.956 seconds. Slow: 410 passed, 1981 skipped, 118.863 seconds.
Testkit: 205 passed, zero failed, minimum 54/69. Real: 35/68 passing and 35/69 accepted.
Mutation stage 1: 79 mutants, 70 caught, nine unviable, zero missed or timed out, 263.5 seconds.
Full CI takes 473.6 seconds. The job exits zero after 867 seconds, with 384 queued and 483 running.

Additional gate: `botster-core-stage1-p4a-real-handoff-recv-d901242d-pool-20261010-222142-3248.log`.
It records head d901242d and base 08ee8f2c. All ten steps pass.
Default: 1609 passed, 484 skipped, 14.939 seconds. Slow: 410 passed, 1991 skipped, 112.139 seconds.
Both testkit reports: 206 passed, zero failed, minimum 55/69.
Real: 35/68 passing and 36/69 accepted. Real-pending: 83, with zero passing in that list.
I read all 79 mutation outcomes: 70 caught, nine unviable, zero missed or timed out.
Stage 1 takes 199.3 seconds. Stage 2 skips because stage 1 misses none.
The condition-2 proof passes in both test targets at 5.014 seconds each.
Full CI takes 422.4 seconds. The job exits zero after 1624 seconds, with 1190 queued and 434 running.
Neither supplied gate measures the later #239 base import.
The lead's pool rule requires one final gate after both CLEAN reviews. It does not require a gate for each disjoint base merge.

The Mac log names d901242d and Darwin 27.0.0 after worker prebuild.
It records 59 worker tests, 17 slow_cli tests, and 35 slow_session tests passing.
The retained reversals record six failed framing reversals and the failed Mac close-on-exec reversal.
The manual mutation record also shows failed loop, receive-boundary, and descriptor-table mutations.
These records are focused evidence. They are not complete gates at the final head.

I ran no build, test, gate, measurement, mutation, reversal, or executable merge-check job.
I used only source reads, static parsing, and source-only Git comparisons.
All three findings remain open. A replacement READY and the package verdict are required for closure.

VERDICT: NOT CLEAN (3 open) at 950527a6ff470b7f893cbb1a73b9d0efb918cf3d

## Round 2 — 2026-10-11: cleanup correction and adoption finding

Head: `f3d6e37066f027666392e65e945599371d428d66`.
Base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
READY: `msg_plugin-w_1791698600_621d29`.
Package NOT CLEAN: `787845a50622c24a81b4e94c26db755204d6532d`, `verdicts/p3-worker.md`, Round 160.
I read that complete round and the five-path correction: 225 insertions and twenty deletions.
The other own paths retain Round 1's review. The HIGH classification and approved PR B scope remain in force.

**SI-1 / F117 MEDIUM — CLOSED for the reported paths.**
`close_unchecked` takes every remaining read before it closes any entry.
It emits CloseDescriptor for an unbound descriptor and ends a bound route with HandoffFailed.
Checked reads have already left the table, so their routes remain.
`close_link` calls this cleanup before it changes state. Decoder errors and frame-triggered closes therefore release the reads.
`close_link_now` covers Terminate and orphan exit. LinkClosed and adoption also call the cleanup.
I checked each return after `link_read_begins`, including decoder errors and the post-frame return.
The new proofs cover oversized frames in both forms, bound routes, Remove, Terminate, and checked-route retention after LinkClosed.
They observe actions and decoded reports without a private table assertion.

**SI-2 / F118 MEDIUM — CLOSED.**
The helper sends a pre-launch Signal request after HELLO and waits for its matching Done error before the descriptor send.
The worker processes that request only after HELLO passes. No payload group exists, so it signals no process.
The response supplies the event boundary before a descriptor read can arrive.
Launch and AttachRoute remain in one descriptor send, so receipt still precedes payload spawn.
The existing bounded report wait supplies the deadline. Source ordering closes the race; repeated passes do not establish closure.

**SI-3 / F119 MEDIUM — CLOSED.**
`check_reads` returns after `descriptor_fault` takes the remaining table and closes the link.
The next iteration cannot index a removed key.
The split non-AttachRoute proof supplies two descriptor reads and observes each close once, LinkClose, and no bind.

**SI-4 / F120 MEDIUM — OPEN at this head: adoption sends a report before HELLO.**
The old Ready link can hold an unchecked bound read after AttachRoute plus a partial following frame.
`adopt` emits AdoptLink, then calls `close_unchecked` before HELLO and the counter reset.
`end_route` reports RouteClosed while the link is Ready. The driver sends it on the adopted transport.
Core-host `driver.rs::deliver` rejects a first frame that is not HELLO, so valid adoption fails.
The worker counts those bytes before its reset, while the driver counts them on the new link.
I confirm the package finding in `msg_plugin-w_1791698672_61d67f` through both action paths and the host handshake.
The correction must preserve HELLO-first ordering, descriptor closure, and new-link byte accounting.
It needs a public adoption proof from that Ready state.

I read `round159-reverts-macos.log` and `mac-slow-f3d6e370.log` completely.
The four selected reversals fail four, five, one, and one proofs, respectively.
The exact-head Mac record reports worker 59, slow_cli 17, and slow_session 35 passed.
Its separate core-sys nextest run reports 115 passed. Ten repeated condition-2 passes add no distinct coverage.
The earlier cargo-test storage failures come from the process-wide RLIMIT_NOFILE test; the lead assigns that separate issue to P6.
No complete correction-head gate or conformance ID flip is claimed. The earlier gates retain their heads and limits.
I run no reviewer execution job.

VERDICT: NOT CLEAN (1 open)

## Round 3 — 2026-10-11: adoption correction and the #241 import

Exact head: `e493ff5f6d8622f64feb1805fe1def887f69adad`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Parents: corrected head `4b437b442a32d5b601afe4507fdb6d36d597cbcc` and the base.
Tree: `6908221c512046d74e314484b772886bd4659c10`.
READY: `msg_plugin-w_1791698812_e16e3e`; base import: `msg_plugin-w_1791699110_823ad2`.
Package correction CLEAN: `d07e100398b8c430f712c9a9b716c9b2d8d5509c`, Round 161.
Package exact-head carry-forward CLEAN: `80f9bed0555e21542a0e12d68f9d73e8362f4e91`, Round 162.
Both records are in `verdicts/p3-worker.md`. I read both complete rounds.
Authority: plan 23ai at `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`, contracts .27, and BUILD.md at `56bd0a5`.
The HIGH classification and the lead's final-gate rule remain in force.

**SI-4 / F120 MEDIUM — CLOSED.**
I read the complete three-path correction at `4b437b44`: 33 insertions and two deletions.
After AdoptLink, the worker resets the new-link counters and clears old output-report state.
It resets the decoder and fences old host requests. `input_fence` sends no report.
The worker sends HELLO and Adopted before it calls `close_unchecked`.
The bound route's HandoffFailed report follows those frames and counts against the new `queued_total`.
No later reset omits those bytes. The driver resets its written total when it installs the transport.
Cleanup still precedes processing bytes after the candidate HELLO.

The public proof starts with a Ready worker, a bound descriptor, and a partial following frame.
It requires one descriptor close through RouteClose and exactly HELLO, Adopted, then HandoffFailed after AdoptLink.
It checks the new epoch's HELLO proof and requires Stop from the new host to produce SIGTERM.
It asserts no private counter or table. Source ordering establishes the counter result.
F117's closure scope and checked-route retention remain intact. F118 and F119 remain closed.

I read `~/botster-sessions/shared/core-stage1/evidence/p3-handoff-recv/base-merge-check-4b437b44-e493ff5f.txt` completely.
Both ancestry checks and all four conditions pass. The two #241 paths are disjoint from the fifteen own paths.
The supplied diff pair has 120958 bytes and SHA256 `b9388bf20e4768d0010586d7b543e8aba89a21165f05069e9ba9e90a9d7c6268`.
My ordinary Git diff pair also matches: 119998 bytes, SHA256 `6d7762813106db42849e5910dbd0255f765837c33330c4d8e596ec0262a200ba`.
The different formats have different sizes; each pair is identical.
The automatic source merge tree equals the actual tree above and has no conflict. The final diff check passes.
GitHub names this exact head and base at the final check.
The import preserves #241's accepted runner selection and all reviewed PR B content.
No conformance ID, pin, dependency, or mutation exclusion changes in the own delta.

I read `f120-revert-macos.log` completely. The faulty order fails its first-frame assertion: kind 17 instead of HELLO kind 1.
One selected proof fails, with 174 filtered out. This reversal is not a complete gate.
I read `mac-slow-4b437b44.log` completely. It names that corrected head and Darwin 27.0.0 after worker prebuild.
Worker slow passes 59 tests in 5.31 seconds. slow_cli passes 17 in 0.19 seconds; slow_session passes 35 in 5.22 seconds.
The earlier individual passes add no distinct coverage. The disjoint #241 import changes no operating-system behavior in this delta.
The earlier Linux gates and full outcome records retain their Round 1 heads and limits.
P3's worker-core 175, clippy zero, and strict 206/0 remain implementer-reported results.
No complete correction-head gate is supplied or claimed.
The final gate must cover the corrections and their mutants after both CLEAN reviews, under the lead's pool rule.
PR C still owns route I/O, read allowances, and the PTY budget. This PR moves no conformance ID.
The lc_12 record still lacks real attach_stream support and supplies no acceptance proof for that ID.
All four findings close. No integration finding remains.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

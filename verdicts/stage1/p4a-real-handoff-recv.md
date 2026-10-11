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

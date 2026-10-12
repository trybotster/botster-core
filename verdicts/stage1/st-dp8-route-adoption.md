# Core #313: integration review

## Round 1: source and initial exact-head evidence

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791818404_e2c385`; train 3c review precedes this review under the later lead order.
- Exact head: `bf8af71915cbe34688489f487ea68cb988383c12`.
- Tree: `f95ea61b36a5c7cdad557726711e9d615b483926`.
- Reviewed #298 input: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
- Imported v1 and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Risk: HIGH; link, host, worker, and shared testkit changes.

Sol-B read design revision 5, the source changes and affected callers, substantive changed tests, both design-document deltas, and the corrected body.
Sol-B read R-98, R-99, and corrected R-104 at contracts `1b2b6f5f317214f0b3ae498a14a75c50cd125031`.
The review includes the worker key conversion, both drivers, host adoption phases, stall integration, and event order under queue pressure.
The imported #301 start controls and #304 stream proof retain their earlier review scope.

### Source integration

The host carries attach's client identity through PendingHandoff and AttachRoute.
The worker keeps that identity and both optional tags in each route.
The worker's RouteKey reaches transport actions, driver callbacks, handoff bookkeeping, and admitted input.
This key stays unchanged while writes and input remain in flight.
The per-link tables alone translate host RouteIds to worker keys and translate route reports back.
Progress uses the same table through send_route_progress. The real driver and testkit use the same worker key type.

Adoption clears old mappings and retained items. Unchecked old-link reads close before the live-route report.
The worker sends its live payload report and one RouteLive per live route, in key order.
Until RouteId arrives, a reported route retains at most one close and one client-input flag.
It reports current stall state and progress after the mapping. A retained input report uses the current client revision.
The crossed-ID proof retains a partial output write and admitted input while a new route receives the old host ID.
The monotonic-revision and second-adoption proofs exercise the separate mapping interval.

The worker separates drain_close from closing. A route draining after session exit stays live under R-99.
Closed routes are excluded. Input and output sites retain their earlier suppression after session end through accepts_input.
Write failure, peer close, and stall timeout retain the first close reason through close_reason.
The tests cover an ended drain before and after mapping and an already Closed route.

Each RouteLive has its own control frame. The serde size proof compares maximal route-record integers with the smallest matching attach form.
Absent and present tags use the same shared-field encoding. The record is smaller than its attach frame.
The frame-fit check remains defensive. The adopting host's different frame bound remains the lead's separate R4 item.
The PR adds no arbitrary limit to cover that separate item.

The host keeps route records until the row state posts. It then allocates new IDs, registers clocks, sends mappings, and parks adoption events.
AdoptAll completion waits for all required adoption events. Link loss ends the wait for records that cannot arrive.
The stalled clock adds its close deadline at the adoption event post and removes it when a later resume is consumed.
The late-poll and pre-post-resume tests exercise the existing stall guard.

The three direct close paths retain the required adoption event: session Finish, removal CloseRoutes, and local Detach.
The scheduler offers no event-producing flow step or local Detach ahead of an unposted adoption event.
Eventless RowWrite, SendStop, and SendRemove steps remain runnable.
The production-driver proofs observe adoption before close and completion after adoption.
Separate Stop and StopAll proofs observe one stop request with no poll while the mandatory slot stays full.
The scheduler filters unavailable work rather than repeatedly executing a blocked step. The pump event budget remains unchanged.

R-98 keeps these required fields in unreleased protocol 1. Missing required fields fail decoding; no older released protocol is fabricated.
R-104 permits absent tags to map to empty strings under the current frozen event type.
The source comment cites that ruling. A later pin must change the event tags to Option<String> under amendment 22.
This review accepts the interim mapping only. It accepts no new refusal of empty attach tags.

### Finding history and closure

ST313-R1 HIGH is CLOSED. The original direct session-end close could remove the entry before RouteAdopted posted.
The production-driver proof observes Running, RouteAdopted, adoption completion, Exited, then RouteClosed with delayed polls.
The r10 and r16 reversals reach the missing-adoption assertion. Removal and local-detach proofs preserve the same route order.

ST313-R2 HIGH is CLOSED. The first repair blocked every session flow and delayed stop effects under queue pressure.
The current condition blocks only event-producing flow steps. The Stop and StopAll proofs require the worker request without polling.
The r15 reversal restores the broad condition and fails all three stop proofs.

ST313-Q1 is CLOSED through corrected R-104. The interim empty-tag mapping is permitted until the amendment pin.

B313-I1 LOW is CLOSED. The body initially said that no DP-8 field had a serde default and every missing field failed decoding.
The actual optional tags have serde defaults, and stalled is optional by meaning.
Sol-B requested a wording-only correction in `msg_plugin-w_1791819639_4c2b05`.
ST corrected the body on the unchanged head in `msg_plugin-w_1791819664_dbd84f`.
Sol-B independently verified that the body now names the seven required fields and distinguishes the three optional fields.
No new test or execution was requested.

### Lists and completed gate

Sol-B independently checked all nine conformance set formulas across the #298 input, its imported base, the DP-8 own delta, and v1.
Every final set matches. The PR removes exactly the three route-adoption IDs from pending.
The two epoch-fence IDs remain pending under the lead's separate control/proof item.
The sole own mutation configuration change moves the existing READ_CHUNK arithmetic exclusion from line 58 to line 57.
The reason and expression remain unchanged. No new exclusion covers adoption, mapping, event order, or the worker route machine.

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-bf8af719-pool-20261012-082406-50709.log`.
SHA256: `039c26290167a5499b208dc50206764fefa280063f5941fe683b0647200a9269`.
Sol-B read the exact-head/base header, lists accounting, changed-test results, raw mutation outcomes, citations, and final exits.
All ten steps PASS in 767.1 seconds. Default passes 2,005 tests; slow passes 620.
Both strict summaries pass 269 with zero failure. Held-full reports 192 expected ignored trials and zero failure for six budget IDs at 32 seeds.
Lists reports 398 pending, 49 real-pending, and seven held identifiers.
The three removed DP-8 IDs pass normally on both tiers.

Stage 1 tests 333 mutants: 306 caught, five missed, and 22 unviable.
The unmutated slow baseline passes all 620 tests.
Stage 2 tests exactly those five misses: three caught and two unviable.
Sol-B independently matches all 309 raw catches to citations and normal PASS results.
Final totals are 309 caught, 24 unviable, zero missed, and zero timeout. Unviable outcomes supply no catch evidence.
No FLAKY record appears. Both link decoder fuzz harnesses complete their configured runs without a reported crash.
Their completion records report 6,916,098 and 3,446,566 runs.
The job and wrapper exit 0 after 775 seconds on msa1, with one second queued and 774 seconds running.

### Platform and reversal evidence limits

Focused Mac log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-d1f41436-pool-20261011-233012-34556.log`.
SHA256: `18dc4e8e59d778066eab3ea32659eeb95c95ba0c6fce193b62622ec30ffa7982`.
The recorded command sets seeds 0–31 and strict pending mode, prebuilds the worker, then runs the five changed packages and selected real conformance.
It passes 1,114 package tests and nineteen real tests, including all three removed DP-8 IDs.
The chained commands and final wrapper exit establish success. The wrapper exits 0 after 131 seconds on mac.
This is focused evidence at d1f41436. Later changes affect host scheduling, tests, comments, and docs only.
The worker transport and platform paths remain unchanged. This review claims no current-head full Mac gate.

Sol-B read the retained reversal scripts, scratch changes, assertions, summaries, and restoration records in `gates/st-dp8-reversals/`.
The r1–r7 records name 6ff9dbb7 and a clean marker before their sequence.
They reach the drain-state, unchecked-read, retained-input, mapping-table, completion, delivered-close, and deadline assertions.
The r4 waiting-route test remains green; its other two tests fail. The review preserves that limitation.
The s1 and s2 records name d1f41436. Each target test fails on its own scratch change while eight others pass.
The r10/r11 records name 2b08608d; r15–r17 name 5015b012 and preserve the no-poll and close-order assertions.
The scripts state same-worktree target use and restoration. Sol-B did not independently execute cleanup or verify exclusive target ownership.
These retained records supply focused reversal evidence at their named heads, not a new current-head reversal run.

Earlier RED gates remain RED: 1a196230 fails taint on the moved exclusion; d1a5e238 leaves three mutation misses.
The line repair and survivor proofs are named in the body. The stopped 5c24322b gate supplies no completed result.
Local check claims retain their author-reported scope. Sol-B ran no local check.
Sol-B read the old adopt_reserved_inner at 72b2e335. It reconnects the control socket and probes live modes; it supplies no equivalent route-ID mapping.

### Package receipt and disposition

Sol-B read the package source and closure records at `cbdbbc651d4c5849a48bbf32dfa13a7ab830a426` and wording receipt `b4600b5f2af614dea3cf0db63fabd8267bc16dc9`.
The exact-head package CLEAN and closed findings agree with this review.
No source, body, contract, or supplied-evidence finding remains on bf8af719.

This CLEAN accepts this source head and initial completed gate only.
The PR's #298 base has since moved to a later head. This reviewed head does not claim that later import or train composition.
The lead controls the final merge slot, applicable base delta, exact-head final gate, and equal-tree/push verification.
The two fence proofs, optional-tag pin, and different adopting-host frame bound remain separate work.
Sol-B changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; initial exact-head source and evidence, later import/train pending)

# ST stall clock: design review

VERDICT: CLEAN (source delta; Core #298 at 7a8592fb)

## Core #298: train-9b and train-10 import, round 8

Reviewed head: `7a8592fb493450c8b046a076cf484317eb58d96e`.
Previous source CLEAN: `8f070c492c17d0be727b914f74edce626065b7f2`.
Imported v1: `644f632f194f62ad7fe14e66482ee01083706e30`.
Three-way base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
VERDICT: CLEAN for the source delta. No source finding remains open.
The final train gate remains due. This receipt does not claim a completed gate at this head.
The reviewer ran no local build or test.

The reviewer inspected the full remerge diff.
The only text resolution retains both `TestkitRoute.reports` and `TestkitRoute.frames`, including their constructor defaults.
The progress-report attachment method remains intact.
`route_accept` sets the progress baseline before the worker's stream can accept bytes.
The frame counter remains separate and serves `route_frames_taken` through its own control path.
The imported real-route frame counter also retains the existing progress hook.
The controls do not overwrite each other's state.
The R-93 prerequisite for a frame-consumption fence remains unchanged: no route_accept limit may be in force before that fence.
This source receipt does not approve a transcript that violates that prerequisite.

The reviewer independently checked every list in `conformance/` against both parents and the actual merge base.
All ten sets match the three-way union-minus-removals rule.
The core sets contain held 7, pending 379, real-only 26, real-pending 44, real-proofs 4 and deferred 2.
The ledger contains 692 ids, and the minimum list contains 69. No resolution loses or invents an id.

Local log: `~/botster-sessions/gates/st-298-r8/local-7a8592fb.log`.
It names this exact head and records the target clean at `2026-10-12T22:30:06Z`.
All 15 checks exit 0. The selected packages pass 1235/1235.
Strict default conformance passes 595/595 at seeds 0-31.

The reviewer verified the updated Round 8 body at this exact head.
The implementer and body report the lead's order: no separate member gate; the train-11 gate covers the merged member.
The lead's handoff records #298's move to train 11 after the route-client conflict and the standing rule against a gate per v1 merge.
The previous completed gate and its evidence limits remain recorded below.
The lead controls the train-11 gate, its applicable final review and any later base update.

## Core #298: import and modes repair, rounds 6 and 7

Reviewed head: `8f070c492c17d0be727b914f74edce626065b7f2`.
Previous source CLEAN: `5d5d4eefd5872864a972bc248f415f9cf6673ef3`.
Final imported v1 and gate base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
VERDICT: CLEAN. No source finding remains open. The reviewer ran no local build or test.

### Source and merge checks

The reviewer inspected the remerge diffs for `c1c6363d`, `ae9d4ced` and `8f070c49`.
The first merge retains the host stall section and imported control-queue section.
The second merge retains the worker's stall/history fields and imported size/report/activity fields in both route constructors.
It also retains both test modules and both groups of real-pending ids.
The final merge retains the three ST budget entries and takes the imported DP-5 budget description with its GS owner.

The modes repair at `393b64f0` adapts the imported coalescing code to the classed queue.
It inspects each entry's bytes, removes only an unstarted modes frame, and subtracts that frame's full queued length.
It queues the replacement as droppable and outside the exempt baseline sequence.
The started-frame rule preserves a frame with an outstanding write or accepted bytes.
The stall code drops the unstarted modes frame; the resumed baseline carries the current size and modes.
The worker DESIGN.md now states that rule.
The new proof checks a queued resize, a stall and a resumed baseline at 30 by 100.
The existing modes tests retain partial writes, outstanding writes and coalescing behavior.

The final imported exit-watch failure path ends the worker through `end_with`; it does not alter the route queue or stall clock.
The imported testkit changes use the hex form of the PTY input log. The reviewed route-report control remains present.
The host's imported control-room and frame-fit paths retain the reviewed due-step budget and scheduler exemption.

The reviewer independently checked all six conformance sets at each merge against its actual parents and merge base.
Every set matches the three-way union-minus-removals rule.
The final sets contain held 7, pending 379, real-only 26, real-pending 45, real-proofs 4 and deferred 2.
The explicit real-pending re-entry is a separate authorized change, not an id lost in a merge.

### R-109 and retained earlier failure

The reviewer read R-109 in contracts `docs/steward-rulings.md`.
It authorizes re-pending `ou_3_progressing_reader_lossless` on the real tier until the progressed fences are pinned and pass.
The reviewed re-entry line names R-109, its cause, contracts #99 and the removal trigger in the required comment form.
The head pins contracts-v0.1.46. This verdict does not claim the missing fenced transcript has been proved on that pin.

The earlier gate at `c1c6363d` remains RED, exit 1 after 437 seconds.
Its real slow run fails `ou_3_progressing_reader_lossless`, seed 0, step 48: Core receives RouteStalled while the transcript expects none.
R-109 identifies the missing progress-consumption fences after route_accept; the later pending disposition follows that ruling.
The earlier local timeout claims remain unclassified local evidence. This review does not turn those runs green.

### Completed evidence

Final gate: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-8f070c49-pool-20261012-142423-35272.log`.
The header names this exact head and final imported base. All ten steps pass.
The wrapper reports exit 0 after 824 seconds on msa1.
Mutation result: 248 tested, 232 caught, 0 missed, 0 timeout and 16 unviable. Three catches came from stage 2.
The normal default run passes 2148 tests, and the slow run passes 679 tests.
The gate retains its inherited KSTATE-1 known-flaky exclusion for `in_8_mode_checked_at_write_start_by_worker`; the stage-2 log explicitly records that skip.
The new modes test passes in the final gate.

Local Mac log: `~/botster-sessions/gates/st-298-r7/local-8f070c49.log`.
It names this head and records the target clean at `2026-10-12T21:14:37Z`.
All 15 checks exit 0. Selected packages pass 1227/1227, including the new modes test; strict default conformance passes 595/595 at seeds 0-31.
The new semantic change concerns queue classes and adds no platform operation. The earlier focused Mac limits remain recorded for the unchanged stall transport paths.

Reversal: `~/botster-sessions/gates/st-298-r6/reversal-modes-kept.log`, at repair head `393b64f0`.
The reviewer inspected its scratch diff and full failed assertion. Keeping the modes frame fails the new stall proof; the other eight selected tests pass.
The log reports an incremental build in the same worktree since its last clean. The reviewer did not run the reversal or independently verify cleanup.

The reviewer verified the updated PR body at this exact head. It retains the earlier RED gate and names both later rounds, the re-entry and completed final gate.
This is the matching package delta receipt. The lead controls the next train and any later base update.

## Core #298: train-2 merge delta, round 5

Reviewed head: `5d5d4eefd5872864a972bc248f415f9cf6673ef3`.
Previous source CLEAN: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Imported train-2 and gate base: `080987829a2e2245a7ea97606020400392c974d6`.
Three-way base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
VERDICT: CLEAN. No source finding remains open. The reviewer ran no local build or test.

The reviewer inspected merge `d1ff70d31b8f0fc71a2e5112c625a86740ee64bf` with `git show --remerge-diff`.
The host DESIGN.md takes the imported KillPayload rule at stop grace and keeps the carried due-step rule and stall section.
The driver test module list keeps both `seeded_trace` and `stall`.
The follow-up replaces the imported `Work::Silent` name with `Work::DueStep` in `Driver::held`.
`DueStep` includes both a due Silent event and a route stall step. Neither can wait for a scheduler hold under the deadline rules.
The exemption also applies when the driver checks runnable work and its wake state, preserving TM-6.
The imported scheduler filter still applies to other work. The merge does not change the reviewed host or worker stall source files.

The reviewer independently checked all six conformance id sets against the three-way rule.
The merged sets contain held 7, pending 400, real-only 26, real-pending 49, real-proofs 4 and deferred 2.
Every set equals the union of both parents minus base ids removed by either parent.
No id was lost or added by the resolutions.

Completed gate: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-5d5d4eef-pool-20261012-083713-16979.log`.
The header names this exact head and imported base. All ten steps pass; the wrapper reports exit 0 after 1238 seconds on msa1.
Mutation result: 244 tested, 228 caught, 0 missed, 0 timeout and 16 unviable. Three catches came from stage 2.
The held-full run covers six budget ids at 32 seeds, one trial per seed.
The carried-step conformance ids and repaired stall-post and resume driver tests pass.

Local log: `~/botster-sessions/gates/st-298-merge-local-5d5d4eef.log`.
It names this head and records the target clean at `2026-10-12T15:25:41Z`.
All 14 steps exit 0. The selected packages pass 1104 tests with one skipped; strict conformance passes 595/595 at seeds 0-31.
The earlier focused Mac evidence and its stated limitations remain recorded for the unchanged stall transport paths.
The merge-specific change is host scheduler logic; the imported train's other changes belong to its base review.

The reviewer verified the updated PR body at this exact head.
It names both resolutions, the semantic follow-up, the imported base and the completed gate.
This is the same-head package delta receipt. The lead controls the next train and any later base update.

## Core #313: completed source receipt

Reviewed head: `bf8af71915cbe34688489f487ea68cb988383c12`.
Declared PR base: #298 `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Imported v1 and gate comparison base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
VERDICT: CLEAN. ST313-R1, R2 and Q1 are closed. No finding or contract question remains open.
The source, proof and corrected R-104 records below supply each disposition.
The reviewer ran no local build or test.

Completed gate: `~/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-bf8af719-pool-20261012-082406-50709.log`.
The header names this exact head and comparison base. All ten steps passed.
The wrapper reports exit 0 after 775 seconds on msa1.
Mutation result: 333 tested, 309 caught, 0 missed, 0 timeout and 24 unviable. Three catches came from stage 2.
The held-full run covers six budget ids at 32 seeds, one trial per seed.
The stopped `5c24322b` gate supplies no completed result.

The earlier exact-head Mac evidence covers the unchanged worker transport and platform paths.
The later host scheduler fixes have engine and production-driver proofs, completed gates and retained reversal logs as recorded below.
All three removed DP-8 ids have full-seed evidence on both tiers.
The two epoch-fence ids remain pending under the lead's separate proof item.
The later pin PR must adapt the tag fields when it takes the optional-field amendment under R-104.

The reviewer read the updated PR body at this exact head.
It names the completed gate, imported base, R-104 disposition and stopped earlier gate.
It retains the earlier evidence and the two pending fence ids.
This is the package source receipt. The lead controls the merge slot and any later base update.

Body follow-up: the reviewer verified Sol-B's B313-I1 correction at unchanged head `bf8af719`.
The body now names the seven required DP-8 fields and distinguishes optional tags and the optional stalled mark.
It limits missing-field refusal to required fields and describes absent optional values correctly.
B313-I1 is closed. The package CLEAN remains unchanged. No new source change or execution was needed.

## Core #313: corrected R-104 citation

Reviewed head: `bf8af71915cbe34688489f487ea68cb988383c12`.
The reviewer read the corrected ruling at contracts `1b2b6f5f317214f0b3ae498a14a75c50cd125031`.
R-104 now assigns the optional tag fields to amendment 22. Amendment 21 concerns worker-link loss.
The interim disposition for #313 is unchanged: use the frozen fields and cite R-104 in the mapping comment.
The reviewer inspected the full delta from `5c24322b`.
The source comment and DESIGN.md now cite R-104 without the incorrect amendment number.
The two commits after `5015b012` change comments and documentation only.
ST313-R1, R2 and Q1 remain closed. No finding or contract question remains open.

The implementer stopped the `5c24322b` gate after the correction. It supplies no completed result.
The implementer announced a new gate at this exact head. The final receipt waits for its completion doorbell.
The reviewer has not read or polled the running gate and ran no local build or test.

## Core #313: R-104 disposition and comment-only follow-up

Reviewed head: `5c24322b4f49cb13d468ee3b2e17da02f6e9c101`.
Previous source-reviewed head: `5015b0120d7e4c9ad205e8fadf34fb5e6b97070a`.
The reviewer read R-104 at contracts `2fa8ce2da4d05533d41ef4daef2c2a14a804221b`.

ST313-Q1: CLOSED. R-104 requires optional tag fields under Core amendment A21.
It permits #313 to use the current frozen String fields and map absent tags to empty strings until the A21 pin PR.
The ruling requires a code comment citing R-104.
The reviewer inspected the full follow-up diff: the source comment above the tag mapping and the host DESIGN.md citation satisfy that requirement.
The diff changes no executable statement. ST313-R1 and R2 remain closed. No finding or contract question remains open.

The implementer announced a running full gate for this exact head.
The reviewer has not read or polled that running gate. The final receipt waits for its completion message.
The completed gate at `5015b012` remains recorded below. The reviewer ran no local build or test.

## Core #313: round 3 source review

Reviewed head: `5015b0120d7e4c9ad205e8fadf34fb5e6b97070a`.
Reviewed delta: `2b08608d27b2ac15daf7c0cc1e0283c663f5518b` to this head.
PR base remains `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Imported v1 and gate comparison base remain `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Risk tier: HIGH. The reviewer ran no local build or test.

ST313-R2: CLOSED. `ready_at` applies the session adoption gate only when `flow_needs_room` identifies an event-producing flow step.
The stop's row write and stop request remain runnable while the adoption event waits for a poll.
The local-detach gate remains in place.
The engine stop test and production-driver Stop and StopAll tests verify that the worker receives one stop request without polling.
The removal test verifies adoption before close for a draining route of an adopted Exited session.
The original session-end tests still pass. ST313-R1 remains closed.
The reviewer found no new source defect in the four-file delta.
No source finding remains open.

ST313-Q1 remains unanswered: the expected RouteAdopted value for absent attach tags needs a contract disposition.
The source still maps absent tags to empty strings, and the PR retains the open question.
This record does not assume that value is approved. The final CLEAN verdict waits for that answer.
No additional source change or execution is requested by this review.
The lead's unavailable inbox does not change the contract requirement; the lead can read this file.

### Completed evidence

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-5015b012-pool-20261012-001052-1684.log`.
The header names this exact head and comparison base `be95ef13`.
All ten steps pass. The wrapper reports exit 0 after 864 seconds on msa1.
Mutation result: 333 tested, 309 caught, 0 missed, 0 timeout and 24 unviable. Three catches came from stage 2.
The held-full run covers six budget ids at 32 seeds, one trial per seed.
The new Stop, StopAll and removal tests pass in this gate.

Local source checks: `~/botster-sessions/gates/st-dp8-local-4e585f76.log`, at the preceding fix commit.
Exact-head host checks: `~/botster-sessions/gates/st-dp8-local-5015b012-host.log`.
The exact-head log names this head and passes fmt, host clippy and 369/369 host tests.
The final commit adds driver tests and extracts their setup helper; the completed exact-head gate covers the resulting tree.
The earlier focused Mac evidence remains applicable to the unchanged worker transport and platform paths.

Reversal logs: `gates/st-dp8-reversals/r15-broad-session-gate-5015b012.log`, `r16-no-session-gate-5015b012.log`, and `r17-no-detach-gate-5015b012.log`.
All three name this exact head and the supplied target clean at `2026-10-12T06:30:06Z`.
The reviewer inspected the scratch changes, assertions and summaries.
R15 restores the broad gate and fails all three stop tests: 53/56 pass.
R16 removes the session gate and fails removal and both session-end tests: 53/56 pass.
R17 removes the local-detach gate and fails that test: 55/56 pass.
The reviewer did not run these reversals or independently check cleanup.

The PR body names the narrow gate, new proofs, exact-head gate and retained earlier evidence.
The final receipt remains pending only for ST313-Q1.

## Core #313: round 2

Reviewed head: `2b08608d27b2ac15daf7c0cc1e0283c663f5518b`.
Reviewed delta: `d1f414362a6be86eb25d84a168782ebeb7380b23` to this head.
PR base remains `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Imported v1 and gate comparison base remain `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Risk tier: HIGH. The reviewer ran no local build or test.

ST313-R1: CLOSED. The scheduler prevents direct session-flow closes and local detach from removing an unposted adopted route.
The new engine and production-driver tests retain the adoption event before the delivered session-end close.
The driver uses one mandatory slot and reads the end reports before polling.
The r10 reversal reproduces the original trace: Running, Exited, RouteClosed, Completed, with no RouteAdopted.
The r11 reversal verifies the local-detach gate. The completed gate passes all three new tests.

### ST313-R2 — HIGH — The adoption gate also blocks the effect of a new Stop

Evidence: `run.rs:243-247`, `adopt.rs:406`, and `flows.rs:390-456` at the reviewed head.

The new gate excludes every session flow while `AdoptedRoutes.unposted` is nonempty.
After the adoption's Running event posts, Core admits operations on that session.
A caller can begin Stop while the route adoption event still waits behind a full mandatory queue.
`request_stop` selects `StopPhase::RowWrite`, followed by `SendStop`.
The new scheduler gate excludes both steps until a poll permits the adoption event to post.
Core therefore sends neither the stop request nor its fallback signal, and it does not arm stop grace, while the queue stays full.
StopAll uses the same stop flow for this admitted session.

EV-5(c) requires the effects of Stop and StopAll to continue under mandatory queue pressure.
The existing `SendStop` comment states this requirement explicitly.
The original finding concerned route-removal steps; it did not require blocking eventless lifecycle effects.

Required change: restrict the adoption gate to steps that can remove an unposted adopted route.
Keep the stop's row write, stop request, fallback signal and grace setup runnable under mandatory queue pressure.
Preserve the repaired ordering for `StopPhase::Finish`, `RemovePhase::CloseRoutes` and local detach.
Add a production-driver proof that begins Stop after adopted Running but before RouteAdopted, then pumps without polling.
Verify that the stop effect occurs while the adoption event still waits and that later events retain the required ordering.
Cover StopAll's use of the same path. Do not change the event budget or create runnable work that makes no progress.

### Completed evidence and pending disposition

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-2b08608d-pool-20261011-235045-22227.log`.
The header names this head and comparison base `be95ef13`. All ten steps pass; the wrapper reports exit 0 after 859 seconds on msa1.
Mutation result: 332 tested, 308 caught, 0 missed, 0 timeout and 24 unviable. Three catches came from stage 2.

Local log: `~/botster-sessions/gates/st-dp8-local-2b08608d.log`.
The selected packages pass 1117/1117; strict default conformance passes 595/595 at seeds 0-31.
The strict real DP-8 run passes the three removed ids at seeds 0-31.
The host-only delta changes no worker transport or platform code; the earlier focused Mac evidence remains applicable to those unchanged paths.

Reversal logs: `gates/st-dp8-reversals/r10-no-session-gate-2b08608d.log` and `r11-no-detach-gate-2b08608d.log`.
Both name this exact head and the supplied target clean at `2026-10-12T06:30:06Z`.
The reviewer inspected their scratch changes, assertions and summaries. R10 fails two of 52 tests; r11 fails one of 52.
The reviewer did not execute the reversals or independently check cleanup.

ST313-Q1 remains pending. The reviewer will not assume the absent-tag contract disposition while the lead's inbox is unavailable.
The lead can read this verdict file. The reviewer will send the important receipt again when the lead restores inbox access.
This is the second NOT CLEAN implementation round for #313.

## Core #313: DP-8 route adoption, round 1

Reviewed head: `d1f414362a6be86eb25d84a168782ebeb7380b23`.
Declared PR base: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6` (#298).
Imported v1 and gate comparison base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Approved design SHA-256: `5bd281fd4c3a2c71265e60b63f764043e53251bfbf208e10eb00dda048e90825`.
Risk tier: HIGH. The reviewer ran no local build or test.
The #298 source verdict and DP-8 design verdict remain CLEAN at their recorded artifacts.

### ST313-R1 — HIGH — A session-end flow can remove a route before its adoption event posts

Evidence at the reviewed head: `inbound.rs:347`, `inbound.rs:412`, `run.rs:245`, `flows.rs:502`, and `run.rs:724`.

The new `route_ended_with_session` parks a delivered close only when the adopted session already shows Exited without a stop flow.
While `Flow::Stop` runs, it records delivery and leaves the close to `StopPhase::Finish`.
That flow calls `close_route` directly. `close_route` posts the close and removes the route entry.
The scheduler lists session work before parked route work.
If the route's adoption event still waits, `post_route_adopted` finds no entry and marks that event done without posting it.

A legal sequence is:

1. Core adopts a Running session with a live route. Its Running event fills the one mandatory slot.
2. Core registers the route and sends its mapping. `ParkedRoute::Adopted` waits for a poll.
3. The worker reports `Exited`, then `RouteClosed{SessionEnded}` after delivering the route's queue.
4. A poll frees room. Core consumes these frames and records delivery while the stop flow runs.
5. Session work posts Exited before the parked adoption event and fills the slot again.
6. Another poll frees room. `StopPhase::Finish` posts the route close before the parked adoption event and removes the entry.
7. The parked adoption work later skips `RouteAdopted` and allows adoption to complete.

DP-8 requires one `RouteAdopted` for the route that was live at adoption.
The approved design section 6.4 also requires the entry to remain until its close posts after its adoption event.
The current `PeerClosed` test uses `route_close`, which parks behind adoption. It does not cover the session-end flow.

Required change: preserve the adoption event and its ordering across session-end, loss and removal paths that close route entries.
Do not let a close remove a reported route before its required adoption event posts.
Do not count a silently skipped adoption event as a completed post.
Keep the pump event budget, mandatory-room behavior and wake rules.
Ensure that a flow blocked by this ordering does not repeatedly run ahead of the parked event.
Add a production-driver proof for the Running adoption followed by Exited and a delivered session-end close before the adoption post.
Use queue backpressure and delayed polls. Verify exactly one adoption event, then exactly one close with the new id.
Verify that adoption completes only after the required adoption event posts.

### Contract question ST313-Q1 — Absent attach tags

The PR maps absent `AttachOptions.route_tag` and `owner` to empty strings in `RouteAdopted`.
The PR explicitly says that no ruling covers this case. The event fields are String; the attach fields are optional.
The reviewer sent the question to the lead with the exact head. The contract disposition remains pending.
The reviewer does not invent the expected value for absent tags.

### Source and design checks

The stable worker key reaches the route machine, pending input, real driver and testkit edge.
The link table clears at adoption and controls translation in both directions.
The worker retains bounded closes and client-input flags while mappings wait.
Current client revisions preserve the approved monotonic rule across mappings.
The size test uses the pinned serde types with maximal integers and absent, short and long tags.
The worker separates the session drain intent from its Closed state under R-99.
The reported limits, stall mark and stable tags reach the host's new route entries.
The ordinary resume and peer-close paths preserve the parked adoption event and its deadline guard.
The source finding concerns the separate session-end flow above.

The PR removes three DP-8 ids from pending. All three pass on both tiers in the supplied full-seed evidence.
The two epoch-fence ids remain pending under the lead's stated follow-up: `lose_host` control and real proof.
The imported #301 and #304 changes belong to the supplied v1 base; the review does not claim them as new DP-8 behavior.

### Completed evidence

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-d1f41436-pool-20261011-231400-62724.log`.
Its header names this exact head and comparison base `be95ef13`.
All ten steps passed. The wrapper reports exit 0 after 940 seconds on msa1, including queue time.
Mutation result: 325 tested, 301 caught, 0 missed, 0 timeout and 24 unviable. Three catches came from stage 2.
The held-full run covers six budget ids at 32 seeds, one trial per seed.

Focused Mac log: `~/botster-sessions/gates/botster-core-stage1-st-dp8-route-adoption-d1f41436-pool-20261011-233012-34556.log`.
Its header names the same head and base. The command sets `BOTSTER_SEEDS=0-31 BOTSTER_PENDING_STRICT=1`.
The five selected packages passed 1114 tests. The selected real conformance run passed 19/19, including the three removed DP-8 ids.
The wrapper reports exit 0 after 131 seconds. This is focused Mac evidence, not a full Mac gate.

Local log: `~/botster-sessions/gates/st-dp8-local-d1f41436.log`.
The strict real DP-8 run passes all three removed ids at seeds 0-31.

Reversal logs: `~/botster-sessions/gates/st-dp8-reversals/`.
The reviewer inspected each scratch change, failed assertion, summary and supplied restoration record.
The r1-r7 logs name earlier head `6ff9dbb7ed7ac079a9395e27ac819245039e3e8e` and record one target clean before the sequence.
They cover drain state, unchecked reads, retained input, table clearing, completion, delivered closes and deadline arming.
The r4 waiting-route test passes on its reversal; the other two tests fail, as the PR correctly states.
The s1 and s2 logs name this exact head. Each new test fails on its own scratch change while the other eight tests pass.
The supplied script restores the edited files and records git status after each run.
These are retained run logs, unlike #298's copied excerpts. The reviewer did not execute the reversals or independently check the worktree cleanup.
The earlier red gates and their repairs remain named in the PR body.

## Core #298: completed merged-head receipt

Reviewed head: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Reviewed imported base and gate base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
VERDICT: CLEAN. No source finding remains open.
The merge delta record below supplies the resolution and three-way list checks.
The reviewer ran no local build or test.

Completed gate: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-4ca7bb2a-pool-20261011-221430-25804.log`.
The header names this exact head and imported base. All ten steps passed.
The gate wrapper reports exit 0 after 963 seconds on msa1.
Mutation result: 241 tested, 225 caught, 0 missed, 0 timeout and 16 unviable. Three catches came from stage 2.
The gate records six held budget ids at 32 seeds, one trial per seed.
The imported drain-room test and repaired stall-post and deferred-resume tests pass.
`in_9_legacy_keys_table` passes in the completed test-budget step at 1.004 seconds.
The local strict timeout remains in the history; the gate result does not turn that earlier run green.
The earlier Mac evidence and copied reversal provenance limits also remain recorded.
The implementer confirmed the deferred-id count correction: two ids, with the set check unchanged.

The PR still names the older round-3 gate and source base in its body at this review.
Before DONE, the implementer must add this merged head, imported base, completed gate and retained local evidence limits to the body.
This is a package receipt for the reviewed head. The lead controls the merge slot and any later base update.

Body follow-up: the reviewer read the announced update at unchanged head `4ca7bb2a`.
The body now names the merged head, imported base, conflict resolutions, correct deferred count and completed gate.
It retains the local strict timeout. The body update requirement is complete. The CLEAN receipt remains unchanged.

## Core #298: merge delta at 4ca7bb2a

Reviewed head: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Previous source CLEAN: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Imported v1: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Three-way base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Status: no source finding in the announced resolution delta. The final receipt waits for the completed exact-head gate.
The reviewer ran no local build or test.

The reviewer inspected merge `24b5d65c374d2775b80b6024287a5974f66ad524` with `git show --remerge-diff`.
Both text conflicts resolve as unions: all ST budget entries remain beside the imported DP-5 budget entry.
The host test module list keeps both `stall` and `stop_drain`.
The following commit adds only `mark: 0` and formatting to the imported drain test's `RouteStalled` construction.
That test registers no route. It tests mandatory-room blocking at the driver, so the mark does not arm a route clock.
The reviewer also inspected the imported driver drain changes where they meet the existing queue and budget paths.

The reviewer independently checked the three-way id sets, excluding comments and parsing the deferred TOML entries.
For every file, merged = (ours union theirs) minus ids removed from the base by either side.
Results: held 7, pending 403, real-only 26, real-pending 49, real-proofs 3, deferred 2; all passed.
The implementer's message said deferred 4. The file contains two `[[deferred]]` entries with two ids.
This count difference does not indicate a missing id or merge defect.

Local evidence: `~/botster-sessions/gates/st-298-merge-local-4ca7bb2a.log`.
Its header names this head and reports the target clean before the run.
The five selected packages passed 1086/1086, including the drain-room test and repaired stall-post tests.
The strict default-tier run used seeds 0-31 and passed 594/595.
It timed out at `conf::in_9_legacy_keys_table` after 2.019 seconds and exited 100.
The implementer reports an isolated pass at 1.758 seconds. This review retains the broad timeout and does not call that run green.
The reviewer has not read or polled the running pool gate. Its completion message remains due.

## DP-8 route adoption: design revision 5

VERDICT: CLEAN (design).

Follow-up artifact SHA-256: `5bd281fd4c3a2c71265e60b63f764043e53251bfbf208e10eb00dda048e90825`.
The reviewer inspected the updated wire definition and size argument after the implementer's message.
`RouteLive` now skips absent `route_tag` and `owner`, as the pinned `AttachOptions` does.
The note requires size tests with absent and present tags and makes the actual serde test decide the margin.
The design verdict remains CLEAN. The lead's approval before code remains required.

Artifact: `shared/core-stage1/handoffs/st-dp8-route-adoption-design.md`.
SHA-256: `0860f14a407f812cd38424c9bcb917977946f407c932078e9af69ab46142e7b3`.
Declared Core base: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Risk tier: HIGH. This is a design verdict, not an implementation verdict.
The reviewer ran no local build or test. The #298 source verdict remains CLEAN.

STDP8-D5: CLOSED. The worker retains an input flag per unmapped route instead of a historical revision.
Every sent client observation carries the worker's current client revision at send time.
The worker advances that revision, and the link preserves send order. The host therefore receives nondecreasing revisions.
Each retained observation still names its own route for client activity.
The planned two-route proof covers reversed input order and fresh input between mapping messages.
STDP8-D1 through D4 remain closed. No design finding remains open.

Section 4 now compares maximal `RouteLive` integers with a minimal attach message on the pinned contracts-v0.1.41 types.
The implementation must verify the size inequality from the actual serde types, with both absent and present tags.
Pinned `AttachOptions` omits absent `owner` and `route_tag`; the shown `RouteLive` fields do not specify that omission.
The test must account for that difference instead of assuming identical shared encoding for absent tags.
This check preserves the size inequality requirement; this verdict does not independently verify the stated 65-byte margin.
The release fallback cannot replace the encoding proof.

R-98 and R-99 remain applied. The completion barrier and adoption event phases remain accepted.
The smaller adopting-host frame bound remains the lead's separate ST item after DP-8.
The implementation still needs the specified driver proofs, full-seed conformance evidence, focused Mac evidence and full Linux gate.
The lead gives approval before code starts. This CLEAN does not replace that approval.

## DP-8 route adoption: design revision 4

Artifact: `shared/core-stage1/handoffs/st-dp8-route-adoption-design.md`.
Reviewed SHA-256: `f9ec544e6608afdefd0d773e73d254ece86c4ca5f2c779d53643d26898a2e542`.
The READY named hash `aa0e796d383405920dbace5d9a8a35f3b7346f044ed2af3e2fcc4248b4495d5e`.
The reviewed file also records the lead's later R4 disposition: a separate ST item after DP-8.
Declared Core base: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Risk tier: HIGH. The reviewer ran no local build or test. The #298 source verdict remains CLEAN.

### Closed design findings

- STDP8-D1: CLOSED. Stable `RouteKey` values replace worker-side handle ids in the machine, driver, input records and callbacks.
  Only the link boundary translates between a key and the current handle's id.
  The design covers pending writes, admitted input, crossed ids and another host loss before mapping.
- STDP8-D2: CLOSED. Section 5.5 now covers route-keyed client observations and reports from routes without a current link id.
  It defines retention per route and suppresses reports from routes excluded under R-99.
  The new retention rule has a separate revision-order defect, STDP8-D5 below.
- STDP8-D3: CLOSED at design level. Separate `RouteLive` and `RouteId` frames remove the aggregate frame dependency on route count.
  The implementation must prove the serialized size argument with maximal integer widths and minimal omitted attach fields.
  The stated release fallback does not replace that proof or establish normal contract behavior for a live route.
  The lead assigned the smaller adopting-host frame bound to a separate item after DP-8, as recorded in `core-lead.md`, line 1403.
- STDP8-D4: CLOSED. The design separates report consumption, state post, registration and mapping, route adoption post, and row completion.
  Core queues the adoption event before it sends the mapping that permits later route reports.
  A later parked resume or close suppresses the adoption snapshot's stale stalled deadline.
  The proposed driver tests cover one slot, one event budget, late polls and closes before the adoption post.

### STDP8-D5 — HIGH — Retained client revisions can move the host's revision backwards

Section 5.5 retains the latest `Observation::ClientInput` separately for each route.
The worker sends that retained observation when that route's `RouteId` arrives.
These mapping messages follow route registration order, not client input order.
At the base, `inbound.rs` assigns `t.input_rev.client = input_rev` for each consumed observation.

For example:

1. The adoption snapshot has client revision 10.
2. Route B receives input at revision 11 while it has no link id.
3. Route A receives input at revision 12 while it has no link id.
4. Core maps A before B. The worker sends A's retained revision 12, then B's retained revision 11.
5. Core's cached client revision moves from 12 to 11.

Fresh input on an already mapped route can also overtake a retained observation from an unmapped route.
This violates the advancing client revision of IN-10 and makes the ST-4 cache older after a newer report.

Required change: keep the host's cached client revision monotonic across retained and fresh observations.
Preserve each route's activity identity. Define the rule explicitly instead of assuming the mappings preserve input order.
Add a two-route proof with reversed retained revision order and fresh input between the two mapping messages.

### Ruling dispositions

R-98 remains applied. Protocol 1 has no compatibility promise to earlier development builds.
The R-99 root fix separates `drain_close` from the Closed transition and lists the affected route paths.
The implementation must supply the draining-after-Exited and already-Closed adoption proofs specified in R-99.
The completion barrier remains accepted. The lead must approve the revised design before code starts.

## DP-8 route adoption: design revision 3

Artifact: `shared/core-stage1/handoffs/st-dp8-route-adoption-design.md`.
SHA-256: `84c85f149fab45954a2b251cc0c8013bfdd17c455a32f65721e50b4907fc92a6`.
Declared Core base: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Risk tier: HIGH. This is a design review. The reviewer ran no local build or test.
The reviewer read revisions 1 and 2 before revision 3 arrived. This verdict applies only to revision 3.
The #298 source verdict remains CLEAN at the declared base.

### STDP8-D1 — HIGH — The rename must include the transport and pending input

Section 5.4 renames the worker's route table, but the real driver also indexes its streams by `RouteId`.
At the base, `botster-worker/src/route_io.rs` keeps `Routes.routes: BTreeMap<RouteId, RouteIo<S>>`.
Its writes, read allowances, closes and returned inputs use that key.
The input machine also retains route ids in admitted input records (`worker/input.rs`).
Renaming only the route machine leaves its transport under the old key.
A new-id write then finds no stream, and an old-id callback can name the wrong route after an id collision.

Required change: define the rename for every route-id reference in the machine, real driver and testkit edge.
Include pending writes, read allowances, poll tokens, admitted input and queued actions or callbacks.
Specify an atomic boundary that preserves the connected stream and bytes already in flight.
Add driver proofs with crossed ids and a pending partial write or input when the rename runs.
Include a second host loss before `RouteIds` arrives, so the next adoption can recover the remaining routes.

### STDP8-D2 — HIGH — The report hold omits route-keyed observations

Section 5.3 holds five named route reports and claims that no message on the new link names an old id.
However, `Observation::ClientInput` also contains `route: RouteId`.
At the base, `worker/route.rs::client_received` sends that observation through `report`.
Client input continues while the worker waits for `RouteIds`, so this path can send an old id on the new link.
Section 5.2 also excludes closing routes without defining how their later reports avoid old ids.

Required change: cover every route-keyed report, including client activity and reports from excluded routes.
Define whether the worker retains, suppresses or translates each report during the mapping window.
Keep the required event order and bounded memory. Do not replay pre-adoption history.
Add a proof that client input during the mapping window cannot name an old id on the new link.

### STDP8-D3 — HIGH — The aggregate route messages can exceed the link frame bound

Sections 4 and 5 place every live route in one `Adopted` frame and every mapping in one `RouteIds` frame.
The design gives no bound that proves either frame fits.
Each route adds tags and applied limits. Many individually valid attaches can exceed the bound in one aggregate report.
At the base, `Worker::send_frame` sends nothing when `encode_frame` rejects the payload against `frame_bound`.
An oversized adoption report can therefore leave a live worker unreachable to the adopting host.

Required change: define a bounded transfer that preserves every live route and the adoption completion barrier.
Account for both messages, the metadata size, and closes retained while the mapping waits.
Do not silently drop routes, enlarge existing limits, or add an unsupported route-count limit.
Add boundary proofs for a report near the link bound and a route set that needs more than one bounded transfer.

### STDP8-D4 — HIGH — Adoption needs a barrier for later route events and state changes

Section 6.1 treats consumption of `Adopted` as the step that posts `SessionState`.
At the base, `adopt_report` selects a flow; `post_adoption` posts the state in a later step with room.
The proposed immediate `RouteIds` lets the worker send resumes and closes while the state or adoption events still wait.
Parking an adoption event only when the queue lacks room does not define this complete ordering boundary.

A route can be reported stalled, resume after `RouteIds`, and have its resume consumed before its adoption event posts.
Section 6.3 would then arm the old stalled deadline at the adoption post, after Core already removed it for that resume.
This repeats the stale-deadline problem that #298's posting guard prevents.

Required change: define the state, route registration, mapping send and event-post phases separately.
Ensure that each adopted route's events follow its `SessionState` and `RouteAdopted`, including with one mandatory slot and one event budget.
Apply consumed resumes and closes to the pending adoption snapshot so its later post cannot restore an ended stall.
Keep the row pending until all required adoption events post, including a route that closes during this interval.
Add driver proofs for queue backpressure, budget exhaustion, a resume before the adoption post, and a close before that post.

### Contract questions and accepted parts

- R1: CLOSED. The reviewer read R-98 at contracts `4fd81f1cb4063659b709ba8aa72a662e4cc2937b`.
  Revision 3 correctly keeps protocol 1 and removes compatibility defaults for required DP-8 fields.
  Both DESIGN.md files must state that protocol 1 becomes fixed at the first release.
- R2: ANSWERED by R-99 at contracts `b11db6b5476f50ba1e0b4d2a962477a0faa99188`.
  The reviewer read the ruling after the lead sent it.
  A live route is `Open` or `Stalled`. A route that drains output before `SessionEnded` remains live and must be reported.
  A route past its `Closed` transition is not reported, even while it delivers its final close frame.
  The design must cite the file and line where `closing: Some` is set and identify its contract transition.
  The implementation must report a route that drains after `Exited` and send its later `RouteClosed{SessionEnded}` with the new id.
  It must also prove that an already `Closed` route is not reported.
  This ruling resolves the question; the next design must apply its conditions.
- R3: the proposed completion barrier is correct. `AdoptAll` must not complete before its required `RouteAdopted` events post.
  STDP8-D4 requires the design to define the intervening state and event phases.
- The old Core comparison and the choice of new handle-owned route ids satisfy the stated design intent.
  The five listed DP-8 ids and full-seed evidence plan remain required.
  The real driver rename adds socket-path scope, so the implementation needs focused Mac evidence before READY.

## Core #298: round 3

VERDICT: CLEAN

Reviewed head: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`.
Reviewed source base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Reviewed delta: round-2 head `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6` to the reviewed head.
Risk tier: HIGH. The reviewer ran no local build or test.

### Finding disposition

ST298-R5: CLOSED. `ParkedRoute::Stalled` retains the worker's mark.
Core arms the close deadline only after `post_mandatory` successfully posts `RouteStalled`.
The deadline uses that pump's clock and the full `stall_close_after` interval.
Core suppresses the deadline if a later event for that route already waits in the parked queue.
This prevents an earlier stall event from adding a deadline after Core consumes its resume or close.

The lead approved the rule "add at post, remove at consume". The steward recorded no objection.
The decision is in `shared/core-stage1/handoffs/core-lead.md`, line 1363.
This decision changes the round-2 request for the resume path.
Core removes the close deadline and releases its reservation when it consumes `RouteResumed`, even if the event must wait.
Otherwise, a late poll can send a close step to the resumed worker and hold the only slot without an answer.
The host DESIGN.md states the approved rule and this reason.

The driver tests cover an older parked close, a reserved last slot, and a late poll.
They verify that the stall event posts before the close deadline starts and that the complete close interval follows the post.
The resume tests cover slot takeover and a resume consumed before its close step.
The engine test covers a stall event with its resume parked behind it.
All four tests pass in the completed exact-head gate.
The reviewer found no new logic defect in the six-file delta.
The PR body names the correct gate comparison base and retains the general R-93 attribution for AM-2.
All earlier findings remain closed. No finding remains open.

### Completed evidence and limits

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-76b369c7-pool-20261011-205842-74210.log`.
All ten steps passed. The final status is exit 0 after 834 seconds on gaming.
Mutation result: 260 tested, 239 caught, 0 missed, 0 timeout and 21 unviable. Three catches came from stage 2.
The log header names the reviewed head and comparison base `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
The source base remains `20b2525c`; this CLEAN is not a final-slot merge receipt.
The stopped gate at `726f2547` provides no completed result.

The round-3 delta changes host state, tests and documentation. It changes no worker, process, PTY or socket code.
The round-2 focused Mac evidence remains applicable to those unchanged paths.
Its selected packages passed 1046/1046. Its broader real run passed 240/242 and exited 100.
The R-66 and AM-2 limitations recorded below remain part of this verdict.

Reversal record: `shared/core-stage1/handoffs/st-298-r3-red-on-revert-excerpts.md`.
SHA-256: `786db4b3addbaccfff85dcfd7a0fbd374e9392a945f18843a29cc653ff2371dd`.
ST copied these session excerpts after the runs. They are not original log files.
The record describes the scratch changes, filtered failures, restoration and use of the same worktree target.
The reviewer inspected this record but did not independently verify the original output or cleanup.
The reviewed source and completed gate independently support the repaired behavior.

## Core #298: round 2

Reviewed head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`.
Reviewed source base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Risk tier: HIGH. The reviewer ran no local build or test.

### Closed findings

- ST298-R1: CLOSED. The source implements the steward's same-link slot takeover rule at contracts `1798a65de4c11855906366ab4a233564d3ec637d`.
  It keeps the asked step after takeover and treats the later answer as an ordinary report.
  The new driver tests cover a queued resume, a parked answer followed by a poll, and several due routes.
  The host tests cover own-slot answers, other workers and frames that do not need mandatory room.
- ST298-R2: CLOSED. A failed resumed baseline now retains the worker's stalled state and original stall mark.
  The due close therefore ends the blocked route with its first close reason.
  Two worker tests cover blocked writes and an existing healthy close. The host test verifies the typed close and slot release.
- ST298-R3: CLOSED. Replacement reader steps, stalls and resumes clear the parked flag.
  The new progress test checks the future deadline while the queue remains full and its later parking behavior.
- ST298-R4: CLOSED. `report_output` now flushes route progress before sending Output.
  The new test checks pending progress before pending Output under link backpressure.
- ST298-E1: CLOSED. The exact-head Mac log supplies the changed checks and retains its broader failures, as recorded below.

### ST298-R5 — HIGH — The close deadline can start before the stall event posts

Evidence: `inbound.rs:312-315`, `inbound.rs:356-359`, `inbound.rs:775-784`, and `stall.rs:222-234` at the reviewed head.

`on_worker_msg` calls `route_stalled` before `route_event`.
`route_stalled` releases the reservation and arms `stall_close_after` from the current pump.
However, `route_event` parks the stall event behind any existing `ParkedRoute` record.
`run_parked` later posts that event without applying its clock transition at the post.

This sequence is possible with an earlier parked route close:

1. The mandatory queue is full, and an earlier route close waits in `self.parked`.
2. A poll frees the last mandatory slot.
3. The next pump runs a due stall step and reserves that slot before it runs the older parked work.
4. Core consumes the worker's `RouteStalled` answer using the reservation.
5. Core arms the close deadline and releases the slot. It appends the stall event after the older parked close.
6. The older close posts and fills the queue. The stall event remains unposted until another poll frees room.

`next_deadline()` already exposes the close deadline while the host has received no stall event.
If the next poll comes after that deadline, Core can send `RouteCloseDue` before it posts `RouteStalled`.
This violates R-90 Q1 condition 1 and section 3 of the approved design, which arms the close deadline when the stall event posts.
The resume path has the same separation between its clock change and deferred event.

Required change: apply each host-visible stall or resume clock transition atomically with its actual event post.
Preserve the worker's mark while a report waits. Use the posting pump's clock for `stall_close_after`.
Keep the existing event order, reservation bounds and event budget rules.
Add a driver proof with an older parked route event, the last slot reserved, and a delayed second poll.
Verify that no close deadline appears before `RouteStalled` posts and that the full close interval starts at that post.
Also cover a deferred resume so its host-visible transition follows the same rule.

### Completed evidence

The lead authorized one re-gate after the known R-88 `in_9` failure. The earlier attribution record remains below.
Re-gate: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-200112-36429.log`.
All ten steps passed. The final status is exit 0 on gaming.
Mutation result: 247 tested, 227 caught, 0 missed, 0 timeout and 20 unviable. Three catches came from stage 2.
The header names this head and gate base `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
The source base remains `20b2525c`; this is review evidence, not a final-slot merge receipt.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195909-27591.log`.
The five selected packages passed 1046/1046 tests. The repaired driver and worker tests passed there and in the Linux gate.
The real run explicitly used `BOTSTER_PENDING_STRICT=1 BOTSTER_SEEDS=0-31`.
It passed 240/242 tests and exited 100. This review does not call that broader run green.

- `ev_5_route_closed_retires_route_keyed_events`, seed 13, step 28, has the recorded R-66 pre-close activity fence failure.
  The expected activity precondition is true; the consumed close report shows false. The contracts #69 fix remains the recorded disposition.
- `am_2_one_admission_point_orders_all_sources`, seed 1, step 14, receives `424143` (BAC) instead of ABC.
  The supplied isolated head log reproduces that result.
  The isolated `20b2525c` base log reproduces the same ID, step and byte order at seed 5.
  The implementer reports a target clean before the base run. The base log shows fresh compilation from its separate worktree.
  #298 does not change the admission queue. The verdict retains this base failure as a limitation of the broader evidence.

The reviewer checked the completed logs and inspected the repaired source and tests.
All five first-round findings are closed. ST298-R5 is the one open finding for round 2.

## Core #298: re-gate attribution at round-2 head

Head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`.
This record addresses plan section 8, revision 23al, condition (2). It is not a round-2 source verdict or a CLEAN.

The reviewer confirms non-interference for `conf::in_9_modes_read_at_write_start` at seed 0, step 8, `step_timeout`.
The failed gate is `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195551-10795.log`.
PC's reproduction is `shared/core-stage1/evidence/pc-r78/in9-repro-f6025aed.log`.
Both logs contain the same ID, seed, step and timeout classification.

PC's reproduction starts from v1 `f6025aed4cdf7da836e835e0040fc128b7e8f32f` and applies a scratch timing change.
The change delays the focus-mode output by 300 ms. It changes no expected value and adds no missing product behavior.
This is a controlled reproduction under 23bl(A), not an unmodified v1 run.

The pinned .41 transcript attaches no route. It begins a focus write under `pty_blocked` and then waits for `ModesChanged`.
That wait can discard an earlier `Completed` event, so the later completion wait times out.
R-88 records this live transcript race and assigns the R-74 held-write fix to PC.
The reviewer read that ruling at contracts `37f93d9394db05ae715548fae5ee333b5f78fd40`.

The reviewer compared #298's head against its declared base.
The host input transaction, model encoder, real program control and PTY backend files do not change.
The new route-progress flush has no route to report in this transcript.
The changed code supplies neither a held-write precondition nor a fence for the completion event.
It can change timing, but it does not change the diagnosed missing precondition.

The reviewer therefore supplies the non-interference line required for the lead's one re-gate decision.
The lead must accept the controlled reproduction and authorize the re-gate in writing under 23al and 23bl(A).
The repeated gate must pass every step on this exact head. This record approves no additional retry or pending-list change.
The reviewer ran no local build or test. The five #298 findings await the round-2 READY and completed evidence.

## Core #298: first implementation review

Reviewed head: `0f9079c44e028644c6d1724c9dae9cd8b03d0459`.
Reviewed base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
PR: <https://github.com/trybotster/botster-core/pull/298>.
Design hash: `5f170fb9b5c6b685dab043358028779db6ca0246824c84c2cb6fa2fac5acd848`.
Risk tier: HIGH. The change affects deadlines, queue reservations, worker route transitions and shared testkit code.
The reviewer inspected the source and supplied evidence. The reviewer ran no local build or test.

### ST298-R1 — HIGH — A reservation can block the report that must release it

Evidence: `engine.rs:284-304`, `stall.rs:98-119`, `stall.rs:204-212`, and the driver's FIFO held-frame path.

Consider a stalled route with an empty event queue and one mandatory slot:

1. The worker accepts bytes and queues `RouteResumed` before Core reads its link.
2. Core pumps at the close deadline. The due step reserves the only slot before Core services the link.
3. `accepts_at` rejects `RouteResumed` because the reservation leaves no mandatory room.
4. The driver holds that frame and stops reading the link.
5. `route_resumed` cannot release the close reservation because Core never receives the frame.

The queue contains no event for a poll to remove. The worker declines the close because it already resumed.
The route remains blocked with no deadline and no runnable work. This violates OU-2, TM-6 and R-90's release condition.

The problem also includes an older mandatory frame before the due answer on the same link.
For example, `Exited` can precede `RouteStalled` while a healthy close waits for route delivery.
Reserving the last slot blocks `Exited`, so Core cannot reach the answer behind it.
Adding only a `RouteResumed` exception does not resolve that FIFO case.

Required change: make the reservation protocol compatible with every earlier frame on the answering link.
Preserve FIFO, mandatory bounds and the later pump's event budget. Route any contract-meaning change through the lead.
Add driver-level proofs for a queued resume before a due close and an older mandatory frame before a due answer.
Use an empty event queue with all available room taken by the reservation. Verify progress without an event poll freeing room.
The current engine-only resume test uses two slots and feeds the report directly. It cannot prove this driver boundary.

### ST298-R2 — HIGH — A failed resume leaves the host and worker with different stall states

Evidence: worker `route/stall.rs:149-157`, host `stall.rs:170-180`, and worker `on_route_close_due`.

When the resumed baseline exceeds a bound, the worker clears `stall.stalled` and starts a healthy `SnapshotTooLarge` close.
The worker sends no `RouteResumed`. The host therefore retains its old stalled state and close deadline.
If the transport accepts the first resumed byte but then blocks, the healthy close remains unwritten.
The later host close step reserves a slot and sends `RouteCloseDue` for the old stall.
The worker rejects that message because its stall state is already clear. It sends no answer.
The host releases a close reservation only on resume or close, so the reservation remains held.

An existing healthy close also takes this path. `close_healthy` returns immediately when `closing` already has the first reason.
The current oversized-baseline test drains all writes, which hides the blocked-close case.

Required change: define and implement the host report and deadline state after a resumed baseline cannot be formed.
Preserve OU-2's first reason and OU-9's typed close. Ensure a blocked healthy close retains a working clock and releases every due reservation.
Add a combined host/worker proof: one accepted byte, baseline failure, then blocked writes across the old close deadline.
Cover both a new `SnapshotTooLarge` close and an existing healthy close.

### ST298-R3 — MEDIUM — New progress retains the old parked flag

Evidence: host `stall.rs:103`, `stall.rs:176-181`, and `run.rs:102-105`.
`run_route_due` sets `parked` when a due step finds no room.
`route_progress` restarts or clears the reader deadline but leaves that flag set.
While the mandatory queue remains full, `deadlines_at` then hides the new deadline, including a future deadline that never ran.
This contradicts TM-3 and the stated E3-1 rule: a new step is carried until it runs; only a step that ran can be parked.

Required change: clear parking state when progress invalidates the old due step and arms a new step.
Check the other clock transitions for the same stale flag.
Add a public-behavior proof: park a due step, consume new progress while room is still full, and inspect `next_deadline()`.
Verify the new step's budget and room behavior at its own deadline.

### ST298-R4 — MEDIUM — Output reports bypass the progress flush rule

Evidence: worker `worker.rs:418-427`, `worker.rs:1096-1104`, and `report_output`.
`report()` flushes `output_unsent` before it flushes route progress.
The `LinkWritten` path also sends a waiting Output report before it calls `route_progress_written`.
`report_output` calls `send_report` directly, so it does not run the progress flush.

With an older unwritten Output report and progress report, a later output can leave both new reports pending.
When the link becomes writable, Core can receive the new Output report before its progress report.
That violates the R-90 report-order rule recorded in the design and worker module.
The title test covers a later title report; it does not cover these Output paths.

Required change: enforce the progress-before-later-report rule on the Output flush paths too.
Add a link-backpressure proof with pending Output and progress reports and a later output with no title report.
Check the resulting link order and the coalescing bound.

### ST298-E1 — MEDIUM — Exact-head focused Mac evidence is missing

The READY and PR body supply a full Linux gate but no completed focused Mac evidence for this head.
The change affects worker route writes, link reads and descriptor receipt.
Plan section 8, revision 23ai, requires focused Mac evidence for these OS-dependent paths before the first READY.

Required change: supply the completed exact-head focused Mac log and retain every failure and its disposition.
The earlier #295 Mac run does not prove the new worker stall and resync code.

### Evidence and scope

The Linux gate at `~/botster-sessions/gates/botster-core-stage1-st-stall-clock-0f9079c4-pool-20261011-185413-82688.log` is green.
Its header names the reviewed head and base. Every step passed, and the final status is exit 0 on msa1.
Mutation result: 230 tested, 211 caught, 0 missed, 0 timeout and 19 unviable.
Three catches came from the slow stage. These results do not cover the source traces above.

The receipt and session checks address the new RouteProgress identity risk raised during revision 4.
The reviewer accepts that change for these three clock reports. The PR retains the separate, pre-existing RouteClosed adoption limit.
The A3-1 and query exclusions remain explicit, and their IDs remain pending.
Six IDs move from testkit pending to real pending because their transcript fences are absent.
Three additional IDs stay pending and gain lead-approved budget holds with full-seed slow proofs.
The reviewer found no new list finding in this round.

## Core #295: merged-head final receipt — CLEAN

Merged head: `113da2366286adcef4a71449c5d362c146f6754f`.
Merged base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
The merge has the previously reviewed head and the named base as its two parents.
The reviewer checked the failed base-merge report and the overlapping diagnostics change.
`parked_on_room` excludes a budget-held frame while room is available. It includes a frame that waits only for room.
The `ready_at` and `accepts_at` refactors preserve the decisions used by #295's driver paths.
No source finding arose from that interaction.
The reviewer compared the PR's patch content after removing index lines and hunk locations. The old and merged patches match.
The reviewer also checked the four conformance lists by ID sets. Each merged set matches the base plus the PR's own changes.
These checks support the delta review; they do not change the failed base-merge-check result into a pass.

Completed final gate: `~/botster-sessions/gates/botster-core-stage1-st-more-input-113da236-pool-20261011-191408-70407.log`.
The header names the merged head and declared base. All ten steps passed. The final status is exit 0 on msa1.
Mutation result: 19 tested, 14 caught, 0 missed, 0 timeout and 5 unviable.
Both removed `e3_1` IDs passed on the testkit and real tiers. The log contains no FLAKY result.
The prior Mac and full-seed evidence remains recorded below. This merge adds no worker or link decoder change beyond the reviewed patch.
The package reviewer reports CLEAN on `113da2366286adcef4a71449c5d362c146f6754f`.
Sol-B separately requested the PR body's final gate and completed Mac record. This receipt does not waive that request.
The reviewer ran no local build or test.

## Core #295: evidence review

Reviewed head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
ST295-E1 is CLOSED. There is no new code commit and no new source finding.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-162605-32994.log`.
The header names the reviewed head. The job runs on macOS and explicitly sets `BOTSTER_PENDING_STRICT=1 BOTSTER_SEEDS=0-31` for real conformance.
All 351 selected host and link tests passed.
Both removed pending IDs passed in the full-seed real run (lines 570-571).
The named `slow_real_core` tests passed, including process exit, link hello and wake checks.

The broader real run passed 230 of 232 tests and exited 100. This review does not call that run green.
The two failures remain recorded:

- `conf::ev_5_route_closed_retires_route_keyed_events`, seed 1, step 28:
  `activity_before_close` was false after the close report was consumed.
  The pinned transcript sends r2 input, uses only `await_quiet`, and then sends Detach.
  It has no fence that proves the worker read r2's input before Detach.
  R-66's follow-up diagnoses this exact precondition gap and requires an activity fence before Detach.
  #295 preserves delivery order for complete frames on a link. It adds no synchronization between route input and Detach.
  The cited P6 log at `9231c06d` has an earlier retirement failure, but its expected value differs from this failure.
  That log alone is not an exact-signature reproduction. The pinned transcript and R-66 support the attribution here.
  The corrected evidence names the controlled v1 reproduction log `botster-core-detached-f68f26a7-f68f26a7-pool-20261011-150206-64641.log`.
  It contains the matching step-28 `{activity_before_close: false, closed: true}` failure at seed 0.
  Its header identifies a scratch snapshot, `9c44c653e9d71c32f276842b870a4c6a8a8dc203`, based on v1 `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
  The reviewer inspected its route_io diff: it delays r2's read input and adds tracing. It is not a clean-v1 gate.
  Sol-B reports that the controlled reproduction was accepted. The verdict retains the scratch qualification.
- `conf::st_6_capture_is_frozen_at_model_rev`, seed 5, step 17:
  the revision inequality compared 1 with 1.
  The pinned transcript sends the second output at step 13 and uses only `await_quiet` before ReadScreen.
  R-80 diagnoses this model-read fence gap. #295 changes neither the worker's output handling nor that transcript.
  The RC Linux log at `46c8c548` contains the same step-17 inequality failure at seed 0 (lines 5753-5755).

These diagnoses identify missing transcript fences outside #295's repair.
The changed driver can change timing, but it supplies neither missing fence and does not change the required worker report order.
The focused Mac checks and full-seed proofs required by ST295-E1 passed on the reviewed head.
The separate full Linux gate remains green on that head.
The implementer relays the lead's disposition: contracts #69 fixes the ev_5 fence; R-80 fixes the st_6 fence.
Both fixes reach Core through RC's .42 pin. Neither live ID is re-pended.

The reviewer ran no local build or test. This CLEAN covers #295's exact head and retains both broader-run failures as known limits.
The normal final-gate and base-merge rules still apply before the lead merges the PR.
The earlier implementation verdict below remains as review history; this evidence review supersedes its open status.

## Core #295: first implementation review

Reviewed head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
PR: <https://github.com/trybotster/botster-core/pull/295>.
Risk tier: HIGH. The driver change crosses into the shared link crate.
This review covers the separate TM-6 driver fix. The stall-clock design CLEAN below remains valid.

The reviewer found no new logic defect in the reviewed diff.
The process exit slot preserves a taken exit when the event budget is exhausted.
Link reads distinguish partial input from complete frames within the existing byte allowance.
The driver holds complete frames and link ends when the event budget is exhausted.
The held-frame path checks engine acceptance before the event budget, so a frame parked on queue room does not set `more_input`.
The decoder's new `ready()` query does not consume a frame or bypass its length check.

The read loop advances through a delivered frame or a read after `Ok(None)`.
The revision removes the loop that caused the first gate's mutation timeout.
The unchanged byte-allowance case remains conservative, as the PR body explicitly states.

The PR body includes Prior art and records coordination with PC's #291.
The reviewer checked the specified order: #295 lands first; #291 uses `end_link` and counts its pending drain as runnable input.
The review approves no unreviewed #291 delta.

### ST295-E1 — MEDIUM — Required Mac evidence is not yet supplied

The READY says the focused Mac job is running. The PR body supplies no completed Mac log.
Plan section 8, revision 23ai, requires focused Mac evidence for portable code whose behavior the OS decides, including sockets.
The changed driver reads link sockets and takes process exit reports.

Required change: supply the completed focused Mac log for this exact head and link it in the PR.
The evidence must pass the selected driver and link checks and the named real-process checks.
Also supply the promised real-tier results for both removed pending IDs with `BOTSTER_SEEDS=0-31`.
The Linux gate's slow tier uses seed 0; its green result alone does not prove those real-tier IDs at seeds 0-31.
The announced Mac job includes that full-seed evidence and can close this requirement when it passes.

### Evidence reviewed

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-161450-67786.log`.
The header names the reviewed head and base. Every gate step passed. The final status is exit 0 on msa1.
Mutation result: 19 tested, 14 caught, 0 missed, 0 timeout and 5 unviable.
Stage 2 did not run because stage 1 missed no mutant.

Both removed IDs pass in the gate's testkit tier and real tier:

- `conf::e3_1_due_step_beyond_budget_is_carried_with_more`
- `conf::e3_1_two_silences_due_together_with_budget_one`

The testkit tier uses seeds 0-31. The real tier in this gate uses seed 0.
The list changes remove only those two pending entries. They change no transcript or timeout value.
The new driver tests check event delivery, session state, transport closure, `more` and wake behavior.
The reviewer read the earlier red gate diagnosis and the exact-head repair in the source and PR body.
The reviewer ran no build or test locally.

There is one open evidence finding. A new code commit requires delta review.
An evidence-only reply can close ST295-E1 on this head without another code review round.

## Revision 3 review

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`, revision 3.
Artifact SHA-256: `2e6d6323dd5743a62962c4e9a9a1148449dcb30f1bf7659e758d56bdd1ba7a01`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. This verdict approves no implementation head or gate evidence.

ST-D2 is CLOSED. Section 3 records the lead's Q4 decision, option (b).
The stall-clock PR documents the adoption snapshot and restoration rule in both DESIGN.md files. It builds no adoption code.
The note states the adoption limit: a surviving route has no stall clock until DP-8 route adoption lands.
Both DESIGN.md files and the PR body must retain that limit.

The note names ST's next PR as the DP-8 dependency. That PR owns route reports, the new RouteId mapping, stable tags and restoration.
It also owns the specified adoption proof and all five listed `dp_8_*` ids.
The delivery order is the driver `more_input` PR, the stall-clock PR, then the DP-8 route adoption PR.

ST-D1 and ST-D3 remain CLOSED. All three design findings are closed. No new finding is added.
The reviewer verified the revision 3 hash and checked the adoption change against the lead's explicit decision.
The reviewer ran no build or test.

The earlier sections retain review history. This revision 3 verdict supersedes their status.

## Revision 2 review

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`, revision 2.
Artifact SHA-256: `42f1222eb18c204323f82305ea4daa50a4a9bf656f346a5ced3caeb11a54bf65`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. The reviewer ran no build or test.

- ST-D1: CLOSED for design. Section 2 flushes held progress before every later report.
  The revised bound allows one additional progress report per route before each other report.
  The specified proof covers the earlier unwritten `waiting: false` report under link backpressure.
  The implementation review will check both flush paths and the bound proof.
- ST-D3: CLOSED. Section 7b identifies old code, current reuse, rejected mechanisms, library alternatives and vault decisions.
  It states that no old code is copied. The reviewer checked the cited old write-attempt bound and close reason.
- ST-D2: OPEN pending Q4's scope decision. The proposed snapshot now identifies state, waiting, mark and applied limits.
  Its restoration rule distinguishes adoption from a new stall transition and specifies the required adoption proof.
  However, section 3 offers two delivery paths and does not select one.

The reviewer confirmed that DP-8 route adoption is absent at the reviewed base (`botster-core-host/DESIGN.md:167`).
The current adoption report has no route records. Thus, the proposed snapshot depends on work outside the existing adoption path.
The snapshot design resolves the original data gap, but the design cannot claim restoration until the delivery path is selected.

Required change for ST-D2: record the lead's Q4 decision in the design note.
If this PR builds DP-8 route adoption, name its files, identity mapping, clauses and proofs.
DP-8 and ID-2 require a new host `RouteId` for each surviving route.
If a later PR builds adoption, name that dependency and state this PR's adoption limit explicitly.
Keep the restoration design and proof requirement with that dependency. Do not claim that this PR restores adopted route clocks.

Section 7a correctly separates mandatory queue room from the later pump's event budget.
It also states all four Q1 conditions and the approved Q2 and Q3 rules.
The applied per-route `stall_deadline` correction follows OU-1.
No new logic finding is added in this round.

The sections below retain the first review and its evidence. Their original status is superseded by this revision 2 review.

## First review record

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`.
Artifact SHA-256: `ac4ac6e239efd9f9b9c8654252b95cf5e12b42e6382033c4e019991538c05353`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. No implementation head exists for this review.

The planned Core PR has the correct HIGH tier. The design assigns the stall clock to Core and route transitions to the worker.
This split follows TM-1, TM-3, OU-2 and R-90. The query clock remains outside this change, as EV-8(b) requires.

## ST-D1 — MEDIUM — Coalescing does not preserve the proposed title fence

Evidence: sections 2 and 5 of the design note.
Section 2 suppresses a second progress report while the first report is unwritten.
Section 5 assumes that the title report follows the progress report that arms the current deadline.

These rules do not establish that order. Consider this sequence:

1. The worker queues a progress report with `waiting: false`. The link has not written that report.
2. New output makes frames pending on a gated route. Coalescing holds the new `waiting: true` report.
3. The worker processes the title sentinel and queues its title report.
4. Core consumes the old progress report and the title report before it consumes the held progress report.

The title fence can complete while Core has no reader deadline for the pending frames.
An `advance_clock` can then precede deadline arming. R-88 and R-90 require a fence that proves Core consumed the relevant report.

The cited precedent has an extra rule. At the reviewed base, `worker.rs:418-425` flushes `output_unsent` before another report.
Its comments at lines 289-295 also permit an extra output report before another report to preserve order.
The proposed strict bound of one unwritten progress report omits that rule.

Required change: define how progress coalescing preserves report order before each report that can satisfy the title fence.
Reconcile that rule with the stated queue bound. Specify a proof with link backpressure and an earlier unwritten progress report.
The proof must show that the title fence cannot complete before Core consumes the current waiting report.

## ST-D2 — MEDIUM — Adoption cannot restore an existing stall from the proposed report

Evidence: sections 2 and 3 of the design note.
The worker sends `RouteProgress` for each live route after `Adopted`.
However, `waiting` is true only for an `Open` route with pending frames.
An existing `Stalled` route therefore reports `waiting: false`.

The proposed snapshot cannot distinguish that route from an `Open` route with an empty queue.
Core arms the close deadline only when it posts `RouteStalled`. The adoption path specifies no replay of that transition or stall state.
Thus, the stated adoption path cannot restore the close deadline for a route that was already stalled.

The steward approved Q3: no stall deadline runs while no host is connected.
This finding concerns the report that restores clocks after adoption, including a route that stalled before the host disappeared.

Required change: specify the adoption snapshot for both `Open` and `Stalled` routes.
Identify how Core receives the state, mark and applicable route limits needed to restore each deadline.
Apply the steward's Q3 answer to deadline arming. Keep state restoration distinct from posting a new transition event.
Specify an adoption proof with a route that stalled before the host restart.

## ST-D3 — MEDIUM — The required Prior art note is missing

Evidence: the design note has no Prior art section.
The `stop_grace` and `output_unsent` references identify current mechanisms but do not complete the required prior-art review.
BUILD.md rule 0 requires a note that identifies existing work, reuse, rejected work and reasons for custom infrastructure.
The reviewer must reject a feature without that note.

Required change: add the Prior art note before design approval.
Record the relevant vault decisions, old code at the authorized SHA, and framework or library alternatives.
State what the design reuses and rejects. Give the reason for each new custom mechanism.
If the implementation copies old code, identify the required `Stolen-From` and `For-Clause` commit trailers.

## Contract decisions received before the verdict was sent

The reviewer read the R-90 follow-up at contracts commit `43647054722f3133cfd30b67a909b8fc427dcb3e`.
The steward approved Q1 with four conditions:

1. No host-visible route fact changes before the event posts.
2. The post consumes event budget in the pump that posts it. Without budget, Core carries the post with `more: true`.
3. Core releases the reservation when it consumes the newer report after the worker declines the transition.
4. Due steps follow TM-5 and E3-1. Stall events across sessions follow link consumption order, with no asserted cross-session order.

The revised design must state these four conditions. The reservation alone does not satisfy the rule for the later post's event budget.
Q2 is approved in contract terms: `progressed` proves consumption of transport progress that occurred after the latest `route_accept`.
The contract definition does not expose the internal mark. The title fence remains conditional on report order, so ST-D1 still applies.
Q3 is approved: the host's absence suspends both stall deadlines. The new host arms deadlines after it consumes progress reports.
That answer does not supply the missing state in ST-D2's adoption snapshot.

The lead also assigned a separate host-driver PR for the TM-6 `more_input` defect before the stall-clock PR.
The pair reviewer and Sol review both Core PRs. This design verdict does not review or approve that separate driver change.

## Validation

The reviewer verified the artifact hash and read the design against the named contracts, rulings, BUILD.md and plan.
The reviewer inspected report ordering, host deadline scheduling and the current adoption report at the reviewed base.
The reviewer ran no build or test. All three findings remain open.

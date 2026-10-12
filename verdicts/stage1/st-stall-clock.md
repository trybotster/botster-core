# PR #298 integration review

## Round 7: source delta after the train-9b and train-10 import

Exact head: `7a8592fb493450c8b046a076cf484317eb58d96e`.
Exact tree: `baf0c3dc73bee4ff7ae3892f30fb7e16b97db3b0`.
Previous reviewed head: `8f070c492c17d0be727b914f74edce626065b7f2`.
Imported v1: `644f632f194f62ad7fe14e66482ee01083706e30`.
Actual merge base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
These are the merge's two parents and their actual base.
This round reviews the owner's Round 8 import. It supplies a source receipt only.

Sol-B compares Git objects without an executable merge or base check.
The PR retains exactly 38 changed paths relative to the imported base.
Every path retains the same added-line and deleted-line multiset as the previously reviewed PR delta.
Sol-B also reads the imported changes in all seven overlapping paths.
The imported worker action change concerns documentation for R-108. The existing stall action logic remains intact.
The imported drain proofs, socket diagnostics module, and real-route frame counter remain present.

The route-client conflict retains both TestkitRoute.reports and TestkitRoute.frames.
The constructor retains reports: None and frames: FrameEnds::default().
The with_reports method retains the progress record attachment.
The route_accept arm updates that record before the stream can accept output under its limit.
The temporary record lock ends before the stream-control call.
Client writes update FrameEnds; route_frames_taken reads the stream's taken-byte count and the separate frame boundaries.
The real route retains its progress hook before allowance and drain changes.
Its frame counter retains separate sent-byte, frame-boundary, and socket-query state.
Neither control replaces the other control's state.

The R-93 fence prerequisites remain in force.
From the route's first frame through its frame-consumption fence, no route_gate or route_accept limit may be in force.
The route_queue_bytes value must remain the Core default. This import adds no new transcript claiming that fence.
The source still pins .46. The R-109 real-pending re-entry and later pin requirement remain unchanged.

Sol-B independently checks all nine conformance text sets against both parents and the actual merge base.
Each set equals the parent union minus identifiers that either parent removes from the base.
The deferred TOML equals the imported base. All ten list checks agree with the owner's retained ID-set log.
Final identifier counts are pending 379, real-pending 44, held seven, ledger 692, real-only 26, and minimum 69.
The real-proof set retains four identifiers. The deferred TOML retains two deferred identifiers.
The imported base removes one real-pending identifier; the merge does not restore that identifier.
Cargo.lock, xtask/src/ci.rs, .gitattributes, and .cargo/mutants.toml retain the imported base's blobs.

Sol-B reads the retained local script and the local result summaries.
Log: `/Users/jasonconigliari/botster-sessions/gates/st-298-r8/local-7a8592fb.log`.
Computed SHA256: `746254c49f0feaa84a1dd84c8b1c914417e0dfca12bda8104588c561a51a023c`.
The log names this exact head and records a target clean at 2026-10-12T22:30:06Z.
All fifteen commands report exit 0. Selected packages pass 1235 tests; strict default conformance passes 595 at seeds 0-31.
These remain owner-run local results. They supply no independent cleanup, pool timing, or new focused Mac proof.

Sol-B reads the updated Round 8 PR body and the complete matching package section.
Package receipt: `4df1d162a66cff477d5da868cbe0195ad6ea2dbf`, verdicts/st-stall-clock.md on stage1/review-st.
The package reviewer reports source CLEAN on this exact head and imported base.
The lead handoff records the route-client conflict and #298's move to train 11.
The owner and package reviewer report the lead's order: no separate member gate; the train-11 gate covers the combined source.
This receipt accepts no completed gate at this new head. The earlier gate remains evidence for its own head and base.
The lead controls the combined source review, final train gate, final receipt, and merge checks.
All earlier findings remain closed. Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (source delta only; 0 open; the final train gate and applicable final review remain due)

## Round 1

Exact head: `0f9079c44e028644c6d1724c9dae9cd8b03d0459`. Reviewed and gate base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Lead assignment: `msg_plugin-w_1791771329_97e289`. Branch: `stage1/st-stall-clock`. Risk: HIGH.

Sol-B reviewed the host/worker clock protocol, queue reservation and driver boundary, progress report order, both testkit control paths, conformance changes, design note, package verdict, and complete PR body.

The host owns both deadlines on the injected clock. The worker owns route transitions and accepted-byte progress. The messages carry the worker's progress mark. No worker stall timer or production test branch is added. Terminal snapshots still come from the existing model and codecs.

The clock accepts the three new clock reports only after RouteReceived and from the route's session. The worker sends the receipt before reports for its bound route. The declared DP-8 adoption limit remains: surviving routes are not restored in this PR. The pre-existing stale RouteClosed limit is retained for DP-8. This review approves no DP-8 implementation.

The testkit and real tap share RouteReports. Both route_accept paths set the progress base before transport acceptance. The progress answer requires a later consumed mark. This proof retains the ruling's scope: no accepted bytes between the previous fence and route_accept. It does not replace the required report-order guarantee.

Package verdict: `2dab92da9aea9798256916956de2ad9a08eda7c1`, `verdicts/st-stall-clock.md`. Sol-B read the exact-head findings and independently checked the relevant boundary paths below.

### ST-I298-1 — HIGH — OPEN: the reservation blocks its release report

Matches ST298-R1. With one empty mandatory slot, the due close reserves it before the driver reads an already queued RouteResumed. The acceptance check permits RouteStalled and RouteClosed to use a reservation, but rejects RouteResumed. The FIFO held-frame path then blocks the report that must release the reservation. No queued event exists for a poll to remove.

An earlier mandatory frame can also block the due answer behind it on the same link. Accepting only RouteResumed does not resolve that case.

Required change: implement the steward's R-90 follow-up at contracts `1798a65d`, relayed by the lead in `msg_plugin-w_1791771819_28471d`. Only an earlier mandatory frame from the answering link, waiting only for room, can take over the slot. The answer then becomes an ordinary parked report. Preserve the mandatory bound, FIFO, event budget, and host-visible state at event post.

Require a driver proof of the empty-queue deadlock, takeover, parked answer, and post after a poll. Require a proof with several routes due together. Record the rule in DESIGN.md.

### ST-I298-2 — HIGH — OPEN: failed resume leaves incompatible clock states

Matches ST298-R2. On baseline failure, the worker clears its stalled state and starts or retains a healthy close without reporting resume. If writes then block, the host retains the old close clock. Its due message reserves a slot, but the worker declines it because the stalled state is gone. No answer releases that slot.

Required change: define and implement the report and clock state after baseline failure. Preserve the first close reason and typed close. Prove one accepted byte, failed baseline, then blocked writes across the old close deadline. Cover both a new SnapshotTooLarge close and an existing healthy close.

### ST-I298-3 — MEDIUM — OPEN: a new deadline retains old parking state

Matches ST298-R3. route_progress restarts the waiting deadline without clearing parked. While the queue remains full, deadlines_at hides that new deadline although its due step never ran.

Required change: clear obsolete parking state when progress invalidates the old step. Check the other transitions. Prove that next_deadline exposes the new deadline after progress while room remains full. Check its budget and room behavior when it becomes due.

### ST-I298-4 — MEDIUM — OPEN: Output bypasses the progress flush

Matches ST298-R4. report flushes Output before route progress. LinkWritten also flushes Output before route_progress_written. report_output sends directly. A later Output report can therefore reach the host before its pending progress report, contrary to the R-90 order rule.

Required change: apply the order rule to Output flush paths. Prove link order with earlier unwritten Output and progress reports, followed by new output without a title. Preserve the coalescing bound.

### ST-I298-5 — MEDIUM — OPEN: exact-head Mac evidence is missing

Matches ST298-E1. Plan section 8, revision 23ai, requires focused Mac evidence for these route writes, link reads, and descriptor receipts. The earlier #295 evidence does not cover the new worker resync or reservation protocol.

Required closure: supply the completed focused Mac log on the reviewed implementation head. Preserve all failures and their dispositions. Obtain package CLEAN on that same head after the source repairs.

### Evidence and accounting

Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-0f9079c4-pool-20261011-185413-82688.log`. The header names the exact head and base. All ten steps PASS. Default: 1,897 passed. Slow: 609 passed. Mutants: 230 total, 211 caught, 0 missed, 0 timeout, 19 unviable. Three catches come from stage 2. The job exits 0 after 919 seconds; the wrapper exits 0 after 920 seconds. These results do not close the source findings.

Six stall IDs leave testkit pending and enter real pending because the real transcripts lack consumption fences. The two additional e3 removals belong to inherited #295. Three lead-approved budget holds remain pending and run through held-full. The gate records five budget IDs at 32 seeds, one trial per seed. Sol-B found no additional accounting defect in this scope. No further ID removal is approved.

The A3-1 output_dropped and EV-8 query exclusions remain explicit and pending. The body states the adoption limit, prior art, earlier red gates, and required fence work. The later current-base merge and final gate remain due after source and evidence acceptance.

Sol-B ran no local build, test, gate, mutation job, reversal, or reproduction campaign.

VERDICT: NOT CLEAN (5 open)

## Diagnostic receipt — one re-gate attribution

Exact head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`. This receipt addresses plan 23al condition 2. It is not a round-2 source verdict or CLEAN.

The red gate `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195551-10795.log` fails slow on `conf::in_9_modes_read_at_write_start`, seed 0, step 8, `limit=step_timeout`. The job and wrapper exit 1.

PC's retained reproduction is `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/pc-r78/in9-repro-f6025aed.log`. It starts from v1 `f6025aed4cdf7da836e835e0040fc128b7e8f32f` and applies a scratch delay of 300 ms to mode output. It reproduces the exact ID, seed, step, and symptom. The change moves timing only, adds no product behavior, and changes no expected result. This is controlled reproduction evidence, not an unmodified-v1 run. The wrapper's exit 0 does not make its failed test green.

Sol-B read both logs, the pinned .41 transcript, the R-88 diagnosis, and the source delta. The transcript attaches no route. It starts a focus write under pty_blocked without the required held-write precondition. Its ModesChanged wait can discard an earlier completion before the later completion wait.

The host input transaction, worker input/model, real program control, and PTY backend paths are unchanged. The new progress flush has no route to report. The changed code supplies no missing held-write fence and does not change the diagnosed cause. Sol-B confirms non-interference for this exact failure.

Package attribution receipt: `d0e5f664dfaabd195534b06d1612c33173963bff`. Sol-B read it. The lead explicitly accepts this controlled reproduction under 23bl(A) and authorizes ONE re-gate in `msg_plugin-w_1791774104_a9ca69`. The repair owner is PC; the fence is assigned to contracts #79 and its next pin.

The one re-gate must pass every step on this exact head. This receipt authorizes no further retry or pending-list change. The five findings remain open until the round-2 READY and completed evidence. Sol-B ran no local build, test, or diagnostic job.

VERDICT: NOT CLEAN (5 open)

## Round 2

Exact head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`. Source base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Gate comparison base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`. No new base merge belongs to this round.

Sol-B read the complete source repair, new driver/host/worker proofs, exact-head Mac evidence, authorized re-gate, updated body, and package verdict `9d59fbec865c5f9d09b86467dd269e31d76213e2`.

The original five findings are CLOSED:

- ST-I298-1: asked state and slot ownership are separate. Same-link takeover preserves the asked step. The driver proofs cover the empty-queue resume, parked answer after a predecessor, and several due routes. Bounds, FIFO, and the steward's rule remain explicit.
- ST-I298-2: a failed baseline retains the original stalled mark. The due close can end blocked writes with the first reason. Worker proofs cover SnapshotTooLarge and an existing healthy close. The host proof covers the typed close and slot release.
- ST-I298-3: a replacement waiting step, stall, or resume clears obsolete parking. The new proof exposes the future deadline while room remains full and checks later parking.
- ST-I298-4: report_output flushes route progress first. The new proof covers pending Output and progress under link backpressure.
- ST-I298-5: completed exact-head Mac evidence supplies the changed checks. Its broader failures and dispositions remain recorded below.

### ST-I298-6 — HIGH — OPEN: clock transitions precede their parked events

Matches package ST298-R5. Sol-B independently checked inbound.rs, stall.rs, and run_parked. inbound applies route_stalled before route_event. That call releases the slot and arms close_at at the report-consumption pump. route_event can then append the stall event behind existing ParkedRoute work. run_parked posts that event later without applying its clock transition at the post.

With the last slot reserved and an older parked close waiting, consuming the stall answer releases room for the predecessor. The predecessor fills the queue. RouteStalled remains unposted, but next_deadline exposes its close clock. A delayed next poll can therefore permit CloseDue before RouteStalled posts. The resume path also changes its clock before a deferred event.

Required change: apply each host-visible stall/resume clock transition atomically with its event post. Preserve the report's mark while it waits. Use the posting pump's time for stall_close_after. Keep event order, mandatory bounds, and event budget.

Require a driver proof with an older parked event, the last slot reserved, and a delayed second poll. No close deadline can appear before RouteStalled posts. The full close interval must start at that post. Cover a deferred resume as well. Sol-B sent this finding to ST and 0181.

### ST-I298-7 — LOW — OPEN: the body overstates two evidence facts

The body calls the re-gate's base 20b2525c. The retained gate header names comparison base bfef8865; 20b2525c remains the source base. The body also calls the AM-2 failure a Mac-only defect. The logs establish Mac failures and sampled Linux passes, while R-93 diagnoses a missing cross-source arrival fence.

Required closure: distinguish the source and gate comparison bases. Describe the observed platform results without claiming that the defect is exclusive to Mac. Preserve the failures and R-93 disposition. Sol-B requested these corrections from ST.

### Completed evidence and limits

The one authorized re-gate is `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-200112-36429.log`. All ten steps PASS. Default: 1,907 passed. Slow: 609 passed. Held-full runs five budget IDs at 32 seeds. Mutants: 247 total, 227 caught, 0 missed, 0 timeout, 20 unviable. Three catches come from stage 2. The job exits 0 after 1,133 seconds; the wrapper exits 0 after 1,134 seconds on gaming. The earlier red in_9 gate remains recorded in the diagnostic receipt above. No further retry is approved.

Mac log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195909-27591.log`. The five changed packages pass 1,046 of 1,046 tests. The broader real run uses strict seeds 0-31, passes 240 of 242, and exits 100. This is not a green broader run.

- ev_5_route_closed_retires_route_keyed_events: seed 13, step 28, activity_before_close false after close. This is the retained R-66 missing input-consumption fence. The repair remains contracts #69 through its pin.
- am_2_one_admission_point_orders_all_sources: seed 1, step 14, BAC instead of ABC. The isolated base log `/Users/jasonconigliari/botster-sessions/gates/st-am2-isolated-base-20b2525c.log` reproduces the same ID, step, and bytes at seed 5. It records a target clean before fresh compilation from the separate base worktree. R-93 diagnoses the missing route-frame arrival fence before the host write. The lead assigns PC the route_frames_taken control and transcript fence. The admission queue is unchanged by #298. Its route timing can change arrival timing, but supplies no missing fence.

The package verdict retains both broader failures and closes its original evidence finding. No conformance file changes in this repair delta. The earlier accounting remains in scope. No additional pending removal is approved.

Sol-B ran no local build, test, gate, mutation job, reversal, or reproduction campaign.

VERDICT: NOT CLEAN (2 open)

## Round 3

Exact head: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`. Source base remains `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Gate comparison base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`. No new base merge belongs to this round.

Sol-B read all five commits and the complete six-file delta from `1d044f8b`. The delta changes host source, proofs, and DESIGN.md. It changes no worker, link protocol, testkit, process, PTY, socket, conformance file, or contracts pin.

ST-I298-6 is CLOSED under the lead's approved rule: add at post, remove at consume. The lead records this decision in shared/core-stage1/handoffs/core-lead.md. The steward records no objection. This decision supersedes Round 2's requirement to defer the resume's deadline removal until its event post.

ParkedRoute::Stalled retains the worker's mark. post_route_progress arms the close deadline only after post_mandatory succeeds. route_stall_posted uses the posting pump's time and the full stall_close_after interval. The parked runner removes the current event before that call. A later parked event for the same route suppresses the deadline, because Core already consumed that route's resume or close. Posting an older stall event therefore cannot restore its ended close step.

RouteResumed removes the close deadline at consumption and releases an outstanding close reservation. Its event retains the existing parked order. Waiting until the event posts can let a due close take the only slot. The resumed worker can then decline that close without an answer, which prevents the resume event from posting. The lead-approved rule avoids this cycle. DESIGN.md states both parts of the rule and their reasons.

The new driver proof uses an older parked close, the last reserved slot, and a late poll. It observes no close deadline before RouteStalled posts. It checks the full close interval from that post and the retained mark in RouteCloseDue. Two resume proofs cover takeover of a due close's slot and consumption before any close step runs. The engine proof covers a stall with its resume parked behind it. All four proofs pass in the completed gate. Event order, mandatory bounds, and the existing event-budget rules remain intact.

ST-I298-7 is CLOSED. Sol-B read the complete updated body. It distinguishes the source base from the comparison bases for both gates. It attributes AM-2 to the general R-93 missing arrival fence. It states the Mac failures and sampled Linux passes without claiming a Mac-only defect. Earlier failures remain recorded.

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-76b369c7-pool-20261011-205842-74210.log`. Its lists step names source merge base `20b2525c`, distinct from comparison base `fcc09461`. All ten steps PASS in 826.8 seconds. Default: 1,911 passed. Slow: 609 passed. Mutants: 260 total, 239 caught, 0 missed, 0 timeout, 21 unviable. Stage 2 tests the five stage-1 misses, catches three, and reports two unviable. Its unmutated baseline passes. No FLAKY result appears. The job and wrapper exit 0 after 834 seconds on gaming. The stopped `726f2547` gate supplies no completed result.

Reversal excerpts: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/handoffs/st-298-r3-red-on-revert-excerpts.md`. Reported SHA256: `786db4b3addbaccfff85dcfd7a0fbd374e9392a945f18843a29cc653ff2371dd`. Sol-B read the complete record. It describes the old-source reversal, the disabled stall-post guard, and the rejected at-post resume variant. It records their filtered failures and restoration.

ST copied these excerpts from its session transcript after the runs. No original log file exists. The record reports the same worktree and target for these checks. Sol-B retains these facts as ST's owner report. Sol-B does not claim independent verification of the original output, cleanup, or artifact provenance. The reviewed source and completed pool gate independently support the repaired behavior. Sol-B requested no rerun.

The original five findings remain CLOSED. Round 2's Mac evidence retains its exact earlier-head scope and its broader failures. Its selected packages passed 1,046 of 1,046; the broader real run passed 240 of 242 and exited 100. The R-66 and R-93 dispositions remain applicable. Round 3 changes no OS-dependent transport or worker path. It claims no new Mac run or repaired transcript fence.

Package CLEAN: `7c422c3c1e0e5f4a61b310f863160b1c67821a7e`, `verdicts/st-stall-clock.md`, on this exact head. Sol-B read the complete Round 3 section. Both reviews close the remaining clock finding and retain the evidence limits. No package or integration finding remains.

This CLEAN covers this exact source head and its supplied initial evidence. The current-base import, applicable base review, final exact-head gate, and final body remain due. The adoption and other pending limits remain recorded in the earlier rounds. This verdict approves no additional status removal or later contracts pin.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)

## Round 4 — conflicting base import and completed evidence

Exact head: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`. Imported and gate base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Tree: `5d8380121d25eadd25e7c5d896f869e81a695d5c`. Merge `24b5d65c374d2775b80b6024287a5974f66ad524` has parents reviewed head `76b369c7a39bec37a2bdb3c31110c887f60d4edb` and this imported base. The next commit supplies the mark-only test repair.

The lead removed #298 from train 1 because its base import conflicts. Sol-B read the announced resolutions, relevant driver composition, complete own diff against the imported base, and the follow-up test change. This is a manual delta review, not automatic carryover from a conflict-free canonical PASS.

The held file retains all three ST budget entries and the imported DP-5 budget entry. The host test module list retains both stall and stop_drain. The imported driver drain test receives mark: 0 in its RouteStalled construction. It registers no route and tests a frame blocked on mandatory room. The mark cannot arm a route clock in that scenario. Its original blocking, wake, exit, and drain assertions remain.

The driver retains the imported drain logic and #298's DueStep order and budget rule. The frame and deadline changes inherited from #295 now belong to the imported base. Imported test modules and exit fixtures remain alongside the stall tests. The reviewed stall-post guard and resume-consumption rule remain unchanged.

Sol-B independently compared the own patches and six three-way ID sets. The union formulas pass: held 7, pending 403, real-only 26, real-pending 49, real-proofs 3, deferred 2. The deferred file contains two IDs, not four. The implementer corrected that reported count. No ID or hold disappears through conflict resolution. No extra own status removal is approved.

Local log: `/Users/jasonconigliari/botster-sessions/gates/st-298-merge-local-4ca7bb2a.log`, with script st-298-merge-local.sh. Sol-B read the script, identity, selected summaries, and exits. The log reports a target clean before the run. Selected packages pass 1,086 of 1,086. The broader strict default run at seeds 0-31 passes 594 of 595 and times out on in_9_legacy_keys_table at 2.019 seconds, exiting 100. The later isolated 1.758-second pass remains an implementer report. Neither result turns the broader run green.

Completed pool gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-4ca7bb2a-pool-20261011-221430-25804.log`. SHA256: `68897193bd2a01c3c03527ea72fa75b57b91c68bd936d0880fc42745ac40778a`. The header names the exact head/base, and the lists step uses the same merge base. All ten steps PASS in 938.9 seconds. Default: 1,966 passed. Slow: 616 passed. Held-full runs six budget IDs at 32 seeds.

The repaired stall-post, deferred-resume, and imported drain-room proofs pass. Mutation tests 241 mutants: 225 caught, 0 missed, 0 timeout, 16 unviable. Stage 2 tests the five stage-1 misses, catches three, and reports two unviable. Its unmutated baseline passes. Sol-B independently matched all 225 raw catches to citations and normal PASS records. No FLAKY result appears. The job exits 0 after 962 seconds; the wrapper exits 0 after 963 seconds on MS-A1.

Package final CLEAN: `8b2d3a41af91a74386354fdd46a84e4feb6e47e7`, with body follow-up `a99f4beb5644dd9f4acabf432daa9148f3720168`, `verdicts/st-stall-clock.md`. Sol-B read both records. Sol-B read the complete updated PR body. It names the import, union resolutions, mark-only follow-up, correct counts, completed gate, and retained local timeout. The package body requirement is complete.

All earlier findings remain closed. The earlier Mac failures, copied reversal provenance, adoption limit, and pending scope remain recorded at their original evidence scope. No source, evidence, or body finding remains on this exact head. This receipt accepts no later train composition or base update. The lead controls that next step and its applicable review and gate. Sol-B ran no execution job.

VERDICT: CLEAN (0 open; reviewed import head and completed evidence)

## Round 5: train 2 import delta and completed evidence

Exact head: `5d5d4eefd5872864a972bc248f415f9cf6673ef3`.
Tree: `ff95a381f46a06a47a16c2285deb904f484dbb04`.
Previous accepted head: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
Imported and gate base: `080987829a2e2245a7ea97606020400392c974d6`.
Three-way base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Merge `d1ff70d31b8f0fc71a2e5112c625a86740ee64bf` has the previous head and imported base as parents.
The follow-up `5d5d4eef` has that merge as its only parent.
The lead orders this import after #298 leaves train 2 for conflicts.
This manual delta review supplies no automatic canonical PASS claim.

Sol-B read the merge objects, both conflict resolutions, semantic follow-up, and complete driver and engine delta against the imported base.
The host design retains train 2's F7 KillPayload rule at stop grace under R-97.
It retains #298's carried due-step rule and stall section.
The driver module list retains `holds`, `seeded_trace`, and `stall`, alongside the imported drain module.
The follow-up replaces `Work::Silent` with `Work::DueStep` in `Driver::held` and updates its comment.
DueStep includes a due Silent event and a route stall step. Both bypass scheduler holds and operation deferral.
Other work retains the imported scheduler filter before choice selection.
Runnable work, synchronous wake, and both `more` calls use that filter.
The driver retains the bounded initial due-step loop and the imported drain check after wake settlement only.
No obsolete `Work::Silent` reference remains in the host crate.

Sol-B independently compares all 38 own file patches before and after the import.
Only the declared held predicate and comment differ. All other added and deleted lines match.
No extra own path appears. The reviewed host and worker stall rules remain unchanged.
The imported stop grace, scheduler, drain, and test changes retain their accepted base scope.

Sol-B independently checks all nine conformance text sets with the three-way formula.
Every result matches the base plus own additions minus own removals.
Held contains seven IDs; pending contains 400; real-only contains 26; real-pending contains 49.
Real-proofs contains seven proof rows for four IDs. Both deferred entries retain their exact fields.
Both mutation exclusion arrays satisfy the same three-way formula.
No process allowlist or known-flaky change appears. No extra status removal is approved.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-5d5d4eef-pool-20261012-083713-16979.log`.
SHA256: `4831462bf016f8212cc7e0aa028b5f7137e3d7d42b145abf6126dfe082e893d6`.
Sol-B reads the identity, base accounting, affected tests, raw mutation results, citations, summaries, and exits.
Gate and merge base both equal the imported base.
All ten steps PASS in 1,124.2 seconds. Default passes 1,987 tests; slow passes 619 tests.
Strict conformance passes 267 tests without failure. Held-full runs six budget IDs at 32 seeds.
The repaired stall-post, resume, imported drain, scheduler hold, and retained seeded trace proofs PASS.

Stage 1 tests 244 mutants: 225 caught, five missed, and 14 unviable, with zero timeout.
The unmutated slow baseline passes all 619 tests.
Stage 2 tests those five exact misses: three caught and two unviable, with zero missed or timeout.
The final result has 228 caught, 16 unviable, zero missed, and zero timeout.
Sol-B independently matches all 228 raw catch identities to their citations. Every catcher has a normal PASS.
No FLAKY or INCONCLUSIVE record appears. Unviable results supply no catch evidence.
The two link fuzz harnesses complete 6,536,902 and 4,248,656 runs.
The job and wrapper exit 0 after 1,238 seconds on msa1, with one second queued and 1,237 seconds running.

Local log: `/Users/jasonconigliari/botster-sessions/gates/st-298-merge-local-5d5d4eef.log`.
Sol-B reads its retained script, identity, selected summaries, and exits without executing it.
The script selects the ST worktree and shared ST target. The log records a clean at `2026-10-12T15:25:41Z`.
All 14 commands report exit 0. Selected packages pass 1,104 tests with one skipped.
Strict default conformance passes 595 tests at seeds 0-31.
These are retained owner-run results. Sol-B does not independently verify target exclusivity or cleanup.
Earlier Mac evidence, broader failures, copied reversal provenance, adoption limit, and pending scope retain their original scope.

Sol-B reads package receipt `5db1fdd56758aeb842346b1c064373ff1055c74d`, its complete #298 Round 5 section.
It agrees with these checks and reports zero findings.
Sol-B reads the full exact-head PR body. It names both text resolutions, the semantic repair, bases, local results, and completed gate.
All earlier findings remain closed. No source, body, or supplied-evidence finding remains on this exact import head.
This receipt accepts no later base update or train composition. The lead controls the next train and its final gate.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; train 2 import delta and completed exact-head evidence)

## Round 6: train imports, R-109 re-entry, and modes repair

Exact head: `8f070c492c17d0be727b914f74edce626065b7f2`.
Tree: `989a0a6373da2395837d9dcf46b47c8ce6682205`.
Final imported source and gate base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Previous reviewed head: `5d5d4eefd5872864a972bc248f415f9cf6673ef3`, on base `08098782`.
Owner READY: `msg_plugin-w_1791841121_b8cef6`.
This round covers the owner's rounds 6 and 7 on the final exact head.

Sol-B reads the merge objects, complete own patch comparison, conflicts, semantic repair, updated body, and evidence.
The three imports are c1c6363d from e37b91d6, ae9d4ced from 007efbde, and 8f070c49 from 2d8e8acc.
The intervening commits add the R-109 re-entry at 77b608d5 and repair modes queueing at 393b64f0.
The complete final own patch has 38 paths, with 3272 additions and 204 deletions.
Its full-index SHA256 is `9e799f64241ce10b7143578f38dc26a0ec4b944c42c3d5c99e909a1166b13b9f`.
The source diff check passes.

### Retained edits and merge boundaries

The final own path set equals the previously reviewed 38-path set, with no new or missing path.
Thirty-four added/deleted line sets equal the previous accepted patch, excluding object indices and hunk positions.
The four differences are real-pending, worker DESIGN.md, route.rs, and its stall test module.
They contain exactly the declared re-entry, modes repair, documentation update, and regression.
No other own semantic change appears in this comparison.

The host design union retains the stall clock, carried due-step rule, and imported control-queue section.
The worker route union retains stall/history alongside size/reported/activity, including constructor initialization.
The test union retains report_order and stall. The real-pending union retains both groups.
The final held conflict retains all three ST budget records and takes the base's current GS DP-5 record.
The imported held-full header remains present.

The driver still exempts Work::Deadline and Work::DueStep from scheduler holds.
Mandatory-room reservation, takeover, parked-step wake, report ownership, and add-at-post/remove-at-consume rules remain intact.
The imported control-room and frame-fit paths do not remove the reviewed due-step behavior.
The inherited R-106 failure handler remains present and changes no stall-clock or route-queue decision.
Both real and simulated route callers retain the imported hex-string form of route_pty_log.
The cached input log and ST's output-copy change coexist; the progressed control remains present on both tiers.
No new platform operation enters the semantic repair.

### Modes queue repair

The imported DP-6 push_modes used the unclassed queue. The repair uses each Queued entry's bytes and full encoded length.
It searches only unstarted frames and replaces at most one modes frame, placing the new frame at the end.
A frame with an outstanding write or accepted bytes remains started and completes.
The replacement has Kind::Droppable and exempt false; it belongs to no exempt baseline sequence.
Removing an older modes frame reduces queued by that frame's byte length and needs no baseline adjustment.

The stall drops that unstarted modes frame under the existing droppable rule.
The resumed baseline carries the current modes and size through baseline_begin.
The regression resizes a waiting route to 30 by 100, stalls it, completes the started output, and reads the resume sequence.
It requires no retained modes frame and requires baseline_begin at 30 by 100.
Existing partial-write, outstanding-write, and latest-unstarted coalescing proofs remain present.
The route and regression blobs remain unchanged through the final train-8 import.

Sol-B reads the complete focused reversal log at repair head 393b64f0.
Path: `/Users/jasonconigliari/botster-sessions/gates/st-298-r6/reversal-modes-kept.log`.
SHA256: `58938c188ff4add0b7e70e240aca8fb064bea53d666bbdeb842146e7f029991f`.
The scratch change replaces only Droppable with Kept. The new test fails on the unexpected retained Modes frame, exit 100.
The other eight selected tests pass. This reaches the intended semantic failure, not a setup failure.
The target and same-worktree statements remain owner reports; no independent cleanup claim is made.

### R-109 and conformance accounting

Sol-B reads R-109 and the lead's explicit re-entry approval and owner assignments.
The earlier c1c6363d gate remains RED at slow: ou_3_progressing_reader_lossless, seed 0, step 48, receives RouteStalled.
Its SHA256 is `04f3eb39217db9e95d10da431670b0fdbfdeefe36f25f40b15fb77e0792b659d`.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-c1c6363d-pool-20261012-111545-85579.log`.
Mutants and fuzz are NOT RUN. The wrapper exits 1 after 437 seconds.
R-109 identifies missing progress-consumption fences after route_accept; await_quiet does not supply that proof.

The final re-entry tag is directly above ou_3_progressing_reader_lossless.
It names R-109, the exact cause, contracts #99's progressed fences, and removal when their tag is pinned.
The body and lead handoff assign the later pin to RC and the list-only follow-up to ST.
The source still pins .46. This review accepts no execution of the later #99 transcript fences.
No unfenced real-tier PASS is accepted for this identifier.

Sol-B independently checks all nine conformance text sets at each of the three merges against their actual parents and bases.
All 27 formulas pass. The explicit re-entry is a separate one-identifier addition after the second merge.
The retained owner ID-set logs also match; the final one identifies its merged input as the worktree.
Independent Git-object checks establish the final committed sets.
Final counts are pending 379, real-pending 45, held seven, ledger 692, real-only 26, minimum 69, and four real-proof identifiers.
The deferred TOML equals the current base and retains two deferred identifiers; deferred text retains its third not-applicable row.
All current-base removals remain removed. The six original testkit removals and their real-pending disposition remain exact.
The three ST budget identifiers stay pending. They are not newly promoted under #329's accounting rule.
Every mutation-configuration value and the structured IN-8 known-flaky entry equal the imported base.

### Completed exact-head evidence and package review

Final gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-8f070c49-pool-20261012-142423-35272.log`.
Computed SHA256: `0757a3b31f61b7a9664a000960342a70a223e52fd8ab101b2fe52d22bdf970a2`.
The header names this exact head and final imported base. All ten steps PASS, with a total of 810.2 seconds.
The job exits 0 after 823 seconds on msa1: one queued and 822 running. The wrapper exits 0 after 824 seconds.
Default passes 2148 tests, with 406 skipped, in 13.908 seconds.
Ordinary slow passes 679 tests, with 2093 skipped, in 42.059 seconds.
Strict conformance reports 287 passed and zero failed. The new modes regression passes in the default run.
The held-full run passes 192 trials: six budget identifiers, each at exactly seeds 0-31, all within two seconds.
The three ST maxima are 0.168, 0.160, and 0.139 seconds; their pending membership remains unchanged.
Real minimum is 66/68 and real-accepted is 67/69, preserving the R-109 evidence debt.

Stage 1 has 248 mutants: 229 caught, five missed, fourteen unviable, and zero timeouts.
The unmutated stage 2 baseline passes 689 tests in 41.657 seconds.
Stage 2 tests exactly those five misses: three caught, two unviable, zero missed or timeout.
Final results are 232 caught, sixteen unviable, zero missed, and zero timeouts.
Sol-B independently matches all 232 raw catch identities to citations and verifies each catcher in ordinary PASS results.
No runtime FLAKY or INCONCLUSIVE result appears. The IN-8 exclusion earns no catch credit.
Both changed link decoder harnesses complete their fuzz runs: 6113865 and 3502986 inputs, each in 61 seconds.

Sol-B reads the retained round-7 local script and its result summaries.
The log names this head and a target clean at 2026-10-12T21:14:37Z.
All fifteen commands report exit 0; selected packages pass 1227 tests, and strict conformance passes 595 at seeds 0-31.
These remain owner-run local results, not independent cleanup, pool timing, or new focused Mac evidence.
Earlier unclassified local timeouts and focused Mac limits retain their original scope.
The earlier 393b64f0 gate remains evidence for its own head and base, not a substitute for the final gate.

Sol-B reads the complete updated body and complete applicable package section at `d273da6eae0f3fed19943d8c94d665145b160235`.
File: verdicts/st-stall-clock.md, Core #298 import and modes repair, rounds 6 and 7.
It agrees on the exact head, imports, re-entry, semantic repair, evidence, and retained limits.
All earlier findings remain closed. No source, body, or supplied-evidence finding remains on this exact head.
The lead controls the next train and any later base update, final receipt, and merge checks.
This CLEAN accepts no future pin, later head, completed merge, or additional retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; resolved imports, R-109 re-entry, modes repair, and completed exact-head evidence)

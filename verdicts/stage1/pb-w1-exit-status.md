# Worker process exit-status integration review

## Round 1: source and Linux evidence; Mac evidence open

Exact head: `572924b29bf4fca3d6a8c76d45fc486c0c070b70`.
Tree: `3160930ba38e79624dc363daad9ee4cb511754d0`.
Source and Linux gate base: `007efbde6a59af403d0937c38361c137a8961a48`.
Lead assignment and interface ruling: `msg_plugin-w_1791839207_377aca`.
The base is an ancestor of this head. The source diff check passes.
The patch changes 24 files, with 157 additions and 266 deletions.

The lead approves removal of the unused worker status instead of carrying an invented Code(-1).
Sol-B reads the complete production interface delta, producer and consumer contexts, affected tests, and full body.
Sol-B reads the complete package receipt at `f71ae45cc55333d7318bd16397a29eab4fa434f6`.
File: verdicts/pb-w1.md. It records no source-logic finding and one required-platform-evidence finding.
The package and integration source conclusions agree.

### Shared interface and observable behavior

Process::poll_exit and HostEdges::poll_process_exit now return only ProcessIdentity.
Input::ProcessExited also contains only identity.
Sol-B searches every tracked Rust file for the producers, consumers, and poll methods.
The real edge, simulated edge, worker spawner, edge tap, report holds, guarded spawner, recorded spawner, and host mocks use the new type.
No old process-exit status field remains in a producer or consumer.
Program::poll_exit still returns payload ExitStatus. That separate interface does not change.

The only production ProcessExited consumer was on_process_exited, which explicitly discarded _status.
Sol-B independently compares its body before and after this patch: the body is byte-identical.
It still matches the complete process identity, marks the worker gone, and retains the existing link-drain and teardown paths.
The driver retains the same identity when its pump budget is exhausted and feeds it on the next pump.
Wake state, more-work reporting, event accounting, frame drain, and classification retain their earlier behavior.

The sys reaper retains the actual std::process::ExitStatus when Child::wait succeeds and records None when it fails.
The exit conversion function and both invented Code(-1) fallbacks are absent.
The reaper still queues the identity and optional raw status before notifying the condition variable and host notifier.
Children::poll_exit and wait_exit retain queue order, one-time consumption, pid matching, and removal from the live set.
The real host edge discards that optional raw status and returns only the identity.
The optional raw status remains available for sys tests; it supplies no contract-visible status.

Testkit Processes::end retains one-time ended state, exit queueing, and the existing wake.
Worker exit and external kill paths use its one-argument form.
The tap retains exits taken ahead. ReportHolds retains worker identity, session mapping, queue order, and release/break rules.
Its gives decision still depends on a releasable identity, not a removed status.
The changed tests retain exit order, one-time delivery, pump-budget retention, drain, hold, release, and cleanup assertions.
Slow sys tests still require exact OS exit codes and signals through code and killed helpers.
The deleted conversion test covered the deleted conversion; real wait-status assertions remain in the slow tests.

No host operation, typed result, Lost reason, Exited payload report, or control message used the removed worker status.
The API snapshot changes one RealEdges::poll_process_exit line to the approved identity-only signature.
The contract facade's operations and outcomes do not change.
Payload watch code, worker machine code, and worker binary code equal this branch base.
The payload's truthful status and R-106 failure path remain separate from this interface.

### Scope, exclusions, and integration debt

The body contains the prior-art note, HIGH tier, exact Linux gate, earlier failure, and #327 overlap.
It names the old background reaper, which also discarded the worker status, and the rejected alternatives.
It claims no copied old code, new dependency, or version change.
Static TOML comparison finds only two removed mutation expressions, both for deleted minus signs.
The patch adds no exclusion and changes no other mutation configuration value.
No conformance list, contracts pin, timeout, budget, or seed count changes.

This member does not contain train 8's #327 ExitFailure arm.
That arm calls Processes::end with a status in train 8 and must lose that argument when this member imports the new base.
The body and both reviewers retain this explicit integration obligation.
It is not a defect in this member's current base, but no later combined head carries this source verdict without a delta review.
The later import must retain B1's failure action, cleanup, and Lost/Stop proof.

### Completed Linux evidence

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pb-w1-exit-status-572924b2-pool-20261012-135330-10095.log`.
Computed SHA256: `38a8d37f78bbe087a653db9ffb2fecadb7ad01a5204cdef99f094f2816b5094c`.
The header names this exact head and base. All ten steps PASS, with a total of 461.2 seconds.
The job and wrapper exit 0 after 761 seconds on msa1: 291 queued and 470 running.
Default passes 2079 tests, with 412 skipped, in 13.203 seconds.
Ordinary slow passes 677 tests, with 2081 skipped, in 44.044 seconds.
The slow_process binary passes 24 tests plus its build-root test. Both named reaper slow_tests also pass.

Mutation stage 1 records 20 mutants: 18 caught, one missed, one unviable, and zero timeouts.
The unmutated stage 2 baseline passes 687 tests, with 2905 skipped, in 44.815 seconds.
Stage 2 catches reap replaced by an empty body with conf::a10_2_corrupted_row_is_lost_registry_corrupt.
Final results are 19 caught, one unviable, zero missed, and zero timeouts.
Sol-B independently matches all 19 raw identities to citations and verifies every catcher in the ordinary PASS results.
No runtime FLAKY or INCONCLUSIVE result appears. Fuzz selects no changed decoder harness and passes without a harness run.

The earlier 30e4a2f7 gate remains RED at public-api because the snapshot retained the old method signature.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pb-w1-exit-status-30e4a2f7-pool-20261012-134435-66436.log`.
SHA256: `5b67348d9d4409a863a03bc95b08fb757c00c47323466a9f3f2135990fed076f`.
Later steps are NOT RUN. The corrected head changes only that snapshot line from the previous head.
The current Linux gate does not provide macOS behavior evidence.

### B330-I1 — MEDIUM — Required focused Mac evidence is absent

This is the same evidence gap as package PBW1-1.
Plan section 8, revision 23ai, requires focused Mac real-process evidence for OS-decided identity, signals, and reaping, even without an OS cfg.
Pin: `/Users/jasonconigliari/botster-sessions/pins/stage1-plan.a3333ad4.md`, line 508.
The changed raw reaper result and real-process code/signal assertions fall within that rule.
The body labels its local Mac runs as not evidence and supplies only the Linux pool gate.
No applicable focused Mac pool log has been supplied on this exact head.

Required change: supply the exact-head focused Mac pool results for slow_process and the affected process.rs slow_tests.
Record the log and head in the body. Preserve the existing assertions, timeouts, and budgets.
This finding requests no source change, separate full gate, or reviewer execution.
The package reviewer reports that PB accepted this finding and awaits pool disk capacity.
The capacity figures remain operator reports; Sol-B makes no independent capacity measurement or cleanup claim.

No other source, interface, body, or supplied-Linux-evidence finding remains.
The focused Mac proof, matching package closure, current-base import, assigned delivery review, and final merge checks remain due.
This review accepts no completed delivery, later head, #327 integration fix, or retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (1 MEDIUM open: required focused Mac evidence)

## Round 2: focused Mac evidence closure

Exact head remains `572924b29bf4fca3d6a8c76d45fc486c0c070b70`.
Reviewed tree remains `3160930ba38e79624dc363daad9ee4cb511754d0`.
Source and completed Linux gate base remains `007efbde6a59af403d0937c38361c137a8961a48`.
This round changes no source review. The earlier source and Linux evidence findings remain closed.

B330-I1: CLOSED. The same package finding, PBW1-1, is also closed.
Sol-B reads the complete focused Mac pool log supplied in msg_plugin-w_1791848245_65778e.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pb-w1-exit-status-572924b2-pool-20261012-163652-79654.log`.
Computed SHA256: `93878c3f9836e48e69a7bd1301d7f13675b3e40d5fe3d8a12e1a08a14dbd7fe6`.
The wrapper and executed HEAD both name the unchanged reviewed commit.
The wrapper records newer origin/v1 644f632f as its base ref. The job does not import that base into the reviewed source.

The job runs on the Mac pool with allocation 15e77b4f, macOS 27.0.1, build 26A434, and arm64.
Both commands select botster-core-sys with the slow feature and slow profile.
The commands retain pipefail, so their output filters do not hide a failing command status.
Cargo build parallelism and nextest test parallelism are both four.
The slow_process binary passes all 24 selected tests with zero skipped, in 0.246 seconds.
The slow library selection passes 25 tests with 43 outside the filter, in 1.052 seconds.
Both affected process reaper tests pass: a_reaper_returns_right_after_its_notify and a_reaper_without_a_child_returns_at_once.
The process selection also passes identity, group kill, own-child wait, exit-code, and notifier checks.
The built-in worktree marker passes in slow_process.
The job exits 0 after ten seconds, with zero queued and ten running. The wrapper also exits 0 after ten seconds.
Sol-B makes no independent target-clean, allocation, or cleanup claim beyond the supplied log.

Sol-B reads the complete updated PR body and verifies its unchanged head and focused Mac citation.
The body retains the earlier RED public-api gate, the completed Linux gate, and the #327 overlap.
Sol-B reads the complete applicable package section at f5696383a670956db075bae743bb4c32f69698a7.
File: verdicts/pb-w1.md on stage1/review-pb, Current verdict: round 2.
The package receipt reports CLEAN on this exact head and closes the same evidence requirement.
The local Mac runs remain owner-run local evidence; this pool run supplies the required focused platform proof.

No finding remains open on this exact head. Current-base import and the #327 ExitFailure adaptation remain due.
That later source needs the assigned delta review. The lead controls the delivery gate, final receipt, and merge checks.
This receipt accepts no later head, completed delivery, additional retry, or already-completed #327 adaptation.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; source, Linux evidence, and focused Mac evidence on the unchanged reviewed head).

## Round 3: current-base source delta

Exact head: `99209ea598aec0c182f35a1645b2a938d2f7d8cc`.
Exact tree: `1ab9f5a0d3a6a0dfb040ea06b385078bea778495`.
Previously reviewed head: `572924b29bf4fca3d6a8c76d45fc486c0c070b70`.
Imported v1: `644f632f194f62ad7fe14e66482ee01083706e30`.
Actual merge base: `007efbde6a59af403d0937c38361c137a8961a48`.
Merge befb21878c974c974b579e2e11b54d83b179eb34 has the previous head and imported v1 as its parents.
Commit 99209ea5 follows that merge and changes only the ExitFailure arm and its comment.

Sol-B checks the source through Git objects. The imported base is an ancestor of this head.
The PR retains exactly the original 24 changed paths relative to the imported base.
Added-line and deleted-line multisets match the original reviewed delta in 23 paths.
The worker.rs difference is exactly the required #327 adaptation, with no other new PR edit.
Sol-B reads the imported changes in all five overlapping paths.
They retain the held-full comments, zero-start-time proof, kill-tail proof, and exit-watch injection and drain proofs.
The conformance directory equals the imported base; no identifier is restored, added, or removed by this PR.
The mutation configuration retains the reviewed removal of the two obsolete literal exclusions and adds no exclusion.

The ExitFailure arm calls Processes::end(self.id) and then self.ended(true).
Processes::end queues the worker identity once and wakes its host.
ended(true) closes the link, removes the endpoint, closes candidates and route streams, and drops the payload.
The imported exit-watch proof still requires Lost(WorkerGone), no Exited event, completed Stop, and an ended payload.
The adaptation removes the unused worker status; it does not change the payload's exit status or R-106's production failure path.
The real worker's failed run still returns an error. This PR does not change that real driver.
The reaper changes and affected platform assertions retain their previously reviewed behavior.
The imported process.rs change adds the zero-start-time test and changes no reaper statement.

Sol-B reads the local script and retained result summaries supplied with the updated body.
Log: `/Users/jasonconigliari/botster-sessions/gates/pb-w1-merge-local-99209ea5-20261012-234925.log`.
Computed SHA256: `10c8bc76280bf827b1a829f51b3ee3c22ffc7fbea0010a8b44fb2a6fc3a774c6`.
The log names this head, a clean worktree, and a target clean at 2026-10-12T23:49:31Z.
The script records workspace test compilation, slow-feature clippy, public-api, taint, worker prebuild, and strict default tests.
Public-api matches; taint passes; the strict run reports 1748 passed and 114 skipped in 28.302 seconds.
The script reports exit 0 and no Code(-1) match in crates.
Its retained PASS lines are partial. Sol-B claims the aggregate result, not an independently visible PASS for each selected test.
These remain owner-run local results, not pool evidence or independent target-clean or cleanup proof.

Sol-B reads the complete updated body at this exact head.
The requested body correction is complete: #327's adaptation is resolved, and the new head, base, local record, and gate order are explicit.
The previous Linux and focused Mac logs retain their original head and base scope.
Sol-B reads the complete matching package source section at 0d2a145da0e7c28b81dfaeb502db1b0284d450c8.
File: verdicts/pb-w1.md on stage1/review-pb, Current-base source delta: round 3.
The package reviewer reports CLEAN on this exact head, tree, and imported base.

All earlier findings remain closed. No source, body, or supplied-record finding remains open.
The owner and package reviewer report the lead's order: no separate member gate; the delivery gate covers the combined source.
This receipt claims source CLEAN only. It does not reuse the previous head's gate as a gate on 99209ea5.
The lead controls the train, any later delta review, delivery gate, final receipt, and merge checks.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (source delta only; 0 open; the delivery gate and applicable final review remain due).

## Round 4: train-12 member composition delta

Exact train head: `7b37e787209e839feb2220b7216d1e417b3838b8`.
Exact tree: `6adc74c7fcc7573548997fcc83a9280a3260dc3c`.
Train base: `131024f3936898a3106af299e6751553c55bbc82`.
Frozen order: #330 at 99209ea598aec0c182f35a1645b2a938d2f7d8cc, then #331 at 57cb242f5d3dda40e524090239222e4824021798.
Lead assignment: msg_plugin-w_1791851132_792fdb.
Scope: preservation of #330 in this exact composition, including the ten paths changed by train 11.
Sol-C owns #331, the combined review, and the one final train receipt.

The #330 insertion is 848800a3dc44f3b575f4b4ab627d2f022eb13b7f, with the train base and member head as its parents.
The final insertion has that first merge and #331 as its parents. Sol-B checks those Git objects.
All 24 member paths retain the exact added-line and deleted-line multisets at the first insertion.
The second insertion changes only .cargo/mutants.toml among those paths.
It moves #331's existing high-tier whole-body exclusion from xtask to gate-kit.
Both #330 removals of obsolete status-literal exclusions remain removed.
The final 23 source and API paths retain #330's exact edit sequences against the train base.
The conformance directory equals the train base. This member changes no conformance identifier.

Sol-B reads the imported changes and merged exit contexts in all ten requested host and testkit paths.
The driver keeps a taken process identity when event budget ends and feeds that identity before a later edge exit.
The due-step import changes neither the held identity nor the ProcessExited payload.
The host still finds the worker by identity and drains a running or stopping worker's complete control-link frames before classification.
Remove still retains its teardown link for the result, with its existing grace bound.
The new route-stall and client-input observations add no worker-status consumer.
The drain fixture retains the queue-pressure and exit-order proof with the required route mark.
The host tests retain identity-only exit inputs alongside the imported cursor ordinal and stall modules.

The tap still queues and releases ProcessIdentity values through the existing report-hold path.
The new route-accept progress hook uses a separate report record.
The worker's imported facts-cost decoder observes LinkSend frames and changes no exit-queue or cleanup action.
Exit and ExitFailure still queue one worker identity, wake its host, and call ended(true).
That cleanup closes the link, endpoint, candidates, descriptors, and route streams, and drops the payload and held spawn.
The sys reaper, affected real-process assertions, and RealEdges source blobs equal the accepted #330 member blobs.
No invented status, discarded identity, duplicate end, or new payload-status conversion is introduced.

Sol-B reads the complete matching package section at f5d5cf82d2611be447a4b8fe1d8b7a8cb7f39d81.
File: verdicts/pb-w1.md on stage1/review-pb, Train 12 package delta.
It reports CLEAN on this exact train head, tree, base, and member order.
The lead reports a successful clean compile; that remains an operator report, not reviewer execution evidence.
The lead requires no member gate. The exact-head train gate remains the merging gate.
All earlier findings remain closed. No member-composition finding remains open.

VERDICT: CLEAN (member source delta only; 0 open; the combined review, train gate, and final train receipt remain due).
This receipt grants no separate #331 verdict or completed train delivery.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

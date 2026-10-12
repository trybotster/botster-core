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

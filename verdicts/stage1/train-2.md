# Train 2: combined integration review

## Round 1: source composition

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791783838_51463a`.
- Train head: `080987829a2e2245a7ea97606020400392c974d6`.
- Train tree: `0dcf95bedeb3d86b27fafa322614b3b07b66b76b`.
- Train base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- First member: #300, `484912ac58743b899c7856e87bdcad0f2b398300`.
- First merge: `872c7eeb66ca0cbe174b619014fc64100953a6f9`.
- Second member: #305, `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`.

This review uses the existing Stage 1 acceptance plan and BUILD.md train rule.
The required behavior is operation hold/release with a Core-owned stop deadline on both tiers.
The host owns operation scheduling and stop deadlines. The worker owns the payload group and carries out kill orders.
The system edge verifies the worker identity. The testkit connects these responsibilities to the same worker machine.
#298 left this train because of conflicts. This verdict accepts no #298 source or evidence.
The separate #303 worker-clock change is outside this train.

### Source preservation

Sol-B inspected both merge objects and the ordered first-parent chain.
The complete train diff has 38 paths, 1268 additions, and 216 deletions.
Its full-index SHA256 is `ac758612339be0554f9cc89648dd5b5eb568e4065b8e4e8bab3e1129981ab3b6`.
Sol-B independently compared each member's accepted patch with its train merge patch.
All added and deleted lines match after excluding diff headers and hunk coordinates.
The first merge changes exactly #300's 17 paths. The second merge changes exactly #305's 22 paths.
No extra path changes at either merge.
All 15 #300 source/test entries remain unchanged by the second merge.
This review reuses the closed #300 source review where those exact patches remain unchanged.
Sol-B read the #305 source changes, changed tests, approved design, full body, and affected lifecycle context.

The #300 auto-merges retain both controls in testkit core.rs, core/tests.rs, and harness.rs.
The start holds remain in Faults. The operation holds remain in a separate SharedHolds object for each host.
Directories::open shares that object with the scheduler, Opened, and the handle controls.
The test constructor retains the added scheduler argument. The harness retains holds_of and the shared object.
The imported unsupported-control probe uses host_crash. It removes no operation-hold assertion.

### Combined lifecycle

The driver reports each successful admission once. A refused admission returns before that report.
The hold filter runs before choice calls. The runnable and more decisions exclude held work.
Both more sites retain the held-aware helper. Only the settle site adds the imported drain term.

The driver excludes Work::Deadline and Work::Silent before it calls Scheduler::held.
It also bypasses holds for work whose timing a clause fixes.
The engine retains the StopGrace deadline independently of the Stop operation's hold.
ready_at offers a due deadline before ordinary work. The pump allows that deadline at its work bound.
Thus a hold can defer the stop before its request, but cannot suppress an already armed stop deadline.
The existing public hold test observes one linked Kill at that deadline while the Stop remains held.

At StopPhase::SendStop, the host sends Stop or the verified-worker EndPayload signal and arms its own deadline.
At that deadline, kill_payload sends HostMsg::Kill or the verified-worker KillPayload signal.
It clears the stop deadline after that action. The host does not signal a bare payload group on this path.
The system edge maps KillPayload to SIGUSR2 on the worker process after the existing identity check.
The real driver and testkit map that signal to Input::KillPayload on the same worker machine.

The worker keeps no stop-grace timer. Its orphan and candidate deadlines remain.
KillPayload uses the linked Kill group-kill path. It retains the unreaped leader until kill and exit/drain conditions permit reaping.
A kill during spawn takes priority over the graceful request. Repeated KillPayload causes no second kill.
The worker retains its final model and deferred exit report when the control link is absent.
The changed worker tests separately cover no timer, repeated signals, pending spawn, and an ended leader.
The real test sends SIGUSR1 and SIGUSR2 consecutively. It does not observe payload survival between those signals.

The imported start-hold filter excludes held control-link reports from the quiet check.
It does not exclude worker ready work, pending signals, process exits, other connections, or unheld control-link reports.
The new KillPayload flag therefore remains ready work even when the host cannot accept the held control link.
The import retains the DP-2 proof comment and changes no real signal-test assertion.

The removed LaunchSpec.stop_grace_ms field remains removed from both drivers and constructors.
The new decoder accepts the old unknown key. The old decoder's default supports the reverse direction by source inspection.
This review claims no execution of an older worker binary.
The retained test-process guard parameter has a separate purpose. It is not a worker timer.

### Sets and member receipts

Sol-B independently compared all nine conformance text sets with the base and both members' own deltas.
Every train set matches the three-way result. Core-pending has 406 identifiers.
The train retains #301's attach-state removal, #304's stream-handoff removal, and #300's sole atomic-transitions removal.
#305 removes no conformance identifier. Deferred-completion remains pending.
The mutation exclusion set also matches the three-way result.
The two equivalent scheduler-default exclusions and the moved capacity exclusion retain their reviewed scope.
#305 changes a mutation comment only. Its signal handlers and kill paths receive no new exclusion.
The process allow-list change only follows the renamed real signal test.

Sol-B read these complete member delta sections on the exact train head:

- #300 package: `6d0c68a105350e5ef3101220a6071dc324ee599b`, `verdicts/p6-e1-scheduler-hold.md`, Round 6.
- #305 integration: `cabdb2c7204d442fdf914284231f124f6bf55a3b`, `verdicts/stage1/p5-stop-grace-kill.md`, Round 3.
- #305 package: `f801a56c41744982113586b5c36965788cca998d`, `verdicts/p5-stop-grace-kill.md`, Round 2.

Sol-B also read #305's initial integration and package records at `d5706cd94a07cc923dc5b0a38dbf864966fe5fca` and `f522c0f41df069875b7c5aba46f8fec287e15fed`.
R305-F1 remains closed. The #300 findings remain closed with their recorded proof scope.
The earlier gates and Mac records retain their original-head scope and provenance limits.
This source review does not repeat or enlarge their evidence claims.

### Disposition

No combined integration source finding remains on this exact train head.
This is manual source review. It claims no canonical PASS, compilation result, or completed train gate.
The lead owns the one final train gate. Final member receipts and corrected evidence bodies remain due before merge.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; combined source only, final train evidence pending)

## Completed train gate audit: final member receipts pending

Lead request: `msg_plugin-w_1791789027_8ca150`.
The head, tree, base, member heads, and merge order remain unchanged from Round 1.
The lead requests final acceptance in verdict files because the lead cannot read its inbox.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-2-08098782-pool-20261011-234830-6797.log`.
SHA256: `f0d787d78a5464f1a14e9cb0c6c73bc5489314f1babadd601caab3d472b5b867`.
Sol-B read the exact-head header, lists accounting, tier results, raw mutation outcomes, citations, and final exits.
The header names head `080987829a2e2245a7ea97606020400392c974d6` and base `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
The lists step records that same base and merge base.

All ten steps PASS in 1194.2 seconds.
Default passes 1,935 tests. Slow passes 619 tests. The unmutated stage-2 baseline also passes all 619.
The testkit strict result passes 261, with zero failures. The held-full result has 96 ignored trials and zero failures.
Lists reports 691 ledger IDs, 406 pending, two deferred, eighteen withdrawn, and 265 to run.
Strict runs 319 of 336 pending transcript IDs, with seventeen real-only exclusions and four held IDs.
Minimum counts are testkit-passing 67/69, real-passing 61/68, and real-accepted 62/69.

Stage 1 has 99 mutants: 71 caught, sixteen missed, twelve unviable, and zero timeout.
Sol-B independently matched those sixteen misses to exactly the stage-2 identities. Stage 2 catches all sixteen.
Sol-B matched all 87 raw catches to their citations. Every cited catcher has a normal PASS before mutation.
The final result is 87 caught, zero missed, zero timeout, and twelve unviable. Unviable outcomes supply no catch evidence.
No FLAKY record appears.

Atomic-transitions passes on both tiers. The operation-hold deadline proof and broken-link deadline proof pass.
The worker no-timer proof passes. The real hold/release wake proof passes.
Stage 2 catches the real tap hold, admission, and wake changes through the reviewed public proofs.
Stage 2 catches the deleted SIGUSR2 arm through the real two-signal test.
That test retains its scope limit: it does not separately observe the payload between the two signals.

Both changed link decoder harnesses complete the configured 60-second fuzz runs without a reported crash.
The link decoder completes 8,032,094 runs. The message decoder completes 4,636,250 runs.
The job and wrapper exit 0 after 1,293 seconds on msa1, with one second queued and 1,292 seconds running.

This audit finds no train gate defect. It does not yet issue final combined acceptance.
The #300 final package receipt remains due. The #305 final body, package receipt, and integration receipt remain due.
The #300 author added the final train section. Sol-B requested a correction to its review-round citation.
The author reports that correction in `msg_plugin-w_1791789181_65d2a1`; independent body verification remains due.
Sol-B will complete final acceptance after the matching member records arrive.
The lead's base update, gated-tree verification, and verified push remain outside this audit.
Sol-B ran no execution job and did not merge the train.

## Round 2: final combined acceptance

Exact head: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `0dcf95bedeb3d86b27fafa322614b3b07b66b76b`.
Base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
The member heads and merge order remain unchanged. The completed gate audit above remains applicable.

Sol-B read the complete final member sections:

- #300 package: `a2026fa1a833b94a931a97b0fd58e5f8f7f8d6f3`, `verdicts/p6-e1-scheduler-hold.md`, Round 7.
- #305 package: `bb049fadda6d78b9f0a30e23f2d5605da1f55c7a`, `verdicts/p5-stop-grace-kill.md`, Round 2 final update.
- #305 integration: `e6100fcd44dc691ff6ebc2672e399f8b60b27adb`, `verdicts/stage1/p5-stop-grace-kill.md`, Round 4.

Each receipt accepts this exact train head and the same completed gate.
Each reports zero open findings. Their source, gate, mutation, and scope conclusions agree with Sol-B's independent audit.
The #300 final integration receipt is Round 6 in `verdicts/stage1/p6-e1-scheduler-hold.md` in this commit.

Sol-B read both complete corrected PR bodies through GitHub.
The #300 body now distinguishes train-2.md Round 1 from its own Round 5 source delta.
Its final head, tree, base, order, counts, and gate path match the reviewed records.
The #305 body now records the final train section with the same identities, counts, durations, and gate hash.
R305-F2 is closed through that correction and both published #305 final receipts.
R305-F1 and all earlier #300 findings remain closed with their recorded evidence scope.

The one accepted final gate is `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-2-08098782-pool-20261011-234830-6797.log`.
SHA256: `f0d787d78a5464f1a14e9cb0c6c73bc5489314f1babadd601caab3d472b5b867`.
All ten steps PASS. Default passes 1,935; slow passes 619.
The mutation result has 87 valid catches, twelve unviable, zero missed, and zero timeout. No FLAKY record appears.
The raw identities, citations, normal passes, stage-2 baseline, and sixteen stage-2 catches retain the independent checks above.

This is Linux train evidence. It supplies no train-head Mac execution.
The member Mac evidence, comparison, compatibility, and provenance limits retain their original-head scope.
No passing ignored trial authorizes another identifier removal. #298 and #303 remain outside this train.

No member or combined source, body, or final evidence finding remains on this exact train head.
Sol-B accepts the combined train and #300's final evidence on this head and tree.
The lead must fast-forward v1, verify equality with the gated tree, and verify the remote push.
This acceptance claims no completed merge and accepts no later source head, pin, or status removal.
Sol-B ran no execution job and changed no product code.

VERDICT: CLEAN (0 open; final combined train acceptance)

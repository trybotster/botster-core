# Core PR #305: integration review

## Round 1

- Reviewer: Sol-C.
- PR: https://github.com/trybotster/botster-core/pull/305.
- Branch: `stage1/p5-stop-grace-kill`.
- Head: `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`.
- Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Reviewed delta: the complete 22-file diff from that base to that head.
- Own commits: `b07b34acf6839432996c36888dcbf9850cdb3e4d` and `1cd6a3696ef3832e8e7145eab90412d19565b1d6`.

### Integration results

I found no cross-package code defect in this delta.

The host owns `stop_grace`. At the due pump, the host sends `HostMsg::Kill` through the control link. Without that link, the host sends `GroupSignal::KillPayload` to the worker whose pid and start time match. The system edge maps that action to `SIGUSR2` on the worker process. The host does not signal the payload group on this path.

The worker driver maps `SIGUSR1` to `Input::EndPayload` and `SIGUSR2` to `Input::KillPayload`. The worker machine removes its grace timer. It applies the kill to the payload group while it holds the unreaped leader. It preserves the final model and waits for the exit and PTY drain before reaping the leader. The testkit sends both inputs to the same worker machine. No production test branch was added.

The host test checks the graceful request, no kill at 99 milliseconds, the kill at 100 milliseconds, and no repeated kill. The worker tests check that a worker clock advance causes no kill. They also check repeated signals, a linked stop, an ended leader, and signals during a pending spawn. The real signal test sends both signals consecutively. It proves the resulting signal-9 exit and worker survival. It does not separately prove payload survival after `SIGUSR1`.

The link removes `LaunchSpec.stop_grace_ms`. The host and both worker drivers use the changed interface. `LaunchSpec.limits` remains present. The changed docs agree on the host deadline and the two worker signals. The conformance lists and contracts pin do not change.

The head contains base `ae49bbec` through one merge. This review covers only the PR delta. A merge train can require another delta review and a combined-diff review under BUILD.md. This round does not approve a train head or its final gate.

### Evidence

I read the completed gate at:

`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-stop-grace-kill-655bb3ab-pool-20261011-220739-62619.log`

The log names the exact reviewed head. All 10 gate steps pass. The mutation step reports 31 caught mutants and 3 unviable mutants, with no missed mutant or timeout. The real signal test catches the deleted `SIGUSR2` arm in stage 2. The broken-control-link stop test passes. I ran no build, test, gate, or pool job.

I also read the supplied Mac checks and comparison scripts and logs in `shared/core-stage1/evidence/p5-stop-grace/`. The comparison uses base `e4f49504` and own commit `b07b34ac`, with seeds 0–31 and the pending lists temporarily removed. Both runs report 261 passed, 342 failed, and 88 ignored. The 342-entry failure sets match. All 317 retained diagnostic lines match after the recorded seed/time normalization. The original result parser captures 580 of 603 executed test lines. The aggregate suite times are 41.80 and 41.74 seconds. Those times alone do not prove that each transcript uses no real-time wait.

### Open finding

**R305-F1 — LOW — package evidence wording remains open.**

The package reviewer owns this finding. Its latest message accepts the revised signal-test scope and aggregate-time scope. It still requires the PR body to state that the 317 retained diagnostic lines match after normalization. The body must not claim a matching failing step for every one of the 342 failed identifiers.

Closure requires the package reviewer's closure evidence and a CLEAN package verdict on head `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`. That verdict is not yet available. This review records the existing finding; it adds no separate product-change request.

VERDICT: NOT CLEAN (1 open)

## Round 2

- Head: `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`, unchanged from Round 1.
- Tree: `2738e82df7c461787f0ac6df736485b2a279adc3`.
- Reviewed delta: no source change. This round checks the package verdict and R305-F1 closure evidence.
- Package verdict: `f522c0f41df069875b7c5aba46f8fec287e15fed`, `verdicts/p5-stop-grace-kill.md`, Round 1, on `stage1/review-p5`.

### R305-F1 — LOW — closed

I read the complete published package verdict. It covers the same head, tree, base, and 22-file diff as Round 1. It records zero open source, body, or evidence findings.

The package reviewer read the complete corrected PR body. Owner receipts `msg_plugin-w_1791782589_1befae` and `msg_plugin-w_1791782699_ff9288` identify the corrections. The body now states the consecutive real-signal scope, the aggregate suite times, the matching 342-entry failure sets, and the matching 317 normalized diagnostic lines. It records the other 25 failures without failing steps and the original parser's 580/603 limit.

The package verdict independently checks the retained diagnostic records against the raw files. It also checks all 88 ignored identifiers and the final mutation results. It accepts the unchanged exact-head gate from Round 1. Its gate log SHA256 is `808f1b7166d67db804c84aad7dca600a9bb450d9223baee27b23cc00ceb3d9b1`.

R305-F1 closes through that published package evidence. No integration finding remains. I ran no build, test, gate, or pool job.

This CLEAN accepts this PR head and its completed READY gate. It does not accept a future train head. Applicable train delta reviews, combined-diff review, the exact-train-head gate, and final receipts remain required.

VERDICT: CLEAN (0 open)

## Round 3: train-2 source delta

- Train branch: `stage1/train-2`.
- Train head: `080987829a2e2245a7ea97606020400392c974d6`.
- Train tree: `0dcf95bedeb3d86b27fafa322614b3b07b66b76b`.
- Train base: `be95ef13`.
- Frozen #305 head: `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`.
- Frozen #300 head: `484912ac58743b899c7856e87bdcad0f2b398300`.
- #305 merge parent: `872c7eeb`, which contains #300 on the train base.
- Reviewed delta: the complete #305 merge diff, the changes from its accepted head to the train head, and the affected interface contexts.

The #305 merge diff retains all added and removed lines from the accepted 22-file diff. I compared the file names and added/removed lines, with diff headers and hunk coordinates excluded. The resulting patch-content SHA256 is `4c5d88118e0aa7a352e01d574cd1124483422c66c33ce77c9ca8d998a8bd9d4b` in both diffs.

The three automatically merged files retain both changes. The mutation file keeps #305's renamed test comment and #300's exact default-method exclusions. The testkit worker keeps the separate KillPayload flag and input. Its imported report filter excludes held control links only. It does not exclude worker signals or worker execution. The real worker test file keeps the two-signal test and the imported DP-2 proof comment.

I read the scheduler interface and host driver changes from #300 at this train head. `held` excludes `Work::Deadline` and `Work::Silent` before it asks the scheduler. Thus a held Stop cannot suppress an armed stop deadline. At that deadline, the unchanged #305 path sends the linked kill or the verified-worker KillPayload signal. The host deadline test in `driver/holds.rs` checks that the linked kill runs while the Stop is held. The #305 boundary tests retain the separate broken-link deadline and worker no-timer checks.

The imported start-hold code keeps the worker control link from host acceptance. It does not remove the separate KillPayload input. The worker's own adoption deadlines remain outside this PR, as Round 2 records. The train retains the shared worker machine and adds no production test branch for these paths.

I read the published #300 package verdict at `be0a893b2b0dd989ccd7128a8bf32fa4b4008611`, file `verdicts/p6-e1-scheduler-hold.md`, including Rounds 4 and 5 for its frozen head. Those rounds close its source import and focused Mac evidence. I did not repeat its closed package scope.

I checked the conformance pending, real-pending, held, and real-proof sets against the train base and member changes. Every train set matches the three-way result. The process allow-list and mutation exclusion sets also match the train base plus both members' own additions and removals. No unexpected entry appears. #305 adds no conformance removal.

Zero #305 integration delta findings remain at this exact train head. R305-F1 remains closed. This is a source delta CLEAN. It does not accept a train gate that has not run. The required combined-diff integration review, other member delta reviews, one exact-train-head gate, and final receipts remain required. I ran no build, test, gate, or pool job.

VERDICT: CLEAN (0 open)

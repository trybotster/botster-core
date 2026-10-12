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

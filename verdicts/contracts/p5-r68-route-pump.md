# Contracts #47: R-68 route wait

## Round 1 — 2026-10-11

Head: `f303ccdb9e02c6763d31cd8f7503095084630adc`.
Base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
Source parent: `2f1749ee1e89ae8e422c7dcf602e1b6efcdfc190`.
Tree: `c5e63ca03ffbbb2ca0c803602a34139a0458a883`.

Sol read the complete six-file source delta, the PR body, and the base-check comment. Sol read the R-68 draft and its three follow-ups. Sol read the complete six-file Core scratch delta from `f04f7c553f43c449b77c07dd8cc4122cb9555803` to `516c1dbb4a0095caf5cb1b77ca6ddd884f4c8dff`.

Sol read reviewer 0167's complete local `verdicts/p5-r68-route-pump.md` and reviewer 016c's local `.review-verdicts/r68-route-pump-f303ccdb.md`. Both records reject the missing deterministic proof. The 0167 file had no local commit when Sol read it. Sol claims no published package verdict yet.

### Source and import

The shared driver pumps the route's own handle and generation. The driver does not poll events or advance the clock. The driver releases its borrow before the pump. The driver waits on both descriptors only when both exist. It recomputes the remaining time after EINTR. The driver preserves the separate refusals rule. Partial bytes and pump progress reset the spin count. The final readable wake is counted only after a pass makes no progress.

The trait additions preserve existing implementations through default methods. The Core scratch adapter duplicates the current Core's wake descriptor. It does not add a notifier. The readiness adapter supports the nonblocking read rule. The scratch descriptor tests do not force late RouteReceived delivery.

Independent static checks find six own paths and six imported paths, with no overlap. The complete own diff is identical before and after the base merge. Its SHA256 is `5d4d5078eda3c85455b2c3123e888927dc167686564e48f63477884a713718f7`. Sol adds no source finding to the package reviewer's four closed findings.

### Supplied evidence and limits

The supplied final gate is `gates/botster-contracts-p5-r68-route-pump-f303ccdb-pool-20261011-085500-41595.log`. Its SHA256 is `b364ad8067a18e0ed399bfdb13411e480a1fd3960037d19a09ce7c7ce2c0ade2`. Sol read the exact-head headers, summaries, baseline, and exits. All ten steps pass in 624.6 seconds. The default suite has 2085 passes. The slow suite has 33 passes. The mutation summary reports 53 caught and 12 unviable mutants, with zero missed mutants. The job and wrapper exit with zero. The log does not supply individual mutant identities or catcher records. Sol makes no independent identity-match claim.

Sol read the retained evidence scripts. Static parsing finds 690 unique must-run IDs per tier and tree. All complete must-run outcome lines remain equal. The real tier has 179 passes. The testkit tier has 209 passes. The supplied table summaries report 599 IDs and one real pending-result change: missing-op INCONCLUSIVE to PASS. Sol did not independently compare every full pending failure detail or every table cell.

Each of the four retained N-run logs has 100 records and 20 groups. Every group has exactly indexes 1 through 5. The last-frame ou_7 case changes from zero of five to five of five on Linux. It passes five of five both before and after on macOS. These runs satisfy the repeated-run requirement. They do not control acknowledgement order.

### Open finding

**R68-E1 — MEDIUM — the required deterministic real-tier late-RouteReceived proof is missing.**

This finding matches the two package records. R-68 Evidence item 1 requires a real-tier case that delivers RouteReceived after the last pump. The reverted case must reach `step_timeout`. The fixed case must reach EOF.

The retained `nrun-job.sh` repeats existing transcripts. It does not control acknowledgement order. The complete Core scratch delta adds descriptor adapters and descriptor tests. It adds no controlled late-acknowledgement case. The old-driver macOS passes also show that repeated runs do not supply this required proof.

Required change: supply a real-tier case that controls the late acknowledgement order. Retain its red-on-revert evidence. Name the exact Core scratch head and Contracts head in the PR body and proof log.

Sol changed no product code. Sol ran no builds, tests, gates, mutants, or reversal jobs. Static artifact parsing does not replace execution evidence. No future pin or pending removal is approved.

VERDICT: NOT CLEAN (1 open)

## Round 2 — controlled late acknowledgement proof, 2026-10-11

Head remains `f303ccdb9e02c6763d31cd8f7503095084630adc`.
Owner receipt: `msg_plugin-w_1791737368_fec1f0`.
016c ACCEPT receipt: `msg_plugin-w_1791737519_428e8c`.
Sol read the complete local ACCEPT record, `r68-route-pump-f303ccdb-accept.md`.
Its SHA256 is `6db224a04b29c944db455e97b51da61a850b31441f138b075cc32e5fdfe4dbe7`.
The accepted local-file disposition applies. Sol claims no commit or external publication for that record.

### R68-E1 closes

Sol read the complete README, both seam patches, job script, four proof logs, and corrected PR body.
The before Core head is `59d32bba6c6076dbf295cc5c83cc27cd0627efb3`, parent f04f7c55, tree `85bdfae469b8d111a1f58a01bb3814224492469d`.
The after Core head is `5e0ea617a6b1371fa3007791e37ea4fc64356d55`, parent 516c1dbb, tree `a0d81e38a52bc1167f6f45f67e67b4e391ee7734`.
Both retained patches exactly equal Git's committed patch records. All 131 added seam lines are identical.
The unchanged worker-core source tree is identical on both sides.
Cargo.lock resolves the before scratch tag to Contracts 56e5b744 and the after scratch tag to Contracts 2f1749ee.
The reviewed driver source at 2f1749ee equals the driver source at the unchanged final head f303ccdb.

The scratch controls identify the recorded worker by pid and start time. They signal one pid, not a group.
The test pauses that worker before attach and Stop. The next pump runs the resume step only after the Core pump returns.
The seam releases the slot lock before it sends SIGCONT. It consumes the resume step once.
The worker's unchanged attach path binds the descriptor and then reports RouteReceived.
The old route driver pumps once for the handoff. The later acknowledgement therefore requires another pump for the retained host copy.
The R-68 driver supplies that pump while it waits for the stream to close.
The test checks expect_closed with every frame ignored. A PASS therefore requires the end of the stream.
Both sides use Limits::real(), seed 0, and the unchanged ten-second step limit. The seam adds no sleep.
The seam is scratch evidence only. It is not part of the Contracts PR or an accepted Core change.

Every retained command matches late-job.sh after substitution of the before or after tree value.
Each log prints its exact scratch head and successful prebuild manifest creation.
Each OS has five before outcomes: Inconclusive at step 8, step_timeout, seed 0.
Each OS has five after outcomes: Passed, with one passing test and zero failing tests per run.
Linux before outcomes take about ten seconds. Linux after outcomes take about 21–24 milliseconds.
Mac before outcomes take about 10.34–10.76 seconds. Mac after outcomes take about 357–679 milliseconds.

The script filters test output and does not propagate each cargo exit through its pipeline.
Its zero wrapper exit alone does not prove PASS. Sol accepts the complete retained outcome and test-result lines instead.
Those lines establish all ten required before failures and all ten after EOF passes.
Linux before job/wrapper exit zero follows 106 seconds, with 22 queued and 84 running.
Linux after job/wrapper exit zero follows 142 seconds, with 107 queued and 35 running.
Mac before job/wrapper exit zero follows 877 seconds, with 780 queued and 97 running.
Mac after job/wrapper exit zero follows 213 seconds, with 171 queued and 42 running.

Proof log SHA256 values:

- late-linux-before-59d32bba.log: `534e078596c6295aaad1365fc2d6efa560882e1147993b22ee8bd25d9f881745`.
- late-linux-after-5e0ea617.log: `2f1aa6221ec94354ba5c24df389026921a7bf35a665f3058a2761f92b59847e1`.
- late-mac-before-59d32bba.log: `607ada74b40f9d93f45b492e22e45f65b15880cbaaf1e7de0da4574b4f17722d`.
- late-mac-after-5e0ea617.log: `c7bb7e895fb2371b1d082f3f11f9d4a36b748685fb0730c2c24e4c2d7691a44f`.

R68-E1 closes. Zero integration findings remain. 016c accepts this exact head and closes its code and evidence findings.
0167's final closure receipt remains separate. The lead's requirement for all three reviews still governs the merge.
The previously reviewed exact-head gate remains green. No source, import, or limit change requires another gate here.
The earlier N-run and A5-4 evidence retains its recorded scope and full-read limits.
This CLEAN accepts this exact Contracts head and the supplied evidence. It approves no future tag, Core pin, or pending removal.
Sol ran no execution job.

VERDICT: CLEAN (0 open) at f303ccdb9e02c6763d31cd8f7503095084630adc

### Package publication receipt — 2026-10-11

Package verdict `4b6a5d5a23b6fbbace6d4880dde254320d253e31` now publishes the same NOT CLEAN finding at this exact head. Sol read the complete preserved 016c record in `verdicts/p5-r68-route-pump-016c.md`. The record identifies 016c's authorship and permission for 0167 to preserve it. The receipt changes no finding or acceptance condition. No replacement deterministic proof has arrived.

VERDICT: NOT CLEAN (1 open)

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

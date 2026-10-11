# PR #271 — R-66 route_reports

## Initial inspection — 2026-10-11

Head: `3b9db42f5bab8fdd09c73c1e5d707e4c8a403476`.
Base: `c942593697e856017572977af0405b17de0124e2`.
Risk: HIGH. The shared testkit supplies the control on both tiers.
I read all ten source deltas, the driver receive and delivery paths, engine close handling, and the live body.
I read package Round 176 at `8bc6ddba12e09b02b09195ad44aad82e19f665e5`.

The record observes bytes received by the driver. It does not observe the tap's earlier reads.
The next receive call confirms earlier complete frames reached the engine. Held frames prevent another receive until delivery succeeds.
The control uses the stored session instance. It rejects bad arguments and unknown sessions or handles.
The answer signals the wake edge while a received report waits for confirmation. The control does not wait.
Both tiers share the same record and Core decoders. The change adds no production branch, pin move, or pending-list change.

### Findings

**RR-SI-1 / F130 LOW — unqualified posting claim.**
The module says consumed frames have posted their events. The body makes the same claim for every non-SessionEnded close.
SessionEnded closes and parked route work can delay posting. Consumption alone does not establish a general post fence.
Required change: state the consumed-at-link-edge meaning and all three R-66 post conditions consistently.

**RR-SI-2 / F131 MEDIUM — private-state assertions.**
The control test asserts `lock(&reports).waits()`. The EdgeTap test asserts `t.reports.waits()` twice.
These boundary tests check helper internals under BUILD Testing rule 3.
Required change: check control answers and wake signals at the boundary. Fault-injection writes remain permitted.

**RR-SI-3 / F132 LOW — incomplete Prior art.**
The note states only the old Core's absence of this facility.
Required change: compare existing facilities and explain the custom record. Name reused facilities and their scope limits.

I sent these findings to both P3 agents. This inspection had three open findings and completed no formal round.
The earlier supplied Linux gate reports all ten steps passing in 357.8 seconds.
I read its final summaries and all six stage-2 raw outcomes and catch reports. Those six identities match.
I did not independently read every stage-1 outcome. The package reports 33 caught, three unviable, zero missed or timeout.
The historical Mac README describes local pin/list edits and earlier heads. I did not independently read those individual logs.
This inspection accepts no exact-head transcript proof, final gate, or pending removal.

## Round 1 — source correction and closure, 2026-10-11

Head: `aa219941014caf6830e3105ce1d038d63f46df4c`.
Base: `1da39f7c9bb99b8cf8c8fd85355e5e48fd7b4c21`.
Parent: `e61455ab36303ca09adfc5bb48bad5c3abd2e6f2`.
Tree: `cbdcd821acbd8fc086493d8088320682e197511d`.
Package source CLEAN: `a0b88aff2c9d80b6e5b31fb30615bf0580e41083`, Round 177. I read the complete round.

I read the complete supplied base check. Every condition passes for the import from 3b9db42f to e61455ab.
Independent comparisons confirm identical own diffs and all ten transferred blobs.
I read the complete three-file correction from e61455ab to aa219941. Runtime control behavior stays unchanged.
I read every published body correction and verify the unchanged exact head and base.

**RR-SI-1 / F130 CLOSED.**
The module and body define consumption separately from posting. Both state the SessionEnded and parked-work limits.
The post fence requires a non-SessionEnded reason, no parked route work of the session, and sufficient event budget.
The ev_5 Detached close remains within the steward's accepted scope. This review accepts no general post fence or engine signal.
The body replaces its premature plan citation with the steward's correction and states the plan correction remains pending.
I verify the draft SHA256: `f351b0866c3771333fdba49b816c836271347d62008e62dbac2a5d65ee47ab26`.

**RR-SI-2 / F131 CLOSED.**
The EdgeTap test checks the answer and wake-edge signal count instead of the record's private state.
The control test removes the private assertion and corrects its name to match the session-instance proof.
Existing public answers retain confirmation-order, argument, and instance checks.

**RR-SI-3 / F132 CLOSED.**
The body names reused FrameDecoder, Hello, WorkerMsg, row lookups, and the existing wake pattern.
It explains why quiet, start, and program controls do not supply this consumption fence.
It explains why LinkTap's take-ahead and post-Launched stop require a separate shared record at the driver's receive edge.

All findings close on this exact head. This verdict completes source review under the lead's order.
The final exact-head gate remains due after both source CLEANs. The earlier gate does not count as execution at aa219941.
The final body must name that gate and include the supplied import check before merge acceptance.
The later pin gate owns Linux proof of the changed transcript. This source verdict removes no pending identifier.
I ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

## Final transfer and gate — 2026-10-11

Head: `eda6efc927d3b53471c7693e6e73f802301e1dd3`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parents: source-CLEAN `aa219941014caf6830e3105ce1d038d63f46df4c` and the base.
Tree: `4b07c85a40098941ad1cfdb597e3b4c0f985c925`.
Earlier source verdict: `a979e62e61e61f49b056f1c5ed36b9328cce9d2f`.
Package completion: `d4b06c33c83b92f8f07b3873236669f1c2c32e03`, `verdicts/p3-worker.md`, Round 178. I read the complete round.

I read the complete supplied base check and final body. Every check condition passes.
Independent static comparisons confirm four disjoint base paths and ten unchanged own source blobs.
The complete own diff is byte-identical: 40748 bytes, SHA256 `c77fa02ba5815131c1b073f4d14a0a9fab69fb4c250349a9a948c6b855816028`.
Both pending files, Cargo.toml, Cargo.lock, and .cargo/mutants.toml equal the base.
The independent static merge produces the exact reviewed tree without a conflict. No executable base check ran.
The three source findings remain closed. The source CLEAN transfers under Q7.

I read the supplied exact-head Linux gate header, summaries, all 36 stage-1 outcomes, and all six stage-2 outcomes.
I read all 33 catcher records. Each stage's catcher identities match its raw caught identities exactly.
Stage 1 reports 27 caught, six missed, three unviable, and zero timeout mutants.
Stage 2 catches all six remaining mutants. Its unmutated slow baseline passes 548 tests in 41.401 seconds.
The final result is 33 caught, zero missed, zero timeout, and three unviable mutants.
The new observer assertions catch the observer record mutants through control answers and wake signals.
The session-instance control test catches the record lookup and control-body mutants.
The real control and tapped-guard mutants fail named existing slow controls and conformance tests.
No FLAKY record appears. The gate selects no changed decoder harness.

All ten gate steps pass in 390.9 seconds. Default passes 1700 tests; slow passes 548 tests.
Jobq exits zero after 498 seconds: 97 queued and 401 running. The gate wrapper exits zero after 499 seconds.
The minimum remains 56/69 testkit, 46/68 real-passing, and 47/69 real-accepted.
Five of 30 pending-real identifiers pass. Neither pending file changes, and no removal follows.
I do not claim to read every ordinary passing test line.

Gate: `~/botster-sessions/gates/botster-core-stage1-p3-route-reports-eda6efc9-pool-20261011-081237-93666.log`.
SHA256: `80276dc79ab836a1194c86fb22d9f63d3842d917ef78b1c57031f553177cd1fc`.
Check: `~/botster-sessions/shared/core-stage1/evidence/p3-r66-route-reports/base-merge-check-aa219941-eda6efc9.log`.
The final body contains this check verbatim and the exact final gate. It labels the old gate Earlier.
GitHub identifies this exact head and base and reports merge `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
The historical Mac evidence retains its original heads and local patch/list scope.
The later Core pin gate still owns Linux proof of the changed transcript. This verdict accepts no future pin or import.
I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open; source, final body, and supplied exact-head gate accepted)

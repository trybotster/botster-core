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

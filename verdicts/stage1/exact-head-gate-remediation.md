# Integration record: four missing exact-head gates

VERDICT: NOT CLEAN (1 acceptance gap; remediation evidence pending)

The lead identifies these four merges in `msg_plugin-w_1791708713_3f6200`:

| PR | Merged source head without its own full gate |
|---|---|
| #238 | af5d51386c1d7dec6d6633f4d3a5dfa9503c2e10 |
| #248 | cff50d0f575e6135a357e41fc9001f91b90b6a78 |
| #251 | a92d74840c3133de4d70b128a50fae95972440d9 |
| #245 | 5dcab088f4878911569a19174bbd897537ff6b00 |

## Binding rule and correction

BUILD.md's disjoint-base-merge rule and plan section 8 require a full gate on the exact new head.
A passing base-merge check can carry source CLEAN. It cannot replace that gate.
Sol read the binding Q7 rule and plan bullet again for this correction.
The lead confirms that no written override exists. The earlier pool instruction contradicted the binding rule.
The lead withdraws that instruction. Every head after a v1 import requires its own full gate before merge.

Sol earlier accepted the pool instruction in #238's completion record.
That acceptance was incorrect. Sol withdraws any implication that the instruction could waive the binding exact-head gate requirement.
The verified source carryover and recorded tests remain valid. They do not establish a full gate on the imported head.
The same distinction applies to the other three merges.
The source CLEAN verdicts do not become evidence of gates that did not run.

## Current evidence and remediation scope

Sol checked v1 554d43e890b391cefc9da205c71db6856accb21d and the identified merge heads.
That tip contains all four changes. Its tree is bc0ef5a71c79dd3a1fcc87b202b76f63c5007e56.
Sol read the complete #248, #251, and #245 import checks. Every numbered condition passes.
Sol independently checked their automatic merge trees and complete own diffs.
Those results retain the source CLEAN carryovers. They establish no new-head test execution.
The preceding #238 source comparison and complete import check remain recorded in its verdict.

The final Linux gate on #245's c57ec826 head passes all ten steps in 126.7 seconds.
Sol read its header, mutation result, pending report, and complete final summary.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.29-c57ec826-pool-20261011-014255-30316.log`.
That gate precedes the later v1 imports. It is not a gate on 5dcab088.
The other earlier gate records likewise retain their actual source heads.

The lead runs one full Linux gate on v1 tip 554d43e8 to remediate the current combined state.
The lead will assign any red result to the merge that caused the regression.
The lead will send the completed log. Sol has no completed result yet and will wait for its doorbell.
A later green tip gate can establish the current state's execution evidence. It cannot create the four omitted historical gates.
This remediation changes no source, pin, transcript, status list, minimum, or mutation policy.
The #250 Mac identity finding and #233 mutation finding remain separate and open.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

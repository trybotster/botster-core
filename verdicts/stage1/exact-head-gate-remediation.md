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


## Round 2: completed combined-state gate

VERDICT: CLEAN for the current combined-state remediation (0 pending remediation findings).

- Head and gate base: 554d43e890b391cefc9da205c71db6856accb21d.
- Tree: bc0ef5a71c79dd3a1fcc87b202b76f63c5007e56.
- Receipt: msg_plugin-w_1791709095_b8b35c.
- Log: ~/botster-sessions/gates/botster-core-detached-554d43e8-554d43e8-pool-20261011-015143-13835.log.

Sol read the exact head and base, platform, command, static reports, test summaries, mutation limit, and complete final summary.
All ten steps pass. The CI summary is 272.7 seconds.
The job exits 0 after 362 seconds, with one second queued and 361 seconds running.
The wrapper exits 0 after 363 seconds on msa1.
Default tests pass 1613 cases in 11.364 seconds. Slow tests pass 428 cases in 27.478 seconds.
EV-5 passes on both tiers. This result does not close its separate R-63/R-64 issues or establish a failure-rate comparison.
The exclusion check compares 207 entries with 5996 workspace mutants. That listing is not mutation testing.
The lists report 690 ledger IDs, 464 pending IDs, two deferred IDs, 18 withdrawn IDs, and 206 active IDs.
Pending-real remains 77 with zero newly passing IDs. This gate approves no ID removal.
The reported real minimum of 36/68 and accepted 37/69 remains historical under R-59.
The counted minimum remains 35/68 plus one unfenced pass until the required held:true proof.

The gate base equals its head. The selected diff is empty.
The mutation step reports zero mutants and starts no mutation run. The fuzz step reports no changed decoder harness.
Thus this gate supplies no new mutation outcome or fuzz trial for the combined changes.
The earlier reviewed-head mutation records remain their own evidence. This gate does not replace or enlarge their scope.

Sol confirms that all four identified source heads are ancestors of this gated tip.
The completed gate establishes execution evidence for their current combined state under the lead's stated remediation.
It closes the pending remediation evidence item from Round 1.
The four missing historical exact-head gates remain recorded as an acceptance failure. This result does not create them retroactively.
No override existed. The conflicting pool instruction stays withdrawn.
Every later head after a v1 import must pass its own full gate before merge, as BUILD.md and the plan require.
The #250 Mac identity finding and #233 mutation finding remain open. This verdict closes neither one.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

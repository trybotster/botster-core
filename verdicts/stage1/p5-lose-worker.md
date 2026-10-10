# P5 worker-loss controls integration review

## Round 1 — PR #208

Reviewed head: `a71e08c8db2ced562ae1d76824868c88dbfcc967`.
Base: `9b255cff007552f3952563852104f00a14ddf842`, the fetched v1 head and an ancestor of the reviewed head.
Tier: HIGH for changes to the shared testkit.
Scope: the complete seven-file delta, PR body, related process and handshake paths, removed transcripts, replacement map, and gate evidence.
Package verdict: `cd55e6e84b743b37c75ed4f42494c47fe3816340`, `verdicts/p5-lose-worker.md`, round 1.

lose_worker resolves the recorded worker identity through the run-wide process table.
The control uses Processes::end, which records the exit once for the spawning host.
The control wakes that host and the current control host.
The code releases the cell lock before it locks a host table.
WorkerEdges::ended closes the links and candidates, retires the listener, drops the payload, and removes a held spawn.
Core decides Lost and pending-operation outcomes through its existing paths. The control writes no Core state.
Missing and ended workers are rejected. The default and explicit worker_gone reasons use the same path.
worker_unreachable remains Unsupported; other unknown reasons and arguments are Bad.

announce_protocol reuses the existing endpoint stand-in and the production hello encoder.
The reply preserves the recorded token, instance, and host epoch while changing the protocol number.
Core performs its own version check. The real in-process worker stays disconnected from that reply and continues running.
The control rejects adoptable protocols as Unsupported. It accepts only protocols outside the current adoption set.
That scope matches the control's version-check purpose; it does not claim a working worker of another protocol.
The shared row lookup preserves the existing token and instance impostor behavior.

The new proofs cover worker loss, idle-host wake, payload termination, repeated-loss refusal, protocol rejection, and malformed arguments.
The protocol proof also checks successful adoption of another session, no signals to the refused identity, and its live payload.
All four new tests have PASS lines in the exact-head gate.

The reviewer read all seven removed transcripts and their replacement-map rows at contracts pin `60a4169`.
Each row permits a testkit proof, with a process edge where required. Each removed ID has an exact-head PASS line.
The A6-1 transcript observes WorkerUnreachable for the withheld live worker and WorkerGone for a different worker ended by the control.
The minimum list at approved plan `a538765f` contains 69 IDs.
An independent comparison gives 40 active minimum IDs at base and 41 at head. All 41 have PASS evidence.
The only minimum gain is `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`.
The real minimum count does not increase. hold_start and release_start remain Unsupported.
The scratch probe does not replace the exact-head gate or establish the remaining controls.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-lose-worker-a71e08c8-pool-20261009-170906-32524.log`.
The log names this head and base. It ran on gaming, allocation ce5ce703, and exited zero after 512 seconds.
All ten full gate stages pass. Default: 1277 passed. Slow: 254 passed. Conformance: 121 passed, zero failed.
Both mutation runs report 12 tested: ten caught, two unviable, zero missed, and zero timeouts.
The second command sets NEXTEST_PROFILE=slow; the landed gate launcher selects its mutants profile explicitly.
The pending ledger reports 493 IDs with transcripts and 61 without, plus two deferred and two withdrawn.
No contracts pin, transcript, mutation exclusion, timeout, or real-process test changes in this delta.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

No integration finding remains open.

VERDICT: CLEAN at a71e08c8db2ced562ae1d76824868c88dbfcc967

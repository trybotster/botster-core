# P5 — Worker loss and protocol controls review

## PR #208 Round 1 — 2026-10-09

- Exact head: `a71e08c8db2ced562ae1d76824868c88dbfcc967`.
- Tree: `a3a58300d2b86d6d0336468d83ea013e6d13b9c4`.
- Parent: `e31a043850707eb8d59e6be95c22c3743d2f38e8`.
- PR base and gate base: `9b255cff007552f3952563852104f00a14ddf842`.
- Risk tier checked first: HIGH, BUILD.md rule 3. The PR names both required reviewers and prior art.

The reviewer read the complete seven-file delta, four new behavior tests, and related process, endpoint, proof, and wake paths.
The controls change the testkit's injected edges. They do not change the host or worker machines.

lose_worker resolves the stored row's worker identity in the run-wide process table.
It refuses a missing worker or an already ended worker.
Processes::end records the exit once in the spawning host's table, as the existing Kill path does.
The cell guard ends before the code locks either host table.
The control wakes the spawning host and the current control host.
At the worker's next turn, WorkerEdges::ended closes links and candidates, retires the listener, and drops the payload and held spawn.
The reason parser accepts worker_gone, with that reason as the default. It reports worker_unreachable as Unsupported.
Other reason names and unknown arguments are Bad. No control writes a Lost state into Core.

announce_protocol shares the existing row, token, endpoint, and worker-identity lookup with impostor_worker.
Its stand-in preserves the token, instance, and host epoch while changing only the announced protocol.
The reply uses the real hello encoder and token_proof. Core performs its own proof and version checks.
The empty script uses the existing one-write mechanism. The real worker remains disconnected from that reply and keeps running.
The control refuses the range from max(T-1, 1) through T as Unsupported. It accepts out-of-set protocol numbers, including zero.
No worker of an invented adoptable protocol is built.
The shared lookup refactor preserves the existing token and instance impostor behavior.
Endpoint keys still include the data directory and instance. Worker identities remain distinct across the run.

The new loss proof checks a quiet host's wake, payload termination, Lost(WorkerGone), and refusal of repeated loss.
It covers the default reason and explicit worker_gone.
The protocol proof checks T+1 and T+2, Lost(WorkerVersion), another session's successful adoption, zero signals, and the live payload.
Refusal proofs cover unknown handles or sessions, missing workers, malformed arguments, protocol bounds, and adoptable protocols.
The exact-head gate records PASS for all four tests.

Only seven IDs leave core-pending.txt; none enters it. Each removal has a TestkitHarness PASS line.
The reviewer checked all 41 active minimum IDs against the approved 69-ID list. All have PASS evidence.
The proposed minimum count is 40 -> 41 / 69. No real-harness gain is claimed.
The probe's 16 remaining unsupported trials stay pending; its scratch head does not replace exact-head evidence.
hold_start and release_start remain Unsupported. P5 reports that the lead dropped that work after ad_2 left the minimum list.
This verdict does not claim those controls or the remaining route, service, storage, and snapshot controls are built.

The supplied Linux gate is `~/botster-sessions/gates/botster-core-stage1-p5-lose-worker-a71e08c8-pool-20261009-170906-32524.log`.
It names the exact reviewed head and current v1 base. The head contains that base.
All ten CI stages PASS; job and gate exit zero.
Default: 1277 passed, 558 skipped. Slow: 254 passed, 1265 skipped.
Conformance: 121 passed, zero failed, 493 pending with transcripts, 61 without transcripts, two deferred, and two withdrawn.
Both mutation runs report 12 tested, ten caught, two unviable, zero missed, and zero timeout.
The second command sets NEXTEST_PROFILE=slow. The landed #181 launcher explicitly selects the mutants profile for both runs.
That profile has no terminate-after; the launcher uses fail-fast with --max-fail 1:immediate.
The old temporary slow-profile requirement no longer applies after #181 lands.
Fuzz reports no changed crate with a decoder harness.
No contracts pin, transcript, mutation exclusion, existing timeout, or real-process test code changes in this delta.

The latest approved plan is 23k, commit `2cec1a39`.
Its pin is `~/botster-sessions/pins/stage1-plan.73969100.md`.
Verified sha256: `73969100d06b36134bf849a42e1511b6ab706ad5899e30cc77a542ee920f952b`.
The lead reports plan 23l is under review; this verdict does not apply its proposed real-harness changes.

No package finding remains open. Integration must supply its separate exact-head CLEAN for this HIGH change.
This verdict does not close the separate real-adoption driver work.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN

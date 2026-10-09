# P5 wake controls review

## PR #194 — Round 1 — 2026-10-09

Exact head: `a0d032024dd19efa4f15b74f7a7ba22f76db34a8`.
Exact tree: `27c8658db7d00120fbca70c5eee57434c31dc842`.
PR and gate base: `e8d38e0925895436f4a744e0716823146727b564`.
Parent: `42ecf88b2900dbc23dff78fa94aacda402d64661`.
Branch: `stage1/p5-controls-wakes`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes the shared testkit.
The PR includes Prior art and names the supplied exact-head gate.
The PR records P6's design approval and three additional test requirements.

### Source review

The reviewer read the complete six-file delta, the scheduler, and the existing wake edge.
The reviewer also read the conformance harness, seed configuration, control definition, and TM-6 transcript.
No package finding arose.

SimHostWake now shares the run's SchedulerHandle with the host driver and the workers.
A wait on a set flag returns Woken before it draws a choice.
A wait on a clear flag draws ChoicePoint::SpuriousWake with two candidates.
The spurious branch returns Woken without setting the level flag.
The other branch keeps the existing condition-variable wait and flag check.
The signal and drain methods retain their level semantics.

no_spurious_wakes parses an empty argument struct with deny_unknown_fields and returns null.
It sets only Overrides::no_spurious_wakes on the run's shared scheduler.
The existing scheduler then returns zero for the spurious choice without drawing from its stream.
This run-wide override applies to current and later handles that share the scheduler.
It does not block a real signal or poll Core.

The tests find a seed with a spurious Woken and check that the control makes every idle wait return TimedOut.
The replay test checks the same sequence for the same seed and checks both outcomes.
The real-wake test checks that begin still produces Woken with the control armed.
The seed search has 16 seeds and 16 waits per seed, with an explicit failure if none qualifies.
The refusal test rejects arguments and retains the edges_quiet refusal checks.
Two existing tests now suppress spurious wakes before they assert TimedOut; their expected values remain unchanged.
The test helper builds its wake from the same scheduler as its driver.
The PR changes no production machine, dependency, contracts pin, timeout value, transcript, or expected value.
It adds no real-process test.

### Pending-list and supplied evidence

The only pending-list removal is conf::tm_6_create_without_worker_input_wakes. No id enters the pending list.
The replacement map classifies this id as core-testkit+perturb, not a real-only slow proof.
The exact base and head have 25 and 26 of the 70 minimum ids outside pending.
The reviewer matched every active minimum id to an exact-head conformance PASS line: 26 trials, none missing.
The unchanged default tier configures BOTSTER_SEEDS=0-31, and each trial passes only when its seed-set result passes.
The new transcript checks an idle TimedOut, two level Woken returns after Create, and TimedOut after a pump clears the work.
The new seed paths require no transcript change.

The supplied full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/controls-wakes-a0d03202.log`.
It names the exact reviewed head and base.
The default tier reports 975 passed and 649 skipped.
The slow tier reports 243 passed and 975 skipped.
The new wake tests and the active conformance trials pass.
The mutation run tests five mutants: three caught, zero missed, zero timeouts, and two unviable.
All ten CI stages pass. Fuzz runs no decoder harness for this delta.
The job and gate exit zero.
The PR describes the earlier clippy failure and the test-helper fix at the reviewed head.

No package finding remains. Integration must supply its separate CLEAN before this HIGH PR merges.
This verdict does not close #176's proof hold or establish real-process minimum conformance.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN

# Contracts #50: R-43(5) spurious wakes

## Round 1 — 2026-10-11

Head: `338008cbeea0bc1e675680e2d1c2935bf4702383`.
Original source base: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
PR and gate base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
Tree: `b4a280352a33f1ba767fa898928448d60beb7d17`.
Package verdict: `70b296a75d6fcb76b9186f9dc03b68db27b60ff4`, `verdicts/gs-gate-speed.md`, R-43(5) Round 3.

Sol read the original complete sixteen-file delta at 9bb6fd56 and the four-file correction through this exact head. Sol read all three committed package rounds, the complete corrected live body, and the R-43(5) follow-up. Sol read 016c's complete local ACCEPT at `.review-verdicts/r43-spurious-wakes-338008cb.md`. Its SHA256 is `25703a989613993edc4378091a799df2e91b4a89b246a0f47d13e489c9de199a`.

### Shared runner and Core boundary

The runner's new start method has a default no-op. Existing Hub drivers retain that default. The runner checks required controls before start and calls setup after start. A failed start uses the existing step-zero error mapping. A structural fake retains its separate structural check. FakeCore receives no behavior change.

Core applies no_spurious_wakes before setup only when the executable scan finds a strict Woken wait and the harness supports the control. This placement prevents a pre-existing seeded level signal from making the strict wait pass falsely. An unsupported harness receives no control call. A supported control that fails ends the run. Other transcripts retain seeded wakes.

The scan now uses the runner's unchanged order classifier. Acknowledgements go directly to exec. Barriers go through run_step and can contain order constructs. Empty expect_set values execute no acknowledgement and disable no seeded wake. Invalid set values are also excluded. Direct waits share the same predicate with exec. The moved wait branch preserves its body and thread behavior. The script helper scans direct steps because it does not run order constructs.

These changes close HIGH GS-R43-1 and 016c's corresponding scope finding. The empty-set proof retains the seeded wake and fails the following TimedOut assertion. The unused-field proof also retains that wake. The negative dropped-signal proof retains a strict failure. Sol adds no finding.

### Transcript forms and accounting

The eight transcript blobs equal their original 9bb6fd56 versions. They implement the steward's widening for TimedOut waits and preserve strict Woken waits. The ordinary post-wake pumps use the same clock and require zero posted events and no more work. The tm_3 exception uses one posted event, pump_idle, and exactly Silent. No further expectation is widened.

Static parsing finds 24 direct wait_wake steps across the entire Core suite. All have only wait_wake, expect, and because keys. The changed dispatch order therefore does not select a different operation for an existing transcript step.

The prior Core comparison retains historical scratch heads f29aeae1 and 8d71a84a. Sol read the reported outcomes and decoded command text, with no command execution. Both testkit runs report 209 passes and the same four failing IDs. The real run changes from eight unsupported no_spurious_wakes results to four passes, one strict-wake failure, and three unsupported route_fill results. The changed step indexes follow the inserted or removed transcript steps. Both job wrappers exit zero despite conformance failures. Their exits are not full conformance PASS evidence. Sol does not claim a complete per-ID whole-suite outcome comparison from these filtered logs.

The body retains these failures and historical reversal heads. LOW GS-R43-R3-LOW-1 closes through the corrected historical test-count labels. This PR removes no Core pending entry. The future Core pin must supply its own PASS before any pending removal. The accepted real-spurious-wake residual remains the steward's stated limit.

### Exact-head gate

Gate: `gates/botster-contracts-stage1-gs-r43-spurious-wakes-338008cb-pool-20261011-092004-41960.log`.
SHA256: `998028e3f0aaf3af8b238498cd6b8420a89802b906e4319f9204517676c93ed8`.

Sol read the exact-head headers, all 13 spurious_wakes PASS records, suite summaries, mutation baseline and summary, and final exits. All ten steps pass in 395.5 seconds. The default suite has 2093 passes and zero skips. The slow suite has 33 passes and zero skips. The mutation summary reports 20 caught and two unviable mutants, with zero missed mutants. The log does not supply individual mutant identities or catcher records. Sol makes no independent identity-match claim. The job exits zero after 404 seconds. The wrapper exits zero after 405 seconds.

The earlier failed mutation gate remains reported history. The correction adds the thread proof that addresses its missed mutant. Sol did not independently inspect that earlier gate or run a reversal. The exact-head gate supplies the current aggregate result.

### Verdict and limits

Both package records accept this exact head. All package findings are closed. Sol adds no finding. The steward owns the merge. A later import of #47 must retain its route loop and apply the required delta review and exact-head gate. This verdict approves no future import, pin, pending removal, or historical catch.

Sol changed no product code. Sol ran no builds, tests, gates, mutants, measurements, or reversal jobs.

VERDICT: CLEAN (0 open)

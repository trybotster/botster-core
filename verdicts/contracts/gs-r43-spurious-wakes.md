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

## Round 2 — #47 import delta — 2026-10-11

Head: `efbb30636b29d506ab04acdb9f7f90031601c674`.
Base: `f303ccdb9e02c6763d31cd8f7503095084630adc`.
Parents: `338008cbeea0bc1e675680e2d1c2935bf4702383` and the named base.
Tree: `01a280eb3082d937b86b85e110051f27fba21ec8`.
Package source CLEAN: `8ad6e6328fad1f0364a4fe451ab03ddff50b82e3`, `verdicts/gs-gate-speed.md`.

Sol read the complete retained base-check log. Its SHA256 is `f2c768caa55cc4c7051059855ee7656f11c3ecc62bb745bd55d66f7e9610b135`.
Conditions 1, 2, and 3 fail. Condition 4 passes. The driver conflicts, and the control documentation overlaps.
Both reviewers must review this delta. Sol read the complete package source verdict and the complete current PR body.

Sol compared both complete own diffs. The old full-index diff has 60894 bytes and SHA256 `25d7c2fe75c4da6c95a8c76e7801196b3fbced249b980aa8d1a2d1df1f2b4766`.
The new full-index diff has 61061 bytes and SHA256 `eb6782c17e03d8e2a91f3f32e68c5a52bf8c373318e7e6d8193a6a09573d6134`.
After removal of index and hunk-position lines, the differences are the wake access and one imported documentation context line.
All other sixteen own file blobs equal the previously reviewed head.

Sol read the complete driver delta. The moved wait obtains its owned wake handle through `core.borrow_mut().wake_handle()`.
The temporary mutable borrow ends at that statement, before either thread waits. The removed old-position branch has the same base access.
The thread selection, timeout, expected value, and failure mapping remain unchanged.
The strict-wait scan and its shared dispatch predicate remain unchanged. The #47 route loop remains in the base.
The imported documentation context adds R-69 to `oracle_resume`. The own `no_spurious_wakes` row remains unchanged.

Both prior findings remain closed. Sol adds no finding.
The body retains the historical `338008cb` gate. It supplies no gate on this import head.
The lead requires the #49 import and one final gate after that import. The later head needs its applicable base check and reviews.
This verdict accepts the source delta only. It does not declare merge readiness or approve a future gate, tag, or Core pin.
Sol ran no build, test, gate, measurement, reversal, or base-check script.

VERDICT: CLEAN (source delta review; 0 open source findings)

## Round 3 — final #49 import and gate — 2026-10-11

Head: `ddce507e2c08e740f899e97dcfec566f3d2275b8`.
Base: `6a715f890a95f5ca92c726e5149c2e5adc01261f`.
Parents: `efbb30636b29d506ab04acdb9f7f90031601c674` and the named base.
Tree: `0b3c1c91c302cb55e05f1d5071f8f3f75b713196`.
Owner receipt: `msg_plugin-w_1791739282_47355d`.

Sol read the complete final Q7 log. Its SHA256 is `dfe2aca415f13ebe945871235cbecdcf9af3249eab014934089bebdc8d4b0c93`.
All four conditions pass. The three imported paths have no overlap with the eighteen own paths.
Sol independently compared the complete full-index own diffs. They are byte-identical at 61061 bytes.
Their SHA256 is `eb6782c17e03d8e2a91f3f32e68c5a52bf8c373318e7e6d8193a6a09573d6134`.
All three imported file blobs equal the base. The static merge tree equals the named head tree.
Both line-set files remain absent from all four objects, as the retained check states.
BUILD Q7 carries both source CLEANs from the reviewed efbb3063 head. Sol adds no source finding.

Gate: `gates/botster-contracts-stage1-gs-r43-spurious-wakes-ddce507e-pool-20261011-101430-59112.log`.
SHA256: `df5065e7ce3628995020420c7b1a47b05de4ba1636562c21b51438fe020891c5`.

Sol read the exact head and base headers, all thirteen spurious-wake PASS records, both suite summaries, mutation records, and final exits.
All ten gate steps pass in 384.4 seconds. The default suite has 2103 passes and zero skips.
The slow suite has 33 passes and zero skips. Both Rust test budgets pass.
The unmutated baseline passes. The mutation summary reports 22 mutants: twenty caught and two unviable, with no missed mutant.
The retained log supplies aggregate counts without individual mutant identities or catcher records. Sol makes no independent identity-match claim.
The complete log contains no FLAKY token. The job and wrapper exit zero after 394 seconds, with no queue wait.
Sol makes no individual fuzz-corpus inspection claim.

Sol read the complete final PR body and verified its head and base.
The body names both imports, their checks, the source verdicts, and the final exact-head gate.
The body retains historical reversal heads and the historical Core comparison limits.
The body reports reviewer 016c's import disposition as no findings. Sol has not independently read that new disposition.
Sol's earlier complete read of 016c's original ACCEPT remains as recorded in Round 1. The steward checks the additional review requirement.

Both source findings remain closed. The final import and exact-head gate conditions for Sol's integration review are complete.
The steward owns the merge. This verdict approves no future tag, Core pin, pending removal, or historical catch.
Sol ran no build, test, gate, measurement, reversal, or base-check script.

VERDICT: CLEAN (0 open)

### Round 3 package completion receipt — 2026-10-11

Package reviewer 017c publishes final carry-over CLEAN at the same exact head in `e70fa9e757f9352f8fff8f81a698bc332409455b`.
Receipt: `msg_plugin-w_1791739333_63eafa`.
Sol read the complete committed final-base section in `verdicts/gs-gate-speed.md`.
The package record confirms Q7 PASS, the unchanged own diff, the final body, and the exact-head gate.
Both package findings remain closed. The package record retains the aggregate-only mutation evidence limit.
Sol requested the existing 016c import disposition through `msg_plugin-w_1791739385_c23408`. Its independent receipt remains pending.
The final integration verdict at `739adc892c910f3a78e741d7c54ca1ced380a6f4` remains unchanged.

VERDICT: CLEAN (0 open)

# Core #281 — Git failure fixture

## Round 1 — 2026-10-11

Verdict: source CLEAN (0 open) at `1f0ab3eb5e8f19b43fed1e34468e86bced9a244b`.
Risk: HIGH. Own-diff, PR, and supplied gate base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Package CLEAN: `eb5cb9b8`, `verdicts/gs-gate-speed.md`, Core #281 section.
Sol read that complete section, the complete source delta, full live body, collector and status-check paths, and gate evidence.

Only the negative fixture in xtask/src/mutants_cited/tests.rs changes.
The test still requires exactly a/b.rs and c.toml from its staged repository.
It still requires the collector to return an error when Git cannot list tracked files.
The new fixture writes a .git file that points to a missing directory inside its owned temporary directory.
The test does not create that directory. Git reaches the invalid reference before it can find an ancestor repository.
The temporary directory owns and removes the fixture. The fix changes no process environment, production code, timeout, or mutation exclusion.
tracked_with_submodules still runs git ls-files -z --recurse-submodules and passes the output through stdout_of.
stdout_of rejects a failed command. The PR removes no error check or rejection assertion.
The old Core pin has no corresponding mutants_cited check or fixture to reuse.

Failure log: `~/botster-sessions/gates/botster-core-stage1-p5-lc2-real-proof-cb7eb0f7-pool-20261011-101307-53515.log`.
Sol reads the original negative assertion failure. Its tracked_with_submodules call unexpectedly succeeds.
The same log retains separate signal-scan and base-merge timeouts. This PR changes neither timeout nor the base-merge test.
The body reports local red-before and green-after runs with TMPDIR inside a repository.
It also reports all 307 xtask default tests passing under that condition. No raw local log accompanies that report.
Sol does not reproduce those runs or claim independent verification of the local execution or pool configuration change.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-1f0ab3eb-pool-20261011-102558-30553.log`.
SHA-256: `e85fb7ed4f8414d19a686219c36f2eefedc6f041e7e9ce66cdeeea8eff26ca55`.
The header names the exact head and base above. All ten steps PASS in 291.1 seconds.
The repaired test passes in 0.006 seconds.
Default: 1729 PASS, 480 skipped, 12.343 seconds. Slow: 555 PASS, 2022 skipped, 40.659 seconds.
The test-only diff selects no mutant. No mutation execution or old-versus-new mutation comparison is claimed.
The complete log has no FLAKY token. Fuzz selects no changed decoder crate.
The job and wrapper exit 0 after 387 seconds, with two seconds queued and 385 seconds running.

Both source CLEANs now cover this exact head. Under plan 23bu, GS must send FINAL-READY and wait for the lead's final-gate slot.
With that slot, GS must import current v1 once, supply the applicable base check, and run one final exact-head gate.
This verdict accepts no later import, gate, timeout change, or unrelated retry.
Sol ran no build, test, gate, mutation, measurement, or executable probe. Wait for the doorbell. Never poll.

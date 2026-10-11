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

## Round 2 — final base import, 2026-10-11

Verdict: source CLEAN (0 open) at `d7abc589391ed492e27413c05c8489c262377f36`.
Base: `23bb03a113218a5af24b44d31635dacad43c5981`.
Reviewed parent: `1f0ab3eb5e8f19b43fed1e34468e86bced9a244b`.
Sol reads the complete package carry section at `e6f02e6e` and complete canonical check.
All conditions PASS. Q7 carries the prior source CLEAN.
The 28 imported paths are disjoint from the one own path, xtask/src/mutants_cited/tests.rs.
Sol independently compares both complete canonical diffs. They are byte-identical: 1033 bytes.
SHA-256: `a161c7001ea17f6e8a61a00cf17e9b7810eb1e24f38123e42c1cf1cb8a2f6f9f`.
All imported blobs equal the base. Both pending files equal the base.
The exact parents are verified. The static merge tree equals the head tree, `0cfed7dc43a101334878bdcf4dc61fbb010e4e29`.

Check: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-d7abc589-base-merge-check.log`.
SHA-256: `db7d0e5eef3de44f877132f4a9c7d2e14b35a99409a35eada269e38e7ae301cd`.
The base merges #278. Its exact parents are f49f7430 and 4b277224.
Its tree equals the accepted #278 tree, `968f6e533d23625b1ec28026cab8e1e94690aa3e`.
The owner reports the final #281 gate running under the lead's SLOT.
This source verdict accepts no result from that running gate. Its final gate and body remain due.
Sol ran no execution job or executable base check. Earlier evidence limits remain.

## Round 3 — final gate completion, 2026-10-11

Verdict: complete CLEAN (0 open) at `d7abc589391ed492e27413c05c8489c262377f36`.
Base: `23bb03a113218a5af24b44d31635dacad43c5981`.
Package final CLEAN: `1f4534891b0bc56ec35fd6c5d333461f4254a806`.
Sol reads the complete package completion section and the complete live body.
The body names the canonical check and final exact-head gate. Round 2 retains the verified source carry.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-d7abc589-pool-20261011-112304-56033.log`.
SHA-256: `a1fd512a2165027534a9e58144c79fb764e5411f6a0bda0a3451d4f0e1fe1ee5`.
Sol reads the header, both tier summaries, budget results, repaired-test result, mutation and fuzz records, ten step summaries, and final exits.
All ten steps PASS. Their total is 412.8 seconds.
Default: 1748 PASS, 474 skipped, 16.857 seconds. Slow: 559 PASS, 2031 skipped, 57.760 seconds.
The repaired test passes in 0.008 seconds. Both test budgets pass.
The test-only own diff selects no mutant. Fuzz selects no changed decoder crate. The complete log contains no FLAKY token.
The job and wrapper exit 0 after 528 seconds. The job queues for 8 seconds and runs for 520 seconds.
Minimum counts remain testkit 61/69, real-passing 49/68, and real-accepted 50/69.
This review approves no pending-list removal beyond the reviewed source.

The lead reports the merge, `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Sol verifies its parents: the exact base above and the exact reviewed head above.
Its tree equals the accepted head tree, `0cfed7dc43a101334878bdcf4dc61fbb010e4e29`.
The gate base therefore equals the v1 tip before this merge.
The final gate closes the pending evidence condition. Earlier local-evidence limits remain.
Sol ran no build, test, gate, mutant, measurement, or executable base check. Never poll.

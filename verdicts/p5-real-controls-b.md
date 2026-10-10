# PR #228 — Real endpoint stand-ins and worker liveness

## Round 1 — 2026-10-10

Reviewed head: `a263ff04532e952687fd1c4b8c9d3a1b3037692d`.
Tree: `ffe83ecf90b2a7386e17d73bde1f861f2743c136`.
Parent: `b53214af87f4004d6d3b0d7e9e50ea3e5ea5b074`.
PR base: `b4847bf51b98a2a720e60d7078b88397e2059bbf`, the #222 branch.
Gate base: `c19ad938bd87c60331bd002c0117102d8756b1ae`.
Both bases are ancestors of the head.
Tier: HIGH, because the PR changes the shared testkit and the process-check configuration.
Authority: approved plan 23y, the accepted P5 controls plan, and BUILD.md at contracts `56bd0a5`.
The controls and R-49 were read at pinned contracts `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
The reviewer read all 13 own-diff files, affected process code, the PR body, and retained gate evidence.

### RC-B-F1 — HIGH — OPEN — `worker_alive` reports an unreaped ended worker as alive

Location: `crates/botster-core-testkit/src/real.rs:437-443`.

The real control returns true when the recorded worker has `IdentityState::Matches`.
The controls row requires whether the worker process still runs.
On Linux, `core-sys::process::start_time` reads the start time from `/proc/<pid>/stat` without checking the process state.
An unreaped ended process, called a zombie, retains that pid and start time.
The identity check returns Matches, and the control returns `{"alive":true}` for an ended worker.
The testkit control checks `!cell.ended`, so the two tiers disagree.

The R-46 follow-up requests an identity-checked read like `payload_alive`.
It does not define identity presence as liveness.
The existing `lives` helper combines matching identity with `runs`, which excludes zombies.
Use that helper or an equivalent identity-and-liveness check.
Correct the method description, which currently describes identity matching instead of whether the worker runs.

Add a real-process proof that retains an ended worker unreaped and expects false for its recorded identity.
Retain true for a live worker and false for a reused identity.
Use the shared process guards and preserve production ownership of reaping.
The current proof waits until the killed worker's identity is Absent before checking false.
That proof omits the unreaped interval.
The existing real proof `a_process_that_exited_and_is_not_reaped_does_not_run` establishes the distinction for `runs`.

The package reviewer sent this finding directly to P5.
Sol integration independently confirms it as R1-2.

### RC-B-F2 — LOW — OPEN — Payload documentation attaches to `worker_alive`

Location: `crates/botster-core-testkit/src/worker.rs:691-693`.

The two existing payload documentation lines precede the new `worker_alive` method.
Rust attaches those lines to `worker_alive`; `payload_alive` loses its description.
Move the payload lines above `payload_alive`.
Keep the worker description above `worker_alive`.
The package reviewer sent this finding directly to P5.
Sol integration independently confirms it as R1-3.

### RC-B-F3 / integration R1-1 — LOW — OPEN — New tests assert helper internals

Locations within `crates/botster-core-testkit/src`:

- `stand_in/tests.rs:91` checks private `links.len()`.
- `stand_in/tests.rs:178` and `:197` check private `links[0].stream.is_none()`.
- `edge_tap/tests.rs:681` and `:841` check the private signal vector.

BUILD.md Testing shape rule 3 prohibits tests of a helper's internals.
The EOF proof already checks observable socket EOF.
The tap exposes the signal record through `signals_since_answer`.
Remove the redundant private assertions.
Use observable socket behavior and `signals_since_answer` for retained proofs of closure, signal order, and exclusion of the harness's kill.
If a test is removed, name the retained proof that covers its clause.
Sol integration found this issue and sent it directly to P5.
The package reviewer read the cited assertions and confirms the finding.

### Other source checks

The stand-in follows the endpoint replacement mechanism accepted by R-49.
The control records an arm from the stored row, token, and identity.
The harness installs the stand-in before reopening Core and only when no live tap holds that data directory.
An attempted second open preserves the real endpoint and retains the arm.
The real proof checks that condition and installation at the later open.

The guard restores the original endpoint on drop.
Failed installation also restores the renamed endpoint.
The tap declares its inner edges before its stand-ins, so it releases the directory lock before endpoint restoration.
The real proof checks the restored endpoint inodes and verifies that both real workers still run.

The tap records every Core `signal_group` call, in order, with each target identity.
The record begins at the stand-in's first answer and includes later calls to all targets, as R-49 Q1 requires.
The harness's own kill bypasses this record.
The record does not infer absence of signals from the stand-in's identity.

Shared parsing and answer helpers preserve the testkit's token, instance, protocol, and script behavior.
The protocol control refuses an in-set protocol.
The stand-in sends the hello and permitted script in one write; a short write fails setup.
The withheld role accepts without answering, so Core's handshake deadline decides the result.
No new branch for tests enters Core or the production worker.

The new process-check allowance names one non-blocking `accept` site.
`StandIn::install` sets its private listener non-blocking before construction returns.
No later path changes that mode.
Each accepted stream becomes non-blocking before a read; a failed mode change drops the stream.
The reviewer accepts this allowance for the reviewed source.

### List and evidence checks

Real pending changes from 81 to 71 IDs, with ten removals and no additions.
All ten removals have actual real-tier PASS lines at this exact head:

- `conf::a10_1_wrong_instance_is_never_signalled`
- `conf::a10_1_wrong_token_is_never_signalled`
- `conf::a2_1_adopt_completes_ok_with_the_state_it_reached`
- `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`
- `conf::a6_2_out_of_set_hello_is_lost_worker_version`
- `conf::ad_4_absent_worker_protocol_is_unknown_never_compatible`
- `conf::ad_4_compatibility_is_not_a_guarantee_of_adoption_auth_and_reachability_can_still_fail`
- `conf::ad_4_unknown_version_is_lost`
- `conf::ad_6_token_and_instance_checked`
- `conf::lc_9_worker_protocol_absent_for_an_unreachable_worker_whose_hello_was_never_read`

Four removals are minimum IDs. Real-passing increases from 31 / 68 to 35 / 68.
The ledger, Core pending, deferred/status copies, minimum list, and real-only map equal the PR base bytes.
The separately planned worker-exit notification control remains required for `ad_2_lost_row_keeps_id_and_allows_retry`.
`git diff --check` reports no whitespace error.

Full gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-b-a263ff04-pool-20261010-133218-79259.log`.

The header names the exact head and gate base. All ten stages pass.
Default: 1500 passed, 497 skipped. Slow: 403 passed, 1935 skipped.
Testkit conformance has 193 passes; real conformance has 122 actual passes.
Real pending reports 71 IDs and zero newly passing IDs.
Minimum: testkit 50 / 69, real-passing 35 / 68, real-accepted 35 / 69.
Both default mutation steps report 105 tested, 88 caught, 17 unviable, zero missed or timeout.
The job and gate exit zero after 837 seconds on msa1.
The repeated default mutation step does not establish slow-feature coverage.

Manual slow-feature evidence: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-b-a263ff04-pool-20261010-134627-9621.log`.

The command selects the own diff against `b4847bf5`, enables `slow`, and uses `--no-config` and `--in-place`.
Every test command uses the slow profile and `--max-fail 1:immediate`.
The testkit baseline passes all 345 tests.
The reviewer parsed all 79 records and checked their retained logs.
The 78 mutants produce 66 named test failures and 12 compile failures, with zero missed or timeout.
All catches exit 100 in the test phase. All unviable mutants exit 101 in the build phase.
Eleven unviable mutants lack a required Default implementation; one cannot select the trait implementation for `Default::default`.
The job and gate exit zero after 289 seconds, including one queued second.
These results omit RC-B-F1's unreaped-worker condition.

The own diff adds no platform-only code and adds no separate native Mac evidence requirement.
The stacked #222 still requires its missing native Mac mutation evidence.
This verdict does not close that finding or approve #222's updated head.

### Integration and next round

Sol integration publishes NOT CLEAN at the same exact head in verdict commit `d7f3e07a6be1f30134583ba13b414dc26769eef4`.
Its three findings match the findings above. The package reviewer read that committed artifact.
The lead records that #225 merged and advanced `v1` to `fc1e8160abfd8248b155383ef1d766c6377f8a11`.
The replacement READY must supply the required base checks and exact-head gate under the current command.
#222 must merge before this stacked PR.
All three findings, including LOW, must close before CLEAN under the HIGH rules.

The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
Wait for replacement READY before Round 2.

VERDICT: NOT CLEAN

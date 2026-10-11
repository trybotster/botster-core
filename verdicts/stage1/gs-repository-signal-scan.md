# Core #278 — repository signal scan

## Round 1 — 2026-10-11

Verdict: source CLEAN (0 open) at `214ade281c890b05e72054f66b2b34e853204d3c`.
Risk: HIGH. Own-diff, PR, and supplied gate base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Package CLEAN: `6e9b11132bdfce6600972704b43ce2aea10b467c`, `verdicts/gs-gate-speed.md`, Core #278 section.
Sol read that complete section, the complete source delta, full live body, scanner and collector paths, and retained gate evidence.

### Gate coverage and scope

Only tests in xtask/src/signals.rs change. The production scanner, file collector, verdict, gate order, and timeout values remain unchanged.
The removed default test walks the tree and parses every Rust file. The gate already runs that scanner in taint_job.
The new fixture checks that raw calls pass only in the guard file. The same call in another Rust file fails the verdict.
It checks the exact Rust-file counts and violating file and line. Non-Rust input does not enter the scan.
The separate repository test checks that the exempt guard exists and that command rejects an argument before file collection.
The existing raw-call, alias, lexer, comment, string, and verdict tests remain selected.
Those fixture tests retain the scanner's rejection rules for mutations of its decision code.
The removed whole-tree test checks a clean tree; it adds no distinct rejection rule.

taint_job still calls signals::command(root, &[]). The gate still runs taint before default tests.
The production command lists tracked files. The removed test uses the test-only walk_files helper.
Sol checks all 228 tracked Rust paths at this head. None lies in a directory that walk_files excludes.
The clean gate snapshot therefore retains the relevant Rust-file set through the production check.
This conclusion does not cover arbitrary untracked files in a developer checkout.
The file walker stays test-only. The change removes no production guard, mutation filter, exclusion, selection, or verdict.

Sol checks the other repository-reading default tests named in the body.
signal_bans walks names and reads clippy.toml files. high_tier walks names and reads the HIGH-path list.
The harness, platform, profile, exclusion, known-flaky, and taint tests read their named files or directories.
The package test uses cargo metadata. The repository-reading slow modules retain their slow-tier scope.
Those checks remain unchanged. Their cost claims in the body remain retained owner measurements, not new Sol measurements.
The 122-log timing range also remains an owner report. Sol does not claim to review all 122 logs.
The unchanged two-second default test budget still applies.

### Evidence

The retained #275 failure names signals::tests::the_repo_has_no_raw_signal_call.
It times out at 2.003 seconds against the two-second limit. Sol reads the failure and budget result.
Failure log: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ff913e68-pool-20261011-093345-95687.log`.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-214ade28-pool-20261011-094124-54121.log`.
SHA-256: `8f82ad42d4a0d6c3665bbf125300eb140b94d552a8b01f2b6515b03a8de78fd7`.
The header names the exact head and base above. All ten steps PASS in 313.4 seconds.
The taint step scans 228 Rust files. All ten signal tests pass, each in at most 0.004 seconds.
Default: 1726 PASS, 481 skipped, 12.109 seconds. Slow: 553 PASS, 2023 skipped, 43.235 seconds.
The mutation step selects no mutant because the diff changes tests only. This is no old-versus-new mutation comparison.
No FLAKY line occurs. Fuzz selects no changed decoder crate. The job and wrapper exit 0 after 407 seconds.
The job records 12 seconds queued and 395 seconds running.

Negative proof: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-214ade28-pool-20261011-094827-75167.log`.
SHA-256: `754c9d449e382a3bc213cd2a0c930fc472b78090ee9acab08379317158966ecd`.
Sol reads the complete log, including the retained command text.
It appends a raw call to tracked crates/botster-core/src/lib.rs and runs the existing taint job.
The job names that file, line 406, and one signal violation. It exits 1.
After restoration, the same job exits 0. The wrapper exits 0 because its final command prints the restored status.
The explicit job statuses support the negative proof. The planted function is scanner input; the proof does not execute its signal call.
The body also reports a local guard-exemption reversal. Sol claims no independent execution or raw-log verification of that local control.
The source assertions and separate pool proof support the retained rejection boundary.

### Remaining work

Both source CLEANs now cover this exact head. GS must import current v1 once, supply the applicable check, and run one final exact-head gate.
This verdict accepts no later import, gate, timeout change, pin, or pending removal.
The old Core pin has no corresponding xtask signal scanner or test-budget mechanism; no replacement mechanism is omitted here.
Sol ran no build, test, gate, mutation, measurement, or executable probe. Wait for the doorbell. Never poll.

## Round 2 — final base import, 2026-10-11

Verdict: source CLEAN (0 open) at `4b277224e9b7e164bdc8bed2e634601e9c8b2a3f`.
Base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Reviewed parent: `214ade281c890b05e72054f66b2b34e853204d3c`.
Package carry: `8abee151`, verdicts/gs-gate-speed.md, complete Core #278 final-base-import section.

Sol reads the complete canonical check. All conditions PASS. Q7 carries the prior source CLEAN.
The 32 imported paths do not overlap the one own path, xtask/src/signals.rs.
Sol independently compares both complete canonical diffs, with full object identifiers.
They are byte-identical: 2929 bytes, SHA-256 `f02ec8df7efbf9e7c40774eecd7fd87a770a1dc75c250c433c8e76641d7852a2`.
All imported blobs equal the base. Both pending files equal the base.
Both merge parents are exact. The static merge tree equals the head tree, `968f6e533d23625b1ec28026cab8e1e94690aa3e`.
Sol also verifies the base merge of #279. Its parents are 9f7085f7 and 80c5b50c.
Its tree equals the accepted #279 source tree. This verifies no #279 gate result.

Check: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-4b277224-base-merge-check.log`.
SHA-256: `ebf0e2806a459fdd3ee0338e25f2f3e6de50a2e1fc2ac3f29cef5f466a5ebe44`.
The owner reports the final gate running at this head under the lead's SLOT.
This verdict accepts no result from that running gate. The completed exact-head gate and final body remain due.
Sol ran no gate, build, test, measurement, mutation, or executable base check.

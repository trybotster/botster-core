# Core #276 — host-supplied worker file name

## Round 2 — 2026-10-11 — overlapping base import

Verdict: source CLEAN (0 open) at `300eccec3f9c2929e137ee33eb2e8cbe45bf66a8`.
First parent: reviewed head `67fd9112d0e7554d39be6eb6b43260c782dda200`.
Second parent and new base: `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
Head tree: `ca4b8722aebb4e0a737b902aa4776c4681a8d966`.
READY: `msg_plugin-w_1791740971_ca538d`. Package CLEAN: `ce5d6b6d1de679907a29b54e1330a7923bfb5625`, Round 2.
Sol read the complete package round and the complete failed base-check report.
Report: `~/botster-sessions/shared/core-stage1/evidence/base-merge-check-276-300eccec.out`.
The report fails conditions 1–3 because the proof file conflicts, real.rs overlaps, and the own diff has different context and blob identities.
Both pending-file conditions pass. Q7 does not apply to this overlap. This round supplies the required integration source review.

Sol verifies the exact two parents and the complete new own diff.
All three own paths retain exactly the same added and removed lines as Round 1.
All nineteen imported paths outside own scope equal the base blobs. The complete real.rs import delta is read.
The imported oracle module, three dispatch arms, capability term, and test entries retain their base text.
The merged capability expression keeps self.tapped around the entire control set, including worker_file_name and the oracle term.
The capability test retains the PC entry and chains the imported oracle names. Plain-harness checks remain present.
The imported oracle names do not intercept worker_file_name. The worker wrapper, candidate validation, and open path remain unchanged.

Sol independently checks both pending files and the proof file against the union of both parents' changes.
All three line unions match, and none has a duplicate identifier row.
The resolved proof file equals the base with exactly one E1-1 row added between AD-6 and the two LC-2 rows.
No base proof row is lost or changed. The proof checker, suite path, transcript, and Contracts pin remain unchanged.
The imported source supplies no OU-9 output fence. Round 1's cause and narrow retry limits remain explicit.

Both source delta CLEANs now cover this head. The owner reports that the final gate is running.
Sol has no completed gate receipt for this head. The old gate remains evidence only for 67fd9112.
The body must record the merge, failed check, both delta verdicts, and final exact-head gate before merge.
This verdict accepts no later import, final gate, pin, or pending removal.
Sol ran no execution job. Wait for the doorbell. Never poll.

## Round 1 — 2026-10-11

Verdict: source CLEAN (0 open) at `67fd9112d0e7554d39be6eb6b43260c782dda200`.
Risk: HIGH. Parent and own-diff base: `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`.
PR base and green gate base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Package CLEAN: `fd6eeeb3343a7768b8550c872139a85cac308757`, `verdicts/pc-e1-1-worker-file-name.md`, Round 1.
Sol read the complete package record, three-file own diff, current body, pinned E1-1 transcript, and relevant harness and suite code.
The body uses review plan 23bk, `00635d10`, and correctly separates the parent from the green gate base.
Both earlier LOW body notes close. The active Contracts pin remains .36 at `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.

### Source and process boundary

Core erratum 1 requires the embedder to supply the worker path. Core requires no fixed worker file name.
The tapped real harness now declares `worker_file_name`. The plain harness still declares no controls.
The existing capability test checks the new name for both harness forms.
The existing wrapper selects the binary by role, independently of its file name.
Candidate::locate reads the manifest, requires one entry per binary role, hashes each binary, and rejects a hash mismatch.
It verifies the worker, probe, and anchor. The real suite loads that candidate before it constructs the harness.
The guard writes its executable wrapper at the supplied name. The wrapper runs the verified anchor and worker.
RealCoreHarness::open passes that path through OpenConfig.worker_path.
The pinned transcript uses `worker-copy-without-the-botster-name` and `w.v2` in separate data directories.
Steps 5 and 9 require Running through the real worker startup protocol. The harness does not model that result.

Only `conf::e1_1_worker_found_by_host_supplied_path` leaves core-pending.
The proof line identifies the real-tier trial in the approved #264 form. The existing AD-6 line stays unchanged.
The real suite requires this trial to pass. The testkit suite retains its real-only ignore.
The same-run result check requires the exact selected, unignored trial to PASS. The mapping counts the identifier once.
The change alters no runtime, transcript, Contracts pin, timeout, mutation exclusion, or real-pending entry.
The current v1 wrapper supplies the prior art. The old Core pin supplies no replacement for this capability declaration.

### Gate and cause evidence

Green log: `~/botster-sessions/gates/botster-core-stage1-pc-e1-1-worker-file-name-67fd9112-pool-20261011-093607-9029.log`.
SHA-256: `1d0d0ea7b0eebfe283f23bb7a7bfaadb8b95577901456cb3d6f54f58f528d5bf`.
The header names the exact head and green gate base above. All ten steps PASS in 551.9 seconds.
Default suite: 1724 PASS and 481 skipped. Slow suite: 554 PASS and 2020 skipped.
The real E1-1 trial passes in 0.477 seconds. The baseline also passes it in 0.466 seconds.
The same-run result check names its proof and checks two proofs.
Both stage-1 has_control mutants are missed. Stage 2 catches both.
Final result: two caught, zero missed, zero timeout, zero unviable. The raw and catcher records identify the same two mutants.
The log contains no FLAKY token. The job exits 0 after 665 seconds; queue wait is zero.
The wrapper exits 0 after 666 seconds. Sol verified the complete log hash.

Red log: `~/botster-sessions/gates/botster-core-stage1-pc-e1-1-worker-file-name-67fd9112-pool-20261011-092132-45421.log`.
SHA-256: `921a0a1b5e191b688c470b30d3210861070aa8f552d3923804abc55325c78a2f`.
The unmutated OU-9 baseline fails at seed 0, step 14. It expects `after1after2` and receives `beforeafter1`.
The wrapper exits 1 after 485 seconds. The baseline has no retry. Fuzz does not run.
This first gate remains RED. Sol verified its complete hash and read its failure record.

The .36 transcript has no output sentinel before the attach. Quiet does not prove that the worker consumed the earlier PTY bytes.
The controlled P3 diagnostic produces the same failure. The historical delayed-read comparison has five failures before the fence and five passes after it.
The plain before and after comparisons each have five passes. These records support the missing-fence cause, not an unmodified current-v1 failure.
Sol previously reviewed that diagnostic evidence and the Contracts #49 structural fence.
The #276 change affects no OU-9 control answer, worker read, route, baseline, output fence, or transcript.
The package reviewer confirms this independent cause. Lead receipt `msg_plugin-w_1791737460_dd4f32` explicitly accepts this case under plan 23bl amendment (A).
That narrow ruling allows the controlled diagnostic to satisfy this case's plain-v1 condition. Sol claims no ordinary plain-v1 reproduction.
The #264 record contains two OU-9 FLAKY outcomes under mutants. Its stage-2 baseline did not fail.
The approved one re-gate is GREEN. The structural fence still needs the later Core pin.

### Remaining work

This verdict approves source on this exact head. It does not approve a later import or gate.
The head does not contain PR base `42d7056e`. The green gate precedes the final import.
After both source CLEANs, PC must merge current v1 once, supply the applicable base check, and run one final exact-head gate.
Current reported and verified v1 is `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
The original program-controls group remains incomplete. No later pending removal or Contracts pin is approved.
Sol ran no build, test, gate, mutation, measurement, or executable base check. Wait for the doorbell. Never poll.

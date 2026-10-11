# PC package review: worker_file_name capability, PR #276

VERDICT: source CLEAN (0 open) at `300eccec3f9c2929e137ee33eb2e8cbe45bf66a8`.

Round 1 below records the source review. The earlier cause record remains as historical evidence.

- Head: `67fd9112d0e7554d39be6eb6b43260c782dda200`.
- Parent and recorded gate base: `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`.
- Current PR base from GitHub: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
- Tier: HIGH. Plan: revision 23bj, `stage1-plan.f561dd4e.md`.
- Plan SHA-256: `bd43104c8b237618e3249f570899fb19c5b6d3e8c71d684fbd114d62f7691007`.
- PC receipts: `msg_plugin-w_1791736235_5c82eb`, `msg_plugin-w_1791736258_fee5b9`.

The reviewer read the brief first and checked the new plan text. Sections 5 and 8 equal the previously read 23bb sections.
The lead's 22:35 handoff attributes this failure to the missing R-57 fence and approves one re-gate, conditional on this reviewer confirmation.
The reviewer read the complete three-file own diff, current PR body, pinned OU-9 transcript, and P3's diagnostic evidence.

## Cause confirmation under plan section 8, revision 23al

The red gate is `~/botster-sessions/gates/botster-core-stage1-pc-e1-1-worker-file-name-67fd9112-pool-20261011-092132-45421.log`.
SHA-256: `921a0a1b5e191b688c470b30d3210861070aa8f552d3923804abc55325c78a2f`.
Its header names the head and base above. The wrapper exits 1 after 485 seconds on msa1.
The unmutated stage-2 baseline fails `conf::ou_9_baseline_then_live_no_gap` at seed 0, step 14.
It expects `616674657231616674657232` (`after1after2`) and receives `6265666f7265616674657231` (`beforeafter1`).
The baseline has no retry. This gate remains RED, and fuzz did not run.
The e1_1 trial passes in the slow step and the baseline. The same-run result check names its proof at line 5139.
Those passes do not establish a full green gate or a package CLEAN.

At contracts-v0.1.36, the transcript emits `before`, then calls `await_quiet`, then attaches the route.
It has no sentinel and no `await_title` before that attach.
Quiet does not prove that the worker consumed those PTY bytes before the baseline cut.
A late PTY read can put `before` in live output. That mechanism explains the observed step-14 prefix.

P3's `evidence/p3-attach/ou9-baseline-then-live-late-read-macos.log` reproduces the same seed, step, expected bytes, and received bytes three times.
Its base is v1 `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`, with a local diagnostic patch that holds PTY reads until the first route.
The reviewer read that patch, `ou9-late-pty-read-local.patch`.
The unpatched run in that log passes. These failures come from controlled diagnostic runs on the v1 source, not an unmodified current-tip run.
A second diagnostic patch delays the first read. `ou9-fence-before-delay-macos.log` reports five failures with `before` as a live prefix.
`ou9-fence-after-delay-macos.log` reports five passes for this ID with the fence under the same delay.
The plain before and after logs each report five passes. The reviewer also read the evidence script and the fenced transcript at contracts `c511323252480d39fec4cc22e7da1f1a02043405`.
The fenced transcript appends an OSC 2 title sentinel to `before` and waits for that title before the attach.
Contracts #49 owns this structural fix. The reviewer accepts this cause attribution.

The #276 own diff changes only the e1_1 pending entry, its proof line, and the worker_file_name capability with its existing capability test.
The OU-9 transcript requires only edges_quiet and pty_output; the capability change does not change either answer.
The diff changes no worker read, program control, route, baseline, output fence, or transcript code.
The worker files used by P3's diagnostic patch are unchanged between the diagnostic base and this PR's recorded base.
The reviewer confirms that #276 does not alter the diagnosed missing-fence cause.
The lead decides the one re-gate under 23al. A second red result still requires a source repair before another gate.
The OU-9 structural fix remains open. It needs its reviewed contracts change and Core pin.

The reviewer ran no build, test, gate, mutation, diagnostic patch, or reproduction.
The package review and both HIGH verdicts remain due after exact-head READY with the required gate evidence.
Wait for the doorbell. Never poll.

## Round 1: exact-head package review

- READY: `msg_plugin-w_1791737263_7d00b5`.
- Head: `67fd9112d0e7554d39be6eb6b43260c782dda200`.
- Parent and own-diff base: `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`.
- PR base and green gate header base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
- Tier: HIGH. Contracts pin: `contracts-v0.1.36`, unchanged.
- Plan: 23bl, `~/botster-sessions/pins/stage1-plan.ddc8da81.md`.
- Plan SHA-256: `01c67e1d00ce13f29dc1991c03a52d4469a75aa3d452140648dd4d88a4411fe7`.

The reviewer read the brief first. The reviewer read the plan changes, the pinned controls row, and accepted Core erratum 1.
The reviewer checked the complete three-file own diff, the current PR body, the transcript, and the existing wrapper and suite paths.
Plan 23bh ruling (1) approves this scope. Plan 23bl section 8 governs the one re-gate.
Plan section 5 is unchanged from the previously read 23bk section.

### Source conclusion

Core erratum 1 requires the embedder to supply the worker path. Core fixes no worker file name.
The pinned `worker_file_name` row names a capability: `open` accepts `worker.file_name`.
The tapped real harness now declares that existing capability. The plain harness still declares no controls.
The existing capability test includes the new name and checks both harness forms.

The unchanged wrapper selects the worker binary by role, independently of the file name.
Candidate loading checks the prebuilt worker and anchor against their SHA-256 manifest before the real suite starts.
The guard writes an executable script at the host-supplied name. That script runs the verified anchor and worker.
`RealCoreHarness::open` supplies that path through `OpenConfig.worker_path`.
The transcript uses `worker-copy-without-the-botster-name` and `w.v2` in separate data directories.
Steps 5 and 9 require each real session to reach Running, through the real worker's startup protocol.
The wrapper supplies a real process boundary. The harness does not model the E1-1 result.

The pending file removes only `conf::e1_1_worker_found_by_host_supplied_path`.
Its proof line names its own real-tier trial under the approved #264 form.
The real suite runs that transcript must-pass. The testkit suite keeps its real-only ignore.
The same-run JUnit check requires the exact selected, unignored trial to PASS.
The mapping counts the ID once. The existing AD-6 line is unchanged.
The change alters no runtime, contract, transcript, timeout, mutation profile, pin, or real-pending entry.
The Prior art note identifies the existing v1 wrapper and guard. Old Core supplies no replacement for this harness capability.

PC corrected the green gate base in receipt `msg_plugin-w_1791737363_98c426`.
The current PR body distinguishes base `42d7056e` from parent and own-diff base `a0adb132`.
The current body also uses plan 23bk pin `00635d10`, which contains the 23bh approval.
That correction closes the plan-pin note Sol raised in `msg_plugin-w_1791737319_39cd3b`.
The reviewer verified both corrections through the GitHub API. No source finding or body note remains open.

### Gate and retry evidence

Green log: `~/botster-sessions/gates/botster-core-stage1-pc-e1-1-worker-file-name-67fd9112-pool-20261011-093607-9029.log`.
SHA-256: `1d0d0ea7b0eebfe283f23bb7a7bfaadb8b95577901456cb3d6f54f58f528d5bf`.
Its header names the exact head and base above. All ten steps PASS in 551.9 seconds.
The Linux job exits 0 after 665 seconds. The wrapper exits 0 after 666 seconds.
Default: 1724 PASS, 481 skipped, 17.980 seconds. Slow: 554 PASS, 2020 skipped, 56.923 seconds.
The E1-1 real trial passes in 0.477 seconds. The same-run check names it at line 5138 and checks two proofs.
The baseline also passes E1-1 in 0.466 seconds.
Minimum testkit count is 57/69. Real-passing count is 47/68. Real-accepted count is 48/69.

Stage 1 misses two whole-body `has_control` mutants. Stage 2 catches both.
Final result: two mutants, two caught, zero missed, zero timeout, zero unviable.
The reviewer matched the four raw records and both named catcher records. The log has no FLAKY token.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge-check job.

Lead receipt `msg_plugin-w_1791737498_600484` explicitly accepts the controlled diagnostic for #276 under plan 23bl amendment (A).
That amendment allows this timing-race reproduction to satisfy the plain-v1 condition for this case.
The earlier cause record confirms that #276 does not alter the missing-fence cause.
No unmodified current-v1 failure is claimed. The first gate remains RED. The approved one re-gate is GREEN.
The earlier #264 record has two OU-9 FLAKY outcomes under mutants, not failures of its stage-2 baseline.
This verdict retains that distinction. It uses the lead's explicit #276 approval and the controlled evidence recorded above.
Contracts #49 and the later Core pin still own the OU-9 structural fix.

### Verdict scope

Package verdict: CLEAN (0 open) on this exact head.
Sol's same-head integration verdict remains required.
The reviewed head does not contain PR base `42d7056e`. This green gate precedes the final base import.
After both CLEANs, PC must merge current v1 once, post the base-merge check, and run one final exact-head gate.
This verdict approves no later source delta. The original program-controls group remains incomplete.
Wait for the doorbell. Never poll.

## Sol source CLEAN receipt

Receipt: `msg_plugin-w_1791740907_873ba1`.
Sol verdict: `5e1a9be28abac3fa274f38a9f59ff82cea8ce25c`, `verdicts/stage1/pc-e1-1-worker-file-name.md`, Round 1.
The reviewer read the complete verdict and verified that `origin/stage1/integration-review-sol` contains its commit.
Sol reports CLEAN with zero open findings on `67fd9112d0e7554d39be6eb6b43260c782dda200`.
Both source CLEANs now cover that exact head and distinguish its own base from the green gate base.
This closes the same-head integration-review condition. The final current-v1 import, applicable base check, and exact-head gate remain due.
Neither source verdict approves a later source delta or gate. The OU-9 structural fix still needs its Core pin.
The reviewer ran no execution job. Wait for the doorbell. Never poll.

## Round 2: review after the base merge

- PC receipt: `msg_plugin-w_1791740971_556be3`.
- Merged head: `300eccec3f9c2929e137ee33eb2e8cbe45bf66a8`.
- First parent, previously reviewed head: `67fd9112d0e7554d39be6eb6b43260c782dda200`.
- Second parent, imported v1, new own-diff and PR base: `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
- Plan: 23bt, `~/botster-sessions/pins/stage1-plan.a730792d.md`.
- Plan SHA-256: `a81ae3e282ff913d27f5469abc8c17476c124cf74feaa3e2feb4948bcbcff1ca`.

The reviewer read the brief first. The reviewer read the complete plan changes from 23bo to 23bt.
Sections 5 and 8 are unchanged. The new rulings govern other work and do not alter the E1-1 proof.
The reviewer checked the complete base-check report, merge parents, imported changes in the shared files, and new three-file own diff.
The reviewer also verified the current GitHub head, base, and body.

Base-check report: `~/botster-sessions/shared/core-stage1/evidence/base-merge-check-276-300eccec.out`.
SHA-256: `d8df10a3da9398056b7134f4693950caf4e65f2a117bb253a414fc1fe25ed6dc`.
The report remains FAIL. The proof file conflicted, and both sides changed `real.rs`.
The own diff also has different blob identities and context. Both pending-file checks pass.
The disjoint-base exception does not apply. This round supplies the required package source review.

The head has exactly the two reported parents. PC made one base merge after the reviewed head.
The new own diff contains the same three paths as Round 1.
For each path, the added and removed lines are byte-identical to the Round 1 lines.
The pending file equals the imported base with only E1-1 removed.
The real-pending file equals the imported base without any PC change.

The resolved proof file equals the imported base with exactly one additional E1-1 row.
It retains AD-6 and both LC-2 rows from #280. The new row stays between AD-6 and LC-2.
The same-run proof checker is unchanged. Each proof retains its exact test identity.
The conflict resolution loses no base row and changes no existing row.

The merged capability expression keeps `self.tapped` around the complete control set.
It retains the new oracle-control term from #275 and the PC `worker_file_name` arm.
The imported oracle set contains only its three oracle names. It does not intercept the E1-1 capability.
The capability test retains the PC entry and now chains the imported oracle set.
The plain harness still declares none of those controls.

Candidate validation, the guard wrapper, the E1-1 suite path, and the proof checker are unchanged from the reviewed head.
The imported `real.rs` changes leave `open`, the worker wrapper, and binary selection unchanged.
The E1-1 transcript and contracts pin remain unchanged. The reviewed real-process boundary remains valid.
The imported source does not change the diagnosed OU-9 fence cause.

Package verdict: source CLEAN (0 open) on `300eccec3f9c2929e137ee33eb2e8cbe45bf66a8`.
No package finding remains open. Sol's same-head delta verdict remains required because the base check failed.
PC reports that the one final gate is running. The reviewer has no completed gate receipt for this head.
The earlier gate at `67fd9112` remains evidence for that earlier head only.
PC must record the merge, failed base check, both delta verdicts, and final exact-head gate in the PR body before merge.
This source verdict does not establish the final gate condition or approve another base import.
The reviewer ran no build, test, gate, mutation, reproduction, or executable base check.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.

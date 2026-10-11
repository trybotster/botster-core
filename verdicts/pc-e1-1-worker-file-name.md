# PC package review: worker_file_name capability, PR #276

Status: package review pending. This record confirms the cause for the lead-approved one re-gate only.

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

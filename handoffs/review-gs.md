# GS reviewer handoff

## Current review result

- Repository and PR: `trybotster/botster-contracts#44`.
- Head: `7176d02b577c9043c72ca4eda3b293104aad6961`.
- Base: `3c74af18f0d8b8eba437db133ae5372083e8d97f`.
- Risk tier: STANDARD, confirmed.
- Verdict: CLEAN.
- Open findings: 0.

GS-CD-44-1 LOW is CLOSED. The corrected body states that the changed lines match Core #266 and the disk decision block is byte-identical.

The exact head is unchanged. Exit 75 cannot become green through the reviewed completed-command status path.

The exact-head Contracts gate passes all ten steps in 237.2 seconds. No repeat gate is needed for the body correction.

Contracts #44 waits for its merge receipt or a new head. The lead relays the verdict to the steward.

Core #266 is merged at `2b8cf08230dc3145dfe3a436bc34feee946c9b07`, with final head `928d2c1cb6509219cddaebeecbd6cf013411e24c`.

I verified the merge parents, equal trees, identical own diff, and the exact-head final gate: all ten steps pass in 116.3 seconds.

The lead reports canonical base-merge-check PASS. The next assigned review is the Contracts shared-script split, risk HIGH.

Wait for its exact head. I ran no gate, build, or test.

## Previous Core PR #266

- PR: #266, Docker data-root disk floor.
- Head: `11a9533f8e3929b528b7d4a762f0019e4e18734d`.
- Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- Scope: CI script; the lead requires package review and a pool gate.
- Verdict: CLEAN.
- GS-DF-R1-1 MEDIUM: CLOSED. Every decision measurement now checks status and numeric output and exits the job shell with 75 on failure.
- GS-DF-R2-1 LOW: CLOSED. The live PR body correctly names `measure()` and `free_on()`.

All nine supplied script scenarios pass. Failed and invalid reads exit 75 with zero container starts. The earlier script fails three required exit-code cases.

The exact-head full gate passes all ten steps in 118.7 seconds. The source has no open finding.

Open findings: 0. The exact-head full gate and focused evidence remain valid. No repeat gate is needed.

This verdict covers Core #266 only. Wait for a merge receipt or new head. I ran no gate, build, or test.

PR #265 is merged at `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`. Its reviewed and gated tree is verified.

The #257 stat-only snapshot condition remains.

## Previous PR #265

- PR: #265, data-directory lock.
- Head: `3c8044f40a6b26400240616b8946b05478b72fca`.
- Base: `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.
- Risk: HIGH.
- Verdict: CLEAN.
- Open findings: 0. GS-DL-R1-1 MEDIUM and GS-DL-R1-2 LOW are CLOSED.

The implementer reports a base-merge-check failure on `Cargo.lock`. I performed a source delta review instead of automatic CLEAN carry-over.

The own diff is identical after removal of Git blob index lines. The lockfile addition is still the one dev-dependency line.

Source CLEAN applies to the new exact head. Its final gate passes all ten steps in 153.0 seconds.

Integration completion CLEAN: `ae7ca202f15ccb271a2b7565e3cdb679952dba42` at the new exact head and base, with zero findings.

Sol checked the failed base-merge-check, own diff, imported blobs, merge tree, final body, gate, and both catches. The #257 snapshot condition remains.

New-head gate: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-3c8044f4-pool-20261011-055225-5278.log`.

The default tier passes 1683 tests; the slow tier passes 499. The mutation result remains two caught and seven unviable, with zero missed or timeout.

Pre-import integration CLEAN: `30f8045fe17057fd6ea2075b05b153e33ed8e2e5` at `80f17bcf`.

The evidence below belongs to the previous reviewed head `80f17bcf`, not the new head.

The corrected test makes one reopen per fixed cycle. A channel event starts the cycles, child work is bounded, and cleanup joins every spawner.

The prior-art note and test descriptions are corrected. The probability estimate states its independent-cycle assumption.

All three evidence logs identify this exact head and base. The full Linux gate passes all ten steps in 88.4 seconds.

Mutants: two caught, zero missed, zero timeout, and seven unviable. Both catch identities match their reports.

The Mac evidence passes 123 core-sys tests; Linux passes 122. Each system reports `id_2` passing all 30 concurrent runs.

Reverting the product lock makes the corrected regression fail on the first refused reopen: cycle 15 on Mac and cycle 115 on Linux.

Full gate: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054802-97802.log`.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054238-54403.log`.

Linux evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054725-96840.log`.

Integration review remains required. P5's #257 stat-only handling of `lock` remains an integration condition for the final base.

The lead owns the merge decision and current-base check. I ran no gate, build, or test.

## Previous PR #262

- PR: #262, exact counts for mutation exclusions.
- Head: `de819474f8f0cdf590d9d15638fb9e2a271f5a47`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Risk: HIGH.
- Verdict: CLEAN.
- Open findings: 0.
- Full verdict: `verdicts/gs-gate-speed.md`.

The check preserves the zero-match failure and requires each configured exclusion to match exactly its stated count.

No note means one match. A count note requires ASCII digits, no sign or leading zero, and a value of at least two.

All TOML values stay identical. Removing the 85 added comments makes the file byte-identical to the base file.

The mutation stages, selection, binary builds, and verdict remain unchanged. The check detects count changes, not identity changes with equal counts.

The exact-head Linux gate passes all ten steps in 161.6 seconds. It checks 208 exclusions against 6201 workspace mutants.

Mutants: 16 caught, zero missed, zero timeout, and two unviable. All 16 catch identities match their reports.

The first gate failed on a signed-count parser mutant. The PR retains that log. The added `# +3 mutants` case catches that mutant.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-de819474-pool-20261011-045219-1794.log`.

Integration CLEAN is published in `7a7b414825cdcd4e008f99c7a2d5e8dc103c41c1` at the exact head and base above. No finding remains open.

Sol independently confirmed the full gate, unchanged TOML values, all 16 catch identities, and the signed-count correction.

This unchanged head requires no repeat gate under section 8. The lead owns the merge decision and current-base check.

The lead holds #262 until #243 merges. GS must then merge the new `v1`, supply a base-merge-check, and run one exact-head gate.

The current CLEAN applies only to `de819474f8f0cdf590d9d15638fb9e2a271f5a47`. I will check CLEAN carry-over on the new head.

I ran no gate, build, or test.

## Previous PR #261

PR #261 is merged at `0c5ac80e512f986edd2ec1455012552a41c7f855`. I verified its parents and tree against the exact reviewed head `727a1c50`.

Package CLEAN: `4be62171`. Gate evidence: `994ca99b`. Integration source CLEAN: `7245cd1c`; the lead records integration completion `33be0d49`.

Its final gate passes all ten steps in 446.3 seconds. Stage 2 catches the exact seven stage-1 misses.

The final result has 53 caught, zero missed, zero timeout, and 13 unviable mutants. All 53 catch report identities match the raw catches.

## Previous PR #260

The base-merge-check from `f41e42e0` to `f291a5e6` passes every check. I read the log and independently verified identical own diffs.

Package CLEAN carries over to this exact head. The integration reviewer reports source CLEAN on the reviewed parent in commit `a57964bc`.

I read the one final gate log. All ten steps PASS, total 444.1 seconds. The check reports 1012 slow-only tests and no violation.

Mutants: 38 caught, zero missed, zero timeout, and two unviable. Stage 2 catches all eight stage-1 misses after its baseline passes.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-slow-coverage-f291a5e6-pool-20261011-035822-73822.log`.

Integration source carry-over CLEAN: `90be012585482b8187af03cb652c77bb672c7ded`. The lead owns the merge decision and current-base check.

PR #260 is merged at `b3cb268f3c2cb3223dadef02033d2bee49f06c59`. I verified its exact parents and equal reviewed/gated tree `2388ba83`.

Integration completion CLEAN: `0246777091b6853e4f608bf0eaf64d508b314e2d`. Package completion commit `995350d8` is now pushed after the DNS outage.

The source review found no gate-safety defect. The exact-head gate passes all ten steps and catches 38 of 40 mutants; two are unviable.

The brief requires agreement with P6 on the file split. The PR body says P6's confirmation of the `slow_job` change is pending.

The lead explicitly permits the one call in `slow_job` without P6's reply. P6 may object through a follow-up PR.

The lead reports that v1 is now `4ab8a985`. GS must merge the then-current tip, post the base-merge-check, and run one final gate.

## Previous PR #256

The base-merge-check from `db4fd197` to `0e222c59` passes every check. I read the log and independently verified identical own diffs.

Source CLEAN carries over to the exact new head. I read its final gate log: all ten steps PASS, total 155.9 seconds.

The mutation step reports 39 mutants: 35 caught, zero missed, zero timeout, and four unviable. It prints 35 catching-test lines.

Gate log: `~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-0e222c59-pool-20261011-033134-11371.log`.

The lead owns the merge decision and current-base check. I ran no gate or build.

The integration reviewer reports completion CLEAN on the same exact head and base in commit `3204d4a5dc9615fb0783e38149c4c0e6a765f17e`.

No merge receipt has been reviewed.

The parser now detects a status-line candidate before it parses the known envelope. An unknown candidate returns an error.

The source and PR now state the retry limit. A fail followed by a pass is no catch. Two failed attempts can count as a catch.

I read the earlier green gate log at `a1b009a1`. I reviewed the delta at `db4fd197`. I ran no gate or build.

The final gate at `db4fd197` failed slow EV-5, seed 0, step 21, with `PendingLimit` and detail `pending_ops is 4`.

I verified the failed log. The mutation step did not run. Source CLEAN remains separate from gate acceptance.

The implementer started a second gate and now states that it is diagnostic only. The implementer will not use it as merge evidence.

Plan revision 23al permits one gate retry after trace diagnosis, plain-v1 reproduction, reviewer cause confirmation, and written lead approval.

I have no such evidence or approval for this occurrence. I asked the lead to resolve the gate acceptance condition.

The lead gives no revision 23al approval for `db4fd197`. That head will not be the merge head.

After #255 merges, GS must merge v1, post the base-merge-check, and run one full gate on the exact new head.

The failed `db4fd197` gate stays in the PR body. Package CLEAN carries over only if the base-merge-check passes.

If the new head fails EV-5, the head waits. The lead requires reviewer cause confirmation before any revision 23al decision.

The lead also requires attribution before the new head's gate counts. I confirmed in writing that #256 does not alter the diagnosed EV-5 cause.

The failed step uses the unchanged slow profile. The transcript pin, worker descriptor path, harness, recovery, and `poll_all` behavior are unchanged.

I read R-63 and the retained lead/PC classification. I did not independently reproduce this occurrence. The confirmation does not approve a retry at `db4fd197`.

The lead agreed to withdraw L2. I verified that PR #253 is CLOSED without merge. Its HIGH finding remains unresolved in withdrawn code.

The reuse record omits permission modes. I proved that removing execute permission preserves a file's path and hash but prevents execution.

The implementer accepted the finding. Withdrawal requires no repair to this PR. It does not establish safety.

I ran no gate or build. I ran one temporary executable check to prove the finding.

PR #252 remains package CLEAN at `afc33981` in `c30732d8`. Its final exact-head gate remains required before merge.

PR #251 remains package CLEAN at `7b942de5` in `7b439442`.

PR #247 received package CLEAN in `c7c538a1` and integration CLEAN in `b5df2e0e`. It is now in the base through `5c480db4`.

PR #248 at `41002679` is CLEAN again. GS-B-SI-1 LOW is closed after the Prior art correction, including the exact `WorkerHarness::new` identifier.

I verified the finding in the pinned old source and read the corrected PR description. The reviewed head remains unchanged.

I read the retained base-merge-check log. The final exact-head gate remains required before merge.

## Remaining review work

Core #266 is merged. Wait for the Contracts shared-script split PR, risk HIGH. PR #262 still waits for its new head and evidence.

The lead assigns the next GS review: the `id_2` data-directory lock fix in `core-sys lock.rs`, risk HIGH.

PR #261 and PR #260 are merged. PR #256 source findings remain closed.

Preliminary baseline reuse review found three gaps: binary content identity, unknown test-list fields, and identity capture after the tests passed.

The implementer addressed those code gaps through `4752bbb36b079d241a828410a6c25e89398b2314`. Formal review then found the permission-mode gap. L2 is now withdrawn.

## Branch setup

The initial review branch pointed to `053148f6` outside the Stage 1 v1 history. I preserved it as `stage1/review-gs-initial-053148f6`.

I created `stage1/review-gs` from the reviewed v1 base `f6778ad8`. This branch contains only reviewer records above that base.

## Messaging

- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- Implementer: `sess-1791701398-017b-5108b9bd7fcd4d0c8f03d3df2317b92c`.
- Integration reviewer: `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`.

End the turn while waiting. After a doorbell, call `receive_messages` once. Never poll.

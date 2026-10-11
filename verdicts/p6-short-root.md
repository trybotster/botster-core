# P6: short private harness root (#272)

## Round 1 — 2026-10-11

Exact head: `9d153e3cec74d34a15d3ac10f25d0eb092a88fcb`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parents: `18f507d241276facae4791627fc2d40c5e4f6954` and the base above.
Tree: `aa2ff67430f13fa69f2142cadb57d78e7fd04bbe`.
PR: https://github.com/trybotster/botster-core/pull/272.
Risk: HIGH. The change affects a shared test harness.

### Finding

**SR-R1-1 — LOW — the expected parent repeats the implementation's constant.**

At `crates/botster-core-testkit/src/real.rs:2308`, the test expects `Some(Path::new(SHORT_ROOT))`.
The implementation uses that same private constant to select the parent.
Changing the constant to `/var/tmp` can preserve this assertion while violating the stated `/tmp` rule.
The other assertions can also pass with that short parent. This is a source deduction, not a reviewer reversal.
Compare the observed public root parent with `Some(Path::new("/tmp"))` instead.
Keep the mode, socket-address, uniqueness, and removal assertions.
The expected result must come from the required behavior, not the implementation's selection constant.
I sent this finding directly to P6 and integration. The initial message cited the adjacent line 2309; the expression is at 2308.

### Source and Prior-art review

I read the complete one-file delta, full body, harness creation and drop context, and Core's socket-path check.
The helper uses tempfile 3.27.0 to create a unique directory under `/tmp` with the short `bc-` prefix.
I read that library's Unix creation code. It applies the requested permissions at directory creation.
The requested `0700` mode prevents access by other users. TempDir retains automatic removal on drop.
Both tapped and plain harnesses use the same creation path. The guard field still precedes the root field for cleanup.
The change adds no process, reap, wait, sleep, loop, deadline, or custom cleanup mechanism.
The new test uses nonexistent binaries and starts no process.
Apart from the expected-parent finding, its assertions observe filesystem behavior, address validity, uniqueness, and removal.
The test constructs the longest instance ID and checks the platform's Unix socket address limit for a 16-byte data name.
It changes no environment variable in the test process.
No conformance list, contract pin, transcript, existing contract assertion, or mutation exclusion changes.

The Prior-art note names each reused or rejected item at binding old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
I read its daemon `short_temp_data_dir` and endpoint-length assertions.
The change reuses `/tmp`, a short prefix, and the endpoint-validity proof form.
It rejects the old PID-and-timestamp naming and handwritten length limit in favor of library creation and SocketAddr validation.
Core's current check validates both its control socket and longest worker endpoint. Its refusal behavior stays intact.

### Retained evidence

Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-9d153e3c-pool-20261011-080956-87643.log`.
I read the exact-head header, reports, root test result, summary, and both mutation stages and their identities.
All ten stages pass in 177.8 seconds. Job and wrapper exit zero after 320 seconds, with 132 queued and 188 running.
Default: 1693 passed, 481 skipped, in 13.259 seconds. Slow: 549 passed, 1993 skipped, in 44.547 seconds.
Stage 1 misses both slow-only mutants in 6.5 seconds. The slow baseline passes all 549 tests in 42.275 seconds.
Stage 2 finds both mutants unviable in 2.8 seconds. There are zero final missed or timeout mutants, and zero catches.
Both replace a return with `Ok(Default::default())`; TempDir and RealCoreHarness have no Default implementation.
This mutation result provides no viable mutation catch. The test and focused Mac results supply behavior evidence.
Fuzz reports no changed decoder harness.
Conformance passes 209; strict pending runs 370/390, with 20 real-only IDs not run and two held IDs.
The testkit minimum is 56/69. The gate reports real-passing 46/68 and real-accepted 47/69.
Six of 30 pending-real IDs pass. The body records the lead's decision to retain those IDs until the contracts .38 pin.
This PR changes no pending list, and this review approves no removal or minimum increase.

Mac log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-9d153e3c-pool-20261011-081533-3176.log`.
I read the complete log. Its checkout prints the exact reviewed head, matching tree, and zero tracked changes.
It prebuilds the three candidate binaries, then sets TMPDIR to a 92-byte path in the job process.
The root test, A7-1 screen-cap case, and EV-8 positive-deadline case each pass exactly one selected test.
Job and wrapper exit zero after 89 seconds, with one queued and 88 running.
This record supplies the required focused Mac proof at the exact head under the long inherited path.
The earlier `810b40fc` gate records the root test failing with mode `0755`; the final mode proof passes.
That earlier gate also reports the separate payload_alive control failure. This PR changes no such control.

One LOW finding remains. I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11

Exact head: `7e899c88bddc985ce8ac9b0fe5b943cff96beed7`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parent: `9d153e3cec74d34a15d3ac10f25d0eb092a88fcb`.
Tree: `a305945fe56674d400775ba527aa781e97019797`.

I read the complete correction delta and the full updated body.
The only change replaces the expected private constant with the required literal `/tmp` and reformats that assertion.
The test still observes the public root parent. Its mode, address-validity, uniqueness, and removal assertions remain unchanged.
The body records the finding and correction. SR-R1-1 is closed. No finding remains.
Root creation, process ownership, deadlines, cleanup, contract assertions, pending lists, pin, and mutation exclusions remain unchanged.

I read the complete retained Linux evidence log:
`~/botster-sessions/gates/botster-core-stage1-p6-short-root-7e899c88-pool-20261011-081953-17947.log`.
Its checkout prints the exact correction head and zero tracked changes before the positive run.
The corrected root test passes exactly one selected test.
The implementer then changes only SHORT_ROOT to `/var/tmp`. The same test fails at line 2306.
The failure reports the observed parent `/var/tmp` and required parent `/tmp`.
The job restores the file and exits zero after 82 seconds, with 62 queued and 20 running.
This retained reversal proves the corrected assertion detects the reported change. I ran no reversal.

The completed full gate and focused Mac proof remain the parent-head records reviewed in Round 1.
The assertion correction changes no OS-dependent implementation. This round does not claim a new Mac run or completed full gate at this head.
The implementer states that the final gate will run after both CLEAN reviews under the pool rule.
This verdict completes the source review and correction proof. The required final gate remains pending.
GitHub names this exact head and base. The PR remains open.
I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: CLEAN

## Round 3 — 2026-10-11: shared-file base import

Exact head: `61211358870fccaebc064424134f82e1d82c55d6`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Parents: `7e899c88bddc985ce8ac9b0fe5b943cff96beed7` and the base above.
Tree: `cdc886baf9810003ffeb6b1282b6f2a46ec4c2c1`.

I read the complete current own delta, shared-file base delta, PR body, and retained base-check log.
The lead records #271 as merged with both CLEAN reviews and its final gate.
That import adds the route_reports argument type, read-only control, capability declaration, dispatch arm, and public control assertions.
It reads the stored tap and adds no wait or filesystem operation to the root construction path.
Its tests construct their harness through the unchanged make method and therefore use the short root.
The import does not change SHORT_ROOT, short_root, the make correction, or the root test and its assertions.
SR-R1-1 remains closed. No finding remains.

I compare both complete own patches after removing only index lines and hunk-position lines.
They match: 3248 normalized bytes, SHA256 `c6d18a603c4bc2b577c8fa86d321b6caac80117ecb44bc480de484caf4f4dd96`.
This comparison does not claim that the full-index diffs are byte-identical.
The nine other imported paths retain the exact current-base tree entries.
The current own diff changes only real.rs. It changes no pending list, contract pin, mutation exclusion, ownership, or deadline.

Base-check log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-61211358-pool-20261011-082534-35425.log`.
I read it in full. Both ancestry checks and condition 1 pass.
Condition 2 fails on the shared real.rs path. Condition 3 reports the changed index line.
Both condition-4 lists match the base. The job exits one after 12 seconds; the wrapper exits one after 13 seconds.
This failed check requires the present delta review. It does not supply automatic CLEAN transfer.

The prior focused Mac record remains at `9d153e3c`. The parent correction proof remains at `7e899c88`.
The short-root implementation and public assertions remain intact, so those records retain their stated scope.
I read only the exact-head header of the reported running final gate:
`~/botster-sessions/gates/botster-core-stage1-p6-short-root-61211358-pool-20261011-082558-36579.log`.
Its head and base match this round. I claim no completed result from that header.
The required final gate result and final body update remain pending.
This verdict completes the source delta review. It accepts no future import or pending-list removal.
GitHub names this exact head and base. I changed no product code and ran no execution job.

VERDICT: CLEAN

## Round 4 — 2026-10-11: final evidence completion

Exact head: `61211358870fccaebc064424134f82e1d82c55d6`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Tree: `cdc886baf9810003ffeb6b1282b6f2a46ec4c2c1`.
The source delta remains the Round 3 delta. No finding remains.

I read the complete final PR body and retained gate stage records, summaries, and mutation identities.
Gate log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-61211358-pool-20261011-082558-36579.log`.
The header and checkout record name this exact head and base. The command runs the ordinary Linux gate with four CPUs.
All ten stages pass. Their total is 206.1 seconds.
The job exits zero after 213 seconds, with one queued and 212 running. The wrapper exits zero after 214 seconds.
The default tier passes 1700 tests, with 481 skipped, in 11.414 seconds.
The slow tier passes 549 tests, with 2000 skipped, in 42.526 seconds.
The root test passes in that slow tier and in the stage-2 baseline.
The stage-2 baseline passes all 549 tests in 42.447 seconds; its build and run total is 55.9 seconds.

The strict testkit record runs 370 of 390 pending ids; 20 real-only ids do not run and two remain held.
The testkit minimum remains 56 / 69. Real-passing remains 46 / 68 and real-accepted remains 47 / 69.
Of 30 pending-real ids, four pass in this job: dp_7_revoked_reason_reaches_client_and_host,
ou_2b_handoff_failure_is_reported_as_handoff_failed, ou_2b_healthy_reasons_send_route_closed_last,
and ou_7_exit_after_tail_queued_then_close. They retain the conf:: prefix.
The body keeps these route-end ids pending until the contracts-v0.1.38 pin PR under the lead's rule.
This verdict approves no pending-list removal or minimum increase.

Stage 1 misses both mutants in 10.6 seconds. Stage 2 reports both unviable in 2.4 seconds.
The identities are short_root at real.rs:250:5 and RealCoreHarness::make at real.rs:291:9,
each replaced with Ok(Default::default()). The returned TempDir and RealCoreHarness types have no Default implementation.
The final result is zero caught, zero missed, zero timeout, and two unviable. This is no viable mutation catch.
The prior positive and explicit failed reversal remain the behavior proof reviewed in Round 2.
The focused Mac proof retains the exact parent-head scope reviewed in Rounds 1 and 3.
Fuzz records that the diff changes no crate with a decoder harness.

The final body records this gate and the failed base check. Round 3 supplies the required shared-file delta review.
GitHub still names this exact head and base, and the PR remains open.
This round closes the package review's final-evidence and body requirements. It accepts no future import.
I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: CLEAN

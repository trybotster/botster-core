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

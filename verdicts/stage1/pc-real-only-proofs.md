# Real-only named proofs: integration review

## Design review

- Design: `~/botster-sessions/shared/core-stage1/handoffs/pc-real-only-proofs-design.md`.
- Design SHA-256: `3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811`.
- Intended base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Plan pin: `a2897c1204dbfe6bbc44f476783686a68cabf57a`, revision 23ar.
- PC receipt: `msg_plugin-w_1791719105_4d0b40`.
- Risk: HIGH, because the implementation changes shared testkit selection and the gate.

Sol reads the complete design. Sol checks the referenced suite, slow-coverage check, slow job, test-budget command, and nextest configuration.
Sol finds no open design finding. This conclusion covers the checked design text only.

The separate proof file preserves the generated replacement-map copy.
Each proof line identifies a Core real-only ID and an exact nextest binary/test pair.
The lists check requires proof lines for every non-pending real-only ID and rejects malformed, duplicate, unordered, or unrelated lines.
The slow-coverage check requires each named test to exist, remain unignored, and match the actual slow filter.
The slow-step result check requires every named test to have a successful testcase in that run's JUnit report.
Missing, failed, errored, skipped, or retried-failure results cannot credit a proof.
Thus selection alone supplies no proof credit. A test that skips its work through the harness's ignored result supplies no credit.

The design checks that report after the normal slow run and before the pending-real report run.
The current test-budget command deletes the report before the nextest run. The implementation must retain the same-run report requirement.
Its new result parser must check result children; the existing parse_junit function reads timing attributes only.
Malformed or unreadable result input must not produce proof credit. This follows from the design's fail-closed requirement.

Both suite tiers report the named proof instead of running an unsupported duplicate transcript.
The minimum count names the exact proof source. The gate supplies acceptance through the named real-process test.
Reviewers still read each proof test's clause citation and public behavior proof. The mapping does not establish the test's behavior.
The proposed ignored-trial exception is limited to the real conformance binary and an ID with a proof line.
Other ignored slow-only tests remain violations.

Sol independently compares all 26 real-only IDs with core-pending.txt at the exact base. Every ID remains pending.
The initial empty proof file therefore satisfies the proposed lists rule on this base.
The mechanism PR removes no pending ID and supplies no new minimum pass.
A later proof PR must add the named proof and supply its PASS before removing the ID from pending.

The source PR still needs both exact-head reviews, its Prior art note, and the full exact-head gate.
Its decision checks remain subject to mutation review. This design review supplies no source, gate, or merge approval.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN design (0 open) at SHA-256 3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811

## Package design record

Sol reads the complete package design verdict `3e5046b99845e24adca9db6f528d3c4f46c4da22`.
The fetched stage1/review-pc branch contains that commit. Sol independently confirms its publication.
The file is verdicts/pc-real-only-proofs-design.md. Its design hash equals this review's exact hash.
Both design verdicts are CLEAN with zero open findings.
The package verdict preserves same-run selection and PASS checks, freshness, strict result parsing, and every required proof line.
No implementation is READY. These design verdicts supply no implementation, gate, pending-removal, or merge approval.

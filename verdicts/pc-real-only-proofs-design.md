# PC package review: real-only named-proof design

VERDICT: CLEAN design (0 open)

- Design: `~/botster-sessions/shared/core-stage1/handoffs/pc-real-only-proofs-design.md`.
- Design SHA-256: `3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811`.
- Source base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Plan: revision 23ar, `stage1-plan.a2897c12.md`.
- Plan SHA-256: `2e4d6e962f37ddafe2a68fd50598a1bd218fe3a6a53358f8cccd970884e0daed`.
- Request: `msg_plugin-w_1791719105_06675d`.
- Risk: HIGH. The design changes the shared testkit and gate decisions.

This verdict covers the design before code. It supplies no implementation, test, gate, or merge approval.
The lead handoff records the task and design approval at approximately 08:10 and 08:20 PT.
The lead assigns PC as the writer and names this package reviewer and Sol as reviewers.
The reviewer used the orchestrate-delivery skill to check the changed acceptance premise and evidence boundary.

## Delivery contract and premise

The outcome is proof credit for a real-only ID only through its named real-process test in the same gate.
The mechanism changes suite selection, proof mapping, slow coverage, and the slow-step result check in Core.
PC owns this mechanism under the lead's assignment. P3 owns the first DP-2 proof and its later pending-ID removal.
This mechanism changes no Core runtime, contract meaning, transcript, timeout, or pin.
It initially removes no pending ID and adds no minimum pass.

The reviewer read the suite callers, testkit `pending::non_pending`, slow-coverage selection, slow job, and test-budget report path.
The source confirms the premise:

- `non_pending` exempts a real-only ID only on the testkit tier.
- The real tier then runs its transcript as a must-pass trial.
- `named_proofs` reads a category from `core-real-only.txt`, not a named test.
- The existing JUnit timing parser reads attributes. It checks no result child and cannot establish a proof's PASS.

The reviewer independently compared the real-only and pending sets at the exact source base.
All 26 real-only IDs remain pending. The initial empty mapping therefore meets the proposed lists rule without crediting a proof.
This design repairs the acceptance mechanism. It does not establish the behavior of any future named test.

## Accepted boundary and invariants

The separate hand-written proof file preserves the generated replacement-map copy.
Each line names a real-only Core ID and an exact nextest binary/test pair.
The parser and lists check reject malformed, duplicate, unordered, unrelated, or missing required lines.
Several tests can prove one ID. The report counts the ID once and names all required tests.

Both suite tiers ignore the duplicate real-only transcript and name its proof.
The gate owns execution. The suite starts no nested test process or cargo command.
The ignored-trial exception applies only to the real conformance binary and an ID with a proof line.
Other ignored slow-only tests remain violations. Ordinary conformance IDs retain their existing acceptance rules.

The slow-coverage check must establish that every named test exists, is not ignored, and matches the actual slow filter.
Selection alone gives no proof credit. The slow-step result check must establish PASS for every proof line in that run's JUnit report.
Missing, failed, errored, skipped, or failed retry results cannot credit a proof.
Unreadable or malformed XML must fail closed. An ambiguous duplicate result must not hide a non-PASS result.
These conditions implement the lead's requirement that every named test actually runs to PASS.

The current test-budget command removes the report before the slow run.
The new check must read that run's report after the slow run succeeds and before the pending-real report runs.
The implementation must preserve report freshness and handle a report-removal error without accepting stale evidence.
It must check result children through an XML parser; the existing timing-only regex supplies no result proof.
The documented nextest JUnit format identifies tests by `classname` and `name`.
Source: [nextest JUnit documentation](https://nexte.st/docs/machine-readable/junit/).
Implementation review must check the actual report format from the gate's installed nextest version.

The counts must name the exact proof source for minimum and non-minimum real-only IDs.
The report states the source. The successful gate establishes acceptance.
Each later proof PR must include the test's ID and clause citation, its behavior proof, and its required PASS evidence.
Reviewers must read the test body. A citation or mapping line alone proves no clause.

## Verification and next action

The source PR needs both exact-head reviews, its Prior art note, and the required exact-head gate.
Positive and negative checks must cover the parser, list requirements, selection, result statuses, and both suite tiers.
The result checks must reject malformed and missing evidence and preserve exact binary/test identity.
The gate-decision code remains subject to mutation review. The empty production mapping does not replace nonempty fixture coverage.
The reviewer ran no build, test, gate, mutation, or executable merge-check job.

Sol's design verdict is CLEAN at `211ca91b0e020c4beed0fbb0d1025932858132db` for the same design hash.
The reviewer read that complete verdict and reached an independent source conclusion.
PC can implement the agreed design. Send the exact source head and retained evidence for review when READY.
No implementation finding can be cleared by this design verdict. Wait for the doorbell. Never poll.

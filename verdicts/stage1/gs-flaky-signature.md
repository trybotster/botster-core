# PR #308 integration review

## Round 1 — revised named signature fields

Exact head: `0a9a6cdedd9abb619a65a225bd0bf3d5594a2a9f`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Branch: `stage1/gs-flaky-signature`. Risk: HIGH. Lead assignment: `msg_plugin-w_1791782721_1fa4f3`.

The lead held review of the earlier comma-clause form and required named fields. Sol-B started on this revised head. Sol-B read all three changed files, the parser, CI and lists callers, mutation filters, BUILD rule 9's new exception, and the complete PR body.

The signature is a required table. Its step must equal a name from ci::JOBS through the new step_names iterator. Its symptom must equal one of step_timeout, test_timeout, assertion, signal, or exit_code. Its seed must be "any" or a nonnegative TOML integer. Optional text must be a string with a nonempty trimmed value; the parser retains its original literal value.

The parser rejects unknown signature fields and missing or malformed required fields. The lists checker uses that parser. The mutation runner also uses it before selecting and reporting listed tests. The report displays the recorded signature beside the diagnosis, owner, and removing PR.

The change supplies metadata validation and report formatting. It supplies no matcher, failure attribution, check that a structural fix is open, or retry authorization. Those policy checks remain required by BUILD rule 9. The lead made the matcher optional for this source change; the body identifies it as follow-up work.

The known-flaky list remains empty. The stage_filter, in_stage, and absent behavior remains unchanged. The unmutated tiers and stage-2 baseline retain their behavior. No timeout, budget, seed, deadline, mutation exclusion, product path, or pending status changes.

### B308-I1 — LOW — OPEN: the body overstates refusal coverage

The evidence paragraph says the twelve new cases cover each signature refusal listed above. The test has no non-string symptom case. The parser implements that refusal through the shared string reader, but the test does not exercise every refusal named in the paragraph.

Required closure: describe the actual selected cases and their scope. A new test is not required. Sol-B independently confirms package GS-FS-R1-1 and sent the wording request to GS and 017c.

Package verdict: `9b33f543bbce923ae7796151006e0dd096043e7d`, `verdicts/gs-gate-speed.md`, on this exact head and base. Sol-B read the complete #308 section. It reports the same LOW finding and no parser or filter defect.

### Evidence and remaining requirements

The body reports a local Mac run with 318 of 320 tests passing and two base-merge test timeouts. It reports clippy success and lists success. These remain implementer reports; this review accepts no completed exact-head gate. The broader local run is not green.

The completed gate remains required before CLEAN, as the lead states. The body correction and final evidence also remain due. This review authorizes no execution or retry. Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (1 open)

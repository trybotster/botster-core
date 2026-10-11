# PC package review: named real-process proofs

VERDICT: CLEAN (0 open) at 46094d5b504861e59b66ac8cf16b399841d7ec71

## Round 1 — historical NOT CLEAN at 617c1422

- PR: #264, `stage1/pc-real-only-proofs`.
- Head: `617c14228183770b4fc32264aa875728910462de`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Plan: revision 23ar, `stage1-plan.a2897c12.md`.
- Design SHA-256: `3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811`.
- Design CLEAN: `3e5046b99845e24adca9db6f528d3c4f46c4da22`.
- Request: `msg_plugin-w_1791722173_6c599c`.
- Tier: HIGH. The PR changes the shared testkit, gate decisions, and mutation exclusions.

The lead assigned this mechanism to PC. This review covers its complete 12-file implementation diff and supplied gate evidence.
The reviewer checked logic against the approved design, plan sections 1, 5, and 8, and BUILD.md.
The reviewer verified the PR head and base through the GitHub API.

## RP-PC-1 — MEDIUM — testcase text can supply false PASS evidence

Status: OPEN at `617c14228183770b4fc32264aa875728910462de`. Closed in Round 2 below.

Evidence: `xtask/src/real_proofs.rs:47-89`, especially the regular expressions at lines 49-51.
`results` removes CDATA text, searches the remaining text for testcase tags, and collects apparent child names.
It does not validate XML structure. It does not distinguish XML comments from actual testcase elements.
`verdicts` at lines 104-122 credits a named case when the collected list contains no disallowed child.

For a proof with binary `b` and test `t`, this well-formed document contains no testcase element:

```xml
<testsuites><!-- <testcase name="t" classname="b"/> --></testsuites>
```

The start expression matches the tag inside the comment. The attributes identify `(b, t)`.
The self-closing branch supplies an empty body. The case receives an empty child list and gets PASS credit.

This truncated document also gets PASS credit:

```xml
<testsuites><testcase name="t" classname="b"/>
```

The function never checks the missing root end tag. This malformed case also gets PASS credit:

```xml
<testsuites><testcase name="t" classname="b"><system-out></testcase></testsuites>
```

The function finds the testcase end tag and accepts `system-out`. It never checks that the child closes.
These results follow from a static source trace. The reviewer ran no reproduction.

The approved design requires malformed XML to fail closed and requires an XML parser for result children.
The lead requires actual same-run PASS evidence for each named test. Text inside a comment supplies no such evidence.
The current tests cover missing case end tags, missing attributes, and unclosed CDATA. They omit these document cases.
Sol independently reported the same defect as RP-SI-1 in `msg_plugin-w_1791722276_49f759`.

Required change:

1. Use an XML parser. Validate the complete document before a named proof gets credit.
2. Count actual testcase elements. Ignore comment text and treat CDATA as text.
3. Reject malformed document structure and missing named cases.
4. Add negative cases for comment-only tags, truncated roots, and mismatched or unclosed children.
5. Preserve exact binary/test identity, disallowed-result rejection, and duplicate cases that contain any failed result.
6. Preserve same-run report cleanup and the check before the pending-real report run.
7. Complete the Prior art note for the XML parser choice. BUILD.md requires an ecosystem check and a reason for handwritten infrastructure.
8. Retain a JUnit excerpt from the installed nextest version. Check its identities and result structure against the parser.
9. Keep the result decisions subject to mutation tests. Update exclusions and citations when the parser functions change.

## Source and gate evidence

The other source changes follow the agreed mechanism:

- Both suite tiers use the same `non_pending` treatment and ignore only the duplicate real-only transcript.
- A missing required mapping makes the suite trial fail and the lists check fail.
- The lists check validates mapping identities, order, and duplicate lines.
- Slow coverage checks every named pair for existence, unignored status, and selection by the actual slow filter.
- The ignored-trial exception requires the conformance binary and an ID with a mapping line.
- The count includes each minimum ID once and names all its proofs. The report also names non-minimum proofs.
- `remove_stale` fails on removal errors except NotFound. The slow job checks results before the pending-real report run.
- The new whole-body mutation exclusions cover I/O shells and cite tested decisions. No additional exclusion finding exists.

All 26 real-only IDs remain pending at the reviewed head. The new mapping contains zero proof lines.
The PR changes no runtime, transcript, timeout, contract pin, or pending ID.
It establishes no new clause pass or minimum pass.

The reviewer read the completed Linux gate:
`~/botster-sessions/gates/botster-core-stage1-pc-real-only-proofs-617c1422-pool-20261011-053009-92833.log`.
The log names the reviewed head and base. All ten steps pass. The gate exits 0 after 335 seconds on msa1.
The default tier has 1648 PASS and 484 skipped tests in 10.305 seconds.
The slow tier has 480 PASS and 2002 skipped tests in 29.360 seconds.
The slow step prints `real-proofs: 0 named proofs checked`.
The mutation result is 65 total, 63 caught, 2 unviable, 0 missed, and 0 timeout.
Stage 2 catches the three stage-1 misses in `slow_coverage::check`.
The two unviable replacements require missing `Default` implementations.

The earlier gate at `1c221c6c5902432f201925a82578c8fe3b40c016` missed five mutants and failed.
The reviewed head removes the slice-offset mutations and the equivalent order comparison.
It excludes only the whole-body replacement of `test_budget::command`, with its tested decisions cited.
The current green gate supplies valid execution evidence. It does not close the parser defect.
Its empty production mapping and omitted malformed-report cases do not prove fail-closed acceptance.

The reviewer ran no build, test, gate, mutation, reproduction, or executable merge check.
PC must fix RP-PC-1 and send the exact new head and required gate evidence for delta review.
No implementation CLEAN or merge approval exists at this head. Wait for the doorbell. Never poll.

## Round 2 — XML correction and approved design extension

VERDICT: CLEAN (0 open)

- Head: `c0f295ab65fdbd18e9c05928ef835a9f23e4c992`.
- Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
- READY: `msg_plugin-w_1791732123_fd124a`.
- Plan: revision 23au, `stage1-plan.0af4db8d.md`.
- Plan SHA-256: `8c4f8bb51f2d3defe02822c4ed0b03d6f7a08f5b693950d4746fd6fd8139e3d9`.
- Tier: HIGH. Package and integration delta reviews are required.

The reviewer verified the PR head, base, and corrected body through the GitHub API.
The reviewer checked the complete current 16-file own diff and the changes since the prior review.
The reviewer used orchestrate-delivery to check the changed acceptance premise and retained the existing handoff.
The lead approves the own-trial extension in the 15:56 record of `shared/core-stage1/handoffs/core-lead.md`.
This extension changes the original design's rule that both tiers ignore every mapped transcript.

### RP-PC-1 closure

Status: CLOSED.

`results` now calls `roxmltree::Document::parse` before it credits any case.
It requires a testsuites root and a testcase directly within a testsuite directly under that root.
It reads actual elements and attributes. Comment text, CDATA, and escaped output cannot create testcase elements.
The XML parser rejects truncated documents, mismatched tags, and unclosed children.
The result check rejects every disallowed element at any depth within the case.
Exact binary/test identities, missing-result rejection, and failed duplicate cases retain their previous checks.
The slow job retains the same-run cleanup check and checks results before the pending-real report run.

The reviewer read the parser's negative cases and its real nextest fixture test.
The fixture records nextest 0.9.146, which `ci/remote/Dockerfile` installs.
It contains a first-attempt pass, a flaky pass, repeated failure, and escaped output with apparent XML tags.
The test accepts only the first-attempt pass. An ignored test has no case and receives no credit.
The reviewer checked the pinned roxmltree 0.21.1 source for its tree, text, attribute, and document behavior.
The dependency is confined to xtask. Its memchr dependency already exists in the lock file.
The revised Prior art note records the quick-junit comparison and the roxmltree choice.
The exclusion citations now name the retained parser decisions and tests. The obsolete regex helpers are removed.
These source and evidence changes close every required change in RP-PC-1.

### Own real-tier trial

A mapping can name `conf::<id>`, `botster-core::slow_conformance`, and the same `conf::<id>`.
For this form, the testkit ignores the transcript, and the real tier runs it must-pass.
The lists check rejects a suite mapping to another ID and still rejects every non-real-only ID.
The coverage check rejects an ignored or unselected own trial.
`ignorable_real_only` excludes an ID with an own-trial mapping, even when it also has separate proof lines.
Separate named tests retain the original treatment: both tiers ignore the duplicate transcript.
The result check requires same-run PASS for every mapping line in either form.
The count includes each minimum ID once and prints every required proof source.
The source tests cover both forms, mixed proof sets, near misses, and the suite-line restriction.
The PR records negative results from each supplied rule reversion. The current gate also catches the generated decision mutants.
The reviewer ran no reversion or test.

The first mapping names `conf::ad_6_reused_pid_is_never_killed` and its own real-tier trial.
The base already removed this ID from pending in #258 under R-55.
The reviewer read the pinned AD-6 transcript, its clause-cited expectations, the real controls, and their existing behavior proof.
The transcript checks WorkerGone, no signals at adoption or Remove, and a live bystander.
The controls preserve the recorded start time, change only the pid, end the real worker first, and use Core's identity check.
This mapping credits the existing proof. This PR removes no pending ID.
All four status files and the nextest configuration equal the recorded base.
Of the 26 real-only IDs, only this AD-6 ID is non-pending at the reviewed head.
The PR's opening description now states both forms and the one mapping. Receipt: `msg_plugin-w_1791732210_ee78ed`.

### Merge and gate evidence

The reviewer read `pc-264-base-merge-check-c0f295ab.txt` and the three supplied merge commits.
The report fails because the own diff changed and the base overlaps mutants.toml and known_flaky.rs.
The reviewer completed the required delta review instead of using the base-only exception.
The mutants.toml conflict preserves the base's `# 4 mutants` note and PC's test-budget shell exclusion.
The known_flaky change adds only the new argument at the existing fixture call.
The two later merges have no conflict diff. The own source diff preserves the base's other changes.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-real-only-proofs-c0f295ab-pool-20261011-080948-87049.log`.
SHA-256: `b3da59005323e61c69db4154ce4e8924ed383b49ab5c774b228dbe48d2dc1700`.
The log names the reviewed head and base. All ten steps pass. The wrapper exits 0 after 686 seconds on Linux.
Default: 1705 PASS, 481 skipped, 17.418 seconds. Slow: 550 PASS, 2003 skipped, 56.483 seconds.
The AD-6 own trial passes in 0.461 seconds. The result check prints its exact identity and `1 named proofs checked`.
The report counts real-passing 46 / 68 and real-accepted 47 / 69. This PR changes no pending list.

Mutation stage 1 reports 83 identities: 78 caught, 3 missed, 2 unviable, and 0 timeout.
Stage 2 tests exactly the three misses and catches all three through the named fixture test.
The reviewer matched all 86 raw records and all 81 named catch reports.
The final result is 81 caught, 2 unviable, 0 missed, and 0 timeout.
The two unviable replacements still require unavailable Default implementations.
The parser, mapping, coverage, own-trial, and report-removal tests pass in the gate.
Fuzz reports no changed crate with a decoder harness. The earlier stopped and red gates supply no current acceptance evidence.

### Separate open structural fix

The gate records two FLAKY outcomes for `conf::ou_9_baseline_then_live_no_gap` at lines 5624 and 5626.
Neither outcome counts as a mutant catch. The separate fixture test catches each mutant, so the mutation result remains valid.
The failing step and raw failure text are unavailable. The cause remains undiagnosed.
The reviewer does not attribute the flake to this PR or claim that the flake is fixed.
PC opened `shared/core-stage1/handoffs/flaky-ou_9-baseline-then-live-no-gap-c0f295ab.md` under BUILD.md Testing rule 9.
The reviewer read the record and the lead's 18:21 assignment to P3, before the attach group, with cause first.
The structural fix remains OPEN. This package CLEAN supplies no acceptance or closure for that separate defect.
The record satisfies the required handling of the observed FLAKY evidence.

The reviewer ran no build, test, gate, mutation, reproduction, or executable merge check.
This package CLEAN covers this exact source head and recorded base only.
Sol's integration delta verdict remains required. The current v1 has advanced beyond this gate's base.
After both CLEANs, PC must follow the standing rule: one v1 merge, its base-merge check, and one final exact-head gate.
This verdict grants no approval of a later head or merge. Wait for the doorbell. Never poll.

## Round 3 — module summary correction

VERDICT: CLEAN (0 open)

- Head: `46094d5b504861e59b66ac8cf16b399841d7ec71`.
- Parent: `c0f295ab65fdbd18e9c05928ef835a9f23e4c992`.
- Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
- Correction receipt: `msg_plugin-w_1791732602_8f8d71`.
- Plan: revision 23au. Tier: HIGH.

Sol published NOT CLEAN with one LOW RP-SI-2 at the parent in `6e9511aa7e3474c63d55b9593979ae14f08c2d75`.
The reviewer read Sol's complete Round 2 verdict and the complete five-file correction.
The reviewer verified the new head, unchanged base, and PR body through the GitHub API.
The correction changes only comments and documentation. It changes no executable code, mapping line, pending ID, pin, or gate setting.
The module summaries in real_proofs.rs, pending.rs, and tests/suite/mod.rs now state both proof forms.
DESIGN.md and the core-real-proofs.txt header state the same distinction.
Separate named tests leave the transcript ignored on both tiers.
An own-trial mapping runs its transcript must-pass on the real tier and ignores it on the testkit tier.
The summaries retain the same-run JUnit PASS requirement. The correction resolves the text that RP-SI-2 identifies.
RP-PC-1 stays closed. The Round 2 source review and its evidence remain applicable to the unchanged executable code.

The supplied green gate ran at the parent c0f295ab, not this corrected head.
This review claims no execution at 46094d5b. The reviewer ran no build, test, gate, mutation, reproduction, or executable merge check.
The separate OU-9 structural fix stays OPEN with P3. This package CLEAN does not close that defect.
Sol's integration delta verdict at this head remains required.
After both CLEANs, PC must merge current v1 once, check that merge, and run one final exact-head gate.
This package CLEAN covers this exact source head and recorded base only. It approves no later head or merge.
Wait for the doorbell. Never poll.

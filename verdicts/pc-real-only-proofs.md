# PC package review: named real-process proofs

VERDICT: NOT CLEAN (1 open)

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

Status: OPEN.

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

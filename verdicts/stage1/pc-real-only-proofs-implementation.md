# Integration review: PR #264

## Round 3 — summary correction, 2026-10-11

Head: `46094d5b504861e59b66ac8cf16b399841d7ec71`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parent: `c0f295ab65fdbd18e9c05928ef835a9f23e4c992`.
Tree: `43c79dbec1a227ea1424331347361be8595286ce`.
Package CLEAN: `f9875367aea4d5b2811ac62b2c93c94532892562`, `verdicts/pc-real-only-proofs.md`, Rounds 2 and 3. I read both complete rounds.

I read the complete five-file correction. It changes only comments and documentation: 28 insertions and 19 deletions.
The two module summaries now distinguish separate named tests from an own real-tier trial.
The suite summary, DESIGN.md, and proof-file header state the same distinction.
Separate named tests leave the transcript ignored on both tiers. The own trial runs must-pass on the real tier.
Both forms retain the exact same-run JUnit PASS requirement. RP-SI-2 closes. RP-SI-1 remains closed.
No executable code, mapping line, pending identifier, pin, or gate setting changes.
GitHub identifies this exact head and base. The body is byte-identical to the body reviewed in Round 2 and already states both forms.

The accepted gate evidence belongs to parent head c0f295ab. I claim no execution at this corrected head.
Both source reviews are CLEAN at this exact head and base. The final merge, base check, gate, and final body record remain due.
The lead's one-merge rule governs those actions. This verdict approves no future head or source transfer.
The separate ou_9 cause and structural fix remain OPEN with P3. This source CLEAN does not close that defect.
I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open; source review complete; final exact-head gate pending)

## Round 2 — parser correction and approved design change, 2026-10-11

Head: `c0f295ab65fdbd18e9c05928ef835a9f23e4c992`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parents: `c6dab671539ff9ab0ad392c4e549834eb3d34d7b` and the base.
Tree: `09094039c926614c9b060827b5eda9b29aca08c1`.
Risk: HIGH. Plan 23au applies.

I read the complete sixteen-file own delta, current body, suite decision context, gate ordering, and supplied failed base check.
The check fails conditions 1, 2, and 3. Both pending-file checks pass. The failed result remains failed.
I review the mutation-file resolution as a delta. It retains the base's pending_real count annotation and the two reviewed glue exclusions.
The known_flaky change supplies the new empty proof argument to its existing test. It changes no flaky exclusion.
All 60 imported-only paths equal the base. Both pending files and ci/known-flaky.toml equal the base.
The code pin remains .36. This PR removes no identifier from a pending list.

### Parser correction — RP-SI-1 CLOSED

The raw-text reader is replaced by roxmltree 0.21.1, locked with checksum `f1964b10c76125c36f8afe190065a4bf9a87bf324842c05701330bba9f1cacbb`.
Only xtask uses the dependency. Its memchr dependency already exists in the lock.
I read the library's parse entry point and default options. DTD processing is disabled by default.
The check validates a complete XML document before it collects actual testcase elements.
It requires the testsuites root and direct testcase-to-testsuite-to-root ancestry. Missing name or classname is an error.
Comments, CDATA, and escaped output cannot create testcase elements.
Every duplicate case must pass. An unknown result element or failure element at any depth prevents PASS.
An unreadable or malformed file remains an error even with no proof lines. A missing named identity cannot establish PASS.
The new tests cover each retained malformed-document witness and the missing-case witnesses. RP-SI-1 closes.

I read the complete installed-nextest fixture and its test. The gate image pins nextest 0.9.146.
The fixture contains a first-try pass, a flakyFailure, a failure with rerunFailure, and no case for the ignored test.
Its printed XML is escaped text. The test grants PASS only to the first-try pass.
The body records the parser comparison and explains why the owner rejected quick-junit's permissive model.
I do not claim independent execution of that library comparison. The chosen parser and explicit closed element set satisfy the reviewed requirement.
The original pinned old-Core comparison remains as recorded in Round 1.

### Approved own-trial design

The lead's handoff records approval on 2026-10-11 around 15:56 PT. The initial approved design remains the base decision.
The approved change permits a proof line to name its own real-tier trial after R-55 gave ad_6 a real-tier form.
I use the orchestrate-delivery skill to check this changed premise against the producer, consumer, and result check.
The own form requires exact binary `botster-core::slow_conformance` and test equal to the mapped identifier.
The list check rejects a non-real-only identifier and a different real-tier trial.
The testkit tier ignores the own trial. The real tier runs its transcript must-pass.
Separate named proofs retain ignored transcript trials on both tiers. A missing proof line makes the trial fail.
Slow coverage requires every named test to exist, remain unignored, and match the slow filter.
An own-trial identifier cannot enter the set of ignorable real-only trials.
The slow step checks exact same-run binary/test identities in its JUnit file before pending-real reporting.
The stale-report removal still fails on every error except NotFound. The configured paths agree with the helper.
Both proof forms have positive and negative decision tests. Minimum accounting counts each identifier once and prints the exact proof source.
The checked-in ad_6 line names its own must-pass trial. Its pending removal belongs to earlier v1, not this PR.
The lead's four conditions hold in the code. The next plan revision must record the approved design change.

### RP-SI-2 LOW — module summaries omit the own-trial exception

Status: OPEN at this head.
`xtask/src/real_proofs.rs:3-4` says that the suite never runs the transcript.
`crates/botster-core-testkit/src/pending.rs:7-8` says that neither tier runs the transcript.
ByOwnRealTrial runs that transcript must-pass on the real tier. These unqualified summaries describe the superseded design.
Correct both summaries to distinguish separate named tests from an own real-tier trial. Retain the same-run JUnit requirement.
I sent the finding to PC and the package reviewer. This is the only remaining integration source finding.

### Supplied gate and separate flaky defect

I read the exact-head gate header, relevant named proof lines, summaries, every raw mutant outcome, and all catcher records.
All ten steps pass in 677.1 seconds. The wrapper exits zero after 686 seconds: one queued and 685 running.
Default passes 1705 tests. Slow passes 550 tests. Slow coverage checks 1051 slow-only tests.
The slow step passes the exact ad_6 own trial and checks one named proof from that run.
The minimum remains 56/69 testkit, 46/68 real-passing, and 47/69 real-accepted. Five of 30 pending-real identifiers pass.
Stage 1 tests 83 mutants: 78 caught, three missed, two unviable, and zero timeout.
Stage 2 catches the three missed slow_coverage::check replacements. Its baseline passes all 550 tests.
Each stage's raw caught identities match its catcher records exactly. Final result: 81 caught, two unviable, zero missed or timeout.

Lines 5624 and 5626 report FLAKY `conf::ou_9_baseline_then_live_no_gap` under two xtask mutants.
The gate rejects those results as catches. The known_flaky slow test independently catches both mutants.
The lead assigns the separate defect to P3 under receipt `msg_plugin-w_1791732394_8c5683`.
I read `shared/core-stage1/handoffs/flaky-ou_9-baseline-then-live-no-gap-c0f295ab.md`.
Its cause and structural fix remain OPEN. The owner reports no retained raw failure artifact for this green gate.
This review neither diagnoses the cause nor closes the defect. It approves no retry campaign, re-pend, or flaky exclusion.
The lead permits #264 to proceed unchanged. The valid independent catches remain evidence for this PR.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-real-only-proofs-c0f295ab-pool-20261011-080948-87049.log`.
SHA256: `b3da59005323e61c69db4154ce4e8924ed383b49ab5c774b228dbe48d2dc1700`.
The earlier red, stopped, and superseded gates retain their stated status. I do not claim execution at a future correction head.
The published same-head package verdict remains pending. I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: NOT CLEAN (1 open)

## Round 1

- Head: `617c14228183770b4fc32264aa875728910462de`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Parent: `1c221c6c5902432f201925a82578c8fe3b40c016`.
- Tree: `d32b70485f40e3c7dc9e02d21cc3d1bfb1a3350c`.
- READY: `msg_plugin-w_1791722173_849112`.
- Risk: HIGH. Plan 23ar applies.
- Approved design: hash `3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811`, Sol verdict 211ca91b.

Sol reads the complete twelve-file source delta and complete PR body.
Sol reads the new JUnit check, surrounding minimum calculation, trial ordering, slow selection, and gate sequence.
The static diff check passes. The proof file contains only its header. No pending ID, pin, or minimum result changes.
The malformed-JUnit requirement from the approved design remains binding.

### Source behavior

The mapping parser requires three tab-separated fields and no whitespace inside a field.
The list check requires sorted unique lines, only real-only IDs, and a mapping for every non-pending real-only ID.
The suite ignores a mapped real-only trial on both tiers. A missing mapping makes that trial fail.
The minimum calculation filters the named-proof IDs through the minimum set and prints each proof's exact binary and test.
The slow-coverage check requires every mapped test to exist, remain unignored, and match the slow filter.
The slow job retains slow-coverage and checks the same tier's JUnit file before pending-real.
test_budget removes that profile's old report before the nextest run and fails on every removal error except NotFound.
The configured JUnit paths and the new path helper agree. No stale report is deliberately retained.

The result decisions reject recognized failure, error, skipped, flaky, rerun, and unknown child elements.
They require every duplicate case for an identity to pass and require an exact binary/test match.
These decisions do not establish a complete XML parse. RP-SI-1 below remains open.

The two new whole-body exclusions cover check_slow_run and test_budget::command as process glue.
Their reasons name tested decisions and required gate output. The parser, verdict, selection, and report-removal decisions remain mutation-tested.
The existing slow_job reason now names the proof check. No timeout or known-flaky entry changes.
This review accepts no exclusion of the parser defect and no waiver from the green mutation result.

### RP-SI-1 MEDIUM: raw XML text can establish a false PASS

Status: OPEN. Location: xtask/src/real_proofs.rs, results and verdicts.
results removes CDATA, then searches raw text for testcase tags. It does not parse comments or the document's structure.
For a mapping with binary b and test t, this well-formed XML has no actual test case:

```xml
<testsuites><!-- <testcase name="t" classname="b"/> --></testsuites>
```

The start regex finds the comment's tag. The attribute regex supplies b and t.
The self-closing branch supplies an empty body. The child list is empty, so verdicts reports no problem and grants PASS.
The same search accepts a self-closing case inside a truncated or mismatched outer document.
It can also accept an unclosed allowed child inside an otherwise terminated testcase.
Thus the same-run file can establish a named proof without an actual passing XML test case.
This conflicts with the design's fail-closed malformed-file and missing-case requirements.
The checked-in empty mapping supplies no currently inflated ID. The defect affects the mechanism intended for future removals.
This is a static source trace. Sol runs no reproduction and claims no observed malformed pool report.

Use XML document validation and count actual testcase elements.
Reject malformed document structure before granting any named proof PASS.
Cover a comment-only tag, malformed outer structure, an unclosed child, and a missing named case.
Preserve CDATA handling, exact identities, duplicate-case failure checks, and same-run report freshness.
Record the parser choice and its ecosystem comparison in Prior art, as the package reviewer also requires.
Owner and package reviewer receive the finding in msg_plugin-w_1791722276_8296ec and msg_plugin-w_1791722276_49f759.
Package receipt msg_plugin-w_1791722308_f7885a independently agrees and adds the unclosed-child witness.
Sol reads the complete package artifact `f07dc3f2a643fae93e8c0c818b448acb05b2d44d`.
The origin/stage1/review-pc ref contains that commit. Its verdict is NOT CLEAN with one MEDIUM finding on this exact head.
RP-PC-1 and RP-SI-1 identify the same defect. The package artifact also requires an installed-nextest report excerpt.

### Prior art

Sol reads OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its conformance module wraps public runtime APIs and uses explicit skip reasons. It supplies no named-proof mapping to reuse.
Sol's search of the pinned crates finds no JUnit, real-only, or real_only text.
The current list parser, BTreeMap identities, slow-list check, tier selection, and verdict helper are reused.
The new handwritten XML reader has no recorded ecosystem choice. RP-SI-1 requires that comparison with its parser correction.

### Supplied gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-real-only-proofs-617c1422-pool-20261011-053009-92833.log`.
SHA256: `b80191d4fdd43bec86ec20dfd57078bb11e900a0e520295694211788c6813222`.
Sol reads the exact header, relevant named tests, tier summaries, baseline, every raw mutant identity, catch reports, summary, and exit.
The gate names the exact head and base above. All ten steps pass in 325.1 seconds.
Jobq exits zero after 334 seconds. The wrapper exits zero after 335 seconds.
Default reports 1648 PASS, 484 skipped, and 10.305 seconds. Slow reports 480 PASS, 2002 skipped, and 29.360 seconds.
Slow coverage checks 1014 slow-only tests. The new mapping, result, coverage, and stale-report tests report PASS.
The proof check reports zero named proofs. This header-only gate proves no mapped real-process outcome.

Stage 1 reports 65 distinct identities: sixty caught, three missed, two unviable, zero timeout, in 132.7 seconds.
The baseline passes 480 tests, six slow, with 2666 skipped in 30.154 seconds. Its complete step takes 37.7 seconds.
Stage 2 tests exactly the three missed slow_coverage::check replacements and catches all three in 64.5 seconds.
Their named catcher is known_flaky::slow_tests::each_stage_leaves_out_every_listed_test_that_it_builds.
Sol independently parses all 68 raw records and matches the three stage-2 identities to the stage-1 miss multiset.
Sol independently matches all sixty stage-1 and three stage-2 catches to their named reports.
The final result is 63 caught, two unviable, zero missed, and zero timeout. Fuzz passes with no changed decoder-harness crate.

Sol also reads the earlier failed gate's exact header, raw missed identities, and exit.
The 1c221c6c gate misses five identities in both stages and exits one after 828 seconds.
The corrected source replaces equivalent slice arithmetic with split_once, replaces the order test with cmp, and adds the process-glue exclusion.
The earlier failed gate stays failed. Neither gate covers the required structural XML correction.

This verdict accepts no merge, pending removal, new minimum count, later source correction, or later import.
The correction needs its applicable package and integration delta review and exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at 617c14228183770b4fc32264aa875728910462de

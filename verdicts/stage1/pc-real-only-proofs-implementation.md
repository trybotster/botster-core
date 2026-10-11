# Integration review: PR #264

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

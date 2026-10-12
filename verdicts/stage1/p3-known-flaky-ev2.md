# Core #317 — EV-2 known-flaky entry

## Round 1 complete — package review

Exact head: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Base and branch point: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Tree: `967b0c056186ed2aeca334898247a8dcb6cd8792`.
Immediate parent: `fd13652b54ed4f1f262fec6e4dd5ed61a7cab827`.
READY receipt: `msg_plugin-w_1791824755_ebcc9e`.
Tier: HIGH because the PR changes ci/known-flaky.toml.
The lead assigns Sol the package review and Sol-C the additional integration review.

### Governing disposition

I read complete R107-SI-2 at contracts `a10b85c363b069f8e3f98cdb6597f4451454c98b` and at `28275d852946ee187a16d8543b2c260facba14b8`.
It supersedes R107-SI-1's EV-2 control/fence and two-fix removal conditions.
Contracts #89, `d764f4731691ba0e93daf15c6a69e1c15e77e302`, already adds await_title TB after the final title output.
I verify its source diff and its inclusion in contracts-v0.1.45, commit `c9cb6daf79e0928a39e2821ec90b29ea222950f8`.
That title fence proves the observation before Stop. The fenced EV-2 id no longer tests the OU-7 tail drain.
The entry therefore leaves in the Core pin PR that takes .45 or later. A31 no longer gates this entry's removal.
A31 retains its separate amendment/id and evidence requirements. This review accepts no new OU-7 id or status change.

I read the lead's complete updated disposition and BUILD's Testing rule 9 and merge-train rule 2.
Those rules permit one retry only for the listed exact signature with an open structural fix and owner.
They do not make an unmutated failure pass. They permit no second retry after another red.
The withdrawn retry of train 03709a0c stays withdrawn. A future train-4b needs its own reviewed head and fresh gate.

### Complete source audit

I read the complete two-file diff: 17 added lines, four deleted lines.
Full-index diff SHA256: `7d1572807eb38d9abd05ebd26e4c76db5cbc27bf48e2674089308f1b85b8dde6`.
Git diff --check passes. I run no executable check.

The TOML has exactly one entry with the five required fields.
It names botster-core::slow_conformance and conf::ev_2_mandatory_and_keyed_never_dropped.
The diagnosis names the pre-.45 fence gap, queued acknowledgement, link-only quiet check, contracts #89, and the exact step-timeout signature.
The wildcard covers the seed only. The id, real binary, step 71, and step_timeout stay exact.
The owner is RC. removed_by is #316, the open draft pin PR.
I read #316's complete current body. It plans the .46 pin, entry deletion, and restoration of the empty-list test in one commit.
No implementation in that draft is accepted by this review.

This member still pins .43. Its EV-2 transcript is byte-identical to the .44 transcript inspected in the train review.
The same unfenced form and step signature apply. The planned train imports #312's .44 separately.
The entry changes no control, transcript, matcher, timeout, public API, or Core behavior.
Neither EV-1 nor EV-2-retired gets an entry. Their R-57 fence work remains separate.

The xtask change replaces the obsolete empty-list assertion with an exact one-entry assertion.
It still parses the repository file through the existing parser. It refuses missing or additional test names.
The production prefix before cfg(test) is byte-identical to the base.
Parser validation, stage filters, reporting, and stale-entry checks remain unchanged.
Mutation stages omit only the named test when their own built test list contains it.
The slow tier and unmutated Stage 2 baseline still run the named test. No new failure suppression enters this PR.

All ten conformance paths, Cargo.toml, Cargo.lock, mutants configuration, nextest configuration, and the remote job script equal the base.
No pending removal, ledger change, budget change, mutation exclusion, or process allowance is added.
Old Core 72b2e335 has neither known-flaky file nor its xtask module; it supplies no earlier form of this entry.

### Completed exact-head gate

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-known-flaky-ev2-9fe97f50-pool-20261012-100218-65007.log`.
SHA256: `dbaae2ff15cb744c4e1c62f6a7bf38b1ea051eae2a88e739622ccbef1eef5028`.
I read its exact head/base, pool snapshot and target identity, relevant step records, test results, summaries, and final exits.
All ten CI steps pass on msa1. CI total is 160.0 seconds.
Jobq and the wrapper exit zero after 169 seconds; queue one, run 168.
Default has 1936 passes, 431 skipped, and 11.054 seconds.
Slow has 621 passes, 2069 skipped, and 40.317 seconds.
The EV-2 trial passes in both tiers: 0.163 seconds on the testkit and 0.113 seconds on the real tier.
The revised repository-list test passes in 0.002 seconds.
The two conformance summaries each have 261 passes and zero failures.
The strict report checks 319 of 336 pending transcript ids, with seventeen real-only and four held.
The held-full run covers three budget ids at 32 seeds and reports no failure in 22.09 seconds.
The mutation listing finds no mutant in this test-only Rust diff, so no mutation run starts.
The fuzz step finds no changed decoder crate. Both steps report PASS under their existing rules.
No RETRY, FLAKY, or INCONCLUSIVE line occurs in this completed log.
This gate gives no measured example of an excluded mutant catch; the unchanged filter source defines that behavior.

The earlier 23dbc478 gate stays RED at the obsolete empty-list test; slow, mutants, and fuzz did not run.
Its SHA256 is `6da3789e4b4eb2c9c06d6cbb44281d493c9d9bbaf6242776dfa56046cb9405fd`.
The fd13652b test commit repairs that assertion. Its gate was stopped before completion when the ruling changed.
That stopped log exits 130 and supplies no completed gate acceptance.
The final commit changes the diagnosis and test comment to R107-SI-2. Its own completed gate is the accepted record above.

### Local provenance and LOW KF317-1 — closed

I read the local-check log's header, failed summaries, exits, lists result, and isolated-run results.
Its header names fd13652b with both changed files dirty. It does not identify the exact final committed tree.
Fmt and lists exit zero. The full xtask run has 319 passes and two timeouts at 2.003 and 2.005 seconds, exit 100.
The isolated run passes those tests in 1.494 and 1.621 seconds. It does not erase the earlier timeout record.
The earlier lists log records cargo clean exit zero and raw removal output on the P3 target.
It does not prove every later target writer. These local records are supporting evidence only.
The exact-head pool snapshot supplies the decisive current-head evidence.

The original body mislabeled that local log as head 9fe97f50 and called 838e4bb0 the immediate parent.
KF317-1 required correct local provenance and branch-point labels, with no new job or source change.
Requests: `msg_plugin-w_1791824929_40e70a` and `msg_plugin-w_1791824929_872f42`.
Owner correction: `msg_plugin-w_1791824954_cf65c1`.
I verify unchanged GitHub head and base. I read the complete original body and the complete corrected body delta.
The correction labels fd13652b's pre-commit working tree and its limits, the separate exact-head pool gate, and the branch point.
KF317-1 closes. No source or body finding remains.
Final raw JSON SHA256: `f6aa403386415d2aa90b85d3ef8261c31bce1adf19ee0b1cbb3567af18e93892`.
Body SHA256 without a terminal LF: `bb5b1335a54be139851a7387d9291d929085f6b9636ef6e89d2a157c6aadf01e`.
The owner's gh-style body-plus-LF hash is `d5c14b3f52ec90776053f9f0bc1e10a189aff3f7c1b7e60da0e55df44d32f707`; I independently reproduce it.

### Verdict scope

No package finding remains, including LOW findings.
This CLEAN covers the exact #317 source, corrected body, ruling scope, and completed pool gate.
Sol-C's matching exact-head integration verdict remains required before merge.
The lead owns Q7 ancestry, equal-tree merge, and remote verification.
The .46 removal, empty-list restoration, future train delta, and final train evidence remain separate required work.
This verdict accepts no future retry, later head, pin, status removal, or merge.
I ran no build, test, gate, mutation, reversal, reproduction, measurement, or executable base check.

VERDICT: CLEAN (0 open; #317 package at exact head 9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec against base 838e4bb01dda53c419ab824913fdc5adae7a16c1)

## Matching integration receipt — both exact-head reviews complete

Sol-C receipt: `msg_plugin-w_1791825317_8ccaab`.
Integration verdict: `ee3dd333685af0bca99a8e879db78cebe620690f`, complete Round 1 in verdicts/stage1/p3-known-flaky-ev2.md.
I read the complete integration verdict. It reads the complete package verdict at `3ec18908dccaccfed7f09276e5a34edd31f24dcb`.
Both reviews name the same head, base, tree, two-file scope, corrected body, ruling, and completed gate.
Both close KF317-1 and identify zero open findings.
Both preserve the local provenance limits and the historical RED and stopped gates.
Both keep the .46 entry removal and empty-list restoration separate from A31.

The two exact-head reviews are complete and CLEAN.
The lead's ancestry, equal-tree merge, and remote verification remain due.
This receipt accepts no merge, later head, train-4b gate, or future retry.
The old train stays RED and will not be re-gated. I ran no execution job.

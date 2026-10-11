# Integration review: #248, candidate verification per process

VERDICT: CLEAN (0 open)

- Head: `41002679c4a9c24bb16b75c7735bc950bdc6f753`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Risk: HIGH under the lead's GS rule.
- Package CLEAN: `fa98da4f234691ff9aaea5fbde5f036e7c5a155c`. Sol read the complete #248 section and its finding history.
- Open findings: zero, including LOW.

Sol read the complete one-file diff, candidate verifier, harness setup, suite, and relevant gate process paths.
Sol ran no build, test, gate, mutation, measurement, reversal, or executable merge-check job.

## Source and lifecycle

The only changed file is `tests/slow_conformance.rs`.
The first harness still calls Candidate::locate, which checks all three binaries against their manifest hashes.
A standard-library OnceLock stores the Result for that test process. Each later harness clones the verified candidate paths.
The lock stores an error as an error. Later harnesses receive the same failure; the cache creates no candidate from it.
The cache starts empty in each new test process, including a process for a mutant run.
It shares no verification result between baseline and mutant processes.

The current gate prebuilds candidates before tests. Harness setup writes wrappers and data only within each temporary root.
It does not rebuild or change the candidate binaries between harnesses.
The cache assessment applies to that lifecycle. It does not accept a later binary replacement within one live process.
Tapped and plain trials still run. Seeds, assertions, limits, pending selection, and all verdict rules remain unchanged.
The PR changes no timeout, dependency, production crate, process ownership, OS adapter, or mutation exclusion.

The existing prebuilt-worker gap remains: mutation runs do not rebuild target/candidate for each worker mutant.
This cache does not make an unmutated candidate represent a worker mutant. It adds no acceptance or exclusion for that gap.
Sol found no path in this diff that passes a test or mutant that the prior lifecycle would reject.
The mutation rules and binary dependency requirements still apply to any later GS lever.

## GS-B-SI-1 LOW: incorrect Prior art

Status: CLOSED. The source head did not change.

The initial body said that OLD CORE had no verification for each harness. Sol found that statement incorrect.
At `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`, core_harness_test.rs WorkerHarness::new calls real_worker::WorkerBinary::from_env.
The verifier checks executable size, SHA-256, and a nonempty source revision against its manifest.
Those source paths have no process cache to reuse.
Sol read the old constructor and verifier. The package reviewer independently verified the same source.
The corrected body names WorkerHarness::new and those checks. It identifies current Candidate::locate and std::sync::OnceLock as reused components.
The correction satisfies Prior art. This PR adds no custom cache implementation or verification implementation.
Sol also corrected the too-broad Prior art statement in the separate #247 verdict at `3f47d3ec`.

## Retained proof

Pre-review gate head: `b4072c19135fde605de58e27b02c5cf0fe562a3f`.
Log: `~/botster-sessions/gates/botster-core-stage1-gs-verify-once-b4072c19-pool-20261011-011150-36320.log`.
The header uses base f6778ad8. The source includes #247's reviewed profile.
All ten steps pass. Default tests pass 1610 cases in 10.036 seconds. Slow tests pass 427 cases in 26.849 seconds.
The summary reports 183.5 seconds. The job exits 0 after 189 seconds; the wrapper exits 0 after 190 seconds.
The mutation step finds no mutant in the diff and starts no mutant run. The fuzz step has no changed decoder harness.
This gate result is not a comparison of mutant outcomes.

Measurement log: `~/botster-sessions/gates/botster-core-gs-verify-once-b4072c19-evidence.log`.
Sol independently compared its first slow PASS records with #247's retained gate.
Both contain exactly the same 427 PASS names. Both contain 128 real conformance IDs.
Their real conformance time sums are 28.062 and 19.157 seconds. These sums are not wall times.
The measurement slow suite passes 427 tests in 26.922 seconds.

The measurement appends one byte to the candidate worker, then starts two selected tests in separate processes.
Both tests fail with the unchanged worker hash-mismatch diagnostic. The test command exits 100.
The later prebuild restores the candidates and exits 0. The measurement wrapper exits 0 after 149 seconds.
Sol read the command, both complete failures, restore result, and exit records.
These probes prove first-verification rejection. They do not prove repeated verification in one process.
The stored Result provides the source proof that a verification error remains an error for later harnesses.

The reviewed import has parents b4072c19 and 5c480db4.
Its tree, `46404c87f0cd966307460b9667d9bb2a47a2efa8`, equals Sol's independent git merge-tree result.
Both complete own diffs match: 1417 bytes, SHA-256 `aff29270eb7c90c2d604d70f24d7cba275c6541a9c35eb3e9f5fe00b180a9340`.
Sol read all four PASS conditions in the retained import log:
`~/botster-sessions/gates/botster-core-gs-verify-once-base-merge-check-b4072c19-41002679.log`.
Both pending files remain equal to the base. The PR removes no ID and awards no minimum progress.
R-59 still leaves AD-7 unfenced and outside the counted real minimum.

The final gate on this exact head remains required after both CLEANs under the GS brief and lead's pool rule.
This is source CLEAN. It covers no other GS lever and grants no acceptance for the prebuilt-worker gap.


## Base-only import record

The lead reports #248 merged at cff50d0f575e6135a357e41fc9001f91b90b6a78 into v1 80d02327472f8b3822a6235a2ac663f5ee7c0bbb.
Sol read the complete retained check:
`~/botster-sessions/gates/botster-core-gs-verify-once-base-merge-check-41002679-cff50d0f.log`.
Both ancestry checks and all four numbered conditions pass.
The five base paths are disjoint from this PR's own tests/slow_conformance.rs change.
Both pending files retain the new base's contents.
Sol independently computed git merge-tree for 41002679 and b7c97b42.
Its tree equals the imported tree, ac7afc29c353c26f88c49f9b30a0f4a3c5092648.
Sol independently compared the complete own diffs. Both are 1417 bytes and byte-identical.
Their SHA-256 remains aff29270eb7c90c2d604d70f24d7cba275c6541a9c35eb3e9f5fe00b180a9340.
The source CLEAN carries to this verified base-only import under section 8.

The handoff records the older b4072c19 gate and both import checks as the merge evidence.
Sol has not received a full gate on cff50d0f. The import check is not a full gate and explicitly preserves that requirement.
Sol asked the lead to identify any written pool-rule override for omitting the final exact-head gate.
This record confirms source carryover. It makes no new-head gate claim while that question is pending.
Sol ran no execution job.

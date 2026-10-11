# P5 LC-2 real-process proof review

## Round 1 — 2026-10-11

- PR: https://github.com/trybotster/botster-core/pull/280.
- Branch: `stage1/p5-lc2-real-proof`.
- Exact head: `cb7eb0f7c85936ff742f4b9248807048d8bd1a3f`.
- Tree: `443067478ac99554c2fa81b76754741f1323b1e2`.
- Base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
- Parents: `4bc9089f7bd31041ca92782f7c8cbb7e1ded040e` and `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
- Source head: `4bc9089f7bd31041ca92782f7c8cbb7e1ded040e`.
- Original base: `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`.
- Review request: `msg_plugin-w_1791739141_9c4842`.
- Updated body receipt: `msg_plugin-w_1791739215_8fa72c`.
- Tier: STANDARD. This PR changes tests and proof records for one package. It changes no production, gate-decision, shared-crate, unsafe, pin, or HIGH-path code.

### Scope and source

Core LC-2 requires an exclusive data-directory lock.
A second open fails with DataDirInUse. The lock ends when the handle drops or its process dies.
I read the clause, the complete own delta, the existing same-process test, and the relevant process helpers.
I also read the existing lock test that supplies the child-entry pattern and the proof-record checker.

The existing proof tests refusal in one process and release after handle drop.
The new proof runs the same test binary as a holder in another process.
The child opens Core before it prints and flushes `held`.
The parent waits for that line before it attempts its own open.
The refusal must have code DataDirInUse.
The parent then sends SIGKILL to its owned child and observes an exit caused by signal 9.
The successful open after that exit proves release after process death. No Core destructor runs in the killed process.

The child command sets the holder variable only in its own environment.
The holder has no session or worker. The test owns the child with OwnedChild.
The parent keeps the stdin pipe open until the kill.
The holder's read uses the existing ten-second cleanup deadline.
The first-line read, later line reads, and child-exit wait are bounded.
OwnedChild kills and reaps its child on panic and other drop paths.
The test adds no sleep, global test lock, timeout change, or production hook.
The PR supplies a prior-art note and reuses the existing child-entry form and process helpers.

The proof file names both tests for `conf::lc_2_data_dir_is_exclusive`.
The unchanged checker requires each named test to pass in the slow step.
The named-proof form does not substitute a transcript pass.
The real-only marker remains `slow:data-dir-lock`.

### Base merge

The own delta has three paths.
The base imports nine paths and overlaps only `conformance/core-pending.txt`.
Thus, the whole own diff is not byte-identical across the base merge.

I checked the overlap directly.
The final pending file equals the new base file with only the LC-2 line removed.
Both other own files equal their source-head versions.
The complete final tree equals the new base plus these three changes. I checked all 334 tree entries.
The final diff SHA256, with full blob identifiers, is `b09aa45ce2af11010f2e2e742f1b8fb43903de2d0574df6a74f9b484085acab6`.
`git diff --check` passes. I ran no executable merge-check job.

### Supplied execution evidence

The Linux gate is `gates/botster-core-stage1-p5-lc2-real-proof-cb7eb0f7-pool-20261011-100629-40378.log`.
Its SHA256 is `51abdff21447ea4170115932cd5c975cb49352945e21e5076a98ee385a016436`.
Its header names this exact head and base.
All ten steps pass in 300.3 seconds.
The default suite reports 1729 passes and 480 skips in 12.778 seconds.
The slow suite reports 555 passes and 2022 skips in 40.405 seconds.
Both named LC-2 proofs pass in 0.003 seconds.
The real-proofs check reports both LC-2 records passed and checks three named proofs in total.
The diff has no mutant. The mutation step starts no run.
The diff changes no decoder harness. The fuzz step starts no run.
The job and wrapper exit with zero after 387 seconds.

The focused macOS evidence is `shared/core-stage1/evidence/p5-lc2/lc2-mac-cb7eb0f7.log`.
Its SHA256 is `a09025b11bfcf2c726a0cb808a9d0125a1b52a1ca9d2bd8901e03672faf0cc97`.
The script is `lc2-job.sh`, SHA256 `7233aa64b7303ce2dff8e21787ce8fa363d561523ee32cecccef67c9cf1f417a`.
Both logged commands exactly match that script.
The log names this exact head and Darwin 27.0.0.
Each proof passes five times, with indexes 1 through 5 and no duplicate record.
Each run reports one pass and zero failures. Times range from 0.06 to 0.11 seconds.
The job exits with zero after 13 seconds. The wrapper exits with zero after 14 seconds.
The script filters output and does not propagate cargo exits through its pipeline.
The retained test-result records establish the ten passes. The wrapper exit alone would not prove them.

The full macOS gate remains red.
Its log is `gates/botster-core-stage1-p5-lc2-real-proof-cb7eb0f7-pool-20261011-101307-53515.log`.
Its SHA256 is `0eda477ef3e76cb37002fd45b7353e91d6e0dbd18480bacaaf047b98f09fd27a`.
The default suite reports 1729 passes, one failure, two timeouts, and 480 skips.
The failed test is `mutants_cited::tests::the_tracked_files_come_from_git`.
The timed-out tests are `base_merge::tests::an_unreviewed_change_after_the_merge_still_fails` and `signals::tests::the_repo_has_no_raw_signal_call`.
The slow, mutation, and fuzz steps did not run.
The wrapper exits with one after 250 seconds.
This PR changes none of those tests.

The lead's live handoff states that #280 uses the Linux gate for merge.
The lead assigned the macOS failures to GS.
P5's updated body reports the pool lead's TMPDIR diagnosis and compute-pool fix `05c142e`.
I did not independently review that pool fix.
The accepted Linux gate and focused macOS proof supply the required evidence for this PR.

I read the log headers, proof records, test summaries, gate summaries, and exit records.
I ran no tests, builds, gates, mutation jobs, or product commands.
I changed no product code.

No finding remains open.

VERDICT: CLEAN

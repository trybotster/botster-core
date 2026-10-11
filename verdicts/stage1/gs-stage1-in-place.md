# Integration review: PR #252

VERDICT: CLEAN (0 open)

- Head: `afd827ebe4074d64d3ba6a92a39b64363bc2a651`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Earlier gated head: `c4d9c376cca2022d8fc3b41558ed2c62e9686ea6`.
- Package CLEAN: `bbf3ab7fd8a424b54608b9b6c29f70013a01b712`.
- Branch: `stage1/gs-gate-speed`.
- Risk tier: HIGH.

## Source and verdict checks

Sol read the complete one-file diff, corrected comment, package finding and closure, published body, and mutation callers.
Stage 1 adds cargo-mutants --in-place. The shared options remove --jobs 1, which conflicts with that option.
Sol checked the installed cargo-mutants 27.1.0 source: the default worker count is one and in-place selects the existing workspace directory.
The option conflict is explicit in the tool's argument definition.
The change retains the mutant selection, exclusions, ordering, iterate skip file, output path, test tool, nextest profile, and failure policy.
It retains the stage-1 baseline and its timeout multiplier. It changes no timeout rule or baseline skip option.
Stage 2 already runs in place. Its slow baseline, fixed deadline, exact-name selection, and verdict logic remain unchanged.
The code still replaces earlier outcome storage before a run. An old outcome cannot supply this run's result.

The new argument proof checks both stages for --in-place and absence of --jobs or -j.
The existing exact argument proof retains the progress options, iterate option, output, and nextest arguments.
The PR changes no test assertion, seed, transcript, pin, pending ID, held ID, minimum, production code, or exclusion.

In-place testing retains .git and target where the former copy did not.
The corrected safety statement now limits baseline detection to an unmutated incompatibility.
A test can pass both baselines and differ only under a mutant. Sol confirms closure of package GS-L1-R1-1.
Sol read the current tests that read the real repository, the tree walker, temporary Git fixtures, and mutation fixtures.
The walker skips hidden directories, target, and vendor. The relevant direct readers use explicit source or configuration paths.
The inspected Git proofs use temporary repositories. The mutation fixtures copy their inputs into temporary directories.
Those paths establish no new test dependency on the gate's own .git directory.
They do not prove universal equivalence for every future test. That limit remains part of this review.

The gate uses a disposable source snapshot. An interrupted mutation can leave its file changed in that snapshot.
The job removes the snapshot afterward. This change supplies no permission to run an in-place gate in a shared development tree.
The existing prebuilt-worker gap remains. This change does not make target/candidate represent each worker mutant.
It adds no mutant acceptance or exclusion for that gap.

## Prior art

Sol searched OLD CORE 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c for cargo-mutants and in-place invocations.
The matches are unrelated lifecycle tests and sorting comments. OLD CORE has no corresponding mutation gate to port.
The PR uses the existing cargo-mutants option and stage-2 pattern. It adds no custom build cache or mutation runner.
The Prior art record matches that scope.

## Retained evidence

Comparison log: `~/botster-sessions/gates/botster-core-gs-l1-copy-vs-inplace-5a41a2c1-evidence.log`.
The evidence head is 5a41a2c1ba407d3882bb2012daef92711f09f8fd. Its selected diff base is e418bf5a8bcc7c18313e7d82775e8ae29368b366.
Sol read the command, both result summaries, script failure, and final exits.
Sol independently parsed every outcome line and compared full identities with multiplicity after removing timing suffixes.
Both runs have 119 matching lines: one passing baseline and 118 mutants.
Each run has 113 caught mutants, two missed mutants, three unviable mutants, and zero timeouts.
The two misses are Route::refusal_room's addition changed to multiplication and Worker::apply_route's || changed to &&.
Both cargo-mutants commands exit 2. This comparison does not resolve those misses or establish a green full gate on its evidence head.
The wrapper exits 0 after 427 seconds. That wrapper exit does not convert the missed mutants to passes.
Python is absent, so the script creates empty comparison files and prints a vacuous SAME-OUTCOMES result.
Sol does not use that line as proof. The independently compared retained lines supply the equality result.
The comparison covers stage 1 only. Both runs start cold, so their 205 and 216 seconds do not measure the intended cache saving.

Gate log: `~/botster-sessions/gates/botster-core-stage1-gs-gate-speed-c4d9c376-pool-20261011-013325-91259.log`.
Sol read the exact-head/base header, new argument proofs, all three mutant identities, and complete final summary.
All ten steps pass. Default tests pass 1611 cases; slow tests pass 427 cases.
Stage 1 catches all three stage1_run_args return mutants. It reports zero misses, timeouts, and unviable mutants.
Stage 2 has no missed mutant to run. Fuzz reports no changed decoder harness.
The CI summary is 239.6 seconds. The job and wrapper exit 0 after 254 seconds.
The head correction after this gate changes comments only. It has no executable delta.
The reported baseline build times, 65 seconds for #251 and nine seconds for #252, concern different diffs and mutant counts.
They show the reported build cost. They are not a controlled full-gate time comparison.
The body reports a local argument-test failure after removing --in-place. Sol has no retained reversal log and makes no independent reversal claim.

The known v1 tip is now 80d02327 after #249 and #248. The PR must import the current tip before its final gate.
A verified base-only import can carry this source CLEAN under section 8. Any failed import condition requires a delta review.
The final gate on the exact reviewed source and current base remains required after both CLEANs.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


## Round 2: current-v1 import with an overlapping path

VERDICT: CLEAN (0 open)

- Head: afc33981b70834589f93c37d25581c731efda30b.
- Base: 554d43e890b391cefc9da205c71db6856accb21d.
- Previous CLEAN head: afd827ebe4074d64d3ba6a92a39b64363bc2a651.
- Package delta CLEAN: c30732d859fdbbbd45d2131ed9f210eaf5a8c8ab.

The new head merges the current base into the previous CLEAN head.
Sol read the complete retained check: ~/botster-sessions/gates/botster-core-gs-gate-speed-base-merge-check-afd827eb-new.log.
The automatic merge and ancestry checks pass. Conditions 2 and 3 fail because both sides change xtask/src/ci.rs.
Both pending-file checks pass. This import requires source delta reviews; the failed check supplies no automatic review waiver.

Sol read the complete ci.rs import delta and own diff against the new base.
The base adds mutants_stale's import and its taint-step invocation, already reviewed in #251.
That addition runs before mutation testing and does not change mutation options, stage arguments, or verdict decisions.
The imported file preserves that check and the complete reviewed in-place change.
Sol independently compares both own diffs after removing only Git index lines and hunk headers.
Every remaining line matches in order. No added or removed source line differs from the previous own change.
The normalized diff SHA-256 is 6fd35ed5136a3f0bd4575072e02cd77c2094548863a963cba3a4322c1a65c0d8.
Sol's independent git merge-tree equals the actual imported tree, 5278781bcb33b4d4351f6fb366452210a56c9f96.
Sol read the complete package delta section. No additional source finding exists.

All preceding source, Prior art, outcome-comparison, and evidence limits remain valid.
The final full gate on this exact head against this exact base remains pending and required before merge.
An earlier gate or base-merge check does not replace it. The withdrawn pool instruction supplies no exception.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


## Round 3: final execution and merge record

VERDICT: CLEAN (0 open) at afc33981b70834589f93c37d25581c731efda30b.

Base: 554d43e890b391cefc9da205c71db6856accb21d.
Receipt: msg_plugin-w_1791709707_e5b402.
The implementation has not changed since Round 2. Package source CLEAN c30732d8 remains on this exact head.

Log: ~/botster-sessions/gates/botster-core-stage1-gs-gate-speed-afc33981-pool-20261011-020042-76967.log.
Sol read the exact head and base, both argument-proof passes, all three mutant identities, test summaries, static reports, and complete final summary.
All ten steps pass. The CI summary is 164.9 seconds.
The job exits 0 after 186 seconds, with one second queued and 185 seconds running.
The wrapper exits 0 after 187 seconds on msa1.
Default tests pass 1614 cases in 11.682 seconds. Slow tests pass 428 cases in 28.952 seconds.
The stale-exclusion check compares 207 entries with 5996 workspace mutants.
Pending-real remains 77, with zero newly passing IDs. This PR awards no minimum progress.
R-59 still excludes unfenced AD-7 from the counted real minimum.

Stage 1 catches all three stage1_run_args return mutants at line 1008: empty vector, empty string, and xyzzy.
The stage takes 14.2 seconds and reports zero misses, timeouts, and unviable mutants.
Stage 2 has no missed mutant to run. The complete mutation step takes 16.4 seconds.
Fuzz reports no changed decoder harness.
This closes the final execution condition at the exact source and base reviewed in Round 2.
The earlier comparison's two misses remain facts about its separate evidence head; this final gate does not rewrite those outcomes.
The prebuilt-worker gap and future repository-dependent test limit remain unchanged.

Sol checked v1 merge 8dab35d14f7038d3cc0598e2cde8da207bd1b723.
Its parents are the exact gate base and reviewed head.
Its tree equals the reviewed and gated tree, 5278781bcb33b4d4351f6fb366452210a56c9f96.
This merge has the required exact-head gate. It adds no case to the four historical omitted-gate record.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

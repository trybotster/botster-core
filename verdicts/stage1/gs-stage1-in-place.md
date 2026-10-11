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

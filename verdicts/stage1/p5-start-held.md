# Integration review: PR #254

VERDICT: NOT CLEAN (1 LOW finding on the READY head).

- READY head: `a1076bb1b4f08be211eefe8227311cc7095cc743`.
- Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
- Head tree: `532c0af1650f5ca0112a4b98ca24148c73b22701`.
- Package NOT CLEAN: `917d15a44a67c34328ce33fb7b689f7913a4a968`.
- Risk tier: HIGH, shared testkit.
- Source correction inspected: `4290034339365e4b925d43d9f5953326f5d5bd1a`.

## SH-SI-1 LOW — private record values duplicate the public control proof

This is the same finding as package SH-F1.
The READY head adds armed=false and kept=true assertions on the private `held_starts` map.
It also changes the other-directory assertion to inspect that map's false value.
BUILD.md Testing rule 3 states that no test tests a helper's internals.
The public `start_controls` proof already checks no hold, armed hold, stopped step, and release.

Required correction: remove the new private-value assertions and retain the existing held-spawn and cleanup proof.
Use the public control for any additional directory-state proof.

The delivered correction addresses this finding in source.
Sol read its complete one-file delta: twelve deletions and two insertions.
It removes the two new value assertions and restores the other-directory presence assertion to its preceding form.
The public control proof, held-spawn proof, and cleanup proof remain.
Replacement acceptance awaits READY and completed evidence for the corrected head.
The old green gate cannot establish the corrected head's mutation result after assertions are removed.

## Source and authority

Sol read the complete five-file diff and the surrounding hold, release, worker-end, parsing, and control-dispatch paths.
Sol also read the complete current package artifact.
No additional source finding exists.

R-59 defines a read-only `start_held {session}` control.
The control must distinguish an armed hold from a hold that has stopped its due step.
Sol read R-59 and the control row at contracts-v0.1.32.
The lead's explicit control-first instruction permits this control-only PR before the later pin work.
The PR moves no pin and changes no conformance list or transcript.

The testkit holds a map keyed by the existing directory-and-instance key.
An armed entry starts false. `keep_spawn` marks it true when the worker keeps its requested payload spawn.
The public control reads that value through the existing row decoder and directory lookup.
Duplicate hold rejection preserves the existing value.
Release removes the entry and uses the existing worker wake.
Worker end removes its entry and discards the kept spawn.
The existing public stop and run-isolation proofs remain.

The real control reads `Tap::launch_held` for the stored row's instance.
That method reports whether the tap has held the first Launch byte, as R-59 requires for the payload form.
An armed hold alone remains false.
The control adds no pump, wait, release, clock change, or report consumption.
The real proof checks false while armed, true after Launch meets the hold, and false after release.
It retains #249's independent worker evidence and no-child checks.
The plain real harness rejects the control. The tapped control list and guarded dispatch include it.
Unknown handles, unknown sessions, and extra arguments are rejected by the existing parsers and lookup paths.

Only the payload hold has a supported form.
`hold_start` and identity/running holds remain unsupported.
This source review accepts no implementation for those unsupported forms.

## Prior art and exclusions

Sol searched OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
There is no matching `hold_start`, `start_held`, or `await_control` mechanism to reuse.
This PR reuses the held spawn, hold records, tap's Launch observation, argument parser, row reader, and directory key.
The additional boolean records whether the existing held boundary was reached. It adds no separate process model.
The PR adds no dependency, decoder, timeout, production test branch, or mutation exclusion.
The complete exclusion file equals the already-reviewed base file.
No exclusion removes the changed hold or control decisions from mutation testing.

## Retained exact-head gate

Sol read the header, suite results, all mutation outcomes, summary, and exit of:
`~/botster-sessions/gates/botster-core-stage1-p5-start-held-a1076bb1-pool-20261011-021238-15211.log`.
The header names the full READY head and base.
All ten steps pass. The CI total is 695.0 seconds.
The default tier passes 1614 tests in 17.231 seconds.
The slow tier passes 428 tests in 41.254 seconds.

Stage 1 catches twelve of eighteen mutants and misses six, with no timeout or unviable mutant.
The unmutated stage-2 baseline passes all 428 tests in 42.802 seconds.
Stage 2 catches exactly the six missed full identities:

- The real control result becomes `Ok(Default::default())`.
- `has_control` becomes `true` or `false`.
- The control dispatch result becomes `Ok(Default::default())`.
- The `start_held` dispatch guard becomes `true` or `false`.

The final result is eighteen caught, zero missed, zero timeouts, and zero unviable mutants.
Fuzz reports no changed crate with a decoder harness and performs no new fuzz trial.
The wrapper exits 0 after 808 seconds. The job runs for 807 seconds after one queued second.
These results cover the READY head with its private-value assertions. They do not close SH-SI-1.

## Focused Mac source and before/after limits

Sol read the headers, comparison commands, selected results, errors, and completion records of four retained jobs:

- Linux before: `~/botster-sessions/gates/botster-core-stage1-p5-start-held-f249a622-pool-20261011-014305-30653.log`.
- Linux after: `~/botster-sessions/gates/botster-core-scratch-p5-r59-after-50403d62-pool-20261011-014434-34258.log`.
- Mac before: `~/botster-sessions/gates/botster-core-scratch-p5-r59-before-f249a622-pool-20261011-020757-5096.log`.
- Mac after: `~/botster-sessions/gates/botster-core-scratch-p5-r59-after-50403d62-pool-20261011-020945-9578.log`.

The before source is `f249a622b5435c68e200d34362e83f3d43a33005`, with contracts-v0.1.27.
The after source is `50403d62e2fa38a7b8544a726a1603482da3e229`, with scratch contracts commit `71760659db9b86765c6045ca6f10bba71618aa05`.
Sol checked that the after scratch changes only Cargo.toml and Cargo.lock from the before source.
Sol checked that the own control implementations and public proofs equal the Mac-tested source.
The correction changes only the private test assertions identified above.
Sol also checked that the sampled fenced AD-7 payload transcript equals that transcript at contracts-v0.1.32.

AD-7 payload passes once on the testkit and three times on the real tier in each job.
The after transcript requires `held: true` before its later payload checks.
Its pass therefore exercises the supported real hold observation on Linux and Mac.
Sol independently compared all 36 outcome rows before and after on each OS. Both pairs are equal.
Other sampled IDs retain unsupported controls or existing failures; the crash transcript is ignored on the testkit.
The evidence command prints these failures and still exits 0.
The wrapper exits therefore supply no nine-ID green claim.
The unfenced before pass does not meet R-59's minimum acceptance requirement.

Core's current .29 pin has no `start_held` use in its transcripts or runner.
This control-only PR establishes no newly counted minimum ID and removes no pending ID.
The later pin PR must supply its own reviewed fenced proof and exact-head gate.
Any replacement head or later base import also requires its exact-head gate before merge.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


## Round 2: correction and v1 import

VERDICT: CLEAN (0 open)

- Head: `6ca56167c66facc5eb8ac3b2c9cafd6988cd5195`.
- Tree: `1b5f378287a006670fbb9442c70369bb502d9f51`.
- Parents: `4290034339365e4b925d43d9f5953326f5d5bd1a` and `517c22350de462256f2b5be0a9fa913a98d0e42f`.
- Base: `517c22350de462256f2b5be0a9fa913a98d0e42f`.
- Replacement READY: `msg_plugin-w_1791712881_bb9244`.
- Package CLEAN: `7ed402639c52b9e4c361b00afe39c5532ff44901`, Round 2.
- Package receipt: `msg_plugin-w_1791713082_2121db`.

Sol read the complete package Round 2 artifact. No package finding remains, including LOW.
SH-SI-1 and package SH-F1 are CLOSED on this replacement head.
The corrected worker test removes both new private-value assertions and restores the other-directory presence assertion.
The held-spawn, release, worker-end, and directory-isolation proofs remain.
The public control proof still checks no hold, armed hold, stopped step, and release.
The exact-head gate catches `keep_spawn` and both `Workers::start_held` constants after the assertion removal.

Package SH-F2 LOW is CLOSED. Sol read the corrected published body on the exact head.
The body describes the retained worker proof, names the current base, and links both supplied logs.
It records the SH-F1 correction and the real.rs conflict. P5 confirms the body-only correction in `msg_plugin-w_1791713018_c6c23e`.

### Source delta and conflict

Sol read the complete five-file own delta, the correction, the combined merge diff, and the surrounding control dispatch.
The own delta has 105 insertions and eighteen deletions.
The complete added/deleted line sequence equals corrected 42900343 in every own file.
The other four own files retain the correction's bytes exactly.
The six nonoverlapping imported files retain the base's bytes exactly.
The seven imported paths equal the complete intervening base delta. No other path changes.

The retained base check fails conditions 1, 2, and 3 on real.rs. Its ancestry and both pending-file conditions pass.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-start-held-6ca56167-pool-20261011-025200-13460.log`.
The check and wrapper exit 1 after 22 seconds. This source delta review resolves that required review boundary.
The conflict retains `start_held` and the base's `fail_handoff` as separate, unchanged methods.
Both control names remain in the tapped control list, guarded dispatch, and registration proofs.
The five imported program controls have distinct names. Their dispatch cannot intercept either control.
The real control retains the stored instance lookup and `launch_held` read.
The hold proof retains the worker evidence before its child check. It retains the public false/true/false sequence.
No own dependency, decoder, timeout, exclusion, pin, transcript, or pending-file edit exists.
Both pending files, Cargo.toml, and Cargo.lock equal the current base.
The inherited DP-2 and PC removals remain. This PR removes no additional ID.
`git diff --check` reports no error.

The current base 517c2235 merges #233. Its tree equals the reviewed #233 head a7be76d5.
That identity check does not approve #233's completed final gate, whose receipt Sol has not yet read.

### Retained exact-head gate

Sol read the header, base, suite results, all 24 mutation outcome lines, summary, and completion of:
`~/botster-sessions/gates/botster-core-stage1-p5-start-held-6ca56167-pool-20261011-025228-16571.log`.
The log names the exact head and base above. All ten steps pass. The CI total is 506.6 seconds.
The default tier passes 1615 tests in 14.493 seconds.
The slow tier passes 464 tests in 35.933 seconds.
The public held-start proof passes in 0.007 seconds. The real hold proof passes in 0.063 seconds.
The stage-2 unmutated baseline passes 464 tests in 29.653 seconds; its complete step takes 41.4 seconds.

Stage 1 catches twelve of eighteen mutants and misses six, with zero timeout or unviable mutants.
Stage 2 catches exactly those six full identities, once each, with zero missed, timeout, or unviable mutants.
Sol independently compared both full identity sets. They are equal.
The six identities cover the real result, both `has_control` constants, the dispatch result, and both tapped-guard constants.
The final result is eighteen caught, zero missed, zero timeouts, and zero unviable mutants.
The complete mutation step takes 195.0 seconds.
Fuzz reports no changed crate with a decoder harness. It performs no new fuzz trial.
The job exits 0 after 514 seconds with no queue wait. The wrapper exits 0 after 515 seconds.

The real pending report retains 58 IDs and reports zero newly passing IDs.
The raw minimum counts still include unfenced AD-7. That pass does not meet R-59's hold-fence requirement.
This control-only PR claims no additional minimum pass.
The previously checked Mac source identity and before/after limits remain applicable to the unchanged control and public proof.
The correction removes private assertions only. The import retains the own implementation and public proof.
Unsupported hold forms remain unsupported. The later .32 pin PR needs its own fenced proof and exact-head gate.
Prior art and reuse remain as recorded in Round 1; this delta adds no new mechanism.

The supplied green gate precedes both completed delta CLEANs. The PR body retains a final gate on the exact head that merges.
The lead must retain that required final receipt. A later head or base change requires the applicable review and exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 6ca56167c66facc5eb8ac3b2c9cafd6988cd5195


## Round 2 addendum: publication and gate receipt

VERDICT: CLEAN (0 open)

Head remains `6ca56167c66facc5eb8ac3b2c9cafd6988cd5195`, base `517c22350de462256f2b5be0a9fa913a98d0e42f`.
Verdict 2bca30b61b0a930179163154fb3a27fe305b30d7 is now pushed. The DNS publication block is closed.
Lead message `msg_plugin-w_1791713324_746f6a` identifies the already reviewed exact-head gate as the merge gate.
Sol checked section 8, "When a head is ready for review", in plan 23am.
That rule accepts the supplied green gate when the reviewers report CLEAN on the same head.
Sol's preceding statement that this head needs a further post-CLEAN gate was incorrect. Sol withdraws that extra condition.
The gate at `025228-16571` covers the exact corrected head and base, and both reviewers are CLEAN on that head.
No new commit or base import occurred in the source. The evidence read in Round 2 is complete for this receipt.
Any later source head or base change still needs its applicable review and exact-head gate.
This correction grants no approval for an omitted gate on an imported head. The four historical gate failures remain recorded.
Sol ran no execution job.

VERDICT: CLEAN (0 open) at 6ca56167c66facc5eb8ac3b2c9cafd6988cd5195

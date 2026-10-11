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

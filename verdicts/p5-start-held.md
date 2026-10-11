# Package review: #254, start_held

## Round 1 — 2026-10-11

- Head: `a1076bb1b4f08be211eefe8227311cc7095cc743`.
- Tree: `532c0af1650f5ca0112a4b98ca24148c73b22701`.
- Parent and base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
- READY: `msg_plugin-w_1791710800_f83cc2`.
- Risk: HIGH, BUILD.md rule 3. The PR changes the shared testkit crate.

### SH-F1 LOW — OPEN on this head

`worker/tests.rs` adds direct assertions on the private `held_starts` map values.
The assertions check armed=false, kept=true, and other-directory=false.
They inspect the helper's internal record and duplicate the public control's false/true proof.
BUILD.md Testing rule 3 states: "none tests a helper's internals".

Remove the new private record-value assertions. Retain the existing held-spawn and cleanup proof.
Use the public control if an additional directory-state proof is needed.
The public `start_controls` test already checks false without a hold, false while armed, true when stopped, and false after release.
Finding messages: `msg_plugin-w_1791710870_f1cb9a` to P5 and `msg_plugin-w_1791710870_3f5e37` to Sol.

### Source and contract checks

The reviewer read all five changed paths and the complete delta: 116 insertions and nineteen deletions.
The reviewer checked held-step scheduling, release, worker end, control parsing, stored-row decoding, and the real hold proof.
No functional source finding remains beyond SH-F1's test issue.

The testkit replaces the armed-start set with an instance-keyed map.
A new entry is false. When the worker keeps the requested payload spawn, `keep_spawn` changes the entry to true.
The control reads that value without pumping, waiting, releasing the hold, or consuming a report.
Release removes the entry and wakes the existing worker. Worker end removes its entry and discards the kept spawn.
The directory remains part of the key. Another directory's identical instance keeps its separate entry.
Duplicate hold rejection retains an existing entry's value.
The existing public stop and run-isolation proofs remain.

The testkit control parses only `session`, decodes the session row, resolves its directory and instance, and reads the hold.
The real control decodes the stored row and reads `Tap::launch_held` for that instance.
This is true only when the tap has stopped the first Launch byte. An armed hold alone remains false.
`has_control` and dispatch expose the control only on the tapped real harness.
The real proof checks false while armed, true after the hold meets Launch, and false after release.
It retains the independent worker-evidence and no-child checks from #249.
The unit proof checks unknown handle, unknown session, and extra argument refusal.

The reviewer read R-59 and the control row in contracts #38 at `91394e726b1bd697307d0d5b05712b876f9189be`.
The testkit marks the due payload spawn; the real tap marks the held Launch byte. Both match the supported payload form.
`hold_start`, identity holds, and running holds remain unsupported as the body states.
The reviewer accepts the lead's explicit control-first ruling in the shared lead handoff.
This PR moves no pin. The code remains on contracts-v0.1.29, which has no `start_held` transcript or runner use.
The later pin PR must prove the fenced AD-7 result before it counts that minimum ID.
This control-only review claims no new accepted minimum pass.

The published body contains HIGH, the deciding rule, scope, evidence, and Prior art.
The old Core source at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no matching hold or await-control mechanism.
The PR reuses the held spawn, hold records, `launch_held`, argument parsers, and row readers.
No old code was copied. No own decoder, dependency, mutation exclusion, timeout, transcript, or conformance list changes.
`git diff --check` reports no error. The reviewer checked the published head, base, body, parent, and tree.

### Retained gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p5-start-held-a1076bb1-pool-20261011-021238-15211.log`.
The header names this exact head and base. All ten steps pass, total 695.0 seconds.
Default tests: 1614 passed, 484 skipped, 17.231 seconds.
Slow tests: 428 passed, 1997 skipped, 41.254 seconds.
The public control proof and argument-refusal proof pass in 0.007 seconds each.
The real hold proof passes in 0.386 seconds.
Testkit conformance passes 206. Strict pending runs 373/394, with 21 real-only and two held.
Real pending remains 77, with zero newly passing IDs. Core pending remains 464.
The ledger has 690 IDs, two deferred IDs, and eighteen withdrawn IDs.
The raw minimum report prints testkit 55/69, real 36/68, and accepted 37/69.
R-59 still excludes unfenced AD-7: the counted real minimum remains 35/68 plus one unfenced pass.

Stage 1 tests eighteen mutants: twelve caught, six missed, zero unviable or timeout, 109.7 seconds.
The slow baseline passes 428 tests in 42.802 seconds; the full baseline step takes 63.4 seconds.
Stage 2 tests exactly the six stage-1 miss identities once each: six caught, zero missed, unviable, or timeout, 144.7 seconds.
The reviewer checked every printed outcome and the full identity counts.
The slow catches cover the real result, both `has_control` constants, the dispatch result, and both tapped-guard constants.
The final result is eighteen caught, zero missed, unviable, or timeout. The complete mutation step takes 322.3 seconds.
Fuzz passes and reports no changed crate with a decoder harness.
The job exits zero after 808 seconds on Linux: one second queued and 807 seconds running. The wrapper exits zero after 808 seconds.

### Before/after evidence and limits

The reviewer read the supplied Linux and Mac evidence headers, selected results, errors, and final tallies.
Before backend: `f249a622b5435c68e200d34362e83f3d43a33005`.
After scratch head: `50403d62e2fa38a7b8544a726a1603482da3e229`, whose parent is that backend.
The scratch changes only Cargo.toml and Cargo.lock to the R-59 transcript tag. It is not a pin change in this PR.
All five own files at the reviewed head equal the before-backend bytes exactly.

Logs:

- Linux before: `~/botster-sessions/gates/botster-core-stage1-p5-start-held-f249a622-pool-20261011-014305-30653.log`.
- Linux after: `~/botster-sessions/gates/botster-core-scratch-p5-r59-after-50403d62-pool-20261011-014434-34258.log`.
- Mac before: `~/botster-sessions/gates/botster-core-scratch-p5-r59-before-f249a622-pool-20261011-020757-5096.log`.
- Mac after: `~/botster-sessions/gates/botster-core-scratch-p5-r59-after-50403d62-pool-20261011-020945-9578.log`.

AD-7 payload passes once on the testkit and three times on the real tier in each log.
The after transcript's successful `await_control` requires held=true before the later payload checks.
Both OS runs therefore exercise the new real control through the actual hold.
The before and after result sequences match for all nine sampled IDs on each OS.
Other sampled IDs retain unsupported controls or existing failures; the crash transcript is ignored on the testkit.
The evidence wrappers print results and exit zero despite those selected failures. Their exits do not mean all nine IDs pass.
This evidence proves the supported payload fence. It proves neither the unsupported hold forms nor future minimum-list changes.

Authority remains approved plan 23al at `1555a98b`, with the lead's later explicit control-first instruction.
The exact-head gate remains required on any replacement head or later base merge.
The reviewer changed no product code and ran no build, test, gate, mutation, measurement, or executable merge check.

P5 supplies source correction `4290034339365e4b925d43d9f5953326f5d5bd1a` in `msg_plugin-w_1791710903_c4eba6`.
Its parent is this Round 1 head; its tree is `869a754ecbe0f992474e43ff0cb40263c6c59b08`.
The reviewer read the complete one-file correction: two insertions and twelve deletions.
It removes both new map-value assertions and restores the other-directory presence check to its prior form.
The held-spawn proof, cleanup proof, and public control proof remain.
SH-F1 is addressed in source. Replacement acceptance awaits the published READY and completed evidence.
No Round 2 verdict exists at this record.

VERDICT: NOT CLEAN

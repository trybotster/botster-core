# Integration review: PR #260

## Round 1: slow-only test check

- Head: `f41e42e016d1dfa3a7015f2c8473b743baf04e9a`.
- Base: `d86bfcc747718452efe57b2aa207492acde96573`.
- Sole parent: `6e3ac680dbe61cd99e75ee43e0817c3ffc4c2c9a`.
- Tree: `c9be10111e5582b54367018f8ead075b3ba92ae1`.
- READY: `msg_plugin-w_1791715795_04559b`.
- Package NOT CLEAN: `b97d8c1fa7f0f2e1dcebfd679d2d6e9200daf2d2`, verdicts/gs-gate-speed.md, #260 round 1.
- Package receipt: `msg_plugin-w_1791715914_b8e1fb`.
- Risk: HIGH. The gate, worker test entry, and mutation exclusion change.

Sol read the complete ten-file delta, surrounding selection and worker code, complete package round, binding brief, and retained evidence.
The delta adds 540 lines and deletes fourteen lines. The merge base is the reviewed base.
Sol found no additional source defect. One package finding remains open.

### GS-SC-R1-1 LOW: P6 file agreement is not recorded

Status: OPEN.
The binding GS brief requires agreement with P6 on the file split before the first commit.
The PR adds `slow_coverage::command(root, &[])?` in P6's slow_job area.
The PR body and lead handoff record requests without a P6 reply.
The package reviewer asked P6 directly. Sol asked the lead in `msg_plugin-w_1791715964_e5ec17`.

Required closure: record P6's agreement or the lead's explicit file-boundary decision permitting this line without that agreement.
The existing lead direction to implement the check does not itself record that specific exception.
All findings, including LOW, must close before CLEAN.

### Cross-package source conclusions

The check reuses `test_budget::tier_selection`. The default workspace and slow package, feature, and filter arguments remain.
It compares binary IDs and full test names. Each unignored slow-only test must match the existing slow filter.
An ignored slow-only trial is accepted only in botster-core::slow_conformance, with the exact named form and closed kind set.
The six kinds match the real conformance harness. TESTKIT_PROVEN is a code constant, not a list file.
Pending-real also requires membership by the full conf:: ID. The later pending-real run remains.
Other binaries, plain ignored names, unknown kinds, malformed names, and missing required list fields fail.
The parser rejects unlisted suites and unknown filter statuses. Empty lists and no slow-only tests fail.
Existing lists checks supply the harness's per-kind accounting. This check copies no per-kind ledger decision.

The list command removes NEXTEST_PROFILE. No current profile has a default filter, so that removal does not narrow selection.
The command uses Cargo metadata's target directory and applies the existing parallelism cap.
The fixture uses its own workspace and temporary target directory. It rejects an unselected module, accepts its rename, and rejects missing pending membership.
Its declared conformance target supplies real nextest list output. The slow fixture test uses the same public check function as the gate.

The worker change renames both the test function and DRIVER_OBSERVER to slow_driver_observer.
The child command passes that constant to --exact. It still starts the same production Driver and existing group guard.
The entry returns immediately without BOTSTER_DRIVER_CONTROL. The rename changes no production Driver behavior.
No new production hook, process owner, dependency, pin, transcript, pending entry, minimum list, timeout, or mutation selector appears.

The new exclusion covers only slow_coverage::command's whole-body Ok(()) replacement.
Its cited parsing, membership, violation, check, and verdict decisions remain mutation-tested.
The existing slow_job exclusion's reason now names the new call. No mutant's slow suite is narrowed.
The prebuilt-worker limitation remains unchanged.

Sol checked Prior art against OLD CORE 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c.
Its Rust and Cargo sources contain no nextest or feature = "slow" match.
The PR uses nextest's own list fields and the current tier-selection helper. It copies no old implementation.

### Evidence and limits

Earlier rule evidence:
`~/botster-sessions/gates/botster-core-gs-slow-coverage-95c31614-evidence.log`.
Its header names earlier head 95c31614 and base 5ac94fab. It checks 1009 slow-only tests and exits 1 with 544 violations.
Sol independently counts 394 pending, 70 no-transcript, 58 pending-real, eighteen withdrawn, two deferred, and one testkit-proven trial.
The remaining violation names driver_observer. This evidence does not cover the final corrected source.

Exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-gs-slow-coverage-f41e42e0-pool-20261011-033934-73592.log`.
The header identifies the reviewed head and base. All ten steps pass in 576.0 seconds.
The default tier passes 1632 tests in 13.965 seconds. The slow tier passes 477 tests in 35.076 seconds.
The check reports 1012 slow-only tests with no violation. The observer and real-tool fixture pass.
Stage 1 tests forty unique mutants: thirty caught, eight missed, two unviable, and zero timeout, in 96.3 seconds.
The unmutated stage-2 baseline passes 477 tests in 34.072 seconds; its complete step takes 48.4 seconds.
Stage 2 tests exactly the eight full missed identities, once each, and catches all eight in 176.0 seconds.
Sol independently compares the stage identity multisets. They are equal.
Sol also matches all thirty stage-1 and eight stage-2 catching-test reports to their caught identities.
The complete mutation step takes 323.0 seconds: 38 caught, two unviable, zero missed, and zero timeout.
Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 588 seconds; the job queues one second and runs 587 seconds.

This green evidence does not close the required coordination finding.
Current v1 has advanced after this reviewed base. A later import needs the applicable review or base check and its own full exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at f41e42e016d1dfa3a7015f2c8473b743baf04e9a

## Round 2: file agreement closure on the same source

- Head and base remain `f41e42e016d1dfa3a7015f2c8473b743baf04e9a` and `d86bfcc747718452efe57b2aa207492acde96573`.
- Lead closure: `msg_plugin-w_1791715978_c6cb39`.
- Package CLEAN: `45eef34451182a1f9beda2e1188dd8cb43fe4d0d`.
- Package receipt: `msg_plugin-w_1791716012_62c14a`.

GS-SC-R1-1 LOW is CLOSED. The lead explicitly permits the one slow-coverage call in slow_job without P6's reply.
The lead records two GS requests and a direct lead ping without a reply. P6 can raise an objection through a follow-up PR.
This permission closes the required coordination record. It changes no source, gate evidence, or file ownership beyond this one call.
Sol read the complete package closure. No finding remains open.

This CLEAN covers only the reviewed source and old base. The existing green gate covers that exact head and base.
Current v1 is 4ab8a985 after #259. A new import needs its applicable review or base check and its own full exact-head gate.
Sol ran no execution job.

VERDICT: CLEAN (0 open) at f41e42e016d1dfa3a7015f2c8473b743baf04e9a

## Base import: source CLEAN carries to f291a5e6

- Head: `f291a5e6d3f31e3ad769f52e125647a292076d52`.
- Base: `cfd6ee364e2eafd329ce4e665100168918d383cb`.
- Parents: reviewed head f41e42e016d1dfa3a7015f2c8473b743baf04e9a and the new base.
- Tree: `2388ba831a7d807aa32cae2488c8547a802d9b01`.
- Receipt: `msg_plugin-w_1791716302_464c7c`.

Sol read the complete shared check log:
`~/botster-sessions/gates/botster-core-gs-slow-coverage-base-merge-check-f41e42e0-f291a5e6.log`.
Ancestry and all four conditions pass. The new head descends from the reviewed head.
The new merge base descends from the reviewed merge base d86bfcc7.
All five changed base paths are outside the ten-file own diff. The two pending files retain the base's exact contents.
The check reports an unchanged own diff of 30764 bytes, SHA-256:
`c0d79f9e593f81d39dcf809da9bbe392aa97ae766a747a488a4677ff82bb02c3`.

Sol independently runs git merge-tree --write-tree. Its tree equals the submitted tree, without a conflict.
Sol also compares both raw git diff --binary outputs. They are byte-identical, 30124 bytes each, SHA-256:
`22c330e994387f35fcbede87526f3e468c6029c0ac45ed9d7d0adabeba726dd6`.
The script's normalized diff and Sol's raw diff are different representations; each comparison is independently equal.
Sol independently confirms the disjoint path sets and exact pending-file contents.

The plan's mechanical carry-over rule applies. No reviewer delta round is required.
The package and integration source CLEANs on f41e42e0 carry to this import through the passing shared check.
The earlier f41e42e0 gate does not cover the new head. The implementer reports the final gate is running.
This verdict accepts no unfinished gate or merge. Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open, source carry-over; final gate pending) at f291a5e6d3f31e3ad769f52e125647a292076d52

## Final gate and merge: f291a5e6

- Head: `f291a5e6d3f31e3ad769f52e125647a292076d52`.
- Base: `cfd6ee364e2eafd329ce4e665100168918d383cb`.
- Package source carry: `f6dbd460e4986b1c42c70e189a57fdc4f4d3ed6d`.
- Package completion: `995350d85f973b6e6fa070dd8fc3d23e68274eef`, read from the local object.
- Package receipts: `msg_plugin-w_1791716864_02b297` and correction `msg_plugin-w_1791716891_19938f`.

Sol checked the exact-head log:
`~/botster-sessions/gates/botster-core-stage1-gs-slow-coverage-f291a5e6-pool-20261011-035822-73822.log`.
Its header names this head and base. All ten steps pass in 444.1 seconds.
The default tier passes 1632 tests in 11.335 seconds. The slow tier passes 477 tests in 28.619 seconds.
The check reports 1012 slow-only tests with zero violations. The observer and real-tool fixture pass.
All 58 real-pending IDs remain pending. The gate reports no new real-pending pass.

Stage 1 tests forty unique mutants in 76.8 seconds: thirty caught, eight missed, two unviable, and zero timeout.
The unmutated stage-2 baseline passes 477 tests in 30.011 seconds. Its complete step takes 41.1 seconds.
Stage 2 catches all eight misses in 173.7 seconds, with zero missed, unviable, or timeout.
Sol independently compares all eight full missed identities with all eight stage-2 identities. The multisets are equal.
Sol matches all thirty stage-1 and eight stage-2 catching-test reports to their caught identities.
The complete mutation step takes 293.7 seconds: 38 caught, two unviable, zero missed, and zero timeout.

The final ids replacement mutant records an EV-5 failure followed by a pass on its one retry.
The gate labels that result FLAKY. EV-5 is not its catching test.
The retained catching test is `xtask::bin/xtask slow_coverage::slow_tests::a_slow_only_test_outside_the_filter_or_an_ignored_one_not_pending_fails`.
This gate does not remove the required EV-5 structural fix.
Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 467 seconds. The job queues one second and runs 466 seconds.

The package reviewer confirms zero open findings and the same green gate.
The package reviewer corrects its first publication claim: completion 995350d8 is local; its push failed on DNS.
The previously published package source carry remains f6dbd460. Sol accepts no false publication claim.

Sol fetched v1 and independently checked merge `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
Its parents are the exact base and head above. Its tree is `2388ba831a7d807aa32cae2488c8547a802d9b01`.
That tree equals the reviewed import tree and the gated head tree.
The supplied green gate meets section 8's exact-head condition. No repeated gate is required after CLEAN.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open; exact-head gate accepted and merge verified) at f291a5e6d3f31e3ad769f52e125647a292076d52

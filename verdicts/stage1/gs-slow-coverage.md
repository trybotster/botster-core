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

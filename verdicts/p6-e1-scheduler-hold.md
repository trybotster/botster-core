# P6 review: E1 scheduler holds — PR #300

## Round 1 — 2026-10-11

Exact head: `d5c3c038d0f9a01fb915ea7b16b2f34900569685`.
PR source base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
Gate base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Tree: `ba80b57d294250018659db3f1c0fe5f04d52b20d`.
Risk: HIGH. The PR changes a production trait, the pump, the shared testkit, and mutation exclusions.

I read the amended design before code review. I read the complete 16-file source delta and relevant driver, engine, and scheduler context.
I read the complete PR body after GitHub MCP truncates it.
Raw body SHA256: `cd60af2200b769b4f708c8e849458a4b70f0a6426fed6d716d14a990a0169f23`.

The provided Scheduler methods preserve the production default: held returns false; admitted discards its arguments.
The admission report occurs once after a successful engine admission. A refused admission returns before the report.
The testkit schedulers record admitted operations and fixed timing without drawing choices.
The real scheduler tap also forwards the admission to its inner scheduler. No Timed CoreApi layer remains in the effective delta.
The controls share one hold implementation, use typed fixed-timing refusal, and signal the host wake after release.
The driver filters held work before choice points. Its more and wake decisions use the remaining runnable work.
Deadline, Silent, and never_deferred work bypass the hold. The existing OperationDeferral pick remains in place.
The public stop/deadline tests, registry tests, and real release/wake test exercise useful behavior.
The new real test uses the existing harness ownership and bounded event-wait helper. It adds no sleep or production-child reap.
No product behavior defect was found in this source review. Two required test findings remain below.

### E1-P1 — MEDIUM — OPEN: the new tests assert implementation state

`crates/botster-core-host/src/tests/driver/holds.rs:150` obtains the engine and asserts three exact ops_of vectors.
The no-flow test makes no public progress or scheduler assertion after it installs the hold.
It therefore tests the internal work mapping instead of the no-flow behavior required by lead condition 1.
The retained mutation record cites this test for three ops_of catches. Those catches do not replace the required public behavior proof.
Replace these assertions with a public scheduler observation and/or public Core progress assertion for the no-flow case.
Keep the condition's default-draw behavior observable.

`crates/botster-core-edges/src/scheduler.rs:200` also compares the whole Production value after admitted.
Its fields hold scheduler state. Assert public choice behavior after admission instead of equality of that stored state.
The recorded policy's choice-call log is an observation of the public scheduler calls and can remain.

### E1-P2 — MEDIUM — OPEN: lead condition 3 has no seeded before/after proof

`crates/botster-core-host/src/tests/driver/holds.rs:159` compares two runs of the new implementation.
Holding delegates choices to Production, not the seeded scheduler. The log records only ChoicePoint and candidate count.
It does not record returned picks. One run has no hold; the other holds an operation that never exists.
The comparison cannot establish the required unchanged seeded sequence before and after this PR.
The focused conformance passes at seeds 0-31 establish transcript results. They do not compare scheduler choices with the earlier behavior.
Supply a comparison of the same seeded public picks, counts, and order against the pre-change behavior.
The test must identify its before/after reference. Update the PR body's condition-3 claim to match that proof.
Integration receipt `msg_plugin-w_1791773954_053299` independently confirms this gap.

### Retained evidence

Focused Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194155-49849.log`.
SHA256: `51a5d271e526823ef22b9438c926103df9d4a51eb3422a44eb7a8e994c085b31`.
I read the decoded command, selected test results, API diff, strict seed commands, and exits.
Clippy exits zero. Edges passes six tests; host passes 299, including all five new holds tests.
The five op_holds tests pass. Both selected real controls tests pass.
The API comparison exits zero for each API extraction. Its diff only adds the two provided methods and their re-exports/defaults.
The API diff exits one because those additions differ. The script restores the edges source before later tests.
The selected atomic-transitions transcript passes on each tier under BOTSTER_SEEDS=0-31. Both commands exit zero.
The job exits zero after 40 seconds. The wrapper exits zero after 41 seconds.

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-d5c3c038-pool-20261011-194248-53397.log`.
SHA256: `22d124d3b70a6e565c276afddcda89938f0b64409fa2bfec6ae0c30cc486f66d`.
I read the exact-head/base header, lists records, tier/stage summaries, raw mutation outcomes, named catches, and exits.
All ten stages pass in 796.9 seconds. Default passes 1855 tests, with 434 skipped, in 22.358 seconds.
Slow passes 608 tests, with 2044 skipped, in 62.267 seconds.
Stage 1 has 64 unique mutants: 42 caught, 13 missed, nine unviable, and no timeout, in 205.9 seconds.
Stage 2 tests exactly those 13 misses and catches all 13, in 315.7 seconds. Its unmutated slow baseline passes.
All 55 raw catches match their named catches. All named catchers also pass normally.
The aggregate has 55 caught, zero missed or timed out, and nine unviable. Unviable outcomes supply no catch evidence.
The complete log has no FLAKY record or failed nextest result.
The job exits zero after 806 seconds: one queued and 805 running. The wrapper exits zero after 807 seconds.

The two new exclusions match equivalent provided-default bodies. Their exact patterns do not exclude the overrides.
The moved LINK_CAPACITY exclusion retains its existing reason and follows the changed line. No broader exclusion is added.
The source removes only conf::a5_2_reads_match_atomic_transitions from core-pending.txt.
I read that transcript at contracts-v0.1.41, commit `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`.
It asserts public get and event behavior around hold/release. The selected 32-seed commands run that exact identifier on both tiers.
The deferred-completion identifier remains pending. This review accepts no future R-92 transcript or pin change.

I read all three named Prior-art functions at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
They report buffered output or held session termination, rather than scheduler control of one operation's progress.
The note rejects each for that reason and uses the approved scheduler policy seam.

I sent both findings directly to P6 and integration. No ordinary NOT CLEAN finding goes to the lead.
The final-slot import, applicable base review, exact-head final gate, and final body remain due after these findings close.
I changed no product code and ran no execution job. This verdict accepts no later head, pin, or status removal.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11: seeded proof added; no-flow coverage remains open

Exact head: `3aeb49672f2546b23f07f1e301026c784fdf3381`.
Source base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
Gate base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Tree: `c8186f3ae5a4063755967b8de55a3f0d9e85276f`.
I read the complete five-file delta from Round 1 and the complete live body delta.
The delta changes tests and corrects one documentation reference from Scheduler::defer to Scheduler::held.
It changes no production behavior, control, pending file, exclusion, or contracts pin.
Body SHA256: `cdb5b15d98ac930908f8c6d22d0895ee525dd3c2bc5eeece2e4e57816197caec`.

E1-P2 is CLOSED. The new seeded_trace policy records each public choice point, count, and returned answer.
It uses a seeded xorshift64* stream and implements only pick and bound. The same file compiles before E1.
Two sessions start, run, and stop. The test compares the trace length and FNV-1a digest for seeds 0-15 with BEFORE_E1.

Trace log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-1d1d7fc9-pool-20261011-200116-36766.log`.
SHA256: `7fa466a5611a21151f847eb27e78604852435a89e5ef08074610e9b12e5a4061`.
I read the complete retained records and decoded command.
The command prints the trace on `1d1d7fc9499895394c9e98b9c3e209d6cabffefc` twice, then on the pre-E1 host and edges crates at the source base.
The first two head outputs compare equal. All 16 head and base trace values match independently.
The base/head output diff exits one because the filtered-test count differs: 295 versus 300. The trace values do not differ.
The wrapper's exit zero alone is not proof of their equality. The recorded values establish the comparison.
The command restores host and edges source; its final status reports no change.
The job exits zero after 504 seconds: 490 queued and 14 running. The wrapper exits zero after 505 seconds.
The current trace file differs from that evidence file only in the stored reference values and their documentation.
Its policy, workload, recorder, and digest remain identical. The current gate passes the comparison test.
This proof covers the specified two-session workload and 16 seeds. It does not claim every possible workload or seed.

### E1-P1 — MEDIUM — OPEN: the no-flow proof exercises the reported-exit bypass

The repaired tests remove the internal ops_of assertions and the whole Production equality assertion.
The default-method test now compares public Session picks. Those representation concerns are resolved.
The no-flow test holds the completed Create and Start, feeds WorkerMsg::Exited, and observes Exited through get.
That observation is public, but it does not cover lead condition 1's no-flow scheduler step.
WorkerMsg::Exited calls begin_end_flow. For an Idle session, that function creates Flow::Stop with end set.
never_deferred then bypasses the hold policy for this reported exit. The test proves that separate exit rule.

A no-flow session can have runnable metadata work. A successful UpdateMetadata sets metadata_pending at inbound.rs:95.
ready() adds Work::Session for that flag before the flow_waiting check at run.rs:187.
run_flow posts public MetadataChanged at flows.rs:96. That step can exercise the empty-operation hold query and the default draw.
Use this public path and scheduler-call/progress observations to cover condition 1. Do not restore internal mapping assertions.
Update the body's condition-1 claim to match the proof. Integration independently confirms this gap and reachable metadata path.
I sent this remaining finding directly to P6 in `msg_plugin-w_1791776227_62fa30`.

### Gate and public forwarding proof

The intermediate gate on `0b623438445d6f7beb6621b31ba627b3f23d8a9e` misses deletion of the Work::Op arm in both stages.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-0b623438-pool-20261011-201016-93160.log`.
SHA256: `122cbf551713bc9f53ceb0e591c67a48ce494f91f1f5af0838ddcedbc411089b`.
I read the relevant raw miss, stage summaries, and failed exits. This gate supplies no acceptance result.
The new forwarded-op test holds a Resize, observes no outgoing worker operation during the hold, then observes one after release.
It preserves a public protocol assertion and catches that arm deletion in the current gate's first stage.

Current log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-3aeb4967-pool-20261011-202357-89922.log`.
SHA256: `7585100fe53d190c1666a78217245514a190ea660c3db3d02585abbc795dc709`.
I read head/base, selected changed-test results, lists, stage/tier summaries, mutation outcome accounting, and exits.
All ten stages pass in 553.5 seconds. Default passes 1857 tests, with 435 skipped, in 17.165 seconds.
Slow passes 608 tests, with 2044 skipped, in 44.286 seconds.
Stage 1 has 42 caught, 13 missed, nine unviable, and no timeout, in 145.7 seconds.
Stage 2 tests exactly the 13 misses and catches all, in 203.8 seconds. Its unmutated baseline passes.
All 64 original mutation identities and raw outcomes match Round 1. The changed ops_of catchers use the public forwarded-op test.
All 55 raw catches match their named catches, and all named catchers pass normally. No FLAKY record occurs.
The job exits zero after 565 seconds: one queued and 564 running. The wrapper exits zero after 565 seconds.
The original focused API and 32-seed evidence retain their recorded original-head scope. Production behavior is unchanged by this repair.

No additional product behavior defect is found. E1-P1 remains the only package finding.
The final import must preserve #291's drain term at the settle site only. Final base review, gate, and body remain due.
I changed no product code and ran no execution job. This verdict accepts no later head, pin, or status removal.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-11: public metadata proof closes condition 1

Exact head: `bd74a168fc1ef7e4774f635503f3707cc0dd2ec3`.
Source base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
Gate base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
Tree: `2f7cc7639866c488772cc1ee6d3f5f8b05d9814b`.
I read the complete one-file delta from Round 2 and the complete live body delta.
Only holds.rs changes. The delta adds a public scheduler-call recorder and replaces the exit-based condition-1 proof.
The remaining tests retain their behavior assertions; their setup now uses the Probes structure.
No production behavior, control, status file, exclusion, or pin changes in this repair.

E1-P1 is CLOSED. The session's Create and Start complete before the test holds their operation IDs.
The test admits UpdateMetadata, runs the driver, and observes the public MetadataChanged event for the session.
The public Scheduler::held recorder observes both the update's operation ID and an empty operation list.
The former belongs to the update's own step. The latter exercises the session's metadata step with no flow.
The test uses the actual metadata_pending path identified in Round 2, rather than the reported-exit bypass.
It observes the scheduler interface and public progress without reading engine.ops_of or production state.
The new documentation states why the payload exit cannot supply this proof.
E1-P2 remains CLOSED. The seeded comparison and its workload are unchanged; the current gate passes that test.
Integration receipt `msg_plugin-w_1791776869_10ac6f` independently confirms the repaired condition-1 proof.

Gate log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-bd74a168-pool-20261011-203726-79012.log`.
SHA256: `29f167fe7e319e92b667a75899a9ea2bc15fb31467939b7cef44d0cc03e3ce29`.
I read the exact-head/base header, lists records, changed-test outcomes, stage/tier summaries, mutation accounting, and final exits.
All ten stages pass in 544.1 seconds. Default passes 1857 tests, with 435 skipped, in 14.680 seconds.
Slow passes 608 tests, with 2044 skipped, in 45.579 seconds.
The repaired metadata test, public forwarded-op test, and permanent seeded comparison pass.
Stage 1 has 42 caught, 13 missed, nine unviable, and no timeout, in 154.1 seconds.
Stage 2 tests exactly those 13 misses and catches all, in 187.6 seconds. Its unmutated baseline passes.
All 64 raw mutation identities and outcomes remain identical to Round 2.
All 55 raw catches match their named catches. Every named catcher passes normally.
No FLAKY record or failed nextest result occurs. Unviable outcomes supply no catch evidence.
The job exits zero after 554 seconds: one queued and 553 running. The wrapper exits zero after 555 seconds.

The live body's condition-1 description now matches the metadata proof and explains the exit bypass.
Its current-head gate records match the retained log. Earlier API, trace, and 32-seed evidence keep their recorded original-head scope.
Raw body SHA256: `d3a23ac4d5be3751c8dec3b5911e5a6a9a4c271c38609d1c515976a3f50c23b8`.
No package source or evidence finding remains for this head. The approved source and initial evidence are CLEAN.
The gate-base environment does not establish a source import. The final import and applicable base review remain due.
That import must preserve #291's drain term at the settle site only. The final exact-head gate and final body remain due.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 4 — 2026-10-11: manual review of the train-2 import delta

Exact head: `484912ac58743b899c7856e87bdcad0f2b398300`.
Imported base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Tree: `cebfddcff16568bd2b487eada87b077a9501d321`.
Merge parents: the Round 3 CLEAN head `bd74a168fc1ef7e4774f635503f3707cc0dd2ec3` and the exact imported base above.
P6 requests this delta round in `msg_plugin-w_1791781322_6f35d2` under the lead's train-2 condition.
This round reviews the source import. It accepts no running evidence job or completed train gate.

I inspect the merge object and the complete combined driver conflict diff.
The first more site keeps the imported comment and calls `self.more(&budget)`.
The settle site calls `self.more(&budget) || self.draining()`.
The helper still computes `self.runnable() || budget.more_input`. The runnable decision still excludes held work.
The drain term therefore appears only at the settle site, as #291 requires.
Both more sites retain the held-aware decision. The driver has no remaining engine.runnable call.
The imported draining implementation observes ended links whose frames do not wait for queue room. E1 does not change that rule.
The hold filter, admission report, fixed-timing bypass, and choice calls retain their reviewed implementation.

The imported base changes 69 paths from the original source base. Eight overlap the PR's 17 owned paths.
I read the merged own diff against the imported base for all eight overlapping paths.
The engine, driver-test module, harness, testkit lib, and real-harness own additions/deletions exactly match the reviewed own changes.
The conflict resolution is the stated driver composition above. It introduces no additional E1 behavior change.
The nine non-overlapping owned entries exactly match the Round 3 entries.
Every non-owned entry exactly matches the imported base. No extra source change appears outside the owned paths.

I independently compare the three-way sets for mutation exclusions and the three status files.
Each merged set equals the imported-base set plus the PR's own additions, minus its own removals.
There is no unexpected added or removed identifier or exclusion.
The PR still removes only conf::a5_2_reads_match_atomic_transitions. The deferred-completion identifier and its own reason remain pending.
The held and real-pending sets equal the imported base. Their imported changes supply no permission for another own removal.
The exclusion delta keeps the two reviewed default-body patterns and moves the existing LINK_CAPACITY pattern to line 34.
The corresponding source sites remain at core.rs:34:33, scheduler.rs:60:9, and scheduler.rs:71:9.
No broader own exclusion appears. The prior reasons and override coverage remain applicable.

No package source finding remains in this import delta. Both earlier findings remain closed with their recorded proof scope.
This is a manual delta CLEAN, not a claim of automatic carryover from a canonical PASS.
The earlier gate, API, seed, and trace records retain their original-head scope.
The Mac compile/test evidence is still running. I read no unfinished output and accept no result from that job in this round.
The completed compile/test evidence, applicable final base record, train gate, and final body remain due.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 5 — 2026-10-11: completed Mac evidence for the import

Exact head: `484912ac58743b899c7856e87bdcad0f2b398300`.
Imported and evidence base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Tree: `cebfddcff16568bd2b487eada87b077a9501d321`.
The head and tree are unchanged. Round 4's manual source-delta review remains valid.

Mac log: `~/botster-sessions/gates/botster-core-stage1-p6-e1-scheduler-hold-484912ac-pool-20261011-220151-15434.log`.
SHA256: `b17b00ed49f1fd15f4cc4d4fdc902c79fdd40deccd5d38d1ab9f9d44856a8dd1`.
I read the retained log and decoded command. The actual rev-parse output names this exact head.
The pool host reports kernel 27.0.0. The command runs clippy for edges, host, Core, testkit, and xtask, with all targets and warnings denied.
Clippy exits zero. The command then runs the edges and host library tests.
Edges passes seven tests. Host passes 339 tests, with zero failed and one ignored, in 0.39 seconds.
The ignored test only prints trace evidence. The permanent seeded comparison passes.
The public metadata, forwarded-op, stop-hold, deadline, and admission proofs pass.
The imported driver-drain and stop-drain proofs also pass, including the settle and queue-room cases.
The cargo test pipeline records exit zero.

The worker prebuild completes and writes its three-binary candidate manifest.
The five selected testkit op_holds tests pass, with zero failed and 509 filtered tests, in 0.02 seconds.
Their cargo pipeline records exit zero. The job and wrapper exit zero after 713 seconds: six queued and 707 running.
This focused job supplies the pending Mac compile/test evidence. It is not the full train gate.
Integration receipt `msg_plugin-w_1791782088_a09240` independently confirms these records.

The imported base pins contracts-v0.1.43 at `c69a3190b4d66e84401006e9d73788ff0e837f56`.
I compare the two E1 transcripts with the original .41 pin. Both are byte-identical across the pins.
The atomic-transitions transcript has SHA256 `a115f37b7727f33f3c132d6bccfca659243bf856e66871520af0182c75255703`.
The deferred-completion transcript has SHA256 `689bb2f0e6ddfb37428d938f2508c568ad07368aab4e9e7bf39954f4f6a22775`.
This comparison preserves their recorded assertion scope. It is not a new conformance execution or permission for another removal.

No package source or focused-evidence finding remains at this exact head.
The applicable final train record, exact train gate, and final body remain due. Earlier evidence keeps its recorded scope.
This verdict accepts no later head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 6 — 2026-10-11: #300 source delta on train 2

Exact train head: `080987829a2e2245a7ea97606020400392c974d6`.
Train tree: `0dcf95bedeb3d86b27fafa322614b3b07b66b76b`.
Train base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Reviewed #300 input: `484912ac58743b899c7856e87bdcad0f2b398300`.
Lead request: `msg_plugin-w_1791783838_e37537`. This round reviews #300's delta on the complete train head before its gate.

I inspect both train merge objects and their first-parent chain.
The #300 merge is `872c7eeb66ca0cbe174b619014fc64100953a6f9`, with parents the exact train base and #300 input above.
The final train head has parents that merge and #305 input `655bb3ab16d34f1bc14991aed6c1a41fbb785dc3`.
The train base contains the #301 and #304 merges. This review does not treat the final head as a single #300 merge.

The base changes 12 paths from #300's imported base ae49bbec. Four overlap #300's 17 owned paths.
Those paths are core-pending.txt and the three testkit files core.rs, core/tests.rs, and harness.rs.
I read the complete changes in those three files against the reviewed #300 input and the effective #300 patch against the train base.
The #301 start holds retain their Faults storage, spawn/accept behavior, and public link tests.
The #300 HandleScheduler retains its own SharedHolds object and its held/admitted methods.
Directories::open creates that object once and shares it between the scheduler, Opened, and the handle's controls.
The tests' constructor retains the second HandleScheduler argument. The harness retains holds_of and stores the same object for the handle.
The imported harness test changes its unsupported-control probe from hold_start to host_crash. It does not remove an E1 assertion.
The start-hold storage and operation-hold storage remain separate. The auto-merges add no new scheduler draw or E1 policy change.

I independently compare all 15 owned source/test patches, excluding the two set files.
Their added and deleted lines exactly match the reviewed #300 patches. Hunk locations and blob metadata can differ.
Every non-owned entry at the #300 merge equals the train base. The subsequent #305 merge changes no owned source/test entry.
Its only changed owned file is mutants.toml, where it changes an imported worker-signal comment, not an exclusion entry.
The final driver retains both held-aware more calls and the drain term at the settle site only.

I independently compare the three-way sets on the final train head.
Mutation exclusions, core-pending, core-held, and core-real-pending exactly retain the base additions and the reviewed #300 changes.
There is no unexpected identifier or exclusion. Core-pending has 406 identifiers.
The #301 attach-state removal and #304 stream-handoff removal remain present as removals from the train base.
#300 still removes only a5_2_reads_match_atomic_transitions. Deferred-completion remains pending with its reason.
The two equivalent default-body exclusions and the moved capacity exclusion retain their reviewed scope.

No #300 package source finding remains on this exact train head. The earlier findings stay closed with their recorded scope.
This is manual source-delta review. It claims no canonical PASS, train compilation result, or completed train gate.
The earlier source and focused-evidence records retain their original-head scope.
The completed exact-head train gate and final evidence body remain due. The lead controls the one train gate.
This verdict accepts no later train head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

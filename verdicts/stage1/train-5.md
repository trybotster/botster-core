# Train 5 combined integration review

## Round 1: combined source before the gate

Exact head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Lead assignment: `msg_plugin-w_1791828964_e1c322`.

### Delivery contract and owners

The outcome is reviewed build-root coverage composed with report holds on the current train base.
The frozen members are #306 `0a09168702048cf34ef3fa30dffc07131b620f0a` and #315 `29dc0efe0f817f729f62582d09a28a239c49272b`, in that order.
Sol-B owns both integration deltas and this combined review. 017c owns #306's package review; 016a owns #315's package review.
The lead owns train construction, the exact-head gate, and merge/tree/push verification.
This review reuses the binding Stage 1 acceptance plan. It creates no separate plan and delegates no work.

The source invariants are preserved member patches, shared testkit state boundaries, selected tests, and unchanged conformance accounting.
The real entry paths remain the host driver and real testkit adapter under the .44 contracts pin.
Decisive final evidence must come from one completed gate on this exact train head and the final member receipts.
This source round cannot establish runtime selection, completed mutation outcomes, fuzz results, or a completed merge.

### Merge and complete patch checks

Sol-B reads both merge objects and verifies their exact parents.
Merge `1034c71e1e3b42a916455e5fc5ee8ee67388f7ff` joins the train base and frozen #306.
The final merge joins that first merge and frozen #315.
The respective merge bases are 838e4bb0 for #306 and 08098782 for #315, with full identities in their member records.
The combined delta has 61 paths, 2,463 added lines, and 146 deleted lines. Git diff --check passes.
Full-index diff SHA256: `dd2f7791751f96c3107389fb021fd158a3b1922f1f35a5ec2c4f89c01a7661ce`.
Every added/deleted line of all 44 #306 paths matches its accepted patch at the first merge.
Every added/deleted line of all 18 #315 paths matches its accepted patch at the final merge.
The merges add no extra own path. Their only shared changed path is testkit src/lib.rs.
The independent blob checks identify two composed #306 files and five composed #315 files; all other own blobs remain identical.
Sol-B reads every composed delta and relevant final interface context.

The testkit library retains queue_controls, report_holds, BUILT_IN, and the shared root macro.
The route-log binary retains the accepted .44 key-based read adaptation and its root macro.
The final host driver equals the accepted train base. It retains the control-room service before each link read.
That path neither replaces the receive edge nor changes the once-per-pump PumpBound observation.
The report controls set no control_queue override and refuse no cancel in their proof workloads.

The simulated receive edge records hello and consumed-report bytes at the same driver boundary.
Report holds keep later bytes outside that record until the driver receives them.
Queue controls and report controls retain separate per-host state and resolve session rows before locking that state.
The reports-to-faults, reports-to-holder, and tap-to-holder orders introduce no reverse lock path.
The real adapter retains both tapped control families and the inner production process ownership.
It defers already-reaped exits; it adds no reap call or production child owner.
The quiet and deadline repairs from #315 remain intact. Root macros change no control behavior or public assertion.
The worker report-order implementation and proof remain intact.

### Coverage of the tests added by #307 and #315

Sol-B inventories the new test functions from each member's source delta and checks every name in the composed source.
The inventory has 46 new tests: 18 from #307 and 28 from #315. All remain present.
The table maps every inventory group to its existing test binary and selected tier.

| Member and module | New tests | Test binary | Tier |
| --- | ---: | --- | --- |
| #307 driver, control_term, deadlines | 17 | botster-core-host library | Default |
| #307 queue_controls::tests | 1 | botster-core-testkit library | Default |
| #315 driver::holds | 1 | botster-core-host library | Default |
| #315 edge_tap::tests | 1 | botster-core-testkit library | Default |
| #315 report_holds::tests | 17 | botster-core-testkit library | Default |
| #315 report_holds::harness_tests | 5 | botster-core-testkit library | Default |
| #315 worker::tests::routes::report_order | 1 | botster-worker-core library | Default |
| #315 real::slow_controls | 3 | botster-core-testkit library | Slow |

Forty-three tests remain ordinary default library tests under reachable test modules.
The three real tests require the testkit slow feature and match the existing slow_controls name filter.
The host, testkit, and worker-core library binaries retain the approved root macro under cfg(test).
The new modules create no separate binary that needs another root entry.
The eleven required-features repairs apply to existing slow integration targets and preserve their names and root entries.
The .44 route-log adaptation creates no new target and removes no root check.

Sol-B reads the final binary inventory, presence checks, tier selection, and CI call sites.
The presence checker retains every listed suite, including an empty suite, and checks each binary's exact selected root test.
The slow-only checker rejects an unselected or improperly ignored slow-only test.
The slow filter retains the exact root-name term and the slow_ module term.
The taint job calls the name checker. The slow job calls coverage after the tier completes.
These source checks establish coverage structure. The fresh gate must still list and run the tests and pass both coverage decisions.
The accepted manual name rule and mixed stale-library limit remain; this review claims no stronger guarantee.

### Accounting, pin, and mutation configuration

Sol-B independently checks all nine conformance text sets after each merge. All 18 checks retain the base sets exactly.
Final pending is 403; real-pending is 34; held is four; ledger is 691; real-only is 26; proof rows are seven; minimum is 69.
Both deferred TOML entries remain unchanged. No pending result supplies an additional promotion.
The train retains the base's .44 pin and exact EV-2 known-flaky entry and parser.
No retired IN-8 entry returns. The ordinary slow tier and unmutated mutation baseline still run EV-2.
EV-2 can supply no credited mutation catch under the inherited exact-name exclusion.
Sol-B checks both accepted member gates: neither credits EV-2 as a catch.
Those gates retain their original heads. The fresh train gate must supply current mutation outcomes.

The sole exclusion change moves LINK_CAPACITY's exact arithmetic coordinate from line 34 to line 35.
The final constant remains `const LINK_CAPACITY: usize = 64 * 1024;`.
No checker function receives an exclusion. No CI-directory file or process allowance changes from the train base.
#306's accepted xtask CI and slow-filter edits remain its own source changes; they are not new base changes.
The train accepts no later .45/.46 pin, EV-2 entry removal, A31 implementation, or status removal.

### Exact-head receipts and disposition

Sol-B reads the complete applicable package records:

- #306: `a9309872c130cbdeb453483a82f3dd037747917d`, train 5 section in verdicts/gs-gate-speed.md.
- #315: `f79ddfa7be40715be30942e326eb175693b028f5`, Round 3 in verdicts/p6-e2-report-holds.md.

Both package records name this exact head, tree, and base and report zero findings.
The two matching integration source records are in this commit: #306 Round 7 and #315 Round 3.
All prior member source findings remain closed. The earlier failed gates and original evidence limits retain their scopes.
The lead reports compile PASS and strict seeds 0-31 at 265/0. No retained pre-check log is supplied here.
Sol-B treats those results as operator reports and accepts no completed train gate from them.

No combined source, interface, test-coverage structure, or accounting finding remains on this exact head.
The exact-head train gate, both final member bodies and receipts, and lead merge/tree/push verification remain due.
The gate must include the three real report-control proofs, repeated-EOF repair proof, root presence decisions, and unchanged contracts-checkout comparison.
It must provide valid mutation catches without EV-2 credit and complete the selected fuzz harnesses.
This source verdict authorizes no specific retry and accepts no running gate, later head, pin, removal, or completed merge.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; combined train 5 source only, final train evidence pending)

## Round 2: final combined train 5 acceptance

Exact head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Gate and merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Lead final request: `msg_plugin-w_1791831674_e19b66`.
The ordered frozen members, delivery contract, complete patch checks, and source invariants from Round 1 remain applicable.
This round completes the exact-head gate, runtime coverage, final bodies, and four final review records.

The complete final member records are:

- #306 package: `1f0f77ddee0a8e5ef30b6294abf293fa6d0f3d8f`, train 5 final section in verdicts/gs-gate-speed.md.
- #306 integration: Round 8 in this commit's verdicts/stage1/gs-built-in-coverage.md.
- #315 package: `46e1a72a5ef550165fcd04c7f7ad257818bd4769`, Round 4 in verdicts/p6-e2-report-holds.md.
- #315 integration: Round 4 in this commit's verdicts/stage1/p6-e2-report-holds.md.

Sol-B reads both complete applicable package sections and both complete final member bodies.
The four records accept the same head, tree, base, completed gate, and frozen members. They have zero open findings.
Both bodies separately record the train evidence and preserve original and failed-gate scopes.
No source change occurs after Round 1's accepted composition.

### Completed gate and coverage

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-5-2683de7c-pool-20261012-112942-61568.log`.
Independently verified SHA256: `bcb49fb7a9644f33b0a5f1b2dbc0e16dda3fb6cd84f1a4f291bfc2427375d536`.
Sol-B reads the exact identity, base accounting, normal test runs, checker results, raw mutations, citations, fuzz completions, summaries, and exits.
Lists reports the recorded base as the merge base. All ten steps PASS in 1,747.7 seconds on msa1.
Default passes 1,998 tests across 23 binaries, with 427 skipped.
Ordinary slow passes 663 tests across 25 binaries, with 2,078 skipped.
The unmutated mutation baseline passes 673 tests across 35 binaries.

Sol-B maps every test in Round 1's 46-test source inventory to its exact binary and qualified name.
All 43 default tests and all three slow tests have normal PASS records in their assigned tiers.
This includes all 18 new #307 tests and all 28 new #315 tests.
Every one of the 23 default run binaries and 25 ordinary slow run binaries has a normal root-test PASS.
The name check scans 251 Rust files. The coverage checker checks 1,098 slow-only tests without a coverage or root-presence violation.
The runtime record therefore completes the earlier coverage-structure condition.
It does not exceed the accepted manual name rule or establish that a rebuilt binary cannot link a stale library.

The real report release, break, and delay proofs PASS. The repeated-EOF, PumpBound, and worker report-order proofs PASS.
The command fixture, root-presence regressions, and contracts-checkout comparison PASS.
The checkout comparison takes 0.104 seconds in ordinary slow.
The inherited determinism test passes in 0.022 seconds.
The compiled shared testkit and its recorded public behavior preserve both control families under the accepted composition.

Strict conformance passes 265 with zero failures. Pending remains 403; real-pending remains 34.
Held-full runs three budget IDs at 32 seeds, with 96 expected ignored trials and zero failures.
The real minimum records 66/68 passing and 67/69 accepted under its stated accounting.
The unchanged sets and gate accounting approve no additional pending promotion.

### Mutation and fuzz evidence

Sol-B independently finds 176 unique tested mutation identities.
Stage 1 catches 147, misses 14, reports 15 unviable, and has zero timeout.
The unmutated slow baseline passes. The stage 1 missed set equals the 14 stage 2 caught identities exactly.
The final result has 161 caught, 15 unviable, zero missed, and zero timeout.
All 161 unique raw catches equal their citation identities. Every cited catcher has a normal PASS.
Unviable results supply no behavioral catch evidence.

The checker files retain the 17 identities from #306's accepted initial gate: 15 caught and two unviable.
The other changed source scope has 159 identities: 146 caught and 13 unviable.
The command-to-Ok replacement and withholds-to-false replacement are caught by their normally passing regression tests.
The train supplies valid current outcomes rather than relabeling the original member evidence.

No runtime FLAKY or INCONCLUSIVE result appears.
The single INCONCLUSIVE string is stored diagnosis text in the expected known-flaky exclusion report at line 7140.
EV-2 passes ordinary slow in 0.103 seconds and the unmutated baseline in 0.115 seconds.
Its exact mutation pair is excluded and supplies no catch credit. These passes neither remove its entry nor close its structural fix.
The .44 pin and EV-2 entry remain. #308 and #316 are not members of this train.

All four selected fuzz harnesses complete their 60-second limits, with 61 seconds reported for each:

- link_decoder: 6,230,000 cases.
- msg_decoder: 3,615,174 cases.
- guardian_command_decoder: 7,088,305 cases.
- guardian_log_decoder: 28,432,924 cases.

The job and wrapper exit 0 after 1,854 seconds, with one second queued and 1,853 seconds running.
The completed gate satisfies the final runtime, coverage, mutation, and fuzz evidence conditions for this exact composition.

### Final disposition

No member or combined source, interface, body, accounting, or supplied-evidence finding remains.
All prior source findings stay closed. The historical RED and stopped gates, copied reversal provenance, and local limits retain their scopes.
The Linux gate supplies no new Mac execution or general full-seed real-tier claim.
The existing CONTROLS warning remains the recorded P6 follow-up; it changes no frozen source or completed verdict.
The lead must still verify ancestry, the final equal-tree merge, and remote publication.
This final acceptance authorizes no specific retry and accepts no later head, pin, status removal, or completed merge.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; final combined train 5 at the exact gated head, lead merge verification pending)

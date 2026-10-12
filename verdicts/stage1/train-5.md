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

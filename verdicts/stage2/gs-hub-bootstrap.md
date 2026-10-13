# Hub bootstrap integration review

## Round 1

Repository: trybotster/botster-hub. PR: #212. Branch: stage2/gs-hub-bootstrap. Risk: HIGH, gate code.
Exact head: `e9f344923fba060c4d88626e7864582b7c904ced`.
Exact tree: `aead64cb1b11dfd1a37270c844c42c359ea62962`.
Base: `c5f088efa0b9dacf5af4f16f46ec6c74d3668f51`, an empty root.
Lead assignment: msg_plugin-w_1791846608_19f9cc. READY: msg_plugin-w_1791850700_6a5bb6.
Sol-B reviews Git objects in the owner's Hub worktree without changing that worktree.

### Source and scope

The patch adds 30 paths and 2983 lines. The source diff check passes.
Sol-B reads all new Rust modules, manifests, profiles, list data, and repository-specific pool settings.
The three product crates are skeletons. They supply no implemented daemon, plugin isolation, or agent story.
The CLI's hub and tui names follow the user's explicit naming decision recorded by the lead.
Every workspace member inherits unsafe_code forbid.
The toolchain is 1.97.0. The lockfile uses one contracts-v0.1.50 checkout and one Core gate-kit revision.
There is no patch override or Core-contract dependency with a competing tag.

Repo-plan section 5 binds this review. The lead explicitly approves the nine-step bootstrap shape in the shared handoff.
Core gate-kit supplies the generic taint, unsafe-exception, high-tier, JUnit, and mutation-log logic by dependency.
The Hub keeps its policy, lists, command assembly, and I/O wrappers. It does not copy Core's full xtask.
The dependency is Core 28a29d32, as temporarily approved by the lead.
Gate-kit source blobs equal the later #331 member head 57cb242f; that later delta changes test-process documentation only.
The lead still requires a move to the Core v1 merge revision before the final Hub gate.
This review supplies no separate Core #331 verdict or delivery approval.

Sol-B independently compares the minimum list with the approved pin and additions.
All 125 original TSV rows remain verbatim. The only additions are the two approved story-2 operation rows.
Counts: HP 59, HC 45, LB 10, CLI 11, UI two; total 127.
Every identifier is pending. The pending set exactly equals the minimum set of contract-and-identifier pairs.
Against the .50 ledger, the only unmatched pair is the explicitly named HP hs_1_hello_first wait.
Its HC pair remains independently matched. The named HP A11 wait does not earn conformance credit.
There is no conformance subject or running identifier at this bootstrap stage, as the approved shape states.

The shared job-common.sh equals the contracts file byte for byte.
SHA256: `0bf5dbc170372d33230a112d0b1562ff17a971e5810f22c5ae46e85cf86acc11`.
The Hub settings select its own project, private dependency fetch, and mutation artifacts.
The gate container receives no GitHub token and runs without network under the shared pool plumbing.

### H212-I1 — HIGH — Mutation report rejection does not fail the gate

Evidence: xtask/src/mutants.rs, lines 125-138 at this head.
The wrapper reads outcomes.json only if the read succeeds.
It prints stage_report errors and then returns only the cargo-mutants exit-code verdict.
Core gate-kit deliberately rejects an unmutated baseline retry, an unknown nextest status, or a caught mutant without a failed test.
The Hub wrapper discards that rejection. With cargo-mutants exit 0, those paths still return success.
A missing outcomes file also silently bypasses the entire report.
Thus, the checked parser does not protect the gate's actual verdict.

Required change: require readable outcomes and logs, propagate stage_report errors, and use the validated report in the final verdict.
Preserve the distinction between a real catch and a fail-then-pass result.
Add a focused decision proof for successful tool exit with rejected report evidence, including a baseline retry.
This requests no new parser or check family; gate-kit already supplies the rejection.

### H212-I2 — HIGH — Broad I/O exclusions hide untested gate decisions

Evidence: .cargo/mutants.toml lines 13-32; xtask/src/ci.rs lines 87-91.
The patterns exclude every mutation inside each named command, not only replacement of the whole I/O body.
For example, the configured regex matches `xtask/src/ci.rs:88:8: delete ! in command`.
That change reverses the final check that a failed gate step makes CI fail.
The summary test proves printed text, but it does not prove this command verdict.
Other excluded command branches also decide whether tool status, evidence, and deadline failures are accepted.
The configuration's claim that each shell decision has a tested pure function is therefore false.

Required change: move independently testable success/failure decisions into tested helpers used by the actual wrapper.
Narrow each exclusion to the specific I/O replacement it justifies.
Retain decision mutants, including the aggregate CI verdict, in the HIGH-path run.
Do not add a broad exclusion or count a helper test as proof of a different excluded branch.
Use Core's shared decision logic where it applies, under section 5's dependency rule.

### H212-I3 — MEDIUM — A tier deadline does not clean up the spawned process tree

Evidence: xtask/src/budget.rs lines 152-170.
The wrapper spawns cargo nextest without a separate process group.
At the deadline, child.kill terminates only that direct child, and child.wait reaps only that child.
The wrapper neither owns nor cleans up the nextest and test descendants.
It also discards kill and wait errors; a wait error after spawn can return without cleanup.
CI continues to later steps after a failed tier, so retained descendants can run beside the later mutation step.

Required change: own the tier's process group and clean up the admitted process tree on every exit path.
Report kill, wait, and leftover failures. Keep cleanup bounded and preserve the existing tier budgets.
Supply a named process proof that reaches the deadline and verifies descendant cleanup.
Reuse applicable Core process-lifecycle infrastructure by dependency; do not copy a new Core check family.
This is a defect in the existing test-budget step, not a request for another gate step.

### Supplied evidence

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-hub-stage2-gs-hub-bootstrap-e9f34492-pool-20261012-171623-39635.log`.
Computed SHA256: `8a8c85ec60ed0debd6e42051697c01f41d3a5d984a301a0609322c052bed7f71`.
The header identifies this exact head and empty-root base.
All nine steps report PASS. The job exits 0 after 37 seconds, with zero queued; the wrapper exits 0 after 38 seconds.
Default tests pass 23 with zero skipped. The largest reported test time is 0.003 seconds.
The lists report 127 minimum, 127 pending, zero running, and the one named wait.
Slow reports no member with a slow feature and runs no slow proof.

Mutation results are 91 tested, 87 caught, four unviable, zero missed, and zero timeouts.
Sol-B independently matches all 87 raw mutant identities to their catch citations.
Every cited catcher appears in the ordinary default PASS results.
The four unviable outcomes are reported. No runtime FLAKY or INCONCLUSIVE result appears in this supplied gate.
These supplied results do not close the fail-open source paths identified above.

The earlier 6bbf6ae9 gate remains RED with one missed subtraction mutant.
Computed log SHA256: `27291fc62f5e764799fad071fd65501f99a821b0e59269a23ed2a44a75dfbbe1`.
Its wrapper exits 1 after 318 seconds. Sol-B reads the exact follow-up diff.
The revised full-list fixture uses four minimum and one pending row, which distinguishes subtraction from division in the public summary.
This is a new-head repair, not a retry that changes the earlier RED result.

Sol-B reads the complete PR body and preserves its dependency-pin and conformance limits.
The owner and package reviewer receive the three findings directly.
The package reviewer confirms I1 and I2 in msg_plugin-w_1791851055_4f659a.
The package reviewer also owns a taint-wrapper finding: tracked-file read errors are silently skipped at taint.rs lines 29-32.
Sol-B confirms that source path. The package finding also requires closure before any future CLEAN.
A matching package receipt remains due before any future CLEAN, along with review of the repairs and required exact-head evidence.
The lead controls the dependency landing, final pin, gate, train, and merge checks.

VERDICT: NOT CLEAN (two HIGH and one MEDIUM integration finding open; the package taint finding also remains due).
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

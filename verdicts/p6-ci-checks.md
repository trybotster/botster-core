# P6 CI checks review

## Round 1 — PR #181 — 2026-10-09

Reviewed head: `f5652517262ee7ce6d64b1787d2b02fc28df72ae`.
Merge base: `da43a1b1f85fd0b62da2e69767fc6ea0ba8cd594`.
Supplied gate base: `c869dbeaf3a2922e4f4e7202f8e55490f7ce99fa`.
PR: https://github.com/trybotster/botster-core/pull/181.

The reviewer inspected source and supplied evidence. The reviewer ran no gate, build, test, or mutation job.
The reviewer changed no product code. The examples below describe source findings, not reviewer-executed tests.
The implementer accepted B1 through B9 and will submit a replacement READY head.

### B1 — HIGH — Import resolution loses chains and lexical scope

Location: `xtask/src/process_check.rs:144-177`, `181-202`.

`Uses::collect` puts every import in one map, including imports from separate modules and functions.
A later import can replace an earlier binding from a different scope.
`Uses::resolve` expands only the first binding once.

For example, `use std::thread as th; use th::sleep as nap;` resolves `nap` to `th::sleep`.
The sleep rule requires the final path segment before `sleep` to be `thread`. Thus, this alias chain escapes this check.
A later module can also replace the binding used to check an earlier module.

Resolve import chains within their Rust scopes. Add fixtures for the chain and the separate-module replacement.
Rust specifies local import bindings and block scope in its [use declaration reference](https://doc.rust-lang.org/reference/items/use-declarations.html).

### B2 — MEDIUM — One allowlist entry can permit two calls

Location: `xtask/src/process_check.rs:90-98`, `379-388`, `630-632`.

`Finding` identifies a call with its file, line, item, and rule. It has no column or distinct call identity.
`dedup` therefore merges two calls on the same line in the same item when both break the same rule.
For example, `child.wait(); child.wait();` becomes one finding. One allowlist entry permits both calls.

Keep a distinct identity for each call. Preserve that identity in macro scans.
Add a fixture where one entry permits one call and rejects the second call on that line.

### B3 — HIGH — External test modules lose test scope

Location: `xtask/src/process_check.rs:111-134`, `523-537`, `611-629`.

The check scans files separately. It inherits test attributes only through inline syntax in the current file.
For example, `#[cfg(test)] #[path = "support.rs"] mod support;` makes `support.rs` test code.
A helper in that file has no local test attribute. Its arbitrary filename selects `Scope::TestItems`.
The check then skips a raw child wait in that helper, even when a test calls the helper.

Walk external module declarations. Inherit test scope through the module tree, including `#[path]` declarations.
Add a fixture with a banned call in an external test helper that has no local test attribute.

### B4 — MEDIUM — Timer enforcement still has syntax and scope gaps

Location: `xtask/src/timers.rs:63-74`, `178-194`, `243-249`.

`has_marker` searches raw line text. A string containing `// timer: deadline — reason` can mark the next timer statement.
The string is not a comment that documents that timer.
The visitor checks calls but does not check function references, such as `let wait = Receiver::recv_timeout;`.
`scan_files` also skips every xtask file, including xtask tests.

Accept markers only from actual comments. Check timer references and aliases.
Check xtask test code. Keep any fixture exclusion limited to the fixture workspace.
Add fixtures for a marker string, a function reference, and an unmarked timer in an xtask test.

### B5 — HIGH — A gate decision can still receive a mutation exclusion

Location: `xtask/src/gate_decisions.rs:265-326`.

The check permits an excluded function when its reason names a tested, unexcluded function that it calls.
It does not establish that the excluded function is an I/O shell rather than a gate decision.
The real `ci::mutation_decision` calls the tested `mutation_verdict` function.
An exclusion for `mutation_decision` with reason `mutation_verdict` meets these acceptance conditions.
That exclusion removes mutation coverage for the missing-outcomes and listed-count decisions.

Reject exclusions of gate decision functions, including decisions that call other decisions.
Add the `mutation_decision` exclusion as a rejected fixture.
The lead's ruling forbids mutation exclusions for gate pass/fail decisions.

### B6 — HIGH — A proof citation can survive deletion or conditional ignore

Location: `xtask/src/mutants_cited.rs:169-189`, `403-450`.

When a cited test name no longer exists, the check treats the name as an ordinary word.
Any occurrence in another tracked file satisfies that fallback, including documentation, comments, and string literals.
A deleted proof test therefore still passes if its name remains in a README, verdict, or fixture string.

`read_attrs` also ignores `cfg_attr`. A test with `#[cfg_attr(all(), ignore)]` is reported as a test that a tier runs.
Nextest skips that ignored test under the gate's normal selection.

Distinguish proof-test citations from source references. Require each proof-test citation to name a selected test.
Evaluate conditional attributes, or reject conditions that the check cannot evaluate.
Add fixtures for a deleted test with a remaining document mention and a conditionally ignored test.

### B7 — HIGH — Tool completion does not own a process group

Location: `crates/botster-test-process/src/child.rs:196-201`, `148-185`.

`run_to_completion` uses `OwnedChild::spawn`, which owns only the tool's leader.
The timeout path kills and reaps that leader. It does not end the tool's descendants.
A shell that starts a FIFO-blocked `/bin/cat` in the background can leave that child after the completion deadline.
Closing output pipes does not end a child blocked while it opens a FIFO.
The helper accepts a general `Command` and its tests already use a shell.

Give the helper process-group ownership. End all group members on success, error, timeout, and panic paths.
Add a real descendant proof with a FIFO and observable EOF. Preserve production's exclusive reap ownership.
BUILD.md testing rule 10 requires every real-process test to own and clean its process group.

### B8 — MEDIUM — Platform derivation can exclude code that the OS compiles

Location: `xtask/src/platform_code.rs:159-172`, `284-292`.

The derivation marks a module file as wholly excluded when any gated declaration reaches it.
It does not check whether another active declaration reaches the same file.
For example, a Mac declaration and a Linux declaration can both use `#[path = "shared.rs"]`.
On Linux, the Mac declaration marks `shared.rs` as excluded, although the Linux declaration compiles it.
The generated regex then removes all mutants from that compiled file.

Exclude a file only when all declarations that reach it are inactive on the selected OS.
Alternatively, reject ambiguous module paths. Add the shared-file fixture.

### B9 — LOW — The read regression asserts private state

Location: `crates/botster-test-process/src/read.rs:419-434`.

`a_silent_writer_fails_both_reads_at_the_deadline` asserts `second.buffered()` after the failure.
That assertion inspects the private buffer rather than public read behavior.

Drop the writers. Use the public `second.to_eof` result to prove that the second reader retains `err`.
Keep the existing assertion for the reported partial stdout.

### Evidence and preserved behavior

The supplied Linux gate log is `botster-core-stage1-p6-ci-checks-f5652517-pool-20261009-064840-8901.log` under `~/botster-sessions/gates/`.
It passes all ten jobs, 991 default tests, and 247 slow tests.
It reports 462 caught mutants and 20 unviable mutants, with no misses or timeouts.
The real hang fixture and the platform exclusion fixture both pass in the slow tier.
The hang fixture checks the timeout result from cargo-mutants. The mutants profile removes nextest's termination limit.
The mutation command uses immediate fail-fast behavior so a separate assertion failure counts as a catch.

The supplied run without mutation configuration is `botster-core-stage1-p6-ci-checks-f5652517-pool-20261009-065835-17428.log`.
It reports the same 462 caught and 20 unviable mutants, with no misses or timeouts.
The supplied Mac log is `botster-core-stage1-p6-ci-checks-f5652517-pool-20261009-071401-29779.log`.
It passes 78 process tests and 200 xtask tests. It skips one prebuilt-anchor test that the Linux gate runs.
Its focused read and child mutation run reports 41 caught and 15 unviable mutants, with no misses or timeouts.

The process check includes rejected fixtures for raw calls, aliases, macros, and shell loops.
These fixtures provide red-on-revert coverage for the syntax forms that they contain. They do not cover B1 through B4.
The missing-outcomes gate defect is fixed: a failed mutation run cannot pass because its outcomes file is absent.
A run with no mutable code must first establish an empty mutant list and report that reason.
B5 concerns exclusion enforcement, not that runtime fix.

The Prior-art table keeps a decision and reason for every required item.
The new completion helper uses the existing bounded read and owned-child components. B7 requires group ownership for that helper.
This PR does not migrate the duplicated process guards. Those migrations still require separate contract-assertion review.
The shared crate retains derived deadlines, event waits, and production's exclusive reap ownership.
The nine findings prevent a CLEAN verdict.

VERDICT: NOT CLEAN

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

## Round 2 — PR #181 — 2026-10-09

Reviewed head: `a4b803e435553d7e8f9851991515189d62dfe67d`.
Previous reviewed head: `f5652517262ee7ce6d64b1787d2b02fc28df72ae`.
Merge base: `13d7db0925cd080b5734a7b118c7fb440aa28c28`.
Supplied full gate base: `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.

The reviewer checked the stated tier first. HIGH is correct under BUILD.md risk rules 1 and 3.
The PR changes gate decisions, the shared crate, and workspace configuration.
Every finding must close, including LOW. The integration review remains required.
The reviewer read BUILD.md at `56bd0a5347a537d25bbee65a67854e0e317a9b9a`.

The reviewer inspected the delta, the base merges, the regression fixtures, and supplied evidence.
The reviewer ran no gate, build, test, or mutation job and changed no product code.
The remaining cases below are source findings. The reviewer did not execute these cases.

### Findings that close

- B1 closes for the reported alias chains and scope replacement. Import scopes now separate files, inline modules, and blocks.
  The resolver follows chains, `self`, `super`, and globs, and stops cycles.
  `a_use_chain_resolves_within_its_scopes` checks the reported cases.
- B2 closes. Findings have columns, and separate calls remain separate sites.
  `two_calls_on_one_line_are_two_sites` checks ordinary code and a token-scanned macro.
- B4 closes for the reported marker strings, timer references, and xtask test exclusion.
  Marker matching excludes literal spans. The visitor checks timer references and rename chains.
  The check includes xtask tests. Its external module lookup still depends on the B3 fix below.
- B7 closes. `run_to_completion` uses `OwnedChild::spawn_group`.
  The existing group owner keeps the leader unreaped until it ends other group members, then reaps only its own leader.
  The new slow test observes EOF from a background child's inherited pipe on the success path.
  The same test also checks timeout cleanup with a zero deadline. That case does not establish background-child readiness.
  Both paths use the same group cleanup on Drop.
- B9 closes. The test drops the writers and asserts the public `second.to_eof` result.
  The private `buffered()` helper is removed. The test keeps the existing partial-stdout error assertion.

### B3 — HIGH — Inline module paths still select the wrong test file

Location: `xtask/src/process_check.rs:734-764`.

The module walk now inherits test scope across external files.
However, it resolves every explicit `#[path]` beside the source file, even inside an inline module.
Rust includes the inline module directory when it resolves that path.

For example, `src/lib.rs` can declare this test module:

```rust
#[cfg(test)]
mod checks {
    #[path = "support.rs"]
    mod helper;
}
```

Rust compiles `src/checks/support.rs`. The check selects `src/support.rs` when that file also exists.
A raw wait in an unannotated helper in `src/checks/support.rs` then escapes the check.
The timer check shares this module walker and has the same scope gap.
The [Rust module reference](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) specifies the inline path rule.

Resolve module paths with the Rust module context, including inline `#[path]` attributes.
Add a fixture with both candidate files and a banned call only in the file that Rust compiles.

The new module fixture also supplies `gen.inc` with a raw wait and omits that file from the expected findings.
`scan_files` accepts that reached file but skips its contents. The command instead filters it out and reports a missing module.
Make these paths consistent. Parse a reached source module regardless of its extension, or reject it explicitly.
The non-Rust-extension case fails closed in the command. The inline path case above is the HIGH false-clean defect.

### B5 — HIGH — A pure callback name still permits a decision exclusion

Location: `xtask/src/gate_decisions.rs:228-241`, `290-316`, `475-498`.

The check now rejects operator mutants and requires I/O for whole-body exclusions.
It rejects the reported `mutation_decision` case when its callback is named `run`.
However, `visit_expr_call` marks I/O from the final function name before `path_call` filters local bindings.

For example, this function can perform only a pure decision:

```rust
fn forwarded(write: fn() -> Option<i32>) -> Result<()> {
    mutation_verdict(write())
}
```

The callback name `write` marks `forwarded` as I/O. The callback's name does not establish I/O.
A whole-body exclusion with reason `mutation_verdict` then passes when that decision is tested and unexcluded.
Method names such as `status` also mark I/O without checking the receiver's identity.

Filter local bindings before I/O classification. Resolve the callee or receiver that establishes I/O.
Reject an exclusion when the check cannot establish that identity.
Add a rejected fixture with the `write` callback and a pure method whose name matches an I/O method.
Transitive I/O classification is acceptable only when a resolved callee establishes I/O.

### B6 — HIGH — A proof citation can still name a function that no tier runs

Location: `xtask/src/mutants_cited.rs:207-241`, `493-567`.

The check now rejects retained document, comment, and string mentions. It evaluates conditional `ignore` attributes.
However, removing `#[test]` from a cited proof function makes its name pass through the source-identifier fallback.
No tier runs that function as a test.
The new deleted-test fixture explicitly accepts a non-test function with the former proof test's name.

Distinguish proof-test citations from source references. Require each proof-test citation to name a selected test.
Add a rejected fixture that removes `#[test]` but keeps the cited function.

`conditional` also ignores a `cfg` attribute inside `cfg_attr`.
For example, `#[cfg_attr(all(), cfg(any()))] #[test] fn proof_test_name() {}` is absent on both supported systems.
The inventory nevertheless reports that a tier runs it.
Rust expands the conditional `cfg` and removes the function, as its [conditional compilation reference](https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute) specifies.

Evaluate conditional `cfg` attributes, including nested attributes, or reject conditions that the check cannot evaluate.
Add the absent-test fixture. Keep the conditional-ignore regressions.

### B8 — MEDIUM — Inline paths still hide an active platform declaration

Location: `xtask/src/platform_code.rs:160-175`, `302-320`.

The new active-declaration search fixes the reported root-level shared-file case.
However, `module_file` uses the source directory for an inline `#[path]`, as in B3.
It can therefore record the wrong file as active and exclude the compiled file.

For example, `src/lib.rs` can declare these modules:

```rust
#[cfg(target_os = "macos")]
#[path = "linux/shared.rs"]
mod mac;
#[cfg(target_os = "linux")]
mod linux {
    #[path = "shared.rs"]
    mod live;
}
```

On Linux, Rust compiles `src/linux/shared.rs`.
If `src/shared.rs` also exists, the derivation records that wrong file as active.
The Mac declaration then causes a whole-file exclusion for the compiled `src/linux/shared.rs`.
The [Rust module reference](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) fixes the applicable path.

Apply the correct module path rule to the platform derivation too. Add this inline shared-file fixture.
Keep the new root-level and descendant fixtures.

### Base merges and evidence

Merge `4f26bc9bc07bfbc29529dcab41110ef93b961a56` brings v1 `c869dbeaf3a2922e4f4e7202f8e55490f7ce99fa`.
Its 40 base-only paths match the base parent. It adds no unexpected path and has no overlapping path.
Merge `d9dff45cc4917e009a3bec9733d9f6a36dac0771` brings v1 `13d7db0925cd080b5734a7b118c7fb440aa28c28`.
Its ten base-only paths match the base parent. It adds no unexpected path.
`Cargo.lock` is the only overlapping path. It preserves the base pin and adds the two previously reviewed xtask dependencies.
The reviewer compared Git objects. The reviewer did not run `base-merge-check` or claim a carried CLEAN verdict.

The supplied full log is `botster-core-stage1-p6-ci-checks-a4b803e4-pool-20261009-084712-58261.log` under `~/botster-sessions/gates/`.
Its header names the exact reviewed head and gate base `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.
It passes all ten jobs, 1006 default tests, and 248 slow tests.
It lists 574 mutants: 549 caught and 25 unviable, with no misses or timeouts.
The reported regression fixtures, the real descendant test, the hang fixture, and the platform fixture pass.

The supplied slow-profile run is `botster-core-stage1-p6-ci-checks-a4b803e4-pool-20261009-090011-78599.log`.
It has no immediate fail-fast argument. It reports 549 caught and 25 unviable mutants, with no misses or timeouts.
The supplied Mac log is `botster-core-stage1-p6-ci-checks-a4b803e4-pool-20261009-091935-8297.log`.
It passes 79 process tests and 214 xtask tests. It skips one prebuilt-anchor test that the Linux gate runs.
Its focused read and child mutation run reports 38 caught and 15 unviable mutants, with no misses or timeouts.

The supplied evidence proves the recorded cases. It does not cover the remaining B3, B5, B6, and B8 cases.
The Prior-art note remains as reviewed. No process-guard migration enters this delta.
The shared crate keeps bounded event waits and production's exclusive reap ownership.
Four findings remain open. This is the second NOT CLEAN round; the round-limit report is not yet due.

VERDICT: NOT CLEAN

## Round 3 — PR #181 — 2026-10-09

Reviewed head: `7bb34d7618238f116613319f0883fae0d1fe82f7`.
Previous reviewed head: `a4b803e435553d7e8f9851991515189d62dfe67d`.
Merge base and full gate base: `67fd748a369d0ed544e3373d20c887024af31fa8`.

The reviewer checked the stated tier first. HIGH remains correct under BUILD.md rules 1 and 3.
The reviewer inspected the delta, regression fixtures, base merges, and supplied evidence.
The reviewer ran no gate, build, test, or mutation job and changed no product code.
The remaining cases below are source findings. The reviewer did not execute these cases.

### Fixes that address the reported examples

The module tree now parses reached files regardless of their extension. The `gen.inc` fixture is covered.
The process, timer, and platform checks resolve an inner `#[path]` under its inline module directory.
The new fixtures include the previous decoy-file cases.

The I/O classifier rejects callback and method names alone. It filters local bindings before path classification.
It follows scoped imports and module paths for transitive calls.
The `forwarded_write` fixture rejects the previously reported pure callback decision.

The proof-name fallback excludes test modules and test files.
The new fixture rejects a function without `#[test]` inside a `#[cfg(test)]` module.
The citation check evaluates conditional `cfg` attributes and keeps the conditional-ignore regressions.

B1, B2, B4, B7, and B9 remain closed for the reported cases.
The shared process crate has no code delta in this round. Its group ownership and production reap separation remain as reviewed.
Four related cases below keep B3, B5, B6, and B8 open.

### B3 — HIGH — The module walk ignores an inline module's own path

Location: `xtask/src/process_check.rs:750-759`.

The fix applies `#[path]` to an external module declared inside an inline module.
It still ignores `#[path]` on the inline module itself.
For example, `src/lib.rs` can declare this test module:

```rust
#[cfg(test)]
#[path = "alt"]
mod checks {
    #[path = "support.rs"]
    mod helper;
}
```

Rust reads `src/alt/support.rs`. The check reads a decoy `src/checks/support.rs` when that file exists.
A raw wait in an unannotated helper in the actual file escapes the check.
The timer check shares this walk. The citation walk also appends the inline module name and ignores its own path attribute.
The [Rust module reference](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) gives this combined path form.

Apply the inline module's own path attribute, or reject the unsupported form explicitly.
Add a fixture with the actual file and a decoy file. Keep the extension and inner-path fixtures.

### B5 — HIGH — Constructing a builder qualifies a decision as an I/O shell

Location: `xtask/src/gate_decisions.rs:54-77`.

`IO_FUNCTIONS` classifies `std::process::Command::new` as I/O.
That call constructs a command builder. It starts no process, as the [Command documentation](https://doc.rust-lang.org/std/process/struct.Command.html#method.new) specifies.

For example, this function contains a gate decision and starts no process:

```rust
fn forwarded(code: Option<i32>) -> Result<()> {
    let _ = std::process::Command::new("unused");
    mutation_verdict(code)
}
```

The classifier marks `forwarded` as I/O.
Its whole-body exclusion with reason `mutation_verdict` passes when that decision is tested and unexcluded.
The `std::fs` prefix has the same problem with `std::fs::OpenOptions::new`, which only constructs an options builder.

Require an actual I/O operation for the shell exemption. Construction alone must not exempt a gate decision.
Add rejected fixtures for both builders. Keep the callback-name and method-name regressions.

### B6 — HIGH — A top-level proof still passes after it loses its test attribute

Location: `xtask/src/mutants_cited.rs:568-599`.

The new fixture removes `#[test]` only inside an enclosing test module.
Rust also permits a top-level proof function in `src/lib.rs` without an enclosing `#[cfg(test)]` module:

```rust
#[test]
pub fn the_cited_proof() {}
```

Remove `#[test]`, and `Items` records `the_cited_proof` as a non-test-code identifier.
The reason still passes, although no tier runs that function as a test.
The [Rust testing reference](https://doc.rust-lang.org/reference/attributes/testing.html#the-test-attribute) defines the test attribute on a free function.

Distinguish proof-test citations explicitly from source references. Require proof-test citations to name selected tests.
Add a rejected fixture for the top-level attribute removal. Keep the test-module and conditional-cfg fixtures.

### B8 — MEDIUM — An inline module path can still exclude the compiled file

Location: `xtask/src/platform_code.rs:160-179`, `visit_item_mod`.

The inner-path fix does not apply an inline module's own `#[path]`.
For example, `src/lib.rs` can declare these modules:

```rust
#[cfg(target_os = "macos")]
#[path = "alt/shared.rs"]
mod mac;
#[cfg(target_os = "linux")]
#[path = "alt"]
mod linux {
    #[path = "shared.rs"]
    mod live;
}
```

On Linux, Rust compiles `src/alt/shared.rs`.
With a decoy `src/linux/shared.rs`, the derivation records the decoy as active.
The Mac declaration then excludes the compiled `src/alt/shared.rs` as a whole file.
The [Rust module reference](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) specifies the applicable path.

Apply the inline module's own path attribute, or reject this form explicitly.
Add the shared-file fixture with the decoy. Keep the root-level and inner-path regressions.

### Base merges and preserved assertions

Merge `927fd2e3e40e80328919594f5a7ba751893518b2` imports v1 `dd07eabd3fb5801ff8438f32d08924a7f440ddfa`.
Its twelve base-only paths match the base parent. `Cargo.lock` is the only overlapping path.
The lockfile preserves the base pin and the two previously reviewed xtask dependencies.
Merge `21c32f4a0f49cb888cfde40c2c2c147c1b74476c` imports v1 `3000ae14bf8b05efe410c22ac66d5915cf36ff45`.
Its two base-only paths match the base parent. It has no overlapping path.
Merge `7bb34d7618238f116613319f0883fae0d1fe82f7` imports v1 `67fd748a369d0ed544e3373d20c887024af31fa8`.
Its one base-only path matches the base parent. The base-merge test file is the only overlapping path.
That test file matches v1 except for the previously reviewed bounded Git helper. Its result assertions remain intact.
No merge adds an unexpected path. The allowlist delta only renames two entries to match the imported guard test name.
The reviewer compared Git objects. The reviewer did not run `base-merge-check` or claim a carried CLEAN verdict.
The Prior-art note remains as reviewed. No process-guard migration enters this round.

### Supplied evidence and its limits

The supplied full log is `botster-core-stage1-p6-ci-checks-7bb34d76-pool-20261009-102251-20084.log` under `~/botster-sessions/gates/`.
It names the exact reviewed head and base `67fd748a369d0ed544e3373d20c887024af31fa8`.
All ten jobs pass. It passes 1047 default tests and 248 slow tests.
It lists 602 mutants: 576 caught and 26 unviable, with no misses or timeouts.
The supplied slow-profile run is `botster-core-stage1-p6-ci-checks-7bb34d76-pool-20261009-103639-53749.log`.
It has no immediate fail-fast argument and reports the same mutation counts.
The existing real hang fixture and process descendant proof remain covered.

The supplied Mac log is `botster-core-stage1-p6-ci-checks-7bb34d76-pool-20261009-105725-95729.log`.
It passes 79 process tests and skips the prebuilt-anchor test that the Linux gate runs.
Its focused read and child mutation run reports 38 caught and 15 unviable mutants, with no misses or timeouts.
Its xtask run reports 216 passed tests and twelve timeouts. The enclosing shell's exit 0 does not make that test run pass.
The later `-113103-54442.log` reports 227 passed tests and one base-merge timeout.

The timing comparison is `botster-core-stage1-p6-ci-checks-7bb34d76-pool-20261009-113223-66706.log`.
It uses the slow profile and temporarily substitutes v1 `1dd1657a2c53f4da6953f7a349d7fa9d59b97ec6`'s base-merge test file.
Both versions pass the 26 base-merge tests under that profile. Several tests exceed two seconds with either helper.
This supports the reported inherited budget problem. It does not provide a passing default-tier Mac xtask run.
The implementer reports that P3 owns the move of these Git-heavy tests to the slow tier.
The reviewer did not treat the repeated runs as a structural fix or change a timeout value.

The evidence proves the recorded regression cases. It does not cover the four remaining cases.
This is the third NOT CLEAN round on #181. BUILD.md requires a lead decision before round four.
The lead supplied that decision before this verdict commit, in plan revision 23c, `stage1/plan` commit `def1d25e`.
Each source-reading check must list its supported forms. It must reject other forms and name the form and file.
For round four, B3/B8 close when the checks reject `#[path]` on an inline module.
B5 requires an explicit list of I/O operations and rejection when classification fails.
B6 requires strict `decision (proof, …)` citations. Wrong results on supported forms remain in scope.
The reviewer will report QUESTION with this exact head and the pushed verdict commit to acknowledge the round-limit decision.
The reviewer will wait for the replacement READY head before round four.

VERDICT: NOT CLEAN

## Round 4 — PR #181 — 2026-10-09

Reviewed head: `24fb122f32cb691bf073292f12b6a61448260c55`.
Previous reviewed head: `7bb34d7618238f116613319f0883fae0d1fe82f7`.
Merge base and full gate base: `7aec2bb917f48994d1705301d2383873a219c031`.

The reviewer checked the stated tier first. HIGH remains correct under BUILD.md rules 1 and 3.
The lead authorized round four under plan revision 23d, `stage1/plan` commit `9b7bc5a0`.
The reviewer read section 8 in `~/botster-sessions/pins/stage1-plan.e3c60694.md` and verified its SHA-256:
`e3c60694b24cd969151ad7948e54229a3ade059d1124124d7a19d528529c2db7`.
Each Rust source check must list supported forms. It must reject other forms and name the form and file.
An I/O shell exclusion must cite at least one proof as `decision (proof, ...)`.
A proof named only in free text does not satisfy that requirement.

The reviewer inspected the source delta, regression fixtures, configuration migration, base merges, and supplied gate log.
The reviewer ran no gate, build, test, or mutation job and changed no product code.
The remaining cases below are source findings. The reviewer did not execute these cases.

### Closed findings and preserved review scope

B3 closes for the reported module form. The shared rejection names an inline module's path attribute and its file.
The process, timer, and citation checks use that rejection. Their fixtures assert the rejection through their check results.
The platform check rejects the reported B8 fixture on Linux and Mac. A parent gate still hides the form, as detailed below.

B6 closes under the lead's citation rule. Proof citations use the strict decision-and-proof form.
The proof check rejects a top-level function after its test attribute is removed.
The fixture also retains the test-module cases and conditional-cfg cases.
The parser fixtures cover comment lines, source references, backticked code, and the count of cited names.
The gate-decision check requires a structured proof citation for a shell exclusion.

B5 rejects the reported `Command::new` and `OpenOptions::new` builders.
The new fixtures retain the callback-name and other-type method cases.
Two accepted forms still classify a pure forwarding decision as an I/O shell, as detailed below.
B1, B2, B4, B7, and B9 remain closed for the reported cases.

### B5 — HIGH — Pure environment functions still qualify as I/O

Location: `xtask/src/gate_decisions.rs:66-104`.

`IO_MODULES` still accepts every lowercase free function of `std::env`.
That accepts `split_paths` and `join_paths`, which parse and combine supplied data without reading or changing the environment.
The [split_paths documentation](https://doc.rust-lang.org/std/env/fn.split_paths.html) describes parsing supplied input.
The [join_paths documentation](https://doc.rust-lang.org/std/env/fn.join_paths.html) describes combining supplied paths.

For example, this forwarding decision still qualifies as an I/O shell:

```rust
fn forwarded(code: Option<i32>) -> Result<()> {
    let _ = std::env::split_paths("fixed");
    mutation_verdict(code)
}
```

Its whole-body exclusion passes with reason `mutation_verdict (verdicts)` when the decision has its tested, unexcluded fixture.
Replacing the parsing call with `std::env::join_paths(["fixed"])` has the same classification error.
These are wrong results on the documented free-function form. The closed-form rule does not close this finding.

Use an explicit list of actual I/O operations. Add rejected fixtures for both pure functions.
Retain the builder and process-start fixtures.

### B5 — HIGH — A shadowed command parameter retains its command classification

Location: `xtask/src/gate_decisions.rs:316-322`, `command_locals`, and `process_start`.

The check collects command bindings for the whole function before it visits calls.
A parameter typed `Command` remains in that map when a later non-call initializer shadows it.
For example:

```rust
struct Pure;
impl Pure {
    fn status(&self) {}
}
fn forwarded(cmd: std::process::Command, code: Option<i32>) -> Result<()> {
    let _ = &cmd;
    let cmd = Pure;
    cmd.status();
    mutation_verdict(code)
}
```

`CommandBindings` records the parameter. It does not record `let cmd = Pure` because the initializer has no call-chain root.
`process_start` then marks `Pure::status` as a process start through the retained parameter entry.
The same whole-body exclusion with reason `mutation_verdict (verdicts)` passes.
The function performs no I/O. A command parameter and a local binding are documented forms, but this case is not rejected.

Reject unsupported binding changes and name the form and file, or resolve the listed bindings correctly.
Add a fixture that checks rejection of the forwarding decision's exclusion after this shadowing.

### B8 — MEDIUM — A gated parent hides an unsupported module form

Location: `xtask/src/platform_code.rs:205-223`.

The rejection runs before the current item's gate. It does not run before an ancestor's gate.
For example:

```rust
#[cfg(target_os = "macos")]
mod outer {
    #[path = "alt"]
    mod inner {}
}
```

On Linux, `visit_item` records the outer module's gated lines and skips its descendants.
The derivation returns without rejecting the unsupported inline-module path.
On Mac, the derivation visits the inner module and rejects it.
The supplied fixture puts a gate on the unsupported module itself. It does not cover a gate on its parent.

The lead requires rejection on every OS. Reject unsupported forms before skipping a gated parent.
Add a fixture with this parent gate and assert rejection for both systems.
Other source checks can reject this fixture in the combined taint step. This finding concerns the derivation's required rejection on each OS.

### Configuration migration, process rules, and base merges

The mutation configuration migration changes comments only. All parsed TOML values remain identical to the previous head.
It retains 185 `exclude_re` entries and two `exclude_globs` entries.
The comments retain the test names and facts. Six shell reasons move their proof lists after their decision names.
The other rewrites mark function lists and `Ok(true)` as code. Free-text source references remain permitted by the lead's ruling.
No test assertion changes in this migration.

The process allowlist adds one testkit channel receive from #195/#196, with P3's ownership and reason.
The main thread's existing deadlines bound the steps before it sends on the channel.
If the test unwinds first, it drops the sender and releases the receive.
The added entry permits one site. It does not add a sleep, busy loop, process wait, or reap.
The shared process crate and Prior-art note have no delta. Their group ownership and production reap separation remain as reviewed.
No process-guard migration enters this round.

Merge `fc54187c45258790f01f0d8f3e5f94f0670f8f70` imports v1 `c06f5b982d05dd65b00661a6cb9bb8b3aeea5514`.
Its nineteen base-only paths match the base parent. `Cargo.lock` is the only overlapping path.
The lockfile retains the base changes and the two previously reviewed xtask dependencies.
Merge `24fb122f32cb691bf073292f12b6a61448260c55` imports v1 `7aec2bb917f48994d1705301d2383873a219c031`.
Its five base-only paths match the base parent. It has no overlapping path.
Neither merge adds an unexpected path. These are Git object comparisons, not a gate run or a carried CLEAN verdict.

### Supplied evidence and its limits

The full Linux log is `botster-core-stage1-p6-ci-checks-24fb122f-pool-20261009-115821-64845.log` under `~/botster-sessions/gates/`.
It names the exact reviewed head and base `7aec2bb917f48994d1705301d2383873a219c031`.
All ten jobs pass. It passes 1094 default tests and 248 slow tests.
It reports 644 mutants: 615 caught and 29 unviable, with no misses or timeouts.
The log shows the new module-rejection, citation, command-builder, and command-start fixtures passing.
The real hang fixture still fails its nested mutation step as a timeout and passes its enclosing contract test.
The platform-exclusion proof also passes in the slow tier.

The implementer supplies no new Mac or separate slow-profile mutation run for this round.
The round changes xtask code and comments, with no shared process-crate or macOS-only code delta.
The earlier Mac process evidence remains as recorded. The reviewer does not convert the earlier Mac xtask timeouts into passes.
The regression assertions turn red on reversal of the reported fixes by source inspection.
The reviewer did not execute a reversal or claim that a supplied run exercised the remaining cases.

B5 and B8 remain open. The reviewer sent the cases directly to the implementer and integration reviewer.
The lead's round-limit decision authorized this review. No further resolution inference is required to close these cases.
Before this verdict commit, the lead authorized round five for the replacement delta only, after P6 sends READY with a full gate.
A new finding in untouched code moves to P6's next HIGH PR. A gate hole still blocks #181.
Wait for the replacement READY head.

VERDICT: NOT CLEAN

## Round 5 — PR #181 — 2026-10-09

Reviewed head: `a05ab9a380c459cd609b5fafd7f4c17e17a9e7da`.
Previous reviewed head: `24fb122f32cb691bf073292f12b6a61448260c55`.
Merge base and full gate base: `7aec2bb917f48994d1705301d2383873a219c031`.

The reviewer checked the stated tier first. HIGH remains correct under BUILD.md rules 1 and 3.
The lead authorized this review for the replacement delta only, with a full gate before READY.
A new finding in untouched code moves to the next HIGH PR. A gate hole still blocks #181.
The reviewer inspected the one-commit, four-file delta and the supplied logs.
The reviewer ran no gate, build, test, or mutation job and changed no product code.

### The three round-four cases close

The I/O classifier now uses explicit lists of filesystem and environment functions.
It does not list `split_paths` or `join_paths`. The fixtures retain accepted filesystem and environment operations.
The gate-level fixture rejects an exclusion of `forwarded_split`.

The command-binding index counts each pattern binding.
It accepts a command binding only when the function binds its name once.
That rejects the reported parameter shadow, the local shadow, and the closure rebinding.
The gate-level fixture rejects an exclusion of `forwarded_shadow`.

The platform derivation now makes a separate rejection pass through the parsed file.
That pass ignores platform gates and reports the unsupported inline-module path below a gated parent.
The new fixture asserts rejection on both Linux and Mac, with the form, file, line, and column.
B8 closes. The previously closed B1, B2, B3, B4, B6, B7, and B9 remain closed for the reported cases.

### B5 — HIGH — A block import does not apply to the stored command binding

Location: `xtask/src/gate_decisions.rs:575-588`, `command_locals` (modified in this delta).

The function collects command bindings before `visit_block` enters a block's import scope.
`command_locals` resolves each initializer through the imports active at the function's entry.
A block import can therefore replace the type at the actual initializer without replacing its stored command root.
For example, in `xtask/src/ci.rs`:

```rust
use std::process::Command;
pub fn builder() -> Command {
    Command::new("unused")
}
mod pure {
    pub struct Command;
    impl Command {
        pub fn new() -> Self { Self }
        pub fn status(&self) {}
    }
}
fn forwarded(code: Option<i32>) -> Result<()> {
    {
        use crate::ci::pure::Command;
        let cmd = Command::new();
        cmd.status();
    }
    mutation_verdict(code)
}
```

`cmd` is bound once. The new binding-count filter accepts it.
The stored `Command::new` resolves to `std::process::Command::new` through the file import.
The actual initializer uses `pure::Command`. Its `status` method does no I/O.
The expression visitor resolves the actual constructor under the block import, but does not replace the stored command root.
`process_start` marks `forwarded` as an I/O shell through that stored root.
Its whole-body exclusion with reason `mutation_verdict (verdicts)` passes when that decision has its tested, unexcluded fixture.

This is a gate hole: the check accepts an exclusion of a pure forwarding decision.
Block imports and single-binding command locals are listed forms. The check does not reject their combination.
The modified command-binding function is in the authorized delta. The lead's gate-hole exception also keeps this case in scope.

Reject command bindings under unsupported local imports and name the form and file, or resolve bindings at their own import scope.
Add a fixture that checks rejection of the forwarding decision's exclusion.
The reviewer sent this source finding directly to the implementer and integration reviewer. The reviewer did not execute the fixture.

### Supplied evidence and preserved contracts

The full Linux log is `botster-core-stage1-p6-ci-checks-a05ab9a3-pool-20261009-122351-51352.log` under `~/botster-sessions/gates/`.
It names the exact reviewed head and base `7aec2bb917f48994d1705301d2383873a219c031`.
All ten jobs pass. It passes 1095 default tests and 248 slow tests.
It reports 652 mutants: 623 caught and 29 unviable, with no misses or timeouts.
The focused delta log is `botster-core-stage1-p6-ci-checks-a05ab9a3-pool-20261009-121906-35982.log`.
It reports 25 mutants tested and 25 caught.
The supplied runs cover the reported regression fixtures. They do not cover the remaining block-import case.
The regression assertions turn red on reversal of the reported fixes by source inspection. The reviewer did not execute a reversal.

This head has no base merge, configuration migration, shared process-crate change, or process-guard migration.
The Prior-art note and process ownership rules have no delta.
The delta retains the gate-level rejection assertions and the existing accepted shell cases.
The reviewer did not broaden this review to untouched code or claim a new Mac pass.

B5 remains open. Wait for the replacement READY head.

VERDICT: NOT CLEAN

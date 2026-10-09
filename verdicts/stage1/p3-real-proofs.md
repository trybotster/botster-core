# Integration review: #192 (the real-proofs check in `cargo xtask lists`; branch stage1/p3-real-proofs)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head d34a629d

Reviewed head: `d34a629d9078aadcc63012adfe2bf47222c2a921`, base v1 `3000ae14` (the current v1). Files: `xtask/src/real_proofs.rs`
(new), `xtask/src/lists.rs` (+21), `xtask/src/main.rs`, `conformance/real-proofs.toml` (no entry) and `.cargo/mutants.toml`
(one exclusion). The stated tier is HIGH by rules 1 and 3, which is correct.

### Checked, no finding

- **The four v1 merges** (`52877811`, `1dbc5cba`, `15a362ae`, `d34a629d`): each tree is the tree of `git merge-tree
  --write-tree` of its parents.
- **The rule.** `running` is the ledger without the pending, deferred and withdrawn ids. Each running id whose
  replacement-map proof starts with `slow:` must have an entry. An entry must name a ledger id with a `slow:*` proof, once.
  `parse` refuses unknown keys and missing fields.
- **The exclusion** (`lists.rs command -> Ok(())`): the body reads files, and its decisions (`check`, `running`, `verdict`,
  `parse`, `proofs_of_map`) have their own tests.
- **The gate log** (`…-p3-real-proofs-d34a629d-pool-20261009-100347-51753.log`) names the head and base `3000ae14`. The
  default tier runs 958 tests and the slow tier 243, all pass. Mutants: 30 caught, 0 missed, 1 unviable. Exit 0.

### R1 MEDIUM — a named test is "defined" when its name appears as `fn <name>(` in any file of the crate's `tests/`

`verdict` (`real_proofs.rs`, the `defined` test) takes the last `::` segment of `test`, and it accepts the entry if any
tracked file under `<crate>/tests/` contains `fn <name>(`. Four false passes follow, and each lets a real-only id leave
pending with no running real test. That is the case this check exists to refuse:
1. **Another binary's file.** The function is defined only in `tests/fast.rs` (a default-tier binary), and the entry names
   `slow_real`. The module's own test data has this shape: `TEST` is in `crates/c/tests/fast.rs` and in
   `crates/c/tests/common/mod.rs`.
2. **Not a test.** A helper `fn <name>(` with no `#[test]`, or a test with `#[ignore]`, which nextest does not run in the
   gate.
3. **The wrong module.** `test` = `a::name` is accepted when `name` exists only in module `b`, or in a file that the
   binary does not include.
4. **A comment or a string.** `// fn <name>(` matches.

Plan revision 23a and the lead's addition make this check the only guard of the real-only exception for a STANDARD
removal-only pending change. So a false pass here reaches v1 with no integration review. Fix: resolve the binary's own
module tree (its root `tests/<binary>.rs` or `tests/<binary>/main.rs`, and the `mod` files that it declares) with `syn`
(already an xtask dependency, and #181's process-check does the same walk). Then require a function at exactly the module
path of `test`, with `#[test]` and with no `#[ignore]`. Add a test for each of the four cases. If the lead prefers to wait for
P6's `mutants_cited` (#181), note that #181's B6 is about the same weakness (a removed `#[test]`) and is still open.

VERDICT: NOT CLEAN at d34a629d9078aadcc63012adfe2bf47222c2a921 (1 open: R1 MEDIUM)

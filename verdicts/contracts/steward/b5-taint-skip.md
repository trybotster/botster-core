# contracts #98 — steward/b5-taint-skip (backlog B-5 step 1: `taint` skips `amendments/` and `verdicts/`), reviewer 017f

HIGH (gate-decision code). Author: the steward. Package reviewer: 016c. The PR's parent is main `11b2a5a840b9e25c8510d304bdf92e38dd5be70f`. Main is `216ea74e`, one rulings-only commit later (the R-115 correction). That is why a diff against `216ea74e` shows six lines of `docs/steward-rulings.md` removed. The commit itself changes only `xtask/src/taint.rs`.

## Round 1: NOT CLEAN on d558cf8e8b76fb28633e785ebde212f97a823175 (1 finding: the gate fails at `cargo fmt`)

- **The change.** `SKIPPED_PREFIXES` gains `amendments/` and `verdicts/`. The skip test moves into `skipped()`, and a unit test covers skipped and scanned paths (including `xtask/amendments/x.rs`, which a prefix must not skip).
- **The steward's claim: no other tree walker reads `amendments/` or `verdicts/`.** I checked every file walk in `xtask/`, `ci/` and the crates. **The claim holds:**
  - `fsutil::tracked_files` (the whole tree) has two callers. One is `taint` (this PR). The other is `timers`, which scans only `.rs`, `.ts`, `.tsx`, `.js` and `.mjs` (`SCANNED_EXTENSIONS`). `git ls-files amendments verdicts` gives 90 files, all `.md`.
  - `fsutil::files_under` callers read fixed directories:
    - `ledger.rs`: `conformance/` and the two vector dirs under `crates/`;
    - `real_only.rs` and `replacement_map.rs`: `conformance/`;
    - `clause_lint.rs`: `crates/`.
  - Other readers:
    - `crate_symbols.rs` reads `crate-symbols/`, plus the files that `contracts.toml` lists. `contracts.toml` lists no `amendments/` or `verdicts/` path. The `verdicts/…` keys in some tables are `n/a` rows, not file reads.
    - `manifest.rs` reads the configured frozen directory.
    - `ledger.rs:135` reads the `contracts.toml` directories.
  - `read_dir` in the crates' build scripts and tests reads only their own `conformance/`, `src/`, `schemas/` or `vectors/` dirs.
  - No file under `ci/` and no TS script names these paths.
  - So after this PR, no gate step reads the contents of `amendments/` or `verdicts/`. The `frozen` job reads only `frozen/`. The `mutants` job takes the diff, but it mutates only Rust code.
  - Step 2 stands on this, under the orchestrator's conditions: every `amendments/` line is an `A`, and the exception ends if anything starts to read these paths. A `.rs`/`.ts`/`.js` file added under `verdicts/` or `amendments/` would also be read by `timers`. Step 2's BUILD.md text should name that case too.

### F1 (MEDIUM): the gate on the exact head is red at `cargo fmt --all --check`

`~/botster-sessions/gates/botster-contracts-steward-b5-taint-skip-d558cf8e-pool-20261012-123556-19073.log` (msa1, base main `216ea74e`): `rust` FAIL after 0.5 s, so `slow`, `codegen`, `ts`, `fuzz` and `mutants` did not run. rustfmt wants the first assertion of the new test (`xtask/src/taint.rs:144`) wrapped:

```
-        assert!(skipped("amendments/core-contract-v1.17-amendment-24-candidate2.md"));
+        assert!(skipped(
+            "amendments/core-contract-v1.17-amendment-24-candidate2.md"
+        ));
```

Fix: run `cargo fmt --all` and commit. Then gate the new head. No other change is needed.

## Final: CLEAN on b65e0f6ba8fe7d6338a800fddc4d6410097d5d3e (F1 closed)

- **The new commit.** `b65e0f6b` ("xtask taint: rustfmt the new test", parent `d558cf8e`) changes only `xtask/src/taint.rs:144`. It is exactly the wrap that F1 quotes. No other line changes.
- **F1: closed.** The pool gate on the exact head is green: `~/botster-sessions/gates/botster-contracts-steward-b5-taint-skip-b65e0f6b-pool-20261012-123735-30871.log`. My `shasum -a 256` of it is `37b274d3cf966b589f931bfebfd26e001dcba32ecc72fc1ed8ab19a8bcaa4ed3`. All ten steps PASS, exit 0 after 834 s on msa1, base main `216ea74e`. The two new tests pass: `taint::tests::amendment_candidates_and_verdicts_are_not_scanned` and `taint_skips_frozen_and_docs_and_flags_other_tracked_files`.
- **Main** is now `3d590705`. Between `216ea74e` and it are the #97 merge (three `st_6b` transcripts) and one rulings commit. That is not a rulings-only move, so main cannot fast-forward to `b65e0f6b`. The paths are disjoint from this PR (`xtask/src/taint.rs`). The steward makes a base merge with its own gate, and I send a Q7 receipt on that merge head.
- **The note on step 2** (name a `.rs`/`.ts`/`.js` file under `amendments/` or `verdicts/` in BUILD.md) stays a note. It is not a finding of this PR.

My finding F1 is closed. CLEAN on `b65e0f6ba8fe7d6338a800fddc4d6410097d5d3e`.

## Q7 receipt: base merge 1b922004a90ea987687f418e8a02709b71eade62

- **The parents** are main `3d590705` and the reviewed head `b65e0f6b`. Tree `8b0be14a` is equal to `git merge-tree --write-tree` of the parents.
- **The diff from main** is `xtask/src/taint.rs` only. Its stable patch-id `e60e15ba` is equal to the patch-id of the reviewed head against its base `11b2a5a8`.
- **The paths are disjoint.** Main since `11b2a5a8` changed only the three `st_6b` transcripts and `docs/steward-rulings.md`.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-detached-1b922004-1b922004-pool-20261012-125241-89227.log`, with sha256 `d14844ea25c8a3c8834e5608306211630f0065e13003bdf0492b53b66f0f2e2f`. All ten steps PASS, exit 0 after 444 s on msa1, base main `3d590705`.
- **Main since then** is `e5709067`. `3d590705..e5709067` changes only `docs/steward-rulings.md`, so the steward's rulings-only merge on top of it needs no new gate.

Q7 receipt given for `1b922004a90ea987687f418e8a02709b71eade62`.

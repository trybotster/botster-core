# contracts #92 — stage0/final42-crate (Core A25: `max_session_id_bytes` has a ceiling of 1,024), reviewer 017f

HIGH: a change to the frozen-crate checks of `botster-core-contract` and a new Core transcript. Base: `steward/final42` `8379a5695e2cfa3da8f5c38cd93c3d938ef0606f` (final42 = final41 + Core A23 c2 + Core A25 c1, staging). Implementer: 016b. Package reviewer: 016c. The steward's ruling: this PR merges into `steward/final42` (staging only) with a gate that is red only on three A23 items, and every other job passes. P6's A23 PR builds on it and must be fully green.

## Round 1: NOT CLEAN on 9075126660a24491624b1df33f6707b54d31e2bf (1 LOW, evidence only)

Checked against A25 candidate 1 (`frozen/current/core-contract-v1.17-amendment-25-candidate1.md`, "The crate change"):
- **`limits.rs` `check_ranges`:** `in_range("max_session_id_bytes", self.max_session_id_bytes.into(), 1, 1024)`, as required. The field doc gives "Range 1 to 1,024." and cites Core A25-1.
- **The tests:**
  - `the_session_id_limit_has_a_ceiling_of_1024` accepts 1,024, refuses 1,025 with `field == "max_session_id_bytes"`, and keeps the default 128;
  - `every_stated_range_is_enforced_at_both_ends` has the row `("max_session_id_bytes", 1, 1024)`.
- **The schemas:** only the `description` of the field changes, in `core-limits.schema.json` and `open-config.schema.json`, by codegen. No TS package has Core limits.
- **The transcript** `conf::a25_1_session_id_limit_above_1024_is_invalid_config` has the form of `conf::lm_1_zero_limit_is_invalid_config`, as A25 requires: `setup.open: false`; `open` with 1,025 gives `InvalidConfig {field: {$type: string}}`; `open` with 1,024 succeeds (the same pair as `lm_1`'s last two steps for `max_key_repeat`).
- **FakeCore:** `FakeCore::open` calls `CoreLimits::validate()` (`botster-fake-core/src/lib.rs:80`). So FakeCore runs the id with no code change, and the id is not structural. This is the same as `lm_1`. The README lists only structural ids, so no README row is due. The proof in `replacement-map.json` is `core-testkit`, as for `lm_1`.
- **Disjoint from A23:** the diff has no A23 path.
- **The gate** (`~/botster-sessions/gates/botster-contracts-stage0-final42-crate-90751266-pool-20261012-110047-68133.log`, msa1, `cargo xtask ci --keep-going`, base main `32d36828`): exit 1. `ledger` FAIL with one problem, the A23 item 1 (`conf::ou_7_…: no transcript`). manifest, frozen, taint, rust, slow, codegen, ts, fuzz and mutants (1 tested, 1 caught) PASS.

### F1 (LOW): the gate does not run the rest of the `ledger` job, so `clause-lint`, `crate-symbols` and `replacement-map` have no gate evidence on this head

- `xtask/src/ci.rs` `ledger_job` runs, in order: `ledger --check --base <base>`, then `clause_lint`, then `crate-symbols --check --base <base>`, then `replacement-map --check`. Each `?` ends the job at its first error.
- So on this head the gate stops at the A23 item 1. It runs none of the three later checks. `--keep-going` runs the later jobs, not the later checks in one job.
- The steward's condition is that the gate is red only on the three A23 items. Items 2 and 3 are in the later checks. So the gate shows item 1 only, and it does not show that the later checks find nothing else. In particular:
  - `clause-lint` is not one of the three items, and nothing proves it on this head;
  - the new A25 table (`core-contract-v1.17-amendment-25-candidate1.toml`) is proven only by a local run;
  - the local `crate-symbols` run used `--base origin/steward/final42`, but the gate's base is main `32d36828`.
- The body gives the local results, but no log, so I cannot check them.

Fix: run one pool evidence job on this exact head, with no code change. The job runs the three later checks with the gate's base, and each check continues after a failure: `cargo xtask clause-lint`; `cargo xtask crate-symbols --check --base origin/main`; `cargo xtask replacement-map --check`. The log must show that `clause-lint` passes, and that the other two report only A23 items 3 and 2. Put the log path in the body. The head stays the same, so the gate need not run again.

016c's package review is in progress, and I do not review its scope.

## Final: CLEAN on 9075126660a24491624b1df33f6707b54d31e2bf (F1 closed by #93's gate)

- **F1: closed, with no new job.** P6's A23 PR #93 (`4e6f725f761b8ca74300658a1543f333cabd2ab9`) has this head as its parent and adds one commit. Its gate is fully green and runs the whole `ledger` job, with the same base, main `32d36828`: "clause-lint: 448 citation(s) checked", "crate-symbols: 59 table(s), 19428 symbol(s) resolved; PENDING checked against 32d36828…", and "replacement-map: 2550 ids … docs/fake-replacement-map.md up to date" (`~/botster-sessions/gates/botster-contracts-p6-a23-ou7-tail-on-a25-4e6f725f-pool-20261012-110146-72292.log`).
- `git diff --name-only 9075126 4e6f725f` gives the A23 transcript, the A23 crate-symbols table, one added line each in `README.md` and `conformance/replacement-map.json`, and the regenerated `docs/fake-replacement-map.md`. None changes an A25 line. So, on this head:
  - `clause-lint` passes (the A23 commit removes no citation);
  - `crate-symbols` resolves the A25 table, and the only problem is the A23 table (item 3);
  - `replacement-map` lacks only the `ou_7` id, and so its generated document is stale (item 2).
  These are exactly the steward's three items.
- The merge into `steward/final42` is staging only, and the branch then takes #93, whose head is fully green.

016c's package review: CLEAN on A25's source and its retained exception evidence (016c's message).

My finding F1 is closed. CLEAN on `9075126660a24491624b1df33f6707b54d31e2bf`, for the staging merge under the steward's ruling.

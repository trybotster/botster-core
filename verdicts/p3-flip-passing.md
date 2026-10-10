# P3 pending-list activation review

## PR #215 — Round 1 — 2026-10-09

Reviewed head: `b661eb4f43bf6559d38131e48d860bb895abbf91`.
Tree: `16295f2f7ff95d105d5d388a7c6dcf1c7ac8c521`.
Parent: `42cdfda92cea812366c84e3ade21d3ba2abc474a`.
PR base, merge base, and gate base: `6ff4be7f6e064d0d4abcb524219a048b9ddc6e57`.
Contracts: `contracts-v0.1.23` at `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`.

The reviewer checked the STANDARD tier first.
The sole changed path is `conformance/core-pending.txt`. The PR changes no code, shared crate, workspace configuration, pin, or HIGH path.
The lead assigned P5 the complete list review. The lead withdrew the earlier integration review assignment for this STANDARD PR.
Astra supplied that explicit withdrawal in message `msg_plugin-w_1791602698_4e878b`.
No integration verdict is required. No finding remains open.

### Complete list and evidence checks

The reviewer read the complete diff, full corrected PR body, proof classifications, relevant static checks, and supplied gate logs.
The pending ID set changes from 545 to 487. Exactly 58 IDs leave; no ID enters and no duplicate exists.
The ledger, deferred files, withdrawn file, contracts pin, transcripts, and test budgets stay unchanged.
Every removed ID has a tagged transcript and is a Core ledger ID.
Every removed ID has an exact-head PASS line. The existing runner accepts only `Outcome::Passed` over the selected seeds.
The default gate environment sets `BOTSTER_SEEDS=0-31`.

Owner counts match the approved plan 23p lists:

- P1 lifecycle: 29.
- P3 worker: eight.
- P4a routes: 12.
- P4b queries and files: one.
- P5 adoption: four, including the two minimum IDs.
- P6 testkit: four.

The authoritative `conformance/replacement-map.json` classifies the 58 removed IDs as follows:

- `core-testkit`: 44.
- `core-testkit+edge:clock`: five.
- `core-testkit+edge:program-backend`: one.
- `core-testkit+edge:storage`: one.
- `core-testkit+edge:wake`: one.
- `core-testkit+perturb`: four.
- `static`: two.

No removed ID requires a slow or real-only proof. No removed ID is deferred or withdrawn.
The `a5_1_not_in_facade_or_ffi` transcript calls `check_crates` through TestkitHarness over the actual workspace.
The existing check reads the public API snapshot, facade exports, and workspace dependency graph. The gate checks public API drift.
The `a6_2_deferred_set_is_exactly_the_enumerated_ids_with_start_conditions` transcript calls the tagged driver's `a6_2_deferred_set(DEFERRED_TXT)`.
That input is tagged contracts `conformance/deferred.txt`.
The separate gate list check validates Core's `core-deferred.toml` against that tagged file and checks the start conditions.
Both static transcripts have PASS lines at this head.

The reviewer read the two minimum transcripts: `lc_12_drop_leaves_workers_running` and `ou_9_adopted_session_attaches_by_baseline`.
The transcripts check the running worker after host replacement and the baseline and subsequent output after adoption.
Both use the permitted testkit proof. Both have exact-head PASS lines.
The minimum list remains 69 IDs. Testkit progress increases from 44 / 69 to 46 / 69.
All 46 active minimum IDs and all 182 active conformance IDs have exact-head PASS lines.
This verdict makes no real-harness progress claim.

`lc_2_data_dir_is_exclusive` stays pending because its proof is `slow:data-dir-lock`.
`ev_8_every_query_stays_in_output` stays pending because the earlier gate terminated it at 2.005 seconds.
The earlier gate failed the unchanged two-second default test budget.
The new comment records that failure and assigns the speed or tier fix to P4b.
The obsolete a2_8 note is removed under plan 23a: a permitted testkit pass is sufficient for this list activation.
Its later real-tier proof remains required for Stage 1 acceptance.

### Body findings — LOW — CLOSED within Round 1

F215-1: the original body lacked the required Prior art note. P3 added the existing list, runner, classification, and budget sources.
F215-2: the original STANDARD tier cited rule 2, which defines unsafe/FFI/C ABI changes. P3 corrected the citation without changing the tier.
F215-3: the original body used stale generated-map counts and incorrectly said no candidate was static.
P3 corrected the counts from replacement-map.json and cited the two static proofs.
P3 also corrected the driver's input: tagged deferred.txt, with Core's TOML checked separately by the gate.
The reviewer verified the full corrected body and unchanged exact head.
All findings closed in this round. No STANDARD LOW follow-up issue is needed.

### Supplied gate

Log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-passing-minimum-b661eb4f-pool-20261009-202023-14533.log`.
The log names the reviewed head and stated base. The head contains that base.
All ten stages report PASS. The job and gate exit zero on msa1.

- Default: 1390 passed, 507 skipped. The slowest default test took 0.970 seconds.
- Slow: 256 passed, 1297 skipped.
- Conformance: 182 passed, zero failed, 417 pending with transcripts, 70 pending without transcripts, two deferred, 18 withdrawn.
- Both mutation steps: zero mutants because the diff changes no Rust source. Cargo-mutants does not run, under the landed #213 rule.
- Fuzz: no changed crate with a decoder harness.

Earlier failed log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-passing-minimum-42cdfda9-pool-20261009-201607-99150.log`.
The reviewer checked its 2.005-second TIMEOUT and budget failure for the retained ev_8 ID.
The submitted head fixes the selection; it does not weaken the budget or repeat the unchanged failing head.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN

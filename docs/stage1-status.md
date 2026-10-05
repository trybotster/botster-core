# Stage 1 Core status (paused 2026-10-05, pause #3)

The user paused Botster v1 again on 2026-10-05, to save token budget. Every Stage 1 Core agent finished its current atomic step, pushed its work and wrote a handoff.

A one-round wind-down was allowed for near-done PRs. Its merge chain stopped, because the shared test guard (#165) needed a second fix round. **Nothing merged during the wind-down. Nothing unaccepted was merged.**

This file lets a NEW lead and new pairs resume without asking. The per-package detail is in `docs/handoffs/`, and the lead's full handoff is `docs/handoffs/core-lead.md`.

## 1. Read first

1. `~/Projects/botster-contracts/docs/BUILD.md` (binding), including the "Ghostty fork policy".
2. `docs/handoffs/core-lead.md`: the rules, the rulings in force (section 8 and the later "Ruling" lines), the staffing, and the resume steps.
3. The plan: `docs/stage1-plan.md` on branch `stage1/plan`.
   - Revision 21 = `ad03636ffbd68048f9f07faf176986f004d5e516`.
   - Read-only pin: `~/botster-sessions/pins/stage1-plan.bdda2359.md` (sha256 `bdda23593c29e3cc654acf0a5a98eee43f218b119ff2cd530cbd31d6b2cff9ac`).
   - **Revision 22 is due on resume** (section 9).
4. The per-package handoffs in `docs/handoffs/` (P3, P5, P6, P7, the A6 fork, the integration reviewer, the package reviewers).
5. The state log: `~/botster-sessions/botster-v1-orchestrator-state.md`.
6. The audit: `audits/v1-pr134-141.md` on branch `stage1/audit-134-141` (commit `02acc4b`), and GitHub issues #143 to #160 (label `stage1-audit`).

## 2. Pins

| What | State |
|---|---|
| Contracts in `v1` | still `contracts-v0.1.13` (`a8db5c9`). |
| Contracts target | `contracts-v0.1.17` = `1725abf` (manifest final35: Core A14, A15, A16; steward rulings R-30 to R-34; R-33 is on contracts main `14c86ab`). PR #161 moves the pin. |
| Ghostty in `v1` | `trybotster/ghostty` `botster/upstream-sync-20261002` @ `3f8eb6810bb673aa782b047de21783ac81fb1121`, on upstream `f523504ea`. |
| Ghostty candidate (A6, R-32, A14) | `botster/upstream-sync-20261004` @ `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`, on upstream `5dc28bb8e`. NOT recorded; under review (section 6). |
| Zig | 0.16.0, 7 packages |
| Rust | 1.97.0 (`env -u RUSTUP_TOOLCHAIN` for local cargo) |

## 3. Merged in `v1` since pause #1 (`v1` head `144b0234fb632bcbb5176b17c2fe55f3239405df`)

| PR | Merge | Content |
|---|---|---|
| #132 | `f96a9df` | Pause #1 status and handoffs |
| #133 | `e43225f` | `xtask lists`: shared withdrawn and deferred ids are checked against `ledger.json`; Core applies only Core ids |
| #134 | `38e6989` | P2 libghostty binding (Ghostty pin `3f8eb68`; P32 Linux allocator fix proven; P38 macOS static link) |
| #135 | `1d25d09` | P1 registry and lifecycle (milestone **M4**) |
| #136 | `01fd389` | P3 M1 worker and PTY, plus P1's testkit wiring (milestone **M1**) |
| #137 | `393f403` | P1 TI-1: Core terminal identity from the binding |
| #138 | `38bbe58` | Binding read and restore APIs for the oracle controls |
| #139 | `a3b8af5` | The GHOSTSNP format spec (the "Worker paging" section stays pending for P3 M2b) |
| #140 | `e8cf150` | P6 oracle controls and process cleanup tests |
| #141 | `144b023` | Testkit per-module control registry |

Each merge: the package reviewer (plus the integration reviewer where cross-package) was CLEAN on the exact head, a gate was green on that head, and the merge tree equals the gated tree.

## 4. Open PRs and branches, with the merge order on resume

Each PR, in this order: merge `origin/v1` into it, get delta CLEANs (the package reviewer, plus the integration reviewer if cross-package), run ONE gate on that exact head (Linux preferred), then merge. Never re-gate a red head to check for flakiness (BUILD testing rule 9).

| # | PR | Branch @ pushed head | Owner | State |
|---|---|---|---|---|
| 1 | #165 → folded into #162 | `stage1/p3-guard-macos` @ `71195e72` | P3 | Fixes the macOS regression of the shared test guard (`process_guard::parent_dies_before_fifo_reader`). P5-F4 is closed (integration `cef28d1`). **Open: C3 MEDIUM** (`an_early_exit_keeps_the_group_owned_until_cleanup` still uses `read_line` and `child.wait()` with no deadline) **and C4 LOW** (`while :; do /bin/sleep 1; done` children; BUILD rule 5). P3 has not yet reviewed `71195e72` at package level. |
| 1 | #162 | `stage1/p5-a10` @ `59cda32` | P5 | The A10 reaper-race fix, carrying #165 `c03bcfb`. C1 is closed. It gets the final #165 head merged in, then delta CLEANs (P5 reviewer plus integration), then ONE gate. **Its gate and #165's each fail on the other's defect, so they gate together.** |
| 2 | #163 | `stage1/p3-audit-fixes` @ `40b63dc` | P3 | Audit A3, A8, A11, A30, A31, A52, A53. It needs the F28 native Mac mutation run and the F33 execution; integration G2 is re-checked in the merge delta. |
| 3 | #164 | `stage1/p5-audit-contract` @ `b8b37a6` | P5 | Audit A1, A2, A4, A5, A7, A9: chunked base32 registry paths and the `data_dir` boundary. **CLEAN** (P5 `89893b2` and `b91cd55`; integration `9edbc27`). |
| 4 | #142 | `stage1/p7-services` @ `6e8d5b3` | P7 | The sans-IO Guardian machine. **CLEAN** (P7 `ac6e612`). It lands AS REVIEWED: merge `v1` only. The wire fixes on `stage1/p7-services-wire` @ `39bdc39` are a separate follow-up PR. |
| 5 | #161 | `stage1/contracts-v0.1.14` @ `dea90ed` | P6 | The pin move to `contracts-v0.1.17` (the a14, a15 and a16 ids added as pending with their owners). **CLEAN** (P6 `d1a3869`). |
| – | (no PR) | `stage1/p2-fork-a6` @ `65b2064`; the fork `0bfddc16` | A6 fork agent | Section 6. |
| – | (no PR) | `stage1/p3-m2a-v1` @ `cce8598` | P3 | M2a in progress; unreviewed. |
| – | (no PR) | `stage1/p5-audit-host-fixes` @ `b3b5fac` | P5 | Fixes for #155, #157 and the P1 part of #156; the findings are not yet checked one by one; unreviewed. (`stage1/p5-audit-host` @ `5308cb1` is history only.) |
| – | (no PR) | `stage1/p6-real-harness` @ `276427d` | P6 | RealCoreHarness with the anchor design; unreviewed. Its latest fixes are unverified. |
| – | (no PR) | `stage1/p7-services-wire` @ `39bdc39` | P7 | The PR 1 wire fixes; untested. PR 2a is a design draft only (`~/botster-sessions/shared/core-stage1/p7-pr2-host-services-design-draft.md`). |

## 5. Audit of PRs #134-#141 (issues #143-#160, all still open)

The audit found: CONTRACT 8, HIGH 4, MEDIUM 32, LOW 20. No reverts. The architecture is sound, and P35 holds. The weaknesses are systematic: mutant-only tests, wide exclusions, slow-test and process-ownership breaks, unbounded waits, and untested edges.

| Issue | Finding | Owner | State |
|---|---|---|---|
| #143 | A1 AdoptAll skips damaged rows | P5 | fixed in #164 (CLEAN) |
| #144 | A2 Create overwrites a durable row | P5 | fixed in #164 |
| #145 | A3 IN-9 bound / KeyInput.text | P3 | fixed in #163 (exact bound, early stop); A15 is now final: P3 worker check and P5 host check follow after #161 |
| #146 | A4 Stop async WrongState | P5 | fixed in #164 |
| #147 | A5 Core::open reads the clock | P5 | fixed in #164 |
| #148 | A6 OSC 5522 over the limit | A6 fork agent, then P3 | R-32, R-33, A14; the fork patch is in review (section 6) |
| #149 | A7 slow test polls | P5 | fixed in #164 |
| #150 | A8 worker fixture group ownership | P3 | fixed in #163 |
| #151 | A9 Remove of an adopted session | P5 | fixed in #164 |
| #152 | A10 reaper race | P5 | #162 (gated with #165) |
| #153 | A11 unbounded waits | P3 | #163; the guard part is in #165 (C3) |
| #154 | A12 every-cut cost | P6 | settled by R-31; implement in the oracle work |
| #155, #157 | host and facade MEDIUM/LOW | P5 | partly in #164; the rest follows |
| #156 | sys MEDIUM/LOW | P5 (storage), P3 (payload) | partly in #163 and #164 |
| #158 | worker MEDIUM/LOW | P3 | partly in #163; A36 goes to M2b; A32 and A33 are an approved follow-up PR (a slow in-diff mutants pass; plan section 8 revision after it) |
| #159 | testkit MEDIUM/LOW | P6 | open |
| #160 | binding MEDIUM/LOW | the A6 fork agent or P6 | open |

Close each issue when its fixing PR merges.

## 6. The A6 Ghostty fork task (user rule: nothing ever goes upstream)

User rule, verbatim: "nothing ever goes upstream. Push only to trybotster/ghostty; no PRs, issues, comments or any other contact with ghostty-org. Fetching upstream main is fine."

- Fork: `botster/upstream-sync-20261004` @ `0bfddc16`, on upstream `5dc28bb8e`.
- It contains the R-32 patch: the over-limit callback carries the decoded size, and libghostty sends no EFBIG.
- Under R-33, types past the 64-type limit are neither decoded nor counted.
- Core branch `stage1/p2-fork-a6` @ `65b2064`. The binding PR is not opened yet. It must set option 39 = `clipboard_bytes` (A14).
- Review: NOT CLEAN, with only F-A6-02 (HIGH, test evidence) open. It needs the final-head Zig tests, the library build, the binding tests on Mac and Linux, the empty-cache package check, the completed sync record and audit, and the PR's Prior-art note.
- Handoffs: `docs/handoffs/fork-a6.md` and `review-fork-a6.md`.

## 7. Contract changes since pause #1 (all final unless marked)

| Item | Effect on Core | Owner |
|---|---|---|
| A14 (final33): `clipboard_bytes` bounds the decoded size; option 39 = `clipboard_bytes` is required | fork and binding (option 39); worker two-step rule | A6 fork agent, P3 |
| A15 (final35): `max_key_text_bytes` (default 256, range 1-4,096) | host limit and admission (P5); worker (P3); route (P4a) | P5, P3, P4a |
| A16 (final34): `ServiceEnd` = `Exited` \| `Lost{reason, payload_may_remain}` | StopService | P7 |
| R-30, R-31: the every-cut fit check (exact oracle measurement; one session per item; no sampling) | oracle controls | P6 |
| R-32, R-33: OSC 5522 over the limit; ignored types not counted | fork patch | A6 fork agent |
| R-34 (user): the A15 numbers; the Web client encodes keys itself (restty) | — | — |

## 8. Agents at the pause

- **Idle, kept for their context:** the implementers P3, P5, P6, P7 and the A6 fork agent; the integration reviewer; and the package reviewers that owe a pending verdict (P3, P5, P6, P7, fork A6).
- **Staffing rule (user):** implementers are Claude Opus; package reviewers are Sol at high effort (`agent_name: sol-high`, definition `~/.botster-dev/agents/sol-high/initialization`); the integration reviewer is Opus.
- The session ids are in `docs/handoffs/core-lead.md`.

## 9. First actions on resume

1. **#165 round 2 (P3):** fix C3 (`first_line` plus `cleanup::Owned` in `an_early_exit_keeps_the_group_owned_until_cleanup`) and C4 (FIFO-blocked `/bin/cat` members instead of sleep loops). Then get CLEAN from P3's reviewer, the integration reviewer and P5's reviewer. Then P5 merges it into #162, gets the delta CLEANs, and runs ONE gate, Linux preferred (#165's Linux branches are still unproven). Then merge #162.
2. **The rest of the merge chain:** #163, #164, #142, #161 (section 4).
3. **Plan revision 22 (lead):**
   - the contracts pin `v0.1.17`, and the Ghostty pin after the A6 binding PR merges;
   - `owners.py` rules: A14 to P3, A15 split P5/P3/P4a, A16 to P7;
   - the slow mutants pass (after P3's A32 and A33 follow-up);
   - review by the integration reviewer.
4. **Prior-art notes (orchestrator):**
   - P3 does a prior-art pass before M2a: shpool, the tmux server/client and its per-pane key re-encoding, the zellij server, the wezterm mux, abduco/dtach. Record reuse or reject per item.
   - P5 and P7 add Prior-art notes too.
5. **Then the paused work:**
   - P3 M2a and M2b (milestone M2; GHOSTSNP Worker paging; A13, A14, A15 worker);
   - P5 adoption and the A15 host check;
   - P6 RealCoreHarness;
   - P7 PR 2a with A16;
   - the A6 fork binding PR;
   - the remaining audit issues.

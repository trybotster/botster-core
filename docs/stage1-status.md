# Stage 1 Core status (paused 2026-10-02)

The user paused Botster v1 on 2026-10-02 ("no rush, finish at a convenient clean point"). Every Stage 1 Core agent stopped at a clean point and pushed its work; nothing unaccepted was merged. This file lets a NEW lead and new pairs resume without asking. Per-package detail is in `docs/handoffs/`.

## 1. Read first

1. `~/Projects/botster-contracts/docs/BUILD.md` (binding; read all of it, including the "Ghostty fork policy" rule added at the pause).
2. The Stage 1 plan: `docs/stage1-plan.md` on branch `stage1/plan`, revision 20, commit `27ffaa4006c3f979fdceb897cbde09de6c25c15e`; read-only pin `~/botster-sessions/pins/stage1-plan.e4862c71.md` (sha256 `e4862c712743e0a4119fbfe138d314eb242a9dfdab1c641bbee7c53035e55b3e`); CLEAN by the integration reviewer (verdict `0b9b923` on `stage1/plan-review`). The clause ownership lists are `docs/stage1-clauses/*.txt` on the same branch (652 active Core ids).
3. The pair rules and briefs: `~/botster-sessions/shared/core-stage1/` (`pair-common.md`, `brief-*.md`, `handoff-p2-implementer.md`).
4. The state log: `~/botster-sessions/botster-v1-orchestrator-state.md` (every spawn, merge, ruling and decision, in order).
5. The handoffs in `docs/handoffs/` of this branch.

## 2. Pins in force at the pause

| What | Pin |
|---|---|
| Contracts (plan) | tag `contracts-v0.1.13` = `a8db5c9a0f43fc440564989fd38a55121fdda38b`, manifest final30 (Core A13 final). Packages pin the tag the lead names, one commit per move (P1 is on v0.1.9, P2 on v0.1.13, P3 on v0.1.9, P6/v1 on v0.1.9). |
| Ghostty (recorded in the plan) | `trybotster/ghostty` branch `botster/vt-core-stage1-b` @ `85a8d8eb197c5752887c017c9a3faa6f1dc1969b` on upstream `ghostty-org/ghostty` main `83edd491e3024ae5e50393d62877b8897da1cccd`. |
| Ghostty (proposed, NOT recorded) | branch `botster/vt-core-stage1-c` @ `ada251c5e99cdb753de8bf72d5d1f307d474518f` (22 commits on `83edd491`; `85a8d8e` is its ancestor). It waits for the P2 reviewer's CLEAN on patches 12 and 13 and the binding delta, and now also for the upstream resync (section 7, action 1). |
| Zig | 0.16.0 |
| Rust | 1.97.0 (`rust-toolchain.toml`); the agent environment sets `RUSTUP_TOOLCHAIN=1.92.0`, so local cargo commands run with `env -u RUSTUP_TOOLCHAIN`; the pinned nightly `nightly-2026-09-30` is in the Linux gate image and installed on the Mac. |

## 3. What landed in `v1` (head `41ebcc0`)

| PR | Merge | Head | Content |
|---|---|---|---|
| #125 | `118c972` | `47759ee` | P0 skeleton (M0): workspace, edges and machine traits, link framing and hello, flock, facade shell, lints, the libtest-mimic conformance harness, `cargo xtask ci` |
| #126 | `7c0ae16` | `9846f9e` | P6 testkit M0b: Sim, seeded scheduler and Entropy, in-memory Link and RouteTransport, scripted program edge (botster-probe-script), TestkitHarness |
| #127 | `ccb04eb` | `4ab9a94` | infra: the gate on the LAN Linux test host (`ci/remote/`) |
| #128 | `06f1f04` | `96140c9` | P6 scope 2 step 1: contracts' deferred/withdrawn files as the source, five report counts, base ref resolved once, verify-only nightly |
| #129 | `67124ec` | `744da57` | P6 scope 2 partial: RefusalScript (plan 4.2a, R-19), edge controls, RealCoreHarness scaffolding, pin contracts-v0.1.9 |
| #130 | `5f15db8` | `2e631a7` | infra: bounded target volumes, early fail below 40 GiB host disk |
| #131 | `41ebcc0` | `5f1201e` | infra: `ci/remote/fetch-public.sh` (libghostty submodule and Zig packages for the offline gate), `/tmp` tmpfs |

Every merge had the package reviewer's CLEAN on the exact head and a green gate on that head; each merge tree equals its gated head.

## 4. Package state

| Pkg | Branch @ pushed head | Done | Left | Last verdict / open findings |
|---|---|---|---|---|
| P0 skeleton | merged (#125) | all | — | pair retired |
| P1 lifecycle | `stage1/p1-lifecycle` @ `ff376cceea6f68db75ca76ebbe1a574ea3ece5fd`; `stage1/p1-testkit-wiring` @ `95a58545a9d5369098ed0074ee150d1b6ca16491` (SimEdges wiring, lands with P3) | HostEngine, registry and Storage edge, facade, R-15/R-16/R-19/R-20, the F7 worker-control signal interface, mutation 1161 → all closed | the two Resize defects P3 found (handoff), the delta review since `2016886`, the fsync-test move to the slow tier, one green gate, then merge; then the testkit-wiring PR with P3 (needs the integration reviewer) | reviewer CLEAN on `2016886` (`stage1/review-p1` `44f5590`); delta since then unreviewed; F14 (TI-1 terminfo) deferred to P2's merge; a Linux gate was still running on `ff376cc` at the pause |
| P2 libghostty | `stage1/p2-libghostty` @ `f711184885f933723c64124a1eaaa3bbc547b51f`; fork `botster/vt-core-stage1-c` @ `ada251c` | audit (CLEAN), fork patches 0-13, binding crate (93 tests, macOS) | the upstream resync (section 7), review of patches 12-13 and the binding delta, the pin record, the Linux gate (P32) | last published verdict `e26e146` (`stage1/review-p2`) at binding head `ab8577a`; heads `52d1f73` and `ada251c` examined, no new verdict; open findings (incl. P34's native image test, patches 12-13 review, P32 gate order) in the handoff |
| P3 worker | `stage1/p3-worker-m1-stack` @ `46b16945` (CLEAN `30bb483`; on P1 `823a1f1`+`befe0ff`); `stage1/p3-worker-m2a` @ `98960e43` (CLEAN `5eb1f13`); `stage1/p3-worker-m2b` @ `7abc54db` (not reviewed); `stage1/p3-worker` @ `306b143a` | M1 and M2a CLEAN; 78 P3 ids pass on 32 seeds | M2b (capture/baseline, ReadFacts, tap, disable_history), the 11 A13 ids, the pin move to v0.1.13, rebase after P1 merges, then gate | no open findings (`stage1/review-p3` `5eb1f13`) |
| P4a, P4b, P4c routes | not started | — | everything; 16 P4a ids still lack a contract boundary (botster-contracts `docs/core-p4a-2-notes.md`) | — |
| P5 adoption | not started | — | everything | — |
| P6 testkit | `stage1/p6-testkit` @ `744da57` (= v1 via #129) | M0b, step 1, partial scope 2 | controls needing P1/P3 identity, terminal-oracle controls after P2, RealCoreHarness integration, its 15 ids | CLEAN `2793fd6`, no open findings |
| P7 services | not started | — | everything | — |
| P8 example | not started | — | everything | — |

## 5. The P2 fork and the Linux defect

- Patch list (0-13, each with its clause) and the pin status: `docs/handoffs/p2-libghostty.md`.
- The `-c` stack (`ada251c`) still contains the "notification source" (`7afa387`) and "paste marker frame" (`970a1c9`) patches: they did NOT leave the stack. `85a8d8e` (`-b`) is an ancestor of `ada251c`; `-c` adds patches 9-13 on top.
- **Linux calloc/free defect (P32):** the Zig-built `libghostty-vt` archive defined its own `calloc` and `free` (wuffs calls `calloc`; without libc, Zig compiles its own allocator) while `malloc`/`realloc` stayed glibc's, so every binding test aborted on Linux. Patch 10 links libc on Linux; `tests_archive.rs` fails on any libc allocator symbol in the archive. Not yet proven by a Linux gate. Closure rule (lead): the reviewer may CLEAN the source logic with P32 pending; the green Linux gate on the exact head closes it.
- The offline Linux gate fetches the submodule at the pinned gitlink and the Zig packages through `ci/remote/fetch-public.sh`. Verify the Zig package list of `build_data.rs` at the new fork head before the gate (P2 counts 7 at `85a8d8e`; the infra engineer reported 9).

## 6. Stacking and milestones

- **Stacking:** P3's M1 stacks on P1 (`823a1f1` + `befe0ff`), M2a on P1 `3512c68`, M2b on M2a + P2 `977d986` + P1 `21adfb2`. P1's testkit wiring (`stage1/p1-testkit-wiring`) lands together with P3's worker. Merge order: P1 → (P2) → P3 (rebased onto the new v1, delta review, one gate) → P1's testkit wiring with P3 (integration reviewer required).
- **Milestones (plan 6.2):** M0 done (#125); M0b done (#126); M1 CLEAN, waits for P1's merge; M2 (M2a CLEAN, M2b in progress); M3 (P4a) not started; M4 (P1) waits for P1's merge.

## 7. First three actions on resume

1. **P2, the Ghostty upstream resync (user rule, BUILD.md "Ghostty fork policy"):** before ANY further fork patch, rebase the `botster/vt-core-stage1-c` stack (22 commits on upstream `83edd491e`) onto the LATEST upstream `ghostty-org/ghostty` main. Rebuild, re-run the libghostty audit and the binding and Core tests, drop every patch that upstream now covers, and record the new upstream SHA and the surviving patch list. Then the P2 reviewer reviews the rebased stack and the binding delta, the lead records the pin move in the plan, and P2 gates once on Linux (closing P32).
2. **P1:** fix the two Resize defects P3 found, get the delta review since `2016886`, move the real-fsync tests to the slow tier, gate once, merge (milestone M4). Then P3 rebases M1 and M2a onto the new v1, gets the delta review, gates and merges (milestone M1), with P1's testkit wiring in the same step (integration reviewer).
3. **Lead:** restaff by the plan's waves (6.3) from the handoffs, with the spawn rule of BUILD.md; P4a starts after M2, P5 and P7 after M4. Move every package to the current contracts tag first.

## 8. Agents at the pause

All Stage 1 Core agents were retired at the pause after their work was confirmed pushed (see the state log for ids). The integration reviewer's role (Sol, long-lived) restarts with the stage: it reviews the plan revisions and every cross-package PR.

# Phase 0 / Stage 0 status

Owner: the Stage 0 (Foundation) lead. (Updated 2026-10-04 while paused: manifests final32-35 mapped on main; ledger 2,503 ids.) **PAUSED 2026-10-02 ~14:50 by the user** (via the orchestrator). Written by Stage 0 lead 2 (Claude Code session `93395ec1-822a-4220-9fff-795f1eb01140`, Botster session `sess-1790906371-009d-f607653bc23d9de48c8a47464787a6ba`, worktree `~/botster-sessions/trybotster-botster-contracts-build-stage0-lead-2`). Everything below lets a NEW agent resume without asking anything. All pairs are retired; every in-flight branch is pushed with a standalone handoff in `docs/handoffs/<feature>.md`.

## 1. Read first

1. `docs/BUILD.md` (binding; "How each stage runs": automated merge, `botster-gate`, the spawn rule (load < ~12 OR CPU idle >= 50 % with at most one Botster job), context protection, agent cleanup, prompt-free git).
2. This file, then the six live handoffs (section 4).
3. `docs/steward-rulings.md` (R-1 to R-29) and `frozen/current/MANIFEST.md` (manifest **final31**).
4. The lead's working files in `~/botster-sessions/shared/foundation/`: `pair-common.md`, `role-implementer.md`, `role-reviewer.md` (rules 1-8), `lead-notes.md`, briefs `brief-*.md`, id lists `ids-*.txt`. Pins (read-only, sha-named) are in `~/botster-sessions/pins/`.
5. The running log: `~/botster-sessions/botster-v1-orchestrator-state.md`.

## 2. What landed on main since 60341ef (contracts-v0.1.0)

Tags `contracts-v0.1.1` .. `contracts-v0.1.13` (latest `contracts-v0.1.13` = a8db5c9); main is now past it by manifest data (final31, a209d40) and the Lua half of lb-cli-suite.

| Feature | What |
|---|---|
| hp-calls-a | 145 HP host-call transcripts |
| probe-script, probe-fix | `botster-probe-script` crate (non-fake); the probe runs steps in order (per-process SIGTERM ignore via a safe handler, SA_RESTART; direct signal_self) |
| core-runner, core-p1, remove-report-fix, core-a8, core-e3, core-p3, core-p4a-dp, core-p4a-ou, core-p4a-2, core-p4b, core-p4c, core-p5, core-p7 | Core transcripts for Stage 1 P0-P7 (structural, proof core-testkit / +edge / slow), the shared Core runner and testkit controls (`docs/core-testkit-controls.md`), `RemoveReport` (A6-3) and `AttachRefused` (R-19) in botster-core-contract |
| await-recheck | every `await_*` runner step re-checks its condition after each pump before deciding Idle |
| hp-events, hp-plugins (2 batches) | HP sections 9.8, 9.11-9.14, 10, 11 |
| hc-runner | the HC runner with the real-subject `HubSubject` trait (`docs/hub-subject-controls.md`), the client-only player, 60 HC + 6 UI transcripts; FakeHub dropped |
| lb-cli-suite part 1 | 54 Lua-binding ids (`LuaHostSubject`) |
| ledger-new-ids, ledger-status, withdraw-authority | pending may gain ids new to the ledger; four id states (transcript, pending, `conformance/deferred.txt`, `conformance/withdrawn.txt`); a clause row with no test id is an authority |
| mutants-jobs1, mutants-features, xtask-inproc, conformance-speed | cargo-mutants `-j 1` with the probe in its own run; xtask is a library and its tests run in-process; the default tier is ~11-14 s (was ~60 s) |
| Manifests final14 .. final31 | Core errata 2-5, Core A7-A13, HP A3-A8, Lua binding r9 A1-A2; every new id is mapped in `conformance/replacement-map.json` by the lead on the steward's branch |

## 3. Counts (main a209d40 + lb part 1)

Ledger 2,478 ids. **Pending 859** (hp 461, hc 210, cli 120, core 38, codec 30). **Deferred 2** (the AD-4 ids, Core A6-2). **Withdrawn 3** (two A3-1 ids, Core A9-2; wp_3, HP A3-2).

## 4. Live work (paused): branches pushed, handoffs on main

| Feature | Branch @ head | State | Handoff |
|---|---|---|---|
| core-a13 | foundation/core-a13 @ 8bfc5b7 | 11 Core A13 ids + R-28's four P3 transcript fixes written; WIP (vocabulary test fails until the crate gets the A13 `ClipboardWrite.contents` shape; lead decision (a) recorded) | docs/handoffs/core-a13.md (+ tools/core-a13-gen.py) |
| hp-surfaces | foundation/hp-surfaces @ 7d3f1bd | ~188 HP section 12-13 transcripts; final READY pending a green gate and CLEAN | docs/handoffs/hp-surfaces.md |
| hp-manifest | foundation/hp-manifest @ c7d0925 | HP A2 sec 1, 2, 3, 6, 16, 17, 25 + the R-22 wp_2 fix + HP A6/A8 ids; last review REJECT f86c468 (13 findings, addressed in c7d0925 except 3 moved to pending); no gate yet | docs/handoffs/hp-manifest.md |
| lb-cli-suite part 2 (CLI) | foundation/lb-cli-suite @ 244d5ed | 120 CLI ids; gate GREEN on this head; not yet CLEAN; HC A6 ids wait | docs/handoffs/lb-cli-suite.md |
| hc-groups | foundation/hc-groups @ 69cbac3 (+ wip-b2 4a05bde, wip-b3 18b0d0f) | 240 HC/codec ids in batches; R-29 (H2-H4) now allows the held layout ids and requires dropping the crate's "pending" marks | docs/handoffs/hc-groups.md |
| ts-player | foundation/ts-player @ 95d9dbb (WIP, not built) | script format for scripted server answers, the server-role Rust player + fault scripts, wasm + TS package, TS runner with parity (list in lead-notes.md) | docs/handoffs/ts-player.md |

Gate and merge procedure (BUILD.md): the reviewer sends `CLEAN <feature> <branch> <sha>` with the implementer's green `~/botster-sessions/shared/tools/botster-gate <worktree>` log on that exact head. If main since READY changed DATA only (amendments, verdicts, docs, frozen, ledger, pending, replacement map), the lead merges it in a gate worktree (`~/botster-sessions/trybotster-botster-contracts-gate`, `-gate2`) and re-runs `cargo xtask ci --job manifest|frozen|taint|ledger|codegen`; if main changed CODE, the pair re-merges and re-gates. Fast-forward main only; never force-push.

## 5. Queue after the live work (in order)

1. Finish the six features above (section 4), each from its handoff.
2. **HC Amendment 6** is FINAL (manifest final32, ids mapped): package.install result {package, version}; worktree row `branch` optional + `head`. Change botster-hub-contract (result + row), then the CLI ids in lb-cli-suite. **Core Amendment 14** is FINAL (final33, 10 a14 ids mapped, Stage 1 P3): clipboard_bytes bounds the decoded payload; add to core-a13's scope (with the A13 crate shape). **Core Amendment 16** is FINAL (final34, 7 a16_1 ids mapped): StopService completes with ServiceEnd = Exited(ServiceExit) | Lost{reason, payload_may_remain}; services ids (Stage 1 P7): a new core-services follow-up (process edge: process_end_guardian; EpochExhausted via A6-1's scripted service at the largest epoch).
3. Remaining Core follow-ups: the 16 P4a ids in `docs/core-p4a-2-notes.md`, the 9 P7 ids in `docs/core-p7-notes.md`, 2 DP-5b ids in `docs/core-p4b-notes.md` (each needs a boundary or observer the contract does not yet give; ask the steward per id).
4. HP remainders: a2_6_every_event_has_a_payload_row (needs client-attach / route-end Hub controls), tr_2_hostile_plugin_contained (slow sandbox).
5. `phase1-readme` (design 12c): how a Phase 1 lead depends on the crates, implements the harness traits (Core testkit, HubSubject, LuaHostSubject, CliSubject), runs the suites and the slow tier.
6. Phase 0 DONE when every ledger id has a transcript (or is deferred/withdrawn), every suite passes its structural check and players, the replacement map is reviewed, taint and budgets run in CI, and the README is complete.

## 6. Open steward items

- None open. Since the pause: final32 (HC A6), final33 (Core A14), final34 (Core A16) and final35 (Core A15, key-text bound; 4 a15_1 ids, numbers by R-34) landed and are mapped. The a15_1 ids belong with the Core input/route follow-ups on resume (ask the Core lead which package).
- Nothing else open from Stage 0 at the pause; every question asked was answered (R-14 .. R-29, Core A7-A13, Core errata 4-5, HP A3-A8, LB A1-A2).

## 7. Known issues

- **Linux gate tail stall:** FIXED by infra (main 0c0b4ea, tmpfs /tmp in the gate container). A branch needs ci/remote from 0c0b4ea or later. An xtask::commands timeout after that is a real finding.
- **Default-tier budget:** fixed by conformance-speed (~11-14 s); keep per-transcript cost low as transcripts grow.
- **HP Amendment 6 ids** (a2_7_status_values_mean_no_plugin_action, a6_1_*) belong to **hp-manifest**, not hp-surfaces.
- **R-22:** the landed transcript `conf::wp_2_attempt_retired_on_completion_expiry_and_plugin_lifecycle` is wrong for completion/expiry (must be `rejected`); the fix is on the hp-manifest branch, not yet on main.
- **Core A13 crate shape:** botster-core-contract still has the pre-A13 `ClipboardWrite` (bytes); core-a13 adds `contents` (decision (a)).
- Stale handoffs of landed features (hp-calls-a, prior-art-tooling, replacement-map, core-p4a-dp-2) remain in docs/handoffs/ as history.

## 8. First three actions on resume

1. Read sections 1, 4 and 7; run `receive_messages`; check the steward for HC Amendment 6 and any new manifest (map its ids on the steward's branch, ledger job, fast-forward main).
2. Respawn pairs from the handoffs, one at a time under the spawn rule: lb-cli-suite part 2 first (gate already green: needs only review), then hp-surfaces, hp-manifest, hc-groups, core-a13, ts-player. Spawn with `create_agent` (`target_id` tgt_e52ec52297404b5ca5a776da5bcd074b, `issue_or_branch` = the pushed branch (the old worktrees are removed), readable labels `<feature> — implementer|reviewer`), and point each at its handoff plus `role-implementer.md` / `role-reviewer.md`.
3. Land each CLEAN head as in section 4, tag `contracts-v0.1.x` for anything Stage 1 needs, and tell the Core lead (`sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`).

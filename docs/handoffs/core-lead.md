# Handoff: Botster v1 Stage 1 Core LEAD

> **Reading order at pause #3 (2026-10-05):** the CURRENT state is `docs/stage1-status.md` on `v1` and the dated "Update" sections at the end of this file, newest last. Sections 5 to 7 below are the 2026-10-04 morning snapshot (history); sections 3, 8 and 9, and every "Ruling" line, are still in force.

Written 2026-10-04 by the current lead, a Claude session: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`, worktree `~/botster-sessions/trybotster-botster-core-stage1-plan`, branch `stage1/plan`. The orchestrator ordered this handoff because the Claude weekly quota is about 97% used (it resets on Oct 8). A new lead, for example a Codex/Sol session, takes over from this file and needs no other context.

Before you act, update this file, or append a dated section at the end, after every merge, spawn, retirement or ruling.

## 1. Who and where

- Orchestrator: `sess-1790725330-0002-e5afba7455773a9e0f22332fb4dc3eeb`. Report up ONLY a QUESTION that only the orchestrator or the user can settle, BLOCKED, or a milestone.
  - Routine merges go only to the state log.
  - Its messages arrive as JSON `{"type":"orchestrator","text":...}` or as plain text.
- Contract steward: `sess-1790792742-003c-b2e696997f3d2e34d8ee89cb35f04cf9`. Contract rulings (R-n) and amendments come from the steward.
- Repository: `trybotster/botster-core`. The integration branch is `v1` (head `144b0234fb632bcbb5176b17c2fe55f3239405df` at pause #3, 2026-10-05; see `docs/stage1-status.md`). Never push to `main`.
- botster-mcp: `create_agent` ALWAYS takes these fields:
  - `target_id: "tgt_1f7bce66eb304881980f9b4a2a5ae3fe"`
  - `workspace_name: "Botster v1 — Stage 1 Core"`
  - `agent_name`: `sol` (Codex gpt-6.1-sol), `opus` or `sonnet`
  - `issue_or_branch`: the branch; cut it first with `--no-track` and `push -u`
  - `label` and `prompt`
- Verify each spawn with `list_hubs`. Its output is large: filter the saved file with `jq '.[].agents[] | select(.workspace_name=="Botster v1 — Stage 1 Core")'`.
- `post_message` takes `{session_uuid, payload}`. Read the inbox with `receive_messages` once per doorbell. Never poll.
- Copy every UUID and SHA from tool output; never type one from memory.

## 2. Binding documents (read in this order)

1. `~/Projects/botster-contracts/docs/BUILD.md` (binding over everything; includes the "Ghostty fork policy" and the merge rules).
2. The plan: `docs/stage1-plan.md` on `stage1/plan`, revision 21 = `ad03636ffbd68048f9f07faf176986f004d5e516`.
   - Read-only pin: `~/botster-sessions/pins/stage1-plan.bdda2359.md` (sha256 `bdda23593c29e3cc654acf0a5a98eee43f218b119ff2cd530cbd31d6b2cff9ac`).
   - Revision 21 is CLEAN by the P2 reviewer (verdict `772ab09` on `stage1/review-p2`, file `verdicts/stage1-plan-r21.md`).
   - Earlier revisions are CLEAN by the integration reviewer on `stage1/plan-review`.
3. `docs/stage1-status.md` and `docs/handoffs/*.md` on `v1`. These are the pause snapshot (PR #132); sections 3 and 4 are history now.
4. The state log `~/botster-sessions/botster-v1-orchestrator-state.md`. It is append-only and holds every spawn, merge, ruling and decision in order. Append to it after every event.
5. The pair rules and briefs in `~/botster-sessions/shared/core-stage1/`: `pair-common.md`, `brief-*.md`, and `handoffs/*.md` for each package.

## 3. Hard rules (user and orchestrator)

- **Merge is automated and done by the lead:**
  - The package reviewer must be CLEAN on the EXACT head.
  - A green `botster-gate` run on that EXACT head is required.
  - Cross-package or stage-completing PRs also need the integration reviewer's CLEAN. An Opus integration reviewer is staffed (`sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`, branch `stage1/integration-review`); it also reviews PRs the lead assigns explicitly.
  - Merge with `gh pr merge <n> -R trybotster/botster-core --merge --match-head-commit <head>`.
  - Then verify that `git rev-parse <merge>^{tree}` equals `<head>^{tree}`.
  - Never force-push, and never merge anything unaccepted.
- **Gate:**
  - Command: `~/botster-sessions/shared/tools/botster-gate [--on auto|linux|mac] <worktree> [-- <command>]`. Prefer `--on linux` (orchestrator, 2026-10-04); the Mac is shared with the user.
  - Exit 4 means a SKIPPED step (not a full gate). Exit 65 means a dirty tracked tree.
  - Logs are in `~/botster-sessions/gates/`.
  - Run one gate at a time per pair.
  - After landing, run `botster-gate --clean <worktree>` to remove the Mac gate tree.
- **Spawn rule (BUILD.md):** spawn only when the 1-minute load is under 12, OR CPU idle is at least 50% and botsterq runs at most one job. Check with `uptime`, `top -l 2 -n 0 -s 2 | grep "CPU usage"` and `botsterq list`.
- **Staffing limit:** at most TWO pairs now (the orchestrator raised it from one on 2026-10-04). Prefer Codex/Sol agents while the Claude quota is low. On HOLD: start no new pairs or gates, let running work finish, and relay the HOLD to the pairs in its exact scope.
- **Context protection:** pairs send the lead only QUESTION, BLOCKED, or terminal events (READY, CLEAN, DONE). Every brief says so.
- **Never kill processes by name or pattern** (no `pkill -f` or `killall`; a hook blocks it). Kill only PIDs you started.
- **Git:** no `git branch -D`; delete a merged branch with `git branch -d`; never use bare `git stash`.
- **Retiring a pair:**
  1. Confirm each worktree head is on origin (`git branch -r --contains HEAD`). The only allowed dirt is the spawner's `.gitignore`.
  2. Run `delete_agent {session_uuid, delete_worktree: true}`.
  3. `delete_agent` LEAVES THE WORKTREE behind, so run `git worktree remove --force <path>` yourself. Then run `git branch -d` for each merged branch, and `botster-gate --clean`.
- **Ghostty fork policy (user):** never patch the Ghostty fork without first rebasing the stack onto the latest upstream `ghostty-org/ghostty` main.

## 4. Pins in force

- Contracts: tag `contracts-v0.1.13` = `a8db5c9a0f43fc440564989fd38a55121fdda38b` (manifest final30). `v1` is on it.
- Ghostty: `trybotster/ghostty` branch `botster/upstream-sync-20261002` @ `3f8eb6810bb673aa782b047de21783ac81fb1121`, on upstream `f523504ea5c9f41d150d1eb93cc7a748b90f9361`. The sync record is botster-contracts `docs/ghostty/upstream-sync-20261002.md`.
- Zig 0.16.0, with 7 Zig packages. Rust 1.97.0. On the Mac, local cargo needs `env -u RUSTUP_TOOLCHAIN CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4`.

## 5. What landed since the pause (in `v1`)

| PR | Merge | What |
|---|---|---|
| #132 | `f96a9df` | Pause status and package handoffs (docs) |
| #133 | `e43225f` | `xtask lists`: each withdrawn or deferred id must exist in the pinned `ledger.json` (any contract); Core applies only `contract == core` |
| #134 | `38e6989` | P2 libghostty binding: Ghostty pin `3f8eb68`; contracts `v0.1.13`; macOS static link (P38); Linux allocator fix proven (P32); the mutants pass |

Milestone: P2 is done. Its pair is retired, and its handoff is history.

## 6. Live pairs (2026-10-04)

| Pkg | Implementer | Reviewer | Branch | Brief | State |
|---|---|---|---|---|---|
| P1 lifecycle (milestone M4) | Sol `sess-1791138159-00fe-ba70137e66e1d137b4d086b3a0697f6f` (swapped from Opus …00f4 at `c22b324`, 2026-10-04) | Sol `sess-1791107155-00f5-06e1a7ef014f4f8db7759a290e899c99` (`stage1/review-p1`, `verdicts/p1-lifecycle.md`) | `stage1/p1-lifecycle` | `brief-p1-resume.md` | Fix round F28 onward is with the reviewer (delta `8c38991..c22b324`); the F28 orphan fix is in `dc6e439`/`c22b324`. Current state: `handoffs/p1-lifecycle.md`. |
| P6 oracle controls | Sol `sess-1791136735-00fc-4b09baad3d85f4ce4397761167aced46` | Sol `sess-1791136735-00fd-5d70db1b6f5f889e162f1932f6ef2da4` (`stage1/review-p6`, `verdicts/p6-oracle.md`) | `stage1/p6-oracle` (cut from `38e6989`) | `brief-p6-oracle.md` | Started 2026-10-04 10:59. |

- **P1 orphan finding (orchestrator):** P1's cargo-mutants run left 7 orphaned `botster-core-sys` ready children, each busy-spinning for about 6 h. The required fix:
  - a test-side process-group guard that kills and reaps the child on Drop and on panic;
  - a child that waits without CPU and exits when its parent is gone;
  - a test of the broken-cleanup path;
  - the same check for the other real-process tests (plan R12).
- **P1 gates and heavy jobs** go to Linux: `botster-gate --on linux <worktree> [-- <command>]`.

## 7. Pending lead actions

1. **The P1 swap to Sol:** DONE 2026-10-04 at `c22b324`.
2. **P1 merge (milestone M4):** when the P1 reviewer is CLEAN on the exact head and the gate is green on that head, merge, verify the tree, append to the state log, and send the orchestrator the MILESTONE.
   - Then: P1's TI-1 follow-up (wire `botster_terminal_ghostty::terminal_identity()`; the `a2_8` ids; finding F14) as a small PR.
   - Then: the P1/P3 testkit stack (`stage1/p1-testkit-wiring` @ `95a5854`, P3 `stage1/p3-worker-m1-stack`, `-m2a`, `-m2b`). P3 rebases onto the new `v1` (merge, no force-push), gets a delta review, gates and merges (milestone M1). This stack is cross-package: it needs the integration reviewer, so ask the orchestrator to staff one.
3. **P6 merge:** single package, so no integration reviewer is needed. Then P6's next scope waits for the P1/P3 testkit stack (`docs/handoffs/p6-testkit.md`).
4. **Later waves (plan 6.3):** P4a after M2 (P3's M2b); P5 and P7 after M4. Each starts from `v1` on the current contracts tag.

## 8. Lead rulings in force (also in the state log)

- **P35:** BUILD.md rule 2 (expected terminal bytes are never hand-written) binds conformance ids and assertions in Botster repositories. Inside the Ghostty fork, libghostty's own Zig tests are the oracle and may use literal bytes. The ruling is recorded in plan revision 21.
- **Gate-evidence closure:** a finding that only a gate can close (P32, P38) does not block the gate. Gate once on the reviewed head where every other finding is closed. If the gate is green, the reviewer closes the finding and gives CLEAN on the SAME head. If it is red, any fix makes a new head and needs a delta review before the next gate.
- **Mutation policy:** every surviving mutant is killed by a test, or excluded with ONE function per `exclude_re` entry with its reason (and a named slow test where one applies), or argued equivalent in writing. No broad regex is allowed. A timeout is a finding. Pre-accepted classes: an operator swap on bits shown to be disjoint (cite the constants), and a Drop that only frees a native handle ("leak only, not observable through the API"). Iterate per file; gate only the final head.
- **Lists:** the contracts' `deferred.txt` and `withdrawn.txt` are cross-repo; each id must exist in the pinned `ledger.json`, and Core applies only `core` ids (PR #133).
- **Tests of a contracts pin move** (for example, the testkit's `the_pinned_files_parse`) may change in the PR that moves the pin; that does not make the PR cross-package. A change to a parser or to production code in another package does.

## 9. Known traps

- `delete_agent` leaves worktrees behind (9 of 9 at the pause, and again since).
- A pair can ask for the partner's UUID before the lead's introduction arrives: the messages cross. Resend once.
- On macOS, ld64 links a dylib that sits next to a static archive; the P2 binding links the archive from its own directory (P38).
- cargo-mutants can break a production cleanup path and orphan test children (section 6).
- The Mac Zig fetch can fail with `TlsInitializationFailed`; retry `zig build --fetch=all`.
- `botster-gate` on a docs-only change finishes in seconds, and that run is valid.
- The spawner rewrites `.gitignore` in new worktrees (deleting the mutants.out and bolero lines). The repo file is a superset: `git restore .gitignore` before a gate; never commit the spawner's version; `--allow-dirty` is never a merge gate.
- P6 in progress: a separate binding PR `stage1/p2-binding-oracle-api` (public restore, hyperlink and parser-state APIs over existing C exports; verdict file `verdicts/p2-binding-oracle-api.md`) lands before the P6 oracle controls.
- Learnings so far are in `~/knowledge/inbox/` (the /capture of the pause).

## Update 2026-10-04 12:35

- MERGED P1, PR #135 -> `v1` `1d25d093301072bd0c13a122c68d8f1b1ca0815c`. Milestone M4.
- Live pairs (all Sol):
  - P1: implementer `sess-1791138159-00fe-ba70137e66e1d137b4d086b3a0697f6f`, reviewer `…00f5`. Doing the TI-1 follow-up on `stage1/p1-ti1`. Retire the pair after it merges.
  - P6: implementer `…00fc`, reviewer `…00fd`. Oracle controls, plus the binding-API PR, plus the `slow_process_group.rs` orphan fix.
  - P3: implementer `sess-1791142478-00ff-2e34fc638a8fdb437255a2fa665f81b6`, reviewer `sess-1791142479-0100-7411ff7507ce89a5e7416311610290d3`. Brief `brief-p3-restack.md`: PR M1 (cross-package, needs the integration reviewer), then M2a, then M2b (milestone M2).
- Asked the orchestrator (QUESTION) whether to spawn one Sol integration reviewer for P3's PR M1.

- Integration reviewer: APPROVED by the orchestrator (Sol; cross-package and stage-completing PRs only). Spawn it when P3 sends "M1 IN REVIEW <head>", on a new branch `stage1/integration-review` from `v1`, with brief `brief-integration-reviewer.md`. Retire it after Stage 1's last cross-package merge.
- 2026-10-04 12:45: integration reviewer SPAWNED: `sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466` on `stage1/integration-review`. First PR: P3 M1 at `049751a`. P1 TI-1 is CLEAN at `96fbb93`, with its gate pending.
- 2026-10-04 12:55: MERGED PR #137 (P1 TI-1) -> `v1` `393f403`. P1 pair RETIRED. Live: P3 pair, P6 pair, integration reviewer. P3 M1 = PR #136; package CLEAN at `ed92707`; it must merge `v1` `393f403`, get a delta review by both reviewers, then gate.
- 2026-10-04: P7 approved (Sol pair). Spawn it right after P3 M1 (PR #136) merges, on new branch `stage1/p7-services` from `v1`, with brief `brief-p7-services.md`.
- Ruling (2026-10-04), tuning-constant mutants: contract equivalence is accepted under four conditions (no clause fixes the value; one test across default, mutant and minimum values; a contract lower bound is enforced, so mutants below it are killed; one exclusion per function or constant, naming the proving tests).
- 2026-10-04: Mac-only HOLD in force (no new pairs, no Mac-local heavy jobs; Linux gates OK). P7 waits for the lift.
- 2026-10-04: HOLD flipped. The Linux host is full: no new Linux jobs; Mac gates are OK; still no new pairs.
- 2026-10-04: HOLD lifted; Linux gates resume. A new pair (P7) needs the orchestrator's go after two READY checks in a row.
- 2026-10-04: MERGED PR #138 (binding oracle APIs) -> `v1` `38bbe58`.
- 2026-10-04: OPEN steward question on the oracle_resume_every_cut fit bound (P6 is waiting; cuts are inconclusive until the ruling).
- 2026-10-04: R-30 answered the every-cut question (exact measurement accepted). Open: P6 checks whether the GHOSTSNP spec names the continuation limit (A8-2); a gap would be a Core conformance gap for the protocol-crate owner.
- 2026-10-04: GHOSTSNP spec gap (A8-2) assigned to the P6 pair as PR `stage1/p2-ghostsnp-spec` (P2 crate). The every-cut fit check depends on it.
- 2026-10-04: Mac-only HOLD again; all gates on Linux; no new pairs.
- 2026-10-04: P3 M2b must complete GHOSTSNP.md "Worker paging" in the same PR (recommended: no in-band framing).
- 2026-10-04: QUEUED: P3 Mac diagnostic (slow_payload, F18/F19). Notify the P3 implementer when the Mac HOLD lifts. Two earlier Mac cleanup hangs of 45 min point to a possible macOS exit-watch defect.
- 2026-10-04: MERGED PR #139 (GHOSTSNP spec) -> `v1` `a3b8af5`.
- 2026-10-04: Mac HOLD lifted; the P3 Mac diagnostic was released.
- 2026-10-04 17:3x: MERGED PR #140 (P6 oracle controls) -> `v1` `e8cf150`. P6 pair parked until M1. P7 is allowed: spawn it right after M1 merges (re-check the spawn rule).
- Ruling (2026-10-04): macOS-only (cfg) code needs a focused Mac mutation run, because the Linux gate cannot compile it. The Mac log is the required evidence, named in the READY and the PR.

## Update 2026-10-04 19:25

- MERGED P3 M1, PR #136 -> `v1` `01fd389`. MILESTONE M1.
- Live:
  - P3 (implementer `…00ff`, reviewer `…0100`): M2a next, then M2b (milestone M2).
  - P6 (`…00fc`, `…00fd`): the testkit control registry PR first, then dispatch and RealCoreHarness.
  - P7 (implementer `sess-1791166872-0106-01e2db49c7fdac64d1229b8ffe0f78bc`, reviewer `sess-1791166873-0107-7c669a52fdaaae4b54b0e4a91ee68e5f`): `brief-p7-services.md`.
  - Integration reviewer `…0101`.
- Rule: testkit controls register per module through P6's registry; nobody grows `harness.rs` dispatch directly.
- 2026-10-04 19:45: MERGED PR #141 (testkit control registry) -> `v1` `144b023`.
- Ruling (2026-10-04), RealCoreHarness ownership: guarded launch wrappers (`botster-test-anchor`) through public inputs (worker_path, the payload program); no production seam.
- 2026-10-04: the Codex-only constraint is LIFTED (Claude usage is at 9 percent). Staff on merit per BUILD.md roles: Opus for heavy, subtle implementers (plan 6.3), Sol reviewers. Live pairs keep their agents.
- 2026-10-04 19:55: QUALITY CHANGE. Implementers are Opus and reviewers Sol. The P3, P6 and P7 implementers swap at HANDOFF READY (respawn opus from_worktree on the same branch, with the shared handoff). The integration reviewer swaps to Opus at its HANDOFF READY. Audit of PRs 134-141: Opus auditor on stage1/audit-134-141, brief brief-audit-134-141.md; report its summary to the orchestrator as a MILESTONE.
- Audit reviewer: `sess-1791168697-0108-bf2ad6d5a72bd31b652336d4c90288d9` (Opus, read-only).
- 2026-10-04: integration reviewer is now Opus `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9` (the Sol …0101 session is retired). Give new implementers this UUID.

## Update 2026-10-04 20:05: staffing after the QUALITY CHANGE

| Role | Session | Model | Branch |
|---|---|---|---|
| P3 implementer | `sess-1791168825-010a-e873347e8fdb315566b5545e5c2d418b` | Opus | `stage1/p3-m2a-v1` |
| P3 reviewer | `sess-1791169299-010e-6e279534401cc89d86c447f2fa43f4bc` | Sol (high effort, `sol-high`) | `stage1/review-p3` |
| P6 implementer | `sess-1791168867-010c-c64ca4250bee21ff7b750bdb8ab111b4` | Opus | `stage1/p6-real-harness` |
| P6 reviewer | `sess-1791169299-010f-cc0146e26529bfbb3589ccc55e082532` | Sol (high effort, `sol-high`) | `stage1/review-p6` |
| P7 implementer | `sess-1791168848-010b-36f58b104381b2f67cc32619dd70ba42` | Opus | `stage1/p7-services` |
| P7 reviewer | `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` | Sol (high effort, `sol-high`) | `stage1/review-p7` |
| Integration reviewer | `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9` | Opus | `stage1/integration-review` |
| Audit reviewer (PRs 134-141) | `sess-1791168697-0108-bf2ad6d5a72bd31b652336d4c90288d9` | Opus | `stage1/audit-134-141` |

- Rule: implementers are Opus and package reviewers are Sol (cross-model review).
- Pending: the audit DONE; report the audit summary to the orchestrator as a MILESTONE.
- 2026-10-04 20:10: Mac-only HOLD; all heavy jobs and gates on Linux; tell the implementers when it lifts.
- 2026-10-04: reviewer effort change pending. At each Sol reviewer HANDOFF READY, respawn as `agent_name: sol-high` (new definition `~/.botster-dev/agents/sol-high`, effort high) from_worktree. Do not edit ~/.codex/config.toml, which is global.
- 2026-10-04 20:25: all three package reviewers now run as `sol-high` (high reasoning effort). Spawn future Sol reviewers with `agent_name: sol-high`.

## Update 2026-10-04 20:35: audit results and fix ownership

- The audit is DONE: `audits/v1-pr134-141.md` (commit `02acc4b`); issues #143-#160 (label `stage1-audit`).

| Owner | Findings |
|---|---|
| P3 | A3, A8, A11, #158, the payload part of #156 (PR `stage1/p3-audit-fixes`, before M2a) |
| P6 | #159; the A63 citation; A12 waits for the steward |
| UNOWNED (asked the orchestrator) | P1 code: A1, A2, A4, A5, A7, A9, A10, #155, #157, the storage part of #156 (proposal: a P5 pair, these fixes first) |
| Fork patch (asked the orchestrator) | A6 #148: upstream sync first, then the patch, then the pin move |
| Steward (asked) | A12 (the every-cut cost), A63 (a tag with R-30) |
- 2026-10-04 20:15: P5 SPAWNED (implementer Opus `sess-1791169804-0110-5d99de2d47b2ff27cfaac8fba5d16d3e`, reviewer sol-high `sess-1791169805-0111-87b8604455fe78bb06902df4c4e64bc3`; brief `brief-p5-adoption.md`). It owns the P1-code audit fixes first. A6 is PARKED pending a steward ruling ((a) amend, (b) Core-side bound, or (c) a fork patch, which also needs the orchestrator to tell the user).
- 2026-10-04 20:25: R-31 settles A12 (P6 proceeds). R-32 settles A6: a fork patch is required. WAITING for the orchestrator's GO (it informs the user) to spawn one short-lived Opus fork agent: upstream sync, then the patch with a Zig test, a sol-high review, then the pin move. Cite rulings as "R-n (main <commit>)": R-30 eb4aba0, R-31 and R-32 9a00db8.
- 2026-10-04 20:30: contracts-v0.1.14 = `9a00db8` (R-30 to R-32). P6 owns the Core pin-move PR `stage1/contracts-v0.1.14`. After it, all pairs merge v1. A6 is parked: wait for the orchestrator GO.
- 2026-10-04 20:45: Mac HOLD lifted. Botster gates run at normal priority (never --priority). No new pairs beyond P3, P5, P6 and P7.
- PENDING lead action: after the P3 driver-mutation PR merges, write a plan revision for the new slow mutants pass (section 8).
- PR #161 (pin move) PARKED until A31 (P3) and A10 (P5, PR stage1/p5-a10) are in v1. Then tell P6: merge v1, delta CLEAN, gate once.
- 2026-10-04 20:35: A6 fork task SPAWNED (implementer Opus `sess-1791171304-0112-f36545cd8fa2e17ba1c1fe6f1863319d`, reviewer sol-high `sess-1791171305-0113-e9a8c18db6f6331ae5c2274348370f8d`; brief `brief-fork-a6.md`). USER RULE: nothing ever goes upstream; push only to trybotster/ghostty; no contact with ghostty-org; fetching upstream main is fine. After its pin-move PR merges: the lead records the new Ghostty pin in a plan revision, and retires the fork pair.
- PENDING: Core Amendment 14 (clipboard_bytes = decoded-size bound). When the steward says it is final: P3 implements the a14_1 ids, a follow-up binding PR sets option 39 = clipboard_bytes, and owners.py assigns A14 to P3 in a plan revision.
- 2026-10-04 20:50: FULL HOLD on both machines. Tell the five implementers when it lifts.
- 2026-10-04: the audit reviewer is retired. Findings: `02acc4b` on `stage1/audit-134-141` (branch kept); issues #143-#160.
- PENDING: contracts-v0.1.15 (A14) tag requested. Then: P6 retargets #161 to it; a plan revision (owners.py A14 to P3, the pins, the new Ghostty pin after the fork PR).
- 2026-10-04 21:00: contracts-v0.1.15 = `69327d5` (final33). PR #161 is retargeted to it. Plan revision 22 to follow (pins, owners.py A14 to P3, Ghostty pin after the fork PR).
- R-33 (main 14c86ab): ignored OSC 5522 types (past 64) are not decoded and not counted. It needs a contracts tag later (v0.1.15 predates it).
- PENDING: Core Amendment 15 (max_key_text_bytes), candidate in review. When final and tagged: assign the ids (host-side and limits validation to P5 as owner of P1 code, the worker check to P3, the route id to P4a); include it in the plan revision.
- 2026-10-04 21:15: Linux full; Mac gates one at a time. Order: #162, then #142, then #163, then #161 (after the v1 merge and a delta review).
- CRITICAL PATH: PR #162 (P5: A10 plus the guard self-test root cause) blocks #142 (P7) and #161 (pin move). After #162 merges, tell P7 and P6 to merge v1, get a delta CLEAN, and gate once.
- MERGE ORDER: (1) P3 `stage1/p3-guard-macos` (the macOS guard regression); (2) #162 P5 A10; (3) #163 P3 audit fixes; (4) #142 P7; (5) #161 pin move to v0.1.15. Each after the first merges v1, gets a delta review and gates once. Open steward question: StopService of a Lost service (P7).
- PENDING: Core Amendment 16 (ServiceEnd), candidate in review. When final and tagged: the P7 ids, and a plan revision.
- PENDING: contracts-v0.1.16 (A16) requested; #161 retargets to it. A15 awaits the user (numbers).
- 2026-10-04 21:35: contracts-v0.1.16 = `7778b1e`. PR #161 is retargeted to it (still last in the merge order). A15 is with the user.
- MERGE ORDER updated: (1) #162, carrying #165 (the combined P5 A10 fix and the P3 macOS guard fix; one combined gate); (2) #163; (3) #142; (4) #161.
- PENDING: contracts-v0.1.17 (A15) requested; #161 retargets to it. A15 split: P5 (limits and host), P3 (worker), P4a (route). A plan revision is due for A14, A15 and A16 owners plus the Ghostty pin.
- 2026-10-04 21:45: contracts-v0.1.17 = `1725abf` (final35). PR #161 is retargeted to it.
- MERGE ORDER: (1) #162 carrying #165; (2) #163 (P3); (3) #164 (P5 audit fixes, CLEAN b8b37a6); (4) #142 (P7); (5) #161 (pin to v0.1.17).
- PAUSE #3 in progress: wind-down chain #165 into #162, then #163, #164, #142, #161 (then the fork PR if near done). After it: the status docs PR, retirements, DONE.

## RESUME STEP (orchestrator, 2026-10-05): prior-art pass

- On resume, before M2a, P3 does a prior-art pass (BUILD.md rule; P3's handoff has no "Prior art" note, while P1, P2 and P6 have one).
- Candidates:
  - shpool (a Rust session-persistence daemon; the closest analogue);
  - the tmux server and client, and tmux's per-pane key re-encoding;
  - the zellij server;
  - the wezterm mux;
  - abduco and dtach.
- Record reuse or reject per item.
- The P5 and P7 handoffs must carry a Prior-art note too: P5 (adoption: the tmux and zellij reattach model, shpool) and P7 (service supervision: the old botster-core launch.rs and supervisor.rs reuse, plus other supervisors).

# Orchestrator handoff: Botster v1 (paused 2026-10-01)

The user paused all Botster v1 work on 2026-10-01: the computer's resources are needed for other work. This file lets a fresh orchestrator, or the same one resumed, pick up the work without asking anything.

## Where things stand

- **Design:** frozen. The user signed off on manifest final6 on 2026-09-30. Additive amendments since then are recorded in `frozen/current/MANIFEST.md` and tagged `manifest-finalN`. `frozen/final6/` is the signed-off record. Every change goes through the amendment path: the steward drafts it, and the Amendment reviewer (Sol) ACKs it.
- **Build:** Phase 0 (Foundation, in this repo) is in progress. The precise state is in [`docs/STATUS.md`](../STATUS.md), written by the Foundation lead, and in each feature's `docs/handoffs/<feature>.md`.
- **The plan and rules:** [`docs/BUILD.md`](../BUILD.md) is binding. It covers roles, the messaging rule (only QUESTION, BLOCKED and DONE go up), architecture facts (libghostty owns terminal semantics), "learn first, then steal deliberately", fast and lean testing with the adopted tools, capture, and agent cleanup.

## Decisions made after the BUILD.md baseline (with the user's authority)

1. **Fake strategy: the HYBRID is ADOPTED** (2026-10-01). It replaces the hand-written fakes:
   - **Real components with injected sans-IO edges.** A Rust-only `botster-core-testkit` runs real Core and libghostty with a scripted program and storage. The Hub has an internal in-memory transport. A deterministic scheduler, a perturbation decorator and shuttle provide interleaving.
   - **Scripted transcript PLAYERS,** never reimplementations, only where an independent peer is needed: the client-subject runner, a TS player for Web, and the HP plugin-side FakeHub as a frame player for plugin authors.
   - **Record and replay is not an oracle.**
   - **Fault injection goes through edges.**
   - **The real-process slow tier is unchanged.**
   - **Before deleting anything,** every conformance id and fault is mapped to its replacement proof (`docs/fake-replacement-map.md`). The frozen fakes stay until their real counterparts pass the same suites.
   - **The client fake-hub feature is stopped permanently.**
   - **Sans-IO edges plus the injected scheduler are a hard Phase 1 requirement.**
   - Assessments are in `~/botster-sessions/shared/fake-mode-steward-assessment.md` and in the steward's and lead's handoffs.
   - **Amendments DONE:** Core A5 and HC A5 were accepted and published as manifest final12 (be1e781); ruling R-12 records what they supersede. **Still to do (lead):** the replacement map and the Phase 0 done-criterion rewrite. See STATUS.md. **Open steward question:** the testkit's service-lane transport edge. The steward recommends YES, through a small Core amendment (see docs/handoffs/steward.md).
2. **Learn first:** taint rules block old MECHANISM, not old KNOWLEDGE. Every feature records a "Prior art" note.
3. **Agent cleanup:** retire a pair after it merges and main's CI is green. Only leads create agents. Use readable labels.

## Machine-level changes made during this work

- **A global Claude Code hook blocks pattern kills** (`pkill`, `killall`, and kills driven by `pgrep`) for every agent: `~/.claude/hooks/block-pattern-kill.sh`, registered in `~/.claude/settings.json` (backup at `settings.json.bak-20261001`).
  - **Why:** on 2026-10-01 at 06:05:29, a Hyperflex agent's pattern kill matching `never` shut down every Claude agent whose launch prompt (in argv) contained "never".
  - The user declined any other change. The launchers still pass the prompt in argv.
- **The botster watchdog** (`~/botster-sessions/shared/tools/botster-watchdog.py`) is stopped. The user asked for it to be stopped.

## Who exists, and how to resume

| Role | Session | Resume |
|---|---|---|
| Orchestrator (Claude) | runs in `~/Projects/botster-hub` | `claude --resume 3924e787-1f81-4dda-b309-754e3aae9e89` |
| Contract steward (the plan auditor, Opus) | sess-1790792742-003c…, workspace "Botster" | `claude --resume a7d7106d-f276-4edb-a69e-e2b62b5bd6e9`, in its worktree `~/botster-sessions/trybotster-botster-hub-audit-plan-20260930`. Handoff: `docs/handoffs/steward.md` |
| Foundation lead (Claude) | sess-1790821502-0048…, workspace "Botster v1 — Foundation" | `claude --resume 161517f1-20fc-47d4-a8e2-7612d295736f`, in `~/botster-sessions/trybotster-botster-contracts-build-foundation-lead`. State: STATUS.md (16b5819); feature handoffs in docs/handoffs/ |
| Amendment reviewer (Sol) | sess-1790827886-0054… | Its state is in `verdicts/amendments.md` |

The feature pairs are retired at the pause, and each one's state is in its `docs/handoffs/<feature>.md`. **Resuming with NEW agents works just as well.** Spawn a Foundation lead with agent `claude`, target "Contracts", and a brief that points at BUILD.md, STATUS.md and this file. Include the BUILD.md "Architecture facts" and rule 0 in every brief.

## Other records

- **User decisions log:** `~/botster-evidence/core-stall-resync-20260925/orchestrator-user-decisions.md`.
- **Orchestrator memory:** `~/.claude/projects/-Users-jasonconigliari-Projects-botster-hub/memory/` (MEMORY.md index).
- **Knowledge vault:** captures from 2026-09-30 and 2026-10-01 are in the `~/knowledge` inbox, waiting for `/seed` and `/ralph`.
- **Archive of the retired modular worktrees:** `~/botster-sessions/archive/modular-worktrees-20260930/`.
- **The interactive design map:** https://claude.ai/artifact/9UJKqSkoNz8iy2WgidYJNh (final6). Its source is in the orchestrator's scratchpad and is not durable. Republishing needs the files in `frozen/current/`.

## First actions on resume

1. Read STATUS.md, then the steward's handoff.
2. Finish the fake-mode amendments, the replacement map and the Phase 0 done-criterion rewrite, then finish Phase 0 under the narrowed criterion.
3. Start the Phase 1 leads (Core, Hub, Clients), all `claude`, with sans-IO edges as a hard requirement.


---

# Pause #2 — 2026-10-02 (supersedes the "Where things stand" and resume sections above)

The user paused Botster v1 again on 2026-10-02 because other work needs the machine. This section is the entry point for whoever resumes, whether those are new agents or the same ones.

## Process (unchanged, binding): docs/BUILD.md
- **Pipeline:** serial stages, parallel inside each stage.
- **Per-PR flow:** follows the HyperFlex playbook.
- **Merging is automated:** reviewer CLEAN on the exact head, plus `botster-gate` green on that head, plus integration CLEAN for stage-completing PRs.
- **Context protection:** at every commanding layer, only QUESTION, BLOCKED and stage milestones go up.
- **Testing rules:** fast and lean; learn first, then steal deliberately.
- **Ghostty fork policy (NEW 2026-10-02):** sync with upstream main before any patch, and at every stage start and pin move.

## Handoffs (read in this order)
1. `docs/handoffs/infra.md` (2a028ba). Run its **host health check FIRST.** The Linux gate host had 75 GB free at the pause. It also covers how `~/botster-sessions/shared/tools/botster-gate` chooses Linux (testq) or the Mac, and the parked self-hosted runners.
2. `docs/STATUS.md` (f5dc3ab), for **Stage 0**:
   - ledger 2,478 ids; pending 859 (hp 461, hc 210, cli 120, core 38, codec 30);
   - tag contracts-v0.1.13;
   - six feature handoffs in docs/handoffs/.
   Stage 0 resume plan: STATUS section 8.
3. `docs/handoffs/steward.md` (88e8195):
   - manifest final31;
   - final32 (HC A6, accepted) staged on steward/final32 with 4 steps left;
   - rulings R-1..R-29.
4. **botster-core v1** `docs/stage1-status.md` (merge f96a9df, PR #132), for **Stage 1 Core**:
   - landed: P0 skeleton, P6 testkit (M0b and scope 2) and infra;
   - in flight: P1 lifecycle, P2 libghostty, P3 worker (M1 CLEAN, stacked on P1), with handoffs in docs/handoffs/.
   **Ghostty upstream resync: DONE during the pause (2026-10-02).** The pin candidate is botster/upstream-sync-20261002 = 3f8eb68: 22 patches on upstream f523504ea, KEEP 21, DROP 0, REWORK 1. See docs/ghostty/upstream-sync-20261002.md (PR #6). **Resume action 1 for P2:** move the Core pin to 3f8eb68 and follow that doc's P2 steps. Sync again with upstream before any further patch (fork policy).

## Who to resume (or replace with fresh agents briefed from the files above)
| Role | Resume |
|---|---|
| Orchestrator | `claude --resume 3924e787-1f81-4dda-b309-754e3aae9e89` in ~/Projects/botster-hub |
| Stage 0 lead (2) | `claude --resume 93395ec1-822a-4220-9fff-795f1eb01140` in ~/botster-sessions/trybotster-botster-contracts-build-stage0-lead-2 |
| Stage 1 Core lead | Botster session sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673 (worktree ~/botster-sessions/trybotster-botster-core-stage1-plan) |
| Steward | `claude --resume a7d7106d-f276-4edb-a69e-e2b62b5bd6e9` |
| Infra | `claude --resume 967c3de3-cd41-4ec7-83a0-0f9dbb0cc1dd` in ~/botster-sessions/trybotster-botster-contracts-infra-remote-gate |

The watchdog cron is session-only. Re-arm it on resume (every 30 min, silent unless there is a problem). The state log is ~/botster-sessions/botster-v1-orchestrator-state.md.


---

# Pause #3 — 2026-10-05 (supersedes Pause #2's "Where things stand")

The user paused Botster v1 on 2026-10-05 to save token budget. Process, rules and BUILD.md are unchanged.

## State at the pause
- **Contracts:** manifest final35; tags `contracts-v0.1.14` to `contracts-v0.1.17` (v0.1.17 = 1725abf carries final35: HC A6, Core A14 clipboard_bytes, A15 max_key_text_bytes, A16 ServiceEnd). Rulings R-1 to R-34. Steward handoff: docs/handoffs/steward.md (9a2e1b6). No amendment in flight.
- **Stage 1 Core:** botster-core v1 head 9ea0c9c (PR #166, pause handoffs). Read `docs/stage1-status.md` first; its section 9 has the resume steps. Merged since pause #2: #133 to #141 and #166 (P2 libghostty binding, P1 lifecycle = M4, P3 M1, P2 binding APIs and the GHOSTSNP spec, P6 oracle controls).
- **Open PRs:** #165 needs review round 2 (F45/F46); then the merge chain #162, #163, #164, #142, #161 (#164, #142 and #161 are already CLEAN).
- **Audit of #134-#141** (Opus, 02acc4b, audits/v1-pr134-141.md): CONTRACT 8, HIGH 4, MEDIUM 32, LOW 20; issues #143-#160. Fixes are in #162 to #165. Architecture sound; P35 holds.
- **A6 Ghostty fork patch** (OSC 5522 over-limit report, R-32): in progress by the fork agent; see Core docs/handoffs for its state.
- **Stage 0 Foundation:** still paused (see Pause #2 section).

## User decisions this session (also in the user decisions log)
- **Staffing:** implementers are Claude Opus; Sol reviews at high effort; the integration reviewer is Opus. Every PR gets a cross-model review.
- **A15:** max_key_text_bytes default 256, range 1 to 4,096 (R-34).
- **Web key encoding:** the Web client encodes keys with restty against the route `modes` frames and sends `bytes` (0x83); the worker's semantic encoding (IN-9) stays for Hub and plugin writes and the TUI. Note for the Clients lead: ~/botster-sessions/shared/handoffs/stage-clients-notes.md.
- **Ghostty fork:** patches are allowed after an upstream sync, but nothing ever goes upstream; all changes stay in trybotster/ghostty.
- **Orphan processes:** the orchestrator may kill known dead or orphaned processes after re-verifying them (by process group).
- **Open suggestion for the user:** a walking skeleton (minimal Hub plus TUI running one session) once Core's worker milestones land, instead of strictly serial stages. Not decided.

## Who to resume
| Role | Resume |
|---|---|
| Orchestrator | `claude --resume 3924e787-1f81-4dda-b309-754e3aae9e89` in ~/Projects/botster-hub |
| Stage 1 Core lead | Botster session sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673. All 11 of its agents (5 implementers, 5 package reviewers, the integration reviewer) are idle with context kept; see Core docs/handoffs/lead.md. |
| Steward | sess-1790792742-003c-b2e696997f3d2e34d8ee89cb35f04cf9 (`claude --resume a7d7106d-f276-4edb-a69e-e2b62b5bd6e9`) |

## Resume procedure
1. Re-create the resource-gated resume loop: a 30-minute cron that runs `~/botster-sessions/shared/tools/botster-resume-check.sh` (state file `~/botster-sessions/shared/botster-resume-state`, now `paused`). Scope each HOLD to the busy host (Mac or Linux) only.
2. On READY, tell the Core lead to resume from stage1-status.md section 9: #165 round 2, the merge chain, plan revision 22, then the P3 prior-art pass (shpool, tmux, zellij, wezterm mux, abduco/dtach) before M2a.
3. Cut a contracts-v tag whenever the lead asks for a new manifest.

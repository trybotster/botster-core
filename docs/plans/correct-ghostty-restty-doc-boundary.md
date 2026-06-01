# Correct Ghostty/Restty Shadow-Terminal Documentation Boundary Plan

Ticket: Correct botster-core docs for Ghostty-only shadow terminal boundary
Run: run_1780289848_421022

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket
  `ticket_1780289832_257116`, run `run_1780289848_421022`, current step
  `botster_plan`, gate `botster_plan_gate`; no prior artifacts, findings,
  questions, or answers.
- Worktree: pipeline-provided ticket worktree at
  `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780289832_257116`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Additional vault/project constraints loaded:
  - [[identity]]
  - [[goals]]
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
- Repo context inspected:
  - `README.md`
  - `docs/ghostty-shadow-terminal-architecture.md`
  - `docs/architecture/ghostty-shadow-terminal-adapter.md`
  - `docs/plans/terminal-screen-snapshot-boundary.md`
  - `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - `docs/plans/default-local-pty-process-runtime.md`
  - `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - `docs/plans/session-process-wire-protocol.md`
  - `docs/plans/ghostty-shadow-terminal-architecture.md`
  - `docs/plans/wire-shadow-terminal-state-managed-session-runtime.md`
  - `Cargo.toml`
- Project Pipelines checklist:
  `checklist_1780289913_737353`. The first create call timed out inside the
  plugin worker, but the checklist persisted and the context item was marked
  done with loaded-note evidence.

## Scope

Correct documentation wording so future agents do not treat restty as possible
core shadow-terminal infrastructure. The implementation should be small and
docs-only.

In scope:

- Keep or strengthen current README wording that distinguishes:
  - `botster-core` as the backend-neutral terminal seam owner.
  - `botster-terminal-ghostty` as the blessed concrete core-side Ghostty adapter
    home.
  - restty as web/client rendering only.
- Keep or strengthen current architecture docs:
  `docs/ghostty-shadow-terminal-architecture.md` and
  `docs/architecture/ghostty-shadow-terminal-adapter.md`.
- Add explicit superseded/historical notes or correct phrasing in stale plan
  docs that still present restty as a terminal backend/parser peer, even inside
  exclusion-context wording. The enumerated stale-doc set from planning is:
  - `docs/plans/terminal-screen-snapshot-boundary.md`
  - `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - `docs/plans/default-local-pty-process-runtime.md`
  - `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - `docs/plans/session-process-wire-protocol.md`
- For each enumerated plan doc, choose the smallest correction: either de-pair
  restty from backend/parser wording directly, or add an adjacent superseding
  note that says restty is a client renderer and the paired wording is
  historical.
- Treat exclusion-context wording like "No Ghostty/restty parser" as stale if it
  teaches restty is a parser/backend peer. Do not assume a sentence is safe only
  because it excludes restty from core.
- Re-run the targeted search after edits and account for any additional docs
  that pair `Ghostty/restty` or list restty as a terminal backend/parser.
- Preserve older plan docs as historical artifacts. Do not rewrite whole plans
  or erase their original context; add superseding notes where needed.
- Keep all changed lines traceable to the ticket's documentation boundary.
- Keep docs free of PII beyond existing pipeline plan metadata style.

Non-scope:

- No Rust code changes.
- No crate layout, dependency, build script, feature flag, or API changes.
- No Ghostty/libghostty-vt implementation work.
- No restty integration, WASM, browser renderer, React/Catalyst, TUI, Rails,
  WebRTC, Lua plugin, MCP, or Project Pipelines UI work.
- No broad cleanup of unrelated documentation wording.
- No migration of historical plan docs into a new canonical document.

Botster layers touched: docs only, covering Rust core architecture and historical
plan artifacts. No plugin, Lua core, Rust hub, session/client worker, TUI,
React SPA, Rails relay, or MCP runtime surface should change.

Worktree/target assumption: implementation agents work in this assigned
Project Pipelines worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`,
targeting `main`.

Pipeline gates/artifacts: this file is the repo-visible Plan artifact. Gate
evidence should cite this file and the checklist id.

## Assumptions And Unknowns

Assumptions:

- The current architecture direction is settled: Ghostty/libghostty-vt is the
  blessed core-side shadow-terminal model for authoritative screen/snapshot
  state.
- `botster-core` remains backend-neutral; the Ghostty concrete adapter belongs
  in `botster-terminal-ghostty`, not directly in core.
- restty remains valid as a web/client renderer that consumes terminal state,
  streams, and snapshots through client data-plane contracts.
- Historical plan docs may remain understandable as time-stamped artifacts if
  they carry a clear superseding note near stale wording.
- The ticket's "cargo docs/tests still pass" acceptance does not require code
  changes, but the implementation should still run Rust verification because
  README/rustdoc-adjacent docs can affect docs builds. `docs/plans/*.md` are
  not rustdoc inputs, so cargo doc/test is a regression guard rather than the
  direct proof for plan-doc wording.

Unknowns for implementation:

- Whether `README.md` and the two current architecture docs need edits, or
  whether they already satisfy the current-doc acceptance and should be left
  unchanged after inspection.
- Whether there are stale restty-as-backend/parser references outside the
  enumerated stale-doc set. The implementer should use targeted `rg` output to
  decide, not a blanket docs rewrite.
- Whether `cargo doc --workspace --no-deps` is enough doc verification or
  whether `cargo test --doc --workspace` should also be run. Prefer both if
  runtime is reasonable.

No human question is blocking. The ticket intent is specific and no acceptance
point needs waiver.

## Affected Surfaces / Files

Expected:

- `docs/plans/terminal-screen-snapshot-boundary.md`
  - Add an explicit superseded note near the context/scope or stale restty
    language stating that later Ghostty architecture supersedes any reading of
    restty as a core backend option.
  - Preserve the historical plan's explanation of the backend-neutral seam.
- `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - Correct or annotate the "restty ... terminal backend implementation" and
    "Ghostty/restty decisions" wording so restty is not framed as a backend.
- `docs/plans/default-local-pty-process-runtime.md`
  - Correct or annotate "Ghostty/restty dependency" and parser-adjacent wording
    so restty is not framed as a parser/backend dependency.
- `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - Correct or annotate "Ghostty/restty parser" wording.
- `docs/plans/session-process-wire-protocol.md`
  - Correct or annotate "Ghostty/restty bindings" and snapshot/parser wording
    where it implies restty is a core parser/backend peer.

Possibly touched after inspection:

- `README.md`
  - Only if current wording needs a small clarification to make restty
    renderer-only and Ghostty blessed-core-side status unmistakable.
- `docs/ghostty-shadow-terminal-architecture.md`
  - Only if it needs a pointer to the stale historical plan note.
- `docs/architecture/ghostty-shadow-terminal-adapter.md`
  - Only if the ADR needs a stronger restty-not-target sentence.
- `docs/plans/ghostty-shadow-terminal-architecture.md`
  - Only if a historical note would prevent agents from treating it as runtime
    implementation guidance. It already says Ghostty is authoritative and
    restty is out-of-core, so avoid churn unless review finds ambiguity.
- Other `docs/plans/*.md`
  - Only if targeted search finds additional restty-as-backend/parser peer
    wording beyond the enumerated set.
- `docs/plans/correct-ghostty-restty-doc-boundary.md`
  - This plan artifact.

Not expected:

- `Cargo.toml` or any crate source/test file.
- Any generated docs or build output.
- Any old TryBotster, hub, browser, TUI, Rails, Lua plugin, or MCP files.

## Risks

- Over-editing historical plans could erase useful rationale. Use superseding
  notes instead of rewriting old implementation context.
- Leaving a stale "Ghostty/restty host dependency choices" sentence without a
  superseded note could recreate the exact confusion this ticket addresses.
- Making README say `botster-core` itself is a Ghostty crate would violate the
  backend-neutral seam convention.
- Making restty sound forbidden entirely would be wrong; it is forbidden as
  core shadow-terminal infrastructure, not as a client/web renderer.
- Paired terms such as "Ghostty/restty parser" or "Ghostty/restty bindings" can
  perpetuate the wrong mental model even inside otherwise-correct exclusion
  wording. The implementer must evaluate the backend/parser implication, not
  only whether the sentence excludes restty from core.
- `docs/plans/*.md` edits will not directly affect rustdoc, so cargo doc/test
  passing cannot by itself prove the documentation wording was corrected.
- Running only text search is insufficient if docs tests break; Rust docs/tests
  still need verification.
- Broad source guards or prose-matching tests would be brittle for a docs-only
  ticket. Prefer manual `rg` review and existing Rust verification commands.

## Acceptance Checks / Tests

Implementation acceptance:

- `README.md` clearly distinguishes the backend-neutral core seam, the blessed
  Ghostty/libghostty-vt core-side adapter path, and restty as web/client
  renderer only.
- `docs/ghostty-shadow-terminal-architecture.md` and
  `docs/architecture/ghostty-shadow-terminal-adapter.md` remain aligned with
  Ghostty/libghostty-vt as the concrete core-side shadow-terminal direction.
- Every enumerated stale plan doc either corrects restty backend/parser peer
  wording or carries an adjacent superseded/historical note:
  - `docs/plans/terminal-screen-snapshot-boundary.md`
  - `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - `docs/plans/default-local-pty-process-runtime.md`
  - `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - `docs/plans/session-process-wire-protocol.md`
- After implementation, the targeted search is a pass/fail check: no remaining
  occurrence may present restty as a terminal backend/parser, including paired
  "Ghostty/restty" wording, unless the adjacent text explicitly marks that
  framing as historical/superseded and restates restty as client rendering.
- No docs claim restty owns authoritative terminal truth, parser state, or core
  shadow-terminal infrastructure.
- No code, dependency, build, or runtime behavior changes are introduced.
- No PII is introduced.

Suggested verification commands:

- `rg -n "Ghostty/restty|Ghostty, libghostty-vt, restty|Ghostty/restty parser|Ghostty/restty bindings|restty.*terminal backend|terminal backend.*restty|parser.*restty|restty.*parser" README.md docs`
  - Required pass/fail review against the enumerated stale-doc set. Remaining
    matches are acceptable only when adjacent text corrects the framing or marks
    it historical/superseded.
- `rg -n "restty|Ghostty|ghostty|libghostty|shadow terminal|shadow-terminal|terminal backend|backend" README.md docs`
  - Broader manual review aid to catch missed references after the pass/fail
    targeted check.
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo test --doc --workspace`
- `cargo doc --workspace --no-deps`

Runtime/user-path proof:

- This ticket intentionally changes docs only. The user path changed is the
  agent/developer documentation path: README/current architecture docs and
  historical plan docs should no longer direct future work toward restty as
  authoritative core shadow-terminal infrastructure.
- Production runtime behavior is intentionally unchanged. The relevant
  production entry point remains future Ghostty adapter work through
  `botster-terminal-ghostty` implementing `TerminalScreenRuntime` and being
  wired through the session/runtime data path in a later ticket.

## Vault Gaps Worth Capturing

- Likely no new durable vault note is needed because the loaded vault already
  contains `ghostty shadow terminal integration belongs outside botster core`
  and `restty is a client renderer not authoritative terminal infrastructure`.
- Capture only if implementation uncovers a new repeatable rule, such as a
  preferred pattern for marking stale Project Pipelines plan docs as superseded
  without rewriting historical artifacts.

## Plan Review Findings Addressed

- `finding_1780290255_500671`: resolved by enumerating the stale-doc set found
  by targeted `rg` planning output and requiring an explicit decision for each
  document, including exclusion-context wording.
- `finding_1780290255_747215`: resolved by adding the risk that paired
  "Ghostty/restty" terms are misleading even when the sentence excludes them
  from core.
- `finding_1780290255_926215`: resolved by replacing subjective acceptance with
  a pass/fail targeted `rg` criterion tied to the enumerated stale-doc set.

No convention conflict was found in planning. The plan follows the loaded
Botster constraints: core stays backend-neutral, Ghostty is the blessed
core-side concrete shadow-terminal direction, restty remains renderer-only, and
Project Pipelines evidence is recorded through gate and checklist artifacts.

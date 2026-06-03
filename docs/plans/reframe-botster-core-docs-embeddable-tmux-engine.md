# Reframe botster-core docs around embeddable tmux-like engine

## Context Loaded

- Pipeline context: ticket `ticket_1780189401_997189`, run `run_1780189442_860343`, returned current step `botster_plan`, gate `botster_plan_gate`; prior plan review returned changes-required findings listed below, with no open human questions or answers.
- Returned plan-review context: review `review_1780192011_140680` requested changes because sibling ticket `ticket_1780189402_297736` / PR #23 already merged the ergonomic `BotsterEngine` facade to current `main` at merge commit `b803d65`.
- Vault role/context notes: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, `identity`, and `goals`.
- Repo context inspected:
  - `README.md`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - `crates/botster-core-dev/src/lib.rs`
  - existing plan artifacts in `docs/plans/`, especially the multiplexer, session worker, subscription, notification, plugin worker, and dev-engine example plans.

## Scope

- Reframe public docs so the north star is explicit: `botster-core` is an embeddable, programmable, tmux-like local execution engine for Botster hosts, not only a pile of transport-neutral contracts.
- Preserve the hub/core split:
  - Core owns reusable local execution mechanics: `BotsterEngine`, `MultiplexerEngine`, default PTY/process-facing contracts and fakes, session/runtime records, subscription fanout, activity/lifecycle state, notification inbox primitives, plugin worker primitives, and consumer test harnesses.
  - Hub/product hosts own auth, device and config locations, persistence policy, cloud federation, marketplace/install/update policy, WebRTC/signaling/API adapters, UI, and product workflow policy.
- Update docs to distinguish what the current repo proves from what this docs reframe adds.
- Add or update examples showing the real consumer API shape for an embedder using `BotsterEngine` with fake/session runtime adapters, subscription fanout, plugin invocation, notification drain, and shutdown.
- Keep examples scrubbed and deterministic.

## Non-Scope

- No Rust behavior changes unless documentation examples fail to compile due to stale API names.
- No new engine abstractions, async runtime policy, config loaders, CLI commands, hub adapters, WebRTC signaling, marketplace/install/update code, Rails/API code, or product UI.
- No broad rewrite of historical plan artifacts beyond any targeted cross-link needed for the new framing.
- No duplicate rewrite of the already-merged `BotsterEngine` facade description from PR #23 except where the docs reframe needs a small additive clarification.
- No claim that product CLI config, auth, persistence, cloud federation, or marketplace policy lives in core.
- No PII, host-specific absolute paths, real tokens, device ids, usernames, or local project details in docs/examples.

## Botster Layers Touched

- Docs layer for `botster-core`.
- Rust crate docs/readme examples only as documentation surfaces.
- No plugin, Lua core, Rust hub, session/client worker runtime implementation, TUI, React SPA, Rails relay, MCP, or provider layer implementation changes.

## Affected Surfaces / Files

- `README.md`
  - Tighten only the opening framing from "reusable Botster runtime workspace" to "embeddable programmable tmux-like local engine."
  - Add a narrow "What core proves today" / "What this project adds" framing, without duplicating the existing boundary table, ban list, or host-owned-policy paragraph.
  - Add a consumer-facing example using the merged `BotsterEngine` facade. The example must either be a rustdoc/doctest or be mirrored by a compiling test path.
- `crates/botster-core/src/lib.rs`
  - Update crate-level rustdoc to match the embeddable engine north star and host/core split.
- `crates/botster-core/src/engine/botster.rs`
  - No implementation change expected. Use as the public API source for the README/module example; touch only if a doc comment needs a small clarification.
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - Prefer adding or adjusting a test that proves the documented `BotsterEngine` example path compiles and drives spawn/attach/write/resize/output/notification/plugin/shutdown through the public facade.
- `crates/botster-core-dev/src/lib.rs`
  - Optionally improve module docs/comments only if examples refer readers to the dev smoke harness as the current executable proof path.
- `docs/plans/reframe-botster-core-docs-embeddable-tmux-engine.md`
  - This plan artifact.
- Optional targeted existing docs links:
  - `docs/plans/assemble-core-multiplexer-engine-api.md`
  - `docs/plans/botster-core-dev-engine-smoke-examples.md`
  - Only if a short cross-reference prevents drift between old "contracts" language and the new north-star doc.

## Assumptions And Unknowns

- Assumption: this ticket is documentation-first. It should not invent missing core mechanics; it should document the direction and clearly mark current proof versus the docs reframe.
- Assumption: "default PTY/process management" in core should be documented as reusable mechanics and contracts/fakes currently proven by the crate, not as product executable selection or daemon lifecycle policy.
- Assumption: PR #23 / sibling ticket `ticket_1780189402_297736` is now part of the base state; implementation should build on `BotsterEngine`, not re-plan an ergonomic facade.
- Assumption: the consumer API example must use the real `BotsterEngine` facade and be compile-checked by a doctest or mirrored Rust test. There is no pseudocode fallback for the ergonomic API.
- Unknown: whether reviewers prefer the README snippet itself to be a rustdoc/doctest or a concise README snippet mirrored by `crates/botster-core/tests/multiplexer_engine_api_test.rs`. Plan bias: keep README readable and mirror the path in a test if doctest ergonomics would bloat the README.
- Unknown: whether every prior plan artifact should be reworded. Plan bias: do not bulk edit old plans; they are historical artifacts. Update only current public docs and targeted cross-links.

## Risks

- Overclaiming runtime support that core does not currently implement. Mitigation: split "currently proves" from the docs reframe and compile-check the public `BotsterEngine` example.
- Pulling hub/product policy into core docs by accident. Mitigation: keep auth, persistence, config locations, federation, marketplace, WebRTC/signaling/API adapters, UI, and workflow policy in the host section.
- Introducing stale examples that imply APIs compile when they do not. Mitigation: compile-check the `BotsterEngine` example through doctest or a mirrored integration test.
- Rewriting historical plan docs too broadly and creating noisy churn. Mitigation: change public docs first; touch old plan docs only for necessary cross-reference.
- Conflicting with the already-merged PR #23 wording in the README Ownership Boundary section. Mitigation: treat the `BotsterEngine` facade paragraph as existing base state and make additive docs changes only where the current ticket acceptance still has gaps.
- PII leakage from local paths or actual device/project values. Mitigation: examples use synthetic ids such as `session-1`, `client-1`, `demo-plugin`, and no absolute local paths.

## Acceptance Checks / Tests

- Documentation review:
  - `README.md` states the new north star: embeddable programmable tmux-like local engine.
  - Docs say what core currently proves versus what this framing adds.
  - Docs preserve the hub/core split and explicitly keep product config/auth/persistence/cloud/marketplace/WebRTC/signaling/API/UI policy out of core.
  - Examples show the desired consumer API shape using the real `BotsterEngine` facade without implying CLI/product config lives in core.
  - No PII or host-specific local paths in changed docs.
- Runtime/user-path proof:
  - Add or point to a compile-checked `BotsterEngine` example path covering spawn, attach, write, resize, output fanout, notification drain, plugin invoke, activity classification, and shutdown.
  - Run `cargo test -p botster-core multiplexer_engine` or a narrower `BotsterEngine` test filter if one is added.
  - Run `cargo test -p botster-core-dev engine_smoke` if README/module docs point readers at the dev smoke harness as executable proof.
- Formatting/quality:
  - `cargo fmt --check`
  - A targeted grep after implementation for forbidden implication terms around core-owned config/auth/marketplace/WebRTC policy.

## Pipeline Gates And Artifacts

- Plan artifact: this file.
- Required plan gate evidence should attach context loaded, scope/non-scope, assumptions/unknowns, affected files, risks, acceptance checks, and vault gaps.
- Worktree/target assumption: implementation target is the assigned `botster-core` pipeline worktree; agents must operate there, not in ambient checkouts.
- Base-state assumption: current `main` includes PR #23 (`b803d65`) with the `BotsterEngine` facade. Implementation must not proceed from a stale base that lacks `crates/botster-core/src/engine/botster.rs`.

## Vault Gaps Worth Capturing

- Capture only if implementation settles a durable docs convention for `botster-core` examples, such as requiring README consumer examples to be mirrored by compile-checked public facade tests.
- No new vault note is needed just for restating the already captured core/hub/plugin boundary.

## Checklist Evidence

- Conventions read: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines notes, explicit target/worktree orchestration notes, `identity`, and `goals`.
- Plan-review findings addressed: stale base corrected to current `main` with PR #23; pseudocode fallback removed; README gap narrowed to opening/north-star, current-proof framing, and real `BotsterEngine` example; sibling-ticket conflict risk added.
- Convention conflicts: none. The plan keeps product workflow policy outside core and avoids speculative implementation.
- Verification planned: targeted doc review plus compile-checked `BotsterEngine` example path and Rust tests.
- Durable capture: not needed unless implementation discovers a reusable docs/examples rule.

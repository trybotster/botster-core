# Route CoreDaemon Attach Engine Output

## Context Loaded

- Pipeline context: ticket `ticket_1782166694_596879`, run `run_1782166706_345956`, step `botster_plan`, gate `botster_plan_gate`; no prior artifacts, reviews, findings, questions, or answers were present.
- Role/context notes: [[identity]], [[goals]], [[planner-playbook]], [[botster-planner-playbook]].
- Botster architecture notes: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan agents must author vault context as wikilinks not home paths]], [[test script required for rust tests not cargo test]].
- Repo code inspected: `crates/botster-core-daemon/src/daemon.rs`, `crates/botster-core-daemon/src/api.rs`, `crates/botster-core-daemon/src/main.rs`, `crates/botster-core-daemon/tests/daemon_integration_test.rs`, `crates/botster-core/src/engine/multiplexer.rs`, `crates/botster-core/src/engine/managed_session_runtime.rs`, `docs/architecture/core-daemon.md`, `README.md`.
- Checklist evidence: `checklist_1782166744_572573` was created and completed with vault context, convention review, planned verification, and capture-candidate evidence.

## Scope

- Patch `CoreDaemon::attach` so the `BotsterEngineOutput` returned by `engine.attach_client(...)` is not dropped.
- Route attach output through the same daemon drain semantics used for ordinary runtime output:
  - rely on `engine.attach_client(...)` to drive the multiplexer/session-worker path;
  - retain the returned `client_egress` for public daemon consumers;
  - deliver retained attach output on the next `CoreDaemon::drain` call before later live PTY output.
- Add focused daemon integration coverage proving late attach to an existing running session receives prior terminal history through the public `CoreDaemon` attach/drain path.
- Assert the replayed payload contains the prior marker and each `TerminalOutput` frame reports `bytes == data.len()`.
- Assert later live `TerminalOutput` still flows after the replay.
- Add or adjust a daemon contract comment/docs note explaining that attach output is load-bearing and must not be dropped.

## Non-Scope

- No hub dependency update, hub test changes, hub `Cargo.lock` changes, or browser work. The ticket explicitly leaves hub follow-up to `ticket_1782163713_316845`.
- No synthetic daemon or hub scrollback cache.
- No broad refactor of `DefaultBotsterEngine`, `WorkerBackedBotsterEngine`, subscription multiplexer, transport DTOs, or public protocol shapes unless required by compile errors from the surgical daemon change.
- No changes to `CoreDaemon::input` or `CoreDaemon::resize`, even though they also discard `BotsterEngineOutput`. That is a known sibling pattern and intentionally out of scope for this attach-history ticket.
- No new configurable retention policy or durable history store.
- No Project Pipelines plugin/UI changes beyond this plan evidence.

## Assumptions And Unknowns

- Assumption: the intended public user path is `CoreDaemon::attach(...)` followed by `CoreDaemon::drain(...)`, because `AttachedSession` currently carries only identity while `DrainResult` is the public egress carrier.
- Assumption: preserving ordering is best done inside `CoreDaemon` with an in-memory pending daemon output queue appended by attach and drained before fresh runtime output.
- Assumption: `engine.attach_client(...)` already drives the assembled multiplexer/session-worker path. The returned `session_requests` are a record of requests already routed by the engine, not daemon follow-up work. `CoreDaemon::attach` should retain the returned `client_egress` and observations for public drain delivery without re-routing `session_requests`.
- Unknown: whether the current core primitive returns history as `Snapshot`, `Scrollback`, or `TerminalOutput` for this specific daemon path. The test should assert renderable terminal data and byte counts, not force a variant unless the production path now guarantees one.
- Unknown: the vault note [[test script required for rust tests not cargo test]] names a `cli/test.sh` wrapper, but this checkout has no `cli/` directory or script. Use the repo README's cargo-based verification, preferably with `BOTSTER_ENV=test` for targeted Rust tests when practical, and record the mismatch instead of inventing a missing wrapper.

## Affected Surfaces And Files

- `crates/botster-core-daemon/src/daemon.rs`
  - Add pending daemon output state to `CoreDaemon`.
  - Change `attach` to retain the attach `BotsterEngineOutput.client_egress` and observations returned after the engine has already routed subscription setup/history replay internally.
  - Change `drain` to prepend pending attach/replay output before live drain output.
  - Add a small internal helper for merging outcomes into `DrainResult` without losing observations/backpressure.
- `crates/botster-core-daemon/src/api.rs`
  - Optional doc comment adjustment if the contract is clearer on `DrainResult` or `AttachedSession`.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Add a focused Unix integration test for late attach replay plus later live output.
  - Possibly add helper functions for terminal frame byte-count assertions.
- `docs/architecture/core-daemon.md`
  - Add the public contract note that attach output is part of subscription setup/history replay and must be retained through daemon drain.
- `docs/plans/route-coredaemon-attach-engine-output.md`
  - This plan artifact.

## Risks

- Ordering regression: if `drain` pulls live PTY output before pending attach replay, late subscribers can see live output before history. Drain pending attach output first.
- Double delivery: if attach output is both returned directly and retained for drain, callers could see duplicate history. Keep public delivery on the established drain surface only.
- Re-routing bug: the returned attach `session_requests` have already been routed by the engine. A daemon helper that routes them again would double-subscribe and can duplicate or corrupt history replay. Retain the returned egress; do not replay the recorded requests.
- Lifecycle/backpressure loss: if pending outcomes ignore observations, existing drain-side registry reconciliation and backpressure reporting can drift. Merge observations/backpressure consistently.
- Test flake: PTY output is asynchronous. Use existing `drain_until` style loops and clear markers instead of single-tick assumptions.
- Verification mismatch: this repo's README uses cargo directly while the vault's broader Botster CLI note prefers `cli/test.sh`; implementation should report exact commands run and why.

## Acceptance Checks And Tests

- Add a daemon integration test similar to:
  - spawn a shell loop session;
  - attach an initial client;
  - wait for a prior marker through daemon drain;
  - attach a second late client;
  - drain and assert the late client's renderable terminal data contains the prior marker;
  - assert every matching `TerminalOutput` has `bytes == data.len()`;
  - send a later marker and assert the late client receives it after replay.
- Run focused daemon tests:
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test late`
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test daemon_spawns_lists_attaches_drains_inputs_resizes_and_shuts_down`
- Run package-level daemon regression:
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon`
- Run formatting/lint checks from README if time permits:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- Run broader workspace verification if feasible:
  - `BOTSTER_ENV=test cargo test --workspace`
- If docs comments are changed, include at least `cargo doc --workspace --no-deps` or explain why it was skipped.

## Pipeline Gates And Artifacts

- Plan gate artifact: this file plus submitted `botster_plan_gate` evidence.
- Implement gate should include the new test name, command output summaries, and a note proving the production path is `CoreDaemon::attach` plus `CoreDaemon::drain`, not merely lower-level engine code.
- Review/Verify should check for no PII, no hub scrollback cache, no synthetic daemon cache, and no unwired helper.

## Vault Gaps Worth Capturing

- Capture candidate if implementation confirms it: `CoreDaemon::attach` output is load-bearing because `engine.attach_client(...)` already routes subscription setup and initial history replay internally, and public daemon consumers only observe that replay if daemon attach retains the returned egress for drain.
- Capture candidate: the current core extraction repo lacks the `cli/test.sh` wrapper referenced by broader Botster CLI verification guidance; repo-local README commands are the operative verification source here.

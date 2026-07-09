# Route CoreDaemon Attach Engine Output

## Context Loaded

- Pipeline context: ticket `ticket_1782166694_596879`, run `run_1782172574_468236`, current step `botster_plan`, gate `botster_plan_gate`, PR #81 linked.
- Current branch state: this is a conflict-resolution re-run on an already implemented branch. `HEAD` is `d1cb3bb` (`Merge main into attach history branch`), with feature commit `113e13c` (`Route attach history through core daemon drain`) already present. This plan scopes the remaining pipeline work to validating the merged result and correcting any conflict drift, not re-deriving a greenfield implementation.
- Plan Review findings addressed: `finding_1782172975_813852`, `finding_1782172975_342223`, `finding_1782172975_628978`, `finding_1782172975_844352`, and `finding_1782172975_807761`.
- Prior human answers from the base run constrain this plan:
  - Keep the public core-daemon boundary as `Vec<(ClientId, TransportEgress)>`; do not add a daemon-level DTO or separate `bytes` field in this ticket.
  - Wire the existing core initial-history primitive into the public attach/subscribe path so attach causes the `SubscribeTerminal` / `GetInitialSnapshot` flow to be emitted and routed, then retain/route the resulting `BotsterEngineOutput` through `CoreDaemon::drain`.
  - Do not add a daemon or hub synthetic scrollback cache and do not broaden beyond attach/subscribe output routing.
- Role/context notes: [[identity]], [[goals]], [[planner-playbook]], [[botster-planner-playbook]].
- Botster architecture notes: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan agents must author vault context as wikilinks not home paths]], [[test script required for rust tests not cargo test]], [[project pipelines checklist worker timeouts require artifact evidence fallback]].
- Repo code inspected: `crates/botster-core/src/contract/client_stream.rs`, `crates/botster-core/src/engine/botster.rs`, `crates/botster-core/src/engine/managed_session_runtime.rs`, `crates/botster-core/src/engine/subscription_multiplexer.rs`, `crates/botster-core-daemon/src/daemon.rs`, `crates/botster-core-daemon/src/api.rs`, `crates/botster-core-daemon/tests/daemon_integration_test.rs`, `crates/botster-core/tests/botster_engine_api_test.rs`, `crates/botster-core/tests/managed_session_runtime_test.rs`, `crates/botster-core/tests/multiplexer_engine_api_test.rs`, `docs/architecture/core-daemon.md`, `README.md`.
- Branch diff checked with `git diff --name-only origin/main...HEAD` and `git diff --stat origin/main...HEAD`.

## Scope

- Validate and preserve the already-landed bounded fix in this conflict-resolution branch.
- Core subscription/client-stream path:
  - `TransportIngress::SubscribeSession` did not previously emit the initial-history primitive for this public attach path.
  - The in-scope fix is to make subscribe/attach emit and route the existing `SubscribeTerminal` / `GetInitialSnapshot` flow for the active subscription.
  - The client-stream path must project the resulting `InitialSnapshotReady` into renderable `TransportEgress` for the subscribed client without inventing a new protocol shape.
- Core engine facade path:
  - Keep `DefaultBotsterEngine` and `WorkerBackedBotsterEngine` attach behavior wired to the corrected subscribe/client-stream path.
  - Preserve tests proving the facade and managed runtime route initial history for late subscribers.
- Core daemon path:
  - `CoreDaemon::attach` must not drop the `BotsterEngineOutput` returned by `engine.attach_client(...)`.
  - Retain attach-produced `client_egress`, observations, and backpressure summaries in session-scoped daemon pending output.
  - Merge retained attach output into the next `CoreDaemon::drain(...)` result for that session before fresh runtime output so late-subscriber replay precedes later live terminal output.
- Tests and docs:
  - Preserve focused core and daemon tests for late attach replay, no-history behavior, ordering, and existing attach/drain/input/resize regression coverage.
  - Preserve the contract note in `docs/architecture/core-daemon.md` explaining that attach output is load-bearing and must not be dropped.

## Non-Scope

- No botster-hub dependency update, hub `Cargo.lock` update, hub tests, browser tests, or downstream DTO work; the ticket names that as follow-up for `ticket_1782163713_316845`.
- No synthetic daemon or hub scrollback/history cache.
- No new daemon-level event/DTO shape and no separate `bytes` metadata field at this crate boundary.
- No broad multiplexer, transport enum, or session-worker protocol redesign beyond the bounded attach/subscribe initial-snapshot wiring authorized by the base-run human answer.
- No speculative retention policy, persistent history store, optional configurability, or adjacent cleanup.
- No Project Pipelines plugin/UI changes beyond durable plan and gate evidence.

## Assumptions And Unknowns

- Assumption: the production/public daemon user path for this ticket is `CoreDaemon::attach(...)` followed by `CoreDaemon::drain(...)`, because `AttachedSession` carries identity and `DrainResult.client_egress` is the daemon egress carrier.
- Assumption corrected from the prior plan: before this fix, the public `SubscribeSession`/attach path only recorded the client subscription and did not emit the initial-history primitive. The valid plan must include the core client-stream/engine wiring that causes `SubscribeTerminal` / `GetInitialSnapshot` and routes `InitialSnapshotReady`.
- Assumption: returned attach `session_requests` are processed through the existing engine/runtime path after the core subscribe wiring, not by a new daemon-side duplicate router.
- Assumption: ordering is preserved by daemon session-scoped pending drain state merged before live `drain_runtime_once` output.
- Unknown: whether the public daemon path will surface prior history as `Snapshot`, `Scrollback`, or `TerminalOutput` in every future adapter. Tests should assert renderable data for the late client and only assert variants where the current core contract guarantees them.
- Unknown resolved by human answer: `bytes == data.len()` is not representable as separate metadata at this boundary. The daemon test can inspect `TransportEgress` data bytes directly, while the explicit metadata assertion belongs in the downstream hub DTO ticket.

## Affected Surfaces And Files

- `crates/botster-core/src/contract/client_stream.rs`
  - Load-bearing attach/subscribe fix site.
  - SubscribeSession should emit the existing subscribe-terminal / initial-snapshot request for the active subscription.
  - Initial snapshot handling should project non-empty history into renderable `TransportEgress` for the targeted client/subscription.
- `crates/botster-core/src/engine/botster.rs`
  - Public engine facade attach paths for `DefaultBotsterEngine` and `WorkerBackedBotsterEngine`.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Runtime routing path that processes session requests and pending runtime events after attach.
- `crates/botster-core/src/engine/subscription_multiplexer.rs`
  - Existing routing boundary to preserve; no broad refactor expected.
- `crates/botster-core-daemon/src/daemon.rs`
  - `CoreDaemon` pending attach/drain output.
  - `CoreDaemon::attach` retaining engine output instead of dropping it.
  - `CoreDaemon::drain` merging pending attach output before live runtime drain output.
  - Private helpers for converting and merging `BotsterEngineOutput` into `DrainResult`.
- `crates/botster-core-daemon/src/api.rs`
  - `DrainResult` contract comment documenting attach-produced history replay through drain.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Public-path daemon test for late attach replay and later live output.
  - Existing attach/drain/input/resize regression coverage.
- `crates/botster-core/tests/botster_engine_api_test.rs`
  - Facade-level attach/history coverage.
- `crates/botster-core/tests/managed_session_runtime_test.rs`
  - Managed-runtime late subscriber and no-history coverage.
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - Engine/multiplexer attach-history coverage.
- `docs/architecture/core-daemon.md`
  - Contract documentation that attach output must be retained because it carries subscription setup and initial-history replay.
- `docs/archive/plans/route-coredaemon-attach-engine-output.md`
  - This corrected plan artifact.

## Risks

- False-premise regression: treating daemon retention alone as sufficient would retain empty output if the core subscribe path does not emit the initial-history primitive.
- Ordering regression: live PTY output could appear before retained initial history if daemon `drain` merges in the wrong order.
- Double delivery: re-routing returned session requests from daemon would duplicate subscription setup/history replay.
- Lost observations/backpressure: retaining only `client_egress` would hide diagnostics and registry-relevant observations from drain.
- Cross-session leakage: pending attach output must be keyed by session and drained only for the matching `SessionId`.
- Test flake: PTY output is asynchronous; tests need existing `drain_until` style polling and distinct markers.
- Scope creep: adding a daemon DTO `bytes` field would contradict the human answer and broaden the downstream protocol surface.
- Conflict drift: this run exists after merging main into an already-implemented branch, so verification must prove the merged tree still satisfies acceptance.

## Acceptance Checks And Tests

- Validate the already-landed public daemon test:
  - spawn a running shell session through `CoreDaemon`;
  - attach an initial client and prove a prior marker reached terminal state;
  - attach a second late client through `CoreDaemon::attach`;
  - drain through `CoreDaemon::drain`;
  - filter `Vec<(ClientId, TransportEgress)>` to the late client;
  - assert renderable output contains the prior marker;
  - write a later marker and assert the late client receives it after replay and after the prior marker.
- Validate core-level coverage for the bounded subscribe/initial-history wiring:
  - facade attach late-subscriber replay;
  - managed runtime replay and no-history behavior;
  - multiplexer/engine routing coverage for initial snapshot replay.
- Confirm daemon tests document that this boundary exposes `TransportEgress` data, not a separate `bytes` field; downstream hub DTO tests own `bytes == data.len()` metadata.
- Run focused daemon coverage:
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test daemon_late_attach_drains_initial_history_before_later_live_output`
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test daemon_spawns_lists_attaches_drains_inputs_resizes_and_shuts_down`
- Run focused core coverage:
  - `BOTSTER_ENV=test cargo test -p botster-core --test botster_engine_api_test`
  - `BOTSTER_ENV=test cargo test -p botster-core --test managed_session_runtime_test`
  - `BOTSTER_ENV=test cargo test -p botster-core --test multiplexer_engine_api_test`
- Run package regression:
  - `BOTSTER_ENV=test cargo test -p botster-core-daemon`
  - `BOTSTER_ENV=test cargo test -p botster-core`
- Run formatting and lint checks before handoff:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- If docs or rustdoc links change, run `cargo doc --workspace --no-deps` or explain why skipped.
- Verification note: broader vault guidance prefers `cli/test.sh`, but this extracted core checkout has no `cli/` directory or wrapper. Use repo-local README commands with `BOTSTER_ENV=test` and record that mismatch.

## Pipeline Gates And Artifacts

- Plan gate artifact: this file plus submitted `botster_plan_gate` evidence.
- Implement gate should prove the merged branch state with `CoreDaemon::attach(...)` plus `CoreDaemon::drain(...)` test evidence and core subscribe/initial-history test evidence.
- Review and Verify should check for no PII, no hub/daemon synthetic cache, no protocol DTO broadening, no duplicate session-request routing, no unwired helper, and no conflict drift from the merge with main.
- Checklist fallback/evidence is embedded here and in gate evidence; run checklists also exist in this run.

## Vault Gaps Worth Capturing

- Capture if implementation/verification confirms it: `CoreDaemon::attach` output is load-bearing only after the public core subscribe path emits and routes the existing initial-history primitive; daemon retention alone is not sufficient.
- Capture if recurring: the current botster-core extraction repo lacks the `cli/test.sh` wrapper referenced by broader Botster CLI verification guidance; repo-local README commands plus `BOTSTER_ENV=test` are operative here.

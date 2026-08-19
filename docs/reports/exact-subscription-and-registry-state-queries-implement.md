# Implementation report: exact non-mutating subscription membership and session registry-state queries

Ticket: `ticket_1787104273_140454`
Run: `run_1787104535_852826`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/exact-subscription-and-registry-state-queries.md` at `22872d6`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core` (`trybotster/botster-core`)
- Independent `list_spawn_targets` resolution matches the approved plan
- Worktree: the pipeline-provided ticket worktree
- Merge policy: `direct` (no PR)
- Runtime-teardown class: not applicable. Both queries are read-only control-plane lookups. [[botster runtime teardown lenses]] was not loaded.

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implementation artifacts must match actual git state]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[workspace cargo test filters miss isolated downstream-shaped consumer crates]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Other repository charters — this run stays inside `botster-core`
- [[botster runtime teardown lenses]] — not a runtime-teardown-class ticket

Targeted notes:

- [[host ShutdownSession classification must call the exact-session Core query]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[colon worktree paths break cargo dyld library paths]] — this worktree path has no colon

Convention conflicts: none.

## Botster layers changed

- `botster-core-daemon` public exact membership and registry-state queries
- `botster-core-test-support` isolated Hub-lifecycle-shaped consumer proof for both queries

No Hub, Web, TUI, Ghostty crate, plugin-admission, or Project Pipelines product layer. Engine primitives in `botster-core` already existed and were not changed.

## Files changed

Create:

- `docs/reports/exact-subscription-and-registry-state-queries-implement.md`

Edit:

- `crates/botster-core-daemon/src/api.rs` — `SessionRegistryStateLookup`
- `crates/botster-core-daemon/src/lib.rs` — export
- `crates/botster-core-daemon/src/daemon.rs` — public queries, `DaemonEngine` arm, work-bound registry-state test
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — architecture, source-shape, membership, and non-mutation tests
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/src/lib.rs` — Hub-Pump and close-suppression consumer tests
- `crates/botster-core-test-support/tests/lifecycle_journal_consumer_test.rs` — wrapper source assertions for the exact `CoreDaemon` calls

## Ownership boundaries preserved

- Core owns both queries as policy-free mechanisms. Host policy (when to call them for Pump or close suppression) stays in Hub.
- `observe_session_lifecycle` remains the reconciling exact-session query.
- `list_terminal_subscriptions` remains the full inventory.
- No terminal bytes, phases, snapshots, or attach state on either new surface.
- `hub-adapter-shaped` is unchanged because it cannot call `CoreDaemon`.

## Cross-repo dependencies or separately routed work

- Consumer: Hub ticket `ticket_1786912569_840742` will bump its Core git pin after this lands on `main`. This run does not implement Hub scheduling.
- Downstream-shaped proof lives in this repository's isolated `hub-lifecycle-shaped` consumer.

## Deviations from plan

- `session_registry_state` does not carry `#[must_use]`. It returns `Result`, which is already `#[must_use]`; Clippy `double_must_use` fails `-D warnings`. `terminal_subscription_generation` keeps `#[must_use]`. This is a clippy-required signature adjustment, not a scope change. The committed plan's decisions, files, and acceptance checks stay valid.

## Tests and downstream proof run

Repository-owned Cargo gates (from [[botster-core uses CI-owned Cargo commands because it has no test script]]):

- `BOTSTER_ENV=test cargo test --workspace` — pass
- `cargo fmt --all -- --check` — pass
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` — pass
- `BOTSTER_ENV=test cargo test --doc --workspace` — pass

Focused proofs:

- `BOTSTER_ENV=test cargo test --package botster-core-daemon --lib exact_registry_state_lookup_does_not_scan_a_large_registry` — pass (`registry.test_load_all_calls() == 0`, ablation increments)
- `BOTSTER_ENV=test cargo test --package botster-core-daemon --test daemon_integration_test -- exact` — pass (`exact_query_methods_are_control_plane_and_work_bound`, `terminal_subscription_generation_is_exact_membership`)
- `BOTSTER_ENV=test cargo test --package botster-core-daemon --test daemon_integration_test -- session_registry_state` — pass (absent, shutdown, UnknownSession anomaly, parked-exit negative with observe positive control)
- `BOTSTER_ENV=test cargo test --package botster-core-test-support --test lifecycle_journal_consumer_test -- isolated_hub_shaped_lifecycle_consumer` — pass. This wrapper starts the isolated nonmember crate; a workspace-root filter would not run it ([[workspace cargo test filters miss isolated downstream-shaped consumer crates]]).

Production entry point: `CoreDaemon::terminal_subscription_generation` and `CoreDaemon::session_registry_state` are the public daemon facade Hub will call. Hub-lifecycle-shaped tests call those methods on `CoreDaemon` after spawn/attach, not crate-local engine helpers.

## Unverified behavior or residual risk

- Hub Pump and close-suppression wiring is intentionally out of scope. Live Hub use waits for the Hub pin bump.
- The parked-exit negative test waits for OS-level exit or zombie state (`ps` state `Z`) without draining. Local `try_wait` still happens only on observe/drain, so the positive observe control is required to prove the exit was pending.
- `UnknownSession` for registry-missing-but-engine-live remains an anomaly path, constructed in tests by deleting the registry row after spawn.

## Missing vault guidance discovered

Capture candidates after merge, not in this ticket:

- CoreDaemon exact-session queries take `&self` where possible, and work bounds are proven with registry `load_all` counters. The Result-returning query cannot take `#[must_use]` under Clippy `double_must_use`.
- Hub-shaped consumer crates remain the standing proof vehicle for new public `CoreDaemon` queries.

No vault capture was written in this step because these are pattern restatements, not a new decision that blocked implementation.

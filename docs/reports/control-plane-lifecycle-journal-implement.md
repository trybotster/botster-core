# Implementation report: control-plane lifecycle journal wake and page API

Ticket: `ticket_1786663581_962361`
Run: `run_1786681965_617006`
Step: `botster_stack_implement`
Plan: `docs/architecture/control-plane-lifecycle-journal.md` revision 4

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core` (`trybotster/botster-core`)
- Independent `list_spawn_targets` resolution matches the approved plan
- Worktree: the pipeline-provided ticket worktree
- Merge policy: `direct` (no PR)
- Runtime-teardown class: yes

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[botster runtime teardown lenses]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implementation artifacts must match actual git state]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[pipeline artifacts should use path neutral worktree references]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Other repository charters — this run stays inside `botster-core`

Targeted notes:

- [[hub drain advances non attached session lifecycle]]
- [[lifecycle guards evaluated before the reconciling drain are one call stale]]
- [[botster core hosts need an explicit drain loop contract]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[botster engine command surface uses botsterengine as facade]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]

## Botster layers changed

- `botster-core-daemon` public observe / wake / page API and journal bit
- `botster-core-test-support` isolated Hub-shaped consume-loop consumer
- Living architecture and README host-loop / lifecycle-projection paragraphs

No Hub, Web, TUI, Ghostty crate, plugin-admission, or Project Pipelines product layer.

## Files changed

Create:

- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/Cargo.toml`
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/src/lib.rs`
- `crates/botster-core-test-support/tests/lifecycle_journal_consumer_test.rs`
- `docs/reports/control-plane-lifecycle-journal-implement.md`

Edit:

- `crates/botster-core-daemon/src/api.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `docs/architecture/control-plane-lifecycle-journal.md`
- `docs/architecture/core-daemon.md`
- `README.md`

## Ownership boundaries preserved

Core owns session/process lifecycle facts, the in-memory journal, the coalesced wake, the bounded page API, and terminal-plane `ProcessExited`. Hub still owns when to consume the wake and pages, host retention, and cleanup. This run does not implement Hub projection or remove Hub Drain discovery.

## Cross-repo dependencies or separately routed work

Hub consumer `ticket_1786663582_169720` already depends on this ticket. No additional dependency ticket was required. Downstream-shaped proof stays in the isolated test-support consumer.

## Deviations from plan

None that change the published contract.

Implementation details inside the contract:

- `ObserveLifecycleResult` lives next to `CoreDaemonError` rather than in `api.rs`, to avoid an `api`/`daemon` module cycle.
- Same-tick sibling isolation injects `OutputFailed` through `CoreDaemonConfig::with_test_fail_runtime_drain_for` so observe still walks `drain_runtime_once` and continues. The later sibling is a local immediate-exit process; waiting on `kill -0` cannot see the unreaped zombie.
- `test_fail_runtime_drain_for` is a test-only config field, matching existing daemon test hooks.

## Runtime-teardown lenses implemented

| Lens | Implementation |
| --- | --- |
| Isolation | One session exit updates that row only. Wake is process-wide. Siblings stay live. |
| Bounds | `observe_lifecycle` is one non-blocking host tick. No `block_on(close)`. Existing ClientWorker hard-stop is unchanged. |
| Late-message matrix | Observe is idempotent and continues after per-session errors. Page validates cursor before budget. Drain still delivers terminal `ProcessExited`. |
| Production-path proof | Worker-backed zero-attach observe publishes `Exited` without `CoreDaemon::drain`. Dropped wake still pages. Sibling error plus later `Exited` is one observe tick. |
| Ownership identity | Journal identity remains `(source_id, sequence)`. Duplicate `Exited` observations do not append. |
| Sibling fail-closed | Observe does not call `drain_runtime_all_once`. A retained earlier error does not stop the later sibling. |

## Tests and downstream proof run

Focused during development:

- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- lifecycle_`
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- observe_ worker_backed_observe worker_backed_dropped`
- `BOTSTER_ENV=test cargo test -p botster-core-test-support --test lifecycle_journal_consumer_test`

Repository gates:

```sh
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --doc --workspace
```

All four repository gates passed.

## Unverified behavior or residual risk

- Hub's later consume loop is not run in a Hub checkout. The isolated consumer encodes the published order.
- `test_fail_runtime_drain_for` is a synthetic drain error, not a live worker I/O fault. Observe's continue-and-retain path is the production code.
- Stale vault notes still tell reviewers to use Drain as the no-Attach lifecycle oracle. Inbox capture names the replacement.

## Missing vault guidance discovered

Known and captured to the vault inbox:

- `core-control-plane-lifecycle-journal-advances-without-terminal-drain`

Existing notes that need a later rewrite, not done in this ticket:

- [[hub drain advances non attached session lifecycle]]
- [[botster core hosts need an explicit drain loop contract]]
- [[botster-runtime-reviewer-playbook]] no-Attach Drain bullet

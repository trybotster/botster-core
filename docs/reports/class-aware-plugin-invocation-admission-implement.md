# Implement report: class-aware plugin invocation admission

Ticket: `ticket_1786663581_723222`
Run: `run_1786663792_890418`
Plan: `docs/architecture/class-aware-plugin-invocation-admission.md` revision 3

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Worktree: this pipeline run worktree (no ambient substitution)
- Runtime-teardown class: does not apply
- Merge policy: direct; no PR

## Repository playbook and other playbooks/notes applied

Repository charter: `botster-core-playbook`

Role overlays:

- `implementer-playbook`
- `botster-implementer-playbook`
- `botster-architecture`
- `implement gate must verify committed work and pr link before review`
- `implementation artifacts must match actual git state`
- `pipeline vault checklists must cite exact resolvable note titles`
- `project pipelines checklist worker timeouts require artifact evidence fallback`

Targeted notes:

- `worker isolated and non blocking are different dispatch guarantees`
- `botster plugin runtime uses supervisor plus per plugin workers`
- `plugin worker queue capacity and executor concurrency are independent host profile knobs`
- `plugin hardening needs lifecycle resource and observability layers`
- `botster core lua owns plugin framework primitives not product policy`
- `botster packages should enforce core hub cli plugin provider boundaries`
- `botster core contract surface needs consumer proof`
- `botster core public enums are breaking until non exhaustive is decided`
- `public dto field additions are source breaking without non exhaustive`
- `workspace struct field changes require workspace cargo gates`
- `botster engine command surface uses botsterengine as facade`
- `botster core public surface needs a narrow start here path`
- `botster core hosts need an explicit drain loop contract`
- `structured output fields need producer paths or explicit scaffold disposition`
- `test script required for rust tests not cargo test` (overridden for this repo by `question_1786664489_333289`)
- `rust repo strict lints must be verified before dismissing warnings`

Not loaded: `project-pipelines-playbook` (no Project Pipelines package path).
Not loaded: `botster runtime teardown lenses` (not teardown class).

## Files changed

- `crates/botster-core/src/contract/actor.rs`
- `crates/botster-core/src/contract/mod.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core/src/engine/plugin_worker.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/engine/multiplexer.rs`
- `crates/botster-core/src/engine/command.rs`
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
- `crates/botster-core/tests/botster_engine_api_test.rs`
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
- `crates/botster-core/tests/plugin_timer_scheduler_test.rs`
- `crates/botster-core/tests/plugin_file_watch_runtime_test.rs`
- `crates/botster-core/tests/plugin_capability_isolation_under_load_test.rs`
- `crates/botster-core-dev/src/lib.rs`
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
- `README.md`
- `docs/architecture/first-party-host-profile-primitives.md`
- `docs/architecture/class-aware-plugin-invocation-admission.md`
- `docs/reports/class-aware-plugin-invocation-admission-implement.md`

## Ownership boundaries preserved

Core owns policy-free plugin-worker mechanics, class queues, reserved RequestResponse executors, completion reservation, and the engine deadline waiter.

Hub product policy is unchanged: no Hub event names, schemas, audiences, routing, package admission, or host control protocol.

No ClientWorker, SessionIo, terminal protocol, terminal adapter, or terminal queue changes.

## Cross-repo dependencies or separately routed work

Registered Hub consumers, not implemented here:

- `ticket_1786663582_483898` / `tgt_7e208a0c76a44980a83b63af976b1f22` (Hub package event router)
- `ticket_1786663582_169720` / `tgt_7e208a0c76a44980a83b63af976b1f22` (Hub non-blocking session projection)

`PluginWorkerEngineConfig` gained required fields. Hub struct literals will break on upgrade; those tickets own the wiring.

## Deviations from plan

None in product behavior. Implementation details required by correctness:

- Unload/reload move undrained async completions into an engine leftover mailbox so typed `WorkerStopped` outcomes remain drainable after the worker is removed.
- Existing `concurrency == 1` configs are invalid under `reserved < concurrency`. Tests that needed serialized execution now occupy both executors before asserting queue pressure.
- Crate-private tests construct payloads without naming `BoundaryJson` so `boundary_test` does not treat test fixtures as unclassified escape hatches.

## Tests and downstream proof run

Human correction `question_1786664489_333289`: repository-documented CI Cargo commands. No replacement wrapper.

Passed:

- `cargo fmt --all -- --check`
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test -p botster-core --lib plugin_worker` (crate-private lock contention, first-commit order, Drop)
- `BOTSTER_ENV=test cargo test -p botster-core --test plugin_worker_engine_test`
- `BOTSTER_ENV=test cargo test -p botster-core --test botster_engine_api_test` including `botster_engine_try_admit_plugin_drains_typed_background_timeout`
- `BOTSTER_ENV=test cargo test -p botster-core-dev` including `public_facade_admits_background_work_and_drains_typed_timeout`

Production entry points:

- `BotsterEngine::try_admit_plugin` / `drain_plugin_completions` wrap `PluginWorkerEngine`.
- `botster-core-dev::run_plugin_admission_proof` is a separate-crate consumer of that facade.

Pre-existing failure, isolated on branch and base `033cd01` with the same command (both exit 101, same assertion):

```
BOTSTER_ENV=test cargo test -p botster-core-test-support --test downstream_conformance_test many_pty_load_adversarial_noisy_reports_reader_backpressure -- --exact
```

This is a SessionIo PTY pressure test. This ticket did not change SessionIo, terminal queues, or daemon attach.

Earlier workspace flake `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output` passed in isolation and on a later full daemon suite (71/71).

## Unverified behavior or residual risk

- Hub live product callers are still scaffold disposition. First product caller is the registered Hub event-router ticket.
- `try_admit` can return `backpressured` on a busy admission lock; that is specified, but hosts must retry.
- `PluginTimerScheduler::drain_due` still uses blocking `invoke` (explicit non-scope).
- `many_pty_load_adversarial_noisy_reports_reader_backpressure` still fails on current `main`; not repaired here.

## Missing vault guidance discovered

Captured to inbox after Implement:

- `botster-core uses CI-owned Cargo commands because it has no test script`
- `Core class-aware plugin admission reserves request-response executors`
- `worker isolated now has a Core try-admit non-blocking primitive`

No Hub routing policy was captured into Core notes.

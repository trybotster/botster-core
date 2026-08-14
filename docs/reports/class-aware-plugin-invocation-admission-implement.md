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

## Review findings addressed (`review_1786668617_282076`)

- Deadlines are bound to worker generation plus request id. Reload with a reused request id cannot be sealed by the prior generation. Deadlines are removed on completion, timeout, unload, and reload.
- `try_admit` uses `try_lock` for registry, admission, deadline book, and cancellation map. Error-path backpressure is built from already-held worker metrics. Zero-timeout and immediate-failure publish into the held admission lock. Contention tests cover those locks.
- Queued timeouts remove `admission.jobs` and cancellation entries. Fast completions remove the long-deadline book entry. Repeated timeout and fast-completion tests prove private tracking stays bounded.
- Occupier tests start one executor at a time. The timer backpressure test uses a gated runtime instead of a sleep window.

## Review findings addressed (`review_1786669717_243333`)

- `drain_completions_honors_item_and_byte_caps` retries only typed `ADMISSION_LOCK_BUSY` within 100ms. Other public `try_admit` loops that asserted an immediate `Queued` use the same helper.
- `try_admit_never_waits_on_slow_in_flight_work` still times a single `try_admit` call and only retries the typed lock-busy reason.
- Facade and `botster-core-dev` consumer proof retry the same typed reason only.
- Exact ticket command `BOTSTER_ENV=test cargo test --workspace` now exits 0. No `--test-threads=1`.

## Review findings addressed (`review_1786670268_662272`)

- `interval_timer_retries_after_backpressure` starts one occupier and waits until that job is in flight before starting the next occupier. Then it fills the queue and asserts timer backpressure.
- This matches the worker-engine occupier pattern. Starting both occupiers together could backpressure the second invoke while the first job was still queued (queue capacity 1).
- Exact ticket command `BOTSTER_ENV=test cargo test --workspace` exits 0, including `interval_timer_retries_after_backpressure`.

## Deviations from plan

None in product behavior. Implementation details required by correctness:

- Unload/reload move undrained async completions into an engine leftover mailbox so typed `WorkerStopped` outcomes remain drainable after the worker is removed.
- Existing `concurrency == 1` configs are invalid under `reserved < concurrency`. Tests that needed serialized execution now occupy both executors before asserting queue pressure.
- Crate-private tests construct payloads without naming `BoundaryJson` so `boundary_test` does not treat test fixtures as unclassified escape hatches.

## Tests and downstream proof run

Human correction `question_1786664489_333289`: repository-documented CI Cargo commands. No replacement wrapper.

Passed after Review return `review_1786670268_662272`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `BOTSTER_ENV=test cargo test --workspace` (exit 0; includes `interval_timer_retries_after_backpressure`, `drain_completions_honors_item_and_byte_caps`, changed admission tests, doc-tests, and `many_pty_load_adversarial_noisy_reports_reader_backpressure`)

Production entry points:

- `BotsterEngine::try_admit_plugin` / `drain_plugin_completions` wrap `PluginWorkerEngine`.
- `botster-core-dev::run_plugin_admission_proof` is a separate-crate consumer of that facade.

## Unverified behavior or residual risk

- Hub live product callers are still scaffold disposition. First product caller is the registered Hub event-router ticket.
- `try_admit` can return `backpressured` on a busy admission lock; that is specified, but hosts must retry.
- `PluginTimerScheduler::drain_due` still uses blocking `invoke` (explicit non-scope).
- `try_admit` tests retry only typed lock-busy backpressure. A persistent lock-busy beyond 100ms still fails the test, which is intended.
- The PTY load test has been intermittently red on `main` in earlier isolated runs; it passed in this exact workspace run.

## Missing vault guidance discovered

Captured to inbox after Implement:

- `botster-core uses CI-owned Cargo commands because it has no test script`
- `Core class-aware plugin admission reserves request-response executors`
- `worker isolated now has a Core try-admit non-blocking primitive`

No Hub routing policy was captured into Core notes.

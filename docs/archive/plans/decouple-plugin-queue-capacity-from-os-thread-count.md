# Decouple Plugin Queue Capacity From OS Thread Count

Ticket: `ticket_1785199689_140456`

## Target And Context

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Pipeline run: `run_1785199752_391097`
- Run worktree: the Project Pipelines worktree for this ticket, verified by its
  `origin` remote rather than inferred from the ambient directory name.
- Repository charter: `botster-core-playbook`
- Role and surface playbooks: `planner-playbook`,
  `botster-planner-playbook`, `botster-runtime-reviewer-playbook`, and
  `botster-runtime-verifier-playbook`
- Architecture maps: `botster-architecture`, `cli-patterns`, and
  `spa-patterns`
- Targeted notes:
  - `botster plugin runtime uses supervisor plus per plugin workers`
  - `plugin hardening needs lifecycle resource and observability layers`
  - `worker isolated and non blocking are different dispatch guarantees`
  - `plugin worker watchers can block tokio runtime shutdown`
  - `plugin tests must prove worker boundaries not hub leakage`
  - `botster core contract surface needs consumer proof`
  - `botster core lua owns plugin framework primitives not product policy`
  - `botster packages should enforce core hub cli plugin provider boundaries`
  - `cold turkey migrations eliminate dual code paths and version suffixes`
  - `workspace struct field changes require workspace cargo gates`
- Repository context:
  - root and docs READMEs, workspace manifest, and `.github/workflows/ci.yml`
  - current plugin worker engine, runtime trait, actor queue contract, engine
    facades, exports, integration tests, capability isolation tests, and
    downstream `botster-core-dev` smoke harness
  - historical `core-plugin-worker-execution-engine` plan
  - current `botster-hub` `CoreEngineOptions` consumer shape on its authoritative
    `main` checkout
- Workflow evidence: run checklist `checklist_1785199910_260134`
- `project-pipelines-playbook` was not loaded because no Project Pipelines
  package path or workflow policy changes are in scope.

## Confirmed Defect

`PluginWorkerEngineConfig::default()` sets `per_plugin_capacity` from
`QueueSource::PluginWorker.default_capacity()` (256). `WorkerState::new` then
loops over that value and eagerly spawns one named OS thread and unbounded
channel per slot. Capacity is therefore simultaneously treated as a
queue/in-flight pressure limit and executor width. Four loaded plugins create
roughly 1,024 idle worker threads before plugin work begins.

The engine also stores no join handles. Load replacement, reload, unload, and
engine drop cancel in-flight tokens and call `PluginRuntime::stop`, but worker
generations retire only when their channel senders eventually disappear.
There is no explicit close-and-join lifecycle or public proof of worker
retirement.

## Scope

Implement the policy-free Core mechanism that:

1. Replaces `per_plugin_capacity` cold turkey with separate per-plugin bounded
   queue capacity and executor concurrency settings. Do not keep a deprecated
   field, alias, or dual configuration path.
2. Gives every loaded plugin its own bounded job queue and small fixed executor
   width. Queue capacity controls waiting jobs; executor concurrency controls
   simultaneously executing jobs. No thread may be allocated per queue slot.
3. Preserves synchronous invocation results, cooperative timeout/cancellation,
   capability checks, typed plugin-key backpressure attribution, and isolation
   between plugins.
4. Makes load replacement, reload, unload, and final engine drop transition a
   worker generation to stopping, reject new work, cancel queued/in-flight work,
   close its queue, stop the host runtime, and join every executor worker before
   teardown completes. A retired generation must not remain detached.
5. Adds a narrow public debug snapshot reachable through
   `PluginWorkerEngine`/the existing engine facade. It must report configured
   queue capacity, configured executor concurrency, live plugin executors, live
   executor workers, queued jobs, and in-flight jobs. Per-plugin counts must be
   available where needed to prove attribution; aggregate counts must be
   available for Hub diagnostics.
6. Updates repository-owned tests and the sibling `botster-core-dev` consumer
   proof to construct the new configuration and observe the real public
   plugin-worker path.

The implementation should use Rust standard-library synchronization and thread
primitives already used by the crate. No new dependency or async runtime is
required by the ticket.

## Non-Scope

- Choosing Botster Hub's production queue capacity or concurrency values.
- Editing `botster-hub`, package manifests, first-party plugins, Lua policy,
  Project Pipelines workflow behavior, browser/TUI presentation, or provider
  behavior in this run.
- Changing `QueueSource::PluginWorker`'s public default bounded queue metadata
  unless required to keep that metadata accurate.
- General actor, session, PTY, daemon, transport, capability-runtime, timer, or
  file-watch refactors.
- Restart/quarantine policy for misbehaving plugins.
- Compatibility aliases for `per_plugin_capacity`.

## Ownership Boundaries And Cross-Repository Seam

`botster-core` owns the reusable bounded queue, executor, cancellation,
shutdown, counters, and public configuration shape. The host-provided
`PluginRuntime` remains responsible for making `stop` unblock runtime-owned
work so Core can complete deterministic joining. Core must document that
lifecycle obligation without adding Lua or Hub policy.

`botster-hub` owns the concrete production values and startup policy. Its
current `CoreEngineOptions` has only `plugin_worker_capacity` and derives its
default from `PluginWorkerEngineConfig::default().per_plugin_capacity`; it does
not yet wire an executor-concurrency value. The downstream change is owned by
`ticket_1785200644_970622` (`Hub: wire split plugin worker queue and executor
configuration`) on the authoritative `botster-hub` target
`tgt_7e208a0c76a44980a83b63af976b1f22`. That ticket depends on this Core ticket.
The production-shaped Hub proof ticket `ticket_1785199716_875648` in turn
depends on the wiring ticket, so neither configuration migration nor runtime
proof can be skipped. This ordering does not broaden the current Core run.

Core must still provide downstream-shaped proof now. Extend the existing
`botster-core-dev` smoke consumer (a sibling crate that depends on the public
API) so a Hub-shaped pair of queue/concurrency values is passed through
`BotsterEngine::with_plugin_config`, plugin work executes, and the public debug
snapshot reports those exact values and bounded live-worker counts.

## Assumptions And Unknowns

Public contract decisions:

- Replace `PluginWorkerEngineConfig::per_plugin_capacity` with exactly:
  - `per_plugin_queue_capacity: usize`
  - `per_plugin_executor_concurrency: usize`
- Defaults are queue capacity 256 from
  `QueueSource::PluginWorker.default_capacity()` and executor concurrency 2.
  Two permits concurrent slow handlers without tying Core to CPU count, while
  four default-configured plugins create eight workers rather than 1,024.
  Hosts remain free to override both values.
- Both values must be greater than zero. `with_config` rejects zero queue
  capacity or zero executor concurrency with a clear configuration assertion;
  Core does not give zero the separate semantics of a rendezvous queue.
- Export one deterministic aggregate vocabulary:
  `PluginWorkerDebugSnapshot`, containing
  `configured_queue_capacity`, `configured_executor_concurrency`,
  `live_plugin_executors`, `live_executor_workers`, `queued_jobs`,
  `in_flight_jobs`, and sorted `plugins: Vec<PluginWorkerPluginDebugSnapshot>`.
  Each per-plugin row contains `plugin_key`, `live_executor_workers`,
  `queued_jobs`, and `in_flight_jobs`.
- Expose it as `PluginWorkerEngine::debug_snapshot()`. Existing facade
  consumers use `BotsterEngine::plugin_workers().debug_snapshot()`; do not add
  a second diagnostics API or duplicate counter vocabulary.

Assumptions:

- Queue capacity means waiting jobs, while executor concurrency means running
  jobs. Backpressure occurs when all executors are occupied and the waiting
  queue is full.
- `BackpressureSummary` remains the typed pressure contract. Its plugin-worker
  `depth` should describe queue depth, while the new debug snapshot separately
  reports in-flight jobs.
- Core remains synchronous at the caller boundary: `invoke` waits for the
  result or deadline even though execution occurs in the plugin executor.
- Executor concurrency 2 is the Core mechanism default. Hub must explicitly own
  its production override rather than deriving policy from OS CPU count.
- The existing `PluginRuntime::stop` hook is the host-runtime bridge used
  before joining. Its documentation and tests must require prompt retirement
  after cancellation/stop.

Unknowns for implementation:

- The exact internal queue primitive and lock arrangement. Prefer the smallest
  standard-library design that supports bounded nonblocking admission,
  cancellation-aware queued jobs, multiple fixed workers, close, and join.

No human decision is required for planning: the ticket explicitly permits a
bounded executor, and the fixed per-plugin executor is the smallest design
consistent with deterministic retirement and slow-plugin isolation.

## Affected Surfaces And Files

Expected:

- `crates/botster-core/src/engine/plugin_worker.rs`
  - split config fields; replace slot-per-capacity dispatch with a bounded
    per-plugin queue and fixed executor; own close/cancel/stop/join lifecycle;
    maintain debug counters and snapshots.
- `crates/botster-core/src/runtime/mod.rs`
  - tighten `PluginRuntime::stop` lifecycle documentation if needed for the
    join contract.
- `crates/botster-core/src/engine/mod.rs` and
  `crates/botster-core/src/lib.rs`
  - export only the public debug/config types required by consumers.
- `crates/botster-core/src/engine/multiplexer.rs` and
  `crates/botster-core/src/engine/botster.rs`
  - expose the snapshot through the existing facade path if
    `plugin_workers().debug_snapshot()` is not sufficient.
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
  - primary queue, concurrency, cancellation, lifecycle, and counter
    regressions; remove test fakes that intentionally leave never-returning
    detached threads.
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`,
  `crates/botster-core/tests/botster_engine_api_test.rs`,
  `crates/botster-core/tests/plugin_timer_scheduler_test.rs`,
  `crates/botster-core/tests/plugin_file_watch_runtime_test.rs`, and
  `crates/botster-core/tests/plugin_capability_isolation_under_load_test.rs`
  - cold-turkey config literal updates and only the assertions affected by the
    corrected queue/in-flight semantics.
- `crates/botster-core-dev/src/lib.rs` and
  `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - downstream-shaped public configuration and debug-counter consumption.
- `README.md`
  - document the exact public plugin-worker config/debug and lifecycle
    contract.
- `docs/architecture/non-blocking-plugin-capability-runtime.md`
  - replace ambiguous references to one "per-plugin capacity" with the
    independent plugin invocation queue capacity/executor concurrency boundary;
    distinguish invocation-queue pressure from capability-runtime queue
    capacity and depth.

Not expected:

- `crates/botster-core-daemon/**`
- `crates/botster-terminal-ghostty/**`
- package/UI/entity/transport/session protocol contracts
- any `botster-hub` file in this repository run

## Implementation Sequence

1. Define the cold-turkey config and debug vocabulary, then update all workspace
   struct literals so compilation establishes the complete impact.
2. Replace `WorkerSlot` with one per-plugin executor owner:
   - bounded queue sender/receiver,
   - explicit stopping/closed state,
   - fixed worker join handles,
   - queued and in-flight accounting,
   - cancellation-token registry.
3. Make admission atomic with queue state. Reject a stopped generation as
   `WorkerStopped`; reject a full queue as attributed `Backpressured`; decrement
   queue/in-flight counters exactly once on every completion, timeout,
   cancellation, send failure, and shutdown path.
4. Centralize teardown in one idempotent worker-generation shutdown path and
   call it from replacement, reload, unload, and final shared engine drop.
   Shutdown ordering is reject/close, cancel, runtime stop, then join.
5. Add public snapshots from live state and thread-liveness accounting. Prove
   counters return to zero after unload/reload retirement and never report queue
   slots as worker threads.
6. Extend direct engine tests, facade tests, load/isolation tests, repeated
   lifecycle tests, and the `botster-core-dev` downstream consumer.
7. Update narrow public docs and run the repository's full CI-equivalent gates.

## Risks

- A shared receiver lock can accidentally serialize execution even when
  concurrency is greater than one. Tests must prove two slow invocations run
  concurrently.
- Counting admission and execution in separate locks can exceed capacity or
  double-decrement on timeout/late completion. Counters need one clear
  transition model.
- Timed-out jobs can remain queued and execute stale work unless workers check
  cancellation before calling the runtime.
- Reload can race with a cloned old `WorkerState`; the stopping generation must
  reject dispatch and cannot publish work into the replacement generation.
- Joining before `PluginRuntime::stop` can deadlock on runtime-owned blocking
  work. The teardown order and runtime obligation must be explicit.
- Calling shutdown while holding the engine worker-map lock can deadlock
  callbacks or block unrelated plugins. Remove the generation from the map,
  release the map lock, then retire it.
- `PluginWorkerEngine` is cloneable. Final drop must occur only when the shared
  engine owner is gone, while explicit unload/reload must retire exactly the
  removed generation once.
- Crate-local tests alone would freeze an unusable public counter/config shape.
  The `botster-core-dev` consumer proof is required.

## Acceptance Checks And Tests

Behavioral tests must prove:

1. One plugin configured with queue capacity 256 and executor concurrency 2
   creates one live plugin executor and two live workers, not 256 workers.
2. Four loaded plugins with the same config report four live executors and
   eight live workers, while retaining independent queues/counters.
3. Configure queue capacity 4 and executor concurrency 1, start six caller
   threads, hold the executing job, and prove four jobs queue while the sixth
   request fails fast with `PluginWorkerEvent::Backpressure` carrying the
   correct plugin key, capacity 4, and depth 4. Because `invoke` is synchronous,
   queue depth is bounded by concurrent caller population; saturation tests
   deliberately use a small capacity below that population rather than the
   default 256.
4. Two slow invocations for one plugin overlap up to configured concurrency,
   and a saturated/slow plugin does not delay a fast neighboring plugin.
5. Timeout and explicit unload/reload cancellation signal the correct tokens;
   a cancelled queued job is skipped; late completion cannot double-release
   queue or in-flight counts.
6. Repeated load/reload/unload cycles close old queues and join workers. Live
   executor/worker counts return to the expected baseline each cycle, and old
   generations cannot accept or complete new work.
7. Dropping the final engine owner stops runtimes and joins workers before drop
   returns; no test may leave an intentional never-returning detached thread.
8. Capability rejection, descriptor/resource cleanup, timer dispatch,
   file-watch pressure, and existing plugin failure attribution remain intact.
9. `botster-core-dev` consumes the new public config and debug snapshot through
   `BotsterEngine`, proving the Hub-shaped production entry path rather than
   merely constructing types.

Manual Implement-gate evidence, not committed test code:

- Temporarily restore thread-per-capacity allocation locally, run the focused
  worker-count test, and record the exact edit plus failing assertion/output.
  Reapply the implementation before commit. This red-on-revert mutation check
  must never ship the defective 256-thread allocation in the test suite.

Repository commands:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p botster-core-daemon --no-default-features --all-targets -- -D warnings
cargo test --workspace
cargo test -p botster-core-daemon --no-default-features
cargo test --doc --workspace
cargo doc --workspace --no-deps
cargo doc -p botster-core --no-default-features --no-deps
cargo test -p botster-core --no-default-features --lib
```

Focused development evidence should include:

```text
cargo test -p botster-core --test plugin_worker_engine_test
cargo test -p botster-core --test plugin_capability_isolation_under_load_test
cargo test -p botster-core-dev --test engine_smoke_test
```

Ghostty-specific rebuild/cross-target cache evidence is not implicated because
no terminal adapter or vendored code should change. The full workspace commands
remain required because the public struct/API changes cross workspace crates.

## Vault Gaps Worth Capturing

After implementation proves the behavior, capture two durable claims if they
are not already represented:

- Plugin queue capacity and executor concurrency are independent Core
  mechanisms; host policy chooses their values.
- Retiring a plugin worker generation requires queue closure, runtime stop, and
  joined executor workers; cancellation without joining is detached lifecycle
  debt.

Do not capture the ticket's proposed design as shipped knowledge before the
implementation and negative-control evidence pass.

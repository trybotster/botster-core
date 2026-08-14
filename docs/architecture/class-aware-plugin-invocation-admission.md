# Class-aware plugin invocation admission

Plan for ticket `ticket_1786663581_723222`: add policy-free
RequestResponse vs Background admission to Core's plugin worker engine so
saturated background work cannot block or consume reserved request-response
capacity.

This is a Core mechanism ticket in the Botster Non-Blocking Event Plane.
Living design belongs here under `docs/architecture/`, not under the retired
`docs/plans/` stub. See `docs/README.md`.

Revision 3 answers Plan Review `review_1786665137_283046`. Prior findings
from `review_1786664717_240989` stay closed. It does not re-litigate
repository routing.

## Target

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Target path is the botster-core spawn target, not the ambient pipeline
  session directory.
- Current subject revision: `033cd01`
- Runtime-teardown class: does not apply
- Hub session-type eligibility parent: does not apply
- Project Pipelines package/plugin paths: out of scope

## Playbooks and notes loaded

Repository charter: [[botster-core-playbook]]

Role / map overlays:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (mixed-generation index only; ownership from the charter)
- [[spa-patterns]] (loaded per planner overlay; no SPA surface in this ticket)
- [[botster repository playbooks are ownership charters composed with role overlays]]
- [[botster-runtime-reviewer-playbook]] (downstream Review overlay for this
  runtime change)

Targeted atomic notes:

- [[worker isolated and non blocking are different dispatch guarantees]]
- [[botster plugin runtime uses supervisor plus per plugin workers]]
- [[plugin worker queue capacity and executor concurrency are independent host profile knobs]]
- [[plugin hardening needs lifecycle resource and observability layers]]
- [[plugin workers use typed mailbox handler refs not lua closures]]
- [[plugin event and http callbacks run in plugin worker vms]]
- [[botster core lua owns plugin framework primitives not product policy]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[workspace struct field changes require workspace cargo gates]]
- [[botster engine command surface uses botsterengine as facade]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core hosts need an explicit drain loop contract]]
- [[structured output fields need producer paths or explicit scaffold disposition]]
- [[test script required for rust tests not cargo test]]
- [[rust repo strict lints must be verified before dismissing warnings]]
- [[vault example paths are not repository placement conventions]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[colon worktree paths break cargo dyld library paths]]
- [[hearth gate runs require restoring a pipeline wiped gitignore before attribution]]

Planner-required pipeline overlays (process only; no package change):

- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]

Not loaded: [[project-pipelines-playbook]] (no Project Pipelines package path).
Not loaded: [[botster runtime teardown lenses]] (no WebRTC, SessionIo,
ClientWorker, or terminal-lifecycle work).

## Context loaded

Current `PluginWorkerEngine` is a policy-free per-plugin execution substrate:

- `PluginWorkerEngineConfig` has `per_plugin_queue_capacity` (default 256) and
  `per_plugin_executor_concurrency` (default 2).
- `invoke` does a non-blocking `try_send` into one FIFO `sync_channel`, then
  **blocks** on `recv_timeout`. That caller wait is the only timeout owner
  today. `PluginCancellationToken` is signalled from that wait, not from an
  engine deadline. Worker isolation is not non-blocking delivery
  ([[worker isolated and non blocking are different dispatch guarantees]]).
- Debug snapshots report configured bounds plus aggregate/per-plugin queued
  and in-flight counts. There is no class, byte, completion, pressure, or
  reserved-capacity accounting.
- Reload, unload, and Drop cancel queued and in-flight work and join executor
  threads.
- `BotsterEngine` / `MultiplexerEngine` expose `plugin_workers()` and
  `invoke_plugin` as the public production entry. Timer drain still calls
  blocking `invoke`.
- `botster-core-test-support` and `botster-core-dev` construct
  `PluginWorkerEngineConfig` and `PluginInvocationRequest` literals.
- botster-core has **no** `cli/test.sh`. Human answer
  `question_1786664489_333289` corrects the ticket: use the
  repository-documented and CI-owned Cargo commands. Do not create a
  replacement wrapper.

Sibling Event Plane tickets (do not implement here):

- Core lifecycle journal wake/page API (independent Core ticket).
- Hub session projection without blocking operation paths.
- Hub bounded package event router, which must deliver plugin handlers
  through Core Background admission.
- Later Web/TUI/package event consumption.

Hub today maps `plugin_worker_queue_capacity` and
`plugin_worker_executor_concurrency` onto Core's two config fields. Class
bounds are a source-breaking Core config expansion at `0.1.0`. Hub wiring is a
registered downstream dependency, not this run.

## Scope

Surgical change inside `PluginWorkerEngine` and its public contracts:

1. Add policy-free `PluginInvocationClass::{RequestResponse, Background}`.
2. Add class-specific waiting-queue **count** and **byte** bounds, measured
   with one stable function over the complete public request.
3. Add `try_admit(class, request)` that returns immediately:
   `queued`, `backpressured`, `rejected_budget`, or `worker_stopped`.
4. Add bounded `drain_completions(max_items, max_bytes)`.
5. Keep blocking `invoke` as the direct RequestResponse path.
6. Reserve one RequestResponse executor by default. Reject configs that cannot
   preserve that reservation **or** that leave zero Background execution
   slots.
7. Give every admitted async job a Core-owned deadline and exactly one typed
   completion, including timeout and unload.
8. Extend debug snapshots and counters with per-class depth, bytes, in-flight
   jobs, completions, pressure, and reserved capacity.
9. Update workspace literals, facade/docs, crate tests, and a separate-crate
   `botster-core-dev` consumer proof.

## Non-scope

- Hub event names, schemas, audiences, routing, package admission, or host
  control protocol.
- ClientWorker, SessionIo, terminal protocol, terminal adapters, terminal
  queues.
- Converting `PluginTimerScheduler::drain_due` from blocking `invoke` to
  Background admission.
- Capability-runtime HTTP/WebSocket/file/store changes.
- Product policy: which Hub/MCP/UI/event handlers are Background vs
  RequestResponse.
- Changing `PluginInvocationRequest` field layout just to carry class.
- New Hub Lua supervisor APIs.
- A new repository test-script wrapper.

## Repository ownership and cross-repo dependencies

Core owns reusable, policy-free plugin-worker mechanics
([[botster-core-playbook]], [[botster packages should enforce core hub cli plugin provider boundaries]]).

Hub owns which operations call `invoke` vs `try_admit(Background)`, host
defaults for class bounds, and sanitized diagnostic projection
([[plugin worker queue capacity and executor concurrency are independent host profile knobs]]).

This run stays on `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` / `botster-core`.
Do not edit Hub, Web, TUI, or Project Pipelines in this worktree.

Cross-repo prerequisite for later consumers, registered against the Hub
target `tgt_7e208a0c76a44980a83b63af976b1f22`:

- `ticket_1786663582_483898` (Hub package event router) depends on this
  ticket because it must deliver handlers through Core Background admission.
- `ticket_1786663582_169720` (Hub non-blocking session projection) depends on
  this ticket because operation paths must stop doing blocking background
  plugin invocation.

Those Hub tickets consume the Core API; they are not implemented here.

## Assumptions

1. Class is an **admission argument**, not a new required field on
   `PluginInvocationRequest`. `invoke` is RequestResponse by definition.
   Completions record the class the engine admitted under.
2. `rejected_budget` means the owned request encoding can never fit the class
   byte bound (`queue_bytes > class.queue_byte_capacity`). Current saturation
   of count, remaining class bytes, or remaining reserved completion budget
   is `backpressured`.
3. Reserved RequestResponse capacity is an **executor** reservation. Background
   jobs may wait. They must not occupy a reserved executor even if that
   executor is idle.
4. Invalid reservation configs are `reserved == 0`,
   `reserved >= per_plugin_executor_concurrency`, or any zero capacity.
   Defaults stay concurrency 2 and reserved 1, which leaves one Background
   slot.
5. Class queue bytes count the complete public `PluginInvocationRequest`
   (request id, handler, timeout, full context including metadata, and
   payload) through a **private** accounting function. Engine-private handles
   (`PluginCancellationToken`, join handles) are not host-supplied queue
   payload and are not counted. The function is not a public export.
6. `invoke` keeps today's `PluginInvocationFailureKind` set. Oversize
   RequestResponse `invoke` maps to `Backpressured`. `rejected_budget` is a
   `try_admit` result.
7. New public enums are `#[non_exhaustive]` at `0.1.0`.
8. Adding required fields to `PluginWorkerEngineConfig` is an explicit
   source-breaking change. Hub upgrades are owned by the registered Hub
   tickets.
9. Hub product callers remain a scaffold disposition. This repository still
   must prove the public non-blocking API through a **separate workspace
   crate** (`botster-core-dev`) in addition to crate-local tests.
10. Ticket test-script wording is corrected by
    `question_1786664489_333289`: use CI-owned Cargo commands with
    `BOTSTER_ENV=test`. Do not invent a wrapper.

## Unknowns

- Exact default byte capacities. Count defaults stay 256 via
  `QueueSource::PluginWorker.default_capacity()`. Byte defaults are a finite
  policy-free Core number: **1 MiB** per class queue and **1 MiB** for the
  completion reservation pool. Hosts may lower them. Do not copy Hub
  event-plane 512 KiB product numbers into Core.
- Whether `PluginWorkerMessage` needs `TryAdmit` / `DrainCompletions`
  variants. Prefer methods on `PluginWorkerEngine` plus facade accessors.
  Add mailbox variants only if actor-contract inventory tests require the
  vocabulary to stay complete. Do not add Hub event names.

## Design

### Config

Keep the two existing fields as executor width and RequestResponse count bound.
Add class/completion/reservation fields:

```text
PluginWorkerEngineConfig
  per_plugin_queue_capacity              // RequestResponse waiting count
  per_plugin_executor_concurrency        // total executor threads
  reserved_request_response_executors    // default 1
  request_response_queue_byte_capacity   // default 1 MiB
  background_queue_capacity              // default 256
  background_queue_byte_capacity         // default 1 MiB
  completion_queue_capacity              // default 256
  completion_queue_byte_capacity         // default 1 MiB
```

`with_config` rejects:

- any capacity `== 0` (existing rule, extended to new fields)
- `reserved_request_response_executors == 0`
- `reserved_request_response_executors >= per_plugin_executor_concurrency`

The last rule is required so a valid engine always has at least one
Background execution slot. `reserved == concurrency` is not a supported
"queue forever" mode.

### Byte accounting

Keep one **private** function in `plugin_worker.rs` that
`serde_json::to_vec`s the complete public `PluginInvocationRequest`. That
encoding is the class queue unit. Do not export it from the crate, prelude,
or facade ([[botster core public surface needs a narrow start here path]]).

- Serialization failure → `rejected_budget`
- `queue_bytes > class.queue_byte_capacity` → `rejected_budget`
- `queued_bytes + queue_bytes > class.queue_byte_capacity` → `backpressured`

Prove only through admission behavior: a small payload plus large
`context.metadata` is `rejected_budget` or `backpressured`. No public
accounting helper.

### Non-waiting admission

`try_admit` must not `recv`, sleep, wait on job completion, or **block on a
mutex**. Use `try_lock` on the worker registry and the per-plugin admission
state.

```text
if any required try_lock fails -> Backpressured (reason: admission lock busy)
if worker missing or stopping -> WorkerStopped
if queue_bytes > class.queue_byte_capacity -> RejectedBudget
if class queued_count >= class.queue_capacity
   or class queued_bytes + queue_bytes > class.queue_byte_capacity
   or reserved_completion_count + 1 > completion_queue_capacity
   or reserved_completion_bytes + reservation_bytes > completion_queue_byte_capacity
   -> Backpressured
else enqueue class queue, reserve one completion, arm deadline, wake
     dispatcher/deadline waiter, return Queued
```

Deterministic lock-contention proof stays in crate-private
`#[cfg(test)]` code next to `plugin_worker.rs`. That test holds the
admission mutex and calls `try_admit` on the same crate. Do not add a
public or `#[doc(hidden)]` lock-hold method. Facade and `botster-core-dev`
tests use only the public admit/drain/snapshot contracts.

`drain_completions` may take locks; it is bounded by item/byte caps, not a
second `try_admit`.

### Dispatch and reservation

Replace the single `sync_channel` FIFO with two bounded waiting queues plus a
shared executor pool. Executors may block waiting for work. Dispatch law:

- RequestResponse may use any free executor, including reserved slots.
- Background may start only when
  `in_flight_total < concurrency` and
  `in_flight_background < concurrency - reserved`.
- Prefer RequestResponse when both classes have waiting work.

Saturated Background therefore cannot consume reserved RequestResponse
capacity, including idle reserved executors. Because `reserved < concurrency`
is required, a queued Background job always has a legal execution slot.

### Core-owned deadline for admitted jobs

Blocking `invoke` keeps today's caller `recv_timeout` owner.

`try_admit` jobs have a different owner: **one deadline-waiter thread per
`PluginWorkerEngine`**. Do not use executor `wait_timeout` as the deadline
owner. Executors can all be blocked inside `PluginRuntime::invoke` while
queued jobs still need to expire.

Waiter lifecycle:

- Spawned from `with_config`.
- Sleeps until the next armed `Instant` or a condvar wake.
- Wakes on admit, job terminal, unload/reload, and Drop.
- Hosts do not have to call drain for a deadline to fire.
- On Drop: set stopping, notify, **join the waiter**, then seal any still-open
  jobs as `WorkerStopped`, then join executors. Idle Drop joins immediately.
  Drop with a future deadline does **not** wait for that deadline.

Deadline starts at successful admit (`Instant::now() + timeout_ms`).
`timeout_ms == 0` is already expired: reserve a completion, do not enqueue
for execution, and try to seal `TimedOut` before `try_admit` returns
`Queued`.

Each admitted job has one atomic terminal state: `Open` or `Sealed(result)`.
The first successful `Open -> Sealed` commit wins. Publish to the completion
mailbox only after that commit. Later deadline, unload, or handler returns
lose the CAS and publish nothing.

Consequences:

- If `TimedOut` is already sealed (and maybe drained), unload cannot replace
  it with `WorkerStopped`.
- If unload seals first, the deadline waiter cannot publish `TimedOut`.
- A late handler return after either seal is discarded.

There is no global "WorkerStopped always beats TimedOut" rule. Forced-order
tests must prove first-commit-wins.

`invoke` does not use this waiter or the completion mailbox.

### Completion reservation

Admission, not completion time, owns mailbox capacity.

Invariant:

```text
reserved_completion_count
  = queued_async + in_flight_async + undrained_async_completions
reserved_completion_count <= completion_queue_capacity
reserved_completion_bytes <= completion_queue_byte_capacity
```

At admit, build the **concrete** compact terminal outcomes for this request
(same `request_id` and handler identity that will appear on the wire):

- `TimedOut` failure
- `WorkerStopped` failure
- oversize-result `Failed` with a fixed reason
  `completion exceeded reserved byte budget`

`reservation_bytes = max(queue_bytes, timed_out_bytes, worker_stopped_bytes,
oversize_failed_bytes)`. If any of those encodings fail, admit returns
`rejected_budget`. There is no global fixture minimum.

When the real completion is encoded:

- if it fits `reservation_bytes`, publish it
- if it does not, publish the **prebuilt** oversize failure for this request
- never block an executor on the mailbox
- never drop an open job without attempting a first-commit seal

Reload, unload, and Drop seal only jobs that are still `Open`. A job whose
`TimedOut` (or handler) completion was already sealed, even if the host
already drained it, is left alone. Retiring generations still appear in
aggregate live worker counters until join.

Prove reservation with minimum and maximum correlation-field lengths
(short vs long `request_id` / handler identity) and with an oversize
handler payload that takes the prebuilt fallback.

### Blocking invoke

`invoke` remains the RequestResponse compatibility path: admit onto the RR
queue with a oneshot result sender, then `recv_timeout` as today. It does
not take a completion-mailbox reservation. Timeouts, handler failures,
backpressure, and worker-stopped keep current shapes.

### Drain

`drain_completions(max_items, max_bytes)` returns at most that many items /
encoded completion bytes and never waits for future completions. It releases
reservation as items leave the mailbox. Hosts own the poll loop
([[botster core hosts need an explicit drain loop contract]]).

### Snapshots

Extend `PluginWorkerDebugSnapshot` and `PluginWorkerPluginDebugSnapshot` with
per-class queued count/bytes, in-flight jobs, reserved completion count/bytes,
undrained completions, pressure flags or counters, and configured reserved
executor capacity. Keep existing aggregate fields.

Live snapshot values must be produced by the same runtime that executes
plugins.

### Public surface

Export only consumer contracts:

- `PluginInvocationClass`
- `PluginAdmissionResult` and the drain/completion types
- snapshot field extensions
- `PluginWorkerEngineConfig` new fields
- `BotsterEngine` / `MultiplexerEngine` `try_admit_plugin` and
  `drain_plugin_completions`
- `PluginWorkerEngine::try_admit` / `drain_completions` already reached
  through `plugin_workers()`

Do not export byte-accounting functions, lock-hold helpers, deadline-waiter
types, or terminal-state enums. `invoke_plugin` stays blocking
RequestResponse.

### Consumer proof

Charter requires downstream-shaped proof
([[botster core contract surface needs consumer proof]]). Hub stays out of
this run.

Required:

1. Crate-private tests for `try_lock` contention, per-request reservation
   with short and long correlation fields, oversize handler fallback,
   first-commit terminal ordering (deadline-first, unload-first, late
   handler), idle Drop, and Drop with a future deadline.
2. Crate tests on public `PluginWorkerEngine` / `BotsterEngine` methods for
   reservation, engine-deadline timeout, bounded drain, reload/unload, and
   metadata-driven byte backpressure (behavior only).
3. **Separate-crate consumer:** `botster-core-dev` imports the public
   facade, admits Background work, drains typed completions (including a
   timed-out slow job), and reads live snapshot class fields. Keep the
   existing blocking `invoke` smoke. No lock-hold helper.
4. Update `botster-core-test-support` only if public helper signatures
   break.

Do not claim Hub live proof. Hub event-router remains the first product
caller.

## Affected surfaces / files

- `crates/botster-core/src/contract/actor.rs` — class, admission, drain types
- `crates/botster-core/src/contract/mod.rs`, `src/lib.rs`, `src/prelude.rs` —
  exports
- `crates/botster-core/src/engine/plugin_worker.rs` — queues, reservation,
  deadline waiter, `try_admit`, `drain_completions`, snapshots, reload/unload
- `crates/botster-core/src/engine/botster.rs`, `engine/multiplexer.rs`,
  `engine/mod.rs` — facade wrappers
- `crates/botster-core/src/engine/plugin_worker.rs` `#[cfg(test)]` —
  crate-private lock contention and terminal-order tests
- `crates/botster-core/tests/plugin_worker_engine_test.rs` — public engine
  proof
- `crates/botster-core/tests/botster_engine_api_test.rs` — facade path
  without lock injection
- `crates/botster-core/tests/actor_contract_test.rs` — serde / inventory if
  new public actor types are serialized
- Workspace config literals: `plugin_timer_scheduler_test.rs`,
  `plugin_file_watch_runtime_test.rs`,
  `plugin_capability_isolation_under_load_test.rs`,
  `multiplexer_engine_api_test.rs`, `crates/botster-core-dev/src/lib.rs`
- `crates/botster-core-dev/src/lib.rs` and
  `crates/botster-core-dev/tests/engine_smoke_test.rs` — separate-crate
  Background admit/drain/snapshot consumer
- `README.md` plugin-worker section
- `docs/architecture/first-party-host-profile-primitives.md` snapshot fields
- this file

Do not touch daemon attach, SessionIo, ClientWorker, terminal crates, or Hub.

## Risks

- Single-FIFO executors cannot enforce reservation. The queue split is
  required.
- One deadline-waiter thread per engine is new Core machinery. Drop must
  join it without waiting for a future deadline, and `try_admit` must not
  join or sleep on it.
- Per-request fallback encodings must be computed at admit or admission
  cannot guarantee a typed outcome.
- First-commit terminal state means a drained `TimedOut` is not rewritten
  by later unload.
- `#[non_exhaustive]` plus new config fields will break Hub struct literals
  on upgrade. Owned by Hub consumer tickets.
- Worktree path has no `:`; `.gitignore` is intact. No `CARGO_TARGET_DIR`
  override required.

## Acceptance checks

Implement must prove the production methods, not merely that types exist.

1. Saturated Background cannot start on a reserved RequestResponse executor
   (`reserved=1`, `concurrency=2`).
2. `try_admit` returns without waiting while a slow job is in flight. A
   crate-private test holds the admission lock and observes immediate
   `backpressured`.
3. `drain_completions` honors `max_items` and `max_bytes` and leaves the rest.
4. An admitted slow job times out through the engine deadline waiter, not a
   caller `recv_timeout`. Forced order: deadline-first then unload yields
   only `TimedOut`; unload-first then deadline yields only `WorkerStopped`;
   a late handler after either seal publishes nothing more. Idle Drop and
   Drop with a future deadline join the waiter and do not hang.
5. Capacity-one completion pool: two concurrent admits, the second is
   `backpressured` until the first completion is drained. Unload of an
   still-open job yields one `WorkerStopped`. Unload after a drained
   `TimedOut` does not invent a second outcome.
6. Large `context.metadata` backpressures or rejects through public
   `try_admit` without a public accounting API. Short and long
   `request_id` / handler identities both reserve a fallback that fits.
   An oversize handler payload uses the prebuilt compact failure.
7. `reserved >= concurrency` and `reserved == 0` are rejected. A valid engine
   can start a Background job.
8. Existing `invoke` tests remain behavior-compatible.
9. Debug snapshot fields are populated by the live engine.
10. `botster-core-dev` consumer admits Background work through the public
    facade, drains a typed completion, and observes live class snapshot
    fields.

Repository verification, per `question_1786664489_333289` (ticket
correction; no replacement wrapper):

```sh
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test -p botster-core --test plugin_worker_engine_test
BOTSTER_ENV=test cargo test -p botster-core --test botster_engine_api_test
BOTSTER_ENV=test cargo test -p botster-core-dev
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
```

Workspace clippy/test are mandatory because `PluginWorkerEngineConfig`
literals exist in sibling crates.

## Vault gaps worth capturing

- botster-core has no `cli/test.sh`. Human answer
  `question_1786664489_333289` authorizes README/CI Cargo commands with
  `BOTSTER_ENV=test` for this ticket. Capture after Implement if the
  correction is durable beyond this run.
- Worker isolation vs non-blocking delivery now has a Core `try_admit`
  primitive plus a Core-owned deadline; after Implement, update
  [[worker isolated and non blocking are different dispatch guarantees]].
- Class-aware plugin admission (reserved RequestResponse executor, class
  count/byte queues, reserved completions, engine deadline) is not yet its
  own vault note.

Do not capture Hub routing policy into Core notes. No inbox capture at Plan
time; the API is still unproven.

## Plan Review findings addressed

Revision 1 → 2 (`review_1786664717_240989`): timeout owner, completion
reservation, full-request bytes, `try_lock`, `reserved < concurrency`,
`botster-core-dev` consumer, `question_1786664489_333289`.

Revision 2 → 3 (`review_1786665137_283046`):

| Finding | Resolution |
|---|---|
| Completion fallback size is not guaranteed | At admit, encode the concrete TimedOut, WorkerStopped, and oversize-failure outcomes for this request. Reserve the max of those encodings and the request encoding. Test short and long correlation fields plus an oversize handler result. |
| Deadline and teardown ordering is inconsistent | One deadline-waiter thread per engine. First `Open -> Sealed` commit wins. Drained TimedOut is not rewritten. Forced-order and Drop tests required. |
| Test/accounting helpers expand the public API | Byte accounting stays private. Lock hold stays in `#[cfg(test)]` crate code. Export only class, admission, completion, snapshot, and facade contracts. |

# Bound PTY Output Queues And Surface Backpressure

Ticket: `Bound PTY output queues and surface backpressure`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `ticket_1780361145_500990`, run `run_1780370113_799140`, current step `botster_plan`, current run step `run_step_1780371227_267982`, gate `botster_plan_gate`.
- Ticket: `Bound PTY output queues and surface backpressure`.
- Ticket description: implementation target is the repository root; replace unbounded local PTY reader queues with bounded per-session buffering or equivalent host-configurable capacity/drop/coalescing policy; noisy PTYs must not grow memory without bound when engine/hub drains slowly; surface overflow/backpressure as typed runtime events or errors embedders can observe without blocking core hot paths; preserve ordering for retained data and make drop/coalescing semantics explicit; tests must prove bounded memory behavior, event emission, quiet sessions continuing while one session overflows, existing local PTY tests passing, and no PII.
- Dependency loaded from context: `ticket_1780361144_556547` / `Add many-PTY load harness and hot-path budgets` is closed.
- Recent events loaded: run created against base ref `main`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, and current Plan agent linked to session `sess-1780370130-0013-1246bfc781900ad136537935946d5a10`.
- Reviews/findings/questions loaded: Plan Review `review_1780371202_119287` returned `changes_required` with three open findings:
  - `finding_1780371202_695534`: pressure signal cannot travel through the same bounded byte queue it reports as full.
  - `finding_1780371202_965489`: `BackpressureSummary.depth` is not available from `std::sync::mpsc::sync_channel`; depth needs explicit accounting or defined semantics.
  - `finding_1780371202_679296`: `QueueSource::SessionIo.default_capacity() == 512` conflates worker-mailbox message count with 8KB PTY reader chunks and yields roughly 4MB/session.
- Open questions loaded: none. Prior answers loaded: none.
- Gate prompt loaded: attach plan with context loaded, scope/non-scope, assumptions/unknowns, affected surfaces/files, risks, acceptance checks/tests, and vault gaps.
- Botster inbox loaded with `receive_messages`: upstream agent requested refreshing this worktree from current `main` because the baseline/load harness ticket merged after this run spawned.
- Worktree refresh completed before finalizing this plan: stashed local changes, fetched `origin main`, rebased `project-pipelines/ticket_1780361145_500990` onto `origin/main`, and re-applied the stash cleanly. Current HEAD is `eebd7a6` / `Merge pull request #49 from trybotster/project-pipelines/ticket_1780361144_556547`.
- Current step from pipeline context and prompt: `botster_plan`.
- Current worktree from environment: the pipeline-provided ticket worktree.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required vault/project notes loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- Ticket-specific vault notes loaded:
  - `sessionioworker is the production read path for session pty output`
  - `botster hub event storms must be rejected before queues grow unbounded`
  - `botster hub events use bounded priority lanes instead of unbounded queue fuses`
  - `backpressure-recovery-uses-broker-snapshots-not-empty-shadow-screen`
  - `backpressure recovery tests must cover empty and failed snapshot branches`
  - `pty-output-observers-must-fire-inline-not-deferred`
- Repo context loaded:
  - `README.md`: the dependency added many-PTY load harness documentation. The current public default-engine path explicitly does not expose queue-depth, backpressure, or slow-client/plugin-pressure counters; the harness reports that limitation.
  - `crates/botster-core/src/runtime/local_process.rs`: default local PTY runtime uses `std::sync::mpsc::channel()` in `spawn_reader`, creating an unbounded per-session PTY reader output queue.
  - `crates/botster-core/src/runtime/mod.rs`: public `SessionRuntimeOutput` currently has PTY output and process-exit variants only; no runtime backpressure output exists.
  - `crates/botster-core/src/contract/actor.rs`: public `QueueSource`, `BackpressureSummary`, `BackpressureRoute`, and `MailboxSendFailure` already define typed bounded-queue pressure contracts. `QueueSource::SessionIo` default capacity is 512.
  - `crates/botster-core/src/engine/managed_session_runtime.rs`: production managed runtime drains `SessionRuntimeOutput` through `DefaultBotsterEngine`/`ManagedSessionRuntime::drain_runtime_once`, records output in terminal state, and fans it out through the session worker and subscription multiplexer.
  - `crates/botster-core/src/engine/session_worker.rs`: session worker preserves snapshot-before-live-output ordering and already reports worker-side mailbox failures in `SessionWorkerOutcome`, but the current runtime reader queue is outside that fake mailbox path.
  - `crates/botster-core/tests/local_process_runtime_test.rs`: Unix, feature-gated local PTY runtime tests are the right place to prove the real local runtime path.
  - `crates/botster-core/tests/managed_session_runtime_test.rs`: tests prove runtime drains reach the subscription multiplexer and terminal state through the production managed runtime entry point.
  - `crates/botster-core/tests/session_worker_engine_test.rs` and `crates/botster-core-test-support/src/fake/session_worker.rs`: examples for typed queue-full/closed failures and route assertions.
  - `crates/botster-core-test-support/src/conformance/mod.rs`: many-PTY load harness uses `DefaultBotsterEngine`, spawns many explicit local PTY sessions, drains round-robin, records delivered bytes/process exits, and currently returns a placeholder queue/backpressure observation saying the public path lacks those counters.
  - `crates/botster-core-test-support/tests/downstream_conformance_test.rs`: contains `many_pty_load_default`, `many_pty_load_adversarial_noisy_reports_missing_slow_client_primitive`, and ignored `many_pty_load_100`.

Project Pipelines checklist discipline:

- Vault/project notes constraining the plan are listed above.
- Convention conflicts: none. The plan keeps terminal bytes in the session/runtime data plane, uses existing typed backpressure vocabulary, avoids product policy, avoids broad refactors, and leaves client transport backpressure with client workers/adapters.
- Verification evidence required from implementation is listed under Acceptance Checks.
- Durable knowledge capture need is listed under Vault Gaps.
- Project Pipelines checklist instructions loaded.
- Run-level vault checklist created: `checklist_1780370854_274634`.
- Checklist evidence completed for vault notes loaded, convention conflict check (`none`), Plan-step verification/worktree-refresh evidence, and durable capture decision.
- Revision after Plan Review: updated this artifact to require an out-of-band pressure signal, explicit queue-depth accounting, and a dedicated PTY reader chunk-capacity constant.

## Scope

Bound the default local PTY reader output queue and surface pressure as typed backpressure without dropping PTY bytes or moving transport policy into core.

In scope:

- Replace the unbounded `mpsc::channel()` used by `LocalProcessRuntime::spawn_reader` with a bounded PTY byte chunk queue.
- Add a dedicated named PTY reader chunk queue capacity constant. Do not reuse `QueueSource::SessionIo.default_capacity()` as the chunk queue depth, because that value describes worker-mailbox message count, not 8KB PTY reader chunks.
- Use `QueueSource::SessionIo` only as the pressure summary source label, because the pressure is session-I/O scoped.
- Preserve byte delivery semantics: PTY output should block the reader thread under pressure rather than silently drop terminal bytes.
- Surface pressure as a typed `BackpressureSummary` with `source = QueueSource::SessionIo`, `capacity`, `depth`, and `BackpressureRoute { session_id: Some(...), client_id: None, subscription_id: None, plugin_key: None }`.
- Surface that pressure through an out-of-band signal path, not through the same bounded PTY byte queue. The byte queue is the blockable bulk lane; pressure is the control signal that must remain observable when the byte queue is full.
- Track PTY reader queue depth explicitly with manual accounting, because `std::sync::mpsc::sync_channel` does not expose queue length.
- Route that pressure through the public managed-runtime outcome path so hosts using `ManagedSessionRuntime::drain_runtime_once` or `DefaultBotsterEngine` can observe it.
- Update the many-PTY load harness to report real queue/backpressure observations instead of the current placeholder limitation when the implementation exposes them.
- Coalesce pressure reports enough that a full queue does not enqueue unbounded backpressure chatter.
- Add focused tests for the bounded reader behavior and the production managed-runtime observation path.
- Add a noisy-session regression that proves quiet sessions still drain and exit while one session experiences reader queue pressure.

Non-scope:

- No browser/WebRTC/TUI transport queue policy changes.
- No hub event lane changes.
- No backpressure recovery snapshot implementation; existing recovery notes only constrain tests if the implementation touches recovery paths, which this plan does not require.
- No client-side replacement snapshot policy.
- No dropping, lossy coalescing, compression, or scrollback trimming of PTY output unless the implementation explicitly documents retained-data ordering and drop/coalescing semantics in the public pressure event. Prefer blocking/backpressure for retained bytes over lossy behavior.
- No command-line flags or broad runtime configuration knobs. A private constructor/options seam or test-only helper for small PTY reader capacity is acceptable if needed to force pressure deterministically.
- No broad redesign of `SessionRuntime`, `SessionWorkerEngine`, `SubscriptionMultiplexer`, or terminal snapshot semantics beyond the minimal typed surface needed to expose runtime reader pressure.

Botster layers touched:

- Rust core local PTY runtime: primary.
- Rust managed session runtime / default engine outcome surface: primary if current public output types cannot carry pressure.
- Rust session/actor contracts: possible minimal public type extension for runtime-originated backpressure.
- Rust test-support many-PTY load harness: primary acceptance surface for noisy-vs-quiet behavior.
- Rust tests and test-support fakes: focused.
- Docs: this plan artifact only unless implementation changes a public contract that needs README/API docs.

Worktree and target assumptions:

- Implementer must work only in this pipeline-assigned worktree.
- Pipeline target id from current context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The branch has already been refreshed onto `origin/main` after the dependency merge. Do not reintroduce pre-refresh assumptions about the load harness being absent.

Pipeline gates/artifacts:

- Plan artifact: `docs/archive/plans/bound-pty-output-queues-and-surface-backpressure.md`.
- Gate evidence should cite this artifact, the worktree refresh, and the exact commands run by the implementer.
- Advancement target: `botster_plan_review`.

## Assumptions And Unknowns

Assumptions:

- "PTY output queues" refers first to the default local PTY runtime reader queue in `LocalProcessRuntime::spawn_reader`, because that is the concrete unbounded queue in the repo's production local PTY path.
- Backpressure should be host-visible typed pressure, not a fatal runtime output error, because existing contracts already distinguish queue pressure from ordinary I/O failure.
- Blocking the reader thread once the bounded queue is full is acceptable as the default retained-byte behavior: it bounds memory while letting the OS PTY buffer apply natural backpressure to the child process. If the implementer chooses drop/coalescing instead, the event semantics and retained-data ordering must be explicit and tested.
- `QueueSource::SessionIo` is the correct pressure source label for the per-session PTY reader, but its default capacity is not the production chunk queue depth.
- `BackpressureSummary.depth` must come from explicit accounting. If the implementation cannot maintain exact depth cheaply, it must define a conservative pressure-episode semantic such as `depth = capacity` at the point a send observes a full queue.
- It is acceptable to add a minimal public variant or observation if no existing public path can surface runtime-originated backpressure.
- The dependency load harness is now the right acceptance vehicle for multi-session noisy pressure, not a future scaffold.
- Pressure signaling follows the same architecture rule as bounded hub lanes: bulk PTY bytes can block under pressure, while pressure notification must use a separate control path.

Unknowns for implementation:

- Whether the smallest public surface is `SessionRuntimeOutput::Backpressure(BackpressureSummary)`, a new `MultiplexerEngineObservation` variant, or a session-worker outcome extension. The implementer should choose the least invasive path that reaches `ManagedSessionRuntime::drain_runtime_once` without conflating pressure with client egress.
- Whether the best out-of-band signal is a dedicated small control channel or shared per-session atomics (`AtomicUsize` depth plus `AtomicBool`/counter for pressure episodes) that `drain_reader_output` reads independently of the byte queue.
- Whether strict clippy requires a named constant for the PTY output queue depth and a small internal helper to keep `spawn_reader` readable.
- Whether the many-PTY harness should expose typed backpressure summaries directly in `ManyPtyLoadReport` or convert them to stable strings for current test output. Prefer typed data internally and string formatting only at the report boundary.
- Exact production chunk capacity is a judgment call. It should be chosen against the 8192-byte reader buffer and documented as a memory ceiling, for example `capacity * 8192` bytes per session.

No human question is currently blocking planning. The ticket has one plausible implementation target after repo inspection: bound the real local PTY reader output queue and surface typed pressure through the managed runtime path.

## Affected Surfaces / Files

Expected changes:

- `crates/botster-core/src/runtime/local_process.rs`
  - Pass `SessionId` into `spawn_reader`.
  - Replace unbounded `mpsc::channel()` with a bounded queue sized by a dedicated PTY reader chunk-capacity constant.
  - Add explicit queue-depth accounting around successful enqueue/dequeue, or define and test a conservative `depth = capacity` pressure-episode report if exact depth is not maintained.
  - Add an out-of-band pressure signal that is not sent through the saturated byte queue. Acceptable shapes are a small dedicated control channel or shared per-session atomics/counters read by `drain_reader_output`.
  - Add a bounded send helper that preserves PTY bytes, records one typed pressure summary per full-queue episode out-of-band, and stops cleanly if the receiver is closed.
  - Do not extend `ReaderEvent` with a pressure event carried on the PTY byte queue; that is the rejected Plan Review design because a full queue cannot enqueue its own pressure report.
- `crates/botster-core/src/runtime/mod.rs`
  - Possible minimal extension to `SessionRuntimeOutput` for runtime-originated backpressure if the pressure must cross the public runtime boundary.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Map runtime-originated backpressure into `MultiplexerEngineOutcome` so the real managed runtime path surfaces it.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Possible minimal `MultiplexerEngineObservation` variant if no existing observation can carry runtime pressure.
- `crates/botster-core/src/lib.rs`
  - Re-export any newly added public contract type or enum variant only as needed.
- `crates/botster-core/tests/local_process_runtime_test.rs`
  - Add a focused Unix/local-runtime regression that proves a bounded reader reports pressure under a forced-small internal capacity or helper.
- `crates/botster-core/tests/managed_session_runtime_test.rs`
  - Add a production-entry regression showing pressure from runtime drain reaches `ManagedSessionRuntime::drain_runtime_once` outcome observations.
- `crates/botster-core/tests/botster_engine_api_test.rs`
  - Add or adjust a facade-level assertion only if the pressure surface is meant to be visible through `DefaultBotsterEngine`.
- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Replace the placeholder "public path does not expose queue-depth/backpressure" report with real observations when available.
  - Keep the harness PII-safe: synthetic ids, counts, bytes, durations, and pressure summaries only.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Update `many_pty_load_adversarial_noisy_reports_missing_slow_client_primitive` or replace it with a regression that proves noisy-session pressure is observed and quiet sessions still drain.
- `README.md`
  - Update the many-PTY harness description only if the implementation changes the report contract from "limitation" to real queue/backpressure observations.

Likely unchanged:

- `crates/botster-core/src/engine/subscription_multiplexer.rs`
- `crates/botster-core/src/contract/client_stream.rs`
- `crates/botster-core/src/engine/plugin_worker.rs`
- Browser/TUI/client transport code, because this ticket targets the PTY runtime output queue, not slow-client egress queues.

## Risks

- Treating queue pressure as `OutputFailed` would hide the typed backpressure contract and risk shutting down healthy sessions during transient slow drains.
- Dropping PTY output to make room would violate terminal correctness and could corrupt shadow terminal state.
- Emitting a backpressure event for every chunk while full could recreate the unbounded queue problem with pressure chatter.
- Sending pressure through the same bounded byte queue is non-working: when the queue is full, the pressure event blocks behind the saturation it is supposed to announce.
- Relying on `sync_channel` for queue depth is non-working: std mpsc exposes no queue length.
- Blocking the reader while holding a registry mutex would deadlock drains; the bounded send must happen in the reader thread outside registry locks.
- Borrowing `QueueSource::SessionIo.default_capacity()` for chunk depth creates a poorly justified memory ceiling; a dedicated constant must make the per-session bound explicit.
- Adding a new broad queue source or public configurable capacity would be more API than the ticket requires.
- Updating only contract tests would not prove the real runtime path changed; implementation must test or otherwise evidence the `LocalProcessRuntime` reader path.
- Backpressure route context must stay session-scoped. Inventing client or subscription ids at the runtime reader boundary would violate the SessionIo/ClientWorker split.
- Leaving the many-PTY harness placeholder intact would undercut the ticket's acceptance because the dependency exists specifically to observe this class of pressure.
- Host-configurable/drop/coalescing policy can sprawl quickly; keep it to the smallest mechanism required to bound memory and surface semantics.

## Acceptance Checks / Tests

Required focused checks:

- `cargo test -p botster-core --test local_process_runtime_test --features local-runtime`
  - Must include a regression that forces or simulates reader queue pressure and asserts a typed `BackpressureSummary` with `QueueSource::SessionIo`, configured capacity/depth semantics, and the source `SessionId`.
  - Must prove pressure is observable even while the byte queue is full, which means the signal is not carried as a `ReaderEvent` on the saturated byte queue.
  - Must prove depth accounting is explicit or that `depth = capacity` pressure-episode semantics are documented and tested.
- `cargo test -p botster-core --test managed_session_runtime_test`
  - Must prove the production managed-runtime drain path surfaces the pressure outcome, not only that the local helper exists.
- `BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_default -- --nocapture`
  - Must show the default 20-session load harness still passes.
- `BOTSTER_ENV=test BOTSTER_CORE_LOAD_SESSIONS=50 cargo test -p botster-core-test-support many_pty_load_default -- --nocapture`
  - Must show the normal 50-session pressure check still passes.
- `BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_adversarial_noisy_reports_missing_slow_client_primitive -- --nocapture`
  - If the test is renamed, run the replacement adversarial noisy-session test. It must prove overflow/backpressure event emission and quiet sessions continuing to drain while one session pressures.
- `cargo test -p botster-core --test botster_engine_api_test`
  - Required if `DefaultBotsterEngine` is the intended public facade for the surfaced pressure.
- `cargo test -p botster-core --test session_worker_engine_test`
  - Required if `SessionWorkerOutcome`, mailbox failure propagation, or session-worker backpressure handling changes.
- `cargo test -p botster-core-test-support`
  - Required if fake runtime/mailbox helpers change.

Required repo-level checks:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

Runtime/user-path proof:

- Evidence must identify the production path as:
  - `DefaultBotsterEngine` or `ManagedSessionRuntime`
  - draining `LocalProcessRuntime`
  - receiving bounded reader pressure from `LocalProcessRuntime::spawn_reader`
  - returning typed pressure in the host-visible outcome.
- Evidence that a helper or enum exists is not sufficient. The many-PTY harness report should show real queue/backpressure observations or typed summaries after the implementation.

## Implementation Shape

Suggested sequence:

1. Add a bounded reader queue helper in `local_process.rs`.
2. Pass `session_id.clone()` into `spawn_reader`.
3. Use bounded send semantics:
   - Define a dedicated PTY reader chunk queue capacity constant sized against the 8192-byte read buffer.
   - Try to enqueue output into the bounded byte queue.
   - Maintain depth with explicit accounting around successful enqueue/dequeue, or report `depth = capacity` at full-queue observation if exact depth is intentionally not tracked.
   - If the byte queue is full, record one typed backpressure report for the pressure episode through an out-of-band control path, then block to preserve the output bytes.
   - Reset the pressure-episode flag after a later non-pressured send succeeds or after drain observes the pressure, whichever keeps the report coalesced and deterministic.
   - Break cleanly on disconnected receiver.
4. Extend the minimal public runtime/engine outcome surface so managed runtime callers can observe the pressure.
5. Add tests in the smallest harness that can deterministically force pressure without waiting for a real process to emit hundreds of chunks.
6. Add one managed-runtime production-path test that proves the new pressure output is surfaced through `drain_runtime_once`.

## Vault Gaps Worth Capturing

- Capture after implementation: `local process PTY reader queues must be bounded and pressure must be typed`.
- The note should record the concrete queue capacity, whether pressure blocks or drops output, and the public outcome surface used by managed/default engines.
- If the implementation adds a new observation variant, capture why runtime-originated backpressure is distinct from client transport backpressure and session worker request mailbox failures.

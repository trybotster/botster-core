# Terminal Metadata Shaping And Drop Diagnostics

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `ticket_1782862717_308093`, run `run_1782929421_208186`, step `botster_plan`, run step `run_step_1782929422_440064`, gate `botster_plan_gate`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Dependency context: `Port OSC semantic metadata producer wiring into botster-core session worker` is closed.
- Required playbooks and vault notes loaded: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[project pipelines checklist worker timeouts require artifact evidence fallback]], [[rust repo strict lints must be verified before dismissing warnings]], [[workspace struct field changes require workspace cargo gates]].
- Repo context inspected:
  - `crates/botster-core/src/contract/actor.rs` already defines `SessionIoCoalescingPolicy`, `SessionIoOrderedEvent`, typed mailbox failures, backpressure routes, and delivery lag/failure structs.
  - `crates/botster-core/src/contract/terminal_metadata.rs` contains the OSC metadata producer.
  - `crates/botster-core/src/engine/session_worker.rs` flushes pending initial output before metadata/control events, preserving snapshot/live-output ordering.
  - `crates/botster-core/src/engine/subscription_multiplexer.rs` has typed delivery lag and delivery failure observations.
  - `crates/botster-core/src/bin/botster-session-worker.rs` currently emits raw PTY output and metadata through one bounded egress channel; `try_send` on a full channel drops silently.
  - `crates/botster-core/src/runtime/worker_process.rs` currently counts parent-side egress overflow as `SessionRuntimeOutput::Backpressure`, but worker-side send drops are not typed.
  - Existing tests cover metadata production, mailbox/coalescing basics, subscription diagnostics, and worker process behavior.
- Project Pipelines checklist: run checklist `checklist_1782929463_493499` was created. Items 1 and 2 were marked done with vault context and no convention conflicts.

## Botster Layers Touched

- Rust `botster-core` contracts and pure engines.
- Local session worker process and parent-side worker runtime adapter.
- Rust tests only.

No Lua plugin, hub policy, TUI, React SPA, Rails relay, WebRTC/DataChannel/browser admission, or package workflow surfaces should be touched.

## Scope

- Make terminal metadata shaping explicit in core-owned types, preferably adjacent to `SessionIoCoalescingPolicy` / terminal metadata contracts:
  - model metadata lane outcomes as typed values for accepted/latest-win/field-merge/dedup/rate-limit/drop decisions;
  - expose counters or observations with typed route/lane context instead of string-only diagnostics;
  - keep ordering-significant metadata represented by `SessionIoOrderedEvent` and preserve output-flush barriers before those events.
- Replace or wrap silent worker egress drop-on-full behavior:
  - classify worker egress into terminal/control and terminal-metadata lanes;
  - ensure metadata floods cannot consume capacity needed for terminal bytes, control, health, process exit, or shutdown frames;
  - record structured counters/observations for metadata coalesce/drop/rate-limit outcomes and egress drop outcomes.
- Keep all queues bounded:
  - no unbounded `mpsc::channel` replacement for hot terminal or worker egress paths;
  - protected lanes remain lossless where the existing contract requires lossless delivery or backpressure rather than silent drop.
- Prove the production path changed:
  - tests must exercise `botster-session-worker` / `WorkerProcessRuntime`, not only pure helper functions;
  - tests should show terminal bytes/control still flow while metadata is flooded.

## Non-Scope

- No hub, browser, WebRTC/DataChannel, Rails, TUI, package admission, or plugin workflow changes.
- No new dependency/gem/crate for queues or rate limiting unless implementation discovers an existing in-repo primitive is insufficient and Plan Review accepts that justification.
- No broad rewrite of `SessionRuntime`, `ManagedSessionRuntime`, subscription fanout, or encrypted stream contracts unless a touched public type must carry the new typed observations.
- No PII-bearing diagnostics. Do not put raw cwd/title/prompt/body contents into counters or route diagnostics beyond existing metadata event payloads.

## Assumptions And Unknowns

- Assumption: the closed OSC dependency means metadata producer wiring is available and should be hardened, not reimplemented.
- Assumption: `botster-session-worker` is the production worker path that must stop silently dropping full egress sends.
- Assumption: metadata shaping can be implemented transport-neutrally in `botster-core` and its worker process without hub/browser admission logic.
- Unknown: whether the cleanest implementation is a pure reusable metadata-lane shaper in `contract/terminal_metadata.rs` or a worker-egress helper near `botster-session-worker`. Prefer the smallest shared pure type that lets tests assert outcomes without turning the worker binary into policy-only code.
- Unknown: exact rate-limit threshold. If no existing threshold exists, introduce the smallest explicit default tied to `SessionIoCoalescingPolicy` timing/capacity and document/test it. Do not add user configurability unless required.
- No human question is currently blocking planning; the ticket intent is specific and no acceptance item needs waiver.

## Affected Surfaces / Files

Likely implementation files:

- `crates/botster-core/src/contract/actor.rs`
  - Extend or reference typed lane outcome/counter contracts near `SessionIoCoalescingPolicy`, `QueueSource`, `BackpressureSummary`, and `MailboxSendFailure`.
- `crates/botster-core/src/contract/terminal_metadata.rs`
  - Add a pure metadata lane shaper/counter if that is the narrowest home for latest-win/dedup/field-merge/rate-limit/drop semantics.
- `crates/botster-core/src/contract/mod.rs`
  - Re-export any public typed outcome/counter structs used by tests or runtime adapters.
- `crates/botster-core/src/bin/botster-session-worker.rs`
  - Replace raw `try_send` helpers with a small egress helper that applies metadata shaping and typed drop diagnostics while preserving terminal/control priority.
- `crates/botster-core/src/runtime/worker_process.rs`
  - Decode/report structured worker egress diagnostics, parent-side overflows, and metadata shaping observations as `SessionRuntimeOutput` or existing observation surfaces.
- `crates/botster-core/src/runtime/local_process.rs`
  - Touch only if the same typed observation shape must also represent in-process local runtime pressure; avoid broad parity work if worker process is the only silent-drop path.

Likely tests:

- `crates/botster-core/tests/session_io_mailbox_test.rs`
  - Pure policy tests for metadata outcome typing, counters, boundedness, and ordered flush preservation.
- `crates/botster-core/tests/terminal_metadata_producer_test.rs`
  - Add shaper tests only if the shaper lives with the producer.
- `crates/botster-core/tests/local_session_worker_process_test.rs`
  - Production path tests proving metadata flood cannot starve PTY output/control and that typed observations/counters are emitted.
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
  - Only if delivery failure/lag observation shape changes.
- `crates/botster-core/tests/session_worker_protocol_contract_test.rs`
  - Add/update protocol fixture tests if a worker frame or public contract enum changes.

## Implementation Shape

1. Add a typed metadata lane outcome model.
   - Include variants or equivalent typed counters for latest-win/coalesced, field-merged, deduplicated, rate-limited, dropped, and accepted.
   - Keep payload-free diagnostics for counts/reasons; do not leak raw title/cwd/notification content into diagnostic summaries.

2. Add a bounded worker egress helper.
   - Separate protected terminal/control sends from lossy metadata sends, or otherwise guarantee protected sends are not dropped because metadata occupied capacity.
   - Use latest-win semantics for high-churn metadata such as title/cwd/mode-like state.
   - Dedup identical repeated metadata where possible.
   - Preserve ordering barriers: when metadata is ordering-significant, flush already accepted terminal bytes before the metadata event.
   - On drop/full/rate-limit, increment typed counters and expose them through the existing runtime observation/backpressure path.

3. Wire diagnostics into the parent runtime.
   - Decode worker-side diagnostic frames or equivalent typed reports.
   - Preserve existing parent-side `BackpressureSummary` for parent queue overflow.
   - Ensure `QueueSource::SessionIo` and `BackpressureRoute` stay populated with session id and no client/subscription when the boundary is worker egress.

4. Update tests before or with implementation.
   - Start with pure shaper tests for deterministic counter behavior.
   - Add worker process tests with tiny metadata capacity and a script that floods OSC metadata while also emitting PTY bytes and responding to ping/shutdown.
   - Keep assertions on bounded retained events and typed counters.

## Risks

- Starvation risk: a naive single bounded queue with `try_send` can still allow metadata to consume capacity needed by terminal/control frames.
- Ordering risk: latest-win metadata must not move ordering-significant events ahead of terminal bytes that preceded them.
- Public API risk: new required fields on public structs require workspace-wide cargo gates and grep for struct literals.
- Flaky test risk: worker-process flood tests must avoid relying on exact dropped counts; assert invariant ranges and existence of typed observations.
- Diagnostic payload risk: titles, cwd, prompts, and notifications can contain sensitive data; counters should carry kinds/reasons/counts, not raw values.
- Over-abstraction risk: avoid building a general transport scheduler. This ticket needs explicit terminal metadata lane shaping and diagnostics only.

## Acceptance Checks / Tests

Focused tests expected after implementation:

- `cargo test -p botster-core --test session_io_mailbox_test`
  - proves pure coalescing/shaping contracts, ordered flush barriers, and bounded policy.
- `cargo test -p botster-core --features local-runtime --test local_session_worker_process_test`
  - proves the actual worker process path emits metadata diagnostics and does not let metadata flood starve terminal/control paths.
- `cargo test -p botster-core --test terminal_metadata_producer_test`
  - proves producer behavior remains intact if producer/shaper code is touched.
- `cargo test -p botster-core --test subscription_multiplexer_engine_test`
  - required if multiplexer delivery observation shapes change.
- `cargo test --workspace`
  - required if any public contract or struct shape changes across crates.
- `cargo clippy --workspace --all-targets -- -D warnings`
  - required by loaded Rust lint convention.

Runtime/user-path proof:

- At least one test must spawn `CARGO_BIN_EXE_botster-session-worker` through `WorkerProcessRuntime` or `DefaultBotsterEngine::worker_backed`, flood metadata, then prove PTY output and a control path such as ping/shutdown still work while typed observations/counters report metadata shaping/drop behavior.

## Vault Gaps Worth Capturing

- No durable vault gap is mandatory from planning alone.
- Capture after implementation if a new reusable convention emerges, especially one of:
  - worker egress should use explicit protected/lossy lanes rather than a single `try_send` queue;
  - terminal metadata diagnostics must be payload-free because OSC values can carry PII;
  - metadata ordering barriers need a specific shaper pattern reusable across worker and non-worker runtimes.

## Checklist Evidence

- Vault notes constrained the plan: listed in Context Loaded.
- Convention conflicts: none.
- Verification evidence planned: commands listed in Acceptance Checks; production-path proof required through worker-backed runtime.
- Durable knowledge capture: none yet; re-evaluate after implementation.

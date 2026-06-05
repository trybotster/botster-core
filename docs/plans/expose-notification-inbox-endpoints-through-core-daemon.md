# Expose Notification Inbox Endpoints Through Core Daemon

## Context Loaded

- Pipeline context: ticket `ticket_1780680552_512741`, run `run_1780680559_853113`, step `botster_plan`, gate `botster_plan_gate`.
- Required playbooks: [[planner-playbook]] and [[botster-planner-playbook]].
- Vault constraints: [[identity]], [[goals]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], and [[plan agents must author vault context as wikilinks not home paths]].
- Repo context:
  - `crates/botster-core/src/contract/notification.rs` already defines `NotificationInbox`, `NotificationItem`, `NotificationTarget`, delivery status, expiry, drain, drop, and acknowledge behavior.
  - `crates/botster-core/src/engine/multiplexer.rs` already exposes `post_notification` and `drain_notifications`.
  - `crates/botster-core/src/engine/botster.rs` exposes notification post/drain on `DefaultBotsterEngine`, but `WorkerBackedBotsterEngine` exposes only session/client/PTY methods and has no notification surface.
  - `crates/botster-core/src/engine/routed_envelope.rs` already owns generic cursor, ack, bounded queue, and backpressure mechanics through `RoutedEnvelopeRouter`.
  - `crates/botster-core-daemon/src/api.rs` and `src/daemon.rs` expose typed session, subscription, output drain, guarded write, health, adoption, and shutdown APIs, but no notification or routed-envelope endpoint. `CoreDaemon` wraps `DaemonEngine::Local(DefaultBotsterEngine)` or `DaemonEngine::Worker(WorkerBackedBotsterEngine)`.
  - `docs/architecture/core-daemon.md`, `docs/architecture/engine-command-surface.md`, and `docs/architecture/routed-envelope-primitive.md` document that core owns policy-free mechanics while hosts/plugins own meaning, presentation, auth, persistence, and workflow policy.

## Scope

Add a core-daemon API surface that lets hub-native coordination tools reach existing core notification/envelope mechanics through `botster-core-daemon::CoreDaemon`.

In scope:

- Add typed daemon request/result structs for posting notification items, draining/receiving notifications, acknowledging notification ids, and querying notification status where supported by the existing notification inbox contract.
- Add typed daemon request/result structs or methods for cursor-aware routed-envelope delivery where cursor, bounded queue, slow-consumer, and per-target ack semantics are required.
- Store the necessary in-memory core primitives inside `CoreDaemon` without introducing hub/product policy.
- Re-export the new daemon API types from `crates/botster-core-daemon/src/lib.rs`.
- Add daemon integration tests proving the runtime path through `CoreDaemon`, not only direct `botster-core` primitives.
- Update daemon docs to describe the API as generic notification/envelope mechanics.
- Keep the hub ticket downstream: it should consume this API after a dependency bump, not edit `botster-core` directly.

Non-scope:

- No Project Pipelines terms such as questions, findings, gates, reviews, post-message workflow semantics, or MCP policy in core.
- No hub auth, spawn-target admission, cloud/Rails/WebRTC policy, marketplace policy, or retention policy.
- No UI notification badges, browser/TUI rendering, operator workbench behavior, or plugin README updates for this core ticket.
- No persistence guarantee for notification inbox state unless the current core primitive already provides it. If state remains in-memory, document that explicitly.
- No merging of guarded-write delivery states with notification inbox states. Guarded PTY writes stay separate from notification queue/cursor/ack semantics.
- No broad refactor of `NotificationInbox`, `RoutedEnvelopeRouter`, `DefaultBotsterEngine`, or session daemon lifecycle unless a small public accessor is necessary to delegate correctly.

## Botster Layers Touched

- Core daemon Rust crate: primary change.
- Core notification and routed-envelope primitives: consumed through existing public APIs; only narrow additions if an accessor is missing.
- Core test support: optional, only if a small conformance helper lets hub prove it routes through the daemon API.
- Docs: daemon/API docs only.
- Hub, Lua plugin runtime, MCP plugin policy, TUI, React SPA, Rails relay: not changed.

## Worktree And Target Assumptions

- The active run target is `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- This plan is authored for the assigned pipeline worktree for run `run_1780680559_853113`.
- Plan artifacts cite vault notes by title or wikilink and do not copy local vault paths.

## State Ownership Decision

`CoreDaemon` has two session-engine backends:

- `DaemonEngine::Local(DefaultBotsterEngine)`
- `DaemonEngine::Worker(WorkerBackedBotsterEngine)`

Only `DefaultBotsterEngine` currently exposes notification post/drain methods. `WorkerBackedBotsterEngine` is the restart/adoption path for PTY sessions, but it does not expose notification APIs. Delegating daemon notifications to the engine facade would therefore either become Local-only or force a broader worker-engine extension.

Plan choice: `CoreDaemon` should own a single daemon-level `NotificationInbox` and `RoutedEnvelopeRouter` field directly. The daemon methods should call those existing core primitives, not the session engine variants.

Boundary consequence:

- Notification/envelope API behavior is the same for Local and Worker-backed daemons because it is daemon-owned, not session-engine-owned.
- No narrow `BotsterEngine`/`MultiplexerEngine` notification acknowledge/status accessor is required for this ticket; `CoreDaemon` can call `NotificationInbox::acknowledge` and `NotificationInbox::status` directly.
- The existing `DefaultBotsterEngine` notification methods remain valid for non-daemon embedders, but hub-native coordination must use the sanctioned `CoreDaemon` path.
- Notification/envelope queues are in-memory daemon process state. Worker-backed PTY sessions may survive daemon restart, but daemon-owned notification/envelope queues do not unless a future persistence ticket adds storage.
- This avoids a Local-only API and avoids extending `WorkerBackedBotsterEngine` with non-PTY coordination state.

## Affected Surfaces And Files

Expected:

- `crates/botster-core-daemon/src/api.rs`
  - Add daemon-level request/result shapes such as `PostNotificationRequest`, `DrainNotificationsRequest`, `NotificationDrainResult`, `AcknowledgeNotificationRequest`, `NotificationStatusResult`, and, if needed, `DrainEnvelopesRequest`/`AcknowledgeEnvelopeRequest` wrappers over the routed-envelope contract.
- `crates/botster-core-daemon/src/daemon.rs`
  - Add `CoreDaemon` fields for `NotificationInbox` and `RoutedEnvelopeRouter`.
  - Add methods that call those existing core primitives directly.
  - Ensure `ensure_running()` gates mutating notification/envelope operations consistently with existing daemon commands.
- `crates/botster-core-daemon/src/lib.rs`
  - Re-export new public daemon API types.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Add tests that call `CoreDaemon` methods directly and assert post -> drain/receive -> ack/status/cursor behavior.
- `docs/architecture/core-daemon.md`
  - Document the notification/envelope API, state guarantee, and policy boundary.

Possible:

- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Add a tiny host-facing helper only if the implementer needs a reusable downstream conformance fixture.

Avoid:

- New crates or dependencies.
- `crates/botster-core/src/engine/botster.rs` or `multiplexer.rs`: no notification ack/status accessor is needed because the daemon owns its inbox directly.
- Product-specific docs in core.
- Changes to hub/plugin/SPA/Rails/TUI files.

## Assumptions And Unknowns

Assumptions:

- The ticket wants `CoreDaemon` to expose existing core mechanics, not create a new daemon-specific notification model.
- Notification-specific post/drain/status should use `NotificationItem`, `NotificationTarget`, `NotificationTimestamp`, and `NotificationDeliveryStatus`.
- Cursor, bounded queue, slow-consumer, and per-target ack behavior should use the routed-envelope primitive when `NotificationInbox` does not provide those semantics.
- `CoreDaemon` owns notification/envelope state directly so Local and Worker-backed daemons share the same API behavior.
- Restart durability for notification/envelope state is in-memory daemon-process state. The docs and tests should explicitly say it is not restart durable today, even when Worker-backed PTY sessions are restart-adoptable.
- Tests can use synthetic ids and generic labels to avoid PII.

Unknowns for implementer to resolve:

- Whether the public daemon API should expose routed envelopes directly beside notification endpoints, or hide routed envelope usage behind notification-specific request/result names. Prefer explicit routed-envelope daemon methods if cursor semantics are truly generic.
- Whether downstream hub conformance needs a helper in `botster-core-test-support` now, or whether daemon integration tests are enough for this ticket.

No human question is required before implementation: the ticket clearly separates core-side daemon API work from downstream hub/MCP wiring.

## Risks

- The daemon-owned inbox means `DefaultBotsterEngine` and `CoreDaemon` have separate notification state. This is acceptable only because hub-native coordination uses `CoreDaemon`; docs should not imply daemon calls share state with a separately embedded `DefaultBotsterEngine`.
- Treating notification endpoints as Project Pipelines messaging would violate the core policy-free boundary.
- Faking cursor behavior on `NotificationInbox` would blur the difference between notification-specific delivery status and routed-envelope cursor semantics.
- Claiming restart durability for daemon-owned notification/envelope state would overstate the contract. Worker-backed PTY restart/adoption does not persist notification queues.
- Merging guarded-write states with notification ack/delivery states would regress the documented PTY readiness boundary.
- Broad refactors of the core engine command surface could introduce unrelated regressions.

## Acceptance Checks And Tests

Implementation should run:

- `cargo fmt`
- `cargo test -p botster-core notification_inbox`
- `cargo test -p botster-core routed_envelope`
- `cargo test -p botster-core-daemon --test daemon_integration_test notification`
- If a test-support helper is added: `cargo test -p botster-core-test-support`

Required test coverage:

1. `daemon_posts_drains_and_acknowledges_notifications`
   - Create `CoreDaemon`, post a synthetic `NotificationItem`, drain by typed `NotificationTarget`, acknowledge by `NotificationId`, and assert the resulting status is acknowledged.

2. `daemon_notification_drain_is_target_scoped_and_once_only`
   - Post session and client notifications, drain each target independently, and assert a second drain for the same target is empty unless the chosen contract intentionally supports cursor replay.

3. `daemon_notification_expiry_matches_core_inbox`
   - Post live and expired notifications with deterministic timestamps, drain at a fixed time, and assert expired items are not delivered.

4. `daemon_routed_envelope_cursor_ack_and_backpressure_are_exposed_when_needed`
   - Use a small routed-envelope queue capacity, publish enough envelopes to hit pressure, drain with cursor/limit, then acknowledge one delivered envelope and assert the delivery state.

5. `daemon_notifications_work_for_worker_backed_daemon_without_worker_engine_notification_methods`
   - Construct a `CoreDaemon` with `CoreDaemonConfig::with_worker_path(...)`, post/drain/ack a notification, and assert behavior matches Local mode without spawning a PTY. This proves notification state is daemon-owned and not dependent on `WorkerBackedBotsterEngine`.

6. `daemon_notification_state_is_not_restart_durable_today`
   - Post a notification, create a fresh `CoreDaemon` over the same `data_dir`, and assert the notification is not drained from the new daemon. Docs must state this current guarantee: worker-backed sessions can be restart-adopted, but daemon-owned notification/envelope queues are in-memory.

Runtime path evidence:

- Tests must call `botster_core_daemon::CoreDaemon` methods, not only instantiate `NotificationInbox` or `RoutedEnvelopeRouter` directly.
- Tests must cover both default Local construction and Worker-backed construction for notification methods, because the API is intentionally daemon-owned across both backends.
- Docs must identify hub/plugin meaning and delivery policy as downstream.

## Pipeline Gates And Artifacts

- Plan artifact: this file.
- Gate evidence should summarize the same context, scope, assumptions, affected surfaces, risks, tests, and vault gaps.
- Checklist evidence should record notes loaded, no convention conflicts, verification expectations, and capture decision. If checklist persistence is unavailable, preserve that evidence in gate submission.
- Next step: `botster_plan_review`.

## Vault Gaps Worth Capturing

No durable vault capture is required before implementation. Existing notes already cover the relevant boundaries.

Capture later only if implementation discovers a reusable rule about one of these:

- How daemon notification inbox state should relate to `BotsterEngine` notification state.
- When notification-specific delivery status should wrap routed-envelope cursor/ack mechanics.
- The exact restart/durability guarantee for core daemon notification/envelope state.

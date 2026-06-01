# Botster Core Engine Command Surface

`BotsterEngine` is the canonical policy-free command facade for embedders, tests, hub adapters, and future plugin/provider layers. `DefaultBotsterEngine` is the local PTY-backed instance of that facade. `MultiplexerEngine` stays the lower-level assembled primitive that the facades delegate to.

Core commands are mechanisms, not product actions. Hosts provide explicit ids, commands, working directories, environment, timestamps, subscription ids, and request ids. Hosts also own executors, queues, transport delivery, persistence, config discovery, auth, cloud/WebRTC/signaling, marketplace/update policy, CLI UX, Rails relay behavior, TUI/browser rendering, provider policy, and Project Pipelines workflow policy.

## Commands

| Command | Request shape | Result/event shape | Core entry point | Policy owner |
| --- | --- | --- | --- | --- |
| Spawn local session | `SessionSpawnRequest` plus `CoreSessionMetadata` | `BotsterSpawnOutcome` | `BotsterEngine::spawn_session`, `DefaultBotsterEngine::spawn_session` | Host resolves executable, cwd, env, target, admission |
| Attach client | `ClientId`, `SessionId`, `SubscriptionId`, caller clock | `BotsterEngineOutput` with subscription/session requests | `BotsterEngine::attach_client`, `DefaultBotsterEngine::attach_client` | Host owns client identity and delivery |
| Detach client | `ClientId`, `SessionId`, `SubscriptionId`, caller clock | `BotsterEngineOutput` | `BotsterEngine::detach_client`, `DefaultBotsterEngine::detach_client` | Host owns reconnect and cleanup policy |
| Send input | typed terminal bytes plus `ClientId` and `SessionId` | `SessionIoRequest::PtyInput` in `BotsterEngineOutput` | `BotsterEngine::write_bytes`, `DefaultBotsterEngine::write_bytes` | Client/host decides what bytes to send |
| Resize | rows and columns plus `ClientId` and `SessionId` | `SessionIoRequest::Resize` in `BotsterEngineOutput` | `BotsterEngine::resize`, `DefaultBotsterEngine::resize` | Client renderer owns measuring terminal size |
| List sessions | none | `Vec<CoreSession>` | `BotsterEngine::list_sessions`, `DefaultBotsterEngine::list_sessions` | Host owns filtering, retention, display ordering |
| Inspect session | `SessionId`, caller clock, activity threshold | `EngineSessionInspection` | `BotsterEngine::inspect_session`, `DefaultBotsterEngine::inspect_session` | Host owns status projection and labels |
| Read screen | `RequestId`, `SessionId`, caller clock | `SessionIoEvent::ScreenReady` in `BotsterEngineOutput` | `BotsterEngine::read_screen`, `DefaultBotsterEngine::read_screen` | Host owns presentation and polling cadence |
| Capture snapshot | `RequestId`, `SessionId`, caller clock | `SessionIoEvent::SnapshotReady` in `BotsterEngineOutput` | `BotsterEngine::capture_snapshot`, `DefaultBotsterEngine::capture_snapshot` | Host owns storage, retention, delivery |
| Replay snapshot | `PreparedSnapshotRequest`, caller clock | `SessionIoEvent::PreparedSnapshotReady` in `BotsterEngineOutput` when adapter supports it | `BotsterEngine::replay_snapshot`, `DefaultBotsterEngine::replay_snapshot` | Host owns recovery intent and persistence |
| Shutdown | `SessionId`, reason, caller clock | `BotsterEngineOutput` with lifecycle observations | `BotsterEngine::shutdown_session`, `DefaultBotsterEngine::shutdown_session` | Host owns reason text and shutdown policy |
| Notifications | `NotificationItem`, `NotificationTarget`, caller clock | `NotificationId` and `Vec<NotificationItem>` | `BotsterEngine::post_notification`, `BotsterEngine::drain_notifications` | Host/plugin owns presentation and delivery policy |

The compile-checked command vocabulary lives in `botster_core::engine_command` and is re-exported at the crate root as `EngineCommandKind`, `ENGINE_COMMAND_KINDS`, `EngineSessionInspection`, and type aliases for the existing request/result/event shapes.

## Error Model

The command facade reuses the existing typed errors:

- `BotsterEngineError` for adapter-backed `BotsterEngine` calls.
- `DefaultBotsterEngineError` for local PTY-backed `DefaultBotsterEngine` calls.
- `ManagedSessionRuntimeError::UnsupportedSessionRequest` when the default managed runtime cannot support a lower-level session request.
- `MultiplexerEngineError::UnknownSession`, `SessionAlreadyExists`, `MetadataTooLarge`, and wrapped runtime errors for core state and adapter failures.

Unsupported work is explicit. Core should return typed unsupported errors or omit commands that are not implemented by the runtime adapter. It should not fake snapshot, replay, send-file, mode-flag, or terminal-state fidelity.

## Sync And Async

The core surface is synchronous and deterministic. A method call mutates in-memory core state and returns typed outcomes that the caller delivers to clients, session workers, or runtime adapters.

Hosts may wrap these calls in actors, async tasks, queues, retry loops, transports, or persistent stores. Those scheduling choices are outside `botster-core`.

## Explicit Exclusions

The core command surface intentionally excludes product CLI UX, config discovery, auth, cloud/WebRTC/signaling, marketplace/update policy, Rails relay behavior, TUI rendering, restty/browser rendering, hub policy, provider policy, Project Pipelines behavior, command discovery, default-shell selection, historical browsing, reconnect policy, notification presentation, and plugin workflow policy.

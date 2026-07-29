# Botster Core Engine Command Surface

`BotsterEngine` is the canonical policy-free command facade for embedders, tests, hub adapters, and future plugin/provider layers. `DefaultBotsterEngine` is the local PTY-backed instance of that facade (use `DefaultBotsterEngine::worker_backed` when sessions should be owned by `botster-session-worker` processes).

**Start here for hosts:** spawn → attach → drain → input → shutdown via `botster_core::prelude` (see the workspace README and rustdoc on that module). Prefer `DefaultBotsterEngine` for the library path and `botster_core_daemon::CoreDaemon` (with `with_worker_path`) for the production durable path. `MultiplexerEngine` and raw `session_protocol` framing are advanced/internal surfaces that the facades already compose.

Core commands are mechanisms, not product actions. Hosts provide explicit ids, commands, working directories, environment, timestamps, subscription ids, and request ids. Hosts also own executors, queues, transport delivery, persistence, config discovery, auth, cloud/WebRTC/signaling, marketplace/update policy, CLI UX, Rails relay behavior, TUI/browser rendering, provider policy, and Project Pipelines workflow policy.

UI payloads and package surface/navigation contracts live in the Hub-owned
`botster-ui-contract`; Core retains only generic plugin-worker routing kinds
such as `UiAction` and `SurfaceRoute`, whose request/response payloads are
opaque to Core.

Related: [`core-daemon.md`](core-daemon.md), [`durable-session-worker-protocol.md`](durable-session-worker-protocol.md).

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
| Load plugin | `PluginWorkerRegistration` | `EngineCommandOutcome::PluginLoaded(PluginKey)` | `BotsterEngine::load_plugin` | Trusted host resolves/admitted package; core owns worker mechanics |
| Reload plugin | `PluginReloadSpec` plus replacement `PluginWorkerRegistration` | `PluginCleanupResult` | `BotsterEngine::reload_plugin` | Trusted host owns reload policy; core scopes descriptor/resource cleanup |
| Unload plugin | `PluginUnloadSpec` | `PluginCleanupResult` | `BotsterEngine::unload_plugin` | Trusted host owns unload policy; core removes worker-owned state |
| Invoke plugin | `PluginInvocationRequest` | `PluginInvocationOutcome` with typed worker events | `BotsterEngine::invoke_plugin` | Trusted host chooses handler/payload; core enforces capability checks, timeout, and queue pressure |

The compile-checked command API lives in `botster_core::engine_command`, is re-exported at the crate root for compatibility, and is included in `botster_core::prelude`:

- `EngineCommand<W>` is the typed request enum for `BotsterEngine<R, W>`. Its spawn variant carries the host-supplied worker runtime because custom embedders own worker construction. Its notification variants are thin inbox delegates over `BotsterEngine::post_notification` and `BotsterEngine::drain_notifications`. Its plugin lifecycle variants delegate to the existing plugin-worker facade methods rather than adding a second runtime path.
- `DefaultEngineCommand` is the typed request enum for `DefaultBotsterEngine` when the `local-runtime` feature is enabled. It covers the local PTY-backed session/client/screen/snapshot/shutdown commands, but intentionally omits notifications and plugin lifecycle because `DefaultBotsterEngine` does not currently expose those methods.
- `EngineCommandOutcome` is the heterogeneous typed result enum. It preserves the existing rich result types: `BotsterSpawnOutcome`, `BotsterEngineOutput`, `Vec<CoreSession>`, `EngineSessionInspection`, `NotificationId`, drained `Vec<NotificationItem>`, `PluginCleanupResult`, and `PluginInvocationOutcome`.
- `EngineCommandError<E>` wraps the underlying typed facade error with the `EngineCommandKind` that failed.
- `EngineCommandKind` and `ENGINE_COMMAND_KINDS` remain the vocabulary/drift guard for the command surface.

Both facades expose `execute_command(...)` as the typed dispatch entry point. The implementation is intentionally a thin exhaustive match that delegates to the public facade methods listed above; it is not a second engine router and does not mutate lower-level state directly.

## Error Model

The command facade reuses the existing typed errors through `EngineCommandError<E>`:

- `BotsterEngineError` for adapter-backed `BotsterEngine` calls.
- `DefaultBotsterEngineError` for local PTY-backed `DefaultBotsterEngine` calls.
- `ManagedSessionRuntimeError::UnsupportedSessionRequest` when the default managed runtime cannot support a lower-level session request.
- `MultiplexerEngineError::UnknownSession`, `SessionAlreadyExists`, `MetadataTooLarge`, and wrapped runtime errors for core state and adapter failures.

Unsupported work is explicit. Core should return typed unsupported errors or omit commands that are not implemented by the runtime adapter. It should not fake snapshot, replay, send-file, mode-flag, or terminal-state fidelity.

## Sync And Async

The core surface is synchronous and deterministic. A method call mutates in-memory core state and returns typed outcomes that the caller delivers to clients, session workers, or runtime adapters.

Hosts may wrap these calls in actors, async tasks, queues, retry loops, transports, or persistent stores. Those scheduling choices are outside `botster-core`.

### Host event-loop expectations

Embedders own the loop:

1. Supply host clocks (`now_seconds`) on attach, drain, input, inspect, and shutdown.
2. Call `drain_runtime_once` / `drain_runtime_all_once` (or `CoreDaemon::drain`) regularly while sessions are live so output and lifecycle observations are delivered.
3. Route returned client egress to transports; do not re-dispatch already-routed session requests as new engine work.
4. Honor backpressure summaries and report slow-client lag through the public report helpers when the host observes delivery lag.
5. Drain notification/envelope APIs separately when using those coordination planes (daemon-owned queues are process memory, not registry-durable).

## Explicit Exclusions

The core command surface intentionally excludes product CLI UX, config discovery, auth, cloud/WebRTC/signaling, marketplace/update policy, git/network install, Rails relay behavior, TUI rendering, restty/browser rendering, hub policy, provider policy, Project Pipelines behavior, command discovery, default-shell selection, historical browsing, reconnect policy, notification presentation, and plugin workflow policy.

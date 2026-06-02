# Non-Blocking Plugin Capability Runtime

Botster core defines plugin capability I/O as a bounded runtime mailbox contract, not as inline HTTP, WebSocket, file, store, watch, or timer execution. Plugin code and first-party host profiles submit typed requests, receive typed handles, and drain typed events through host-provided runtimes. PTY, session, client, and plugin-worker hot paths must not perform blocking capability I/O inline.

This document describes the scaffold in `botster_core::runtime::capability` plus the first concrete runtime primitive: `HttpCapabilityRuntime`. HTTP now has a bounded, non-blocking runtime around a host-implemented transport boundary. WebSocket, filesystem, plugin-store, watcher, timer scheduler, and Lua integration remain scaffold-only.

The HTTP deliverable is intentionally a core runtime primitive exercised by tests. There is no Lua `http.request` path, no `PluginWorkerEngine` ownership of the runtime, and no hub/profile default policy wiring in this ticket.

## Boundary

The existing `PluginWorkerEngine` remains the plugin execution boundary. Plugin handlers run through stable `PluginHandlerRef` values, per-plugin capacity, timeout attribution, cooperative cancellation, and unload/reload cleanup. Capability I/O composes with that boundary:

- Plugin code requests capability I/O by submitting a `CapabilityRuntimeRequest`.
- The host-owned `PluginCapabilityRuntime` enqueues the request and returns a `CapabilityRuntimeHandle` or a typed `CapabilityRuntimeError`.
- Long-running I/O continues outside the worker invocation path.
- Completion, inbound WebSocket messages, file-watch events, timer firings, failures, cancellations, and pressure observations return as `CapabilityRuntimeEvent` values.
- Runtime resources are tracked as `PluginResourceRef` values so plugin unload/reload can release only the owning plugin's resources.

The queue source for pressure reports is `QueueSource::PluginWorker`. The capability runtime is part of the plugin-worker boundary rather than a separate public actor family in this scaffold.

## Operation Families

The scaffold names these operation families:

- `HttpCapabilityRequest`: outbound HTTP request metadata accepted by `HttpCapabilityRuntime`.
- `WebSocketCapabilityRequest`: connect, send, and close operations.
- `WatchCapabilityRequest`: scoped file-watch register and unregister operations.
- `FilesystemCapabilityRequest`: scoped read, write, list, stat, and remove operations.
- `PluginStoreCapabilityRequest`: plugin-scoped JSON get, set, delete, and list operations.
- `TimerCapabilityRequest`: one-shot, interval, and cancel operations.

Each request carries:

- `PluginKey` ownership.
- A stable `CapabilityOperationId`.
- A timeout budget in milliseconds.
- An optional `PluginHandlerRef` for callbacks.
- An explicit required `Capability`.

HTTP maps to `CapabilitySurface::Network` with scope `http`. WebSocket maps to `CapabilitySurface::Network` with scope `websocket`. File watch and scoped filesystem operations map to `CapabilitySurface::Filesystem` with the host-owned scope id. Plugin-store operations map to `CapabilitySurface::PluginDb` with the store namespace. Timers map to `CapabilitySurface::Timers` with scope `callbacks`.

Core names the capability requirement. Hub/profile policy decides which packages receive which grants, allowed origins, filesystem scopes, namespaces, credentials, quotas, and backend implementations.

## Scoped Filesystem

Core does not trust or resolve absolute filesystem paths. Filesystem and watch requests carry:

- An opaque host-owned `scope_id`.
- A `ScopedRelativePath` below that scope.

`ScopedRelativePath::is_scoped_relative()` verifies the contract-level shape: non-empty, not absolute, and no `..` traversal segments. Host profiles still own path resolution, symlink policy, allowed roots, and platform-specific safety rules.

## Handles And Resources

The runtime returns `CapabilityRuntimeHandle` after accepting a request into the bounded mailbox. Persistent or in-flight resources use `PluginResourceRef` with plugin ownership and a stable resource id.

Existing resource kinds are reused for already-named families:

- `PluginResourceKind::HttpRequest`
- `PluginResourceKind::Watch`
- `PluginResourceKind::Timer`

The scaffold adds only the missing cleanup families:

- `PluginResourceKind::NetworkConnection`
- `PluginResourceKind::FilesystemOperation`
- `PluginResourceKind::PluginStoreOperation`

These resource refs fit the existing `PluginCleanupResult` unload/reload cleanup path.

## Queue And Backpressure

The capability runtime must use bounded submit and callback/event queues. When it cannot accept more work, it reports `CapabilityRuntimeErrorKind::Backpressured` or emits `CapabilityRuntimeEvent::Backpressure(BackpressureSummary)`.

`HttpCapabilityRuntime::submit` validates capability grants, host/scheme policy, headers, body limits, timeout, and capacity without performing transport I/O inline. Accepted work runs on runtime-owned background worker threads through `HttpCapabilityTransport`; `drain_events` pulls completion, failure, timeout, cancellation, and pressure events.

Pressure reports must include:

- `QueueSource::PluginWorker`
- The configured capacity.
- Current depth.
- `BackpressureRoute.plugin_key`

The runtime must reject or report pressure before unbounded memory growth. It must not block PTY/session/client hot paths waiting for queue space.

## Timeout And Cancellation

Every request carries `timeout_ms`. Runtime implementations should treat timeout as the host-side budget for accepting or completing the operation according to the operation family.

Cancellation is explicit:

- `PluginCapabilityRuntime::cancel(plugin_key, operation_id)` requests cancellation for an operation.
- `PluginCapabilityRuntime::release_resource(resource)` releases one runtime resource.
- `PluginCapabilityRuntime::cleanup_plugin(plugin_key)` releases all resources owned by one plugin.

Timeouts, cancellations, invalid requests, runtime stops, unknown operations, and unknown resources are represented with `CapabilityRuntimeErrorKind` and mirrored in failure events.

The HTTP runtime reuses `PluginCancellationToken` for in-flight transport cancellation. `cancel`, timeout detection, and `cleanup_plugin` all trip the token. Host transports must poll it while blocking or collecting chunks; late transport completions after timeout, cancel, or cleanup are ignored.

HTTP response body limits are enforced by the host transport while collecting response chunks. Core supplies `HttpTransportRequest` limits and `HttpCapabilityRuntime::validate_response` so host transports and tests can apply the same bounded header/body checks before emitting a response.

## Events

`CapabilityRuntimeEvent` is the typed event surface for future hub/profile/plugin-runtime integration:

- `Completed`
- `ResourceOpened`
- `ResourceReleased`
- `WebSocketMessage`
- `Watch`
- `TimerFired`
- `TimedOut`
- `Cancelled`
- `Failed`
- `Backpressure`
- `CleanupCompleted`

Events carry plugin identity, operation ids, and resource refs where applicable. Plugin-owned JSON store values and WebSocket messages are payload data, while stable Botster control fields remain typed.

## Hub/Profile-Owned Policy

The host profile owns policy and backend selection:

- Admission rules.
- Capability grants and default grants.
- Network origin allowlists.
- Filesystem root resolution and symlink behavior.
- Plugin-store namespace backing.
- Credentials and secret lookup.
- Timer quotas.
- Queue capacities.
- Retry policy.
- Concrete async runtime, threads, tasks, and watchers.

Botster core owns reusable contract mechanics only: typed requests, capabilities, handles, events, errors, cleanup refs, and pressure metadata.

## Runtime Path Evidence

This ticket is scaffold-only by design at the production entry-point layer. There is no live Lua, hub, WebSocket, watch, filesystem, plugin-store, or timer entry point yet.

The verifiable changed runtime path is the public `botster_core` contract and HTTP runtime surface that future host profiles and plugin runtimes import. `plugin_capability_runtime_test` exercises accepted and denied HTTP submissions, typed validation errors, worker-thread non-blocking behavior, timeout and cancellation through `PluginCancellationToken`, response size limits, cleanup isolation, and typed transport failures. `PluginWorkerEngine` cleanup remains descriptor/resource-ledger cleanup only; real in-flight HTTP cleanup is owned by `HttpCapabilityRuntime::cleanup_plugin` until a future ticket wires the engine to own a runtime instance.

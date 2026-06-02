# Non-Blocking Plugin Capability Runtime

Botster core defines plugin capability I/O as a bounded runtime mailbox contract, not as inline HTTP, WebSocket, file, store, watch, or timer execution. Plugin code and first-party host profiles submit typed requests, receive typed handles, and drain typed events through host-provided runtimes. PTY, session, client, and plugin-worker hot paths must not perform blocking capability I/O inline.

This document describes the reusable contracts in
`botster_core::runtime::capability` plus concrete core runtime primitives for
HTTP and file watching. `HttpCapabilityRuntime` provides bounded, non-blocking
HTTP around a host-implemented transport boundary. `FileWatchRuntime` provides
capability-scoped watch registration, scoped-path validation,
debounce/coalescing, bounded delivery, and cleanup over a host-provided event
source. WebSocket, filesystem, plugin-store, timer scheduler, Lua integration,
and concrete OS watcher adapters remain host/profile-owned.

The HTTP and file-watch deliverables are intentionally core runtime primitives
exercised by tests. There is no Lua `http.request` or `watch.directory` path, no
`PluginWorkerEngine` ownership of these runtime instances, and no hub/profile
default policy wiring in these tickets.

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

## File Watch Runtime

The watch family now has a core-owned runtime over a host-provided
`FileWatchEventSource`. Core owns the policy-free mechanics that every host
adapter needs:

- watch registration state;
- same-plugin callback and capability-scope checks;
- `ScopedRelativePath` shape validation;
- deterministic debounce/coalescing over injected event timestamps;
- bounded per-plugin delivery with `QueueSource::PluginWorker` pressure;
- single-resource release through `WatchCapabilityRequest::Unregister` or
  `PluginCapabilityRuntime::release_resource`;
- all-watch cleanup for one plugin through `cleanup_plugin`.

The coalescing key is plugin, watch resource, scoped relative path, and overflow
family. Path-specific events for the same key use last-writer-wins semantics
inside the debounce window. Backend overflow is held on a separate overflow key
for the watch resource and is emitted as `WatchChangeKind::Overflow`, not
collapsed into a path-specific successful change.

## Hub/Profile-Owned Policy

The host profile owns policy and backend selection:

- Admission rules.
- Capability grants and default grants.
- Network origin allowlists.
- Filesystem root resolution and symlink behavior.
- Plugin-store namespace backing.
- Credentials and secret lookup.
- Timer quotas.
- Queue-capacity config values.
- Retry policy.
- Concrete async runtime, threads, tasks, and OS watcher adapters.
- Directory selection, filesystem root resolution, and symlink behavior for
  watched scopes.

Botster core owns reusable contract mechanics: typed requests, capabilities,
handles, events, errors, cleanup refs, pressure metadata, and the watch runtime
mechanism listed above.

## Runtime Path Evidence

These tickets are scaffold-only by design at the production entry-point layer.
There is no live Lua, hub, WebSocket, filesystem, plugin-store, timer, HTTP, or
watch entry point yet.

The verifiable changed runtime paths are the public `botster_core` contract
surface plus `HttpCapabilityRuntime<T: HttpCapabilityTransport>` and
`FileWatchRuntime<S: FileWatchEventSource>` implementing the existing
`PluginCapabilityRuntime` trait. `plugin_capability_runtime_test` exercises
accepted and denied HTTP submissions, typed validation errors, worker-thread
non-blocking behavior, timeout and cancellation through `PluginCancellationToken`,
response size limits, cleanup isolation, and typed transport failures.
`plugin_file_watch_runtime_test` instantiates the watch runtime, submits allowed
and denied watch requests, injects fake source events with deterministic
timestamps, drains coalesced watch events, and asserts source-side unregister
calls for single-resource and plugin cleanup paths. `PluginWorkerEngine` cleanup
remains descriptor/resource-ledger cleanup only; real in-flight HTTP and watch
cleanup is owned by the concrete capability runtime until a future ticket wires
the engine to own runtime instances.
